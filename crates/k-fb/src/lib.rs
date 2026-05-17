#![no_std]

//! Phase I.3.1 — VGA mode 13h linear framebuffer driver.
//!
//! The bootloader's `vga_320x200` feature switches the VGA card into
//! mode 13h (320×200 × 256-colour palette) before jumping to
//! `kernel_main`.  This crate wraps the resulting framebuffer at
//! physical address `0xA0000` (mapped at `phys_offset() + 0xA0000`
//! by the `map_physical_memory` bootloader feature) and exposes the
//! minimum primitives the kernel UI needs for Gen-1:
//!
//!   * `init(phys_offset)` — stash the framebuffer base and program a
//!     known palette (the `Color` enum below).
//!   * `clear(color)` / `put_pixel(x, y, color)` / `fill_rect(...)`
//!     for solid drawing.
//!   * `present()` is a no-op (mode 13h is a direct linear FB; writes
//!     are immediately visible).  Kept as a hook so future double-
//!     buffered paths (`Phase I.x` VBE LFB) slot in without rewriting
//!     call sites.
//!
//! Synchronisation: all writes go through a `spin::Mutex` so the
//! eventual multi-core path is sound today.  Single-CPU boot just
//! pays the uncontended-mutex cost.
//!
//! Text rendering, 3D rasterisation, and dirty-region tracking are
//! follow-up slices.  Today the UI is rectangles only — enough to
//! prove the framebuffer pipeline boots and shows kernel state.

use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;
use x86_64::instructions::port::Port;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const PIXELS: usize = WIDTH * HEIGHT;

/// Physical address of the mode 13h framebuffer.  Mapped by the
/// bootloader's `map_physical_memory` feature into the kernel virtual
/// address space at `phys_offset() + FB_PHYS`.
pub const FB_PHYS: u64 = 0xA0000;

/// VGA DAC ports.  Writes to 0x3C8 select the palette index to load
/// next; subsequent writes to 0x3C9 take three bytes (R, G, B) each
/// in the 0..63 range (the legacy 6-bit DAC).
const DAC_INDEX: u16 = 0x3C8;
const DAC_DATA: u16 = 0x3C9;

/// Cached framebuffer virtual address.  Zero before `init`.
static FB_VIRT: AtomicU64 = AtomicU64::new(0);

/// Coarse-grained lock around the framebuffer.  Per-pixel locking
/// would tank throughput; per-frame locking is more than enough for
/// the Gen-1 UI which redraws on demand only.
static LOCK: Mutex<()> = Mutex::new(());

/// Named palette slots.  The numeric value is the VGA palette index
/// programmed in `init`.  Callers only ever pass a `Color` so we can
/// re-tune the palette (e.g. theme.wabi vs theme.shoji) without
/// rewriting drawing code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Background = 0,    // dark slate
    HeaderBar = 1,     // deep teal
    Foreground = 2,    // warm off-white text
    NodeKernel = 3,    // muted ochre
    NodeService = 4,   // jade
    NodeDriver = 5,    // amber
    NodeApp = 6,       // violet
    NodeOther = 7,     // dusty rose
    BarEmpty = 8,      // soft grey
    Highlight = 9,     // sun yellow
    Error = 10,        // crimson
    DimWhite = 11,     // soft frame line
}

impl Color {
    pub const fn idx(self) -> u8 {
        self as u8
    }
}

/// 6-bit RGB triplet (0..63 per channel) as the VGA DAC expects.
const PALETTE: &[(u8, u8, u8)] = &[
    (5, 6, 9),     // 0 Background   — near-black slate
    (8, 18, 24),   // 1 HeaderBar    — deep teal
    (52, 50, 44),  // 2 Foreground   — warm off-white
    (40, 30, 12),  // 3 NodeKernel   — ochre
    (18, 36, 22),  // 4 NodeService  — jade
    (50, 36, 14),  // 5 NodeDriver   — amber
    (28, 18, 40),  // 6 NodeApp      — violet
    (40, 24, 28),  // 7 NodeOther    — dusty rose
    (18, 18, 22),  // 8 BarEmpty     — soft grey
    (58, 52, 16),  // 9 Highlight    — sun yellow
    (50, 12, 14),  // 10 Error       — crimson
    (35, 35, 38),  // 11 DimWhite    — soft frame line
];

/// Install the palette + cache the framebuffer base.  Call once from
/// `kernel_main` immediately after `gos_hal::phys::set_phys_offset`.
///
/// # Safety
/// Writes to the VGA DAC ports and dereferences `phys_offset +
/// 0xA0000`.  Caller must guarantee the bootloader switched the card
/// into mode 13h (i.e. the `vga_320x200` feature is enabled) and that
/// the physical memory at 0xA0000 is mapped into the kernel virtual
/// address space.
pub unsafe fn init(phys_offset: u64) {
    FB_VIRT.store(phys_offset + FB_PHYS, Ordering::SeqCst);

    // Program the 12 palette slots we own.  Bootloader's mode-13h
    // setup leaves a default palette in place; we overwrite only
    // what we use so anything still indexing into the default high
    // slots (debug code, future themes) keeps working.
    let mut idx_port: Port<u8> = Port::new(DAC_INDEX);
    let mut data_port: Port<u8> = Port::new(DAC_DATA);
    for (i, &(r, g, b)) in PALETTE.iter().enumerate() {
        unsafe {
            idx_port.write(i as u8);
            data_port.write(r);
            data_port.write(g);
            data_port.write(b);
        }
    }
}

/// True once `init` has cached the framebuffer pointer.  Callers in
/// fault paths use this to skip drawing rather than dereferencing a
/// null pointer mid-panic.
pub fn ready() -> bool {
    FB_VIRT.load(Ordering::Acquire) != 0
}

fn fb_ptr() -> Option<*mut u8> {
    let v = FB_VIRT.load(Ordering::Acquire);
    if v == 0 {
        None
    } else {
        Some(v as *mut u8)
    }
}

/// Paint the whole framebuffer with a single colour.
pub fn clear(color: Color) {
    let Some(fb) = fb_ptr() else { return };
    let _guard = LOCK.lock();
    unsafe {
        core::ptr::write_bytes(fb, color.idx(), PIXELS);
    }
}

/// Set a single pixel.  Out-of-bounds coordinates silently no-op;
/// the boot UI is statically sized so we never see this in practice,
/// but the bounds check keeps a stray future draw from clobbering
/// memory past the framebuffer.
pub fn put_pixel(x: usize, y: usize, color: Color) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let Some(fb) = fb_ptr() else { return };
    let _guard = LOCK.lock();
    unsafe {
        fb.add(y * WIDTH + x).write(color.idx());
    }
}

/// Solid filled rectangle, clipped against the screen.
pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: Color) {
    let Some(fb) = fb_ptr() else { return };
    if x >= WIDTH || y >= HEIGHT || w == 0 || h == 0 {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let row_len = x_end - x;
    let _guard = LOCK.lock();
    for py in y..y_end {
        let row_start = unsafe { fb.add(py * WIDTH + x) };
        unsafe {
            core::ptr::write_bytes(row_start, color.idx(), row_len);
        }
    }
}

/// Single-pixel horizontal hairline.  Convenience wrapper used for
/// frame borders.
pub fn hline(x: usize, y: usize, w: usize, color: Color) {
    fill_rect(x, y, w, 1, color);
}

/// Single-pixel vertical hairline.
pub fn vline(x: usize, y: usize, h: usize, color: Color) {
    fill_rect(x, y, 1, h, color);
}

/// Draw a 1-pixel-wide outline around the given rectangle.
pub fn stroke_rect(x: usize, y: usize, w: usize, h: usize, color: Color) {
    if w == 0 || h == 0 {
        return;
    }
    hline(x, y, w, color);
    if h > 1 {
        hline(x, y + h - 1, w, color);
    }
    if h > 2 {
        vline(x, y + 1, h - 2, color);
        if w > 1 {
            vline(x + w - 1, y + 1, h - 2, color);
        }
    }
}

/// Mode 13h writes are immediately visible; this hook exists for
/// future VBE LFB / virtio-gpu paths that need an explicit flush.
pub fn present() {}
