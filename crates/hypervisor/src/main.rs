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
    paint_frame(0);
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
            paint_frame(frame_counter);
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
/// Bottom edge of the scene-body clipping window.  All node/edge
/// paint stays above this Y; the command bar + scrollback overlay
/// (I.5) own everything below.
const FOOTER_Y: i32 = 178;
/// Command-input bar geometry (I.5).  Always visible at the bottom
/// of the screen.  Slim 14 px row that holds `> <typed>_` with a
/// blinking cursor.
const CMD_BAR_TOP: i32 = 184;
const CMD_BAR_H: i32 = 14;
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
    for slot in 0..depth_count {
        let (i, d_sq) = depths[slot];
        let centre = node_world_position(i, returned_n);
        let hue_base = classify_node_hue(&nodes[i]);
        // sqrt → linear distance, then normalise.  Apply a soft curve
        // so middle-distance nodes don't dim too aggressively.
        let lin = libm::sqrtf(d_sq / max_depth_sq);
        let fog = (lin * lin * 0.9).clamp(0.0, 1.0);
        draw_node_solid(centre, hue_base, fog, &view_proj);
    }

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
    use core::cell::Cell;
    let pulse_phase = libm::sinf(frame as f32 * 0.22);
    let pulse_on = pulse_phase > -0.4; // ~70% duty cycle for "active" feel
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
        let style = edge_style(edges[i].edge_type);
        let step = Cell::new(0i32);
        let total_len_est = (tx - fx).abs().max((ty - fy).abs()).max(1);
        match style {
            EdgeStyle::Solid => {
                k_rast::draw_line(
                    |x, y| {
                        if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                            k_fb::put_pixel(x as usize, y as usize, color);
                        }
                    },
                    fx, fy, tx, ty,
                );
            }
            EdgeStyle::Dashed => {
                k_rast::draw_line(
                    |x, y| {
                        let t = step.get();
                        step.set(t + 1);
                        // 2 on, 2 off → period 4
                        if (t & 0x03) < 2
                            && x >= 0 && x < SCENE_WIDTH
                            && y >= HEADER_H && y < FOOTER_Y
                        {
                            k_fb::put_pixel(x as usize, y as usize, color);
                        }
                    },
                    fx, fy, tx, ty,
                );
            }
            EdgeStyle::Dotted => {
                k_rast::draw_line(
                    |x, y| {
                        let t = step.get();
                        step.set(t + 1);
                        // 1 on, 3 off → period 4
                        if (t & 0x03) == 0
                            && x >= 0 && x < SCENE_WIDTH
                            && y >= HEADER_H && y < FOOTER_Y
                        {
                            k_fb::put_pixel(x as usize, y as usize, color);
                        }
                    },
                    fx, fy, tx, ty,
                );
            }
            EdgeStyle::SolidPulsed => {
                let draw_color = if pulse_on { color } else { k_fb::Color::DimWhite };
                k_rast::draw_line(
                    |x, y| {
                        if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                            k_fb::put_pixel(x as usize, y as usize, draw_color);
                        }
                    },
                    fx, fy, tx, ty,
                );
            }
            EdgeStyle::GradientEnds => {
                // Bright color in the first/last quarter, DimWhite in
                // the middle half — communicates "metadata link" as
                // a soft thread between two solid endpoints.
                let q1 = total_len_est / 4;
                let q3 = total_len_est - q1;
                k_rast::draw_line(
                    |x, y| {
                        let t = step.get();
                        step.set(t + 1);
                        if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                            let c = if t < q1 || t > q3 { color } else { k_fb::Color::DimWhite };
                            k_fb::put_pixel(x as usize, y as usize, c);
                        }
                    },
                    fx, fy, tx, ty,
                );
            }
            EdgeStyle::DoubleSolid => {
                // Render the line twice with a 1-px perpendicular
                // offset to read as a "rail" — Mount is a structural
                // attachment and deserves visual weight.
                let dx = tx - fx;
                let dy = ty - fy;
                let len = libm::sqrtf((dx * dx + dy * dy) as f32).max(1.0);
                let ox = libm::roundf(-(dy as f32) / len) as i32;
                let oy = libm::roundf((dx as f32) / len) as i32;
                let put_solid = |x: i32, y: i32| {
                    if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                        k_fb::put_pixel(x as usize, y as usize, color);
                    }
                };
                k_rast::draw_line(put_solid, fx, fy, tx, ty);
                k_rast::draw_line(put_solid, fx + ox, fy + oy, tx + ox, ty + oy);
            }
        }
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

    // ── Gizmo: 3-axis orientation indicator ──────────────────────
    // Upper-right corner widget.  Projects the three world-space
    // axes through the camera's view (no translation, no perspective
    // — just the rotation effect) so the user always sees which
    // direction is X / Y / Z.  Reads as "orbit handle" even though
    // it isn't actually clickable; the whole empty-space area now
    // serves that role via the drag handler above.
    const GIZMO_CX: i32 = SCENE_WIDTH - 24;
    const GIZMO_CY: i32 = HEADER_H + 18;
    const GIZMO_LEN: f32 = 12.0;
    // Camera basis: the same three vectors look_at builds from
    // (eye, target=origin, up).  We don't need the full matrix —
    // the dot of each world axis with right / true_up / forward
    // gives its screen projection directly.
    let forward = Vec3::new(-eye.x, -eye.y, -eye.z).normalize();
    let world_up = Vec3::new(0.0, 1.0, 0.0);
    let right = forward.cross(world_up).normalize();
    let true_up = right.cross(forward);
    // Helper: project a world axis to gizmo-local screen coords.
    // Screen X = world_axis · right; screen Y flipped because image
    // space Y is down.
    let project_axis = |axis: Vec3| -> (i32, i32) {
        let sx = (axis.dot(right) * GIZMO_LEN) as i32;
        let sy = -(axis.dot(true_up) * GIZMO_LEN) as i32;
        (sx, sy)
    };
    let (ax_x, ay_x) = project_axis(Vec3::new(1.0, 0.0, 0.0));
    let (ax_y, ay_y) = project_axis(Vec3::new(0.0, 1.0, 0.0));
    let (ax_z, ay_z) = project_axis(Vec3::new(0.0, 0.0, 1.0));
    // Three colored Bresenham lines from gizmo centre.  R = X, G = Y,
    // B = Z (standard convention).  Brightness encodes whether axis
    // points toward camera (bright) or away (dim) via the forward
    // dot — gives the gizmo subtle depth cue.
    let axis_brightness = |axis: Vec3| -> k_fb::Color {
        let d = axis.dot(forward);
        if d > 0.3 {
            k_fb::Color::DimWhite
        } else {
            k_fb::Color::Foreground
        }
    };
    let draw_axis = |dx: i32, dy: i32, hue: k_fb::Color, _shade: k_fb::Color| {
        let put = |x: i32, y: i32| {
            if x >= 0 && x < SCENE_WIDTH && y >= HEADER_H && y < FOOTER_Y {
                k_fb::put_pixel(x as usize, y as usize, hue);
            }
        };
        k_rast::draw_line(put, GIZMO_CX, GIZMO_CY, GIZMO_CX + dx, GIZMO_CY + dy);
    };
    // X axis — drawn in NodeService (yellow) for warmth, since the
    // "red" we'd normally use is reserved for Error.
    draw_axis(
        ax_x,
        ay_x,
        k_fb::Color::NodeService,
        axis_brightness(Vec3::new(1.0, 0.0, 0.0)),
    );
    draw_axis(
        ax_y,
        ay_y,
        k_fb::Color::NodeApp,
        axis_brightness(Vec3::new(0.0, 1.0, 0.0)),
    );
    draw_axis(
        ax_z,
        ay_z,
        k_fb::Color::NodeKernel,
        axis_brightness(Vec3::new(0.0, 0.0, 1.0)),
    );
    // Centre marker pixel so the gizmo origin is visible even when
    // all three axes point sideways.
    k_fb::put_pixel(GIZMO_CX as usize, GIZMO_CY as usize, k_fb::Color::Foreground);

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

    // Header (I.4.4 refresh): brand chip on the left, three count
    // sections separated by ASCII bar glyphs.  Brand uses the brighter
    // foreground; counts use foreground; separators use DimWhite so
    // the eye sees the count numbers as the data.
    //
    //   GOS-KRN | 25 NOD | 63 EDG | G147
    //
    // 8 px font × 40 chars = 320 px exactly; the layout caps at 38
    // chars to leave a 2-char right margin.
    k_fb::draw_text(4, 3, "GOS-KRN", k_fb::Color::Highlight);
    k_fb::draw_text(4 + 7 * 8, 3, "|", k_fb::Color::DimWhite);
    let mut count_a = TextBuf::<10>::new();
    count_a.push_dec(returned_n as u64);
    count_a.push_str(" NOD");
    k_fb::draw_text(4 + 9 * 8, 3, count_a.as_str(), k_fb::Color::Foreground);
    k_fb::draw_text(4 + 16 * 8, 3, "|", k_fb::Color::DimWhite);
    let mut count_b = TextBuf::<10>::new();
    count_b.push_dec(snapshot.edge_count as u64);
    count_b.push_str(" EDG");
    k_fb::draw_text(4 + 18 * 8, 3, count_b.as_str(), k_fb::Color::Foreground);
    k_fb::draw_text(4 + 25 * 8, 3, "|", k_fb::Color::DimWhite);
    let mut count_c = TextBuf::<10>::new();
    count_c.push_str("G");
    count_c.push_dec(gos_runtime::graph_generation());
    k_fb::draw_text(4 + 27 * 8, 3, count_c.as_str(), k_fb::Color::Foreground);

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
            let step = Cell::new(0i32);
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

const NODE_HALF: f32 = 0.13;

/// 6 vertices: ±X, ±Y, ±Z poles of a unit octahedron.
/// Indices: 0=-X, 1=+X, 2=-Y, 3=+Y, 4=-Z, 5=+Z.
const OCTA_CORNERS_LOCAL: [(f32, f32, f32); 6] = [
    (-1.0, 0.0, 0.0),  // 0 -X
    (1.0, 0.0, 0.0),   // 1 +X
    (0.0, -1.0, 0.0),  // 2 -Y (bottom)
    (0.0, 1.0, 0.0),   // 3 +Y (top)
    (0.0, 0.0, -1.0),  // 4 -Z
    (0.0, 0.0, 1.0),   // 5 +Z
];

/// 8 triangular faces, CCW winding when viewed from outside.
/// Upper hemisphere fan around +Y, lower hemisphere fan around -Y.
const OCTA_TRIS: [[usize; 3]; 8] = [
    // Upper hemisphere (apex = 3 = +Y).
    [3, 5, 1], // +Y → +Z → +X
    [3, 1, 4], // +Y → +X → -Z
    [3, 4, 0], // +Y → -Z → -X
    [3, 0, 5], // +Y → -X → +Z
    // Lower hemisphere (apex = 2 = -Y).
    [2, 1, 5], // -Y → +X → +Z
    [2, 4, 1], // -Y → -Z → +X
    [2, 0, 4], // -Y → -X → -Z
    [2, 5, 0], // -Y → +Z → -X
];

/// Sci-fi octahedral crystal draw (I.4.1+I.4.2): per-face Lambertian
/// shading pulls a slot from the node's 8-step hue ramp; depth-fog
/// then biases that slot down for nodes near the far plane so the
/// scene reads in spatial layers.  Rim outline uses the brightest
/// shade so silhouettes pop against the dark background.
///
/// `fog`: 0.0 = full strength (near camera), 1.0 = fully faded
/// (at the far plane).  Subtracts up to ~4 shade slots so the
/// farthest nodes still draw but recede visually.
fn draw_node_solid(centre: Vec3, hue_base: u8, fog: f32, view_proj: &Mat4) {
    // Fixed key light: upper-right-front, biased toward Y so the
    // top facets read as the brightest in the default camera framing.
    const LIGHT: Vec3 = Vec3 { x: 0.55, y: 0.72, z: -0.42 };
    const LIGHT_LEN: f32 = 1.0;
    const AMBIENT: f32 = 0.18;

    // Project all 6 vertices.  Keep world coords for normal recovery.
    let mut screen = [(0i32, 0i32, true); 6];
    let mut world: [Vec3; 6] = [Vec3::new(0.0, 0.0, 0.0); 6];
    for j in 0..6 {
        let l = OCTA_CORNERS_LOCAL[j];
        world[j] = Vec3::new(
            centre.x + l.0 * NODE_HALF,
            centre.y + l.1 * NODE_HALF,
            centre.z + l.2 * NODE_HALF,
        );
        let clip = view_proj.transform_point(world[j]);
        match project_to_screen(clip, k_fb::WIDTH as u32, k_fb::HEIGHT as u32) {
            Some((sx, sy, _)) => screen[j] = (sx, sy, true),
            None => screen[j] = (0, 0, false),
        }
    }

    // Depth-fog: subtract up to 4 shade slots based on `fog`.  Clamp
    // so faces always pick a valid slot inside [0..=7].
    let fog_bias = (fog.clamp(0.0, 1.0) * 4.0) as u8;

    for tri in &OCTA_TRIS {
        let (x0, y0, ok0) = screen[tri[0]];
        let (x1, y1, ok1) = screen[tri[1]];
        let (x2, y2, ok2) = screen[tri[2]];
        if !(ok0 && ok1 && ok2) {
            continue;
        }
        // Screen-space back-face cull (CCW winding ⇒ positive area).
        let area2 = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        if area2 <= 0 {
            continue;
        }
        let v0 = world[tri[0]];
        let v1 = world[tri[1]];
        let v2 = world[tri[2]];
        let edge_a = v1.sub(v0);
        let edge_b = v2.sub(v0);
        let normal = edge_a.cross(edge_b).normalize();
        let n_dot_l = normal.dot(LIGHT) / LIGHT_LEN;
        let intensity = (n_dot_l * 0.5 + 0.5).max(AMBIENT).min(1.0);
        let shade_raw = (intensity * 7.999) as u8;
        let shade = shade_raw.saturating_sub(fog_bias).min(7);
        let palette_idx = hue_base + shade;
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

    // Outline: brightest shade, also fogged so distant rims dim too.
    let rim_shade = 7u8.saturating_sub(fog_bias).min(7);
    let rim_idx = hue_base + rim_shade;
    for tri in &OCTA_TRIS {
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
        }
    }

    fn append_char(&mut self, b: u8) {
        if self.line_len < CMD_LINE_CAP {
            self.line[self.line_len] = b;
            self.line_len += 1;
        }
    }

    fn backspace(&mut self) {
        if self.line_len > 0 {
            self.line_len -= 1;
        }
    }

    fn clear_line(&mut self) {
        self.line_len = 0;
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

/// Drain queued keystrokes from `k_fb::pop_typed_char`, applying
/// them to the input line.  Enter submits the line through
/// `interpret_command`; Esc toggles mode; Backspace edits.
fn drain_ui_input() {
    while let Some(b) = k_fb::pop_typed_char() {
        match b {
            b'\r' | b'\n' => {
                // Snapshot the line, clear it, then interpret.  We
                // copy out so `interpret_command` can hold the lock
                // again to append to the scrollback.
                let mut buf = [0u8; CMD_LINE_CAP];
                let len;
                {
                    let mut ui = UI_STATE.lock();
                    len = ui.line_len;
                    buf[..len].copy_from_slice(&ui.line[..len]);
                    ui.clear_line();
                }
                // SAFETY: only printable ASCII written via append_char.
                let line = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
                interpret_command(line);
            }
            0x08 => UI_STATE.lock().backspace(),
            0x1B => {
                // Esc — toggle mode.
                use core::sync::atomic::Ordering;
                let cur = k_fb::UI_MODE.load(Ordering::Relaxed);
                let next = if cur == k_fb::UI_MODE_KERNEL_VIEW {
                    k_fb::UI_MODE_OS_SHELL
                } else {
                    k_fb::UI_MODE_KERNEL_VIEW
                };
                k_fb::UI_MODE.store(next, Ordering::Relaxed);
            }
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
        echo.push_str("> ");
        let take = line.len().min(56);
        echo.push_str(unsafe { core::str::from_utf8_unchecked(&line.as_bytes()[..take]) });
        UI_STATE.lock().log(echo.as_str());
    }

    if line.is_empty() {
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
            ui.log("  nodes / edges     graph stats");
            ui.log("  gen               graph generation");
            ui.log("  log               toggle scrollback (F9)");
            ui.log("  clear             wipe scrollback");
            ui.log("  Esc               toggle mode");
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

/// Top-level paint coordinator.  Drains input, paints the header
/// + body (mode-dependent) + scrollback (when expanded) + command
/// bar.  Replaces the direct `paint_3d_view` call from boot.
fn paint_frame(frame: u64) {
    if !k_fb::ready() {
        return;
    }
    drain_ui_input();

    use core::sync::atomic::Ordering;
    let mode = k_fb::UI_MODE.load(Ordering::Relaxed);
    let scrollback_open = k_fb::UI_SCROLLBACK_EXPANDED.load(Ordering::Relaxed);

    k_fb::clear(k_fb::Color::Background);
    k_fb::fill_rect(0, 0, k_fb::WIDTH, HEADER_H as usize, k_fb::Color::HeaderBar);

    match mode {
        k_fb::UI_MODE_KERNEL_VIEW => paint_3d_view(frame),
        _ => paint_os_shell_body(frame),
    }

    if scrollback_open {
        paint_scrollback();
    }
    paint_command_bar(frame, mode);

    k_fb::present();
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

    k_fb::draw_text(4, 3, "GOS-OS", k_fb::Color::Highlight);
    k_fb::draw_text(4 + 6 * 8, 3, "|", k_fb::Color::DimWhite);
    let mut count_a = TextBuf::<14>::new();
    count_a.push_dec(returned_n as u64);
    count_a.push_str(" NOD");
    k_fb::draw_text(4 + 8 * 8, 3, count_a.as_str(), k_fb::Color::Foreground);
    k_fb::draw_text(4 + 15 * 8, 3, "|", k_fb::Color::DimWhite);
    let mut count_b = TextBuf::<14>::new();
    count_b.push_dec(snap.edge_count as u64);
    count_b.push_str(" EDG");
    k_fb::draw_text(4 + 17 * 8, 3, count_b.as_str(), k_fb::Color::Foreground);
    k_fb::draw_text(4 + 24 * 8, 3, "|", k_fb::Color::DimWhite);
    k_fb::draw_text(4 + 26 * 8, 3, "shell", k_fb::Color::NodeService);

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
fn paint_command_bar(frame: u64, mode: u8) {
    let bar_y = CMD_BAR_TOP as usize;
    let bar_h = CMD_BAR_H as usize;
    k_fb::fill_rect(0, bar_y, k_fb::WIDTH, bar_h, k_fb::Color::HeaderBar);
    k_fb::hline(0, bar_y, k_fb::WIDTH, k_fb::Color::DimWhite);

    // Prompt and typed text.
    let prompt_x = 4usize;
    let prompt_y = bar_y + 3;
    k_fb::draw_text(prompt_x, prompt_y, ">", k_fb::Color::Highlight);

    let ui = UI_STATE.lock();
    let line = ui.current_line();
    let line_len = line.len();
    let text_x = prompt_x + 2 * 8;
    // Truncate from the head if the user types past the visible width.
    let max_visible_chars = ((k_fb::WIDTH - text_x - 12) / 8).min(CMD_LINE_CAP);
    let visible_start = line_len.saturating_sub(max_visible_chars);
    let visible = unsafe {
        core::str::from_utf8_unchecked(&line.as_bytes()[visible_start..line_len])
    };
    k_fb::draw_text(text_x, prompt_y, visible, k_fb::Color::Foreground);

    // Blinking cursor: 1-px solid line under the insertion point.
    let cursor_on = (frame / 16) & 1 == 0;
    if cursor_on {
        let cursor_x = text_x + (line_len - visible_start) * 8;
        if cursor_x + 6 < k_fb::WIDTH {
            k_fb::fill_rect(cursor_x, prompt_y, 6, 8, k_fb::Color::Foreground);
        }
    }

    // Right-side mode pill: shows `[KRN]` or `[SHELL]` so the user
    // always knows which view is current.
    let pill = match mode {
        k_fb::UI_MODE_KERNEL_VIEW => "[KRN]",
        _ => "[SHELL]",
    };
    let px = k_fb::WIDTH - pill.len() * 8 - 4;
    k_fb::draw_text(px, prompt_y, pill, k_fb::Color::NodeService);
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
