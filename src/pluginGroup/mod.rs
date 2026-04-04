//! GOS Plugin Group
//! This is the central repository for all functional plugins in the Kernel.

#![allow(non_snake_case)]

pub mod K_PANIC;
pub mod K_VGA;
pub mod K_SERIAL;
pub mod K_GDT;
pub mod K_IDT;
pub mod K_PIC;
pub mod K_PIT;
pub mod K_PS2;
pub mod K_CPUID;

use bootloader::BootInfo;
use crate::serial_println;

/// Initialize all HAL-related plugins.
pub fn init_hal(_boot_info: &'static BootInfo) {
    // Phase 0: Base Hardware
    K_SERIAL::init();
    K_VGA::init();

    // Phase 1: Interrupts & Core Hardware
    K_GDT::init();
    K_IDT::init();
    K_PIC::init();
    K_PIT::init();
    K_PS2::init();
    K_CPUID::init();

    serial_println!("[HAL] Hardware initialized");
}
