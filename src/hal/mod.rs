//! HAL Module Root
//!
//! # Node: hal (aggregator)
//! This module is an **aggregator node** — it collects all hardware-facing
//! plugin nodes and exposes a single `init()` edge to K_BOOT.
//!
//! # Edges
//! - K_BOOT --[init]--> hal::init()
//! - hal    --[init]--> K_VGA   (lazy singleton, initializes on first use)
//! - hal    --[init]--> K_SERIAL (lazy singleton, initializes on first use)
//!
//! Plugins are added to this module as phases progress:
//!   Phase 0: vga_buffer, serial
//!   Phase 1: gdt, interrupts, pic, pit, ps2_kbd, cpuid

pub mod vga_buffer;
pub mod serial;
// Phase 1 — uncomment as each plugin is implemented:
// pub mod gdt;
// pub mod interrupts;
// pub mod pic;
// pub mod pit;
// pub mod ps2_kbd;
// pub mod cpuid;

use bootloader::BootInfo;
use crate::serial_println;

/// Top-level HAL initialization.
/// Called once from K_BOOT after the bootloader hands off control.
/// Each sub-call activates the corresponding plugin node.
pub fn init(_boot_info: &'static BootInfo) {
    // K_SERIAL: Force lazy initialization so serial is ready before any panic.
    // Touching the static triggers uart_16550::SerialPort::init().
    let _ = &*serial::SERIAL1;
    serial_println!("[K_SERIAL] online  COM1 @ 0x3F8");

    // K_VGA: Force lazy initialization.
    // Touching the static assigns the VGA buffer pointer.
    let _ = &*vga_buffer::WRITER;
    serial_println!("[K_VGA]    online  0xb8000  80x25");

    // Phase 1 inits will be added here:
    // gdt::init();          // K_GDT
    // interrupts::init();   // K_IDT + K_PIC
}
