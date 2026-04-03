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

// ── Module Graph (Plugin Declarations) ────────────────────────────────────
// Each `mod` declaration is an edge: K_BOOT --[owns]--> plugin_node

mod panic;       // K_PANIC: error terminal node
pub mod hal;     // HAL aggregator: K_VGA, K_SERIAL (Phase 1+: K_GDT, K_IDT …)

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
    // Edge: K_BOOT --[init]--> K_SERIAL (must be first for panic visibility)
    // Edge: K_BOOT --[init]--> K_VGA   (text output ready after this)
    hal::init(boot_info);

    // ── Boot Banner ───────────────────────────────────────────────────────
    print_boot_banner();

    // ── Phase 1+: Additional inits will be inserted here ─────────────────
    // gdt::init();          // K_GDT
    // interrupts::init();   // K_IDT + K_PIC
    // memory::init(...);    // K_PMM + K_VMM + K_HEAP
    // shell::run();         // K_SHELL (replaces hlt loop)

    // ── Idle loop — active until K_SHELL takes over in Phase 4 ───────────
    serial_println!("[K_BOOT] init complete — entering idle HLT loop");
    loop {
        x86_64::instructions::hlt();
    }
}

// ── Boot Banner ────────────────────────────────────────────────────────────

/// Display the GOS startup banner.
/// Uses direct Writer access to batch color changes without extra lock acquisitions.
fn print_boot_banner() {
    use hal::vga_buffer::{Color, WRITER};
    use core::fmt::Write;

    let mut w = WRITER.lock();

    w.clear();

    // ── Header bar ────────────────────────────────────────────────────────
    w.set_color(Color::Black, Color::Cyan);
    let _ = write!(w, "{:^80}", "  GOS  v0.1.0  |  Graph-Oriented System  |  x86_64 Bare-Metal  ");

    // ── ASCII logo ────────────────────────────────────────────────────────
    w.set_color(Color::LightCyan, Color::Black);
    let _ = writeln!(w);
    let _ = writeln!(w, "   ___  ____  ____");
    let _ = writeln!(w, "  / __||  _ \\/ ___|");
    let _ = writeln!(w, " | |  _| | | \\___ \\");
    let _ = writeln!(w, " | |_| | |_| |___) |");
    let _ = writeln!(w, "  \\____|____/|____/");

    // ── Subtitle ──────────────────────────────────────────────────────────
    w.set_color(Color::White, Color::Black);
    let _ = writeln!(w, "  Graph-Oriented System  v0.1.0");
    let _ = writeln!(w);

    // ── System info ───────────────────────────────────────────────────────
    w.set_color(Color::DarkGray, Color::Black);
    let _ = writeln!(w, "  Platform : x86_64 (qemu-system-x86_64)");
    let _ = writeln!(w, "  Kernel   : Rust no_std bare-metal");
    let _ = writeln!(w, "  Phase    : 0 — Boot / Display");
    let _ = writeln!(w);

    // ── Plugin status ─────────────────────────────────────────────────────
    w.set_color(Color::LightGreen, Color::Black);
    let _ = writeln!(w, "  [OK] K_BUILD  — build infrastructure");
    let _ = writeln!(w, "  [OK] K_BOOT   — kernel entry point");
    let _ = writeln!(w, "  [OK] K_VGA    — vga text buffer driver");
    let _ = writeln!(w, "  [OK] K_SERIAL — uart 16550 serial port");
    let _ = writeln!(w, "  [OK] K_PANIC  — panic handler");
    w.set_color(Color::DarkGray, Color::Black);
    let _ = writeln!(w, "  [ ] K_GDT    — phase 1");
    let _ = writeln!(w, "  [ ] K_IDT    — phase 1");
    let _ = writeln!(w, "  [ ] K_PS2    — phase 1");
    let _ = writeln!(w, "  [ ] K_HEAP   — phase 2");
    let _ = writeln!(w, "  [ ] K_GRAPH  — phase 3");
    let _ = writeln!(w, "  [ ] K_SHELL  — phase 4");
    let _ = writeln!(w);

    // ── Bottom separator ──────────────────────────────────────────────────
    w.set_color(Color::Black, Color::Cyan);
    let _ = write!(w, "{:^80}", "  Waiting for Phase 1 ...  ");

    // Reset to working color
    w.set_color(Color::LightGreen, Color::Black);
}
