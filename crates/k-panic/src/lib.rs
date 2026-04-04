#![no_std]

use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 0, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "BOOT", "PANIC");
}

pub struct PanicCell { state: NodeState }

impl PanicCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for PanicCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("HALT", 0x04, 0);
        CellDeclaration {
            vec: NODE_VEC, domain_label: "BOOT", name: "PANIC",
            edges, edge_count: 1, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) {
        init_node_state();
        self.state = NodeState::Ready;
    }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }

    fn on_signal(&mut self, signal: Signal) -> CellResult {
        match signal {
            Signal::Interrupt { irq: 0xFF } => {
                loop { x86_64::instructions::hlt(); }
            }
            _ => CellResult::Done,
        }
    }

    fn on_suspend(&mut self) {}
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static PANIC_CELL: spin::Mutex<PanicCell> = spin::Mutex::new(PanicCell::new());

impl PluginEntry for PanicCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 0;

    fn plugin_main(_ctx: &mut BootContext) {
        gos_hal::ngr::try_mount_cell(Self::VEC, &PANIC_CELL);
    }
}
