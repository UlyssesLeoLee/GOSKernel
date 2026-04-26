//! k-nim — NVIDIA NIM / OpenAI-Compatible Inference Plugin
//!
//! Provides an interactive multi-turn AI chat session accessible from the GOS
//! shell. Inference requests are dispatched directly over TCP (via k-net's
//! e1000 driver) to a NIM-compatible endpoint running OpenAI's
//! `/v1/chat/completions` API (e.g., `nvcr.io/nim/meta/llama-3.1-8b-instruct`).
//!
//! No Python bridge or COM2 serial link is required — all I/O is bare-metal TCP.
//!
//! ## Default endpoint
//!
//! | Parameter | Default                     |
//! |-----------|------------------------------|
//! | IP        | `10.0.2.2` (QEMU SLIRP host) |
//! | Port      | `8000`                       |
//! | Model     | `meta/llama-3.1-8b-instruct` |
//!
//! Start the NIM container on the host:
//! ```
//! docker run --gpus all -p 8000:8000 \
//!   nvcr.io/nim/meta/llama-3.1-8b-instruct:latest
//! ```
//!
//! ## Multi-turn history
//!
//! Each completed exchange is appended to `state.history_buf` as a JSON
//! fragment: `{"role":"user","content":"..."},{"role":"assistant","content":"..."},`
//! (with trailing comma). On the next request this fragment is inserted directly
//! between the system message and the new user turn inside the `messages` array.
//! History is automatically cleared when it would overflow, or on demand via the
//! `NIM_CONTROL_CLEAR_HISTORY` signal.

// ============================================================
// GOS KERNEL TOPOLOGY — k-nim
//
// MERGE (p:Plugin {id: "K_NIM", name: "k-nim"})
// SET p.executor = "k_nim::EXECUTOR_ID", p.node_type = "PluginEntry"
// SET p.state_schema = "0x2011"
//
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
// MERGE (dep_K_NET:Plugin {id: "K_NET"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_NET)
//
// MERGE (cap_nim_infer:Capability {namespace: "nim", name: "inference"})
// MERGE (p)-[:EXPORTS]->(cap_nim_infer)
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

use gos_protocol::{
    derive_node_id, signal_to_packet,
    ExecStatus, ExecutorContext, ExecutorId,
    KernelAbi, NodeEvent, NodeExecutorVTable, PluginId,
    Signal, VectorAddress,
};

// ── Public plugin identity ─────────────────────────────────────────────────────

pub const NODE_VEC: VectorAddress = gos_protocol::vectors::SVC_NIM;
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.nim");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init:     Some(nim_on_init),
    on_event:    Some(nim_on_event),
    on_suspend:  Some(nim_on_suspend),
    on_resume:   None,
    on_teardown: None,
    on_telemetry: None,
};

// ── Plugin constants ───────────────────────────────────────────────────────────

const NIM_PLUGIN_ID: PluginId = PluginId::from_ascii("K_NIM");
const NIM_NODE_ID: gos_protocol::NodeId = derive_node_id(NIM_PLUGIN_ID, "nim.inference");

const VGA_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);

/// Maximum user input buffer.
pub const INPUT_BUF_SIZE: usize = 512;
/// HTTP response staging buffer.
pub const RESP_BUF_SIZE: usize = 4096;
/// Conversation history buffer (JSON fragment, comma-terminated entries).
pub const HISTORY_BUF_SIZE: usize = 2048;
/// HTTP request scratch buffer: headers (~300) + JSON body (~1700).
const HTTP_REQ_BUF_SIZE: usize = 4096;
/// Raw TCP response buffer (headers + JSON body).
const HTTP_RAW_BUF_SIZE: usize = 8192;

// ── State ──────────────────────────────────────────────────────────────────────

/// All runtime state for the NIM plugin.
#[repr(C)]
pub struct NimState {
    /// VGA console target vector (resolved at init).
    pub console_target: u64,
    /// k-net node vector (resolved at init).
    pub net_target: u64,
    /// Current user input buffer.
    pub input_buf: [u8; INPUT_BUF_SIZE],
    pub input_len: usize,
    /// NIM response staging buffer (filled by `nim_do_request`).
    pub resp_buf: [u8; RESP_BUF_SIZE],
    pub resp_len: usize,
    /// Model name (default: `meta/llama-3.1-8b-instruct`).
    pub model: [u8; 64],
    pub model_len: u8,
    /// Destination TCP port (default: 8000).
    pub nim_port: u16,
    /// Destination IP (default: 10.0.2.2 — QEMU SLIRP host).
    pub nim_host_ip: [u8; 4],
    /// 0=none, 1=streaming model name, 2=streaming port digits.
    pub streaming_mode: u8,
    /// Multi-turn conversation history (raw JSON fragment with trailing comma).
    pub history_buf: [u8; HISTORY_BUF_SIZE],
    pub history_len: usize,
    /// Number of completed turns in the current session.
    pub turn_count: u16,
    /// Scratch space for port digit accumulation during PORT_BEGIN streaming.
    pub port_digits: [u8; 6],
    pub port_digit_len: u8,
}

/// DMA-safe static wrapper — same governance pattern as k-net and k-chat.
/// SAFETY: uniprocessor cooperative kernel — no concurrent access.
struct NimCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for NimCell<T> {}

/// Default model name bytes (ASCII, null-padded).
const DEFAULT_MODEL: &[u8] = b"meta/llama-3.1-8b-instruct";

static NIM_STATE: NimCell<NimState> = NimCell(UnsafeCell::new(NimState {
    console_target: 0,
    net_target:     0,
    input_buf:      [0u8; INPUT_BUF_SIZE],
    input_len:      0,
    resp_buf:       [0u8; RESP_BUF_SIZE],
    resp_len:       0,
    model:          {
        // Copy DEFAULT_MODEL into a const [u8; 64], zero-padding the rest.
        let mut m = [0u8; 64];
        let src = b"meta/llama-3.1-8b-instruct";
        let mut i = 0usize;
        while i < src.len() { m[i] = src[i]; i += 1; }
        m
    },
    model_len:      DEFAULT_MODEL.len() as u8,
    nim_port:       8000,
    nim_host_ip:    [10, 0, 2, 2],
    streaming_mode: 0,
    history_buf:    [0u8; HISTORY_BUF_SIZE],
    history_len:    0,
    turn_count:     0,
    port_digits:    [0u8; 6],
    port_digit_len: 0,
}));

// ── State accessors ────────────────────────────────────────────────────────────

pub(crate) unsafe fn state_mut(_ctx: *mut ExecutorContext) -> &'static mut NimState {
    unsafe { &mut *NIM_STATE.0.get() }
}

// ── ABI cache ─────────────────────────────────────────────────────────────────
// Stashed for the duration of an on_event call so nim_do_request can reach
// the emit_signal ABI without threading it through every call.
//
// SAFETY: written once at the top of on_event, read only within the same
// synchronous call. Uniprocessor cooperative kernel — no concurrent access.

struct AbiCacheCell(UnsafeCell<Option<*const KernelAbi>>);
unsafe impl Sync for AbiCacheCell {}

static ABI_CACHE: AbiCacheCell = AbiCacheCell(UnsafeCell::new(None));

#[inline(always)]
fn abi_cache_store(val: Option<*const KernelAbi>) {
    unsafe { *ABI_CACHE.0.get() = val; }
}

/// Read the cached `KernelAbi` pointer.  Paired with `abi_cache_store`;
/// retained even when no current consumer reads from it because removing
/// half of a paired API breaks future call sites that need to recover
/// the cached ABI in IRQ-context (where `ctx.abi` isn't readily
/// available).
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn abi_cache_load() -> Option<*const KernelAbi> {
    unsafe { *ABI_CACHE.0.get() }
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
    let state   = unsafe { &*NIM_STATE.0.get() };
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

// ── HTTP scratch buffers ───────────────────────────────────────────────────────

struct HttpReqCell(UnsafeCell<[u8; HTTP_REQ_BUF_SIZE]>);
unsafe impl Sync for HttpReqCell {}
static HTTP_REQ: HttpReqCell = HttpReqCell(UnsafeCell::new([0u8; HTTP_REQ_BUF_SIZE]));

struct HttpRawCell(UnsafeCell<[u8; HTTP_RAW_BUF_SIZE]>);
unsafe impl Sync for HttpRawCell {}
static HTTP_RAW: HttpRawCell = HttpRawCell(UnsafeCell::new([0u8; HTTP_RAW_BUF_SIZE]));

// ── Buffer utility functions ───────────────────────────────────────────────────

/// Append `data` into `buf[*pos..]`, advancing `*pos`. Returns `false` on overflow.
pub(crate) fn buf_append(buf: &mut [u8], pos: &mut usize, data: &[u8]) -> bool {
    if *pos + data.len() > buf.len() { return false; }
    buf[*pos..*pos + data.len()].copy_from_slice(data);
    *pos += data.len();
    true
}

/// Append a decimal `u32` into `buf`. Returns `false` on overflow.
pub(crate) fn buf_append_u32(buf: &mut [u8], pos: &mut usize, mut val: u32) -> bool {
    let mut tmp = [0u8; 10];
    let mut len = 0usize;
    if val == 0 {
        tmp[0] = b'0'; len = 1;
    } else {
        while val > 0 {
            tmp[len] = b'0' + (val % 10) as u8;
            val /= 10; len += 1;
        }
    }
    let mut out = [0u8; 10];
    for i in 0..len { out[i] = tmp[len - 1 - i]; }
    buf_append(buf, pos, &out[..len])
}

/// Append a JSON-escaped string into `buf`, escaping `"`, `\`, `\n`, `\r`, `\t`.
/// Returns `false` on overflow.
pub(crate) fn buf_append_json_str(buf: &mut [u8], pos: &mut usize, s: &[u8]) -> bool {
    for &b in s {
        let ok = match b {
            b'"'  => buf_append(buf, pos, b"\\\""),
            b'\\' => buf_append(buf, pos, b"\\\\"),
            b'\n' => buf_append(buf, pos, b"\\n"),
            b'\r' => buf_append(buf, pos, b"\\r"),
            b'\t' => buf_append(buf, pos, b"\\t"),
            other => {
                if *pos < buf.len() { buf[*pos] = other; *pos += 1; true }
                else { false }
            }
        };
        if !ok { return false; }
    }
    true
}

/// Find the first occurrence of `needle` in `haystack`. Returns `None` if not found.
pub(crate) fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    'outer: for i in 0..haystack.len().saturating_sub(needle.len() - 1) {
        for j in 0..needle.len() {
            if haystack[i + j] != needle[j] { continue 'outer; }
        }
        return Some(i);
    }
    None
}

/// Extract the string value of a JSON field named `key` from `json`.
/// Searches for `"key":"<value>"` and returns a slice for `<value>`.
/// Handles `\"` escape sequences inside the value (skips escaped quotes).
pub(crate) fn extract_json_str<'a>(json: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let mut search = [0u8; 128];
    let mut sp = 0usize;
    search[sp] = b'"'; sp += 1;
    for &b in key {
        if sp < search.len() { search[sp] = b; sp += 1; }
    }
    if sp + 3 > search.len() { return None; }
    search[sp] = b'"'; sp += 1;
    search[sp] = b':'; sp += 1;
    search[sp] = b'"'; sp += 1;

    let pattern = &search[..sp];
    let start = find_bytes(json, pattern)? + sp;
    let mut end = start;
    while end < json.len() {
        if json[end] == b'\\' {
            end += 2; // skip escaped character
        } else if json[end] == b'"' {
            return Some(&json[start..end]);
        } else {
            end += 1;
        }
    }
    None
}

// ── OpenAI request builder ─────────────────────────────────────────────────────

/// Build an OpenAI `/v1/chat/completions` JSON body into `body_buf`.
///
/// Format:
/// ```json
/// {"model":"<model>","messages":[
///   {"role":"system","content":"You are a helpful AI assistant. Reply concisely in plain prose."},
///   <history bytes inserted inline>,
///   {"role":"user","content":"<user_msg>"}
/// ],"max_tokens":512}
/// ```
///
/// Returns the body byte length, or `0` on buffer overflow.
pub(crate) fn build_openai_body(
    body_buf:    &mut [u8],
    model:       &[u8],
    history_buf: &[u8],
    history_len: usize,
    user_msg:    &[u8],
) -> usize {
    let mut p = 0usize;

    if !buf_append(body_buf, &mut p, b"{\"model\":\"") { return 0; }
    if !buf_append_json_str(body_buf, &mut p, model)   { return 0; }
    if !buf_append(body_buf, &mut p, b"\",\"messages\":[") { return 0; }
    // System message
    if !buf_append(body_buf, &mut p, b"{\"role\":\"system\",\"content\":\"You are a helpful AI assistant. Reply concisely in plain prose.\"}") { return 0; }
    // History (comma-terminated fragment — already has trailing comma if non-empty)
    if history_len > 0 {
        if !buf_append(body_buf, &mut p, b",") { return 0; }
        if p + history_len > body_buf.len() { return 0; }
        body_buf[p..p + history_len].copy_from_slice(&history_buf[..history_len]);
        p += history_len;
    }
    // Current user message
    if !buf_append(body_buf, &mut p, b",{\"role\":\"user\",\"content\":\"") { return 0; }
    if !buf_append_json_str(body_buf, &mut p, user_msg) { return 0; }
    if !buf_append(body_buf, &mut p, b"\"}],\"max_tokens\":512}") { return 0; }

    p
}

/// Build a full HTTP/1.0 POST request into `req_buf`.
/// Returns the request byte length, or `0` on overflow.
pub(crate) fn build_nim_http_request(
    req_buf: &mut [u8],
    body:    &[u8],
    host_ip: [u8; 4],
    port:    u16,
) -> usize {
    let mut p = 0usize;

    if !buf_append(req_buf, &mut p, b"POST /v1/chat/completions HTTP/1.0\r\nHost: ") { return 0; }
    // Host: <ip>:<port>
    for (i, &octet) in host_ip.iter().enumerate() {
        if i > 0 {
            if p >= req_buf.len() { return 0; }
            req_buf[p] = b'.'; p += 1;
        }
        if !buf_append_u32(req_buf, &mut p, octet as u32) { return 0; }
    }
    if p >= req_buf.len() { return 0; }
    req_buf[p] = b':'; p += 1;
    if !buf_append_u32(req_buf, &mut p, port as u32) { return 0; }

    if !buf_append(req_buf, &mut p, b"\r\nContent-Type: application/json\r\nContent-Length: ") { return 0; }
    if !buf_append_u32(req_buf, &mut p, body.len() as u32) { return 0; }
    if !buf_append(req_buf, &mut p, b"\r\n\r\n") { return 0; }
    if !buf_append(req_buf, &mut p, body) { return 0; }

    p
}

// ── History management ─────────────────────────────────────────────────────────

/// Append one completed exchange to `state.history_buf`.
///
/// Appended format (with trailing comma):
/// `{"role":"user","content":"<user_msg>"},{"role":"assistant","content":"<asst_reply>"},`
///
/// If the new entry would overflow `history_buf`, the entire history is cleared
/// before appending the new entry (graceful degradation — always keeps the
/// most-recent turn).
pub(crate) fn append_history(state: &mut NimState, user_msg: &[u8], asst_reply: &[u8]) {
    // Estimate encoded length (worst case: every char is escaped → 2×)
    // We do a trial encode to check fit.
    let mut trial = [0u8; HISTORY_BUF_SIZE];
    let mut tp = state.history_len;
    let fits = {
        let ok = buf_append(&mut trial[..], &mut tp, b"{\"role\":\"user\",\"content\":\"")
            && buf_append_json_str(&mut trial[..], &mut tp, user_msg)
            && buf_append(&mut trial[..], &mut tp, b"\"},{\"role\":\"assistant\",\"content\":\"")
            && buf_append_json_str(&mut trial[..], &mut tp, asst_reply)
            && buf_append(&mut trial[..], &mut tp, b"\"},");
        ok
    };

    if !fits {
        // Clear history and try again from scratch
        state.history_len = 0;
        tp = 0;
        let _ = buf_append(&mut trial[..], &mut tp, b"{\"role\":\"user\",\"content\":\"")
            && buf_append_json_str(&mut trial[..], &mut tp, user_msg)
            && buf_append(&mut trial[..], &mut tp, b"\"},{\"role\":\"assistant\",\"content\":\"")
            && buf_append_json_str(&mut trial[..], &mut tp, asst_reply)
            && buf_append(&mut trial[..], &mut tp, b"\"},");
    }

    // Copy result into history_buf
    let new_len = tp;
    if new_len > 0 && new_len <= HISTORY_BUF_SIZE {
        state.history_buf[..new_len].copy_from_slice(&trial[..new_len]);
        state.history_len = new_len;
        state.turn_count = state.turn_count.saturating_add(1);
    }
}

// ── TCP inference request ──────────────────────────────────────────────────────

/// Perform a NIM HTTP POST request and populate `state.resp_buf`.
///
/// Uses a two-stage scratch approach:
///   1. Build the JSON body into the second half of `HTTP_REQ`.
///   2. Build the full HTTP request into the first half of `HTTP_REQ`.
///   3. Send via `k_net::net_http_post_sync`.
///   4. Parse the response: skip HTTP headers, extract `"content"` field.
///
/// On any error (overflow, TCP failure, parse failure) a human-readable
/// error message is written to `state.resp_buf` instead.
pub(crate) unsafe fn nim_do_request(state: &mut NimState) {
    state.resp_len = 0;

    let model: &[u8] = if state.model_len > 0 {
        &state.model[..state.model_len as usize]
    } else {
        DEFAULT_MODEL
    };

    let user_msg = &state.input_buf[..state.input_len];

    let req_buf  = unsafe { &mut *HTTP_REQ.0.get() };
    let resp_raw = unsafe { &mut *HTTP_RAW.0.get() };

    // ── Stage 1: JSON body into scratch buffer ────────────────────────────────
    // We use a separate fixed-size body scratch so we know the exact length
    // before building the HTTP headers.
    let mut body_scratch = [0u8; 2048];
    let body_len = build_openai_body(
        &mut body_scratch,
        model,
        &state.history_buf,
        state.history_len,
        user_msg,
    );
    if body_len == 0 {
        let err = b"[NIM] JSON body overflow - message too long";
        let n = err.len().min(RESP_BUF_SIZE);
        state.resp_buf[..n].copy_from_slice(&err[..n]);
        state.resp_len = n;
        return;
    }

    // ── Stage 2: Full HTTP request ────────────────────────────────────────────
    let req_len = build_nim_http_request(
        req_buf,
        &body_scratch[..body_len],
        state.nim_host_ip,
        state.nim_port,
    );
    if req_len == 0 {
        let err = b"[NIM] HTTP request buffer overflow";
        let n = err.len().min(RESP_BUF_SIZE);
        state.resp_buf[..n].copy_from_slice(&err[..n]);
        state.resp_len = n;
        return;
    }

    // ── Stage 3: TCP send ─────────────────────────────────────────────────────
    let result = unsafe {
        k_net::net_http_post_sync(
            state.nim_host_ip,
            state.nim_port,
            &req_buf[..req_len],
            resp_raw,
        )
    };

    match result {
        None => {
            let err = b"[NIM] TCP failed - is NIM running? docker run --gpus all -p 8000:8000 nvcr.io/nim/meta/llama-3.1-8b-instruct:latest";
            let n = err.len().min(RESP_BUF_SIZE);
            state.resp_buf[..n].copy_from_slice(&err[..n]);
            state.resp_len = n;
        }
        Some(0) => {
            let err = b"[NIM] Empty TCP response - server may still be loading";
            let n = err.len().min(RESP_BUF_SIZE);
            state.resp_buf[..n].copy_from_slice(&err[..n]);
            state.resp_len = n;
        }
        Some(raw_len) => {
            let raw = &resp_raw[..raw_len];

            // Skip HTTP headers (find \r\n\r\n separator)
            let body_off = find_bytes(raw, b"\r\n\r\n")
                .map(|o| o + 4)
                .unwrap_or(0);
            let json = &raw[body_off..];

            // Extract "content" field from the OpenAI response JSON
            if let Some(content) = extract_json_str(json, b"content") {
                let n = content.len().min(RESP_BUF_SIZE);
                state.resp_buf[..n].copy_from_slice(&content[..n]);
                state.resp_len = n;
            } else {
                // Fallback: return the raw JSON body so the user can debug
                let n = json.len().min(RESP_BUF_SIZE);
                state.resp_buf[..n].copy_from_slice(&json[..n]);
                state.resp_len = n;
            }
        }
    }
}

// ── Banner ─────────────────────────────────────────────────────────────────────

/// Draw the NIM mode banner at the top of the screen.
pub(crate) fn draw_nim_banner(sink: &ConsoleSink) {
    set_color(sink, 0, 10); // black on bright-green
    print_str(sink, "  GOS NIM -- NVIDIA NIM / OpenAI Inference                                        ");
    set_color(sink, 8, 0);  // dark-grey on black
    print_str(sink, "  Type your message and press Enter. Commands: :model :port :clear :exit      \n");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

// ── Node lifecycle callbacks ───────────────────────────────────────────────────

unsafe extern "C" fn nim_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
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

    let state = unsafe { &mut *NIM_STATE.0.get() };
    state.console_target = console_target;
    state.net_target     = net_target;

    ExecStatus::Done
}

unsafe extern "C" fn nim_on_event(
    ctx:   *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    // Cache the ABI pointer for the duration of this call so nim_do_request
    // can reach it without threading it through every stack frame.
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

unsafe extern "C" fn nim_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

// ── Public exports ─────────────────────────────────────────────────────────────

pub const NIM_PLUGIN_ID_PUB: PluginId                = NIM_PLUGIN_ID;
pub const NIM_NODE_ID_PUB: gos_protocol::NodeId      = NIM_NODE_ID;
