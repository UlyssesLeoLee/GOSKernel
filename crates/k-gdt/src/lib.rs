#![no_std]

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

#[repr(C)]
pub struct GdtState {
    pub tss: TaskStateSegment,
    pub gdt: GlobalDescriptorTable,
    pub selectors: Selectors,
}

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 3, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub fn gdt_state() -> &'static GdtState {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_GDT Matrix not initialized"); }
        &*(p.add(1024) as *mut GdtState)
    }
}

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "GDT");

    let state_ptr = p.add(1024) as *mut GdtState;

    core::ptr::write(state_ptr, GdtState {
        tss: TaskStateSegment::new(),
        gdt: GlobalDescriptorTable::new(),
        selectors: Selectors {
            code_selector: SegmentSelector(0),
            tss_selector: SegmentSelector(0)
        },
    });

    let state = &mut *state_ptr;

    state.tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096 * 5;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
        let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(STACK) as *const ());
        stack_start + STACK_SIZE
    };

    let code_selector = state.gdt.add_entry(Descriptor::kernel_code_segment());
    let tss_selector  = state.gdt.add_entry(Descriptor::tss_segment(&state.tss));

    state.selectors.code_selector = code_selector;
    state.selectors.tss_selector  = tss_selector;
}

pub struct GdtCell { state: NodeState }

impl GdtCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for GdtCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("LOAD", 0x01, 0); // Call: load GDT into CPU
        CellDeclaration {
            vec: NODE_VEC, domain_label: "HAL", name: "GDT",
            edges, edge_count: 1, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { init_node_state(); self.state = NodeState::Ready; }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }
    fn on_signal(&mut self, _: Signal) -> CellResult { CellResult::Done }
    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static GDT_CELL: spin::Mutex<GdtCell> = spin::Mutex::new(GdtCell::new());

impl PluginEntry for GdtCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 1;

    fn plugin_main(_ctx: &mut BootContext) {
        gos_hal::ngr::try_mount_cell(Self::VEC, &GDT_CELL);
    }
}

pub fn init_gdt() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;
    unsafe {
        let state = gdt_state();
        state.gdt.load();
        CS::set_reg(state.selectors.code_selector);
        load_tss(state.selectors.tss_selector);
    }
}
