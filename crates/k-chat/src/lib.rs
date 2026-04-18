//! k-chat — AI Chat Bridge Plugin
//!
//! Provides an interactive AI chat session accessible from the GOS shell.
//! User messages are sent over COM2 (0x2F8) to a host-side bridge process
//! (`tools/chat-bridge.py`) which forwards them to a configured AI API.
//! Responses stream back over the same serial link and are rendered to VGA.
//!
//! ## Bridge Protocol (COM2, 115200 8N1)
//!
//! | Direction          | Frame              | Meaning                              |
//! |--------------------|--------------------|------------------------------------- |
//! | Kernel → Bridge    | `GCHAT:<msg>\n`    | Send user message                    |
//! | Bridge → Kernel    | `GRESP:<text>\n`   | One line of AI response text         |
//! | Bridge → Kernel    | `GTOOL:<t>:<a>\n`  | Request kernel tool execution        |
//! | Bridge → Kernel    | `GDONE:\n`         | End of AI turn                       |
//! | Kernel → Bridge    | `GRSLT:<res>\n`    | Tool execution result                |
//!
//! ## Supported Tools
//!
//! | Tool frame          | Action                                |
//! |---------------------|---------------------------------------|
//! | `GTOOL:ping:<ip>`   | Emit `NET_CONTROL_PING` to k-net      |
//! | `GTOOL:net:status`  | Emit `NET_CONTROL_REPORT` to k-net    |
//! | `GTOOL:clear`       | Clear the VGA canvas                  |
//!
// ============================================================
// GOS KERNEL TOPOLOGY — k-chat
//
// MERGE (p:Plugin {id: "K_CHAT", name: "k-chat"})
// SET p.executor = "k_chat::EXECUTOR_ID", p.node_type = "PluginEntry"
// SET p.state_schema = "0x2010"
//
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
// MERGE (dep_K_NET:Plugin {id: "K_NET"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_NET)
//
// MERGE (cap_chat_bridge:Capability {namespace: "chat", name: "bridge"})
// MERGE (p)-[:EXPORTS]->(cap_chat_bridge)
//
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
// MERGE (cap_net_uplink:Capability {namespace: "net", name: "uplink"})
// MERGE (p)-[:IMPORTS]->(cap_net_uplink)
// ============================================================

#![no_std]

mod pre;
mod proc;
mod post;

use core::cell::UnsafeCell;
use core::hint::spin_loop;

use gos_protocol::{
    derive_node_id, signal_to_packet,
    ExecStatus, ExecutorContext, ExecutorId,
    KernelAbi, NodeEvent, NodeExecutorVTable, PluginId,
    Signal, VectorAddress,
    NET_CONTROL_PING, NET_CONTROL_REPORT,
};

// ── Public plugin identity ─────────────────────────────────────────────────────

pub const NODE_VEC: VectorAddress = gos_protocol::vectors::SVC_CHAT;
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.chat");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init:     Some(chat_on_init),
    on_event:    Some(chat_on_event),
    on_suspend:  Some(chat_on_suspend),
    on_resume:   None,
    on_teardown: None,
    on_telemetry: None,
};

// ── Plugin constants ───────────────────────────────────────────────────────────

const CHAT_PLUGIN_ID: PluginId = PluginId::from_ascii("K_CHAT");
const CHAT_NODE_ID: gos_protocol::NodeId = derive_node_id(CHAT_PLUGIN_ID, "chat.bridge");

const VGA_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);

/// COM2 base I/O port.
const COM2: u16 = 0x2F8;
/// Bridge detection probe timeout in poll iterations.
const PROBE_TIMEOUT: usize = 5_000_000;
/// Per-line read timeout (≈ 5 s worth of polling).
const LINE_TIMEOUT: usize = 50_000_000;
/// Maximum message buffer (user input).
pub const MSG_BUF_SIZE: usize = 512;
/// Response accumulator size.
pub const RESP_BUF_SIZE: usize = 4096;
/// API key storage.
pub const API_KEY_BUF: usize = 256;

// ── State ──────────────────────────────────────────────────────────────────────

/// All runtime state for the chat plugin.
#[repr(C)]
pub struct ChatState {
    /// VGA console target vector (resolved at init).
    pub console_target: u64,
    /// k-net node vector (resolved at init, used for tool dispatch).
    pub net_target: u64,
    /// Whether COM2 probe succeeded.
    pub com2_ready: u8,
    /// Current user input buffer.
    pub input_buf: [u8; MSG_BUF_SIZE],
    pub input_len: usize,
    /// AI response staging buffer (filled by `collect_bridge_response`).
    pub resp_buf: [u8; RESP_BUF_SIZE],
    pub resp_len: usize,
    /// Stored API key (passed to bridge on each call if non-empty).
    pub api_key: [u8; API_KEY_BUF],
    pub api_key_len: u8,
}

/// DMA-safe static wrapper (same pattern as k-net).
struct ChatCell<T>(UnsafeCell<T>);
// SAFETY: uniprocessor cooperative kernel — no concurrent access.
unsafe impl<T> Sync for ChatCell<T> {}

static CHAT_STATE: ChatCell<ChatState> = ChatCell(UnsafeCell::new(ChatState {
    console_target: 0,
    net_target: 0,
    com2_ready: 0,
    input_buf: [0u8; MSG_BUF_SIZE],
    input_len: 0,
    resp_buf: [0u8; RESP_BUF_SIZE],
    resp_len: 0,
    api_key: [0u8; API_KEY_BUF],
    api_key_len: 0,
}));

// ── State accessors ────────────────────────────────────────────────────────────

pub(crate) unsafe fn state_mut(_ctx: *mut ExecutorContext) -> &'static mut ChatState {
    unsafe { &mut *CHAT_STATE.0.get() }
}

// ── Console helpers ────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(crate) struct ConsoleSink {
    target: u64,
    from:   u64,
    abi:    &'static KernelAbi,
}

pub(crate) fn sink_from_ctx(ctx: *mut ExecutorContext) -> ConsoleSink {
    let ctx_ref = unsafe { &*ctx };
    let abi     = unsafe { &*ctx_ref.abi };
    let state   = unsafe { &*CHAT_STATE.0.get() };
    ConsoleSink {
        target: if state.console_target != 0 { state.console_target } else { VGA_VEC.as_u64() },
        from:   ctx_ref.vector.as_u64(),
        abi,
    }
}

pub(crate) fn print_byte(sink: &ConsoleSink, byte: u8) {
    if let Some(emit) = sink.abi.emit_signal {
        let pkt = signal_to_packet(Signal::Data { from: sink.from, byte });
        unsafe { let _ = emit(sink.target, pkt); }
    }
}

pub(crate) fn print_str(sink: &ConsoleSink, s: &str) {
    for b in s.bytes() { print_byte(sink, b); }
}

pub(crate) fn print_bytes(sink: &ConsoleSink, bytes: &[u8]) {
    for &b in bytes { print_byte(sink, b); }
}

pub(crate) fn set_color(sink: &ConsoleSink, fg: u8, bg: u8) {
    if let Some(emit) = sink.abi.emit_signal {
        unsafe {
            let _ = emit(sink.target, signal_to_packet(Signal::Control { cmd: 1, val: fg }));
            let _ = emit(sink.target, signal_to_packet(Signal::Control { cmd: 2, val: bg }));
        }
    }
}

pub(crate) fn clear_canvas(sink: &ConsoleSink) {
    if let Some(emit) = sink.abi.emit_signal {
        unsafe { let _ = emit(sink.target, signal_to_packet(Signal::Control { cmd: 7, val: 0 })); }
    }
}

fn emit_to(abi: &KernelAbi, target: u64, signal: Signal) {
    if target == 0 { return; }
    if let Some(emit) = abi.emit_signal {
        unsafe { let _ = emit(target, signal_to_packet(signal)); }
    }
}

// ── COM2 low-level I/O ─────────────────────────────────────────────────────────

#[inline(always)]
unsafe fn out8(port: u16, val: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port, in("al") val,
            options(nostack, preserves_flags)
        );
    }
}

#[inline(always)]
unsafe fn in8(port: u16) -> u8 {
    let v: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") v, in("dx") port,
            options(nostack, preserves_flags)
        );
    }
    v
}

/// Initialise COM2 at 115200 8N1 with FIFOs enabled.
fn com2_init() {
    unsafe {
        out8(COM2 + 1, 0x00); // disable interrupts
        out8(COM2 + 3, 0x80); // enable DLAB
        out8(COM2 + 0, 0x01); // divisor LSB → 115200 baud
        out8(COM2 + 1, 0x00); // divisor MSB
        out8(COM2 + 3, 0x03); // 8-N-1
        out8(COM2 + 2, 0xC7); // enable + clear FIFOs, 14-byte threshold
        out8(COM2 + 4, 0x0B); // RTS + DTR + OUT2
    }
}

/// Returns `true` if a byte is waiting in the RX FIFO.
#[inline(always)]
fn com2_rx_ready() -> bool {
    unsafe { in8(COM2 + 5) & 0x01 != 0 }
}

/// Returns `true` if the TX holding register is empty (ready to send).
#[inline(always)]
fn com2_tx_ready() -> bool {
    unsafe { in8(COM2 + 5) & 0x20 != 0 }
}

/// Blocking single-byte write to COM2.
fn com2_write_byte(b: u8) {
    let mut spins = 0usize;
    while !com2_tx_ready() {
        spin_loop();
        spins += 1;
        if spins > 10_000_000 { return; } // TX stuck — give up
    }
    unsafe { out8(COM2, b); }
}

/// Write `prefix` + `msg` + `\n` to COM2.
/// Strips embedded newlines from `msg` to keep frames single-line.
pub(crate) fn com2_write_line(prefix: &[u8], msg: &[u8]) {
    for &b in prefix { com2_write_byte(b); }
    for &b in msg {
        if b != b'\n' && b != b'\r' { com2_write_byte(b); }
    }
    com2_write_byte(b'\n');
}

/// Read one byte with a per-iteration spin budget.
/// Returns `None` on timeout.
fn com2_read_byte_timed(budget: &mut usize) -> Option<u8> {
    loop {
        if com2_rx_ready() {
            return Some(unsafe { in8(COM2) });
        }
        if *budget == 0 { return None; }
        *budget -= 1;
        spin_loop();
    }
}

/// Read one complete `\n`-terminated line from COM2 into `dst`.
/// Returns the number of bytes written (0 on timeout before any data).
fn com2_read_line(dst: &mut [u8]) -> usize {
    let mut len = 0usize;
    let mut budget = LINE_TIMEOUT;
    loop {
        match com2_read_byte_timed(&mut budget) {
            Some(b'\n') => return len,
            Some(b'\r') => {}
            Some(b) => {
                if len < dst.len() { dst[len] = b; len += 1; }
            }
            None => return len,
        }
    }
}

/// Probe COM2 by sending a `GHELO:\n` and waiting for `GOKAY:\n`.
/// Returns `true` if the bridge acknowledges within `PROBE_TIMEOUT` polls.
fn com2_probe() -> bool {
    com2_write_line(b"GHELO:", b"gos-kernel");
    let mut line = [0u8; 64];
    let mut budget = PROBE_TIMEOUT;
    loop {
        if com2_rx_ready() {
            let len = com2_read_line(&mut line);
            if len >= 6 && &line[..6] == b"GOKAY:" {
                return true;
            }
            // Any other response: bridge is alive but speaking different version
            return false;
        }
        if budget == 0 { return false; }
        budget -= 1;
        spin_loop();
    }
}

// ── Bridge response collection ─────────────────────────────────────────────────

/// Process `GTOOL:<tool>:<arg>` frames inline, emitting kernel signals.
/// Sends a `GRSLT:<result>\n` back to the bridge after each tool.
fn execute_tool(state: &ChatState, abi: &KernelAbi, sink: &ConsoleSink, tool: &[u8]) {
    if tool.starts_with(b"ping:") {
        let ip = &tool[5..];
        set_color(sink, 13, 0); // magenta for tool label
        print_str(sink, "\n[TOOL] ping ");
        print_bytes(sink, ip);
        print_str(sink, " →");
        set_color(sink, 7, 0);
        emit_to(abi, state.net_target, Signal::Control { cmd: NET_CONTROL_PING, val: 0 });
        print_str(sink, " dispatched\n");
        com2_write_line(b"GRSLT:", b"ping dispatched to k-net; reply will appear in console");
    } else if tool == b"net:status" || tool == b"net" {
        set_color(sink, 13, 0);
        print_str(sink, "\n[TOOL] net status\n");
        set_color(sink, 7, 0);
        emit_to(abi, state.net_target, Signal::Control { cmd: NET_CONTROL_REPORT, val: 0 });
        com2_write_line(b"GRSLT:", b"net status dispatched to k-net");
    } else if tool == b"clear" {
        clear_canvas(sink);
        draw_chat_banner(sink);
        com2_write_line(b"GRSLT:", b"screen cleared");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "\n[TOOL] unknown: ");
        print_bytes(sink, tool);
        print_byte(sink, b'\n');
        set_color(sink, 7, 0);
        com2_write_line(b"GRSLT:", b"unknown tool");
    }
}

/// Read the full bridge turn (multiple `GRESP:` lines, optional `GTOOL:` frames,
/// terminated by `GDONE:`). Appends all response text to `state.resp_buf`.
pub(crate) fn collect_bridge_response(state: &mut ChatState) {
    // We need the ABI for tool dispatch — obtain from CHAT_STATE indirectly via
    // a temporary ConsoleSink. Since this is called from proc, we build a
    // minimal sink here using the stored console_target.
    //
    // Tool signals are emitted via the KernelAbi stored in a thread-local
    // during on_event execution. We stash it at the start of on_event.
    let abi = match abi_cache_load() {
        Some(abi) => unsafe { &*abi },
        None => {
            // No ABI available — collect text-only, skip tools
            collect_text_only(state);
            return;
        }
    };

    let console_target = state.console_target;
    let net_target     = state.net_target;
    let sink = ConsoleSink {
        target: if console_target != 0 { console_target } else { VGA_VEC.as_u64() },
        from:   NODE_VEC.as_u64(),
        abi,
    };

    let mut line = [0u8; 512];

    loop {
        let len = com2_read_line(&mut line);
        if len == 0 {
            // Timeout — treat as end of turn
            break;
        }
        let frame = &line[..len];

        if frame.starts_with(b"GRESP:") {
            let text = &frame[6..];
            // Append to resp_buf, inserting a newline separator after each line
            let remaining = RESP_BUF_SIZE - state.resp_len;
            let to_copy   = text.len().min(remaining.saturating_sub(1));
            state.resp_buf[state.resp_len..state.resp_len + to_copy]
                .copy_from_slice(&text[..to_copy]);
            state.resp_len += to_copy;
            if state.resp_len < RESP_BUF_SIZE {
                state.resp_buf[state.resp_len] = b'\n';
                state.resp_len += 1;
            }
        } else if frame.starts_with(b"GTOOL:") {
            let tool_frame = &frame[6..];
            // Tool lines are rendered and acknowledged inline; NOT appended to resp_buf
            let state_ref = unsafe { &*CHAT_STATE.0.get() };
            execute_tool(state_ref, abi, &sink, tool_frame);
            // After tool execution k-net was signalled — continue reading
            let _ = net_target; // used in execute_tool via state_ref
        } else if frame.starts_with(b"GDONE:") {
            break;
        }
        // Unknown frame prefix — silently ignore and keep reading
    }

    // Trim trailing newline from resp_buf
    while state.resp_len > 0 && state.resp_buf[state.resp_len - 1] == b'\n' {
        state.resp_len -= 1;
    }
}

fn collect_text_only(state: &mut ChatState) {
    let mut line = [0u8; 512];
    loop {
        let len = com2_read_line(&mut line);
        if len == 0 { break; }
        let frame = &line[..len];
        if frame.starts_with(b"GRESP:") {
            let text = &frame[6..];
            let remaining = RESP_BUF_SIZE - state.resp_len;
            let to_copy = text.len().min(remaining.saturating_sub(1));
            state.resp_buf[state.resp_len..state.resp_len + to_copy]
                .copy_from_slice(&text[..to_copy]);
            state.resp_len += to_copy;
            if state.resp_len < RESP_BUF_SIZE {
                state.resp_buf[state.resp_len] = b'\n';
                state.resp_len += 1;
            }
        } else if frame.starts_with(b"GDONE:") {
            break;
        }
    }
    while state.resp_len > 0 && state.resp_buf[state.resp_len - 1] == b'\n' {
        state.resp_len -= 1;
    }
}

// ── ABI cache (stored during on_event so COM2 tool dispatch can reach it) ──────

/// Stashed pointer to the KernelAbi for the duration of an on_event call.
/// SAFETY: Only written at the start of on_event, only read within the same
/// synchronous call. Uniprocessor cooperative kernel — no concurrent access.
/// Wrapped in ChatCell to satisfy the graph-governance `static mut` ban.
struct AbiCacheCell(UnsafeCell<Option<*const KernelAbi>>);
unsafe impl Sync for AbiCacheCell {}

static ABI_CACHE: AbiCacheCell = AbiCacheCell(UnsafeCell::new(None));

#[inline(always)]
fn abi_cache_store(val: Option<*const KernelAbi>) {
    unsafe { *ABI_CACHE.0.get() = val; }
}

#[inline(always)]
fn abi_cache_load() -> Option<*const KernelAbi> {
    unsafe { *ABI_CACHE.0.get() }
}

// ── Chat UI helpers ────────────────────────────────────────────────────────────

/// Draw the chat mode banner (top of screen).
pub(crate) fn draw_chat_banner(sink: &ConsoleSink) {
    set_color(sink, 0, 11);  // black on cyan
    print_str(sink, "  GOS CHAT — AI Bridge                                                          ");
    set_color(sink, 8, 0);
    print_str(sink, "  Type your message and press Enter. 'exit' returns to shell.                   \n");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

// ── Node lifecycle callbacks ───────────────────────────────────────────────────

unsafe extern "C" fn chat_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let ctx_ref = unsafe { &*ctx };
    let abi     = unsafe { &*ctx_ref.abi };

    // Resolve console (VGA) capability
    let console_target = if let Some(resolve) = abi.resolve_capability {
        unsafe { resolve(b"console".as_ptr(), 7, b"write".as_ptr(), 5) }
    } else {
        VGA_VEC.as_u64()
    };

    // Resolve net uplink capability
    let net_target = if let Some(resolve) = abi.resolve_capability {
        unsafe { resolve(b"net".as_ptr(), 3, b"uplink".as_ptr(), 6) }
    } else {
        0
    };

    // Initialise COM2 UART
    com2_init();

    // Probe the bridge
    let com2_ready = if com2_probe() { 1u8 } else { 0u8 };

    // Populate state
    let state = unsafe { &mut *CHAT_STATE.0.get() };
    state.console_target = console_target;
    state.net_target     = net_target;
    state.com2_ready     = com2_ready;

    ExecStatus::Done
}

unsafe extern "C" fn chat_on_event(
    ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    // Cache the ABI pointer for the duration of this call
    let abi_ptr = unsafe { (*ctx).abi };
    abi_cache_store(Some(abi_ptr));

    let result = (|| {
        let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
        let Some(output) = (unsafe { proc::process(ctx, input) }) else { return ExecStatus::Done; };
        unsafe { post::emit(ctx, output) }
    })();

    abi_cache_store(None);
    result
}

unsafe extern "C" fn chat_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

// ── Graph metadata (for `show` command in k-shell) ────────────────────────────

pub const CHAT_PLUGIN_ID_PUB: PluginId = CHAT_PLUGIN_ID;
pub const CHAT_NODE_ID_PUB: gos_protocol::NodeId = CHAT_NODE_ID;
