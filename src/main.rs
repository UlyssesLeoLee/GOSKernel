//! Plugin: K_BOOT — Kernel Entry Point
//!
//! # Node Contract
//! - **Node ID** : `K_BOOT`
//! - **Node Type**: `Plugin` (bootstrap)
//! - **Input**   : `BootInfo` from bootloader (memory map, framebuffer info)
//! - **Output**  : Initialized kernel environment + boot banner on VGA
//! - **State**   : Stateless after init — delegates all state to sub-plugins
//!
//! # Edges (outgoing, in init order)
//! - K_BOOT --[init]----> hal::init()  → K_SERIAL, K_VGA
//! - K_BOOT --[display]-> VGA banner (via K_VGA)
//! - K_BOOT --[control]-> idle HLT loop (terminal until Phase 4 adds K_SHELL)
//!
//! # Design
//! K_BOOT is the **root node** of the entire GOS plugin graph.
//! It is the only node with no incoming plugin edges (only the bootloader ABI).
//! Its sole job: initialize the environment, then yield control to sub-systems.

#![no_std]
#![no_main]
// Required for Phase 1: abi_x86_interrupt enables interrupt handler function ABI.
#![feature(abi_x86_interrupt)]

#![allow(non_snake_case)]

// ── Module Graph (Plugin Declarations) ────────────────────────────────────
// Each `mod` declaration is an edge: K_BOOT --[owns]--> plugin_node

pub mod pluginGroup;

// Phase 2: uncomment when K_HEAP is implemented
// extern crate alloc;

// ── Bootloader Entry Point ─────────────────────────────────────────────────

use bootloader::{entry_point, BootInfo};

// `entry_point!` generates a safe `_start` symbol that:
//   1. Receives raw BootInfo pointer from bootloader.
//   2. Calls `kernel_main` with a validated `&'static BootInfo` reference.
entry_point!(kernel_main);

/// K_BOOT: Main kernel function — the first Rust code to run.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // ── Phase 0: HAL initialization ───────────────────────────────────────
    pluginGroup::init_hal(boot_info);

    // ── Boot Banner ───────────────────────────────────────────────────────
    print_boot_banner();

    // ── Phase 1+: Additional inits will be inserted here ─────────────────
    // gdt::init();          // K_GDT
    // interrupts::init();   // K_IDT + K_PIC
    // memory::init(...);    // K_PMM + K_VMM + K_HEAP
    // shell::run();         // K_SHELL (replaces hlt loop)

    // ── Idle loop — active until K_SHELL takes over in Phase 4 ───────────
    // ── Enable Interrupts ──────────────────────────────────────────────────
    // Enables the PIC, PIT, and PS2 to begin delivering events.
    // Done AFTER the boot banner to prevent VGA Lock deadlocks!
    x86_64::instructions::interrupts::enable();
    serial_println!("[HAL] CPU interrupts globally enabled");

    serial_println!("[K_BOOT] init complete — entering idle HLT loop");
    loop {
        x86_64::instructions::hlt();
    }
}

// ── Boot Banner ────────────────────────────────────────────────────────────

fn print_boot_banner() {
    use pluginGroup::K_VGA::{Color, clear_screen, set_screen_color};

    clear_screen();

    // ── Header bar ────────────────────────────────────────────────────────
    set_screen_color(Color::Black, Color::Cyan);
    crate::print!("{:^80}", "  GOS  v0.1.0  |  Graph Operating System  |  x86_64 Bare-Metal  ");

    // ── ASCII logo ────────────────────────────────────────────────────────
    set_screen_color(Color::LightCyan, Color::Black);
    crate::println!();
    crate::println!("   ___  ____  ____");
    crate::println!("  / __||  _ \\/ ___|");
    crate::println!(" | |  _| | | \\___ \\");
    crate::println!(" | |_| | |_| |___) |");
    crate::println!("  \\____|____/|____/");

    // ── Subtitle ──────────────────────────────────────────────────────────
    set_screen_color(Color::White, Color::Black);
    crate::println!("  Graph Operating System  v0.1.0");
    crate::println!();

    // ── System info ───────────────────────────────────────────────────────
    set_screen_color(Color::DarkGray, Color::Black);
    crate::println!("  Platform : x86_64 (qemu-system-x86_64)");
    crate::println!("  Kernel   : Rust no_std bare-metal");
    crate::println!("  Phase    : 0 — Boot / Display");
    crate::println!();

    // ── Plugin status ─────────────────────────────────────────────────────
    set_screen_color(Color::LightGreen, Color::Black);
    crate::println!("  [OK] K_BUILD  — build infrastructure");
    crate::println!("  [OK] K_BOOT   — kernel entry point");
    crate::println!("  [OK] K_VGA    — vga text buffer driver");
    crate::println!("  [OK] K_SERIAL — uart 16550 serial port");
    crate::println!("  [OK] K_PANIC  — panic handler");
    set_screen_color(Color::DarkGray, Color::Black);
    crate::println!("  [OK] K_GDT    — phase 1");
    crate::println!("  [OK] K_IDT    — phase 1");
    crate::println!("  [OK] K_PIC    — phase 1");
    crate::println!("  [OK] K_PIT    — phase 1");
    crate::println!("  [OK] K_PS2    — phase 1");
    crate::println!("  [ ] K_HEAP   — phase 2");
    crate::println!("  [ ] K_GRAPH  — phase 3");
    crate::println!("  [ ] K_SHELL  — phase 4");
    crate::println!();

    // ── Bottom separator ──────────────────────────────────────────────────
    set_screen_color(Color::Black, Color::Cyan);
    crate::print!("{:^80}", "  Waiting for Phase 1 ...  ");

    // Reset to working color
    set_screen_color(Color::LightGreen, Color::Black);
}
