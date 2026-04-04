#![no_std]

use pic8259::ChainedPics;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 5, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
    Mouse = PIC_2_OFFSET + 4,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 { self as u8 }
    pub fn as_usize(self) -> usize { usize::from(self.as_u8()) }
}

pub fn pics() -> &'static Mutex<ChainedPics> {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_PIC Matrix not initialized"); }
        &*(p.add(1024) as *mut Mutex<ChainedPics>)
    }
}

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "PIC");
    let state_ptr = p.add(1024) as *mut Mutex<ChainedPics>;
    core::ptr::write(state_ptr, Mutex::new(ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)));
}

pub struct PicCell { state: NodeState }

impl PicCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for PicCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("MASK", 0x01, 0);
        edges[1] = CellEdge::new("ACK", 0x01, 0);
        CellDeclaration {
            vec: NODE_VEC, domain_label: "HAL", name: "PIC",
            edges, edge_count: 2, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { init_node_state(); self.state = NodeState::Ready; }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }
    fn on_signal(&mut self, _: Signal) -> CellResult { CellResult::Done }
    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static PIC_CELL: spin::Mutex<PicCell> = spin::Mutex::new(PicCell::new());

impl PluginEntry for PicCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 1; // Unrelated to GDT/CPUID

    fn plugin_main(_ctx: &mut BootContext) {
        gos_hal::ngr::try_mount_cell(Self::VEC, &PIC_CELL);
    }
}

pub fn init_pic() {
    unsafe {
        pics().lock().initialize();
        pics().lock().write_masks(0xF8, 0xEF);
    }
}
