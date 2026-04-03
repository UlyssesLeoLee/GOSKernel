//! Plugin: K_VGA — VGA Text Buffer Driver
//!
//! # Node Contract
//! - **Node ID** : `K_VGA`
//! - **Node Type**: `Plugin`
//! - **Input**   : Raw byte streams (`write_byte`, `write_str`)
//! - **Output**  : Characters rendered to VGA memory at `0xb8000`
//! - **State**   : Global `WRITER` singleton (cursor position + color)
//!
//! # Edges (dependencies)
//! - K_VGA has **no upstream edges** — it is a hardware leaf node.
//!   All other plugins that produce output depend on K_VGA.
//!
//! # Design
//! - No external crates for volatile — uses `core::ptr::{read,write}_volatile`
//!   to prevent the compiler from optimizing away memory-mapped I/O writes.
//! - Fully encapsulated: callers use only `println!` / `print!` macros.
//! - Thread-safety: `spin::Mutex<Writer>` prevents concurrent corruption.

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

// ── Constants ──────────────────────────────────────────────────────────────

/// VGA text buffer physical address (identity-mapped by bootloader).
const VGA_BUFFER_ADDR: usize = 0xb8000;
const BUFFER_WIDTH: usize = 80;
const BUFFER_HEIGHT: usize = 25;

// ── Color Node ─────────────────────────────────────────────────────────────

/// VGA 4-bit color palette.
/// This is a data node: pure value, no behavior.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black      = 0,
    Blue       = 1,
    Green      = 2,
    Cyan       = 3,
    Red        = 4,
    Magenta    = 5,
    Brown      = 6,
    LightGray  = 7,
    DarkGray   = 8,
    LightBlue  = 9,
    LightGreen = 10,
    LightCyan  = 11,
    LightRed   = 12,
    Pink       = 13,
    Yellow     = 14,
    White      = 15,
}

/// Packed foreground+background color byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    #[inline]
    pub fn new(fg: Color, bg: Color) -> Self {
        ColorCode((bg as u8) << 4 | (fg as u8))
    }
}

// ── ScreenChar Node ────────────────────────────────────────────────────────

/// A single VGA character cell: ASCII byte + color attribute.
/// `repr(C)` ensures layout matches the VGA hardware specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii: u8,
    color: ColorCode,
}

// ── Buffer Node ────────────────────────────────────────────────────────────

/// The full 80×25 VGA text buffer, memory-mapped at `0xb8000`.
/// `repr(transparent)` makes it layout-compatible with the raw array.
#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

impl Buffer {
    /// Volatile write: prevents compiler reordering/elision of MMIO writes.
    #[inline]
    fn write_char(&mut self, row: usize, col: usize, sc: ScreenChar) {
        // SAFETY: 0xb8000 is always valid in identity-mapped VGA memory.
        unsafe {
            core::ptr::write_volatile(
                &mut self.chars[row][col] as *mut ScreenChar,
                sc,
            );
        }
    }

    /// Volatile read: prevents compiler from caching stale values.
    #[inline]
    fn read_char(&self, row: usize, col: usize) -> ScreenChar {
        // SAFETY: 0xb8000 is always valid in identity-mapped VGA memory.
        unsafe {
            core::ptr::read_volatile(
                &self.chars[row][col] as *const ScreenChar,
            )
        }
    }
}

// ── Writer Node ────────────────────────────────────────────────────────────

/// Stateful VGA text writer.
/// Maintains cursor position and current color.
/// This is the main behavior node for K_VGA.
pub struct Writer {
    col: usize,
    row: usize,
    color: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    /// Write a single ASCII byte. Handles `\n` and screen scrolling.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => { self.col = 0; }
            byte  => {
                if self.col >= BUFFER_WIDTH {
                    self.newline();
                }
                self.buffer.write_char(self.row, self.col, ScreenChar {
                    ascii: byte,
                    color: self.color,
                });
                self.col += 1;
            }
        }
    }

    /// Write a UTF-8 string, replacing non-printable bytes with `■` (0xFE).
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            if byte.is_ascii() && (byte == b'\n' || byte == b'\r' || (byte >= 0x20 && byte <= 0x7e)) {
                self.write_byte(byte);
            } else {
                self.write_byte(0xfe); // ■ for non-printable
            }
        }
    }

    /// Advance the cursor to the next line, scrolling if at the bottom.
    fn newline(&mut self) {
        if self.row < BUFFER_HEIGHT - 1 {
            self.row += 1;
        } else {
            // Scroll: shift every row up by one (volatile reads + writes).
            for r in 1..BUFFER_HEIGHT {
                for c in 0..BUFFER_WIDTH {
                    let ch = self.buffer.read_char(r, c);
                    self.buffer.write_char(r - 1, c, ch);
                }
            }
            self.clear_row(BUFFER_HEIGHT - 1);
        }
        self.col = 0;
    }

    /// Blank out an entire row with spaces in the current color.
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar { ascii: b' ', color: self.color };
        for c in 0..BUFFER_WIDTH {
            self.buffer.write_char(row, c, blank);
        }
    }

    /// Change the current foreground/background color.
    /// Edge: caller --[configure]--> K_VGA (color control flow).
    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.color = ColorCode::new(fg, bg);
    }

    /// Fill entire screen with blanks and reset cursor to (0, 0).
    pub fn clear(&mut self) {
        self.row = 0;
        self.col = 0;
        for r in 0..BUFFER_HEIGHT {
            self.clear_row(r);
        }
    }

    /// Return current cursor column (used by K_SHELL for line editing).
    pub fn col(&self) -> usize { self.col }

    /// Backspace: erase the previous character.
    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.buffer.write_char(self.row, self.col, ScreenChar {
                ascii: b' ',
                color: self.color,
            });
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// ── Global Singleton (Plugin Output Port) ─────────────────────────────────

lazy_static! {
    /// Global K_VGA writer — the single output port for all VGA text output.
    /// Protected by a spinlock: interrupt-safe, no deadlock risk from K_PANIC.
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        col:    0,
        row:    0,
        color:  ColorCode::new(Color::LightGreen, Color::Black),
        // SAFETY: 0xb8000 is the VGA framebuffer, identity-mapped by bootloader.
        buffer: unsafe { &mut *(VGA_BUFFER_ADDR as *mut Buffer) },
    });
}

// ── Public Macros (Plugin Interface / Output Edges) ───────────────────────

/// Print without newline to the VGA buffer.
/// Edge: any plugin --[data_flow]--> K_VGA via `print!`
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::hal::vga_buffer::_print(format_args!($($arg)*))
    };
}

/// Print with newline to the VGA buffer.
#[macro_export]
macro_rules! println {
    ()              => ($crate::print!("\n"));
    ($($arg:tt)*)   => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // `disable_interrupts` prevents deadlock if an interrupt fires while
    // K_VGA's lock is held. In Phase 0 interrupts are off, but this is
    // defensive programming for Phase 1+.
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}
