#![no_std]


// ==============================================================
// GOS KERNEL TOPOLOGY — k-pit (native.pit)
// 以下 Cypher 脚本可直接导入 Neo4j，与其他模块共同还原内核完整图谱。
//
// MERGE (p:Plugin {id: "K_PIT", name: "k-pit"})
// SET p.executor = "native.pit", p.node_type = "Driver", p.state_schema = "0x2007"
//
// // ── 启动依赖 (DEPENDS_ON) ──────────────────────────────────
// MERGE (k_pic:Plugin {id: "K_PIC"})
// MERGE (p)-[:DEPENDS_ON {required: true}]->(k_pic)
//
// // ── 硬件资源边界 ──────────────────────────────────────────
// MERGE (hw_40:PortRange {start: "0x40", end: "0x43", label: "PIT Channels"})
// MERGE (p)-[:REQUIRES_PORT]->(hw_40)
// MERGE (irq_0:InterruptLine {irq: "0", label: "IRQ0 Timer"})
// MERGE (p)-[:BINDS_IRQ]->(irq_0)
// ==============================================================

use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::instructions::port::Port;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 6, 0, 0);
const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;
const PIT_BASE_HZ: u32 = 1_193_182;

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub fn ticks() -> &'static AtomicUsize {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_PIT Matrix not initialized"); }
        &*(p.add(1024) as *mut AtomicUsize)
    }
}

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "PIT");
    let state_ptr = p.add(1024) as *mut AtomicUsize;
    core::ptr::write(state_ptr, AtomicUsize::new(0));
}

pub struct PitCell { state: NodeState }

impl PitCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for PitCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("CLOCK", 0x01, 0); // Call: get ticks
        CellDeclaration {
            vec: NODE_VEC, domain_label: "HAL", name: "PIT",
            edges, edge_count: 1, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { init_node_state(); self.state = NodeState::Ready; }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }
    fn on_signal(&mut self, signal: Signal) -> CellResult {
        if let Signal::Interrupt { irq } = signal {
            if irq == k_pic::InterruptIndex::Timer.as_u8() {
                ticks().fetch_add(1, Ordering::Relaxed);
                gos_hal::ngr::post_signal(
                    k_shell::NODE_VEC,
                    Signal::Interrupt { irq },
                );
            }
        }
        CellResult::Done 
    }
    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static PIT_CELL: spin::Mutex<PitCell> = spin::Mutex::new(PitCell::new());

impl PluginEntry for PitCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 2; // Depends on PIC

    fn plugin_main(_ctx: &mut BootContext) {
        gos_hal::ngr::try_mount_cell(Self::VEC, &PIT_CELL);
    }
}

pub fn get_ticks() -> usize { ticks().load(Ordering::Relaxed) }

pub fn init_pit_hz(hz: u32) {
    let requested = hz.clamp(30, 240);
    let divisor = (PIT_BASE_HZ / requested).max(1).min(u16::MAX as u32) as u16;
    let mut command = Port::<u8>::new(PIT_COMMAND);
    let mut channel0 = Port::<u8>::new(PIT_CHANNEL0);
    unsafe {
        command.write(0x36);
        channel0.write((divisor & 0x00FF) as u8);
        channel0.write((divisor >> 8) as u8);
    }
}

