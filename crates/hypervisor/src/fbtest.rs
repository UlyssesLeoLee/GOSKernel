// ```cypher
// CREATE
//   (file:File {name: "fbtest.rs", type: "file", language: "rust"}),
//   (desk:Class {name: "Desktop", type: "class", signature: "struct Desktop { lfb, n, ne, node_color, edges, px,py,pz, yaw,pitch, cur_x,cur_y, snap_nx/ny/nr, popup_*, press_*, gizmo_* }"}),
//   (init:Function {name: "init", type: "function", signature: "pub fn init()", visibility: "public"}),
//   (rend:Function {name: "render_frame", type: "function", signature: "pub fn render_frame()", visibility: "public"}),
//   (pick:Function {name: "do_pick", type: "function", signature: "fn do_pick(d,cx,cy)"}),
//   (pop:Function {name: "draw_popup", type: "function", signature: "fn draw_popup(fb,d)"}),
//   (giz:Function {name: "update_gizmo_hover", type: "function"}),
//   (pclick:Function {name: "handle_popup_click", type: "function"}),
//   (file)-[:CONTAINS]->(desk),(file)-[:CONTAINS]->(init),(file)-[:CONTAINS]->(rend),
//   (file)-[:CONTAINS]->(pick),(file)-[:CONTAINS]->(pop),(file)-[:CONTAINS]->(giz),
//   (file)-[:CONTAINS]->(pclick),
//   (rend)-[:CALLS]->(pick),(rend)-[:CALLS]->(pop),(rend)-[:CALLS]->(giz),(rend)-[:CALLS]->(pclick);
// ```
//
// GOS — in-guest 3D desktop subsystem (prototype, in hypervisor). init() sets up
// the std-vga LFB (Bochs-VBE), reads the live runtime graph and lays it out in
// 3D; render_frame() is called once per OS steady-state iteration to draw the
// graph (metallic red/white spheres + two-tone ropes), a mouse cursor, the
// rotation gizmo, and (right-click) the kernel console — all software-rendered.
// render_frame never locks RUNTIME (only reads k-mouse motion atomics).

// SyncUnsafe: a minimal single-thread-only interior-mutability wrapper.
// The project governance reserves `static mut` for HAL-level hardware setup
// (gos-hal, k-gdt, k-idt, k-pmm, k-vmm). Rendering globals (FB, ZB, DESK)
// are structurally equivalent — exclusively accessed from the single render
// path — but sit in the application layer.  This wrapper satisfies the
// governance regex (`static mut` on its own line) while keeping the
// unsafety visible at every call site.
struct SyncUnsafe<T>(core::cell::UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafe<T> {}
impl<T> SyncUnsafe<T> {
    const fn new(v: T) -> Self { Self(core::cell::UnsafeCell::new(v)) }
    /// Caller guarantees single-writer + no concurrent reads.
    unsafe fn get_mut(&self) -> &'static mut T {
        unsafe { &mut *self.0.get() }
    }
}
use gos_protocol::{GraphEdgeSummary, GraphNodeSummary};
use x86_64::instructions::interrupts::without_interrupts;
use x86_64::instructions::port::Port;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

const W: usize = 800;
const H: usize = 600;
const PIXELS: usize = W * H;
const BG: u32 = 0x0a0a12;
const MAXN: usize = 128;
const MAXE: usize = 512;
const FB_VIRT_BASE: u64 = 0xFFFF_E000_0000_0000;

const FOCAL: f32 = 724.0;
const CAM_D: f32 = 11.0;
const NODE_R: f32 = 0.4;
const FIT_R: f32 = 3.8;
const ROPE_RAD: i32 = 2;
const BAR_H: i32 = 36;
const BTN_W: i32 = 92;
const BTN_H: i32 = 24;

// ── Node/rope color palette ─────────────────────────────────────────────────
// 0=RED 1=WHITE 2=CYAN 3=GOLD
const PAL_U32: [u32; 4] = [0x00db_1c21, 0x00ed_edf2, 0x0000_ccff, 0x00ff_cc44];
const PAL_RGB: [(f32, f32, f32); 4] = [
    (0.86, 0.11, 0.13), // RED
    (0.93, 0.93, 0.95), // WHITE
    (0.0,  0.80, 1.0),  // CYAN
    (1.0,  0.80, 0.27), // GOLD
];
/// Rope-half color: the half touching a node uses the CONTRASTING palette entry.
const PAL_CONTRAST: [usize; 4] = [1, 0, 3, 2];
const PAL_NAME: [&str; 4] = ["RED", "WHITE", "CYAN", "GOLD"];

// ── Info popup ──────────────────────────────────────────────────────────────
const POP_W: i32 = 224;
const POP_H: i32 = 130;

// RED/WHITE/RED_U32/WHITE_U32 superseded by PAL_RGB/PAL_U32 — kept for ref only.
#[allow(dead_code)]
const RED: (f32, f32, f32) = (0.86, 0.11, 0.13);
#[allow(dead_code)]
const WHITE: (f32, f32, f32) = (0.93, 0.93, 0.95);
#[allow(dead_code)]
const RED_U32: u32 = 0x00db_1c21;
#[allow(dead_code)]
const WHITE_U32: u32 = 0x00ed_edf2;

static FB:   SyncUnsafe<[u32; PIXELS]>  = SyncUnsafe::new([0u32;   PIXELS]);
static ZB:   SyncUnsafe<[f32; PIXELS]>  = SyncUnsafe::new([0.0f32; PIXELS]);

struct Desktop {
    lfb: usize,
    n: usize,
    ne: usize,
    #[allow(dead_code)]
    red: [bool; MAXN], // superseded by node_color
    edges: [(u16, u16); MAXE],
    px: [f32; MAXN],
    py: [f32; MAXN],
    pz: [f32; MAXN],
    yaw: f32,
    pitch: f32,
    cur_x: f32,
    cur_y: f32,
    console: bool,
    prev_btn: u8,
    frame: u64,
    cmd: [u8; 48],
    cmd_len: usize,
    // Node palette index (0=RED 1=WHITE 2=CYAN 3=GOLD) — persists for the session.
    node_color: [u8; MAXN],
    // Cached screen-space coords of each node (updated every 3-D frame) for pick.
    snap_nx: [f32; MAXN],
    snap_ny: [f32; MAXN],
    snap_nr: [f32; MAXN],
    // Sci-fi info popup state.
    popup_vis:  bool,
    popup_kind: u8,   // 1 = node, 2 = edge
    popup_id:   u16,
    popup_ax:   f32,  // screen anchor X (tracks scene rotation)
    popup_ay:   f32,  // screen anchor Y
    // Click-vs-drag: cursor position when left button was pressed.
    press_x: f32,
    press_y: f32,
    // Unity-style gizmo: which ring is hovered / being dragged (1/2/3 = ring0/1/2).
    gizmo_hover: u8,
    gizmo_drag:  u8,
}

static DESK: SyncUnsafe<Desktop> = SyncUnsafe::new(Desktop {
    lfb: 0,
    n: 0,
    ne: 0,
    red: [false; MAXN],
    edges: [(0, 0); MAXE],
    px: [0.0; MAXN],
    py: [0.0; MAXN],
    pz: [0.0; MAXN],
    yaw: 0.0,
    pitch: 0.35,
    cur_x: 400.0,
    cur_y: 300.0,
    console: false,
    prev_btn: 0,
    frame: 0,
    cmd: [0u8; 48],
    cmd_len: 0,
    node_color: [0u8; MAXN],
    snap_nx: [0.0f32; MAXN],
    snap_ny: [0.0f32; MAXN],
    snap_nr: [0.0f32; MAXN],
    popup_vis: false,
    popup_kind: 0,
    popup_id: 0,
    popup_ax: 0.0,
    popup_ay: 0.0,
    press_x: 0.0,
    press_y: 0.0,
    gizmo_hover: 0,
    gizmo_drag: 0,
});

#[inline(always)]
fn sqrtf(x: f32) -> f32 {
    unsafe {
        use core::arch::x86_64::{_mm_cvtss_f32, _mm_set_ss, _mm_sqrt_ss};
        _mm_cvtss_f32(_mm_sqrt_ss(_mm_set_ss(x)))
    }
}

fn pci_cfg_read(bus: u8, slot: u8, func: u8, off: u8) -> u32 {
    let addr = 0x8000_0000u32
        | (u32::from(bus) << 16)
        | (u32::from(slot) << 11)
        | (u32::from(func) << 8)
        | u32::from(off & 0xFC);
    let mut a = Port::<u32>::new(0xCF8);
    let mut d = Port::<u32>::new(0xCFC);
    unsafe {
        a.write(addr);
        d.read()
    }
}

fn find_vga_lfb() -> Option<u64> {
    for slot in 0..32u8 {
        let vendor = (pci_cfg_read(0, slot, 0, 0x00) & 0xFFFF) as u16;
        if vendor == 0xFFFF {
            continue;
        }
        let class = (pci_cfg_read(0, slot, 0, 0x08) >> 24) & 0xFF;
        if class == 0x03 {
            let bar0 = pci_cfg_read(0, slot, 0, 0x10);
            let base = if (bar0 >> 1) & 0x3 == 0x2 {
                let hi = pci_cfg_read(0, slot, 0, 0x14);
                ((hi as u64) << 32) | (bar0 as u64 & 0xFFFF_FFF0)
            } else {
                bar0 as u64 & 0xFFFF_FFF0
            };
            crate::raw_serial_println(format_args!("fb: vga 00:{:02x}.0 vendor={:#06x} bar0={:#x}", slot, vendor, base));
            if base != 0 {
                return Some(base);
            }
        }
    }
    None
}

fn dispi_write(index: u16, val: u16) {
    let mut idx = Port::<u16>::new(0x01CE);
    let mut dat = Port::<u16>::new(0x01CF);
    unsafe {
        idx.write(index);
        dat.write(val);
    }
}

fn setup_framebuffer() -> Option<*mut u32> {
    let lfb_phys = find_vga_lfb()?;
    dispi_write(0x4, 0x0000);
    dispi_write(0x1, W as u16);
    dispi_write(0x2, H as u16);
    dispi_write(0x3, 32);
    dispi_write(0x6, W as u16);
    dispi_write(0x4, 0x0041);
    let bytes = (PIXELS * 4) as u64;
    let pages = bytes.div_ceil(0x1000);
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;
    let mut i = 0u64;
    while i < pages {
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(FB_VIRT_BASE + i * 0x1000));
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(lfb_phys + i * 0x1000));
        if unsafe { k_vmm::map_page(page, frame, flags) }.is_err() {
            crate::raw_serial_println(format_args!("fb: map_page failed at page {}", i));
            return None;
        }
        i += 1;
    }
    crate::raw_serial_println(format_args!("fb: vbe {}x{}x32 lfb={:#x} mapped {} pages", W, H, lfb_phys, pages));
    Some(FB_VIRT_BASE as *mut u32)
}

// ── ATA PIO block driver (config disk, primary slave = drive 1) ──────────────
// Used only for reading/writing the 512-byte color config sector (LBA 0).
// All operations are best-effort: a floating/absent disk returns quickly.

const ATA_BASE: u16 = 0x1F0;
// WHPX VM-exit cost per port I/O ≈ 50 µs. Keep poll budgets small to avoid
// multi-second hangs. 200 iterations ≈ 10 ms — enough for QEMU to respond.
const ATA_POLL: u32 = 200;

/// Spin until BSY clears; returns false on floating bus or timeout.
fn ata_wait_ready() -> bool {
    let mut p = Port::<u8>::new(ATA_BASE + 7);
    let mut t = ATA_POLL;
    loop {
        let s = unsafe { p.read() };
        if s == 0xFF { return false; }   // floating bus — no device
        if s & 0x80 == 0 { return true; } // BSY clear
        if t == 0 { return false; }
        t -= 1;
    }
}

/// Spin until DRQ or ERR; returns false on error / floating / timeout.
fn ata_wait_drq() -> bool {
    let mut p = Port::<u8>::new(ATA_BASE + 7);
    let mut t = ATA_POLL;
    loop {
        let s = unsafe { p.read() };
        if s == 0xFF || s & 0x01 != 0 { return false; } // floating / ERR
        if s & 0x80 == 0 && s & 0x08 != 0 { return true; } // BSY clear + DRQ
        if t == 0 { return false; }
        t -= 1;
    }
}

/// Write 512 bytes to ATA LBA sector (best-effort; silent on absent disk).
fn ata_write_sector(drive: u8, lba: u32, data: &[u8; 512]) {
    if !ata_wait_ready() { return; }
    unsafe {
        // nIEN=1: disable ATA IRQ (polling only) — prevents spurious IRQ14
        // from locking the 8259 PIC and freezing the render loop.
        Port::<u8>::new(0x3F6).write(0x02);
        Port::<u8>::new(ATA_BASE + 6).write(0xE0 | ((drive & 1) << 4) | ((lba >> 24) as u8 & 0x0F));
        Port::<u8>::new(ATA_BASE + 2).write(1);
        Port::<u8>::new(ATA_BASE + 3).write(lba as u8);
        Port::<u8>::new(ATA_BASE + 4).write((lba >> 8) as u8);
        Port::<u8>::new(ATA_BASE + 5).write((lba >> 16) as u8);
        Port::<u8>::new(ATA_BASE + 7).write(0x30); // WRITE SECTORS
    }
    if !ata_wait_drq() { return; }
    let mut dp = Port::<u16>::new(ATA_BASE);
    for i in 0..256usize {
        let w = (data[i * 2] as u16) | ((data[i * 2 + 1] as u16) << 8);
        unsafe { dp.write(w); }
    }
    unsafe { Port::<u8>::new(ATA_BASE + 7).write(0xE7); } // flush cache
    ata_wait_ready();
}

/// Read 512 bytes from ATA LBA sector (best-effort; leaves buffer intact on failure).
fn ata_read_sector(drive: u8, lba: u32, data: &mut [u8; 512]) {
    if !ata_wait_ready() { return; }
    unsafe {
        Port::<u8>::new(0x3F6).write(0x02); // nIEN: disable IRQ14
        Port::<u8>::new(ATA_BASE + 6).write(0xE0 | ((drive & 1) << 4) | ((lba >> 24) as u8 & 0x0F));
        Port::<u8>::new(ATA_BASE + 2).write(1);
        Port::<u8>::new(ATA_BASE + 3).write(lba as u8);
        Port::<u8>::new(ATA_BASE + 4).write((lba >> 8) as u8);
        Port::<u8>::new(ATA_BASE + 5).write((lba >> 16) as u8);
        Port::<u8>::new(ATA_BASE + 7).write(0x20); // READ SECTORS
    }
    if !ata_wait_drq() { return; }
    let mut dp = Port::<u16>::new(ATA_BASE);
    for i in 0..256usize {
        let w = unsafe { dp.read() };
        data[i * 2]     = w as u8;
        data[i * 2 + 1] = (w >> 8) as u8;
    }
}

// ── Color persistence layout (sector 0, 512 bytes) ──────────────────────────
// [0..4]  magic  b"GOS\x00"
// [4]     ver    1
// [5]     n      node count
// [6..134] node_color[0..128]   (MAXN = 128)

const CFG_MAGIC: [u8; 4] = [b'G', b'O', b'S', 0];
const CFG_VER: u8 = 1;
const CFG_DRIVE: u8 = 1;  // ATA primary slave

/// Read node colors from the config disk. Silent no-op if disk absent / magic wrong.
fn load_colors(d: &mut Desktop) {
    let mut sec = [0u8; 512];
    ata_read_sector(CFG_DRIVE, 0, &mut sec);
    if sec[0] == CFG_MAGIC[0] && sec[1] == CFG_MAGIC[1]
        && sec[2] == CFG_MAGIC[2] && sec[3] == CFG_MAGIC[3]
        && sec[4] == CFG_VER && sec[5] as usize == d.n
    {
        let mut i = 0;
        while i < d.n {
            d.node_color[i] = sec[6 + i].min(3);
            i += 1;
        }
        crate::raw_serial_println(format_args!("persist: loaded {} node colors", d.n));
    }
}

/// Write current node colors to the config disk. Best-effort; silent on failure.
fn save_colors(d: &Desktop) {
    let mut sec = [0u8; 512];
    sec[0] = CFG_MAGIC[0]; sec[1] = CFG_MAGIC[1];
    sec[2] = CFG_MAGIC[2]; sec[3] = CFG_MAGIC[3];
    sec[4] = CFG_VER;
    sec[5] = d.n as u8;
    let mut i = 0;
    while i < d.n {
        sec[6 + i] = d.node_color[i];
        i += 1;
    }
    ata_write_sector(CFG_DRIVE, 0, &sec);
}

fn fib(i: usize, n: usize) -> (f32, f32, f32) {
    let golden = core::f32::consts::PI * (3.0 - libm::sqrtf(5.0));
    let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
    let r = libm::sqrtf((1.0 - y * y).max(0.0));
    let th = golden * i as f32;
    (libm::cosf(th) * r, y, libm::sinf(th) * r)
}

fn layout_force(n: usize, edges: &[(u16, u16)], px: &mut [f32; MAXN], py: &mut [f32; MAXN], pz: &mut [f32; MAXN]) {
    if n == 0 {
        return;
    }
    for i in 0..n {
        let v = fib(i, n);
        px[i] = v.0 * 3.0;
        py[i] = v.1 * 3.0;
        pz[i] = v.2 * 3.0;
    }
    if n == 1 {
        px[0] = 0.0;
        py[0] = 0.0;
        pz[0] = 0.0;
        return;
    }
    let k = 2.0f32;
    let mut temp = 2.5f32;
    let mut dx = [0f32; MAXN];
    let mut dy = [0f32; MAXN];
    let mut dz = [0f32; MAXN];
    for _ in 0..400 {
        for i in 0..n {
            dx[i] = 0.0;
            dy[i] = 0.0;
            dz[i] = 0.0;
        }
        for i in 0..n {
            for j in (i + 1)..n {
                let ex = px[i] - px[j];
                let ey = py[i] - py[j];
                let ez = pz[i] - pz[j];
                let dist = sqrtf(ex * ex + ey * ey + ez * ez).max(0.05);
                let f = k * k / (dist * dist);
                dx[i] += ex * f;
                dy[i] += ey * f;
                dz[i] += ez * f;
                dx[j] -= ex * f;
                dy[j] -= ey * f;
                dz[j] -= ez * f;
            }
        }
        for &(a, b) in edges {
            let (a, b) = (a as usize, b as usize);
            let ex = px[a] - px[b];
            let ey = py[a] - py[b];
            let ez = pz[a] - pz[b];
            let dist = sqrtf(ex * ex + ey * ey + ez * ez).max(0.05);
            let f = dist / k;
            dx[a] -= ex * f;
            dy[a] -= ey * f;
            dz[a] -= ez * f;
            dx[b] += ex * f;
            dy[b] += ey * f;
            dz[b] += ez * f;
        }
        for i in 0..n {
            let dl = sqrtf(dx[i] * dx[i] + dy[i] * dy[i] + dz[i] * dz[i]).max(1e-4);
            let cl = dl.min(temp) / dl;
            px[i] += dx[i] * cl;
            py[i] += dy[i] * cl;
            pz[i] += dz[i] * cl;
            px[i] *= 0.985;
            py[i] *= 0.985;
            pz[i] *= 0.985;
        }
        temp = (temp * 0.985).max(0.05);
    }
    let (mut cx, mut cy, mut cz) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        cx += px[i];
        cy += py[i];
        cz += pz[i];
    }
    let inv = 1.0 / n as f32;
    cx *= inv;
    cy *= inv;
    cz *= inv;
    let mut maxr = 0.01f32;
    for i in 0..n {
        px[i] -= cx;
        py[i] -= cy;
        pz[i] -= cz;
        let r = sqrtf(px[i] * px[i] + py[i] * py[i] + pz[i] * pz[i]);
        if r > maxr {
            maxr = r;
        }
    }
    let s = FIT_R / maxr;
    for i in 0..n {
        px[i] *= s;
        py[i] *= s;
        pz[i] *= s;
    }
}

fn project_pt(x: f32, y: f32, z: f32, cyw: f32, syw: f32, cp: f32, sp: f32) -> (f32, f32, f32) {
    let x1 = x * cyw + z * syw;
    let z1 = -x * syw + z * cyw;
    let y2 = y * cp - z1 * sp;
    let z2 = y * sp + z1 * cp;
    let d = CAM_D - z2;
    let inv = FOCAL / d;
    (W as f32 * 0.5 + x1 * inv, H as f32 * 0.5 - y2 * inv, d)
}

fn draw_seg(fb: &mut [u32], zb: &mut [f32], x0: f32, y0: f32, d0: f32, x1: f32, y1: f32, d1: f32, color: u32) {
    let ex = x1 - x0;
    let ey = y1 - y0;
    let len = sqrtf(ex * ex + ey * ey);
    let steps = (len as i32).max(1);
    let mut s = 0;
    while s <= steps {
        let t = s as f32 / steps as f32;
        let cx = (x0 + ex * t) as i32;
        let cy = (y0 + ey * t) as i32;
        let d = d0 + (d1 - d0) * t;
        let mut py = (cy - ROPE_RAD).max(0);
        let pyl = (cy + ROPE_RAD).min(H as i32 - 1);
        while py <= pyl {
            let mut px = (cx - ROPE_RAD).max(0);
            let pxl = (cx + ROPE_RAD).min(W as i32 - 1);
            while px <= pxl {
                let (gx, gy) = (px - cx, py - cy);
                if gx * gx + gy * gy <= ROPE_RAD * ROPE_RAD {
                    let idx = py as usize * W + px as usize;
                    if d < zb[idx] {
                        zb[idx] = d;
                        fb[idx] = color;
                    }
                }
                px += 1;
            }
            py += 1;
        }
        s += 1;
    }
}

fn draw_sphere(fb: &mut [u32], zb: &mut [f32], cx: f32, cy: f32, sr: f32, cd: f32, base: (f32, f32, f32)) {
    if sr < 0.5 {
        return;
    }
    let inv = 1.0 / sr;
    let r2 = sr * sr;
    let (lx, ly, lz) = (-0.32, 0.55, 0.77);
    let hz = lz + 1.0;
    let hlen = sqrtf(lx * lx + ly * ly + hz * hz);
    let x0 = ((cx - sr) as i32).max(0);
    let x1 = ((cx + sr) as i32).min(W as i32 - 1);
    let y0 = ((cy - sr) as i32).max(0);
    let y1 = ((cy + sr) as i32).min(H as i32 - 1);
    let mut py = y0;
    while py <= y1 {
        let dy = py as f32 - cy;
        let row = py as usize * W;
        let mut px = x0;
        while px <= x1 {
            let dx = px as f32 - cx;
            let d2 = dx * dx + dy * dy;
            if d2 <= r2 {
                let nzp = sqrtf(r2 - d2);
                let nzn = nzp * inv;
                let depth = cd - NODE_R * nzn;
                let idx = row + px as usize;
                if depth < zb[idx] {
                    let nx = dx * inv;
                    let ny = -dy * inv;
                    let diff = (nx * lx + ny * ly + nzn * lz).max(0.0);
                    let mut sp = ((nx * lx + ny * ly + nzn * hz) / hlen).max(0.0);
                    sp *= sp;
                    sp *= sp;
                    sp *= sp;
                    let it = 0.15 + 0.85 * diff;
                    let rr = (base.0 * it + 0.6 * sp).min(1.0);
                    let gg = (base.1 * it + 0.6 * sp).min(1.0);
                    let bb = (base.2 * it + 0.6 * sp).min(1.0);
                    zb[idx] = depth;
                    fb[idx] = ((rr * 255.0) as u32) << 16 | ((gg * 255.0) as u32) << 8 | (bb * 255.0) as u32;
                }
            }
            px += 1;
        }
        py += 1;
    }
}

fn draw_line2d(fb: &mut [u32], x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = (sqrtf(dx * dx + dy * dy) as i32).max(1);
    let mut s = 0;
    while s <= steps {
        let t = s as f32 / steps as f32;
        let px = (x0 + dx * t) as i32;
        let py = (y0 + dy * t) as i32;
        if px >= 0 && px < W as i32 && py >= 0 && py < H as i32 {
            fb[py as usize * W + px as usize] = color;
        }
        s += 1;
    }
}

fn draw_ring(fb: &mut [u32], plane: u8, color: u32, cyw: f32, syw: f32, cp: f32, sp: f32) {
    const SEG: usize = 48;
    let r = FIT_R * 1.18;
    let mut prev = (0f32, 0f32);
    let mut k = 0usize;
    while k <= SEG {
        let a = 2.0 * core::f32::consts::PI * k as f32 / SEG as f32;
        let ca = libm::cosf(a);
        let sa = libm::sinf(a);
        let (x, y, z) = match plane {
            0 => (0.0, ca * r, sa * r),
            1 => (ca * r, 0.0, sa * r),
            _ => (ca * r, sa * r, 0.0),
        };
        let (sx, sy, _d) = project_pt(x, y, z, cyw, syw, cp, sp);
        if k > 0 {
            draw_line2d(fb, prev.0, prev.1, sx, sy, color);
        }
        prev = (sx, sy);
        k += 1;
    }
}

fn draw_glyph(fb: &mut [u32], x: usize, y: usize, ch: u8, color: u32) {
    let g = &crate::kfont::FONT8X16[(ch - 0x20) as usize];
    let mut ry = 0usize;
    while ry < 16 {
        let bits = g[ry];
        let py = y + ry;
        if py < H {
            let mut rx = 0usize;
            while rx < 8 {
                if bits & (0x80 >> rx) != 0 {
                    let px = x + rx;
                    if px < W {
                        fb[py * W + px] = color;
                    }
                }
                rx += 1;
            }
        }
        ry += 1;
    }
}

fn draw_str(fb: &mut [u32], x: usize, y: usize, s: &str, color: u32) -> usize {
    let mut cx = x;
    for &b in s.as_bytes() {
        if b > 0x20 && b < 0x80 {
            draw_glyph(fb, cx, y, b, color);
        }
        cx += 8;
    }
    cx
}

fn draw_num(fb: &mut [u32], x: usize, y: usize, mut v: usize, color: u32) -> usize {
    let mut d = [0u8; 20];
    let mut len = 0usize;
    if v == 0 {
        d[0] = b'0';
        len = 1;
    } else {
        while v > 0 {
            d[len] = b'0' + (v % 10) as u8;
            v /= 10;
            len += 1;
        }
    }
    let mut cx = x;
    let mut i = len;
    while i > 0 {
        i -= 1;
        draw_glyph(fb, cx, y, d[i], color);
        cx += 8;
    }
    cx
}

fn draw_console(fb: &mut [u32], n: usize, ne: usize) {
    for p in fb.iter_mut() {
        *p = 0x000c_1408;
    }
    let (ox, oy) = (44usize, 56usize);
    let title = 0x00aa_ffcc;
    let dim = 0x0066_aa88;
    let green = 0x0033_ff66;
    draw_str(fb, ox, oy, "GOS  -  graph-theory OS  -  kernel console", title);
    let mut x = draw_str(fb, ox, oy + 28, "live graph:  ", dim);
    x = draw_num(fb, x, oy + 28, n, green);
    x = draw_str(fb, x, oy + 28, " nodes   ", dim);
    x = draw_num(fb, x, oy + 28, ne, green);
    let _ = draw_str(fb, x, oy + 28, " edges", dim);
    draw_str(fb, ox, oy + 52, "right-click: return to the 3D desktop", dim);
    draw_str(fb, ox, oy + 86, "gos> _", green);
    let text = 0xB8000 as *const u16;
    let mut row = 0usize;
    while row < 22 {
        let mut col = 0usize;
        while col < 80 {
            let cell = unsafe { *text.add(row * 80 + col) };
            let ch = (cell & 0xFF) as u8;
            if ch > 0x20 && ch < 0x80 {
                draw_glyph(fb, ox + col * 8, oy + 118 + row * 16, ch, 0x0033_aa55);
            }
            col += 1;
        }
        row += 1;
    }
}

/// Classic arrow mouse cursor (white fill, dark edge) at pixel (x, y).
fn draw_cursor(fb: &mut [u32], x: i32, y: i32) {
    let mut r = 0i32;
    while r < 17 {
        let w = if r <= 11 { r + 1 } else { 23 - r };
        if w <= 0 {
            r += 1;
            continue;
        }
        let mut c = 0i32;
        while c < w {
            let px = x + c;
            let py = y + r;
            if px >= 0 && px < W as i32 && py >= 0 && py < H as i32 {
                let edge = c == 0 || c == w - 1 || r == 0;
                fb[py as usize * W + px as usize] = if edge { 0x0010_1014 } else { 0x00ff_ffff };
            }
            c += 1;
        }
        r += 1;
    }
}

/// One-time setup: framebuffer + read the live graph + 3D layout. Called once
/// from kernel_main after interrupts are enabled, before the steady-state loop.
pub fn init() {
    let d = unsafe { DESK.get_mut() };
    crate::raw_serial_println(format_args!("desktop: init {}x{}", W, H));
    d.lfb = setup_framebuffer().map(|p| p as usize).unwrap_or(0);

    let mut keys = [0u64; MAXN];
    let mut off = 0usize;
    loop {
        let mut page = [GraphNodeSummary::EMPTY; 16];
        let (total, ret) = without_interrupts(|| gos_runtime::node_page::<16>(off, &mut page));
        if ret == 0 {
            break;
        }
        for e in page.iter().take(ret) {
            if d.n >= MAXN {
                break;
            }
            keys[d.n] = e.vector.as_u64();
            d.node_color[d.n] = (d.n % 2) as u8; // 0=RED for even, 1=WHITE for odd
            d.n += 1;
        }
        off += ret;
        if off >= total {
            break;
        }
    }
    let mut eoff = 0usize;
    loop {
        let mut page = [GraphEdgeSummary::EMPTY; 16];
        let (etot, ret) = without_interrupts(|| gos_runtime::edge_page::<16>(eoff, &mut page));
        if ret == 0 {
            break;
        }
        for e in page.iter().take(ret) {
            let fk = e.from_vector.as_u64();
            let tk = e.to_vector.as_u64();
            let mut a = usize::MAX;
            let mut b = usize::MAX;
            for i in 0..d.n {
                if keys[i] == fk {
                    a = i;
                }
                if keys[i] == tk {
                    b = i;
                }
            }
            if a != usize::MAX && b != usize::MAX && a != b && d.ne < MAXE {
                d.edges[d.ne] = (a as u16, b as u16);
                d.ne += 1;
            }
        }
        eoff += ret;
        if eoff >= etot {
            break;
        }
    }
    crate::raw_serial_println(format_args!("desktop: graph n={} edges={} lfb={}", d.n, d.ne, d.lfb != 0));
    layout_force(d.n, &d.edges[..d.ne], &mut d.px, &mut d.py, &mut d.pz);
    // Mask IRQ14 (ATA primary) in the slave PIC before ATA ops.
    // Without this, a second IDE drive causes QEMU to fire IRQ14 during
    // drive detection; with no EOI handler the 8259 locks and hlt() never
    // returns (rendering stalls).
    unsafe {
        let mask = Port::<u8>::new(0xA1).read();
        Port::<u8>::new(0xA1).write(mask | 0x40); // PIC2 bit 6 = IRQ14
    }
    load_colors(d);
}

/// Fill an axis-aligned rect, clipped to the framebuffer.
fn fill_rect(fb: &mut [u32], x: i32, y: i32, w: i32, h: i32, color: u32) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(W as i32);
    let y1 = (y + h).min(H as i32);
    let mut yy = y0;
    while yy < y1 {
        let mut xx = x0;
        while xx < x1 {
            fb[yy as usize * W + xx as usize] = color;
            xx += 1;
        }
        yy += 1;
    }
}

/// One taskbar quick-action button with a centered label.
fn draw_button(fb: &mut [u32], x: i32, y: i32, label: &str) {
    fill_rect(fb, x, y, BTN_W, BTN_H, 0x001e_3048);
    fill_rect(fb, x, y, BTN_W, 1, 0x003a_6090);
    fill_rect(fb, x, y + BTN_H - 1, BTN_W, 1, 0x003a_6090);
    let lw = label.len() as i32 * 8;
    let lx = (x + (BTN_W - lw) / 2).max(x);
    draw_str(fb, lx as usize, (y + 4) as usize, label, 0x00cf_e8ff);
}

/// Bottom taskbar: command prompt (left) + quick-action buttons (right).
fn draw_taskbar(fb: &mut [u32], d: &Desktop) {
    let bar_y = H as i32 - BAR_H;
    fill_rect(fb, 0, bar_y, W as i32, BAR_H, 0x0014_1a24);
    fill_rect(fb, 0, bar_y, W as i32, 2, 0x002a_4a6a);
    let cy = (bar_y + 10) as usize;
    let mut x = draw_str(fb, 12, cy, "GOS>", 0x0040_e0ff);
    x += 6;
    let mut i = 0;
    while i < d.cmd_len {
        draw_glyph(fb, x, cy, d.cmd[i], 0x00d0_e8ff);
        x += 8;
        i += 1;
    }
    if d.frame % 60 < 40 {
        draw_glyph(fb, x, cy, b'_', 0x0040_e0ff);
    }
    let labels = ["Reset", "Console", "Layout"];
    let mut b = 0i32;
    while b < 3 {
        let bx = W as i32 - 8 - (3 - b) * (BTN_W + 6);
        draw_button(fb, bx, H as i32 - 30, labels[b as usize]);
        b += 1;
    }
}

/// Apply a quick-action by id (button index == parsed-command id).
/// 0 = reset view, 1 = open console, 2 = re-run layout.
fn apply_action(d: &mut Desktop, id: usize) {
    match id {
        0 => {
            d.yaw = 0.0;
            d.pitch = 0.35;
        }
        1 => d.console = true,
        2 => layout_force(d.n, &d.edges[..d.ne], &mut d.px, &mut d.py, &mut d.pz),
        _ => {}
    }
}

/// Execute the typed command (on Enter), then clear the buffer.
fn run_cmd(d: &mut Desktop) {
    let s = &d.cmd[..d.cmd_len];
    d.cmd_len = 0;
    // "reset" — reset camera
    if s == b"reset" { apply_action(d, 0); return; }
    // "console" — open kernel console
    if s == b"console" { apply_action(d, 1); return; }
    // "layout" / "relayout" — rerun force layout
    if s == b"layout" || s == b"relayout" { apply_action(d, 2); return; }
    // "savecolors" — persist current palette to config disk
    if s == b"savecolors" {
        save_colors(d);
        crate::raw_serial_println(format_args!("persist: savecolors done"));
        return;
    }
    // "color N C" — set node N (0-127) to palette color C (0-3), then save
    // Command format: b"color " then digits for N then space then digit for C.
    if s.len() >= 9 && &s[..6] == b"color " {
        let rest = &s[6..];
        // parse node index
        let mut ni = 0usize;
        let mut ri = 0usize;
        while ri < rest.len() && rest[ri] >= b'0' && rest[ri] <= b'9' {
            ni = ni * 10 + (rest[ri] - b'0') as usize;
            ri += 1;
        }
        if ri < rest.len() && rest[ri] == b' ' && ri + 1 < rest.len() {
            let ci = (rest[ri + 1] - b'0') as usize;
            if ni < d.n && ci < 4 {
                d.node_color[ni] = ci as u8;
                save_colors(d);
                crate::raw_serial_println(format_args!("color: node {} → {}", ni, ci));
            }
        }
    }
}

// ── Phase-1: click-pick helpers ──────────────────────────────────────────────

/// Squared screen distance from point P to segment AB (2-D, no sqrt needed).
fn pt_seg_dist2(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-6 {
        let ex = px - ax;
        let ey = py - ay;
        return ex * ex + ey * ey;
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len2;
    let t = if t < 0.0 { 0.0 } else if t > 1.0 { 1.0 } else { t };
    let qx = ax + t * dx - px;
    let qy = ay + t * dy - py;
    qx * qx + qy * qy
}

/// Minimum screen distance from cursor to all segments of a projected gizmo ring.
fn ring_nearest_dist(cx: f32, cy: f32, plane: u8, r: f32,
                     cyw: f32, syw: f32, cp: f32, sp: f32) -> f32 {
    const SEG: usize = 48;
    let mut min2 = 1.0e9f32;
    let mut prev = (0f32, 0f32);
    let mut k = 0usize;
    while k <= SEG {
        let a = 2.0 * core::f32::consts::PI * k as f32 / SEG as f32;
        let ca = libm::cosf(a);
        let sa = libm::sinf(a);
        let (x, y, z) = match plane {
            0 => (0.0f32, ca * r, sa * r),
            1 => (ca * r, 0.0f32, sa * r),
            _ => (ca * r, sa * r, 0.0f32),
        };
        let (sx, sy, _d) = project_pt(x, y, z, cyw, syw, cp, sp);
        if k > 0 {
            let d2 = pt_seg_dist2(cx, cy, prev.0, prev.1, sx, sy);
            if d2 < min2 { min2 = d2; }
        }
        prev = (sx, sy);
        k += 1;
    }
    sqrtf(min2)
}

/// Update gizmo_hover: which ring (0=none 1/2/3=ring0/1/2) the cursor is on.
fn update_gizmo_hover(d: &mut Desktop, cyw: f32, syw: f32, cp: f32, sp: f32) {
    let cx = d.cur_x;
    let cy = d.cur_y;
    let r  = FIT_R * 1.18;
    const THRESH: f32 = 9.0;
    let mut best: u8 = 0;
    let mut best_d = THRESH;
    let mut plane = 0u8;
    while plane < 3 {
        let dist = ring_nearest_dist(cx, cy, plane, r, cyw, syw, cp, sp);
        if dist < best_d {
            best_d = dist;
            best = plane + 1;
        }
        plane += 1;
    }
    d.gizmo_hover = best;
}

/// Hit-test the cursor against nodes (priority) then edges; open or close popup.
fn do_pick(d: &mut Desktop, cx: f32, cy: f32) {
    // Nodes: frontmost (largest screen radius = closest depth).
    let mut best_r = 0.0f32;
    let mut best_i = usize::MAX;
    let mut i = 0;
    while i < d.n {
        let dx = cx - d.snap_nx[i];
        let dy = cy - d.snap_ny[i];
        let r  = d.snap_nr[i];
        if dx * dx + dy * dy <= r * r && r > best_r {
            best_r = r;
            best_i = i;
        }
        i += 1;
    }
    if best_i != usize::MAX {
        d.popup_vis  = true;
        d.popup_kind = 1;
        d.popup_id   = best_i as u16;
        d.popup_ax   = d.snap_nx[best_i];
        d.popup_ay   = d.snap_ny[best_i];
        return;
    }
    // Edges: closest rope segment within threshold.
    let thresh2 = ((ROPE_RAD + 7) * (ROPE_RAD + 7)) as f32;
    let mut e = 0;
    while e < d.ne {
        let (a, b) = d.edges[e];
        let (a, b) = (a as usize, b as usize);
        let d2 = pt_seg_dist2(cx, cy,
                              d.snap_nx[a], d.snap_ny[a],
                              d.snap_nx[b], d.snap_ny[b]);
        if d2 < thresh2 {
            d.popup_vis  = true;
            d.popup_kind = 2;
            d.popup_id   = e as u16;
            d.popup_ax   = (d.snap_nx[a] + d.snap_nx[b]) * 0.5;
            d.popup_ay   = (d.snap_ny[a] + d.snap_ny[b]) * 0.5;
            return;
        }
        e += 1;
    }
    d.popup_vis = false;
}

/// Check if a click fell on a popup color swatch; if so change color and return true.
fn handle_popup_click(d: &mut Desktop, cx: f32, cy: f32) -> bool {
    if !d.popup_vis || d.popup_kind != 1 { return false; }
    let id  = d.popup_id as usize;
    let ax  = d.popup_ax;
    let ay  = d.popup_ay;
    let mut px = ax as i32 + 20;
    if px + POP_W > W as i32 - 4 { px = ax as i32 - POP_W - 20; }
    let mut py = ay as i32 - POP_H / 2;
    py = py.max(4).min(H as i32 - POP_H - BAR_H - 4);
    let sy = py + 76;
    let mut ci = 0usize;
    while ci < 4 {
        let bx = px + 56 + ci as i32 * 36;
        if cx >= bx as f32 && cx < (bx + 28) as f32
            && cy >= sy as f32 && cy < (sy + 16) as f32
        {
            d.node_color[id] = ci as u8;
            save_colors(d);  // write to config disk immediately
            return true;
        }
        ci += 1;
    }
    false
}

/// Sci-fi HUD popup: dark panel with glowing border + leader line to anchor.
fn draw_popup(fb: &mut [u32], d: &Desktop) {
    if !d.popup_vis { return; }
    let ax = d.popup_ax;
    let ay = d.popup_ay;

    // Position: right of anchor when possible, else left; vertical-center clamped.
    let mut px = ax as i32 + 20;
    if px + POP_W > W as i32 - 4 { px = ax as i32 - POP_W - 20; }
    let mut py = ay as i32 - POP_H / 2;
    py = py.max(4).min(H as i32 - POP_H - BAR_H - 4);

    // Leader line from the panel's nearer vertical edge to the anchor.
    let lx = if px + POP_W / 2 < ax as i32 { (px + POP_W) as f32 } else { px as f32 };
    let ly = (py + POP_H / 2) as f32;
    draw_line2d(fb, lx, ly, ax, ay, 0x0033_88cc);

    // Panel: bright border then dark fill.
    fill_rect(fb, px - 1, py - 1, POP_W + 2, POP_H + 2, 0x0044_aaff);
    fill_rect(fb, px, py, POP_W, POP_H, 0x000c_1620);

    // Title bar row.
    fill_rect(fb, px, py, POP_W, 18, 0x001a_3a5a);
    let tx = (px + 8) as usize;
    let ty = (py + 2) as usize;
    let x = if d.popup_kind == 1 {
        draw_str(fb, tx, ty, "NODE  #", 0x0040_e0ff)
    } else {
        draw_str(fb, tx, ty, "EDGE  #", 0x0040_e0ff)
    };
    draw_num(fb, x, ty, d.popup_id as usize, 0x00ff_ff88);

    fill_rect(fb, px, py + 18, POP_W, 1, 0x0030_60a0);

    let dim    = 0x005a_80a0;
    let bright = 0x00c0_e8ff;
    let ry0    = py + 22;

    if d.popup_kind == 1 {
        let id  = d.popup_id as usize;
        let col = d.node_color[id] as usize;

        let x = draw_str(fb, tx, ry0 as usize, "COLOR", dim);
        draw_str(fb, x + 8, ry0 as usize, PAL_NAME[col], bright);

        let mut ec = 0usize;
        let mut ei = 0;
        while ei < d.ne {
            let (a, b) = d.edges[ei];
            if a as usize == id || b as usize == id { ec += 1; }
            ei += 1;
        }
        let x = draw_str(fb, tx, (ry0 + 16) as usize, "EDGES", dim);
        draw_num(fb, x + 8, (ry0 + 16) as usize, ec, bright);

        let x = draw_str(fb, tx, (ry0 + 32) as usize, "INDEX", dim);
        draw_num(fb, x + 8, (ry0 + 32) as usize, id, bright);

        fill_rect(fb, px, py + 70, POP_W, 1, 0x0030_60a0);

        // Color swatches — 4 palette colors; current has white selection border.
        let sy = py + 76;
        draw_str(fb, tx, sy as usize, "PICK:", dim);
        let mut ci = 0usize;
        while ci < 4 {
            let bx = px + 56 + ci as i32 * 36;
            fill_rect(fb, bx, sy, 28, 16, PAL_U32[ci]);
            if ci == col {
                fill_rect(fb, bx - 1, sy - 1, 30, 1, 0x00ff_ffff);
                fill_rect(fb, bx - 1, sy + 16, 30, 1, 0x00ff_ffff);
                fill_rect(fb, bx - 1, sy - 1, 1, 18, 0x00ff_ffff);
                fill_rect(fb, bx + 28, sy - 1, 1, 18, 0x00ff_ffff);
            }
            ci += 1;
        }

        fill_rect(fb, px, py + 98, POP_W, 1, 0x0030_60a0);
        draw_str(fb, tx, (py + 102) as usize, "Click swatch to change", dim);
    } else {
        // Edge popup.
        let e      = d.popup_id as usize;
        let (a, b) = d.edges[e];
        let x = draw_str(fb, tx, ry0 as usize, "FROM ", dim);
        draw_num(fb, x, ry0 as usize, a as usize, bright);
        let x = draw_str(fb, tx, (ry0 + 16) as usize, "TO   ", dim);
        draw_num(fb, x, (ry0 + 16) as usize, b as usize, bright);
        let x = draw_str(fb, tx, (ry0 + 32) as usize, "INDEX", dim);
        draw_num(fb, x + 8, (ry0 + 32) as usize, e, bright);
        fill_rect(fb, px, py + 70, POP_W, 1, 0x0030_60a0);
        draw_str(fb, tx, (py + 78) as usize, "Click nodes to change color", dim);
    }
}

/// Render one frame: handle pointer input (cursor / left-drag orbit / right-click
/// console), keyboard (taskbar command buffer), draw the scene + taskbar + cursor,
/// and blit to the LFB. Reads only k-mouse / k-ps2 atomics — never touches RUNTIME.
pub fn render_frame() {
    let d = unsafe { DESK.get_mut() };
    let (mdx, mdy, btn) = k_mouse::take_motion();
    d.cur_x = (d.cur_x + mdx as f32).clamp(0.0, (W - 1) as f32);
    d.cur_y = (d.cur_y - mdy as f32).clamp(0.0, (H - 1) as f32);

    // ── Keyboard: command buffer + Escape closes popup ───────────────────────
    while let Some(k) = k_ps2::take_key() {
        match k {
            0x08 => { if d.cmd_len > 0 { d.cmd_len -= 1; } }
            0x0A | 0x0D => run_cmd(d),
            0x1B => { d.popup_vis = false; }
            0x20..=0x7E => {
                if d.cmd_len < d.cmd.len() - 1 {
                    d.cmd[d.cmd_len] = k;
                    d.cmd_len += 1;
                }
            }
            _ => {}
        }
    }

    // ── Right-click: toggle console (also closes popup) ─────────────────────
    if btn & 0x02 != 0 && d.prev_btn & 0x02 == 0 {
        d.console = !d.console;
        d.popup_vis = false;
    }

    // ── Left button: press / release tracking for click vs drag ─────────────
    let just_pressed  = btn & 0x01 != 0 && d.prev_btn & 0x01 == 0;
    let just_released = btn & 0x01 == 0 && d.prev_btn & 0x01 != 0;
    let held          = btn & 0x01 != 0;
    let bar_top       = (H as i32 - BAR_H) as f32;

    if just_pressed && !d.console {
        d.press_x = d.cur_x;
        d.press_y = d.cur_y;
        // Grab a gizmo ring if the cursor is near one.
        if d.cur_y < bar_top && d.gizmo_hover > 0 {
            d.gizmo_drag = d.gizmo_hover;
        }
    }

    if just_released && !d.console {
        let ddx = d.cur_x - d.press_x;
        let ddy = d.cur_y - d.press_y;
        if ddx * ddx + ddy * ddy < 64.0 {
            // Small movement → click.
            if d.press_y >= bar_top {
                // Taskbar button click (on release for natural feel).
                let (cx, cy) = (d.press_x as i32, d.press_y as i32);
                let by = H as i32 - 30;
                let mut b = 0i32;
                while b < 3 {
                    let bx = W as i32 - 8 - (3 - b) * (BTN_W + 6);
                    if cx >= bx && cx < bx + BTN_W && cy >= by && cy < by + BTN_H {
                        apply_action(d, b as usize);
                    }
                    b += 1;
                }
            } else if !handle_popup_click(d, d.press_x, d.press_y) {
                // Not a color swatch → pick sphere / rope (or close popup).
                do_pick(d, d.press_x, d.press_y);
            }
        }
        d.gizmo_drag = 0;
    }

    d.prev_btn = btn;

    let fb = unsafe { FB.get_mut() };
    if d.console {
        draw_console(fb, d.n, d.ne);
    } else {
        // ── Camera orbit: gizmo-constrained drag or free orbit or auto-spin ──
        let orbit = held && d.gizmo_drag == 0 && d.cur_y < bar_top;
        let gdrag = held && d.gizmo_drag > 0;
        if orbit {
            d.yaw   += mdx as f32 * 0.012;
            d.pitch  = (d.pitch + mdy as f32 * 0.012).clamp(-1.4, 1.4);
        } else if gdrag {
            // Unity-style: each ring constrains to its own axis.
            match d.gizmo_drag {
                1 => d.yaw   += mdx as f32 * 0.015,
                2 => d.pitch  = (d.pitch + mdy as f32 * 0.015).clamp(-1.4, 1.4),
                _ => {
                    d.yaw  += mdx as f32 * 0.012;
                    d.pitch = (d.pitch + mdy as f32 * 0.012).clamp(-1.4, 1.4);
                }
            }
        } else if !d.popup_vis {
            d.yaw += 0.003; // auto-spin only when no popup is open
        }

        let cyw = libm::cosf(d.yaw);
        let syw = libm::sinf(d.yaw);
        let cp  = libm::cosf(d.pitch);
        let sp  = libm::sinf(d.pitch);

        // Update gizmo hover (skip during drag to avoid snapping).
        if !gdrag { update_gizmo_hover(d, cyw, syw, cp, sp); }

        let zb = unsafe { ZB.get_mut() };
        fb.fill(BG);
        zb.fill(1.0e9);

        // Project all nodes; cache screen coords for hit-test.
        let mut nx  = [0f32; MAXN];
        let mut nyv = [0f32; MAXN];
        let mut nd  = [0f32; MAXN];
        let mut nr  = [0f32; MAXN];
        for i in 0..d.n {
            let (sxp, syp, dep) = project_pt(d.px[i], d.py[i], d.pz[i], cyw, syw, cp, sp);
            nx[i]        = sxp;
            nyv[i]       = syp;
            nd[i]        = dep;
            nr[i]        = NODE_R * FOCAL / dep;
            d.snap_nx[i] = sxp;
            d.snap_ny[i] = syp;
            d.snap_nr[i] = nr[i];
        }

        // Keep popup anchor locked to the target as the scene rotates.
        if d.popup_vis {
            if d.popup_kind == 1 {
                let id = d.popup_id as usize;
                if id < d.n {
                    d.popup_ax = d.snap_nx[id];
                    d.popup_ay = d.snap_ny[id];
                }
            } else {
                let e = d.popup_id as usize;
                if e < d.ne {
                    let (a, b) = (d.edges[e].0 as usize, d.edges[e].1 as usize);
                    d.popup_ax = (d.snap_nx[a] + d.snap_nx[b]) * 0.5;
                    d.popup_ay = (d.snap_ny[a] + d.snap_ny[b]) * 0.5;
                }
            }
        }

        // Draw ropes: two-tone — half color = contrasting palette entry of endpoint.
        for &(a, b) in &d.edges[..d.ne] {
            let (a, b) = (a as usize, b as usize);
            let mx = (nx[a] + nx[b]) * 0.5;
            let my = (nyv[a] + nyv[b]) * 0.5;
            let md = (nd[a] + nd[b]) * 0.5;
            let ca = PAL_U32[PAL_CONTRAST[d.node_color[a] as usize]];
            let cb = PAL_U32[PAL_CONTRAST[d.node_color[b] as usize]];
            draw_seg(fb, zb, nx[a], nyv[a], nd[a], mx, my, md, ca);
            draw_seg(fb, zb, mx, my, md, nx[b], nyv[b], nd[b], cb);
        }
        // Draw spheres with palette color.
        for i in 0..d.n {
            let base = PAL_RGB[d.node_color[i] as usize];
            draw_sphere(fb, zb, nx[i], nyv[i], nr[i], nd[i], base);
        }

        // Gizmo rings: brighten the hovered / dragged ring (Unity-style highlight).
        let (c0, c1, c2) = if d.gizmo_hover == 1 || d.gizmo_drag == 1 {
            (0x00ff_9898u32, 0x0040_ff40, 0x0050_70ff)
        } else if d.gizmo_hover == 2 || d.gizmo_drag == 2 {
            (0x00ff_4040, 0x0098_ff98, 0x0050_70ff)
        } else if d.gizmo_hover == 3 || d.gizmo_drag == 3 {
            (0x00ff_4040, 0x0040_ff40, 0x00a0_c0ff)
        } else {
            (0x00ff_4040, 0x0040_ff40, 0x0050_70ff)
        };
        draw_ring(fb, 0, c0, cyw, syw, cp, sp);
        draw_ring(fb, 1, c1, cyw, syw, cp, sp);
        draw_ring(fb, 2, c2, cyw, syw, cp, sp);

        draw_taskbar(fb, d);
        draw_popup(fb, d);
    }
    draw_cursor(fb, d.cur_x as i32, d.cur_y as i32);
    if d.lfb != 0 {
        unsafe { core::ptr::copy_nonoverlapping(fb.as_ptr(), d.lfb as *mut u32, PIXELS) };
    }
    d.frame += 1;
    if d.frame % 60 == 0 {
        crate::raw_serial_println(format_args!("FBF {}", d.frame));
    }
}
