//! Plugin: K_PANIC — Kernel Panic Handler
//!
//! # Node Contract
//! - **Node ID** : `K_PANIC`
//! - **Node Type**: `Plugin` (error handler)
//! - **Input**   : `PanicInfo` from Rust runtime
//! - **Output**  : Error message to K_VGA + K_SERIAL, then CPU halt
//! - **State**   : Stateless — pure function
//!
//! # Edges (incoming)
//! - `*` (any plugin) --[error]--> K_PANIC  (implicit, via Rust panic mechanism)
//!
//! # Edges (outgoing)
//! - K_PANIC --[data_flow]--> K_VGA    (display panic message)
//! - K_PANIC --[data_flow]--> K_SERIAL (duplicate to serial for debug capture)
//! - K_PANIC --[control]---> CPU halt  (system stop)
//!
//! # Design
//! This plugin is a **terminal node**: once entered, execution never returns.
//! The red background distinguishes panics visually from normal output.

use core::panic::PanicInfo;
use crate::{println, serial_println};

/// Required `#[panic_handler]` — the Rust ABI entry point for all panics.
/// Registered as a static edge from any plugin to this handler.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Switch VGA to red-on-white — visually unmistakable panic state.
    use crate::hal::vga_buffer::{Color, WRITER};
    WRITER.lock().set_color(Color::White, Color::Red);

    // Output to both K_VGA (visual) and K_SERIAL (capture/log).
    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║          !!! KERNEL PANIC !!!            ║");
    println!("╚══════════════════════════════════════════╝");
    println!("{}", info);

    // Mirror to serial for offline analysis.
    serial_println!("[K_PANIC] {}", info);

    // Halt all CPU cores — this node never exits.
    loop {
        x86_64::instructions::hlt();
    }
}
