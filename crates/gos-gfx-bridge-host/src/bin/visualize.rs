//! `gos-visualize` — Phase I.2.2 startable, rotatable 3D graph view.
//!
//! Run with: `rustup run stable cargo run --bin gos-visualize`
//! (from the bridge-host crate dir).
//!
//! Opens a real window via winit, attaches a wgpu surface, generates
//! a small set of synthetic graph nodes (each rendered as a small
//! cube via `k_scene::write_cube_*`), and runs a continuous render
//! loop:
//!
//!   * Camera orbits the origin at a constant radius.  Yaw auto-
//!     advances each frame; arrow keys nudge yaw/pitch manually.
//!     Mouse-drag rotation is a follow-up.
//!   * View + perspective matrices are uploaded each frame as a 64-
//!     byte push constant; the shader lives in `k-scene::NODE_CUBE_WGSL`.
//!
//! This binary intentionally bypasses the `RenderBackend` trait /
//! `WgpuBackend` headless path used by the unit tests — windowed
//! rendering and the carrier-bridged frame-loop API will converge
//! in a Phase I.x slice once the wire format gains push-constant /
//! uniform commands.  For Gen-1 the demo is a thin wgpu app that
//! consumes k-scene's *geometry helpers* as plain byte producers.
//!
//! Tested manually on Windows + DX12; CI gates the library half via
//! the existing harness suites.

use std::sync::Arc;
use std::time::Instant;

use k_scene::{
    cube_index_buffer_bytes_for, cube_vertex_buffer_bytes_for, write_cube_indices,
    write_cube_vertices, BYTES_PER_CUBE_VERTEX, INDICES_PER_CUBE, NODE_CUBE_WGSL, SceneNode,
};
use gos_protocol::VectorAddress;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

const NODE_COUNT: usize = 64;
const WINDOW_W: u32 = 1024;
const WINDOW_H: u32 = 768;

/// Camera state.  Orbit form: yaw + pitch around origin at fixed
/// radius.  Auto-yaw advances every frame; arrow keys add to yaw /
/// pitch deltas.
#[derive(Debug, Clone, Copy)]
struct Camera {
    radius: f32,
    yaw: f32,
    pitch: f32,
    /// Per-frame yaw drift (radians).  Zero pauses auto-rotate.
    auto_yaw_per_sec: f32,
    fov_y: f32,
    aspect: f32,
    near: f32,
    far: f32,
}

impl Camera {
    fn new(aspect: f32) -> Self {
        Self {
            radius: 3.0,
            yaw: 0.6,
            pitch: 0.4,
            auto_yaw_per_sec: 0.4,
            fov_y: 60.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 100.0,
        }
    }

    fn view_proj(&self) -> [[f32; 4]; 4] {
        let eye = [
            self.radius * self.pitch.cos() * self.yaw.sin(),
            self.radius * self.pitch.sin(),
            self.radius * self.pitch.cos() * self.yaw.cos(),
        ];
        let view = look_at(eye, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let proj = perspective(self.fov_y, self.aspect, self.near, self.far);
        mat4_mul(proj, view)
    }
}

fn main() {
    // wgpu writes its diagnostics straight to stderr via its built-in
    // logger init when no other subscriber is installed, so we don't
    // pull env_logger into this binary's dep graph.
    let event_loop = EventLoop::new().expect("create event loop");
    // `Arc<Window>` so the wgpu surface can hold a `'static` lifetime
    // (wgpu 0.20 accepts `Arc<Window>` via `Into<SurfaceTarget>` and
    // returns `Surface<'static>`) while the event-loop closure also
    // moves the Arc for `request_redraw`.
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("gos-visualize · Phase I.2.2")
            .with_inner_size(winit::dpi::PhysicalSize::new(WINDOW_W, WINDOW_H))
            .build(&event_loop)
            .expect("create window"),
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let surface = instance
        .create_surface(window.clone())
        .expect("create surface");
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("request adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("gos-visualize"),
            required_features: wgpu::Features::PUSH_CONSTANTS,
            required_limits: wgpu::Limits {
                max_push_constant_size: 64,
                ..wgpu::Limits::downlevel_defaults()
            },
        },
        None,
    ))
    .expect("request device");

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);
    let mut surface_cfg = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: WINDOW_W,
        height: WINDOW_H,
        present_mode: surface_caps
            .present_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::PresentMode::Mailbox)
            .unwrap_or(wgpu::PresentMode::Fifo),
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_cfg);

    // Depth attachment — without it, back faces of the cube draw on
    // top of front faces depending on submission order.
    let mut depth_view = create_depth(&device, surface_cfg.width, surface_cfg.height);

    // ---- Geometry from k-scene ---------------------------------------
    let nodes = synth_nodes(NODE_COUNT);
    let mut vbuf_bytes = vec![0u8; cube_vertex_buffer_bytes_for(NODE_COUNT)];
    let mut ibuf_bytes = vec![0u8; cube_index_buffer_bytes_for(NODE_COUNT)];
    write_cube_vertices(&nodes, &mut vbuf_bytes);
    write_cube_indices(NODE_COUNT, &mut ibuf_bytes);

    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cube-vertices"),
        size: vbuf_bytes.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertex_buffer, 0, &vbuf_bytes);
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cube-indices"),
        size: ibuf_bytes.len() as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&index_buffer, 0, &ibuf_bytes);

    // ---- Pipeline ----------------------------------------------------
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("cube-shader"),
        source: wgpu::ShaderSource::Wgsl(NODE_CUBE_WGSL.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cube-pipeline-layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::VERTEX,
            range: 0..64,
        }],
    });
    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: BYTES_PER_CUBE_VERTEX as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: (4 * 3) as wgpu::BufferAddress,
                shader_location: 1,
            },
        ],
    };
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("cube-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_layout],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let mut camera = Camera::new(WINDOW_W as f32 / WINDOW_H as f32);
    let mut last_frame = Instant::now();
    let mut manual_yaw: f32 = 0.0;
    let mut manual_pitch: f32 = 0.0;
    let mut auto_rotate_on = true;
    println!(
        "gos-visualize: window up, {} cubes; arrow keys orbit, Space pauses auto-rotate, Esc quits",
        NODE_COUNT
    );

    event_loop
        .run(move |event, target| {
            target.set_control_flow(ControlFlow::Poll);
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => target.exit(),
                    WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                        surface_cfg.width = size.width;
                        surface_cfg.height = size.height;
                        surface.configure(&device, &surface_cfg);
                        depth_view = create_depth(&device, size.width, size.height);
                        camera.aspect = size.width as f32 / size.height as f32;
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(code),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    } => match code {
                        KeyCode::Escape => target.exit(),
                        KeyCode::ArrowLeft => manual_yaw -= 0.1,
                        KeyCode::ArrowRight => manual_yaw += 0.1,
                        KeyCode::ArrowUp => manual_pitch += 0.1,
                        KeyCode::ArrowDown => manual_pitch -= 0.1,
                        KeyCode::Space => auto_rotate_on = !auto_rotate_on,
                        _ => {}
                    },
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last_frame).as_secs_f32();
                        last_frame = now;
                        if auto_rotate_on {
                            camera.yaw += camera.auto_yaw_per_sec * dt;
                        }
                        camera.yaw += manual_yaw;
                        camera.pitch =
                            (camera.pitch + manual_pitch).clamp(-1.4, 1.4);
                        manual_yaw = 0.0;
                        manual_pitch = 0.0;

                        let view_proj = camera.view_proj();
                        let push_bytes: [u8; 64] = mat4_to_le_bytes(view_proj);

                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                surface.configure(&device, &surface_cfg);
                                return;
                            }
                            Err(e) => {
                                eprintln!("surface error: {:?}", e);
                                return;
                            }
                        };
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("frame"),
                            },
                        );
                        {
                            let mut pass = encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("main"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                                    r: 0.04,
                                                    g: 0.05,
                                                    b: 0.08,
                                                    a: 1.0,
                                                }),
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: Some(
                                        wgpu::RenderPassDepthStencilAttachment {
                                            view: &depth_view,
                                            depth_ops: Some(wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(1.0),
                                                store: wgpu::StoreOp::Store,
                                            }),
                                            stencil_ops: None,
                                        },
                                    ),
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                },
                            );
                            pass.set_pipeline(&pipeline);
                            pass.set_push_constants(
                                wgpu::ShaderStages::VERTEX,
                                0,
                                &push_bytes,
                            );
                            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                            pass.set_index_buffer(
                                index_buffer.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            pass.draw_indexed(
                                0..(NODE_COUNT * INDICES_PER_CUBE) as u32,
                                0,
                                0..1,
                            );
                        }
                        queue.submit(Some(encoder.finish()));
                        frame.present();

                        window.request_redraw();
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .expect("event loop");
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn synth_nodes(n: usize) -> Vec<SceneNode> {
    (0..n)
        .map(|i| SceneNode {
            vector: VectorAddress::new(
                ((i >> 24) & 0xFF) as u8,
                ((i >> 16) & 0xFFFF) as u16,
                ((i >> 8) & 0xFFFF) as u16,
                (i & 0xFFFF) as u16,
            ),
            color: [
                ((i % 7) as f32) / 6.0,
                ((i % 11) as f32) / 10.0,
                ((i % 13) as f32) / 12.0,
            ],
        })
        .collect()
}

// ── Minimal mat4 column-major math (no glam dep) ───────────────────

fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Row-major multiplication; both inputs and the result are stored
    // as `[row][col]`.  Push-constant layout reorders into column-
    // major in `mat4_to_le_bytes`.
    let mut out = [[0.0f32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[r][k] * b[k][c];
            }
            out[r][c] = s;
        }
    }
    out
}

fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    // Right-handed, z in [0,1] (wgpu convention).
    let f = 1.0 / (fov_y / 2.0).tan();
    let nf = 1.0 / (near - far);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far * nf, -1.0],
        [0.0, 0.0, far * near * nf, 0.0],
    ]
}

fn look_at(eye: [f32; 3], centre: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = vec3_normalize(vec3_sub(centre, eye));
    let s = vec3_normalize(vec3_cross(f, up));
    let u = vec3_cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [
            -vec3_dot(s, eye),
            -vec3_dot(u, eye),
            vec3_dot(f, eye),
            1.0,
        ],
    ]
}

fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = vec3_dot(v, v).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn mat4_to_le_bytes(m: [[f32; 4]; 4]) -> [u8; 64] {
    // WGSL `mat4x4<f32>` push-constant memory layout is column-major
    // (each column is a `vec4<f32>` aligned to 16 bytes).  Our `m`
    // is `[row][col]` — transpose during the copy.
    let mut out = [0u8; 64];
    let mut off = 0;
    for col in 0..4 {
        for row in 0..4 {
            out[off..off + 4].copy_from_slice(&m[row][col].to_le_bytes());
            off += 4;
        }
    }
    out
}
