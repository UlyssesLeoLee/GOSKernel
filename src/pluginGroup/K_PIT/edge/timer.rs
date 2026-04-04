//! Edge: timer
use core::sync::atomic::Ordering;
use x86_64::structures::idt::InterruptStackFrame;
use crate::pluginGroup::K_PIC::InterruptIndex;
use crate::pluginGroup::K_PIC::PICS;
use crate::pluginGroup::K_PIT::node::TICKS;

pub fn get_ticks() -> usize {
    TICKS.load(Ordering::Relaxed)
}

pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let ticks = TICKS.fetch_add(1, Ordering::Relaxed);
    
    // Heartbeat every ~1.5 seconds (assuming 50Hz/100Hz pit)
    if ticks % 100 == 0 {
        crate::print!(".");
    }
    
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}
