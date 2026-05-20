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

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

use spin::Mutex;
use x86_64::instructions::port::Port;

// ── Logical canvas size kept at the historic 320×200 ──────────────
//
// The kernel's UI code is written in 320×200 coordinates so we
// preserve that as the LOGICAL surface.  When the in-kernel Bochs
// DispI mode-set (I.11.B) succeeds, each logical pixel is rendered
// as a `SCALE × SCALE` block of 32-bpp BGRX pixels in the HD linear
// framebuffer.  Result: razor-sharp pixel art at native 1280×720
// (or whatever HD mode succeeded), no fuzzy GL stretch.
pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const PIXELS: usize = WIDTH * HEIGHT;

/// Physical address of the legacy mode 13h framebuffer (fallback).
pub const FB_PHYS: u64 = 0xA0000;

/// Bochs DispI / QEMU stdvga MMIO ports.
const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;
const VBE_DISPI_INDEX_ID: u16 = 0x0;
const VBE_DISPI_INDEX_XRES: u16 = 0x1;
const VBE_DISPI_INDEX_YRES: u16 = 0x2;
const VBE_DISPI_INDEX_BPP: u16 = 0x3;
const VBE_DISPI_INDEX_ENABLE: u16 = 0x4;
const VBE_DISPI_INDEX_VIRT_WIDTH: u16 = 0x6;
const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 0x7;
const VBE_DISPI_DISABLED: u16 = 0x00;
const VBE_DISPI_ENABLED: u16 = 0x01;
const VBE_DISPI_LFB_ENABLED: u16 = 0x40;

/// Fallback LFB physical address used when PCI enumeration fails.
/// 0xfd00_0000 is QEMU's default placement on the i440FX chipset
/// for the stdvga BAR0; q35 and newer setups may place it elsewhere
/// (the PCI enumerator in `discover_stdvga_lfb` walks the bus and
/// reads the actual BAR0, falling back to this constant only when
/// no device matching vendor 0x1234 / device 0x1111 is found).
const HD_LFB_PHYS_FALLBACK: u64 = 0xfd00_0000;

/// PCI configuration mechanism #1 ports (chipset-independent on x86).
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// QEMU std-vga PCI identifiers (Bochs Graphics Adapter clone).
const STDVGA_VENDOR_ID: u16 = 0x1234;
const STDVGA_DEVICE_ID: u16 = 0x1111;

/// Target HD mode dimensions and integer upscale factor.  At
/// SCALE=4, the 320×200 logical canvas covers 1280×800 of the
/// native framebuffer — perfect 4× nearest-neighbor upscale.
const HD_WIDTH: u32 = 1280;
const HD_HEIGHT: u32 = 800; // logical 200 × scale 4 = 800
const HD_SCALE: u32 = 4;

/// VGA DAC ports for the legacy mode 13h palette (fallback path).
const DAC_INDEX: u16 = 0x3C8;
const DAC_DATA: u16 = 0x3C9;

/// Cached framebuffer virtual address.  Zero before `init`.
static FB_VIRT: AtomicU64 = AtomicU64::new(0);

/// Active framebuffer format:
///   1 = legacy mode 13h, 8-bpp palette at 0xA0000
///   4 = HD VBE mode, 32-bpp BGRX linear framebuffer
static FB_BPP: AtomicU8 = AtomicU8::new(1);

/// Native framebuffer width in pixels (320 for mode 13h, 1280 for HD).
static FB_NATIVE_W: AtomicU32 = AtomicU32::new(WIDTH as u32);
/// Native framebuffer height in pixels (200 for mode 13h, 800 for HD).
static FB_NATIVE_H: AtomicU32 = AtomicU32::new(HEIGHT as u32);
/// Integer upscale: logical → native pixel ratio (1 for mode 13h, 4 for HD).
static FB_SCALE: AtomicU32 = AtomicU32::new(1);

/// Discovered LFB physical address (HD path only).  0 means mode 13h.
static FB_LFB_PHYS: AtomicU64 = AtomicU64::new(0);

/// 256-entry palette → BGRX lookup table.  Populated at init for the
/// HD path so per-pixel writes don't go through the named-Color
/// match.  Mode 13h ignores this and writes raw 8-bit indices.
static PALETTE_BGRX: Mutex<[u32; 256]> = Mutex::new([0; 256]);

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
    // ── Phase I.13.c — HD path is opt-in, NOT default ─────────────
    //
    // The Bochs DispI HD mode-set works correctly (320×200 logical
    // → 1280×800 native via 4× upscale, palette → BGRX LUT, PCI BAR
    // discovery), but each per-pixel u32 write goes through QEMU's
    // softmmu callback which is ~1000× slower than RAM in TCG mode.
    // Each frame requires ~2M MMIO writes → ~2-3 seconds per frame
    // → user sees a frozen "header only" state because the body
    // paint hasn't completed.
    //
    // The proper fix is a RAM back-buffer rendered into at logical
    // 320×200 + one bulk REP MOVSD blit per frame to the LFB.  That
    // architecture is the I.14 follow-up; for now we keep mode 13h
    // as the default so the metal-ball view reliably displays.  The
    // HD code stays in place but disabled.
    #[allow(unused_unsafe)]
    let hd_succeeded = false;

    if !hd_succeeded {
        // ── Legacy mode 13h fallback ──
        FB_VIRT.store(phys_offset + FB_PHYS, Ordering::SeqCst);
        FB_BPP.store(1, Ordering::SeqCst);
        FB_NATIVE_W.store(WIDTH as u32, Ordering::SeqCst);
        FB_NATIVE_H.store(HEIGHT as u32, Ordering::SeqCst);
        FB_SCALE.store(1, Ordering::SeqCst);

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
        for (hue_idx, &peak) in HUE_PEAKS.iter().enumerate() {
            let base_slot = 16 + (hue_idx as u8) * 8;
            for shade in 0..8u8 {
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

    // Build the 256-entry palette → BGRX lookup table.  Used by the
    // HD path on every per-pixel write to avoid re-deriving the BGR
    // colour each time.  Mode 13h ignores this (writes raw indices).
    build_palette_lookup();
}

/// Attempt a Bochs DispI / QEMU stdvga mode-set to HD.  Returns true
/// when the mode-set succeeds and the global framebuffer state has
/// been updated to point at the HD linear framebuffer; returns false
/// when no Bochs DispI device is present (caller falls back to mode
/// 13h).
///
/// SAFETY: writes to ports 0x1CE/0x1CF and reads back the version
/// register to detect device presence.  Reads are no-ops on hardware
/// that doesn't claim those ports.
///
/// Currently dead code — kept in place for the I.14 back-buffer
/// re-enablement.  See `init` for context.
#[allow(dead_code)]
unsafe fn try_set_hd_mode(phys_offset: u64) -> bool {
    let mut idx: Port<u16> = Port::new(VBE_DISPI_IOPORT_INDEX);
    let mut data: Port<u16> = Port::new(VBE_DISPI_IOPORT_DATA);

    // Probe the VBE_DISPI version register.  QEMU stdvga returns
    // 0xB0C0..=0xB0CF; anything outside that range = no Bochs DispI.
    unsafe {
        idx.write(VBE_DISPI_INDEX_ID);
    }
    let version = unsafe { data.read() };
    if !(0xB0C0..=0xB0CF).contains(&version) {
        return false;
    }

    // Disable the device, configure the new mode, then re-enable
    // with the LFB flag (linear framebuffer) set.
    unsafe {
        idx.write(VBE_DISPI_INDEX_ENABLE);
        data.write(VBE_DISPI_DISABLED);

        idx.write(VBE_DISPI_INDEX_XRES);
        data.write(HD_WIDTH as u16);
        idx.write(VBE_DISPI_INDEX_YRES);
        data.write(HD_HEIGHT as u16);
        idx.write(VBE_DISPI_INDEX_BPP);
        data.write(32);
        idx.write(VBE_DISPI_INDEX_VIRT_WIDTH);
        data.write(HD_WIDTH as u16);
        idx.write(VBE_DISPI_INDEX_VIRT_HEIGHT);
        data.write(HD_HEIGHT as u16);

        idx.write(VBE_DISPI_INDEX_ENABLE);
        data.write(VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED);
    }

    // Locate the LFB physical address by enumerating PCI for the
    // QEMU stdvga (vendor 0x1234 / device 0x1111) and reading its
    // BAR0.  Without this discovery the hardcoded 0xfd00_0000 only
    // works on QEMU's older i440FX chipset — q35 and pc-q35 place
    // the BAR elsewhere, producing a blank screen because writes
    // land in mapped-but-unused RAM rather than the framebuffer.
    let lfb_phys = unsafe { discover_stdvga_lfb() }.unwrap_or(HD_LFB_PHYS_FALLBACK);

    // bootloader 0.9's `map_physical_memory` feature mapped *all*
    // physical memory at `phys_offset()`, so the LFB virtual
    // address is just `phys_offset + lfb_phys` — no new page-table
    // work needed.
    FB_VIRT.store(phys_offset + lfb_phys, Ordering::SeqCst);
    FB_LFB_PHYS.store(lfb_phys, Ordering::SeqCst);
    FB_BPP.store(4, Ordering::SeqCst);
    FB_NATIVE_W.store(HD_WIDTH, Ordering::SeqCst);
    FB_NATIVE_H.store(HD_HEIGHT, Ordering::SeqCst);
    FB_SCALE.store(HD_SCALE, Ordering::SeqCst);
    true
}

/// Walk PCI bus 0 (and a couple of common bridged buses) looking
/// for the QEMU stdvga device.  Returns the LFB physical address
/// from its BAR0 if found, else None.  Bus scan is cheap enough
/// (256 slots × 32-bit reads) that we don't bother with the formal
/// "secondary bus" walk that a full PCI driver would do.
///
/// SAFETY: reads the chipset PCI config ports (0xCF8 / 0xCFC).
/// These are no-ops on hardware that doesn't claim them.
unsafe fn discover_stdvga_lfb() -> Option<u64> {
    for bus in 0..=0u8 {
        for slot in 0..32u8 {
            let id = unsafe { pci_read_u32(bus, slot, 0, 0x00) };
            let vendor = (id & 0xFFFF) as u16;
            if vendor == 0xFFFF {
                continue; // no device at this slot
            }
            let device = ((id >> 16) & 0xFFFF) as u16;
            if vendor == STDVGA_VENDOR_ID && device == STDVGA_DEVICE_ID {
                // BAR0 lives at config offset 0x10.  For QEMU stdvga
                // this is a 32-bit memory BAR; mask off the low 4
                // bits (type/prefetch flags) to get the address.
                let bar0 = unsafe { pci_read_u32(bus, slot, 0, 0x10) };
                let addr = (bar0 & 0xFFFF_FFF0) as u64;
                if addr != 0 {
                    return Some(addr);
                }
            }
        }
    }
    None
}

unsafe fn pci_read_u32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let addr: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | (((slot as u32) & 0x1F) << 11)
        | (((func as u32) & 0x07) << 8)
        | ((offset as u32) & 0xFC);
    let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
    let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);
    unsafe {
        addr_port.write(addr);
        data_port.read()
    }
}

/// Build the 256-entry palette → BGRX lookup table at init.  Each
/// entry packs as `0x00_RR_GG_BB` in little-endian memory so a single
/// `*u32` write paints one HD pixel.  6-bit channels from the
/// historic palette are stretched to 8-bit via `v*4 + v/16` which
/// distributes the missing low-bit signal cleanly.
fn build_palette_lookup() {
    let mut lut = PALETTE_BGRX.lock();
    let conv6 = |v: u8| -> u32 {
        let v8 = (v as u32) * 4 + (v as u32) / 16;
        v8.min(255)
    };
    let pack_bgrx = |r: u8, g: u8, b: u8| -> u32 {
        (conv6(r) << 16) | (conv6(g) << 8) | conv6(b)
    };

    // Named palette slots 0..11.
    for (i, &(r, g, b)) in PALETTE.iter().enumerate() {
        lut[i] = pack_bgrx(r, g, b);
    }
    // Generate the five Lambertian ramps starting at slot 16.
    for (hue_idx, &peak) in HUE_PEAKS.iter().enumerate() {
        let base_slot = (16 + (hue_idx as u8) * 8) as usize;
        for shade in 0..8u8 {
            let scale = (1u32 + shade as u32).min(8);
            let r = ((peak.0 as u32 * scale) / 8).min(63) as u8;
            let g = ((peak.1 as u32 * scale) / 8).min(63) as u8;
            let b = ((peak.2 as u32 * scale) / 8).min(63) as u8;
            lut[base_slot + shade as usize] = pack_bgrx(r, g, b);
        }
    }
}

/// True once `init` has cached the framebuffer pointer.  Callers in
/// fault paths use this to skip drawing rather than dereferencing a
/// null pointer mid-panic.
pub fn ready() -> bool {
    FB_VIRT.load(Ordering::Acquire) != 0
}

/// Native framebuffer width in pixels (320 for mode 13h, 1280 for HD).
pub fn native_width() -> u32 {
    FB_NATIVE_W.load(Ordering::Relaxed)
}

/// Native framebuffer height in pixels.
pub fn native_height() -> u32 {
    FB_NATIVE_H.load(Ordering::Relaxed)
}

/// True when the in-kernel Bochs DispI mode-set succeeded and the
/// framebuffer is the 32-bpp HD linear surface.
pub fn is_hd() -> bool {
    FB_BPP.load(Ordering::Relaxed) == 4
}

/// Discovered LFB physical address (HD path only).  Returns 0 when
/// the kernel is in mode 13h fallback.
pub fn lfb_physical_address() -> u64 {
    FB_LFB_PHYS.load(Ordering::Relaxed)
}

fn fb_ptr() -> Option<*mut u8> {
    let v = FB_VIRT.load(Ordering::Acquire);
    if v == 0 {
        None
    } else {
        Some(v as *mut u8)
    }
}

// ── Pixel-write primitives — dual-format dispatch ─────────────────
//
// Every drawing helper now checks `FB_BPP`:
//   * 1 (mode 13h)      — write 1 byte palette index at logical
//                         pixel position.
//   * 4 (HD VBE)        — write a SCALE×SCALE block of 32-bit BGRX
//                         pixels into the native framebuffer.
//
// The logical (x, y) coordinates stay 320×200 throughout — only the
// final memory store changes — so all callers (k-rast, hypervisor)
// keep working unchanged.

fn paint_logical_pixel(fb: *mut u8, x: usize, y: usize, palette_idx: u8) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let bpp = FB_BPP.load(Ordering::Relaxed);
    if bpp == 1 {
        unsafe { fb.add(y * WIDTH + x).write(palette_idx); }
        return;
    }
    // 32-bpp HD path.  Look up the BGRX colour once and stamp a
    // SCALE×SCALE block into the LFB.
    let scale = FB_SCALE.load(Ordering::Relaxed) as usize;
    let native_w = FB_NATIVE_W.load(Ordering::Relaxed) as usize;
    let bgrx = PALETTE_BGRX.lock()[palette_idx as usize];
    let base_x = x * scale;
    let base_y = y * scale;
    let fb32 = fb as *mut u32;
    for dy in 0..scale {
        let row_off = (base_y + dy) * native_w + base_x;
        for dx in 0..scale {
            unsafe { fb32.add(row_off + dx).write(bgrx); }
        }
    }
}

/// Paint the whole framebuffer with a single colour.
pub fn clear(color: Color) {
    let Some(fb) = fb_ptr() else { return };
    let _guard = LOCK.lock();
    let bpp = FB_BPP.load(Ordering::Relaxed);
    if bpp == 1 {
        unsafe { core::ptr::write_bytes(fb, color.idx(), PIXELS); }
        return;
    }
    // HD path: solid BGRX flood across the entire native framebuffer.
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    let native_w = FB_NATIVE_W.load(Ordering::Relaxed) as usize;
    let native_h = FB_NATIVE_H.load(Ordering::Relaxed) as usize;
    let fb32 = fb as *mut u32;
    for i in 0..(native_w * native_h) {
        unsafe { fb32.add(i).write(bgrx); }
    }
}

/// Set a single pixel by Color enum.  Logical 320×200 coordinates;
/// the back-end stamps a SCALE×SCALE block in HD mode.
pub fn put_pixel(x: usize, y: usize, color: Color) {
    let Some(fb) = fb_ptr() else { return };
    let _guard = LOCK.lock();
    paint_logical_pixel(fb, x, y, color.idx());
}

/// Set a single pixel by raw palette index.  Used by the 3D scene
/// painter to write per-shade ramp colours.
pub fn put_pixel_raw(x: usize, y: usize, palette_idx: u8) {
    let Some(fb) = fb_ptr() else { return };
    let _guard = LOCK.lock();
    paint_logical_pixel(fb, x, y, palette_idx);
}

/// Solid filled rectangle, clipped against the screen.
pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: Color) {
    let Some(fb) = fb_ptr() else { return };
    if x >= WIDTH || y >= HEIGHT || w == 0 || h == 0 {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let _guard = LOCK.lock();
    let bpp = FB_BPP.load(Ordering::Relaxed);
    if bpp == 1 {
        let row_len = x_end - x;
        for py in y..y_end {
            let row_start = unsafe { fb.add(py * WIDTH + x) };
            unsafe { core::ptr::write_bytes(row_start, color.idx(), row_len); }
        }
        return;
    }
    // HD path: stamp blocks for each logical pixel.
    let scale = FB_SCALE.load(Ordering::Relaxed) as usize;
    let native_w = FB_NATIVE_W.load(Ordering::Relaxed) as usize;
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    let fb32 = fb as *mut u32;
    let base_x_native = x * scale;
    let w_native = (x_end - x) * scale;
    for py in y..y_end {
        let base_y_native = py * scale;
        for dy in 0..scale {
            let row_off = (base_y_native + dy) * native_w + base_x_native;
            for dx in 0..w_native {
                unsafe { fb32.add(row_off + dx).write(bgrx); }
            }
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
    let bpp = FB_BPP.load(Ordering::Relaxed);
    if bpp == 1 {
        unsafe { core::ptr::write_bytes(fb, color.idx(), PIXELS); }
        return;
    }
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    let native_w = FB_NATIVE_W.load(Ordering::Relaxed) as usize;
    let native_h = FB_NATIVE_H.load(Ordering::Relaxed) as usize;
    let fb32 = fb as *mut u32;
    for i in 0..(native_w * native_h) {
        unsafe { fb32.add(i).write(bgrx); }
    }
}

pub unsafe fn force_fill_rect(x: usize, y: usize, w: usize, h: usize, color: Color) {
    let Some(fb) = fb_ptr() else { return };
    if x >= WIDTH || y >= HEIGHT || w == 0 || h == 0 {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let bpp = FB_BPP.load(Ordering::Relaxed);
    if bpp == 1 {
        let row_len = x_end - x;
        for py in y..y_end {
            let row_start = unsafe { fb.add(py * WIDTH + x) };
            unsafe { core::ptr::write_bytes(row_start, color.idx(), row_len); }
        }
        return;
    }
    let scale = FB_SCALE.load(Ordering::Relaxed) as usize;
    let native_w = FB_NATIVE_W.load(Ordering::Relaxed) as usize;
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    let fb32 = fb as *mut u32;
    let base_x_native = x * scale;
    let w_native = (x_end - x) * scale;
    for py in y..y_end {
        let base_y_native = py * scale;
        for dy in 0..scale {
            let row_off = (base_y_native + dy) * native_w + base_x_native;
            for dx in 0..w_native {
                unsafe { fb32.add(row_off + dx).write(bgrx); }
            }
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
        // Unsupported codepoint sentinel — draw a 4×4 dot so it
        // stands out without consuming the whole 8×8 cell.
        let _guard = LOCK.lock();
        for py in 0..4 {
            for px in 0..4 {
                let sx = x + 2 + px;
                let sy = y + 2 + py;
                paint_logical_pixel(fb, sx, sy, color.idx());
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
                paint_logical_pixel(fb, px, py, color.idx());
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
