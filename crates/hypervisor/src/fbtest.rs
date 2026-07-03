// ```cypher
// CREATE
//   (file:File {name: "fbtest.rs", type: "file", language: "rust"}),
//   (desk:Class {name: "Desktop", type: "class", signature: "struct Desktop { lfb, n, ne, node_color, edges, px,py,pz, yaw,pitch, cur_x,cur_y, snap_nx/ny/nr, popup_*, press_*, gizmo_*, corner_hover }"}),
//   (init:Function {name: "init", type: "function", signature: "pub fn init()", visibility: "public"}),
//   (rend:Function {name: "render_frame", type: "function", signature: "pub fn render_frame()", visibility: "public"}),
//   (pick:Function {name: "do_pick", type: "function", signature: "fn do_pick(d,cx,cy)"}),
//   (pop:Function {name: "draw_popup", type: "function", signature: "fn draw_popup(fb,d)"}),
//   (giz:Function {name: "update_gizmo_hover", type: "function"}),
//   (pclick:Function {name: "handle_popup_click", type: "function"}),
//   (cgiz:Function {name: "draw_corner_gizmo", type: "function", signature: "fn draw_corner_gizmo(fb,d,cyw,syw,cp,sp)"}),
//   (csnap:Function {name: "corner_snap", type: "function", signature: "fn corner_snap(d) -> bool"}),
//   (cmdr:Function {name: "draw_cmd_result", type: "function", signature: "fn draw_cmd_result(fb,d)"}),
//   (file)-[:CONTAINS]->(desk),(file)-[:CONTAINS]->(init),(file)-[:CONTAINS]->(rend),
//   (file)-[:CONTAINS]->(pick),(file)-[:CONTAINS]->(pop),(file)-[:CONTAINS]->(giz),
//   (file)-[:CONTAINS]->(pclick),(file)-[:CONTAINS]->(cgiz),(file)-[:CONTAINS]->(csnap),
//   (file)-[:CONTAINS]->(cmdr),
//   (rend)-[:CALLS]->(pick),(rend)-[:CALLS]->(pop),(rend)-[:CALLS]->(giz),(rend)-[:CALLS]->(pclick),
//   (rend)-[:CALLS]->(cgiz),(rend)-[:CALLS]->(csnap),(rend)-[:CALLS]->(cmdr);
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

// HD default (was 800x600 — "overall resolution too low" feedback). setup_framebuffer()
// already parametrizes the Bochs-VBE mode-set on W/H, so no mode-set changes are needed.
// Tradeoff (surfaced to and accepted by the user): FB+ZB are static arrays sized by
// PIXELS at compile time, and the software rasterizer's fill/blit cost scales linearly
// with pixel count — 1920x1080 is 4.3x the pixels of 800x600, so the ~18 FPS measured
// after the post.rs heartbeat-throttle fix is expected to drop to roughly ~8-13 FPS.
// 4K/8K were deliberately NOT wired in: at 17x/69x pixels they would reproduce (or
// worsen) the ~2.6 FPS stall that fix resolved, and their linear framebuffers (33MB/
// 133MB) exceed default QEMU std-vga VRAM (~16MB) — would need run-arg changes too.
const W: usize = 1920;
const H: usize = 1080;
const PIXELS: usize = W * H;
const BG: u32 = 0x0a0a12;
const MAXN: usize = 128;
const MAXE: usize = 512;
const FB_VIRT_BASE: u64 = 0xFFFF_E000_0000_0000;

const FOCAL: f32 = 724.0;
const CAM_D: f32 = 11.0;
const NODE_R: f32 = 0.4;
const FIT_R: f32 = 3.8;
/// World-space position of the standalone "minimized console" sphere — sits
/// above the graph cluster (which fits within FIT_R=3.8 of the origin), so
/// it reads as a separate companion object that orbits along with the scene
/// rather than blending into the node cluster.
const CONSOLE_BALL_POS: (f32, f32, f32) = (0.0, 5.4, 0.0);
/// Slightly larger than a graph node (NODE_R=0.4) so the minimized console
/// reads as a distinct, special object rather than just another graph node.
const CONSOLE_BALL_R: f32 = 0.55;
const ROPE_RAD: i32 = 2;
const BAR_H: i32 = 36;
const BTN_W: i32 = 92;
const BTN_H: i32 = 24;

/// Cursor-speed multiplier applied to raw PS/2 motion deltas before moving
/// the on-screen cursor (the orbit-drag camera below reads the raw deltas
/// directly and is independently tuned — not affected by this constant).
/// Each PS/2 packet reports a small per-event integer delta, so a 1:1
/// mapping made crossing the 800px-wide screen take a long physical sweep
/// ("too low" sensitivity). 3x triples on-screen travel per physical
/// movement while staying easy to land on small targets (buttons, swatches,
/// gizmo tips); retune here if it ever feels too twitchy or still sluggish.
const MOUSE_SENSITIVITY: f32 = 3.0;

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

// ── Command-result panel (collapsed strip above the taskbar input line;
// click toggles a taller detail view) ───────────────────────────────────────
const CMDR_W:     i32 = 300;
const CMDR_H_MIN: i32 = 24;
const CMDR_H_MAX: i32 = 72;

// ── Corner orientation gizmo (Unity-style, top-right) ───────────────────────
// Small 92px-diameter widget showing X/Y/Z axes rotating with the camera.
// Clicking an axis tip snaps the camera to that canonical view direction.
const CGIZ_CX:  i32 = (W as i32) - 54; // center X  (= 1866 for the 1920-wide HD screen)
const CGIZ_CY:  i32 = 54;              // center Y
const CGIZ_R:   f32 = 34.0;            // arm radius in pixels
const CGIZ_HIT: f32 = 11.0;            // pick radius around each tip

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
    /// Consecutive frames the right mouse button has been held. The console
    /// toggle fires only when this reaches 2, so a single-frame phantom button
    /// bit (residual PS/2 packet glitch) can't flip the console and flicker it.
    rbtn_hold: u8,
    frame: u64,
    cmd: [u8; 48],
    cmd_len: usize,
    // Result of the last executed command — drawn as a panel anchored just
    // above the taskbar's input line (see show_cmd_result/draw_cmd_result).
    // Starts collapsed (one line); click toggles cmd_result_open.
    cmd_result: [u8; 48],
    cmd_result_len: usize,
    cmd_result_ok: bool,
    cmd_result_vis: bool,
    cmd_result_open: bool,
    // Node palette index (0=RED 1=WHITE 2=CYAN 3=GOLD) — persists for the session.
    node_color: [u8; MAXN],
    // V2.57: live palette colors read from graph node attrs at init (indices match
    // PAL_U32; entries [0]/[1] sourced from theme.wabi/shoji node_attr_get).
    pal_u32: [u32; 4],
    // Cached screen-space coords of each node (updated every 3-D frame) for pick.
    snap_nx: [f32; MAXN],
    snap_ny: [f32; MAXN],
    snap_nr: [f32; MAXN],
    // Standalone "minimized console": when the console overlay isn't shown,
    // it collapses into a clickable red sphere floating at CONSOLE_BALL_POS
    // (see also draw_console / in_console_ball). Cached screen-space circle,
    // refreshed every 3-D frame — same one-frame-stale pick pattern as
    // snap_n* above.
    console_ball_sx: f32,
    console_ball_sy: f32,
    console_ball_sr: f32,
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
    // Corner gizmo: which axis tip (1=+X 2=+Y 3=+Z 4=-X 5=-Y 6=-Z) is hovered, 0=none.
    corner_hover: u8,
    // PIT tick at which auto-spin last advanced (ensures spin rate = PIT Hz, not render Hz).
    last_spin_tick: u64,
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
    rbtn_hold: 0,
    frame: 0,
    cmd: [0u8; 48],
    cmd_len: 0,
    cmd_result: [0u8; 48],
    cmd_result_len: 0,
    cmd_result_ok: false,
    cmd_result_vis: false,
    cmd_result_open: false,
    node_color: [0u8; MAXN],
    pal_u32: PAL_U32,
    snap_nx: [0.0f32; MAXN],
    snap_ny: [0.0f32; MAXN],
    snap_nr: [0.0f32; MAXN],
    console_ball_sx: 0.0,
    console_ball_sy: 0.0,
    console_ball_sr: 0.0,
    popup_vis: false,
    popup_kind: 0,
    popup_id: 0,
    popup_ax: 0.0,
    popup_ay: 0.0,
    press_x: 0.0,
    press_y: 0.0,
    gizmo_hover: 0,
    gizmo_drag: 0,
    corner_hover: 0,
    last_spin_tick: 0,
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

// ── Console close button (top-right corner) ────────────────────────────────
// The console view is a static full-screen overlay (no camera/scene to
// offset against), so its geometry is fixed regardless of scene state.
const CLOSE_BTN_W: i32 = 34;
const CLOSE_BTN_H: i32 = 26;
const CLOSE_BTN_X: i32 = W as i32 - 12 - CLOSE_BTN_W;
const CLOSE_BTN_Y: i32 = 12;

fn draw_close_button(fb: &mut [u32]) {
    fill_rect(fb, CLOSE_BTN_X, CLOSE_BTN_Y, CLOSE_BTN_W, CLOSE_BTN_H, 0x002a_1418);
    fill_rect(fb, CLOSE_BTN_X, CLOSE_BTN_Y, CLOSE_BTN_W, 1, 0x00aa_5060);
    fill_rect(fb, CLOSE_BTN_X, CLOSE_BTN_Y + CLOSE_BTN_H - 1, CLOSE_BTN_W, 1, 0x00aa_5060);
    draw_str(fb, (CLOSE_BTN_X + 11) as usize, (CLOSE_BTN_Y + 5) as usize, "X", 0x00ff_8080);
}

/// Hit-test the close button. `(x, y)` is screen-space (the console overlay
/// has no camera transform, so raw cursor coords are already button-local).
fn in_close_button(x: f32, y: f32) -> bool {
    let (xi, yi) = (x as i32, y as i32);
    xi >= CLOSE_BTN_X && xi < CLOSE_BTN_X + CLOSE_BTN_W
        && yi >= CLOSE_BTN_Y && yi < CLOSE_BTN_Y + CLOSE_BTN_H
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
    draw_close_button(fb);
    let mut x = draw_str(fb, ox, oy + 28, "live graph:  ", dim);
    x = draw_num(fb, x, oy + 28, n, green);
    x = draw_str(fb, x, oy + 28, " nodes   ", dim);
    x = draw_num(fb, x, oy + 28, ne, green);
    let _ = draw_str(fb, x, oy + 28, " edges", dim);
    draw_str(fb, ox, oy + 52, "ESC, the X (top-right), or right-click: return to the 3D desktop, where the console becomes a red sphere", dim);
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
    // V2.57: populate pal_u32[0..1] from graph node attrs (set at boot by shell_on_init).
    // Falls back silently to PAL_U32 constants if the attrs are absent.
    if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::THEME_WABI_NODE_VEC)) {
        d.pal_u32[0] = c;
    }
    if let Some(c) = without_interrupts(|| gos_runtime::node_attr_get(k_shell::THEME_SHOJI_NODE_VEC)) {
        d.pal_u32[1] = c;
    }
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

/// Execute the typed command (on Enter), stash the outcome for the
/// on-screen result panel (show_cmd_result/draw_cmd_result), then clear
/// the input buffer.
fn run_cmd(d: &mut Desktop) {
    // Copy into a local buffer first: this severs the borrow from `d.cmd`
    // so every branch below is free to mutate `d` (including stashing the
    // text back into `d.cmd_result`) while still holding the command text.
    let len = d.cmd_len.min(d.cmd.len());
    let mut buf = [0u8; 48];
    buf[..len].copy_from_slice(&d.cmd[..len]);
    let s = &buf[..len];
    d.cmd_len = 0;
    if s.is_empty() {
        return; // Enter on an empty line — nothing to report.
    }

    // "reset" / "console" / "layout"|"relayout" — quick-action shortcuts;
    // "savecolors" / "color N C" — persist / change the node palette.
    let ok = if s == b"reset" {
        apply_action(d, 0);
        true
    } else if s == b"console" {
        apply_action(d, 1);
        true
    } else if s == b"layout" || s == b"relayout" {
        apply_action(d, 2);
        true
    } else if s == b"savecolors" {
        save_colors(d);
        crate::raw_serial_println(format_args!("persist: savecolors done"));
        true
    } else if s.len() >= 9 && &s[..6] == b"color " {
        // Command format: b"color " then digits for N then space then digit for C.
        let rest = &s[6..];
        let mut ni = 0usize;
        let mut ri = 0usize;
        while ri < rest.len() && rest[ri] >= b'0' && rest[ri] <= b'9' {
            ni = ni * 10 + (rest[ri] - b'0') as usize;
            ri += 1;
        }
        let mut applied = false;
        if ri < rest.len() && rest[ri] == b' ' && ri + 1 < rest.len() {
            let ci = (rest[ri + 1] - b'0') as usize;
            if ni < d.n && ci < 4 {
                d.node_color[ni] = ci as u8;
                save_colors(d);
                crate::raw_serial_println(format_args!("color: node {} → {}", ni, ci));
                applied = true;
            }
        }
        applied
    } else {
        false
    };
    show_cmd_result(d, s, ok);
}

/// Stash the just-run command text + outcome as the result-panel content.
/// Always (re)opens collapsed — a fresh result starts as a one-line strip;
/// see draw_cmd_result / handle_cmd_result_click for the click-to-expand UI.
fn show_cmd_result(d: &mut Desktop, cmd: &[u8], ok: bool) {
    let n = cmd.len().min(d.cmd_result.len());
    d.cmd_result[..n].copy_from_slice(&cmd[..n]);
    d.cmd_result_len = n;
    d.cmd_result_ok = ok;
    d.cmd_result_vis = true;
    d.cmd_result_open = false;
}

/// Result-panel bounds (rx, ry, rh) for the current open/collapsed state —
/// shared by draw_cmd_result and handle_cmd_result_click so the click
/// hit-test always agrees with what is actually drawn on screen.
fn cmdr_rect(d: &Desktop) -> (i32, i32, i32) {
    let rh = if d.cmd_result_open { CMDR_H_MAX } else { CMDR_H_MIN };
    let rx = 8i32;
    let ry = (H as i32 - BAR_H) - rh - 6;
    (rx, ry, rh)
}

/// Result strip: last typed command + an OK/ERR tag, anchored just above
/// the taskbar's input line. Click toggles a taller detail view — the same
/// "click a HUD panel to inspect / collapse" idiom as the node/edge popup.
fn draw_cmd_result(fb: &mut [u32], d: &Desktop) {
    if !d.cmd_result_vis {
        return;
    }
    let (rx, ry, rh) = cmdr_rect(d);

    // Glowing border + dark fill — same idiom as draw_popup.
    fill_rect(fb, rx - 1, ry - 1, CMDR_W + 2, rh + 2, 0x0044_aaff);
    fill_rect(fb, rx, ry, CMDR_W, rh, 0x000c_1620);

    // Echo the command bytes directly via draw_glyph — same idiom as
    // draw_taskbar's prompt echo (the buffer only ever holds
    // validated-ASCII keyboard input, so no UTF-8 conversion is needed).
    let x = draw_str(fb, (rx + 8) as usize, (ry + 4) as usize, "> ", 0x0040_e0ff);
    let n = d.cmd_result_len.min(28);
    let mut tx = x;
    for &b in &d.cmd_result[..n] {
        draw_glyph(fb, tx, (ry + 4) as usize, b, 0x00d0_e8ff);
        tx += 8;
    }
    let (tag, tcol): (&str, u32) = if d.cmd_result_ok {
        ("OK", 0x0050_ff90)
    } else {
        ("ERR", 0x00ff_7060)
    };
    let tagx = (rx + CMDR_W - 8 - tag.len() as i32 * 8) as usize;
    draw_str(fb, tagx, (ry + 4) as usize, tag, tcol);

    if d.cmd_result_open {
        fill_rect(fb, rx, ry + 22, CMDR_W, 1, 0x0030_60a0);
        let hint: &str = if d.cmd_result_ok {
            "Command applied - state updated."
        } else {
            "Unrecognised command (ignored)."
        };
        draw_str(fb, (rx + 8) as usize, (ry + 30) as usize, hint, 0x005a_80a0);
        draw_str(fb, (rx + CMDR_W - 15 * 8) as usize, (ry + rh - 22) as usize, "click to close", 0x003a_5a78);
    }
}

/// True if (cx, cy) lands on the result panel — toggles open/collapsed as a
/// side effect (consumes the click whenever the visible panel is hit).
fn handle_cmd_result_click(d: &mut Desktop, cx: f32, cy: f32) -> bool {
    if !d.cmd_result_vis {
        return false;
    }
    let (rx, ry, rh) = cmdr_rect(d);
    if cx >= rx as f32 && cx < (rx + CMDR_W) as f32 && cy >= ry as f32 && cy < (ry + rh) as f32 {
        d.cmd_result_open = !d.cmd_result_open;
        true
    } else {
        false
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
            fill_rect(fb, bx, sy, 28, 16, d.pal_u32[ci]);
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

// ── Corner orientation gizmo helpers ────────────────────────────────────────

/// Filled circle, clipped to the framebuffer.
fn fill_circ(fb: &mut [u32], cx: i32, cy: i32, r: i32, color: u32) {
    let r2 = r * r;
    let mut dy = -r;
    while dy <= r {
        let mut dx = -r;
        while dx <= r {
            if dx * dx + dy * dy <= r2 {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && px < W as i32 && py >= 0 && py < H as i32 {
                    fb[py as usize * W + px as usize] = color;
                }
            }
            dx += 1;
        }
        dy += 1;
    }
}

/// Project a unit axis direction through the camera rotation.
/// axis: 0=+X 1=+Y 2=+Z 3=-X 4=-Y 5=-Z
/// Returns (screen_dx, screen_dy, z_depth) — multiply dx/dy by CGIZ_R to get pixels.
fn cgiz_proj(axis: u8, cyw: f32, syw: f32, cp: f32, sp: f32) -> (f32, f32, f32) {
    let (dx, dy, dz): (f32, f32, f32) = match axis {
        0 => ( 1.0,  0.0,  0.0),
        1 => ( 0.0,  1.0,  0.0),
        2 => ( 0.0,  0.0,  1.0),
        3 => (-1.0,  0.0,  0.0),
        4 => ( 0.0, -1.0,  0.0),
        _ => ( 0.0,  0.0, -1.0),
    };
    let x1 =  dx * cyw + dz * syw;
    let z1 = -dx * syw + dz * cyw;
    let y2 =  dy * cp  - z1 * sp;
    let z2 =  dy * sp  + z1 * cp;
    (x1 * CGIZ_R, -y2 * CGIZ_R, z2)
}

/// Draw the corner orientation gizmo: dark circle background + 3 colored axis arms
/// (X=red, Y=green, Z=blue) with dim negative stubs. Hovered tip enlarges and
/// turns white (Unity-style highlight). Back-to-front sorted each frame.
fn draw_corner_gizmo(fb: &mut [u32], d: &Desktop, cyw: f32, syw: f32, cp: f32, sp: f32) {
    let gcx = CGIZ_CX;
    let gcy = CGIZ_CY;
    // Subtle border ring then dark fill.
    fill_circ(fb, gcx, gcy, 47, 0x001e_3050);
    fill_circ(fb, gcx, gcy, 46, 0x0008_0d1a);

    // Compute projected screen offsets for all 6 axis tips.
    let mut tips = [(0f32, 0f32, 0f32); 6];
    for i in 0u8..6 {
        tips[i as usize] = cgiz_proj(i, cyw, syw, cp, sp);
    }

    // Sort back-to-front (larger z2 = further from camera = draw first).
    let mut ord = [0u8, 1, 2, 3, 4, 5];
    for i in 0..6usize {
        let mut mi = i;
        for j in (i + 1)..6 {
            if tips[ord[j] as usize].2 > tips[ord[mi] as usize].2 {
                mi = j;
            }
        }
        ord.swap(i, mi);
    }

    // Colors: +X=red +Y=green +Z=blue; dim versions for negative halves.
    let col: [u32; 6] = [
        0x00e8_4040, 0x0040_e040, 0x0050_90ff,  // +X +Y +Z
        0x0052_1818, 0x0018_5018, 0x0018_2848,  // -X -Y -Z
    ];
    let lbl = [b'X', b'Y', b'Z'];

    for &ai in &ord {
        let ai = ai as usize;
        let (sdx, sdy, _) = tips[ai];
        let tx = gcx + sdx as i32;
        let ty = gcy + sdy as i32;
        let hov = d.corner_hover == (ai as u8 + 1);
        let c = if hov { 0x00ff_ffff } else { col[ai] };
        draw_line2d(fb, gcx as f32, gcy as f32, tx as f32, ty as f32, c);
        let dot_r = if hov { 7i32 } else { 5i32 };
        fill_circ(fb, tx, ty, dot_r, c);
        if ai < 3 {
            // Label slightly above-left of tip (positive axes only).
            let lc = if hov { 0x00ff_ffff } else { 0x00d0_e8ff };
            let lx = (tx - 4).max(0) as usize;
            let ly = (ty - 8).max(0) as usize;
            if lx < W - 8 && ly < H - 16 {
                draw_glyph(fb, lx, ly, lbl[ai], lc);
            }
        }
    }
    fill_circ(fb, gcx, gcy, 3, 0x0090_a8c0);
}

/// Update d.corner_hover each frame: which axis tip (1..=6) the cursor is near.
fn update_corner_hover(d: &mut Desktop, cyw: f32, syw: f32, cp: f32, sp: f32) {
    let (cx, cy) = (d.cur_x, d.cur_y);
    let (gcx, gcy) = (CGIZ_CX as f32, CGIZ_CY as f32);
    let mut best = 0u8;
    let mut best_d = CGIZ_HIT;
    for i in 0u8..6 {
        let (sdx, sdy, _) = cgiz_proj(i, cyw, syw, cp, sp);
        let tx = gcx + sdx;
        let ty = gcy + sdy;
        let dist = sqrtf((cx - tx) * (cx - tx) + (cy - ty) * (cy - ty));
        if dist < best_d {
            best_d = dist;
            best = i + 1;
        }
    }
    d.corner_hover = best;
}

/// On left-click: if the press position landed on a corner gizmo axis tip, snap
/// the camera to that canonical view and return true (consuming the click).
fn corner_snap(d: &mut Desktop) -> bool {
    let cyw = libm::cosf(d.yaw);
    let syw = libm::sinf(d.yaw);
    let cp  = libm::cosf(d.pitch);
    let sp  = libm::sinf(d.pitch);
    let (cx, cy) = (d.press_x, d.press_y);
    let (gcx, gcy) = (CGIZ_CX as f32, CGIZ_CY as f32);
    for i in 0u8..6 {
        let (sdx, sdy, _) = cgiz_proj(i, cyw, syw, cp, sp);
        let tx = gcx + sdx;
        let ty = gcy + sdy;
        let dx = cx - tx;
        let dy = cy - ty;
        if dx * dx + dy * dy < CGIZ_HIT * CGIZ_HIT {
            const H_PI: f32 = core::f32::consts::PI * 0.5;
            let (ny, np): (f32, f32) = match i {
                0 => (-H_PI,  0.0 ),               // +X: look from +X side
                1 => ( 0.0,   1.35),               // +Y: top-down
                2 => ( 0.0,   0.0 ),               // +Z: front view
                3 => ( H_PI,  0.0 ),               // -X: look from -X side
                4 => ( 0.0,  -1.35),               // -Y: bottom-up
                _ => (core::f32::consts::PI, 0.0), // -Z: back view
            };
            d.yaw   = ny;
            d.pitch = np;
            d.popup_vis = false;
            return true;
        }
    }
    false
}

/// Return true if (x,y) is inside the corner gizmo background circle.
/// Used to swallow clicks that land in the gizmo area but miss every axis tip.
fn in_corner_gizmo(x: f32, y: f32) -> bool {
    let dx = x - CGIZ_CX as f32;
    let dy = y - CGIZ_CY as f32;
    dx * dx + dy * dy <= 47.0 * 47.0
}

/// Hit-test the minimized-console sphere using its screen-space projection
/// from the most recent 3-D frame (console_ball_s{x,y,r} — refreshed every
/// frame in render_frame's 3-D branch; one-frame-stale, same as snap_n* /
/// do_pick above — imperceptible at frame rate).
fn in_console_ball(d: &Desktop, x: f32, y: f32) -> bool {
    let dx = x - d.console_ball_sx;
    let dy = y - d.console_ball_sy;
    dx * dx + dy * dy <= d.console_ball_sr * d.console_ball_sr
}

/// Render one frame: handle pointer input (cursor / left-drag orbit / right-click
/// console), keyboard (taskbar command buffer), draw the scene + taskbar + cursor,
/// and blit to the LFB. Reads only k-mouse / k-ps2 atomics — never touches RUNTIME.
pub fn render_frame() {
    let d = unsafe { DESK.get_mut() };
    let (mdx, mdy, btn) = k_mouse::take_motion();
    d.cur_x = (d.cur_x + mdx as f32 * MOUSE_SENSITIVITY).clamp(0.0, (W - 1) as f32);
    d.cur_y = (d.cur_y - mdy as f32 * MOUSE_SENSITIVITY).clamp(0.0, (H - 1) as f32);

    // ── Keyboard: command buffer + Escape closes console / popup ─────────────
    while let Some(k) = k_ps2::take_key() {
        match k {
            0x08 => { if d.cmd_len > 0 { d.cmd_len -= 1; } }
            0x0A | 0x0D => run_cmd(d),
            // Context-sensitive: from the console, ESC returns to the 3-D
            // desktop (mirrors the X button / right-click — see render below
            // for how the console then reappears as a red sphere); otherwise
            // it just dismisses an open info popup. The two states are
            // mutually exclusive (entering the console always clears
            // popup_vis — see the right-click toggle below), so this can't
            // accidentally do both / neither.
            0x1B => { if d.console { d.console = false; } else { d.popup_vis = false; } }
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
    // Debounced: a genuine right-click is held across many frames, while a
    // phantom button bit from a (rare, residual) PS/2 packet glitch lasts a
    // single frame. Require the button to persist 2 consecutive frames and
    // toggle exactly once (on the rising count==2 edge), so noise can't flip
    // the console and flicker its green overlay.
    if btn & 0x02 != 0 {
        d.rbtn_hold = d.rbtn_hold.saturating_add(1);
        if d.rbtn_hold == 2 {
            d.console = !d.console;
            d.popup_vis = false;
        }
    } else {
        d.rbtn_hold = 0;
    }

    // ── Left button: press / release tracking for click vs drag ─────────────
    let just_pressed  = btn & 0x01 != 0 && d.prev_btn & 0x01 == 0;
    let just_released = btn & 0x01 == 0 && d.prev_btn & 0x01 != 0;
    let held          = btn & 0x01 != 0;
    let bar_top       = (H as i32 - BAR_H) as f32;

    if just_pressed {
        d.press_x = d.cur_x;
        d.press_y = d.cur_y;
        // Grab a gizmo ring if the cursor is near one (3-D view only — the
        // console overlay has no gizmo; press tracking above is still needed
        // there too, for the close-button click-vs-drag check below).
        if !d.console && d.cur_y < bar_top && d.gizmo_hover > 0 {
            d.gizmo_drag = d.gizmo_hover;
        }
    }

    // ── Console overlay: click the X button (top-right) to return to 3-D ────
    if just_released && d.console {
        let ddx = d.cur_x - d.press_x;
        let ddy = d.cur_y - d.press_y;
        if ddx * ddx + ddy * ddy < 64.0 && in_close_button(d.press_x, d.press_y) {
            // Same effect as ESC / right-click: back to the 3-D desktop,
            // where the console reappears as a red sphere (see render below).
            d.console = false;
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
            } else if handle_cmd_result_click(d, d.press_x, d.press_y) {
                // Result panel expanded/collapsed — click consumed.
            } else if corner_snap(d) {
                // Camera snapped to corner gizmo axis — click consumed.
            } else if in_corner_gizmo(d.press_x, d.press_y) {
                // Click landed in gizmo background but missed every tip — ignore.
            } else if in_console_ball(d, d.press_x, d.press_y) {
                // Minimized-console sphere — restores the console (mirrors
                // the X button / ESC / right-click, just in reverse).
                d.console = true;
                d.popup_vis = false;
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
            // Gate auto-spin on PIT ticks so the spin rate is frame-rate independent.
            // PIT runs at 120 Hz → spin = 0.003 × 120 ≈ 0.36 rad/s regardless of FPS.
            let ct = k_pit::get_ticks() as u64;
            if ct != d.last_spin_tick {
                d.last_spin_tick = ct;
                d.yaw += 0.003;
            }
        }

        let cyw = libm::cosf(d.yaw);
        let syw = libm::sinf(d.yaw);
        let cp  = libm::cosf(d.pitch);
        let sp  = libm::sinf(d.pitch);

        // Update gizmo hover (skip during drag to avoid snapping).
        if !gdrag { update_gizmo_hover(d, cyw, syw, cp, sp); }
        if !gdrag { update_corner_hover(d, cyw, syw, cp, sp); }

        let zb = unsafe { ZB.get_mut() };
        let t_fill = unsafe { core::arch::x86_64::_rdtsc() };
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
        let t_rope = unsafe { core::arch::x86_64::_rdtsc() };
        for &(a, b) in &d.edges[..d.ne] {
            let (a, b) = (a as usize, b as usize);
            let mx = (nx[a] + nx[b]) * 0.5;
            let my = (nyv[a] + nyv[b]) * 0.5;
            let md = (nd[a] + nd[b]) * 0.5;
            let ca = d.pal_u32[PAL_CONTRAST[d.node_color[a] as usize]];
            let cb = d.pal_u32[PAL_CONTRAST[d.node_color[b] as usize]];
            draw_seg(fb, zb, nx[a], nyv[a], nd[a], mx, my, md, ca);
            draw_seg(fb, zb, mx, my, md, nx[b], nyv[b], nd[b], cb);
        }
        // Draw spheres with palette color.
        let t_sphere = unsafe { core::arch::x86_64::_rdtsc() };
        for i in 0..d.n {
            let base = PAL_RGB[d.node_color[i] as usize];
            draw_sphere(fb, zb, nx[i], nyv[i], nr[i], nd[i], base);
        }

        // Standalone "minimized console": while the console overlay is
        // hidden, it lives in the scene as its own red sphere (orbits with
        // the camera like everything else here) — clicking it, like ESC /
        // the X button / right-click in reverse, restores the console (see
        // in_console_ball / the click chain above). Cache its projected
        // circle for that one-frame-stale pick, same as snap_n* / nx/nyv/nr.
        {
            let (bx, by, bz) = CONSOLE_BALL_POS;
            let (bsx, bsy, bd) = project_pt(bx, by, bz, cyw, syw, cp, sp);
            let bsr = CONSOLE_BALL_R * FOCAL / bd;
            d.console_ball_sx = bsx;
            d.console_ball_sy = bsy;
            d.console_ball_sr = bsr;
            draw_sphere(fb, zb, bsx, bsy, bsr, bd, PAL_RGB[0]);
        }

        // Gizmo rings: brighten the hovered / dragged ring (Unity-style highlight).
        let t_ring = unsafe { core::arch::x86_64::_rdtsc() };
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

        let t_ui = unsafe { core::arch::x86_64::_rdtsc() };
        draw_corner_gizmo(fb, d, cyw, syw, cp, sp);
        draw_taskbar(fb, d);
        draw_cmd_result(fb, d);
        draw_popup(fb, d);

        // Log section timings every 60 frames.
        // wrapping_sub avoids debug-mode panic if RDTSC skips (can happen with WHPX exits).
        // Divide by 3000 → microseconds at ~3 GHz.
        if d.frame % 60 == 59 {
            let t_end = unsafe { core::arch::x86_64::_rdtsc() };
            crate::raw_serial_println(format_args!(
                "PERF fill={}us rope={}us sphere={}us ring={}us ui={}us",
                t_rope.wrapping_sub(t_fill)   / 3_000,
                t_sphere.wrapping_sub(t_rope)  / 3_000,
                t_ring.wrapping_sub(t_sphere)  / 3_000,
                t_ui.wrapping_sub(t_ring)      / 3_000,
                t_end.wrapping_sub(t_ui)       / 3_000,
            ));
        }
    }
    draw_cursor(fb, d.cur_x as i32, d.cur_y as i32);
    let t_blit0 = unsafe { core::arch::x86_64::_rdtsc() };
    if d.lfb != 0 {
        // Non-temporal blit: _mm_stream_si128 (MOVNTDQ) writes 16 bytes at a time.
        unsafe {
            use core::arch::x86_64::{__m128i, _mm_loadu_si128, _mm_stream_si128, _mm_sfence};
            let dst = d.lfb as *mut __m128i;
            let src = fb.as_ptr() as *const __m128i;
            let count = PIXELS / 4; // 4 pixels = 16 bytes = one _mm_stream_si128
            let mut i = 0usize;
            while i < count {
                _mm_stream_si128(dst.add(i), _mm_loadu_si128(src.add(i)));
                i += 1;
            }
            _mm_sfence(); // ensure all NT stores are visible before next frame
        }
    }
    let t_blit1 = unsafe { core::arch::x86_64::_rdtsc() };
    d.frame += 1;
    if d.frame % 60 == 0 {
        crate::raw_serial_println(format_args!("FBF {} blit={}us",
            d.frame, t_blit1.wrapping_sub(t_blit0) / 3_000));
    }
}
