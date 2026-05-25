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

/// Target HD mode dimensions and integer upscale factor.  N.13 bumps
/// from 1280×800 (4×) to 1920×1200 (6×) — fills modern 1080p / 1200p
/// displays at exact integer scale, no fuzzy stretch.  Per-frame LFB
/// blit grows from 4 MiB to 9.2 MiB; offset by the N.13 PaintSession
/// batching which removes ~100 k spin-mutex acquires per frame.
const HD_WIDTH: u32 = 1920;
const HD_HEIGHT: u32 = 1200; // logical 200 × scale 6 = 1200
const HD_SCALE: u32 = 6;

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

/// Phase N.9 — true-colour RAM back-buffer.  Stores BGRX (0x00_RR_GG_BB)
/// at 320×200 logical resolution; `present()` blits to LFB with the
/// per-mode upscale.  N.7 was a palette-indexed (u8) backbuffer,
/// which capped the sphere shader at 8 quantised shades per hue
/// ramp — visible banding.  N.9 promotes it to 32-bpp so the shader
/// writes the ACES-tonemapped 24-bit colour directly: smooth gradients,
/// no banding, real photographic look.
///
/// 256 KiB of kernel BSS — well below the 4 MiB k-heap.
///
/// Compatibility layer: legacy palette-indexed callers (text, rects,
/// edges, UI chrome) continue to work via the in-init PALETTE_BGRX
/// LUT — `paint_logical_pixel(idx)` resolves to the BGRX and stores
/// it.  Sphere shader uses the new `put_pixel_rgb(bgrx)` for direct
/// 24-bit writes.
static BACKBUFFER: Mutex<[u32; PIXELS]> = Mutex::new([0; PIXELS]);

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
    // ── Phase N.x — HD path now enabled ──────────────────────────
    //
    // The earlier veto (I.13.c) said HD was unusable because each
    // per-pixel u32 LFB write hit a softmmu callback in QEMU TCG and
    // a single frame took ~2-3 sec.  We've since added a RAM
    // back-buffer (BACKBUFFER) so all per-pixel writes land in cached
    // memory, and `present()` blits the whole frame in one REP MOVSD
    // burst per scanline.  TCG fast-paths bulk MMIO copies, so the
    // frame budget is now ~10-30 ms at SCALE=4 (1280×800) — comfortable
    // for the 30+ fps paint loop.
    //
    // Falls back to mode 13h if the Bochs DispI device isn't present
    // (e.g. headless test, exotic chipsets).
    let hd_succeeded = unsafe { try_set_hd_mode(phys_offset) };

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
/// N.x — live again now that BACKBUFFER + bulk `present()` make HD
/// frame budget feasible under QEMU TCG.  See `init` for context.
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

    // Phase N.8 — derive a luminance table from BGRX so shader code
    // can convert a palette index back to a 0..=255 brightness scalar
    // without re-running the colour-pack math.  Uses ITU-R BT.601 weights.
    let mut lum = PALETTE_LUM.lock();
    for i in 0..256 {
        let bgrx = lut[i];
        let r = (bgrx >> 16) & 0xFF;
        let g = (bgrx >> 8) & 0xFF;
        let b = bgrx & 0xFF;
        lum[i] = ((r * 299 + g * 587 + b * 114) / 1000).min(255) as u8;
    }
}

/// 256-entry palette → luminance (BT.601 weighted).  Populated by
/// `build_palette_lookup` so the shader (N.8 cubemap reflection)
/// can read a baked palette-indexed pixel and reuse its brightness
/// as an additive light contribution.
static PALETTE_LUM: Mutex<[u8; 256]> = Mutex::new([0; 256]);

/// Look up the cached BT.601 luminance of a palette index.
/// Returns 0..=255.  Cheap atomic-ish access; safe from any context.
pub fn palette_luminance(idx: u8) -> u8 {
    PALETTE_LUM.lock()[idx as usize]
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

/// Write a single palette-indexed pixel into the RAM back-buffer.
/// The palette index is resolved to BGRX via PALETTE_BGRX so the
/// back-buffer always stores true colour.  `present()` is the only
/// thing that touches the real framebuffer.
fn paint_logical_pixel(_fb: *mut u8, x: usize, y: usize, palette_idx: u8) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let bgrx = PALETTE_BGRX.lock()[palette_idx as usize];
    BACKBUFFER.lock()[y * WIDTH + x] = bgrx;
}

/// Paint the whole back-buffer with a single colour.  Bulk fill in RAM.
pub fn clear(color: Color) {
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    BACKBUFFER.lock().fill(bgrx);
}

/// N.10 — paint with a vertical gradient between two colors.  The top
/// row uses `top`, bottom row uses `bottom`, intermediate rows linearly
/// interpolate per-channel.  Used for the modern UI chrome (header,
/// status bar) instead of the old flat-colour rectangle.
pub fn clear_gradient_vertical(top: u32, bottom: u32) {
    let mut bb = BACKBUFFER.lock();
    for y in 0..HEIGHT {
        let t = y as u32 * 256 / HEIGHT as u32; // 0..256
        let bgrx = lerp_bgrx(top, bottom, t as u8);
        let row = y * WIDTH;
        for x in 0..WIDTH {
            bb[row + x] = bgrx;
        }
    }
}

/// Linear interpolation between two BGRX colours.  `t` is a 0..=255
/// blend factor (0 = a, 255 = b).
#[inline]
pub fn lerp_bgrx(a: u32, b: u32, t: u8) -> u32 {
    let t = t as u32;
    let it = 255 - t;
    let blend = |ch_shift: u32| -> u32 {
        let av = (a >> ch_shift) & 0xFF;
        let bv = (b >> ch_shift) & 0xFF;
        ((av * it + bv * t) / 255) & 0xFF
    };
    (blend(16) << 16) | (blend(8) << 8) | blend(0)
}

/// Look up the BGRX value for a Color enum slot.  Convenience for
/// callers that need direct RGB math (the rope shader scales the
/// hue by per-pixel intensity).
#[inline]
pub fn color_bgrx(c: Color) -> u32 {
    PALETTE_BGRX.lock()[c.idx() as usize]
}

/// Scale a BGRX colour by an intensity in [0, 1].  Per-channel
/// multiply, clamped to 255.  Used by the rope cross-section shader.
#[inline]
pub fn scale_bgrx(bgrx: u32, intensity: f32) -> u32 {
    let t = intensity.clamp(0.0, 1.5);
    let r = ((((bgrx >> 16) & 0xFF) as f32) * t).min(255.0) as u32;
    let g = ((((bgrx >> 8) & 0xFF) as f32) * t).min(255.0) as u32;
    let b = (((bgrx & 0xFF) as f32) * t).min(255.0) as u32;
    (r << 16) | (g << 8) | b
}

/// HSV → BGRX (24-bit).  Standard 6-segment conversion.  Used by the
/// N.16 chromatic-vapor border effect to cycle along the full hue
/// wheel.  `h` should be in [0, 1).
#[inline]
pub fn hsv_to_bgrx(h: f32, s: f32, v: f32) -> u32 {
    let mut h = h - libm::floorf(h); // wrap to [0,1)
    if h < 0.0 { h += 1.0; }
    let h6 = h * 6.0;
    let i = h6 as i32 % 6;
    let f = h6 - libm::floorf(h6);
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let r = (r * 255.0).min(255.0) as u32;
    let g = (g * 255.0).min(255.0) as u32;
    let b = (b * 255.0).min(255.0) as u32;
    (r << 16) | (g << 8) | b
}

/// N.16 — chromatic vapor: paint a 1-pixel line whose hue cycles
/// along its length + drifts on `phase` per frame, with a soft 2-px
/// alpha-blended glow halo on both sides.  Reads as "neon mist
/// flowing along an edge" — high-impact chrome accent without
/// disturbing the 3D scene.
///
/// `freq`  = hue cycles per pixel (use ~0.01 for one wheel per 100 px).
/// `phase` = frame-driven offset (just `frame_f * 0.002` is enough
///           for a subtle ~10 s cycle).
pub fn vapor_h_line(y: usize, x0: usize, x1: usize, phase: f32, freq: f32) {
    if y >= HEIGHT { return; }
    let x_end = x1.min(WIDTH);
    if x0 >= x_end { return; }
    let mut bb = BACKBUFFER.lock();
    for x in x0..x_end {
        let hue = x as f32 * freq + phase;
        let bgrx = hsv_to_bgrx(hue, 0.85, 0.95);
        // Centre pixel — opaque.
        bb[y * WIDTH + x] = bgrx;
        // Inner halo (1 px above + below at 60% alpha).
        if y > 0 {
            let off = (y - 1) * WIDTH + x;
            bb[off] = lerp_bgrx(bb[off], bgrx, 160);
        }
        if y + 1 < HEIGHT {
            let off = (y + 1) * WIDTH + x;
            bb[off] = lerp_bgrx(bb[off], bgrx, 160);
        }
        // Outer halo (2 px out at 30% alpha).
        if y >= 2 {
            let off = (y - 2) * WIDTH + x;
            bb[off] = lerp_bgrx(bb[off], bgrx, 80);
        }
        if y + 2 < HEIGHT {
            let off = (y + 2) * WIDTH + x;
            bb[off] = lerp_bgrx(bb[off], bgrx, 80);
        }
    }
}

/// Vertical companion to `vapor_h_line` — 1-px column with side glow.
pub fn vapor_v_line(x: usize, y0: usize, y1: usize, phase: f32, freq: f32) {
    if x >= WIDTH { return; }
    let y_end = y1.min(HEIGHT);
    if y0 >= y_end { return; }
    let mut bb = BACKBUFFER.lock();
    for y in y0..y_end {
        let hue = y as f32 * freq + phase;
        let bgrx = hsv_to_bgrx(hue, 0.85, 0.95);
        bb[y * WIDTH + x] = bgrx;
        if x > 0 {
            let off = y * WIDTH + x - 1;
            bb[off] = lerp_bgrx(bb[off], bgrx, 160);
        }
        if x + 1 < WIDTH {
            let off = y * WIDTH + x + 1;
            bb[off] = lerp_bgrx(bb[off], bgrx, 160);
        }
        if x >= 2 {
            let off = y * WIDTH + x - 2;
            bb[off] = lerp_bgrx(bb[off], bgrx, 80);
        }
        if x + 2 < WIDTH {
            let off = y * WIDTH + x + 2;
            bb[off] = lerp_bgrx(bb[off], bgrx, 80);
        }
    }
}

/// N.16 — vapor border around a rectangle.  Walks the four edges
/// continuously with a single phase parameter so the hue rolls
/// around the perimeter like neon piping.  Useful for floating
/// panel chrome (the new centred command box).
pub fn vapor_rect_border(x: usize, y: usize, w: usize, h: usize, phase: f32) {
    if w < 2 || h < 2 { return; }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    // Use a single freq that gives ~1 full wheel around the perimeter.
    let perimeter = (2 * w + 2 * h) as f32;
    let freq = 1.0 / perimeter;

    // Top edge (left → right): hue index 0..w
    vapor_h_line(y, x, x_end, phase, freq);
    // Right edge (top → bottom): hue index w..w+h
    if x_end > 0 {
        let phase_r = phase + (w as f32) * freq;
        vapor_v_line(x_end - 1, y, y_end, phase_r, freq);
    }
    // Bottom edge (right → left): hue index w+h..2w+h
    if y_end > 0 {
        let phase_b = phase + (w + h) as f32 * freq;
        // Render right-to-left by reversing the freq sign.
        vapor_h_line(y_end - 1, x, x_end, phase_b, -freq);
    }
    // Left edge (bottom → top)
    let phase_l = phase + (2 * w + h) as f32 * freq;
    vapor_v_line(x, y, y_end, phase_l, -freq);
}

/// Walk along the perimeter of `(x, y, w, h)` and return the (px, py)
/// at distance `p` (in pixels) from the top-left corner, moving
/// clockwise.  Wraps automatically.
#[inline]
fn perimeter_pos(x: usize, y: usize, w: usize, h: usize, p: f32) -> (i32, i32) {
    let perimeter = (2 * w + 2 * h) as f32;
    let mut p = p;
    while p < 0.0   { p += perimeter; }
    while p >= perimeter { p -= perimeter; }
    let wf = (w - 1) as f32;
    let hf = (h - 1) as f32;
    if p < wf {
        // Top edge L → R
        ((x as f32 + p) as i32, y as i32)
    } else if p < wf + hf {
        // Right edge T → B
        ((x + w - 1) as i32, (y as f32 + (p - wf)) as i32)
    } else if p < 2.0 * wf + hf {
        // Bottom edge R → L
        ((x as f32 + wf - (p - wf - hf)) as i32, (y + h - 1) as i32)
    } else {
        // Left edge B → T
        (x as i32, (y as f32 + hf - (p - 2.0 * wf - hf)) as i32)
    }
}

/// Render a soft AA disc into the back-buffer (alpha-blended).
/// Used by the particle vapor + by future gas / spark effects.
#[inline]
fn render_soft_disc(
    bb: &mut [u32],
    cx: i32, cy: i32, radius: f32,
    bgrx: u32, peak_alpha: u8,
) {
    let ri = libm::ceilf(radius + 0.5) as i32;
    for dy in -ri..=ri {
        let py = cy + dy;
        if py < 0 || py >= HEIGHT as i32 { continue; }
        for dx in -ri..=ri {
            let px = cx + dx;
            if px < 0 || px >= WIDTH as i32 { continue; }
            let d2 = (dx * dx + dy * dy) as f32;
            let dist = libm::sqrtf(d2);
            let cov = (radius + 0.5 - dist).clamp(0.0, 1.0);
            if cov == 0.0 { continue; }
            let alpha = ((cov * peak_alpha as f32) as u32).min(255) as u8;
            let off = py as usize * WIDTH + px as usize;
            bb[off] = lerp_bgrx(bb[off], bgrx, alpha);
        }
    }
}

/// N.17 — gas-particle vapor border.  Replaces the continuous
/// HSV-line look with discrete drifting "gas" particles strung
/// around the perimeter.  Each particle:
///   * 2-px radius soft AA disc
///   * full-spectrum hue determined by index + phase
///   * advances clockwise at `phase` rate
///   * additive trail (one fainter copy 6 px behind) for motion blur
///
/// Particle density: 1 per ~10 px of perimeter, clamped 6..36.
/// The whole pass takes one BACKBUFFER lock.
pub fn vapor_rect_particles(x: usize, y: usize, w: usize, h: usize, phase: f32) {
    if w < 4 || h < 4 { return; }
    let perimeter = (2 * w + 2 * h) as f32;
    let n = ((perimeter / 10.0) as usize).clamp(6, 36);
    let inv_n = 1.0 / n as f32;
    let mut bb = BACKBUFFER.lock();
    for i in 0..n {
        // Phase per particle: evenly spaced + global drift.
        let t = i as f32 * inv_n + phase;
        let pos_px = t * perimeter;
        let (cx, cy) = perimeter_pos(x, y, w, h, pos_px);
        // Hue: spatially varied + slow temporal drift.
        let hue = i as f32 * inv_n * 1.3 + phase * 0.7;
        let bgrx = hsv_to_bgrx(hue, 0.85, 1.0);
        // Main particle (full alpha)
        render_soft_disc(&mut bb[..], cx, cy, 1.8, bgrx, 230);
        // Trail (one fainter disc ~5 px back along the perimeter)
        let trail_pos = pos_px - 5.0;
        let (tx, ty) = perimeter_pos(x, y, w, h, trail_pos);
        render_soft_disc(&mut bb[..], tx, ty, 1.4, bgrx, 110);
        let trail2_pos = pos_px - 10.0;
        let (tx2, ty2) = perimeter_pos(x, y, w, h, trail2_pos);
        render_soft_disc(&mut bb[..], tx2, ty2, 1.1, bgrx, 50);
    }
}

// ── N.12 — alpha-blended primitives for next-gen UI ────────────────

/// Alpha-blend a pixel onto the back-buffer.  `alpha` is 0..=255
/// where 0 = transparent (no change), 255 = opaque (overwrite).
/// Cornerstone of every AA / glass / shadow primitive.
#[inline]
pub fn blend_pixel_rgb(x: usize, y: usize, fg_bgrx: u32, alpha: u8) {
    if x >= WIDTH || y >= HEIGHT || alpha == 0 {
        return;
    }
    let mut bb = BACKBUFFER.lock();
    let bg = bb[y * WIDTH + x];
    bb[y * WIDTH + x] = lerp_bgrx(bg, fg_bgrx, alpha);
}

/// Fill a rectangle with a translucent colour.  `alpha` is 0..=255.
/// Each pixel is `lerp(existing, fg, alpha)` — glass / tint / shadow
/// panel implementation.
pub fn fill_rect_alpha(x: usize, y: usize, w: usize, h: usize, fg_bgrx: u32, alpha: u8) {
    if x >= WIDTH || y >= HEIGHT || w == 0 || h == 0 || alpha == 0 {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let mut bb = BACKBUFFER.lock();
    for py in y..y_end {
        let row = py * WIDTH;
        for px in x..x_end {
            let bg = bb[row + px];
            bb[row + px] = lerp_bgrx(bg, fg_bgrx, alpha);
        }
    }
}

/// 1-pixel stroke at an explicit BGRX colour (no palette indirection).
/// Used by the N.12 design language for cyan accent rims.
pub fn stroke_rect_rgb(x: usize, y: usize, w: usize, h: usize, bgrx: u32) {
    if w == 0 || h == 0 || x >= WIDTH || y >= HEIGHT {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let mut bb = BACKBUFFER.lock();
    // Top
    if y < HEIGHT {
        let row = y * WIDTH;
        for px in x..x_end { bb[row + px] = bgrx; }
    }
    // Bottom
    if y_end > 0 && y_end - 1 < HEIGHT && y_end - 1 > y {
        let row = (y_end - 1) * WIDTH;
        for px in x..x_end { bb[row + px] = bgrx; }
    }
    // Sides
    for py in (y + 1)..(y_end.saturating_sub(1)) {
        let row = py * WIDTH;
        if x < WIDTH { bb[row + x] = bgrx; }
        if x_end > 0 && x_end - 1 < WIDTH { bb[row + x_end - 1] = bgrx; }
    }
}

/// N.12 — rounded-corner translucent rectangle.  Corners are
/// quarter-circles with per-pixel coverage (Bresenham-style AA),
/// so the panel reads as a modern Win11 / iOS card.  `radius`
/// caps at min(w, h) / 2.
pub fn fill_round_rect_alpha(
    x: usize, y: usize, w: usize, h: usize,
    radius: usize, fg_bgrx: u32, alpha: u8,
) {
    if w == 0 || h == 0 || alpha == 0 { return; }
    let r = radius.min(w / 2).min(h / 2);
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let mut bb = BACKBUFFER.lock();
    for py in y..y_end {
        if py >= HEIGHT { break; }
        let row = py * WIDTH;
        // Compute the per-row x range, with quarter-circle clipping
        // near the rounded corners.
        let dy = if py < y + r {
            (y + r - py) as i32
        } else if py >= y + h - r {
            (py + r + 1 - (y + h)) as i32
        } else {
            0
        };
        // Inscribed circle test: dx² + dy² <= r²
        // For each x in [x, x+r): allowed iff (r - 1 - dx)² + dy² < r²
        // Then [x+r .. x_end-r] is fully inside.
        let mut px = x;
        while px < x_end {
            if px >= WIDTH { break; }
            // Per-pixel corner test + sub-pixel AA at the boundary
            let in_corner = (px < x + r && dy > 0) || (px >= x + w - r && dy > 0);
            let alpha_used = if in_corner {
                let dx_c = if px < x + r { (x + r - 1 - px) as i32 } else { (px + r - (x + w)) as i32 };
                let r2 = (r as i32 * r as i32) as f32;
                let d2 = (dx_c * dx_c + dy * dy) as f32;
                let cover = (r2 - d2) / (2.0 * r as f32);
                if cover <= 0.0 { px += 1; continue; }
                let cov_alpha = (alpha as f32 * cover.min(1.0)) as u8;
                cov_alpha
            } else {
                alpha
            };
            let bg = bb[row + px];
            bb[row + px] = lerp_bgrx(bg, fg_bgrx, alpha_used);
            px += 1;
        }
    }
}

/// N.12 — Wu's anti-aliased line.  Two-pixel-per-step plot with
/// coverage = (1 - frac) / frac, so diagonals don't stairstep.
/// Cheap: ~2× the cost of a Bresenham line, no division per step.
pub fn draw_line_aa_rgb(x0: i32, y0: i32, x1: i32, y1: i32, bgrx: u32) {
    let dx = (x1 - x0) as f32;
    let dy = (y1 - y0) as f32;
    let steep = dy.abs() > dx.abs();
    let (mut x0, mut y0, mut x1, mut y1, dx, dy) = if steep {
        (y0 as f32, x0 as f32, y1 as f32, x1 as f32, dy.abs(), dx)
    } else {
        (x0 as f32, y0 as f32, x1 as f32, y1 as f32, dx.abs(), dy)
    };
    if x0 > x1 {
        core::mem::swap(&mut x0, &mut x1);
        core::mem::swap(&mut y0, &mut y1);
    }
    let gradient = if dx == 0.0 { 1.0 } else { (y1 - y0) / (x1 - x0) };
    let mut intery = y0;
    let mut x = x0 as i32;
    let x_end = x1 as i32;
    while x <= x_end {
        let frac = intery - libm::floorf(intery);
        let y_int = intery as i32;
        if steep {
            blend_pixel_rgb(y_int as usize, x as usize, bgrx, ((1.0 - frac) * 255.0) as u8);
            blend_pixel_rgb((y_int + 1) as usize, x as usize, bgrx, (frac * 255.0) as u8);
        } else {
            blend_pixel_rgb(x as usize, y_int as usize, bgrx, ((1.0 - frac) * 255.0) as u8);
            blend_pixel_rgb(x as usize, (y_int + 1) as usize, bgrx, (frac * 255.0) as u8);
        }
        intery += gradient;
        x += 1;
    }
    let _ = dy;
}

// ── N.13 — PaintSession: batched lock for hot loops ──────────────
//
// The per-call `BACKBUFFER.lock()` pattern is fine for the cold paths
// (chrome, text headers, a handful of UI rectangles) but tanks the
// sphere + rope shaders which each issue 20k-50k single-pixel writes
// per frame.  Under QEMU TCG each spin-mutex acquire is ~200 cycles;
// 100k acquires × 200 cycles ≈ 20 M cycles per frame purely on
// uncontested locking.
//
// `PaintSession` hands the caller a single &mut into BACKBUFFER for
// the duration of a closure, plus a snapshot of the palette LUT
// (so palette-indexed paths don't have to re-lock per call).  All
// the same hot primitives are mirrored as inherent methods → the
// inner loops become a direct slice write.

/// A scoped, single-acquire access to the back-buffer.  Wrap any hot
/// per-pixel loop in `with_session(|s| { ... })` and call methods on
/// `s` instead of the top-level `k_fb::put_pixel_rgb` family — the
/// behaviour is identical but the throughput goes up by ~10× under TCG.
pub struct PaintSession<'a> {
    bb: &'a mut [u32],
    palette: [u32; 256],
}

impl<'a> PaintSession<'a> {
    /// Write a raw BGRX pixel (clipped).
    #[inline]
    pub fn put_rgb(&mut self, x: usize, y: usize, bgrx: u32) {
        if x < WIDTH && y < HEIGHT {
            self.bb[y * WIDTH + x] = bgrx;
        }
    }

    /// Write a palette-indexed colour (clipped).
    #[inline]
    pub fn put_color(&mut self, x: usize, y: usize, c: Color) {
        if x < WIDTH && y < HEIGHT {
            self.bb[y * WIDTH + x] = self.palette[c.idx() as usize];
        }
    }

    /// Alpha-blend a single pixel over the existing back-buffer pixel.
    /// `alpha` is 0..=255 (0 = transparent, 255 = opaque).
    #[inline]
    pub fn blend(&mut self, x: usize, y: usize, fg: u32, alpha: u8) {
        if x >= WIDTH || y >= HEIGHT || alpha == 0 { return; }
        let off = y * WIDTH + x;
        let bg = self.bb[off];
        self.bb[off] = lerp_bgrx(bg, fg, alpha);
    }

    /// Saturating add — additive bloom / glow stamping.
    #[inline]
    pub fn add(&mut self, x: usize, y: usize, add_bgrx: u32) {
        if x >= WIDTH || y >= HEIGHT { return; }
        let off = y * WIDTH + x;
        let cur = self.bb[off];
        let sat = |sh: u32| -> u32 {
            let a = (cur >> sh) & 0xFF;
            let b = (add_bgrx >> sh) & 0xFF;
            (a + b).min(255)
        };
        self.bb[off] = (sat(16) << 16) | (sat(8) << 8) | sat(0);
    }

    /// Read a pixel back from the session-held buffer.
    #[inline]
    pub fn get_rgb(&self, x: usize, y: usize) -> u32 {
        if x >= WIDTH || y >= HEIGHT { return 0; }
        self.bb[y * WIDTH + x]
    }

    /// Resolve a `Color` to its BGRX without re-locking the palette LUT.
    #[inline]
    pub fn color_bgrx(&self, c: Color) -> u32 {
        self.palette[c.idx() as usize]
    }
}

/// Open a paint session.  Inside the closure, the back-buffer is
/// exclusively yours; per-pixel operations become a direct slice
/// write.  When the closure returns, the back-buffer lock is released
/// and the next external `k_fb::*` call works normally.
pub fn with_session<F: FnOnce(&mut PaintSession)>(f: F) {
    let mut bb = BACKBUFFER.lock();
    // Snapshot the palette LUT into the session so palette-indexed
    // writes don't re-acquire its mutex per pixel.
    let palette = *PALETTE_BGRX.lock();
    let mut sess = PaintSession { bb: &mut bb[..], palette };
    f(&mut sess);
}

/// N.12 — 1-iter box-blur of a back-buffer region in-place.
/// 3×3 kernel; used by glass panels to fake "frosted" backdrop.
/// Reads the existing scene, blurs into a stack buffer, copies back.
/// 24-bit per pixel; cost ~9 reads + 1 write per pixel.
pub fn box_blur_region(x: usize, y: usize, w: usize, h: usize) {
    if w == 0 || h == 0 || x >= WIDTH || y >= HEIGHT { return; }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let rw = x_end - x;
    let rh = y_end - y;
    if rw == 0 || rh == 0 { return; }

    // Stack scratch — caps at 200×60 = 12 KB.  Panels stay well under this.
    const MAX_W: usize = 200;
    const MAX_H: usize = 80;
    if rw > MAX_W || rh > MAX_H { return; }
    let mut scratch = [[0u32; MAX_W]; MAX_H];

    let bb = BACKBUFFER.lock();
    // Sample 3×3 around each pixel, average channels.
    for ry in 0..rh {
        for rx in 0..rw {
            let mut acc_r: u32 = 0;
            let mut acc_g: u32 = 0;
            let mut acc_b: u32 = 0;
            let mut n: u32 = 0;
            for dy in -1i32..=1 {
                let py = y as i32 + ry as i32 + dy;
                if py < 0 || py >= HEIGHT as i32 { continue; }
                for dx in -1i32..=1 {
                    let px = x as i32 + rx as i32 + dx;
                    if px < 0 || px >= WIDTH as i32 { continue; }
                    let v = bb[py as usize * WIDTH + px as usize];
                    acc_r += (v >> 16) & 0xFF;
                    acc_g += (v >> 8) & 0xFF;
                    acc_b += v & 0xFF;
                    n += 1;
                }
            }
            let r = (acc_r / n) & 0xFF;
            let g = (acc_g / n) & 0xFF;
            let b = (acc_b / n) & 0xFF;
            scratch[ry][rx] = (r << 16) | (g << 8) | b;
        }
    }
    drop(bb);
    let mut bb = BACKBUFFER.lock();
    for ry in 0..rh {
        for rx in 0..rw {
            bb[(y + ry) * WIDTH + (x + rx)] = scratch[ry][rx];
        }
    }
}

/// Set a single pixel by Color enum.  Writes the back-buffer; the
/// real LFB is updated when `present()` runs.
pub fn put_pixel(x: usize, y: usize, color: Color) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    BACKBUFFER.lock()[y * WIDTH + x] = bgrx;
}

/// Read a single pixel's BGRX value from the back-buffer.  N.9 — the
/// M.3 bloom post-pass now operates in 24-bit colour space.
pub fn get_pixel_rgb(x: usize, y: usize) -> u32 {
    if x >= WIDTH || y >= HEIGHT {
        return 0;
    }
    BACKBUFFER.lock()[y * WIDTH + x]
}

/// Legacy luminance-approximation accessor.  Kept for callers that
/// only need a "rough brightness" probe; returns a 0..=7 bucket.
pub fn get_pixel_raw(x: usize, y: usize) -> u8 {
    let bgrx = get_pixel_rgb(x, y);
    let r = (bgrx >> 16) & 0xFF;
    let g = (bgrx >> 8) & 0xFF;
    let b = bgrx & 0xFF;
    let lum = (r * 30 + g * 59 + b * 11) / 100;
    (lum / 32).min(7) as u8
}

/// Set a single pixel by raw palette index.  Internal lookup goes
/// through PALETTE_BGRX so output is true colour.
pub fn put_pixel_raw(x: usize, y: usize, palette_idx: u8) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let bgrx = PALETTE_BGRX.lock()[palette_idx as usize];
    BACKBUFFER.lock()[y * WIDTH + x] = bgrx;
}

/// N.9 — direct 24-bit RGB write.  Used by the sphere shader so the
/// ACES-tonemapped HDR output isn't quantised through the 8-shade
/// palette ramp.  `bgrx` packs as `0x00_RR_GG_BB` (matches LFB).
pub fn put_pixel_rgb(x: usize, y: usize, bgrx: u32) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    BACKBUFFER.lock()[y * WIDTH + x] = bgrx;
}

/// N.9 — per-channel saturating add.  Used for additive bloom blending.
#[inline]
pub fn add_pixel_rgb(x: usize, y: usize, add_bgrx: u32) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let mut bb = BACKBUFFER.lock();
    let cur = bb[y * WIDTH + x];
    let sat = |sh: u32| -> u32 {
        let a = (cur >> sh) & 0xFF;
        let b = (add_bgrx >> sh) & 0xFF;
        (a + b).min(255)
    };
    bb[y * WIDTH + x] = (sat(16) << 16) | (sat(8) << 8) | sat(0);
}

/// Solid filled rectangle, clipped against the screen.
pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: Color) {
    if x >= WIDTH || y >= HEIGHT || w == 0 || h == 0 {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    let row_len = x_end - x;
    let mut bb = BACKBUFFER.lock();
    for py in y..y_end {
        let row_start = py * WIDTH + x;
        let row_slice = &mut bb[row_start..row_start + row_len];
        row_slice.fill(bgrx);
    }
}

/// N.10 — vertical-gradient filled rectangle for modern UI chrome.
pub fn fill_rect_gradient(x: usize, y: usize, w: usize, h: usize, top: u32, bottom: u32) {
    if x >= WIDTH || y >= HEIGHT || w == 0 || h == 0 {
        return;
    }
    let x_end = (x + w).min(WIDTH);
    let y_end = (y + h).min(HEIGHT);
    let h_active = y_end - y;
    let mut bb = BACKBUFFER.lock();
    for py in y..y_end {
        let t = (py - y) as u32 * 255 / h_active.max(1) as u32;
        let bgrx = lerp_bgrx(top, bottom, t as u8);
        let row_start = py * WIDTH + x;
        let row_slice = &mut bb[row_start..row_start + (x_end - x)];
        row_slice.fill(bgrx);
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

/// Phase N.x — flush the RAM back-buffer to the real framebuffer.
///
/// Called once per frame by `paint_frame` after all per-pixel writes
/// complete.  Two paths:
///
///  * **Mode 13h (1 bpp)** — straight `memcpy` of the 64 000-byte
///    back-buffer to VGA memory at 0xA0000.  Single REP MOVSB in
///    TCG; ~100 µs typical.
///
///  * **HD VBE (4 bpp)** — expand each palette index to BGRX, stamp
///    `SCALE × SCALE` block per logical pixel.  We build one scanline
///    of expanded BGRX in a stack buffer, then `copy_nonoverlapping`
///    it into the LFB SCALE times (once per native row).  This funnels
///    every MMIO write through REP MOVSD, which TCG fast-paths as a
///    bulk transfer per page instead of one softmmu callback per u32.
///    Frame budget: ~10–30 ms at SCALE=4 (1280×800), comfortable for
///    a 30+ fps paint loop in QEMU TCG.
pub fn present() {
    let Some(fb) = fb_ptr() else { return };
    let bpp = FB_BPP.load(Ordering::Relaxed);
    let bb = BACKBUFFER.lock();

    if bpp == 1 {
        // Mode 13h: pal-quantise each BGRX to the closest palette
        // index (BT.601 luminance bucket) and 1-byte-copy to VGA.
        // Lossy fallback; the shader has full 24-bit on HD path.
        let lum_table = PALETTE_LUM.lock();
        // Tiny inline kNN: pick the palette index whose stored
        // luminance is closest to ours.  Good enough for the legacy
        // fallback path; HD is the canonical output now.
        let mut row = [0u8; WIDTH];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let bgrx = bb[y * WIDTH + x];
                let r = (bgrx >> 16) & 0xFF;
                let g = (bgrx >> 8) & 0xFF;
                let b = bgrx & 0xFF;
                let lum = ((r * 30 + g * 59 + b * 11) / 100) as u8;
                // 8-shade ramp: just luminance/32, biased to slot 16
                // (cyan ramp).  Crude but workable for mode-13h fallback.
                row[x] = if lum < 4 { 0 } else { 16 + (lum / 32).min(7) };
                let _ = lum_table; // silence unused; future smarter match goes here
            }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    row.as_ptr(),
                    fb.add(y * WIDTH),
                    WIDTH,
                );
            }
        }
        return;
    }

    // HD path: scanline expand BGRX → BGRX (no palette indirection now)
    // + bulk REP MOVSD per native row.
    let scale = FB_SCALE.load(Ordering::Relaxed) as usize;
    let native_w = FB_NATIVE_W.load(Ordering::Relaxed) as usize;
    let fb32 = fb as *mut u32;

    let mut line_buf = [0u32; WIDTH * HD_SCALE as usize];
    let line_len = WIDTH * scale;

    for y in 0..HEIGHT {
        let bb_row = y * WIDTH;
        for x in 0..WIDTH {
            let bgrx = bb[bb_row + x];
            let base = x * scale;
            let lim = (base + scale).min(line_buf.len());
            for slot in &mut line_buf[base..lim] {
                *slot = bgrx;
            }
        }
        let src = line_buf.as_ptr();
        for dy in 0..scale {
            let row_off = (y * scale + dy) * native_w;
            unsafe { core::ptr::copy_nonoverlapping(src, fb32.add(row_off), line_len); }
        }
    }
}

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
/// Default OFF — N.9: a 2026 OS should stay still when the user
/// isn't touching it.  Press F1 to start the orbit demo.
pub static CAMERA_AUTO_ROTATE: AtomicBool = AtomicBool::new(false);

/// Cumulative camera bias in fixed-point milli-radians.
/// `i32::MAX ≈ 2.1×10⁶ rad` — comfortably more headroom than the
/// camera ever needs.  Painter divides by 1000.0 to convert to f32
/// radians before applying.
pub static CAMERA_YAW_BIAS_MRAD: AtomicI32 = AtomicI32::new(0);
pub static CAMERA_PITCH_BIAS_MRAD: AtomicI32 = AtomicI32::new(0);

/// Camera orbit radius in millimetre-equivalent fixed-point.  The
/// actual default is computed by `hypervisor::frame_all_nodes()` at
/// boot from the grid bounding sphere; this constant is just a
/// safety fallback used before that call completes.
pub static CAMERA_RADIUS_MM: AtomicI32 = AtomicI32::new(4800);

/// N.13 — Unity "F-key" frame-all request flag.  k-ps2 sets this
/// when the user presses F6; the painter checks each frame and, if
/// set, runs `hypervisor::frame_all_nodes()` to recompute the orbit
/// radius from the current graph's bounding sphere then clears the
/// flag.  Provides a runtime callback channel between the input
/// driver and the painter without a static fn pointer.
pub static CAMERA_FRAME_REQUEST: AtomicBool = AtomicBool::new(false);

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

/// N.10 — 2× scaled glyph for headlines.  Each font pixel becomes a
/// 2×2 block; total cell is 16×16.  Gives a "premium" weight to the
/// header brand text instead of the bitmap-y 8×8 default.
pub fn draw_glyph_2x(x: usize, y: usize, ch: char, color: Color) {
    if x + 2 * GLYPH_W > WIDTH || y + 2 * GLYPH_H > HEIGHT {
        return;
    }
    let glyph = if (ch as u32) < 128 {
        &font8x8::legacy::BASIC_LEGACY[ch as usize]
    } else {
        return;
    };
    let bgrx = PALETTE_BGRX.lock()[color.idx() as usize];
    let mut bb = BACKBUFFER.lock();
    for row in 0..GLYPH_H {
        let bits = glyph[row];
        let py0 = y + row * 2;
        let py1 = py0 + 1;
        for col in 0..GLYPH_W {
            if bits & (1 << col) == 0 { continue; }
            let px0 = x + col * 2;
            let px1 = px0 + 1;
            bb[py0 * WIDTH + px0] = bgrx;
            bb[py0 * WIDTH + px1] = bgrx;
            bb[py1 * WIDTH + px0] = bgrx;
            bb[py1 * WIDTH + px1] = bgrx;
        }
    }
}

/// 2× draw_text — string at 16×16 per glyph.  Width: 16 px per char.
pub fn draw_text_2x(x: usize, y: usize, text: &str, color: Color) {
    let mut cx = x;
    for ch in text.chars() {
        if cx + 2 * GLYPH_W > WIDTH {
            break;
        }
        draw_glyph_2x(cx, y, ch, color);
        cx += 2 * GLYPH_W;
    }
}

/// N.17 — N× scaled BIOS 8×8 glyph with alpha blend, BGRX colour.
/// Used by the boot splash to draw chunky pixel-style "GOS" letters
/// that fade in / out.  Each font pixel becomes a `scale × scale`
/// block, blended into the back-buffer with the supplied alpha.
pub fn draw_glyph_pixel_scaled(
    x: usize, y: usize, ch: char,
    scale: usize, bgrx: u32, alpha: u8,
) {
    if scale == 0 || (ch as u32) >= 128 { return; }
    let glyph = &font8x8::legacy::BASIC_LEGACY[ch as usize];
    let mut bb = BACKBUFFER.lock();
    for row in 0..GLYPH_H {
        let bits = glyph[row];
        for col in 0..GLYPH_W {
            if bits & (1 << col) == 0 { continue; }
            let base_x = x + col * scale;
            let base_y = y + row * scale;
            for dy in 0..scale {
                let py = base_y + dy;
                if py >= HEIGHT { break; }
                for dx in 0..scale {
                    let px = base_x + dx;
                    if px >= WIDTH { break; }
                    let off = py * WIDTH + px;
                    bb[off] = lerp_bgrx(bb[off], bgrx, alpha);
                }
            }
        }
    }
}

/// N.17 — N× scaled string render, alpha-blended.  Wraps
/// `draw_glyph_pixel_scaled` for each char in the input.
pub fn draw_text_pixel_scaled(
    x: usize, y: usize, text: &str,
    scale: usize, bgrx: u32, alpha: u8,
) {
    let cell_w = GLYPH_W * scale;
    let mut cx = x;
    for ch in text.chars() {
        if cx + cell_w > WIDTH { break; }
        draw_glyph_pixel_scaled(cx, y, ch, scale, bgrx, alpha);
        cx += cell_w;
    }
}

// ── N.11 — anti-aliased TTF-baked text rendering ──────────────────
//
// Replaces the 8×8 pixel font with Noto Sans SC / Cascadia Code
// alpha-blended into the 24-bit back-buffer.  Per-pixel alpha →
// sub-pixel-quality edges, no jaggies, no "BIOS POST" retro feel.
//
// One look-up per character, one alpha-blend per glyph pixel.  At
// 14 px and 80 cols × 25 rows of text this is ~30 000 blends per
// frame — negligible vs the sphere shader's ~200 000 pixels.

/// Draw an ASCII string with full per-pixel alpha blending, using
/// the requested font atlas.  Out-of-range codepoints advance the
/// cursor by `cell_w` without rendering — caller pre-checks for
/// codepoints beyond `char_first..char_first + char_count` if it
/// cares about CJK fallback.
///
/// The colour is the foreground; the background is read from the
/// back-buffer in-place so the text composites cleanly over the
/// underlying gradient / scene without needing a pre-filled box.
pub fn draw_text_aa(
    x: usize,
    y: usize,
    text: &str,
    color: Color,
    font: &k_assets::FontAtlas,
) {
    let fg = PALETTE_BGRX.lock()[color.idx() as usize];
    draw_text_aa_rgb(x, y, text, fg, font);
}

/// 24-bit RGB variant — caller supplies the BGRX directly so they
/// can use accent colours outside the palette (e.g. the cyan rim
/// `#4ad0ff` used by the N.10 chrome).
pub fn draw_text_aa_rgb(
    x: usize,
    y: usize,
    text: &str,
    fg_bgrx: u32,
    font: &k_assets::FontAtlas,
) {
    if y >= HEIGHT { return; }
    let cell_w = font.cell_w as usize;
    let cell_h = font.cell_h as usize;
    let atlas_w = font.atlas_w as usize;
    let mut cx = x;
    let mut bb = BACKBUFFER.lock();
    for ch in text.chars() {
        // Map char to glyph rect; non-ASCII codepoints get a blank cell
        // for now (CJK extension is a follow-up phase).
        let cp = if (ch as u32) <= 0x7F { ch as u8 } else { b'?' };
        let Some(rect) = font.glyph(cp) else { cx += cell_w; continue; };
        if cx + cell_w > WIDTH { break; }

        for py in 0..cell_h {
            let target_y = y + py;
            if target_y >= HEIGHT { break; }
            let row_base = target_y * WIDTH + cx;
            let atlas_row = (rect.atlas_y as usize + py) * atlas_w + rect.atlas_x as usize;
            for px in 0..cell_w {
                let alpha = font.alpha[atlas_row + px];
                if alpha == 0 { continue; }
                let dst = row_base + px;
                let bg = bb[dst];
                bb[dst] = lerp_bgrx(bg, fg_bgrx, alpha);
            }
        }
        cx += cell_w;
    }
}

/// Convenience — UI default (Noto Sans SC 14 px) into a Color.
pub fn draw_text_ui(x: usize, y: usize, text: &str, color: Color) {
    draw_text_aa(x, y, text, color, &k_assets::FONT_UI_14);
}

/// Convenience — header / brand text (Noto Sans SC 18 px).
pub fn draw_text_ui_lg(x: usize, y: usize, text: &str, color: Color) {
    draw_text_aa(x, y, text, color, &k_assets::FONT_UI_18);
}

/// Convenience — monospace (Cascadia Code 13 px) for command line.
pub fn draw_text_mono(x: usize, y: usize, text: &str, color: Color) {
    draw_text_aa(x, y, text, color, &k_assets::FONT_MONO_13);
}

/// String pixel width helper — caller uses this to centre / right-align
/// text in the new variable-width font.
pub fn text_width(text: &str, font: &k_assets::FontAtlas) -> usize {
    text.chars().count() * font.cell_w as usize
}

/// N.10 — 1-pixel offset drop-shadow text.  Renders the background-
/// colored ghost first, then the foreground.  Makes labels legible
/// against any backdrop without a full text box.
pub fn draw_text_shadowed(x: usize, y: usize, text: &str, fg: Color, shadow: Color) {
    if x > 0 && y > 0 {
        draw_text(x - 1, y - 1, text, shadow);
        draw_text(x + 1, y + 1, text, shadow);
    }
    draw_text(x, y, text, fg);
}
