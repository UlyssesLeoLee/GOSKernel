#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;
mod fbtest;
mod kfont;
mod ring3;

use bootloader_api::{entry_point, BootInfo};
use bootloader_api::config::{BootloaderConfig, Mapping};
use core::fmt::{self, Write};
use gos_log::{LogLevel, log};
use gos_protocol::{GraphEdgeSummary, GraphNodeSummary, RuntimeNodeType, VectorAddress};
use k_rast::{project_to_screen, sort_by_depth_desc, Mat4, Vec3};

// ADR-018: bootloader 0.9's `map_physical_memory` Cargo feature always
// mapped all physical memory; bootloader_api 0.11 defaults to NOT
// mapping it (`Mappings::physical_memory: Option<Mapping> = None`) and
// requires opting in via config -- k_fb's Bochs-VBE and DMA address
// translation (gos_hal::phys) both need the offset this provides.
const BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    // Default is 80 KiB with a guard page (bootloader_api docs). GOS's
    // boot call depth (supervisor bootstrap, builtin graph construction)
    // page-faults into that guard page under the default -- bootloader
    // 0.9 had no such guard, so this was never visible as a fault
    // before, presumably silently corrupting adjacent memory instead.
    // 1 MiB is a generous first cut, not a measured minimum -- revisit
    // if boot time/image size ever matters.
    config.kernel_stack_size = 16 * 1024 * 1024;
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

/// `extern "C"` serial write backend for gos-log — writes the message
/// bytes directly to COM1 followed by a newline.
unsafe extern "C" fn log_serial_write(
    _level: u8,
    _source: *const u8,
    msg: *const u8,
    len: u32,
) {
    let mut port = x86_64::instructions::port::Port::<u8>::new(0x3F8);
    let msg_bytes = unsafe { core::slice::from_raw_parts(msg, len as usize) };
    for &b in msg_bytes {
        unsafe { port.write(b); }
    }
    unsafe { port.write(b'\n'); }
}

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Install gos-log serial backend so log!() calls populate both the
    // COM1 serial stream and the in-kernel ring buffer (for the shell's
    // `log` command).
    gos_log::install_serial_backend(gos_log::SerialBackend {
        write: log_serial_write,
    });
    gos_log::set_min_level(gos_log::LogLevel::Info);

    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "kernel_main entered");

    unsafe {
        use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

        let mut cr0 = Cr0::read();
        cr0.remove(Cr0Flags::EMULATE_COPROCESSOR);
        cr0.insert(Cr0Flags::MONITOR_COPROCESSOR);
        Cr0::write(cr0);

        let mut cr4 = Cr4::read();
        cr4.insert(Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE);
        Cr4::write(cr4);
    }
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "cpu features enabled (FPU/SSE)");

    // Minimal bootstrap only owns compatibility addressing and metadata schemas.
    gos_hal::vaddr::init();
    gos_hal::meta::init();
    // ADR-018: bootloader_api 0.11 only maps physical memory when asked
    // (BOOTLOADER_CONFIG above sets Mapping::Dynamic) -- the offset
    // itself is carried as Optional<u64>, unwrapped once here.
    let phys_offset = boot_info
        .physical_memory_offset
        .into_option()
        .expect("bootloader did not map physical memory (check BOOTLOADER_CONFIG)");
    // Store the physical memory offset for DMA address translation in k-net and other drivers.
    gos_hal::phys::set_phys_offset(phys_offset);
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "vaddr/meta/phys initialized");

    // Bring up the framebuffer. Three tiers, in priority order: (1)
    // k_fb self-drives Bochs VBE/DispI via port I/O when present --
    // independent of what the bootloader set up, see crates/k-fb's own
    // doc comment -- gives the sharpest QEMU dev-loop result; (2) ADR-
    // 013/018 GOP, the UEFI-firmware-provided linear framebuffer real
    // hardware (no Bochs DispI) actually needs, sourced from
    // BootInfo.framebuffer; (3) legacy VGA mode 13h. Clearing to
    // Background immediately gives any observer a "kernel is alive"
    // signal instead of a blank/garbage screen.
    let gop_fb = boot_info.framebuffer.as_mut().and_then(|fb| {
        let info = fb.info();
        Some(k_fb::GopFramebuffer {
            // Already the resolved kernel virtual address -- bootloader_api
            // maps the GOP framebuffer via its own independent Mapping
            // (Mappings::framebuffer), not through phys_offset, so this
            // is used as-is, not added to phys_offset (see GopFramebuffer's
            // own doc comment in crates/k-fb).
            virt_addr: fb.buffer_mut().as_mut_ptr() as u64,
            width: info.width,
            height: info.height,
            stride: info.stride,
            bytes_per_pixel: info.bytes_per_pixel,
            bgr: matches!(info.pixel_format, bootloader_api::info::PixelFormat::Bgr),
        })
    });
    unsafe {
        k_fb::init(phys_offset, gop_fb);
    }
    k_fb::clear(k_fb::Color::Background);
    // Header bar serves as the boot-progress indicator: dim teal
    // throughout init, becomes solid at "entering steady-state".
    k_fb::fill_rect(0, 0, k_fb::WIDTH, 18, k_fb::Color::HeaderBar);
    if k_fb::is_hd() {
        raw_serial_println(format_args!(
            "boot: framebuffer up ({} LFB {}x{} @ 32bpp, phys=0x{:x}, logical 320x200 @ {}x upscale)",
            if k_fb::is_gop() { "GOP" } else { "Bochs VBE" },
            k_fb::native_width(),
            k_fb::native_height(),
            k_fb::lfb_physical_address(),
            k_fb::scale(),
        ));
    } else {
        raw_serial_println(format_args!("boot: framebuffer up (mode 13h fallback, 320x200)"));
    }

    raw_serial_println(format_args!("boot: staging supervisor domains"));
    gos_supervisor::bootstrap(boot_info as *const _ as u64);
    for descriptor in builtin_bundle::builtin_supervisor_modules() {
        gos_supervisor::install_module(*descriptor)
            .expect("supervisor failed to install module descriptor");
    }
    log!(LogLevel::Info, *b"SUPERVISOR\0\0\0\0\0\0", "module descriptors registered");

    let report = builtin_bundle::boot_builtin_graph(boot_info as *const _ as u64)
        .expect("builtin graph boot failed");
    log!(LogLevel::Info, *b"RUNTIME\0\0\0\0\0\0\0\0\0", "builtin graph booted: plugins={} loaded={} stable={}",
        report.discovered_plugins, report.loaded_plugins, report.stable_after_load);

    let supervisor_report = gos_supervisor::realize_boot_modules()
        .expect("supervisor failed to realize isolated domains");
    log!(LogLevel::Info, *b"SUPERVISOR\0\0\0\0\0\0", "boot realized: modules={} running={} domains={} caps={} failed={}",
        supervisor_report.discovered_modules, supervisor_report.running_modules,
        supervisor_report.isolated_domains, supervisor_report.published_capabilities,
        supervisor_report.failed_modules);

    let snapshot = gos_runtime::snapshot();
    log!(LogLevel::Info, *b"RUNTIME\0\0\0\0\0\0\0\0\0", "graph ready: nodes={} edges={} ready={} signals={}",
        snapshot.node_count, snapshot.edge_count, snapshot.ready_queue_len, snapshot.signal_queue_len);

    // Phase G.1: synchronously initialize kernel-tier drivers (GDT,
    // IDT, PIC) before interrupts come up.  Builtin modules' on_init
    // never ran via runtime pump because their ModuleEntry was None;
    // hardware setup must happen on the direct path here.
    builtin_bundle::init_kernel_tier_drivers();
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "kernel-tier drivers ready (GDT/IDT/PIC)");

    // Phase E.2: program the syscall MSRs once the GDT is live.
    unsafe { ring3::init(); }
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "ring3 syscall surface armed (STAR/LSTAR)");

    // Boot-time sanity check (ADR-019 §五-1): every bracketed native
    // dispatch should have produced a real Cr3::write_raw -- if these two
    // counts ever diverge, the CR3 trampoline is silently short-circuiting
    // again (e.g. a future domain root missing a PML4 clone) and isolated
    // domains are back to implicitly sharing the caller's CR3.
    log!(LogLevel::Info, *b"SUPERVISOR\0\0\0\0\0\0",
        "cr3 trampoline: domain_switch_count={} real_cr3_switch_count={}",
        gos_runtime::domain_switch_count(), gos_supervisor::real_cr3_switch_count());

    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "interrupts enabled — steady-state loop");
    x86_64::instructions::interrupts::enable();

    // One-shot supervisor drain: clears the post-boot ready queue before the
    // render loop starts. service_system_cycle loops until dispatched==0 &&
    // !restarted (hard cap: 2048 iterations). Running it outside the loop keeps
    // the first frame latency bounded regardless of startup cascade depth.
    fbtest::init();
    x86_64::instructions::interrupts::without_interrupts(gos_supervisor::service_system_cycle);
    // Steady-state loop: per-frame supervisor cycle (delegates input + signals +
    // ready-work to gos_supervisor::service_system_cycle) then one render frame
    // then hlt. In steady state the supervisor queue is empty so the cycle exits
    // on the first iteration (dispatched==0).
    // RDTSC loop-level timing (svc=service_system_cycle, rf=render_frame) is
    // logged on the first few iterations and then every 60 — a permanent,
    // lightweight FPS/latency trace (see also fbtest's PERF/FBF logs).
    let mut loop_iter: u64 = 0;
    // Host-bridged GPU surface (Phase B): in addition to the in-guest software
    // desktop, emit the live graph as an `@gos.vk` display-list frame to COM3
    // (TCP:14445) so `gos-vk-viewer --live` can render it on the host GPU
    // (smooth, high-res, host-VRAM). Throttled on the 120 Hz PIT and gated on
    // graph_epoch inside vk_auto_refresh, so an idle graph costs ~one epoch
    // read. Runs OUTSIDE without_interrupts (the slow UART emit must not stall
    // IRQs). fbtest stays during the transition; once the viewer is solid the
    // in-guest renderer can be retired.
    const VK_REFRESH_TICKS: u64 = 30; // ~0.25s: emit promptly when the graph changes
    const VK_KEEPALIVE_TICKS: u64 = 120; // ~1s: full frame so a late viewer still gets the scene
    let mut vk_last_tick = k_pit::get_ticks() as u64;
    let mut vk_keepalive_tick = vk_last_tick;
    loop {
        let lt0 = unsafe { core::arch::x86_64::_rdtsc() };
        let report = x86_64::instructions::interrupts::without_interrupts(
            gos_supervisor::service_system_cycle,
        );
        if !report.quiesced {
            // ADR-002 §4: a non-quiescent cycle is reported, not silently
            // truncated — see gos_supervisor::CYCLE_DEPTH_CAP.
            raw_serial_println(format_args!(
                "service_system_cycle: causal-depth guard tripped at depth={} (steps={})",
                report.max_causal_depth, report.steps
            ));
        }
        let lt1 = unsafe { core::arch::x86_64::_rdtsc() };
        fbtest::render_frame();
        let lt2 = unsafe { core::arch::x86_64::_rdtsc() };
        let now = k_pit::get_ticks() as u64;
        if now.wrapping_sub(vk_last_tick) >= VK_REFRESH_TICKS {
            vk_last_tick = now;
            k_vk_host::vk_auto_refresh();
        }
        if now.wrapping_sub(vk_keepalive_tick) >= VK_KEEPALIVE_TICKS {
            vk_keepalive_tick = now;
            k_vk_host::vk_force_refresh();
        }
        // B3b: drain viewer→kernel input (COM3 RX) and feed it to the key queue.
        // Echo each byte to the boot serial (COM1) so the round-trip is
        // observable in terminal A. k_ps2::inject_byte mirrors the real PS/2
        // IRQ path (push into the desktop ring buffer + route to k-shell), so
        // viewer keystrokes drive the same graph CLI (theme switches, cypher
        // commands, ...) that a physical keyboard does.
        while let Some(b) = k_vk_host::vk_drain_input() {
            raw_serial_println(format_args!("vk-input: {:#04x}", b));
            k_ps2::inject_byte(b);
        }
        loop_iter = loop_iter.wrapping_add(1);
        // Print timing on iteration 1, 2, 3, then every 60 iterations.
        if loop_iter <= 3 || loop_iter % 60 == 0 {
            raw_serial_println(format_args!("LOOP #{} svc={}us rf={}us",
                loop_iter,
                lt1.wrapping_sub(lt0) / 3_000,
                lt2.wrapping_sub(lt1) / 3_000,
            ));
        }
        x86_64::instructions::hlt();
    }
}

// ─── Phase I.3.8 — software 3D scene painter ────────────────────────
//
// Renders the kernel graph as a rotating 3D scene in mode 13h:
// nodes are coloured cubes laid out in a 3D grid, edges are
// straight lines between cube centres.  Camera auto-orbits the
// origin; `frame_counter` advances the yaw so the view animates
// without any input plumbing.
//
// Pipeline per frame:
//   1. Clear framebuffer to Background, paint header bar + status text.
//   2. Compute view_proj from the current camera yaw.
//   3. For each visible node: project its centre + 8 cube corners.
//      Front-face-cull each of the 12 triangles via screen-space
//      signed area sign.  Submit surviving tris to fill_triangle.
//      Sort cubes by view-space depth so painter's algorithm
//      produces a correct image without a per-pixel z-buffer.
//   4. For each edge: project the two endpoint centres, draw a
//      Bresenham line in the edge-type's colour.
//
// 320×200×256 leaves a tight pixel budget; ~30 nodes × 12 tris ≈ 360
// triangles is comfortably within one PIT tick.

const SCENE_WIDTH: i32 = k_fb::WIDTH as i32;
const SCENE_HEIGHT: i32 = k_fb::HEIGHT as i32;
// N.17 — chrome consolidated to a single slim 22-px footer.  The old
// 28-px top header is gone (per user request "顶部的显示内容合并到底部")
// — brand + node/edge stats + uptime + heartbeat + command input +
// mode pill all coexist in one bottom strip ~1/3 the height of the
// former two-strip chrome.  Scene body grows top→bottom from 0 to
// FOOTER_Y, gaining 26 px of usable height.
const HEADER_H: i32 = 0;
const FOOTER_Y: i32 = 178;
const CMD_BAR_TOP: i32 = 178;
const CMD_BAR_H: i32 = 22;

// ── N.12 — design tokens ──────────────────────────────────────────
//
// Centralised colour & spacing system so the kernel UI reads as one
// designed product instead of a collage of ad-hoc rectangles.
// Inspired by Win11 Fluent + Material 3 + Apple HIG: dark "deep
// space" surface family, cyan-magenta accent dyad, text scale
// (caption/body/heading) with explicit contrast targets.
//
// All colours are 24-bit BGRX so the design tokens compose with the
// 24-bit shader output without palette quantisation.
#[allow(dead_code)]
mod design {
    // Surface ramp — vertical gradient endpoints for nested elevations.
    pub const SURFACE_0_TOP:    u32 = 0x0a0f24; // body bg top (deep navy)
    pub const SURFACE_0_BOTTOM: u32 = 0x05080f; // body bg bottom (near-black)
    pub const SURFACE_1_TOP:    u32 = 0x12183a; // panel bg top
    pub const SURFACE_1_BOTTOM: u32 = 0x0a0f24; // panel bg bottom
    pub const SURFACE_2_TOP:    u32 = 0x1a2348; // elevated (header/dock) top
    pub const SURFACE_2_BOTTOM: u32 = 0x0a0f24; // elevated bottom
    pub const SURFACE_3:        u32 = 0x2d3b6e; // interactive hover

    // Accent dyad — primary cyan + secondary hot magenta.
    pub const ACCENT_PRIMARY:    u32 = 0x4ad0ff; // electric cyan
    pub const ACCENT_PRIMARY_DIM:u32 = 0x2a7a99;
    pub const ACCENT_HOT:        u32 = 0xff3d8b; // magenta
    pub const ACCENT_WARN:       u32 = 0xffa14a; // amber
    pub const ACCENT_OK:         u32 = 0x4adf95; // mint

    // Text — three-level hierarchy with WCAG-AA contrast on SURFACE_0.
    pub const TEXT_PRIMARY:   u32 = 0xeef2f8; // off-white headings
    pub const TEXT_SECONDARY: u32 = 0xa4b0c0; // body
    pub const TEXT_TERTIARY:  u32 = 0x6878a0; // captions / dim labels

    // Spacing tokens — 4-px grid: xs=4, sm=8, md=12, lg=16, xl=24.
    pub const SP_XS: usize = 2;
    pub const SP_SM: usize = 4;
    pub const SP_MD: usize = 8;
    pub const SP_LG: usize = 12;
    pub const SP_XL: usize = 16;

    // Card corner radius — sm for chips, md for panels.
    pub const RADIUS_SM: usize = 3;
    pub const RADIUS_MD: usize = 6;

    // Pill / chip metrics.
    pub const CHIP_H: usize = 18;
}
/// Height of the scrollback panel when F9 has expanded it.  Sits
/// just above the command bar, overlapping the lower portion of
/// the scene area as a translucent-feeling deck.
const SCROLLBACK_H: i32 = 84;
const MAX_NODES: usize = 64;
const MAX_EDGES: usize = 128;

/// Auto-rotation rate.  At ~100 Hz PIT and REPAINT_TICKS = 2 we get
/// ~50 fps; 0.04 rad/frame ≈ 115°/sec — fast enough to read as
/// motion without being dizzying.
const YAW_PER_FRAME: f32 = 0.04;

// ── Phase I.3.11 — mouse hit-test + click latch ────────────────────
//
// `SELECTED_NODE_SLOT` indexes into the snapshot's node array.  -1
// means "no selection".  Updated when the user left-clicks while
// hovering a cube; cleared by F6 (camera reset) or by clicking
// empty space.  The detail panel reads it each frame.
//
// `MOUSE_PREV_BTN` provides cheap edge-detection so a held button
// doesn't re-fire the selection action every frame.

use core::sync::atomic::{AtomicI8, AtomicI32 as AtomicI32M, AtomicU8 as AtomicU8Btn};
static SELECTED_NODE_SLOT: AtomicI8 = AtomicI8::new(-1);
static MOUSE_PREV_BTN: AtomicU8Btn = AtomicU8Btn::new(0);

// Phase I.3.x — mouse-drag orbit.  Each frame compares the current
// k_mouse cursor against the previous-frame snapshot; when the left
// button is held over empty space the delta drives the camera yaw /
// pitch biases in k-fb.  Sentinel -1 indicates "no prior frame yet"
// so the very first drag tick doesn't apply a junk delta from the
// initial cursor (160, 100).
static PREV_MOUSE_X: AtomicI32M = AtomicI32M::new(-1);
static PREV_MOUSE_Y: AtomicI32M = AtomicI32M::new(-1);

/// Half-extent in screen pixels used for hit-testing each cube.
/// Larger than the rendered cube on screen so the user has a
/// reasonable click target even when the perspective shrinks
/// far-side cubes.
const HIT_HALF_PX: i32 = 8;

/// Mouse-orbit sensitivity in mrad / pixel.  60 mrad ≈ 3.4°.
/// Tuned by feel — fast enough that small drags rotate visibly,
/// slow enough that the camera doesn't whip past every cube.
const ORBIT_YAW_MRAD_PER_PX: i32 = 8;
const ORBIT_PITCH_MRAD_PER_PX: i32 = 6;

fn paint_3d_view(frame: u64) {
    if !k_fb::ready() {
        return;
    }

    let snapshot = gos_runtime::snapshot();
    let mut nodes = [GraphNodeSummary::EMPTY; MAX_NODES];
    let (_total_n, returned_n) = gos_runtime::node_page(0, &mut nodes);
    let mut edges = [GraphEdgeSummary::EMPTY; MAX_EDGES];
    let (_total_e, returned_e) = gos_runtime::edge_page(0, &mut edges);

    // ── Phase I.6.3 — advance the soft-body Verlet step once per
    //    frame BEFORE any projection so node_world_position sees
    //    today's positions, not yesterday's.
    physics_step(&nodes[..returned_n], returned_n, &edges[..returned_e], returned_e);

    // Camera: orbit around origin at fixed radius + pitch.  Phase
    // I.3.9 — F-keys (handled by k-ps2) bias yaw/pitch/radius via
    // the shared atomics in k-fb; F1 toggles auto-yaw.
    use core::sync::atomic::Ordering;
    let auto_on = k_fb::CAMERA_AUTO_ROTATE.load(Ordering::Relaxed);
    let yaw_bias = k_fb::CAMERA_YAW_BIAS_MRAD.load(Ordering::Relaxed) as f32 / 1000.0;
    let pitch_bias = k_fb::CAMERA_PITCH_BIAS_MRAD.load(Ordering::Relaxed) as f32 / 1000.0;
    let radius = (k_fb::CAMERA_RADIUS_MM.load(Ordering::Relaxed).max(800) as f32) / 1000.0;
    let auto_yaw = if auto_on { (frame as f32) * YAW_PER_FRAME } else { 0.0 };
    let yaw = auto_yaw + yaw_bias;
    let pitch: f32 = (0.45 + pitch_bias).clamp(-1.4, 1.4);
    let eye = Vec3::new(
        radius * libm::cosf(pitch) * libm::sinf(yaw),
        radius * libm::sinf(pitch),
        radius * libm::cosf(pitch) * libm::cosf(yaw),
    );
    let view = Mat4::look_at(eye, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let aspect = SCENE_WIDTH as f32 / SCENE_HEIGHT as f32;
    let proj = Mat4::perspective(60.0_f32.to_radians(), aspect, 0.1, 100.0);
    let view_proj = proj.mul(view);

    // Background + header band painted by `paint_frame` (I.5).
    // This function now only owns the kernel-view body.

    // ── I.10.1 — animated starfield ───────────────────────────────
    // Procedural stars drifting horizontally with parallax-by-layer.
    // 96 stars in 3 depth layers (slow / medium / fast).  Brightness
    // pulses on a sin curve so the field feels alive without any
    // per-pixel jitter cost.  Seeded by index → deterministic so
    // the pattern is stable across boots / save-states.
    paint_starfield(frame);

    // Project node centres + classify colour.  We need both screen
    // coordinates (for edges + I.3.10 labels) and view-space depth
    // (for painter's sort).
    let mut node_centre_screen = [(0i32, 0i32, false); MAX_NODES];
    let mut node_depth_z = [0f32; MAX_NODES];
    let mut depths: [(usize, f32); MAX_NODES] = [(0, 0.0); MAX_NODES];
    let mut depth_count = 0usize;
    for i in 0..returned_n {
        let centre = node_world_position(i, returned_n);
        let clip = view_proj.transform_point(centre);
        if let Some((sx, sy, z)) = project_to_screen(clip, k_fb::WIDTH as u32, k_fb::HEIGHT as u32)
        {
            node_centre_screen[i] = (sx, sy, true);
            node_depth_z[i] = z;
            // depth: distance² from camera.  Larger = farther.
            let to_cam = centre.sub(eye);
            depths[depth_count] = (i, to_cam.dot(to_cam));
            depth_count += 1;
        }
    }
    sort_by_depth_desc(&mut depths[..depth_count]);

    // I.4.7 — sub-domain halos.  Before drawing any octahedral core,
    // paint a thin 1-px ring around each node's projected centre with
    // a colour derived from `NodeSubDomain` (the partition introduced
    // by audit P2 #4).  All halos paint first as a separate pass so
    // the octahedra naturally overlap and hide the inner portion,
    // leaving a thin colored frame that reads as "this node belongs
    // to <class>" — finally making the orthogonal partition visible.
    //
    // Skip halos for nodes whose projected position is near the
    // header/footer edge so we don't paint into reserved bands.
    let halo_half: i32 = 9; // slightly larger than HIT_HALF_PX (8) so
                            // ~1 px of halo pokes out around the octa
    for slot in 0..depth_count {
        let i = depths[slot].0;
        let (sx, sy, ok) = node_centre_screen[i];
        if !ok {
            continue;
        }
        let halo_color = sub_domain_color(nodes[i].sub_domain);
        let x = (sx - halo_half).max(0);
        let y = (sy - halo_half).max(HEADER_H);
        let w = (halo_half * 2).min(SCENE_WIDTH - x);
        let h = (halo_half * 2).min(FOOTER_Y - y);
        if w > 0 && h > 0 {
            k_fb::stroke_rect(x as usize, y as usize, w as usize, h as usize, halo_color);
        }
    }

    // Octahedral cores (painter's order: far first, near last).  Fog
    // factor [0,1] is the normalised distance-from-camera bucketed by
    // the precomputed depth² we already sorted on — gives "near = full
    // colour, far = receding into nebula" without a second pass.
    let max_depth_sq = depths[..depth_count]
        .iter()
        .fold(0.0_f32, |acc, &(_, d)| if d > acc { d } else { acc })
        .max(1e-3);
    // N.13 — wrap sphere loop in one paint session (single BACKBUFFER
    // lock acquire) so the 20k+ per-pixel writes are direct slice
    // stores instead of repeated spin-mutex acquires.  ~10× faster
    // under QEMU TCG.
    k_fb::with_session(|sess| {
        for slot in 0..depth_count {
            let (i, d_sq) = depths[slot];
            let (sx, sy, ok) = node_centre_screen[i];
            if !ok {
                continue;
            }
            let centre = node_world_position(i, returned_n);
            let offset = Vec3::new(centre.x + NODE_HALF, centre.y, centre.z);
            let r_px = match project_to_screen(
                view_proj.transform_point(offset),
                k_fb::WIDTH as u32,
                k_fb::HEIGHT as u32,
            ) {
                Some((ox, _oy, _)) => (ox - sx).abs().max(2),
                None => 4,
            };
            let hue_base = classify_node_hue(&nodes[i]);
            let lin = libm::sqrtf(d_sq / max_depth_sq);
            let fog = (lin * lin * 0.9).clamp(0.0, 1.0);
            let flash = physics_flash_value(i) as f32;
            let specular_boost = flash * 0.045;
            let material = material_for_sub_domain(nodes[i].sub_domain);
            draw_node_sphere(sess, sx, sy, r_px, hue_base, fog, specular_boost, material);
        }
    });

    // Edges: lines between projected centres, styled by edge type
    // (I.4.3).  Pattern is applied via a step counter incremented in
    // the per-pixel closure that Bresenham invokes once per step.
    //
    //   Mount   — solid 2-px parallel (heavy structural attachment)
    //   Use     — dashed 2-on / 2-off (capability lookup)
    //   Depend  — dotted 1-on / 3-off (cold/declarative)
    //   Signal  — solid pulsed (frame-modulated brightness)
    //   Link    — bright gradient ends (metadata correspondence)
    //   _       — DimWhite hairline
    //
    // The pulse phase comes from `frame` so animation runs at the
    // repaint rate (~50 Hz @ REPAINT_TICKS=2).
    // ── Phase I.6.2 — rope edges with catenary sag + thickness ──
    //
    // Edges are now ropes: each one sampled as a chain of short
    // straight segments along a parabolic "sag" curve so long edges
    // visibly droop the way a real rope in zero-g + screen-gravity
    // would.  Thickness (2-4 px) is drawn by stamping the per-pixel
    // Bresenham closure with a center brightest + perpendicular halo
    // dimmer.  Style (pattern, pulse, gradient, mount-rigid) is
    // layered on top.
    // N.10 — edge pulse only animates while the camera auto-rotates;
    // otherwise stays solid-on so the scene reads as a still tableau.
    let pulse_on = if k_fb::CAMERA_AUTO_ROTATE.load(Ordering::Relaxed) {
        let pulse_phase = libm::sinf(frame as f32 * 0.22);
        pulse_phase > -0.4
    } else {
        true
    };
    k_fb::with_session(|sess| {
        for i in 0..returned_e {
            let from_idx = find_node_index(&nodes[..returned_n], edges[i].from_vector);
            let to_idx = find_node_index(&nodes[..returned_n], edges[i].to_vector);
            let (Some(fi), Some(ti)) = (from_idx, to_idx) else { continue };
            let (fx, fy, fok) = node_centre_screen[fi];
            let (tx, ty, tok) = node_centre_screen[ti];
            if !(fok && tok) {
                continue;
            }
            // N.14 — per-endpoint rope colour.  User request: "red ball
            // end → rope is white on that half, white ball end → rope
            // is red on that half".  Compute each endpoint's redness
            // from its sphere hue and pass to the rope shader, which
            // gradient-lerps along its length.
            let from_red = ball_is_red(classify_node_hue(&nodes[fi]));
            let to_red   = ball_is_red(classify_node_hue(&nodes[ti]));
            let from_w = node_world_position(fi, returned_n);
            let to_w = node_world_position(ti, returned_n);
            let dxw = to_w.x - from_w.x;
            let dyw = to_w.y - from_w.y;
            let dzw = to_w.z - from_w.z;
            let len_w = libm::sqrtf(dxw * dxw + dyw * dyw + dzw * dzw);
            let strain = libm::fabsf(len_w - PHYS_REST_LEN) / PHYS_REST_LEN;
            let highlight = strain > 0.15;
            let style = edge_style(edges[i].edge_type);
            draw_rope_edge(sess, fx, fy, tx, ty, from_red, to_red, highlight, style, pulse_on);
        }
    });

    // I.3.10 — node labels.  Project each node's centre to screen,
    // place the first ~6 chars of its plugin_name just above the
    // cube.  We iterate in NEAR-FIRST order (reverse of the cube
    // draw order) so labels for closer cubes overdraw labels for
    // farther ones when they overlap.  Frustum-clipped cubes have
    // `node_centre_screen[i].2 == false` and skip silently.  Skip
    // labels whose ndc.z is past the far plane (z > 0.99) — they're
    // basically dust on the horizon.
    for slot in (0..depth_count).rev() {
        let i = depths[slot].0;
        let (sx, sy, ok) = node_centre_screen[i];
        if !ok {
            continue;
        }
        if node_depth_z[i] > 0.99 {
            continue;
        }
        let name = nodes[i].plugin_name;
        let trimmed = name.strip_prefix("K_").unwrap_or(name);
        let bytes = trimmed.as_bytes();
        let label_len = bytes.len().min(10);
        let label = unsafe { core::str::from_utf8_unchecked(&bytes[..label_len]) };
        // N.11 — AA Noto Sans SC 14 px label.  cell_w = 14, so
        // centre on cell width × char count.
        let text_w = (label_len * 14) as i32;
        let lx = (sx - text_w / 2).max(4) as usize;
        let ly = (sy - 28).max(HEADER_H + 2) as usize;
        if lx + label_len * 14 > k_fb::WIDTH || ly + 20 >= FOOTER_Y as usize {
            continue;
        }
        // 1-px drop shadow + 1-px soft glow for hover-ready labels.
        k_fb::draw_text_ui(lx + 1, ly + 1, label, k_fb::Color::Background);
        k_fb::draw_text_ui(lx, ly, label, k_fb::Color::Foreground);
    }

    // ── I.3.11 / I.3.12 — mouse cursor + click-to-select + drag-orbit ──
    // Snapshot atomics once per frame.
    let mouse_x = k_mouse::MOUSE_X.load(Ordering::Relaxed);
    let mouse_y = k_mouse::MOUSE_Y.load(Ordering::Relaxed);
    let mouse_btn = k_mouse::MOUSE_BUTTONS.load(Ordering::Relaxed);
    let prev_btn = MOUSE_PREV_BTN.swap(mouse_btn, Ordering::Relaxed);
    let left_pressed = (mouse_btn & 0x01) != 0;
    let left_edge = left_pressed && (prev_btn & 0x01) == 0;

    // Hit-test the topmost cube under the cursor (iterate near→far so
    // the closer of two overlapping cubes wins).
    let mut hover_slot: i8 = -1;
    for slot in (0..depth_count).rev() {
        let i = depths[slot].0;
        let (sx, sy, ok) = node_centre_screen[i];
        if !ok {
            continue;
        }
        if (mouse_x - sx).abs() <= HIT_HALF_PX && (mouse_y - sy).abs() <= HIT_HALF_PX {
            hover_slot = i as i8;
            break;
        }
    }

    // Click edge: latch new selection (or clear if cursor was over
    // empty space).  Triggers ONLY on press, not during a drag —
    // dragging from a cube initiates orbit + leaves selection alone.
    if left_edge && hover_slot >= 0 {
        SELECTED_NODE_SLOT.store(hover_slot, Ordering::Relaxed);
        // I.6.4 — click flash: bump the per-node specular boost so
        // the sphere reads a tactile "ping" on the next ~14 frames.
        physics_flash(hover_slot as usize);
        // I.8 — stash the clicked node's vector address so the
        // command-bar's Tab handler can expand it into a literal
        // at the cursor, letting the user compose Cypher mutations
        // (LINK / CREATE USE / ...) by clicking + Tab instead of
        // typing four-component vector addresses by hand.
        let clicked_vec = nodes[hover_slot as usize].vector;
        k_fb::UI_LAST_CLICK_VECTOR.store(clicked_vec.as_u64(), Ordering::Relaxed);
    } else if left_edge && hover_slot < 0 {
        // Clicking empty space clears selection.
        SELECTED_NODE_SLOT.store(-1, Ordering::Relaxed);
    }
    let selected = SELECTED_NODE_SLOT.load(Ordering::Relaxed);

    // Drag-orbit: while the left button is held over empty space,
    // mouse deltas drive the camera's yaw/pitch bias atomics.  When
    // the user is hovering a cube and presses, that's a select-click;
    // dragging off-cube subsequently still orbits because we re-test
    // hover each frame.  Initial PREV_MOUSE = -1 sentinel suppresses
    // the very first delta after boot.
    let prev_mx = PREV_MOUSE_X.swap(mouse_x, Ordering::Relaxed);
    let prev_my = PREV_MOUSE_Y.swap(mouse_y, Ordering::Relaxed);
    if left_pressed && hover_slot < 0 && prev_mx >= 0 && prev_my >= 0 {
        let dx = mouse_x - prev_mx;
        let dy = mouse_y - prev_my;
        if dx != 0 {
            k_fb::CAMERA_YAW_BIAS_MRAD
                .fetch_add(dx * ORBIT_YAW_MRAD_PER_PX, Ordering::Relaxed);
        }
        if dy != 0 {
            // Inverted: dragging up rotates view up (raises pitch).
            k_fb::CAMERA_PITCH_BIAS_MRAD
                .fetch_add(-dy * ORBIT_PITCH_MRAD_PER_PX, Ordering::Relaxed);
        }
    }

    // ── N.15 — futuristic targeting-reticle cursor ────────────────
    //
    // Layout (mouse_x, mouse_y at the centre):
    //                .              <- 1-px dim cyan tick (vertical top)
    //              · · ·
    //              ·   ·            <- AA ring r=4 in cyan
    //         .  ·  X  ·  .         <- white core pixel +
    //              ·   ·               4 cardinal tick marks (cyan)
    //              · · ·
    //                .
    //          ◦         ◦          <- diagonal corner pips, very dim
    //              ◦       ◦
    //
    // Reads as a sci-fi reticle: precision aim point (1-px white) +
    // halo ring (orientation) + cardinal range marks (depth cue) +
    // diagonal corner pips (subtle "tracking" feel).  No yellow.
    if mouse_x >= 0 && mouse_x < SCENE_WIDTH
        && mouse_y >= HEADER_H && mouse_y < FOOTER_Y {
        use design::*;
        let cx = mouse_x;
        let cy = mouse_y;
        // 1) AA ring (radius 4, 16 tap)
        for theta in 0..24 {
            let ang = theta as f32 * 6.2832 / 24.0;
            let rx = libm::cosf(ang) * 4.0;
            let ry = libm::sinf(ang) * 4.0;
            let sx = cx + rx as i32;
            let sy = cy + ry as i32;
            if sx >= 0 && sx < SCENE_WIDTH && sy >= HEADER_H && sy < FOOTER_Y {
                k_fb::blend_pixel_rgb(sx as usize, sy as usize, ACCENT_PRIMARY, 220);
            }
        }
        // 2) Four cardinal ticks at distance 7..8 — like an external
        // reticle's "range" notches.
        for d in 7..=8 {
            for &(dx, dy) in &[(d, 0), (-d, 0), (0, d), (0, -d)] {
                let sx = cx + dx;
                let sy = cy + dy;
                if sx >= 0 && sx < SCENE_WIDTH && sy >= HEADER_H && sy < FOOTER_Y {
                    k_fb::put_pixel_rgb(sx as usize, sy as usize, ACCENT_PRIMARY);
                }
            }
        }
        // 3) Diagonal corner pips at distance 6 (dim, "tracking" feel)
        for &(dx, dy) in &[(6, 6), (-6, 6), (6, -6), (-6, -6)] {
            let sx = cx + dx;
            let sy = cy + dy;
            if sx >= 0 && sx < SCENE_WIDTH && sy >= HEADER_H && sy < FOOTER_Y {
                k_fb::blend_pixel_rgb(sx as usize, sy as usize, ACCENT_PRIMARY, 110);
            }
        }
        // 4) Precision core: 1-px white at the exact cursor position.
        if cx >= 0 && cx < SCENE_WIDTH && cy >= HEADER_H && cy < FOOTER_Y {
            k_fb::put_pixel_rgb(cx as usize, cy as usize, 0xffffff);
        }
    }

    // Hover halo: yellow ring around the hovered cube.
    if hover_slot >= 0 {
        let i = hover_slot as usize;
        let (sx, sy, _ok) = node_centre_screen[i];
        k_fb::stroke_rect(
            (sx - HIT_HALF_PX - 1).max(0) as usize,
            (sy - HIT_HALF_PX - 1).max(0) as usize,
            ((HIT_HALF_PX * 2 + 2) as usize).min(k_fb::WIDTH),
            ((HIT_HALF_PX * 2 + 2) as usize).min(k_fb::HEIGHT),
            k_fb::Color::Highlight,
        );
    }

    // Selection halo: sci-fi pulsing two-ring effect.  Outer ring is
    // fixed at +3 px and dims with the breathing phase; inner ring
    // pulses radius ±2 px on a ~1.2 Hz cycle (sin of frame counter).
    if selected >= 0 && (selected as usize) < returned_n {
        let i = selected as usize;
        let (sx, sy, ok) = node_centre_screen[i];
        if ok {
            let phase = libm::sinf(frame as f32 * 0.18);
            let inner_r = HIT_HALF_PX + 2 + (phase * 2.0) as i32;
            let outer_r = HIT_HALF_PX + 5;
            // Outer ring (steady neon-cyan rim using brightest mint
            // shade — reads as "highlight" without colliding with any
            // node's ramp).
            k_fb::stroke_rect(
                (sx - outer_r).max(0) as usize,
                (sy - outer_r).max(0) as usize,
                ((outer_r * 2) as usize).min(k_fb::WIDTH),
                ((outer_r * 2) as usize).min(k_fb::HEIGHT),
                k_fb::Color::Foreground,
            );
            // Inner pulsing ring in Highlight (electric yellow) —
            // shifts radius with phase so the eye reads it as
            // "breathing".
            k_fb::stroke_rect(
                (sx - inner_r).max(0) as usize,
                (sy - inner_r).max(0) as usize,
                ((inner_r * 2) as usize).min(k_fb::WIDTH),
                ((inner_r * 2) as usize).min(k_fb::HEIGHT),
                k_fb::Color::Highlight,
            );
        }
    }

    // ── N.11 — Unity-style ViewCube gizmo ──────────────────────────
    //
    // Upper-right corner widget.  Renders 6 axis-end caps (±X ±Y ±Z),
    // each clickable: a left-mouse click on a cap snaps the camera to
    // look down that axis (resets yaw/pitch atomics accordingly).  Cap
    // colours follow Unity's convention: red = X, green = Y, blue = Z.
    // The corresponding negative cap is the same hue at 40 % intensity
    // so the user can tell "+X bright, -X faded" at a glance.
    //
    // Layout (12-px arm length):
    //         +Y
    //          |
    //   -X --(o)-- +X
    //          |
    // ── N.15 — minimalist game-engine gizmo ────────────────────────
    //
    // Per user feedback ("gizmo 造型简洁一些, 就像游戏引擎里那样"):
    // strip the previous busy 6-cap ViewCube down to the same shape
    // Unity / Blender / Godot use in their viewport corner:
    //   * 3 thin AA Wu-line arms in canonical R/G/B (X/Y/Z)
    //   * tiny 2-px filled cap at each + axis end (clickable hit target)
    //   * single 1-px white centre pivot
    //   * no negative-axis stubs, no halo rings, no hover overlay
    //   * hover state encoded as a brighter cap fill, not an extra ring
    //
    // Click any +axis cap → camera snaps to look down that axis.
    // The same `frame_all` behaviour stays on F6.
    // N.17 — viewport spans 0..FOOTER_Y now; place gizmo in the
    // top-left with a comfortable margin instead of below a header.
    const GIZMO_CX: i32 = 32;
    const GIZMO_CY: i32 = 28;
    const GIZMO_LEN: f32 = 14.0;
    const GIZMO_CAP_R: i32 = 2;
    const GIZMO_HIT_R: i32 = 5;
    let forward = Vec3::new(-eye.x, -eye.y, -eye.z).normalize();
    let world_up = Vec3::new(0.0, 1.0, 0.0);
    let right = forward.cross(world_up).normalize();
    let true_up = right.cross(forward);
    let project_axis = |axis: Vec3| -> (i32, i32) {
        let sx = (axis.dot(right) * GIZMO_LEN) as i32;
        let sy = -(axis.dot(true_up) * GIZMO_LEN) as i32;
        (sx, sy)
    };
    // Three +axes only (matches Unity / Blender minimal view gizmo).
    let axes: [(Vec3, u32); 3] = [
        (Vec3::new(1.0, 0.0, 0.0), 0xff4d57),  // X red
        (Vec3::new(0.0, 1.0, 0.0), 0x6cdf78),  // Y green
        (Vec3::new(0.0, 0.0, 1.0), 0x4ad0ff),  // Z cyan
    ];

    // 1-px AA Wu line per axis arm.  Matches Blender's thin-line look.
    for (axis, hue) in &axes {
        let (dx, dy) = project_axis(*axis);
        k_fb::draw_line_aa_rgb(GIZMO_CX, GIZMO_CY, GIZMO_CX + dx, GIZMO_CY + dy, *hue);
    }

    // Tiny solid caps + click hit test.
    let mut hovered_axis: Option<(i32, i32, i32)> = None;
    for (axis, hue) in &axes {
        let (dx, dy) = project_axis(*axis);
        let cap_x = GIZMO_CX + dx;
        let cap_y = GIZMO_CY + dy;
        // Hit test against mouse (slightly larger than visual disc).
        let dx_m = mouse_x - cap_x;
        let dy_m = mouse_y - cap_y;
        let hovering = dx_m * dx_m + dy_m * dy_m <= GIZMO_HIT_R * GIZMO_HIT_R;
        if hovering {
            hovered_axis = Some((axis.x as i32, axis.y as i32, axis.z as i32));
        }
        // Cap colour: hover → brighter / full saturation; idle → 85 %.
        let cap_color = if hovering { 0xffffff } else { k_fb::scale_bgrx(*hue, 0.85) };
        // Filled 2-px disc (5-pixel cluster).
        for ry in -GIZMO_CAP_R..=GIZMO_CAP_R {
            for rx in -GIZMO_CAP_R..=GIZMO_CAP_R {
                if rx * rx + ry * ry > GIZMO_CAP_R * GIZMO_CAP_R { continue; }
                let sx = cap_x + rx;
                let sy = cap_y + ry;
                if sx >= 0 && sx < SCENE_WIDTH && sy >= 0 && sy < FOOTER_Y {
                    k_fb::put_pixel_rgb(sx as usize, sy as usize, cap_color);
                }
            }
        }
    }

    // Single 1-px white centre pivot.
    if GIZMO_CX >= 0 && GIZMO_CX < SCENE_WIDTH && GIZMO_CY < FOOTER_Y {
        k_fb::put_pixel_rgb(GIZMO_CX as usize, GIZMO_CY as usize, 0xffffff);
    }

    // Click-to-snap (same as before, just shorter list — only +axes now).
    if left_edge {
        if let Some((ax, ay, az)) = hovered_axis {
            let (target_yaw, target_pitch) = match (ax, ay, az) {
                ( 1, 0, 0) => ( core::f32::consts::FRAC_PI_2, 0.0),
                (0,  1, 0) => (0.0,  1.35),
                (0, 0,  1) => (0.0, 0.0),
                _ => (0.0, 0.45),
            };
            k_fb::CAMERA_YAW_BIAS_MRAD.store((target_yaw * 1000.0) as i32, Ordering::Relaxed);
            k_fb::CAMERA_PITCH_BIAS_MRAD.store(((target_pitch - 0.45) * 1000.0) as i32, Ordering::Relaxed);
            k_fb::CAMERA_AUTO_ROTATE.store(false, Ordering::Relaxed);
            SELECTED_NODE_SLOT.store(-2, Ordering::Relaxed);
        }
    }

    // Detail panel: bottom-right corner for the SELECTED node.
    if selected >= 0 && (selected as usize) < returned_n {
        let node = &nodes[selected as usize];
        // ── N.12 — glass detail card ───────────────────────────────
        // Frosted glass background (box-blurred underlying scene) +
        // translucent surface tint + cyan accent stroke + 3 typography
        // rows: label, value (vector), pill (type).
        use design::*;
        const PANEL_W: usize = 158;
        const PANEL_H: usize = 64;
        let px = k_fb::WIDTH - PANEL_W - SP_MD;
        let py = FOOTER_Y as usize - PANEL_H - SP_MD;
        // Glass: blur background, then tint with elevated surface.
        k_fb::box_blur_region(px, py, PANEL_W, PANEL_H);
        k_fb::fill_round_rect_alpha(px, py, PANEL_W, PANEL_H, RADIUS_MD, SURFACE_1_TOP, 200);
        // Left-edge cyan accent bar (vertical) — keeps the panel's
        // "selected" emphasis anchored, complements the vapor halo.
        k_fb::fill_rect_alpha(px, py + SP_SM, 3, PANEL_H - 2 * SP_SM, ACCENT_PRIMARY, 255);
        // N.17 — gas-particle vapor perimeter (replaces solid line).
        let vp = frame as f32 * 0.0018 + 0.67;
        k_fb::vapor_rect_particles(px, py, PANEL_W, PANEL_H, vp);

        // Row 1 — eyebrow label (small, dim).
        k_fb::draw_text_aa_rgb(px + SP_LG, py + SP_SM, "SELECTED NODE", TEXT_TERTIARY, &k_assets::FONT_UI_14);
        // Row 2 — plugin name (large, primary).
        let mut row1 = TextBuf::<20>::new();
        row1.push_str(node.plugin_name);
        k_fb::draw_text_aa_rgb(px + SP_LG, py + SP_SM + 16, row1.as_str(), TEXT_PRIMARY, &k_assets::FONT_UI_14);
        // Row 3 — vector address (mono).
        let mut row2 = TextBuf::<24>::new();
        row2.push_dec(node.vector.l4 as u64);
        row2.push_str(".");
        row2.push_dec(node.vector.l3 as u64);
        row2.push_str(".");
        row2.push_dec(node.vector.l2 as u64);
        row2.push_str(".");
        row2.push_dec(node.vector.offset as u64);
        k_fb::draw_text_aa_rgb(px + SP_LG, py + SP_SM + 36, row2.as_str(), TEXT_SECONDARY, &k_assets::FONT_MONO_13);

        // Right side — node-type pill.
        let type_label = match node.node_type {
            RuntimeNodeType::Hardware => "HW",
            RuntimeNodeType::Driver => "DRV",
            RuntimeNodeType::Service => "SVC",
            RuntimeNodeType::PluginEntry => "PE",
            RuntimeNodeType::Compute => "CPU",
            RuntimeNodeType::Router => "RTR",
            RuntimeNodeType::Aggregator => "AGG",
            RuntimeNodeType::Vector => "VEC",
        };
        let pill_w = type_label.len() * 10 + 2 * SP_MD;
        let pill_x = px + PANEL_W - pill_w - SP_MD;
        let pill_y = py + SP_SM;
        k_fb::fill_round_rect_alpha(pill_x, pill_y, pill_w, CHIP_H, RADIUS_SM, ACCENT_HOT, 110);
        k_fb::draw_text_aa_rgb(pill_x + SP_MD, pill_y + 1, type_label, ACCENT_HOT, &k_assets::FONT_MONO_13);
    }

    // Header (I.4.4 refresh): brand chip on the left, three count
    // sections separated by ASCII bar glyphs.  Brand uses the brighter
    // foreground; counts use foreground; separators use DimWhite so
    // the eye sees the count numbers as the data.
    //
    //   GOS-KRN | 25 NOD | 63 EDG | G147
    //
    // 8 px font × 40 chars = 320 px exactly; the layout caps at 38
    // chars to leave a 2-char right margin.
    // I.10 — compacter header that yields the right edge to the
    // I.10.2 uptime + heartbeat widget.  Three fixed-width chips,
    // no generation number (use `gen` command for that).
    // N.17 — brand + stats moved into the consolidated bottom strip.
    // No in-viewport header chrome.  Viewport now spans full top-to-
    // FOOTER_Y, giving the 3D scene the full canvas.
    let _ = (returned_n, snapshot.edge_count); // consumed by paint_command_bar

    // ── I.4.5 — edge-style legend strip ──────────────────────────────
    // Bottom-left of the scene area, just above the footer hairline.
    // Four miniature edge samples teach the user the colour+pattern
    // language without consuming a separate help screen.  Each chip:
    // 14-px-wide sample line, then a 3-char label.
    {
        let chip_y = FOOTER_Y as usize - 10;
        let chip_baseline_y = FOOTER_Y as usize - 11;
        let sample_w: i32 = 14;
        let chip_pitch: i32 = sample_w + 4 * 8 + 4;
        let chips: [(EdgeStyle, k_fb::Color, &str); 4] = [
            (EdgeStyle::DoubleSolid, k_fb::Color::NodeDriver, "MNT"),
            (EdgeStyle::Dashed, k_fb::Color::NodeService, "USE"),
            (EdgeStyle::SolidPulsed, k_fb::Color::NodeApp, "SIG"),
            (EdgeStyle::GradientEnds, k_fb::Color::Highlight, "LNK"),
        ];
        let mut cx: i32 = 4;
        for &(style, color, label) in &chips {
            // Sample mini-edge from (cx, chip_y+3) to (cx+sample_w, chip_y+3).
            let x0 = cx;
            let x1 = cx + sample_w;
            let y_line = chip_y as i32 + 3;
            let step = core::cell::Cell::new(0i32);
            match style {
                EdgeStyle::DoubleSolid => {
                    k_rast::draw_line(
                        |x, y| if x >= 0 && x < SCENE_WIDTH && y >= 0 && y < SCENE_HEIGHT {
                            k_fb::put_pixel(x as usize, y as usize, color);
                        },
                        x0, y_line, x1, y_line,
                    );
                    k_rast::draw_line(
                        |x, y| if x >= 0 && x < SCENE_WIDTH && y >= 0 && y < SCENE_HEIGHT {
                            k_fb::put_pixel(x as usize, y as usize, color);
                        },
                        x0, y_line + 1, x1, y_line + 1,
                    );
                }
                EdgeStyle::Dashed => {
                    k_rast::draw_line(
                        |x, y| {
                            let t = step.get();
                            step.set(t + 1);
                            if (t & 0x03) < 2
                                && x >= 0 && x < SCENE_WIDTH
                                && y >= 0 && y < SCENE_HEIGHT
                            {
                                k_fb::put_pixel(x as usize, y as usize, color);
                            }
                        },
                        x0, y_line, x1, y_line,
                    );
                }
                EdgeStyle::SolidPulsed => {
                    let draw_color = if pulse_on { color } else { k_fb::Color::DimWhite };
                    k_rast::draw_line(
                        |x, y| if x >= 0 && x < SCENE_WIDTH && y >= 0 && y < SCENE_HEIGHT {
                            k_fb::put_pixel(x as usize, y as usize, draw_color);
                        },
                        x0, y_line, x1, y_line,
                    );
                }
                EdgeStyle::GradientEnds => {
                    let q = sample_w / 4;
                    k_rast::draw_line(
                        |x, y| {
                            let t = step.get();
                            step.set(t + 1);
                            if x >= 0 && x < SCENE_WIDTH && y >= 0 && y < SCENE_HEIGHT {
                                let c = if t < q || t > sample_w - q { color } else { k_fb::Color::DimWhite };
                                k_fb::put_pixel(x as usize, y as usize, c);
                            }
                        },
                        x0, y_line, x1, y_line,
                    );
                }
                _ => {
                    k_rast::draw_line(
                        |x, y| if x >= 0 && x < SCENE_WIDTH && y >= 0 && y < SCENE_HEIGHT {
                            k_fb::put_pixel(x as usize, y as usize, color);
                        },
                        x0, y_line, x1, y_line,
                    );
                }
            }
            k_fb::draw_text((cx + sample_w + 3) as usize, chip_baseline_y, label, k_fb::Color::Foreground);
            cx += chip_pitch;
        }
    }

    // ── I.4.6 — hover tooltip ────────────────────────────────────────
    // 1-line label near the cursor when hovering an unselected node.
    // Shows `<plugin> · <TYPE>` so the user can read the node id
    // without committing to a click.  Renders with a thin background
    // box for legibility against any cube colour.
    if hover_slot >= 0 && hover_slot != selected {
        let i = hover_slot as usize;
        let node = &nodes[i];
        let type_label = match node.node_type {
            RuntimeNodeType::Hardware => "HW",
            RuntimeNodeType::Driver => "DRV",
            RuntimeNodeType::Service => "SVC",
            RuntimeNodeType::PluginEntry => "PE",
            RuntimeNodeType::Compute => "CPU",
            RuntimeNodeType::Router => "RTR",
            RuntimeNodeType::Aggregator => "AGG",
            RuntimeNodeType::Vector => "VEC",
        };
        let trimmed = node.plugin_name.strip_prefix("K_").unwrap_or(node.plugin_name);
        let trimmed_bytes = trimmed.as_bytes();
        let name_len = trimmed_bytes.len().min(8);
        let name_str = unsafe { core::str::from_utf8_unchecked(&trimmed_bytes[..name_len]) };
        let mut tip = TextBuf::<20>::new();
        tip.push_str(name_str);
        tip.push_str(" ");
        tip.push_str(type_label);
        let txt = tip.as_str();
        let txt_w = (txt.len() * 8) as i32;
        let txt_h: i32 = 10;
        // Place 8 px to the right and above the cursor; flip side if
        // it would clip past the right edge.
        let mut tx = mouse_x + 10;
        let mut ty = mouse_y - txt_h - 4;
        if tx + txt_w + 4 > SCENE_WIDTH {
            tx = mouse_x - txt_w - 6;
        }
        if ty < HEADER_H + 2 {
            ty = mouse_y + 8;
        }
        if tx >= 0 && ty >= HEADER_H && tx + txt_w + 4 < SCENE_WIDTH && ty + txt_h < FOOTER_Y {
            k_fb::fill_rect(tx as usize, ty as usize, (txt_w + 4) as usize, txt_h as usize, k_fb::Color::HeaderBar);
            k_fb::stroke_rect(tx as usize, ty as usize, (txt_w + 4) as usize, txt_h as usize, k_fb::Color::DimWhite);
            k_fb::draw_text((tx + 2) as usize, (ty + 1) as usize, txt, k_fb::Color::Foreground);
        }
    }

    // I.5 — scene-bottom hairline (was the old footer hairline).
    k_fb::hline(0, FOOTER_Y as usize, k_fb::WIDTH, k_fb::Color::DimWhite);
    let _ = snapshot; // RDY/SIG counters return when there's row space
    let _ = auto_on;
}

// ── Phase I.6.3 — Verlet physics for the node/edge graph ──────────
//
// The boot UI's nodes are no longer fixed at their grid positions —
// they're now particles in a soft mass-spring system.  Each frame:
//   1. Verlet integrate: pos += (pos - prev_pos) * damping
//   2. Each node is pulled gently back toward its grid "home"
//      (anchor spring) so the overall layout stays recognizable.
//   3. Each edge is a Hooke spring between its endpoints with a
//      rest length matching the grid spacing.  Stretched edges
//      pull their endpoints together; compressed ones push apart.
//   4. Flash counters decay (used by I.6.4 to render selection
//      pings as a specular boost on the metallic sphere).
//
// Stability tuning:
//   ANCHOR_K = 0.05   weak: lets the graph deform but never drift
//   EDGE_K   = 0.06   moderate: ropes have visible tension flex
//   DAMPING  = 0.86   below 1 always so kinetic energy bleeds off
//
// On graph-generation change (new nodes added via Cypher) we
// re-seed: every node snaps to its grid home and history resets.

const PHYS_NODES: usize = MAX_NODES;
// M.4.b — physics frozen; constants kept for the N.x adaptive
// force-atlas pass which will re-enable spring relaxation.
#[allow(dead_code)] const PHYS_REST_LEN: f32 = 0.45;
#[allow(dead_code)] const PHYS_ANCHOR_K: f32 = 0.05;
#[allow(dead_code)] const PHYS_EDGE_K: f32 = 0.06;
#[allow(dead_code)] const PHYS_DAMPING: f32 = 0.86;

struct PhysicsState {
    pos: [Vec3; PHYS_NODES],
    prev_pos: [Vec3; PHYS_NODES],
    home: [Vec3; PHYS_NODES],
    /// Per-node "ping" timer.  Set by `physics_flash` when the user
    /// hovers or selects a node; decays one frame at a time and
    /// drives the specular boost in the sphere shader.
    flash: [u8; PHYS_NODES],
    node_count: usize,
    seeded: bool,
}

impl PhysicsState {
    const fn new() -> Self {
        Self {
            pos: [Vec3 { x: 0.0, y: 0.0, z: 0.0 }; PHYS_NODES],
            prev_pos: [Vec3 { x: 0.0, y: 0.0, z: 0.0 }; PHYS_NODES],
            home: [Vec3 { x: 0.0, y: 0.0, z: 0.0 }; PHYS_NODES],
            flash: [0; PHYS_NODES],
            node_count: 0,
            seeded: false,
        }
    }
}

static PHYSICS: spin::Mutex<PhysicsState> = spin::Mutex::new(PhysicsState::new());

/// Compute the rigid "home" grid position for the i-th of `total`
/// nodes.  Pre-I.6.3 this was the per-frame position; now it's the
/// anchor that the spring system relaxes toward.
fn node_home_position(i: usize, total: usize) -> Vec3 {
    let cols = isqrt_ceil(total).max(1);
    let row = (i / cols) as i32;
    let col = (i % cols) as i32;
    let layer = (i % 3) as i32 - 1;
    let cols_i = cols as i32;
    let span_x = (col - cols_i / 2) as f32 * 0.45;
    let span_z = (row - cols_i / 2) as f32 * 0.45;
    let span_y = layer as f32 * 0.18;
    Vec3::new(span_x, span_y, span_z)
}

/// N.13 — Unity-style "Frame All" / "F-key recenter".  Computes a
/// bounding sphere of the requested point set + sets the camera
/// orbit radius so the whole bound fits inside the vertical FoV with
/// 25 % margin.  Resets yaw/pitch bias (canonical default view).
///
/// Called at boot for the "show the complete graph by default"
/// presentation, and bound to the F-key for live recentring.
pub fn frame_all_nodes(node_total: usize) {
    use core::sync::atomic::Ordering;
    let n = node_total.min(MAX_NODES);
    if n == 0 { return; }

    // Centroid + max-radial-distance bounding sphere.
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    let mut cz = 0.0f32;
    for i in 0..n {
        let p = node_home_position(i, n);
        cx += p.x; cy += p.y; cz += p.z;
    }
    let inv_n = 1.0 / n as f32;
    cx *= inv_n; cy *= inv_n; cz *= inv_n;
    let mut max_r2: f32 = 0.0;
    for i in 0..n {
        let p = node_home_position(i, n);
        let dx = p.x - cx;
        let dy = p.y - cy;
        let dz = p.z - cz;
        let r2 = dx * dx + dy * dy + dz * dz;
        if r2 > max_r2 { max_r2 = r2; }
    }
    let bound_r = libm::sqrtf(max_r2) + NODE_HALF * 1.5;

    // N.17 — fit camera to the *effective* scene window, not the full
    // canvas.  The painter clips body geometry to [HEADER_H, FOOTER_Y]
    // which is only ~72 % of HEIGHT — the previous fit ignored that
    // and let nodes near the vertical edges get cropped by chrome.
    // Now: compute the reduced vertical FOV the scene clip actually
    // shows, then pad with a 1.6× margin so the floating gizmo + stats
    // chip + detail card don't sit on top of any node.
    let half_fov_v = 30.0f32.to_radians();
    let scene_frac = (FOOTER_Y - HEADER_H) as f32 / SCENE_HEIGHT as f32;
    let effective_tan = libm::tanf(half_fov_v) * scene_frac;
    let fit_dist = bound_r / effective_tan * 1.6;
    let radius_mm = ((fit_dist * 1000.0) as i32).max(1500);

    k_fb::CAMERA_RADIUS_MM.store(radius_mm, Ordering::Relaxed);
    k_fb::CAMERA_YAW_BIAS_MRAD.store(0, Ordering::Relaxed);
    k_fb::CAMERA_PITCH_BIAS_MRAD.store(0, Ordering::Relaxed);
    k_fb::CAMERA_AUTO_ROTATE.store(false, Ordering::Relaxed);
}

/// N.17 — chunky pixel-style boot splash.  Runs once at boot before
/// the main paint loop starts; renders ~60 animation frames showing
/// "GOS" in 8×-scaled BIOS 8×8 font (so each font pixel is a 8×8
/// physical block, total letter = 64×64 logical = 384×384 native at
/// 6× HD scale).  Three phases:
///   * t ∈ [0, 0.30]  fade in    (alpha 0 → 255)
///   * t ∈ [0.30, 0.75] hold      (full)
///   * t ∈ [0.75, 1.00] fade out  (255 → 0)
///
/// Letters render in white over a deep-space vertical gradient with
/// a small tagline below in Noto Sans SC 14.  The whole splash
/// finishes well under 2 s under TCG and serves as both a "GOS
/// brand" beat and a "kernel is alive" reassurance before the 3D
/// scene paints for the first time.
pub fn boot_splash() {
    use design::*;
    const N_FRAMES: u32 = 60;
    for f in 0..N_FRAMES {
        let t = f as f32 / (N_FRAMES - 1) as f32;
        paint_splash_frame(t);
        k_fb::present();
    }
}

fn paint_splash_frame(t: f32) {
    use design::*;
    // Deep gradient background.
    k_fb::clear_gradient_vertical(SURFACE_0_TOP, SURFACE_0_BOTTOM);

    // Three-phase alpha envelope.
    let alpha_f = if t < 0.30 {
        t / 0.30
    } else if t < 0.75 {
        1.0
    } else {
        ((1.0 - t) / 0.25).max(0.0)
    };
    let alpha = (alpha_f * 255.0) as u8;
    if alpha == 0 { return; }

    // Big GOS letters — BIOS 8×8 font scaled 5× so each letter is
    // 40×40 logical = 240×240 native at HD 6× scale (chunky pixel).
    let scale = 5usize;
    let letter_w = 8 * scale; // 40 logical px
    let letter_h = 8 * scale;
    let spacing = 6usize;
    let total_w = 3 * letter_w + 2 * spacing;
    let start_x = (k_fb::WIDTH - total_w) / 2;
    let y_brand = (k_fb::HEIGHT - letter_h) / 2 - 8;

    for (i, ch) in "GOS".chars().enumerate() {
        let x = start_x + i * (letter_w + spacing);
        // Subtle hue cycle per letter so the brand feels "alive".
        let hue = (t * 0.5 + i as f32 * 0.22) % 1.0;
        let color = k_fb::hsv_to_bgrx(hue, 0.45, 1.0);
        k_fb::draw_glyph_pixel_scaled(x, y_brand, ch, scale, color, alpha);
    }

    // Tagline below in AA Noto 14.
    let tag = "graph-theory OS";
    let tag_w = tag.len() * 14;
    let tag_x = (k_fb::WIDTH - tag_w) / 2;
    let tag_y = y_brand + letter_h + 14;
    let tag_color = k_fb::scale_bgrx(TEXT_SECONDARY, alpha_f);
    k_fb::draw_text_aa_rgb(tag_x, tag_y, tag, tag_color, &k_assets::FONT_UI_14);

    // Subtle progress line at the bottom — animates the loading feel.
    let bar_y = k_fb::HEIGHT - 12;
    let bar_w = 120;
    let bar_x = (k_fb::WIDTH - bar_w) / 2;
    let fill_w = ((bar_w as f32) * t.min(1.0)) as usize;
    k_fb::fill_rect_alpha(bar_x, bar_y, bar_w, 2, SURFACE_2_TOP, 200);
    if fill_w > 0 {
        let phase = t * 2.0;
        for px in 0..fill_w {
            let hue = (px as f32 * 0.008 + phase) % 1.0;
            let bgrx = k_fb::hsv_to_bgrx(hue, 0.85, 1.0);
            k_fb::put_pixel_rgb(bar_x + px, bar_y, bgrx);
            k_fb::put_pixel_rgb(bar_x + px, bar_y + 1, bgrx);
        }
    }
}

/// Phase M.4.b — physics frozen.  The earlier Verlet + anchor + edge
/// spring system was over-actuated (edges across the 5×5 grid have
/// chord length ≈ 1.9 but `PHYS_REST_LEN=0.45`, so the springs
/// permanently want to collapse the graph; anchor + damping can't
/// fully settle the resulting oscillation).  Users saw the nodes
/// jiggling instead of presenting a stable, readable layout.
///
/// Until a Force-Atlas / adaptive-rest-length pass lands in N.x we
/// freeze positions at their grid home.  We keep `flash` decay and
/// the (re-)seed logic so click feedback + Cypher LINK mutations
/// still drop new nodes correctly.
fn physics_step(
    _nodes: &[GraphNodeSummary],
    returned_n: usize,
    _edges: &[GraphEdgeSummary],
    _returned_e: usize,
) {
    let mut p = PHYSICS.lock();
    let n = returned_n.min(PHYS_NODES);

    if !p.seeded || p.node_count != n {
        for i in 0..n {
            let h = node_home_position(i, n);
            p.pos[i] = h;
            p.prev_pos[i] = h;
            p.home[i] = h;
            p.flash[i] = 0;
        }
        p.node_count = n;
        p.seeded = true;
        return;
    }

    // Flash decay (I.6.4 click-feedback) is the only per-frame mutation.
    for i in 0..n {
        if p.flash[i] > 0 {
            p.flash[i] -= 1;
        }
    }
}

/// Trigger a selection/click "ping" on the given node — boosts the
/// metallic sphere's specular highlight for several frames so the
/// click reads as a tactile flash.
fn physics_flash(node_idx: usize) {
    if node_idx >= PHYS_NODES {
        return;
    }
    PHYSICS.lock().flash[node_idx] = 14;
}

/// Read the current flash counter for the i-th node.  Returns 0 if
/// out of range — caller treats that as "no boost".
fn physics_flash_value(i: usize) -> u8 {
    PHYSICS.lock().flash.get(i).copied().unwrap_or(0)
}

/// Read a snapshot of the i-th node's simulated position.
fn node_world_position(i: usize, _total: usize) -> Vec3 {
    PHYSICS.lock().pos[i.min(PHYS_NODES - 1)]
}

/// Original grid-derived position (pre-physics).  Kept for any
/// callsites that intentionally want the rigid layout.  Currently
/// unused in render but referenced by the physics seed.
#[allow(dead_code)]
fn node_world_position_grid(i: usize, total: usize) -> Vec3 {
    node_home_position(i, total)
}

/// World-space centre for the i-th node — kept as the legacy entry
/// for the original grid formula.  Now superseded by the physics
/// state above; left here in case future code wants to ask for the
/// rigid layout.
#[allow(dead_code)]
fn node_world_position_legacy(i: usize, total: usize) -> Vec3 {
    let cols = isqrt_ceil(total).max(1);
    let row = (i / cols) as i32;
    let col = (i % cols) as i32;
    let layer = (i % 3) as i32 - 1; // -1, 0, +1
    let cols_i = cols as i32;
    let span_x = (col - cols_i / 2) as f32 * 0.45;
    let span_z = (row - cols_i / 2) as f32 * 0.45;
    let span_y = layer as f32 * 0.18;
    Vec3::new(span_x, span_y, span_z)
}

fn isqrt_ceil(n: usize) -> usize {
    let mut s: usize = 1;
    while s * s < n {
        s += 1;
    }
    s
}

// ── Node shape: octahedral crystal core (I.4.1) ────────────────────
//
// We previously rendered every node as a small cube (8 verts / 12
// triangles).  Switching to a regular octahedron (6 verts / 8 tris)
// is cheaper to rasterise AND gives every node a faceted "crystal"
// silhouette that reads as a graph-vertex rather than a building.
// Each face is a single triangle, so the 8-step Lambertian shading
// produces sharper highlight/shadow contrast — more sci-fi, less
// blockprint.
//
// Octahedron half-extent slightly larger than the old cube's so the
// projected on-screen footprint is comparable.

// N.8.b — bumped from 0.13 to 0.17 so spheres render at hero size
// in HD (~35-50 px radius after 4× upscale) and the PBR shader
// detail (micro-bump streaks, anisotropic highlight, fresnel rim,
// cubemap reflection) becomes legible.
const NODE_HALF: f32 = 0.17;

// (Octahedron facet arrays removed in I.6.1 — sphere LOD replaces them.)

/// Sci-fi octahedral crystal draw (I.4.1+I.4.2): per-face Lambertian
/// shading pulls a slot from the node's 8-step hue ramp; depth-fog
/// then biases that slot down for nodes near the far plane so the
/// scene reads in spatial layers.  Rim outline uses the brightest
/// shade so silhouettes pop against the dark background.
///
/// `fog`: 0.0 = full strength (near camera), 1.0 = fully faded
/// (at the far plane).  Subtracts up to ~4 shade slots so the
/// farthest nodes still draw but recede visually.
// ── Phase I.6.1 — metallic-sphere LOD renderer ─────────────────────
//
// Replaces the octahedral facets with per-pixel sphere shading
// + screen-space LOD.  Each node is a metallic ball lit by a fixed
// key light; the shader runs Lambert diffuse + a tight Phong
// specular highlight and maps the final intensity into the node's
// per-hue 8-slot ramp (the same ramps used by all previous
// versions, so the palette doesn't change).
//
// Three LOD tiers based on the projected screen radius `r_px`:
//
//   LOD_HIGH   r_px >= 6   — full per-pixel sphere with specular
//   LOD_MID    r_px >= 3   — 5×5 disc with Lambert ramp, no specular
//   LOD_LOW    r_px  < 3   — 2-pixel dot in the node's rim shade
//
// The caller pre-computes the screen-space centre and radius from
// the projected world centre + a unit-scaled offset so we don't
// need view_proj here (also lets the physics step move the node
// without a re-project per LOD pass).
//
// The "metal" feel comes from:
//   * a high specular exponent (32) → a tight bright spot
//   * the spot is biased toward the brightest shade slot (7)
//   * background ambient stays low (AMBIENT = 0.10) → high contrast
//
// `fog`: 0.0 near, 1.0 far — biases the shade index down so distant
//   balls recede into the nebula background.

// Phase M — multi-light setup for AAA-equivalent shading on the
// 256-color framebuffer.  Three lights composite per pixel:
//   * KEY    主光 — 标准 PBR specular + diffuse, high intensity
//   * FILL   补光 — opposite-side soft diffuse, fills shadows
//   * SKY    天光 — upward bounce, prevents bottom darkening
const LIGHT_DIR: Vec3 = Vec3 { x: 0.55, y: 0.72, z: -0.42 };
const FILL_LIGHT_DIR: Vec3 = Vec3 { x: -0.40, y: 0.20, z: 0.30 };
const SKY_LIGHT_DIR: Vec3 = Vec3 { x: 0.00, y: 1.00, z: 0.00 };
const KEY_INTENSITY: f32 = 1.0;
const FILL_INTENSITY: f32 = 0.32;
const SKY_INTENSITY: f32 = 0.18;

/// Phase M.1 — Bayer 4×4 ordered dither matrix.  Each cell is in
/// [0..16); when added to a per-pixel intensity-fraction, decides
/// whether to round up or down to the nearest palette ramp slot.
/// Eliminates banding on smooth sphere gradients without needing an
/// HDR backbuffer.
// N.9 — superseded by direct 24-bit RGB output in the sphere shader,
// but retained because future 2-D UI shading (dithered gradients,
// stylised text effects) will want the matrix back.
#[allow(dead_code)]
const BAYER_4X4: [[u8; 4]; 4] = [
    [ 0,  8,  2, 10],
    [12,  4, 14,  6],
    [ 3, 11,  1,  9],
    [15,  7, 13,  5],
];

/// Pick a shade slot 0..=7 from a continuous intensity in [0, 1] using
/// the Bayer dither matrix at the given pixel position.  This is the
/// single point where the AAA-style continuous shading meets the
/// 256-color palette quantization.
#[inline]
#[allow(dead_code)]
fn dither_shade(intensity: f32, x: i32, y: i32) -> u8 {
    let scaled = (intensity.clamp(0.0, 1.0) * 7.999_99) as f32;
    let base = libm::floorf(scaled);
    let frac = scaled - base;
    // Bayer cell in [0, 1).
    let bx = (x.rem_euclid(4)) as usize;
    let by = (y.rem_euclid(4)) as usize;
    let threshold = (BAYER_4X4[by][bx] as f32 + 0.5) / 16.0;
    let shade = if frac >= threshold { base as u8 + 1 } else { base as u8 };
    shade.min(7)
}
const AMBIENT: f32 = 0.10;

/// PBR material — drives the sphere shader's GGX response per node.
/// Different sub-domains map to different material profiles so the
/// scene reads as a mix of polished chrome (driver), satin (service),
/// matte (vector) etc. — the eye picks up the architectural partition
/// even before reading labels.
#[derive(Debug, Clone, Copy)]
struct PbrMaterial {
    /// 0 = dielectric, 1 = pure metal.  Drives base-reflectance F0
    /// and how much diffuse contribution survives.
    metallic: f32,
    /// 0 = mirror, 1 = matte.  Drives GGX lobe width.
    roughness: f32,
    /// Schlick rim strength multiplier.  Higher = more pronounced
    /// silhouette glow.
    rim: f32,
    /// M.2 — micro-surface normal perturbation amplitude.  0 = polished
    /// (no perturbation, mirror-smooth gradient), higher values
    /// modulate the per-pixel normal via `sin(nx*K) * cos(ny*K) * amp`
    /// to fake brushed / satin / matte textures.  Combined with the
    /// `roughness` GGX widening this gives each material class a
    /// distinct surface "feel" instead of every ball looking like the
    /// same plastic.
    micro_bump_amp: f32,
    /// M.2 — frequency of the micro_bump perturbation.  Higher = finer
    /// pattern (brushed steel); lower = larger pebbled feel.
    micro_bump_freq: f32,
    /// M.4 — anisotropic specular stretch.  0 = isotropic (default
    /// circular highlight); +1 = highlight stretched horizontally
    /// (sphere's world-X axis); −1 = stretched vertically.  Mimics
    /// brushed metal where the highlight runs perpendicular to the
    /// brushing direction.  Most natural at moderate amplitudes
    /// (±0.4..0.7).
    anisotropy: f32,
}

fn material_for_sub_domain(sub: gos_protocol::NodeSubDomain) -> PbrMaterial {
    use gos_protocol::NodeSubDomain;
    match sub {
        NodeSubDomain::Hardware =>
            PbrMaterial {
                metallic: 0.95, roughness: 0.18, rim: 1.2,
                micro_bump_amp: 0.00, micro_bump_freq:  0.0,
                anisotropy:  0.00,                              // polished mirror — isotropic
            },
        NodeSubDomain::KernelDriver =>
            PbrMaterial {
                metallic: 0.92, roughness: 0.25, rim: 1.0,
                micro_bump_amp: 0.025, micro_bump_freq: 32.0,
                anisotropy:  0.65,                              // brushed chrome — horizontal streak
            },
        NodeSubDomain::Service =>
            PbrMaterial {
                metallic: 0.80, roughness: 0.38, rim: 0.85,
                micro_bump_amp: 0.018, micro_bump_freq: 22.0,
                anisotropy:  0.30,                              // satin — mild stretch
            },
        NodeSubDomain::Compute =>
            PbrMaterial {
                metallic: 0.88, roughness: 0.30, rim: 1.05,
                micro_bump_amp: 0.030, micro_bump_freq: 28.0,
                anisotropy: -0.55,                              // brushed deeper — vertical streak
            },
        NodeSubDomain::Routing =>
            PbrMaterial {
                metallic: 0.70, roughness: 0.45, rim: 0.90,
                micro_bump_amp: 0.022, micro_bump_freq: 18.0,
                anisotropy:  0.20,                              // anodized — soft horizontal
            },
        NodeSubDomain::Vector =>
            PbrMaterial {
                metallic: 0.40, roughness: 0.65, rim: 0.70,
                micro_bump_amp: 0.055, micro_bump_freq: 14.0,
                anisotropy:  0.00,                              // matte — no directional sheen
            },
    }
}

/// N.8 — baked-cubemap environment sampler.  The 6-face cubemap is
/// pre-rendered offline by `tools/assets/generate.py` (path-traced
/// "kernel nebula" sky with a key-light sun lobe + cyan zenith + warm
/// horizon) and embedded into the kernel binary via `k-assets`.
///
/// `k_assets::sample_cubemap` returns a palette index; we map it to
/// a 0..1 luminance using k-fb's BT.601 LUT and add a small additive
/// sun contribution so polished spheres still pick up a tight specular
/// reflection of the key light even when the cubemap quantization
/// loses that detail.
fn sample_environment(reflection: Vec3) -> f32 {
    let idx = k_assets::sample_cubemap([reflection.x, reflection.y, reflection.z]);
    let cube = (k_fb::palette_luminance(idx) as f32) / 255.0;
    // Tight sun lobe layered on top so polished chrome catches the
    // primary highlight reflection even after palette quantization.
    let r_dot_l = reflection.dot(LIGHT_DIR).max(0.0);
    let sun = pow_approx(r_dot_l, 24.0) * 0.6;
    (cube + sun).clamp(0.0, 1.5)
}

/// N.8 — ACES filmic tone-mapping (Narkowicz 2015 approximation).
/// Squashes the HDR composite from `[0, 1.5+]` into perceptually-
/// uniform `[0, 1]` with a soft highlight rolloff so over-exposed
/// pixels don't all clip flat to white — they retain colour
/// information up to the limit of the 8-shade ramp.  Cheap: 5 muls,
/// 2 adds, 1 div, then the existing Bayer dither.
#[inline]
fn aces_tonemap(x: f32) -> f32 {
    let a = 2.51_f32; let b = 0.03_f32;
    let c = 2.43_f32; let d = 0.59_f32; let e = 0.14_f32;
    ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
}

/// GGX/Trowbridge-Reitz normal-distribution function.  Standard PBR
/// microfacet model — produces the characteristic tight-hot-spot of
/// real reflective materials with smooth falloff (vs Phong's hard
/// edge).  alpha = roughness² is the canonical mapping.
fn ggx_d(n_dot_h: f32, roughness: f32) -> f32 {
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let denom = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    alpha2 / (denom * denom + 0.001)
}

/// Schlick fresnel approximation.  F0 is the base reflectance at
/// normal incidence; cos_theta is the angle between view and half-
/// vector.  Drives both specular boost at glancing angles and the
/// signature "fresnel rim" silhouette glow of PBR materials.
fn schlick_fresnel(f0: f32, cos_theta: f32) -> f32 {
    f0 + (1.0 - f0) * pow_approx(1.0 - cos_theta, 5.0)
}

fn draw_node_sphere(
    sess: &mut k_fb::PaintSession,
    sx: i32,
    sy: i32,
    r_px: i32,
    hue_base: u8,
    fog: f32,
    specular_boost: f32,
    material: PbrMaterial,
) {
    if r_px < 1 {
        return;
    }
    let fog_bias = (fog.clamp(0.0, 1.0) * 4.0) as u8;

    // ── LOD_FAR (r ≤ 3): AA disc, no PBR ──
    //
    // N.14 — replace the old "4-pixel chunk" with a true coverage-AA
    // disc.  For each pixel within radius+1, compute the analytic
    // fraction of the unit-pixel that overlaps the sphere disc; use
    // it as the alpha to blend the hue colour over the existing scene.
    // No shading variation — distant spheres can't show PBR detail —
    // but they DON'T look like dithered chunks either.
    if r_px <= 3 {
        let r_f = r_px as f32;
        let r_plus = r_px + 1;
        let core_shade = 7u8.saturating_sub(fog_bias).min(7) as f32 / 7.0;
        let bgrx_core = pack_hue_intensity(hue_base, core_shade);
        for dy in -r_plus..=r_plus {
            let py = sy + dy;
            if py < HEADER_H || py >= FOOTER_Y { continue; }
            for dx in -r_plus..=r_plus {
                let px = sx + dx;
                if px < 0 || px >= SCENE_WIDTH { continue; }
                // Distance from disc centre; sub-pixel coverage at the
                // boundary  (see "filled circle aa" — Crow's analytic disc).
                let d = libm::sqrtf((dx * dx + dy * dy) as f32);
                let cover = (r_f + 0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 { continue; }
                let alpha = (cover * 255.0) as u8;
                sess.blend(px as usize, py as usize, bgrx_core, alpha);
            }
        }
        return;
    }

    // ── LOD_MID and LOD_HIGH: full PBR shader ──
    //
    // N.14 LOD knobs (cheaper paths for smaller spheres):
    //   r_px ≥  6  → specular lobe enabled       (do_specular)
    //   r_px ≥  8  → micro-bump trig enabled     (gate at site)
    //   r_px ≥ 10  → environment reflection      (cubemap sample)
    //   r_px ≥ 14  → anisotropic specular extra  (cheap, threshold low)
    //   r_px ≥ 16  → rim Schlick fresnel        (always cheap, gate not needed)
    let do_specular = r_px >= 6;
    let do_env      = r_px >= 10;
    let r_f = r_px as f32;
    let r2 = r_f * r_f;

    // PBR setup.  The screen-space sphere normal has positive Z
    // (pointing out of the screen, toward the camera), so the view
    // vector FROM-surface-TO-camera is (0, 0, +1).  Half-vector is
    // light + view, renormalised.
    let view = Vec3::new(0.0, 0.0, 1.0);
    let half = Vec3::new(LIGHT_DIR.x, LIGHT_DIR.y, LIGHT_DIR.z + view.z).normalize();

    // Base reflectance F0: dielectrics ≈ 0.04, metals tint with hue.
    // For colored metals we still drive a single scalar here and rely
    // on the per-hue palette ramp to tint the result.
    let f0 = 0.04 * (1.0 - material.metallic) + 0.95 * material.metallic;

    // Sun colour energy — calibrated to make the highlight punchy.
    let sun_energy = 1.85;

    // N.14 — iterate one pixel past the radius so the AA rim ring gets
    // coverage-blended into the underlying scene.  Inside the disc
    // (d² < r²) we paint with full alpha; on the ~1-px boundary
    // (r² ≤ d² < (r+1)²) we compute the analytic disc-pixel overlap
    // and alpha-blend.  Hard pixel jumps at the sphere silhouette → gone.
    let r_outer = r_px + 1;
    let r_outer_2 = (r_f + 1.0) * (r_f + 1.0);
    for dy in -r_outer..=r_outer {
        let py = sy + dy;
        if py < HEADER_H || py >= FOOTER_Y {
            continue;
        }
        let dy_f = dy as f32;
        let row_outside = r_outer_2 - dy_f * dy_f;
        if row_outside <= 0.0 {
            continue;
        }
        let row_outer_half = libm::sqrtf(row_outside) as i32;
        for dx in -row_outer_half..=row_outer_half {
            let px = sx + dx;
            if px < 0 || px >= SCENE_WIDTH {
                continue;
            }
            let dx_f = dx as f32;
            let d2 = dx_f * dx_f + dy_f * dy_f;
            // Coverage: 1.0 inside, smooth taper to 0.0 between r and r+1.
            // For pixels well inside the disc (d <= r - 0.5) coverage is
            // saturated; the rim AA only kicks in for the outer ring.
            let d = libm::sqrtf(d2);
            let coverage = (r_f + 0.5 - d).clamp(0.0, 1.0);
            if coverage <= 0.0 { continue; }
            // Use a slightly inflated z so the surface normal at the
            // very rim stays meaningful (avoids NaN from sqrt(0)).
            let z2 = (r2 - d2).max(0.0001);
            let nx = dx_f / r_f;
            let ny = dy_f / r_f;
            let nz = libm::sqrtf(z2) / r_f;
            // Y is screen-down → flip so up-light reads correctly.
            // M.2 — micro-surface normal perturbation.  Each material
            // declares amp + freq; we modulate the X/Y components of
            // the normal by a 2-axis sin/cos product, then renormalise.
            // Effect: brushed metals show stripey reflections; matte
            // surfaces have a pebbled diffusion; polished stays
            // mirror-smooth.
            //
            // LOD gate: 4 trig calls per pixel × ~2000 pixels per
            // sphere × ~25 spheres = 200000 trig/frame.  Under TCG
            // that's ~2 seconds per frame, which leaves the user
            // staring at the `clear()` background between paints.
            // For r_px < 8 the texture is sub-pixel anyway so skip
            // the perturbation — only the "hero" foreground spheres
            // get the brushed/satin/matte feel.
            let normal = if material.micro_bump_amp > 0.0 && r_px >= 8 {
                let f = material.micro_bump_freq;
                let bx = libm::sinf(nx * f) * libm::cosf(ny * f * 0.7) * material.micro_bump_amp;
                let by = libm::cosf(nx * f * 0.6) * libm::sinf(ny * f) * material.micro_bump_amp;
                Vec3::new(nx + bx, -ny + by, nz).normalize()
            } else {
                Vec3::new(nx, -ny, nz)
            };

            // ── M.1 — three-light Lambertian + GGX setup ──
            let n_dot_l_key  = normal.dot(LIGHT_DIR).max(0.0);
            let n_dot_l_fill = normal.dot(FILL_LIGHT_DIR).max(0.0);
            let n_dot_l_sky  = normal.dot(SKY_LIGHT_DIR).max(0.0);
            let n_dot_v = normal.dot(view).max(0.001);

            // Diffuse — sum of three lights, each scaled by intensity
            // and metallic damping.  Multi-light fills shadows naturally,
            // killing the "single-direction lit" look of single-light
            // PBR demos.
            let diffuse = (n_dot_l_key * KEY_INTENSITY
                        + n_dot_l_fill * FILL_INTENSITY
                        + n_dot_l_sky * SKY_INTENSITY)
                        * (1.0 - material.metallic);

            // Specular only from the key light (fill/sky are too soft
            // to make a coherent highlight).  PBR specular (GGX × Schlick).
            // M.4 — anisotropic stretch modulates the lobe by a factor
            // that depends on the half-vector's alignment with the
            // material's "brushing axis" (X for positive anisotropy,
            // Y for negative).  Highlight gets compressed along the
            // brushing direction → reads as a streak.
            let mut spec = 0.0_f32;
            if do_specular {
                let n_dot_h = normal.dot(half).max(0.001);
                let v_dot_h = view.dot(half).max(0.001);
                let d = ggx_d(n_dot_h, material.roughness);
                let f = schlick_fresnel(f0, v_dot_h);
                let aniso_factor = if material.anisotropy.abs() < 0.01 {
                    1.0
                } else {
                    // h.x (or h.y) ≈ 1 along the brushing axis; we
                    // attenuate the lobe by 1 - |comp|·|aniso|, so
                    // the streak runs perpendicular to it.
                    let comp = if material.anisotropy >= 0.0 {
                        half.x
                    } else {
                        half.y
                    };
                    let mag = material.anisotropy.abs();
                    let mut a = 1.0 - (comp.abs()) * mag;
                    if a < 0.15 { a = 0.15; } // clamp so streak doesn't kill specular entirely
                    a
                };
                spec = d * f * 0.30 * aniso_factor * (1.0 + specular_boost);
            }

            // Environment reflection (cheap procedural sampler).
            // With view = (0, 0, +1), the reflection of the view ray
            // off the sphere normal is R = 2(N·V)N - V → screen
            // centre reflects back at camera, silhouettes reflect
            // outward to the "horizon".
            // N.14 — env reflection gated by LOD.  cubemap sampling is
            // the most expensive single per-pixel op (it does an axis
            // dot + face select + uv project + palette LUT lookup +
            // luminance fetch).  For r_px < 10 the sub-pixel reflection
            // detail is lost in nearest-neighbour upscale anyway, so
            // skip and use a constant ambient term.
            let env_intensity = if do_env {
                let two_ndotv = 2.0 * normal.dot(view);
                let refl = Vec3::new(
                    two_ndotv * normal.x,
                    two_ndotv * normal.y,
                    two_ndotv * normal.z - 1.0,
                );
                sample_environment(refl)
                    * (1.0 - material.roughness * 0.6)
                    * (0.25 + 0.5 * material.metallic)
            } else {
                // Cheap constant ambient that matches the cubemap's average.
                0.25 * (1.0 - material.roughness * 0.6)
                     * (0.25 + 0.5 * material.metallic)
            };

            // Fresnel rim glow — Schlick on N·V drives an emissive-
            // looking ring around silhouettes.  Always cheap (one
            // pow_approx); kept at all LODs as it is the signature
            // PBR tell.
            let rim = schlick_fresnel(f0 * 0.2, n_dot_v) * material.rim * 0.65;

            let hdr = AMBIENT
                + diffuse
                + spec * sun_energy
                + env_intensity
                + rim;
            let intensity = aces_tonemap(hdr);

            let fog_atten = 1.0 - fog.clamp(0.0, 1.0) * 0.55;
            let bgrx = pack_hue_intensity(hue_base, intensity * fog_atten);

            // N.14 — coverage-blend into the back-buffer.  Interior
            // pixels (coverage = 1.0) blit straight (alpha 255), rim
            // pixels (0 < coverage < 1) alpha-blend over the existing
            // scene background — no jaggies on the silhouette.
            if coverage >= 0.999 {
                sess.put_rgb(px as usize, py as usize, bgrx);
            } else {
                let alpha = (coverage * 255.0) as u8;
                sess.blend(px as usize, py as usize, bgrx, alpha);
            }

            // Additive bloom (skipped for rim AA pixels to keep glow
            // attached to the solid core).
            if coverage >= 0.999 && hdr > 1.05 {
                let glow = (hdr - 1.0).min(0.6);
                let add = pack_hue_intensity(hue_base, glow * 0.55);
                for (gx, gy) in [(px - 1, py), (px + 1, py), (px, py - 1), (px, py + 1)] {
                    if gx >= 0 && gx < SCENE_WIDTH && gy >= HEADER_H && gy < FOOTER_Y {
                        sess.add(gx as usize, gy as usize, add);
                    }
                }
            }
        }
    }
}

/// N.14 — red ⇄ white metallic palette.  User request: "金属球红白
/// 两色".  We collapse the previous 5-hue ramp into a binary scheme
/// driven by the sub-domain's role in the graph:
///   * RED   = "active / power" nodes (Hardware, KernelDriver, Compute)
///   * WHITE = "logical / chrome" nodes (Service, PluginEntry, Router,
///             Aggregator, Vector, Routing)
///
/// Each ball renders as polished metal in its assigned hue; the
/// per-pixel intensity from the PBR shader scales the chromaticity.
///
/// Backing peaks tuned for high-saturation 24-bit:
///   RED   peak BGRX = (255, 60, 70)   — slight orange-cyan kiss for warmth
///   WHITE peak BGRX = (245, 248, 252) — near-platinum with cool tilt
///
/// `hue_base` keeps the legacy u8 protocol (which palette ramp
/// classify_node_hue returned) but is now interpreted as a flag:
/// CYAN/MAGENTA/MINT (16/24/40) → RED, everything else → WHITE.
pub fn ball_is_red(hue_base: u8) -> bool {
    matches!(hue_base,
        k_fb::HUE_CYAN_BASE | k_fb::HUE_MAGENTA_BASE | k_fb::HUE_MINT_BASE)
}

const BALL_RED_PEAK:   (u32, u32, u32) = (255, 60, 70);
const BALL_WHITE_PEAK: (u32, u32, u32) = (245, 248, 252);

fn pack_hue_intensity(hue_base: u8, intensity: f32) -> u32 {
    let (r0, g0, b0) = if ball_is_red(hue_base) {
        BALL_RED_PEAK
    } else {
        BALL_WHITE_PEAK
    };
    let t = intensity.clamp(0.0, 1.0);
    let r = (r0 as f32 * t).min(255.0) as u32;
    let g = (g0 as f32 * t).min(255.0) as u32;
    let b = (b0 as f32 * t).min(255.0) as u32;
    (r << 16) | (g << 8) | b
}

/// N.14 — straight 24-bit ramp keyed by `red`.  Used by the rope
/// renderer for per-half colour (the half nearer a red ball renders
/// white, and vice versa, so the line ends always contrast against
/// their endpoint sphere).
fn pack_red_or_white(red: bool, intensity: f32) -> u32 {
    let (r0, g0, b0) = if red { BALL_RED_PEAK } else { BALL_WHITE_PEAK };
    let t = intensity.clamp(0.0, 1.5);
    let r = (r0 as f32 * t).min(255.0) as u32;
    let g = (g0 as f32 * t).min(255.0) as u32;
    let b = (b0 as f32 * t).min(255.0) as u32;
    (r << 16) | (g << 8) | b
}

// ── Phase N.11 — true cable rope edge renderer ───────────────────
//
// Each edge is sampled at sub-pixel resolution (one stamp per screen
// pixel along the path) and rendered as a 5-pixel-wide profile with:
//
//   * cos²-falloff cross-section  → round cable look (not flat ribbon)
//   * per-channel scaled BGRX     → 24-bit smooth shading (no banding)
//   * sinusoidal braid pattern    → "twisted strands" highlight
//   * additive 1-px outer halo    → soft glow into the void backdrop
//   * catenary sag (Solid family) → gravity pull on a non-rigid wire
//
// Per-style overrides
// ───────────────────
//   Solid          standard 3-px cable
//   Dashed         period 8: 4 on, 4 off (cleaner than the old 2-on/2-off)
//   Dotted         period 12: 2 on, 10 off
//   SolidPulsed    intensity pulse via `pulse_on` (kept for compat,
//                  but the auto-rotate-gated pulse is now solid-on
//                  when the user isn't running the demo)
//   GradientEnds   bright endpoints, dim middle (LNK edges)
//   DoubleSolid    5-px thick rigid mount, no sag
/// N.14 — red ⇄ white per-endpoint gradient rope with sub-pixel AA.
///
/// `from_red` / `to_red` are the booleans from the two endpoint
/// spheres' `ball_is_red()`.  For each point along the rope at
/// parameter t∈[0,1] the rope hue is the OPPOSITE of the endpoint it
/// is closer to: a white ball at the from end ⇒ rope is red there.
/// Equal endpoints (both red / both white) ⇒ rope is solid in the
/// opposite hue.  Mixed endpoints ⇒ rope is a sharp 50/50 split that
/// reads as "this connection runs between two roles".
///
/// `highlight` (carried from the legacy strain check) tints the
/// middle 50 % toward the bright accent so over-strained edges still
/// read as anomalous.
fn draw_rope_edge(
    sess: &mut k_fb::PaintSession,
    fx: i32,
    fy: i32,
    tx: i32,
    ty: i32,
    from_red: bool,
    to_red: bool,
    highlight: bool,
    style: EdgeStyle,
    pulse_on: bool,
) {
    let dx = (tx - fx) as f32;
    let dy = (ty - fy) as f32;
    let len_f = libm::sqrtf(dx * dx + dy * dy).max(1.0);
    if len_f < 2.0 { return; }

    let rigid = matches!(style, EdgeStyle::DoubleSolid);
    // N.14 — thinner, finer ropes.  Old 2-3 px core often blew out
    // the silhouette around small spheres; new 1.4 px nominal core +
    // 1.5 px halo reads as a "wire", not a "hose".
    let core_radius: f32 = if rigid { 2.0 } else { 1.5 };
    let halo_radius: f32 = core_radius + 1.5;
    let sag_px: f32 = if rigid { 0.0 } else { (len_f * 0.10).min(6.0) };

    let pulse_dim = if matches!(style, EdgeStyle::SolidPulsed) && !pulse_on { 0.55 } else { 1.0 };
    // (highlight_bgrx removed in N.15 — ropes are pure red/white only)

    let perp_x = -dy / len_f;
    let perp_y = dx / len_f;

    // One stamp every 0.5 px along the path → sub-pixel coverage AA.
    // ~2× the old sample count (was 1 px steps).  Paired with the
    // coverage profile below this produces Wu-quality silhouettes.
    let n_samples = (len_f as usize * 2).max(8);
    let inv_n = 1.0 / (n_samples - 1) as f32;
    let braid_freq = len_f * 0.45;

    for i in 0..n_samples {
        let t = i as f32 * inv_n;

        // Position along catenary.
        let lx = fx as f32 + dx * t;
        let ly = fy as f32 + dy * t;
        let sag = sag_px * 4.0 * t * (1.0 - t);
        let cx = lx;
        let cy = ly + sag;

        // Per-half colour — INVERT each endpoint's hue.  t=0 → opposite
        // of from_red; t=1 → opposite of to_red.  Linear blend keeps
        // the mid-region clean when endpoints differ.
        let from_half_red = !from_red;
        let to_half_red   = !to_red;
        let red_t = if from_half_red == to_half_red {
            // Both halves want the same hue ⇒ uniform rope.
            if from_half_red { 1.0 } else { 0.0 }
        } else {
            // Sharp split at the midpoint with 0.1-wide cross-fade.
            ((t - 0.45) / 0.1).clamp(0.0, 1.0)
            // When from_half_red=true, that side (t=0) wants red → red_t = 1
            // at t=0, fading toward to_half_red (false → red_t = 0) at t=1.
            // So actually swap the mapping based on which half is red.
        };
        let red_amount = if from_half_red == to_half_red {
            red_t
        } else if from_half_red {
            1.0 - red_t   // t=0 is red, t=1 is white
        } else {
            red_t          // t=0 is white, t=1 is red
        };

        let braid = libm::sinf(t * braid_freq) * 0.10 + 0.90;
        let _ = highlight; // N.15 — pure red/white only, no highlight tinting

        // Build the BGRX for this rope segment by lerping red↔white peaks.
        let base_red   = pack_red_or_white(true,  braid * pulse_dim);
        let base_white = pack_red_or_white(false, braid * pulse_dim);
        let main_bgrx = k_fb::lerp_bgrx(base_white, base_red, (red_amount * 255.0) as u8);

        // Outer halo: additive falloff, gives the rope a soft glow
        // against the dark backdrop.
        let halo_steps_outer = libm::ceilf(halo_radius) as i32;
        for d in -halo_steps_outer..=halo_steps_outer {
            let df = d as f32;
            if df.abs() <= core_radius - 0.5 { continue; }
            let stamp_x = libm::roundf(cx + perp_x * df) as i32;
            let stamp_y = libm::roundf(cy + perp_y * df) as i32;
            if stamp_x < 0 || stamp_x >= SCENE_WIDTH
                || stamp_y < HEADER_H || stamp_y >= FOOTER_Y { continue; }
            let halo_t = (df.abs() - core_radius) / (halo_radius - core_radius);
            let halo_t = halo_t.clamp(0.0, 1.0);
            let halo_i = 0.18 * (1.0 - halo_t) * pulse_dim;
            let glow = k_fb::scale_bgrx(main_bgrx, halo_i);
            sess.add(stamp_x as usize, stamp_y as usize, glow);
        }

        // Core: per-pixel coverage AA against a soft round profile.
        let core_steps = libm::ceilf(core_radius + 0.5) as i32;
        for d in -core_steps..=core_steps {
            let df = d as f32;
            let stamp_x = libm::roundf(cx + perp_x * df) as i32;
            let stamp_y = libm::roundf(cy + perp_y * df) as i32;
            if stamp_x < 0 || stamp_x >= SCENE_WIDTH
                || stamp_y < HEADER_H || stamp_y >= FOOTER_Y { continue; }
            // Coverage = analytic 1-d Gaussian-ish profile.  Pixel
            // 0.5 inside the boundary gets full alpha; pixel 0.5
            // outside gets none; transition is linear in df.
            let cover = (core_radius + 0.5 - df.abs()).clamp(0.0, 1.0);
            if cover <= 0.0 { continue; }
            // Smooth cross-section shading: brightest at d=0, dim toward edge.
            let p = df / (core_radius + 0.5);
            let prof: f32 = (1.0 - p * p).max(0.0);
            let pixel = k_fb::scale_bgrx(main_bgrx, prof);
            // Use alpha-blend so the rope's softer edges don't punch
            // through the underlying sphere / scene with aliased stamps.
            let alpha = (cover * 255.0) as u8;
            // Per-sample stamps overlap when we use 0.5-px steps, so
            // BLEND (not ADD) keeps the rope at its intended brightness
            // instead of saturating to white in the middle.
            sess.blend(stamp_x as usize, stamp_y as usize, pixel, alpha);
        }
    }
}

// ── Phase I.10.1 — animated starfield ─────────────────────────────
//
// Procedural deep-space starfield painted before the spheres so the
// nodes appear to float in a living void rather than a flat blue
// background.  96 stars in 3 parallax layers: the back layer drifts
// slowly (1 px per few frames), the middle a bit faster, the front
// the fastest.  Brightness modulated by sin(time + index*phi) so
// each star "twinkles" independently.
//
// Position generator: a tiny LCG seeded from the star index keeps
// the X / Y / brightness phases stable boot-to-boot.  Cost is
// 96 single-pixel writes per frame — negligible.

const STAR_COUNT: usize = 96;

fn paint_starfield(frame: u64) {
    // N.10 — frozen unless camera auto-rotate is active.  An OS that
    // sits still when nobody touches it shouldn't have a swirling
    // starfield demanding attention; the field becomes a static
    // backdrop, only twinkling/drifting while the orbit demo runs.
    use core::sync::atomic::Ordering;
    let frame_f = if k_fb::CAMERA_AUTO_ROTATE.load(Ordering::Relaxed) {
        frame as f32
    } else {
        0.0
    };
    let _ = frame; // retain param for forward-compat
    for i in 0..STAR_COUNT {
        // LCG-derived properties: X seed, Y seed, layer, phase.
        let seed = (i as u32).wrapping_mul(2654435761);
        let x_seed = seed;
        let y_seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let layer = (i % 3) as u32; // 0 = back, 1 = mid, 2 = front
        let phase = (seed >> 8) as f32 * 0.0001;

        let base_x = (x_seed % SCENE_WIDTH as u32) as i32;
        let base_y =
            HEADER_H + ((y_seed % (FOOTER_Y - HEADER_H) as u32) as i32);

        // Drift speed by layer.  Back = slow, front = fast.  Modulo
        // wraps so stars loop horizontally without visible seams.
        let speed = match layer {
            0 => 0.06,
            1 => 0.18,
            _ => 0.45,
        };
        let drift = (frame_f * speed) as i32;
        let x = (base_x + drift) % SCENE_WIDTH;
        let x = if x < 0 { x + SCENE_WIDTH } else { x };
        let y = base_y;

        if x < 0 || x >= SCENE_WIDTH || y < HEADER_H || y >= FOOTER_Y {
            continue;
        }

        // Twinkle: brightness oscillates on a sin curve with
        // per-star phase offset so the field doesn't pulse in unison.
        let twinkle = libm::sinf(frame_f * 0.08 + phase);
        // Layer determines base brightness band.
        let color = match layer {
            0 => {
                // Back layer — dim cyan from the kernel hue ramp.
                let shade = if twinkle > 0.4 { 1 } else { 0 };
                k_fb::HUE_CYAN_BASE + shade
            }
            1 => {
                // Mid layer — soft mint dust.
                let shade = if twinkle > 0.5 { 2 } else { 1 };
                k_fb::HUE_MINT_BASE + shade
            }
            _ => {
                // Front layer — bright white-ish twinkles using the
                // foreground palette index plus a sin window.
                if twinkle > 0.7 {
                    // Bright moment — paint a 1-pixel cross.
                    k_fb::put_pixel(x as usize, y as usize, k_fb::Color::Foreground);
                    if x + 1 < SCENE_WIDTH {
                        k_fb::put_pixel((x + 1) as usize, y as usize, k_fb::Color::DimWhite);
                    }
                    if x > 0 {
                        k_fb::put_pixel((x - 1) as usize, y as usize, k_fb::Color::DimWhite);
                    }
                    if y + 1 < FOOTER_Y {
                        k_fb::put_pixel(x as usize, (y + 1) as usize, k_fb::Color::DimWhite);
                    }
                    if y > HEADER_H {
                        k_fb::put_pixel(x as usize, (y - 1) as usize, k_fb::Color::DimWhite);
                    }
                    continue;
                } else if twinkle > 0.2 {
                    // Mid-brightness — single foreground pixel.
                    k_fb::Color::Foreground.idx()
                } else {
                    k_fb::Color::DimWhite.idx()
                }
            }
        };
        k_fb::put_pixel_raw(x as usize, y as usize, color);
    }
}

/// Cheap pow approximation good enough for specular falloff in
/// 256-colour space.  Uses 5 squarings → exponent up to 32.
/// libm::powf would be the proper call but pulls a chunkier path;
/// this stays branch-light for the hot per-pixel loop.
fn pow_approx(base: f32, exp: f32) -> f32 {
    // For exponent ≈ 32, repeated squaring of base does it in 5 steps.
    let mut acc = base.max(0.0);
    let target_steps = libm::log2f(exp.max(1.0)) as i32;
    let steps = target_steps.clamp(0, 6);
    for _ in 0..steps {
        acc *= acc;
    }
    acc
}

fn find_node_index(
    nodes: &[GraphNodeSummary],
    vector: VectorAddress,
) -> Option<usize> {
    nodes.iter().position(|n| n.vector == vector)
}

/// Visual style applied to an edge in the 3D scene.  Decoupled from
/// hue (which is set by `classify_edge`) so a future palette tweak
/// doesn't have to thread style through too.  See the rendering loop
/// in `paint_3d_view` for per-style implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeStyle {
    Solid,
    SolidPulsed,
    Dashed,
    Dotted,
    GradientEnds,
    DoubleSolid,
}

fn edge_style(kind: gos_protocol::RuntimeEdgeType) -> EdgeStyle {
    use gos_protocol::RuntimeEdgeType;
    match kind {
        RuntimeEdgeType::Mount => EdgeStyle::DoubleSolid,
        RuntimeEdgeType::Use => EdgeStyle::Dashed,
        RuntimeEdgeType::Depend => EdgeStyle::Dotted,
        RuntimeEdgeType::Signal | RuntimeEdgeType::Call | RuntimeEdgeType::Spawn => {
            EdgeStyle::SolidPulsed
        }
        RuntimeEdgeType::Link => EdgeStyle::GradientEnds,
        _ => EdgeStyle::Solid,
    }
}

/// Map a `NodeSubDomain` partition to a palette colour for the
/// halo ring drawn under each node (I.4.7).  Mirrors the per-class
/// intent set by audit P2 #4: Hardware/KernelDriver/Service are the
/// three dense kernel classes, Compute/Routing are graph-flow
/// classes, Vector is the rare "raw pointer addressed" fallback.
fn sub_domain_color(sub: gos_protocol::NodeSubDomain) -> k_fb::Color {
    use gos_protocol::NodeSubDomain;
    match sub {
        NodeSubDomain::Hardware => k_fb::Color::NodeKernel,    // cyan
        NodeSubDomain::KernelDriver => k_fb::Color::NodeDriver, // amber
        NodeSubDomain::Service => k_fb::Color::NodeService,    // mint
        NodeSubDomain::Compute => k_fb::Color::NodeApp,        // magenta
        NodeSubDomain::Routing => k_fb::Color::NodeOther,      // rose
        NodeSubDomain::Vector => k_fb::Color::DimWhite,
    }
}

#[allow(dead_code)] // N.14 — edges no longer classified by colour
                    // (rope hue is now per-endpoint red/white).
                    // Kept for legacy callers + future re-use.
fn classify_edge(kind: gos_protocol::RuntimeEdgeType) -> k_fb::Color {
    use gos_protocol::RuntimeEdgeType;
    match kind {
        RuntimeEdgeType::Mount => k_fb::Color::NodeDriver,
        RuntimeEdgeType::Use => k_fb::Color::NodeService,
        RuntimeEdgeType::Link => k_fb::Color::Highlight,
        RuntimeEdgeType::Call | RuntimeEdgeType::Spawn | RuntimeEdgeType::Signal => {
            k_fb::Color::NodeApp
        }
        _ => k_fb::Color::DimWhite,
    }
}

/// Tiny no_std string builder used by the framebuffer UI to format
/// counts inline.  Stack-only; truncates silently if the caller asks
/// to push past `N`.  Kept inline here rather than promoting to k-fb
/// because it's specific to the kernel-side panel layout.
struct TextBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> TextBuf<N> {
    fn new() -> Self {
        Self { buf: [0u8; N], len: 0 }
    }

    fn push_str(&mut self, s: &str) {
        for b in s.bytes() {
            if self.len >= N {
                break;
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
    }

    fn push_dec(&mut self, mut value: u64) {
        if value == 0 {
            self.push_str("0");
            return;
        }
        let mut digits = [0u8; 20];
        let mut n = 0;
        while value > 0 {
            digits[n] = b'0' + (value % 10) as u8;
            value /= 10;
            n += 1;
        }
        for i in (0..n).rev() {
            if self.len >= N {
                break;
            }
            self.buf[self.len] = digits[i];
            self.len += 1;
        }
    }

    fn as_str(&self) -> &str {
        // SAFETY: every byte we pushed was either ASCII from a `&str`
        // (which is UTF-8) or an ASCII digit, so the prefix is valid
        // UTF-8.
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase I.5 — kernel-UI command bar + scrollback + mode switch
// ═══════════════════════════════════════════════════════════════════
//
// The boot UI now runs in two modes (toggle via typed commands or
// Esc):
//
//   OsShell    — title + system status; the "desktop" entry point
//                where the user types commands to navigate.  This
//                is the boot default.
//   KernelView — the existing 3D graph scene (octahedra + edges
//                + halos + gizmo).  Reached by typing `kernel`.
//
// A 14-px command bar pinned to the bottom is always visible.  An
// 84-px scrollback panel toggled by F9 (or `log` / `clear` to clear)
// floats above the bar showing recent output.  Both are painted by
// `paint_frame`, which dispatches to either `paint_3d_view`
// (kernel-view body) or `paint_os_shell_body` for the upper region.
//
// Input arrives via `k_fb::pop_typed_char` (fed by k-ps2's `proc`
// stage), so the existing capability route to k-shell is unchanged
// — both consumers see every keystroke.

const CMD_LINE_CAP: usize = 56;     // typed input chars
const SCROLLBACK_LINES: usize = 10; // scrollback ring depth
const SCROLLBACK_LINE_CAP: usize = 44; // chars per line at 8 px width
const HISTORY_LINES: usize = 12;    // command history ring depth

struct UiState {
    /// Current input line being edited; bytes[..len] is valid ASCII.
    line: [u8; CMD_LINE_CAP],
    line_len: usize,
    /// Scrollback ring.  `lines[(head + N - i - 1) % N]` is the i-th
    /// most recent line (0 = newest).  `count` tracks how many slots
    /// have been written; clamps at `SCROLLBACK_LINES`.
    lines: [[u8; SCROLLBACK_LINE_CAP]; SCROLLBACK_LINES],
    line_lens: [usize; SCROLLBACK_LINES],
    head: usize,
    count: usize,
    /// Phase I.8 — command history ring.  Each submitted non-empty
    /// line is pushed here; ArrowUp/Down recall through these.
    /// `hist_cursor` is the offset back from newest:
    ///   None      = current edit (no history navigation in progress)
    ///   Some(0)   = newest history entry
    ///   Some(N-1) = oldest
    hist: [[u8; CMD_LINE_CAP]; HISTORY_LINES],
    hist_lens: [usize; HISTORY_LINES],
    hist_head: usize,
    hist_count: usize,
    hist_cursor: Option<usize>,
    /// K.6 — WATCH JOURNAL: when true, paint_frame tails the journal
    /// and prints any newly-arrived envelopes into the scrollback.
    /// `watch_journal_last_lifetime` records the lifetime counter
    /// from the last drain so we know which entries are new.
    watch_journal: bool,
    watch_journal_last_lifetime: u64,
    /// L.9 — optional filter: when Some(kind_u8), tick_journal_watcher
    /// only emits envelopes whose `kind as u8 == filter`.  Unset
    /// → emit all (the L.6 default behaviour).
    watch_filter: Option<u8>,
}

impl UiState {
    const fn new() -> Self {
        Self {
            line: [0; CMD_LINE_CAP],
            line_len: 0,
            lines: [[0; SCROLLBACK_LINE_CAP]; SCROLLBACK_LINES],
            line_lens: [0; SCROLLBACK_LINES],
            head: 0,
            count: 0,
            hist: [[0; CMD_LINE_CAP]; HISTORY_LINES],
            hist_lens: [0; HISTORY_LINES],
            hist_head: 0,
            hist_count: 0,
            hist_cursor: None,
            watch_journal: false,
            watch_journal_last_lifetime: 0,
            watch_filter: None,
        }
    }

    fn append_char(&mut self, b: u8) {
        if self.line_len < CMD_LINE_CAP {
            self.line[self.line_len] = b;
            self.line_len += 1;
        }
        // Any edit cancels history navigation.
        self.hist_cursor = None;
    }

    fn append_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if self.line_len >= CMD_LINE_CAP {
                break;
            }
            self.line[self.line_len] = b;
            self.line_len += 1;
        }
        self.hist_cursor = None;
    }

    fn backspace(&mut self) {
        if self.line_len > 0 {
            self.line_len -= 1;
        }
        self.hist_cursor = None;
    }

    fn clear_line(&mut self) {
        self.line_len = 0;
        self.hist_cursor = None;
    }

    /// Push the current input line into the history ring before
    /// clearing it.  Caller invokes this on Enter, before clearing.
    fn push_history(&mut self) {
        if self.line_len == 0 {
            return;
        }
        let slot = self.hist_head;
        self.hist[slot][..self.line_len].copy_from_slice(&self.line[..self.line_len]);
        self.hist_lens[slot] = self.line_len;
        self.hist_head = (self.hist_head + 1) % HISTORY_LINES;
        if self.hist_count < HISTORY_LINES {
            self.hist_count += 1;
        }
    }

    /// Navigate history by ±1 step.  `delta = -1` moves toward older
    /// entries (ArrowUp), `+1` toward newer (ArrowDown).  Replaces
    /// the current edit line in-place; setting cursor to None when
    /// we walk past the newest entry restores an empty line.
    fn history_step(&mut self, delta: i32) {
        if self.hist_count == 0 {
            return;
        }
        let new_cursor: Option<usize> = match (self.hist_cursor, delta) {
            (None, d) if d < 0 => Some(0),
            (None, _) => None,
            (Some(c), d) if d < 0 => Some((c + 1).min(self.hist_count - 1)),
            (Some(c), _) => {
                if c == 0 {
                    None
                } else {
                    Some(c - 1)
                }
            }
        };
        self.hist_cursor = new_cursor;
        match new_cursor {
            None => self.line_len = 0,
            Some(c) => {
                // newest is at hist_head - 1; offset back by c.
                let idx = (self.hist_head + HISTORY_LINES - 1 - c) % HISTORY_LINES;
                let n = self.hist_lens[idx];
                self.line[..n].copy_from_slice(&self.hist[idx][..n]);
                self.line_len = n;
            }
        }
    }

    fn current_line(&self) -> &str {
        // SAFETY: only printable ASCII appended via `append_char`.
        unsafe { core::str::from_utf8_unchecked(&self.line[..self.line_len]) }
    }

    fn log(&mut self, text: &str) {
        let slot = self.head;
        let bytes = text.as_bytes();
        let n = bytes.len().min(SCROLLBACK_LINE_CAP);
        self.lines[slot][..n].copy_from_slice(&bytes[..n]);
        self.line_lens[slot] = n;
        self.head = (self.head + 1) % SCROLLBACK_LINES;
        if self.count < SCROLLBACK_LINES {
            self.count += 1;
        }
    }

    fn clear_log(&mut self) {
        self.count = 0;
        self.head = 0;
    }

    /// Iterate lines from oldest to newest (display order top→bottom).
    fn iter_oldest_first(&self) -> impl Iterator<Item = &str> + '_ {
        let count = self.count;
        let head = self.head;
        (0..count).map(move |i| {
            let idx = (head + SCROLLBACK_LINES - count + i) % SCROLLBACK_LINES;
            // SAFETY: only printable ASCII written via `log`.
            unsafe { core::str::from_utf8_unchecked(&self.lines[idx][..self.line_lens[idx]]) }
        })
    }
}

static UI_STATE: spin::Mutex<UiState> = spin::Mutex::new(UiState::new());

/// I.10.5 — global frame counter exposed for the `uptime` command +
/// any future tooling that wants wall-clock-ish time without
/// threading the local `frame_counter` through every call.  Updated
/// by `paint_frame` once per frame; readers see the value of the
/// last completed frame.
static FRAME_COUNTER: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// Drain queued keystrokes from `k_fb::pop_typed_char`, applying
/// them to the input line.  Enter submits the line through
/// `interpret_command`; Esc toggles mode; Backspace edits.
/// K.6 — WATCH JOURNAL tick.  If the user has enabled watching,
/// compare the runtime's journal lifetime to what we last saw; for
/// every new envelope, format a one-line summary and push into the
/// scrollback so it flows through the chat HUD.
fn tick_journal_watcher() {
    let (enabled, last_lifetime, filter) = {
        let ui = UI_STATE.lock();
        (ui.watch_journal, ui.watch_journal_last_lifetime, ui.watch_filter)
    };
    if !enabled {
        return;
    }
    let now_lifetime = gos_runtime::journal_lifetime();
    if now_lifetime <= last_lifetime {
        return;
    }
    let new_count = (now_lifetime - last_lifetime).min(64) as usize;
    let stored = gos_runtime::journal_len();
    let start = stored.saturating_sub(new_count);
    use gos_protocol::ControlPlaneMessageKind::*;
    for i in start..stored {
        if let Some(env) = gos_runtime::journal_envelope_at(i) {
            // L.9 — apply optional filter.
            if let Some(want) = filter {
                if (env.kind as u8) != want {
                    continue;
                }
            }
            let mut row = TextBuf::<48>::new();
            row.push_str("J ");
            row.push_dec(i as u64);
            row.push_str(": ");
            row.push_str(match env.kind {
                Hello => "Hello",
                PluginDiscovered => "PluginDiscovered",
                NodeUpsert => "NodeUpsert",
                EdgeUpsert => "EdgeUpsert",
                StateDelta => "StateDelta",
                SnapshotChunk => "SnapshotChunk",
                Fault => "Fault",
                Metric => "Metric",
                MutationAudit => "MutationAudit",
                CausalOverflow => "CausalOverflow",
                RuleApplied => "RuleApplied",
                SubscribeTriggered => "SubscribeTriggered",
            });
            UI_STATE.lock().log(row.as_str());
        }
    }
    UI_STATE.lock().watch_journal_last_lifetime = now_lifetime;
}

fn drain_ui_input() {
    use gos_protocol::{INPUT_KEY_DOWN, INPUT_KEY_UP};
    while let Some(b) = k_fb::pop_typed_char() {
        match b {
            b'\r' | b'\n' => {
                // Snapshot the line, push to history, clear, then
                // interpret.  Copying out so `interpret_command` can
                // re-acquire the lock to append to the scrollback.
                let mut buf = [0u8; CMD_LINE_CAP];
                let len;
                {
                    let mut ui = UI_STATE.lock();
                    len = ui.line_len;
                    buf[..len].copy_from_slice(&ui.line[..len]);
                    ui.push_history();
                    ui.clear_line();
                }
                let line = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
                interpret_command(line);
            }
            0x08 => UI_STATE.lock().backspace(),
            0x09 => {
                // Phase I.8 — Tab inserts the last-clicked node's
                // vector address as a quoted literal at the cursor.
                // No-op if no node has been clicked yet.
                use core::sync::atomic::Ordering;
                let packed = k_fb::UI_LAST_CLICK_VECTOR.load(Ordering::Relaxed);
                if packed != 0 {
                    let vec = gos_protocol::VectorAddress::from_u64(packed);
                    let mut lit = TextBuf::<24>::new();
                    lit.push_str("'");
                    lit.push_dec(vec.l4 as u64);
                    lit.push_str(".");
                    lit.push_dec(vec.l3 as u64);
                    lit.push_str(".");
                    lit.push_dec(vec.l2 as u64);
                    lit.push_str(".");
                    lit.push_dec(vec.offset as u64);
                    lit.push_str("' ");
                    UI_STATE.lock().append_str(lit.as_str());
                }
            }
            0x1B => {
                // Esc — clear the current input line (was: toggle
                // mode, but that was a UX trap because Esc is a
                // common reflex to exit full-screen; users would
                // accidentally swap to the OS shell and lose the
                // metal-ball view).  Mode switching is now explicit
                // via the `os` / `kernel` commands.
                UI_STATE.lock().clear_line();
            }
            INPUT_KEY_UP => UI_STATE.lock().history_step(-1),
            INPUT_KEY_DOWN => UI_STATE.lock().history_step(1),
            0x20..=0x7E => UI_STATE.lock().append_char(b),
            _ => {} // ignore other control codes
        }
    }
}

/// Interpret a submitted command line.  Echoes the input + any
/// output into the scrollback.  Unknown commands produce a hint.
fn interpret_command(raw: &str) {
    use core::sync::atomic::Ordering;

    // Trim trailing whitespace.
    let line = raw.trim_end_matches(|c: char| c == ' ' || c == '\t');
    {
        // Echo the prompt+command into the scrollback first.
        let mut echo = TextBuf::<60>::new();
        echo.push_str("you> ");
        let take = line.len().min(52);
        echo.push_str(unsafe { core::str::from_utf8_unchecked(&line.as_bytes()[..take]) });
        UI_STATE.lock().log(echo.as_str());
    }

    if line.is_empty() {
        return;
    }

    // ── Phase J.1 — read-side Cypher (queries) ──
    //
    // Try the line first as a SHOW/MATCH query.  If recognised,
    // each row is logged into the chat HUD via the emitter; if
    // NotQuery, fall through to the I.7 mutation path.
    struct HudEmitter;
    impl k_cypher::QueryEmitter for HudEmitter {
        fn emit_row(&mut self, row: &str) {
            UI_STATE.lock().log(row);
        }
    }
    let mut hud = HudEmitter;
    let query_outcome = k_cypher::dispatch_cypher_query(line, &mut hud);
    match query_outcome {
        k_cypher::CypherQueryOutcome::NotQuery => { /* fall through */ }
        k_cypher::CypherQueryOutcome::BadSyntax(msg) => {
            let mut row = TextBuf::<60>::new();
            row.push_str("cypher> syntax: ");
            row.push_str(msg);
            UI_STATE.lock().log(row.as_str());
            return;
        }
        k_cypher::CypherQueryOutcome::EndpointNotFound(msg) => {
            let mut row = TextBuf::<60>::new();
            row.push_str("cypher> ");
            row.push_str(msg);
            UI_STATE.lock().log(row.as_str());
            return;
        }
        k_cypher::CypherQueryOutcome::Rows { count } => {
            let mut row = TextBuf::<60>::new();
            row.push_str("cypher> ");
            row.push_dec(count as u64);
            row.push_str(" row(s)");
            UI_STATE.lock().log(row.as_str());
            return;
        }
    }

    // ── Phase I.7 — Cypher dispatch in the command bar ──
    //
    // Before falling through to the built-in commands, try the line
    // as a Cypher mutation (CREATE MOUNT / CREATE USE / LINK /
    // DELETE EDGE / REBIND USE).  k-cypher's sink-free parser
    // returns `NotCypher` when the line isn't a Cypher verb so we
    // can keep the existing command set as a fallback.
    //
    // Source attribution: "K_HYPERVISOR" 16-byte ASCII null-padded.
    // Distinct from k-cypher's K_CYPHER source so the audit log can
    // tell command-bar mutations from in-VM cypher> shell ones.
    const HYPERVISOR_AUDIT_SOURCE: [u8; 16] = *b"K_HYPERVISOR\0\0\0\0";
    let cypher_outcome = k_cypher::dispatch_cypher_text(line, HYPERVISOR_AUDIT_SOURCE);
    if !matches!(cypher_outcome, k_cypher::CypherDispatchOutcome::NotCypher) {
        // Render a one-line result into the scrollback.
        let mut row = TextBuf::<60>::new();
        match cypher_outcome {
            k_cypher::CypherDispatchOutcome::Applied(verb) => {
                row.push_str("cypher> ");
                row.push_str(verb);
                row.push_str(" ok");
            }
            k_cypher::CypherDispatchOutcome::BadSyntax(msg) => {
                row.push_str("cypher> syntax: ");
                row.push_str(msg);
            }
            k_cypher::CypherDispatchOutcome::EndpointNotFound(msg) => {
                row.push_str("cypher> ");
                row.push_str(msg);
            }
            k_cypher::CypherDispatchOutcome::DispatchFailed(err) => {
                row.push_str("cypher> gate rejected: ");
                use gos_cypher_mut::MutationError;
                match err {
                    MutationError::UnsupportedMutation => row.push_str("unsupported"),
                    MutationError::UnknownEndpoint(_) => row.push_str("unknown endpoint"),
                    MutationError::InvalidMountTarget(_) => row.push_str("invalid mount target"),
                    MutationError::DispatcherRejected(tag) => {
                        row.push_str("gate(");
                        row.push_dec(tag as u64);
                        row.push_str(")");
                    }
                }
            }
            k_cypher::CypherDispatchOutcome::NotCypher => unreachable!(),
        }
        UI_STATE.lock().log(row.as_str());
        return;
    }

    // Lower-case first token compare (manual since we're no_std).
    let token_end = line.find(' ').unwrap_or(line.len());
    let cmd = &line[..token_end];

    let mut ui = UI_STATE.lock();
    match cmd {
        "kernel" | "k" | "kview" => {
            k_fb::UI_MODE.store(k_fb::UI_MODE_KERNEL_VIEW, Ordering::Relaxed);
            ui.log("[mode] kernel view");
        }
        "os" | "shell" | "exit" => {
            k_fb::UI_MODE.store(k_fb::UI_MODE_OS_SHELL, Ordering::Relaxed);
            ui.log("[mode] os shell");
        }
        "log" => {
            // Toggle scrollback expand/collapse.
            let cur = k_fb::UI_SCROLLBACK_EXPANDED.load(Ordering::Relaxed);
            k_fb::UI_SCROLLBACK_EXPANDED.store(!cur, Ordering::Relaxed);
        }
        "clear" | "cls" => {
            ui.clear_log();
        }
        "help" | "?" => {
            ui.log("commands:");
            ui.log("  kernel | k        enter 3D graph view");
            ui.log("  os | exit         return to OS shell");
            ui.log("  ps                list plugins by class");
            ui.log("  inspect <vec>     deep-dive on a node");
            ui.log("  nodes / edges     graph stats");
            ui.log("  uptime / gen      runtime info");
            ui.log("  journal           audit log tail + counts");
            ui.log("  watch [filter]    live-tail (filters: fault/mutation/node/edge/state)");
            ui.log("  unwatch           stop tailing");
            ui.log("  bench [rpc] [N]   RDTSC measure RPC echo latency");
            ui.log("  log / clear       scrollback control (F9)");
            ui.log("  Esc               clear input line");
            ui.log("cypher reads (live graph):");
            ui.log("  SHOW STATS");
            ui.log("  SHOW NODES [OF CLASS X]");
            ui.log("  SHOW EDGES [OF KIND X]");
            ui.log("  SHOW EDGES FROM 'V'  / TO 'V'");
            ui.log("  SHOW PLUGINS");
            ui.log("  SHOW JOURNAL [LIMIT N]");
            ui.log("cypher actions:");
            ui.log("  SET PRIORITY 'V' = N      (N in 0..255)");
            ui.log("  SHOW PRIORITY 'V'");
            ui.log("  RESET PRIORITY 'V'        (back to 128)");
            ui.log("  SET DEADLINE 'V' = N      (RDTSC cycles, 0 disables)");
            ui.log("  SHOW DEADLINE 'V'");
            ui.log("  INVOKE 'V' [WITH N]       (RPC, returns u64)");
            ui.log("cypher mutations:");
            ui.log("  CREATE MOUNT 'F' -> 'T'");
            ui.log("  CREATE USE   'F' -> 'T'");
            ui.log("  LINK 'F' -> 'T'");
            ui.log("  REBIND USE 'F' -> 'T'");
            ui.log("  DELETE EDGE 'e:V'");
            ui.log("repl ergonomics:");
            ui.log("  Tab    insert last-clicked node's vector");
            ui.log("  Up/Dn  recall previous commands");
        }
        "nodes" => {
            let mut buf = [GraphNodeSummary::EMPTY; MAX_NODES];
            let (total, returned) = gos_runtime::node_page(0, &mut buf);
            // Tally per sub_domain.
            let mut hw = 0usize;
            let mut drv = 0usize;
            let mut svc = 0usize;
            let mut cpu = 0usize;
            let mut rtr = 0usize;
            let mut vec_ct = 0usize;
            use gos_protocol::NodeSubDomain;
            for n in &buf[..returned] {
                match n.sub_domain {
                    NodeSubDomain::Hardware => hw += 1,
                    NodeSubDomain::KernelDriver => drv += 1,
                    NodeSubDomain::Service => svc += 1,
                    NodeSubDomain::Compute => cpu += 1,
                    NodeSubDomain::Routing => rtr += 1,
                    NodeSubDomain::Vector => vec_ct += 1,
                }
            }
            let mut row = TextBuf::<48>::new();
            row.push_str("nodes total=");
            row.push_dec(total as u64);
            row.push_str(" returned=");
            row.push_dec(returned as u64);
            ui.log(row.as_str());
            let mut row2 = TextBuf::<48>::new();
            row2.push_str("  HW=");
            row2.push_dec(hw as u64);
            row2.push_str(" DRV=");
            row2.push_dec(drv as u64);
            row2.push_str(" SVC=");
            row2.push_dec(svc as u64);
            ui.log(row2.as_str());
            let mut row3 = TextBuf::<48>::new();
            row3.push_str("  CPU=");
            row3.push_dec(cpu as u64);
            row3.push_str(" RTR=");
            row3.push_dec(rtr as u64);
            row3.push_str(" VEC=");
            row3.push_dec(vec_ct as u64);
            ui.log(row3.as_str());
        }
        "edges" => {
            let snap = gos_runtime::snapshot();
            let mut row = TextBuf::<48>::new();
            row.push_str("edges count=");
            row.push_dec(snap.edge_count as u64);
            ui.log(row.as_str());
        }
        "gen" => {
            let mut row = TextBuf::<48>::new();
            row.push_str("graph_generation=");
            row.push_dec(gos_runtime::graph_generation());
            ui.log(row.as_str());
        }
        "watch" => {
            // K.6 + L.9 — tail new journal envelopes, optionally
            // filtered by kind.  Syntax:
            //   watch                show all kinds (default)
            //   watch fault          only Fault envelopes
            //   watch mutation       only MutationAudit
            //   watch node           only NodeUpsert
            //   watch edge           only EdgeUpsert
            //   watch state          only StateDelta
            let after = line.get(token_end..).unwrap_or("");
            let arg = after.trim_start_matches(|c: char| c == ' ' || c == '\t');
            use gos_protocol::ControlPlaneMessageKind as K;
            ui.watch_filter = if arg.is_empty() || arg.eq_ignore_ascii_case("all") {
                None
            } else if arg.eq_ignore_ascii_case("fault") {
                Some(K::Fault as u8)
            } else if arg.eq_ignore_ascii_case("mutation") || arg.eq_ignore_ascii_case("audit") {
                Some(K::MutationAudit as u8)
            } else if arg.eq_ignore_ascii_case("node") {
                Some(K::NodeUpsert as u8)
            } else if arg.eq_ignore_ascii_case("edge") {
                Some(K::EdgeUpsert as u8)
            } else if arg.eq_ignore_ascii_case("state") {
                Some(K::StateDelta as u8)
            } else if arg.eq_ignore_ascii_case("metric") {
                Some(K::Metric as u8)
            } else if arg.eq_ignore_ascii_case("plugin") {
                Some(K::PluginDiscovered as u8)
            } else {
                let mut r = TextBuf::<60>::new();
                r.push_str("unknown watch filter: ");
                let take = arg.len().min(32);
                r.push_str(unsafe { core::str::from_utf8_unchecked(&arg.as_bytes()[..take]) });
                ui.log(r.as_str());
                ui.log("  filters: all fault mutation node edge state metric plugin");
                return;
            };
            ui.watch_journal = true;
            ui.watch_journal_last_lifetime = gos_runtime::journal_lifetime();
            let mut r = TextBuf::<60>::new();
            r.push_str("watch journal: ON  filter=");
            r.push_str(match ui.watch_filter {
                None => "all",
                Some(x) if x == K::Fault as u8 => "fault",
                Some(x) if x == K::MutationAudit as u8 => "mutation",
                Some(x) if x == K::NodeUpsert as u8 => "node",
                Some(x) if x == K::EdgeUpsert as u8 => "edge",
                Some(x) if x == K::StateDelta as u8 => "state",
                Some(x) if x == K::Metric as u8 => "metric",
                Some(x) if x == K::PluginDiscovered as u8 => "plugin",
                _ => "?",
            });
            ui.log(r.as_str());
        }
        "unwatch" => {
            ui.watch_journal = false;
            ui.watch_filter = None;
            ui.log("watch journal: OFF");
        }
        "bench" => {
            // L.7 — bench RPC echo: BENCH RPC [N]
            // Times N consecutive rpc_invoke(0.0.0.0, i) calls
            // using RDTSC, reports total / avg / min / max.
            let after = line.get(token_end..).unwrap_or("");
            let arg = after.trim_start_matches(|c: char| c == ' ' || c == '\t');
            // Optional "RPC" subcommand prefix.
            let arg = if arg.len() >= 3 && arg.as_bytes()[..3].eq_ignore_ascii_case(b"rpc") {
                arg[3..].trim_start()
            } else {
                arg
            };
            let count: u64 = arg.parse().unwrap_or(1000);
            let count = count.clamp(1, 100_000);

            let mut total: u64 = 0;
            let mut min: u64 = u64::MAX;
            let mut max: u64 = 0;
            for i in 0..count {
                let t0 = unsafe { core::arch::x86_64::_rdtsc() };
                let _ = gos_runtime::rpc_invoke(
                    gos_runtime::RPC_ECHO_VECTOR,
                    i,
                );
                let t1 = unsafe { core::arch::x86_64::_rdtsc() };
                let dt = t1.wrapping_sub(t0);
                total = total.wrapping_add(dt);
                if dt < min { min = dt; }
                if dt > max { max = dt; }
            }
            let avg = total / count;

            let mut r = TextBuf::<60>::new();
            r.push_str("bench rpc echo n=");
            r.push_dec(count);
            ui.log(r.as_str());

            let mut r2 = TextBuf::<60>::new();
            r2.push_str("  total cycles: ");
            r2.push_dec(total);
            ui.log(r2.as_str());

            let mut r3 = TextBuf::<60>::new();
            r3.push_str("  avg/call: ");
            r3.push_dec(avg);
            ui.log(r3.as_str());

            let mut r4 = TextBuf::<60>::new();
            r4.push_str("  min: ");
            r4.push_dec(min);
            r4.push_str("  max: ");
            r4.push_dec(max);
            ui.log(r4.as_str());
        }
        "journal" => {
            // J.2 — show journal stats and the most recent entries.
            let stored = gos_runtime::journal_len();
            let lifetime = gos_runtime::journal_lifetime();
            let mut row = TextBuf::<48>::new();
            row.push_str("journal stored=");
            row.push_dec(stored as u64);
            row.push_str(" lifetime=");
            row.push_dec(lifetime);
            ui.log(row.as_str());
            // Tail: last 6 envelopes (newest at the bottom).
            let tail_start = stored.saturating_sub(6);
            for i in tail_start..stored {
                if let Some(env) = gos_runtime::journal_envelope_at(i) {
                    let mut r = TextBuf::<48>::new();
                    r.push_str("  ");
                    r.push_dec(i as u64);
                    r.push_str(": ");
                    use gos_protocol::ControlPlaneMessageKind::*;
                    r.push_str(match env.kind {
                        Hello => "Hello",
                        PluginDiscovered => "PluginDiscovered",
                        NodeUpsert => "NodeUpsert",
                        EdgeUpsert => "EdgeUpsert",
                        StateDelta => "StateDelta",
                        SnapshotChunk => "SnapshotChunk",
                        Fault => "Fault",
                        Metric => "Metric",
                        MutationAudit => "MutationAudit",
                        CausalOverflow => "CausalOverflow",
                        RuleApplied => "RuleApplied",
                        SubscribeTriggered => "SubscribeTriggered",
                    });
                    r.push_str(" arg0=");
                    r.push_dec(env.arg0);
                    ui.log(r.as_str());
                }
            }
        }
        "uptime" => {
            let frame = FRAME_COUNTER.load(Ordering::Relaxed);
            let secs = frame / 50;
            let hh = secs / 3600;
            let mm = (secs / 60) % 60;
            let ss = secs % 60;
            let mut row = TextBuf::<48>::new();
            row.push_str("uptime ");
            if hh < 10 { row.push_str("0"); }
            row.push_dec(hh);
            row.push_str(":");
            if mm < 10 { row.push_str("0"); }
            row.push_dec(mm);
            row.push_str(":");
            if ss < 10 { row.push_str("0"); }
            row.push_dec(ss);
            row.push_str("  frames=");
            row.push_dec(frame);
            ui.log(row.as_str());
        }
        "ps" => {
            // I.10.3 — plugin list with state.  Walks the node page
            // and prints one row per unique plugin: PID  COUNT  CLASS
            // where COUNT is the number of nodes the plugin owns and
            // CLASS is its primary node's sub-domain abbreviation.
            let mut buf = [GraphNodeSummary::EMPTY; MAX_NODES];
            let (_total, returned) = gos_runtime::node_page(0, &mut buf);
            // Group by plugin_id (assume small N — fine to do N²).
            let mut seen_ids: [Option<gos_protocol::PluginId>; MAX_NODES] = [None; MAX_NODES];
            let mut seen_counts: [usize; MAX_NODES] = [0; MAX_NODES];
            let mut seen_classes: [u8; MAX_NODES] = [0; MAX_NODES];
            let mut seen_names: [&'static str; MAX_NODES] = [""; MAX_NODES];
            let mut seen_n = 0usize;
            for n in &buf[..returned] {
                let mut idx = None;
                for s in 0..seen_n {
                    if seen_ids[s] == Some(n.plugin_id) {
                        idx = Some(s);
                        break;
                    }
                }
                match idx {
                    Some(s) => seen_counts[s] += 1,
                    None => {
                        seen_ids[seen_n] = Some(n.plugin_id);
                        seen_counts[seen_n] = 1;
                        seen_classes[seen_n] = n.sub_domain as u8;
                        seen_names[seen_n] = n.plugin_name;
                        seen_n += 1;
                    }
                }
            }
            let mut header = TextBuf::<48>::new();
            header.push_str("ps  ");
            header.push_dec(seen_n as u64);
            header.push_str(" plugins / ");
            header.push_dec(returned as u64);
            header.push_str(" nodes");
            ui.log(header.as_str());
            for s in 0..seen_n {
                use gos_protocol::NodeSubDomain;
                let class_label = match seen_classes[s] {
                    x if x == NodeSubDomain::Hardware as u8 => "HW",
                    x if x == NodeSubDomain::KernelDriver as u8 => "DRV",
                    x if x == NodeSubDomain::Service as u8 => "SVC",
                    x if x == NodeSubDomain::Compute as u8 => "CPU",
                    x if x == NodeSubDomain::Routing as u8 => "RTR",
                    _ => "VEC",
                };
                let mut row = TextBuf::<48>::new();
                row.push_str("  ");
                // Trim K_ prefix for readability.
                let raw = seen_names[s];
                let trimmed = raw.strip_prefix("K_").unwrap_or(raw);
                let take = trimmed.len().min(14);
                row.push_str(unsafe {
                    core::str::from_utf8_unchecked(&trimmed.as_bytes()[..take])
                });
                // Pad to ~14 cols.
                for _ in take..14 {
                    row.push_str(" ");
                }
                row.push_str(class_label);
                row.push_str(" x");
                row.push_dec(seen_counts[s] as u64);
                ui.log(row.as_str());
            }
        }
        "inspect" => {
            // I.10.4 — deep-dive on a single node by vector address.
            // Usage: `inspect 6.6.0.0`  (or any V_l4.V_l3.V_l2.V_off)
            let after = line.get(token_end..).unwrap_or("");
            let arg = after.trim_start_matches(|c: char| c == ' ' || c == '\t');
            // Strip optional surrounding quotes.
            let arg = arg
                .trim_start_matches('\'')
                .trim_end_matches('\'')
                .trim_end_matches(|c: char| c == ' ' || c == '\t');
            let Some(vec) = gos_protocol::VectorAddress::parse(arg) else {
                ui.log("inspect: bad vector (try 6.6.0.0)");
                return;
            };
            // Walk node_page to find a matching summary.
            let mut buf = [GraphNodeSummary::EMPTY; MAX_NODES];
            let (_, returned) = gos_runtime::node_page(0, &mut buf);
            let found = buf[..returned].iter().find(|n| n.vector == vec);
            match found {
                None => {
                    let mut row = TextBuf::<48>::new();
                    row.push_str("inspect: no node at ");
                    let take = arg.len().min(24);
                    row.push_str(unsafe { core::str::from_utf8_unchecked(&arg.as_bytes()[..take]) });
                    ui.log(row.as_str());
                }
                Some(n) => {
                    let mut h = TextBuf::<48>::new();
                    h.push_str("inspect ");
                    h.push_dec(n.vector.l4 as u64);
                    h.push_str(".");
                    h.push_dec(n.vector.l3 as u64);
                    h.push_str(".");
                    h.push_dec(n.vector.l2 as u64);
                    h.push_str(".");
                    h.push_dec(n.vector.offset as u64);
                    ui.log(h.as_str());
                    let mut r1 = TextBuf::<48>::new();
                    r1.push_str("  plugin: ");
                    let nm = n.plugin_name;
                    let take = nm.len().min(28);
                    r1.push_str(unsafe { core::str::from_utf8_unchecked(&nm.as_bytes()[..take]) });
                    ui.log(r1.as_str());
                    let mut r2 = TextBuf::<48>::new();
                    r2.push_str("  key:    ");
                    let k = n.local_node_key;
                    let take2 = k.len().min(28);
                    r2.push_str(unsafe { core::str::from_utf8_unchecked(&k.as_bytes()[..take2]) });
                    ui.log(r2.as_str());
                    use gos_protocol::{NodeSubDomain, RuntimeNodeType};
                    let type_label = match n.node_type {
                        RuntimeNodeType::Hardware => "HW",
                        RuntimeNodeType::Driver => "DRV",
                        RuntimeNodeType::Service => "SVC",
                        RuntimeNodeType::PluginEntry => "PE",
                        RuntimeNodeType::Compute => "CPU",
                        RuntimeNodeType::Router => "RTR",
                        RuntimeNodeType::Aggregator => "AGG",
                        RuntimeNodeType::Vector => "VEC",
                    };
                    let sub_label = match n.sub_domain {
                        NodeSubDomain::Hardware => "Hardware",
                        NodeSubDomain::KernelDriver => "KernelDriver",
                        NodeSubDomain::Service => "Service",
                        NodeSubDomain::Compute => "Compute",
                        NodeSubDomain::Routing => "Routing",
                        NodeSubDomain::Vector => "Vector",
                    };
                    let mut r3 = TextBuf::<48>::new();
                    r3.push_str("  type:   ");
                    r3.push_str(type_label);
                    r3.push_str(" / ");
                    r3.push_str(sub_label);
                    ui.log(r3.as_str());
                    let mut r4 = TextBuf::<48>::new();
                    r4.push_str("  exports=");
                    r4.push_dec(n.export_count as u64);
                    ui.log(r4.as_str());
                }
            }
        }
        _ => {
            let mut row = TextBuf::<48>::new();
            row.push_str("unknown: ");
            let take = cmd.len().min(32);
            row.push_str(unsafe { core::str::from_utf8_unchecked(&cmd.as_bytes()[..take]) });
            row.push_str(" (try 'help')");
            ui.log(row.as_str());
        }
    }
}

// ── Phase I.10.2 — uptime / heartbeat widget ──────────────────────
//
// Always-visible corner widget above the command bar showing the
// kernel's wall-clock uptime + a 1-pixel pulsing heartbeat dot that
// confirms the paint loop is alive at a glance.  Painted by
// `paint_frame` in BOTH modes (kernel view and OS shell) so the
// user always has a "this kernel is running" affordance.

// N.17 — paint_status_widget / paint_stats_chip retired; all of
// their data lives in the consolidated bottom strip painted by
// paint_command_bar.

/// Top-level paint coordinator.  Drains input, paints the header
/// + body (mode-dependent) + scrollback (when expanded) + command
/// bar.  Replaces the direct `paint_3d_view` call from boot.
fn paint_frame(frame: u64) {
    if !k_fb::ready() {
        return;
    }
    // I.10.5 — publish the current frame so commands can read uptime.
    FRAME_COUNTER.store(frame, core::sync::atomic::Ordering::Relaxed);
    drain_ui_input();
    // K.6 — WATCH JOURNAL: tail new envelopes into the scrollback.
    tick_journal_watcher();

    use core::sync::atomic::Ordering;
    let mode = k_fb::UI_MODE.load(Ordering::Relaxed);
    let scrollback_open = k_fb::UI_SCROLLBACK_EXPANDED.load(Ordering::Relaxed);

    // N.17 — no top header anymore.  Body fills the whole canvas
    // top→FOOTER_Y; the only chrome is the consolidated bottom strip
    // painted by `paint_command_bar` below.
    use design::*;
    k_fb::clear_gradient_vertical(SURFACE_0_TOP, SURFACE_0_BOTTOM);

    match mode {
        k_fb::UI_MODE_KERNEL_VIEW => paint_3d_view(frame),
        _ => paint_os_shell_body(frame),
    }

    // N.17 — paint_status_widget is gone (uptime + heartbeat folded
    // into the consolidated bottom strip below).
    if scrollback_open {
        paint_scrollback();
    } else {
        paint_chat_hud();
    }
    // Snapshot the live runtime stats for the bottom strip.
    let snap = gos_runtime::snapshot();
    paint_command_bar(frame, mode, snap.node_count, snap.edge_count);

    k_fb::present();
}

/// Phase I.12 — chat HUD overlay.  Renders the last 4 scrollback
/// lines as outlined text immediately above the command bar.  No
/// background fill so the 3D scene stays visible beneath.  Skipped
/// when the full scrollback (`F9`) is open.
fn paint_chat_hud() {
    const HUD_LINES: usize = 4;
    let ui = UI_STATE.lock();
    let total = ui.count;
    if total == 0 {
        return;
    }
    let show = total.min(HUD_LINES);
    // N.11 — AA chat lines in Noto Sans SC 14 (cell_h = 20).
    let line_h = 20usize;
    let baseline_y = (CMD_BAR_TOP - 4) as usize;
    let mut y = baseline_y - show * line_h;
    let start = total - show;
    for (i, line) in ui.iter_oldest_first().enumerate() {
        if i < start {
            continue;
        }
        let take = line.len().min(SCROLLBACK_LINE_CAP);
        let trimmed = unsafe { core::str::from_utf8_unchecked(&line.as_bytes()[..take]) };
        // Single 1-px shadow below for legibility on any backdrop.
        let x = 8usize;
        k_fb::draw_text_ui(x + 1, y + 1, trimmed, k_fb::Color::Background);
        k_fb::draw_text_ui(x, y, trimmed, k_fb::Color::Foreground);
        y += line_h;
    }
}

/// OS-shell body: the entry-point view the user sees by default.
/// Big brand title centred near the top of the body region, then a
/// 2-column status grid showing plugin/node/edge counts plus the
/// current mode + scrollback state.  Designed to feel like a clean
/// boot terminal, not a wallpaper.
fn paint_os_shell_body(frame: u64) {
    use core::sync::atomic::Ordering;
    let _ = frame;

    // Header text mirrors kernel-view but tagged with mode.
    let snap = gos_runtime::snapshot();
    let mut nodes_buf = [GraphNodeSummary::EMPTY; MAX_NODES];
    let (total_n, returned_n) = gos_runtime::node_page(0, &mut nodes_buf);
    let _ = total_n;

    // I.10 — match the kernel-view header layout; uptime widget
    // owns the right edge.
    k_fb::draw_text(4, 3, "GOS-OS", k_fb::Color::Highlight);
    k_fb::draw_text(4 + 6 * 8, 3, "|", k_fb::Color::DimWhite);
    let mut count_a = TextBuf::<10>::new();
    count_a.push_dec(returned_n as u64);
    count_a.push_str("N");
    k_fb::draw_text(4 + 8 * 8, 3, count_a.as_str(), k_fb::Color::Foreground);
    k_fb::draw_text(4 + 12 * 8, 3, "|", k_fb::Color::DimWhite);
    let mut count_b = TextBuf::<10>::new();
    count_b.push_dec(snap.edge_count as u64);
    count_b.push_str("E");
    k_fb::draw_text(4 + 14 * 8, 3, count_b.as_str(), k_fb::Color::Foreground);

    // Big brand title centred at ~y=40.
    let title = "GOS  /  GRAPH OS";
    let tx = (k_fb::WIDTH as i32 - title.len() as i32 * 8) / 2;
    k_fb::draw_text(tx as usize, 40, title, k_fb::Color::Foreground);
    let sub = "graph theory kernel";
    let sx = (k_fb::WIDTH as i32 - sub.len() as i32 * 8) / 2;
    k_fb::draw_text(sx as usize, 52, sub, k_fb::Color::DimWhite);

    // Status grid centred at y=78.  Three rows, two columns each.
    // Read several runtime stats and lay them out as `label  value`
    // pairs.  Uses DimWhite for labels and Highlight for the values
    // so the data jumps off the dark background.
    let mode_label = if k_fb::UI_MODE.load(Ordering::Relaxed) == k_fb::UI_MODE_KERNEL_VIEW {
        "KERNEL"
    } else {
        "SHELL"
    };
    let scroll_label = if k_fb::UI_SCROLLBACK_EXPANDED.load(Ordering::Relaxed) {
        "OPEN"
    } else {
        "HIDDEN"
    };

    let row_y = [78usize, 92, 106];
    let labels = [
        ("plugins  ", "nodes    "),
        ("edges    ", "gen      "),
        ("mode     ", "scroll   "),
    ];
    let mut plugins_buf = TextBuf::<8>::new();
    plugins_buf.push_dec(snap.plugin_count as u64);
    let mut nodes_buf2 = TextBuf::<8>::new();
    nodes_buf2.push_dec(snap.node_count as u64);
    let mut edges_buf = TextBuf::<8>::new();
    edges_buf.push_dec(snap.edge_count as u64);
    let mut gen_buf = TextBuf::<12>::new();
    gen_buf.push_str("G");
    gen_buf.push_dec(gos_runtime::graph_generation());
    let values: [(&str, &str); 3] = [
        (plugins_buf.as_str(), nodes_buf2.as_str()),
        (edges_buf.as_str(), gen_buf.as_str()),
        (mode_label, scroll_label),
    ];

    let col_left_x = 60usize;
    let col_right_x = 180usize;
    for i in 0..3 {
        let (l_lbl, r_lbl) = labels[i];
        let (l_val, r_val) = values[i];
        k_fb::draw_text(col_left_x, row_y[i], l_lbl, k_fb::Color::DimWhite);
        k_fb::draw_text(col_left_x + 9 * 8, row_y[i], l_val, k_fb::Color::Highlight);
        k_fb::draw_text(col_right_x, row_y[i], r_lbl, k_fb::Color::DimWhite);
        k_fb::draw_text(col_right_x + 9 * 8, row_y[i], r_val, k_fb::Color::Highlight);
    }

    // Hint near the bottom of body (just above command bar / scrollback).
    let hint = "type 'kernel' to launch 3D view  |  'help' for commands";
    let hx = (k_fb::WIDTH as i32 - hint.len() as i32 * 8) / 2;
    // Clamp left if hint is wider than screen.
    let hint_x = hx.max(2) as usize;
    k_fb::draw_text(hint_x, 130, hint, k_fb::Color::Foreground);

    // Subtle bottom hairline above the command bar zone.
    k_fb::hline(0, FOOTER_Y as usize, k_fb::WIDTH, k_fb::Color::DimWhite);
}

/// Paint the always-visible command-input bar at the bottom of the
/// screen.  Layout: HeaderBar-tinted strip, `> ` prompt in Highlight,
/// typed text in Foreground, blinking 1-px cursor in Foreground at
/// the current insertion point.  Frame counter drives the blink at
/// ~2.5 Hz so the user has a clear "I can type here" affordance.
fn paint_command_bar(frame: u64, mode: u8, node_n: usize, edge_n: usize) {
    use design::*;
    // ── N.17 — consolidated slim chrome strip ────────────────────────
    //
    // One 22-px tall bar holds everything the old two-strip layout
    // showed: brand, stats, uptime, heartbeat, command input, mode pill.
    // Layout (left → right) in the 320-px logical width:
    //
    //   GOS · 25N · 63E · 03:42 ●     ┌─› cmd ──┐    KERNEL
    //   └─── status cluster ────┘  └─ input ─┘  └─ pill ─┘
    let bar_y = CMD_BAR_TOP as usize;
    let bar_h = CMD_BAR_H as usize;

    // Slim gradient underlay across the full width — gives the chrome
    // a quiet "shelf" without being a heavy bar.
    k_fb::fill_rect_gradient(0, bar_y, k_fb::WIDTH, bar_h, SURFACE_2_TOP, SURFACE_2_BOTTOM);
    // Vapor accent line along the top edge — animated, ties the chrome
    // visually to the floating cards (which share the same vapor style).
    let vapor_phase = frame as f32 * 0.0018;
    k_fb::vapor_h_line(bar_y, 0, k_fb::WIDTH, vapor_phase, 0.005);

    // Text baseline: centre 18-px mono inside 22-px strip.
    let text_y = bar_y + (bar_h - 18) / 2;
    let mono_w: usize = 10;

    // ── LEFT cluster: brand · stats · uptime · heartbeat ──
    let mut hx: usize = SP_MD;
    k_fb::draw_text_aa_rgb(hx, text_y, "GOS", ACCENT_PRIMARY, &k_assets::FONT_MONO_13);
    hx += 3 * mono_w + SP_SM;
    k_fb::draw_text_aa_rgb(hx, text_y, "·", TEXT_TERTIARY, &k_assets::FONT_MONO_13);
    hx += mono_w + SP_SM;
    // Stats: "25N · 63E"
    let mut stats = TextBuf::<12>::new();
    stats.push_dec(node_n as u64);
    stats.push_str("N");
    k_fb::draw_text_aa_rgb(hx, text_y, stats.as_str(), TEXT_SECONDARY, &k_assets::FONT_MONO_13);
    hx += stats.as_str().len() * mono_w + SP_SM;
    k_fb::draw_text_aa_rgb(hx, text_y, "·", TEXT_TERTIARY, &k_assets::FONT_MONO_13);
    hx += mono_w + SP_SM;
    let mut stats2 = TextBuf::<12>::new();
    stats2.push_dec(edge_n as u64);
    stats2.push_str("E");
    k_fb::draw_text_aa_rgb(hx, text_y, stats2.as_str(), TEXT_SECONDARY, &k_assets::FONT_MONO_13);
    hx += stats2.as_str().len() * mono_w + SP_SM;
    k_fb::draw_text_aa_rgb(hx, text_y, "·", TEXT_TERTIARY, &k_assets::FONT_MONO_13);
    hx += mono_w + SP_SM;
    // Uptime "MM:SS" — strip hours for compactness, ~99 min wraps.
    let total_secs = frame / 50;
    let mm = (total_secs / 60) % 100;
    let ss = total_secs % 60;
    let mut upt = TextBuf::<8>::new();
    if mm < 10 { upt.push_str("0"); }
    upt.push_dec(mm);
    upt.push_str(":");
    if ss < 10 { upt.push_str("0"); }
    upt.push_dec(ss);
    k_fb::draw_text_aa_rgb(hx, text_y, upt.as_str(), TEXT_SECONDARY, &k_assets::FONT_MONO_13);
    hx += upt.as_str().len() * mono_w + SP_SM;
    // Heartbeat dot.
    let pulsing = (frame % 50) < 6;
    let dot = if pulsing { ACCENT_PRIMARY } else { ACCENT_PRIMARY_DIM };
    let dot_size = 4usize;
    let dot_y = bar_y + (bar_h - dot_size) / 2;
    for dy in 0..dot_size {
        for dx in 0..dot_size {
            let drx = dx as i32 * 2 - dot_size as i32;
            let dry = dy as i32 * 2 - dot_size as i32;
            if drx * drx + dry * dry <= (dot_size as i32) * (dot_size as i32) {
                k_fb::put_pixel_rgb(hx + dx, dot_y + dy, dot);
            }
        }
    }
    let left_end = hx + dot_size + SP_MD;

    // ── RIGHT cluster: mode pill ──
    let pill = match mode {
        k_fb::UI_MODE_KERNEL_VIEW => "KERNEL",
        _ => "SHELL",
    };
    let pill_text_w = pill.len() * mono_w;
    let pill_w = pill_text_w + 2 * SP_MD;
    let pill_h = (bar_h - 2).min(16);
    let pill_x = k_fb::WIDTH - pill_w - SP_MD;
    let pill_y = bar_y + (bar_h - pill_h) / 2;
    k_fb::fill_round_rect_alpha(pill_x, pill_y, pill_w, pill_h, RADIUS_SM, ACCENT_PRIMARY_DIM, 160);
    k_fb::draw_text_aa_rgb(pill_x + SP_MD, pill_y, pill, ACCENT_PRIMARY, &k_assets::FONT_MONO_13);
    let right_start = pill_x - SP_MD;

    // ── CENTRE cluster: command input box ──
    // Fits whatever horizontal budget is left between the two clusters.
    // Win11 Start-style centred glass card with vapor particle border.
    let box_w = right_start.saturating_sub(left_end).min(196);
    if box_w >= 80 {
        let box_h = bar_h - 2;
        let box_x = left_end + (right_start - left_end - box_w) / 2;
        let box_y = bar_y + 1;

        k_fb::box_blur_region(box_x, box_y, box_w, box_h);
        k_fb::fill_round_rect_alpha(box_x, box_y, box_w, box_h, RADIUS_MD, SURFACE_1_TOP, 215);
        // N.17 — gas-particle vapor border (replaces solid line vapor).
        k_fb::vapor_rect_particles(box_x, box_y, box_w, box_h, vapor_phase);

        // Prompt + typed text.
        let prompt_x = box_x + SP_SM;
        let prompt_y = box_y + (box_h.saturating_sub(18)) / 2;
        k_fb::draw_text_aa_rgb(prompt_x, prompt_y, "›", ACCENT_PRIMARY, &k_assets::FONT_MONO_13);
        let body_x = prompt_x + mono_w + 2;

        let ui = UI_STATE.lock();
        let line = ui.current_line();
        let line_len = line.len();
        let max_visible_chars =
            ((box_x + box_w).saturating_sub(body_x + SP_SM) / mono_w).min(CMD_LINE_CAP);
        let visible_start = line_len.saturating_sub(max_visible_chars);
        let visible = unsafe {
            core::str::from_utf8_unchecked(&line.as_bytes()[visible_start..line_len])
        };
        k_fb::draw_text_aa_rgb(body_x, prompt_y, visible, TEXT_PRIMARY, &k_assets::FONT_MONO_13);

        // 2-px cyan cursor.
        let cursor_on = (frame / 16) & 1 == 0;
        if cursor_on {
            let cursor_x = body_x + (line_len - visible_start) * mono_w;
            if cursor_x + 2 < box_x + box_w - SP_SM {
                for cy in 1..(box_h - 2) {
                    let py = box_y + cy;
                    if py >= k_fb::HEIGHT { break; }
                    k_fb::put_pixel_rgb(cursor_x, py, ACCENT_PRIMARY);
                    k_fb::put_pixel_rgb(cursor_x + 1, py, ACCENT_PRIMARY);
                }
            }
        }
    }
}

/// Paint the collapsible scrollback panel.  Stacks the N most recent
/// log lines bottom-up (newest just above the command bar).  Painted
/// only when `UI_SCROLLBACK_EXPANDED` is true.
fn paint_scrollback() {
    let panel_top = (CMD_BAR_TOP - SCROLLBACK_H - 1) as usize;
    let panel_h = SCROLLBACK_H as usize;
    // Body fill + 1-px frame in DimWhite so it reads as a "deck".
    k_fb::fill_rect(0, panel_top, k_fb::WIDTH, panel_h, k_fb::Color::HeaderBar);
    k_fb::stroke_rect(0, panel_top, k_fb::WIDTH, panel_h, k_fb::Color::DimWhite);

    // Title pip in the top-left corner.
    k_fb::draw_text(4, panel_top + 1, "LOG", k_fb::Color::Highlight);
    let toggle = " F9 close";
    k_fb::draw_text(4 + 3 * 8, panel_top + 1, toggle, k_fb::Color::DimWhite);

    // Lines, packed bottom-up so the newest sits just above the cmd bar.
    let ui = UI_STATE.lock();
    let inset_x = 4usize;
    let line_h = 9usize; // 8 px glyph + 1 px gap
    // Reserve top 10 px for the title row.
    let usable_top = panel_top + 11;
    let usable_bot = panel_top + panel_h - 2;
    let max_rows = (usable_bot - usable_top) / line_h;
    // Pull the most recent `max_rows` lines.
    let total = ui.count;
    let skip = total.saturating_sub(max_rows);
    let visible_count = total - skip;
    let start_y = usable_bot - visible_count * line_h;
    let mut y = start_y;
    for (i, line) in ui.iter_oldest_first().enumerate() {
        if i < skip {
            continue;
        }
        let take = line.len().min(SCROLLBACK_LINE_CAP);
        let trimmed = unsafe { core::str::from_utf8_unchecked(&line.as_bytes()[..take]) };
        k_fb::draw_text(inset_x, y, trimmed, k_fb::Color::Foreground);
        y += line_h;
    }
}

/// Phase I.3.x sci-fi shading — map node type to the base palette
/// index of an 8-step Lambertian ramp.  Cube draw code adds the
/// per-face shade (0..7) to this base to pick the actual lit colour.
fn classify_node_hue(node: &GraphNodeSummary) -> u8 {
    match node.node_type {
        RuntimeNodeType::Hardware => k_fb::HUE_CYAN_BASE,
        RuntimeNodeType::Driver => k_fb::HUE_MAGENTA_BASE,
        RuntimeNodeType::Service => k_fb::HUE_YELLOW_BASE,
        RuntimeNodeType::PluginEntry | RuntimeNodeType::Compute => k_fb::HUE_MINT_BASE,
        _ => k_fb::HUE_ROSE_BASE,
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    raw_serial_println(format_args!("KERNEL PANIC"));
    if let Some(location) = info.location() {
        raw_serial_println(format_args!(
            "panic at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        ));
    }
    raw_serial_println(format_args!("{}", info));
    // Visual cue if the framebuffer is up: solid crimson with a tiny
    // amber band so a passer-by in QEMU can tell the kernel halted
    // even without serial visibility.
    //
    // Disable interrupts first, then bypass `k-fb`'s mutex via the
    // `force_*` no-lock variants: a panic mid-`paint_3d_view` would
    // otherwise spin-deadlock on the held `LOCK` and the crimson
    // screen would never paint.  See `k_fb::force_clear` safety
    // comment for the contract — the only caller that satisfies it
    // is this panic_handler.
    if k_fb::ready() {
        unsafe {
            x86_64::instructions::interrupts::disable();
            k_fb::force_clear(k_fb::Color::Error);
            k_fb::force_fill_rect(0, 0, k_fb::WIDTH, 8, k_fb::Color::Highlight);
        }
    }
    loop {
        x86_64::instructions::hlt();
    }
}

struct RawSerial;

impl Write for RawSerial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut port = x86_64::instructions::port::Port::<u8>::new(0x3F8);
        for byte in s.bytes() {
            unsafe {
                port.write(byte);
            }
        }
        Ok(())
    }
}

fn raw_serial_print(args: fmt::Arguments) {
    let _ = RawSerial.write_fmt(args);
}

pub(crate) fn raw_serial_println(args: fmt::Arguments) {
    raw_serial_print(format_args!("{}\n", args));
}
