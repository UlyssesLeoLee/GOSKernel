#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;
mod ring3;

use bootloader::{entry_point, BootInfo};
use core::fmt::{self, Write};
use gos_protocol::{GraphEdgeSummary, GraphNodeSummary, RuntimeNodeType, VectorAddress};
use k_rast::{project_to_screen, sort_by_depth_desc, Mat4, Vec3};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    raw_serial_println(format_args!("boot: kernel_main entered"));

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
    raw_serial_println(format_args!("boot: cpu features enabled"));

    // Minimal bootstrap only owns compatibility addressing and metadata schemas.
    gos_hal::vaddr::init();
    gos_hal::meta::init();
    // Store the physical memory offset for DMA address translation in k-net and other drivers.
    gos_hal::phys::set_phys_offset(boot_info.physical_memory_offset);
    raw_serial_println(format_args!("boot: vaddr/meta initialized, phys_offset={:#x}", boot_info.physical_memory_offset));

    // Phase I.3.1 — bring up the VGA mode-13h framebuffer (the
    // bootloader's `vga_320x200` feature switched the card before
    // entering `kernel_main`).  Clearing to Background immediately
    // gives any observer in QEMU a "kernel is alive" signal instead
    // of the leftover BIOS splash.
    unsafe {
        k_fb::init(boot_info.physical_memory_offset);
    }
    k_fb::clear(k_fb::Color::Background);
    // Header bar serves as the boot-progress indicator: dim teal
    // throughout init, becomes solid at "entering steady-state".
    k_fb::fill_rect(0, 0, k_fb::WIDTH, 18, k_fb::Color::HeaderBar);
    raw_serial_println(format_args!("boot: framebuffer up (mode 13h, 320x200)"));

    raw_serial_println(format_args!("boot: staging supervisor domains"));
    gos_supervisor::bootstrap(boot_info as *const _ as u64);
    for descriptor in builtin_bundle::builtin_supervisor_modules() {
        gos_supervisor::install_module(*descriptor)
            .expect("supervisor failed to install module descriptor");
    }
    raw_serial_println(format_args!("boot: supervisor registered module descriptors"));

    raw_serial_println(format_args!("boot: bootstrapping builtin graph"));
    let report = builtin_bundle::boot_builtin_graph(boot_info as *const _ as u64)
        .expect("builtin graph boot failed");
    raw_serial_println(format_args!("boot: builtin graph booted"));

    let supervisor_report = gos_supervisor::realize_boot_modules()
        .expect("supervisor failed to realize isolated domains");
    raw_serial_println(format_args!("boot: supervisor staged isolated domains"));

    k_serial::serial_println!(
        "supervisor modules={} running={} domains={} caps={}",
        supervisor_report.discovered_modules,
        supervisor_report.running_modules,
        supervisor_report.isolated_domains,
        supervisor_report.published_capabilities
    );

    k_serial::serial_println!("\n=== GOS v0.2 BUNDLE LOAD ===");
    k_serial::serial_println!(
        "plugins discovered={} loaded={} stable={}",
        report.discovered_plugins,
        report.loaded_plugins,
        report.stable_after_load
    );

    let snapshot = gos_runtime::snapshot();
    k_serial::serial_println!(
        "runtime nodes={} edges={} ready={} signals={}",
        snapshot.node_count,
        snapshot.edge_count,
        snapshot.ready_queue_len,
        snapshot.signal_queue_len
    );

    // Phase G.1: synchronously initialize kernel-tier drivers (GDT,
    // IDT, PIC) before interrupts come up.  Builtin modules' on_init
    // never ran via runtime pump because their ModuleEntry was None;
    // hardware setup must happen on the direct path here.
    raw_serial_println(format_args!("boot: kernel-tier drivers init"));
    builtin_bundle::init_kernel_tier_drivers();
    raw_serial_println(format_args!("boot: kernel-tier drivers ready (GDT/IDT/PIC)"));

    // Phase E.2: program the syscall MSRs once the GDT is live.
    raw_serial_println(format_args!("boot: arming ring3 syscall surface"));
    unsafe { ring3::init(); }
    raw_serial_println(format_args!("boot: ring3 syscall surface armed"));

    // Phase I.3.8 — first 3D paint before going interactive.  Idle
    // loop below repaints continuously to keep the camera rotation
    // smooth.
    paint_3d_view(0);
    raw_serial_println(format_args!("boot: framebuffer 3D scene painted"));

    raw_serial_println(format_args!("boot: enabling interrupts; entering steady-state"));
    x86_64::instructions::interrupts::enable();

    // I.3.5 + I.3.8 — paint loop.  Every PIT-driven idle iteration
    // bumps `frame_counter` and advances the camera's yaw; graph
    // mutations (Cypher LINK et al) bump `graph_generation` which
    // schedules an immediate full repaint.  In between, we still
    // repaint every `REPAINT_TICKS` iterations to keep the cube
    // rotation smooth.
    const REPAINT_TICKS: u64 = 2;
    let mut last_painted_gen = gos_runtime::graph_generation();
    let mut frame_counter: u64 = 0;
    loop {
        x86_64::instructions::interrupts::without_interrupts(|| {
            gos_supervisor::service_system_cycle();
        });
        frame_counter = frame_counter.wrapping_add(1);
        let gen_now = gos_runtime::graph_generation();
        let graph_dirty = gen_now != last_painted_gen;
        if graph_dirty || frame_counter % REPAINT_TICKS == 0 {
            paint_3d_view(frame_counter);
            last_painted_gen = gen_now;
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
const HEADER_H: i32 = 14;
const FOOTER_Y: i32 = 192;
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

use core::sync::atomic::{AtomicI8, AtomicU8 as AtomicU8Btn};
static SELECTED_NODE_SLOT: AtomicI8 = AtomicI8::new(-1);
static MOUSE_PREV_BTN: AtomicU8Btn = AtomicU8Btn::new(0);

/// Half-extent in screen pixels used for hit-testing each cube.
/// Larger than the rendered cube on screen so the user has a
/// reasonable click target even when the perspective shrinks
/// far-side cubes.
const HIT_HALF_PX: i32 = 8;

fn paint_3d_view(frame: u64) {
    if !k_fb::ready() {
        return;
    }

    let snapshot = gos_runtime::snapshot();
    let mut nodes = [GraphNodeSummary::EMPTY; MAX_NODES];
    let (_total_n, returned_n) = gos_runtime::node_page(0, &mut nodes);
    let mut edges = [GraphEdgeSummary::EMPTY; MAX_EDGES];
    let (_total_e, returned_e) = gos_runtime::edge_page(0, &mut edges);

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

    // Background + header band + footer band so the body region paints
    // fresh every frame (no leftover ghosts from the previous tick).
    k_fb::clear(k_fb::Color::Background);
    k_fb::fill_rect(0, 0, k_fb::WIDTH, HEADER_H as usize, k_fb::Color::HeaderBar);

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

    // Cubes (painter's order: far first, near last).
    for slot in 0..depth_count {
        let i = depths[slot].0;
        let centre = node_world_position(i, returned_n);
        let hue_base = classify_node_hue(&nodes[i]);
        draw_cube(centre, hue_base, &view_proj);
    }

    // Edges: lines between projected centres.  Drawn AFTER cubes so
    // they always read as overlay.  Lines are short enough that we
    // don't bother with depth-aware drawing.
    for i in 0..returned_e {
        let from_idx = find_node_index(&nodes[..returned_n], edges[i].from_vector);
        let to_idx = find_node_index(&nodes[..returned_n], edges[i].to_vector);
        let (Some(fi), Some(ti)) = (from_idx, to_idx) else { continue };
        let (fx, fy, fok) = node_centre_screen[fi];
        let (tx, ty, tok) = node_centre_screen[ti];
        if !(fok && tok) {
            continue;
        }
        let color = classify_edge(edges[i].edge_type);
        k_rast::draw_line(
            |x, y| {
                if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                    k_fb::put_pixel(x as usize, y as usize, color);
                }
            },
            fx,
            fy,
            tx,
            ty,
        );
    }

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
        // Drop the K_ prefix (every kernel module is K_FOO) for legibility.
        let trimmed = name.strip_prefix("K_").unwrap_or(name);
        let bytes = trimmed.as_bytes();
        let label_len = bytes.len().min(6);
        let label = unsafe { core::str::from_utf8_unchecked(&bytes[..label_len]) };
        // Position: a few pixels above the cube top, centred-ish.
        // 6 chars × 8 px = 48 px wide; cube screen height varies with
        // distance, so use a fixed 14 px offset.
        let text_w = (label_len * 8) as i32;
        let lx = (sx - text_w / 2).max(0) as usize;
        let ly = (sy - 16).max(HEADER_H + 1) as usize;
        if lx + (label_len * 8) > k_fb::WIDTH || ly + 8 >= FOOTER_Y as usize {
            continue;
        }
        // Two-pass shadow so the label reads on any cube colour: a
        // 1-pixel offset Background outline behind the Foreground.
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let bx = lx as i32 + dx;
            let by = ly as i32 + dy;
            if bx >= 0 && by >= 0 {
                k_fb::draw_text(bx as usize, by as usize, label, k_fb::Color::Background);
            }
        }
        k_fb::draw_text(lx, ly, label, k_fb::Color::Foreground);
    }

    // ── I.3.11 — mouse cursor + click-to-select ──────────────────
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
    // empty space).
    if left_edge {
        SELECTED_NODE_SLOT.store(hover_slot, Ordering::Relaxed);
    }
    let selected = SELECTED_NODE_SLOT.load(Ordering::Relaxed);

    // Cursor crosshair: 5-pixel '+' in Highlight, clipped to screen.
    if mouse_x >= 0 && mouse_x < SCENE_WIDTH && mouse_y >= 0 && mouse_y < SCENE_HEIGHT {
        for dx in -2..=2 {
            let cx = mouse_x + dx;
            if cx >= 0 && cx < SCENE_WIDTH && mouse_y >= HEADER_H && mouse_y < FOOTER_Y {
                k_fb::put_pixel(cx as usize, mouse_y as usize, k_fb::Color::Highlight);
            }
        }
        for dy in -2..=2 {
            let cy = mouse_y + dy;
            if cy >= HEADER_H && cy < FOOTER_Y && mouse_x >= 0 && mouse_x < SCENE_WIDTH {
                k_fb::put_pixel(mouse_x as usize, cy as usize, k_fb::Color::Highlight);
            }
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

    // Detail panel: bottom-right corner for the SELECTED node.
    // 132×40 box with 3 text rows (plugin, vector, type).
    if selected >= 0 && (selected as usize) < returned_n {
        let node = &nodes[selected as usize];
        const PANEL_W: usize = 132;
        const PANEL_H: usize = 44;
        let px = k_fb::WIDTH - PANEL_W - 2;
        let py = FOOTER_Y as usize - PANEL_H - 2;
        k_fb::fill_rect(px, py, PANEL_W, PANEL_H, k_fb::Color::HeaderBar);
        k_fb::stroke_rect(px, py, PANEL_W, PANEL_H, k_fb::Color::DimWhite);

        // Row 1: plugin name (full)
        let mut row1 = TextBuf::<20>::new();
        row1.push_str(node.plugin_name);
        k_fb::draw_text(px + 4, py + 4, row1.as_str(), k_fb::Color::Foreground);

        // Row 2: vector address
        let mut row2 = TextBuf::<24>::new();
        row2.push_dec(node.vector.l4 as u64);
        row2.push_str(".");
        row2.push_dec(node.vector.l3 as u64);
        row2.push_str(".");
        row2.push_dec(node.vector.l2 as u64);
        row2.push_str(".");
        row2.push_dec(node.vector.offset as u64);
        k_fb::draw_text(px + 4, py + 16, row2.as_str(), k_fb::Color::Foreground);

        // Row 3: node type
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
        let mut row3 = TextBuf::<24>::new();
        row3.push_str("TYPE: ");
        row3.push_str(type_label);
        k_fb::draw_text(px + 4, py + 28, row3.as_str(), k_fb::Color::Foreground);
    }

    // Header text: "GOS  N NOD  M EDG  G<gen>"
    let mut hdr = TextBuf::<40>::new();
    hdr.push_str("GOS  ");
    hdr.push_dec(returned_n as u64);
    hdr.push_str(" NOD  ");
    hdr.push_dec(snapshot.edge_count as u64);
    hdr.push_str(" EDG  G");
    hdr.push_dec(gos_runtime::graph_generation());
    k_fb::draw_text(4, 3, hdr.as_str(), k_fb::Color::Foreground);

    // Footer status.
    k_fb::hline(0, FOOTER_Y as usize, k_fb::WIDTH, k_fb::Color::DimWhite);
    let mut footer = TextBuf::<48>::new();
    footer.push_str(if auto_on { "3D  " } else { "PAUSE  " });
    footer.push_str("F1 PAUSE  F2/3 YAW  F4/5 PIT  F7/8 Z  F6 RST");
    k_fb::draw_text(4, 192, footer.as_str(), k_fb::Color::Foreground);
    let _ = snapshot; // RDY/SIG counters can return when there's row space

    k_fb::present();
}

/// World-space centre for the i-th node.  Lays nodes out on a 3D
/// grid centred at the origin.  Spread tuned so the scene fits the
/// frustum at the camera radius used above.
fn node_world_position(i: usize, total: usize) -> Vec3 {
    // Use isqrt to pick a square grid in the XZ plane; Y is the
    // index `mod 3` so we get some vertical separation that reads
    // as 3D when the camera orbits.
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

const CUBE_HALF: f32 = 0.10;

/// 8 corners of a unit cube centred at the origin (the per-node
/// world transform just translates).  Order matches `CUBE_FACES`
/// below.
const CUBE_CORNERS_LOCAL: [(f32, f32, f32); 8] = [
    (-1.0, -1.0, -1.0),
    (1.0, -1.0, -1.0),
    (1.0, 1.0, -1.0),
    (-1.0, 1.0, -1.0),
    (-1.0, -1.0, 1.0),
    (1.0, -1.0, 1.0),
    (1.0, 1.0, 1.0),
    (-1.0, 1.0, 1.0),
];

/// 12 triangles, 2 per face, CCW winding when viewed from outside.
const CUBE_TRIS: [[usize; 3]; 12] = [
    // -z face
    [0, 1, 2], [0, 2, 3],
    // +z face
    [4, 6, 5], [4, 7, 6],
    // -y face
    [0, 4, 5], [0, 5, 1],
    // +y face
    [3, 2, 6], [3, 6, 7],
    // -x face
    [0, 3, 7], [0, 7, 4],
    // +x face
    [1, 5, 6], [1, 6, 2],
];

/// Sci-fi cube draw: per-face Lambertian shading against a fixed
/// world-space "key light" pulls each face's shade slot from the
/// node's 8-step palette ramp, so cubes read as 3D-lit solids
/// instead of flat sprites.  Outline accent uses the brightest
/// shade so silhouettes glow.
fn draw_cube(centre: Vec3, hue_base: u8, view_proj: &Mat4) {
    // Fixed key light, normalised once at compile-time-ish.  Coming
    // from upper-right-front; bias toward Y so the top face is the
    // brightest in the default camera framing.
    const LIGHT: Vec3 = Vec3 { x: 0.55, y: 0.72, z: -0.42 };
    const LIGHT_LEN: f32 = 1.0; // pre-normalised in choice of components
    const AMBIENT: f32 = 0.18;

    // Project all 8 corners.  Also keep world coords so we can build
    // each triangle's geometric normal for shading.
    let mut screen = [(0i32, 0i32, true); 8];
    let mut world: [Vec3; 8] = [Vec3::new(0.0, 0.0, 0.0); 8];
    for j in 0..8 {
        let l = CUBE_CORNERS_LOCAL[j];
        world[j] = Vec3::new(
            centre.x + l.0 * CUBE_HALF,
            centre.y + l.1 * CUBE_HALF,
            centre.z + l.2 * CUBE_HALF,
        );
        let clip = view_proj.transform_point(world[j]);
        match project_to_screen(clip, k_fb::WIDTH as u32, k_fb::HEIGHT as u32) {
            Some((sx, sy, _)) => screen[j] = (sx, sy, true),
            None => screen[j] = (0, 0, false),
        }
    }

    for tri in &CUBE_TRIS {
        let (x0, y0, ok0) = screen[tri[0]];
        let (x1, y1, ok1) = screen[tri[1]];
        let (x2, y2, ok2) = screen[tri[2]];
        if !(ok0 && ok1 && ok2) {
            continue;
        }
        // Screen-space back-face cull.
        let area2 = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        if area2 <= 0 {
            continue;
        }
        // World-space face normal: (v1-v0) × (v2-v0), normalised.
        let v0 = world[tri[0]];
        let v1 = world[tri[1]];
        let v2 = world[tri[2]];
        let edge_a = v1.sub(v0);
        let edge_b = v2.sub(v0);
        let normal = edge_a.cross(edge_b).normalize();
        // Lambertian intensity: clamp to [AMBIENT, 1.0].  Light is
        // unit-length so no extra divide.  Bias slightly so totally
        // back-facing surfaces still have some shadow tone.
        let n_dot_l = normal.dot(LIGHT) / LIGHT_LEN;
        let intensity = (n_dot_l * 0.5 + 0.5).max(AMBIENT).min(1.0);
        // Map to one of 8 shading slots (0 darkest, 7 brightest).
        let shade = (intensity * 7.999) as u8;
        let palette_idx = hue_base + shade.min(7);
        k_rast::fill_triangle(
            |x, y| {
                if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                    k_fb::put_pixel_raw(x as usize, y as usize, palette_idx);
                }
            },
            SCENE_WIDTH,
            SCENE_HEIGHT,
            (x0, y0),
            (x1, y1),
            (x2, y2),
        );
    }

    // Outline the cube silhouette using the brightest shade of this
    // node's ramp — gives every cube a subtle neon rim consistent
    // with its hue rather than a uniform grey frame.
    let rim_idx = hue_base + 7;
    for tri in &CUBE_TRIS {
        let (x0, y0, ok0) = screen[tri[0]];
        let (x1, y1, ok1) = screen[tri[1]];
        let (x2, y2, ok2) = screen[tri[2]];
        if !(ok0 && ok1 && ok2) {
            continue;
        }
        let area2 = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        if area2 <= 0 {
            continue;
        }
        let put = |x: i32, y: i32| {
            if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                k_fb::put_pixel_raw(x as usize, y as usize, rim_idx);
            }
        };
        k_rast::draw_line(put, x0, y0, x1, y1);
        k_rast::draw_line(put, x1, y1, x2, y2);
        k_rast::draw_line(put, x2, y2, x0, y0);
    }
}

fn find_node_index(
    nodes: &[GraphNodeSummary],
    vector: VectorAddress,
) -> Option<usize> {
    nodes.iter().position(|n| n.vector == vector)
}

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
            unsafe { port.write(byte); }
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
