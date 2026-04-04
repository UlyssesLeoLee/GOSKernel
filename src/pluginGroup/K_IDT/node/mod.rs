use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;
use crate::pluginGroup::K_GDT;
use crate::pluginGroup::K_PIC::InterruptIndex;
use crate::pluginGroup::{K_PIT, K_PS2};
use crate::pluginGroup::K_IDT::edge::route::{breakpoint_handler, double_fault_handler, page_fault_handler};

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(K_GDT::DOUBLE_FAULT_IST_INDEX);
        }

        idt[InterruptIndex::Timer.as_usize()]
            .set_handler_fn(K_PIT::timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(K_PS2::keyboard_interrupt_handler);

        idt
    };
}
