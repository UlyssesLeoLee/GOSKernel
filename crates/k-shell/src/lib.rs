#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-shell
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_SHELL", name: "k-shell"})
// SET p.executor = "k_shell::EXECUTOR_ID", p.node_type = "PluginEntry", p.state_schema = "0x200E"
//
// -- Dependencies
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
// MERGE (dep_K_PS2:Plugin {id: "K_PS2"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PS2)
// MERGE (dep_K_HEAP:Plugin {id: "K_HEAP"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_HEAP)
// MERGE (dep_K_IME:Plugin {id: "K_IME"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_IME)
// MERGE (dep_K_NET:Plugin {id: "K_NET"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_NET)
// MERGE (dep_K_CYPHER:Plugin {id: "K_CYPHER"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_CYPHER)
// MERGE (dep_K_CUDA:Plugin {id: "K_CUDA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_CUDA)
//
// -- Hardware Resources
//
// -- Exported Capabilities (APIs)
// MERGE (cap_shell_input:Capability {namespace: "shell", name: "input"})
// MERGE (p)-[:EXPORTS]->(cap_shell_input)
//
// -- Imported Capabilities (Dependencies)
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
// MERGE (cap_ime_control:Capability {namespace: "ime", name: "control"})
// MERGE (p)-[:IMPORTS]->(cap_ime_control)
// MERGE (cap_ai_supervisor:Capability {namespace: "ai", name: "supervisor"})
// MERGE (p)-[:IMPORTS]->(cap_ai_supervisor)
// MERGE (cap_cypher_query:Capability {namespace: "cypher", name: "query"})
// MERGE (p)-[:IMPORTS]->(cap_cypher_query)
// MERGE (cap_net_uplink:Capability {namespace: "net", name: "uplink"})
// MERGE (p)-[:IMPORTS]->(cap_net_uplink)
// MERGE (cap_cuda_bridge:Capability {namespace: "cuda", name: "bridge"})
// MERGE (p)-[:IMPORTS]->(cap_cuda_bridge)
// ============================================================


use core::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};

use gos_protocol::{
    derive_edge_id, derive_node_id, packet_to_signal, signal_to_packet,
    AI_CONTROL_API_BEGIN, AI_CONTROL_API_COMMIT,
    CHAT_CONTROL_EXIT, CHAT_CONTROL_SEND,
    CLIPBOARD_DATA_BEGIN, CLIPBOARD_DATA_CLEAR,
    CLIPBOARD_DATA_COMMIT, CUDA_CONTROL_JOB_BEGIN, CUDA_CONTROL_JOB_COMMIT,
    CYPHER_CONTROL_QUERY_BEGIN,
    CYPHER_CONTROL_QUERY_COMMIT, DISPLAY_THEME_SHOJI,
    DISPLAY_THEME_WABI, EdgeSpec, EdgeVector, ExecStatus, ExecutorContext,
    ExecutorId, GraphDiffKind, GraphEdgeDirection, GraphEdgeSummary, GraphNodeSummary,
    IME_CONTROL_SET_MODE, IME_MODE_ASCII, IME_MODE_ZH_PINYIN, INPUT_KEY_DOWN,
    INPUT_KEY_PAGE_DOWN, INPUT_KEY_PAGE_UP, INPUT_KEY_UP, KernelAbi,
    NodeEvent,
    NodeExecutorVTable, PluginId, RoutePolicy, RuntimeEdgeType, Signal,
    VectorAddress,
};

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 0, 0);
pub const THEME_WABI_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 1, 0);
pub const THEME_SHOJI_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 2, 0);
pub const THEME_CURRENT_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 3, 0);
pub const CLIPBOARD_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 4, 0);
pub const PALETTE_CYAN_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 5, 0); // V2.62
pub const PALETTE_GOLD_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 6, 0); // V2.62
/// ADR-016 option A — the conventional mount anchor `gpm install` hangs
/// each package's provisional node off of, so installed packages are
/// discoverable as one hub's `Mount` out-edges instead of scattered
/// unconnected nodes. Purely a graph anchor point (mirrors
/// `PALETTE_EXECUTOR_VTABLE`'s all-`None` shape below) — it runs no code
/// of its own.
pub const PACKAGES_ROOT_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 7, 0); // V3.1 ADR-016
const VGA_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.shell");
pub const THEME_EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.theme");
pub const CLIPBOARD_EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.clip");
pub const PALETTE_EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pal"); // V2.62
pub const PACKAGES_EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pkg"); // V3.1 ADR-016
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(shell_on_init),
    on_event: Some(shell_on_event),
    on_suspend: Some(shell_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};
pub const THEME_EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: THEME_EXECUTOR_ID,
    on_init: None,
    on_event: None,
    on_suspend: Some(shell_on_suspend),
    on_resume: Some(theme_on_resume),
    on_teardown: None,
    on_telemetry: None,
};
pub const CLIPBOARD_EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: CLIPBOARD_EXECUTOR_ID,
    on_init: Some(clipboard_on_init),
    on_event: Some(clipboard_on_event),
    on_suspend: Some(shell_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};
// V2.62: passive data-store executor for CYAN/GOLD palette nodes — no event handlers needed.
pub const PALETTE_EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: PALETTE_EXECUTOR_ID,
    on_init: None,
    on_event: None,
    on_suspend: None,
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};
pub const PACKAGES_EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: PACKAGES_EXECUTOR_ID,
    on_init: None,
    on_event: None,
    on_suspend: None,
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

const SCREEN_WIDTH: usize = 80;
const STAGE_COUNT: usize = 5;
const PULSE_COUNT: usize = 3;
const FRAME_COUNT: usize = STAGE_COUNT * PULSE_COUNT;
const EVENT_LINES: usize = 4;
const LIVE_SIGIL_FRAMES: usize = 12;
/// Heartbeat ticks (120 Hz) between watch command replays ≈ 2 seconds.
pub const WATCH_INTERVAL_TICKS: u8 = 240;
const COMMAND_DECK_TOP: usize = 2;
const COMMAND_DECK_LEFT: usize = 2;
const COMMAND_DECK_WIDTH: usize = 47;
const COMMAND_DECK_HEIGHT: usize = 10;
const AI_PANEL_TOP: usize = 2;
const AI_PANEL_LEFT: usize = 52;
const AI_PANEL_WIDTH: usize = 26;
const AI_PANEL_HEIGHT: usize = 12;
const AI_PANEL_LINES: usize = 4;
const AI_PANEL_LINE_WIDTH: usize = 22;
const LIVE_SIGIL_TOP: usize = 5;
const LIVE_SIGIL_LEFT: usize = 50;
const LIVE_SIGIL_WIDTH: usize = 3;
const LIVE_SIGIL_HEIGHT: usize = 5;
const GRAPH_PAGE_ITEMS: usize = 6;
const GRAPH_OVERVIEW_ITEMS: usize = 3;
const GRAPH_VIEW_TITLE_ROW: usize = COMMAND_SCROLL_TOP;
const GRAPH_VIEW_FIRST_ITEM_ROW: usize = COMMAND_SCROLL_TOP + 1;
const GRAPH_VIEW_FOOT_ROW: usize = COMMAND_SCROLL_BOTTOM;
const COMMAND_SCROLL_TOP: usize = 14;
const COMMAND_SCROLL_BOTTOM: usize = 21;
const FOOTER_SHORTCUT_ROW: usize = 22;
const FOOTER_STATUS_ROW: usize = 23;
const FOOTER_INPUT_ROW: usize = 24;
const MENU_MODE_COMMAND: u8 = 0;
const MENU_MODE_AI_API: u8 = 1;
const GRAPH_MODE_NONE: u8 = 0;
const GRAPH_MODE_OVERVIEW: u8 = 1;
const GRAPH_MODE_NODE_LIST: u8 = 2;
const GRAPH_MODE_EDGE_LIST: u8 = 3;
const GRAPH_MODE_NODE_DETAIL: u8 = 4;
const GRAPH_MODE_EDGE_DETAIL: u8 = 5;
const GRAPH_MODE_INFO: u8 = 6;
const GRAPH_CTX_NONE: u8 = 0;
const GRAPH_CTX_OVERVIEW: u8 = 1;
const GRAPH_CTX_NODE: u8 = 2;
const GRAPH_CTX_EDGE: u8 = 3;
const GRAPH_CTX_METRICS: u8 = 4;
const MAX_IME_PREVIEW: usize = 24;
const GRAPH_NAV_DEPTH: usize = 8;
const COMMAND_HISTORY_ITEMS: usize = 16;

const CP437_LIGHT: u8 = 176;
const CP437_MEDIUM: u8 = 177;
const CP437_DARK: u8 = 178;
const CP437_BLOCK: u8 = 219;
const CP437_HLINE: u8 = 205;
const CP437_VLINE: u8 = 186;
const CP437_TL: u8 = 201;
const CP437_TR: u8 = 187;
const CP437_BL: u8 = 200;
const CP437_BR: u8 = 188;

const WABI_INK: u8 = 0;
const WABI_INDIGO: u8 = 1;
const WABI_MOSS: u8 = 2;
const WABI_STONE: u8 = 8;
const WABI_PAPER: u8 = 7;
const WABI_MOON: u8 = 15;
const WABI_TEA: u8 = 6;
const WABI_SAGE: u8 = 10;
const CLIPBOARD_EDGE_KEY: &str = "clipboard.mount";
const THEME_NAME_WABI: &str = "wabi";
const THEME_NAME_SHOJI: &str = "shoji";
const THEME_KIND_WABI: u8 = DISPLAY_THEME_WABI;
const THEME_KIND_SHOJI: u8 = DISPLAY_THEME_SHOJI;
const CLIPBOARD_MAX_BYTES: usize = 224;

const SHELL_PLUGIN_ID: PluginId = PluginId::from_ascii("K_SHELL");
const THEME_WABI_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "theme.wabi");
const THEME_SHOJI_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "theme.shoji");
const THEME_CURRENT_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "theme.current");
const CLIPBOARD_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "clipboard.mount");
const PALETTE_CYAN_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "palette.cyan"); // V2.62
const PALETTE_GOLD_NODE_ID: gos_protocol::NodeId = derive_node_id(SHELL_PLUGIN_ID, "palette.gold"); // V2.62

static ACTIVE_THEME: AtomicU8 = AtomicU8::new(THEME_KIND_WABI);
static CLIPBOARD_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Resolved vector address of the k-chat node (0 = not available).
static CHAT_TARGET: AtomicU64 = AtomicU64::new(0);
/// 0 = normal shell, 1 = chat mode.
static CHAT_MODE: AtomicU8 = AtomicU8::new(0);
/// 0 = COM2 bridge mode, 1 = direct TCP/HTTP mode.
static CHAT_HTTP_MODE: AtomicU8 = AtomicU8::new(0);
/// Resolved vector address of the k-nim node (0 = not available).
static NIM_TARGET: AtomicU64 = AtomicU64::new(0);
/// 0 = normal shell, 1 = NIM inference mode.
static NIM_MODE: AtomicU8 = AtomicU8::new(0);
/// Resolved vector address of the k-virtio-gpu node (0 = not available).
/// ADR-013 §选项A discovery skeleton — see `gpu status`.
static GPU_TARGET: AtomicU64 = AtomicU64::new(0);
/// Pinned graph epoch for `graph diff` — 0 means "since boot", any other value
/// means "since epoch N was pinned via `graph diff pin`".
pub(crate) static GRAPH_DIFF_PIN_EPOCH: AtomicU64 = AtomicU64::new(0);
/// 0 = normal VECTOR DECK view, 1 = live proc watch mode (like `watch -n1 proc`).
pub(crate) static WATCH_PROC_MODE: AtomicU8 = AtomicU8::new(0);

const BOOT_PHASES: [&str; STAGE_COUNT] = [
    "DISCOVER",
    "DEPEND",
    "ARENA",
    "SYNC",
    "HANDOFF",
];

const BOOT_COPY: [&str; STAGE_COUNT] = [
    "manifest mesh entering sensor range",
    "capability routes and plugin edges are locking in",
    "stable node identity mapped onto page-aligned arenas",
    "control-plane mirror is absorbing graph deltas",
    "shell focus granted to the live command surface",
];

const BOOT_EVENTS: [[&str; EVENT_LINES]; STAGE_COUNT] = [
    ["bundle sweep live", "abi gate green", "entry nodes armed", "graph census warm"],
    ["depend edges fused", "imports resolved", "legacy sync active", "permits authorized"],
    ["arena pages carved", "stable ids rebound", "adjacency mesh wide", "registry map locked"],
    ["delta mirror live", "snapshot telemetry", "policy gate intact", "advice stays soft"],
    ["shell node focused", "command deck live", "startup mesh calm", "awaiting operator"],
];

const STARFIELD: [(usize, usize); 28] = [
    (1, 3), (1, 18), (1, 32), (1, 47), (1, 63), (1, 75),
    (3, 6), (4, 74), (5, 22), (6, 52), (7, 11), (7, 70),
    (9, 4), (10, 34), (11, 19), (12, 49), (13, 8), (13, 72),
    (15, 26), (16, 68), (18, 12), (19, 57), (20, 7), (20, 73),
    (22, 17), (22, 41), (23, 5), (23, 70),
];

const ORBIT_POINTS: [(usize, usize); 14] = [
    (4, 37), (4, 43), (5, 48), (7, 52), (10, 52), (13, 48), (14, 43),
    (14, 36), (13, 31), (10, 28), (7, 28), (5, 31), (8, 50), (9, 30),
];

const LIVE_SIGIL_ROWS: [[u8; 1]; 1] = [[b'G']];

const LIVE_SHAKE_X: [i8; LIVE_SIGIL_FRAMES] = [0, 0, 1, 0, -1, 0, 1, 0, -1, 0, 0, 0];
const LIVE_SHAKE_Y: [i8; LIVE_SIGIL_FRAMES] = [0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1];
const LIVE_SPARKS: [[(i8, i8); 4]; LIVE_SIGIL_FRAMES] = [
    [(-1, -1), (0, 1), (1, -1), (2, 0)],
    [(-1, 0), (0, 1), (1, 1), (2, 0)],
    [(-1, 1), (0, 0), (1, 1), (2, -1)],
    [(0, 1), (1, -1), (2, 0), (1, 1)],
    [(1, 1), (2, 0), (1, -1), (0, -1)],
    [(2, 0), (1, -1), (0, 0), (-1, -1)],
    [(1, -1), (0, -1), (-1, 0), (0, 1)],
    [(0, -1), (-1, 0), (0, 1), (1, 1)],
    [(-1, 0), (0, 1), (1, 0), (2, 1)],
    [(0, 1), (1, 0), (2, -1), (1, -1)],
    [(1, 0), (2, -1), (1, -1), (0, 0)],
    [(0, -1), (1, -1), (2, 0), (1, 1)],
];
const BOOT_WOBBLE_X: [i32; LIVE_SIGIL_FRAMES] = [0, 1, -1, 2, -2, 1, -1, 0, 1, -1, 0, 0];
const BOOT_WOBBLE_Y: [i32; LIVE_SIGIL_FRAMES] = [0, 0, 1, -1, 1, -1, 0, 1, -1, 0, 0, 0];

#[derive(Clone, Copy, PartialEq, Eq)]
struct GraphNavState {
    selected_node: Option<VectorAddress>,
    selected_edge: Option<EdgeVector>,
    graph_mode: u8,
    graph_context: u8,
    graph_offset: usize,
    graph_total: usize,
}

impl GraphNavState {
    const EMPTY: Self = Self {
        selected_node: None,
        selected_edge: None,
        graph_mode: GRAPH_MODE_NONE,
        graph_context: GRAPH_CTX_NONE,
        graph_offset: 0,
        graph_total: 0,
    };
}

#[repr(C)]
struct ShellState {
    buffer: [u8; 128],
    len: usize,
    command_history: [[u8; 128]; COMMAND_HISTORY_ITEMS],
    command_history_lens: [usize; COMMAND_HISTORY_ITEMS],
    command_history_len: usize,
    command_history_cursor: usize,
    command_history_active: u8,
    command_history_draft: [u8; 128],
    command_history_draft_len: usize,
    selected_node: Option<VectorAddress>,
    selected_edge: Option<EdgeVector>,
    graph_mode: u8,
    graph_context: u8,
    graph_offset: usize,
    graph_total: usize,
    graph_nav: [GraphNavState; GRAPH_NAV_DEPTH],
    graph_nav_len: usize,
    ai_lines: [[u8; AI_PANEL_LINE_WIDTH]; AI_PANEL_LINES],
    ai_line_lens: [u8; AI_PANEL_LINES],
    ai_stream: [u8; AI_PANEL_LINE_WIDTH],
    ai_stream_len: u8,
    ime_preview: [u8; MAX_IME_PREVIEW],
    ime_preview_len: usize,
    ime_utf8_tail: u8,
    api_buffer: [u8; 128],
    api_edit_len: usize,
    api_len: usize,
    console_target: u64,
    ime_target: u64,
    ai_target: u64,
    cypher_target: u64,
    net_target: u64,
    cuda_target: u64,
    clipboard_target: u64,
    /// graph_epoch at the last draw_command_deck_panel call; enables the
    /// V2.3 epoch-diff idle skip (zero unnecessary panel repaints).
    last_rendered_epoch: u64,
    console_live: u8,
    sigil_frame: u8,
    heartbeat_divider: u8,
    menu_mode: u8,
    input_lang: u8,
    api_configured: u8,
    /// Non-zero when `watch` mode is active.
    watch_active: u8,
    /// Heartbeat ticks since last watch re-run (resets at WATCH_INTERVAL_TICKS).
    watch_tick: u8,
    /// Length of the command stored in watch_buf.
    watch_buf_len: u8,
    /// The command to re-run in watch mode (NUL-padded).
    watch_buf: [u8; 64],
}

#[repr(C)]
struct ClipboardState {
    bytes: [u8; CLIPBOARD_MAX_BYTES],
    len: usize,
    capture_from: u64,
    capture_len: usize,
    capture_active: u8,
}

#[derive(Clone, Copy)]
struct ConsoleSink {
    target: u64,
    from: u64,
    abi: &'static KernelAbi,
}

impl ConsoleSink {
    fn emit(&self, signal: Signal) {
        if let Some(emit_signal) = self.abi.emit_signal {
            unsafe {
                let _ = emit_signal(self.target, signal_to_packet(signal));
            }
        }
    }
}

struct LineBuf<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> LineBuf<N> {
    fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    fn push_byte(&mut self, byte: u8) {
        if self.len < N {
            self.bytes[self.len] = byte;
            self.len += 1;
        }
    }

    fn push_str(&mut self, text: &str) {
        for byte in text.bytes() {
            self.push_byte(byte);
        }
    }

    fn push_slice(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.push_byte(*byte);
        }
    }

    fn push_dec(&mut self, mut value: u64) {
        let mut buf = [0u8; 20];
        let mut len = 0usize;
        if value == 0 {
            self.push_byte(b'0');
            return;
        }
        while value > 0 {
            buf[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
        while len > 0 {
            len -= 1;
            self.push_byte(buf[len]);
        }
    }

    fn push_hex(&mut self, mut value: u64) {
        let mut buf = [0u8; 16];
        let mut len = 0usize;
        if value == 0 {
            self.push_byte(b'0');
            return;
        }
        while value > 0 {
            let nibble = (value & 0xF) as u8;
            buf[len] = if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + (nibble - 10)
            };
            value >>= 4;
            len += 1;
        }
        while len > 0 {
            len -= 1;
            self.push_byte(buf[len]);
        }
    }

    fn push_fixed_ascii(&mut self, bytes: &[u8; 16]) {
        let mut len = 0usize;
        while len < bytes.len() && bytes[len] != 0 {
            len += 1;
        }
        self.push_slice(&bytes[..len]);
    }

    fn push_vector(&mut self, vector: VectorAddress) {
        self.push_dec(vector.l4 as u64);
        self.push_byte(b'.');
        self.push_dec(vector.l3 as u64);
        self.push_byte(b'.');
        self.push_dec(vector.l2 as u64);
        self.push_byte(b'.');
        self.push_dec(vector.offset as u64);
    }

    fn push_edge_vector(&mut self, vector: EdgeVector) {
        self.push_str("e:");
        self.push_dec(vector.l4 as u64);
        self.push_byte(b'.');
        self.push_dec(vector.l3 as u64);
        self.push_byte(b'.');
        self.push_dec(vector.l2 as u64);
        self.push_byte(b'.');
        self.push_dec(vector.offset as u64);
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn len(&self) -> usize {
        self.len
    }
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut ShellState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut ShellState) }
}

fn sink_from_ctx(ctx: *mut ExecutorContext) -> ConsoleSink {
    let ctx_ref = unsafe { &*ctx };
    let abi = unsafe { &*ctx_ref.abi };
    let state = unsafe { state_mut(ctx) };
    ConsoleSink {
        target: if state.console_target == 0 {
            VGA_VEC.as_u64()
        } else {
            state.console_target
        },
        from: ctx_ref.vector.as_u64(),
        abi,
    }
}

fn emit_vga(sink: &ConsoleSink, signal: Signal) {
    sink.emit(signal);
}

fn send_ctrl(sink: &ConsoleSink, cmd: u8, val: u8) {
    emit_vga(sink, Signal::Control { cmd, val });
}

fn goto(sink: &ConsoleSink, row: usize, col: usize) {
    send_ctrl(sink, 5, row as u8);
    send_ctrl(sink, 6, col as u8);
}

fn clear_canvas(sink: &ConsoleSink) {
    send_ctrl(sink, 7, 0);
}

fn save_cursor(sink: &ConsoleSink, slot: u8) {
    send_ctrl(sink, 9, slot);
}

fn restore_cursor(sink: &ConsoleSink, slot: u8) {
    send_ctrl(sink, 10, slot);
}

fn set_scroll_top(sink: &ConsoleSink, row: usize) {
    send_ctrl(sink, 11, row as u8);
}

fn set_scroll_bottom(sink: &ConsoleSink, row: usize) {
    send_ctrl(sink, 12, row as u8);
}

fn print_byte(sink: &ConsoleSink, byte: u8) {
    emit_vga(sink, Signal::Data { from: sink.from, byte });
}

fn print_str(sink: &ConsoleSink, s: &str) {
    for byte in s.bytes() {
        print_byte(sink, byte);
    }
}

fn emit_target_signal(sink: &ConsoleSink, target: u64, signal: Signal) -> bool {
    emit_target_signal_raw(sink.abi, target, signal)
}

fn emit_target_signal_raw(abi: &KernelAbi, target: u64, signal: Signal) -> bool {
    if target == 0 {
        return false;
    }
    if let Some(emit_signal) = abi.emit_signal {
        unsafe { emit_signal(target, signal_to_packet(signal)) == 0 }
    } else {
        false
    }
}

fn theme_name(theme: u8) -> &'static str {
    match theme {
        THEME_KIND_SHOJI => THEME_NAME_SHOJI,
        _ => THEME_NAME_WABI,
    }
}

fn current_theme() -> u8 {
    ACTIVE_THEME.load(Ordering::SeqCst)
}

fn is_theme_vector(vector: VectorAddress) -> bool {
    vector == THEME_CURRENT_NODE_VEC || vector == THEME_WABI_NODE_VEC || vector == THEME_SHOJI_NODE_VEC
}

fn theme_kind_for_vector(vector: VectorAddress) -> Option<u8> {
    if vector == THEME_WABI_NODE_VEC {
        Some(THEME_KIND_WABI)
    } else if vector == THEME_SHOJI_NODE_VEC {
        Some(THEME_KIND_SHOJI)
    } else {
        None
    }
}

fn theme_vector(theme: u8) -> VectorAddress {
    match theme {
        THEME_KIND_SHOJI => THEME_SHOJI_NODE_VEC,
        _ => THEME_WABI_NODE_VEC,
    }
}

fn theme_node_id(theme: u8) -> gos_protocol::NodeId {
    match theme {
        THEME_KIND_SHOJI => THEME_SHOJI_NODE_ID,
        _ => THEME_WABI_NODE_ID,
    }
}

fn linked_theme_kind() -> Option<u8> {
    let mut edges = [GraphEdgeSummary::EMPTY; 4];
    let Ok((_total, returned)) = gos_runtime::edge_page_for_node(THEME_CURRENT_NODE_VEC, 0, &mut edges) else {
        return None;
    };

    for summary in edges.iter().take(returned) {
        if summary.edge_type != RuntimeEdgeType::Use || summary.from_vector != THEME_CURRENT_NODE_VEC {
            continue;
        }
        if let Some(theme) = theme_kind_for_vector(summary.to_vector) {
            return Some(theme);
        }
    }

    None
}

fn selected_theme() -> u8 {
    linked_theme_kind().unwrap_or_else(current_theme)
}

fn clipboard_len() -> usize {
    CLIPBOARD_BYTES.load(Ordering::SeqCst)
}

fn node_has_mount_edge(source: VectorAddress, target: VectorAddress) -> bool {
    let mut edges = [GraphEdgeSummary::EMPTY; 12];
    let Ok((_total, returned)) = gos_runtime::edge_page_for_node(source, 0, &mut edges) else {
        return false;
    };

    for summary in edges.iter().take(returned) {
        if summary.edge_type == RuntimeEdgeType::Mount
            && summary.from_vector == source
            && summary.to_vector == target
        {
            return true;
        }
    }

    false
}

fn clipboard_mounted(source: VectorAddress) -> bool {
    node_has_mount_edge(source, CLIPBOARD_NODE_VEC)
}

fn sync_clipboard_mount_for_vector(source: VectorAddress, mounted: bool) -> bool {
    let Some(source_node) = gos_runtime::node_id_for_vec(source) else {
        return false;
    };

    let edge_id = derive_edge_id(source_node, CLIPBOARD_NODE_ID, CLIPBOARD_EDGE_KEY);
    if !mounted {
        return gos_runtime::unregister_edge(edge_id).is_ok()
            || !node_has_mount_edge(source, CLIPBOARD_NODE_VEC);
    }

    gos_runtime::register_edge(EdgeSpec {
        edge_id,
        from_node: source_node,
        to_node: CLIPBOARD_NODE_ID,
        edge_type: RuntimeEdgeType::Mount,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: Some("clipboard"),
        capability_binding: Some("buffer"),
        vector_ref: None,
    })
    .is_ok()
}

fn clipboard_clear(sink: &ConsoleSink, target: u64) -> bool {
    if !clipboard_mounted(VectorAddress::from_u64(sink.from)) {
        return false;
    }
    emit_target_signal(
        sink,
        target,
        Signal::Data {
            from: sink.from,
            byte: CLIPBOARD_DATA_CLEAR,
        },
    )
}

fn clipboard_store(sink: &ConsoleSink, target: u64, bytes: &[u8]) -> bool {
    if !clipboard_mounted(VectorAddress::from_u64(sink.from)) {
        return false;
    }
    if !emit_target_signal(
        sink,
        target,
        Signal::Data {
            from: sink.from,
            byte: CLIPBOARD_DATA_BEGIN,
        },
    ) {
        return false;
    }

    for byte in bytes.iter().copied() {
        if !emit_target_signal(sink, target, Signal::Data { from: sink.from, byte }) {
            return false;
        }
    }

    emit_target_signal(
        sink,
        target,
        Signal::Data {
            from: sink.from,
            byte: CLIPBOARD_DATA_COMMIT,
        },
    )
}

fn clipboard_request_paste(sink: &ConsoleSink, target: u64) -> bool {
    if !clipboard_mounted(VectorAddress::from_u64(sink.from)) {
        return false;
    }
    emit_target_signal(sink, target, Signal::Call { from: sink.from })
}

fn active_input_len(state: &ShellState) -> usize {
    if state.menu_mode == MENU_MODE_AI_API {
        state.api_edit_len
    } else {
        state.len
    }
}

fn clipboard_copy_active_input(sink: &ConsoleSink, state: &mut ShellState) -> bool {
    if state.clipboard_target == 0 || !clipboard_mounted(NODE_VEC) {
        return false;
    }

    let active_len = active_input_len(state);
    if state.menu_mode == MENU_MODE_AI_API {
        clipboard_store(sink, state.clipboard_target, &state.api_buffer[..active_len])
    } else {
        clipboard_store(sink, state.clipboard_target, &state.buffer[..active_len])
    }
}

fn clipboard_cut_active_input(sink: &ConsoleSink, state: &mut ShellState) -> bool {
    if !clipboard_copy_active_input(sink, state) {
        return false;
    }

    if state.menu_mode == MENU_MODE_AI_API {
        state.api_buffer = [0; 128];
        state.api_edit_len = 0;
    } else {
        state.buffer = [0; 128];
        state.len = 0;
        state.ime_utf8_tail = 0;
        clear_ime_preview(state);
    }
    reset_command_history_cursor(state);
    redraw_footer(sink, state, false);
    focus_footer_input(sink, state);
    true
}

fn clipboard_paste_active_input(sink: &ConsoleSink, state: &mut ShellState) -> bool {
    if state.clipboard_target == 0 || !clipboard_mounted(NODE_VEC) {
        return false;
    }
    clipboard_request_paste(sink, state.clipboard_target)
}

fn append_api_edit_byte(state: &mut ShellState, byte: u8) {
    if state.api_edit_len < state.api_buffer.len() {
        state.api_buffer[state.api_edit_len] = byte;
        state.api_edit_len += 1;
    }
    reset_command_history_cursor(state);
}

fn append_clipboard_byte(sink: &ConsoleSink, state: &mut ShellState, byte: u8) {
    if state.menu_mode == MENU_MODE_AI_API {
        append_api_edit_byte(state, byte);
        redraw_footer(sink, state, false);
        return;
    }

    if state.ime_preview_len > 0 {
        clear_ime_preview(state);
    }
    append_command_byte(sink, state, byte, false);
}

/// Repoint `theme.current`'s exclusive `Use` edge to the chosen theme.
///
/// Uses the atomic single-epoch `rebind_use` (ADR-004 §2.2): it removes any
/// existing `Use` edge sourced at `theme.current` and inserts the new one under
/// **one** `graph_epoch` bump, so no reader (e.g. the desktop renderer) ever
/// observes `theme.current` holding zero `Use` edges mid-switch. This replaces
/// the former `unregister`×2 + `register` dance, which bumped the epoch three
/// times with zero-`Use` windows in between. Edge identity is now the unified
/// `"use"` key (the rebind scans by source+type to remove, so it also cleans up
/// any pre-rebind `"theme.use"`-keyed edge seeded before this change).
fn sync_theme_use_edges(theme: u8) -> bool {
    gos_runtime::rebind_use(THEME_CURRENT_NODE_ID, theme_node_id(theme)).is_ok()
}

fn apply_theme_choice_raw(_abi: &KernelAbi, from: u64, _console_target: u64, theme: u8) -> bool {
    // V2.15: sync_theme_use_edges triggers fire_subscribers → Subscribe signal
    // delivered to k-vga automatically; explicit DISPLAY_CONTROL_THEME removed.
    let graph_ok = sync_theme_use_edges(theme);
    ACTIVE_THEME.store(theme, Ordering::SeqCst);
    if from != 0 && from != NODE_VEC.as_u64() {
        let _ = gos_runtime::post_signal(NODE_VEC, Signal::Interrupt { irq: 32 });
    }
    graph_ok
}

fn apply_theme_choice(sink: &ConsoleSink, theme: u8) -> bool {
    apply_theme_choice_raw(sink.abi, sink.from, sink.target, theme)
}

/// ADR-016 option A — `gpm install <name>`: mint a provisional node
/// representing the package (`CypherMutation::CreateNode`, ADR-005 option
/// A) and `Mount` it under [`PACKAGES_ROOT_NODE_VEC`], so installed
/// packages are discoverable as one hub's `Mount` out-edges. The Mount
/// edge is stamped `b"K_GPM"` and goes through the exact same
/// `gos-cypher-mut` gate a human (`b"K_SHELL"`) or AI (`b"K_AI"`) mutation
/// uses — `gpm` isn't a second privileged path (V3 Parity invariant).
///
/// `manifest.cypher v0`'s capability declarations (`IMPORTS`/`EXPORTS`/
/// `DEPENDS_ON`) are explicitly **not** wired here — they stay `//`-style
/// prose (same convention as `k-net`'s own header comment) until a future
/// option-B slice extends `ReceptiveEdgeKind` to cover them (ADR-016 §二
/// option B). `name` itself isn't stored anywhere yet either (`CreateNode`
/// doesn't carry a `{props}` payload — V2.5e's own documented limitation)
/// — this v0 proves the graph shape, not a full package registry.
///
/// Returns the new package node's id on success.
pub fn gpm_install_raw(_name: &str) -> Option<gos_protocol::NodeId> {
    use gos_cypher_mut::{apply_mutation, CypherMutation, MutationDispatcher};

    // CreateNode bypasses the audited-envelope gate the same way
    // k-cypher's own `CREATE (n)` shell verb does: apply_mutation's
    // CreateNode arm has no `EdgeId` to hand a caller and no `{props}`
    // payload to audit yet (see this fn's own doc comment above).
    let mut dispatcher = gos_runtime::RuntimeDispatcher;
    let pkg_id = apply_mutation(&mut dispatcher, CypherMutation::CreateNode).ok()??;

    const GPM_SOURCE: [u8; 16] = *b"K_GPM\0\0\0\0\0\0\0\0\0\0\0";
    let root_id = gos_runtime::node_id_for_vec(PACKAGES_ROOT_NODE_VEC)?;
    gos_supervisor::apply_cypher_mutation(
        gos_cypher_mut::CypherMutation::AddEdge {
            from: pkg_id,
            to: root_id,
            edge_kind: gos_cypher_mut::ReceptiveEdgeKind::Mount,
        },
        GPM_SOURCE,
    )
    .ok()?;
    Some(pkg_id)
}

/// ADR-016 option A — `gpm list`: walk [`PACKAGES_ROOT_NODE_VEC`]'s `Mount`
/// out-edges. Read-only, not a mutation, so it doesn't go through the
/// cypher-mut gate — mirrors how `theme`/`clipboard status` are plain
/// reads over `edge_page_for_node`.
pub fn gpm_list_raw(out: &mut [gos_protocol::GraphEdgeSummary]) -> usize {
    let mut buf = [gos_protocol::GraphEdgeSummary::EMPTY; 64];
    let (_total, returned) = match gos_runtime::edge_page_for_node::<64>(PACKAGES_ROOT_NODE_VEC, 0, &mut buf) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let n = returned.min(out.len());
    out[..n].copy_from_slice(&buf[..n]);
    n
}

fn parse_theme_selector(cmd: &str) -> Option<u8> {
    match cmd.trim() {
        THEME_NAME_WABI | "sabi" | "theme.wabi" | "6.1.1.0" => Some(THEME_KIND_WABI),
        THEME_NAME_SHOJI | "miyabi" | "theme.shoji" | "6.1.2.0" => Some(THEME_KIND_SHOJI),
        _ => None,
    }
}

fn parse_clipboard_vector(cmd: &str) -> Option<VectorAddress> {
    VectorAddress::parse(cmd.trim())
}

fn set_command_buffer(state: &mut ShellState, bytes: &[u8]) {
    state.buffer = [0; 128];
    let len = bytes.len().min(state.buffer.len());
    if len > 0 {
        state.buffer[..len].copy_from_slice(&bytes[..len]);
    }
    state.len = len;
    state.ime_utf8_tail = 0;
    clear_ime_preview(state);
}

fn reset_command_history_cursor(state: &mut ShellState) {
    state.command_history_active = 0;
    state.command_history_cursor = state.command_history_len;
    state.command_history_draft = [0; 128];
    state.command_history_draft_len = 0;
}

fn command_history_prev(state: &mut ShellState) -> bool {
    if state.command_history_len == 0 {
        return false;
    }

    if state.command_history_active == 0 {
        state.command_history_draft = [0; 128];
        state.command_history_draft[..state.len].copy_from_slice(&state.buffer[..state.len]);
        state.command_history_draft_len = state.len;
        state.command_history_cursor = state.command_history_len;
        state.command_history_active = 1;
    }

    if state.command_history_cursor == 0 {
        return true;
    }

    state.command_history_cursor -= 1;
    let idx = state.command_history_cursor;
    let len = state.command_history_lens[idx].min(state.command_history[idx].len());
    let entry = state.command_history[idx];
    set_command_buffer(state, &entry[..len]);
    true
}

fn command_history_next(state: &mut ShellState) -> bool {
    if state.command_history_active == 0 {
        return false;
    }

    if state.command_history_cursor + 1 < state.command_history_len {
        state.command_history_cursor += 1;
        let idx = state.command_history_cursor;
        let len = state.command_history_lens[idx].min(state.command_history[idx].len());
        let entry = state.command_history[idx];
        set_command_buffer(state, &entry[..len]);
    } else {
        let draft_len = state.command_history_draft_len.min(state.command_history_draft.len());
        let draft = state.command_history_draft;
        set_command_buffer(state, &draft[..draft_len]);
        reset_command_history_cursor(state);
    }

    true
}

fn record_command_history(state: &mut ShellState) {
    if state.len == 0 {
        reset_command_history_cursor(state);
        return;
    }

    if state.command_history_len > 0 {
        let last_idx = state.command_history_len - 1;
        let last_len = state.command_history_lens[last_idx];
        if last_len == state.len
            && state.command_history[last_idx][..last_len] == state.buffer[..state.len]
        {
            reset_command_history_cursor(state);
            return;
        }
    }

    if state.command_history_len == COMMAND_HISTORY_ITEMS {
        let mut idx = 1usize;
        while idx < COMMAND_HISTORY_ITEMS {
            state.command_history[idx - 1] = state.command_history[idx];
            state.command_history_lens[idx - 1] = state.command_history_lens[idx];
            idx += 1;
        }
        state.command_history_len = COMMAND_HISTORY_ITEMS - 1;
    }

    let slot = state.command_history_len;
    state.command_history[slot] = [0; 128];
    state.command_history[slot][..state.len].copy_from_slice(&state.buffer[..state.len]);
    state.command_history_lens[slot] = state.len;
    state.command_history_len += 1;
    reset_command_history_cursor(state);
}

fn command_pop_scalar(state: &mut ShellState) -> bool {
    if state.len == 0 {
        return false;
    }

    let mut idx = state.len - 1;
    while idx > 0 && (state.buffer[idx] & 0xC0) == 0x80 {
        idx -= 1;
    }
    state.len = idx;
    true
}

fn utf8_tail_len(byte: u8) -> u8 {
    if (byte & 0xE0) == 0xC0 {
        1
    } else if (byte & 0xF0) == 0xE0 {
        2
    } else if (byte & 0xF8) == 0xF0 {
        3
    } else {
        0
    }
}

fn append_command_byte(sink: &ConsoleSink, state: &mut ShellState, byte: u8, from_ime: bool) {
    reset_command_history_cursor(state);
    if state.len < state.buffer.len() {
        state.buffer[state.len] = byte;
        state.len += 1;
    }

    state.ime_utf8_tail = if from_ime && !byte.is_ascii() {
        if (byte & 0xC0) == 0x80 {
            state.ime_utf8_tail.saturating_sub(1)
        } else {
            utf8_tail_len(byte)
        }
    } else {
        0
    };
    redraw_footer(sink, state, false);
    focus_footer_input(sink, state);
}

fn clear_ime_preview(state: &mut ShellState) {
    state.ime_preview = [0; MAX_IME_PREVIEW];
    state.ime_preview_len = 0;
}

fn clear_ai_panel(state: &mut ShellState) {
    state.ai_lines = [[0; AI_PANEL_LINE_WIDTH]; AI_PANEL_LINES];
    state.ai_line_lens = [0; AI_PANEL_LINES];
    state.ai_stream = [0; AI_PANEL_LINE_WIDTH];
    state.ai_stream_len = 0;
}

fn clear_graph_nav(state: &mut ShellState) {
    state.graph_nav = [GraphNavState::EMPTY; GRAPH_NAV_DEPTH];
    state.graph_nav_len = 0;
}

fn current_graph_nav_state(state: &ShellState) -> GraphNavState {
    GraphNavState {
        selected_node: state.selected_node,
        selected_edge: state.selected_edge,
        graph_mode: state.graph_mode,
        graph_context: state.graph_context,
        graph_offset: state.graph_offset,
        graph_total: state.graph_total,
    }
}

fn push_graph_nav_state(state: &mut ShellState) {
    let snapshot = current_graph_nav_state(state);
    if state.graph_nav_len > 0 && state.graph_nav[state.graph_nav_len - 1] == snapshot {
        return;
    }
    if state.graph_nav_len == GRAPH_NAV_DEPTH {
        for idx in 1..GRAPH_NAV_DEPTH {
            state.graph_nav[idx - 1] = state.graph_nav[idx];
        }
        state.graph_nav_len = GRAPH_NAV_DEPTH - 1;
        state.graph_nav[state.graph_nav_len] = GraphNavState::EMPTY;
    }
    state.graph_nav[state.graph_nav_len] = snapshot;
    state.graph_nav_len += 1;
}

fn pop_graph_nav_state(state: &mut ShellState) -> Option<GraphNavState> {
    if state.graph_nav_len == 0 {
        return None;
    }
    state.graph_nav_len -= 1;
    let snapshot = state.graph_nav[state.graph_nav_len];
    state.graph_nav[state.graph_nav_len] = GraphNavState::EMPTY;
    Some(snapshot)
}

fn clear_graph_selection(state: &mut ShellState) {
    state.selected_node = None;
    state.selected_edge = None;
    state.graph_mode = GRAPH_MODE_NONE;
    state.graph_context = GRAPH_CTX_NONE;
    state.graph_offset = 0;
    state.graph_total = 0;
    clear_graph_nav(state);
}

fn node_type_label(node_type: gos_protocol::RuntimeNodeType) -> &'static str {
    match node_type {
        gos_protocol::RuntimeNodeType::Hardware => "hw",
        gos_protocol::RuntimeNodeType::Driver => "drv",
        gos_protocol::RuntimeNodeType::Service => "svc",
        gos_protocol::RuntimeNodeType::PluginEntry => "entry",
        gos_protocol::RuntimeNodeType::Compute => "compute",
        gos_protocol::RuntimeNodeType::Router => "router",
        gos_protocol::RuntimeNodeType::Aggregator => "agg",
        gos_protocol::RuntimeNodeType::Vector => "vector",
    }
}

fn lifecycle_label(state: gos_protocol::NodeLifecycle) -> &'static str {
    match state {
        gos_protocol::NodeLifecycle::Discovered => "discover",
        gos_protocol::NodeLifecycle::Loaded => "loaded",
        gos_protocol::NodeLifecycle::Registered => "register",
        gos_protocol::NodeLifecycle::Allocated => "alloc",
        gos_protocol::NodeLifecycle::Ready => "ready",
        gos_protocol::NodeLifecycle::Running => "run",
        gos_protocol::NodeLifecycle::Waiting => "wait",
        gos_protocol::NodeLifecycle::Suspended => "suspend",
        gos_protocol::NodeLifecycle::Terminated => "term",
        gos_protocol::NodeLifecycle::Faulted => "fault",
    }
}

/// Text-mode listing of all live graph nodes — the GOS equivalent of `ps`.
///
/// Prints each node's vector, plugin/key, and lifecycle to the scrolling console.
/// When `faulted_only` is true, only nodes in `NodeLifecycle::Faulted` are shown.
/// Color-codes lifecycle: green = ready/running, yellow = waiting/suspended,
/// red = faulted, gray = boot-phase (discovered/loaded/registered/allocated).
pub fn dispatch_nodes_list(sink: &ConsoleSink, faulted_only: bool) {
    use gos_protocol::{GraphNodeSummary, NodeLifecycle};
    const PAGE: usize = 8;
    let mut items = [GraphNodeSummary::EMPTY; PAGE];
    let mut offset = 0usize;
    let mut printed = 0usize;

    set_color(sink, 11, 0);
    print_str(sink, if faulted_only { " faulted nodes\n" } else { " live nodes\n" });
    set_color(sink, 7, 0);

    loop {
        let (total, returned) = gos_runtime::node_page::<PAGE>(offset, &mut items);
        for item in items.iter().take(returned) {
            if faulted_only && item.lifecycle != NodeLifecycle::Faulted {
                continue;
            }
            let fg: u8 = match item.lifecycle {
                NodeLifecycle::Ready | NodeLifecycle::Running => 10,
                NodeLifecycle::Faulted => 12,
                NodeLifecycle::Waiting | NodeLifecycle::Suspended => 14,
                _ => 7,
            };
            set_color(sink, fg, 0);
            print_str(sink, "  ");
            let mut vec_buf = LineBuf::<20>::new();
            vec_buf.push_vector(item.vector);
            print_str(sink, core::str::from_utf8(vec_buf.as_slice()).unwrap_or("?.?.?.?"));
            set_color(sink, 7, 0);
            print_str(sink, "  ");
            print_str(sink, item.plugin_name);
            print_str(sink, "/");
            print_str(sink, item.local_node_key);
            print_str(sink, "  ");
            set_color(sink, 8, 0);
            print_str(sink, lifecycle_label(item.lifecycle));
            print_str(sink, "\n");
            set_color(sink, 7, 0);
            printed += 1;
        }
        offset += returned;
        if returned == 0 || offset >= total {
            break;
        }
    }
    if printed == 0 {
        set_color(sink, 8, 0);
        print_str(sink, if faulted_only { "  (no faulted nodes)\n" } else { "  (no nodes)\n" });
        set_color(sink, 7, 0);
    }
}

/// Lifecycle distribution summary — counts per state, coloured for quick triage.
/// Analogous to `ps aux | awk '{print $8}' | sort | uniq -c` in Linux.
pub fn dispatch_lifecycle_summary(sink: &ConsoleSink) {
    use gos_protocol::{GraphNodeSummary, NodeLifecycle};
    const PAGE: usize = 8;
    let mut items = [GraphNodeSummary::EMPTY; PAGE];
    let mut offset = 0usize;
    let mut n_boot     = 0usize;  // Discovered | Loaded | Registered
    let mut n_alloc    = 0usize;
    let mut n_ready    = 0usize;
    let mut n_run      = 0usize;
    let mut n_wait     = 0usize;
    let mut n_suspend  = 0usize;
    let mut n_term     = 0usize;
    let mut n_fault    = 0usize;

    loop {
        let (total, returned) = gos_runtime::node_page::<PAGE>(offset, &mut items);
        for item in items.iter().take(returned) {
            match item.lifecycle {
                NodeLifecycle::Discovered
                | NodeLifecycle::Loaded
                | NodeLifecycle::Registered => n_boot += 1,
                NodeLifecycle::Allocated  => n_alloc += 1,
                NodeLifecycle::Ready      => n_ready += 1,
                NodeLifecycle::Running    => n_run += 1,
                NodeLifecycle::Waiting    => n_wait += 1,
                NodeLifecycle::Suspended  => n_suspend += 1,
                NodeLifecycle::Terminated => n_term += 1,
                NodeLifecycle::Faulted    => n_fault += 1,
            }
        }
        offset += returned;
        if returned == 0 || offset >= total {
            break;
        }
    }

    set_color(sink, 11, 0);
    print_str(sink, " node lifecycle summary\n");
    set_color(sink, 7, 0);

    let total_live = n_boot + n_alloc + n_ready + n_run + n_wait + n_suspend + n_term + n_fault;
    if total_live == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes)\n");
        set_color(sink, 7, 0);
        return;
    }

    macro_rules! print_count {
        ($label:expr, $count:expr, $fg:expr) => {
            if $count > 0 {
                set_color(sink, $fg, 0);
                print_str(sink, "  ");
                print_str(sink, $label);
                print_str(sink, ": ");
                print_num_inline(sink, $count);
                print_str(sink, "\n");
            }
        };
    }
    print_count!("boot-phase", n_boot,    7);
    print_count!("alloc",      n_alloc,   7);
    print_count!("ready",      n_ready,  10);
    print_count!("running",    n_run,    10);
    print_count!("waiting",    n_wait,   14);
    print_count!("suspended",  n_suspend, 14);
    print_count!("terminated", n_term,    8);
    print_count!("faulted",    n_fault,  12);
    set_color(sink, 7, 0);
    print_str(sink, "  total: ");
    print_num_inline(sink, total_live);
    print_str(sink, "\n");
}

/// Display the boot manifest verification report stored by hypervisor at boot.
///
/// Reads the two atomic counters written by `gos_runtime::record_boot_manifest_report`
/// and formats them as a human-readable health check — the GOS equivalent of
/// `systemctl status` for the dependency graph.
pub fn dispatch_boot_verify(sink: &ConsoleSink) {
    let rules  = gos_runtime::boot_manifest_rules_checked();
    let healed = gos_runtime::boot_manifest_edges_healed();
    set_color(sink, 11, 0);
    print_str(sink, " boot manifest\n");
    set_color(sink, 7, 0);
    print_str(sink, "  rules checked: ");
    print_num_inline(sink, rules);
    print_str(sink, "\n  edges healed:  ");
    print_num_inline(sink, healed);
    print_str(sink, "\n  status:        ");
    if rules == 0 {
        set_color(sink, 14, 0);
        print_str(sink, "pending (boot not yet completed)\n");
    } else if healed == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "OK — all ");
        print_num_inline(sink, rules);
        print_str(sink, " depend edges present\n");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "WARNING — ");
        print_num_inline(sink, healed);
        print_str(sink, " edge(s) self-healed at boot (imperative pass missed edges)\n");
    }
    set_color(sink, 7, 0);
}

/// Output all runtime telemetry metrics as `key=value\n` lines.
///
/// Machine-parseable counterpart to the `metrics` graph-panel view.
/// Host-side serial scripts can collect these values without parsing TUI escape codes.
pub fn dispatch_metrics_export(sink: &ConsoleSink) {
    let g_epoch = gos_runtime::graph_epoch();
    let r_epoch = gos_supervisor::render_epoch();
    let snap    = gos_runtime::snapshot();
    set_color(sink, 11, 0);
    print_str(sink, " telemetry export\n");
    set_color(sink, 7, 0);

    macro_rules! kv {
        ($key:expr, $val:expr) => {
            print_str(sink, "  ");
            print_str(sink, $key);
            print_str(sink, "=");
            print_num_inline(sink, $val as usize);
            print_str(sink, "\n");
        };
    }
    kv!("graph_epoch",          g_epoch);
    kv!("render_epoch",         r_epoch);
    kv!("idle_cycles",          gos_supervisor::idle_cycle_count());
    kv!("causal_depth_max",     gos_supervisor::causal_depth_max());
    kv!("subscribe_pairs",      gos_runtime::subscribe_pair_count());
    kv!("tick",                 snap.tick);
    kv!("plugins",              snap.plugin_count);
    kv!("nodes",                snap.node_count);
    kv!("edges",                snap.edge_count);
    kv!("domain_switches",      gos_runtime::domain_switch_count());
    kv!("preemptions",          gos_runtime::preempt_count());
    kv!("boot_fallback_allocs", gos_runtime::boot_fallback_alloc_count());
    kv!("boot_rules_checked",   gos_runtime::boot_manifest_rules_checked());
    kv!("boot_edges_healed",    gos_runtime::boot_manifest_edges_healed());
    set_color(sink, 7, 0);
}

/// Report gos-journal on-disk format constants and capability summary.
///
/// Analogous to `journalctl --version`: confirms the magic, version, and record
/// geometry that replay will expect.  Pure read — no runtime state is touched.
pub fn dispatch_journal_info(sink: &ConsoleSink) {
    set_color(sink, 11, 0);
    print_str(sink, " journal format\n");
    set_color(sink, 7, 0);
    print_str(sink, "  envelope magic:      GOSJ\n");
    print_str(sink, "  envelope version:    ");
    print_num_inline(sink, gos_journal::JOURNAL_VERSION as usize);
    print_str(sink, "\n  header_bytes:        ");
    print_num_inline(sink, gos_journal::HEADER_BYTES);
    print_str(sink, "\n  envelope_record:     ");
    print_num_inline(sink, gos_journal::ENVELOPE_RECORD_BYTES);
    print_str(sink, " bytes (fixed)\n");
    print_str(sink, "  snapshot magic:      GOSS\n");
    print_str(sink, "  snapshot version:    ");
    print_num_inline(sink, gos_journal::SNAPSHOT_VERSION as usize);
    print_str(sink, "\n  snapshot_hdr:        ");
    print_num_inline(sink, gos_journal::SNAPSHOT_HEADER_BYTES);
    print_str(sink, " bytes\n  node_record:         ");
    print_num_inline(sink, gos_journal::SNAPSHOT_NODE_BYTES);
    print_str(sink, " bytes\n  edge_record:         ");
    print_num_inline(sink, gos_journal::SNAPSHOT_EDGE_BYTES);
    print_str(sink, " bytes\n");
    print_str(sink, "  kinds:               12 (Hello..SubscribeTriggered)\n");
    set_color(sink, 10, 0);
    print_str(sink, "  status:              F.4 control-plane journal -- replay-ready\n");
    set_color(sink, 7, 0);
}

/// Parse a decimal string into a u64 epoch number, no_std-compatible.
/// Returns None if the input is empty or contains any non-ASCII-digit character.
pub(crate) fn parse_epoch_decimal(s: &str) -> Option<u64> {
    if s.is_empty() { return None; }
    let mut val: u64 = 0;
    for b in s.bytes() {
        if b < b'0' || b > b'9' { return None; }
        val = val.saturating_mul(10).saturating_add((b - b'0') as u64);
    }
    Some(val)
}

/// Parse an edge-type filter word from the `edges <type>` command.
fn parse_edge_type_filter(s: &str) -> Option<RuntimeEdgeType> {
    match s.trim() {
        "call"   => Some(RuntimeEdgeType::Call),
        "spawn"  => Some(RuntimeEdgeType::Spawn),
        "depend" => Some(RuntimeEdgeType::Depend),
        "signal" => Some(RuntimeEdgeType::Signal),
        "return" => Some(RuntimeEdgeType::Return),
        "mount"  => Some(RuntimeEdgeType::Mount),
        "sync"   => Some(RuntimeEdgeType::Sync),
        "stream" => Some(RuntimeEdgeType::Stream),
        "use"    => Some(RuntimeEdgeType::Use),
        _ => None,
    }
}

/// Text-mode listing of all live graph edges — the GOS equivalent of `ss -a` / `lsof`.
///
/// Prints each edge's from_vector, type, to_vector, and key to the scrolling console.
/// When `filter_type` is `Some(t)`, only edges of that type are shown.
/// Color-codes edge type: green = call, yellow = mount, cyan = use, blue = depend.
pub fn dispatch_edges_list(sink: &ConsoleSink, filter_type: Option<RuntimeEdgeType>) {
    const PAGE: usize = 8;
    let mut items = [GraphEdgeSummary::EMPTY; PAGE];
    let mut offset = 0usize;
    let mut printed = 0usize;

    let title = match filter_type {
        None                           => " live edges\n",
        Some(RuntimeEdgeType::Call)    => " call edges\n",
        Some(RuntimeEdgeType::Spawn)   => " spawn edges\n",
        Some(RuntimeEdgeType::Depend)  => " depend edges\n",
        Some(RuntimeEdgeType::Signal)  => " signal edges\n",
        Some(RuntimeEdgeType::Return)  => " return edges\n",
        Some(RuntimeEdgeType::Mount)   => " mount edges\n",
        Some(RuntimeEdgeType::Sync)    => " sync edges\n",
        Some(RuntimeEdgeType::Stream)  => " stream edges\n",
        Some(RuntimeEdgeType::Use)     => " use edges\n",
        Some(RuntimeEdgeType::Link)    => " link edges\n",
    };
    set_color(sink, 11, 0);
    print_str(sink, title);
    set_color(sink, 7, 0);

    loop {
        let (total, returned) = gos_runtime::edge_page::<PAGE>(offset, &mut items);
        for item in items.iter().take(returned) {
            if let Some(ft) = filter_type {
                if item.edge_type != ft {
                    continue;
                }
            }
            let fg: u8 = match item.edge_type {
                RuntimeEdgeType::Call   => 10,
                RuntimeEdgeType::Spawn  => 10,
                RuntimeEdgeType::Mount  => 14,
                RuntimeEdgeType::Use    => 11,
                RuntimeEdgeType::Depend => 9,
                RuntimeEdgeType::Sync   => 13,
                RuntimeEdgeType::Signal => 12,
                RuntimeEdgeType::Stream => 6,
                RuntimeEdgeType::Return => 7,
                RuntimeEdgeType::Link   => 8,
            };
            set_color(sink, fg, 0);
            print_str(sink, "  ");
            let mut from_buf = LineBuf::<20>::new();
            from_buf.push_vector(item.from_vector);
            print_str(sink, core::str::from_utf8(from_buf.as_slice()).unwrap_or("?"));
            set_color(sink, 8, 0);
            print_str(sink, " -[");
            set_color(sink, fg, 0);
            print_str(sink, edge_type_label(item.edge_type));
            set_color(sink, 8, 0);
            print_str(sink, "]-> ");
            set_color(sink, 7, 0);
            let mut to_buf = LineBuf::<20>::new();
            to_buf.push_vector(item.to_vector);
            print_str(sink, core::str::from_utf8(to_buf.as_slice()).unwrap_or("?"));
            if !item.from_key.is_empty() {
                set_color(sink, 8, 0);
                print_str(sink, "  ");
                print_str(sink, item.from_key);
            }
            print_str(sink, "\n");
            set_color(sink, 7, 0);
            printed += 1;
        }
        offset += returned;
        if returned == 0 || offset >= total {
            break;
        }
    }

    if printed == 0 {
        set_color(sink, 8, 0);
        print_str(sink, if filter_type.is_some() { "  (no edges of that type)\n" } else { "  (no edges)\n" });
        set_color(sink, 7, 0);
    }
}

/// Report total edge count — lightweight `edges count` variant.
///
/// Reads only the total from `edge_page` without enumerating all summaries.
/// Analogous to `ss --summary` in Linux.
pub fn dispatch_edge_count(sink: &ConsoleSink) {
    let mut items = [GraphEdgeSummary::EMPTY; 1];
    let (total, _) = gos_runtime::edge_page::<1>(0, &mut items);
    set_color(sink, 11, 0);
    print_str(sink, " edge count\n");
    set_color(sink, 7, 0);
    print_str(sink, "  total: ");
    print_num_inline(sink, total);
    print_str(sink, "\n  status: ");
    if total == 0 {
        set_color(sink, 14, 0);
        print_str(sink, "no edges (graph topology empty)\n");
    } else {
        set_color(sink, 10, 0);
        print_str(sink, "edges active\n");
    }
    set_color(sink, 7, 0);
}

/// Edge type distribution summary — counts per type, colour-coded for quick
/// triage. Analogous to `ss -s` / `netstat -s`: aggregated topology health
/// at a glance without walking every edge by hand.
pub fn dispatch_edge_type_summary(sink: &ConsoleSink) {
    use gos_protocol::RuntimeEdgeType;
    const PAGE: usize = 8;
    let mut items = [GraphEdgeSummary::EMPTY; PAGE];
    let mut offset = 0usize;

    let mut n_call = 0usize;
    let mut n_spawn = 0usize;
    let mut n_depend = 0usize;
    let mut n_signal = 0usize;
    let mut n_return = 0usize;
    let mut n_mount = 0usize;
    let mut n_sync = 0usize;
    let mut n_stream = 0usize;
    let mut n_use = 0usize;
    let mut n_link = 0usize;

    loop {
        let (total, returned) = gos_runtime::edge_page::<PAGE>(offset, &mut items);
        for item in items.iter().take(returned) {
            match item.edge_type {
                RuntimeEdgeType::Call => n_call += 1,
                RuntimeEdgeType::Spawn => n_spawn += 1,
                RuntimeEdgeType::Depend => n_depend += 1,
                RuntimeEdgeType::Signal => n_signal += 1,
                RuntimeEdgeType::Return => n_return += 1,
                RuntimeEdgeType::Mount => n_mount += 1,
                RuntimeEdgeType::Sync => n_sync += 1,
                RuntimeEdgeType::Stream => n_stream += 1,
                RuntimeEdgeType::Use => n_use += 1,
                RuntimeEdgeType::Link => n_link += 1,
            }
        }
        offset += returned;
        if returned == 0 || offset >= total {
            break;
        }
    }

    set_color(sink, 11, 0);
    print_str(sink, " edge type summary\n");
    set_color(sink, 7, 0);

    let total_edges =
        n_call + n_spawn + n_depend + n_signal + n_return + n_mount + n_sync + n_stream + n_use + n_link;
    if total_edges == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no edges)\n");
        set_color(sink, 7, 0);
        return;
    }

    macro_rules! print_edge_count {
        ($label:expr, $count:expr, $fg:expr) => {
            if $count > 0 {
                set_color(sink, $fg, 0);
                print_str(sink, "  ");
                print_str(sink, $label);
                print_str(sink, ": ");
                print_num_inline(sink, $count);
                print_str(sink, "\n");
            }
        };
    }
    print_edge_count!("call", n_call, 14);
    print_edge_count!("spawn", n_spawn, 7);
    print_edge_count!("depend", n_depend, 10);
    print_edge_count!("signal", n_signal, 11);
    print_edge_count!("return", n_return, 7);
    print_edge_count!("mount", n_mount, 13);
    print_edge_count!("sync", n_sync, 7);
    print_edge_count!("stream", n_stream, 7);
    print_edge_count!("use", n_use, 13);
    print_edge_count!("link", n_link, 8);
    set_color(sink, 7, 0);
    print_str(sink, "  total: ");
    print_num_inline(sink, total_edges);
    print_str(sink, "\n");
}

/// `graph diff` — structural topology changelog since pinned epoch.
///
/// Like `git log` for the kernel graph: shows every node/edge add or remove
/// recorded in the diff ring, color-coded green (+) / red (-) like a diff.
///
/// - `graph diff`        → show all mutations since the pinned epoch (or boot if never pinned)
/// - `graph diff pin`    → pin the current epoch as the diff baseline
/// - `graph diff reset`  → reset baseline to 0 (show all since boot)
pub fn dispatch_graph_diff(sink: &ConsoleSink, since_epoch: u64) {
    use gos_protocol::GraphDiffEntry;
    const PAGE: usize = 16;
    let mut entries = [GraphDiffEntry::EMPTY; PAGE];
    let (total, filled) = gos_runtime::graph_diff_since::<PAGE>(since_epoch, &mut entries);
    let current_epoch = gos_runtime::graph_epoch();

    set_color(sink, 11, 0);
    print_str(sink, " graph diff");
    set_color(sink, 8, 0);
    print_str(sink, " (epoch ");
    print_num_inline(sink, since_epoch as usize);
    print_str(sink, " -> ");
    print_num_inline(sink, current_epoch as usize);
    print_str(sink, ")\n");
    set_color(sink, 7, 0);

    for entry in entries.iter().take(filled) {
        let (prefix, fg) = match entry.kind {
            GraphDiffKind::NodeAdded  | GraphDiffKind::EdgeAdded  => ("+", 10u8),
            GraphDiffKind::NodeRemoved | GraphDiffKind::EdgeRemoved => ("-", 12u8),
            GraphDiffKind::NodeCheckpoint => ("·", 14u8),
        };
        let kind_label = match entry.kind {
            GraphDiffKind::NodeAdded      => "node+ ",
            GraphDiffKind::NodeRemoved    => "node- ",
            GraphDiffKind::EdgeAdded      => "edge+ ",
            GraphDiffKind::EdgeRemoved    => "edge- ",
            GraphDiffKind::NodeCheckpoint => "ckpt  ",
        };
        set_color(sink, fg, 0);
        print_str(sink, " ");
        print_str(sink, prefix);
        print_str(sink, " [");
        print_str(sink, kind_label);
        print_str(sink, "] ");
        let mut vec_buf = LineBuf::<20>::new();
        vec_buf.push_vector(entry.from_vector);
        print_str(sink, core::str::from_utf8(vec_buf.as_slice()).unwrap_or("?"));
        if entry.kind.is_node() {
            set_color(sink, 8, 0);
            print_str(sink, "  ");
            print_str(sink, entry.label_str());
        } else {
            set_color(sink, 8, 0);
            print_str(sink, " -[");
            set_color(sink, fg, 0);
            print_str(sink, entry.label_str());
            set_color(sink, 8, 0);
            print_str(sink, "]-> ");
            set_color(sink, 7, 0);
            let mut to_buf = LineBuf::<20>::new();
            to_buf.push_vector(entry.to_vector);
            print_str(sink, core::str::from_utf8(to_buf.as_slice()).unwrap_or("?"));
        }
        set_color(sink, 8, 0);
        print_str(sink, "  @epoch ");
        print_num_inline(sink, entry.epoch as usize);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
    }

    if filled == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no changes since epoch ");
        print_num_inline(sink, since_epoch as usize);
        print_str(sink, ")\n");
        set_color(sink, 7, 0);
    } else if total > filled {
        set_color(sink, 8, 0);
        print_str(sink, "  ... ");
        print_num_inline(sink, total - filled);
        print_str(sink, " more (ring capped at ");
        print_num_inline(sink, PAGE);
        print_str(sink, " per page)\n");
        set_color(sink, 7, 0);
    }

    set_color(sink, 8, 0);
    print_str(sink, "  total: ");
    print_num_inline(sink, total);
    print_str(sink, " change(s)");
    if total > 0 {
        print_str(sink, "  |  use 'graph diff pin' to update baseline\n");
    } else {
        print_str(sink, "\n");
    }
    set_color(sink, 7, 0);
}

/// Show a ps-style table of all live graph nodes with their cumulative signal
/// counts and outbound edge counts — analogous to `ps aux` on Linux.
pub fn dispatch_proc_list(sink: &ConsoleSink) {
    use gos_protocol::NodeProcSummary;
    const PAGE: usize = 32;
    let mut summaries = [NodeProcSummary::EMPTY; PAGE];
    let (total, filled) = gos_runtime::proc_page::<PAGE>(0, &mut summaries);

    set_color(sink, 11, 0);
    print_str(sink, " proc");
    set_color(sink, 8, 0);
    print_str(sink, "  vector              sig    out  state       plugin/key\n");
    set_color(sink, 7, 0);

    for summary in summaries.iter().take(filled) {
        let fg: u8 = match summary.lifecycle {
            gos_protocol::NodeLifecycle::Running    => 10,
            gos_protocol::NodeLifecycle::Faulted    => 12,
            gos_protocol::NodeLifecycle::Suspended  => 14,
            _                                       => 7,
        };
        set_color(sink, fg, 0);
        print_str(sink, "  ");
        let mut vec_buf = LineBuf::<20>::new();
        vec_buf.push_vector(summary.vector);
        let vec_str = core::str::from_utf8(vec_buf.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        // pad to 20 chars
        let pad = 20usize.saturating_sub(vec_str.len());
        for _ in 0..pad { print_str(sink, " "); }
        set_color(sink, 11, 0);
        print_num_right4(sink, summary.signal_count as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  ");
        print_num_right4(sink, summary.edge_out_count as usize);
        print_str(sink, "  ");
        set_color(sink, fg, 0);
        let state_label = node_lifecycle_label(summary.lifecycle);
        print_str(sink, state_label);
        let state_pad = 12usize.saturating_sub(state_label.len());
        for _ in 0..state_pad { print_str(sink, " "); }
        set_color(sink, 8, 0);
        print_str(sink, summary.plugin_name);
        print_str(sink, "/");
        print_str(sink, summary.local_node_key);
        print_str(sink, "\n");
    }

    if filled == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
    } else if total > filled {
        set_color(sink, 8, 0);
        print_str(sink, "  ... ");
        print_num_inline(sink, total - filled);
        print_str(sink, " more (page capped at ");
        print_num_inline(sink, PAGE);
        print_str(sink, ")\n");
    }
    set_color(sink, 8, 0);
    print_str(sink, "  total: ");
    print_num_inline(sink, total);
    print_str(sink, " node(s)");
    if total > 0 {
        print_str(sink, "  |  sig = cumulative signal dispatches\n");
    } else {
        print_str(sink, "\n");
    }
    set_color(sink, 7, 0);
}

/// Show a detailed stat block for the single node at `vec` — analogous to
/// `cat /proc/<pid>/status` on Linux.  Shows vector, key, plugin, lifecycle,
/// signal count, and outbound edge count.  Prints an error if the vector is
/// not registered.
pub fn dispatch_node_stat(sink: &ConsoleSink, vec: VectorAddress) {
    set_color(sink, 11, 0);
    print_str(sink, " node stat\n");
    set_color(sink, 7, 0);
    match gos_runtime::proc_stat_for_vector(vec) {
        None => {
            set_color(sink, 12, 0);
            print_str(sink, "  not found: ");
            let mut line = LineBuf::<20>::new();
            line.push_vector(vec);
            print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("?"));
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
        Some(s) => {
            let fg: u8 = match s.lifecycle {
                gos_protocol::NodeLifecycle::Running   => 10,
                gos_protocol::NodeLifecycle::Faulted   => 12,
                gos_protocol::NodeLifecycle::Suspended => 14,
                _                                      => 7,
            };
            print_str(sink, "  vector:        ");
            let mut vec_line = LineBuf::<20>::new();
            vec_line.push_vector(s.vector);
            set_color(sink, fg, 0);
            print_str(sink, core::str::from_utf8(vec_line.as_slice()).unwrap_or("?"));
            set_color(sink, 7, 0);
            print_str(sink, "\n  key:           ");
            print_str(sink, s.local_node_key);
            print_str(sink, "\n  plugin:        ");
            print_str(sink, s.plugin_name);
            print_str(sink, "\n  lifecycle:     ");
            set_color(sink, fg, 0);
            print_str(sink, node_lifecycle_label(s.lifecycle));
            set_color(sink, 7, 0);
            print_str(sink, "\n  signal_count:  ");
            set_color(sink, 11, 0);
            print_num_inline(sink, s.signal_count as usize);
            set_color(sink, 7, 0);
            print_str(sink, "\n  edge_out:      ");
            print_num_inline(sink, s.edge_out_count as usize);
            print_str(sink, "\n");
        }
    }
}

/// Forcibly fault the node at `vec` — the graph-OS equivalent of `kill -9`.
/// Reports the faulted vector on success (green) or "not found" (red) when the
/// vector is not registered.
pub fn dispatch_node_kill(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");
    match gos_runtime::fault_node(vec) {
        Ok(()) => {
            set_color(sink, 10, 0);
            print_str(sink, " kill: node faulted\n");
            set_color(sink, 7, 0);
            print_str(sink, "  vector:   ");
            print_str(sink, vec_str);
            print_str(sink, "\n  lifecycle -> Faulted  |  fault queue enqueued\n");
            set_color(sink, 8, 0);
            print_str(sink, "  hint: use `nodes faulted` to list faulted nodes\n");
            set_color(sink, 7, 0);
        }
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " kill: node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
    }
}

/// Resume a faulted or suspended node at `vec` — graph-OS equivalent of
/// `systemctl restart`.  Sets the node's lifecycle to `Ready` so it can
/// receive signals again.  Reports success (green) or "not found" (red).
pub fn dispatch_node_resume(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");
    match gos_runtime::resume_node(vec) {
        Ok(()) => {
            set_color(sink, 10, 0);
            print_str(sink, " resume: node ready\n");
            set_color(sink, 7, 0);
            print_str(sink, "  vector:   ");
            print_str(sink, vec_str);
            print_str(sink, "\n  lifecycle -> Ready  |  node may receive signals\n");
            set_color(sink, 8, 0);
            print_str(sink, "  hint: use `proc` to verify new lifecycle state\n");
            set_color(sink, 7, 0);
        }
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " resume: node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
    }
}

/// `node info <vec>` — comprehensive single-node status view.
///
/// The graph-OS analogue of `systemctl status <unit>`: shows a node's
/// identity (vector, key, plugin), lifecycle, cumulative signal count, and
/// a full inline listing of every edge that touches this node — both
/// outbound (this node as source) and inbound (this node as target).
///
/// This is the single-pane-of-glass command: `stat` + `edges` for one node.
pub fn dispatch_node_info(sink: &ConsoleSink, vec: VectorAddress) {
    use gos_protocol::GraphEdgeSummary;

    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    set_color(sink, 11, 0);
    print_str(sink, " node info\n");
    set_color(sink, 7, 0);

    match gos_runtime::proc_stat_for_vector(vec) {
        None => {
            set_color(sink, 12, 0);
            print_str(sink, "  not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
            return;
        }
        Some(s) => {
            let fg: u8 = match s.lifecycle {
                gos_protocol::NodeLifecycle::Running   => 10,
                gos_protocol::NodeLifecycle::Faulted   => 12,
                gos_protocol::NodeLifecycle::Suspended => 14,
                _                                      => 7,
            };
            print_str(sink, "  vector:        ");
            set_color(sink, fg, 0);
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "\n  key:           ");
            print_str(sink, s.local_node_key);
            print_str(sink, "\n  plugin:        ");
            print_str(sink, s.plugin_name);
            print_str(sink, "\n  lifecycle:     ");
            set_color(sink, fg, 0);
            print_str(sink, node_lifecycle_label(s.lifecycle));
            set_color(sink, 7, 0);
            print_str(sink, "\n  signal_count:  ");
            set_color(sink, 11, 0);
            print_num_inline(sink, s.signal_count as usize);
            set_color(sink, 7, 0);
            print_str(sink, "\n  edge_out:      ");
            print_num_inline(sink, s.edge_out_count as usize);
            print_str(sink, "\n");
        }
    }

    // Edge listing — all edges touching this node.
    const EDGE_PAGE: usize = 16;
    let mut edges = [GraphEdgeSummary::EMPTY; EDGE_PAGE];
    match gos_runtime::edge_page_for_node(vec, 0, &mut edges) {
        Err(_) => {
            set_color(sink, 8, 0);
            print_str(sink, "  edges:         (unavailable)\n");
            set_color(sink, 7, 0);
        }
        Ok((total, returned)) => {
            print_str(sink, "  edges (");
            print_num_inline(sink, total);
            print_str(sink, "):\n");
            if returned == 0 {
                set_color(sink, 8, 0);
                print_str(sink, "    (none)\n");
                set_color(sink, 7, 0);
            } else {
                for edge in edges.iter().take(returned) {
                    let is_out = edge.from_vector == vec;
                    let (dir_label, fg): (&str, u8) = if is_out {
                        ("out", 10)
                    } else {
                        ("in ", 13)
                    };
                    set_color(sink, fg, 0);
                    print_str(sink, "    ");
                    print_str(sink, dir_label);
                    set_color(sink, 8, 0);
                    print_str(sink, "  ");
                    if !is_out {
                        let mut from_buf = LineBuf::<20>::new();
                        from_buf.push_vector(edge.from_vector);
                        set_color(sink, 7, 0);
                        print_str(sink, core::str::from_utf8(from_buf.as_slice()).unwrap_or("?"));
                        set_color(sink, 8, 0);
                        print_str(sink, " -[");
                        set_color(sink, fg, 0);
                        print_str(sink, edge_type_label(edge.edge_type));
                        set_color(sink, 8, 0);
                        print_str(sink, "]-> ");
                        set_color(sink, 11, 0);
                        print_str(sink, vec_str);
                    } else {
                        set_color(sink, 11, 0);
                        print_str(sink, vec_str);
                        set_color(sink, 8, 0);
                        print_str(sink, " -[");
                        set_color(sink, fg, 0);
                        print_str(sink, edge_type_label(edge.edge_type));
                        set_color(sink, 8, 0);
                        print_str(sink, "]-> ");
                        set_color(sink, 7, 0);
                        let mut to_buf = LineBuf::<20>::new();
                        to_buf.push_vector(edge.to_vector);
                        print_str(sink, core::str::from_utf8(to_buf.as_slice()).unwrap_or("?"));
                    }
                    if !edge.from_key.is_empty() {
                        set_color(sink, 8, 0);
                        print_str(sink, "  ");
                        print_str(sink, edge.from_key);
                    }
                    print_str(sink, "\n");
                    set_color(sink, 7, 0);
                }
                if total > returned {
                    set_color(sink, 8, 0);
                    print_str(sink, "    ... ");
                    print_num_inline(sink, total - returned);
                    print_str(sink, " more edges\n");
                    set_color(sink, 7, 0);
                }
            }
        }
    }
    set_color(sink, 8, 0);
    print_str(sink, "  hint: stat <vec> for counters | edges <type> for type filter\n");
    set_color(sink, 7, 0);
}

/// `node trace <vec>` / `ntrace <vec>` — per-node signal dispatch history.
///
/// Analogous to `strace -p <pid>`: shows the most recent signals dispatched
/// to a single node, most recent first.  Each row: seq | kind | cmd | from.
pub fn dispatch_node_trace(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; gos_runtime::MAX_NODE_TRACE];
    match gos_runtime::node_trace_page(vec, &mut ring) {
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
            return;
        }
        Ok((total, returned)) => {
            set_color(sink, 11, 0);
            print_str(sink, " node trace  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            print_str(sink, "  total=");
            set_color(sink, 7, 0);
            print_num_inline(sink, total as usize);
            set_color(sink, 8, 0);
            print_str(sink, "  showing=");
            set_color(sink, 7, 0);
            print_num_inline(sink, returned);
            print_str(sink, "\n");
            set_color(sink, 8, 0);
            print_str(sink, "   seq  kind      cmd  from\n");
            set_color(sink, 7, 0);
            if returned == 0 {
                set_color(sink, 8, 0);
                print_str(sink, "   (no signals dispatched yet)\n");
                set_color(sink, 7, 0);
            }
            for i in 0..returned {
                let e = ring[i];
                if e.kind == 0 { continue; } // EMPTY sentinel
                print_num_right4(sink, e.serial as usize);
                print_str(sink, "  ");
                let (kind_label, kind_color) = signal_kind_entry(e.kind);
                set_color(sink, kind_color, 0);
                print_str(sink, kind_label);
                set_color(sink, 7, 0);
                print_str(sink, "  ");
                if e.cmd != 0 {
                    set_color(sink, 14, 0);
                    print_num_right4(sink, e.cmd as usize);
                    set_color(sink, 7, 0);
                } else {
                    print_str(sink, "   0");
                }
                print_str(sink, "  ");
                if e.from != 0 {
                    let from_vec = VectorAddress::from_u64(e.from);
                    let mut from_buf = LineBuf::<20>::new();
                    from_buf.push_vector(from_vec);
                    set_color(sink, 11, 0);
                    print_str(sink, core::str::from_utf8(from_buf.as_slice()).unwrap_or("?"));
                    set_color(sink, 7, 0);
                } else {
                    set_color(sink, 8, 0);
                    print_str(sink, "kernel");
                    set_color(sink, 7, 0);
                }
                print_str(sink, "\n");
            }
            set_color(sink, 8, 0);
            print_str(sink, "  hint: node info <vec> for static view | proc for all nodes\n");
            set_color(sink, 7, 0);
        }
    }
}

fn signal_kind_entry(kind: u8) -> (&'static str, u8) {
    match kind {
        0x01 => ("call    ", 10),
        0x02 => ("spawn   ", 13),
        0x03 => ("irq     ", 9),
        0x04 => ("data    ", 7),
        0x05 => ("control ", 14),
        0xFF => ("term    ", 12),
        _    => ("?       ", 8),
    }
}

/// `node log <vec>` / `nlog <vec>` — per-node lifecycle event log.
///
/// Analogous to `journalctl -u <service>`: shows the most recent lifecycle
/// state transitions for one node, newest first.
/// Each row: tick | lifecycle state label.
pub fn dispatch_node_log(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    let mut log = [gos_protocol::NodeLogEntry::EMPTY; gos_runtime::MAX_NODE_LOG];
    match gos_runtime::node_log_page(vec, &mut log) {
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
        Ok((total, returned)) => {
            set_color(sink, 11, 0);
            print_str(sink, " node log  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            print_str(sink, "  total=");
            set_color(sink, 7, 0);
            print_num_inline(sink, total);
            set_color(sink, 8, 0);
            print_str(sink, "  showing=");
            set_color(sink, 7, 0);
            print_num_inline(sink, returned);
            print_str(sink, "\n");
            set_color(sink, 8, 0);
            print_str(sink, "    tick  lifecycle\n");
            set_color(sink, 7, 0);
            if returned == 0 {
                set_color(sink, 8, 0);
                print_str(sink, "   (no lifecycle events recorded yet)\n");
                set_color(sink, 7, 0);
            }
            for i in 0..returned {
                let e = log[i];
                print_num_right4(sink, e.tick as usize);
                print_str(sink, "  ");
                let (label, color) = lifecycle_log_entry(e.lifecycle);
                set_color(sink, color, 0);
                print_str(sink, label);
                set_color(sink, 7, 0);
                print_str(sink, "\n");
            }
            set_color(sink, 8, 0);
            print_str(sink, "  hint: node trace <vec> for signal history | ninfo <vec> for full view\n");
            set_color(sink, 7, 0);
        }
    }
}

/// `node trace clear <vec>` / `ntrace clear <vec>` — clear per-node signal trace ring.
///
/// Analogous to `perf trace --no-inherit` reset or `truncate -s0 /var/log/strace.log`:
/// discards the buffered signal dispatch history for one node.  The cumulative
/// signal_count shown by `proc` is not affected — only the trace ring is cleared.
pub fn dispatch_node_trace_clear(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    match gos_runtime::clear_node_trace(vec) {
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
        Ok(()) => {
            set_color(sink, 10, 0);
            print_str(sink, " node trace cleared  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
    }
}

/// `node stat clear <vec>` / `nstat clear <vec>` — reset per-node signal_count to zero.
///
/// Analogous to `perf stat reset` or `echo 0 > /proc/<pid>/clear_refs`:
/// zeroes the cumulative signal dispatch counter shown by `proc` and `stat`.
/// Useful after node recovery or when starting a fresh measurement window.
/// Does not affect the trace ring or lifecycle log.
pub fn dispatch_node_stat_clear(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    match gos_runtime::reset_node_stat(vec) {
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
        Ok(()) => {
            set_color(sink, 10, 0);
            print_str(sink, " node stat cleared  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
            print_str(sink, "  signal_count -> 0  (trace ring and log unaffected)\n");
        }
    }
}

/// V2.51: `node checkpoint <vec>` / `ncp <vec>` / `checkpoint <vec>` —
/// snapshot current node state into the structural diff ring.
///
/// Analogous to `perf record --event=mark` or `gdb checkpoint`: records the
/// node's vector address, key, signal_count, lifecycle, and edge_out_count as a
/// `GraphDiffKind::NodeCheckpoint` entry in the diff ring.  Graph epoch is NOT
/// bumped — only the diff ring is touched.  The captured state is displayed
/// immediately; `graph diff` will show it tagged as `[ckpt]`.
pub fn dispatch_node_checkpoint(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    match gos_runtime::node_checkpoint(vec) {
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
        Ok(s) => {
            let fg: u8 = match s.lifecycle {
                gos_protocol::NodeLifecycle::Running   => 10,
                gos_protocol::NodeLifecycle::Faulted   => 12,
                gos_protocol::NodeLifecycle::Suspended => 14,
                _                                      => 7,
            };
            set_color(sink, 11, 0);
            print_str(sink, " node checkpoint  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
            print_str(sink, "  key:          ");
            print_str(sink, s.local_node_key);
            print_str(sink, "\n  lifecycle:     ");
            set_color(sink, fg, 0);
            print_str(sink, node_lifecycle_label(s.lifecycle));
            set_color(sink, 7, 0);
            print_str(sink, "\n  signal_count:  ");
            set_color(sink, 11, 0);
            print_num_inline(sink, s.signal_count as usize);
            set_color(sink, 7, 0);
            print_str(sink, "\n  edge_out:      ");
            print_num_inline(sink, s.edge_out_count as usize);
            set_color(sink, 8, 0);
            print_str(sink, "\n  → recorded in diff ring as [ckpt]  (graph diff to view)\n");
            set_color(sink, 7, 0);
        }
    }
}

/// V2.55: `node attr set <vec> <hex>` / `nattr set <vec> <hex>` — store a u32 attribute on a node.
///
/// Stores an arbitrary u32 scalar (palette color, flag, counter) keyed by the node's
/// NodeId.  Analogous to `xattr -w` on macOS or `setfattr` on Linux: attaches metadata
/// to a graph node without touching the node's structural state or epoch.
pub fn dispatch_node_attr_set(sink: &ConsoleSink, vec: VectorAddress, val: u32) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    match gos_runtime::node_attr_set(vec, val) {
        Err(gos_runtime::RuntimeError::NodeNotFound) => {
            set_color(sink, 12, 0);
            print_str(sink, " node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " node attr table full (max ");
            print_num_inline(sink, gos_runtime::MAX_NODE_PROPS_U32);
            print_str(sink, " entries)\n");
            set_color(sink, 7, 0);
        }
        Ok(()) => {
            set_color(sink, 11, 0);
            print_str(sink, " node attr set  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "  =  0x");
            print_hex32_inline(sink, val);
            print_str(sink, "\n");
        }
    }
}

/// V2.55: `node attr get <vec>` / `nattr get <vec>` — read the u32 attribute of a node.
///
/// Reads the u32 attribute stored on the node at `vec`.  Returns `none` when no
/// attribute has been set.  Useful for inspecting palette colors, flags, or other
/// graph-native scalar data attached to nodes.
pub fn dispatch_node_attr_get(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    match gos_runtime::node_attr_get(vec) {
        None => {
            set_color(sink, 8, 0);
            print_str(sink, " node attr  ");
            print_str(sink, vec_str);
            print_str(sink, "  none\n");
            set_color(sink, 7, 0);
        }
        Some(val) => {
            set_color(sink, 10, 0);
            print_str(sink, " node attr  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "  =  0x");
            print_hex32_inline(sink, val);
            print_str(sink, "\n");
        }
    }
}

/// V2.58: `node attr list` / `nattr list` — show all nodes with a u32 attribute set.
///
/// Prints a table of (VectorAddress, hex value) for every occupied slot in
/// node_props_u32, plus a slot-usage footer.  Useful for palette/flag audits.
pub fn dispatch_node_attr_list(sink: &ConsoleSink) {
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U32];
    let mut vals = [0u32; gos_runtime::MAX_NODE_PROPS_U32];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);

    set_color(sink, 11, 0);
    print_str(sink, " node attr list\n");
    set_color(sink, 7, 0);

    if count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no u32 attributes set)\n");
        set_color(sink, 7, 0);
    } else {
        let mut i = 0usize;
        while i < count {
            let mut vec_line = LineBuf::<20>::new();
            vec_line.push_vector(vecs[i]);
            let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");
            set_color(sink, 10, 0);
            print_str(sink, "  ");
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "  0x");
            print_hex32_inline(sink, vals[i]);
            print_str(sink, "\n");
            i += 1;
        }
    }

    // Slot-usage footer.
    set_color(sink, 8, 0);
    print_str(sink, "  ");
    print_num_inline(sink, count);
    print_str(sink, " / ");
    print_num_inline(sink, gos_runtime::MAX_NODE_PROPS_U32);
    print_str(sink, " slots used\n");
    set_color(sink, 7, 0);
}

/// V2.61: `graph clustering` — display the global clustering coefficient.
///
/// Watts-Strogatz style: for each node with >= 2 undirected neighbors, counts
/// the fraction of neighbor pairs that are connected. Expressed in ppm.
pub fn dispatch_graph_clustering(sink: &ConsoleSink) {
    let (clustering_ppm, n) = gos_runtime::graph_clustering();
    set_color(sink, 11, 0);
    print_str(sink, " graph clustering\n");
    set_color(sink, 7, 0);

    if clustering_ppm == 0 && n < 2 {
        set_color(sink, 8, 0);
        print_str(sink, "  clustering: undefined (fewer than 2 nodes)\n");
        set_color(sink, 7, 0);
    } else if clustering_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  clustering: 0%  (0 ppm) — no triangles\n");
        set_color(sink, 7, 0);
    } else {
        let pct_int  = clustering_ppm / 10_000;
        let pct_frac = (clustering_ppm % 10_000) / 100;
        set_color(sink, 10, 0);
        print_str(sink, "  clustering: ");
        print_num_inline(sink, pct_int as usize);
        print_str(sink, ".");
        if pct_frac < 10 { print_str(sink, "0"); }
        print_num_inline(sink, pct_frac as usize);
        print_str(sink, "%");
        set_color(sink, 8, 0);
        print_str(sink, "  (");
        print_num_inline(sink, clustering_ppm as usize);
        print_str(sink, " ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, n);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.63: `graph transitivity` — global transitivity (3×triangles / open_triplets).
///
/// Watts-Strogatz CC (V2.61) averages per-node local CCs; global transitivity
/// gives each triplet equal weight regardless of which node it's centred on,
/// so high-degree hub nodes dominate the metric.
pub fn dispatch_graph_transitivity(sink: &ConsoleSink) {
    let (transitivity_ppm, triangles, triplets, n) = gos_runtime::graph_transitivity();
    set_color(sink, 11, 0);
    print_str(sink, " graph transitivity\n");
    set_color(sink, 7, 0);

    if n == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  transitivity: undefined (empty graph)\n");
        set_color(sink, 7, 0);
    } else if triplets == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  transitivity: 0%  (0 ppm) — no triplets\n");
        set_color(sink, 7, 0);
    } else {
        let pct_int  = transitivity_ppm / 10_000;
        let pct_frac = (transitivity_ppm % 10_000) / 100;
        set_color(sink, 10, 0);
        print_str(sink, "  transitivity: ");
        print_num_inline(sink, pct_int as usize);
        print_str(sink, ".");
        if pct_frac < 10 { print_str(sink, "0"); }
        print_num_inline(sink, pct_frac as usize);
        print_str(sink, "%");
        set_color(sink, 8, 0);
        print_str(sink, "  (");
        print_num_inline(sink, transitivity_ppm as usize);
        print_str(sink, " ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, n);
    print_str(sink, "  triangles=");
    print_num_inline(sink, triangles as usize);
    print_str(sink, "  triplets=");
    print_num_inline(sink, triplets as usize);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.64: `graph kcore` — k-core / coreness decomposition of the kernel graph.
///
/// Each node receives a coreness value: the largest k such that the node is
/// in the k-core (the maximal subgraph where every node has undirected
/// degree ≥ k).  The graph degeneracy is the maximum coreness.
///
/// Role labels (colour-coded):
///   core      — coreness == max_coreness (densest inner shell)
///   inner     — coreness > 0, < max_coreness (intermediate shell)
///   periphery — coreness == 0 (isolated or pendant)
pub fn dispatch_graph_kcore(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, coreness, total, max_core) = gos_runtime::graph_kcore::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph kcore  (k-core decomposition)\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              k     role\n");
    set_color(sink, 7, 0);

    for i in 0..total {
        let k    = coreness[i];
        let is_core = max_core > 0 && k == max_core;
        let is_peri = k == 0;

        if is_core {
            set_color(sink, 10, 0); // bright green — core
        } else if is_peri {
            set_color(sink, 8, 0);  // grey — periphery
        } else {
            set_color(sink, 11, 0); // cyan — inner
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        print_str(sink, " ");
        print_num_right6(sink, k as usize);
        print_str(sink, "  ");

        if is_core {
            set_color(sink, 10, 0);
            print_str(sink, "core");
        } else if is_peri {
            set_color(sink, 8, 0);
            print_str(sink, "periphery");
        } else {
            set_color(sink, 11, 0);
            print_str(sink, "inner");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  degeneracy=");
    set_color(sink, 10, 0);
    print_num_inline(sink, max_core as usize);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.65: `graph assortativity` — degree assortativity coefficient (Newman 2002).
///
/// Measures whether high-degree nodes preferentially connect to other high-degree
/// nodes (assortative, r > 0) or to low-degree nodes (disassortative, r < 0).
/// Degree = undirected neighbour count per node; edges counted once each.
///
/// Displays the coefficient as a percentage and ppm value, plus raw edge/node counts.
pub fn dispatch_graph_assortativity(sink: &ConsoleSink) {
    let (r_ppm, edges, nodes) = gos_runtime::graph_assortativity();

    set_color(sink, 11, 0);
    print_str(sink, " graph assortativity\n");
    set_color(sink, 7, 0);

    if edges == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  assortativity: undefined (no edges)\n");
        set_color(sink, 7, 0);
    } else if r_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  assortativity: 0.00%  (0 ppm)  — uncorrelated\n");
        set_color(sink, 7, 0);
    } else {
        let abs_ppm  = if r_ppm < 0 { -(r_ppm as i64) } else { r_ppm as i64 } as usize;
        let pct_int  = abs_ppm / 10_000;
        let pct_frac = (abs_ppm % 10_000) / 100;
        set_color(sink, 10, 0);
        print_str(sink, "  assortativity: ");
        if r_ppm < 0 { print_str(sink, "-"); }
        print_num_inline(sink, pct_int);
        print_str(sink, ".");
        if pct_frac < 10 { print_str(sink, "0"); }
        print_num_inline(sink, pct_frac);
        print_str(sink, "%");
        set_color(sink, 8, 0);
        print_str(sink, "  (");
        if r_ppm < 0 { print_str(sink, "-"); }
        print_num_inline(sink, abs_ppm);
        print_str(sink, " ppm)");
        set_color(sink, 7, 0);
        if r_ppm > 0 {
            set_color(sink, 8, 0);
            print_str(sink, "  assortative");
            set_color(sink, 7, 0);
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "  disassortative");
            set_color(sink, 7, 0);
        }
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, nodes);
    print_str(sink, "  edges=");
    print_num_inline(sink, edges);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.66: `graph reciprocity` — fraction of directed edges that are mutual.
///
/// For each directed edge (u→v), checks whether the reverse edge (v→u) exists.
/// reciprocity_ppm = mutual_edges / total_edges × 1_000_000.
///
/// Displays the reciprocity as a percentage and ppm, plus mutual/total counts.
pub fn dispatch_graph_reciprocity(sink: &ConsoleSink) {
    let (recip_ppm, mutual, total) = gos_runtime::graph_reciprocity();

    set_color(sink, 11, 0);
    print_str(sink, " graph reciprocity\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  reciprocity: undefined (no edges)\n");
        set_color(sink, 7, 0);
    } else if recip_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  reciprocity: 0.00%  (0 ppm)  \u{2014} no mutual edges\n");
        set_color(sink, 7, 0);
    } else {
        let pct_int  = recip_ppm / 10_000;
        let pct_frac = (recip_ppm % 10_000) / 100;
        set_color(sink, 10, 0);
        print_str(sink, "  reciprocity: ");
        print_num_inline(sink, pct_int as usize);
        print_str(sink, ".");
        if pct_frac < 10 { print_str(sink, "0"); }
        print_num_inline(sink, pct_frac as usize);
        print_str(sink, "%");
        set_color(sink, 8, 0);
        print_str(sink, "  (");
        print_num_inline(sink, recip_ppm as usize);
        print_str(sink, " ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  mutual=");
    print_num_inline(sink, mutual);
    print_str(sink, "  total=");
    print_num_inline(sink, total);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.67: `graph modularity` — Newman–Girvan modularity Q of the LPA community partition.
///
/// Runs the same LPA as `graph community` to detect communities, then evaluates
/// Q = Σ_c [ L_c/m − (d_c/(2m))² ] over those communities.
///
/// modularity_ppm ∈ [0, 1_000_000] for any LPA-detected partition.
///   0        → single community (connected graph) or no edges
///   500_000  → two equal-sized disconnected cliques (theoretical benchmark)
///   1_000_000 → hypothetically perfect partition (rarely achievable in practice)
///
/// Displays: modularity as % and ppm, plus community count / edge count / node count.
pub fn dispatch_graph_modularity(sink: &ConsoleSink) {
    let (q_ppm, comms, edges, nodes) = gos_runtime::graph_modularity();

    set_color(sink, 11, 0);
    print_str(sink, " graph modularity\n");
    set_color(sink, 7, 0);

    if edges == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  modularity: undefined (no edges)\n");
        set_color(sink, 7, 0);
    } else if q_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  modularity: 0.00%  (0 ppm)  \u{2014} single community\n");
        set_color(sink, 7, 0);
    } else {
        let abs_ppm  = q_ppm.max(0) as usize;
        let pct_int  = abs_ppm / 10_000;
        let pct_frac = (abs_ppm % 10_000) / 100;
        set_color(sink, 10, 0);
        print_str(sink, "  modularity: ");
        print_num_inline(sink, pct_int);
        print_str(sink, ".");
        if pct_frac < 10 { print_str(sink, "0"); }
        print_num_inline(sink, pct_frac);
        print_str(sink, "%");
        set_color(sink, 8, 0);
        print_str(sink, "  (");
        print_num_inline(sink, abs_ppm);
        print_str(sink, " ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  communities=");
    print_num_inline(sink, comms);
    print_str(sink, "  edges=");
    print_num_inline(sink, edges);
    print_str(sink, "  nodes=");
    print_num_inline(sink, nodes);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.68: `graph rich club <k>` — rich-club coefficient for degree threshold k.
///
/// Counts nodes with undirected degree > k ("rich" nodes) and measures how
/// densely they connect to each other:
///   ρ(k) = E_{>k} / [N_{>k} × (N_{>k}−1) / 2]
/// where E_{>k} = undirected edges among rich nodes; N_{>k} = rich node count.
/// Directed edges treated as undirected; self-loops excluded.
///
///   1_000_000 ppm → rich nodes form a clique (maximally connected).
///   0             → no rich nodes, < 2 rich nodes, or no edges among them.
///
/// Displays: ρ(k) as %, raw ppm, plus rich_nodes / edges_among_rich / k.
pub fn dispatch_graph_rich_club(sink: &ConsoleSink, k: u8) {
    let (rho_ppm, n_rich, e_rich) = gos_runtime::graph_rich_club(k);

    set_color(sink, 11, 0);
    print_str(sink, " graph rich club\n");
    set_color(sink, 7, 0);

    if n_rich < 2 {
        set_color(sink, 8, 0);
        print_str(sink, "  rich club: undefined (fewer than 2 rich nodes for k=");
        print_num_inline(sink, k as usize);
        print_str(sink, ")\n");
        set_color(sink, 7, 0);
    } else if rho_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  rich club: 0.00%  (0 ppm)  \u{2014} no edges among rich nodes\n");
        set_color(sink, 7, 0);
    } else {
        let pct_int  = rho_ppm / 10_000;
        let pct_frac = (rho_ppm % 10_000) / 100;
        set_color(sink, 10, 0);
        print_str(sink, "  rich club: ");
        print_num_inline(sink, pct_int as usize);
        print_str(sink, ".");
        if pct_frac < 10 { print_str(sink, "0"); }
        print_num_inline(sink, pct_frac as usize);
        print_str(sink, "%");
        set_color(sink, 8, 0);
        print_str(sink, "  (");
        print_num_inline(sink, rho_ppm as usize);
        print_str(sink, " ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  k=");
    print_num_inline(sink, k as usize);
    print_str(sink, "  rich_nodes=");
    print_num_inline(sink, n_rich);
    print_str(sink, "  edges_among_rich=");
    print_num_inline(sink, e_rich);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.69: `graph girth` — shortest directed cycle length.
///
/// Displays the girth of the live directed graph (length of its shortest
/// directed cycle).  Self-loops → girth = 1; mutual pairs A↔B → girth = 2;
/// directed k-cycle → girth = k.  Acyclic (DAG) → "acyclic".
pub fn dispatch_graph_girth(sink: &ConsoleSink) {
    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();

    set_color(sink, 11, 0);
    print_str(sink, " graph girth\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  girth: undefined (empty graph)\n");
        set_color(sink, 7, 0);
    } else if is_acyclic {
        set_color(sink, 10, 0);
        print_str(sink, "  girth: acyclic (no directed cycle)\n");
        set_color(sink, 7, 0);
    } else {
        set_color(sink, 10, 0);
        print_str(sink, "  girth: ");
        print_num_inline(sink, girth as usize);
        set_color(sink, 8, 0);
        if girth == 1 {
            print_str(sink, "  (self-loop)");
        } else if girth == 2 {
            print_str(sink, "  (mutual pair)");
        } else {
            print_str(sink, "  (shortest directed cycle)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.70: `graph wiener` / `gwiener` — Wiener index (sum of pairwise BFS distances).
///
/// Prints W(G), reachable pair count, average path length (W/pairs), and node count.
/// Disconnected pairs (no directed path) are excluded from both W and the average.
pub fn dispatch_graph_wiener(sink: &ConsoleSink) {
    let (wiener, pairs, node_count) = gos_runtime::graph_wiener();

    set_color(sink, 11, 0);
    print_str(sink, " graph wiener\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  wiener: undefined (empty graph)\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 10, 0);
    print_str(sink, "  W(G) = ");
    // Print wiener_index (u64) using existing u64 helper if available, else decompose.
    // We decompose into high/low u32 and print as decimal.
    {
        // Simple decimal print for u64 using a byte buffer.
        let mut buf = [b'0'; 20];
        let mut pos = 20usize;
        let mut v = wiener;
        if v == 0 {
            pos -= 1;
            buf[pos] = b'0';
        } else {
            while v > 0 {
                pos -= 1;
                buf[pos] = b'0' + (v % 10) as u8;
                v /= 10;
            }
        }
        for i in pos..20 {
            print_byte(sink, buf[i]);
        }
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    set_color(sink, 8, 0);
    print_str(sink, "  reachable pairs = ");
    print_num_inline(sink, pairs);
    print_str(sink, "\n");

    if pairs > 0 {
        // Average path length: integer part + 3 decimal places
        let avg_whole = (wiener / pairs as u64) as usize;
        let avg_frac  = ((wiener % pairs as u64) * 1000 / pairs as u64) as usize;
        print_str(sink, "  avg path length = ");
        print_num_inline(sink, avg_whole);
        print_str(sink, ".");
        // 3-digit zero-padded fraction
        if avg_frac < 10 {
            print_str(sink, "00");
        } else if avg_frac < 100 {
            print_str(sink, "0");
        }
        print_num_inline(sink, avg_frac);
        print_str(sink, "\n");
    }

    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.71: `graph harmonic` / `gharm` — harmonic centrality (sum of reciprocal distances).
///
/// HC[v] = Σ 1_000_000/d(v,u) for all reachable u≠v.  Handles disconnected graphs.
pub fn dispatch_graph_harmonic(sink: &ConsoleSink) {
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();

    set_color(sink, 11, 0);
    print_str(sink, " graph harmonic centrality\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  harmonic: undefined (empty graph)\n");
        set_color(sink, 7, 0);
        return;
    }

    let mut i = 0usize;
    while i < total {
        let mut vec_line = LineBuf::<20>::new();
        vec_line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

        set_color(sink, 10, 0);
        print_str(sink, "  ");
        print_str(sink, vec_str);
        set_color(sink, 7, 0);
        print_str(sink, "  HC=");
        print_num_inline(sink, hc[i] as usize);
        print_str(sink, "\n");
        i += 1;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, total);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.72: `graph peripheral` / `gperiph` — nodes whose eccentricity equals the graph diameter.
///
/// Peripheral nodes are the "boundary" of the graph — they lie farthest from some other node.
/// Returns peripheral_count, node_count, and diameter.
pub fn dispatch_graph_peripheral(sink: &ConsoleSink) {
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();

    set_color(sink, 11, 0);
    print_str(sink, " graph peripheral nodes\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
        set_color(sink, 7, 0);
        return;
    }

    if diameter == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  all nodes isolated \u{2014} no peripheral nodes (diameter=0)\n");
        print_str(sink, "  nodes=");
        print_num_inline(sink, node_count);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              ecc\n");
    set_color(sink, 7, 0);

    let mut i = 0usize;
    while i < periph_count {
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");

        set_color(sink, 12, 0); // red — peripheral (boundary)
        print_str(sink, "  ");
        print_str(sink, vec_str);

        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        print_str(sink, " ");
        print_num_right6(sink, ecc[i] as usize);
        set_color(sink, 7, 0);
        print_str(sink, "\n");
        i += 1;
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    print_str(sink, "  diameter=");
    print_num_inline(sink, diameter as usize);
    print_str(sink, "  peripheral=");
    print_num_inline(sink, periph_count);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.73: `graph center` / `gcenter` — nodes whose eccentricity equals the graph radius.
///
/// Centre nodes have the minimum worst-case distance to any reachable peer —
/// the structural complement of peripheral nodes (ecc == diameter).
pub fn dispatch_graph_center(sink: &ConsoleSink) {
    let (vecs, ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();

    set_color(sink, 11, 0);
    print_str(sink, " graph center nodes\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
        set_color(sink, 7, 0);
        return;
    }

    if radius == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  all nodes isolated \u{2014} no center nodes (radius=0)\n");
        print_str(sink, "  nodes=");
        print_num_inline(sink, node_count);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              ecc\n");
    set_color(sink, 7, 0);

    let mut i = 0usize;
    while i < center_count {
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");

        set_color(sink, 10, 0); // green — centre (lowest ecc)
        print_str(sink, "  ");
        print_str(sink, vec_str);

        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        print_str(sink, " ");
        print_num_right6(sink, ecc[i] as usize);
        set_color(sink, 7, 0);
        print_str(sink, "\n");
        i += 1;
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    print_str(sink, "  radius=");
    print_num_inline(sink, radius as usize);
    print_str(sink, "  center=");
    print_num_inline(sink, center_count);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.74: `graph efficiency` / `geff` — global graph efficiency.
///
/// E(G) = 1/(n*(n-1)) * Σ_{i≠j, d(i,j)<∞} 1/d(i,j)
///
/// Quantifies how efficiently the graph exchanges information on average.
/// Disconnected pairs contribute 0 (not infinity), making this metric
/// well-defined for disconnected graphs — unlike average path length.
/// E=1.0 for complete directed graphs; E=0.0 for fully disconnected.
/// Printed as X.XXXXXX with 6 decimal places (1_000_000 ppm scale).
pub fn dispatch_graph_global_efficiency(sink: &ConsoleSink) {
    let (ppm, pairs_max, node_count) = gos_runtime::graph_global_efficiency();

    set_color(sink, 11, 0);
    print_str(sink, " graph global efficiency\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count < 2 {
        set_color(sink, 8, 0);
        if node_count == 0 {
            print_str(sink, "  E(G): undefined (empty graph)\n");
        } else {
            print_str(sink, "  E(G): 0.000000 (single node, no pairs)\n");
        }
        print_str(sink, "  nodes=");
        print_num_inline(sink, node_count);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
        return;
    }

    // Print E(G) = W.XXXXXX (ppm gives 6 decimal places; whole part is 0 or 1).
    set_color(sink, 10, 0);
    print_str(sink, "  E(G) = ");
    let whole = (ppm / 1_000_000) as usize;
    let frac  = (ppm % 1_000_000) as usize;
    print_num_inline(sink, whole);
    print_str(sink, ".");
    // Zero-pad frac to 6 digits.
    if frac < 10 {
        print_str(sink, "00000");
    } else if frac < 100 {
        print_str(sink, "0000");
    } else if frac < 1_000 {
        print_str(sink, "000");
    } else if frac < 10_000 {
        print_str(sink, "00");
    } else if frac < 100_000 {
        print_str(sink, "0");
    }
    print_num_inline(sink, frac);
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    print_str(sink, "  pairs_max=");
    print_num_inline(sink, pairs_max);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.75: `graph avg clustering` / `gavgcc` — per-node average clustering coefficient.
///
/// avg_CC = (1/n) × Σ CC(v)  where CC(v) = triangles(v) / C(k_v, 2)
/// Unweighted average — distinct from `graph clustering` (V2.61) which is global transitivity.
/// Nodes with undirected degree < 2 contribute CC(v) = 0.
/// avg_CC = 1.0 for complete graphs; = 0 when no triangle neighbourhood exists.
pub fn dispatch_graph_avg_clustering(sink: &ConsoleSink) {
    let (avg_ppm, nodes_computed, node_count) = gos_runtime::graph_avg_clustering();

    set_color(sink, 11, 0);
    print_str(sink, " graph avg clustering\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  avg CC: undefined (empty graph)\n");
        print_str(sink, "  nodes=0\n");
        set_color(sink, 7, 0);
        return;
    }

    // Print avg CC = W.XXXXXX (6 decimal places via ppm scale).
    set_color(sink, 10, 0);
    print_str(sink, "  avg CC = ");
    let whole = (avg_ppm / 1_000_000) as usize;
    let frac  = (avg_ppm % 1_000_000) as usize;
    print_num_inline(sink, whole);
    print_str(sink, ".");
    if frac < 10 {
        print_str(sink, "00000");
    } else if frac < 100 {
        print_str(sink, "0000");
    } else if frac < 1_000 {
        print_str(sink, "000");
    } else if frac < 10_000 {
        print_str(sink, "00");
    } else if frac < 100_000 {
        print_str(sink, "0");
    }
    print_num_inline(sink, frac);
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    print_str(sink, "  nodes_computed=");
    print_num_inline(sink, nodes_computed);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.76: `graph local efficiency` / `gleff` — local network efficiency (Latora-Marchiori 2001).
///
/// E_loc(G) = (1/n) × Σ_v E(G_v)
/// where G_v is the directed subgraph induced by the undirected neighbours of v.
/// Complement to global efficiency: measures local fault-tolerance / resilience.
pub fn dispatch_graph_local_efficiency(sink: &ConsoleSink) {
    let (ppm, nodes_computed, node_count) = gos_runtime::graph_local_efficiency();

    set_color(sink, 11, 0);
    print_str(sink, " graph local efficiency\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  E_loc: undefined (empty graph)\n");
        print_str(sink, "  nodes=0\n");
        set_color(sink, 7, 0);
        return;
    }

    // Print E_loc = W.XXXXXX (6 decimal places via ppm scale).
    set_color(sink, 10, 0);
    print_str(sink, "  E_loc = ");
    let whole = (ppm / 1_000_000) as usize;
    let frac  = (ppm % 1_000_000) as usize;
    print_num_inline(sink, whole);
    print_str(sink, ".");
    if frac < 10 {
        print_str(sink, "00000");
    } else if frac < 100 {
        print_str(sink, "0000");
    } else if frac < 1_000 {
        print_str(sink, "000");
    } else if frac < 10_000 {
        print_str(sink, "00");
    } else if frac < 100_000 {
        print_str(sink, "0");
    }
    print_num_inline(sink, frac);
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    print_str(sink, "  nodes_computed=");
    print_num_inline(sink, nodes_computed);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.77: `graph small world` / `gsmallworld` — small-world coefficient \u{03c3}.
///
/// \u{03c3} = (CC / CC_rand) / (L / L_rand)
/// CC_rand \u{2248} 2m / n(n-1), L_rand \u{2248} ln(n)/ln(\u{27e8}k\u{27e9}).
/// \u{03c3} > 1 indicates small-world structure; \u{03c3} = 0 if insufficient connectivity.
pub fn dispatch_graph_small_world(sink: &ConsoleSink) {
    let (sigma_ppm, cc_ppm, l_ppm, l_rand_ppm, node_count, m_undir) =
        gos_runtime::graph_small_world();

    set_color(sink, 11, 0);
    print_str(sink, " graph small-world coefficient\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  \u{03c3}: undefined (empty graph)\n");
        print_str(sink, "  nodes=0\n");
        set_color(sink, 7, 0);
        return;
    }

    // Helper: print a ppm value as W.XXXXXX (6 decimal places).
    fn print_ppm(sink: &ConsoleSink, ppm: u64) {
        let whole = (ppm / 1_000_000) as usize;
        let frac  = (ppm % 1_000_000) as usize;
        print_num_inline(sink, whole);
        print_str(sink, ".");
        if frac < 10           { print_str(sink, "00000"); }
        else if frac < 100     { print_str(sink, "0000"); }
        else if frac < 1_000   { print_str(sink, "000"); }
        else if frac < 10_000  { print_str(sink, "00"); }
        else if frac < 100_000 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    // σ value.
    set_color(sink, 10, 0);
    print_str(sink, "  \u{03c3}       = ");
    if sigma_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "undefined");
        set_color(sink, 10, 0);
        print_str(sink, "  (avg_k<2 or L=0)\n");
    } else {
        print_ppm(sink, sigma_ppm as u64);
        print_str(sink, "\n");
    }

    // CC and CC_rand.
    set_color(sink, 7, 0);
    print_str(sink, "  CC      = ");
    print_ppm(sink, cc_ppm as u64);
    print_str(sink, "\n");

    // L and L_rand.
    print_str(sink, "  L       = ");
    print_ppm(sink, l_ppm);
    print_str(sink, "\n");
    print_str(sink, "  L_rand  = ");
    print_ppm(sink, l_rand_ppm);
    print_str(sink, "\n");

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "  edges_undir=");
    print_num_inline(sink, m_undir);
    print_str(sink, "  avg_k=");
    let avg_k = if node_count > 0 { (2 * m_undir) / node_count } else { 0 };
    print_num_inline(sink, avg_k);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.78: `graph scale free` / `gscalefree` / `scale free`
///
/// Displays the degree heterogeneity index κ = ⟨k²⟩/⟨k⟩ and classifies the graph
/// topology as scale-free, heterogeneous, or homogeneous (regular/random-like).
pub fn dispatch_graph_scale_free(sink: &ConsoleSink) {
    let (kappa_ppm, max_degree, avg_degree_ppm, node_count, m_undir) =
        gos_runtime::graph_scale_free();

    set_color(sink, 11, 0);
    print_str(sink, " graph scale-free detection\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  \u{03ba}: undefined (empty graph)\n");
        print_str(sink, "  nodes=0\n");
        set_color(sink, 7, 0);
        return;
    }

    // Helper: print a ppm value as W.XXXXXX (6 decimal places).
    fn print_ppm6(sink: &ConsoleSink, ppm: u64) {
        let whole = (ppm / 1_000_000) as usize;
        let frac  = (ppm % 1_000_000) as usize;
        print_num_inline(sink, whole);
        print_str(sink, ".");
        if frac < 10           { print_str(sink, "00000"); }
        else if frac < 100     { print_str(sink, "0000"); }
        else if frac < 1_000   { print_str(sink, "000"); }
        else if frac < 10_000  { print_str(sink, "00"); }
        else if frac < 100_000 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    // κ value.
    set_color(sink, 10, 0);
    print_str(sink, "  \u{03ba}        = ");
    if kappa_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "undefined");
        set_color(sink, 10, 0);
        print_str(sink, "  (no edges)\n");
    } else {
        print_ppm6(sink, kappa_ppm as u64);
        print_str(sink, "\n");
    }

    // ⟨k⟩ (average degree).
    set_color(sink, 7, 0);
    print_str(sink, "  \u{27e8}k\u{27e9}      = ");
    print_ppm6(sink, avg_degree_ppm as u64);
    print_str(sink, "\n");

    // k_max.
    print_str(sink, "  k_max    = ");
    print_num_inline(sink, max_degree as usize);
    print_str(sink, "\n");

    // Classification.
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    // kappa > 3 * avg_k → likely scale-free; > 2 → heterogeneous; else homogeneous.
    let threshold3 = (avg_degree_ppm as u64).saturating_mul(3);
    let threshold2 = (avg_degree_ppm as u64).saturating_mul(2);
    if kappa_ppm as u64 > threshold3 {
        set_color(sink, 10, 0);
        print_str(sink, "  topology: likely scale-free (\u{03ba} >> \u{27e8}k\u{27e9})\n");
    } else if kappa_ppm as u64 > threshold2 {
        set_color(sink, 14, 0);
        print_str(sink, "  topology: heterogeneous (\u{03ba} > 2\u{27e8}k\u{27e9})\n");
    } else {
        set_color(sink, 7, 0);
        print_str(sink, "  topology: homogeneous (regular/random-like)\n");
    }
    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "  edges_undir=");
    print_num_inline(sink, m_undir);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.80: `graph power law` / `gpowerlaw` — power-law exponent MLE.
///
/// Computes the Clauset–Newman–Shalizi (2009) maximum-likelihood estimator:
///   γ̂ = 1 + n_fit / Σ ln(k_i)   (k_min = 1, non-isolated nodes only)
///
/// Typical scale-free networks have γ ∈ [2, 3].
/// gamma=0 means MLE is undefined (all non-isolated nodes have k=1).
pub fn dispatch_graph_power_law(sink: &ConsoleSink) {
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();

    set_color(sink, 11, 0);
    print_str(sink, " graph power-law exponent (MLE)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  \u{03b3}: undefined (empty graph)\n");
        print_str(sink, "  nodes=0\n");
        set_color(sink, 7, 0);
        return;
    }

    // Helper: print a ppm value as W.XXXXXX (6 decimal places).
    fn print_ppm6(sink: &ConsoleSink, ppm: u64) {
        let whole = (ppm / 1_000_000) as usize;
        let frac  = (ppm % 1_000_000) as usize;
        print_num_inline(sink, whole);
        print_str(sink, ".");
        if frac < 10           { print_str(sink, "00000"); }
        else if frac < 100     { print_str(sink, "0000"); }
        else if frac < 1_000   { print_str(sink, "000"); }
        else if frac < 10_000  { print_str(sink, "00"); }
        else if frac < 100_000 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    // γ̂ value.
    set_color(sink, 10, 0);
    print_str(sink, "  \u{03b3}\u{0302}         = ");
    if gamma_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "undefined");
        set_color(sink, 10, 0);
        print_str(sink, "  (all non-isolated k=1; MLE degenerate)\n");
    } else {
        print_ppm6(sink, gamma_ppm as u64);
        print_str(sink, "\n");
    }

    // Fit info.
    set_color(sink, 7, 0);
    print_str(sink, "  n_fit      = ");
    print_num_inline(sink, n_fit);
    print_str(sink, "  (nodes with k\u{2265}1, k_min=1)\n");

    // Classification banner.
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    if gamma_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  \u{03b3}: insufficient degree heterogeneity for MLE\n");
    } else if gamma_ppm <= 3_000_000 {
        set_color(sink, 10, 0);
        print_str(sink, "  \u{03b3}\u{2208}[1,3]: compatible with power-law / scale-free\n");
    } else if gamma_ppm <= 4_000_000 {
        set_color(sink, 14, 0);
        print_str(sink, "  \u{03b3}\u{2208}(3,4]: steep tail; weakly heterogeneous\n");
    } else {
        set_color(sink, 7, 0);
        print_str(sink, "  \u{03b3}>4: not consistent with a power-law degree distribution\n");
    }
    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "  n_fit=");
    print_num_inline(sink, n_fit);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.82: `graph diameter` / `gdiameter` — combined center + peripheral view.
///
/// Displays the graph's diameter (max eccentricity), radius (min nonzero eccentricity),
/// center nodes (ecc == radius, green) and peripheral nodes (ecc == diameter, red)
/// in a single panel — the structural "core vs. boundary" summary.
pub fn dispatch_graph_diameter(sink: &ConsoleSink) {
    let (p_vecs, p_ecc, periph_count, p_node_count, diameter) =
        gos_runtime::graph_peripheral::<64>();
    let (c_vecs, c_ecc, center_count, _c_node_count, radius) =
        gos_runtime::graph_center::<64>();
    let node_count = p_node_count;

    set_color(sink, 11, 0);
    print_str(sink, " graph diameter view\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
        set_color(sink, 7, 0);
        return;
    }

    if diameter == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  all nodes isolated \u{2014} diameter=0, radius=0\n");
        print_str(sink, "  nodes=");
        print_num_inline(sink, node_count);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
        return;
    }

    // Column header.
    set_color(sink, 8, 0);
    print_str(sink, "  vector              ecc   role\n");
    set_color(sink, 7, 0);

    // Center nodes (ecc == radius) in green.
    let mut i = 0usize;
    while i < center_count {
        let mut line = LineBuf::<20>::new();
        line.push_vector(c_vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");

        set_color(sink, 10, 0);
        print_str(sink, "  ");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }
        print_str(sink, " ");
        print_num_right6(sink, c_ecc[i] as usize);
        print_str(sink, "  center\n");
        set_color(sink, 7, 0);
        i += 1;
    }

    // Peripheral nodes (ecc == diameter) in red.
    // Skip any that are also center nodes (radius == diameter) to avoid double-listing.
    if radius != diameter {
        let mut j = 0usize;
        while j < periph_count {
            let mut line = LineBuf::<20>::new();
            line.push_vector(p_vecs[j]);
            let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");

            set_color(sink, 12, 0);
            print_str(sink, "  ");
            print_str(sink, vec_str);
            let vlen = vec_str.len();
            for _ in vlen..16 { print_str(sink, " "); }
            print_str(sink, " ");
            print_num_right6(sink, p_ecc[j] as usize);
            print_str(sink, "  peripheral\n");
            set_color(sink, 7, 0);
            j += 1;
        }
    }

    // Footer.
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    print_str(sink, "  radius=");
    print_num_inline(sink, radius as usize);
    print_str(sink, "  diameter=");
    print_num_inline(sink, diameter as usize);
    print_str(sink, "  center=");
    print_num_inline(sink, center_count);
    print_str(sink, "  periph=");
    print_num_inline(sink, periph_count);
    print_str(sink, "  nodes=");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// V2.79: `graph summary` / `gsummary` / `topology summary`
///
/// One-shot topology report: gathers density, transitivity, avg clustering,
/// global efficiency, local efficiency, small-world σ, and scale-free κ in
/// a single human-readable panel.
pub fn dispatch_graph_summary(sink: &ConsoleSink) {
    // Gather all metrics (each takes its own RUNTIME lock; all pure reads).
    let (density_ppm, edge_count, node_count)              = gos_runtime::graph_density();
    let (trans_ppm,   _tri, _trip, _)                      = gos_runtime::graph_transitivity();
    let (avgcc_ppm,   _nodes_cc,   _)                      = gos_runtime::graph_avg_clustering();
    let (geff_ppm,    _pairs,      _)                      = gos_runtime::graph_global_efficiency();
    let (leff_ppm,    _nodes_le,   _)                      = gos_runtime::graph_local_efficiency();
    let (sigma_ppm,   _cc_sw, _l_sw, _lr_sw, _, m_undir)  = gos_runtime::graph_small_world();
    let (kappa_ppm,   max_k,  avg_k_ppm, _,   _)          = gos_runtime::graph_scale_free();
    let (gamma_ppm,   _n_fit, _)                           = gos_runtime::graph_power_law();

    set_color(sink, 11, 0);
    print_str(sink, " graph topology summary\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph — no nodes)\n");
        set_color(sink, 7, 0);
        return;
    }

    // Helper: print ppm as W.XXX (3 decimal places; compact for tabular view).
    fn print_ppm3(sink: &ConsoleSink, ppm: u64) {
        let whole = (ppm / 1_000_000) as usize;
        let frac  = (ppm / 1_000 % 1_000) as usize;
        print_num_inline(sink, whole);
        print_str(sink, ".");
        if frac < 10  { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    // ── Structure ─────────────────────────────────────────────────────────────
    set_color(sink, 14, 0);
    print_str(sink, "  [structure]\n");
    set_color(sink, 7, 0);
    print_str(sink, "  nodes        = ");
    print_num_inline(sink, node_count);
    print_str(sink, "\n  edges_undir  = ");
    print_num_inline(sink, m_undir);
    print_str(sink, "\n  edges_dir    = ");
    print_num_inline(sink, edge_count);
    print_str(sink, "\n  density      = ");
    print_ppm3(sink, density_ppm as u64);
    print_str(sink, "\n  k_max        = ");
    print_num_inline(sink, max_k as usize);
    print_str(sink, "\n  avg_k        = ");
    print_ppm3(sink, avg_k_ppm as u64);
    print_str(sink, "\n");

    // ── Clustering / transitivity ─────────────────────────────────────────────
    set_color(sink, 14, 0);
    print_str(sink, "  [clustering]\n");
    set_color(sink, 7, 0);
    print_str(sink, "  global CC    = ");
    print_ppm3(sink, trans_ppm as u64);
    print_str(sink, "  (transitivity)\n  avg CC       = ");
    print_ppm3(sink, avgcc_ppm as u64);
    print_str(sink, "  (Watts-Strogatz)\n");

    // ── Efficiency ───────────────────────────────────────────────────────────
    set_color(sink, 14, 0);
    print_str(sink, "  [efficiency]\n");
    set_color(sink, 7, 0);
    print_str(sink, "  E_global     = ");
    print_ppm3(sink, geff_ppm as u64);
    print_str(sink, "\n  E_local      = ");
    print_ppm3(sink, leff_ppm as u64);
    print_str(sink, "\n");

    // ── Small-world & scale-free ──────────────────────────────────────────────
    set_color(sink, 14, 0);
    print_str(sink, "  [network model]\n");
    set_color(sink, 7, 0);
    print_str(sink, "  \u{03c3} (small-world) = ");
    if sigma_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "undef");
        set_color(sink, 7, 0);
    } else {
        print_ppm3(sink, sigma_ppm as u64);
    }
    print_str(sink, "\n  \u{03ba} (heterogen.)  = ");
    if kappa_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "undef");
        set_color(sink, 7, 0);
    } else {
        print_ppm3(sink, kappa_ppm as u64);
    }
    print_str(sink, "\n  \u{03b3}\u{0302} (power-law)  = ");
    if gamma_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "undef");
        set_color(sink, 7, 0);
    } else {
        print_ppm3(sink, gamma_ppm as u64);
    }
    print_str(sink, "\n");

    // ── Classification banner ─────────────────────────────────────────────────
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    // Small-world label.
    if sigma_ppm > 1_000_000 {
        set_color(sink, 10, 0);
        print_str(sink, "  \u{03c3}>1: small-world network");
    } else if sigma_ppm > 0 {
        set_color(sink, 7, 0);
        print_str(sink, "  \u{03c3}\u{2248}1: random-like topology");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  \u{03c3}: insufficient connectivity for small-world test");
    }
    print_str(sink, "\n");
    // Scale-free label.
    let avg3 = (avg_k_ppm as u64).saturating_mul(3);
    let avg2 = (avg_k_ppm as u64).saturating_mul(2);
    if kappa_ppm as u64 > avg3 {
        set_color(sink, 10, 0);
        print_str(sink, "  \u{03ba}\u{226b}\u{27e8}k\u{27e9}: likely scale-free\n");
    } else if kappa_ppm as u64 > avg2 {
        set_color(sink, 14, 0);
        print_str(sink, "  \u{03ba}>2\u{27e8}k\u{27e9}: heterogeneous degree distribution\n");
    } else {
        set_color(sink, 7, 0);
        print_str(sink, "  \u{03ba}\u{2248}\u{27e8}k\u{27e9}: homogeneous (regular/random-like)\n");
    }
    // Power-law exponent γ̂ classification.
    if gamma_ppm == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  \u{03b3}\u{0302}: MLE undefined (all non-isolated k=1 or no edges)\n");
    } else if gamma_ppm <= 3_000_000 {
        set_color(sink, 10, 0);
        print_str(sink, "  \u{03b3}\u{0302}\u{2208}[1,3]: compatible with power-law / scale-free tail\n");
    } else if gamma_ppm <= 4_000_000 {
        set_color(sink, 14, 0);
        print_str(sink, "  \u{03b3}\u{0302}\u{2208}(3,4]: steep tail; weakly heterogeneous\n");
    } else {
        set_color(sink, 7, 0);
        print_str(sink, "  \u{03b3}\u{0302}>4: not consistent with power-law degree distribution\n");
    }
    set_color(sink, 7, 0);
}

/// V2.60: `node attr list u8` / `nattr list u8` — show all nodes with a u8 attribute set.
///
/// Prints a table of (VectorAddress, decimal val) for every occupied slot in
/// node_props_u8, plus a slot-usage footer.  Useful for theme and signal-val audits.
pub fn dispatch_node_attr_list_u8(sink: &ConsoleSink) {
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);

    set_color(sink, 11, 0);
    print_str(sink, " node attr list u8\n");
    set_color(sink, 7, 0);

    if count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no u8 attributes set)\n");
        set_color(sink, 7, 0);
    } else {
        let mut i = 0usize;
        while i < count {
            let mut vec_line = LineBuf::<20>::new();
            vec_line.push_vector(vecs[i]);
            let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");
            set_color(sink, 10, 0);
            print_str(sink, "  ");
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "  val=");
            print_num_inline(sink, vals[i] as usize);
            print_str(sink, "\n");
            i += 1;
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, "  ");
    print_num_inline(sink, count);
    print_str(sink, " / ");
    print_num_inline(sink, gos_runtime::MAX_NODE_PROPS_U8);
    print_str(sink, " slots used\n");
    set_color(sink, 7, 0);
}

/// V2.59: `graph density` — display the ratio of actual edges to max-possible edges.
///
/// For a directed graph: density = E / (N*(N-1)).
/// Prints density as both ppm and percentage, plus raw node/edge counts.
pub fn dispatch_graph_density(sink: &ConsoleSink) {
    let (density_ppm, n, e) = gos_runtime::graph_density();
    set_color(sink, 11, 0);
    print_str(sink, " graph density\n");
    set_color(sink, 7, 0);

    if n < 2 {
        set_color(sink, 8, 0);
        print_str(sink, "  density: undefined (fewer than 2 nodes)\n");
        set_color(sink, 7, 0);
    } else {
        // Print "density: NNN.NNN% (E=E N=N max=N*(N-1))"
        let pct_int  = density_ppm / 10_000;
        let pct_frac = (density_ppm % 10_000) / 100;
        set_color(sink, 10, 0);
        print_str(sink, "  density: ");
        print_num_inline(sink, pct_int as usize);
        print_str(sink, ".");
        if pct_frac < 10 { print_str(sink, "0"); }
        print_num_inline(sink, pct_frac as usize);
        print_str(sink, "%");
        set_color(sink, 8, 0);
        print_str(sink, "  (");
        print_num_inline(sink, density_ppm as usize);
        print_str(sink, " ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, "  nodes=");
    print_num_inline(sink, n);
    print_str(sink, "  edges=");
    print_num_inline(sink, e);
    if n >= 2 {
        print_str(sink, "  max=");
        print_num_inline(sink, n * (n - 1));
    }
    print_str(sink, "\n");
    set_color(sink, 7, 0);
}

/// `watch` / `graph watch` / `watch proc` / `watch nodes` — enter live proc watch mode.
///
/// Sets WATCH_PROC_MODE = 1 so the heartbeat repaints the VECTOR DECK panel with a
/// live proc table on every tick (like `watch -n1 proc`).  Any key press exits watch mode.
pub fn dispatch_watch_proc(sink: &ConsoleSink) {
    WATCH_PROC_MODE.store(1, Ordering::SeqCst);
    set_color(sink, 11, 0);
    print_str(sink, " watch proc  (live view in VECTOR DECK — press any key to stop)\n");
    set_color(sink, 7, 0);
}

/// `watch stop` / `watch exit` — exit live proc watch mode explicitly.
pub fn dispatch_watch_stop(sink: &ConsoleSink) {
    WATCH_PROC_MODE.store(0, Ordering::SeqCst);
    set_color(sink, 8, 0);
    print_str(sink, " watch stopped\n");
    set_color(sink, 7, 0);
}

/// `node log clear <vec>` / `nlog clear <vec>` — clear per-node lifecycle event log.
///
/// Analogous to `journalctl --vacuum-time` or `truncate -s0 /var/log/…`:
/// discards the stored lifecycle history for one node.  Useful after node
/// recovery to obtain a clean-slate log for subsequent monitoring.
pub fn dispatch_node_log_clear(sink: &ConsoleSink, vec: VectorAddress) {
    let mut vec_line = LineBuf::<20>::new();
    vec_line.push_vector(vec);
    let vec_str = core::str::from_utf8(vec_line.as_slice()).unwrap_or("?");

    match gos_runtime::clear_node_log(vec) {
        Err(_) => {
            set_color(sink, 12, 0);
            print_str(sink, " node not found: ");
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
        Ok(()) => {
            set_color(sink, 10, 0);
            print_str(sink, " node log cleared  ");
            set_color(sink, 8, 0);
            print_str(sink, vec_str);
            print_str(sink, "\n");
            set_color(sink, 7, 0);
        }
    }
}

fn lifecycle_log_entry(lc: u8) -> (&'static str, u8) {
    use gos_protocol::NodeLifecycle;
    match lc {
        x if x == NodeLifecycle::Discovered  as u8 => ("Discovered ", 8),
        x if x == NodeLifecycle::Loaded      as u8 => ("Loaded     ", 7),
        x if x == NodeLifecycle::Registered  as u8 => ("Registered ", 10),
        x if x == NodeLifecycle::Allocated   as u8 => ("Allocated  ", 9),
        x if x == NodeLifecycle::Ready       as u8 => ("Ready      ", 10),
        x if x == NodeLifecycle::Running     as u8 => ("Running    ", 14),
        x if x == NodeLifecycle::Waiting     as u8 => ("Waiting    ", 13),
        x if x == NodeLifecycle::Suspended   as u8 => ("Suspended  ", 11),
        x if x == NodeLifecycle::Terminated  as u8 => ("Terminated ", 8),
        x if x == NodeLifecycle::Faulted     as u8 => ("Faulted    ", 12),
        _                                          => ("?          ", 8),
    }
}

/// `graph topo` — hierarchical L4-domain topology view.
///
/// Analogous to `ip route show` / `lshw -short`: reveals how live nodes are
/// distributed across the l4 domain layer of the vector address space.
///
/// - `graph topo`      → count per l4 domain (overview, like `ip route`)
/// - `graph topo <L4>` → list all nodes in that domain (like `ip link show`)
pub fn dispatch_graph_topo(sink: &ConsoleSink, l4_filter: Option<u8>) {
    use gos_protocol::GraphNodeSummary;

    if let Some(l4) = l4_filter {
        // Filtered detail view for a specific l4 domain.
        const PAGE: usize = 16;
        let mut items = [GraphNodeSummary::EMPTY; PAGE];
        let mut offset = 0usize;
        let mut printed = 0usize;

        set_color(sink, 11, 0);
        print_str(sink, " graph topology  l4=");
        print_num_inline(sink, l4 as usize);
        print_str(sink, "\n");
        set_color(sink, 7, 0);

        loop {
            let (total, returned) = gos_runtime::node_page_l4::<PAGE>(l4, offset, &mut items);
            for item in items.iter().take(returned) {
                let fg: u8 = match item.lifecycle {
                    gos_protocol::NodeLifecycle::Ready | gos_protocol::NodeLifecycle::Running => 10,
                    gos_protocol::NodeLifecycle::Faulted => 12,
                    gos_protocol::NodeLifecycle::Waiting | gos_protocol::NodeLifecycle::Suspended => 14,
                    _ => 7,
                };
                set_color(sink, fg, 0);
                print_str(sink, "  ");
                let mut vec_buf = LineBuf::<20>::new();
                vec_buf.push_vector(item.vector);
                let vec_str = core::str::from_utf8(vec_buf.as_slice()).unwrap_or("?");
                print_str(sink, vec_str);
                let pad = 16usize.saturating_sub(vec_str.len());
                for _ in 0..pad { print_str(sink, " "); }
                set_color(sink, 8, 0);
                print_str(sink, lifecycle_label(item.lifecycle));
                print_str(sink, "  ");
                set_color(sink, 7, 0);
                print_str(sink, item.plugin_name);
                print_str(sink, "/");
                print_str(sink, item.local_node_key);
                print_str(sink, "\n");
                printed += 1;
            }
            offset += returned;
            if returned == 0 || offset >= total { break; }
        }

        if printed == 0 {
            set_color(sink, 8, 0);
            print_str(sink, "  (no nodes in l4=");
            print_num_inline(sink, l4 as usize);
            print_str(sink, ")\n");
        }
        set_color(sink, 8, 0);
        print_str(sink, "  total: ");
        print_num_inline(sink, gos_runtime::node_count_for_l4(l4));
        print_str(sink, " node(s) in l4=");
        print_num_inline(sink, l4 as usize);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
    } else {
        // Overview: bucket all live nodes by l4 domain.
        set_color(sink, 11, 0);
        print_str(sink, " graph topology\n");
        set_color(sink, 7, 0);

        const MAX_DOMAINS: usize = 64;
        let mut domain_l4s    = [0u8;    MAX_DOMAINS];
        let mut domain_counts = [0usize; MAX_DOMAINS];
        let mut num_domains   = 0usize;
        let mut grand_total   = 0usize;

        const SCAN: usize = 16;
        let mut scan_items = [GraphNodeSummary::EMPTY; SCAN];
        let mut scan_off   = 0usize;
        loop {
            let (total, returned) = gos_runtime::node_page::<SCAN>(scan_off, &mut scan_items);
            for item in scan_items.iter().take(returned) {
                let l4 = item.vector.l4;
                let mut found = false;
                for d in 0..num_domains {
                    if domain_l4s[d] == l4 {
                        domain_counts[d] += 1;
                        found = true;
                        break;
                    }
                }
                if !found && num_domains < MAX_DOMAINS {
                    domain_l4s[num_domains] = l4;
                    domain_counts[num_domains] = 1;
                    num_domains += 1;
                }
                grand_total += 1;
            }
            scan_off += returned;
            if returned == 0 || scan_off >= total { break; }
        }

        // Insertion-sort domain_l4s/domain_counts by l4 value for stable output.
        let mut i = 1usize;
        while i < num_domains {
            let key_l4  = domain_l4s[i];
            let key_cnt = domain_counts[i];
            let mut j = i;
            while j > 0 && domain_l4s[j - 1] > key_l4 {
                domain_l4s[j]    = domain_l4s[j - 1];
                domain_counts[j] = domain_counts[j - 1];
                j -= 1;
            }
            domain_l4s[j]    = key_l4;
            domain_counts[j] = key_cnt;
            i += 1;
        }

        for d in 0..num_domains {
            let l4    = domain_l4s[d];
            let count = domain_counts[d];
            set_color(sink, 11, 0);
            print_str(sink, "  [l4=");
            print_num_inline(sink, l4 as usize);
            print_str(sink, "]");
            // Pad so node counts align at a consistent column.
            let digits = if l4 < 10 { 1usize } else if l4 < 100 { 2 } else { 3 };
            for _ in digits..3 { print_str(sink, " "); }
            set_color(sink, 7, 0);
            print_str(sink, "  ");
            print_num_inline(sink, count);
            print_str(sink, if count == 1 { " node\n" } else { " nodes\n" });
        }

        if num_domains == 0 {
            set_color(sink, 8, 0);
            print_str(sink, "  (no nodes)\n");
        }
        set_color(sink, 8, 0);
        print_str(sink, "  ");
        print_num_inline(sink, num_domains);
        print_str(sink, " domain(s)  |  ");
        print_num_inline(sink, grand_total);
        print_str(sink, " total node(s)");
        if num_domains > 0 {
            print_str(sink, "  |  use 'graph topo <l4>' to list a domain\n");
        } else {
            print_str(sink, "\n");
        }
        set_color(sink, 7, 0);
    }
}

/// Holistic system health report — the GOS equivalent of `systemctl status`
/// combined with `dmesg --level=err,warn`.
///
/// Aggregates node fault counts, diff ring saturation, subscription pairs,
/// runtime preemptions, domain switches, and boot manifest results into a
/// single colour-coded health banner + detail table.
pub fn dispatch_graph_health(sink: &ConsoleSink) {
    let total        = gos_runtime::proc_count();
    let faulted      = gos_runtime::faulted_node_count();
    let epoch        = gos_runtime::graph_epoch();
    let diff_fill    = gos_runtime::diff_ring_fill();
    let diff_tot     = gos_runtime::diff_total();
    let sub_pairs    = gos_runtime::subscribe_pair_count();
    let preempts     = gos_runtime::preempt_count();
    let dom_sw       = gos_runtime::domain_switch_count();
    let rules        = gos_runtime::boot_manifest_rules_checked();
    let healed       = gos_runtime::boot_manifest_edges_healed();
    let edge_count   = gos_runtime::snapshot().edge_count;

    // Health classification: DEGRADED > WARNING > OK
    // DEGRADED: faulted nodes exceed 25 % of total (or any fault when total < 4)
    // WARNING:  any faulted node, or diff ring > 93 % full (>= 120/128)
    let degraded = faulted > 0 && (total < 4 || faulted * 4 >= total);
    let warning  = faulted > 0 || diff_fill >= 120;

    // Banner
    if degraded {
        set_color(sink, 15, 4); // white on red
        print_str(sink, " graph health  DEGRADED                                                     ");
    } else if warning {
        set_color(sink, 0, 14); // black on yellow
        print_str(sink, " graph health  WARNING                                                      ");
    } else {
        set_color(sink, 0, 10); // black on green
        print_str(sink, " graph health  OK                                                           ");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    // Nodes
    set_color(sink, 11, 0);
    print_str(sink, "  nodes\n");
    set_color(sink, 7, 0);
    print_str(sink, "    total:    ");
    print_num_inline(sink, total);
    print_str(sink, "\n");
    print_str(sink, "    faulted:  ");
    if faulted > 0 { set_color(sink, 12, 0); }
    print_num_inline(sink, faulted);
    if faulted > 0 {
        print_str(sink, "  (!!)");
        set_color(sink, 7, 0);
    }
    print_str(sink, "\n");
    print_str(sink, "    edges:    ");
    print_num_inline(sink, edge_count);
    print_str(sink, "\n");
    print_str(sink, "    subs:     ");
    print_num_inline(sink, sub_pairs);
    print_str(sink, "  subscribe pairs\n");

    // Graph mutation activity
    set_color(sink, 11, 0);
    print_str(sink, "  mutations\n");
    set_color(sink, 7, 0);
    print_str(sink, "    epoch:    ");
    print_num_inline(sink, epoch as usize);
    print_str(sink, "\n");
    print_str(sink, "    total:    ");
    print_num_inline(sink, diff_tot as usize);
    print_str(sink, "  structural diffs ever pushed\n");
    print_str(sink, "    ring:     ");
    if diff_fill >= 120 { set_color(sink, 12, 0); }
    print_num_inline(sink, diff_fill);
    print_str(sink, " / 128");
    if diff_fill >= 120 {
        print_str(sink, "  (near full — oldest entries being overwritten)");
        set_color(sink, 7, 0);
    }
    print_str(sink, "\n");

    // Runtime metrics
    set_color(sink, 11, 0);
    print_str(sink, "  runtime\n");
    set_color(sink, 7, 0);
    print_str(sink, "    preempts: ");
    print_num_inline(sink, preempts as usize);
    print_str(sink, "  scheduler preemptions\n");
    print_str(sink, "    dom-sw:   ");
    print_num_inline(sink, dom_sw as usize);
    print_str(sink, "  l4 domain switches\n");

    // Boot manifest
    set_color(sink, 11, 0);
    print_str(sink, "  boot\n");
    set_color(sink, 7, 0);
    print_str(sink, "    rules:    ");
    print_num_inline(sink, rules);
    print_str(sink, "  manifest rules checked\n");
    print_str(sink, "    healed:   ");
    if healed > 0 { set_color(sink, 14, 0); }
    print_num_inline(sink, healed);
    if healed > 0 {
        print_str(sink, "  edges auto-healed at boot");
        set_color(sink, 7, 0);
    }
    print_str(sink, "\n");

    // Advisory if not OK
    if degraded {
        set_color(sink, 12, 0);
        print_str(sink, "  action: run 'nodes faulted' to inspect faulted nodes\n");
        set_color(sink, 7, 0);
    } else if faulted > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  hint: run 'nodes faulted' for fault details\n");
        set_color(sink, 7, 0);
    }
}

/// V2.31: `graph path <from> <to>` — BFS shortest-path trace between two nodes.
///
/// Analogous to `traceroute` on Linux / `pathping` on Windows: shows the sequence
/// of graph hops from one node vector to another, following directed edges.
/// Prints each hop's vector and node key so operators can trace data-flow paths
/// through the graph topology at a glance.
pub fn dispatch_graph_path(sink: &ConsoleSink, from: VectorAddress, to: VectorAddress) {
    const MAX_PATH: usize = 32;
    let mut path = [VectorAddress::new(0, 0, 0, 0); MAX_PATH];
    let (filled_path, path_len) = gos_runtime::find_graph_path::<MAX_PATH>(from, to);
    for i in 0..MAX_PATH {
        path[i] = filled_path[i];
    }

    // Header banner
    set_color(sink, 0, 11); // black on cyan
    print_str(sink, " GRAPH PATH ");
    set_color(sink, 11, 0);
    let mut from_buf = LineBuf::<20>::new();
    from_buf.push_vector(from);
    print_str(sink, core::str::from_utf8(from_buf.as_slice()).unwrap_or("?"));
    print_str(sink, " \u{2192} "); // →
    let mut to_buf = LineBuf::<20>::new();
    to_buf.push_vector(to);
    print_str(sink, core::str::from_utf8(to_buf.as_slice()).unwrap_or("?"));
    print_str(sink, "\n");
    set_color(sink, 7, 0);

    if path_len == 0 {
        set_color(sink, 12, 0);
        print_str(sink, "  no path found (nodes unreachable or not registered)\n");
        set_color(sink, 7, 0);
        return;
    }

    // Hop list
    print_str(sink, "\n");
    for i in 0..path_len {
        let vec = path[i];
        // hop number
        print_str(sink, "  hop ");
        if i + 1 < 10 {
            print_str(sink, " ");
        }
        print_num_inline(sink, i + 1);
        print_str(sink, "   ");

        // vector address
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vstr = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vstr);
        // pad to 12 chars
        let pad = 12usize.saturating_sub(vstr.len());
        for _ in 0..pad {
            print_str(sink, " ");
        }
        print_str(sink, "  ");

        // node key + plugin name
        if let Some(summary) = gos_runtime::node_summary(vec) {
            // colour: first hop and last hop get accent colour
            if i == 0 || i + 1 == path_len {
                set_color(sink, 10, 0); // green = endpoints
            } else {
                set_color(sink, 14, 0); // yellow = intermediate
            }
            print_str(sink, summary.local_node_key);
            set_color(sink, 8, 0);
            print_str(sink, "  (");
            print_str(sink, summary.plugin_name);
            print_str(sink, ")");
            set_color(sink, 7, 0);
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "(unregistered)");
            set_color(sink, 7, 0);
        }
        print_str(sink, "\n");
    }

    // Footer
    print_str(sink, "\n");
    set_color(sink, 11, 0);
    print_num_inline(sink, path_len);
    print_str(sink, " hop");
    if path_len != 1 { print_str(sink, "s"); }
    set_color(sink, 7, 0);
    print_str(sink, "  |  from: ");
    let mut fb2 = LineBuf::<20>::new();
    fb2.push_vector(from);
    print_str(sink, core::str::from_utf8(fb2.as_slice()).unwrap_or("?"));
    print_str(sink, "  |  to: ");
    let mut tb2 = LineBuf::<20>::new();
    tb2.push_vector(to);
    print_str(sink, core::str::from_utf8(tb2.as_slice()).unwrap_or("?"));
    print_str(sink, "\n");
}

/// V2.32: `graph cycles` / `cycles` — directed cycle detection in the live graph.
///
/// Analogous to `tsort` detecting circular dependencies, or `cargo`'s
/// dependency-cycle error: scans every directed edge via iterative 3-color DFS and
/// reports the first back-edge-closed cycle found.  If the graph is acyclic (a DAG)
/// the command confirms this — useful for verifying that plugin dependency graphs,
/// signal routing graphs, and causal chains remain deadlock-free.
///
/// Output shows each node on the cycle path from the entry node back to itself, so
/// operators can see which plugins/nodes form the ring.
pub fn dispatch_graph_cycles(sink: &ConsoleSink) {
    const MAX_CYCLE: usize = 32;
    let (cycle, cycle_len) = gos_runtime::find_graph_cycle::<MAX_CYCLE>();

    set_color(sink, 0, 11); // black on cyan
    print_str(sink, " GRAPH CYCLES ");
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    if cycle_len == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "  no cycles detected");
        set_color(sink, 8, 0);
        print_str(sink, "  (directed acyclic graph)\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 12, 0);
    print_str(sink, "  CYCLE DETECTED  ");
    set_color(sink, 8, 0);
    print_num_inline(sink, cycle_len);
    print_str(sink, " nodes\n");
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    for i in 0..cycle_len {
        let vec = cycle[i];
        let is_closing = i + 1 == cycle_len;

        // Indent + hop label
        print_str(sink, "  ");
        if is_closing {
            set_color(sink, 12, 0);
            print_str(sink, "\u{21A9} "); // ↩ (back to cycle start)
        } else {
            set_color(sink, 14, 0);
            print_str(sink, "  ");
        }

        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vstr = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        set_color(sink, if is_closing { 12 } else { 11 }, 0);
        print_str(sink, vstr);

        // Pad vector to 12 chars
        let pad = 12usize.saturating_sub(vstr.len());
        for _ in 0..pad { print_str(sink, " "); }
        print_str(sink, "  ");

        if let Some(summary) = gos_runtime::node_summary(vec) {
            set_color(sink, if is_closing { 12 } else { 14 }, 0);
            print_str(sink, summary.local_node_key);
            set_color(sink, 8, 0);
            print_str(sink, "  (");
            print_str(sink, summary.plugin_name);
            print_str(sink, ")");
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "(unregistered)");
        }

        if !is_closing && i + 2 < cycle_len {
            // Arrow pointing to next
            set_color(sink, 8, 0);
            print_str(sink, "  \u{2193}"); // ↓
        }

        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    print_str(sink, "\n");
    set_color(sink, 12, 0);
    print_num_inline(sink, cycle_len - 1);
    print_str(sink, "-node cycle");
    set_color(sink, 8, 0);
    print_str(sink, "  |  hint: graph path <from> <to> to trace a specific route\n");
    set_color(sink, 7, 0);
}

/// V2.33: `graph toposort` — topological ordering of the live node graph.
///
/// Uses Kahn's BFS algorithm (in-degree queue, O(V+E)) to produce a dependency
/// ordering where every source (in-degree 0) precedes its successors.  Analogous
/// to `tsort(1)` on POSIX, `cmake --build` dependency ordering, or `cargo build`'s
/// crate graph resolution — exposes the boot/init ordering of the GOS graph at a glance.
///
/// If the graph contains cycles the command shows the partial sort and warns that
/// cycle detection via `graph cycles` should be run first.
pub fn dispatch_graph_toposort(sink: &ConsoleSink) {
    const MAX_TOPO: usize = 128;
    let (order, order_len, is_dag) = gos_runtime::graph_toposort::<MAX_TOPO>();
    let total = gos_runtime::snapshot().node_count;

    // Header
    set_color(sink, 0, 11); // black on cyan
    print_str(sink, " GRAPH TOPOSORT ");
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  no nodes registered\n");
        set_color(sink, 7, 0);
        return;
    }

    if !is_dag {
        set_color(sink, 12, 0);
        print_str(sink, "  WARNING: graph contains cycles — toposort is incomplete\n");
        set_color(sink, 8, 0);
        print_str(sink, "  run 'graph cycles' to identify the cyclic component\n");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    // Ordered node list
    print_str(sink, "\n");
    for i in 0..order_len {
        let vec = order[i];

        // Rank number (1-based)
        print_str(sink, "  ");
        if i + 1 < 10  { print_str(sink, "  "); }
        else if i + 1 < 100 { print_str(sink, " "); }
        print_num_inline(sink, i + 1);
        print_str(sink, "   ");

        // Vector address
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vstr = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        set_color(sink, 11, 0);
        print_str(sink, vstr);
        // Pad vector to 12 chars
        let pad = 12usize.saturating_sub(vstr.len());
        for _ in 0..pad { print_str(sink, " "); }
        print_str(sink, "  ");
        set_color(sink, 7, 0);

        // Node key + plugin
        if let Some(summary) = gos_runtime::node_summary(vec) {
            set_color(sink, 10, 0);
            print_str(sink, summary.local_node_key);
            set_color(sink, 8, 0);
            print_str(sink, "  (");
            print_str(sink, summary.plugin_name);
            print_str(sink, ")");
            set_color(sink, 7, 0);
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "(unregistered)");
            set_color(sink, 7, 0);
        }
        print_str(sink, "\n");
    }

    // Footer
    print_str(sink, "\n");
    if is_dag {
        set_color(sink, 10, 0);
        print_num_inline(sink, order_len);
        print_str(sink, " nodes  ");
        set_color(sink, 8, 0);
        print_str(sink, "(complete DAG — dependency order is unique up to ties)\n");
    } else {
        set_color(sink, 14, 0);
        print_num_inline(sink, order_len);
        print_str(sink, "/");
        print_num_inline(sink, total);
        print_str(sink, " nodes sorted  ");
        set_color(sink, 12, 0);
        print_num_inline(sink, total - order_len);
        print_str(sink, " in cyclic component (unsortable)\n");
    }
    set_color(sink, 7, 0);
}

/// V2.34: `graph scc` — strongly connected components via Kosaraju's algorithm.
///
/// Analogous to `scc(1)` (POSIX graph utilities), `sccmap` (Graphviz), or
/// the condensation step in `cargo build`'s dependency resolver.
///
/// An SCC with > 1 node contains a directed cycle; an SCC with exactly 1 node
/// is either isolated or connected only via forward edges (DAG edges).
/// When scc_count == node_count the graph is a DAG — no directed cycles.
pub fn dispatch_graph_scc(sink: &ConsoleSink) {
    const MAX_SCC: usize = 128;
    let (nodes, labels, total, scc_count) = gos_runtime::graph_scc::<MAX_SCC>();

    // Header
    set_color(sink, 0, 11);
    print_str(sink, " GRAPH SCC ");
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  no nodes registered\n");
        set_color(sink, 7, 0);
        return;
    }

    // Summary line
    print_str(sink, "\n");
    set_color(sink, 11, 0);
    print_num_inline(sink, scc_count);
    set_color(sink, 7, 0);
    print_str(sink, " components  /  ");
    set_color(sink, 11, 0);
    print_num_inline(sink, total);
    set_color(sink, 7, 0);
    print_str(sink, " nodes");
    if scc_count == total {
        set_color(sink, 10, 0);
        print_str(sink, "   (graph is a DAG — no directed cycles)\n");
        set_color(sink, 7, 0);
    } else {
        print_str(sink, "\n");
    }
    print_str(sink, "\n");

    // Per-SCC display: walk through sorted nodes, detect label boundaries
    let mut pos = 0usize;
    while pos < total {
        let cur_label = labels[pos];

        // Count members of this SCC
        let mut end = pos;
        while end < total && labels[end] == cur_label {
            end += 1;
        }
        let size = end - pos;

        // SCC header
        set_color(sink, 0, 11);
        print_str(sink, " SCC #");
        print_num_inline(sink, cur_label as usize);
        print_str(sink, " ");
        set_color(sink, 7, 0);
        print_str(sink, "  ");
        set_color(sink, 11, 0);
        print_num_inline(sink, size);
        set_color(sink, 7, 0);
        if size == 1 {
            print_str(sink, " node");
        } else {
            print_str(sink, " nodes");
            set_color(sink, 12, 0);
            print_str(sink, "  \u{25C6} cycle");
            set_color(sink, 7, 0);
        }
        print_str(sink, "\n");

        // Node list (up to 8 per row for readability)
        let mut col = 0usize;
        for i in pos..end {
            if col == 0 { print_str(sink, "   "); }
            let mut vbuf = LineBuf::<20>::new();
            vbuf.push_vector(nodes[i]);
            let vstr = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
            set_color(sink, 11, 0);
            print_str(sink, vstr);
            set_color(sink, 7, 0);
            col += 1;
            if col == 4 || i + 1 == end {
                print_str(sink, "\n");
                col = 0;
            } else {
                print_str(sink, "  ");
            }
        }

        // Node keys (one per line, indented)
        for i in pos..end {
            if let Some(summary) = gos_runtime::node_summary(nodes[i]) {
                print_str(sink, "   ");
                set_color(sink, 10, 0);
                print_str(sink, summary.local_node_key);
                set_color(sink, 8, 0);
                print_str(sink, "  (");
                print_str(sink, summary.plugin_name);
                print_str(sink, ")");
                set_color(sink, 7, 0);
                print_str(sink, "\n");
            }
        }
        print_str(sink, "\n");

        pos = end;
    }

    // Footer hint
    set_color(sink, 8, 0);
    if scc_count < total {
        print_str(sink, "  hint: graph cycles to trace a specific cycle path\n");
    } else {
        print_str(sink, "  hint: graph toposort to compute the dependency order\n");
    }
    set_color(sink, 7, 0);
}

/// V2.35: `graph condensation` — condensation DAG of the live node graph.
///
/// Collapses each Strongly Connected Component into a single super-node
/// (labelled C#N) and shows the inter-SCC edges as the condensation DAG.
/// The condensation is always a DAG regardless of cycles in the source graph.
/// Analogous to `sccmap -F` (Graphviz) or `cargo tree` inter-package deps.
pub fn dispatch_graph_condensation(sink: &ConsoleSink) {
    const MAX_C: usize = 128;
    let (nodes, labels, total, scc_count, adj, cond_edges) =
        gos_runtime::graph_condensation::<MAX_C>();

    set_color(sink, 0, 11);
    print_str(sink, " GRAPH CONDENSATION ");
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  no nodes registered\n");
        set_color(sink, 7, 0);
        return;
    }

    print_str(sink, "\n");
    set_color(sink, 11, 0);
    print_num_inline(sink, scc_count);
    set_color(sink, 7, 0);
    print_str(sink, " components  /  ");
    set_color(sink, 11, 0);
    print_num_inline(sink, cond_edges);
    set_color(sink, 7, 0);
    print_str(sink, " condensation edges  /  ");
    set_color(sink, 11, 0);
    print_num_inline(sink, total);
    set_color(sink, 7, 0);
    print_str(sink, " nodes\n\n");

    // Per-SCC block: members + outgoing condensation edges.
    let mut pos = 0usize;
    while pos < total {
        let cur_label = labels[pos];
        let mut end = pos;
        while end < total && labels[end] == cur_label { end += 1; }
        let size = end - pos;

        set_color(sink, 0, 11);
        print_str(sink, " C#");
        print_num_inline(sink, cur_label as usize);
        print_str(sink, " ");
        set_color(sink, 7, 0);
        print_str(sink, "  ");
        set_color(sink, 11, 0);
        print_num_inline(sink, size);
        set_color(sink, 7, 0);
        if size == 1 {
            print_str(sink, " node\n");
        } else {
            print_str(sink, " nodes");
            set_color(sink, 12, 0);
            print_str(sink, "  \u{25C6} cycle");
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }

        // Member vectors.
        let mut col = 0usize;
        for i in pos..end {
            if col == 0 { print_str(sink, "   "); }
            let mut vbuf = LineBuf::<20>::new();
            vbuf.push_vector(nodes[i]);
            let vstr = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
            set_color(sink, 11, 0);
            print_str(sink, vstr);
            set_color(sink, 7, 0);
            col += 1;
            if col == 4 || i + 1 == end {
                print_str(sink, "\n");
                col = 0;
            } else {
                print_str(sink, "  ");
            }
        }

        // Node keys.
        for i in pos..end {
            if let Some(summary) = gos_runtime::node_summary(nodes[i]) {
                print_str(sink, "   ");
                set_color(sink, 10, 0);
                print_str(sink, summary.local_node_key);
                set_color(sink, 8, 0);
                print_str(sink, "  (");
                print_str(sink, summary.plugin_name);
                print_str(sink, ")");
                set_color(sink, 7, 0);
                print_str(sink, "\n");
            }
        }

        // Condensation edges from this super-node.
        let ci = cur_label as usize;
        if ci < 128 && adj[ci] != 0 {
            print_str(sink, "   ");
            set_color(sink, 14, 0);
            print_str(sink, "\u{2192} ");
            set_color(sink, 7, 0);
            let mut first = true;
            for j in 0..scc_count {
                if (adj[ci] >> j) & 1 == 1 {
                    if !first { print_str(sink, ", "); }
                    set_color(sink, 11, 0);
                    print_str(sink, "C#");
                    print_num_inline(sink, j);
                    set_color(sink, 7, 0);
                    first = false;
                }
            }
            print_str(sink, "\n");
        }

        print_str(sink, "\n");
        pos = end;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  condensation is always a DAG  |  use 'graph scc' to see cycle details\n");
    set_color(sink, 7, 0);
}

/// V2.36: `graph reachable <from>` — all nodes transitively reachable from
/// a given node vector via directed edges.
///
/// Returns the reachable set sorted by vector address (ascending), excluding
/// the source node itself.  An empty set means either the node is isolated
/// or not registered.
///
/// OS analogy: `systemctl list-dependencies --all <svc>`,
/// `cargo tree -p <crate>`, `ldd --recursive <bin>`.
pub fn dispatch_graph_reachable(sink: &ConsoleSink, from: VectorAddress) {
    const MAX_REACH: usize = 128;
    let (nodes, reach_len) = gos_runtime::graph_reachable::<MAX_REACH>(from);

    set_color(sink, 11, 0);
    print_str(sink, " graph reachable from ");
    let mut from_line = LineBuf::<20>::new();
    from_line.push_vector(from);
    print_str(sink, core::str::from_utf8(from_line.as_slice()).unwrap_or("?"));
    print_str(sink, "\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if reach_len == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no reachable nodes — isolated or not registered)\n");
        set_color(sink, 7, 0);
    } else {
        for i in 0..reach_len {
            let vec = nodes[i];
            print_str(sink, "  ");
            let mut line = LineBuf::<20>::new();
            line.push_vector(vec);
            print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("?"));
            print_str(sink, "\n");
        }
        set_color(sink, 8, 0);
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        print_str(sink, "  ");
        print_num_inline(sink, reach_len);
        set_color(sink, 8, 0);
        print_str(sink, " reachable  |  use 'graph path <from> <to>' to trace a specific route\n");
        set_color(sink, 7, 0);
    }
}

/// V2.37: `graph bipartite` — 2-coloring check on the live directed graph.
///
/// A graph is bipartite iff it contains no odd-length cycle, i.e. every edge
/// connects a node from set A to a node from set B (or vice versa).  The check
/// is performed on the *undirected* projection of the directed live graph.
///
/// Output when bipartite:
///   result:   bipartite
///   set A:    <vec> <vec> ...
///   set B:    <vec> <vec> ...
///
/// Output when not bipartite:
///   result:   NOT bipartite  (odd-length cycle detected)
///
/// OS analogy: `bipartite_check` on a scheduling graph to verify that
/// producers and consumers can be cleanly split into two non-conflicting tiers.
pub fn dispatch_graph_bipartite(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, colors, total, is_bipartite) = gos_runtime::graph_bipartite::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph bipartite\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered — vacuously bipartite)\n");
        set_color(sink, 7, 0);
        return;
    }

    if is_bipartite {
        set_color(sink, 10, 0);
        print_str(sink, "  result:   bipartite\n");
        set_color(sink, 7, 0);

        // Print set A (color 0).
        let mut count_a = 0usize;
        for i in 0..total {
            if colors[i] == 0 { count_a += 1; }
        }
        set_color(sink, 11, 0);
        print_str(sink, "  set A (");
        print_num_inline(sink, count_a);
        print_str(sink, "):  ");
        set_color(sink, 7, 0);
        for i in 0..total {
            if colors[i] == 0 {
                let mut line = LineBuf::<20>::new();
                line.push_vector(vecs[i]);
                print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("?"));
                print_str(sink, "  ");
            }
        }
        print_str(sink, "\n");

        // Print set B (color 1).
        let mut count_b = 0usize;
        for i in 0..total {
            if colors[i] == 1 { count_b += 1; }
        }
        set_color(sink, 11, 0);
        print_str(sink, "  set B (");
        print_num_inline(sink, count_b);
        print_str(sink, "):  ");
        set_color(sink, 7, 0);
        for i in 0..total {
            if colors[i] == 1 {
                let mut line = LineBuf::<20>::new();
                line.push_vector(vecs[i]);
                print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("?"));
                print_str(sink, "  ");
            }
        }
        print_str(sink, "\n");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "  result:   NOT bipartite  (odd-length cycle detected)\n");
        set_color(sink, 8, 0);
        print_str(sink, "  hint: use 'graph cycles' to find the cycle, 'graph scc' for components\n");
        set_color(sink, 7, 0);
    }

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s) checked\n");
    set_color(sink, 7, 0);
}

/// V2.38: `graph degree` — in/out degree census + hub identification.
///
/// Prints a table sorted by descending total degree.  Nodes are annotated:
///   hub      — high connectivity (total ≥ 3 and ≥ half of max total degree)
///   source   — no incoming edges (out > 0, in == 0)
///   sink     — no outgoing edges (out == 0, in > 0)
///   isolated — no edges at all (out == 0, in == 0)
///
/// OS analogy: `ip -s link show` / `netstat -s` per-interface TX/RX statistics.
pub fn dispatch_graph_degree(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph degree\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 7, 0);
        set_color(sink, 8, 0);
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        return;
    }

    // Header row.
    set_color(sink, 8, 0);
    print_str(sink, "  vector           out    in   total  role\n");
    set_color(sink, 7, 0);

    // Hub threshold: total_degree >= 3 AND >= ceil(max_total / 2).
    let mut max_total = 0u32;
    for i in 0..total {
        let t = (out_degs[i] as u32) + (in_degs[i] as u32);
        if t > max_total { max_total = t; }
    }
    let hub_thresh: u32 = if max_total >= 3 { (max_total + 1) / 2 } else { u32::MAX };
    let mut hub_count = 0usize;

    for i in 0..total {
        let t     = (out_degs[i] as u32) + (in_degs[i] as u32);
        let is_hub      = t >= 3 && t >= hub_thresh;
        let is_isolated = out_degs[i] == 0 && in_degs[i] == 0;
        let is_sink     = out_degs[i] == 0 && in_degs[i] > 0;
        let is_source   = in_degs[i] == 0 && out_degs[i] > 0;
        if is_hub { hub_count += 1; }

        if is_hub {
            set_color(sink, 14, 0); // bright yellow
        } else if is_isolated {
            set_color(sink, 8, 0);  // dark grey
        } else {
            set_color(sink, 7, 0);  // white
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        // Pad vector string to 14 chars.
        let vlen = vec_str.len();
        for _ in vlen..14 { print_str(sink, " "); }

        // out-degree (right-aligned, 4 wide, green)
        set_color(sink, 10, 0);
        print_str(sink, "  ");
        print_num_right4(sink, out_degs[i] as usize);

        // in-degree (right-aligned, 4 wide, red)
        set_color(sink, 12, 0);
        print_str(sink, "  ");
        print_num_right4(sink, in_degs[i] as usize);

        // total (right-aligned, 4 wide, white)
        set_color(sink, 7, 0);
        print_str(sink, "  ");
        print_num_right4(sink, t as usize);
        print_str(sink, "  ");

        // role label
        if is_hub {
            set_color(sink, 14, 0);
            print_str(sink, "hub");
        } else if is_isolated {
            set_color(sink, 8, 0);
            print_str(sink, "isolated");
        } else if is_sink {
            set_color(sink, 11, 0);
            print_str(sink, "sink");
        } else if is_source {
            set_color(sink, 10, 0);
            print_str(sink, "source");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  max-total-degree: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, max_total as usize);
    if hub_count > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  hubs: ");
        print_num_inline(sink, hub_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.39: `graph centrality` — betweenness centrality per node (Brandes, directed).
///
/// Identifies nodes that sit on the most shortest paths between other nodes.
/// Betweenness centrality BC[v] = Σ_{s≠v≠t} σ(s,t,v)/σ(s,t) — the fraction
/// of all-pairs shortest paths that pass through v, summed over all pairs.
///
/// High BC → critical routing bottleneck (removing it disrupts most paths).
/// BC = 0  → node is never an intermediary (leaf, isolated, or parallel routes).
///
/// Output: table sorted descending by BC score, annotated with role:
///   bottleneck — BC > 0, most critical intermediary in the graph
///   relay      — BC > 0, carries some cross-node traffic
///   endpoint   — BC = 0 (leaf / source / sink with no intermediary role)
///
/// OS analogy: `ip route show` + `traceroute` hop-frequency analysis — which
/// kernel service node lies on the most inter-service communication paths?
pub fn dispatch_graph_centrality(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, bc, total) = gos_runtime::graph_centrality::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph centrality\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              bc    role\n");
    set_color(sink, 7, 0);

    // Find max BC for relative annotation.
    let mut max_bc = 0u32;
    for i in 0..total {
        if bc[i] > max_bc { max_bc = bc[i]; }
    }

    let mut bottleneck_count = 0usize;

    for i in 0..total {
        let score   = bc[i];
        let is_top  = max_bc > 0 && score == max_bc;
        let is_relay = score > 0 && !is_top;

        if is_top {
            set_color(sink, 14, 0); // bright yellow — top bottleneck
        } else if is_relay {
            set_color(sink, 11, 0); // cyan — relay
        } else {
            set_color(sink, 8, 0);  // dark grey — endpoint
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);

        // Pad vector to 16 chars.
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        // BC score (right-aligned, 6 wide).
        set_color(sink, if is_top { 14 } else if is_relay { 11 } else { 8 }, 0);
        print_str(sink, " ");
        print_num_right6(sink, score as usize);
        print_str(sink, "  ");

        // Role label.
        if is_top {
            set_color(sink, 14, 0);
            print_str(sink, "bottleneck");
            bottleneck_count += 1;
        } else if is_relay {
            set_color(sink, 11, 0);
            print_str(sink, "relay");
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "endpoint");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  max-bc: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, max_bc as usize);
    if bottleneck_count > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  bottlenecks: ");
        print_num_inline(sink, bottleneck_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.53: `graph between` — weighted betweenness centrality per node (Brandes + Dijkstra).
///
/// Like `graph centrality` (V2.39) but uses `edge.spec.weight` to find
/// minimum-weight paths instead of minimum-hop paths.  Diverges from the
/// unweighted version when a low-weight indirect path is cheaper than a
/// high-weight direct edge.  Uniform-weight graphs produce identical results.
///
/// WBC[v] = Σ_{s≠v≠t} σ_w(s,t,v)/σ_w(s,t) — the fraction of all-pairs
/// minimum-weight paths that pass through v, summed over all pairs.
///
/// Output: table sorted descending by WBC score, annotated with role:
///   keystone   — WBC = max, most critical weighted routing node
///   relay      — WBC > 0, carries some minimum-weight traffic
///   endpoint   — WBC = 0 (leaf / isolated / not on any shortest-weight path)
///
/// OS analogy: `traceroute` with latency weights — which kernel service node
/// sits on the most minimum-latency paths between other service pairs?
pub fn dispatch_graph_between(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, wbc, total) = gos_runtime::graph_between::<MAX_N>();

    set_color(sink, 13, 0); // bright magenta
    print_str(sink, " graph between  (weighted Dijkstra)\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              wbc   role\n");
    set_color(sink, 7, 0);

    let mut max_wbc = 0u32;
    for i in 0..total {
        if wbc[i] > max_wbc { max_wbc = wbc[i]; }
    }

    let mut keystone_count = 0usize;

    for i in 0..total {
        let score    = wbc[i];
        let is_top   = max_wbc > 0 && score == max_wbc;
        let is_relay = score > 0 && !is_top;

        if is_top {
            set_color(sink, 13, 0); // bright magenta — keystone
        } else if is_relay {
            set_color(sink, 11, 0); // cyan — relay
        } else {
            set_color(sink, 8, 0);  // dark grey — endpoint
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);

        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        set_color(sink, if is_top { 13 } else if is_relay { 11 } else { 8 }, 0);
        print_str(sink, " ");
        print_num_right6(sink, score as usize);
        print_str(sink, "  ");

        if is_top {
            set_color(sink, 13, 0);
            print_str(sink, "keystone");
            keystone_count += 1;
        } else if is_relay {
            set_color(sink, 11, 0);
            print_str(sink, "relay");
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "endpoint");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  max-wbc: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, max_wbc as usize);
    if keystone_count > 0 {
        set_color(sink, 13, 0);
        print_str(sink, "  keystones: ");
        print_num_inline(sink, keystone_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.40: `graph closeness` — outgoing closeness centrality per node (directed BFS).
///
/// Closeness centrality measures how quickly a node can reach all other nodes
/// via directed edges.  For node v:
///   CC[v] = r_v × 1_000_000 / Σ_{u reachable from v, u≠v} d(v,u)
/// where r_v = number of nodes reachable from v (excl. v), d(v,u) = BFS distance.
/// Isolated nodes (r_v = 0) → CC[v] = 0.
///
/// High CC → node can reach the rest of the graph in very few directed hops.
/// CC = 0  → node is isolated or cannot reach any other node (pure sink).
///
/// Output: table sorted descending by CC score, annotated with role:
///   central    — top CC score: most efficiently connected broadcaster
///   relay      — moderate CC: reaches others but not the most efficiently
///   peripheral — zero or very low CC: isolated or deep in the graph
///
/// OS analogy: `ping` average RTT census — which kernel service node can
/// broadcast to all others via the fewest outgoing hops on average?
pub fn dispatch_graph_closeness(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, cc, total) = gos_runtime::graph_closeness::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph closeness\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              cc    role\n");
    set_color(sink, 7, 0);

    let mut max_cc = 0u32;
    for i in 0..total {
        if cc[i] > max_cc { max_cc = cc[i]; }
    }

    // Threshold: "relay" if CC > 0 and < max; "central" if CC == max (and > 0).
    let mut central_count = 0usize;

    for i in 0..total {
        let score      = cc[i];
        let is_central = max_cc > 0 && score == max_cc;
        let is_relay   = score > 0 && !is_central;

        if is_central {
            set_color(sink, 14, 0); // bright yellow
        } else if is_relay {
            set_color(sink, 11, 0); // cyan
        } else {
            set_color(sink, 8, 0);  // dark grey
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);

        // Pad vector to 16 chars.
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        // CC score (right-aligned, 6 wide).
        set_color(sink, if is_central { 14 } else if is_relay { 11 } else { 8 }, 0);
        print_str(sink, " ");
        print_num_right6(sink, score as usize);
        print_str(sink, "  ");

        // Role label.
        if is_central {
            set_color(sink, 14, 0);
            print_str(sink, "central");
            central_count += 1;
        } else if is_relay {
            set_color(sink, 11, 0);
            print_str(sink, "relay");
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "peripheral");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  max-cc: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, max_cc as usize);
    set_color(sink, 8, 0);
    print_str(sink, " (×1e-6)");
    if central_count > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  central: ");
        print_num_inline(sink, central_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.41: `graph eccentricity` — per-node worst-case directed hop count.
///
/// Displays eccentricity for every node plus the graph radius and diameter.
/// Sorted ascending so centre nodes (ecc == radius) appear first.
/// Isolated nodes (ecc = 0, no reachable neighbours) appear last in dark grey.
///
/// OS analogy: `traceroute` worst-case latency census.
pub fn dispatch_graph_eccentricity(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, ecc, total, radius, diameter) = gos_runtime::graph_eccentricity::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph eccentricity\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              ecc   role\n");
    set_color(sink, 7, 0);

    let mut center_count = 0usize;

    for i in 0..total {
        let score        = ecc[i];
        let is_center    = radius > 0 && score == radius;
        let is_periphery = diameter > 0 && score == diameter && score != radius;
        let is_isolated  = score == 0;

        if is_center {
            set_color(sink, 14, 0); // bright yellow
        } else if is_periphery {
            set_color(sink, 12, 0); // red
        } else if is_isolated {
            set_color(sink, 8, 0);  // dark grey
        } else {
            set_color(sink, 11, 0); // cyan
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);

        // Pad vector column to 16 chars.
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        // Eccentricity (right-aligned, 6 wide).
        set_color(sink,
            if is_center { 14 } else if is_periphery { 12 } else if is_isolated { 8 } else { 11 },
            0);
        print_str(sink, " ");
        print_num_right6(sink, score as usize);
        print_str(sink, "  ");

        // Role label.
        if is_center {
            set_color(sink, 14, 0);
            print_str(sink, "center");
            center_count += 1;
        } else if is_periphery {
            set_color(sink, 12, 0);
            print_str(sink, "periphery");
        } else if is_isolated {
            set_color(sink, 8, 0);
            print_str(sink, "isolated");
        } else {
            set_color(sink, 11, 0);
            print_str(sink, "relay");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  radius: ");
    set_color(sink, 14, 0);
    print_num_inline(sink, radius as usize);
    set_color(sink, 8, 0);
    print_str(sink, "  diameter: ");
    set_color(sink, 12, 0);
    print_num_inline(sink, diameter as usize);
    if center_count > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  center: ");
        print_num_inline(sink, center_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.42: `graph katz` — incoming Katz centrality per node (walk-count influence).
///
/// Katz centrality counts all directed walks of every length ending at each node,
/// attenuated by α = 1/8 per hop.  Unlike closeness (which only weighs shortest
/// paths) Katz also credits indirect influence through longer walks.
///
///   KC[v] = Σ_{k=1}^{∞} (1/8)^k × (directed walks of length k ending at v)
///
/// Score interpretation (×10⁻⁶):
///   0          → leaf   (no walks reach this node; pure source or disconnected)
///   0 < s ≤ 1M → relay  (receives moderate walk-influence)
///   s > 1M     → hub    (receives heavy walk-influence above α⁻¹ = 8× normalisation)
///
/// Output sorted descending: highest-influence nodes first.
/// OS analogy: `netstat -s` hop-weight — which kernel service accumulates the
/// most signal traffic across all directed path lengths?
pub fn dispatch_graph_katz(sink: &ConsoleSink) {
    const MAX_N:  usize = 128;
    const SCALE:  usize = 1_000_000;
    let (vecs, katz, total) = gos_runtime::graph_katz::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph katz\n");
    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " ───────────────────────────────────────────────────────────\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              katz  role\n");
    set_color(sink, 7, 0);

    let mut hub_count = 0usize;

    for i in 0..total {
        let score  = katz[i] as usize;
        let is_hub  = score > SCALE;
        let is_leaf = score == 0;

        if is_hub {
            set_color(sink, 14, 0); // bright yellow
        } else if !is_leaf {
            set_color(sink, 11, 0); // cyan
        } else {
            set_color(sink, 8, 0);  // dark grey
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);

        // Pad vector column to 16 chars.
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        // Katz score (right-aligned, 6 wide).
        set_color(sink, if is_hub { 14 } else if !is_leaf { 11 } else { 8 }, 0);
        print_str(sink, " ");
        print_num_right6(sink, score);
        print_str(sink, "  ");

        // Role label.
        if is_hub {
            set_color(sink, 14, 0);
            print_str(sink, "hub");
            hub_count += 1;
        } else if !is_leaf {
            set_color(sink, 11, 0);
            print_str(sink, "relay");
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "leaf");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    let max_katz = if total > 0 { katz[0] as usize } else { 0 }; // sorted descending, so [0] is max

    set_color(sink, 8, 0);
    print_str(sink, " ───────────────────────────────────────────────────────────\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  α=1/8  max-katz: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, max_katz);
    set_color(sink, 8, 0);
    print_str(sink, " (×1e-6)");
    if hub_count > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  hubs: ");
        print_num_inline(sink, hub_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.43: `graph pagerank` — PageRank centrality per node (random-walk stationary distribution).
///
/// Classical PageRank (d = 0.85) answers: "which node would a random walker
/// following directed signal edges land on most often?"  Unlike Katz centrality
/// (which counts all walks equally) PageRank normalises each node's contribution
/// by its out-degree, so high-degree hubs dilute their vote.
///
///   PR[v] = (1−d) × SCALE + d × Σ_{u→v, outdeg(u)>0} PR[u] / outdeg(u)
///
/// Score interpretation (×10⁻⁶):
///   PR ≥ 1_000_000     → authority  (dominates random-walk traffic)
///   300_000 < PR < 1M  → relay      (above-floor, some inbound link mass)
///   PR ≤ 300_000        → sink       (≈ teleportation floor, few/no inbound links)
///
/// Output sorted descending: highest-authority nodes first.
/// OS analogy: `top` by incoming-signal weight — structural importance of each
/// kernel node to the overall random-walk flow.
pub fn dispatch_graph_pagerank(sink: &ConsoleSink) {
    const MAX_N:      usize = 128;
    const SCALE:      usize = 1_000_000;
    const AUTHORITY:  usize = SCALE;          // ≥ 1_000_000
    const RELAY_FLOOR: usize = 300_000;       // > 300_000

    let (vecs, pr, total) = gos_runtime::graph_pagerank::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph pagerank\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector           pagerank  role\n");
    set_color(sink, 7, 0);

    let mut auth_count = 0usize;

    for i in 0..total {
        let score = pr[i] as usize;
        let is_authority = score >= AUTHORITY;
        let is_sink      = score <= RELAY_FLOOR;

        if is_authority {
            set_color(sink, 14, 0); // bright yellow
        } else if !is_sink {
            set_color(sink, 11, 0); // cyan
        } else {
            set_color(sink, 8, 0);  // dark grey
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);

        // Pad vector column to 16 chars.
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        // PageRank score (right-aligned, 9 wide).
        set_color(sink, if is_authority { 14 } else if !is_sink { 11 } else { 8 }, 0);
        print_str(sink, " ");
        print_num_right6(sink, score / 1000); // show as integer (÷1000 for display)
        print_str(sink, "k ");

        // Role label.
        if is_authority {
            set_color(sink, 14, 0);
            print_str(sink, "authority");
            auth_count += 1;
        } else if !is_sink {
            set_color(sink, 11, 0);
            print_str(sink, "relay");
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "sink");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    let max_pr = if total > 0 { pr[0] as usize } else { 0 };

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  d=0.85  max-pr: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, max_pr / 1000);
    set_color(sink, 8, 0);
    print_str(sink, "k (×1e-3)");
    if auth_count > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  authorities: ");
        print_num_inline(sink, auth_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.44: `graph hits` — HITS hub and authority scores per node.
///
/// Kleinberg's HITS algorithm decomposes the graph into a bipartite hub/authority
/// view.  Each node receives two scores:
///   hub score      — how well it points to high-authority nodes
///   authority score — how well it is pointed to by high-hub nodes
///
/// Update (simultaneous, 20 iterations, L∞-normalised):
///   new_a[v] = Σ_{u→v} h[u]   (in-neighbors' hub mass)
///   new_h[v] = Σ_{v→w} a[w]   (out-neighbors' authority mass)
///
/// Output sorted by authority score descending.
/// OS analogy: `vmstat` / `top` bipartite — signal-forwarder vs cited-destination.
pub fn dispatch_graph_hits(sink: &ConsoleSink) {
    const MAX_N:     usize = 128;
    const SCALE:     usize = 1_000_000;
    const TOP_FLOOR: usize = 800_000;   // ≥ 800_000 → top hub or top authority
    const LOW_CEIL:  usize = 200_000;   // < 200_000 → isolated / no structural role

    let (vecs, hub, auth, total) = gos_runtime::graph_hits::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph hits\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector             hub   authority  role\n");
    set_color(sink, 7, 0);

    let mut top_auth_count = 0usize;
    let mut top_hub_count  = 0usize;

    for i in 0..total {
        let h_score = hub[i]  as usize;
        let a_score = auth[i] as usize;
        let is_top_auth = a_score >= TOP_FLOOR;
        let is_top_hub  = h_score >= TOP_FLOOR;
        let is_isolated = a_score < LOW_CEIL && h_score < LOW_CEIL;

        let color = if is_top_auth && is_top_hub {
            13 // magenta — both hub+authority (e.g. cycle node)
        } else if is_top_auth {
            14 // bright yellow — pure authority
        } else if is_top_hub {
            11 // cyan — pure hub
        } else if is_isolated {
            8  // dark grey — no structural role
        } else {
            7  // white — relay
        };

        set_color(sink, color, 0);
        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);

        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        // Hub score (right-aligned 6 wide, ×1e-3)
        set_color(sink, if is_top_hub { 11 } else { 8 }, 0);
        print_str(sink, "  ");
        print_num_right6(sink, h_score / 1000);
        print_str(sink, "k");

        // Authority score
        set_color(sink, if is_top_auth { 14 } else { 8 }, 0);
        print_str(sink, "  ");
        print_num_right6(sink, a_score / 1000);
        print_str(sink, "k");

        // Role label
        set_color(sink, color, 0);
        print_str(sink, "  ");
        if is_top_auth && is_top_hub {
            print_str(sink, "hub+authority");
            top_auth_count += 1;
            top_hub_count  += 1;
        } else if is_top_auth {
            print_str(sink, "authority");
            top_auth_count += 1;
        } else if is_top_hub {
            print_str(sink, "hub");
            top_hub_count += 1;
        } else if is_isolated {
            print_str(sink, "isolated");
        } else {
            print_str(sink, "relay");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  HITS/20iter");
    if top_hub_count > 0 {
        set_color(sink, 11, 0);
        print_str(sink, "  hubs: ");
        print_num_inline(sink, top_hub_count);
    }
    if top_auth_count > 0 {
        set_color(sink, 14, 0);
        print_str(sink, "  authorities: ");
        print_num_inline(sink, top_auth_count);
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");

    let _ = SCALE;
}

/// V2.45: `graph community` — Label Propagation community detection.
///
/// Detects communities (clusters) in the kernel graph by treating all directed
/// edges as undirected and running Label Propagation (LPA) for 20 iterations.
/// Each node adopts the most-frequent label of its neighbors each round;
/// tie-break: smallest label.  After convergence the algorithm assigns community
/// ids 0, 1, 2... sorted by community size (largest = 0).
///
/// Community roles:
///   major-community — the largest community (id 0)
///   minor-community — smaller but multi-node community
///   isolated        — single-node community (no undirected neighbors)
///
/// OS analogy: `iproute2 bridge vlan show` + `systemd-analyze critical-chain`
/// — which kernel services naturally cluster into tightly coupled sub-systems?
pub fn dispatch_graph_community(sink: &ConsoleSink) {
    const MAX_N: usize = 128;

    let (vecs, comm_ids, total, comm_count) = gos_runtime::graph_community::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph community\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Count size of largest community for role annotation.
    let mut comm_sizes = [0u8; MAX_N];
    for i in 0..total {
        let c = comm_ids[i] as usize;
        if c < MAX_N { comm_sizes[c] = comm_sizes[c].saturating_add(1); }
    }
    let largest_size = if comm_count > 0 { comm_sizes[0] as usize } else { 0 };

    // Per-community block: walk through sorted nodes detecting id boundaries.
    let mut pos = 0usize;
    while pos < total {
        let cur_comm = comm_ids[pos];
        let mut end  = pos;
        while end < total && comm_ids[end] == cur_comm { end += 1; }
        let size      = end - pos;
        let is_major  = cur_comm == 0 && largest_size > 1;
        let is_minor  = size > 1 && !is_major;
        let _is_isolated = size == 1;

        let hdr_color = if is_major { 13 } else if is_minor { 11 } else { 8 };

        // Community header line
        set_color(sink, hdr_color, 0);
        print_str(sink, "  [C");
        print_num_inline(sink, cur_comm as usize);
        print_str(sink, "]  ");
        set_color(sink, 7, 0);
        print_num_inline(sink, size);
        print_str(sink, if size == 1 { " node" } else { " nodes" });
        print_str(sink, "  ");
        set_color(sink, hdr_color, 0);
        if is_major {
            print_str(sink, "major-community");
        } else if is_minor {
            print_str(sink, "minor-community");
        } else {
            print_str(sink, "isolated");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Member node list (up to 4 per row)
        let mut col = 0usize;
        for i in pos..end {
            if col == 0 { print_str(sink, "      "); }
            let mut vbuf = LineBuf::<20>::new();
            vbuf.push_vector(vecs[i]);
            let vstr = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
            set_color(sink, if is_major { 13 } else if is_minor { 11 } else { 8 }, 0);
            print_str(sink, vstr);
            set_color(sink, 7, 0);
            col += 1;
            if col == 4 || i + 1 == end {
                print_str(sink, "\n");
                col = 0;
            } else {
                print_str(sink, "  ");
            }
        }

        pos = end;
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  LPA/20iter  communities: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, comm_count);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.46: `graph spanning` — BFS spanning forest over the undirected kernel graph.
///
/// Covers all live nodes; treats every directed edge as undirected.  Roots are
/// chosen in ascending slot order — each new unvisited node starts a new tree.
/// Output is shown in BFS visit order: root (depth 0), then level-1 children,
/// then level-2 grandchildren, etc., tree by tree.
///
/// Node roles:
///   root     — depth 0; the BFS root of a spanning tree
///   branch   — depth ≥ 1 and has children in the spanning tree
///   leaf     — depth ≥ 1 and has no children in the spanning tree
///
/// OS analogy: `ip route show` / STP spanning-tree protocol — the minimal
/// backbone connecting all kernel sub-systems without redundant cross-links.
pub fn dispatch_graph_spanning(sink: &ConsoleSink) {
    const MAX_N: usize = 128;

    let (vecs, parents, depths, total, tree_count) = gos_runtime::graph_spanning::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph spanning\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Compute which nodes are children (have at least one child in the spanning tree).
    // A node is a branch if some other node has it as parent (and that node isn't itself).
    let mut has_child = [false; MAX_N];
    for i in 0..total {
        if depths[i] > 0 {
            // Find the parent index so we can mark it.
            for j in 0..total {
                if vecs[j] == parents[i] {
                    has_child[j] = true;
                    break;
                }
            }
        }
    }

    // Walk through output, detecting tree boundaries (depth resets to 0).
    let mut tree_idx = 0usize;
    let mut i = 0usize;
    while i < total {
        // Each tree starts at depth 0.
        let root_vec = vecs[i];

        // Count nodes in this tree.
        let mut tree_end = i;
        while tree_end < total && (tree_end == i || depths[tree_end] > 0) {
            tree_end += 1;
        }
        let tree_size = tree_end - i;

        // Tree header.
        set_color(sink, 13, 0);
        print_str(sink, "  [T");
        print_num_inline(sink, tree_idx);
        print_str(sink, "]  root: ");
        set_color(sink, 11, 0);
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(root_vec);
        print_str(sink, core::str::from_utf8(vbuf.as_slice()).unwrap_or("?"));
        set_color(sink, 7, 0);
        print_str(sink, "  \u{2500}\u{2500}  ");
        print_num_inline(sink, tree_size);
        print_str(sink, if tree_size == 1 { " node\n" } else { " nodes\n" });

        // Column header.
        set_color(sink, 8, 0);
        print_str(sink, "    depth  vector           parent           role\n");
        set_color(sink, 7, 0);

        for k in i..tree_end {
            let d    = depths[k];
            let is_root   = d == 0;
            let is_branch = !is_root && has_child[k];
            let role_col  = if is_root { 13u8 } else if is_branch { 11 } else { 7 };

            // depth column (5 chars wide)
            print_str(sink, "    ");
            print_num_inline(sink, d as usize);
            print_str(sink, "      ");

            // vector column
            let mut vbuf2 = LineBuf::<20>::new();
            vbuf2.push_vector(vecs[k]);
            set_color(sink, role_col, 0);
            let vs = core::str::from_utf8(vbuf2.as_slice()).unwrap_or("?");
            print_str(sink, vs);
            // pad to 17 chars
            let pad = 17usize.saturating_sub(vs.len());
            for _ in 0..pad { print_str(sink, " "); }
            set_color(sink, 7, 0);

            // parent column
            if is_root {
                set_color(sink, 8, 0);
                print_str(sink, "(root)           ");
                set_color(sink, 7, 0);
            } else {
                let mut pbuf = LineBuf::<20>::new();
                pbuf.push_vector(parents[k]);
                let ps = core::str::from_utf8(pbuf.as_slice()).unwrap_or("?");
                print_str(sink, ps);
                let ppad = 17usize.saturating_sub(ps.len());
                for _ in 0..ppad { print_str(sink, " "); }
            }

            // role column
            set_color(sink, role_col, 0);
            if is_root {
                print_str(sink, "root");
            } else if is_branch {
                print_str(sink, "branch");
            } else {
                print_str(sink, "leaf");
            }
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }

        tree_idx += 1;
        i = tree_end;
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  BFS spanning-forest  trees: ");
    set_color(sink, 7, 0);
    print_num_inline(sink, tree_count);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.47: `graph color` — Welsh-Powell greedy graph coloring.
///
/// Assigns each live node a color index (0-based) such that no two directly
/// connected nodes share the same color.  Nodes are sorted in descending
/// total-degree order (Welsh-Powell heuristic) before greedy assignment.
///
/// Output: color index, node vector, and role label:
///   - `center`   — color 0 (first assigned, highest degree)
///   - `domain-N` — color N (N > 0)
///   - `isolated` — no edges at all (always color 0)
///
/// OS analogy: each color is a conflict-free scheduling domain / CPU-affinity
/// group (like Linux cgroups cpuset.cpus assignments, or NUMA node binding).
pub fn dispatch_graph_color(sink: &ConsoleSink) {
    const MAX_N: usize = 128;

    let (vecs, colors, degrees, total, chromatic) = gos_runtime::graph_color::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph color\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Summary header.
    set_color(sink, 7, 0);
    print_str(sink, "  chromatic number: ");
    set_color(sink, 11, 0);
    print_num_inline(sink, chromatic as usize);
    set_color(sink, 7, 0);
    print_str(sink, "   nodes: ");
    print_num_inline(sink, total);
    print_str(sink, "\n\n");

    // Column header.
    set_color(sink, 8, 0);
    print_str(sink, "  color  vector           role\n");
    set_color(sink, 7, 0);

    // Color-to-terminal-color mapping: cycle through bright colors.
    // Colors 0-7: yellow, cyan, green, magenta, red, white, dark-cyan, dark-green
    const TERM: [u8; 8] = [11, 14, 10, 13, 12, 15, 6, 2];

    for i in 0..total {
        let c   = colors[i];
        let d   = degrees[i];
        let vec = vecs[i];
        let tc  = TERM[(c as usize) % 8];

        set_color(sink, tc, 0);
        // color column (6 chars)
        print_str(sink, "  C");
        print_num_inline(sink, c as usize);
        // pad to 7 chars total (leading "  C" = 3 chars + digits + spaces)
        let digits = if c < 10 { 1 } else if c < 100 { 2 } else { 3 };
        for _ in 0..(4usize.saturating_sub(digits)) { print_str(sink, " "); }

        // vector column
        set_color(sink, 7, 0);
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vs);
        let pad = 17usize.saturating_sub(vs.len());
        for _ in 0..pad { print_str(sink, " "); }

        // role column
        set_color(sink, tc, 0);
        if d == 0 {
            print_str(sink, "isolated");
        } else if c == 0 {
            print_str(sink, "center");
        } else {
            print_str(sink, "domain-");
            print_num_inline(sink, c as usize);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
}

/// V2.48: `graph mst` — Prim's Minimum Spanning Forest over the live kernel graph.
///
/// Treats every directed edge as undirected with weight `edge.spec.weight`
/// (default 1.0).  Disconnected components each get their own MST root.
///
/// Output columns: role (root/branch/leaf), weight-to-parent, vector, parent vector.
/// Footer: N node(s)  Prim MST  total weight: W.mmm
pub fn dispatch_graph_mst(sink: &ConsoleSink) {
    const MAX_N: usize = 128;

    let (vecs, parents, weights, total, mst_w) = gos_runtime::graph_mst::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph mst\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Column header.
    set_color(sink, 8, 0);
    print_str(sink, "  role    weight    vector           parent\n");
    set_color(sink, 7, 0);

    for i in 0..total {
        let vec     = vecs[i];
        let parent  = parents[i];
        let w_u32   = weights[i];
        let is_root = vec == parent;

        // Role column (6 chars)
        if is_root {
            set_color(sink, 13, 0); // magenta = root
            print_str(sink, "  root  ");
        } else {
            set_color(sink, 11, 0); // cyan = branch / child
            print_str(sink, "  child ");
        }

        // Weight column "W.mmm" (8 chars + 2 spaces)
        set_color(sink, 14, 0); // yellow
        let whole = w_u32 / 1000;
        let frac  = w_u32 % 1000;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        // Print frac with leading zeros (3 digits).
        if frac < 10  { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac as usize);
        print_str(sink, "  ");

        // Vector column (17 chars)
        set_color(sink, 7, 0);
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vs);
        let vpad = 17usize.saturating_sub(vs.len());
        for _ in 0..vpad { print_str(sink, " "); }

        // Parent column
        if is_root {
            set_color(sink, 8, 0);
            print_str(sink, "(root)");
        } else {
            set_color(sink, 7, 0);
            let mut pbuf = LineBuf::<20>::new();
            pbuf.push_vector(parent);
            let ps = core::str::from_utf8(pbuf.as_slice()).unwrap_or("?");
            print_str(sink, ps);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, total);
    print_str(sink, " node(s)  Prim MST  total weight: ");
    set_color(sink, 14, 0);
    let whole = mst_w / 1000;
    let frac  = mst_w % 1000;
    print_num_inline(sink, whole as usize);
    print_str(sink, ".");
    if frac < 10  { print_str(sink, "00"); }
    else if frac < 100 { print_str(sink, "0"); }
    print_num_inline(sink, frac as usize);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.49: `graph shortest <vec>` — Dijkstra single-source shortest-path tree.
///
/// Displays directed distances from `source` to every live node.
/// Unreachable nodes (no directed path from source) show distance `∞`.
///
/// Output columns: status (source/reachable/∞), distance, vector, parent.
pub fn dispatch_graph_shortest(sink: &ConsoleSink, source: VectorAddress) {
    const MAX_N: usize = 128;

    let (vecs, parents, dists, total) = gos_runtime::graph_shortest::<MAX_N>(source);

    set_color(sink, 11, 0);
    print_str(sink, " graph shortest ");
    let mut src_buf = LineBuf::<20>::new();
    src_buf.push_vector(source);
    let src_str = core::str::from_utf8(src_buf.as_slice()).unwrap_or("?");
    print_str(sink, src_str);
    print_str(sink, "\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Column header.
    set_color(sink, 8, 0);
    print_str(sink, "  status    dist      vector           parent\n");
    set_color(sink, 7, 0);

    let mut reachable_count = 0usize;

    for i in 0..total {
        let vec  = vecs[i];
        let par  = parents[i];
        let d    = dists[i];
        let zero = VectorAddress::new(0, 0, 0, 0);
        let is_src       = vec == source && d == 0;
        let is_reachable = d != u32::MAX;

        // Status column (10 chars).
        if is_src {
            set_color(sink, 13, 0); // magenta = source
            print_str(sink, "  source   ");
        } else if is_reachable {
            set_color(sink, 10, 0); // green = reachable
            print_str(sink, "  reach    ");
            reachable_count += 1;
        } else {
            set_color(sink, 8, 0); // dark = unreachable
            print_str(sink, "  \u{221e}         "); // ∞ symbol
        }

        // Distance column "D.mmm" (9 chars).
        set_color(sink, 14, 0); // yellow
        if !is_reachable {
            print_str(sink, "\u{221e}        ");
        } else {
            let whole = d / 1000;
            let frac  = d % 1000;
            print_num_inline(sink, whole as usize);
            print_str(sink, ".");
            if frac < 10  { print_str(sink, "00"); }
            else if frac < 100 { print_str(sink, "0"); }
            print_num_inline(sink, frac as usize);
            print_str(sink, "  ");
        }

        // Vector column (17 chars).
        set_color(sink, 7, 0);
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vs);
        let vpad = 17usize.saturating_sub(vs.len());
        for _ in 0..vpad { print_str(sink, " "); }

        // Parent column.
        if is_src {
            set_color(sink, 8, 0);
            print_str(sink, "(source)");
        } else if par == zero || !is_reachable {
            set_color(sink, 8, 0);
            print_str(sink, "(unreachable)");
        } else {
            set_color(sink, 7, 0);
            let mut pbuf = LineBuf::<20>::new();
            pbuf.push_vector(par);
            let ps = core::str::from_utf8(pbuf.as_slice()).unwrap_or("?");
            print_str(sink, ps);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, total);
    print_str(sink, " node(s)  Dijkstra SPT from ");
    set_color(sink, 13, 0);
    print_str(sink, src_str);
    set_color(sink, 7, 0);
    print_str(sink, "  reachable: ");
    set_color(sink, 10, 0);
    print_num_inline(sink, reachable_count);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.50: `graph flow <source> <sink>` — maximum network flow (Edmonds-Karp).
///
/// Shows the maximum throughput from `source` to `sink` over directed edges,
/// treating edge weights as capacities.  Lists each node's role and its
/// net flow volume in the final flow assignment.
///
/// OS analogy: `tc -s qdisc show` — per-subsystem bandwidth saturation view.
pub fn dispatch_graph_flow(sink: &ConsoleSink, source: VectorAddress, snk_vec: VectorAddress) {
    const MAX_N: usize = 128;

    let (vecs, out_flows, in_flows, total, max_flow) =
        gos_runtime::graph_flow::<MAX_N>(source, snk_vec);

    set_color(sink, 11, 0);
    print_str(sink, " graph flow ");
    let mut src_buf = LineBuf::<20>::new();
    src_buf.push_vector(source);
    let src_str = core::str::from_utf8(src_buf.as_slice()).unwrap_or("?");
    print_str(sink, src_str);
    print_str(sink, " \u{2192} ");
    let mut snk_buf = LineBuf::<20>::new();
    snk_buf.push_vector(snk_vec);
    let snk_str = core::str::from_utf8(snk_buf.as_slice()).unwrap_or("?");
    print_str(sink, snk_str);
    print_str(sink, "\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Column header.
    set_color(sink, 8, 0);
    print_str(sink, "  role       out-flow  in-flow   vector\n");
    set_color(sink, 7, 0);

    let zero = VectorAddress::new(0, 0, 0, 0);

    for i in 0..total {
        let vec = vecs[i];
        let of  = out_flows[i];
        let inf = in_flows[i];
        let is_src = vec == source;
        let is_snk = vec == snk_vec && !is_src;
        let has_flow = of > 0 || inf > 0;

        // Role column (12 chars).
        if is_src {
            set_color(sink, 13, 0);
            print_str(sink, "  source     ");
        } else if is_snk {
            set_color(sink, 10, 0);
            print_str(sink, "  sink       ");
        } else if has_flow {
            set_color(sink, 14, 0);
            print_str(sink, "  relay      ");
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "  isolated   ");
        }

        // Out-flow column (10 chars).
        set_color(sink, 14, 0);
        let whole = of / 1000;
        let frac  = of % 1000;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10  { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac as usize);
        print_str(sink, "  ");

        // In-flow column (10 chars).
        set_color(sink, 11, 0);
        let whole2 = inf / 1000;
        let frac2  = inf % 1000;
        print_num_inline(sink, whole2 as usize);
        print_str(sink, ".");
        if frac2 < 10  { print_str(sink, "00"); }
        else if frac2 < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac2 as usize);
        print_str(sink, "  ");

        // Vector column.
        set_color(sink, 7, 0);
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vs);
        print_str(sink, "\n");
        let _ = zero; // suppress unused warning
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, total);
    print_str(sink, " node(s)  max-flow: ");
    set_color(sink, 13, 0);
    let mf_whole = max_flow / 1000;
    let mf_frac  = max_flow % 1000;
    print_num_inline(sink, mf_whole as usize);
    print_str(sink, ".");
    if mf_frac < 10  { print_str(sink, "00"); }
    else if mf_frac < 100 { print_str(sink, "0"); }
    print_num_inline(sink, mf_frac as usize);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.52: `graph sim [N]` — random walk simulation over the live graph.
///
/// Simulates N random-walk steps (default 16, clamped to 256) starting from
/// a random live node, sampling outgoing edges proportional to their weight.
/// Dead-end nodes cause a teleport to a random live node (PageRank restart).
/// Nodes are listed highest-to-lowest by visit count — the "hottest" signal
/// paths in the graph topology under random load.
///
/// OS analogy: `strace -e trace=signal` — identifies which kernel subsystems
/// dominate signal traffic under simulated random load.
pub fn dispatch_graph_sim(sink: &ConsoleSink, steps: u32) {
    const MAX_N: usize = 128;

    let epoch = gos_runtime::graph_epoch();
    let seed  = (epoch as u32) ^ steps ^ 0xDEAD_BEEF;
    let (vecs, visits, total, actual, stuck) =
        gos_runtime::graph_sim::<MAX_N>(steps, seed);

    set_color(sink, 11, 0);
    print_str(sink, " graph sim  steps=");
    print_num_inline(sink, steps as usize);
    set_color(sink, 8, 0);
    print_str(sink, "  seed=");
    print_num_inline(sink, seed as usize);
    print_str(sink, "\n");
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 8, 0);
    print_str(sink, "  rank  visits  vector\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    for i in 0..total {
        let v  = visits[i];
        let fg: u8 = if v == 0 { 8 } else if i == 0 { 13 } else if i < 3 { 11 } else { 7 };
        set_color(sink, fg, 0);
        print_str(sink, "  ");
        print_num_inline(sink, i + 1);
        print_str(sink, "      ");
        print_num_inline(sink, v as usize);
        print_str(sink, "  ");
        set_color(sink, 7, 0);
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vecs[i]);
        let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vs);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, total);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, actual as usize);
    print_str(sink, " walk steps  ");
    if stuck > 0 {
        set_color(sink, 14, 0);
        print_num_inline(sink, stuck as usize);
        print_str(sink, " teleport(s)");
        set_color(sink, 7, 0);
    } else {
        set_color(sink, 10, 0);
        print_str(sink, "no dead ends");
        set_color(sink, 7, 0);
    }
    print_str(sink, "\n");
}

/// V2.54: `graph attractor` — classify every live node into attractor / drain / transient.
///
/// An **attractor** (bottom SCC) is a strongly-connected component with no
/// outgoing edges to the rest of the graph.  Signal or execution flow that
/// enters an attractor can never leave it.
///
/// Node roles:
///   attractor  — role 0; member of a bottom SCC; no condensation out-edges.
///   drain      — role 1; not in a bottom SCC but has a direct condensation
///                edge to at least one attractor SCC (one step from stability).
///   transient  — role 2; SCC has out-edges, but none lead directly to an
///                attractor SCC (two or more hops from stability).
///
/// Output is sorted: attractors first, drains second, transients last.
///
/// OS analogy: `systemctl list-units --state=running` service stability audit —
/// attractor nodes are always-running service loops, drain nodes are one-hop
/// from a stable loop, transient nodes are far from any stable loop.
pub fn dispatch_graph_attractor(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<MAX_N>();

    set_color(sink, 10, 0); // bright green — stable attractors
    print_str(sink, " graph attractor\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              role\n");
    set_color(sink, 7, 0);

    let mut drain_count = 0usize;
    let mut transient_count = 0usize;

    for i in 0..total {
        let role = roles[i];

        let fg: u8 = match role {
            0 => 10, // bright green — attractor
            1 => 14, // bright yellow — drain
            _ => 8,  // dark grey — transient
        };
        set_color(sink, fg, 0);
        print_str(sink, "  ");

        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vecs[i]);
        let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vs);

        // Pad to fixed column width (20 chars for vector).
        let vlen = vs.len();
        let pad = if vlen < 20 { 20 - vlen } else { 0 };
        for _ in 0..pad {
            print_str(sink, " ");
        }

        match role {
            0 => { set_color(sink, 10, 0); print_str(sink, "attractor"); }
            1 => { set_color(sink, 14, 0); print_str(sink, "drain");     drain_count += 1; }
            _ => { set_color(sink, 8,  0); print_str(sink, "transient"); transient_count += 1; }
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, total);
    print_str(sink, " node(s)  ");
    set_color(sink, 10, 0);
    print_num_inline(sink, attractor_count);
    print_str(sink, " attractor");
    set_color(sink, 7, 0);
    if drain_count > 0 {
        print_str(sink, "  ");
        set_color(sink, 14, 0);
        print_num_inline(sink, drain_count);
        print_str(sink, " drain");
        set_color(sink, 7, 0);
    }
    if transient_count > 0 {
        print_str(sink, "  ");
        set_color(sink, 8, 0);
        print_num_inline(sink, transient_count);
        print_str(sink, " transient");
        set_color(sink, 7, 0);
    }
    print_str(sink, "\n");
}

/// V2.83: `graph snapshot` / `gsnapshot` — save current topology metrics.
///
/// Captures density, transitivity, avg-clustering, global/local efficiency,
/// small-world σ, scale-free κ, and power-law γ̂ in a single atomic read,
/// storing them in the persistent `METRIC_SNAPSHOT` slot for later comparison.
/// Analogous to `sar -o snap.bin` (Linux sysstat) — establish a monitoring baseline.
pub fn dispatch_graph_snapshot(sink: &ConsoleSink) {
    let epoch_saved = gos_runtime::graph_snapshot_save();
    let (_, cur) = gos_runtime::graph_snapshot_compare();

    // Helper: print ppm as W.XXX (3 decimal places).
    fn print_ppm3(sink: &ConsoleSink, ppm: u64) {
        let whole = (ppm / 1_000_000) as usize;
        let frac  = (ppm / 1_000 % 1_000) as usize;
        print_num_inline(sink, whole);
        print_str(sink, ".");
        if frac < 10  { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    set_color(sink, 10, 0);
    print_str(sink, " graph snapshot saved\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    print_str(sink, "  epoch      = ");
    print_num_inline(sink, epoch_saved as usize);
    print_str(sink, "\n  nodes       = ");
    print_num_inline(sink, cur.node_count);
    print_str(sink, "\n  edges       = ");
    print_num_inline(sink, cur.edge_count);
    print_str(sink, "\n  density     = ");
    print_ppm3(sink, cur.density_ppm as u64);
    print_str(sink, "\n  global CC   = ");
    print_ppm3(sink, cur.trans_ppm as u64);
    print_str(sink, "\n  avg CC      = ");
    print_ppm3(sink, cur.avgcc_ppm as u64);
    print_str(sink, "\n  E_global    = ");
    print_ppm3(sink, cur.geff_ppm);
    print_str(sink, "\n  E_local     = ");
    print_ppm3(sink, cur.leff_ppm as u64);
    if cur.sigma_ppm == 0 {
        print_str(sink, "\n  \u{03c3}           = undef");
    } else {
        print_str(sink, "\n  \u{03c3}           = ");
        print_ppm3(sink, cur.sigma_ppm as u64);
    }
    if cur.kappa_ppm == 0 {
        print_str(sink, "\n  \u{03ba}           = undef");
    } else {
        print_str(sink, "\n  \u{03ba}           = ");
        print_ppm3(sink, cur.kappa_ppm as u64);
    }
    if cur.gamma_ppm == 0 {
        print_str(sink, "\n  \u{03b3}\u{0302}           = undef");
    } else {
        print_str(sink, "\n  \u{03b3}\u{0302}           = ");
        print_ppm3(sink, cur.gamma_ppm as u64);
    }
    print_str(sink, "\n");
    set_color(sink, 8, 0);
    print_str(sink, "  run `graph compare` later to diff against this baseline\n");
    set_color(sink, 7, 0);
}

/// V2.83: `graph compare` / `gcompare` — diff current metrics vs saved snapshot.
///
/// Shows a side-by-side table of saved vs current values with signed deltas.
/// Ideal for monitoring topology drift after structural mutations.
/// Analogous to `sar -s <start> -e <end>` (Linux sysstat) or `diff /proc/net/stat`.
pub fn dispatch_graph_compare(sink: &ConsoleSink) {
    let (saved, cur) = gos_runtime::graph_snapshot_compare();

    // Helper: print ppm as W.XXX (3 decimal places).
    fn print_ppm3(sink: &ConsoleSink, ppm: u64) {
        let whole = (ppm / 1_000_000) as usize;
        let frac  = (ppm / 1_000 % 1_000) as usize;
        print_num_inline(sink, whole);
        print_str(sink, ".");
        if frac < 10  { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    // Helper: print signed ppm delta (+W.XXX or -W.XXX) with colour (green=+, red=-, grey=0).
    fn print_delta_ppm(sink: &ConsoleSink, saved_ppm: u64, cur_ppm: u64) {
        if cur_ppm == saved_ppm {
            set_color(sink, 8, 0);
            print_str(sink, "  \u{b1}0");
        } else if cur_ppm > saved_ppm {
            set_color(sink, 10, 0);
            print_str(sink, "  +");
            // delta may exceed u32; use u64 for safety.
            let d = cur_ppm - saved_ppm;
            let w = (d / 1_000_000) as usize;
            let f = (d / 1_000 % 1_000) as usize;
            print_num_inline(sink, w);
            print_str(sink, ".");
            if f < 10  { print_str(sink, "00"); }
            else if f < 100 { print_str(sink, "0"); }
            print_num_inline(sink, f);
        } else {
            set_color(sink, 12, 0);
            print_str(sink, "  -");
            let d = saved_ppm - cur_ppm;
            let w = (d / 1_000_000) as usize;
            let f = (d / 1_000 % 1_000) as usize;
            print_num_inline(sink, w);
            print_str(sink, ".");
            if f < 10  { print_str(sink, "00"); }
            else if f < 100 { print_str(sink, "0"); }
            print_num_inline(sink, f);
        }
        set_color(sink, 7, 0);
    }

    // Helper: print signed count delta with colour.
    fn print_delta_count(sink: &ConsoleSink, saved_n: usize, cur_n: usize) {
        if cur_n == saved_n {
            set_color(sink, 8, 0);
            print_str(sink, "  \u{b1}0");
        } else if cur_n > saved_n {
            set_color(sink, 10, 0);
            print_str(sink, "  +");
            print_num_inline(sink, cur_n - saved_n);
        } else {
            set_color(sink, 12, 0);
            print_str(sink, "  -");
            print_num_inline(sink, saved_n - cur_n);
        }
        set_color(sink, 7, 0);
    }

    set_color(sink, 11, 0);
    print_str(sink, " graph compare\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if !saved.valid {
        set_color(sink, 14, 0);
        print_str(sink, "  no baseline — run `graph snapshot` first\n");
        set_color(sink, 7, 0);
        return;
    }

    // Column header.
    set_color(sink, 8, 0);
    print_str(sink, "  metric       saved        current      delta\n");
    set_color(sink, 7, 0);

    // nodes
    print_str(sink, "  nodes        ");
    print_num_inline(sink, saved.node_count);
    print_str(sink, "            ");
    print_num_inline(sink, cur.node_count);
    print_delta_count(sink, saved.node_count, cur.node_count);
    print_str(sink, "\n");

    // edges
    print_str(sink, "  edges        ");
    print_num_inline(sink, saved.edge_count);
    print_str(sink, "            ");
    print_num_inline(sink, cur.edge_count);
    print_delta_count(sink, saved.edge_count, cur.edge_count);
    print_str(sink, "\n");

    // density
    print_str(sink, "  density      ");
    print_ppm3(sink, saved.density_ppm as u64);
    print_str(sink, "        ");
    print_ppm3(sink, cur.density_ppm as u64);
    print_delta_ppm(sink, saved.density_ppm as u64, cur.density_ppm as u64);
    print_str(sink, "\n");

    // global CC
    print_str(sink, "  global CC    ");
    print_ppm3(sink, saved.trans_ppm as u64);
    print_str(sink, "        ");
    print_ppm3(sink, cur.trans_ppm as u64);
    print_delta_ppm(sink, saved.trans_ppm as u64, cur.trans_ppm as u64);
    print_str(sink, "\n");

    // avg CC
    print_str(sink, "  avg CC       ");
    print_ppm3(sink, saved.avgcc_ppm as u64);
    print_str(sink, "        ");
    print_ppm3(sink, cur.avgcc_ppm as u64);
    print_delta_ppm(sink, saved.avgcc_ppm as u64, cur.avgcc_ppm as u64);
    print_str(sink, "\n");

    // E_global
    print_str(sink, "  E_global     ");
    print_ppm3(sink, saved.geff_ppm);
    print_str(sink, "        ");
    print_ppm3(sink, cur.geff_ppm);
    print_delta_ppm(sink, saved.geff_ppm, cur.geff_ppm);
    print_str(sink, "\n");

    // E_local
    print_str(sink, "  E_local      ");
    print_ppm3(sink, saved.leff_ppm as u64);
    print_str(sink, "        ");
    print_ppm3(sink, cur.leff_ppm as u64);
    print_delta_ppm(sink, saved.leff_ppm as u64, cur.leff_ppm as u64);
    print_str(sink, "\n");

    // sigma
    print_str(sink, "  \u{03c3}            ");
    if saved.sigma_ppm == 0 { print_str(sink, "undef"); } else { print_ppm3(sink, saved.sigma_ppm as u64); }
    print_str(sink, "        ");
    if cur.sigma_ppm == 0   { print_str(sink, "undef"); } else { print_ppm3(sink, cur.sigma_ppm as u64); }
    print_delta_ppm(sink, saved.sigma_ppm as u64, cur.sigma_ppm as u64);
    print_str(sink, "\n");

    // kappa
    print_str(sink, "  \u{03ba}            ");
    if saved.kappa_ppm == 0 { print_str(sink, "undef"); } else { print_ppm3(sink, saved.kappa_ppm as u64); }
    print_str(sink, "        ");
    if cur.kappa_ppm == 0   { print_str(sink, "undef"); } else { print_ppm3(sink, cur.kappa_ppm as u64); }
    print_delta_ppm(sink, saved.kappa_ppm as u64, cur.kappa_ppm as u64);
    print_str(sink, "\n");

    // gamma
    print_str(sink, "  \u{03b3}\u{0302}            ");
    if saved.gamma_ppm == 0 { print_str(sink, "undef"); } else { print_ppm3(sink, saved.gamma_ppm as u64); }
    print_str(sink, "        ");
    if cur.gamma_ppm == 0   { print_str(sink, "undef"); } else { print_ppm3(sink, cur.gamma_ppm as u64); }
    print_delta_ppm(sink, saved.gamma_ppm as u64, cur.gamma_ppm as u64);
    print_str(sink, "\n");

    // Footer: epoch range.
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  epoch: ");
    print_num_inline(sink, saved.epoch as usize);
    print_str(sink, " \u{2192} ");
    print_num_inline(sink, cur.epoch as usize);
    if cur.epoch == saved.epoch {
        set_color(sink, 8, 0);
        print_str(sink, "  (no structural mutations since snapshot)");
    } else {
        let adv = cur.epoch - saved.epoch;
        set_color(sink, 14, 0);
        print_str(sink, "  (epoch advanced by ");
        print_num_inline(sink, adv as usize);
        print_str(sink, ")");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.84: `graph predict <u> <v>` — link prediction metrics for node pair (u, v).
///
/// Computes four classical link-prediction scores that quantify how likely a
/// missing edge u→v is to form, based on shared neighbourhood structure:
///   CN  — Common Neighbors:    |N(u) \u{2229} N(v)|
///   J   — Jaccard Coefficient: CN / |N(u) \u{222a} N(v)|
///   AA  — Adamic-Adar index:   \u{03a3}_{w\u{2208}CN} 1/ln(deg(w))
///   RA  — Resource Allocation: \u{03a3}_{w\u{2208}CN} 1/deg(w)
/// All scores shown per-million (ppm).  Higher \u{2192} stronger missing-edge signal.
/// OS analogy: LLDP / CDP neighbour prediction — which kernel subsystems are
/// structurally primed to form a new dependency edge?
pub fn dispatch_graph_predict(sink: &ConsoleSink, u: VectorAddress, v: VectorAddress) {
    let (cn, jaccard_ppm, aa_ppm, ra_ppm, node_count) =
        gos_runtime::graph_link_predict(u, v);

    set_color(sink, 11, 0);
    print_str(sink, " graph predict ");
    let mut ubuf = LineBuf::<20>::new();
    ubuf.push_vector(u);
    let us = core::str::from_utf8(ubuf.as_slice()).unwrap_or("?");
    print_str(sink, us);
    print_str(sink, " \u{2192} ");
    let mut vbuf = LineBuf::<20>::new();
    vbuf.push_vector(v);
    let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
    print_str(sink, vs);
    print_str(sink, "\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    fn print_predict_ppm(sink: &ConsoleSink, ppm: u32) {
        let whole = ppm / 1_000_000;
        let frac  = ppm % 1_000_000;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10       { print_str(sink, "00000"); }
        else if frac < 100 { print_str(sink, "0000"); }
        else if frac < 1_000 { print_str(sink, "000"); }
        else if frac < 10_000 { print_str(sink, "00"); }
        else if frac < 100_000 { print_str(sink, "0"); }
        print_num_inline(sink, frac as usize);
    }

    set_color(sink, 8, 0);
    print_str(sink, "  metric                  score    interpretation\n");
    set_color(sink, 7, 0);

    let cn_fg: u8 = if cn == 0 { 8 } else if cn >= 3 { 10 } else { 14 };
    set_color(sink, cn_fg, 0);
    print_str(sink, "  common neighbors        ");
    print_num_inline(sink, cn);
    print_str(sink, "        ");
    if cn == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "no shared neighbours\n");
    } else {
        print_num_inline(sink, cn);
        print_str(sink, " shared neighbour");
        if cn != 1 { print_str(sink, "s"); }
        print_str(sink, "\n");
    }

    let j_fg: u8 = if jaccard_ppm == 0 { 8 } else if jaccard_ppm >= 500_000 { 10 } else { 14 };
    set_color(sink, j_fg, 0);
    print_str(sink, "  jaccard coeff           ");
    print_predict_ppm(sink, jaccard_ppm);
    print_str(sink, "  N(u)\u{2229}N(v) / N(u)\u{222a}N(v)\n");

    let aa_fg: u8 = if aa_ppm == 0 { 8 } else if aa_ppm >= 1_000_000 { 10 } else { 14 };
    set_color(sink, aa_fg, 0);
    print_str(sink, "  adamic-adar             ");
    print_predict_ppm(sink, aa_ppm);
    print_str(sink, "  \u{03a3} 1/ln(deg(w)) over CN\n");

    let ra_fg: u8 = if ra_ppm == 0 { 8 } else if ra_ppm >= 500_000 { 10 } else { 14 };
    set_color(sink, ra_fg, 0);
    print_str(sink, "  resource allocation     ");
    print_predict_ppm(sink, ra_ppm);
    print_str(sink, "  \u{03a3} 1/deg(w) over CN\n");

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    print_str(sink, " node(s)  prediction: ");
    if cn == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "none (no shared neighbours)");
    } else if jaccard_ppm >= 200_000 || cn >= 2 {
        set_color(sink, 10, 0);
        print_str(sink, "likely");
    } else {
        set_color(sink, 14, 0);
        print_str(sink, "weak");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.85: `graph articulation` — cut vertices of the live kernel graph.
/// An articulation point (cut vertex) is a node whose removal increases the
/// number of connected components in the undirected projection.
/// OS analogy: `systemctl list-dependencies --reverse` identifying
/// single-point-of-failure subsystems with no redundant dependency path.
pub fn dispatch_graph_articulation(sink: &ConsoleSink) {
    let (art_vecs, art_count, node_count) =
        gos_runtime::graph_articulation::<128>();

    set_color(sink, 11, 0);
    print_str(sink, " graph articulation points\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if art_count == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "  no single points of failure (fully biconnected)\n");
    } else {
        for i in 0..art_count {
            let fg: u8 = 12; // red — critical node
            set_color(sink, fg, 0);
            print_str(sink, "  cut vertex  ");
            let mut buf = LineBuf::<20>::new();
            buf.push_vector(art_vecs[i]);
            let s = core::str::from_utf8(buf.as_slice()).unwrap_or("?");
            print_str(sink, s);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, art_count);
    print_str(sink, " cut ");
    if art_count == 1 { print_str(sink, "vertex"); } else { print_str(sink, "vertices"); }
    print_str(sink, "  of  ");
    print_num_inline(sink, node_count);
    print_str(sink, " node(s)  resilience: ");
    if art_count == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "fully biconnected");
    } else if art_count * 4 <= node_count {
        set_color(sink, 14, 0);
        print_str(sink, "moderate risk");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "high risk");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.86: `graph bridges` — cut-edge detection.
/// Displays all bridges (edges whose removal disconnects the graph) in the
/// undirected projection of the live kernel graph.
/// OS analogy: identifying single uplinks between network segments — a link
/// whose failure partitions the routing fabric into isolated islands.
pub fn dispatch_graph_bridges(sink: &ConsoleSink) {
    let (from_vecs, to_vecs, bridge_count, node_count) =
        gos_runtime::graph_bridges::<{ gos_runtime::MAX_NODES }>();

    set_color(sink, 11, 0);
    print_str(sink, " graph bridges (cut edges)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if bridge_count == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "  no bridges (graph is 2-edge-connected or acyclic-free)\n");
    } else {
        for i in 0..bridge_count {
            set_color(sink, 12, 0); // red — critical edge
            print_str(sink, "  bridge  ");
            let mut buf = LineBuf::<20>::new();
            buf.push_vector(from_vecs[i]);
            let s = core::str::from_utf8(buf.as_slice()).unwrap_or("?");
            print_str(sink, s);
            set_color(sink, 8, 0);
            print_str(sink, "  \u{2500}\u{2500}  ");
            set_color(sink, 12, 0);
            let mut buf2 = LineBuf::<20>::new();
            buf2.push_vector(to_vecs[i]);
            let s2 = core::str::from_utf8(buf2.as_slice()).unwrap_or("?");
            print_str(sink, s2);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, bridge_count);
    print_str(sink, " bridge");
    if bridge_count != 1 { print_str(sink, "s"); }
    print_str(sink, "  of  ");
    print_num_inline(sink, node_count);
    print_str(sink, " node(s)  link resilience: ");
    if bridge_count == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "2-edge-connected");
    } else if bridge_count * 4 <= node_count {
        set_color(sink, 14, 0);
        print_str(sink, "moderate risk");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "high risk");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.87: `graph eulerian` — Eulerian path/circuit detection.
/// Checks whether the directed live kernel graph admits an Eulerian circuit
/// (closed walk visiting every edge exactly once) or Eulerian path (open walk).
/// OS analogy: can a maintenance daemon visit every IPC channel exactly once
/// and return to base (circuit), or perform a complete single-pass audit
/// (path)?
pub fn dispatch_graph_eulerian(sink: &ConsoleSink) {
    let (has_circuit, has_path, start_vec, end_vec, node_count) =
        gos_runtime::graph_eulerian();

    set_color(sink, 11, 0);
    print_str(sink, " graph eulerian\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if has_circuit {
        set_color(sink, 10, 0);
        print_str(sink, "  \u{2713} Eulerian circuit exists\n");
        set_color(sink, 7, 0);
        print_str(sink, "  every edge can be traversed exactly once in a closed walk\n");
        print_str(sink, "  (any node may serve as start/end)\n");
    } else if has_path {
        set_color(sink, 14, 0);
        print_str(sink, "  \u{2713} Eulerian path exists (not a circuit)\n");
        set_color(sink, 7, 0);
        print_str(sink, "  start  ");
        set_color(sink, 10, 0);
        let mut buf = LineBuf::<20>::new();
        buf.push_vector(start_vec);
        let s = core::str::from_utf8(buf.as_slice()).unwrap_or("?");
        print_str(sink, s);
        set_color(sink, 7, 0);
        print_str(sink, "   end  ");
        set_color(sink, 12, 0);
        let mut buf2 = LineBuf::<20>::new();
        buf2.push_vector(end_vec);
        let s2 = core::str::from_utf8(buf2.as_slice()).unwrap_or("?");
        print_str(sink, s2);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
        print_str(sink, "  every edge traversable exactly once in a single open walk\n");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "  \u{2717} no Eulerian path or circuit\n");
        set_color(sink, 7, 0);
        print_str(sink, "  graph is disconnected or degree imbalance > 1\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  nodes: ");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
}

/// V2.88: `graph dag longest` — critical path (longest directed path) in the DAG.
/// Analogous to `systemd-analyze critical-chain` for graph-native subsystems.
pub fn dispatch_graph_dag_longest(sink: &ConsoleSink) {
    let (path_hops, is_dag, start_vec, end_vec, node_count) =
        gos_runtime::graph_dag_longest();

    set_color(sink, 11, 0);
    print_str(sink, " graph dag longest\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if !is_dag {
        set_color(sink, 12, 0);
        print_str(sink, "  \u{2717} graph has directed cycles (not a DAG)\n");
        set_color(sink, 7, 0);
        print_str(sink, "  critical path is undefined for cyclic graphs\n");
        print_str(sink, "  use `graph cycles` or `graph scc` to inspect cycles\n");
    } else if path_hops == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  \u{2014} no directed edges (trivial DAG)\n");
        set_color(sink, 7, 0);
        print_str(sink, "  all nodes are isolated; critical path length = 0\n");
    } else {
        set_color(sink, 10, 0);
        print_str(sink, "  \u{2713} DAG  critical path: ");
        print_num_inline(sink, path_hops as usize);
        print_str(sink, " hop");
        if path_hops != 1 { print_str(sink, "s"); }
        print_str(sink, "\n");
        set_color(sink, 7, 0);
        print_str(sink, "  start  ");
        set_color(sink, 10, 0);
        let mut buf = LineBuf::<20>::new();
        buf.push_vector(start_vec);
        let s = core::str::from_utf8(buf.as_slice()).unwrap_or("?");
        print_str(sink, s);
        set_color(sink, 7, 0);
        print_str(sink, "   end  ");
        set_color(sink, 14, 0);
        let mut buf2 = LineBuf::<20>::new();
        buf2.push_vector(end_vec);
        let s2 = core::str::from_utf8(buf2.as_slice()).unwrap_or("?");
        print_str(sink, s2);
        print_str(sink, "\n");
        set_color(sink, 7, 0);
        print_str(sink, "  (minimum serial depth any parallel schedule must traverse)\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    let dag_str = if is_dag { "yes" } else { "no" };
    print_str(sink, "  is_dag: ");
    print_str(sink, dag_str);
    print_str(sink, "   nodes: ");
    print_num_inline(sink, node_count);
    print_str(sink, "\n");
}

/// V2.89: `graph dag layers` — topological level assignment for all DAG nodes.
/// Each node gets its minimum parallel-execution layer (0 = source, 1 = one hop, …).
/// Analogous to `systemd --analyze` dependency levels — which services start in parallel.
pub fn dispatch_graph_dag_layers(sink: &ConsoleSink) {
    let (vecs, layers, node_count, layer_count, is_dag) =
        gos_runtime::graph_dag_layers::<64>();

    set_color(sink, 11, 0);
    print_str(sink, " graph dag layers\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if !is_dag {
        set_color(sink, 12, 0);
        print_str(sink, "  \u{2717} graph has directed cycles (not a DAG)\n");
        set_color(sink, 7, 0);
        print_str(sink, "  topological layers are undefined for cyclic graphs\n");
        print_str(sink, "  use `graph scc` or `graph dag longest` to inspect structure\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        print_str(sink, "  nodes: ");
        print_num_inline(sink, node_count);
        print_str(sink, "\n");
        return;
    }

    set_color(sink, 7, 0);
    print_str(sink, "  layer  vector\n");
    set_color(sink, 8, 0);
    print_str(sink, "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    let n_show = node_count.min(64);
    let mut prev_layer = u32::MAX;
    for i in 0..n_show {
        let lyr = layers[i];
        // Print a blank separator between different layers for readability.
        if lyr != prev_layer && prev_layer != u32::MAX {
            print_str(sink, "\n");
        }
        prev_layer = lyr;
        set_color(sink, if lyr == 0 { 14 } else if lyr % 2 == 0 { 10 } else { 11 }, 0);
        print_str(sink, "  ");
        if lyr < 10   { print_str(sink, "    "); }
        else if lyr < 100  { print_str(sink, "   "); }
        else { print_str(sink, "  "); }
        print_num_inline(sink, lyr as usize);
        print_str(sink, "  ");
        let mut buf = LineBuf::<20>::new();
        buf.push_vector(vecs[i]);
        let s = core::str::from_utf8(buf.as_slice()).unwrap_or("?");
        print_str(sink, s);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  nodes: ");
    print_num_inline(sink, node_count);
    print_str(sink, "   layers: ");
    print_num_inline(sink, layer_count as usize);
    print_str(sink, "\n");
}

/// V2.90: `graph domtree <start>` — dominator tree from a given entry node.
/// Reports each reachable node and its immediate dominator (closest mandatory
/// predecessor on all paths from `start`).  The start node lists itself as
/// its own dominator.
///
/// OS analogy: which kernel subsystem must be running (with no bypass) before
/// each component can start — like `systemd-analyze critical-chain --all`
/// but giving every service its immediate mandatory antecedent.
pub fn dispatch_graph_domtree(sink: &ConsoleSink, start: gos_protocol::VectorAddress) {
    let (vecs, idoms, node_count, reachable_count) =
        gos_runtime::graph_domtree::<64>(start);

    set_color(sink, 11, 0);
    print_str(sink, " graph domtree  [start: ");
    let mut sbuf = LineBuf::<20>::new();
    sbuf.push_vector(start);
    let ss = core::str::from_utf8(sbuf.as_slice()).unwrap_or("?");
    print_str(sink, ss);
    print_str(sink, "]\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if reachable_count == 0 {
        set_color(sink, 12, 0);
        print_str(sink, "  start node not found in graph\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        print_str(sink, "  nodes: ");
        print_num_inline(sink, node_count);
        print_str(sink, "\n");
        return;
    }

    set_color(sink, 7, 0);
    print_str(sink, "  node           immediate dominator\n");
    set_color(sink, 8, 0);
    print_str(sink, "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}   \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    let n_show = reachable_count.min(64);
    for i in 0..n_show {
        let is_root = vecs[i] == idoms[i];
        if is_root {
            set_color(sink, 14, 0); // yellow for root
        } else {
            set_color(sink, 10, 0); // green for dominated nodes
        }
        print_str(sink, "  ");
        let mut nbuf = LineBuf::<20>::new();
        nbuf.push_vector(vecs[i]);
        let ns = core::str::from_utf8(nbuf.as_slice()).unwrap_or("?");
        print_str(sink, ns);
        // Pad to column 16.
        let nlen = nbuf.len();
        let pad  = if nlen < 14 { 14 - nlen } else { 1 };
        for _ in 0..pad { print_str(sink, " "); }
        set_color(sink, if is_root { 8 } else { 7 }, 0);
        let mut dbuf = LineBuf::<20>::new();
        dbuf.push_vector(idoms[i]);
        let ds = core::str::from_utf8(dbuf.as_slice()).unwrap_or("?");
        print_str(sink, ds);
        if is_root {
            print_str(sink, "  \u{2190} root");
        }
        print_str(sink, "\n");
    }
    if reachable_count > 64 {
        set_color(sink, 8, 0);
        print_str(sink, "  ... (truncated at 64)\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  nodes: ");
    print_num_inline(sink, node_count);
    print_str(sink, "   reachable: ");
    print_num_inline(sink, reachable_count);
    print_str(sink, "\n");
}

/// V2.91: `graph feedback arc` — feedback arc set (directed cycle-causing edges).
/// Lists every directed back-edge found by iterative DFS 3-coloring.
/// Removing these arcs makes the graph acyclic (a valid DAG).
/// OS analogy: circular kernel module dependencies — the exact edges that create
/// boot-order deadlocks; equivalent to what `tsort` prints as "cycle detected".
pub fn dispatch_graph_feedback_arc(sink: &ConsoleSink) {
    let (from_vecs, to_vecs, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<{ gos_runtime::MAX_EDGES }>();

    set_color(sink, 11, 0);
    print_str(sink, " graph feedback arc\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if arc_count == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "  no feedback arcs (graph is a DAG)\n");
    } else {
        set_color(sink, 7, 0);
        print_str(sink, "  from              \u{2192}  to\n");
        set_color(sink, 8, 0);
        print_str(sink, "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}     \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        let n_show = arc_count.min(gos_runtime::MAX_EDGES);
        for i in 0..n_show {
            set_color(sink, 12, 0);
            print_str(sink, "  ");
            let mut fbuf = LineBuf::<20>::new();
            fbuf.push_vector(from_vecs[i]);
            let fs = core::str::from_utf8(fbuf.as_slice()).unwrap_or("?");
            print_str(sink, fs);
            let flen = fbuf.len();
            let pad  = if flen < 16 { 16 - flen } else { 1 };
            for _ in 0..pad { print_str(sink, " "); }
            set_color(sink, 8, 0);
            print_str(sink, "\u{2192}  ");
            set_color(sink, 12, 0);
            let mut tbuf = LineBuf::<20>::new();
            tbuf.push_vector(to_vecs[i]);
            let ts = core::str::from_utf8(tbuf.as_slice()).unwrap_or("?");
            print_str(sink, ts);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, arc_count);
    print_str(sink, " feedback arc");
    if arc_count != 1 { print_str(sink, "s"); }
    print_str(sink, "  of  ");
    print_num_inline(sink, node_count);
    print_str(sink, " node(s)  dag status: ");
    if arc_count == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "acyclic");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "cyclic (");
        print_num_inline(sink, arc_count);
        print_str(sink, " arc");
        if arc_count != 1 { print_str(sink, "s"); }
        print_str(sink, " to remove)");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.92: `graph bipartite match` — maximum bipartite matching (Kuhn's algorithm).
///
/// Displays the matched side-A / side-B pairs and the matching size.
/// If the graph is not bipartite, reports the conflict.
/// OS analogy: `taskset` optimal CPU↔task affinity assignment.
pub fn dispatch_graph_bipartite_match(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (left_vecs, right_vecs, match_count, is_bipartite, node_count) =
        gos_runtime::graph_bipartite_match::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph bipartite match\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if !is_bipartite {
        set_color(sink, 12, 0);
        print_str(sink, "  NOT bipartite  (odd-length cycle detected)\n");
        set_color(sink, 8, 0);
        print_str(sink, "  hint: use 'graph bipartite' to inspect the partition, 'graph cycles' for cycles\n");
    } else if match_count == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "  bipartite graph with no edges  (empty matching)\n");
    } else {
        set_color(sink, 7, 0);
        print_str(sink, "  side A              \u{2194}  side B\n");
        set_color(sink, 8, 0);
        print_str(sink, "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}     \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        for i in 0..match_count.min(MAX_N) {
            set_color(sink, 14, 0);
            print_str(sink, "  ");
            let mut lbuf = LineBuf::<20>::new();
            lbuf.push_vector(left_vecs[i]);
            let ls = core::str::from_utf8(lbuf.as_slice()).unwrap_or("?");
            print_str(sink, ls);
            let llen = lbuf.len();
            let pad = if llen < 16 { 16 - llen } else { 1 };
            for _ in 0..pad { print_str(sink, " "); }
            set_color(sink, 8, 0);
            print_str(sink, "\u{2194}  ");
            set_color(sink, 14, 0);
            let mut rbuf = LineBuf::<20>::new();
            rbuf.push_vector(right_vecs[i]);
            let rs = core::str::from_utf8(rbuf.as_slice()).unwrap_or("?");
            print_str(sink, rs);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    if is_bipartite {
        print_str(sink, "  max matching: ");
        print_num_inline(sink, match_count);
        print_str(sink, " pair");
        if match_count != 1 { print_str(sink, "s"); }
        print_str(sink, "  of  ");
        print_num_inline(sink, node_count);
        print_str(sink, " node(s)\n");
    } else {
        print_str(sink, "  ");
        print_num_inline(sink, node_count);
        print_str(sink, " node(s) checked\n");
    }
}

/// V2.93: `graph 2ecc` — 2-edge-connected components.
///
/// Shows every 2ECC with its member nodes grouped by component ID.
/// Each component is a maximal set of nodes where any single edge failure
/// cannot partition the group (≥2 edge-disjoint paths exist between all pairs).
pub fn dispatch_graph_2ecc(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, comp_ids, node_count, comp_count) =
        gos_runtime::graph_2ecc::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph 2-edge-connected components\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Print nodes grouped by component ID (output is already sorted by comp_id)
    let mut cur_cid = u8::MAX;
    let mut cid_size = 0usize;
    let mut i = 0usize;
    while i <= node_count.min(MAX_N) {
        let flush = i == node_count.min(MAX_N)
            || comp_ids[i] != cur_cid;

        if flush && cur_cid != u8::MAX {
            print_str(sink, "\n");
        }

        if i == node_count.min(MAX_N) { break; }

        if comp_ids[i] != cur_cid {
            cur_cid  = comp_ids[i];
            cid_size = 0;
            // Count size of this component
            let mut k = i;
            while k < node_count.min(MAX_N) && comp_ids[k] == cur_cid {
                cid_size += 1;
                k += 1;
            }
            set_color(sink, 14, 0);
            print_str(sink, "  comp ");
            print_num_inline(sink, cur_cid as usize);
            set_color(sink, 8, 0);
            print_str(sink, "  [");
            print_num_inline(sink, cid_size);
            print_str(sink, " node");
            if cid_size != 1 { print_str(sink, "s"); }
            print_str(sink, "]\n");
            set_color(sink, 7, 0);
            print_str(sink, "    ");
        } else {
            print_str(sink, "  ");
        }

        let mut buf = LineBuf::<20>::new();
        buf.push_vector(vecs[i]);
        let s = core::str::from_utf8(buf.as_slice()).unwrap_or("?");
        print_str(sink, s);
        i += 1;
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, comp_count);
    print_str(sink, " component");
    if comp_count != 1 { print_str(sink, "s"); }
    print_str(sink, " across ");
    print_num_inline(sink, node_count);
    print_str(sink, " node");
    if node_count != 1 { print_str(sink, "s"); }
    print_str(sink, "\n");
}

/// V2.94: `graph truss` — k-truss decomposition (edge-triangle cohesion).
///
/// Complements k-core (V2.64): while k-core measures per-node degree within
/// a subgraph, k-truss measures per-EDGE triangle participation.
/// A node's trussness = max k such that it has an incident edge in the k-truss.
///   trussness 0 = isolated node
///   trussness 2 = has edges but no triangle (a "bridge" node)
///   trussness k ≥ 3 = participates in tightly-knit triangle cluster
///
/// OS analogy: subsystems with high trussness have redundant mutual dependencies
/// (multiple paths through shared intermediaries); hard to isolate by removing
/// any single node — like bonded/redundant kernel module clusters.
pub fn dispatch_graph_truss(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, trussness, total, max_truss) = gos_runtime::graph_truss::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph truss  (k-truss decomposition)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              k     role\n");
    set_color(sink, 7, 0);

    for i in 0..total.min(MAX_N) {
        let t = trussness[i];
        let is_core = max_truss > 0 && t == max_truss;
        let is_iso  = t == 0;
        let is_leaf = t == 2; // edge but no triangle

        if is_core {
            set_color(sink, 10, 0); // bright green — densest truss
        } else if is_iso {
            set_color(sink, 8, 0);  // grey — isolated
        } else if is_leaf {
            set_color(sink, 14, 0); // yellow — leaf (edge, no triangle)
        } else {
            set_color(sink, 11, 0); // cyan — inner truss
        }

        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..16 { print_str(sink, " "); }

        print_str(sink, " ");
        print_num_right6(sink, t as usize);
        print_str(sink, "  ");

        if is_core {
            set_color(sink, 10, 0);
            print_str(sink, "truss-core");
        } else if is_iso {
            set_color(sink, 8, 0);
            print_str(sink, "isolated");
        } else if is_leaf {
            set_color(sink, 14, 0);
            print_str(sink, "leaf");
        } else {
            set_color(sink, 11, 0);
            print_str(sink, "inner");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  truss-number=");
    set_color(sink, 10, 0);
    print_num_inline(sink, max_truss as usize);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.95: `graph clique` — maximum clique (Bron-Kerbosch with Tomita pivot).
/// Density hierarchy apex: every k-clique is a (k-1)-core and a k-truss.
/// OS analogy: the tightest fully-interconnected kernel subsystem cluster.
pub fn dispatch_graph_clique(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, clique_size, clique_count, total) = gos_runtime::graph_clique::<MAX_N>();

    set_color(sink, 11, 0);
    print_str(sink, " graph clique  (\u{3c9}(G) maximum clique)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if clique_size == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no edges \u{2014} \u{3c9}(G) = 0)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        print_str(sink, "  ");
        print_num_inline(sink, total);
        set_color(sink, 8, 0);
        print_str(sink, " node(s)  \u{3c9}(G)=");
        set_color(sink, 10, 0);
        print_num_inline(sink, clique_size);
        set_color(sink, 8, 0);
        print_str(sink, "  distinct-max-cliques=");
        set_color(sink, 10, 0);
        print_num_inline(sink, clique_count);
        set_color(sink, 7, 0);
        print_str(sink, "\n");
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              role\n");
    set_color(sink, 7, 0);

    for i in 0..clique_size.min(MAX_N) {
        set_color(sink, 10, 0); // bright green — clique member
        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..18 { print_str(sink, " "); }
        print_str(sink, "clique-member");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  \u{3c9}(G)=");
    set_color(sink, 10, 0);
    print_num_inline(sink, clique_size);
    set_color(sink, 8, 0);
    print_str(sink, "  distinct-max-cliques=");
    set_color(sink, 10, 0);
    print_num_inline(sink, clique_count);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_independent_set(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, is_size, is_count, total) = gos_runtime::graph_independent_set::<MAX_N>();

    set_color(sink, 13, 0);
    print_str(sink, " graph independent set  (\u{3b1}(G) maximum independent set)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              role\n");
    set_color(sink, 7, 0);

    for i in 0..is_size.min(MAX_N) {
        set_color(sink, 9, 0); // bright blue — independent set member
        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..18 { print_str(sink, " "); }
        print_str(sink, "indep-member");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  \u{3b1}(G)=");
    set_color(sink, 9, 0);
    print_num_inline(sink, is_size);
    set_color(sink, 8, 0);
    print_str(sink, "  distinct-max-indep-sets=");
    set_color(sink, 9, 0);
    print_num_inline(sink, is_count);
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.97: `graph vertex cover` — minimum vertex cover τ(G).
///
/// Bipartite graphs: exact via König's theorem (τ = ν, max matching size).
/// General graphs: 2-approximation via greedy maximal matching.
/// Gallai invariant (bipartite): α(G) + τ(G) = node_count.
pub fn dispatch_graph_vertex_cover(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, cover_size, is_exact, total) =
        gos_runtime::graph_vertex_cover::<MAX_N>();

    set_color(sink, 11, 0); // bright-cyan header
    print_str(sink, " graph vertex cover  (\u{3c4}(G) minimum vertex cover)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              role\n");
    set_color(sink, 7, 0);

    for i in 0..cover_size.min(MAX_N) {
        set_color(sink, 10, 0); // bright-green — cover vertex
        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..18 { print_str(sink, " "); }
        print_str(sink, "cover-vertex");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  \u{3c4}(G)=");
    set_color(sink, 10, 0);
    print_num_inline(sink, cover_size);
    set_color(sink, 8, 0);
    if is_exact {
        print_str(sink, "  exact (bipartite K\u{f6}nig)");
    } else {
        print_str(sink, "  2-approx (non-bipartite)");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.98 — `graph domset` / `gdomset` — minimum dominating set (greedy).
///
/// Displays the greedy dominating set D and summary statistics.
/// γ(G) = |D| (minimum dominating set size).
/// Greedy achieves ≤ H(Δ)+1 ≈ ln(Δ)+1 approximation.
/// OS analogy: minimum monitor deployment — fewest subsystems to instrument so
/// every uninstrumented module has at least one directly-adjacent instrumented neighbour.
pub fn dispatch_graph_dominating_set(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, dom_size, total) =
        gos_runtime::graph_dominating_set::<MAX_N>();

    set_color(sink, 14, 0); // bright-yellow header
    print_str(sink, " graph dominating set  (\u{3b3}(G) minimum dominating set)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  vector              role\n");
    set_color(sink, 7, 0);

    for i in 0..dom_size.min(MAX_N) {
        set_color(sink, 11, 0); // bright-cyan — dominator
        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..18 { print_str(sink, " "); }
        print_str(sink, "dominator");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  \u{3b3}(G)=");
    set_color(sink, 11, 0);
    print_num_inline(sink, dom_size);
    set_color(sink, 8, 0);
    print_str(sink, "  greedy \u{2264} ln(\u{394})+1 approx");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.99: minimum path cover of the live kernel graph (DAG only).
pub fn dispatch_graph_min_path_cover(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, path_ids, path_count, is_dag, total) =
        gos_runtime::graph_min_path_cover::<MAX_N>();

    set_color(sink, 14, 0); // bright-yellow header
    print_str(sink, " minimum path cover  (MPC \u{2014} DAG only)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if !is_dag {
        set_color(sink, 12, 0);
        print_str(sink, "  graph has cycles \u{2014} MPC undefined (not a DAG)\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    // Path color palette — cycles through 6 distinct bright colors.
    const PATH_COLORS: [u8; 6] = [10, 11, 13, 9, 14, 12];

    set_color(sink, 8, 0);
    print_str(sink, "  path  vector\n");
    set_color(sink, 7, 0);

    let mut prev_pid: u8 = u8::MAX;
    for i in 0..total.min(MAX_N) {
        let pid = path_ids[i];
        if pid != prev_pid && prev_pid != u8::MAX {
            set_color(sink, 8, 0);
            print_str(sink, "  \u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\u{2508}\n");
            set_color(sink, 7, 0);
        }
        prev_pid = pid;
        let color = PATH_COLORS[(pid as usize) % 6];
        set_color(sink, color, 0);
        print_str(sink, "  P");
        print_num_inline(sink, pid as usize);
        print_str(sink, "    ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  MPC=");
    set_color(sink, 14, 0);
    print_num_inline(sink, path_count);
    set_color(sink, 8, 0);
    print_str(sink, "  (n\u{2212}\u{3bd}=");
    print_num_inline(sink, total);
    print_str(sink, "\u{2212}");
    print_num_inline(sink, total - path_count);
    print_str(sink, ")  K\u{f6}nig\u{2215}Dilworth");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.00: `graph arborescence <root>` — minimum spanning arborescence
/// (directed MST) from a given root node via Chu-Liu / Edmonds 1967.
pub fn dispatch_graph_arborescence(sink: &ConsoleSink, root: gos_protocol::VectorAddress) {
    const MAX_N: usize = 128;
    let (vecs, parents, weights, total, total_w, is_conn) =
        gos_runtime::graph_arborescence::<MAX_N>(root);

    set_color(sink, 11, 0); // bright-cyan header
    print_str(sink, " min spanning arborescence  (Chu-Liu / Edmonds)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if total == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if !is_conn {
        set_color(sink, 12, 0);
        print_str(sink, "  not connected \u{2014} no spanning arborescence from root\n");
        set_color(sink, 8, 0);
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    set_color(sink, 8, 0);
    print_str(sink, "  role    weight    vector           parent\n");
    set_color(sink, 7, 0);

    for i in 0..total {
        let vec    = vecs[i];
        let parent = parents[i];
        let w_u32  = weights[i];
        let is_root = vec == parent;

        if is_root {
            set_color(sink, 13, 0); // magenta = root
            print_str(sink, "  root  ");
        } else {
            set_color(sink, 11, 0); // cyan = child
            print_str(sink, "  child ");
        }

        set_color(sink, 14, 0);
        let whole = w_u32 / 1000;
        let frac  = w_u32 % 1000;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10  { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac as usize);
        print_str(sink, "  ");

        set_color(sink, 7, 0);
        let mut vbuf = LineBuf::<20>::new();
        vbuf.push_vector(vec);
        let vs = core::str::from_utf8(vbuf.as_slice()).unwrap_or("?");
        print_str(sink, vs);
        let vpad = 17usize.saturating_sub(vs.len());
        for _ in 0..vpad { print_str(sink, " "); }

        if is_root {
            set_color(sink, 8, 0);
            print_str(sink, "(root)");
        } else {
            set_color(sink, 7, 0);
            let mut pbuf = LineBuf::<20>::new();
            pbuf.push_vector(parent);
            let ps = core::str::from_utf8(pbuf.as_slice()).unwrap_or("?");
            print_str(sink, ps);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, total);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  MSA-weight=");
    set_color(sink, 14, 0);
    let w_whole = total_w / 1000;
    let w_frac  = total_w % 1000;
    print_num_inline(sink, w_whole as usize);
    print_str(sink, ".");
    if w_frac < 10  { print_str(sink, "00"); }
    else if w_frac < 100 { print_str(sink, "0"); }
    print_num_inline(sink, w_frac as usize);
    set_color(sink, 8, 0);
    print_str(sink, "  Chu-Liu\u{2215}Edmonds");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.01: `graph fvs` — feedback vertex set (greedy Kahn-based).
///
/// Returns the minimum-ish set of nodes whose removal makes the directed graph
/// acyclic.  Uses iterative Kahn BFS: each round picks the undrained node with
/// the highest in-degree × out-degree score and removes it until no cycles remain.
///
/// OS analogy: minimum set of kernel subsystems to suspend / quarantine to break
/// all boot-order dependency cycles — like `systemctl mask` targeting the
/// cycle-causing services identified by `systemd-analyze verify`.
/// V3.02: `graph min cut` — Stoer-Wagner global minimum edge cut.
/// Analogous to `ip route show` finding the minimum bottleneck partition —
/// the fewest IPC channels to sever to split the kernel into two isolated fault domains.
pub fn dispatch_graph_min_cut(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, sides, node_count, min_cut, side_b_size) =
        gos_runtime::graph_min_cut::<MAX_N>();

    set_color(sink, 11, 0); // bright-cyan header — partition theme
    print_str(sink, " graph min cut  (Stoer-Wagner global minimum edge cut)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    let side_a_size = node_count - side_b_size;

    // Header row
    set_color(sink, 8, 0);
    print_str(sink, "  vector              side\n");
    set_color(sink, 7, 0);

    for i in 0..node_count.min(MAX_N) {
        if sides[i] == 0 {
            set_color(sink, 10, 0); // bright-green for side A
        } else {
            set_color(sink, 13, 0); // bright-magenta for side B
        }
        print_str(sink, "  ");
        let mut line = LineBuf::<20>::new();
        line.push_vector(vecs[i]);
        let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
        print_str(sink, vec_str);
        let vlen = vec_str.len();
        for _ in vlen..18 { print_str(sink, " "); }
        if sides[i] == 0 {
            print_str(sink, "side-A");
        } else {
            print_str(sink, "side-B");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  A=");
    set_color(sink, 10, 0);
    print_num_inline(sink, side_a_size);
    set_color(sink, 8, 0);
    print_str(sink, "  B=");
    set_color(sink, 13, 0);
    print_num_inline(sink, side_b_size);
    set_color(sink, 8, 0);
    print_str(sink, "  \u{03ba}'(G)=");
    set_color(sink, 11, 0);
    print_num_inline(sink, min_cut as usize);
    set_color(sink, 8, 0);
    if min_cut == 0 {
        print_str(sink, "  (disconnected or empty)");
    } else {
        print_str(sink, "  Stoer-Wagner 1997");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_fvs(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, fvs_size, node_count) = gos_runtime::graph_fvs::<MAX_N>();

    set_color(sink, 12, 0); // bright-red header — "removal" theme
    print_str(sink, " graph feedback vertex set  (FVS cycle-breaking)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if fvs_size == 0 {
        set_color(sink, 10, 0); // green — no cycles
        print_str(sink, "  no feedback vertices (graph is a DAG)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  vector              role\n");
        set_color(sink, 7, 0);

        for i in 0..fvs_size.min(MAX_N) {
            set_color(sink, 12, 0); // bright-red — cycle contributor
            print_str(sink, "  ");
            let mut line = LineBuf::<20>::new();
            line.push_vector(vecs[i]);
            let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
            print_str(sink, vec_str);
            let vlen = vec_str.len();
            for _ in vlen..18 { print_str(sink, " "); }
            print_str(sink, "fvs-member");
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  FVS=");
    set_color(sink, 12, 0);
    print_num_inline(sink, fvs_size);
    set_color(sink, 8, 0);
    print_str(sink, "  greedy in\u{2218}out score  dag-status: ");
    if fvs_size == 0 {
        set_color(sink, 10, 0);
        print_str(sink, "acyclic");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "cyclic (remove ");
        print_num_inline(sink, fvs_size);
        print_str(sink, " node");
        if fvs_size != 1 { print_str(sink, "s"); }
        print_str(sink, ")");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_hamiltonian(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<MAX_N>();

    set_color(sink, 10, 0); // bright-green header — "path found" theme
    print_str(sink, " graph hamiltonian  (Hamiltonian path/circuit detection)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
        print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
        set_color(sink, 7, 0);
        return;
    }

    if !has_path {
        set_color(sink, 12, 0); // bright-red — not found
        print_str(sink, "  no Hamiltonian path or circuit found\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  pos  vector\n");
        set_color(sink, 7, 0);

        for i in 0..path_len.min(MAX_N) {
            if has_circuit {
                set_color(sink, 10, 0); // bright-green for circuit
            } else {
                set_color(sink, 14, 0); // bright-yellow for path-only
            }
            print_str(sink, "  ");
            // Position number (1-indexed), right-aligned to 3 chars
            let pos = i + 1;
            if pos < 10 {
                print_str(sink, "  ");
            } else if pos < 100 {
                print_str(sink, " ");
            }
            print_num_inline(sink, pos);
            print_str(sink, "  ");
            let mut line = LineBuf::<20>::new();
            line.push_vector(vecs[i]);
            let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
        if has_circuit {
            // Show the closing edge back to start
            set_color(sink, 8, 0);
            print_str(sink, "  \u{21ba} back to start (circuit)\n");
            set_color(sink, 7, 0);
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    if has_circuit {
        set_color(sink, 10, 0);
        print_str(sink, "Hamiltonian circuit");
        set_color(sink, 8, 0);
        print_str(sink, "  (visits every node once, returns to start)");
    } else if has_path {
        set_color(sink, 14, 0);
        print_str(sink, "Hamiltonian path");
        set_color(sink, 8, 0);
        print_str(sink, "  (visits every node once, no circuit)");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "no Hamiltonian path");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.04: `graph chordal` — chordal graph recognition (LexBFS + PEO check).
pub fn dispatch_graph_chordal(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, is_chordal, node_count) = gos_runtime::graph_chordal::<MAX_N>();

    set_color(sink, 11, 0); // bright-cyan header
    print_str(sink, " graph chordal  (chordal graph recognition)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
    } else {
        // ── Chordal status line ──────────────────────────────────────────
        if is_chordal {
            set_color(sink, 10, 0); // bright-green
            print_str(sink, "  \u{2713} chordal");
            set_color(sink, 8, 0);
            print_str(sink, "  (every cycle \u{2265}4 has a chord; PEO valid)\n");
        } else {
            set_color(sink, 12, 0); // bright-red
            print_str(sink, "  \u{2717} not chordal");
            set_color(sink, 8, 0);
            print_str(sink, "  (chordless cycle of length \u{2265}4 detected)\n");
        }

        // ── PEO table ────────────────────────────────────────────────────
        set_color(sink, 8, 0);
        print_str(sink, "\n  LexBFS elimination ordering:\n");
        print_str(sink, "  pos  vector\n");
        for i in 0..node_count.min(MAX_N) {
            if is_chordal {
                set_color(sink, 11, 0); // bright-cyan
            } else {
                set_color(sink, 13, 0); // bright-magenta
            }
            print_str(sink, "  ");
            let pos = i + 1;
            if pos < 10 {
                print_str(sink, "  ");
            } else if pos < 100 {
                print_str(sink, " ");
            }
            print_num_inline(sink, pos);
            print_str(sink, "  ");
            let mut line = LineBuf::<20>::new();
            line.push_vector(vecs[i]);
            let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    if is_chordal {
        set_color(sink, 10, 0);
        print_str(sink, "chordal");
        set_color(sink, 8, 0);
        print_str(sink, "  PEO valid  LexBFS (Rose, Tarjan & Lueker 1976)");
    } else {
        set_color(sink, 12, 0);
        print_str(sink, "not chordal");
        set_color(sink, 8, 0);
        print_str(sink, "  chordless 4+ cycle  LexBFS (Rose, Tarjan & Lueker 1976)");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.05: `graph bcc` — biconnected components.
/// Shows each node's BCC id and flags articulation points (bcc_id=255).
pub fn dispatch_graph_bcc(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, bcc_ids, node_count, bcc_count) = gos_runtime::graph_bcc::<MAX_N>();

    set_color(sink, 14, 0); // bright-yellow header
    print_str(sink, " graph bcc  (biconnected components)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  bcc  role          vector\n");

        for i in 0..node_count.min(MAX_N) {
            let bid = bcc_ids[i];
            if bid == 255 {
                // Articulation point: bright-red label
                set_color(sink, 12, 0);
                print_str(sink, "  AP   cut-vertex    ");
            } else {
                // Regular BCC member: cycle through 6 colours per BCC
                let colour: u8 = match bid % 6 {
                    0 => 10, // bright-green
                    1 => 11, // bright-cyan
                    2 => 13, // bright-magenta
                    3 => 9,  // bright-blue
                    4 => 14, // bright-yellow
                    _ => 15, // bright-white
                };
                set_color(sink, colour, 0);
                print_str(sink, "  ");
                if (bid as usize) < 10  { print_str(sink, "  "); }
                else if (bid as usize) < 100 { print_str(sink, " "); }
                print_num_inline(sink, bid as usize);
                print_str(sink, "   BCC-member    ");
            }
            let mut line = LineBuf::<20>::new();
            line.push_vector(vecs[i]);
            let vec_str = core::str::from_utf8(line.as_slice()).unwrap_or("?");
            print_str(sink, vec_str);
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
    }

    // Count AP nodes for footer
    let mut ap_count = 0usize;
    for i in 0..node_count.min(MAX_N) {
        if bcc_ids[i] == 255 { ap_count += 1; }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    set_color(sink, 14, 0);
    print_num_inline(sink, bcc_count);
    set_color(sink, 8, 0);
    print_str(sink, " BCC(s)  ");
    if ap_count > 0 {
        set_color(sink, 12, 0);
        print_num_inline(sink, ap_count);
        set_color(sink, 8, 0);
        print_str(sink, " AP(s)  ");
    }
    print_str(sink, "Tarjan 1972");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.06: Edge Betweenness Centrality — directed graph link criticality.
/// Each edge's score = number of ordered source-target pairs whose unique
/// shortest path traverses that edge.  High-score edges are critical links.
pub fn dispatch_graph_betweenness_edge(sink: &ConsoleSink) {
    const MAX_N: usize = 512;
    let (from_vecs, to_vecs, scores, edge_count) =
        gos_runtime::graph_betweenness_edge::<MAX_N>();

    set_color(sink, 14, 0);
    print_str(sink, " graph ebc  (edge betweenness centrality)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if edge_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no directed edges)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  score     from              to\n");

        const COLOURS: [u8; 6] = [10, 11, 13, 9, 14, 15];
        for i in 0..edge_count.min(MAX_N) {
            let col = COLOURS[i % 6];
            set_color(sink, col, 0);
            // Right-align score in 6 chars.
            let sc = scores[i] as usize;
            if sc < 10      { print_str(sink, "       "); }
            else if sc < 100  { print_str(sink, "      "); }
            else if sc < 1000 { print_str(sink, "     "); }
            else if sc < 10000 { print_str(sink, "    "); }
            else              { print_str(sink, "   "); }
            print_num_inline(sink, sc);
            print_str(sink, "  ");
            let mut fl = LineBuf::<20>::new();
            fl.push_vector(from_vecs[i]);
            print_str(sink, core::str::from_utf8(fl.as_slice()).unwrap_or("?"));
            print_str(sink, "  \u{2192}  ");
            let mut tl = LineBuf::<20>::new();
            tl.push_vector(to_vecs[i]);
            print_str(sink, core::str::from_utf8(tl.as_slice()).unwrap_or("?"));
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, edge_count);
    set_color(sink, 8, 0);
    print_str(sink, " directed edge(s)  Brandes 2001");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

// ── V3.07: graph vertex connectivity ─────────────────────────────────────────
pub fn dispatch_graph_vertex_connectivity(sink: &ConsoleSink) {
    const MAX_N: usize = 128;
    let (vecs, node_count, kappa, min_deg) =
        gos_runtime::graph_vertex_connectivity::<MAX_N>();

    set_color(sink, 11, 0); // bright-cyan header — connectivity theme
    print_str(sink, " graph vertex connectivity  (\u{03ba}(G) minimum vertex cut)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no nodes registered)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  vector              deg\n");
        set_color(sink, 7, 0);

        for i in 0..node_count.min(MAX_N) {
            // Compute undirected degree for display (re-use kappa path data)
            set_color(sink, 10, 0); // bright-green nodes
            print_str(sink, "  ");
            let mut lb = LineBuf::<20>::new();
            lb.push_vector(vecs[i]);
            let vs = core::str::from_utf8(lb.as_slice()).unwrap_or("?");
            print_str(sink, vs);
            let vlen = vs.len();
            for _ in vlen..18 { print_str(sink, " "); }
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_str(sink, "  ");
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    set_color(sink, 11, 0);
    print_str(sink, "\u{03ba}(G)=");
    set_color(sink, 14, 0);
    print_num_inline(sink, kappa as usize);
    set_color(sink, 8, 0);
    print_str(sink, "  \u{03b4}(G)=");
    set_color(sink, 14, 0);
    print_num_inline(sink, min_deg as usize);
    set_color(sink, 8, 0);
    if node_count == 0 || kappa == 0 {
        print_str(sink, "  (disconnected or empty)");
    } else {
        print_str(sink, "  Whitney: \u{03ba}\u{2264}\u{03b4}  Menger 1927");
    }
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

// ── V3.08: Edge Colouring ─────────────────────────────────────────────────────
pub fn dispatch_graph_edge_color(sink: &ConsoleSink) {
    const MAX_N: usize = 512;
    let (from_vecs, to_vecs, edge_colors, edge_count, chromatic_index) =
        gos_runtime::graph_edge_color::<MAX_N>();

    set_color(sink, 10, 0); // bright-green — coloring theme
    print_str(sink, " graph edge color  (\u{03c7}'(G) chromatic index, Vizing 1964)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if edge_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no edges)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  slot  from              \u{2014}  to\n");
        set_color(sink, 7, 0);

        const SLOT_COLOURS: [u8; 6] = [10, 11, 13, 9, 14, 15];
        for i in 0..edge_count.min(MAX_N) {
            let col = SLOT_COLOURS[edge_colors[i] as usize % 6];
            set_color(sink, col, 0);
            let sc = edge_colors[i] as usize;
            if sc < 10 { print_str(sink, "   "); } else { print_str(sink, "  "); }
            print_num_inline(sink, sc);
            print_str(sink, "  ");
            let mut fl = LineBuf::<20>::new();
            fl.push_vector(from_vecs[i]);
            let fs = core::str::from_utf8(fl.as_slice()).unwrap_or("?");
            print_str(sink, fs);
            let flen = fs.len();
            for _ in flen..18 { print_str(sink, " "); }
            print_str(sink, "  \u{2014}  ");
            let mut tl = LineBuf::<20>::new();
            tl.push_vector(to_vecs[i]);
            print_str(sink, core::str::from_utf8(tl.as_slice()).unwrap_or("?"));
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, edge_count);
    set_color(sink, 8, 0);
    print_str(sink, " undirected edge(s)  ");
    set_color(sink, 10, 0);
    print_str(sink, "\u{03c7}'(G)=");
    set_color(sink, 14, 0);
    print_num_inline(sink, chromatic_index as usize);
    set_color(sink, 8, 0);
    print_str(sink, "  Vizing 1964");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.09: `graph spectral` — spectral radius ρ(A) and algebraic connectivity λ₂(L).
/// Fiedler 1973 / power iteration (60 steps per phase).
pub fn dispatch_graph_spectral(sink: &ConsoleSink) {
    let (rho_ppm, lambda2_ppm, node_count) = gos_runtime::graph_spectral();

    set_color(sink, 9, 0); // bright-blue — spectral theme
    print_str(sink, " graph spectral  (\u{03c1}(A) + \u{03bb}\u{2082}(L) via power iteration)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    // Print helper: value_ppm as "W.XXX" (integer.3-decimal).
    fn print_ppm3_spectral(sink: &ConsoleSink, ppm: u32) {
        let whole = ppm / 1_000_000;
        let frac  = (ppm % 1_000_000) / 1_000;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac as usize);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  spectral radius  \u{03c1}(A)  =  ");
        set_color(sink, 14, 0); // bright-yellow value
        print_ppm3_spectral(sink, rho_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   epidemic threshold: 1/\u{03c1}");
        if rho_ppm > 0 {
            print_str(sink, " = ");
            set_color(sink, 11, 0);
            let thresh = 1_000_000u64 * 1_000_000 / rho_ppm as u64;
            print_ppm3_spectral(sink, thresh.min(u32::MAX as u64) as u32);
            set_color(sink, 8, 0);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        set_color(sink, 8, 0);
        print_str(sink, "  algebraic conn  \u{03bb}\u{2082}(L) =  ");
        if lambda2_ppm == 0 {
            set_color(sink, 12, 0); // bright-red — disconnected
            print_str(sink, "0.000");
            set_color(sink, 8, 0);
            print_str(sink, "   graph is disconnected");
        } else {
            set_color(sink, 10, 0); // bright-green — connected
            print_ppm3_spectral(sink, lambda2_ppm);
            set_color(sink, 8, 0);
            print_str(sink, "   Cheeger bound: h \u{2265} \u{03bb}\u{2082}/2");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  power iteration (60 steps)  Fiedler 1973");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.10: `graph entropy` — Shannon entropy H of the undirected degree distribution.
/// Shannon 1948 / Dehmer & Mowshowitz 2011.
pub fn dispatch_graph_entropy(sink: &ConsoleSink) {
    let (entropy_ppm, normalized_ppm, node_count) = gos_runtime::graph_entropy();

    set_color(sink, 13, 0); // bright-magenta — information-theoretic theme
    print_str(sink, " graph entropy  (H = \u{2212}\u{03a3} p(d) ln p(d)  degree distribution)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_ent(sink: &ConsoleSink, ppm: u32) {
        let whole = ppm / 1_000_000;
        let frac  = (ppm % 1_000_000) / 1_000;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac as usize);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // Row 1: H (entropy in nats)
        set_color(sink, 8, 0);
        print_str(sink, "  entropy      H   =  ");
        set_color(sink, 14, 0); // bright-yellow
        print_ppm3_ent(sink, entropy_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   nat");
        if entropy_ppm == 0 {
            print_str(sink, "   (regular: all nodes share one degree)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Row 2: H' = H / ln(n) normalised
        set_color(sink, 8, 0);
        print_str(sink, "  normalised  H\u{2019}  =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_ent(sink, normalized_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [H / ln(n)]   ");
        if normalized_ppm >= 900_000 {
            print_str(sink, "high diversity");
        } else if normalized_ppm >= 500_000 {
            print_str(sink, "moderate diversity");
        } else if normalized_ppm > 0 {
            print_str(sink, "low diversity");
        } else {
            print_str(sink, "uniform (one degree class)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Row 3: max entropy ln(n) — recover from entropy_ppm / normalized_ppm when both > 0
        if entropy_ppm > 0 && normalized_ppm > 0 {
            let max_ppm = (entropy_ppm as u64 * 1_000_000 / normalized_ppm as u64) as u32;
            set_color(sink, 8, 0);
            print_str(sink, "  max entropy     =  ");
            set_color(sink, 11, 0); // bright-cyan
            print_ppm3_ent(sink, max_ppm);
            set_color(sink, 8, 0);
            print_str(sink, "   [ln(n)]");
            set_color(sink, 7, 0);
            print_str(sink, "\n");
        }
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  Shannon 1948  Dehmer & Mowshowitz 2011");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.11: `graph zagreb` — Zagreb indices M1/M2, Randić R, and Albertson irregularity I.
/// Gutman & Trinajstić 1972 (M1/M2) / Randić 1975 (R) / Albertson 1997 (I).
pub fn dispatch_graph_zagreb(sink: &ConsoleSink) {
    let (m1, m2, randic_ppm, irregularity, edge_count, node_count) =
        gos_runtime::graph_zagreb();

    set_color(sink, 11, 0); // bright-cyan — chemical/topological index theme
    print_str(sink, " graph zagreb  (M1 + M2 + Rand\u{00ed}c R + Albertson I)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_zg(sink: &ConsoleSink, ppm: u32) {
        let whole = ppm / 1_000_000;
        let frac  = (ppm % 1_000_000) / 1_000;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac as usize);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // M1 — integer, no ppm
        set_color(sink, 8, 0);
        print_str(sink, "  first Zagreb   M\u{2081}  =  ");
        set_color(sink, 14, 0); // bright-yellow
        print_num_inline(sink, m1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} deg(v)\u{00b2}]");
        if irregularity == 0 { print_str(sink, "   (regular graph)"); }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // M2 — integer, no ppm
        set_color(sink, 8, 0);
        print_str(sink, "  second Zagreb  M\u{2082}  =  ");
        set_color(sink, 14, 0);
        print_num_inline(sink, m2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} deg(u)\u{00d7}deg(v)]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Randić
        set_color(sink, 8, 0);
        print_str(sink, "  Rand\u{00ed}c index  R   =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_zg(sink, randic_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/\u{221a}(deg(u)\u{00d7}deg(v))]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Albertson irregularity
        set_color(sink, 8, 0);
        print_str(sink, "  irregularity   I   =  ");
        if irregularity == 0 {
            set_color(sink, 10, 0); // green — regular
            print_num_inline(sink, 0);
            set_color(sink, 8, 0);
            print_str(sink, "   [\u{03a3} |deg(u)\u{2212}deg(v)|]   regular graph");
        } else {
            set_color(sink, 12, 0); // bright-red — irregular
            print_num_inline(sink, irregularity as usize);
            set_color(sink, 8, 0);
            print_str(sink, "   [\u{03a3} |deg(u)\u{2212}deg(v)|]");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Gutman & Trinajsti\u{0107} 1972  Rand\u{00ed}c 1975");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.12: `graph topo` — SC, GA, AZI degree-based topological indices.
/// Zhou & Trinajstić 2009 (SC) / Vukičević & Furtula 2009 (GA) / Furtula et al. 2010 (AZI).
pub fn dispatch_graph_topo_indices(sink: &ConsoleSink) {
    let (sc_ppm, ga_ppm, azi_milli, edge_count, node_count) =
        gos_runtime::graph_topo_indices();

    set_color(sink, 14, 0); // bright-yellow — chemical index theme
    print_str(sink, " graph topo  (SC + GA + AZI degree-based indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    // Print a u64 ppm value as "X.XXX" (3 decimal places).
    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    // Print a u64 milli value as "X.XXX" (3 decimal places).
    fn print_milli3(sink: &ConsoleSink, milli: u64) {
        let whole = milli / 1_000;
        let frac  = (milli % 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // SC index
        set_color(sink, 8, 0);
        print_str(sink, "  sum-connectivity   SC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, sc_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/\u{221a}(deg(u)+deg(v))]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // GA index
        set_color(sink, 8, 0);
        print_str(sink, "  geom-arithmetic    GA  =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, ga_ppm);
        set_color(sink, 8, 0);
        if edge_count > 0 && ga_ppm == edge_count as u64 * 1_000_000 {
            print_str(sink, "   [\u{03a3} 2\u{221a}(d\u{22c5}d)/(d+d)]   (regular graph: GA=|E|)");
        } else {
            print_str(sink, "   [\u{03a3} 2\u{221a}(deg(u)\u{22c5}deg(v))/(deg(u)+deg(v))]");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // AZI index
        set_color(sink, 8, 0);
        print_str(sink, "  augmented Zagreb   AZI =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_milli3(sink, azi_milli);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (d\u{22c5}d/(d+d\u{2212}2))\u{00b3}]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Zhou 2009  Vuki\u{010d}evi\u{0107} 2009  Furtula 2010");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.13: `graph topo2` — H + ABC + F degree-based topological indices.
/// Analogous to `ethtool -S` showing per-interface chemical bond metrics.
///   H   = Zhong 2012 (harmonic index)
///   ABC = Estrada et al. 2008 (atom-bond connectivity)
///   F   = Furtula & Gutman 2015 (forgotten topological index)
pub fn dispatch_graph_topo_indices2(sink: &ConsoleSink) {
    let (h_ppm, abc_ppm, f_index, edge_count, node_count) =
        gos_runtime::graph_topo_indices2();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo2  (H + ABC + F degree-based indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // H index
        set_color(sink, 8, 0);
        print_str(sink, "  harmonic index     H   =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, h_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 2/(deg(u)+deg(v))]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // ABC index
        set_color(sink, 8, 0);
        print_str(sink, "  atom-bond conn     ABC =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, abc_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}((d+d\u{2212}2)/(d\u{22c5}d))]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // F index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  forgotten index    F   =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, f_index as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v deg(v)\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Zhong 2012  Estrada et al. 2008  Furtula & Gutman 2015");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.14: `graph topo3` — SDD + ISI + Nirmala degree-based topological indices.
///   SDD = Vasilyev 2014 (symmetric division degree)
///   ISI = Sedlar et al. 2011 (inverse sum indeg)
///   NI  = Rather et al. 2021 (Nirmala index)
pub fn dispatch_graph_topo_indices3(sink: &ConsoleSink) {
    let (sdd_ppm, isi_ppm, ni_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices3();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo3  (SDD + ISI + Nirmala indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // SDD index
        set_color(sink, 8, 0);
        print_str(sink, "  sym. div. deg      SDD =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, sdd_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (d\u{00b2}+d\u{00b2})/(d\u{22c5}d)]");
        if edge_count > 0 && sdd_ppm == (edge_count as u64) * 2_000_000 {
            set_color(sink, 10, 0); // bright-green
            print_str(sink, "  \u{2261}2|E| (regular)");
            set_color(sink, 8, 0);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // ISI index
        set_color(sink, 8, 0);
        print_str(sink, "  inv. sum indeg     ISI =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, isi_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} d\u{22c5}d/(d+d)]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NI index
        set_color(sink, 8, 0);
        print_str(sink, "  nirmala index      NI  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, ni_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}(d+d)]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Vasilyev 2014  Sedlar et al. 2011  Rather et al. 2021");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.15: `graph topo4` — Sombor + Reduced Second Zagreb + Sigma degree-based topological indices.
///   SO  = Gutman 2021 (Sombor index)
///   RM₂ = Furtula, Gutman & Ediz 2014 (reduced second Zagreb)
///   σ   = Gutman et al. 2014 (sigma index / total irregularity)
pub fn dispatch_graph_topo_indices4(sink: &ConsoleSink) {
    let (so_ppm, rm2, sigma, edge_count, node_count) =
        gos_runtime::graph_topo_indices4();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo4  (Sombor + RM\u{2082} + Sigma indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // SO index
        set_color(sink, 8, 0);
        print_str(sink, "  sombor index       SO  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, so_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}(d\u{00b2}+d\u{00b2})]");
        if edge_count > 0 && sigma == 0 {
            set_color(sink, 10, 0); // bright-green: regular annotation
            print_str(sink, "  (regular)");
            set_color(sink, 8, 0);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // RM₂ index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  reduced 2nd zagreb RM\u{2082}=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, rm2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "        [\u{03a3} (d-1)\u{22c5}(d-1)]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // σ index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  sigma index        \u{03c3}   =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, sigma as usize);
        if sigma == 0 && edge_count > 0 {
            set_color(sink, 10, 0); // bright-green: regular graph (σ=0)
            print_str(sink, " (regular: \u{03c3}=0)");
            set_color(sink, 8, 0);
        } else {
            set_color(sink, 8, 0);
            print_str(sink, "        [\u{03a3} (d-d)\u{00b2}]");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Gutman 2021  Furtula & Ediz 2014  Gutman et al. 2014");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.16: `graph topo5` — HM₁ + HM₂ + AG degree-based topological indices.
///   HM₁ = Shirdel et al. 2013 (Hyper-Zagreb index, exact integer)
///   HM₂ = Das & Trinajstić 2011 (Second Hyper-Zagreb index, exact integer)
///   AG  = Zheng et al. 2020 (Arithmetic-Geometric index, ppm via isqrt)
pub fn dispatch_graph_topo_indices5(sink: &ConsoleSink) {
    let (hm1, hm2, ag_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices5();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo5  (HM\u{2081} + HM\u{2082} + AG degree-based indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // HM₁ (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  hyper-zagreb 1st   HM\u{2081}=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, hm1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "        [\u{03a3} (d+d)\u{00b2}]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // HM₂ (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  hyper-zagreb 2nd   HM\u{2082}=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, hm2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "        [\u{03a3} (d\u{00b7}d)\u{00b2}]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // AG index (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  arith-geo index    AG  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, ag_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (d+d)/(2\u{221a}d\u{00b7}d)]");
        if edge_count > 0 && ag_ppm == edge_count as u64 * 1_000_000 {
            set_color(sink, 10, 0); // bright-green: regular annotation
            print_str(sink, "  (regular: AG=m)");
            set_color(sink, 8, 0);
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Shirdel et al. 2013  Das & Trinajsti\u{0107} 2011  Zheng et al. 2020");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.17: `graph topo6` — EM₁ + ABS + RRR degree-based topological indices.
///   EM₁ = Milićević et al. 2004 (Reformulated First Zagreb, exact integer)
///   ABS = Chen et al. 2022 (Atom-Bond Sum Connectivity, ppm via isqrt)
///   RRR = Li & Shi 2008 (Reduced Reciprocal Randić, ppm via isqrt)
pub fn dispatch_graph_topo_indices6(sink: &ConsoleSink) {
    let (em1, abs_ppm, rrr_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices6();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo6  (EM\u{2081} + ABS + RRR degree-based indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // EM₁ (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  ref. 1st zagreb  EM\u{2081}=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, em1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "        [\u{03a3} (d+d-2)\u{00b2}]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // ABS (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  atom-bond sum    ABS =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, abs_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}((d+d-2)/(d+d))]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // RRR (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  red. recip. rand. RRR=  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, rrr_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}((d-1)\u{22c5}(d-1))]");
        if edge_count > 0 && rrr_ppm == 0 {
            set_color(sink, 8, 0);
            print_str(sink, "  (all pendant)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Mili\u{0107}evi\u{0107} et al. 2004  Chen et al. 2022  Li & Shi 2008");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.18: `graph topo7` — Wiener W + Harary H + Hyper-Wiener WW distance-based topological indices.
///   W  = Wiener 1947        (sum of all pairwise shortest-path distances, exact integer)
///   H  = Plavšić et al. 1993  (sum of reciprocal distances ×10^6, floor ppm)
///   WW = Klein & Randić 1993  (hyper-Wiener = (1/2)Σ(d+d²), exact integer)
/// Disconnected pairs contribute 0. BFS on undirected projection, O(n·(n+m)).
pub fn dispatch_graph_topo_indices7(sink: &ConsoleSink) {
    let (wiener, harary_ppm, hyper_wiener, edge_count, node_count) =
        gos_runtime::graph_topo_indices7();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo7  (W + H + WW distance-based indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // W: Wiener index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  wiener            W  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, wiener as usize);
        set_color(sink, 8, 0);
        print_str(sink, "        [\u{03a3} d(u,v), u<v]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // H: Harary index (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  harary             H  =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, harary_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/d(u,v), u<v]");
        if node_count > 1 && wiener == 0 {
            // all pairs disconnected
            set_color(sink, 8, 0);
            print_str(sink, "  (disconnected)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // WW: Hyper-Wiener index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  hyper-wiener      WW  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, hyper_wiener as usize);
        set_color(sink, 8, 0);
        print_str(sink, "        [\u{bd}\u{03a3}(d+d\u{00b2}), u<v]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Wiener 1947  Plav\u{0161}i\u{0107} et al. 1993  Klein & Randi\u{0107} 1993");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.19: `graph topo8` — eccentricity-based topological indices.
///   ECI      = ξ(G) = Σ_v deg(v) × ecc(v)           (Sharma, Goswami & Madan 1997)
///   avg_ecc  = (Σ_v ecc(v)) / n                      (Buckley & Harary 1990)
///   D        = diameter = max eccentricity            (0 if all isolated)
///   R        = radius   = min positive eccentricity   (0 if no connected pairs)
/// ecc(v) = max reachable BFS distance from v; 0 for isolated nodes.
pub fn dispatch_graph_topo_indices8(sink: &ConsoleSink) {
    let (eci, avg_ecc_ppm, diameter, radius, edge_count, node_count) =
        gos_runtime::graph_topo_indices8();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo8  (ECI + D + R + avg-ecc eccentricity-based indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // ECI: eccentric connectivity index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  eccentric conn   \u{03be}  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, eci as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} deg(v)\u{22c5}ecc(v)]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Diameter (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  diameter          D  =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, diameter as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [max ecc(v)]");
        if diameter == 0 && node_count > 1 {
            print_str(sink, "  (all isolated)");
        } else if diameter > 0 && radius == diameter {
            print_str(sink, "  (self-centered)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Radius (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  radius            R  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, radius as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [min ecc(v)>0]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Average eccentricity (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  avg eccentricity     =  ");
        set_color(sink, 9, 0); // bright-blue
        print_ppm3_u64(sink, avg_ecc_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} ecc(v)/n]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Sharma et al. 1997  Buckley & Harary 1990");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.20: `graph topo9` — Schultz MTI + Gutman Index + Connective Eccentric Index.
///   W_S  = Schultz MTI   = Σ_{u<v} (deg(u)+deg(v))×d(u,v)   (exact; Schultz 1989)
///   W_G  = Gutman index  = Σ_{u<v} deg(u)×deg(v)×d(u,v)     (exact; Gutman 1994)
///   CξE  = connective eccentric = Σ_v deg(v)/ecc(v) × 10^6   (floor ppm; Gupta et al. 2000)
/// Disconnected pairs contribute 0. Isolated nodes contribute 0 to CξE.
/// BFS on undirected projection, O(n·(n+m)).
pub fn dispatch_graph_topo_indices9(sink: &ConsoleSink) {
    let (ws, wg, cxe_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices9();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo9  (W_S + W_G + CxiE degree-distance hybrid indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // W_S: Schultz MTI (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  schultz mti      W\u{209b} =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ws as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} (d\u{1d64}+d\u{1d65})\u{22c5}d(u,v), u<v]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // W_G: Gutman index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  gutman index     W\u{1d33} =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, wg as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} d\u{1d64}\u{22c5}d\u{1d65}\u{22c5}d(u,v), u<v]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // CξE: connective eccentric index (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  conn. eccentric C\u{03be}E =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, cxe_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} deg(v)/ecc(v)]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Schultz 1989  Gutman 1994  Gupta et al. 2000");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.21: `graph topo10` — Szeged Sz + Revised Szeged rSz + Mostar Mo.
///   Sz  = Σ_{uv∈E} n_u·n_v                             (exact; Gutman & Klavžar 1995)
///   rSz = Σ_{uv∈E} (n_u+n₀/2)·(n_v+n₀/2) × 10^6      (ppm; Pisanski & Randić 2010)
///   Mo  = Σ_{uv∈E} |n_u−n_v|                           (exact; Doslić et al. 2018)
/// n_u, n_v, n₀ = vertex partition counts for each edge's BFS bisection.
pub fn dispatch_graph_topo_indices10(sink: &ConsoleSink) {
    let (sz, rsz_ppm, mo, edge_count, node_count) =
        gos_runtime::graph_topo_indices10();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo10 (Sz + rSz + Mo edge-partition distance indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // Sz: Szeged index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  szeged index   Sz =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, sz as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} n\u{1d64}\u{22c5}n\u{1d65}, uv\u{2208}E]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // rSz: Revised Szeged (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  revised szeged rSz =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, rsz_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} (n\u{1d64}+n\u{2080}/2)\u{22c5}(n\u{1d65}+n\u{2080}/2)]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Mo: Mostar index (exact integer)
        set_color(sink, 8, 0);
        print_str(sink, "  mostar index   Mo =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, mo as usize);
        set_color(sink, 8, 0);
        if mo == 0 {
            print_str(sink, "  [\u{03a3} |n\u{1d64}\u{2212}n\u{1d65}|]  (Mo=0: vertex-transitive)");
        } else {
            print_str(sink, "  [\u{03a3} |n\u{1d64}\u{2212}n\u{1d65}|]");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Gutman & Klav\u{017e}ar 1995  Pisanski & Randi\u{107} 2010  Dosli\u{107} et al. 2018");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.22: `graph topo11` — Balaban J + Transmission Irregularity TI + Vertex PI.
///   J     = (m/μ) × Σ_{uv∈E} 1/√(T_u·T_v)  (ppm; Balaban 1982)
///   TI    = Σ_{uv∈E} |T_u − T_v|             (exact; Abdo & Dimitrov 2014)
///   PI_v  = Σ_{uv∈E} (T_u + T_v)             (exact; Khalifeh et al. 2008)
/// T_v = vertex transmittance = Σ_{w reachable} d(v,w).
pub fn dispatch_graph_topo_indices11(sink: &ConsoleSink) {
    let (j_ppm, ti, piv, edge_count, node_count) =
        gos_runtime::graph_topo_indices11();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo11 (J + TI + PI_v transmission indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // J: Balaban J connectivity index (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  balaban J      J =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, j_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "  [(m/\u{03bc}) \u{03a3} 1/\u{221a}(T\u{1d64}\u{22c5}T\u{1d65})]");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // TI: Transmission Irregularity (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  transmis. irr. TI =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, ti as usize);
        set_color(sink, 8, 0);
        if ti == 0 {
            print_str(sink, "  [\u{03a3} |T\u{1d64}\u{2212}T\u{1d65}|]  (TI=0: transmission-regular)");
        } else {
            print_str(sink, "  [\u{03a3} |T\u{1d64}\u{2212}T\u{1d65}|]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // PI_v: Vertex PI index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  vertex PI    PI_v =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, piv as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3} (T\u{1d64}+T\u{1d65})]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Balaban 1982  Abdo & Dimitrov 2014  Khalifeh et al. 2008");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.23: `graph topo12` — Zagreb eccentricity indices M1* + M2* + M3*.
///   M1*(G) = Σ_v ecc(v)²                  (exact; Vukičević & Graovac 2010)
///   M2*(G) = Σ_{uv∈E} ecc(u)×ecc(v)       (exact; Das et al. 2013)
///   M3*(G) = Σ_{uv∈E} |ecc(u)−ecc(v)|     (exact; Farooq & Ali 2021)
/// ecc(v) = max BFS distance from v (0 for isolated nodes).
pub fn dispatch_graph_topo_indices12(sink: &ConsoleSink) {
    let (m1e, m2e, m3e, edge_count, node_count) =
        gos_runtime::graph_topo_indices12();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo12 (M1* + M2* + M3* Zagreb eccentricity indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // M1*: First Zagreb eccentricity index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  first Zagreb ecc.  M1* =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, m1e as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_v ecc(v)\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // M2*: Second Zagreb eccentricity index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  second Zagreb ecc. M2* =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, m2e as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_{uv\u{2208}E} ecc(u)\u{00d7}ecc(v)]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // M3*: Third Zagreb eccentricity index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  third Zagreb ecc.  M3* =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, m3e as usize);
        set_color(sink, 8, 0);
        if m3e == 0 {
            print_str(sink, "  [\u{03a3} |ecc(u)\u{2212}ecc(v)|]  (M3*=0: self-centered)");
        } else {
            print_str(sink, "  [\u{03a3} |ecc(u)\u{2212}ecc(v)|]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Vukičević & Graovac 2010  Das et al. 2013  Farooq & Ali 2021");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.24: `graph topo13` — Transmission Zagreb TM₁ + TM₂ + GA_t indices.
///   TM₁(G)  = Σ_v T_v²                                (exact; Xing & Gutman 2012)
///   TM₂(G)  = Σ_{uv∈E} T_u·T_v                       (exact)
///   GA_t(G) = Σ_{uv∈E} 2√(T_u·T_v)/(T_u+T_v) × 10^6 (floor ppm; Alizadeh et al. 2013)
/// T_v = vertex transmission = Σ_{w reachable,w≠v} d(v,w).
pub fn dispatch_graph_topo_indices13(sink: &ConsoleSink) {
    let (tm1, tm2, ga_t, edge_count, node_count) =
        gos_runtime::graph_topo_indices13();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo13 (TM1 + TM2 + GA_t transmission Zagreb indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // TM₁: first transmission Zagreb index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  first trans. Zagreb  TM1 =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, tm1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_v T_v\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // TM₂: second transmission Zagreb index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  second trans. Zagreb TM2 =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, tm2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_{uv\u{2208}E} T_u\u{00d7}T_v]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // GA_t: geometric-arithmetic transmission index (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  geom-arith. trans.  GA_t =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, ga_t);
        set_color(sink, 8, 0);
        if ga_t == (edge_count as u64) * 1_000_000 && edge_count > 0 {
            print_str(sink, "  [\u{03a3} 2\u{221a}(T_u\u{00b7}T_v)/(T_u+T_v)]  (trans-regular)");
        } else {
            print_str(sink, "  [\u{03a3} 2\u{221a}(T_u\u{00b7}T_v)/(T_u+T_v)]  (ppm)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Xing & Gutman 2012  Alizadeh et al. 2013");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.25: `graph topo14` — TE + EDS + GEA eccentricity-based indices.
///   TE(G)  = Σ_v ecc(v)                                    (exact; Dankelmann et al. 2004)
///   EDS(G) = Σ_v ecc(v)·T_v                                (exact; Gupta et al. 2008)
///   GEA(G) = Σ_{uv∈E} 2√(ecc(u)·ecc(v))/(ecc(u)+ecc(v)) × 10^6 (floor ppm)
/// GEA = |E|×10^6 iff graph is self-centered (all eccentricities equal).
pub fn dispatch_graph_topo_indices14(sink: &ConsoleSink) {
    let (te, eds, gea, edge_count, node_count) =
        gos_runtime::graph_topo_indices14();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo14 (TE + EDS + GEA eccentricity indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // TE: total eccentricity (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  total eccentricity    TE  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, te as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_v ecc(v)]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // EDS: eccentric distance sum (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  eccentric dist. sum   EDS =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, eds as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_v ecc(v)\u{00b7}T_v]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // GEA: geometric-arithmetic eccentricity index (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  geom-arith. ecc.     GEA =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, gea);
        set_color(sink, 8, 0);
        if gea == (edge_count as u64) * 1_000_000 && edge_count > 0 {
            print_str(sink, "  [\u{03a3} 2\u{221a}(ea\u{00b7}eb)/(ea+eb)]  (self-centered)");
        } else {
            print_str(sink, "  [\u{03a3} 2\u{221a}(ea\u{00b7}eb)/(ea+eb)]  (ppm)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Dankelmann et al. 2004  Gupta et al. 2008");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.26: `graph topo15` — LM₁ + LM₂ + LM₃ Leap Zagreb indices.
///   LM₁(G) = Σ_v d₂(v)²                          (exact; Naji et al. 2017)
///   LM₂(G) = Σ_{uv∈E} d₂(u)·d₂(v)               (exact)
///   LM₃(G) = Σ_{uv∈E} (d₂(u)+d₂(v))             (exact)
/// d₂(v) = |{w : d(v,w) = 2}| = 2-distance degree.
/// LM₁=LM₂=LM₃=0 for complete graphs (d₂=0 everywhere).
pub fn dispatch_graph_topo_indices15(sink: &ConsoleSink) {
    let (lm1, lm2, lm3, edge_count, node_count) =
        gos_runtime::graph_topo_indices15();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo15 (LM\u{2081} + LM\u{2082} + LM\u{2083} Leap Zagreb indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // LM1: first Leap Zagreb index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  first leap zagreb    LM\u{2081} =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, lm1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_v d\u{2082}(v)\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // LM2: second Leap Zagreb index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  second leap zagreb   LM\u{2082} =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, lm2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_{uv\u{2208}E} d\u{2082}(u)\u{00b7}d\u{2082}(v)]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // LM3: third Leap Zagreb index (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  third leap zagreb    LM\u{2083} =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, lm3 as usize);
        set_color(sink, 8, 0);
        if lm1 == 0 && edge_count > 0 {
            print_str(sink, "  [\u{03a3}_{uv\u{2208}E} d\u{2082}(u)+d\u{2082}(v)]  (d\u{2082}=0: complete graph)");
        } else {
            print_str(sink, "  [\u{03a3}_{uv\u{2208}E} d\u{2082}(u)+d\u{2082}(v)]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Naji, Soner & Gutman 2017");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.27: `graph topo16` — R_{1/2} + R_{-1} + Lanzhou Lz generalized Randić family.
///   ir_ppm = R_{1/2}(G)×10^6 = Σ_{uv∈E} √(d_u·d_v)×10^6  (Product Connectivity; Bollobás & Erdős 1998)
///   rr_ppm = R_{-1}(G)×10^6  = Σ_{uv∈E} 10^6/(d_u·d_v)   (Reciprocal Randić; Bollobás & Erdős 1998)
///   lz     = Lz(G) = Σ_v d_v²·(n−1−d_v)                   (Lanzhou Index; Xia et al. 2019)
/// IR = m·Δ·10^6 iff Δ-regular; RR = m·10^6/Δ² iff Δ-regular; Lz=0 for complete graphs.
pub fn dispatch_graph_topo_indices16(sink: &ConsoleSink) {
    let (ir_ppm, rr_ppm, lz, edge_count, node_count) =
        gos_runtime::graph_topo_indices16();

    // Print a u64 ppm value as "X.XXX" (3 decimal places).
    fn print_ppm3(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10   { print_str(sink, "00"); }
        else if frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo16 (R\u{2081}\u{2082} + R\u{208b}\u{2081} + Lz generalized Rand\u{00ed}c family)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // IR = R_{1/2} (Product Connectivity)
        set_color(sink, 8, 0);
        print_str(sink, "  product connect.  R\u{2081}\u{2082} =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3(sink, ir_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}\u{221a}(d\u{22c5}d)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // RR = R_{-1} (Reciprocal Randic)
        set_color(sink, 8, 0);
        print_str(sink, "  reciprocal rand.  R\u{208b}\u{2081} =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3(sink, rr_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/(d\u{22c5}d)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Lz = Lanzhou Index
        set_color(sink, 8, 0);
        print_str(sink, "  lanzhou index       Lz =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, lz as usize);
        set_color(sink, 8, 0);
        if lz == 0 && edge_count > 0 {
            print_str(sink, "  [\u{03a3}_v d\u{00b2}(n\u{2212}1\u{2212}d)]  (Lz=0: complete graph)");
        } else {
            print_str(sink, "  [\u{03a3}_v d\u{00b2}(n\u{2212}1\u{2212}d)]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Bollob\u{00e1}s & Erd\u{0151}s 1998  Xia et al. 2019");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.28: `graph topo17` — Zagreb coindices M̄₁ + M̄₂ + forgotten coindex F̄.
///   M̄₁(G) = Σ_{uv∉E} (d_u+d_v)   = 2m(n−1) − M₁  (Ashrafi, Došlić & Hamzeh 2010)
///   M̄₂(G) = Σ_{uv∉E} d_u·d_v     = 2m² − M₁/2 − M₂ (Ashrafi, Došlić & Hamzeh 2010)
///   F̄(G)  = Σ_{uv∉E} (d_u²+d_v²) = (n−1)·M₁ − F   (forgotten coindex; De 2016)
/// All three are zero for complete graphs (no non-edges exist).
pub fn dispatch_graph_topo_indices17(sink: &ConsoleSink) {
    let (mbar1, mbar2, fbar, edge_count, node_count) =
        gos_runtime::graph_topo_indices17();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo17 (M\u{0305}\u{2081} + M\u{0305}\u{2082} + F\u{0305} Zagreb coindices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        let complete = mbar1 == 0 && edge_count > 0;

        // M̄₁: first Zagreb coindex (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  1st zagreb coindex  M\u{0305}\u{2081} =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, mbar1 as usize);
        set_color(sink, 8, 0);
        if complete {
            print_str(sink, "  [\u{03a3}_{uv\u{2209}E}(d+d)]  (=0: complete graph)");
        } else {
            print_str(sink, "  [\u{03a3}_{uv\u{2209}E}(d_u+d_v)]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // M̄₂: second Zagreb coindex (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  2nd zagreb coindex  M\u{0305}\u{2082} =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, mbar2 as usize);
        set_color(sink, 8, 0);
        if complete {
            print_str(sink, "  [\u{03a3}_{uv\u{2209}E} d_u\u{00b7}d_v]  (=0: complete graph)");
        } else {
            print_str(sink, "  [\u{03a3}_{uv\u{2209}E} d_u\u{00b7}d_v]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // F̄: forgotten coindex (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  forgotten coindex    F\u{0305}  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, fbar as usize);
        set_color(sink, 8, 0);
        if complete {
            print_str(sink, "  [\u{03a3}_{uv\u{2209}E}(d\u{00b2}+d\u{00b2})]  (=0: complete graph)");
        } else {
            print_str(sink, "  [\u{03a3}_{uv\u{2209}E}(d_u\u{00b2}+d_v\u{00b2})]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Ashrafi, Do\u{0161}li\u{0107} & Hamzeh 2010  De 2016");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.29: `graph topo18` — NM₁ + NM₂ + GA₂ Neighborhood Zagreb indices.
///   S(v) = Σ_{u∈N(v)} deg(u): neighbor-degree sum ("2nd-order degree").
///   NM₁(G) = Σ_v S(v)²
///   NM₂(G) = Σ_{uv∈E} S(u)·S(v)
///   GA₂(G) = Σ_{uv∈E} 2√(S(u)·S(v))/(S(u)+S(v))   (ppm; S-uniform → =|E|×10^6)
pub fn dispatch_graph_topo_indices18(sink: &ConsoleSink) {
    let (nm1, nm2, ga2_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices18();

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo18 (NM\u{2081} + NM\u{2082} + GA\u{2082} Neighborhood Zagreb)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        let uniform = ga2_ppm == (edge_count as u64) * 1_000_000 && edge_count > 0;

        // NM₁: first Neighborhood Zagreb (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  1st neighborhood    NM\u{2081} =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nm1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_v S(v)\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NM₂: second Neighborhood Zagreb (exact)
        set_color(sink, 8, 0);
        print_str(sink, "  2nd neighborhood    NM\u{2082} =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nm2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "  [\u{03a3}_{uv\u{2208}E} S(u)\u{00b7}S(v)]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // GA₂: Neighborhood Geometric-Arithmetic (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  neighborhood GA     GA\u{2082} =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, ga2_ppm);
        set_color(sink, 8, 0);
        if uniform {
            print_str(sink, "   [\u{03a3} 2\u{221a}(S\u{00b7}S)/(S+S)]  (=|E|\u{00d7}1: S-uniform)");
        } else {
            print_str(sink, "   [\u{03a3} 2\u{221a}(S_u\u{00b7}S_v)/(S_u+S_v)]  (ppm)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Mondal et al. 2019");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.30: `graph topo19` — Reverse Wiener Λ + Reciprocal Complementary Wiener RCW + Terminal Wiener TW.
///   Λ(G)   = Σ_c [C(n_c,2) × D_c − W_c]                   (exact; Randić et al. 2000)
///   RCW(G) = Σ_{u<v,conn} 1/(D_c+1−d(u,v))                (ppm; Vukičević 2010)
///   TW(G)  = Σ_{u<v, both pendant (deg=1)} d(u,v)          (exact; Gutman et al. 2004)
pub fn dispatch_graph_topo_indices19(sink: &ConsoleSink) {
    let (rw, rcw_ppm, tw, edge_count, node_count) =
        gos_runtime::graph_topo_indices19();

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo19 (\u{039b} + RCW + TW Wiener variants)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // Reverse Wiener Λ
        set_color(sink, 8, 0);
        print_str(sink, "  reverse wiener    \u{039b}   =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, rw as usize);
        set_color(sink, 8, 0);
        if rw == 0 {
            print_str(sink, "  [\u{03a3}_c C(n_c,2)\u{00d7}D_c−W_c]  (\u{039b}=0: complete blocks)");
        } else {
            print_str(sink, "  [\u{03a3}_c C(n_c,2)\u{00d7}D_c−W_c]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Reciprocal Complementary Wiener RCW (ppm)
        set_color(sink, 8, 0);
        print_str(sink, "  reciprocal compl   RCW =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, rcw_ppm);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/(D_c+1−d)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Terminal Wiener TW
        set_color(sink, 8, 0);
        print_str(sink, "  terminal wiener    TW  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, tw as usize);
        set_color(sink, 8, 0);
        if tw == 0 {
            print_str(sink, "  [\u{03a3}_{pend pairs} d]  (TW=0: no pendant pairs)");
        } else {
            print_str(sink, "  [\u{03a3}_{u,v pendant} d(u,v)]  (exact)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Randi\u{0107} et al. 2000  Vuki\u{010d}evi\u{0107} 2010  Gutman et al. 2004");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices20(sink: &ConsoleSink) {
    let (so_star, rso, rso_red, edge_count, node_count) =
        gos_runtime::graph_topo_indices20();

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo20 (SO* + RSO + rSO Sombor variants)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // Modified Sombor SO*
        set_color(sink, 8, 0);
        print_str(sink, "  modified sombor  SO* =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, so_star);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} d_u\u{00b7}d_v/\u{221a}(d_u\u{00b2}+d_v\u{00b2})]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Reciprocal Sombor RSO
        set_color(sink, 8, 0);
        print_str(sink, "  reciprocal somb  RSO =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, rso);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/\u{221a}(d_u\u{00b2}+d_v\u{00b2})]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // Reduced Sombor rSO
        set_color(sink, 8, 0);
        print_str(sink, "  reduced sombor  rSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, rso_red);
        set_color(sink, 8, 0);
        if rso_red == 0 && edge_count > 0 {
            print_str(sink, "   [\u{03a3} \u{221a}((d_u-1)\u{00b2}+(d_v-1)\u{00b2})]  (rSO=0: all pendant-pendant)");
        } else {
            print_str(sink, "   [\u{03a3} \u{221a}((d_u-1)\u{00b2}+(d_v-1)\u{00b2})]  (ppm)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Ghanbari & Rajabi-Parsa 2021  Gutman 2022  Doslic et al. 2022");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.32: `graph topo21` — ABC₄ + Neighborhood Harmonic NH + Neighborhood Sombor NSO.
/// Three 4th-generation S-variant topological indices using S(v)=Σ_{w∈N(v)} d(w).
pub fn dispatch_graph_topo_indices21(sink: &ConsoleSink) {
    let (abc4, nh, nso, edge_count, node_count) =
        gos_runtime::graph_topo_indices21();

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo21 (ABC\u{2084} + NH + NSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // ABC₄
        set_color(sink, 8, 0);
        print_str(sink, "  4th ABC index   ABC\u{2084} =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, abc4);
        set_color(sink, 8, 0);
        if abc4 == 0 && edge_count > 0 {
            print_str(sink, "   [\u{03a3} \u{221a}((S_u+S_v\u{22122})/(S_u\u{00b7}S_v))]  (=0: S_u+S_v=2 for all edges)");
        } else {
            print_str(sink, "   [\u{03a3} \u{221a}((S_u+S_v\u{22122})/(S_u\u{00b7}S_v))]  (ppm)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NH (Neighborhood Harmonic)
        set_color(sink, 8, 0);
        print_str(sink, "  neighbor harm    NH =  ");
        set_color(sink, 10, 0); // bright-green
        print_ppm3_u64(sink, nh);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 2/(S_u+S_v)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NSO (Neighborhood Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  neighbor sombor  NSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, nso);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}(S_u\u{00b2}+S_v\u{00b2})]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Ghorbani & Hosseinzadeh 2010  (NH/NSO: S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices22(sink: &ConsoleSink) {
    let (nr, nf, nsc, edge_count, node_count) =
        gos_runtime::graph_topo_indices22();

    fn print_ppm3_u64(sink: &ConsoleSink, ppm: u64) {
        let whole = ppm / 1_000_000;
        let frac  = ((ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, whole as usize);
        print_str(sink, ".");
        if frac < 10        { print_str(sink, "00"); }
        else if frac < 100  { print_str(sink, "0"); }
        print_num_inline(sink, frac);
    }

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo22 (NR + NF + NSC S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NR (Neighborhood Randić)
        set_color(sink, 8, 0);
        print_str(sink, "  neighbor randic   NR =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_ppm3_u64(sink, nr);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/\u{221a}(S_u\u{00b7}S_v)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NF (Neighborhood Forgotten)
        set_color(sink, 8, 0);
        print_str(sink, "  neighbor forgot   NF =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nf as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NSC (Neighborhood Sum Connectivity)
        set_color(sink, 8, 0);
        print_str(sink, "  neighbor sumconn  NSC =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_ppm3_u64(sink, nsc);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/\u{221a}(S_u+S_v)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Randic 1975 Furtula & Gutman 2015 Zhou & Trinajstic 2009  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.34: `graph topo23` — NHM1 + NSDD + NM3 (S-variant HM₁, SDD, M₃).
pub fn dispatch_graph_topo_indices23(sink: &ConsoleSink) {
    let (nhm1, nsdd_ppm, nm3, edge_count, node_count) =
        gos_runtime::graph_topo_indices23();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo23 (NHM1 + NSDD + NM3 S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHM1 (Neighborhood Hyper-Zagreb M₁)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hyper zagreb M\u{2081}  NHM1 =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhm1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u+S_v)\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NSDD (Neighborhood Symmetric Division Deg)
        let two_edge_ppm = 2u64 * edge_count as u64 * 1_000_000u64;
        set_color(sink, 8, 0);
        print_str(sink, "  S-sym-div-deg    NSDD =  ");
        set_color(sink, 10, 0); // bright-green
        let nsdd_whole = nsdd_ppm / 1_000_000;
        let nsdd_frac  = ((nsdd_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nsdd_whole as usize);
        print_str(sink, ".");
        if nsdd_frac < 10       { print_str(sink, "00"); }
        else if nsdd_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nsdd_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S\u{00b2}_u+S\u{00b2}_v)/(S_u\u{00b7}S_v)]  (ppm)");
        if edge_count > 0 && nsdd_ppm == two_edge_ppm {
            set_color(sink, 11, 0);
            print_str(sink, "  \u{2261}2|E| (S-regular)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NM3 (Neighborhood M₃ irregularity)
        set_color(sink, 8, 0);
        print_str(sink, "  S-irregularity   NM3  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nm3 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} |S_u\u{2212}S_v|]  (exact)");
        if edge_count > 0 && nm3 == 0 {
            set_color(sink, 11, 0);
            print_str(sink, "  NM3=0: S-regular");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Shirdel et al. 2013  Vasilyev 2014  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.35: `graph topo24` — NISI + NAZI + NEM1 (S-variant ISI, AZI, EM₁).
pub fn dispatch_graph_topo_indices24(sink: &ConsoleSink) {
    let (nisi_ppm, nazi_milli, nem1, edge_count, node_count) =
        gos_runtime::graph_topo_indices24();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo24 (NISI + NAZI + NEM1 S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NISI (Neighborhood Inverse Sum Indegree)
        set_color(sink, 8, 0);
        print_str(sink, "  S-inv-sum-indeg   NISI =  ");
        set_color(sink, 11, 0); // bright-cyan
        let nisi_whole = nisi_ppm / 1_000_000;
        let nisi_frac  = ((nisi_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nisi_whole as usize);
        print_str(sink, ".");
        if nisi_frac < 10       { print_str(sink, "00"); }
        else if nisi_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nisi_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} S_u\u{00b7}S_v/(S_u+S_v)]  (ppm)");
        if edge_count > 0 {
            let s_half_ppm = nisi_ppm / edge_count as u64;
            let _ = s_half_ppm; // annotation omitted for variable-S case
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAZI (Neighborhood Augmented Zagreb Index)
        set_color(sink, 8, 0);
        print_str(sink, "  S-augmented-azi   NAZI =  ");
        set_color(sink, 10, 0); // bright-green
        let nazi_whole = nazi_milli / 1_000;
        let nazi_frac  = (nazi_milli % 1_000) as usize;
        print_num_inline(sink, nazi_whole as usize);
        print_str(sink, ".");
        if nazi_frac < 10       { print_str(sink, "00"); }
        else if nazi_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nazi_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u\u{00b7}S_v/(S_u+S_v-2))\u{00b3}]  (milli)");
        if edge_count > 0 && nazi_milli == 0 {
            set_color(sink, 11, 0);
            print_str(sink, "  NAZI=0: all pendant-pair (S=1)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NEM1 (Neighborhood Reformulated First Zagreb)
        set_color(sink, 8, 0);
        print_str(sink, "  S-reform-zagreb   NEM1 =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nem1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u+S_v-2)\u{00b2}]  (exact)");
        if edge_count > 0 && nem1 == 0 {
            set_color(sink, 11, 0);
            print_str(sink, "  NEM1=0: all S\u{2081}\u{208a}S\u{2082}=2");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Sedlar et al. 2011  Furtula et al. 2010  Milicevic et al. 2004  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.36: `graph topo25` — NHM₂ + NAG + NABS (S-variant HM₂, AG ratio, ABS).
pub fn dispatch_graph_topo_indices25(sink: &ConsoleSink) {
    let (nhm2, nag_ppm, nabs_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices25();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo25 (NHM\u{2082} + NAG + NABS S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHM₂ (Neighborhood Hyper-Second Zagreb)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hyper-M2        NHM\u{2082} =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhm2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u\u{00b7}S_v)\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAG (Neighborhood Arithmetic-Geometric ratio)
        set_color(sink, 8, 0);
        print_str(sink, "  S-arith-geom      NAG  =  ");
        set_color(sink, 10, 0); // bright-green
        let nag_whole = nag_ppm / 1_000_000;
        let nag_frac  = ((nag_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nag_whole as usize);
        print_str(sink, ".");
        if nag_frac < 10       { print_str(sink, "00"); }
        else if nag_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nag_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u+S_v)/(2\u{221a}(S_u\u{00b7}S_v))]  (ppm)");
        if edge_count > 0 && nag_ppm == (edge_count as u64) * 1_000_000 {
            set_color(sink, 11, 0);
            print_str(sink, "  \u{2261}|E| (S-regular)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NABS (Neighborhood Atom-Bond Sum connectivity)
        set_color(sink, 8, 0);
        print_str(sink, "  S-atom-bond-sum   NABS =  ");
        set_color(sink, 13, 0); // bright-magenta
        let nabs_whole = nabs_ppm / 1_000_000;
        let nabs_frac  = ((nabs_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nabs_whole as usize);
        print_str(sink, ".");
        if nabs_frac < 10       { print_str(sink, "00"); }
        else if nabs_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nabs_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}((S_u+S_v-2)/(S_u+S_v))]  (ppm)");
        if edge_count > 0 && nabs_ppm == 0 {
            set_color(sink, 11, 0);
            print_str(sink, "  NABS=0: all S\u{2081}+S\u{2082}=2");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Das & Trinajsti\u{107} 2011  Zheng et al. 2020  Chen et al. 2022  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.37: `graph topo26` — NPC + NRM₂ + NRSO (S-variant Product-Connectivity, Reduced-Zagreb-2, Reciprocal-Sombor).
pub fn dispatch_graph_topo_indices26(sink: &ConsoleSink) {
    let (npc_ppm, nrm2, nrso_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices26();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo26 (NPC + NRM\u{2082} + NRSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NPC (Neighborhood Product Connectivity)
        set_color(sink, 8, 0);
        print_str(sink, "  S-prod-conn       NPC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        let npc_whole = npc_ppm / 1_000_000;
        let npc_frac  = ((npc_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, npc_whole as usize);
        print_str(sink, ".");
        if npc_frac < 10       { print_str(sink, "00"); }
        else if npc_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, npc_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}(S_u\u{00b7}S_v)]  (ppm)");
        if edge_count > 0 && npc_ppm == (edge_count as u64) * 1_000_000 {
            set_color(sink, 11, 0);
            print_str(sink, "  \u{2261}|E| (S=1 uniform)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NRM₂ (Neighborhood Reduced Second Zagreb)
        set_color(sink, 8, 0);
        print_str(sink, "  S-reduced-M2      NRM\u{2082} =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nrm2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u-1)(S_v-1)]  (exact)");
        if edge_count > 0 && nrm2 == 0 {
            set_color(sink, 11, 0);
            print_str(sink, "  NRM\u{2082}=0: all S=1 edges");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NRSO (Neighborhood Reciprocal Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-recip-sombor    NRSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        let nrso_whole = nrso_ppm / 1_000_000;
        let nrso_frac  = ((nrso_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nrso_whole as usize);
        print_str(sink, ".");
        if nrso_frac < 10       { print_str(sink, "00"); }
        else if nrso_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nrso_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/\u{221a}(S_u\u{00b2}+S_v\u{00b2})]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Bollob\u{00e1}s & Erd\u{0151}s 1998  Furtula, Gutman & Ediz 2014  Gutman 2022  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V3.38: `graph topo27` — NRR + NSO* + NrSO (S-variant Reciprocal-Randić, Modified-Sombor, Reduced-Sombor).
pub fn dispatch_graph_topo_indices27(sink: &ConsoleSink) {
    let (nrr_ppm, nsos_ppm, nrso_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices27();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo27 (NRR + NSO* + NrSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NRR (Neighborhood Reciprocal Randić)
        set_color(sink, 8, 0);
        print_str(sink, "  S-recip-randic    NRR  =  ");
        set_color(sink, 11, 0); // bright-cyan
        let nrr_whole = nrr_ppm / 1_000_000;
        let nrr_frac  = ((nrr_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nrr_whole as usize);
        print_str(sink, ".");
        if nrr_frac < 10       { print_str(sink, "00"); }
        else if nrr_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nrr_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} 1/(S_u\u{00b7}S_v)]  (ppm)");
        if edge_count > 0 && nrr_ppm == (edge_count as u64) * 1_000_000 {
            set_color(sink, 11, 0);
            print_str(sink, "  \u{2261}|E| (S=1 uniform)");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NSO* (Neighborhood Modified Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-mod-sombor      NSO* =  ");
        set_color(sink, 10, 0); // bright-green
        let nsos_whole = nsos_ppm / 1_000_000;
        let nsos_frac  = ((nsos_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nsos_whole as usize);
        print_str(sink, ".");
        if nsos_frac < 10       { print_str(sink, "00"); }
        else if nsos_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nsos_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} S_u\u{00b7}S_v/\u{221a}(S_u\u{00b2}+S_v\u{00b2})]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NrSO (Neighborhood Reduced Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-red-sombor      NrSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        let nrso_whole = nrso_ppm / 1_000_000;
        let nrso_frac  = ((nrso_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nrso_whole as usize);
        print_str(sink, ".");
        if nrso_frac < 10       { print_str(sink, "00"); }
        else if nrso_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nrso_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}((S_u-1)\u{00b2}+(S_v-1)\u{00b2})]  (ppm)");
        if edge_count > 0 && nrso_ppm == 0 {
            set_color(sink, 13, 0);
            print_str(sink, "  NrSO=0: all S=1 edges");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Bollob\u{00e1}s & Erd\u{0151}s 1998  Ghanbari & Rajabi-Parsa 2021  Doslic et al. 2022  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices28(sink: &ConsoleSink) {
    let (nni_ppm, nnmi_ppm, nsm1, edge_count, node_count) =
        gos_runtime::graph_topo_indices28();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo28 (NNI + NNMI + NSM1 Neighborhood Nirmala S-variant)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NNI (Neighborhood Nirmala Index)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nirmala         NNI  =  ");
        set_color(sink, 11, 0); // bright-cyan
        let nni_whole = nni_ppm / 1_000_000;
        let nni_frac  = ((nni_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nni_whole as usize);
        print_str(sink, ".");
        if nni_frac < 10       { print_str(sink, "00"); }
        else if nni_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nni_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} \u{221a}(S_u+S_v)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NNMI (Neighborhood Modified Nirmala Index)
        set_color(sink, 8, 0);
        print_str(sink, "  S-mod-nirmala     NNMI =  ");
        set_color(sink, 10, 0); // bright-green
        let nnmi_whole = nnmi_ppm / 1_000_000;
        let nnmi_frac  = ((nnmi_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nnmi_whole as usize);
        print_str(sink, ".");
        if nnmi_frac < 10       { print_str(sink, "00"); }
        else if nnmi_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nnmi_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u+S_v)^{3/2}]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NSM1 (Neighborhood S-edge M1)
        set_color(sink, 8, 0);
        print_str(sink, "  S-edge-m1         NSM1 =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nsm1 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u+S_v)]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Nirmala, Mathad & Usha 2021  Kumar et al. 2022  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices29(sink: &ConsoleSink) {
    let (nz0_ppm, nem2, nse_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices29();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo29 (NZ0 + NEM2 + NSe S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NZ₀ (S-variant zero-order Randić)
        set_color(sink, 8, 0);
        print_str(sink, "  S-zero-randic     NZ0  =  ");
        set_color(sink, 11, 0); // bright-cyan
        let nz0_whole = nz0_ppm / 1_000_000;
        let nz0_frac  = ((nz0_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nz0_whole as usize);
        print_str(sink, ".");
        if nz0_frac < 10       { print_str(sink, "00"); }
        else if nz0_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nz0_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v 1/\u{221a}S(v)]  (ppm)");
        if node_count > 0 && nz0_ppm == 0 {
            set_color(sink, 11, 0);
            print_str(sink, "  NZ0=0: all isolated");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NEM₂ (S-Reformulated Second Zagreb)
        set_color(sink, 8, 0);
        print_str(sink, "  S-reform-zagreb2  NEM2 =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nem2 as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} S_u\u{00b7}S_v\u{00b7}(S_u+S_v\u{2212}2)]  (exact)");
        if edge_count > 0 && nem2 == 0 {
            set_color(sink, 10, 0);
            print_str(sink, "  NEM2=0: all S=1 edges");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NSe (S-sqrt vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-sqrt-vertex     NSe  =  ");
        set_color(sink, 13, 0); // bright-magenta
        let nse_whole = nse_ppm / 1_000_000;
        let nse_frac  = ((nse_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nse_whole as usize);
        print_str(sink, ".");
        if nse_frac < 10       { print_str(sink, "00"); }
        else if nse_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nse_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v \u{221a}S(v)]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  Randi\u{0107} 1975  Mili\u{010d}evi\u{0107} et al. 2004  (S-variant family)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices30(sink: &ConsoleSink) {
    let (nvq, nrgs_ppm, nhcs, edge_count, node_count) =
        gos_runtime::graph_topo_indices30();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo30 (NVQ + NRGS + NHCS S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NVQ (S-Quartic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-quartic-vertex  NVQ  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nvq as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NRGS (S-Generalized Randić 3/2)
        set_color(sink, 8, 0);
        print_str(sink, "  S-randic-3/2      NRGS =  ");
        set_color(sink, 10, 0); // bright-green
        let nrgs_whole = nrgs_ppm / 1_000_000;
        let nrgs_frac  = ((nrgs_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nrgs_whole as usize);
        print_str(sink, ".");
        if nrgs_frac < 10       { print_str(sink, "00"); }
        else if nrgs_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nrgs_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u\u{00b7}S_v)^{3/2}]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHCS (S-Cubic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-cubic-edge-sum  NHCS =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nhcs as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u+S_v)\u{00b3}]  (exact)");
        if edge_count > 0 && nhcs == 0 {
            set_color(sink, 13, 0);
            print_str(sink, "  NHCS=0: no edges");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NVQ extends NM\u{2081}/NF to 4th power; NRGS=\u{03c7}_{3/2} S-variant; NHCS extends NHM\u{2081} to 3rd power)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices31(sink: &ConsoleSink) {
    let (nsig, nhqs, nps, edge_count, node_count) =
        gos_runtime::graph_topo_indices31();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo31 (NSig + NHQS + NPS S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NSig (S-Sigma irregularity)
        set_color(sink, 8, 0);
        print_str(sink, "  S-sigma-irr       NSig =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nsig as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{2212}S_v)\u{00b2}]  (exact)");
        if edge_count > 0 && nsig == 0 {
            set_color(sink, 11, 0);
            print_str(sink, "  NSig=0: S-regular");
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHQS (S-Quartic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-quartic-edge    NHQS =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhqs as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NPS (S-Penta vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-penta-vertex    NPS  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nps as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NSig=S-Sigma irregularity; NHQS extends NHCS to 4th power; NPS extends NVQ to 5th power)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices32(sink: &ConsoleSink) {
    let (nsh, nhps, nwso_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices32();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo32 (NSH + NHPS + NWSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NSH (S-Hextic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hextic-vertex   NSH  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nsh as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHPS (S-Quintic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-quintic-edge    NHPS =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhps as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NWSO (S-Weighted Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-weighted-sombor NWSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        let nwso_whole = nwso_ppm / 1_000_000;
        let nwso_frac  = ((nwso_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, nwso_whole as usize);
        print_str(sink, ".");
        if nwso_frac < 10       { print_str(sink, "00"); }
        else if nwso_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, nwso_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} S_u\u{00b7}S_v\u{00b7}\u{221a}(S_u\u{00b2}+S_v\u{00b2})]  (ppm)");
        if nwso_ppm == (edge_count as u64) * 1_414_213 || nwso_ppm == (edge_count as u64) * 1_414_214 {
            // approximate S=1 uniform check — skip annotation; let value speak for itself
        }
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NSH extends NPS to 6th power; NHPS extends NHQS to 5th power; NWSO=S\u{00b2}\u{00b7}NSO per edge)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices33(sink: &ConsoleSink) {
    let (nshp, nhse, ncso_ppm, edge_count, node_count) =
        gos_runtime::graph_topo_indices33();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo33 (NSHP + NHSE + NCSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NSHP (S-Heptic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptic-vertex   NSHP =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nshp as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHSE (S-Sextic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-sextic-edge     NHSE =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhse as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NCSO (S-Cubic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-cubic-sombor    NCSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        let ncso_whole = ncso_ppm / 1_000_000;
        let ncso_frac  = ((ncso_ppm % 1_000_000) / 1_000) as usize;
        print_num_inline(sink, ncso_whole as usize);
        print_str(sink, ".");
        if ncso_frac < 10       { print_str(sink, "00"); }
        else if ncso_frac < 100 { print_str(sink, "0"); }
        print_num_inline(sink, ncso_frac);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{0302}\u{00b2}]  (ppm)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NSHP extends NSH to 7th power; NHSE extends NHPS to 6th power; NCSO=SO\u{00b3} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices34(sink: &ConsoleSink) {
    let (noc, nhhs, nfso, edge_count, node_count) =
        gos_runtime::graph_topo_indices34();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo34 (NOC + NHHS + NFSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOC (S-Octic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octic-vertex    NOC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHS (S-Septic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-septic-edge     NHHS =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhs as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NFSO (S-Fourth Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-fourth-sombor   NFSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nfso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOC extends NSHP to 8th power; NHHS extends NHSE to 7th; NFSO=SO\u{2074} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices35(sink: &ConsoleSink) {
    let (nnc, nhoc, nhso, edge_count, node_count) =
        gos_runtime::graph_topo_indices35();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo35 (NNC + NHOC + NHSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NNC (S-Nonic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonic-vertex    NNC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nnc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOC (S-Octic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octic-edge      NHOC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHSO (S-Hextic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hextic-sombor   NHSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nhso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NNC extends NOC to 9th power; NHOC extends NHHS to 8th; NHSO=SO\u{2076} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices36(sink: &ConsoleSink) {
    let (ndc, nhnc, noso, edge_count, node_count) =
        gos_runtime::graph_topo_indices36();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo36 (NDC + NHNC + NOSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NDC (S-Decic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-decic-vertex    NDC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ndc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHNC (S-Nonic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonic-edge      NHNC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhnc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NOSO (S-Octic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octic-sombor    NOSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, noso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NDC extends NNC to 10th power; NHNC extends NHOC to 9th; NOSO=SO\u{2078} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices37(sink: &ConsoleSink) {
    let (nuc, nhdc, ntso, edge_count, node_count) =
        gos_runtime::graph_topo_indices37();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo37 (NUC + NHDC + NTSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NUC (S-Undecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-undecic-vertex  NUC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nuc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHDC (S-Decic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-decic-edge      NHDC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhdc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NTSO (S-Tenth Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tenth-sombor    NTSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, ntso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NUC extends NDC to 11th power; NHDC extends NHNC to 10th; NTSO=SO\u{00b9}\u{2070} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices38(sink: &ConsoleSink) {
    let (ndoc, nhuc, ndso, edge_count, node_count) =
        gos_runtime::graph_topo_indices38();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo38 (NDoC + NHUC + NDSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NDoC (S-Dodecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dodecic-vertex  NDoC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ndoc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHUC (S-Undecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-undecic-edge    NHUC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhuc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NDSO (S-Duodecic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-duodecic-sombor NDSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, ndso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NDoC extends NUC to 12th power; NHUC extends NHDC to 11th; NDSO=SO\u{00b9}\u{00b2} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices39(sink: &ConsoleSink) {
    let (ntc, nhdoc, neso, edge_count, node_count) =
        gos_runtime::graph_topo_indices39();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo39 (NTC + NHDOC + NESO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTC (S-Tridecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tridecic-vertex   NTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHDOC (S-Dodecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dodecic-edge    NHDOC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhdoc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NESO (S-Tetradecic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetradecic-sombor NESO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, neso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTC extends NDoC to 13th power; NHDOC extends NHUC to 12th; NESO=SO\u{00b9}\u{2074} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices40(sink: &ConsoleSink) {
    let (nqtc, nhtc, ngso, edge_count, node_count) =
        gos_runtime::graph_topo_indices40();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo40 (NQTC + NHTC + NGSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NQTC (S-Tetradecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetradecic-vertex  NQTC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nqtc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTC (S-Tridecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tridecic-edge      NHTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NGSO (S-Hexadecic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexadecic-sombor   NGSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, ngso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NQTC extends NTC to 14th power; NHTC extends NHDOC to 13th; NGSO=SO\u{00b9}\u{2076} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices41(sink: &ConsoleSink) {
    let (nptc, nhqtc, nioso, edge_count, node_count) =
        gos_runtime::graph_topo_indices41();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo41 (NPTC + NHQTC + NIOSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NPTC (S-Pentadecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentadecic-vertex  NPTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nptc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHQTC (S-Tetradecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetradecic-edge    NHQTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhqtc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NIOSO (S-Octadecic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octadecic-sombor   NIOSO =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nioso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NPTC extends NQTC to 15th power; NHQTC extends NHTC to 14th; NIOSO=SO\u{00b9}\u{2078} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices42(sink: &ConsoleSink) {
    let (nstc, nhptc, njso, edge_count, node_count) =
        gos_runtime::graph_topo_indices42();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo42 (NSTC + NHPTC + NJSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NSTC (S-Hexadecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexadecic-vertex   NSTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nstc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHPTC (S-Pentadecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentadecic-edge    NHPTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhptc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NJSO (S-Eicosic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-eicosic-sombor     NJSO  =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, njso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NSTC extends NPTC to 16th power; NHPTC extends NHQTC to 15th; NJSO=SO\u{00b2}\u{2070} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices43(sink: &ConsoleSink) {
    let (nheptc, nhstc, nkso, edge_count, node_count) =
        gos_runtime::graph_topo_indices43();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo43 (NHEPTC + NHSTC + NKSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTC (S-Heptadecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptadecic-vertex  NHEPTC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHSTC (S-Hexadecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexadecic-edge     NHSTC  =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhstc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NKSO (S-Docosic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-docosic-sombor     NKSO   =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nkso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTC extends NSTC to 17th power; NHSTC extends NHPTC to 16th; NKSO=SO\u{00b2}\u{00b2} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices44(sink: &ConsoleSink) {
    let (noctc, nhoctc, nlso, edge_count, node_count) =
        gos_runtime::graph_topo_indices44();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo44 (NOCTC + NHOCTC + NLSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTC (S-Octadecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octadecic-vertex   NOCTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTC (S-Heptadecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptadecic-edge    NHOCTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NLSO (S-Tetracosic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetracosic-sombor  NLSO   =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nlso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTC extends NHEPTC to 18th power; NHOCTC extends NHSTC to 17th; NLSO=SO\u{00b2}\u{2074} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices45(sink: &ConsoleSink) {
    let (nnontc, nhnontc, nmso, edge_count, node_count) =
        gos_runtime::graph_topo_indices45();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo45 (NNONTC + NHNONTC + NMSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NNONTC (S-Nonadecic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonadecic-vertex   NNONTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nnontc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b9}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHNONTC (S-Octadecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octadecic-edge    NHNONTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhnontc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NMSO (S-Hexacosic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexacosic-sombor  NMSO    =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nmso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NNONTC extends NOCTC to 19th power; NHNONTC extends NHOCTC to 18th; NMSO=SO\u{00b2}\u{2076} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices46(sink: &ConsoleSink) {
    let (neictc, nheictc, nnso, edge_count, node_count) =
        gos_runtime::graph_topo_indices46();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo46 (NEICTC + NHEICTC + NNSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NEICTC (S-Eicosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-eicosic-vertex    NEICTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, neictc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHEICTC (S-Nonadecic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonadecic-edge   NHEICTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nheictc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b9}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NNSO (S-Octacosic Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octacosic-sombor NNSO    =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nnso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NEICTC extends NNONTC to 20th power; NHEICTC extends NHNONTC to 19th; NNSO=SO\u{00b2}\u{2078} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices47(sink: &ConsoleSink) {
    let (nhentc, nhhentc, npso, edge_count, node_count) =
        gos_runtime::graph_topo_indices47();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo47 (NHENTC + NHHENTC + NPSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHENTC (S-Heneicosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heneicosic-vertex  NHENTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhentc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHENTC (S-Eicosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-eicosic-edge      NHHENTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhentc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NPSO (S-Triacontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-triacontyl-sombor NPSO    =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, npso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHENTC extends NEICTC to 21st power; NHHENTC extends NHEICTC to 20th; NPSO=SO\u{00b3}\u{2070} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices57(sink: &ConsoleSink) {
    let (nhentriactc, nhhentriactc, nbso, edge_count, node_count) =
        gos_runtime::graph_topo_indices57();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo57 (NHENTRIACTC + NHHENTRIACTC + NBSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHENTRIACTC (S-Hentriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hentriacontic-vtx  NHENTRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhentriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHENTRIACTC (S-Triacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-triacontic-edge    NHHENTRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhentriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBSO (S-Pentacontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentacontyl-sombor NBSO      =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHENTRIACTC extends NTRIACTC to 31st power; NHHENTRIACTC extends NHTRIACTC to 30th; NBSO=SO\u{2075}\u{2070} on S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices65(sink: &ConsoleSink) {
    let (nnonatriactc, nhnonatriactc, nahso, edge_count, node_count) =
        gos_runtime::graph_topo_indices65();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo65 (NNONATRIACTC + NHNONATRIACTC + NAHSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NNONATRIACTC (S-Nonatriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonatriacontic-vtx   NNONATRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nnonatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHNONATRIACTC (S-Octatriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatriacontic-edge  NHNONATRIACTC= ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhnonatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAHSO (S-Hexahexacontyl Sombor, α=66)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexahexacontyl-sb    NAHSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nahso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NNONATRIACTC extends NOCTATRIACTC to 39th power; NHNONATRIACTC extends NHOCTATRIACTC to 38th; NAHSO=SO\u{00b5}\u{2076}\u{2076} on S, 3rd-pass AH)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices66(sink: &ConsoleSink) {
    let (ntetraactc, nhtetraactc, naiso, edge_count, node_count) =
        gos_runtime::graph_topo_indices66();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo66 (NTETRAACTC + NHTETRAACTC + NAISO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTETRAACTC (S-Tetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetracontic-vtx      NTETRAACTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTETRAACTC (S-Nonatriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonatriacontic-edge  NHTETRAACTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAISO (S-Octahexacontyl Sombor, α=68)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octahexacontyl-sb    NAISO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, naiso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTETRAACTC extends NNONATRIACTC to 40th power; NHTETRAACTC extends NHNONATRIACTC to 39th; NAISO=SO\u{00b5}\u{2076}\u{2078} on S, 3rd-pass AI)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices67(sink: &ConsoleSink) {
    let (nhentetraactc, nhhentetraactc, najso, edge_count, node_count) =
        gos_runtime::graph_topo_indices67();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo67 (NHENTETRAACTC + NHHENTETRAACTC + NAJSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHENTETRAACTC (S-Hentetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hentetracontic-vtx   NHENTETRAACTC= ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhentetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHENTETRAACTC (S-Tetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetracontic-edge     NHHENTETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhentetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAJSO (S-Tetracontyl Sombor, α=70)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetracontyl-sb       NAJSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, najso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHENTETRAACTC extends NTETRAACTC to 41st power; NHHENTETRAACTC extends NHTETRAACTC to 40th; NAJSO=SO\u{00b5}\u{2077}\u{2070} on S, 3rd-pass AJ)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices68(sink: &ConsoleSink) {
    let (ndotetraactc, nhdotetraactc, nakso, edge_count, node_count) =
        gos_runtime::graph_topo_indices68();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo68 (NDOTETRAACTC + NHDOTETRAACTC + NAKSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NDOTETRAACTC (S-Dotetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dotetracontic-vtx    NDOTETRAACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ndotetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHDOTETRAACTC (S-Hentetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hentetracontic-edge  NHDOTETRAACTC= ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhdotetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAKSO (S-Dotetracontyl Sombor, α=72)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dotetracontyl-sb     NAKSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nakso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NDOTETRAACTC extends NHENTETRAACTC to 42nd power; NHDOTETRAACTC extends NHHENTETRAACTC to 41st; NAKSO=SO\u{00b5}\u{2077}\u{00b2} on S, 3rd-pass AK)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices69(sink: &ConsoleSink) {
    let (ntritetraactc, nhtritetraactc, nalso, edge_count, node_count) =
        gos_runtime::graph_topo_indices69();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo69 (NTRITETRAACTC + NHTRITETRAACTC + NALSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTRITETRAACTC (S-Tritetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tritetracontic-vtx   NTRITETRAACTC= ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntritetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTRITETRAACTC (S-Dotetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dotetracontic-edge   NHTRITETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtritetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NALSO (S-Tritetracontyl Sombor, α=74)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tritetracontyl-sb    NALSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nalso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTRITETRAACTC extends NDOTETRAACTC to 43rd power; NHTRITETRAACTC extends NHDOTETRAACTC to 42nd; NALSO=SO\u{00b5}\u{2077}\u{2074} on S, 3rd-pass AL)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices70(sink: &ConsoleSink) {
    let (ntetratetraactc, nhtetratetraactc, namso, edge_count, node_count) =
        gos_runtime::graph_topo_indices70();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo70 (NTETRATETRAACTC + NHTETRATETRAACTC + NAMSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTETRATETRAACTC (S-Tetratetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetratetracontic-vtx NTETRATETRAACTC= ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntetratetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTETRATETRAACTC (S-Tritetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tritetracontic-edge  NHTETRATETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtetratetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAMSO (S-Tetratetracontyl Sombor, α=76)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetratetracontyl-sb  NAMSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, namso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTETRATETRAACTC extends NTRITETRAACTC to 44th power; NHTETRATETRAACTC extends NHTRITETRAACTC to 43rd; NAMSO=SO\u{00b5}\u{2077}\u{2076} on S, 3rd-pass AM)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices71(sink: &ConsoleSink) {
    let (npentetraactc, nhpentetraactc, nanso, edge_count, node_count) =
        gos_runtime::graph_topo_indices71();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo71 (NPENTETRAACTC + NHPENTETRAACTC + NANSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NPENTETRAACTC (S-Pentatetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentatetracontic-vtx NPENTETRAACTC = ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, npentetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHPENTETRAACTC (S-Tetratetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetratetracontic-edge NHPENTETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhpentetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NANSO (S-Pentatetracontyl Sombor, α=78)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentatetracontyl-sb   NANSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nanso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NPENTETRAACTC extends NTETRATETRAACTC to 45th power; NHPENTETRAACTC extends NHTETRATETRAACTC to 44th; NANSO=SO\u{00b5}\u{2077}\u{2078} on S, 3rd-pass AN)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices72(sink: &ConsoleSink) {
    let (nhextetraactc, nhhextetraactc, naoso, edge_count, node_count) =
        gos_runtime::graph_topo_indices72();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo72 (NHEXTETRAACTC + NHHEXTETRAACTC + NAOSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXTETRAACTC (S-Hexatetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatetracontic-vtx  NHEXTETRAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhextetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXTETRAACTC (S-Pentatetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentatetracontic-edge NHHEXTETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhextetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAOSO (S-Octacontyl Sombor, α=80)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octacontyl-sb         NAOSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, naoso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXTETRAACTC extends NPENTETRAACTC to 46th power; NHHEXTETRAACTC extends NHPENTETRAACTC to 45th; NAOSO=SO\u{00b5}\u{2078}\u{2070} on S, 3rd-pass AO)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices73(sink: &ConsoleSink) {
    let (nheptetraactc, nhheptetraactc, napso, edge_count, node_count) =
        gos_runtime::graph_topo_indices73();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo73 (NHEPTETRAACTC + NHHEPTETRAACTC + NAPSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTETRAACTC (S-Heptatetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptatetracontic-vtx  NHEPTETRAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTETRAACTC (S-Hexatetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatetracontic-edge  NHHEPTETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAPSO (S-Docosacontyl Sombor, α=82)
        set_color(sink, 8, 0);
        print_str(sink, "  S-docosacontyl-sb        NAPSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, napso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTETRAACTC extends NHEXTETRAACTC to 47th power; NHHEPTETRAACTC extends NHHEXTETRAACTC to 46th; NAPSO=SO\u{00b5}\u{2078}\u{00b2} on S, 3rd-pass AP)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices74(sink: &ConsoleSink) {
    let (noctotetraactc, nhoctotetraactc, naqso, edge_count, node_count) =
        gos_runtime::graph_topo_indices74();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo74 (NOCTOTETRAACTC + NHOCTOTETRAACTC + NAQSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTOTETRAACTC (S-Octotetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octotetracontic-vtx   NOCTOTETRAACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctotetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTOTETRAACTC (S-Heptotetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptotetracontic-edge NHOCTOTETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctotetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAQSO (S-Tetrahexacontyl Sombor, α=84)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetrahexacontyl-sb     NAQSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, naqso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTOTETRAACTC extends NHEPTETRAACTC to 48th power; NHOCTOTETRAACTC extends NHHEPTETRAACTC to 47th; NAQSO=SO\u{00b5}\u{2078}\u{2074} on S, 3rd-pass AQ)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices75(sink: &ConsoleSink) {
    let (nnonatetraactc, nhnonatetraactc, narso, edge_count, node_count) =
        gos_runtime::graph_topo_indices75();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo75 (NNONATETRAACTC + NHNONATETRAACTC + NARSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NNONATETRAACTC (S-Nonatetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonatetracontic-vtx   NNONATETRAACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nnonatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2074}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHNONATETRAACTC (S-Octotetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octotetracontic-edge  NHNONATETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhnonatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NARSO (S-Hexaoctacontyl Sombor, α=86)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaoctacontyl-sb      NARSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, narso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NNONATETRAACTC extends NOCTOTETRAACTC to 49th power; NHNONATETRAACTC extends NHOCTOTETRAACTC to 48th; NARSO=SO\u{00b5}\u{2078}\u{2076} on S, 3rd-pass AR)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices77(sink: &ConsoleSink) {
    let (nhenpentaactc, nhhenpentaactc, natso, edge_count, node_count) =
        gos_runtime::graph_topo_indices77();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo77 (NHENPENTAACTC + NHHENPENTAACTC + NATSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHENPENTAACTC (S-Henpentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-henpentacontic-vtx    NHENPENTAACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhenpentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHENPENTAACTC (S-Pentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentacontic-edge      NHHENPENTAACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhenpentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NATSO (S-Variant Sombor, α=90)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonacontyl-sb          NATSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, natso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHENPENTAACTC extends NPENTAACTC to 51st power; NHHENPENTAACTC extends NHPENTAACTC to 50th; NATSO=SO\u{00b5}\u{2079}\u{2070} on S, 3rd-pass AT)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices78(sink: &ConsoleSink) {
    let (ndopentaactc, nhdopentaactc, nauso, edge_count, node_count) =
        gos_runtime::graph_topo_indices78();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo78 (NDOPENTAACTC + NHDOPENTAACTC + NAUSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NDOPENTAACTC (S-Dopentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dopentacontic-vtx     NDOPENTAACTC   =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ndopentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHDOPENTAACTC (S-Henpentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-henpentacontic-edge   NHDOPENTAACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhdopentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAUSO (S-Variant Sombor, α=92)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dinonacontyl-sb        NAUSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nauso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NDOPENTAACTC extends NHENPENTAACTC to 52nd power; NHDOPENTAACTC extends NHHENPENTAACTC to 51st; NAUSO=SO\u{00b5}\u{2079}\u{00b2} on S, 3rd-pass AU)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices79(sink: &ConsoleSink) {
    let (ntripentaactc, nhtripentaactc, navso, edge_count, node_count) =
        gos_runtime::graph_topo_indices79();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo79 (NTRIPENTAACTC + NHTRIPENTAACTC + NAVSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTRIPENTAACTC (S-Tripentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tripentacontic-vtx     NTRIPENTAACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntripentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTRIPENTAACTC (S-Dopentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dopentacontic-edge     NHTRIPENTAACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtripentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAVSO (S-Variant Sombor, α=94)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetranonacontyl-sb      NAVSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, navso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTRIPENTAACTC extends NDOPENTAACTC to 53rd power; NHTRIPENTAACTC extends NHDOPENTAACTC to 52nd; NAVSO=SO\u{00b5}\u{2079}\u{2074} on S, 3rd-pass AV)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices80(sink: &ConsoleSink) {
    let (ntetrapentaactc, nhtetrapentaactc, nawso, edge_count, node_count) =
        gos_runtime::graph_topo_indices80();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo80 (NTETRAPENTAACTC + NHTETRAPENTAACTC + NAWSO S-variant indices)\n");
    print_str(sink, " graph topo81 (NPENTAPENTAACTC + NHPENTAPENTAACTC + NAXSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTETRAPENTAACTC (S-Tetrapentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetrapentacontic-vtx   NTETRAPENTAACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntetrapentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTETRAPENTAACTC (S-Tripentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tripentacontic-edge    NHTETRAPENTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtetrapentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAWSO (S-Variant Sombor, α=96)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexanonacontyl-sb       NAWSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nawso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTETRAPENTAACTC extends NTRIPENTAACTC to 54th power; NHTETRAPENTAACTC extends NHTRIPENTAACTC to 53rd; NAWSO=SO\u{00b5}\u{2079}\u{2076} on S, 3rd-pass AW)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices81(sink: &ConsoleSink) {
    let (npentapentaactc, nhpentapentaactc, naxso, edge_count, node_count) =
        gos_runtime::graph_topo_indices81();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo81 (NPENTAPENTAACTC + NHPENTAPENTAACTC + NAXSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NPENTAPENTAACTC (S-Pentapentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentapentacontic-vtx   NPENTAPENTAACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, npentapentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHPENTAPENTAACTC (S-Tetrapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetrapentacontic-edge  NHPENTAPENTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhpentapentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAXSO (S-Variant Sombor, α=98)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octanonacontyl-sb       NAXSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, naxso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NPENTAPENTAACTC extends NTETRAPENTAACTC to 55th power; NHPENTAPENTAACTC extends NHTETRAPENTAACTC to 54th; NAXSO=SO\u{00b5}\u{2079}\u{2078} on S, 3rd-pass AX)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices82(sink: &ConsoleSink) {
    let (nhexpentaactc, nhhexpentaactc, nayso, edge_count, node_count) =
        gos_runtime::graph_topo_indices82();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo82 (NHEXPENTAACTC + NHHEXPENTAACTC + NAYSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXPENTAACTC (S-Hexapentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexapentacontic-vtx    NHEXPENTAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexpentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXPENTAACTC (S-Pentapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentapentacontic-edge  NHHEXPENTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexpentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAYSO (S-Variant Sombor, α=100)
        set_color(sink, 8, 0);
        print_str(sink, "  S-centyl-sombor           NAYSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nayso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXPENTAACTC extends NPENTAPENTAACTC to 56th power; NHHEXPENTAACTC extends NHPENTAPENTAACTC to 55th; NAYSO=SO\u{00b5}\u{00b9}\u{2070}\u{2070} on S, 3rd-pass AY)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices83(sink: &ConsoleSink) {
    let (nheptpentaactc, nhheptpentaactc, nazso, edge_count, node_count) =
        gos_runtime::graph_topo_indices83();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo83 (NHEPTPENTAACTC + NHHEPTPENTAACTC + NAZSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTPENTAACTC (S-Heptapentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptapentacontic-vtx    NHEPTPENTAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptpentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTPENTAACTC (S-Hexapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexapentacontic-edge   NHHEPTPENTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptpentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAZSO (S-Variant Sombor, α=102)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor         NAZSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nazso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTPENTAACTC extends NHEXPENTAACTC to 57th power; NHHEPTPENTAACTC extends NHHEXPENTAACTC to 56th; NAZSO=SO\u{00b5}\u{00b9}\u{2070}\u{00b2} on S, 3rd-pass AZ)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices85(sink: &ConsoleSink) {
    let (nnonapentaactc, nhnonapentaactc, nbbso, edge_count, node_count) =
        gos_runtime::graph_topo_indices85();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo85 (NNONAPENTAACTC + NHNONAPENTAACTC + NBBSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NNONAPENTAACTC (S-Nonapentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonapentacontic-vtx    NNONAPENTAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nnonapentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHNONAPENTAACTC (S-Octapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octapentacontic-edge   NHNONAPENTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhnonapentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBBSO (S-Variant Sombor, α=106)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bb      NBBSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbbso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NNONAPENTAACTC extends NOCTOPENTAACTC to 59th power; 10th/last of pentacontic; NBBSO=SO\u{00b5}\u{00b9}\u{2070}\u{2076} on S, 4th-pass BB)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices86(sink: &ConsoleSink) {
    let (nhexaactc, nhhexaactc, nbcso, edge_count, node_count) =
        gos_runtime::graph_topo_indices86();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo86 (NHEXAACTC + NHHEXAACTC + NBCSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXAACTC (S-Hexacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexacontic-vtx          NHEXAACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXAACTC (S-Nonapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonapentacontic-edge    NHHEXAACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBCSO (S-Variant Sombor, α=108)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bc       NBCSO      =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbcso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXAACTC extends NNONAPENTAACTC to 60th power; 1st of hexacontic; NBCSO=SO\u{00b5}\u{00b9}\u{2070}\u{2078} on S, 3rd of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices87(sink: &ConsoleSink) {
    let (nhexaenactc, nhhexaenactc, nbdso, edge_count, node_count) =
        gos_runtime::graph_topo_indices87();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo87 (NHEXAENACTC + NHHEXAENACTC + NBDSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXAENACTC (S-Hexaencontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaencontic-vtx        NHEXAENACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexaenactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXAENACTC (S-Hexacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexacontic-edge         NHHEXAENACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexaenactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBDSO (S-Variant Sombor, α=110)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bd       NBDSO      =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbdso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXAENACTC extends NHEXAACTC to 61st power; 2nd of hexacontic; NBDSO=SO\u{00b5}\u{00b9}\u{00b9}\u{2070} on S, 4th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices88(sink: &ConsoleSink) {
    let (nhexadyactc, nhhexadyactc, nbeso, edge_count, node_count) =
        gos_runtime::graph_topo_indices88();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo88 (NHEXADYACTC + NHHEXADYACTC + NBESO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXADYACTC (S-Hexadycontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexadycontic-vtx        NHEXADYACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexadyactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXADYACTC (S-Hexaencontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaencontic-edge       NHHEXADYACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexadyactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBESO (S-Variant Sombor, α=112)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-be       NBESO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbeso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXADYACTC extends NHEXAENACTC to 62nd power; 3rd of hexacontic; NBESO=SO\u{00b5}\u{00b9}\u{00b9}\u{00b2} on S, 5th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices89(sink: &ConsoleSink) {
    let (nhexatriactc, nhhexatriactc, nbfso, edge_count, node_count) =
        gos_runtime::graph_topo_indices89();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo89 (NHEXATRIACTC + NHHEXATRIACTC + NBFSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXATRIACTC (S-Hexatricontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatricontic-vtx       NHEXATRIACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXATRIACTC (S-Hexadycontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexadycontic-edge       NHHEXATRIACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBFSO (S-Variant Sombor, α=114)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bf       NBFSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbfso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXATRIACTC extends NHEXADYACTC to 63rd power; 4th of hexacontic; NBFSO=SO\u{00b5}\u{00b9}\u{00b9}\u{2074} on S, 6th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices91(sink: &ConsoleSink) {
    let (nhexapentactc, nhhexapentactc, nbhso, edge_count, node_count) =
        gos_runtime::graph_topo_indices91();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo91 (NHEXAPENTACTC + NHHEXAPENTACTC + NBHSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXAPENTACTC (S-Hexapentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexapentactic-vtx       NHEXAPENTACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexapentactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXAPENTACTC (S-Hexatetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatetracontic-edge    NHHEXAPENTACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexapentactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBHSO (S-Variant Sombor, α=118)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bh       NBHSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbhso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXAPENTACTC extends NHEXATETRAACTC to 65th power; 6th of hexacontic; NBHSO=SO\u{00b5}\u{00b9}\u{00b9}\u{2078} on S, 8th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices92(sink: &ConsoleSink) {
    let (nhexahexaactc, nhhexahexaactc, nbisos, edge_count, node_count) =
        gos_runtime::graph_topo_indices92();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo92 (NHEXAHEXAACTC + NHHEXAHEXAACTC + NBISOS S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXAHEXAACTC (S-Hexahexacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexahexactic-vtx        NHEXAHEXAACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexahexaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXAHEXAACTC (S-Hexahexacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexahexacontic-edge     NHHEXAHEXAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexahexaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBISOS (S-Variant Sombor, α=120)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bi       NBISOS      =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbisos as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXAHEXAACTC extends NHEXAPENTACTC to 66th power; 7th of hexacontic; NBISOS=SO\u{00b5}\u{00b9}\u{00b2}\u{2070} on S, 9th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices90(sink: &ConsoleSink) {
    let (nhexatetraactc, nhhexatetraactc, nbgso, edge_count, node_count) =
        gos_runtime::graph_topo_indices90();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo90 (NHEXATETRAACTC + NHHEXATETRAACTC + NBGSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXATETRAACTC (S-Hexacontictetradic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatetractic-vtx       NHEXATETRAACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXATETRAACTC (S-Hexatricontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatricontic-edge      NHHEXATETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBGSO (S-Variant Sombor, α=116)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bg       NBGSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbgso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXATETRAACTC extends NHEXATRIACTC to 64th power; 5th of hexacontic; NBGSO=SO\u{00b5}\u{00b9}\u{00b9}\u{2076} on S, 7th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices93(sink: &ConsoleSink) {
    let (nhexaheptactc, nhhexaheptactc, nbjso, edge_count, node_count) =
        gos_runtime::graph_topo_indices93();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo93 (NHEXAHEPTACTC + NHHEXAHEPTACTC + NBJSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXAHEPTACTC (S-Hexaheptacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaheptactic-vtx       NHEXAHEPTACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexaheptactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXAHEPTACTC (S-Hexaheptacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexahexacontic-edge    NHHEXAHEPTACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexaheptactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBJSO (S-Variant Sombor, α=122)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bj       NBJSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbjso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXAHEPTACTC extends NHEXAHEXAACTC to 67th power; 8th of hexacontic; NBJSO=SO\u{00b5}\u{00b9}\u{00b2}\u{00b2} on S, 10th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices94(sink: &ConsoleSink) {
    let (nhexaoctactc, nhhexaoctactc, nbkso, edge_count, node_count) =
        gos_runtime::graph_topo_indices94();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo94 (NHEXAOCTACTC + NHHEXAOCTACTC + NBKSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXAOCTACTC (S-Hexaoctatontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaoctactic-vtx        NHEXAOCTACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexaoctactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXAOCTACTC (S-Hexaoctatontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaheptcontic-edge     NHHEXAOCTACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexaoctactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBKSO (S-Variant Sombor, α=124)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bk       NBKSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbkso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXAOCTACTC extends NHEXAHEPTACTC to 68th power; 9th of hexacontic; NBKSO=SO\u{00b5}\u{00b9}\u{00b2}\u{2074} on S, 11th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices95(sink: &ConsoleSink) {
    let (nhexaennactc, nhhexaennactc, nblso, edge_count, node_count) =
        gos_runtime::graph_topo_indices95();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo95 (NHEXAENNACTC + NHHEXAENNACTC + NBLSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXAENNACTC (S-Hexaennacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaennacontic-vtx      NHEXAENNACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexaennactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2076}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXAENNACTC (S-Hexaennacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexaoctcontic-edge     NHHEXAENNACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexaennactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBLSO (S-Variant Sombor, α=126)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bl       NBLSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nblso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXAENNACTC extends NHEXAOCTACTC to 69th power; 10th (LAST) of hexacontic; NBLSO=SO\u{00b5}\u{00b9}\u{00b2}\u{2076} on S, 12th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices96(sink: &ConsoleSink) {
    let (nheptaactc, nhheptaactc, nbmso, edge_count, node_count) =
        gos_runtime::graph_topo_indices96();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo96 (NHEPTAACTC + NHHEPTAACTC + NBMSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTAACTC (S-Heptacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptacontic-vtx          NHEPTAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTAACTC (S-Heptacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptacontic-edge        NHHEPTAACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2076}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBMSO (S-Variant Sombor, α=128)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bm        NBMSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbmso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTAACTC extends hexacontic to 70th power; FIRST of heptacontic (70-79); NBMSO=SO\u{00b5}\u{00b9}\u{00b2}\u{2078} on S, 13th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices97(sink: &ConsoleSink) {
    let (nheptaenactc, nhheptaenactc, nbnso, edge_count, node_count) =
        gos_runtime::graph_topo_indices97();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo97 (NHEPTAENACTC + NHHEPTAENACTC + NBNSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTAENACTC (S-Heptaencontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaencontic-vtx       NHEPTAENACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptaenactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTAENACTC (S-Heptaencontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaencontic-edge     NHHEPTAENACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptaenactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBNSO (S-Variant Sombor, α=130)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bn        NBNSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbnso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTAENACTC=\u{03a3} S\u{2077}\u{00b9} SECOND of heptacontic (70-79); NBNSO=SO\u{00b5}\u{00b9}\u{00b3}\u{2070} on S, 14th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices117(sink: &ConsoleSink) {
    let (nennamonoactc, nhennamonoactc, nbhhso, edge_count, node_count) =
        gos_runtime::graph_topo_indices117();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo117 (NENNAMONOACTC + NHENNAMONOACTC + NBHHSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  S-ennamonocontic-vtx    NENNAMONOACTC  =");
        set_color(sink, 11, 0);
        print_num_inline(sink, nennamonoactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2079}\u{00b9}]  ");
        print_str(sink, sat_label(nennamonoactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        set_color(sink, 8, 0);
        print_str(sink, "  S-ennamonocontic-edge  NHENNAMONOACTC  =");
        set_color(sink, 10, 0);
        print_num_inline(sink, nhennamonoactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2079}\u{2070}]  ");
        print_str(sink, sat_label(nhennamonoactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-hh         NBHHSO     =  ");
        set_color(sink, 13, 0);
        print_num_inline(sink, nbhhso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2078}\u{2075}]  ");
        print_str(sink, sat_label(nbhhso));
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NENNAMONOACTC=\u{03a3} S\u{2079}\u{00b9} 2nd of ennacontic (90-99); NBHHSO=SO\u{00b5}\u{00b9}\u{2077}\u{2070} on S, 34th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices116(sink: &ConsoleSink) {
    let (nennaactc, nhennaactc, nbggso, edge_count, node_count) =
        gos_runtime::graph_topo_indices116();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo116 (NENNAACTC + NHENNAACTC + NBGGSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        set_color(sink, 8, 0);
        print_str(sink, "  S-ennacontic-vtx      NENNAACTC   =");
        set_color(sink, 11, 0);
        print_num_inline(sink, nennaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2079}\u{2070}]  ");
        print_str(sink, sat_label(nennaactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        set_color(sink, 8, 0);
        print_str(sink, "  S-ennacontic-edge    NHENNAACTC   =");
        set_color(sink, 10, 0);
        print_num_inline(sink, nhennaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{2079}]  ");
        print_str(sink, sat_label(nhennaactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-gg          NBGGSO     =  ");
        set_color(sink, 13, 0);
        print_num_inline(sink, nbggso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2078}\u{2074}]  ");
        print_str(sink, sat_label(nbggso));
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NENNAACTC=\u{03a3} S\u{2079}\u{2070} FIRST of ennacontic (90-99); NBGGSO=SO\u{00b5}\u{00b9}\u{2076}\u{2078} on S, 33rd of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices115(sink: &ConsoleSink) {
    let (noctaennactc, nhoctaennactc, nbffso, edge_count, node_count) =
        gos_runtime::graph_topo_indices115();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo115 (NOCTAENNACTC + NHOCTAENNACTC + NBFFSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTAENNACTC (S-Octaennacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octaennacontic-vtx      NOCTAENNACTC   =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctaennactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{2079}]  ");
        print_str(sink, sat_label(noctaennactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTAENNACTC (S-Octaennacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octaennacontic-edge    NHOCTAENNACTC   =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctaennactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{2078}]  ");
        print_str(sink, sat_label(nhoctaennactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBFFSO (S-Variant Sombor, a=166)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-ff          NBFFSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbffso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2078}\u{00b3}]  ");
        print_str(sink, sat_label(nbffso));
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTAENNACTC=\u{03a3} S\u{2078}\u{2079} TENTH/FINAL of octacontic (80-89); NBFFSO=SO\u{00b5}\u{00b9}\u{2076}\u{2076} on S, 32nd of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices114(sink: &ConsoleSink) {
    let (noctaoctactc, nhoctaoctactc, nbeeso, edge_count, node_count) =
        gos_runtime::graph_topo_indices114();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo114 (NOCTAOCTACTC + NHOCTAOCTACTC + NBEESO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTAOCTACTC (S-Octaoctocontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octaoctocontic-vtx      NOCTAOCTACTC   =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctaoctactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{2078}]  ");
        print_str(sink, sat_label(noctaoctactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTAOCTACTC (S-Octaoctocontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octaoctocontic-edge    NHOCTAOCTACTC   =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctaoctactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{2077}]  ");
        print_str(sink, sat_label(nhoctaoctactc));
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBEESO (S-Variant Sombor, α=164)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-ee          NBEESO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbeeso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2078}\u{00b2}]  ");
        print_str(sink, sat_label(nbeeso));
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTAOCTACTC=\u{03a3} S\u{2078}\u{2078} NINTH of octacontic (80-89); NBEESO=SO\u{00b5}\u{00b9}\u{2076}\u{2074} on S, 31st of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices113(sink: &ConsoleSink) {
    let (noctaheptactc, nhoctaheptactc, nbddso, edge_count, node_count) =
        gos_runtime::graph_topo_indices113();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo113 (NOCTAHEPTACTC + NHOCTAHEPTACTC + NBDDSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTAHEPTACTC (S-Octaheptic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octaheptic-vtx         NOCTAHEPTACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctaheptactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTAHEPTACTC (S-Octaheptic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octaheptic-edge       NHOCTAHEPTACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctaheptactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBDDSO (S-Variant Sombor, α=162)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-dd         NBDDSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbddso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2078}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTAHEPTACTC=\u{03a3} S\u{2078}\u{2077} EIGHTH of octacontic (80-89); NBDDSO=SO\u{00b5}\u{00b9}\u{2076}\u{00b2} on S, 30th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices112(sink: &ConsoleSink) {
    let (noctahexactc, nhoctahexactc, nbccso, edge_count, node_count) =
        gos_runtime::graph_topo_indices112();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo112 (NOCTAHEXACTC + NHOCTAHEXACTC + NBCCSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTAHEXACTC (S-Octahexic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octahexic-vtx          NOCTAHEXACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctahexactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTAHEXACTC (S-Octahexic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octahexic-edge        NHOCTAHEXACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctahexactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBCCSO (S-Variant Sombor, α=160)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-cc         NBCCSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbccso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2078}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTAHEXACTC=\u{03a3} S\u{2078}\u{2076} SEVENTH of octacontic (80-89); NBCCSO=SO\u{00b5}\u{00b9}\u{2076}\u{2070} on S, 29th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices111(sink: &ConsoleSink) {
    let (noctapentactc, nhoctapentactc, nbbso, edge_count, node_count) =
        gos_runtime::graph_topo_indices111();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo111 (NOCTAPENTACTC + NHOCTAPENTACTC + NBBSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTAPENTACTC (S-Octapentic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octapentic-vtx        NOCTAPENTACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctapentactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTAPENTACTC (S-Octapentic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octapentic-edge      NHOCTAPENTACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctapentactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBBSO (S-Variant Sombor, α=158)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bb         NBBSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbbso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTAPENTACTC=\u{03a3} S\u{2078}\u{2075} SIXTH of octacontic (80-89); NBBSO=SO\u{00b5}\u{00b9}\u{2075}\u{2078} on S, 28th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices110(sink: &ConsoleSink) {
    let (noctatetraactc, nhoctatetraactc, nbaaso, edge_count, node_count) =
        gos_runtime::graph_topo_indices110();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo110 (NOCTATETRAACTC + NHOCTATETRAACTC + NBAASO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTATETRAACTC (S-Octatetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatetracontic-vtx   NOCTATETRAACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTATETRAACTC (S-Octatetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatetracontic-edge NHOCTATETRAACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBAASO (S-Variant Sombor, α=156)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-baa        NBAASO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbaaso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTATETRAACTC=\u{03a3} S\u{2078}\u{2074} FIFTH of octacontic (80-89); NBAASO=SO\u{00b5}\u{00b9}\u{2075}\u{2076} on S, 27th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices109(sink: &ConsoleSink) {
    let (noctatriactc, nhoctatriactc, nbzso, edge_count, node_count) =
        gos_runtime::graph_topo_indices109();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo109 (NOCTATRIACTC + NHOCTATRIACTC + NBZSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTATRIACTC (S-Octatricontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatricontic-vtx      NOCTATRIACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTATRIACTC (S-Octatricontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatricontic-edge    NHOCTATRIACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBZSO (S-Variant Sombor, α=154)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bz         NBZSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbzso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTATRIACTC=\u{03a3} S\u{2078}\u{00b3} FOURTH of octacontic (80-89); NBZSO=SO\u{00b5}\u{00b9}\u{2075}\u{2074} on S, 26th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices108(sink: &ConsoleSink) {
    let (noctadiactc, nhoctadiactc, nbyso, edge_count, node_count) =
        gos_runtime::graph_topo_indices108();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo108 (NOCTADIACTC + NHOCTADIACTC + NBYSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTADIACTC (S-Octadicontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octadicontic-vtx       NOCTADIACTC  =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctadiactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTADIACTC (S-Octadicontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octadicontic-edge     NHOCTADIACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctadiactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBYSO (S-Variant Sombor, α=152)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-by         NBYSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbyso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTADIACTC=\u{03a3} S\u{2078}\u{00b2} THIRD of octacontic (80-89); NBYSO=SO\u{00b5}\u{00b9}\u{2075}\u{00b2} on S, 25th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices107(sink: &ConsoleSink) {
    let (noctamonoactc, nhoctamonoactc, nbxso, edge_count, node_count) =
        gos_runtime::graph_topo_indices107();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo107 (NOCTAMONOACTC + NHOCTAMONOACTC + NBXSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTAMONOACTC (S-Octamonocontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octamonocontic-vtx    NOCTAMONOACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctamonoactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTAMONOACTC (S-Octamonocontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octamonocontic-edge  NHOCTAMONOACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctamonoactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2078}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBXSO (S-Variant Sombor, α=150)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bx        NBXSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbxso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTAMONOACTC=\u{03a3} S\u{2078}\u{00b9} SECOND of octacontic (80-89); NBXSO=SO\u{00b5}\u{00b9}\u{2075}\u{2070} on S, 24th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices106(sink: &ConsoleSink) {
    let (noctaactc, nhoctaactc, nbwso, edge_count, node_count) =
        gos_runtime::graph_topo_indices106();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo106 (NOCTAACTC + NHOCTAACTC + NBWSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTAACTC (S-Octacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octacontic-vtx         NOCTAACTC   =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2078}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTAACTC (S-Octacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octacontic-edge        NHOCTAACTC  =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBWSO (S-Variant Sombor, α=148)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bw        NBWSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbwso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTAACTC=\u{03a3} S\u{2078}\u{2070} FIRST of octacontic (80-89); NBWSO=SO\u{00b5}\u{00b9}\u{2074}\u{2078} on S, 23rd of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices105(sink: &ConsoleSink) {
    let (nheptaennactc, nhheptaennactc, nbvso, edge_count, node_count) =
        gos_runtime::graph_topo_indices105();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo105 (NHEPTAENNACTC + NHHEPTAENNACTC + NBVSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTAENNACTC (S-Heptaennacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaennacontic-vtx  NHEPTAENNACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptaennactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTAENNACTC (S-Heptaennacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaennacontic-edge NHHEPTAENNACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptaennactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBVSO (S-Variant Sombor, α=146)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bv        NBVSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbvso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTAENNACTC=\u{03a3} S\u{2077}\u{2079} TENTH (final) of heptacontic (70-79); NBVSO=SO\u{00b5}\u{00b9}\u{2074}\u{2076} on S, 22nd of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices104(sink: &ConsoleSink) {
    let (nheptaoctaactc, nhheptaoctaactc, nbuso, edge_count, node_count) =
        gos_runtime::graph_topo_indices104();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo104 (NHEPTAOCTAACTC + NHHEPTAOCTAACTC + NBUSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTAOCTAACTC (S-Heptaoctacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaoctacontic-vtx  NHEPTAOCTAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptaoctaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTAOCTAACTC (S-Heptaoctacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaoctacontic-edge NHHEPTAOCTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptaoctaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBUSO (S-Variant Sombor, α=144)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bu        NBUSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbuso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTAOCTAACTC=\u{03a3} S\u{2077}\u{2078} NINTH of heptacontic (70-79); NBUSO=SO\u{00b5}\u{00b9}\u{2074}\u{2074} on S, 21st of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices103(sink: &ConsoleSink) {
    let (nheptaheptaactc, nhheptaheptaactc, nbtso, edge_count, node_count) =
        gos_runtime::graph_topo_indices103();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo103 (NHEPTAHEPTAACTC + NHHEPTAHEPTAACTC + NBTSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTAHEPTAACTC (S-Heptaheptacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaheptacontic-vtx  NHEPTAHEPTAACTC=");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptaheptaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTAHEPTAACTC (S-Heptaheptacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptaheptacontic-edge NHHEPTAHEPTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptaheptaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBTSO (S-Variant Sombor, α=142)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bt        NBTSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbtso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTAHEPTAACTC=\u{03a3} S\u{2077}\u{2077} EIGHTH of heptacontic (70-79); NBTSO=SO\u{00b5}\u{00b9}\u{2074}\u{00b2} on S, 20th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices102(sink: &ConsoleSink) {
    let (nheptahexaactc, nhheptahexaactc, nbsso, edge_count, node_count) =
        gos_runtime::graph_topo_indices102();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo102 (NHEPTAHEXAACTC + NHHEPTAHEXAACTC + NBSSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTAHEXAACTC (S-Heptahexacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptahexacontic-vtx  NHEPTAHEXAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptahexaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTAHEXAACTC (S-Heptahexacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptahexacontic-edge NHHEPTAHEXAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptahexaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBSSO (S-Variant Sombor, α=140)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bs        NBSSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbsso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2077}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTAHEXAACTC=\u{03a3} S\u{2077}\u{2076} SEVENTH of heptacontic (70-79); NBSSO=SO\u{00b5}\u{00b9}\u{2074}\u{2070} on S, 19th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices101(sink: &ConsoleSink) {
    let (nheptapentactc, nhheptapentactc, nbrso, edge_count, node_count) =
        gos_runtime::graph_topo_indices101();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo101 (NHEPTAPENTACTC + NHHEPTAPENTACTC + NBRSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTAPENTACTC (S-Heptapentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptapentacontic-vtx  NHEPTAPENTACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptapentactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTAPENTACTC (S-Heptapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptapentacontic-edge NHHEPTAPENTACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptapentactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBRSO (S-Variant Sombor, α=138)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-br        NBRSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbrso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTAPENTACTC=\u{03a3} S\u{2077}\u{2075} SIXTH of heptacontic (70-79); NBRSO=SO\u{00b5}\u{00b9}\u{00b3}\u{2078} on S, 18th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices100(sink: &ConsoleSink) {
    let (nheptatetraactc, nhheptatetraactc, nbqso, edge_count, node_count) =
        gos_runtime::graph_topo_indices100();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo100 (NHEPTATETRAACTC + NHHEPTATETRAACTC + NBQSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTATETRAACTC (S-Heptatetracontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptatetracontic-vtx  NHEPTATETRAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTATETRAACTC (S-Heptatetracontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptatetracontic-edge NHHEPTATETRAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptatetraactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBQSO (S-Variant Sombor, α=136)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bq        NBQSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbqso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTATETRAACTC=\u{03a3} S\u{2077}\u{2074} FIFTH of heptacontic (70-79); NBQSO=SO\u{00b5}\u{00b9}\u{00b3}\u{2076} on S, 17th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices99(sink: &ConsoleSink) {
    let (nheptatriactc, nhheptatriactc, nbpso, edge_count, node_count) =
        gos_runtime::graph_topo_indices99();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo99 (NHEPTATRIACTC + NHHEPTATRIACTC + NBPSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTATRIACTC (S-Heptatricontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptatricontic-vtx      NHEPTATRIACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTATRIACTC (S-Heptatricontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptatricontic-edge    NHHEPTATRIACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBPSO (S-Variant Sombor, α=134)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bp        NBPSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbpso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTATRIACTC=\u{03a3} S\u{2077}\u{00b3} FOURTH of heptacontic (70-79); NBPSO=SO\u{00b5}\u{00b9}\u{00b3}\u{2074} on S, 16th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices98(sink: &ConsoleSink) {
    let (nheptadiactc, nhheptadiactc, nboso, edge_count, node_count) =
        gos_runtime::graph_topo_indices98();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo98 (NHEPTADIACTC + NHHEPTADIACTC + NBOSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTADIACTC (S-Heptadycontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptadicontic-vtx       NHEPTADIACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptadiactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2077}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTADIACTC (S-Heptadycontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptadicontic-edge     NHHEPTADIACTC =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptadiactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2077}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBOSO (S-Variant Sombor, α=132)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dohectyl-sombor-bo        NBOSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nboso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2076}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTADIACTC=\u{03a3} S\u{2077}\u{00b2} THIRD of heptacontic (70-79); NBOSO=SO\u{00b5}\u{00b9}\u{00b3}\u{00b2} on S, 15th of NB series)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices84(sink: &ConsoleSink) {
    let (noctopentaactc, nhoctopentaactc, nbaso, edge_count, node_count) =
        gos_runtime::graph_topo_indices84();

    set_color(sink, 7, 0);
    print_str(sink, " graph topo84 (NOCTOPENTAACTC + NHOCTOPENTAACTC + NBASO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTOPENTAACTC (S-Octopentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octopentacontic-vtx    NOCTOPENTAACTC =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctopentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTOPENTAACTC (S-Heptapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptapentacontic-edge  NHOCTOPENTAACTC=");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctopentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2075}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NBASO (S-Variant Sombor, α=104)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetrahectyl-sombor      NBASO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nbaso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2075}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTOPENTAACTC extends NHEPTPENTAACTC to 58th power; NHOCTOPENTAACTC extends NHHEPTPENTAACTC to 57th; NBASO=SO\u{00b5}\u{00b9}\u{2070}\u{2074} on S, 4th-pass BA)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices76(sink: &ConsoleSink) {
    let (npentaactc, nhpentaactc, nasso, edge_count, node_count) =
        gos_runtime::graph_topo_indices76();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo76 (NPENTAACTC + NHPENTAACTC + NASSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NPENTAACTC (S-Pentacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentacontic-vtx       NPENTAACTC     =");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, npentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{2075}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHPENTAACTC (S-Nonapentacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonapentacontic-edge  NHPENTAACTC    =");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhpentaactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{2074}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NASSO (S-Variant Sombor, α=88)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octaocontyl-sb         NASSO          =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nasso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{2074}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NPENTAACTC extends NNONATETRAACTC to 50th power; NHPENTAACTC extends NHNONATETRAACTC to 49th; NASSO=SO\u{00b5}\u{2078}\u{2078} on S, 3rd-pass AS)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices64(sink: &ConsoleSink) {
    let (noctatriactc, nhoctatriactc, nagso, edge_count, node_count) =
        gos_runtime::graph_topo_indices64();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo64 (NOCTATRIACTC + NHOCTATRIACTC + NAGSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTATRIACTC (S-Octatriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatriacontic-vtx   NOCTATRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTATRIACTC (S-Heptatriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptatriacontic-edge  NHOCTATRIACTC= ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAGSO (S-Tetrahexacontyl Sombor, α=64)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetrahexacontyl-sb    NAGSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nagso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTATRIACTC extends NHEPTATRIACTC to 38th power; NHOCTATRIACTC extends NHHEPTATRIACTC to 37th; NAGSO=SO\u{00b5}\u{2078} on S, 3rd-pass AG)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices63(sink: &ConsoleSink) {
    let (nheptatriactc, nhheptatriactc, nafso, edge_count, node_count) =
        gos_runtime::graph_topo_indices63();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo63 (NHEPTATRIACTC + NHHEPTATRIACTC + NAFSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTATRIACTC (S-Heptatriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptatriacontic-vtx  NHEPTATRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTATRIACTC (S-Hexatriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatriacontic-edge  NHHEPTATRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAFSO (S-Hexahexacontyl Sombor, α=62)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexahexacontyl-sb    NAFSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nafso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTATRIACTC extends NHEXATRIACTC to 37th power; NHHEPTATRIACTC extends NHHEXATRIACTC to 36th; NAFSO=SO\u{00b5}\u{b6} on S, 3rd-pass AF)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices62(sink: &ConsoleSink) {
    let (nhexatriactc, nhhexatriactc, naeso, edge_count, node_count) =
        gos_runtime::graph_topo_indices62();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo62 (NHEXATRIACTC + NHHEXATRIACTC + NAESO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXATRIACTC (S-Hexatriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatriacontic-vtx   NHEXATRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXATRIACTC (S-Pentatriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentatriacontic-edge NHHEXATRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexatriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAESO (S-Hexacontyl Sombor, α=60)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexacontyl-sb        NAESO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, naeso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b3}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXATRIACTC extends NPENTTRIACTC to 36th power; NHHEXATRIACTC extends NHPENTTRIACTC to 35th; NAESO=SO\u{00b5}\u{b4} on S, 3rd-pass AE)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices61(sink: &ConsoleSink) {
    let (npenttriactc, nhpenttriactc, nadso, edge_count, node_count) =
        gos_runtime::graph_topo_indices61();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo61 (NPENTTRIACTC + NHPENTTRIACTC + NADSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NPENTTRIACTC (S-Pentatriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentatriacontic-vtx  NPENTTRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, npenttriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHPENTTRIACTC (S-Tetratriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetratriacontic-edge NHPENTTRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhpenttriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NADSO (S-Octopentacontyl Sombor, α=58)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octopentacontyl-sb   NADSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nadso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NPENTTRIACTC extends NTETRTRIACTC to 35th power; NHPENTTRIACTC extends NHTETRTRIACTC to 34th; NADSO=SO\u{00b5}\u{b2} on S, 3rd-pass AD)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices60(sink: &ConsoleSink) {
    let (ntetrtriactc, nhtetrtriactc, nacso, edge_count, node_count) =
        gos_runtime::graph_topo_indices60();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo60 (NTETRTRIACTC + NHTETRTRIACTC + NACSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTETRTRIACTC (S-Tetratriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetratriacontic-vtx  NTETRTRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntetrtriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTETRTRIACTC (S-Tritriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tritriacontic-edge  NHTETRTRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtetrtriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NACSO (S-Hexapentacontyl Sombor, α=56)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexapentacontyl-sb  NACSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nacso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTETRTRIACTC extends NTRITRIACTC to 34th power; NHTETRTRIACTC extends NHTRITRIACTC to 33rd; NACSO=SO\u{00b5}\u{b2} on S, 3rd-pass AC)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices59(sink: &ConsoleSink) {
    let (ntritriactc, nhtritriactc, nabso, edge_count, node_count) =
        gos_runtime::graph_topo_indices59();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo59 (NTRITRIACTC + NHTRITRIACTC + NABSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTRITRIACTC (S-Tritriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tritriacontic-vtx  NTRITRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntritriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTRITRIACTC (S-Dotriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dotriacontic-edge  NHTRITRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtritriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NABSO (S-Dopentatecontyl Sombor, α=54)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dopentatecontyl-sb NABSO       =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nabso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTRITRIACTC extends NDOTRIACTC to 33rd power; NHTRITRIACTC extends NHDOTRIACTC to 32nd; NABSO=SO\u{00b5}\u{b2} on S, 3rd-pass AB)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices58(sink: &ConsoleSink) {
    let (ndotriactc, nhdotriactc, naaso, edge_count, node_count) =
        gos_runtime::graph_topo_indices58();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo58 (NDOTRIACTC + NHDOTRIACTC + NAASO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NDOTRIACTC (S-Dotriacontic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dotriacontic-vtx   NDOTRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ndotriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHDOTRIACTC (S-Hentriacontic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hentriacontic-edge NHDOTRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhdotriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b3}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NAASO (S-Dopentecontyl Sombor, α=52)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dopentecontyl-smbr NAASO      =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, naaso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NDOTRIACTC extends NHENTRIACTC to 32nd power; NHDOTRIACTC extends NHHENTRIACTC to 31st; NAASO=SO\u{00b5}\u{00b2} on S, 3rd-pass AA)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices56(sink: &ConsoleSink) {
    let (ntriactc, nhtriactc, naso, edge_count, node_count) =
        gos_runtime::graph_topo_indices56();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo56 (NTRIACTC + NHTRIACTC + NASO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTRIACTC (S-Triacontyl vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-triacontyl-vertex  NTRIACTC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b3}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTRIACTC (S-Nonacosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonacosic-edge     NHTRIACTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtriactc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NASO (S-Octatetracontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatetracontyl-s  NASO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, naso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTRIACTC extends NNONATC to 30th power; NHTRIACTC extends NHNONATC to 29th; NASO=SO\u{2074}\u{2078} on S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices55(sink: &ConsoleSink) {
    let (nnonatc, nhnonatc, nzso, edge_count, node_count) =
        gos_runtime::graph_topo_indices55();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo55 (NNONATC + NHNONATC + NZSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NNONATC (S-Nonacosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-nonacosic-vertex   NNONATC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nnonatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHNONATC (S-Octacosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octacosic-edge     NHNONATC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhnonatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NZSO (S-Hexatetracontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatetracontyl-s  NZSO    =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nzso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NNONATC extends NOCTATC to 29th power; NHNONATC extends NHOCTATC to 28th; NZSO=SO\u{2074}\u{2076} on S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices54(sink: &ConsoleSink) {
    let (noctatc, nhoctatc, nyso, edge_count, node_count) =
        gos_runtime::graph_topo_indices54();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo54 (NOCTATC + NHOCTATC + NYSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NOCTATC (S-Octacosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octacosic-vertex   NOCTATC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, noctatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHOCTATC (S-Heptacosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptacosic-edge    NHOCTATC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhoctatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NYSO (S-Tetratetracontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetratetracontyl-s NYSO    =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nyso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NOCTATC extends NHEPTATC to 28th power; NHOCTATC extends NHHEPTATC to 27th; NYSO=SO\u{2074}\u{2074} on S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices53(sink: &ConsoleSink) {
    let (nheptatc, nhheptatc, nxso, edge_count, node_count) =
        gos_runtime::graph_topo_indices53();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo53 (NHEPTATC + NHHEPTATC + NXSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEPTATC (S-Heptacosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heptacosic-vertex  NHEPTATC=  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nheptatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEPTATC (S-Hexacosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexacosic-edge     NHHEPTATC= ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhheptatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NXSO (S-Dotetracontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dotetracontyl-somb NXSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nxso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEPTATC extends NHEXATC to 27th power; NHHEPTATC extends NHHEXATC to 26th; NXSO=SO\u{2074}\u{00b2} on S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices52(sink: &ConsoleSink) {
    let (nhexatc, nhhexatc, nvso, edge_count, node_count) =
        gos_runtime::graph_topo_indices52();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo53 (NHEPTATC + NHHEPTATC + NXSO S-variant indices)\n");
    print_str(sink, " graph topo52 (NHEXATC + NHHEXATC + NVSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NHEXATC (S-Hexacosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexacosic-vertex   NHEXATC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, nhexatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHHEXATC (S-Pentacosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentacosic-edge    NHHEXATC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhhexatc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NVSO (S-Tetracontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetracontyl-somb   NVSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nvso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b2}\u{2070}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NHEXATC extends NPENTTC to 26th power; NHHEXATC extends NHPENTTC to 25th; NVSO=SO\u{2074}\u{2070} on S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices51(sink: &ConsoleSink) {
    let (npenttc, nhpenttc, nuso, edge_count, node_count) =
        gos_runtime::graph_topo_indices51();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo51 (NPENTTC + NHPENTTC + NUSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NPENTTC (S-Pentacosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-pentacosic-vertex  NPENTTC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, npenttc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{2075}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHPENTTC (S-Tetracosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetracosic-edge    NHPENTTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhpenttc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NUSO (S-Octatriacontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-octatriacont-somb  NUSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nuso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{2079}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NPENTTC extends NTETRTC to 25th power; NHPENTTC extends NHTETRTC to 24th; NUSO=SO\u{00b3}\u{2078} on S; T skipped)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices50(sink: &ConsoleSink) {
    let (ntetrtc, nhtetrtc, nsso, edge_count, node_count) =
        gos_runtime::graph_topo_indices50();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo53 (NHEPTATC + NHHEPTATC + NXSO S-variant indices)\n");
    print_str(sink, " graph topo52 (NHEXATC + NHHEXATC + NVSO S-variant indices)\n");
    print_str(sink, " graph topo51 (NPENTTC + NHPENTTC + NUSO S-variant indices)\n");
    print_str(sink, " graph topo50 (NTETRTC + NHTETRTC + NSSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTETRTC (S-Tetracosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetracosic-vertex  NTETRTC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntetrtc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{2074}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTETRTC (S-Tricosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tricosic-edge      NHTETRTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtetrtc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NSSO (S-Hexatriacontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-hexatriacont-somb  NSSO     =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nsso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{2078}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTETRTC extends NTRICTC to 24th power; NHTETRTC extends NHTRICTC to 23rd; NSSO=SO\u{00b3}\u{2076} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices49(sink: &ConsoleSink) {
    let (ntrictc, nhtrictc, nrso, edge_count, node_count) =
        gos_runtime::graph_topo_indices49();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo49 (NTRICTC + NHTRICTC + NRSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NTRICTC (S-Tricosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tricosic-vertex   NTRICTC =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ntrictc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{00b3}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHTRICTC (S-Docosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-docosic-edge      NHTRICTC=  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhtrictc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NRSO (S-Tetratriacontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-tetratriacont-somb NRSO    =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nrso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{2077}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NTRICTC extends NDOCTC to 23rd power; NHTRICTC extends NHDOCTC to 22nd; NRSO=SO\u{00b3}\u{2074} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

pub fn dispatch_graph_topo_indices48(sink: &ConsoleSink) {
    let (ndoctc, nhdoctc, nqso, edge_count, node_count) =
        gos_runtime::graph_topo_indices48();

    set_color(sink, 14, 0); // bright-yellow
    print_str(sink, " graph topo48 (NDOCTC + NHDOCTC + NQSO S-variant indices)\n");
    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);

    if node_count == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (empty graph)\n");
    } else {
        // NDOCTC (S-Docosic vertex sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-docosic-vertex    NDOCTC  =  ");
        set_color(sink, 11, 0); // bright-cyan
        print_num_inline(sink, ndoctc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_v S(v)\u{00b2}\u{00b2}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NHDOCTC (S-Heneicosic edge-sum)
        set_color(sink, 8, 0);
        print_str(sink, "  S-heneicosic-edge   NHDOCTC =  ");
        set_color(sink, 10, 0); // bright-green
        print_num_inline(sink, nhdoctc as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u+S_v)\u{00b2}\u{00b9}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");

        // NQSO (S-Dotriacontyl Sombor)
        set_color(sink, 8, 0);
        print_str(sink, "  S-dotriacontyl-somb NQSO    =  ");
        set_color(sink, 13, 0); // bright-magenta
        print_num_inline(sink, nqso as usize);
        set_color(sink, 8, 0);
        print_str(sink, "   [\u{03a3}_{uv\u{2208}E} (S_u\u{00b2}+S_v\u{00b2})\u{00b9}\u{2076}]  (exact)");
        set_color(sink, 7, 0);
        print_str(sink, "\n");
    }

    set_color(sink, 8, 0);
    print_str(sink, " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    set_color(sink, 7, 0);
    print_num_inline(sink, node_count);
    set_color(sink, 8, 0);
    print_str(sink, " node(s)  ");
    print_num_inline(sink, edge_count);
    print_str(sink, " edge(s)  (S-variant family: NDOCTC extends NHENTC to 22nd power; NHDOCTC extends NHHENTC to 21st; NQSO=SO\u{00b3}\u{00b2} applied to S)");
    set_color(sink, 7, 0);
    print_str(sink, "\n");
}

/// V2.28: `uname` — kernel version and capacity limits.
/// Analogous to `uname -a` + `sysctl kern.*` on Linux/BSD.
/// Shows GOS version, ABI, capacity limits, and queue/ring depths.
pub fn dispatch_uname(sink: &ConsoleSink) {
    let cap = gos_runtime::runtime_capacity();
    let snapshot = gos_runtime::snapshot();
    set_color(sink, 11, 0);
    print_str(sink, " kernel info\n");
    set_color(sink, 7, 0);
    print_str(sink, "  GOS v2.28 (graph-kernel)  abi: ");
    print_num_inline(sink, cap.abi_major as usize);
    print_str(sink, ".");
    print_num_inline(sink, cap.abi_minor as usize);
    print_str(sink, ".");
    print_num_inline(sink, cap.abi_patch as usize);
    print_str(sink, "  protocol: ");
    print_num_inline(sink, cap.protocol_version as usize);
    print_str(sink, "\n  capacity");
    set_color(sink, 11, 0);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
    print_str(sink, "    nodes:          ");
    print_num_inline(sink, snapshot.node_count);
    print_str(sink, " / ");
    print_num_inline(sink, cap.max_nodes);
    print_str(sink, "\n    edges:          ");
    print_num_inline(sink, snapshot.edge_count);
    print_str(sink, " / ");
    print_num_inline(sink, cap.max_edges);
    print_str(sink, "\n    plugins:        ");
    print_num_inline(sink, snapshot.plugin_count);
    print_str(sink, " / ");
    print_num_inline(sink, cap.max_plugins);
    print_str(sink, "\n    ready-queue:    ");
    print_num_inline(sink, cap.max_ready_queue);
    print_str(sink, "  signal-queue: ");
    print_num_inline(sink, cap.max_signal_queue);
    print_str(sink, "  fault-queue: ");
    print_num_inline(sink, cap.max_fault_queue);
    print_str(sink, "\n    diff-ring:      ");
    print_num_inline(sink, cap.max_diff_ring);
    print_str(sink, "  subscribe-pairs: ");
    print_num_inline(sink, cap.max_subscribe_pairs);
    print_str(sink, "\n    node-trace:     ");
    print_num_inline(sink, cap.max_node_trace);
    print_str(sink, " (ring depth per node)");
    print_str(sink, "\n    node-log:       ");
    print_num_inline(sink, cap.max_node_log);
    print_str(sink, " (ring depth per node)");
    print_str(sink, "\n  arch: x86_64  no_std  tick: ");
    print_num_inline(sink, snapshot.tick as usize);
    print_str(sink, "\n");
}

/// Right-align a number in 4 columns (spaces then digits).
fn print_num_right4(sink: &ConsoleSink, n: usize) {
    if n >= 1000 {
        print_num_inline(sink, n);
    } else if n >= 100 {
        print_str(sink, " ");
        print_num_inline(sink, n);
    } else if n >= 10 {
        print_str(sink, "  ");
        print_num_inline(sink, n);
    } else {
        print_str(sink, "   ");
        print_num_inline(sink, n);
    }
}

/// Right-align a number in 6 columns (spaces then digits).
fn print_num_right6(sink: &ConsoleSink, n: usize) {
    if n >= 100_000 {
        print_num_inline(sink, n);
    } else if n >= 10_000 {
        print_str(sink, " ");
        print_num_inline(sink, n);
    } else if n >= 1_000 {
        print_str(sink, "  ");
        print_num_inline(sink, n);
    } else if n >= 100 {
        print_str(sink, "   ");
        print_num_inline(sink, n);
    } else if n >= 10 {
        print_str(sink, "    ");
        print_num_inline(sink, n);
    } else {
        print_str(sink, "     ");
        print_num_inline(sink, n);
    }
}

fn node_lifecycle_label(lc: gos_protocol::NodeLifecycle) -> &'static str {
    match lc {
        gos_protocol::NodeLifecycle::Discovered  => "discovered",
        gos_protocol::NodeLifecycle::Loaded      => "loaded",
        gos_protocol::NodeLifecycle::Registered  => "registered",
        gos_protocol::NodeLifecycle::Allocated   => "allocated",
        gos_protocol::NodeLifecycle::Ready       => "ready",
        gos_protocol::NodeLifecycle::Running     => "running",
        gos_protocol::NodeLifecycle::Waiting     => "waiting",
        gos_protocol::NodeLifecycle::Suspended   => "suspended",
        gos_protocol::NodeLifecycle::Faulted     => "faulted",
        gos_protocol::NodeLifecycle::Terminated  => "terminated",
    }
}

fn module_lifecycle_label(state: gos_protocol::ModuleLifecycle) -> &'static str {
    match state {
        gos_protocol::ModuleLifecycle::Installed => "installed",
        gos_protocol::ModuleLifecycle::Validated => "validated",
        gos_protocol::ModuleLifecycle::Mapped => "mapped",
        gos_protocol::ModuleLifecycle::Instantiated => "instantiated",
        gos_protocol::ModuleLifecycle::Running => "running",
        gos_protocol::ModuleLifecycle::Quiescing => "quiescing",
        gos_protocol::ModuleLifecycle::Stopped => "stopped",
        gos_protocol::ModuleLifecycle::Faulted => "faulted",
    }
}

/// List all registered plugins — `lsmod`-style inventory.
/// Shows name, version, load state, and node count per plugin.
pub fn dispatch_plugin_list(sink: &ConsoleSink) {
    use gos_protocol::PluginSummary;
    const PAGE: usize = 32;
    let mut summaries = [PluginSummary::EMPTY; PAGE];
    let (total, filled) = gos_runtime::plugin_page::<PAGE>(0, &mut summaries);

    set_color(sink, 11, 0);
    print_str(sink, " plugins");
    set_color(sink, 8, 0);
    print_str(sink, "  name                 ver   state       nodes\n");
    set_color(sink, 7, 0);

    for summary in summaries.iter().take(filled) {
        let fg: u8 = match summary.state {
            gos_protocol::PluginState::Loaded     => 10,
            gos_protocol::PluginState::Faulted    => 12,
            gos_protocol::PluginState::Discovered => 8,
        };
        set_color(sink, fg, 0);
        print_str(sink, "  ");
        let name = summary.name;
        print_str(sink, name);
        let pad = 22usize.saturating_sub(name.len());
        for _ in 0..pad { print_str(sink, " "); }
        print_num_right4(sink, summary.version as usize);
        print_str(sink, "  ");
        let state_str = summary.state.as_str();
        print_str(sink, state_str);
        let state_pad = 12usize.saturating_sub(state_str.len());
        for _ in 0..state_pad { print_str(sink, " "); }
        print_num_right4(sink, summary.node_count);
        print_str(sink, "\n");
    }

    if filled == 0 {
        set_color(sink, 8, 0);
        print_str(sink, "  (no plugins registered)\n");
    }
    set_color(sink, 8, 0);
    print_str(sink, "  total: ");
    print_num_inline(sink, total);
    print_str(sink, " plugin(s)\n");
    set_color(sink, 7, 0);
}

fn module_fault_policy_label(policy: gos_protocol::ModuleFaultPolicy) -> &'static str {
    match policy {
        gos_protocol::ModuleFaultPolicy::FaultKernelDegraded => "kernel-degrade",
        gos_protocol::ModuleFaultPolicy::Restart => "restart",
        gos_protocol::ModuleFaultPolicy::RestartAlways => "restart-always",
        gos_protocol::ModuleFaultPolicy::Manual => "manual",
    }
}

fn instance_lifecycle_label(state: gos_protocol::NodeInstanceLifecycle) -> &'static str {
    match state {
        gos_protocol::NodeInstanceLifecycle::Allocated => "allocated",
        gos_protocol::NodeInstanceLifecycle::Ready => "ready",
        gos_protocol::NodeInstanceLifecycle::Running => "running",
        gos_protocol::NodeInstanceLifecycle::WaitingClaim => "waiting-claim",
        gos_protocol::NodeInstanceLifecycle::Suspended => "suspended",
        gos_protocol::NodeInstanceLifecycle::Stopped => "stopped",
        gos_protocol::NodeInstanceLifecycle::Faulted => "faulted",
    }
}

fn entry_policy_label(policy: gos_protocol::EntryPolicy) -> &'static str {
    match policy {
        gos_protocol::EntryPolicy::Manual => "manual",
        gos_protocol::EntryPolicy::Bootstrap => "bootstrap",
        gos_protocol::EntryPolicy::OnDemand => "ondemand",
        gos_protocol::EntryPolicy::Background => "bg",
    }
}

fn edge_type_label(edge_type: gos_protocol::RuntimeEdgeType) -> &'static str {
    match edge_type {
        gos_protocol::RuntimeEdgeType::Call => "call",
        gos_protocol::RuntimeEdgeType::Spawn => "spawn",
        gos_protocol::RuntimeEdgeType::Depend => "depend",
        gos_protocol::RuntimeEdgeType::Signal => "signal",
        gos_protocol::RuntimeEdgeType::Return => "return",
        gos_protocol::RuntimeEdgeType::Mount => "mount",
        gos_protocol::RuntimeEdgeType::Sync => "sync",
        gos_protocol::RuntimeEdgeType::Stream => "stream",
        gos_protocol::RuntimeEdgeType::Use => "use",
        gos_protocol::RuntimeEdgeType::Link => "link",
    }
}

fn edge_direction_label(direction: GraphEdgeDirection) -> &'static str {
    match direction {
        GraphEdgeDirection::Outbound => "OUT",
        GraphEdgeDirection::Inbound => "IN ",
    }
}

fn graph_mode_label(mode: u8) -> &'static str {
    match mode {
        GRAPH_MODE_OVERVIEW => "overview",
        GRAPH_MODE_NODE_LIST => "nodes",
        GRAPH_MODE_EDGE_LIST => "edges",
        GRAPH_MODE_NODE_DETAIL => "node",
        GRAPH_MODE_EDGE_DETAIL => "edge",
        GRAPH_MODE_INFO => "graph",
        _ => "command",
    }
}

fn graph_context_label(context: u8) -> &'static str {
    match context {
        GRAPH_CTX_OVERVIEW => "overview",
        GRAPH_CTX_NODE => "node",
        GRAPH_CTX_EDGE => "edge",
        GRAPH_CTX_METRICS => "metrics",
        _ => "none",
    }
}

fn ai_panel_byte(byte: u8) -> u8 {
    if byte.is_ascii_graphic() || byte == b' ' {
        byte
    } else if byte >= 0x80 {
        b'#'
    } else {
        b' '
    }
}

fn push_ai_line(state: &mut ShellState, bytes: &[u8]) {
    for idx in 1..AI_PANEL_LINES {
        state.ai_lines[idx - 1] = state.ai_lines[idx];
        state.ai_line_lens[idx - 1] = state.ai_line_lens[idx];
    }

    let mut line = [0u8; AI_PANEL_LINE_WIDTH];
    let mut len = 0usize;
    for byte in bytes.iter().copied().take(AI_PANEL_LINE_WIDTH) {
        line[len] = ai_panel_byte(byte);
        len += 1;
    }

    state.ai_lines[AI_PANEL_LINES - 1] = line;
    state.ai_line_lens[AI_PANEL_LINES - 1] = len as u8;
}

fn push_ai_text(state: &mut ShellState, text: &str) {
    push_ai_line(state, text.as_bytes());
}

fn flush_ai_stream(state: &mut ShellState) {
    let len = state.ai_stream_len as usize;
    if len == 0 {
        return;
    }

    let mut line = [0u8; AI_PANEL_LINE_WIDTH];
    for (idx, byte) in state.ai_stream[..len].iter().enumerate() {
        line[idx] = *byte;
    }
    push_ai_line(state, &line[..len]);
    state.ai_stream = [0; AI_PANEL_LINE_WIDTH];
    state.ai_stream_len = 0;
}

fn append_ai_stream_byte(state: &mut ShellState, byte: u8) {
    if byte == b'\r' {
        return;
    }
    if byte == b'\n' {
        flush_ai_stream(state);
        return;
    }

    let len = state.ai_stream_len as usize;
    if len < AI_PANEL_LINE_WIDTH {
        state.ai_stream[len] = ai_panel_byte(byte);
        state.ai_stream_len += 1;
    }
}

fn seed_ai_panel(state: &mut ShellState) {
    clear_ai_panel(state);
    push_ai_text(state, "sys> ai control online");
    push_ai_text(state, "sys> ask <text> to steer");
    push_ai_text(state, "sys> ^A adds api key");
}

fn ime_mode_label(lang: u8) -> &'static str {
    if lang == IME_MODE_ZH_PINYIN {
        "zh-py"
    } else {
        "en-us"
    }
}

fn sync_input_lang(sink: &ConsoleSink, state: &mut ShellState, lang: u8) -> bool {
    if !emit_target_signal(
        sink,
        state.ime_target,
        Signal::Control {
            cmd: IME_CONTROL_SET_MODE,
            val: lang,
        },
    ) {
        return false;
    }

    state.input_lang = lang;
    clear_ime_preview(state);
    true
}

fn commit_ime_preview(sink: &ConsoleSink, state: &mut ShellState, selector: u8) {
    if state.ime_preview_len == 0 {
        return;
    }
    let _ = emit_target_signal(
        sink,
        state.ime_target,
        Signal::Data {
            from: sink.from,
            byte: selector,
        },
    );
    clear_ime_preview(state);
}

fn is_ascii_punctuation(byte: u8) -> bool {
    matches!(
        byte,
        b'.' | b',' | b';' | b':' | b'!' | b'?' | b'(' | b')' | b'[' | b']' | b'{' | b'}'
            | b'"' | b'\'' | b'-' | b'_' | b'/' | b'\\' | b'@' | b'#' | b'$' | b'%'
            | b'^' | b'&' | b'*' | b'+' | b'='
    )
}

fn set_color(sink: &ConsoleSink, fg: u8, bg: u8) {
    send_ctrl(sink, 1, fg);
    send_ctrl(sink, 2, bg);
}

fn draw_byte(sink: &ConsoleSink, row: usize, col: usize, fg: u8, bg: u8, byte: u8) {
    set_color(sink, fg, bg);
    goto(sink, row, col);
    print_byte(sink, byte);
}

fn draw_bytes(sink: &ConsoleSink, row: usize, col: usize, fg: u8, bg: u8, bytes: &[u8]) {
    set_color(sink, fg, bg);
    goto(sink, row, col);
    for byte in bytes {
        print_byte(sink, *byte);
    }
}

fn draw_text(sink: &ConsoleSink, row: usize, col: usize, fg: u8, bg: u8, text: &str) {
    set_color(sink, fg, bg);
    goto(sink, row, col);
    print_str(sink, text);
}

fn draw_center(sink: &ConsoleSink, row: usize, fg: u8, bg: u8, text: &str) {
    let col = if text.len() >= SCREEN_WIDTH {
        0
    } else {
        (SCREEN_WIDTH - text.len()) / 2
    };
    draw_text(sink, row, col, fg, bg, text);
}

fn draw_repeat(sink: &ConsoleSink, row: usize, col: usize, fg: u8, bg: u8, ch: u8, count: usize) {
    set_color(sink, fg, bg);
    goto(sink, row, col);
    for _ in 0..count {
        print_byte(sink, ch);
    }
}

fn fill_band(sink: &ConsoleSink, row: usize, left: usize, width: usize, fg: u8, bg: u8) {
    draw_repeat(sink, row, left, fg, bg, b' ', width);
}

#[allow(clippy::too_many_arguments)]
fn draw_box(
    sink: &ConsoleSink,
    top: usize,
    left: usize,
    width: usize,
    height: usize,
    title: &str,
    fg: u8,
    bg: u8,
) {
    if width < 2 || height < 2 {
        return;
    }

    draw_byte(sink, top, left, fg, bg, CP437_TL);
    draw_repeat(sink, top, left + 1, fg, bg, CP437_HLINE, width - 2);
    draw_byte(sink, top, left + width - 1, fg, bg, CP437_TR);

    for row in top + 1..top + height - 1 {
        draw_byte(sink, row, left, fg, bg, CP437_VLINE);
        draw_repeat(sink, row, left + 1, fg, bg, b' ', width - 2);
        draw_byte(sink, row, left + width - 1, fg, bg, CP437_VLINE);
    }

    draw_byte(sink, top + height - 1, left, fg, bg, CP437_BL);
    draw_repeat(sink, top + height - 1, left + 1, fg, bg, CP437_HLINE, width - 2);
    draw_byte(sink, top + height - 1, left + width - 1, fg, bg, CP437_BR);

    if !title.is_empty() && width > title.len() + 4 {
        draw_text(sink, top, left + 2, WABI_PAPER, bg, title);
    }
}

fn draw_badge(sink: &ConsoleSink, row: usize, col: usize, fg: u8, bg: u8, text: &str) {
    let width = text.len() + 2;
    fill_band(sink, row, col, width, fg, bg);
    draw_text(sink, row, col + 1, fg, bg, text);
}

fn draw_runtime_header(sink: &ConsoleSink, state: &ShellState, snapshot: gos_protocol::GraphSnapshot) {
    let phase = (state.sigil_frame as usize) / 2;
    let pulse_col = 38 + ((phase * 2) % 14);
    let mode_label = if state.menu_mode == MENU_MODE_AI_API {
        "api"
    } else {
        graph_mode_label(state.graph_mode)
    };

    fill_band(sink, 0, 0, SCREEN_WIDTH, WABI_INK, WABI_INDIGO);
    fill_band(sink, 1, 0, SCREEN_WIDTH, WABI_INK, WABI_INK);

    draw_badge(sink, 0, 2, WABI_MOON, WABI_TEA, "GOS v0.2");
    draw_text(sink, 0, 14, WABI_PAPER, WABI_INDIGO, "VECTOR MESH TERMINAL");
    draw_repeat(sink, 0, 37, WABI_STONE, WABI_INDIGO, CP437_LIGHT, 18);
    draw_repeat(
        sink,
        0,
        pulse_col,
        if gos_runtime::is_stable() {
            WABI_SAGE
        } else {
            WABI_TEA
        },
        WABI_INDIGO,
        CP437_MEDIUM,
        2,
    );
    draw_text(sink, 0, 58, WABI_STONE, WABI_INDIGO, "mode");
    draw_badge(
        sink,
        0,
        63,
        WABI_MOON,
        if state.menu_mode == MENU_MODE_AI_API {
            WABI_INDIGO
        } else {
            WABI_STONE
        },
        mode_label,
    );
    draw_badge(
        sink,
        0,
        74,
        WABI_MOON,
        if gos_runtime::is_stable() {
            WABI_MOSS
        } else {
            WABI_TEA
        },
        if gos_runtime::is_stable() { "SYNC" } else { "LIVE" },
    );

    draw_text(sink, 1, 2, WABI_STONE, WABI_INK, "mesh");
    let mut mesh = LineBuf::<24>::new();
    mesh.push_byte(b'p');
    mesh.push_dec(snapshot.plugin_count as u64);
    mesh.push_str(" n");
    mesh.push_dec(snapshot.node_count as u64);
    mesh.push_str(" e");
    mesh.push_dec(snapshot.edge_count as u64);
    mesh.push_str(" rq");
    mesh.push_dec(snapshot.ready_queue_len as u64);
    draw_linebuf(sink, 1, 7, WABI_PAPER, WABI_INK, &mesh);

    draw_repeat(sink, 1, 29, WABI_STONE, WABI_INK, CP437_LIGHT, 17);
    draw_repeat(sink, 1, 30 + ((phase * 2) % 12), WABI_TEA, WABI_INK, CP437_MEDIUM, 2);
    draw_byte(sink, 1, 45 + (phase % 3), WABI_STONE, WABI_INK, b'.');

    draw_badge(
        sink,
        1,
        50,
        if state.ai_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.ai_target == 0 {
            WABI_INK
        } else {
            WABI_INDIGO
        },
        "AI",
    );
    draw_badge(
        sink,
        1,
        56,
        if state.cypher_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cypher_target == 0 {
            WABI_INK
        } else {
            WABI_STONE
        },
        "CY",
    );
    draw_badge(
        sink,
        1,
        62,
        if state.net_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.net_target == 0 {
            WABI_INK
        } else {
            WABI_MOSS
        },
        "NET",
    );
    draw_badge(
        sink,
        1,
        69,
        if state.cuda_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cuda_target == 0 {
            WABI_INK
        } else {
            WABI_TEA
        },
        "CUDA",
    );
}

fn draw_runtime_gap_flux(sink: &ConsoleSink, state: &ShellState) {
    let phase = (state.sigil_frame as usize) / 2;
    clear_rect(sink, 2, 49, 2, 12);
    for idx in 0..12 {
        let row = 2 + idx;
        let fg = match (idx + phase) % 3 {
            0 => WABI_STONE,
            1 => WABI_PAPER,
            _ => WABI_TEA,
        };
        let glyph = match (idx + phase) % 3 {
            0 => CP437_LIGHT,
            1 => CP437_MEDIUM,
            _ => CP437_DARK,
        };
        let col = 49 + ((idx + phase) % 2);
        draw_byte(sink, row, col, fg, WABI_INK, glyph);
        if (idx + phase).is_multiple_of(4) {
            draw_byte(sink, row, 50, WABI_STONE, WABI_INK, b'.');
        }
    }
    draw_byte(sink, 3 + (phase % 8), 50, WABI_PAPER, WABI_INK, b'.');
}

/// Render the VECTOR DECK panel in live proc watch mode (like `htop` or `watch -n1 proc`).
///
/// Shows the top 6 nodes sorted by vector address with cumulative signal count,
/// outbound edge count, and lifecycle state.  Updated on every heartbeat tick while
/// WATCH_PROC_MODE is set, giving a continuously-refreshing view without threads.
fn draw_watch_proc_panel(sink: &ConsoleSink, snapshot: gos_protocol::GraphSnapshot) {
    use gos_protocol::NodeProcSummary;
    const WATCH_PAGE: usize = 6;
    let mut summaries = [NodeProcSummary::EMPTY; WATCH_PAGE];
    let (total, filled) = gos_runtime::proc_page::<WATCH_PAGE>(0, &mut summaries);

    draw_box(
        sink,
        COMMAND_DECK_TOP,
        COMMAND_DECK_LEFT,
        COMMAND_DECK_WIDTH,
        COMMAND_DECK_HEIGHT,
        " PROC WATCH ",
        WABI_TEA,
        WABI_INK,
    );

    // Row 3 — status line: tick + nodes + hint
    fill_band(sink, 3, COMMAND_DECK_LEFT + 1, COMMAND_DECK_WIDTH - 2, WABI_INK, WABI_INK);
    draw_text(sink, 3, 4, WABI_STONE, WABI_INK, "tick ");
    draw_usize(sink, 3, 9, WABI_MOON, WABI_INK, snapshot.tick as usize);
    draw_text(sink, 3, 20, WABI_STONE, WABI_INK, "nodes ");
    draw_usize(sink, 3, 26, WABI_PAPER, WABI_INK, total);
    draw_text(sink, 3, 33, WABI_STONE, WABI_INK, "any key stops");

    // Row 4 — column header
    fill_band(sink, 4, COMMAND_DECK_LEFT + 1, COMMAND_DECK_WIDTH - 2, WABI_INK, WABI_INK);
    draw_text(sink, 4, 4, WABI_STONE, WABI_INK, "vector           sig  out  lifecycle");

    // Rows 5-10 — node rows (up to 6)
    for i in 0..WATCH_PAGE {
        let row = 5 + i;
        fill_band(sink, row, COMMAND_DECK_LEFT + 1, COMMAND_DECK_WIDTH - 2, WABI_INK, WABI_INK);
        if i < filled {
            let summary = &summaries[i];
            let fg: u8 = match summary.lifecycle {
                gos_protocol::NodeLifecycle::Running   => WABI_SAGE,
                gos_protocol::NodeLifecycle::Faulted   => 12,
                gos_protocol::NodeLifecycle::Suspended => WABI_TEA,
                _                                      => WABI_STONE,
            };
            set_color(sink, fg, WABI_INK);
            goto(sink, row, 4);
            let mut vec_buf = LineBuf::<16>::new();
            vec_buf.push_vector(summary.vector);
            let vec_str = core::str::from_utf8(vec_buf.as_slice()).unwrap_or("?");
            print_str(sink, vec_str);
            let pad = 16usize.saturating_sub(vec_str.len());
            for _ in 0..pad { print_byte(sink, b' '); }
            set_color(sink, WABI_MOON, WABI_INK);
            print_num_right4(sink, summary.signal_count as usize);
            set_color(sink, WABI_STONE, WABI_INK);
            print_str(sink, " ");
            print_num_right4(sink, summary.edge_out_count as usize);
            set_color(sink, fg, WABI_INK);
            print_str(sink, "  ");
            print_str(sink, node_lifecycle_label(summary.lifecycle));
        } else if i == filled && total > WATCH_PAGE {
            set_color(sink, WABI_STONE, WABI_INK);
            goto(sink, row, 4);
            print_str(sink, "...");
            print_num_inline(sink, total - WATCH_PAGE);
            print_str(sink, " more");
        }
    }
}

fn draw_command_deck_panel(
    sink: &ConsoleSink,
    state: &ShellState,
    snapshot: gos_protocol::GraphSnapshot,
) {
    if WATCH_PROC_MODE.load(Ordering::SeqCst) != 0 {
        draw_watch_proc_panel(sink, snapshot);
        return;
    }
    let phase = (state.sigil_frame as usize) / 2;
    draw_box(
        sink,
        COMMAND_DECK_TOP,
        COMMAND_DECK_LEFT,
        COMMAND_DECK_WIDTH,
        COMMAND_DECK_HEIGHT,
        " VECTOR DECK ",
        WABI_PAPER,
        WABI_INK,
    );
    draw_text(sink, 3, 4, WABI_STONE, WABI_INK, "graph-native shell // quiet vector core");
    draw_text(sink, 4, 4, WABI_STONE, WABI_INK, "plugins");
    draw_usize(sink, 4, 12, WABI_MOON, WABI_INK, snapshot.plugin_count);
    draw_text(sink, 4, 18, WABI_STONE, WABI_INK, "nodes");
    draw_usize(sink, 4, 24, WABI_PAPER, WABI_INK, snapshot.node_count);
    draw_text(sink, 4, 30, WABI_STONE, WABI_INK, "edges");
    draw_usize(sink, 4, 36, WABI_TEA, WABI_INK, snapshot.edge_count);

    draw_text(sink, 5, 4, WABI_STONE, WABI_INK, "stability");
    draw_badge(
        sink,
        5,
        14,
        WABI_MOON,
        if gos_runtime::is_stable() {
            WABI_MOSS
        } else {
            WABI_TEA
        },
        if gos_runtime::is_stable() { "locked" } else { "surge" },
    );
    draw_text(sink, 5, 24, WABI_STONE, WABI_INK, "focus");
    draw_badge(
        sink,
        5,
        31,
        WABI_MOON,
        if state.graph_context == GRAPH_CTX_NONE {
            WABI_STONE
        } else {
            WABI_INDIGO
        },
        graph_context_label(state.graph_context),
    );

    draw_text(sink, 6, 4, WABI_STONE, WABI_INK, "rq");
    draw_meter(
        sink,
        6,
        8,
        11,
        (snapshot.ready_queue_len * 2).min(11),
        WABI_PAPER,
        WABI_INK,
    );
    draw_text(sink, 6, 22, WABI_STONE, WABI_INK, "sig");
    draw_meter(
        sink,
        6,
        27,
        11,
        ((snapshot.signal_queue_len + snapshot.control_queue_len) * 2).min(11),
        WABI_TEA,
        WABI_INK,
    );

    draw_text(sink, 7, 4, WABI_STONE, WABI_INK, "quick");
    draw_badge(sink, 7, 10, WABI_MOON, WABI_STONE, "show");
    draw_badge(sink, 7, 17, WABI_MOON, WABI_INDIGO, "node");
    draw_badge(sink, 7, 24, WABI_MOON, WABI_TEA, "edge");
    draw_badge(sink, 7, 31, WABI_MOON, WABI_MOSS, "back");
    draw_badge(sink, 7, 38, WABI_MOON, WABI_STONE, "where");
    draw_badge(sink, 7, 46, WABI_MOON, WABI_STONE, "metrics");

    draw_text(sink, 8, 4, WABI_STONE, WABI_INK, "query");
    draw_text(sink, 8, 11, WABI_PAPER, WABI_INK, "cypher MATCH ...");
    draw_text(sink, 9, 4, WABI_STONE, WABI_INK, "lanes");
    draw_badge(
        sink,
        9,
        11,
        if state.ai_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.ai_target == 0 { WABI_INK } else { WABI_INDIGO },
        "AI",
    );
    draw_badge(
        sink,
        9,
        17,
        if state.cuda_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cuda_target == 0 { WABI_INK } else { WABI_TEA },
        "CUDA",
    );
    draw_badge(
        sink,
        9,
        25,
        if state.net_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.net_target == 0 { WABI_INK } else { WABI_MOSS },
        "NET",
    );
    draw_text(sink, 9, 32, WABI_STONE, WABI_INK, "ask / submit / probe");

    draw_text(sink, 10, 4, WABI_STONE, WABI_INK, "vector weave");
    draw_repeat(sink, 10, 18, WABI_STONE, WABI_INK, CP437_LIGHT, 22);
    draw_repeat(sink, 10, 18 + ((phase * 2) % 18), WABI_TEA, WABI_INK, CP437_MEDIUM, 2);
    draw_byte(sink, 10, 41, WABI_PAPER, WABI_INK, b'.');
}

fn draw_operator_band(
    sink: &ConsoleSink,
    state: &ShellState,
    snapshot: gos_protocol::GraphSnapshot,
) {
    let phase = (state.sigil_frame as usize) / 2;
    fill_band(sink, 12, 2, 47, WABI_INK, WABI_INK);
    fill_band(sink, 13, 2, 47, WABI_INK, WABI_INK);

    draw_text(sink, 12, 4, WABI_STONE, WABI_INK, "operator");
    draw_badge(sink, 12, 13, WABI_MOON, WABI_STONE, "deck");
    draw_badge(
        sink,
        12,
        20,
        if state.ai_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.ai_target == 0 { WABI_INK } else { WABI_INDIGO },
        "ai",
    );
    draw_badge(
        sink,
        12,
        25,
        if state.cuda_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cuda_target == 0 { WABI_INK } else { WABI_TEA },
        "cu",
    );
    draw_badge(
        sink,
        12,
        30,
        if state.net_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.net_target == 0 { WABI_INK } else { WABI_MOSS },
        "net",
    );
    draw_repeat(sink, 12, 36, WABI_STONE, WABI_INK, CP437_LIGHT, 11);
    draw_repeat(sink, 12, 37 + ((phase * 2) % 7), WABI_PAPER, WABI_INK, CP437_MEDIUM, 2);
    draw_byte(sink, 12, 47, WABI_STONE, WABI_INK, b'.');

    draw_text(sink, 13, 4, WABI_STONE, WABI_INK, "route");
    let mut route = LineBuf::<34>::new();
    route.push_str(graph_mode_label(state.graph_mode));
    route.push_str(" :: ");
    route.push_str(graph_context_label(state.graph_context));
    route.push_str(" :: rq ");
    route.push_dec(snapshot.ready_queue_len as u64);
    route.push_str(" / sg ");
    route.push_dec(snapshot.signal_queue_len as u64);
    if snapshot.control_queue_len > 0 {
        route.push_str("+");
        route.push_dec(snapshot.control_queue_len as u64);
    }
    draw_linebuf(sink, 13, 11, WABI_PAPER, WABI_INK, &route);
}

fn draw_ai_panel(sink: &ConsoleSink, state: &ShellState) {
    let snapshot = gos_runtime::snapshot();
    let phase = (state.sigil_frame as usize) / 2;
    draw_box(
        sink,
        AI_PANEL_TOP,
        AI_PANEL_LEFT,
        AI_PANEL_WIDTH,
        AI_PANEL_HEIGHT,
        " AI CONTROL ",
        WABI_PAPER,
        WABI_INK,
    );
    draw_badge(sink, AI_PANEL_TOP + 1, AI_PANEL_LEFT + 2, WABI_MOON, WABI_TEA, "NEURAL");
    draw_repeat(sink, AI_PANEL_TOP + 1, AI_PANEL_LEFT + 12, WABI_STONE, WABI_INK, CP437_LIGHT, 10);
    draw_repeat(
        sink,
        AI_PANEL_TOP + 1,
        AI_PANEL_LEFT + 12 + ((phase * 2) % 8),
        WABI_PAPER,
        WABI_INK,
        CP437_MEDIUM,
        2,
    );
    draw_byte(sink, AI_PANEL_TOP + 1, AI_PANEL_LEFT + 22, WABI_STONE, WABI_INK, b'.');

    draw_text(sink, AI_PANEL_TOP + 2, AI_PANEL_LEFT + 2, WABI_STONE, WABI_INK, "link");
    draw_badge(
        sink,
        AI_PANEL_TOP + 2,
        AI_PANEL_LEFT + 7,
        if state.ai_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.ai_target == 0 { WABI_INK } else { WABI_INDIGO },
        if state.ai_target == 0 { "down" } else { "live" },
    );
    draw_text(sink, AI_PANEL_TOP + 2, AI_PANEL_LEFT + 15, WABI_STONE, WABI_INK, "api");
    draw_badge(
        sink,
        AI_PANEL_TOP + 2,
        AI_PANEL_LEFT + 19,
        if state.api_configured != 0 { WABI_MOON } else { WABI_STONE },
        if state.api_configured != 0 { WABI_TEA } else { WABI_INK },
        if state.api_configured != 0 { "key" } else { "void" },
    );

    draw_text(sink, AI_PANEL_TOP + 3, AI_PANEL_LEFT + 2, WABI_STONE, WABI_INK, "mesh");
    let mut mesh = LineBuf::<16>::new();
    mesh.push_byte(b'p');
    mesh.push_dec(snapshot.plugin_count as u64);
    mesh.push_str(" n");
    mesh.push_dec(snapshot.node_count as u64);
    mesh.push_str(" e");
    mesh.push_dec(snapshot.edge_count as u64);
    draw_linebuf(sink, AI_PANEL_TOP + 3, AI_PANEL_LEFT + 7, WABI_PAPER, WABI_INK, &mesh);

    draw_text(sink, AI_PANEL_TOP + 4, AI_PANEL_LEFT + 2, WABI_STONE, WABI_INK, "rq");
    draw_meter(
        sink,
        AI_PANEL_TOP + 4,
        AI_PANEL_LEFT + 5,
        6,
        (snapshot.ready_queue_len * 2).min(6),
        WABI_PAPER,
        WABI_INK,
    );
    draw_text(sink, AI_PANEL_TOP + 4, AI_PANEL_LEFT + 13, WABI_STONE, WABI_INK, "sg");
    draw_meter(
        sink,
        AI_PANEL_TOP + 4,
        AI_PANEL_LEFT + 16,
        6,
        ((snapshot.signal_queue_len + snapshot.control_queue_len) * 2).min(6),
        WABI_TEA,
        WABI_INK,
    );

    draw_text(sink, AI_PANEL_TOP + 5, AI_PANEL_LEFT + 2, WABI_STONE, WABI_INK, "focus");
    draw_badge(
        sink,
        AI_PANEL_TOP + 5,
        AI_PANEL_LEFT + 8,
        WABI_MOON,
        if state.graph_context == GRAPH_CTX_NONE {
            WABI_STONE
        } else {
            WABI_INDIGO
        },
        graph_context_label(state.graph_context),
    );
    let focus_label = if state.selected_node.is_some() {
        "N"
    } else if state.selected_edge.is_some() {
        "E"
    } else {
        "-"
    };
    draw_badge(sink, AI_PANEL_TOP + 5, AI_PANEL_LEFT + 19, WABI_MOON, WABI_TEA, focus_label);

    draw_text(sink, AI_PANEL_TOP + 6, AI_PANEL_LEFT + 2, WABI_STONE, WABI_INK, "ops");
    draw_badge(sink, AI_PANEL_TOP + 6, AI_PANEL_LEFT + 6, WABI_MOON, WABI_MOSS, "ask");
    draw_badge(
        sink,
        AI_PANEL_TOP + 6,
        AI_PANEL_LEFT + 12,
        if state.cypher_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cypher_target == 0 { WABI_INK } else { WABI_STONE },
        "cy",
    );
    draw_badge(
        sink,
        AI_PANEL_TOP + 6,
        AI_PANEL_LEFT + 17,
        if state.cuda_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cuda_target == 0 { WABI_INK } else { WABI_TEA },
        "cu",
    );
    draw_repeat(sink, AI_PANEL_TOP + 6, AI_PANEL_LEFT + 20, WABI_STONE, WABI_INK, CP437_LIGHT, 4);
    draw_repeat(
        sink,
        AI_PANEL_TOP + 6,
        AI_PANEL_LEFT + 20 + (phase % 3),
        WABI_PAPER,
        WABI_INK,
        CP437_MEDIUM,
        2,
    );
    draw_byte(sink, AI_PANEL_TOP + 6, AI_PANEL_LEFT + 23, WABI_STONE, WABI_INK, b'.');

    for row in 0..AI_PANEL_LINES {
        let line_row = AI_PANEL_TOP + 7 + row;
        fill_band(sink, line_row, AI_PANEL_LEFT + 2, AI_PANEL_WIDTH - 4, 0, 0);
        let len = state.ai_line_lens[row] as usize;
        if len == 0 {
            continue;
        }

        let fg = if len >= 4
            && state.ai_lines[row][0] == b'y'
            && state.ai_lines[row][1] == b'o'
            && state.ai_lines[row][2] == b'u'
            && state.ai_lines[row][3] == b'>'
        {
            WABI_TEA
        } else if len >= 4
            && state.ai_lines[row][0] == b's'
            && state.ai_lines[row][1] == b'y'
            && state.ai_lines[row][2] == b's'
            && state.ai_lines[row][3] == b'>'
        {
            WABI_STONE
        } else {
            WABI_PAPER
        };

        draw_bytes(
            sink,
            line_row,
            AI_PANEL_LEFT + 2,
            fg,
            WABI_INK,
            &state.ai_lines[row][..len],
        );
    }
}

fn draw_usize(sink: &ConsoleSink, row: usize, col: usize, fg: u8, bg: u8, mut value: usize) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while value > 0 {
            buf[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
    }

    set_color(sink, fg, bg);
    goto(sink, row, col);
    while len > 0 {
        len -= 1;
        print_byte(sink, buf[len]);
    }
}

fn frame_index(stage: usize, pulse: usize) -> usize {
    stage * PULSE_COUNT + pulse
}

fn progress_percent(stage: usize, pulse: usize) -> usize {
    ((frame_index(stage, pulse) + 1) * 100) / FRAME_COUNT
}

fn scaled_frame(total: usize, stage: usize, pulse: usize) -> usize {
    let value = total.saturating_mul(frame_index(stage, pulse) + 1) / FRAME_COUNT;
    if value == 0 && total > 0 {
        1
    } else {
        value.min(total)
    }
}

fn glyph_palette(stage: usize, pulse: usize) -> (u8, u8, u8) {
    match (stage, pulse) {
        (0, _) => (8, 11, 15),
        (1, 0) => (9, 11, 15),
        (1, _) => (11, 15, 10),
        (2, _) => (15, 11, 3),
        (3, 0) => (13, 11, 15),
        (3, _) => (11, 13, 15),
        (_, 0) => (10, 11, 15),
        (_, _) => (11, 15, 10),
    }
}

fn draw_meter(sink: &ConsoleSink, row: usize, left: usize, width: usize, filled: usize, fg: u8, bg: u8) {
    let clamped = filled.min(width);
    draw_repeat(sink, row, left, 8, bg, CP437_LIGHT, width);
    if clamped > 0 {
        draw_repeat(sink, row, left, fg, bg, CP437_BLOCK, clamped);
    }
    if clamped < width {
        draw_repeat(sink, row, left + clamped, 8, bg, CP437_MEDIUM, width - clamped);
    }
}

fn draw_header_bar(sink: &ConsoleSink, stage: usize, pulse: usize) {
    let frame = frame_index(stage, pulse);
    fill_band(sink, 0, 0, SCREEN_WIDTH, 0, 1);
    draw_text(sink, 0, 2, 15, 1, " GOS v0.2 ");
    draw_text(sink, 0, 14, 11, 1, "NEXT-GEN GRAPH BOOT");
    draw_repeat(sink, 0, 41, 8, 1, CP437_MEDIUM, 14);
    draw_repeat(sink, 0, 41 + (frame * 3 % 11), 11, 1, CP437_BLOCK, 2);
    draw_text(sink, 0, 60, 10, 1, BOOT_PHASES[stage]);
}

fn draw_backdrop(sink: &ConsoleSink, stage: usize, pulse: usize) {
    let frame = frame_index(stage, pulse);
    for (idx, (row, col)) in STARFIELD.iter().enumerate() {
        let phase = (frame + idx) % 5;
        let (byte, fg) = match phase {
            0 => (b'*', 15),
            1 => (CP437_LIGHT, 11),
            2 => (b'.', 8),
            3 => (b'+', 9),
            _ => (CP437_DARK, 8),
        };
        draw_byte(sink, *row, *col, fg, 0, byte);
    }

    let left_head = 24 + (frame % 3);
    let right_head = 53 + (frame % 3);
    draw_repeat(sink, 8, 23, 11, 0, CP437_LIGHT, 3);
    draw_byte(sink, 8, left_head, 10, 0, b'>');
    draw_repeat(sink, 8, 53, 11, 0, CP437_LIGHT, 3);
    draw_byte(sink, 8, right_head, 10, 0, b'>');
    draw_repeat(sink, 12, 23, 8, 0, CP437_MEDIUM, 3);
    draw_byte(sink, 12, 24 + ((frame + 1) % 3), 11, 0, b'>');
    draw_repeat(sink, 12, 53, 8, 0, CP437_MEDIUM, 3);
    draw_byte(sink, 12, 53 + ((frame + 2) % 3), 11, 0, b'>');

    let _ = stage;
}

fn draw_phase_panel(sink: &ConsoleSink, stage: usize, pulse: usize) {
    draw_box(sink, 2, 2, 23, 14, " BOOT GRAPH ", 11, 0);
    draw_text(sink, 3, 4, 8, 0, "graph activation lane");

    for (idx, phase) in BOOT_PHASES.iter().enumerate() {
        let row = 5 + idx * 2;
        if idx < stage {
            draw_byte(sink, row, 4, 10, 0, CP437_BLOCK);
            draw_text(sink, row, 6, 10, 0, phase);
        } else if idx == stage {
            draw_byte(sink, row, 4, 11, 0, b'>');
            draw_byte(sink, row, 5 + pulse, 11, 0, CP437_BLOCK);
            draw_text(sink, row, 7, 15, 0, phase);
        } else {
            draw_byte(sink, row, 4, 8, 0, CP437_MEDIUM);
            draw_text(sink, row, 6, 8, 0, phase);
        }
    }

    draw_text(sink, 14, 4, 7, 0, "frame");
    draw_usize(sink, 14, 10, 15, 0, frame_index(stage, pulse) + 1);
    draw_text(sink, 14, 13, 7, 0, "/15");
    draw_meter(sink, 14, 17, 5, pulse + 1, 11, 0);
}

fn draw_core_glyph(sink: &ConsoleSink, stage: usize, pulse: usize) {
    let frame = frame_index(stage, pulse);
    let (main_fg, edge_fg, spark_fg) = glyph_palette(stage, pulse);
    let wobble = frame % LIVE_SIGIL_FRAMES;
    let top = (4i32 + BOOT_WOBBLE_Y[wobble]).max(3) as usize;
    let left = (29i32 + BOOT_WOBBLE_X[wobble]).max(26) as usize;
    let height = 11usize;

    for y in 0..height {
        let mut row = [b' '; 23];
        let dy = y as i32 - 5;
        for (x, cell) in row.iter_mut().enumerate() {
            let dx = x as i32 - 11;
            let ax = dx * 2;
            let ay = dy * 3;
            let dist = ax * ax + ay * ay;

            let mut byte = if (250..=720).contains(&dist) {
                CP437_BLOCK
            } else if (180..=790).contains(&dist) {
                CP437_MEDIUM
            } else {
                b' '
            };

            if dx > 5 && dy < 0 {
                byte = b' ';
            }

            if (-1..=1).contains(&dy) && (0..=8).contains(&dx) {
                byte = CP437_BLOCK;
            }

            if dx >= 9 && dy == 0 {
                byte = CP437_LIGHT;
            }

            if byte != b' ' && (x + frame + y).is_multiple_of(9) {
                byte = CP437_LIGHT;
            }

            *cell = byte;
        }

        let fg = if y == 5 || y == 6 {
            spark_fg
        } else if y % 2 == 0 {
            main_fg
        } else {
            edge_fg
        };
        draw_bytes(sink, top + y, left, fg, 0, &row);
    }

    for (idx, (row, col)) in ORBIT_POINTS.iter().enumerate() {
        let phase = (idx + frame) % ORBIT_POINTS.len();
        let row = (*row as i32 + BOOT_WOBBLE_Y[wobble]).max(2) as usize;
        let col = (*col as i32 + BOOT_WOBBLE_X[wobble]).max(25) as usize;
        if phase == 0 || phase == 1 {
            draw_byte(sink, row, col, spark_fg, 0, b'*');
        } else if phase == 2 || phase == 3 {
            draw_byte(sink, row, col, edge_fg, 0, CP437_LIGHT);
        }
    }
}

fn draw_sigil_panel(sink: &ConsoleSink, stage: usize, pulse: usize) {
    draw_box(sink, 2, 26, 29, 14, " SIGIL CORE ", 11, 0);
    draw_text(sink, 3, 31, 8, 0, "dynamic G resonance");
    draw_core_glyph(sink, stage, pulse);
}

#[allow(clippy::too_many_arguments)]
fn draw_metric_line(
    sink: &ConsoleSink,
    row: usize,
    label: &str,
    value: usize,
    total: usize,
    stage: usize,
    pulse: usize,
    fg: u8,
) {
    draw_text(sink, row, 58, 7, 0, label);
    draw_usize(sink, row, 63, 15, 0, value);
    let scaled = scaled_frame(total, stage, pulse);
    let width = 8usize;
    let fill = if total == 0 {
        0
    } else {
        (scaled * width).div_ceil(total)
    };
    draw_meter(sink, row, 68, width, fill, fg, 0);
}

fn draw_telemetry_panel(sink: &ConsoleSink, stage: usize, pulse: usize, snapshot: gos_protocol::GraphSnapshot) {
    draw_box(sink, 2, 56, 22, 8, " TELEMETRY ", 11, 0);
    draw_metric_line(sink, 3, "plg", snapshot.plugin_count, snapshot.plugin_count, stage, pulse, 11);
    draw_metric_line(sink, 4, "nod", snapshot.node_count, snapshot.node_count, stage, pulse, 15);
    draw_metric_line(sink, 5, "edg", snapshot.edge_count, snapshot.edge_count, stage, pulse, 14);
    draw_metric_line(sink, 6, "rq ", snapshot.ready_queue_len, snapshot.ready_queue_len.max(1), stage, pulse, 10);
    draw_metric_line(sink, 7, "sig", snapshot.signal_queue_len + snapshot.control_queue_len, (snapshot.signal_queue_len + snapshot.control_queue_len).max(1), stage, pulse, 12);
    draw_text(sink, 8, 58, 7, 0, "mesh");
    draw_text(
        sink,
        8,
        63,
        if stage + 1 == STAGE_COUNT && pulse + 1 == PULSE_COUNT && gos_runtime::is_stable() {
            10
        } else {
            14
        },
        0,
        if stage + 1 == STAGE_COUNT && pulse + 1 == PULSE_COUNT && gos_runtime::is_stable() {
            "stable"
        } else {
            "sync  "
        },
    );
    draw_text(sink, 8, 70, 7, 0, "tk");
    draw_usize(sink, 8, 73, 15, 0, (snapshot.tick as usize) + frame_index(stage, pulse));
}

fn draw_event_panel(sink: &ConsoleSink, stage: usize, pulse: usize) {
    draw_box(sink, 10, 56, 22, 6, " EVENT BUS ", 11, 0);
    let active = (stage + pulse) % EVENT_LINES;
    for (idx, line) in BOOT_EVENTS[stage].iter().enumerate() {
        let row = 11 + idx;
        let fg = if idx == active { 15 } else { 8 + (idx as u8 % 3) };
        draw_text(sink, row, 58, fg, 0, line);
    }
}

fn draw_boot_footer(sink: &ConsoleSink, stage: usize, pulse: usize) {
    let percent = progress_percent(stage, pulse);
    let fill = (52 * percent) / 100;
    draw_box(sink, 17, 2, 76, 6, " STARTUP FLOW ", 11, 0);
    draw_center(sink, 18, 11, 0, BOOT_COPY[stage]);
    draw_center(sink, 19, 8, 0, "graph-native bootstrap is compositing into a live command mesh");
    draw_repeat(sink, 20, 6, 8, 0, CP437_LIGHT, 52);
    if fill > 0 {
        draw_repeat(sink, 20, 6, 11, 0, CP437_BLOCK, fill.min(52));
    }
    draw_text(sink, 20, 60, 15, 0, "BOOT");
    draw_usize(sink, 20, 66, 11, 0, percent);
    draw_text(sink, 20, 69, 11, 0, "%");
    draw_text(sink, 21, 6, 11, 0, "stable ids");
    draw_text(sink, 21, 18, 15, 0, "vector mesh");
    draw_text(sink, 21, 33, 10, 0, "ai-native ctl");
    draw_text(sink, 21, 49, 11, 0, "sigil online");
}

fn render_boot_frame(sink: &ConsoleSink, stage: usize, pulse: usize) {
    let snapshot = gos_runtime::snapshot();
    clear_canvas(sink);
    draw_header_bar(sink, stage, pulse);
    draw_backdrop(sink, stage, pulse);
    draw_phase_panel(sink, stage, pulse);
    draw_sigil_panel(sink, stage, pulse);
    draw_telemetry_panel(sink, stage, pulse, snapshot);
    draw_event_panel(sink, stage, pulse);
    draw_boot_footer(sink, stage, pulse);
    draw_center(sink, 23, 8, 0, "G-sigil boot cinema is rendered natively in VGA text mode");
}

fn spin_delay(mut cycles: usize) {
    while cycles > 0 {
        core::hint::spin_loop();
        cycles -= 1;
    }
}

fn play_boot_sequence(sink: &ConsoleSink) {
    for stage in 0..STAGE_COUNT {
        for pulse in 0..PULSE_COUNT {
            render_boot_frame(sink, stage, pulse);
            spin_delay(900_000);
        }
    }
}

fn command_display_bytes(state: &ShellState, out: &mut [u8; 128]) -> usize {
    let mut written = 0usize;
    let mut idx = 0usize;
    while idx < state.len && idx < state.buffer.len() && written < out.len() {
        let byte = state.buffer[idx];
        if byte.is_ascii() {
            if byte >= 0x20 {
                out[written] = byte;
                written += 1;
            }
            idx += 1;
            continue;
        }

        if (byte & 0xC0) != 0x80 {
            out[written] = b'#';
            written += 1;
        }
        idx += 1;
    }
    written
}

fn draw_linebuf<const N: usize>(
    sink: &ConsoleSink,
    row: usize,
    col: usize,
    fg: u8,
    bg: u8,
    buf: &LineBuf<N>,
) {
    draw_bytes(sink, row, col, fg, bg, buf.as_slice());
}

fn clear_command_area(sink: &ConsoleSink) {
    for row in COMMAND_SCROLL_TOP..=COMMAND_SCROLL_BOTTOM {
        fill_band(sink, row, 0, SCREEN_WIDTH, 0, 0);
    }
}

fn last_page_offset(total: usize) -> usize {
    if total == 0 {
        0
    } else {
        ((total - 1) / GRAPH_PAGE_ITEMS) * GRAPH_PAGE_ITEMS
    }
}

fn normalize_page_offset(offset: usize, total: usize) -> usize {
    offset.min(last_page_offset(total))
}

fn render_graph_footer(sink: &ConsoleSink, state: &ShellState, label: &str) {
    fill_band(sink, GRAPH_VIEW_FOOT_ROW, 0, SCREEN_WIDTH, 15, 0);
    draw_text(sink, GRAPH_VIEW_FOOT_ROW, 2, 8, 0, label);
    draw_ai_panel(sink, state);
    redraw_footer(sink, state, false);
    focus_footer_input(sink, state);
}

/// Phase H.1.x.4 — render the audited-mutation ring captured by the
/// supervisor `apply_cypher_mutation` gate.  Newest entries are shown
/// first; an empty ring renders a "no mutations yet" notice.
fn render_mutations_overview(sink: &ConsoleSink, state: &mut ShellState) {
    use gos_protocol::ControlPlaneEnvelope;
    const ROWS: usize = 6;
    let mut buf: [Option<ControlPlaneEnvelope>; ROWS] = [None; ROWS];
    let returned = gos_runtime::snapshot_audit_ring(&mut buf);
    let lifetime = gos_runtime::audit_ring_total();

    state.graph_mode = GRAPH_MODE_INFO;
    state.graph_offset = 0;
    state.graph_total = returned;
    clear_command_area(sink);

    let mut title = LineBuf::<72>::new();
    title.push_str("AUDITED MUTATIONS  ring=");
    title.push_dec(returned as u64);
    title.push_str("  lifetime=");
    title.push_dec(lifetime);
    draw_linebuf(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, &title);

    if returned == 0 {
        draw_text(
            sink,
            GRAPH_VIEW_FIRST_ITEM_ROW,
            4,
            8,
            0,
            "no mutations yet",
        );
        draw_text(
            sink,
            GRAPH_VIEW_FIRST_ITEM_ROW + 1,
            4,
            8,
            0,
            "try: CREATE MOUNT 'V_from' -> 'V_to' (in cypher)",
        );
        render_graph_footer(sink, state, "audit ring");
        return;
    }

    for row in 0..ROWS {
        fill_band(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 0, SCREEN_WIDTH, 0, 0);
        let Some(env) = buf[row] else { break };
        let mut line = LineBuf::<72>::new();
        line.push_str("src=");
        line.push_fixed_ascii(&env.subject);
        line.push_str("  arg0=");
        line.push_hex(env.arg0);
        line.push_str("  arg1=");
        line.push_hex(env.arg1);
        draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 4, 7, 0, &line);
    }
    render_graph_footer(sink, state, "audit ring");
}

fn render_graph_notice(sink: &ConsoleSink, state: &mut ShellState, title: &str, line1: &str, line2: &str, fg: u8) {
    state.graph_mode = GRAPH_MODE_INFO;
    state.graph_offset = 0;
    state.graph_total = 0;
    clear_command_area(sink);
    draw_text(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, title);
    draw_text(sink, GRAPH_VIEW_FIRST_ITEM_ROW, 4, fg, 0, line1);
    if !line2.is_empty() {
        draw_text(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 1, 4, 8, 0, line2);
    }
    render_graph_footer(sink, state, "graph notice");
}

#[allow(clippy::needless_range_loop)]
fn render_graph_overview(sink: &ConsoleSink, state: &mut ShellState, requested_offset: usize) {
    let mut nodes = [GraphNodeSummary::EMPTY; GRAPH_OVERVIEW_ITEMS];
    let mut edges = [GraphEdgeSummary::EMPTY; GRAPH_OVERVIEW_ITEMS];
    let (node_total, _) = gos_runtime::node_page(0, &mut nodes);
    let (edge_total, _) = gos_runtime::edge_page(0, &mut edges);
    let total = node_total.max(edge_total);
    let offset = normalize_page_offset(requested_offset, total);
    let (_, node_returned) = gos_runtime::node_page(offset, &mut nodes);
    let (_, edge_returned) = gos_runtime::edge_page(offset, &mut edges);

    state.graph_mode = GRAPH_MODE_OVERVIEW;
    state.graph_context = GRAPH_CTX_OVERVIEW;
    state.graph_offset = offset;
    state.graph_total = total;
    clear_command_area(sink);
    draw_text(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, "GRAPH OVERVIEW  node <vec> / edge <vec>");

    for row in 0..GRAPH_OVERVIEW_ITEMS {
        fill_band(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 0, SCREEN_WIDTH, 0, 0);
        if row < node_returned {
            let item = nodes[row];
            let mut line = LineBuf::<72>::new();
            line.push_str("N ");
            line.push_vector(item.vector);
            line.push_str("  ");
            line.push_str(item.plugin_name);
            line.push_byte(b'/');
            line.push_str(item.local_node_key);
            line.push_str("  ");
            line.push_str(node_type_label(item.node_type));
            draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 4, 15, 0, &line);
        } else {
            draw_text(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 4, 8, 0, "N -");
        }
    }

    for row in 0..GRAPH_OVERVIEW_ITEMS {
        let draw_row = GRAPH_VIEW_FIRST_ITEM_ROW + GRAPH_OVERVIEW_ITEMS + row;
        fill_band(sink, draw_row, 0, SCREEN_WIDTH, 0, 0);
        if row < edge_returned {
            let item = edges[row];
            let mut line = LineBuf::<72>::new();
            line.push_str("E ");
            line.push_edge_vector(item.edge_vector);
            line.push_str("  ");
            line.push_str(edge_type_label(item.edge_type));
            line.push_str("  ");
            line.push_vector(item.from_vector);
            line.push_str(" -> ");
            line.push_vector(item.to_vector);
            draw_linebuf(sink, draw_row, 4, 15, 0, &line);
        } else {
            draw_text(sink, draw_row, 4, 8, 0, "E -");
        }
    }

    let mut footer = LineBuf::<72>::new();
    footer.push_str("overview page ");
    footer.push_dec((offset / GRAPH_OVERVIEW_ITEMS + 1) as u64);
    footer.push_byte(b'/');
    footer.push_dec(total.div_ceil(GRAPH_OVERVIEW_ITEMS).max(1) as u64);
    footer.push_str("  nodes ");
    footer.push_dec((offset + node_returned).min(node_total) as u64);
    footer.push_byte(b'/');
    footer.push_dec(node_total as u64);
    footer.push_str("  edges ");
    footer.push_dec((offset + edge_returned).min(edge_total) as u64);
    footer.push_byte(b'/');
    footer.push_dec(edge_total as u64);
    render_graph_footer(
        sink,
        state,
        core::str::from_utf8(footer.as_slice()).unwrap_or("overview"),
    );
}

#[allow(clippy::needless_range_loop)]
fn render_node_list(sink: &ConsoleSink, state: &mut ShellState, requested_offset: usize) {
    let mut page = [GraphNodeSummary::EMPTY; GRAPH_PAGE_ITEMS];
    let (total, _) = gos_runtime::node_page(0, &mut page);
    let offset = normalize_page_offset(requested_offset, total);
    let (total, returned) = gos_runtime::node_page(offset, &mut page);
    state.graph_mode = GRAPH_MODE_NODE_LIST;
    state.graph_context = GRAPH_CTX_NODE;
    state.graph_offset = offset;
    state.graph_total = total;
    clear_command_area(sink);
    draw_text(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, "NODE LIST  node <vector> selects a node");
    for row in 0..GRAPH_PAGE_ITEMS {
        fill_band(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 0, SCREEN_WIDTH, 0, 0);
        if row >= returned {
            continue;
        }
        let item = page[row];
        let mut line = LineBuf::<72>::new();
        line.push_vector(item.vector);
        line.push_str("  ");
        line.push_str(item.plugin_name);
        line.push_byte(b'/');
        line.push_str(item.local_node_key);
        line.push_str("  ");
        line.push_str(node_type_label(item.node_type));
        line.push_str("  ");
        line.push_str(lifecycle_label(item.lifecycle));
        let fg = if state.selected_node == Some(item.vector) { 14 } else { 15 };
        draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 4, fg, 0, &line);
    }

    let mut footer = LineBuf::<44>::new();
    footer.push_str("nodes ");
    footer.push_dec((offset + 1).min(total) as u64);
    footer.push_byte(b'-');
    footer.push_dec((offset + returned).min(total) as u64);
    footer.push_byte(b'/');
    footer.push_dec(total as u64);
    footer.push_str("  page ");
    footer.push_dec((offset / GRAPH_PAGE_ITEMS + 1) as u64);
    footer.push_byte(b'/');
    footer.push_dec(total.div_ceil(GRAPH_PAGE_ITEMS).max(1) as u64);
    render_graph_footer(
        sink,
        state,
        core::str::from_utf8(footer.as_slice()).unwrap_or("nodes"),
    );
}

fn render_node_detail(sink: &ConsoleSink, state: &mut ShellState, vector: VectorAddress) {
    let Some(summary) = gos_runtime::node_summary(vector) else {
        render_graph_notice(sink, state, "NODE DETAIL", "node not found", "try show first", 12);
        return;
    };
    state.selected_node = Some(vector);
    state.selected_edge = None;
    state.graph_mode = GRAPH_MODE_NODE_DETAIL;
    state.graph_context = GRAPH_CTX_NODE;
    state.graph_offset = 0;
    state.graph_total = 1;
    clear_command_area(sink);

    let mut title = LineBuf::<72>::new();
    title.push_str("NODE DETAIL ");
    title.push_vector(summary.vector);
    draw_linebuf(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, &title);

    let mut line = LineBuf::<72>::new();
    line.push_str("vector: ");
    line.push_vector(summary.vector);
    line.push_str("  state: ");
    line.push_str(lifecycle_label(summary.lifecycle));
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("plugin: ");
    line.push_str(summary.plugin_name);
    line.push_str("  id: ");
    line.push_fixed_ascii(&summary.plugin_id.0);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 1, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("local: ");
    line.push_str(summary.local_node_key);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 2, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("type: ");
    line.push_str(node_type_label(summary.node_type));
    line.push_str("  entry: ");
    line.push_str(entry_policy_label(summary.entry_policy));
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 3, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("exec: ");
    line.push_fixed_ascii(&summary.executor_id.0);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 4, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("exports: ");
    line.push_dec(summary.export_count as u64);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 5, 4, 15, 0, &line);

    render_graph_footer(sink, state, "show toggles to related edges");
}

fn selected_edge_direction(state: &ShellState, edge: &GraphEdgeSummary) -> GraphEdgeDirection {
    match state.selected_node {
        Some(vector) if vector == edge.to_vector && vector != edge.from_vector => GraphEdgeDirection::Inbound,
        _ => GraphEdgeDirection::Outbound,
    }
}

#[allow(clippy::needless_range_loop)]
fn render_edge_list(sink: &ConsoleSink, state: &mut ShellState, requested_offset: usize) {
    let Some(node_vec) = state.selected_node else {
        render_graph_notice(sink, state, "EDGE LIST", "no node selected", "use node <vector> first", 12);
        return;
    };

    let mut page = [GraphEdgeSummary::EMPTY; GRAPH_PAGE_ITEMS];
    let (total, _) = match gos_runtime::edge_page_for_node(node_vec, 0, &mut page) {
        Ok(page) => page,
        Err(_) => {
            render_graph_notice(sink, state, "EDGE LIST", "node has no runtime entry", "", 12);
            return;
        }
    };
    let offset = normalize_page_offset(requested_offset, total);
    let (total, returned) = match gos_runtime::edge_page_for_node(node_vec, offset, &mut page) {
        Ok(page) => page,
        Err(_) => {
            render_graph_notice(sink, state, "EDGE LIST", "edge query failed", "", 12);
            return;
        }
    };

    state.graph_mode = GRAPH_MODE_EDGE_LIST;
    state.graph_context = GRAPH_CTX_EDGE;
    state.graph_offset = offset;
    state.graph_total = total;
    clear_command_area(sink);

    let mut title = LineBuf::<72>::new();
    title.push_str("EDGE LIST ");
    title.push_vector(node_vec);
    title.push_str("  edge <vector> selects an edge");
    draw_linebuf(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, &title);

    for row in 0..GRAPH_PAGE_ITEMS {
        fill_band(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 0, SCREEN_WIDTH, 0, 0);
        if row >= returned {
            continue;
        }
        let item = page[row];
        let mut line = LineBuf::<72>::new();
        line.push_str(edge_direction_label(item.direction));
        line.push_byte(b' ');
        line.push_edge_vector(item.edge_vector);
        line.push_byte(b' ');
        line.push_str(edge_type_label(item.edge_type));
        line.push_byte(b' ');
        line.push_vector(item.from_vector);
        line.push_str(" -> ");
        line.push_vector(item.to_vector);
        if let (Some(namespace), Some(name)) = (item.capability_namespace, item.capability_binding) {
            line.push_str("  cap=");
            line.push_str(namespace);
            line.push_byte(b'/');
            line.push_str(name);
        }
        let fg = if state.selected_edge == Some(item.edge_vector) { 14 } else { 15 };
        draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + row, 4, fg, 0, &line);
    }

    let mut footer = LineBuf::<44>::new();
    footer.push_str("edges ");
    footer.push_dec((offset + 1).min(total) as u64);
    footer.push_byte(b'-');
    footer.push_dec((offset + returned).min(total) as u64);
    footer.push_byte(b'/');
    footer.push_dec(total as u64);
    footer.push_str("  page ");
    footer.push_dec((offset / GRAPH_PAGE_ITEMS + 1) as u64);
    footer.push_byte(b'/');
    footer.push_dec(total.div_ceil(GRAPH_PAGE_ITEMS).max(1) as u64);
    render_graph_footer(
        sink,
        state,
        core::str::from_utf8(footer.as_slice()).unwrap_or("edges"),
    );
}

fn render_edge_detail(sink: &ConsoleSink, state: &mut ShellState, edge_vector: EdgeVector) {
    let Some(summary) = gos_runtime::edge_summary(edge_vector) else {
        render_graph_notice(sink, state, "EDGE DETAIL", "edge not found", "run edge to browse edges", 12);
        return;
    };

    state.selected_edge = Some(edge_vector);
    if state.selected_node.is_none() {
        state.selected_node = Some(summary.from_vector);
    }
    state.graph_mode = GRAPH_MODE_EDGE_DETAIL;
    state.graph_context = GRAPH_CTX_EDGE;
    state.graph_offset = 0;
    state.graph_total = 1;
    clear_command_area(sink);

    let direction = selected_edge_direction(state, &summary);
    let mut title = LineBuf::<72>::new();
    title.push_str("EDGE DETAIL ");
    title.push_edge_vector(summary.edge_vector);
    draw_linebuf(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, &title);

    let mut line = LineBuf::<72>::new();
    line.push_str("dir: ");
    line.push_str(edge_direction_label(direction));
    line.push_str("  type: ");
    line.push_str(edge_type_label(summary.edge_type));
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("from: ");
    line.push_vector(summary.from_vector);
    line.push_str("  ");
    line.push_str(summary.from_key);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 1, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("to:   ");
    line.push_vector(summary.to_vector);
    line.push_str("  ");
    line.push_str(summary.to_key);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 2, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("route: ");
    line.push_str(match summary.route_policy {
        gos_protocol::RoutePolicy::Direct => "direct",
        gos_protocol::RoutePolicy::Weighted => "weighted",
        gos_protocol::RoutePolicy::Broadcast => "broadcast",
        gos_protocol::RoutePolicy::FailFast => "failfast",
    });
    line.push_str("  weight: ");
    line.push_dec(summary.weight as u64);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 3, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("acl: ");
    line.push_dec(summary.acl_mask);
    if let (Some(namespace), Some(name)) = (summary.capability_namespace, summary.capability_binding) {
        line.push_str("  cap=");
        line.push_str(namespace);
        line.push_byte(b'/');
        line.push_str(name);
    }
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 4, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("edge-id: ");
    line.push_fixed_ascii(&summary.edge_id.0);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 5, 4, 15, 0, &line);

    render_graph_footer(sink, state, "show toggles to node view");
}

fn render_nodes_for_selected_edge(sink: &ConsoleSink, state: &mut ShellState) {
    let Some(edge_vector) = state.selected_edge else {
        if let Some(vector) = state.selected_node {
            render_node_detail(sink, state, vector);
        } else {
            render_graph_overview(sink, state, 0);
        }
        return;
    };

    let Some(edge) = gos_runtime::edge_summary(edge_vector) else {
        render_graph_notice(sink, state, "EDGE NODES", "selected edge missing", "run show from overview again", 12);
        return;
    };
    let Some(from_node) = gos_runtime::node_summary(edge.from_vector) else {
        render_graph_notice(sink, state, "EDGE NODES", "from-node missing", "", 12);
        return;
    };
    let Some(to_node) = gos_runtime::node_summary(edge.to_vector) else {
        render_graph_notice(sink, state, "EDGE NODES", "to-node missing", "", 12);
        return;
    };

    state.graph_mode = GRAPH_MODE_NODE_DETAIL;
    state.graph_context = GRAPH_CTX_NODE;
    state.graph_offset = 0;
    state.graph_total = 2;
    if state.selected_node.is_none() {
        state.selected_node = Some(edge.from_vector);
    }
    clear_command_area(sink);

    let mut title = LineBuf::<72>::new();
    title.push_str("EDGE NODES ");
    title.push_edge_vector(edge.edge_vector);
    draw_linebuf(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, &title);

    let mut line = LineBuf::<72>::new();
    line.push_str("edge: ");
    line.push_str(edge_type_label(edge.edge_type));
    line.push_str("  ");
    line.push_vector(edge.from_vector);
    line.push_str(" -> ");
    line.push_vector(edge.to_vector);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("from: ");
    line.push_vector(from_node.vector);
    line.push_str("  ");
    line.push_str(from_node.plugin_name);
    line.push_byte(b'/');
    line.push_str(from_node.local_node_key);
    line.push_str("  ");
    line.push_str(lifecycle_label(from_node.lifecycle));
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 1, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("to:   ");
    line.push_vector(to_node.vector);
    line.push_str("  ");
    line.push_str(to_node.plugin_name);
    line.push_byte(b'/');
    line.push_str(to_node.local_node_key);
    line.push_str("  ");
    line.push_str(lifecycle_label(to_node.lifecycle));
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 2, 4, 15, 0, &line);

    render_graph_footer(sink, state, "show toggles back to edge view  node <vec> selects");
}

fn render_metrics(sink: &ConsoleSink, state: &mut ShellState) {
    clear_command_area(sink);
    state.graph_mode = GRAPH_MODE_INFO;
    state.graph_context = GRAPH_CTX_METRICS;
    draw_text(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, "V2.3 RUNTIME METRICS  (refresh: metrics)");

    let g_epoch = gos_runtime::graph_epoch();
    let r_epoch = gos_supervisor::render_epoch();
    let mut line = LineBuf::<72>::new();
    line.push_str("graph_epoch: ");
    line.push_dec(g_epoch);
    line.push_str("  render_epoch: ");
    line.push_dec(r_epoch);
    let lag = g_epoch.saturating_sub(r_epoch);
    if lag > 0 {
        line.push_str("  lag: ");
        line.push_dec(lag);
    }
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("idle_cycles: ");
    line.push_dec(gos_supervisor::idle_cycle_count());
    line.push_str("  (quiescent service cycles)");
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 1, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("causal_depth_max: ");
    line.push_dec(gos_supervisor::causal_depth_max());
    line.push_str("  cap=2048");
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 2, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("subscribe_pairs: ");
    line.push_dec(gos_runtime::subscribe_pair_count() as u64);
    line.push_byte(b'/');
    line.push_dec(gos_runtime::MAX_SUBSCRIBE_PAIRS as u64);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 3, 4, 15, 0, &line);

    let snap = gos_runtime::snapshot();
    line = LineBuf::new();
    line.push_str("tick: ");
    line.push_dec(snap.tick);
    line.push_str("  plugins: ");
    line.push_dec(snap.plugin_count as u64);
    line.push_str("  nodes: ");
    line.push_dec(snap.node_count as u64);
    line.push_str("  edges: ");
    line.push_dec(snap.edge_count as u64);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 4, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("domain_switches: ");
    line.push_dec(gos_runtime::domain_switch_count());
    line.push_str("  preemptions: ");
    line.push_dec(gos_runtime::preempt_count());
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 5, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("boot_fallback_allocs: ");
    line.push_dec(gos_runtime::boot_fallback_alloc_count());
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 6, 4, 8, 0, &line);

    render_graph_footer(sink, state, "metrics  back  where");
}

fn render_where(sink: &ConsoleSink, state: &mut ShellState) {
    clear_command_area(sink);
    state.graph_mode = GRAPH_MODE_INFO;
    draw_text(sink, GRAPH_VIEW_TITLE_ROW, 4, 11, 0, "GRAPH SELECTION");
    let mut line = LineBuf::<72>::new();
    line.push_str("context: ");
    line.push_str(graph_context_label(state.graph_context));
    line.push_str("  view: ");
    line.push_str(graph_mode_label(state.graph_mode));
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("node: ");
    match state.selected_node {
        Some(vector) => line.push_vector(vector),
        None => line.push_str("none"),
    }
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 1, 4, 15, 0, &line);

    line = LineBuf::new();
    line.push_str("edge: ");
    match state.selected_edge {
        Some(vector) => line.push_edge_vector(vector),
        None => line.push_str("none"),
    }
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 2, 4, 15, 0, &line);

    // Surface supervisor instance + heap quota state for the selected
    // node when one is present.  Reads through the B.2/B.3 bridges:
    //   selected_node -> runtime instance binding -> supervisor quota.
    if let Some(vector) = state.selected_node {
        let mut quota_line = LineBuf::<72>::new();
        quota_line.push_str("quota: ");
        match gos_runtime::instance_id_for_vec(vector) {
            Some(instance_id) if instance_id.0 != 0 => {
                match gos_supervisor::instance_heap_usage(instance_id) {
                    Some((used, max)) => {
                        quota_line.push_dec(used as u64);
                        quota_line.push_byte(b'/');
                        quota_line.push_dec(max as u64);
                        quota_line.push_str(" pages  inst#");
                        quota_line.push_dec(instance_id.0);
                        if let Some(restart_gen) =
                            gos_supervisor::instance_restart_generation(instance_id)
                        {
                            quota_line.push_str("  restarts=");
                            quota_line.push_dec(restart_gen as u64);
                        }
                        if gos_supervisor::instance_is_degraded(instance_id) {
                            quota_line.push_str("  DEGRADED");
                        }
                    }
                    None => quota_line.push_str("instance not registered"),
                }
            }
            _ => quota_line.push_str("unbound (boot fallback)"),
        }
        draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 3, 4, 15, 0, &quota_line);
    }

    // Boot-fallback audit: any non-zero count after realize_boot_modules
    // indicates a builtin that escaped B.3.3's rebind sweep.  Useful as a
    // boot-conformance check from the live shell.
    let mut audit = LineBuf::<72>::new();
    audit.push_str("audit: boot-fallback allocs ");
    audit.push_dec(gos_runtime::boot_fallback_alloc_count());
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 4, 4, 15, 0, &audit);

    // Phase B.4.1: domain PML4 root.  Non-zero confirms map_module ->
    // build_domain -> k_vmm::create_isolated_address_space ran for this
    // instance's owning module.
    if let Some(vector) = state.selected_node
        && let Some(instance_id) = gos_runtime::instance_id_for_vec(vector)
        && instance_id.0 != 0
        && let Some(root) = gos_supervisor::instance_domain_root(instance_id)
    {
        let mut domain_line = LineBuf::<72>::new();
        domain_line.push_str("domain root_phys=0x");
        domain_line.push_hex(root);
        if root == 0 {
            domain_line.push_str("  (UNMAPPED)");
        }
        draw_linebuf(
            sink,
            GRAPH_VIEW_FIRST_ITEM_ROW + 5,
            4,
            15,
            0,
            &domain_line,
        );
    }

    // V2.3 epoch/idle/causal/subscribe telemetry — row +6 (last row in command area)
    let mut v23 = LineBuf::<72>::new();
    v23.push_str("ep:");
    v23.push_dec(gos_runtime::graph_epoch());
    v23.push_str("  idle:");
    v23.push_dec(gos_supervisor::idle_cycle_count());
    v23.push_str("  depth:");
    v23.push_dec(gos_supervisor::causal_depth_max());
    v23.push_str("  subs:");
    v23.push_dec(gos_runtime::subscribe_pair_count() as u64);
    draw_linebuf(sink, GRAPH_VIEW_FIRST_ITEM_ROW + 6, 4, 8, 0, &v23);

    render_graph_footer(sink, state, "where  select clear  metrics");
}

fn restore_graph_nav_state(sink: &ConsoleSink, state: &mut ShellState, snapshot: GraphNavState) {
    state.selected_node = snapshot.selected_node;
    state.selected_edge = snapshot.selected_edge;
    state.graph_mode = snapshot.graph_mode;
    state.graph_context = snapshot.graph_context;
    state.graph_offset = snapshot.graph_offset;
    state.graph_total = snapshot.graph_total;

    match snapshot.graph_mode {
        GRAPH_MODE_NONE => {
            clear_command_area(sink);
            redraw_footer(sink, state, false);
            focus_footer_input(sink, state);
        }
        GRAPH_MODE_OVERVIEW => render_graph_overview(sink, state, snapshot.graph_offset),
        GRAPH_MODE_NODE_LIST => render_node_list(sink, state, snapshot.graph_offset),
        GRAPH_MODE_EDGE_LIST => render_edge_list(sink, state, snapshot.graph_offset),
        GRAPH_MODE_NODE_DETAIL => {
            if snapshot.selected_edge.is_some() && snapshot.graph_total == 2 {
                render_nodes_for_selected_edge(sink, state);
            } else if let Some(vector) = snapshot.selected_node {
                render_node_detail(sink, state, vector);
            } else {
                render_graph_overview(sink, state, 0);
            }
        }
        GRAPH_MODE_EDGE_DETAIL => {
            if let Some(vector) = snapshot.selected_edge {
                render_edge_detail(sink, state, vector);
            } else {
                render_graph_overview(sink, state, 0);
            }
        }
        GRAPH_MODE_INFO => {
            if state.graph_context == GRAPH_CTX_METRICS {
                render_metrics(sink, state);
            } else {
                render_where(sink, state);
            }
        }
        _ => {}
    }
}

fn begin_graph_command(sink: &ConsoleSink, state: &mut ShellState) {
    state.len = 0;
    clear_command_area(sink);
}

fn parse_node_command(cmd: &str) -> Option<VectorAddress> {
    let trimmed = cmd.trim();
    let payload = trimmed.strip_prefix("node ")?;
    VectorAddress::parse(payload.trim())
}

fn is_vector_wrapper_char(ch: char) -> bool {
    matches!(ch, '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';')
}

fn parse_edge_vector_payload(payload: &str) -> Option<EdgeVector> {
    for raw in payload.split_ascii_whitespace() {
        let token = raw.trim_matches(is_vector_wrapper_char);
        let token = token
            .strip_prefix("vector=")
            .or_else(|| token.strip_prefix("vector:"))
            .or_else(|| token.strip_prefix("vec="))
            .or_else(|| token.strip_prefix("vec:"))
            .unwrap_or(token);
        let token = token.trim_matches(is_vector_wrapper_char);
        let token = token.strip_prefix("e:").unwrap_or(token);
        if let Some(vector) = EdgeVector::parse(token.trim_matches(is_vector_wrapper_char)) {
            return Some(vector);
        }
    }
    None
}

fn parse_edge_command(cmd: &str) -> Option<EdgeVector> {
    let trimmed = cmd.trim();
    let payload = trimmed.strip_prefix("edge ")?;
    parse_edge_vector_payload(payload.trim())
}

fn graph_page_stride(state: &ShellState) -> usize {
    match state.graph_mode {
        GRAPH_MODE_OVERVIEW => GRAPH_OVERVIEW_ITEMS,
        GRAPH_MODE_NODE_LIST | GRAPH_MODE_EDGE_LIST => GRAPH_PAGE_ITEMS,
        _ => GRAPH_PAGE_ITEMS,
    }
}

fn graph_page_offset_for_next(state: &ShellState) -> usize {
    normalize_page_offset(state.graph_offset + graph_page_stride(state), state.graph_total)
}

fn graph_page_offset_for_prev(state: &ShellState) -> usize {
    state.graph_offset.saturating_sub(graph_page_stride(state))
}

fn render_graph_next_page(sink: &ConsoleSink, state: &mut ShellState) {
    let offset = graph_page_offset_for_next(state);
    match state.graph_mode {
        GRAPH_MODE_OVERVIEW => render_graph_overview(sink, state, offset),
        GRAPH_MODE_NODE_LIST => render_node_list(sink, state, offset),
        GRAPH_MODE_EDGE_LIST => render_edge_list(sink, state, offset),
        _ => {}
    }
}

fn render_graph_prev_page(sink: &ConsoleSink, state: &mut ShellState) {
    let offset = graph_page_offset_for_prev(state);
    match state.graph_mode {
        GRAPH_MODE_OVERVIEW => render_graph_overview(sink, state, offset),
        GRAPH_MODE_NODE_LIST => render_node_list(sink, state, offset),
        GRAPH_MODE_EDGE_LIST => render_edge_list(sink, state, offset),
        _ => {}
    }
}

fn show_by_context(sink: &ConsoleSink, state: &mut ShellState, reset_offset: bool) {
    let offset = if reset_offset { 0 } else { state.graph_offset };
    match state.graph_context {
        GRAPH_CTX_NODE => render_edge_list(sink, state, offset),
        GRAPH_CTX_EDGE => render_nodes_for_selected_edge(sink, state),
        _ => render_graph_overview(sink, state, offset),
    }
}

fn handle_graph_page_key(sink: &ConsoleSink, state: &mut ShellState, byte: u8) -> bool {
    if state.menu_mode != MENU_MODE_COMMAND {
        return false;
    }
    match byte {
        INPUT_KEY_PAGE_UP => {
            if matches!(state.graph_mode, GRAPH_MODE_OVERVIEW | GRAPH_MODE_NODE_LIST | GRAPH_MODE_EDGE_LIST) {
                begin_graph_command(sink, state);
                render_graph_prev_page(sink, state);
                return true;
            }
        }
        INPUT_KEY_PAGE_DOWN => {
            if matches!(state.graph_mode, GRAPH_MODE_OVERVIEW | GRAPH_MODE_NODE_LIST | GRAPH_MODE_EDGE_LIST) {
                begin_graph_command(sink, state);
                render_graph_next_page(sink, state);
                return true;
            }
        }
        _ => {}
    }
    false
}

fn handle_command_history_key(sink: &ConsoleSink, state: &mut ShellState, byte: u8) -> bool {
    if state.menu_mode != MENU_MODE_COMMAND {
        return false;
    }

    let changed = match byte {
        INPUT_KEY_UP => command_history_prev(state),
        INPUT_KEY_DOWN => command_history_next(state),
        _ => false,
    };

    if changed {
        redraw_footer(sink, state, false);
        focus_footer_input(sink, state);
    }

    changed
}

fn handle_graph_command(sink: &ConsoleSink, state: &mut ShellState, cmd: &str) -> bool {
    if cmd == "back" {
        if state.graph_mode == GRAPH_MODE_NONE {
            return false;
        }
        begin_graph_command(sink, state);
        if let Some(snapshot) = pop_graph_nav_state(state) {
            restore_graph_nav_state(sink, state, snapshot);
        } else {
            render_graph_notice(sink, state, "GRAPH BACK", "no previous graph view", "", 12);
        }
        return true;
    }
    if cmd == "show" {
        begin_graph_command(sink, state);
        push_graph_nav_state(state);
        show_by_context(sink, state, true);
        return true;
    }
    if cmd == "show next" {
        begin_graph_command(sink, state);
        if state.graph_mode == GRAPH_MODE_NONE {
            push_graph_nav_state(state);
            render_graph_overview(sink, state, GRAPH_OVERVIEW_ITEMS);
        } else {
            render_graph_next_page(sink, state);
        }
        return true;
    }
    if cmd == "show prev" {
        begin_graph_command(sink, state);
        if state.graph_mode == GRAPH_MODE_NONE {
            push_graph_nav_state(state);
            render_graph_overview(sink, state, 0);
        } else {
            render_graph_prev_page(sink, state);
        }
        return true;
    }
    if cmd == "show mutations" {
        begin_graph_command(sink, state);
        push_graph_nav_state(state);
        render_mutations_overview(sink, state);
        return true;
    }
    if cmd == "node" {
        begin_graph_command(sink, state);
        if let Some(vector) = state.selected_node {
            push_graph_nav_state(state);
            render_node_detail(sink, state, vector);
        } else {
            render_graph_notice(sink, state, "NODE DETAIL", "no node selected", "use node <vector> first", 12);
        }
        return true;
    }
    if cmd == "edge" {
        begin_graph_command(sink, state);
        if let Some(vector) = state.selected_edge {
            push_graph_nav_state(state);
            render_edge_detail(sink, state, vector);
        } else {
            render_graph_notice(sink, state, "EDGE DETAIL", "no edge selected", "use edge <vector> or show from node", 12);
        }
        return true;
    }
    if cmd == "edge next" {
        begin_graph_command(sink, state);
        if state.graph_mode == GRAPH_MODE_EDGE_LIST {
            render_graph_next_page(sink, state);
        } else {
            if state.selected_node.is_some() {
                push_graph_nav_state(state);
            }
            render_edge_list(sink, state, GRAPH_PAGE_ITEMS);
        }
        return true;
    }
    if cmd == "edge prev" {
        begin_graph_command(sink, state);
        if state.graph_mode == GRAPH_MODE_EDGE_LIST {
            render_graph_prev_page(sink, state);
        } else {
            if state.selected_node.is_some() {
                push_graph_nav_state(state);
            }
            render_edge_list(sink, state, 0);
        }
        return true;
    }
    if cmd == "where" {
        begin_graph_command(sink, state);
        push_graph_nav_state(state);
        render_where(sink, state);
        return true;
    }
    if cmd == "metrics" {
        begin_graph_command(sink, state);
        push_graph_nav_state(state);
        render_metrics(sink, state);
        return true;
    }
    if cmd == "select clear" {
        clear_graph_selection(state);
        clear_command_area(sink);
        redraw_footer(sink, state, false);
        focus_footer_input(sink, state);
        return true;
    }
    if cmd == "activate" {
        begin_graph_command(sink, state);
        if let Some(vector) = state.selected_node {
            match gos_runtime::activate(vector) {
                Ok(_) => {
                    if is_theme_vector(vector) {
                        let theme = selected_theme();
                        let mut detail = LineBuf::<48>::new();
                        detail.push_str("theme.current -> ");
                        detail.push_vector(theme_vector(theme));
                        let message = core::str::from_utf8(detail.as_slice()).unwrap_or("theme link applied");
                        render_graph_notice(sink, state, "ACTIVATE", "theme relation applied", message, 10);
                    } else {
                        render_graph_notice(sink, state, "ACTIVATE", "node activation completed", "run node or show to refresh summaries", 10);
                    }
                }
                Err(_) => render_graph_notice(sink, state, "ACTIVATE", "node activation failed", "selected node is not activatable", 12),
            }
        } else {
            render_graph_notice(sink, state, "ACTIVATE", "no node selected", "use node <vector> first", 12);
        }
        return true;
    }
    if cmd == "spawn" {
        begin_graph_command(sink, state);
        if let Some(vector) = state.selected_node {
            match gos_runtime::post_signal(vector, Signal::Spawn { payload: 0 }) {
                Ok(_) => {
                    gos_runtime::pump();
                    render_graph_notice(sink, state, "SPAWN", "spawn signal dispatched", "run node or show to refresh summaries", 10);
                }
                Err(_) => render_graph_notice(sink, state, "SPAWN", "spawn dispatch failed", "selected node rejected the signal", 12),
            }
        } else {
            render_graph_notice(sink, state, "SPAWN", "no node selected", "use node <vector> first", 12);
        }
        return true;
    }
    if cmd == "vk" {
        begin_graph_command(sink, state);
        match gos_runtime::post_signal(
            gos_protocol::vectors::SVC_VK,
            Signal::Control { cmd: gos_protocol::VK_CONTROL_REPORT, val: 0 },
        ) {
            Ok(_) => {
                gos_runtime::pump();
                render_graph_notice(sink, state, "VK", "live graph frame dispatched", "run tools/gfx-bridge.py to view the graph surface", 10);
            }
            Err(_) => render_graph_notice(sink, state, "VK", "visual bridge unavailable", "k-vk-host did not accept the frame", 12),
        }
        return true;
    }
    if let Some(edge_vector) = parse_edge_command(cmd) {
        begin_graph_command(sink, state);
        push_graph_nav_state(state);
        render_edge_detail(sink, state, edge_vector);
        return true;
    }
    if let Some(vector) = parse_node_command(cmd) {
        begin_graph_command(sink, state);
        push_graph_nav_state(state);
        render_node_detail(sink, state, vector);
        return true;
    }
    false
}

fn starts_with_ci(text: &str, needle: &str) -> bool {
    let text = text.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > text.len() {
        return false;
    }
    for idx in 0..needle.len() {
        if !text[idx].eq_ignore_ascii_case(&needle[idx]) {
            return false;
        }
    }
    true
}

fn looks_like_cypher_query(cmd: &str) -> bool {
    let trimmed = cmd.trim_start();
    starts_with_ci(trimmed, "match ")
        || starts_with_ci(trimmed, "match(")
        || trimmed.eq_ignore_ascii_case("match")
}

fn dispatch_cypher_query(sink: &ConsoleSink, state: &mut ShellState, query: &str) -> bool {
    if state.cypher_target == 0 {
        set_color(sink, 12, 0);
        print_str(sink, " cypher node unresolved\n");
        return false;
    }

    if !emit_target_signal(
        sink,
        state.cypher_target,
        Signal::Control {
            cmd: CYPHER_CONTROL_QUERY_BEGIN,
            val: 0,
        },
    ) {
        set_color(sink, 12, 0);
        print_str(sink, " cypher lane refused query begin\n");
        return false;
    }

    for byte in query.bytes() {
        if !emit_target_signal(
            sink,
            state.cypher_target,
            Signal::Data {
                from: sink.from,
                byte,
            },
        ) {
            set_color(sink, 12, 0);
            print_str(sink, " cypher lane dropped query payload\n");
            return false;
        }
    }

    if !emit_target_signal(
        sink,
        state.cypher_target,
        Signal::Control {
            cmd: CYPHER_CONTROL_QUERY_COMMIT,
            val: 0,
        },
    ) {
        set_color(sink, 12, 0);
        print_str(sink, " cypher lane refused query commit\n");
        return false;
    }

    gos_runtime::pump();
    true
}

fn dispatch_cuda_submit(sink: &ConsoleSink, state: &mut ShellState, job: &str) -> bool {
    if state.cuda_target == 0 {
        set_color(sink, 12, 0);
        print_str(sink, " cuda bridge unresolved\n");
        return false;
    }

    if !emit_target_signal(
        sink,
        state.cuda_target,
        Signal::Control {
            cmd: CUDA_CONTROL_JOB_BEGIN,
            val: 0,
        },
    ) {
        set_color(sink, 12, 0);
        print_str(sink, " cuda bridge refused job begin\n");
        return false;
    }

    for byte in job.bytes() {
        if !emit_target_signal(
            sink,
            state.cuda_target,
            Signal::Data {
                from: sink.from,
                byte,
            },
        ) {
            set_color(sink, 12, 0);
            print_str(sink, " cuda bridge dropped job payload\n");
            return false;
        }
    }

    if !emit_target_signal(
        sink,
        state.cuda_target,
        Signal::Control {
            cmd: CUDA_CONTROL_JOB_COMMIT,
            val: 0,
        },
    ) {
        set_color(sink, 12, 0);
        print_str(sink, " cuda bridge refused job commit\n");
        return false;
    }

    gos_runtime::pump();
    true
}

fn draw_footer_shortcuts(sink: &ConsoleSink, state: &ShellState) {
    let phase = (state.sigil_frame as usize) / 2;
    fill_band(sink, FOOTER_SHORTCUT_ROW, 0, SCREEN_WIDTH, WABI_INK, WABI_INDIGO);
    draw_badge(
        sink,
        FOOTER_SHORTCUT_ROW,
        1,
        WABI_MOON,
        if state.menu_mode == MENU_MODE_AI_API {
            WABI_INDIGO
        } else {
            WABI_TEA
        },
        if state.menu_mode == MENU_MODE_AI_API { "API" } else { "CMD" },
    );
    if state.menu_mode == MENU_MODE_AI_API {
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 9, WABI_MOON, WABI_STONE, "^S");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 14, WABI_PAPER, WABI_INDIGO, "save");
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 21, WABI_MOON, WABI_MOSS, "ENT");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 27, WABI_PAPER, WABI_INDIGO, "apply");
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 35, WABI_MOON, WABI_TEA, "ESC");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 41, WABI_PAPER, WABI_INDIGO, "cancel");
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 51, WABI_MOON, WABI_STONE, "DEL");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 57, WABI_PAPER, WABI_INDIGO, "erase");
    } else {
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 9, WABI_MOON, WABI_STONE, "^A");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 14, WABI_PAPER, WABI_INDIGO, "ai-key");
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 23, WABI_MOON, WABI_STONE, "^L");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 28, WABI_PAPER, WABI_INDIGO, "ime");
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 34, WABI_MOON, WABI_STONE, "^C");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 39, WABI_PAPER, WABI_INDIGO, "copy");
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 46, WABI_MOON, WABI_TEA, "^X");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 51, WABI_PAPER, WABI_INDIGO, "cut");
        draw_badge(sink, FOOTER_SHORTCUT_ROW, 57, WABI_MOON, WABI_MOSS, "^V");
        draw_text(sink, FOOTER_SHORTCUT_ROW, 62, WABI_PAPER, WABI_INDIGO, "paste");
    }
    draw_repeat(sink, FOOTER_SHORTCUT_ROW, 69, WABI_STONE, WABI_INDIGO, CP437_LIGHT, 8);
    draw_repeat(
        sink,
        FOOTER_SHORTCUT_ROW,
        69 + ((phase * 2) % 6),
        WABI_PAPER,
        WABI_INDIGO,
        CP437_MEDIUM,
        2,
    );
    draw_byte(sink, FOOTER_SHORTCUT_ROW, 78, WABI_STONE, WABI_INDIGO, b'.');
}

fn draw_footer_status(sink: &ConsoleSink, state: &ShellState) {
    let shown_len = if state.menu_mode == MENU_MODE_AI_API {
        state.api_edit_len
    } else {
        state.api_len
    };
    fill_band(sink, FOOTER_STATUS_ROW, 0, SCREEN_WIDTH, WABI_INK, WABI_INK);
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        1,
        WABI_MOON,
        if state.input_lang == IME_MODE_ZH_PINYIN {
            WABI_MOSS
        } else {
            WABI_STONE
        },
        ime_mode_label(state.input_lang),
    );
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        9,
        if state.ai_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.ai_target == 0 { WABI_INK } else { WABI_INDIGO },
        "AI",
    );
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        14,
        if state.cypher_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cypher_target == 0 { WABI_INK } else { WABI_STONE },
        "CY",
    );
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        19,
        if state.net_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.net_target == 0 { WABI_INK } else { WABI_MOSS },
        "NET",
    );
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        25,
        if state.cuda_target == 0 { WABI_STONE } else { WABI_MOON },
        if state.cuda_target == 0 { WABI_INK } else { WABI_TEA },
        "CUDA",
    );
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        32,
        if clipboard_mounted(NODE_VEC) { WABI_MOON } else { WABI_STONE },
        if clipboard_mounted(NODE_VEC) { WABI_MOSS } else { WABI_INK },
        "CLIP",
    );
    draw_text(sink, FOOTER_STATUS_ROW, 39, WABI_PAPER, WABI_INK, "buf");
    draw_usize(sink, FOOTER_STATUS_ROW, 43, WABI_PAPER, WABI_INK, shown_len);
    draw_text(sink, FOOTER_STATUS_ROW, 48, WABI_PAPER, WABI_INK, "mode");
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        53,
        WABI_MOON,
        WABI_STONE,
        if state.menu_mode == MENU_MODE_AI_API {
            "api"
        } else {
            graph_mode_label(state.graph_mode)
        },
    );
    draw_text(sink, FOOTER_STATUS_ROW, 63, WABI_PAPER, WABI_INK, "ctx");
    draw_badge(
        sink,
        FOOTER_STATUS_ROW,
        67,
        WABI_MOON,
        if state.graph_context == GRAPH_CTX_NONE {
            WABI_STONE
        } else {
            WABI_INDIGO
        },
        graph_context_label(state.graph_context),
    );
}

fn draw_footer_input(sink: &ConsoleSink, state: &ShellState) {
    let phase = (state.sigil_frame as usize) / 2;
    fill_band(sink, FOOTER_INPUT_ROW, 0, SCREEN_WIDTH, WABI_PAPER, WABI_INK);
    draw_badge(
        sink,
        FOOTER_INPUT_ROW,
        0,
        WABI_MOON,
        if state.menu_mode == MENU_MODE_AI_API {
            WABI_INDIGO
        } else {
            WABI_STONE
        },
        if state.menu_mode == MENU_MODE_AI_API { "API" } else { "RUN" },
    );
    if state.menu_mode == MENU_MODE_AI_API {
        draw_text(sink, FOOTER_INPUT_ROW, 6, WABI_TEA, WABI_INK, "AI API KEY >");
        if state.api_edit_len == 0 {
            draw_text(
                sink,
                FOOTER_INPUT_ROW,
                20,
                WABI_STONE,
                WABI_INK,
                "type token for this boot session",
            );
        } else {
            let visible_width = SCREEN_WIDTH.saturating_sub(22);
            let start = state.api_edit_len.saturating_sub(visible_width);
            if start > 0 {
                draw_text(sink, FOOTER_INPUT_ROW, 20, WABI_STONE, WABI_INK, "...");
            }
            let col = if start > 0 { 23 } else { 20 };
            draw_bytes(
                sink,
                FOOTER_INPUT_ROW,
                col,
                WABI_MOON,
                WABI_INK,
                &state.api_buffer[start..state.api_edit_len],
            );
        }
    } else {
        let mut visible = [0u8; 128];
        let visible_len = command_display_bytes(state, &mut visible);
        draw_text(sink, FOOTER_INPUT_ROW, 6, WABI_TEA, WABI_INK, ">");

        let available = SCREEN_WIDTH.saturating_sub(9);
        if visible_len == 0 {
            draw_text(
                sink,
                FOOTER_INPUT_ROW,
                8,
                WABI_STONE,
                WABI_INK,
                "show / back / node <vec> / edge <vec> / ask <prompt>",
            );
        } else {
            let start = visible_len.saturating_sub(available);
            draw_bytes(
                sink,
                FOOTER_INPUT_ROW,
                8,
                WABI_MOON,
                WABI_INK,
                &visible[start..visible_len],
            );
        }

        if state.input_lang == IME_MODE_ZH_PINYIN && state.ime_preview_len > 0 {
            let preview_col = 58usize;
            if preview_col < SCREEN_WIDTH {
                draw_text(sink, FOOTER_INPUT_ROW, preview_col, WABI_SAGE, WABI_INK, "py:");
                let remaining = SCREEN_WIDTH.saturating_sub(preview_col + 3);
                let preview_len = state.ime_preview_len.min(remaining);
                draw_bytes(
                    sink,
                    FOOTER_INPUT_ROW,
                    preview_col + 3,
                    WABI_PAPER,
                    WABI_INK,
                    &state.ime_preview[..preview_len],
                );
            }
        }
    }
    draw_repeat(sink, FOOTER_INPUT_ROW, 70, WABI_STONE, WABI_INK, CP437_LIGHT, 8);
    draw_repeat(
        sink,
        FOOTER_INPUT_ROW,
        70 + ((phase * 2) % 6),
        WABI_PAPER,
        WABI_INK,
        CP437_MEDIUM,
        2,
    );
    draw_byte(sink, FOOTER_INPUT_ROW, 79, WABI_STONE, WABI_INK, b'.');
}

fn focus_footer_input(sink: &ConsoleSink, state: &ShellState) {
    let col = if state.menu_mode == MENU_MODE_AI_API {
        let visible_width = SCREEN_WIDTH.saturating_sub(22);
        let visible_len = state.api_edit_len.min(visible_width);
        if state.api_edit_len > visible_width {
            23 + visible_len
        } else {
            20 + visible_len
        }
    } else {
        let mut visible = [0u8; 128];
        let visible_len = command_display_bytes(state, &mut visible);
        let available = SCREEN_WIDTH.saturating_sub(9);
        let shown_len = visible_len.min(available);
        8 + shown_len
    };
    goto(sink, FOOTER_INPUT_ROW, col.min(SCREEN_WIDTH - 1));
    set_color(sink, WABI_MOON, WABI_INK);
}

fn restore_output_cursor(sink: &ConsoleSink) {
    restore_cursor(sink, 1);
}

fn save_output_cursor(sink: &ConsoleSink) {
    save_cursor(sink, 1);
}

fn echo_command_line(sink: &ConsoleSink, state: &ShellState) {
    if state.len == 0 {
        return;
    }

    let mut visible = [0u8; 128];
    let visible_len = command_display_bytes(state, &mut visible);
    set_color(sink, WABI_TEA, WABI_INK);
    print_str(sink, "> ");
    set_color(sink, WABI_MOON, WABI_INK);
    if visible_len > 0 {
        let text = core::str::from_utf8(&visible[..visible_len]).unwrap_or("");
        print_str(sink, text);
    }
    print_str(sink, "\n");
}

fn redraw_footer(sink: &ConsoleSink, state: &ShellState, preserve_cursor: bool) {
    if preserve_cursor {
        save_cursor(sink, 0);
    }
    draw_footer_shortcuts(sink, state);
    draw_footer_status(sink, state);
    draw_footer_input(sink, state);
    let _ = preserve_cursor;
    focus_footer_input(sink, state);
}

fn enter_ai_api_mode(sink: &ConsoleSink, state: &mut ShellState) {
    state.menu_mode = MENU_MODE_AI_API;
    state.api_buffer = [0; 128];
    state.api_edit_len = 0;
    state.len = 0;
    redraw_footer(sink, state, false);
}

fn exit_ai_api_mode(sink: &ConsoleSink, state: &mut ShellState, message: &str, fg: u8) {
    state.menu_mode = MENU_MODE_COMMAND;
    restore_cursor(sink, 1);
    print_str(sink, "\n");
    set_color(sink, fg, 0);
    print_str(sink, message);
    print_str(sink, "\n");
    save_cursor(sink, 1);
    redraw_ai_panel(sink, state, true);
    redraw_footer(sink, state, false);
    focus_footer_input(sink, state);
}

fn commit_ai_api(sink: &ConsoleSink, state: &mut ShellState) -> bool {
    if !emit_target_signal(sink, state.ai_target, Signal::Control { cmd: AI_CONTROL_API_BEGIN, val: 0 }) {
        return false;
    }

    for byte in &state.api_buffer[..state.api_edit_len] {
        if !emit_target_signal(sink, state.ai_target, Signal::Data { from: sink.from, byte: *byte }) {
            return false;
        }
    }

    if !emit_target_signal(sink, state.ai_target, Signal::Control { cmd: AI_CONTROL_API_COMMIT, val: 0 }) {
        return false;
    }

    state.api_len = state.api_edit_len;
    state.api_configured = u8::from(state.api_len > 0);
    if state.api_configured != 0 {
        push_ai_text(state, "sys> api key armed");
    }
    true
}

fn clear_rect(sink: &ConsoleSink, top: usize, left: usize, width: usize, height: usize) {
    for row in 0..height {
        fill_band(sink, top + row, left, width, 0, 0);
    }
}

fn draw_sigil_layer(sink: &ConsoleSink, top: i32, left: i32, primary_fg: u8, secondary_fg: u8) {
    let top = top.clamp(4, 8) as usize;
    let left = left.clamp(49, 50) as usize;
    for (idx, row) in LIVE_SIGIL_ROWS.iter().enumerate() {
        let fg = if idx % 2 == 0 { primary_fg } else { secondary_fg };
        draw_bytes(sink, top + idx, left, fg, 0, row);
    }
}

fn draw_console_sigil(sink: &ConsoleSink, frame: usize) {
    let phase = frame % LIVE_SIGIL_FRAMES;
    let current_x = LIVE_SHAKE_X[phase] as i32;
    let current_y = LIVE_SHAKE_Y[phase] as i32;
    let prev_phase = if phase == 0 { LIVE_SIGIL_FRAMES - 1 } else { phase - 1 };
    let velocity_x = current_x - LIVE_SHAKE_X[prev_phase] as i32;
    let velocity_y = current_y - LIVE_SHAKE_Y[prev_phase] as i32;
    let base_top = LIVE_SIGIL_TOP as i32 + current_y;
    let base_left = LIVE_SIGIL_LEFT as i32 + current_x;
    let primary_fg = match phase {
        0 | 4 => WABI_PAPER,
        1 | 2 => WABI_STONE,
        3 => WABI_MOON,
        5 | 6 => WABI_TEA,
        _ => WABI_SAGE,
    };
    let secondary_fg = match phase {
        0 | 1 => WABI_STONE,
        2 | 3 => WABI_PAPER,
        4 | 5 => WABI_SAGE,
        _ => WABI_TEA,
    };

    clear_rect(
        sink,
        LIVE_SIGIL_TOP.saturating_sub(1),
        LIVE_SIGIL_LEFT.saturating_sub(1),
        LIVE_SIGIL_WIDTH,
        LIVE_SIGIL_HEIGHT,
    );

    draw_sigil_layer(
        sink,
        base_top - velocity_y,
        base_left - velocity_x,
        WABI_STONE,
        WABI_INDIGO,
    );
    draw_sigil_layer(
        sink,
        base_top - velocity_y,
        base_left - velocity_x,
        WABI_STONE,
        WABI_MOSS,
    );
    draw_sigil_layer(sink, base_top, base_left, primary_fg, secondary_fg);

    for (idx, (dy, dx)) in LIVE_SPARKS[phase].iter().enumerate() {
        let row = (base_top + *dy as i32).clamp(4, 9) as usize;
        let col = (base_left + *dx as i32).clamp(49, 51) as usize;
        let (fg, byte) = if idx % 2 == 0 {
            (WABI_PAPER, b'.')
        } else {
            (WABI_STONE, CP437_LIGHT)
        };
        draw_byte(sink, row, col, fg, WABI_INK, byte);
        if velocity_x != 0 || velocity_y != 0 {
            let trail_row = (row as i32 - velocity_y).clamp(4, 9) as usize;
            let trail_col = (col as i32 - velocity_x).clamp(49, 51) as usize;
            draw_byte(sink, trail_row, trail_col, WABI_STONE, WABI_INK, CP437_LIGHT);
        }
    }
}

fn redraw_ai_panel(sink: &ConsoleSink, state: &ShellState, preserve_cursor: bool) {
    if preserve_cursor {
        save_cursor(sink, 0);
    }
    draw_ai_panel(sink, state);
    if preserve_cursor {
        restore_cursor(sink, 0);
    }
}

fn redraw_console(sink: &ConsoleSink, state: &ShellState) {
    let snapshot = gos_runtime::snapshot();
    clear_canvas(sink);
    set_scroll_top(sink, COMMAND_SCROLL_TOP);
    set_scroll_bottom(sink, COMMAND_SCROLL_BOTTOM);
    draw_runtime_header(sink, state, snapshot);
    draw_command_deck_panel(sink, state, snapshot);
    draw_runtime_gap_flux(sink, state);
    draw_console_sigil(sink, state.sigil_frame as usize);
    draw_ai_panel(sink, state);
    draw_operator_band(sink, state, snapshot);
    goto(sink, COMMAND_SCROLL_TOP, 4);
    save_cursor(sink, 1);
    redraw_footer(sink, state, false);
    focus_footer_input(sink, state);
}

/// Label for a topological-index value that may have been clamped.
///
/// The `graph_topo_indices*` runtime functions accumulate in `u128` and finish
/// with `.min(u64::MAX as u128) as u64`, so any index whose true value exceeds
/// `u64::MAX` is reported as `u64::MAX`. Tagging such a value "(exact)" claims a
/// precision the arithmetic did not deliver; high-exponent indices saturate on
/// essentially every non-trivial graph.
fn sat_label(v: u64) -> &'static str {
    if v == u64::MAX {
        "(SATURATED \u{2014} exceeds u64)"
    } else {
        "(exact)"
    }
}

fn print_num_inline(sink: &ConsoleSink, mut value: usize) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while value > 0 {
            buf[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
    }

    while len > 0 {
        len -= 1;
        print_byte(sink, buf[len]);
    }
}

fn print_hex32_inline(sink: &ConsoleSink, value: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = value.to_be_bytes();
    for b in bytes {
        print_byte(sink, HEX[(b >> 4) as usize]);
        print_byte(sink, HEX[(b & 0xF) as usize]);
    }
}

fn resolve_capability_target(
    ctx: *mut ExecutorContext,
    namespace: &'static [u8],
    capability: &'static [u8],
) -> u64 {
    let ctx_ref = unsafe { &*ctx };
    let abi = unsafe { &*ctx_ref.abi };
    if let Some(resolve_capability) = abi.resolve_capability {
        unsafe {
            resolve_capability(
                namespace.as_ptr(),
                namespace.len(),
                capability.as_ptr(),
                capability.len(),
            )
        }
    } else {
        0
    }
}

unsafe fn clipboard_state_mut(ctx: *mut ExecutorContext) -> &'static mut ClipboardState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut ClipboardState) }
}

fn clipboard_request_allowed(from: u64) -> bool {
    if from == 0 {
        return false;
    }
    clipboard_mounted(VectorAddress::from_u64(from))
}

unsafe extern "C" fn clipboard_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut ClipboardState,
            ClipboardState {
                bytes: [0; CLIPBOARD_MAX_BYTES],
                len: 0,
                capture_from: 0,
                capture_len: 0,
                capture_active: 0,
            },
        );
    }
    CLIPBOARD_BYTES.store(0, Ordering::SeqCst);
    ExecStatus::Done
}

unsafe extern "C" fn clipboard_on_event(
    ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    let state = unsafe { clipboard_state_mut(ctx) };
    let signal = packet_to_signal(unsafe { (*event).signal });

    match signal {
        Signal::Call { from } => {
            if !clipboard_request_allowed(from) {
                return ExecStatus::Done;
            }

            let target = VectorAddress::from_u64(from);
            let mut idx = 0usize;
            while idx < state.len {
                let _ = gos_runtime::post_signal(
                    target,
                    Signal::Data {
                        from: CLIPBOARD_NODE_VEC.as_u64(),
                        byte: state.bytes[idx],
                    },
                );
                idx += 1;
            }
            ExecStatus::Done
        }
        Signal::Data { from, byte } => {
            if !clipboard_request_allowed(from) {
                return ExecStatus::Done;
            }

            match byte {
                CLIPBOARD_DATA_BEGIN => {
                    state.capture_from = from;
                    state.capture_len = 0;
                    state.capture_active = 1;
                }
                CLIPBOARD_DATA_COMMIT => {
                    if state.capture_active != 0 && state.capture_from == from {
                        state.len = state.capture_len.min(state.bytes.len());
                        CLIPBOARD_BYTES.store(state.len, Ordering::SeqCst);
                    }
                    state.capture_active = 0;
                    state.capture_from = 0;
                    state.capture_len = 0;
                }
                CLIPBOARD_DATA_CLEAR => {
                    state.bytes = [0; CLIPBOARD_MAX_BYTES];
                    state.len = 0;
                    state.capture_active = 0;
                    state.capture_from = 0;
                    state.capture_len = 0;
                    CLIPBOARD_BYTES.store(0, Ordering::SeqCst);
                }
                _ => {
                    if state.capture_active != 0
                        && state.capture_from == from
                        && state.capture_len < state.bytes.len()
                    {
                        state.bytes[state.capture_len] = byte;
                        state.capture_len += 1;
                    }
                }
            }
            ExecStatus::Done
        }
        _ => ExecStatus::Done,
    }
}

unsafe extern "C" fn theme_on_resume(ctx: *mut ExecutorContext) -> ExecStatus {
    let vector = unsafe { (*ctx).vector };
    let theme = if vector == THEME_CURRENT_NODE_VEC {
        selected_theme()
    } else if let Some(theme) = theme_kind_for_vector(vector) {
        theme
    } else {
        return ExecStatus::Done;
    };
    let console_target = resolve_capability_target(ctx, b"console", b"write");
    let ctx_ref = unsafe { &*ctx };
    let abi = unsafe { &*ctx_ref.abi };
    let _ = apply_theme_choice_raw(abi, vector.as_u64(), console_target, theme);
    ExecStatus::Done
}

unsafe extern "C" fn shell_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let console_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"console".as_ptr(),
                    b"console".len(),
                    b"write".as_ptr(),
                    b"write".len(),
                )
            }
        } else {
            0
        }
    };

    let ai_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"ai".as_ptr(),
                    b"ai".len(),
                    b"supervisor".as_ptr(),
                    b"supervisor".len(),
                )
            }
        } else {
            0
        }
    };

    let cypher_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"cypher".as_ptr(),
                    b"cypher".len(),
                    b"query".as_ptr(),
                    b"query".len(),
                )
            }
        } else {
            0
        }
    };

    let ime_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"ime".as_ptr(),
                    b"ime".len(),
                    b"control".as_ptr(),
                    b"control".len(),
                )
            }
        } else {
            0
        }
    };

    let net_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"net".as_ptr(),
                    b"net".len(),
                    b"uplink".as_ptr(),
                    b"uplink".len(),
                )
            }
        } else {
            0
        }
    };

    let cuda_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"cuda".as_ptr(),
                    b"cuda".len(),
                    b"bridge".as_ptr(),
                    b"bridge".len(),
                )
            }
        } else {
            0
        }
    };

    let clipboard_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"clipboard".as_ptr(),
                    b"clipboard".len(),
                    b"buffer".as_ptr(),
                    b"buffer".len(),
                )
            }
        } else {
            0
        }
    };

    // Resolve k-chat capability
    let chat_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"chat".as_ptr(),
                    b"chat".len(),
                    b"bridge".as_ptr(),
                    b"bridge".len(),
                )
            }
        } else {
            0
        }
    };
    CHAT_TARGET.store(chat_target, Ordering::SeqCst);
    CHAT_MODE.store(0, Ordering::SeqCst);

    // Resolve k-nim capability
    let nim_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"nim".as_ptr(),
                    b"nim".len(),
                    b"inference".as_ptr(),
                    b"inference".len(),
                )
            }
        } else {
            0
        }
    };
    NIM_TARGET.store(nim_target, Ordering::SeqCst);
    NIM_MODE.store(0, Ordering::SeqCst);

    // Resolve k-virtio-gpu capability (ADR-013 discovery skeleton)
    let gpu_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"gpu".as_ptr(),
                    b"gpu".len(),
                    b"status".as_ptr(),
                    b"status".len(),
                )
            }
        } else {
            0
        }
    };
    GPU_TARGET.store(gpu_target, Ordering::SeqCst);

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut ShellState,
            ShellState {
                buffer: [0; 128],
                len: 0,
                selected_node: None,
                selected_edge: None,
                graph_mode: GRAPH_MODE_NONE,
                graph_context: GRAPH_CTX_NONE,
                graph_offset: 0,
                graph_total: 0,
                graph_nav: [GraphNavState::EMPTY; GRAPH_NAV_DEPTH],
                graph_nav_len: 0,
                ai_lines: [[0; AI_PANEL_LINE_WIDTH]; AI_PANEL_LINES],
                ai_line_lens: [0; AI_PANEL_LINES],
                ai_stream: [0; AI_PANEL_LINE_WIDTH],
                ai_stream_len: 0,
                ime_preview: [0; MAX_IME_PREVIEW],
                ime_preview_len: 0,
                ime_utf8_tail: 0,
                command_history: [[0; 128]; COMMAND_HISTORY_ITEMS],
                command_history_lens: [0; COMMAND_HISTORY_ITEMS],
                command_history_len: 0,
                command_history_cursor: 0,
                command_history_active: 0,
                command_history_draft: [0; 128],
                command_history_draft_len: 0,
                api_buffer: [0; 128],
                api_edit_len: 0,
                api_len: 0,
                console_target: if console_target == 0 {
                    VGA_VEC.as_u64()
                } else {
                    console_target
                },
                ime_target,
                ai_target,
                cypher_target,
                net_target,
                cuda_target,
                clipboard_target: if clipboard_target == 0 {
                    CLIPBOARD_NODE_VEC.as_u64()
                } else {
                    clipboard_target
                },
                last_rendered_epoch: 0,
                console_live: 0,
                sigil_frame: 0,
                heartbeat_divider: 0,
                menu_mode: MENU_MODE_COMMAND,
                input_lang: IME_MODE_ASCII,
                api_configured: 0,
                watch_active: 0,
                watch_tick: 0,
                watch_buf_len: 0,
                watch_buf: [0; 64],
            },
        );
    }
    // V2.15: register reactive node props so fire_subscribers encodes the active
    // theme index in DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED signal val.
    let _ = gos_runtime::register_node_prop_u8(THEME_WABI_NODE_ID, DISPLAY_THEME_WABI);
    let _ = gos_runtime::register_node_prop_u8(THEME_SHOJI_NODE_ID, DISPLAY_THEME_SHOJI);
    // V2.56: bind each theme node's primary palette color so the renderer can
    // call node_attr_get(theme_vec) instead of indexing the hardcoded PAL_U32 array.
    // PAL_U32[DISPLAY_THEME_WABI=0]=0x00DB_1C21 (RED), [1]=0x00ED_EDF2 (WHITE).
    let _ = gos_runtime::register_node_prop_u32(THEME_WABI_NODE_ID, 0x00DB_1C21);
    let _ = gos_runtime::register_node_prop_u32(THEME_SHOJI_NODE_ID, 0x00ED_EDF2);
    // V2.62: bind CYAN and GOLD to dedicated palette nodes — all 4 palette entries
    // are now graph-native.  PAL_U32[2]=0x0000_CCFF (CYAN), [3]=0x00FF_CC44 (GOLD).
    let _ = gos_runtime::register_node_prop_u32(PALETTE_CYAN_NODE_ID, 0x0000_CCFF);
    let _ = gos_runtime::register_node_prop_u32(PALETTE_GOLD_NODE_ID, 0x00FF_CC44);
    // Subscribe: k-vga auto-repaints when theme.current Use-edge changes.
    let k_vga_node_id = derive_node_id(PluginId::from_ascii("K_VGA"), "vga.entry");
    let _ = gos_runtime::register_subscribe(THEME_CURRENT_NODE_ID, k_vga_node_id);

    let sink = sink_from_ctx(ctx);
    let _ = apply_theme_choice(&sink, THEME_KIND_WABI);
    seed_ai_panel(unsafe { state_mut(ctx) });
    ExecStatus::Done
}

unsafe extern "C" fn shell_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    // ── Pre-processing: decode signal, classify by source ────────────────────────────
    let Some(input) = (unsafe { pre::prepare(ctx, event) }) else {
        return ExecStatus::Done;
    };
    // ── Main processing: run shell state machine ────────────────────────────────────
    let Some(output) = (unsafe { proc::process(ctx, input) }) else {
        return ExecStatus::Done;
    };
    // ── Post-processing: return ExecStatus to scheduler ─────────────────────────────
    post::emit(output)
}

unsafe extern "C" fn shell_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

#[cfg(test)]
mod tests {
    use super::{parse_edge_command, parse_edge_vector_payload};
    use gos_protocol::EdgeVector;

    #[test]
    fn parse_edge_command_accepts_plain_vector() {
        assert_eq!(
            parse_edge_command("edge e:17.34.51.68"),
            Some(EdgeVector::new(17, 34, 51, 68))
        );
    }

    #[test]
    fn parse_edge_command_accepts_vector_embedded_in_edge_row_text() {
        assert_eq!(
            parse_edge_command("edge out e:17.34.51.68 call 6.1.0.0 -> 6.1.0.1"),
            Some(EdgeVector::new(17, 34, 51, 68))
        );
    }

    #[test]
    fn parse_edge_payload_accepts_vector_field_wrappers() {
        assert_eq!(
            parse_edge_vector_payload("vector:'e:17.34.51.68'"),
            Some(EdgeVector::new(17, 34, 51, 68))
        );
    }
}
