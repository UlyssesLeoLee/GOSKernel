#![no_std]
#![feature(abi_x86_interrupt)]

use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::idt::InterruptStackFrame;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 4, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub fn idt() -> &'static InterruptDescriptorTable {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_IDT Matrix not initialized"); }
        &*(p.add(1024) as *mut InterruptDescriptorTable)
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    // Note: Can't print without k_vga. We rely on QEMU / serial.
}

extern "x86-interrupt" fn double_fault_handler(_stack_frame: InterruptStackFrame, _error_code: u64) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT");
}

extern "x86-interrupt" fn page_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    panic!("EXCEPTION: PAGE FAULT");
}

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "IDT");

    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(k_gdt::DOUBLE_FAULT_IST_INDEX);
    }
    
    // We expect k-pit and k-ps2 handlers to be attached later or here if we have direct deps
    // For now we'll write an empty IDT, and then we could manually inject it, OR
    // we make k-idt depend on k-pit and k-ps2 to set them up right here.
    
    let state_ptr = p.add(1024) as *mut InterruptDescriptorTable;
    core::ptr::write(state_ptr, idt);
}

pub struct IdtCell { state: NodeState }

impl IdtCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for IdtCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("ROUTE",  0x04, 0);
        CellDeclaration {
            vec: NODE_VEC, domain_label: "HAL", name: "IDT",
            edges, edge_count: 1, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { init_node_state(); self.state = NodeState::Ready; }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }
    fn on_signal(&mut self, _signal: Signal) -> CellResult { CellResult::Done }
    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static IDT_CELL: spin::Mutex<IdtCell> = spin::Mutex::new(IdtCell::new());

impl PluginEntry for IdtCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 3; // Depends on GDT, PIT, PS2

    fn plugin_main(_ctx: &mut BootContext) {
        gos_hal::ngr::try_mount_cell(Self::VEC, &IDT_CELL);
    }
}

pub fn init_idt() {
    unsafe { idt().load(); }
}

// Phase 2 workaround to inject handlers from other plugins that depend on PIC
pub fn inject_irq_handler(index: usize, handler: extern "x86-interrupt" fn(InterruptStackFrame)) {
    unsafe {
        let p = node_ptr();
        let idt_ptr = p.add(1024) as *mut InterruptDescriptorTable;
        let idt = &mut *idt_ptr;
        idt[index].set_handler_fn(handler);
    }
}
