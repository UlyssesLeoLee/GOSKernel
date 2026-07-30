#![no_std]

// ============================================================
// GOS KERNEL TOPOLOGY — k-vk-host
// Graph-native visual bridge (V0 host-bridged surface, Phase I groundwork).
//
// This node mirrors `k-cuda-host`: it is a graph executor that emits a stream
// of structured frames to a host-side helper. Where k-cuda-host emits CUDA job
// frames, k-vk-host emits an `@gos.vk` *display list* (nodes = render units,
// edges = conduits) over a dedicated COM3 UART, which QEMU exposes as a TCP
// server. The host watcher `tools/gfx-bridge.py` renders it into a window.
//
// The display list is the architectural seam: a later Vulkan host backend can
// consume the same frames with no kernel-side change.
//
// MERGE (p:Plugin {id: "K_VK", name: "k-vk-host"})
// SET p.executor = "k_vk_host::EXECUTOR_ID", p.node_type = "Render", p.transport = "serial:com3"
//
// -- Dependencies
// MERGE (dep_K_SERIAL:Plugin {id: "K_SERIAL"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_SERIAL)
//
// -- Exported Capabilities (APIs)
// MERGE (cap_vk_bridge:Capability {namespace: "vk", name: "bridge"})
// MERGE (p)-[:EXPORTS]->(cap_vk_bridge)
//
// -- Live-graph mode (V1): the REPORT path reads the runtime graph and renders it
// MERGE (dep_gos_runtime:Module {id: "gos-runtime"})
// MERGE (p)-[:READS_FROM]->(dep_gos_runtime)
// ============================================================

use core::fmt::Write as _;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use x86_64::instructions::interrupts;

use gos_protocol::{
    packet_to_signal, ExecStatus, ExecutorContext, ExecutorId, GraphEdgeSummary, GraphNodeSummary,
    NodeEvent, NodeExecutorVTable, RuntimeEdgeType, RuntimeNodeType, Signal, VectorAddress,
    VK_CONTROL_CLEAR, VK_CONTROL_DEMO, VK_CONTROL_REPORT,
};

/// Topological address of the visual bridge node.
pub const NODE_VEC: VectorAddress = gos_protocol::vectors::SVC_VK;

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.vk");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(vk_on_init),
    on_event: Some(vk_on_event),
    on_suspend: None,
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

// ── @gos.vk display-list frames (see tools/gfx-bridge.py for the protocol) ──
const HELLO_FRAME: &[u8] = b"VKHEL:gos-kernel\n";
const CLEAR_FRAME: &[u8] = b"VKCLR:101018\nVKEND:\n";
/// A small slice of the GOS graph: kernel core wired to a VGA render unit and a
/// shell unit. Immediate-mode: the whole scene is restated and presented.
const DEMO_FRAME: &[u8] = b"VKDIM:800x600\n\
VKCLR:101018\n\
VKNOD:1:320:60:160:64:2a6cff:gos-kernel\n\
VKNOD:2:120:300:140:56:23a06a:k-vga\n\
VKNOD:3:540:300:140:56:c2410c:k-shell\n\
VKEDG:1:2:5566aa\n\
VKEDG:1:3:5566aa\n\
VKEND:\n";

// ── COM3 (0x3E8) — dedicated visual-bridge UART ─────────────────────────────
// QEMU's 3rd `-serial` maps to COM3; we expose it as TCP:14445 so the host
// watcher owns a clean channel separate from the boot log (COM1) and chat (COM2).
const COM3: u16 = 0x3E8;
static COM3_READY: AtomicBool = AtomicBool::new(false);

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

/// Initialise COM3 at 115200 8N1 with FIFOs enabled (mirror of k-chat's COM2).
fn com3_init() {
    unsafe {
        out8(COM3 + 1, 0x00); // disable interrupts
        out8(COM3 + 3, 0x80); // enable DLAB
        out8(COM3, 0x01); // divisor LSB → 115200 baud
        out8(COM3 + 1, 0x00); // divisor MSB
        out8(COM3 + 3, 0x03); // 8-N-1
        out8(COM3 + 2, 0xC7); // enable + clear FIFOs, 14-byte threshold
        out8(COM3 + 4, 0x0B); // RTS + DTR + OUT2
    }
}

/// Initialise COM3 exactly once, regardless of init/event ordering.
fn ensure_com3() {
    if !COM3_READY.swap(true, Ordering::AcqRel) {
        com3_init();
    }
}

/// Returns `true` if the TX holding register is empty (ready to send).
#[inline(always)]
fn com3_tx_ready() -> bool {
    unsafe { in8(COM3 + 5) & 0x20 != 0 }
}

/// Blocking single-byte write to COM3, with a spin budget so a stuck UART
/// cannot wedge the scheduler.
fn com3_write_byte(b: u8) {
    let mut spins = 0usize;
    while !com3_tx_ready() {
        spin_loop();
        spins += 1;
        if spins > 10_000_000 {
            return; // TX stuck — give up rather than hang
        }
    }
    unsafe { out8(COM3, b); }
}

/// Emit a display-list frame buffer to the visual bridge verbatim.
fn com3_emit(bytes: &[u8]) {
    for &b in bytes {
        com3_write_byte(b);
    }
}

unsafe extern "C" fn vk_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    ensure_com3();
    com3_emit(HELLO_FRAME);
    com3_emit(DEMO_FRAME); // boot-time auto-draw of the seed graph
    ExecStatus::Done
}

unsafe extern "C" fn vk_on_event(_ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    if event.is_null() {
        return ExecStatus::Done;
    }
    ensure_com3();
    let signal = packet_to_signal(unsafe { (*event).signal });
    match signal {
        Signal::Spawn { .. } => com3_emit(HELLO_FRAME),
        Signal::Control { cmd, .. } => match cmd {
            VK_CONTROL_DEMO => com3_emit(DEMO_FRAME),
            VK_CONTROL_CLEAR => com3_emit(CLEAR_FRAME),
            VK_CONTROL_REPORT => render_live_graph(),
            _ => {}
        },
        _ => {}
    }
    ExecStatus::Done
}

// ── Live-graph rendering (V1) ────────────────────────────────────────────────
// The REPORT control walks the actual runtime graph and restates it as an
// `@gos.vk` display list, so the visual surface mirrors the live OS instead of
// a fixed seed. Boot auto-draw and the DEMO control keep emitting the static
// seed (clean first frame + offline `gfx-bridge.py --check` contract).

/// Fixed-capacity builder for one display-list line. Formatting is best-effort:
/// an over-long line is truncated rather than panicking (labels are short
/// static node keys, so truncation never trips in practice).
struct FrameBuf {
    buf: [u8; 160],
    len: usize,
}

impl FrameBuf {
    fn new() -> Self {
        Self { buf: [0u8; 160], len: 0 }
    }
    fn reset(&mut self) {
        self.len = 0;
    }
    fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl core::fmt::Write for FrameBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let n = bytes.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
        Ok(())
    }
}

/// Stable color per node kind so the graph reads at a glance.
fn node_color(t: RuntimeNodeType) -> u32 {
    match t {
        RuntimeNodeType::Hardware => 0xc2410c,
        RuntimeNodeType::Driver => 0xb45309,
        RuntimeNodeType::Service => 0x23a06a,
        RuntimeNodeType::PluginEntry => 0x2a6cff,
        RuntimeNodeType::Compute => 0x7c3aed,
        RuntimeNodeType::Router => 0x0891b2,
        RuntimeNodeType::Aggregator => 0xca8a04,
        RuntimeNodeType::Vector => 0x64748b,
    }
}

/// Architectural tier (row) for a node kind: lower tiers are the physical /
/// foundation layers, higher tiers the logical / data layers. `render_live_graph`
/// lays the surface out in these bands so the graph reads as layers instead of a
/// structure-blind grid.
fn node_tier(t: RuntimeNodeType) -> usize {
    match t {
        RuntimeNodeType::Hardware => 0,
        RuntimeNodeType::Driver => 1,
        RuntimeNodeType::Service => 2,
        RuntimeNodeType::Router => 3,
        RuntimeNodeType::Aggregator => 4,
        RuntimeNodeType::Compute => 5,
        RuntimeNodeType::PluginEntry => 6,
        RuntimeNodeType::Vector => 7,
    }
}

/// Stable color per edge kind so conduits are distinguishable in the hairball.
fn edge_color(t: RuntimeEdgeType) -> u32 {
    match t {
        RuntimeEdgeType::Call => 0x5566aa,
        RuntimeEdgeType::Spawn => 0x6b8e23,
        RuntimeEdgeType::Depend => 0x8a5cf0,
        RuntimeEdgeType::Signal => 0x46a3c2,
        RuntimeEdgeType::Return => 0x9a7b4f,
        RuntimeEdgeType::Mount => 0xb06f3a,
        RuntimeEdgeType::Sync => 0x4f9a7b,
        RuntimeEdgeType::Stream => 0xc25e8a,
        RuntimeEdgeType::Use => 0x5b6b8c,
        // Metadata/reference edge (no runtime routing) — neutral gray,
        // visually distinct from the active data-flow edge colors above.
        RuntimeEdgeType::Link => 0x999999,
    }
}

fn find_key(keys: &[u64], key: u64) -> Option<usize> {
    keys.iter().position(|&k| k == key)
}

/// Shorten a node label to roughly fit `node_w` px (the host renders a bold
/// monospace at ~7 px/char). Tries the full key, then the head before the first
/// '.' (so `clipboard.mount` -> `clipboard`), then a char-safe truncation.
/// Returns a borrowed slice — no allocation. Only crowded tiers trigger
/// shortening, so roomy rows (e.g. `theme.*`) keep their full keys.
fn fit_label(label: &str, node_w: u32) -> &str {
    let max = (node_w / 7).max(3) as usize;
    if label.len() <= max {
        return label;
    }
    let head = match label.find('.') {
        Some(i) => &label[..i],
        None => label,
    };
    if head.len() <= max {
        return head;
    }
    let mut n = max.min(head.len());
    while n > 0 && !head.is_char_boundary(n) {
        n -= 1;
    }
    &head[..n]
}

/// Structural `graph_epoch` of the last frame emitted to the visual surface.
/// `u64::MAX` means "nothing emitted yet", so the first auto-refresh always
/// draws. Both the manual REPORT path and the auto path funnel through
/// `render_live_graph`, which writes this; `vk_auto_refresh` reads it.
static LAST_EMITTED_EPOCH: AtomicU64 = AtomicU64::new(u64::MAX);

/// Walk the live runtime graph and emit it as one `@gos.vk` frame.
///
/// Safe to call from `on_event`: `route_signal` releases the runtime lock
/// before invoking the executor, so the `node_page`/`edge_page` locks taken
/// here cannot deadlock. All layout arithmetic uses `saturating_sub` because
/// the kernel is built with overflow checks on.
fn render_live_graph() {
    const PAGE: usize = 8;
    const CAP: usize = gos_runtime::MAX_NODES;
    const CANVAS_W: u32 = 800;
    const CANVAS_H: u32 = 600;
    const MARGIN: u32 = 50;

    // Snapshot the structural epoch this frame represents; stored after emit so
    // `vk_auto_refresh` can skip re-drawing an unchanged graph. Every
    // `gos_runtime` read below holds the RUNTIME lock, so each is bracketed with
    // `without_interrupts`: the timer IRQ also locks RUNTIME, and the auto path
    // runs with interrupts enabled. The slow `com3_emit`s hold no lock and stay
    // interruptible, so they never stall IRQs.
    let epoch = interrupts::without_interrupts(gos_runtime::graph_epoch);

    // Size the grid from the live node count before emitting anything.
    let mut probe = [GraphNodeSummary::EMPTY; PAGE];
    let (total, _) =
        interrupts::without_interrupts(|| gos_runtime::node_page::<PAGE>(0, &mut probe));
    if total == 0 {
        com3_emit(CLEAR_FRAME);
        LAST_EMITTED_EPOCH.store(epoch, Ordering::Relaxed);
        return;
    }
    let total = total.min(CAP);

    // Tier the nodes by kind so the surface reads as architectural layers
    // (drivers -> services -> routing -> compute -> plugins/data) instead of a
    // structure-blind grid. Pass A: tally how many live nodes fall in each tier;
    // only non-empty tiers get a row, so the bands compact. (node_page is cached
    // per epoch, so this extra walk is cheap.)
    const NUM_TIERS: usize = 8;
    let mut tier_total = [0usize; NUM_TIERS];
    {
        let mut counted = 0usize;
        let mut offset = 0usize;
        while offset < total {
            let mut page = [GraphNodeSummary::EMPTY; PAGE];
            let (_, returned) =
                interrupts::without_interrupts(|| gos_runtime::node_page::<PAGE>(offset, &mut page));
            if returned == 0 {
                break;
            }
            for entry in page.iter().take(returned) {
                if counted >= total {
                    break;
                }
                counted += 1;
                tier_total[node_tier(entry.node_type)] += 1;
            }
            offset += returned;
        }
    }
    let mut row_of_tier = [u32::MAX; NUM_TIERS];
    let mut num_rows = 0u32;
    for t in 0..NUM_TIERS {
        if tier_total[t] > 0 {
            row_of_tier[t] = num_rows;
            num_rows += 1;
        }
    }
    let num_rows = num_rows.max(1);
    let row_h = (CANVAS_H - 2 * MARGIN) / num_rows;
    let node_h = 40u32.min(row_h.saturating_sub(8)).max(18);

    com3_emit(b"VKDIM:800x600\nVKCLR:101018\n");

    // Pass B: one VKNOD per live node, positioned by tier (row) and spread
    // evenly within its tier (column). keys[] records each vector in emission
    // order so Pass 2 can resolve edge endpoints to frame-local ids.
    let mut keys = [0u64; CAP];
    let mut tier_placed = [0u32; NUM_TIERS];
    let mut node_count = 0usize;
    let mut fb = FrameBuf::new();
    let mut offset = 0usize;
    while offset < total {
        let mut page = [GraphNodeSummary::EMPTY; PAGE];
        let (_, returned) =
            interrupts::without_interrupts(|| gos_runtime::node_page::<PAGE>(offset, &mut page));
        if returned == 0 {
            break;
        }
        for entry in page.iter().take(returned) {
            let gi = node_count;
            if gi >= total {
                break;
            }
            keys[gi] = entry.vector.as_u64();
            node_count += 1;
            let tier = node_tier(entry.node_type);
            let n_tier = tier_total[tier].max(1) as u32;
            let within = tier_placed[tier];
            tier_placed[tier] += 1;
            let col_w = (CANVAS_W - 2 * MARGIN) / n_tier;
            let node_w = col_w.saturating_sub(12).min(140).max(34);
            let x = MARGIN + within * col_w + col_w.saturating_sub(node_w) / 2;
            let y = MARGIN + row_of_tier[tier] * row_h + row_h.saturating_sub(node_h) / 2;
            let raw_label = if !entry.local_node_key.is_empty() {
                entry.local_node_key
            } else if !entry.plugin_name.is_empty() {
                entry.plugin_name
            } else {
                "node"
            };
            let label = fit_label(raw_label, node_w);
            fb.reset();
            let _ = writeln!(
                fb,
                "VKNOD:{}:{}:{}:{}:{}:{:06x}:{}",
                gi + 1,
                x,
                y,
                node_w,
                node_h,
                node_color(entry.node_type),
                label
            );
            com3_emit(fb.as_bytes());
        }
        offset += returned;
    }

    // Pass 2: one VKEDG per live edge whose endpoints are both on screen.
    let mut eoffset = 0usize;
    loop {
        let mut page = [GraphEdgeSummary::EMPTY; PAGE];
        let (etotal, returned) =
            interrupts::without_interrupts(|| gos_runtime::edge_page::<PAGE>(eoffset, &mut page));
        if returned == 0 {
            break;
        }
        for edge in page.iter().take(returned) {
            let from = find_key(&keys[..node_count], edge.from_vector.as_u64());
            let to = find_key(&keys[..node_count], edge.to_vector.as_u64());
            if let (Some(f), Some(t)) = (from, to) {
                if f != t {
                    fb.reset();
                    let _ = writeln!(
                        fb,
                        "VKEDG:{}:{}:{:06x}",
                        f + 1,
                        t + 1,
                        edge_color(edge.edge_type)
                    );
                    com3_emit(fb.as_bytes());
                }
            }
        }
        eoffset += returned;
        if eoffset >= etotal {
            break;
        }
    }

    com3_emit(b"VKEND:\n");
    LAST_EMITTED_EPOCH.store(epoch, Ordering::Relaxed);
}

/// Auto-live refresh hook, driven by the kernel steady-state loop.
///
/// Re-emits the live graph frame only when the runtime's structural
/// `graph_epoch` has advanced since the last emit, so an idle graph costs just
/// one epoch read. Call with interrupts ENABLED: the slow UART emit then runs
/// without stalling IRQs, while the short RUNTIME reads inside
/// `render_live_graph` bracket themselves with `without_interrupts`. Manual
/// `vk` (VK_CONTROL_REPORT) still forces a redraw via `render_live_graph`.
pub fn vk_auto_refresh() {
    let epoch = interrupts::without_interrupts(gos_runtime::graph_epoch);
    if epoch != LAST_EMITTED_EPOCH.load(Ordering::Relaxed) {
        ensure_com3();
        render_live_graph();
    }
}

/// Force-emit the current graph as an `@gos.vk` frame regardless of whether the
/// structural epoch changed. `vk_auto_refresh` only emits on change, so a visual
/// surface (gos-vk-viewer) that connects *after* boot — when the graph is idle —
/// would otherwise never receive a frame. Call this on a slow keepalive timer so
/// a late-joining viewer obtains the current scene within ~1s.
pub fn vk_force_refresh() {
    ensure_com3();
    render_live_graph();
}

/// Drain one byte of input from the visual bridge (COM3 RX), or `None` if none
/// is waiting (B3b). gos-vk-viewer forwards host keyboard back over the SAME
/// TCP serial it reads @gos.vk frames from — serial is full-duplex, so kernel
/// TX (frames) and RX (input) share the one 14445 connection. The caller (the
/// steady-state loop) routes these bytes into the input queue, so typing in the
/// viewer drives the OS and the resulting graph change streams back as a frame.
pub fn vk_drain_input() -> Option<u8> {
    ensure_com3();
    // LSR (COM3+5) bit 0 = receive-data-ready.
    if unsafe { in8(COM3 + 5) } & 0x01 != 0 {
        Some(unsafe { in8(COM3) })
    } else {
        None
    }
}
