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

/// Phase I.3.x — cyber/neon palette (6-bit RGB per channel).
///
/// Slots 0..11 are the named `Color` variants above.  Slots 16..55
/// hold five 8-step Lambertian shading ramps used by the 3D scene
/// painter: each cube face picks `HUE_*_RAMP_BASE + (shade 0..7)`
/// based on `normal · light` so cubes read as 3D-lit objects rather
/// than flat sprites.
const PALETTE: &[(u8, u8, u8)] = &[
    (2, 4, 10),    // 0 Background   — deep space blue (near void)
    (4, 18, 28),   // 1 HeaderBar    — neon-cyan dim band
    (55, 58, 60),  // 2 Foreground   — warm phosphor white
    (8, 40, 56),   // 3 NodeKernel   — cyan mid (kept for legacy)
    (16, 50, 32),  // 4 NodeService  — mint mid
    (52, 44, 8),   // 5 NodeDriver   — sun-amber mid
    (48, 12, 40),  // 6 NodeApp      — magenta mid
    (48, 24, 32),  // 7 NodeOther    — rose mid
    (8, 12, 18),   // 8 BarEmpty     — abyssal grey
    (58, 58, 16),  // 9 Highlight    — electric yellow
    (60, 8, 12),   // 10 Error       — neon red
    (30, 40, 46),  // 11 DimWhite    — cool dim frame line
];

/// Base palette indices for each 8-step Lambertian shading ramp.
/// `(slot 0 = dark shadow, slot 7 = full-lit highlight)`.  Reserved
/// from index 16 onward so the named Color enum stays at 0..15 even
/// if we add a few more named entries later.
pub const HUE_CYAN_BASE: u8 = 16;
pub const HUE_MAGENTA_BASE: u8 = 24;
pub const HUE_YELLOW_BASE: u8 = 32;
pub const HUE_MINT_BASE: u8 = 40;
pub const HUE_ROSE_BASE: u8 = 48;

/// Peak (full-lit) 6-bit RGB for each hue ramp.  Shading divides
/// each channel by `8 / (1 + shade)` to roughly approximate a 25%
/// ambient floor (`shade=0`) up to 100% direct lighting (`shade=7`).
const HUE_PEAKS: &[(u8, u8, u8)] = &[
    (16, 48, 60),  // cyan   — hardware / kernel
    (60, 16, 50),  // magenta — driver
    (60, 50, 12),  // yellow — service
    (20, 58, 42),  // mint   — app / plugin entry
    (58, 28, 38),  // rose   — other / generic
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

    // Program the 12 named palette slots (0..11).
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
    // Generate the five Lambertian shading ramps starting at slot 16.
    // Each ramp interpolates from a 12.5%-of-peak shadow (shade 0) up
    // to the full peak (shade 7) linearly per channel.
    for (hue_idx, &peak) in HUE_PEAKS.iter().enumerate() {
        let base_slot = 16 + (hue_idx as u8) * 8;
        for shade in 0..8u8 {
            // Brightness scale: (1 + shade) / 8 — i.e. shade 0 = 1/8,
            // shade 7 = 8/8.  Channel = peak * scale (saturated at 63).
            let scale = (1u32 + shade as u32).min(8);
            let scaled = |c: u8| {
                let v = (c as u32 * scale) / 8;
                v.min(63) as u8
            };
            unsafe {
                idx_port.write(base_slot + shade);
                data_port.write(scaled(peak.0));
                data_port.write(scaled(peak.1));
                data_port.write(scaled(peak.2));
            }
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

/// Set a single pixel by raw 8-bit palette index.  Used by the 3D
/// scene painter to pick from the Lambertian shading ramps (slots
/// 16..55) without enumerating each shade in the `Color` enum.
pub fn put_pixel_raw(x: usize, y: usize, palette_idx: u8) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let Some(fb) = fb_ptr() else { return };
    let _guard = LOCK.lock();
    unsafe {
        fb.add(y * WIDTH + x).write(palette_idx);
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

// ── Panic-safe (no-lock) paint variants ────────────────────────────
//
// The regular `clear` / `fill_rect` paths acquire `LOCK` per call.
// That is fine from the idle paint loop, but DANGEROUS from the
// `#[panic_handler]`: if the panic fired while the idle loop held the
// mutex (e.g. mid-`paint_3d_view`'s pixel loop), the panic UI would
// spin-deadlock and the user would see a frozen mid-frame image
// instead of the diagnostic crimson screen.
//
// `force_*` variants bypass the mutex.  Caller is responsible for
// disabling interrupts first so no IRQ writer can race.  Intended
// EXCLUSIVELY for the panic-paint path on its way to `hlt` forever.
//
// # Safety
// Two contracts:
//   1. Interrupts must be disabled when called.
//   2. The kernel must be on a one-way path to halt (no rendering
//      loop will resume).  Concurrent live writers ARE a data race
//      and UB; `force_*` is named loudly to discourage misuse.

pub unsafe fn force_clear(color: Color) {
    let Some(fb) = fb_ptr() else { return };
    unsafe {
        core::ptr::write_bytes(fb, color.idx(), PIXELS);
    }
}

pub unsafe fn force_fill_rect(x: usize, y: usize, w: usize, h: usize, color: Color) {
    let Some(fb) = fb_ptr() else { return };
    if x >= WIDTH || y >= HEIGHT || w == 0 || h == 0 {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let row_len = x_end - x;
    for py in y..y_end {
        let row_start = unsafe { fb.add(py * WIDTH + x) };
        unsafe {
            core::ptr::write_bytes(row_start, color.idx(), row_len);
        }
    }
}

// ── Phase I.3.9 — shared camera input atomics ─────────────────────
//
// k-fb hosts this state because it's the only kernel-side crate that
// both the input driver (k-ps2) and the boot UI painter
// (hypervisor::paint_3d_view) already depend on / are reachable from.
// Atomics keep the IRQ-context writer (PS/2 post stage) safely
// concurrent with the idle-loop reader without needing a Mutex.
//
// Deltas are *accumulators*: the painter snapshots+clears them each
// frame so a held key produces continuous motion at the painter's
// repaint rate, not at the keyboard's autorepeat rate.

use core::sync::atomic::{AtomicBool, AtomicI32};

/// True when auto-rotate yaw advance should run.  F1 toggles.
pub static CAMERA_AUTO_ROTATE: AtomicBool = AtomicBool::new(true);

/// Cumulative camera bias in fixed-point milli-radians.
/// `i32::MAX ≈ 2.1×10⁶ rad` — comfortably more headroom than the
/// camera ever needs.  Painter divides by 1000.0 to convert to f32
/// radians before applying.
pub static CAMERA_YAW_BIAS_MRAD: AtomicI32 = AtomicI32::new(0);
pub static CAMERA_PITCH_BIAS_MRAD: AtomicI32 = AtomicI32::new(0);

/// Camera orbit radius in millimetre-equivalent fixed-point.  Start
/// at 3.5 units = 3500 mrad-equivalent.  F6 resets, F7/F8 zoom.
pub static CAMERA_RADIUS_MM: AtomicI32 = AtomicI32::new(3500);

// ── Phase I.5 — kernel-UI command bar + mode switch ───────────────
//
// The boot UI runs in two modes:
//   * `UI_MODE_OS_SHELL`     — title + system status + command bar.
//                              The default after boot.
//   * `UI_MODE_KERNEL_VIEW`  — the live 3D graph (octahedra + edges +
//                              halos + gizmo).
//
// A character ring buffer fed by k-ps2 lets the painter drain typed
// keystrokes per frame.  Commands typed into the bar (`kernel`, `os`,
// `help`, …) switch modes / produce log lines.  The scrollback panel
// is collapsed by default and toggled with F9.
//
// Why state lives in k-fb: it's already the kernel-side crate every
// other UI consumer depends on (k-ps2, hypervisor::main, k-panic),
// so adding the shared input ring here avoids a fresh crate or
// circular dep.  Single-CPU boot ⇒ Mutex is uncontended in practice.

pub const UI_MODE_OS_SHELL: u8 = 0;
pub const UI_MODE_KERNEL_VIEW: u8 = 1;

// I.9 — boot default is the rotating 3D metal-ball + rope scene; the
// user can drop back to the OS shell with `os` / `exit` / Esc.
pub static UI_MODE: core::sync::atomic::AtomicU8 =
    core::sync::atomic::AtomicU8::new(UI_MODE_KERNEL_VIEW);

pub static UI_SCROLLBACK_EXPANDED: AtomicBool = AtomicBool::new(false);

/// Phase I.8 — last-clicked node's `VectorAddress` packed into a
/// u64.  Set by the 3D-view click handler when the user picks a
/// ball; consumed by the command-bar's Tab handler which expands
/// it into the literal `'<l4>.<l3>.<l2>.<offset>'` form at the
/// cursor.  0 means "no node has been clicked yet" — Tab is a
/// no-op in that state.
pub static UI_LAST_CLICK_VECTOR: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// Fixed-capacity SPSC byte ring for keystrokes flowing from the
/// PS/2 driver into the boot UI loop.  Capacity 64 is plenty: the
/// painter drains every frame (~50 Hz) and the keyboard tops out
/// at ~30 cps even with autorepeat on.
const TYPED_RING_CAP: usize = 64;

struct TypedRing {
    buf: [u8; TYPED_RING_CAP],
    head: usize, // write index
    tail: usize, // read index
}

impl TypedRing {
    const fn new() -> Self {
        Self { buf: [0; TYPED_RING_CAP], head: 0, tail: 0 }
    }
    fn push(&mut self, b: u8) -> bool {
        let next = (self.head + 1) % TYPED_RING_CAP;
        if next == self.tail {
            return false; // drop on overflow rather than block
        }
        self.buf[self.head] = b;
        self.head = next;
        true
    }
    fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            return None;
        }
        let b = self.buf[self.tail];
        self.tail = (self.tail + 1) % TYPED_RING_CAP;
        Some(b)
    }
}

static TYPED_RING: Mutex<TypedRing> = Mutex::new(TypedRing::new());

/// Called from k-ps2's `proc::process` to mirror an ASCII keystroke
/// into the kernel-UI input channel in parallel with the existing
/// shell route.  Dropping on overflow is acceptable: the user types
/// way slower than the painter drains.
pub fn push_typed_char(b: u8) {
    TYPED_RING.lock().push(b);
}

/// Drain one queued keystroke for the boot UI loop.  Returns None
/// when the ring is empty.
pub fn pop_typed_char() -> Option<u8> {
    TYPED_RING.lock().pop()
}

// ── Phase I.3.4 — 8×8 ASCII glyph rendering ────────────────────────
//
// Backed by `font8x8::legacy::BASIC_LEGACY` (public-domain BIOS-PC
// 8×8 font, 128 ASCII glyphs × 8 bytes).  Each glyph byte encodes
// one pixel row, LSB = leftmost pixel.
//
// `draw_glyph` writes only the SET bits — background pixels are left
// untouched.  Callers that want a solid background fill it first via
// `fill_rect`.  This keeps the text path cheap and makes labels on
// arbitrary tile colours readable without per-glyph alpha logic.

pub const GLYPH_W: usize = 8;
pub const GLYPH_H: usize = 8;

/// Draw a single ASCII character.  Out-of-range chars become a small
/// solid block (visible diagnostic for "unsupported codepoint" without
/// crashing the renderer).
pub fn draw_glyph(x: usize, y: usize, ch: char, color: Color) {
    let Some(fb) = fb_ptr() else { return };
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let glyph = if (ch as u32) < 128 {
        &font8x8::legacy::BASIC_LEGACY[ch as usize]
    } else {
        // Unsupported codepoint sentinel — draw a 4×4 dot so it stands
        // out without consuming the whole 8×8 cell.
        let _guard = LOCK.lock();
        for py in 0..4 {
            for px in 0..4 {
                let sx = x + 2 + px;
                let sy = y + 2 + py;
                if sx < WIDTH && sy < HEIGHT {
                    unsafe {
                        fb.add(sy * WIDTH + sx).write(color.idx());
                    }
                }
            }
        }
        return;
    };
    let _guard = LOCK.lock();
    for row in 0..GLYPH_H {
        let bits = glyph[row];
        let py = y + row;
        if py >= HEIGHT {
            break;
        }
        for col in 0..GLYPH_W {
            if bits & (1 << col) != 0 {
                let px = x + col;
                if px >= WIDTH {
                    continue;
                }
                unsafe {
                    fb.add(py * WIDTH + px).write(color.idx());
                }
            }
        }
    }
}

/// Draw an ASCII string left-to-right starting at (x, y).  Non-ASCII
/// chars render via the `draw_glyph` sentinel.  No wrapping — callers
/// that need it pre-truncate.
pub fn draw_text(x: usize, y: usize, text: &str, color: Color) {
    let mut cx = x;
    for ch in text.chars() {
        if cx + GLYPH_W > WIDTH {
            break;
        }
        draw_glyph(cx, y, ch, color);
        cx += GLYPH_W;
    }
}

/// Convenience: draw text inside a colored background box that's
/// exactly the right size, with 1 px inset.  Used by the boot UI
/// header so the title reads cleanly on any palette.
pub fn draw_text_boxed(x: usize, y: usize, text: &str, fg: Color, bg: Color) {
    let w = text.chars().count() * GLYPH_W + 4;
    let h = GLYPH_H + 4;
    fill_rect(x, y, w.min(WIDTH.saturating_sub(x)), h, bg);
    draw_text(x + 2, y + 2, text, fg);
}
