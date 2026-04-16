#![no_std]


// ==============================================================
// GOS KERNEL TOPOLOGY — k-shell (native.shell)
// 以下 Cypher 脚本可直接导入 Neo4j，与其他模块共同还原内核完整图谱。
//
// MERGE (p:Plugin {id: "K_SHELL", name: "k-shell"})
// SET p.executor = "native.shell", p.node_type = "PluginEntry", p.state_schema = "0x200E"
//
// // ── 启动依赖 (DEPENDS_ON) ──────────────────────────────────
// MERGE (k_vga:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_vga)
// MERGE (k_ps2:Plugin {id: "K_PS2"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_ps2)
// MERGE (k_heap:Plugin {id: "K_HEAP"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_heap)
// MERGE (k_ime:Plugin {id: "K_IME"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_ime)
// MERGE (k_net:Plugin {id: "K_NET"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_net)
// MERGE (k_cypher:Plugin {id: "K_CYPHER"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_cypher)
// MERGE (k_cuda:Plugin {id: "K_CUDA"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_cuda)
//
// // ── 能力导出 (EXPORTS Capability) ────────────────────────
// MERGE (cap_shell_input:Capability {namespace: "shell", name: "input"})
// MERGE (p)-[:EXPORTS]->(cap_shell_input)
// MERGE (cap_clipboard_buffer:Capability {namespace: "clipboard", name: "buffer"})
// MERGE (p)-[:EXPORTS]->(cap_clipboard_buffer)
//
// // ── 能力消费 (IMPORTS Capability, resolved at on_init) ───
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
// ==============================================================

use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use gos_protocol::{
    derive_edge_id, derive_node_id, packet_to_signal, signal_to_packet,
    AI_CONTROL_API_BEGIN, AI_CONTROL_API_COMMIT, AI_CONTROL_CHAT_BEGIN,
    AI_CONTROL_CHAT_COMMIT, CLIPBOARD_DATA_BEGIN, CLIPBOARD_DATA_CLEAR,
    CLIPBOARD_DATA_COMMIT, CUDA_CONTROL_JOB_BEGIN, CUDA_CONTROL_JOB_COMMIT,
    CUDA_CONTROL_REPORT, CUDA_CONTROL_RESET, CYPHER_CONTROL_QUERY_BEGIN,
    CYPHER_CONTROL_QUERY_COMMIT, DISPLAY_CONTROL_THEME, DISPLAY_THEME_SHOJI,
    DISPLAY_THEME_WABI, EdgeSpec, EdgeVector, ExecStatus, ExecutorContext,
    ExecutorId, GraphEdgeDirection, GraphEdgeSummary, GraphNodeSummary,
    IME_CONTROL_SET_MODE, IME_MODE_ASCII, IME_MODE_ZH_PINYIN, INPUT_KEY_DOWN,
    INPUT_KEY_PAGE_DOWN, INPUT_KEY_PAGE_UP, INPUT_KEY_UP, KernelAbi,
    NET_CONTROL_PROBE, NET_CONTROL_REPORT, NET_CONTROL_RESET, NodeEvent,
    NodeExecutorVTable, PluginId, RoutePolicy, RuntimeEdgeType, Signal,
    VectorAddress,
};

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 0, 0);
pub const THEME_WABI_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 1, 0);
pub const THEME_SHOJI_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 2, 0);
pub const THEME_CURRENT_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 3, 0);
pub const CLIPBOARD_NODE_VEC: VectorAddress = VectorAddress::new(6, 1, 4, 0);
const VGA_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.shell");
pub const THEME_EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.theme");
pub const CLIPBOARD_EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.clip");
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

const SCREEN_WIDTH: usize = 80;
const STAGE_COUNT: usize = 5;
const PULSE_COUNT: usize = 3;
const FRAME_COUNT: usize = STAGE_COUNT * PULSE_COUNT;
const EVENT_LINES: usize = 4;
const LIVE_SIGIL_FRAMES: usize = 12;
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
const THEME_EDGE_KEY: &str = "theme.use";
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

static ACTIVE_THEME: AtomicU8 = AtomicU8::new(THEME_KIND_WABI);
static CLIPBOARD_BYTES: AtomicUsize = AtomicUsize::new(0);

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
    console_live: u8,
    sigil_frame: u8,
    heartbeat_divider: u8,
    menu_mode: u8,
    input_lang: u8,
    api_configured: u8,
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

fn theme_edge_id(theme: u8) -> gos_protocol::EdgeId {
    derive_edge_id(THEME_CURRENT_NODE_ID, theme_node_id(theme), THEME_EDGE_KEY)
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

fn sync_theme_use_edges(theme: u8) -> bool {
    let _ = gos_runtime::unregister_edge(theme_edge_id(THEME_KIND_WABI));
    let _ = gos_runtime::unregister_edge(theme_edge_id(THEME_KIND_SHOJI));

    let spec = EdgeSpec {
        edge_id: theme_edge_id(theme),
        from_node: THEME_CURRENT_NODE_ID,
        to_node: theme_node_id(theme),
        edge_type: RuntimeEdgeType::Use,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    };

    gos_runtime::register_edge(spec).is_ok()
}

fn apply_theme_choice_raw(abi: &KernelAbi, from: u64, console_target: u64, theme: u8) -> bool {
    let graph_ok = sync_theme_use_edges(theme);
    let target = if console_target == 0 {
        VGA_VEC.as_u64()
    } else {
        console_target
    };
    let visual_ok = emit_target_signal_raw(
        abi,
        target,
        Signal::Control {
            cmd: DISPLAY_CONTROL_THEME,
            val: theme,
        },
    );
    ACTIVE_THEME.store(theme, Ordering::SeqCst);
    if from != 0 && from != NODE_VEC.as_u64() {
        let _ = gos_runtime::post_signal(NODE_VEC, Signal::Interrupt { irq: 32 });
    }
    graph_ok && visual_ok
}

fn apply_theme_choice(sink: &ConsoleSink, theme: u8) -> bool {
    apply_theme_choice_raw(sink.abi, sink.from, sink.target, theme)
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
        if (idx + phase) % 4 == 0 {
            draw_byte(sink, row, 50, WABI_STONE, WABI_INK, b'.');
        }
    }
    draw_byte(sink, 3 + (phase % 8), 50, WABI_PAPER, WABI_INK, b'.');
}

fn draw_command_deck_panel(
    sink: &ConsoleSink,
    state: &ShellState,
    snapshot: gos_protocol::GraphSnapshot,
) {
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
        (snapshot.signal_queue_len * 2).min(11),
        WABI_TEA,
        WABI_INK,
    );

    draw_text(sink, 7, 4, WABI_STONE, WABI_INK, "quick");
    draw_badge(sink, 7, 10, WABI_MOON, WABI_STONE, "show");
    draw_badge(sink, 7, 17, WABI_MOON, WABI_INDIGO, "node");
    draw_badge(sink, 7, 24, WABI_MOON, WABI_TEA, "edge");
    draw_badge(sink, 7, 31, WABI_MOON, WABI_MOSS, "back");
    draw_badge(sink, 7, 38, WABI_MOON, WABI_STONE, "where");

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
        (snapshot.signal_queue_len * 2).min(6),
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
    let width = 23usize;
    let height = 11usize;

    for y in 0..height {
        let mut row = [b' '; 23];
        let dy = y as i32 - 5;
        for x in 0..width {
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

            if byte != b' ' && ((x + frame + y) % 9 == 0) {
                byte = CP437_LIGHT;
            }

            row[x] = byte;
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
    draw_metric_line(sink, 7, "sig", snapshot.signal_queue_len, snapshot.signal_queue_len.max(1), stage, pulse, 12);
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
    footer.push_dec(((total + GRAPH_OVERVIEW_ITEMS - 1) / GRAPH_OVERVIEW_ITEMS).max(1) as u64);
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
    footer.push_dec(((total + GRAPH_PAGE_ITEMS - 1) / GRAPH_PAGE_ITEMS).max(1) as u64);
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
    footer.push_dec(((total + GRAPH_PAGE_ITEMS - 1) / GRAPH_PAGE_ITEMS).max(1) as u64);
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

    render_graph_footer(sink, state, "where  select clear");
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
        GRAPH_MODE_INFO => render_where(sink, state),
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
        if text[idx].to_ascii_lowercase() != needle[idx].to_ascii_lowercase() {
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
                console_live: 0,
                sigil_frame: 0,
                heartbeat_divider: 0,
                menu_mode: MENU_MODE_COMMAND,
                input_lang: IME_MODE_ASCII,
                api_configured: 0,
            },
        );
    }
    let sink = sink_from_ctx(ctx);
    let _ = apply_theme_choice(&sink, THEME_KIND_WABI);
    seed_ai_panel(unsafe { state_mut(ctx) });
    ExecStatus::Done
}

unsafe extern "C" fn shell_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };
    let signal = packet_to_signal(unsafe { (*event).signal });

    match signal {
        Signal::Data { from, byte } => {
            if from == state.ime_target {
                if state.menu_mode == MENU_MODE_COMMAND {
                    append_command_byte(&sink, state, byte, true);
                }
                return ExecStatus::Done;
            }

            if from == state.clipboard_target {
                append_clipboard_byte(&sink, state, byte);
                return ExecStatus::Done;
            }

            if from == state.ai_target {
                append_ai_stream_byte(state, byte);
                redraw_ai_panel(&sink, state, true);
                return ExecStatus::Done;
            }

            if handle_graph_page_key(&sink, state, byte) {
                return ExecStatus::Done;
            }

            if handle_command_history_key(&sink, state, byte) {
                return ExecStatus::Done;
            }

            if byte == 0x01 && state.menu_mode != MENU_MODE_AI_API {
                enter_ai_api_mode(&sink, state);
                return ExecStatus::Done;
            }

            if state.menu_mode == MENU_MODE_AI_API {
                match byte {
                    0x03 => {
                        let _ = clipboard_copy_active_input(&sink, state);
                    }
                    0x16 => {
                        let _ = clipboard_paste_active_input(&sink, state);
                    }
                    0x18 => {
                        let _ = clipboard_cut_active_input(&sink, state);
                    }
                    b'\n' | b'\r' | 0x13 => {
                        if commit_ai_api(&sink, state) {
                            exit_ai_api_mode(&sink, state, " ai uplink armed for this boot session", 10);
                        } else {
                            state.api_configured = 0;
                            exit_ai_api_mode(&sink, state, " ai uplink commit failed", 12);
                        }
                    }
                    0x1B => {
                        exit_ai_api_mode(&sink, state, " ai uplink edit cancelled", 14);
                    }
                    0x08 | 0x7F => {
                        if state.api_edit_len > 0 {
                            reset_command_history_cursor(state);
                            state.api_edit_len -= 1;
                            state.api_buffer[state.api_edit_len] = 0;
                        }
                        redraw_footer(&sink, state, false);
                    }
                    0x20..=0x7E => {
                        if state.api_edit_len < state.api_buffer.len() {
                            state.api_buffer[state.api_edit_len] = byte;
                            state.api_edit_len += 1;
                        }
                        redraw_footer(&sink, state, false);
                    }
                    _ => {}
                }
                return ExecStatus::Done;
            }

            if byte == 0x0C {
                let next_lang = if state.input_lang == IME_MODE_ZH_PINYIN {
                    IME_MODE_ASCII
                } else {
                    IME_MODE_ZH_PINYIN
                };
                if sync_input_lang(&sink, state, next_lang) {
                    redraw_footer(&sink, state, true);
                } else {
                    restore_output_cursor(&sink);
                    set_color(&sink, 12, 0);
                    print_str(&sink, "\n ime node unresolved\n");
                    save_output_cursor(&sink);
                    redraw_footer(&sink, state, false);
                }
                return ExecStatus::Done;
            }

            if state.input_lang == IME_MODE_ZH_PINYIN {
                match byte {
                    b'a'..=b'z' | b'A'..=b'Z' => {
                        if state.ime_preview_len < state.ime_preview.len() {
                            state.ime_preview[state.ime_preview_len] = byte.to_ascii_lowercase();
                            state.ime_preview_len += 1;
                            let _ = emit_target_signal(
                                &sink,
                                state.ime_target,
                                Signal::Data {
                                    from: sink.from,
                                    byte,
                                },
                            );
                            redraw_footer(&sink, state, true);
                        }
                        return ExecStatus::Done;
                    }
                    0x08 | 0x7F => {
                        if state.ime_preview_len > 0 {
                            state.ime_preview_len -= 1;
                            state.ime_preview[state.ime_preview_len] = 0;
                            let _ = emit_target_signal(
                                &sink,
                                state.ime_target,
                                Signal::Data {
                                    from: sink.from,
                                    byte: 0x08,
                                },
                            );
                            redraw_footer(&sink, state, true);
                            return ExecStatus::Done;
                        }
                    }
                    0x1B | 0x03 => {
                        if state.ime_preview_len > 0 {
                            let _ = emit_target_signal(
                                &sink,
                                state.ime_target,
                                Signal::Data {
                                    from: sink.from,
                                    byte: 0x1B,
                                },
                            );
                            clear_ime_preview(state);
                            redraw_footer(&sink, state, true);
                            return ExecStatus::Done;
                        }
                    }
                    b'1'..=b'9' => {
                        if state.ime_preview_len > 0 {
                            commit_ime_preview(&sink, state, byte);
                            redraw_footer(&sink, state, true);
                            return ExecStatus::Done;
                        }
                    }
                    b' ' => {
                        if state.ime_preview_len > 0 {
                            commit_ime_preview(&sink, state, b' ');
                            redraw_footer(&sink, state, true);
                            return ExecStatus::Done;
                        }
                    }
                    b'\n' | b'\r' => {
                        if state.ime_preview_len > 0 {
                            commit_ime_preview(&sink, state, b'\n');
                            redraw_footer(&sink, state, true);
                            return ExecStatus::Done;
                        }
                    }
                    _ if is_ascii_punctuation(byte) && state.ime_preview_len > 0 => {
                        let _ = emit_target_signal(
                            &sink,
                            state.ime_target,
                            Signal::Data {
                                from: sink.from,
                                byte,
                            },
                        );
                        clear_ime_preview(state);
                        redraw_footer(&sink, state, true);
                        return ExecStatus::Done;
                    }
                    _ => {}
                }
            }

            if byte == b'\n' || byte == b'\r' {
                let cmd_len = state.len.min(state.buffer.len());
                let mut cmd_buf = [0u8; 128];
                cmd_buf[..cmd_len].copy_from_slice(&state.buffer[..cmd_len]);
                let cmd = core::str::from_utf8(&cmd_buf[..cmd_len]).unwrap_or("");

                if !cmd.is_empty() {
                    record_command_history(state);
                }

                if handle_graph_command(&sink, state, cmd) {
                    return ExecStatus::Done;
                }

                if state.graph_mode != GRAPH_MODE_NONE {
                    clear_graph_nav(state);
                    state.graph_mode = GRAPH_MODE_NONE;
                    state.graph_offset = 0;
                    state.graph_total = 0;
                    clear_command_area(&sink);
                    goto(&sink, COMMAND_SCROLL_TOP, 4);
                    save_output_cursor(&sink);
                }

                restore_output_cursor(&sink);
                echo_command_line(&sink, state);

                if cmd == "cypher" {
                    set_color(&sink, 11, 0);
                    print_str(&sink, " cypher usage\n");
                    set_color(&sink, 7, 0);
                    print_str(&sink, "  cypher MATCH (n) RETURN n\n");
                    print_str(&sink, "  cypher MATCH (n {vector:'6.1.0.0'}) CALL activate(n)\n");
                    print_str(&sink, "  cypher MATCH ()-[e {vector:'e:6.1.0.0'}]-() CALL route(e)\n");
                } else if let Some(query) = cmd.strip_prefix("cypher ") {
                    let trimmed = query.trim();
                    if trimmed.is_empty() {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " empty cypher query\n");
                    } else {
                        let _ = dispatch_cypher_query(&sink, state, trimmed);
                    }
                } else if looks_like_cypher_query(cmd) {
                    let _ = dispatch_cypher_query(&sink, state, cmd.trim());
                } else if cmd == "help" {
                    set_color(&sink, 11, 0);
                    print_str(&sink, " command index\n");
                    set_color(&sink, 7, 0);
                    print_str(&sink, "  help    show commands\n");
                    print_str(&sink, "  info    runtime snapshot\n");
                    print_str(&sink, "  graph   graph counters\n");
                    print_str(&sink, "  show    overview, or toggle node/edge context\n");
                    print_str(&sink, "  back    return to the previous graph view\n");
                    print_str(&sink, "  node <vector>  select/show one node\n");
                    print_str(&sink, "  edge <vector>  select/show one edge\n");
                    print_str(&sink, "  PgUp/PgDn  page graph overview/lists\n");
                    print_str(&sink, "  where   show current graph selection\n");
                    print_str(&sink, "  select clear  clear node/edge selection\n");
                    print_str(&sink, "  activate  activate selected node\n");
                    print_str(&sink, "  spawn     spawn selected node\n");
                    print_str(&sink, "  Up/Down   browse previous command history\n");
                    print_str(&sink, "  cypher <query>  send cypher v1 query into graph node\n");
                    print_str(&sink, "  MATCH ...       direct cypher entry without prefix\n");
                    print_str(&sink, "  net / net status  print uplink status\n");
                    print_str(&sink, "  net probe         rescan pci and refresh nic state\n");
                    print_str(&sink, "  net reset         re-init nic registers and report\n");
                    print_str(&sink, "  cuda / cuda status  print host bridge status\n");
                    print_str(&sink, "  cuda submit <job>   submit one host-backed cuda job\n");
                    print_str(&sink, "  cuda demo           send a sample saxpy-style job\n");
                    print_str(&sink, "  cuda reset          clear bridge counters and capture state\n");
                    print_str(&sink, "  clipboard          show clipboard.mount node and mount edges\n");
                    print_str(&sink, "  clipboard clear    clear shared clipboard buffer\n");
                    print_str(&sink, "  clipboard mount <vector>    add node -[mount]-> clipboard.mount\n");
                    print_str(&sink, "  clipboard unmount <vector>  remove node -[mount]-> clipboard.mount\n");
                    print_str(&sink, "  theme              show theme.current and its active use edge\n");
                    print_str(&sink, "  theme wabi         repoint theme.current -> theme.wabi\n");
                    print_str(&sink, "  theme shoji        repoint theme.current -> theme.shoji\n");
                    print_str(&sink, "  ai      open bottom ai api editor\n");
                    print_str(&sink, "  ask     send prompt into ai chat lane\n");
                    print_str(&sink, "  ^C/^X/^V copy, cut, paste active input through clipboard.mount\n");
                    print_str(&sink, "  ctrl+l  toggle input language en/zh-py\n");
                    print_str(&sink, "  clear   redraw command deck\n");
                    print_str(&sink, "  splash  replay boot cinema\n");
                } else if cmd == "info" || cmd == "graph" {
                    let snapshot = gos_runtime::snapshot();
                    set_color(&sink, 10, 0);
                    print_str(&sink, " runtime snapshot\n");
                    set_color(&sink, 7, 0);
                    print_str(&sink, "  plugins: ");
                    print_num_inline(&sink, snapshot.plugin_count);
                    print_str(&sink, "  nodes: ");
                    print_num_inline(&sink, snapshot.node_count);
                    print_str(&sink, "  edges: ");
                    print_num_inline(&sink, snapshot.edge_count);
                    print_str(&sink, "\n  ready: ");
                    print_num_inline(&sink, snapshot.ready_queue_len);
                    print_str(&sink, "  signals: ");
                    print_num_inline(&sink, snapshot.signal_queue_len);
                    print_str(&sink, "  stable: ");
                    print_str(&sink, if gos_runtime::is_stable() { "yes" } else { "no" });
                    print_str(&sink, "  tick: ");
                    print_num_inline(&sink, snapshot.tick as usize);
                    print_str(&sink, "\n  net: ");
                    print_str(
                        &sink,
                        if state.net_target == 0 {
                            "unresolved"
                        } else {
                            "ctl-ready"
                        },
                    );
                    if state.net_target != 0 {
                        print_str(&sink, "  path: qemu nic -> nat -> host wifi  cmds: net/net probe/net reset");
                    }
                    print_str(&sink, "\n  ai: ");
                    print_str(&sink, if state.ai_target == 0 { "offline" } else { "online" });
                    print_str(&sink, "  cypher: ");
                    print_str(&sink, if state.cypher_target == 0 { "offline" } else { "online" });
                    print_str(&sink, "  cuda: ");
                    print_str(&sink, if state.cuda_target == 0 { "offline" } else { "online" });
                    print_str(&sink, "  clip: ");
                    print_str(&sink, if clipboard_mounted(NODE_VEC) { "mounted" } else { "detached" });
                    print_str(&sink, "  bytes: ");
                    print_num_inline(&sink, clipboard_len());
                    print_str(&sink, "  api-key: ");
                    print_str(&sink, if state.api_configured != 0 { "armed" } else { "empty" });
                    print_str(&sink, "  bytes: ");
                    print_num_inline(&sink, state.api_len);
                    print_str(&sink, "\n  theme: ");
                    let theme = selected_theme();
                    print_str(&sink, theme_name(theme));
                    print_str(&sink, "  theme-node: ");
                    let mut current_line = LineBuf::<20>::new();
                    current_line.push_vector(THEME_CURRENT_NODE_VEC);
                    print_str(&sink, core::str::from_utf8(current_line.as_slice()).unwrap_or("set"));
                    print_str(&sink, "\n  use-> ");
                    let mut theme_line = LineBuf::<20>::new();
                    theme_line.push_vector(theme_vector(theme));
                    print_str(&sink, core::str::from_utf8(theme_line.as_slice()).unwrap_or("set"));
                    print_str(&sink, "\n  lang: ");
                    print_str(&sink, ime_mode_label(state.input_lang));
                    print_str(&sink, "  ime-preview: ");
                    print_num_inline(&sink, state.ime_preview_len);
                    print_str(&sink, "\n  graph-mode: ");
                    print_str(&sink, graph_mode_label(state.graph_mode));
                    print_str(&sink, "  selected-node: ");
                    if let Some(vector) = state.selected_node {
                        let mut line = LineBuf::<24>::new();
                        line.push_vector(vector);
                        print_str(&sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
                    } else {
                        print_str(&sink, "none");
                    }
                    print_str(&sink, "\n");
                } else if cmd == "theme" || cmd == "themes" || cmd == "theme list" {
                    let theme = selected_theme();
                    set_color(&sink, 11, 0);
                    print_str(&sink, " terminal themes\n");
                    set_color(&sink, 7, 0);
                    print_str(&sink, "  active: ");
                    print_str(&sink, theme_name(theme));
                    print_str(&sink, "  edge: theme.current -[use]-> ");
                    let mut active_line = LineBuf::<20>::new();
                    active_line.push_vector(theme_vector(theme));
                    print_str(&sink, core::str::from_utf8(active_line.as_slice()).unwrap_or("set"));
                    print_str(&sink, "\n  ");
                    let mut current = LineBuf::<20>::new();
                    current.push_vector(THEME_CURRENT_NODE_VEC);
                    print_str(&sink, core::str::from_utf8(current.as_slice()).unwrap_or("6.1.3.0"));
                    print_str(&sink, "  theme.current active theme state\n  ");
                    let mut wabi = LineBuf::<20>::new();
                    wabi.push_vector(THEME_WABI_NODE_VEC);
                    print_str(&sink, core::str::from_utf8(wabi.as_slice()).unwrap_or("6.1.1.0"));
                    print_str(&sink, "  theme.wabi  quiet ink / tea / moss\n  ");
                    let mut shoji = LineBuf::<20>::new();
                    shoji.push_vector(THEME_SHOJI_NODE_VEC);
                    print_str(&sink, core::str::from_utf8(shoji.as_slice()).unwrap_or("6.1.2.0"));
                    print_str(&sink, "  theme.shoji paper / indigo / brass\n");
                } else if let Some(selector) = cmd.strip_prefix("theme ") {
                    if let Some(theme) = parse_theme_selector(selector) {
                        if apply_theme_choice(&sink, theme) {
                            set_color(&sink, 11, 0);
                            print_str(&sink, " theme switched -> ");
                            set_color(&sink, 15, 0);
                            print_str(&sink, theme_name(theme));
                            print_str(&sink, "  edge theme.current -[use]-> ");
                            let mut line = LineBuf::<20>::new();
                            line.push_vector(theme_vector(theme));
                            print_str(&sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
                            print_str(&sink, "\n");
                        } else {
                            set_color(&sink, 12, 0);
                            print_str(&sink, " theme switch failed\n");
                        }
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " unknown theme, use: theme wabi | theme shoji\n");
                    }
                } else if cmd == "clipboard" || cmd == "clip" || cmd == "clipboard status" {
                    let mut edges = [GraphEdgeSummary::EMPTY; 12];
                    let (_, returned) =
                        gos_runtime::edge_page_for_node(CLIPBOARD_NODE_VEC, 0, &mut edges).unwrap_or((0, 0));
                    set_color(&sink, 11, 0);
                    print_str(&sink, " clipboard.mount\n");
                    set_color(&sink, 7, 0);
                    print_str(&sink, "  vector: ");
                    let mut node_line = LineBuf::<20>::new();
                    node_line.push_vector(CLIPBOARD_NODE_VEC);
                    print_str(&sink, core::str::from_utf8(node_line.as_slice()).unwrap_or("6.1.4.0"));
                    print_str(&sink, "\n  bytes: ");
                    print_num_inline(&sink, clipboard_len());
                    print_str(&sink, "\n  mounts:\n");
                    let mut listed = 0usize;
                    for summary in edges.iter().take(returned) {
                        if summary.edge_type != RuntimeEdgeType::Mount
                            || summary.to_vector != CLIPBOARD_NODE_VEC
                        {
                            continue;
                        }
                        print_str(&sink, "    ");
                        let mut line = LineBuf::<24>::new();
                        line.push_vector(summary.from_vector);
                        print_str(&sink, core::str::from_utf8(line.as_slice()).unwrap_or("node"));
                        print_str(&sink, "  ");
                        print_str(&sink, summary.from_key);
                        print_str(&sink, "\n");
                        listed += 1;
                    }
                    if listed == 0 {
                        print_str(&sink, "    none\n");
                    }
                } else if cmd == "clipboard clear" || cmd == "clip clear" {
                    if clipboard_clear(&sink, state.clipboard_target) {
                        set_color(&sink, 11, 0);
                        print_str(&sink, " clipboard cleared\n");
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " clipboard clear failed\n");
                    }
                } else if let Some(selector) = cmd
                    .strip_prefix("clipboard mount ")
                    .or_else(|| cmd.strip_prefix("clip mount "))
                {
                    if let Some(vector) = parse_clipboard_vector(selector) {
                        if sync_clipboard_mount_for_vector(vector, true) {
                            set_color(&sink, 11, 0);
                            print_str(&sink, " clipboard mounted <- ");
                            let mut line = LineBuf::<20>::new();
                            line.push_vector(vector);
                            print_str(&sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
                            print_str(&sink, "\n");
                        } else {
                            set_color(&sink, 12, 0);
                            print_str(&sink, " clipboard mount failed\n");
                        }
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " clipboard mount requires node vector\n");
                    }
                } else if let Some(selector) = cmd
                    .strip_prefix("clipboard unmount ")
                    .or_else(|| cmd.strip_prefix("clip unmount "))
                {
                    if let Some(vector) = parse_clipboard_vector(selector) {
                        if sync_clipboard_mount_for_vector(vector, false) {
                            set_color(&sink, 11, 0);
                            print_str(&sink, " clipboard unmounted <- ");
                            let mut line = LineBuf::<20>::new();
                            line.push_vector(vector);
                            print_str(&sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
                            print_str(&sink, "\n");
                        } else {
                            set_color(&sink, 12, 0);
                            print_str(&sink, " clipboard unmount failed\n");
                        }
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " clipboard unmount requires node vector\n");
                    }
                } else if cmd == "net" || cmd == "net status" || cmd == "uplink" {
                    if emit_target_signal(
                        &sink,
                        state.net_target,
                        Signal::Control {
                            cmd: NET_CONTROL_REPORT,
                            val: 0,
                        },
                    ) {
                        set_color(&sink, 11, 0);
                        print_str(&sink, " net status requested\n");
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " net uplink unresolved\n");
                    }
                } else if cmd == "net probe" {
                    if emit_target_signal(
                        &sink,
                        state.net_target,
                        Signal::Control {
                            cmd: NET_CONTROL_PROBE,
                            val: 0,
                        },
                    ) {
                        set_color(&sink, 11, 0);
                        print_str(&sink, " net reprobe dispatched\n");
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " net uplink unresolved\n");
                    }
                } else if cmd == "net reset" {
                    if emit_target_signal(
                        &sink,
                        state.net_target,
                        Signal::Control {
                            cmd: NET_CONTROL_RESET,
                            val: 0,
                        },
                    ) {
                        set_color(&sink, 11, 0);
                        print_str(&sink, " net reset dispatched\n");
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " net uplink unresolved\n");
                    }
                } else if cmd == "cuda" || cmd == "cuda status" || cmd == "gpu" || cmd == "gpu status" {
                    if emit_target_signal(
                        &sink,
                        state.cuda_target,
                        Signal::Control {
                            cmd: CUDA_CONTROL_REPORT,
                            val: 0,
                        },
                    ) {
                        gos_runtime::pump();
                        set_color(&sink, 11, 0);
                        print_str(&sink, " cuda bridge status requested\n");
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " cuda bridge unresolved\n");
                    }
                } else if cmd == "cuda reset" {
                    if emit_target_signal(
                        &sink,
                        state.cuda_target,
                        Signal::Control {
                            cmd: CUDA_CONTROL_RESET,
                            val: 0,
                        },
                    ) {
                        gos_runtime::pump();
                        set_color(&sink, 11, 0);
                        print_str(&sink, " cuda bridge reset dispatched\n");
                    } else {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " cuda bridge unresolved\n");
                    }
                } else if cmd == "cuda demo" {
                    let _ = dispatch_cuda_submit(
                        &sink,
                        state,
                        "kernel=saxpy grid=120 block=256 bytes=4096 dtype=f32",
                    );
                } else if let Some(job) = cmd.strip_prefix("cuda submit ") {
                    let trimmed = job.trim();
                    if trimmed.is_empty() {
                        set_color(&sink, 12, 0);
                        print_str(&sink, " empty cuda job\n");
                    } else {
                        let _ = dispatch_cuda_submit(&sink, state, trimmed);
                    }
                } else if cmd == "ai" || cmd == "api" || cmd == "ai-api" {
                    state.len = 0;
                    enter_ai_api_mode(&sink, state);
                    return ExecStatus::Done;
                } else if cmd == "ask" {
                    push_ai_text(state, "sys> usage: ask <text>");
                    redraw_ai_panel(&sink, state, true);
                } else if let Some(_prompt) = cmd.strip_prefix("ask ") {
                    let mut prompt = [0u8; 124];
                    let prompt_len = state.len.saturating_sub(4).min(prompt.len());
                    prompt[..prompt_len].copy_from_slice(&state.buffer[4..4 + prompt_len]);
                    if prompt_len > 0 {
                        let mut prefixed = [0u8; AI_PANEL_LINE_WIDTH];
                        let prefix = b"you> ";
                        let mut line_len = 0usize;
                        for byte in prefix.iter().copied() {
                            if line_len < prefixed.len() {
                                prefixed[line_len] = byte;
                                line_len += 1;
                            }
                        }
                        for byte in prompt.iter().copied().take(prompt_len).take(prefixed.len().saturating_sub(line_len)) {
                            prefixed[line_len] = ai_panel_byte(byte);
                            line_len += 1;
                        }
                        push_ai_line(state, &prefixed[..line_len]);
                    }
                    if !emit_target_signal(&sink, state.ai_target, Signal::Control { cmd: AI_CONTROL_CHAT_BEGIN, val: 0 }) {
                        push_ai_text(state, "sys> ai lane unresolved");
                    } else {
                        for byte in prompt.iter().copied().take(prompt_len) {
                            let _ = emit_target_signal(
                                &sink,
                                state.ai_target,
                                Signal::Data {
                                    from: sink.from,
                                    byte,
                                },
                            );
                        }
                        let _ = emit_target_signal(
                            &sink,
                            state.ai_target,
                            Signal::Control {
                                cmd: AI_CONTROL_CHAT_COMMIT,
                                val: 0,
                            },
                        );
                    }
                    redraw_ai_panel(&sink, state, true);
                } else if cmd == "clear" {
                    state.len = 0;
                    redraw_console(&sink, state);
                    return ExecStatus::Done;
                } else if cmd == "splash" || cmd == "reboot-splash" {
                    state.console_live = 0;
                    play_boot_sequence(&sink);
                    redraw_console(&sink, state);
                    state.console_live = 1;
                    state.len = 0;
                    return ExecStatus::Done;
                } else if !cmd.is_empty() {
                    set_color(&sink, 12, 0);
                    if cmd.is_ascii() {
                        print_str(&sink, " unknown command: ");
                        set_color(&sink, 15, 0);
                        print_str(&sink, cmd);
                        print_str(&sink, "\n");
                    } else {
                        print_str(&sink, " unknown command payload contains non-ascii bytes\n");
                    }
                }

                save_output_cursor(&sink);
                state.len = 0;
                redraw_footer(&sink, state, false);
            } else if byte == 0x03 {
                let _ = clipboard_copy_active_input(&sink, state);
            } else if byte == 0x16 {
                let _ = clipboard_paste_active_input(&sink, state);
            } else if byte == 0x18 {
                let _ = clipboard_cut_active_input(&sink, state);
            } else if byte == 0x08 || byte == 0x7F {
                if command_pop_scalar(state) {
                    reset_command_history_cursor(state);
                    redraw_footer(&sink, state, false);
                }
            } else if byte >= 0x20 {
                append_command_byte(&sink, state, byte, false);
            }
            ExecStatus::Done
        }
        Signal::Spawn { .. } => {
            play_boot_sequence(&sink);
            redraw_console(&sink, state);
            state.console_live = 1;
            ExecStatus::Done
        }
        Signal::Interrupt { irq } => {
            if irq == 32 && state.console_live != 0 {
                state.heartbeat_divider = state.heartbeat_divider.wrapping_add(1);
                state.sigil_frame = (state.sigil_frame + 1) % LIVE_SIGIL_FRAMES as u8;
                save_cursor(&sink, 0);
                let snapshot = gos_runtime::snapshot();
                draw_runtime_header(&sink, state, snapshot);
                draw_runtime_gap_flux(&sink, state);
                draw_console_sigil(&sink, state.sigil_frame as usize);
                draw_ai_panel(&sink, state);
                draw_operator_band(&sink, state, snapshot);
                if state.heartbeat_divider % 4 == 0 {
                    draw_command_deck_panel(&sink, state, snapshot);
                    redraw_footer(&sink, state, false);
                }
                restore_cursor(&sink, 0);
            }
            ExecStatus::Done
        }
        _ => ExecStatus::Done,
    }
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
