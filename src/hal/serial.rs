//! Plugin: K_SERIAL — UART 16550 Serial Port Driver
//!
//! # Node Contract
//! - **Node ID** : `K_SERIAL`
//! - **Node Type**: `Plugin`
//! - **Input**   : Formatted strings via `serial_print!` / `serial_println!`
//! - **Output**  : Bytes written to COM1 (port 0x3F8), visible in QEMU stdio
//! - **State**   : Global `SERIAL1` singleton (lazy-initialized)
//!
//! # Edges
//! - K_SERIAL has **no upstream plugin edges** — it directly drives UART hardware.
//! - K_PANIC --[debug_output]--> K_SERIAL  (all panics are mirrored here)
//! - K_BOOT  --[init]---------> K_SERIAL
//!
//! # Purpose
//! Serial output bypasses the VGA buffer and is visible in the QEMU terminal.
//! Essential for debugging Phase 1+ (interrupts, memory panics).

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::SerialPort;

// ── Global Singleton ───────────────────────────────────────────────────────

lazy_static! {
    /// COM1 serial port singleton.
    /// `0x3F8` is the standard base address for COM1 on x86.
    pub static ref SERIAL1: Mutex<SerialPort> = {
        // SAFETY: 0x3F8 is the COM1 I/O port. We only create one instance.
        let mut port = unsafe { SerialPort::new(0x3F8) };
        port.init();
        Mutex::new(port)
    };
}

// ── Internal Print (Plugin Output Port) ───────────────────────────────────

#[doc(hidden)]
pub fn _serial_print(args: fmt::Arguments) {
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).unwrap();
    });
}

// ── Public Macros (Plugin Interface) ──────────────────────────────────────

/// Print to serial (no newline).
/// Edge: any plugin --[debug_flow]--> K_SERIAL via `serial_print!`
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::hal::serial::_serial_print(format_args!($($arg)*))
    };
}

/// Print to serial with newline.
#[macro_export]
macro_rules! serial_println {
    ()            => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
