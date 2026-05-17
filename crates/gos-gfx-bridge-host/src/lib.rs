//! Phase I.1.1 — host-side bridge implementation backed by `wgpu`.
//!
//! Layering reminder (see doc/PHASE_I_GRAPHICS.md):
//!
//!   k-vk-host (kernel, no_std) ──┐
//!                                 │ RenderCommand frames over wire
//!   gos-gfx-bridge-host (this) ──┘
//!                                 │ direct dispatch (this crate)
//!   wgpu / Vulkan ───────────────┘
//!
//! Today the kernel-side encoder is not yet plumbed end-to-end (that
//! lands once a real carrier — shared memory ring or hypervisor escape —
//! is built).  For Gen-1 I.1.1 the harness drives `WgpuBackend`
//! *directly* via the `RenderBackend` trait, bypassing the wire layer
//! that was added in I.1.0.  This proves the renderer half stands on
//! its own; later slices wire the two halves together.
//!
//! Carrier-agnostic guarantee: any future carrier just decodes frames
//! and calls `WgpuBackend::submit`.  Nothing in this crate cares how
//! the bytes arrived.
//!
//! Shader format note: gos-gfx-protocol's `CreatePipeline.shader_spirv`
//! is a generic blob; Gen-1 interprets it as **WGSL UTF-8 source** so
//! we don't pull a SPIR-V compiler into the test loop.  When
//! `k-vk-host` ships its real pipeline encoder we'll either tighten to
//! pre-compiled SPIR-V (via `glslc` at build time) or run naga in the
//! bridge.  Either way the wire format stays a `&[u8]` blob.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use gos_gfx_protocol::{
    BufferId, BufferKind, GfxError, PipelineId, RenderBackend, RenderCommand, SurfaceId,
};

/// Off-screen surface state.  Gen-1 only supports one surface format
/// (`Rgba8Unorm`) so PNG readback is trivial and `assert!` against
/// fixed pixel offsets is well-defined.
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Bytes per pixel for `Rgba8Unorm`.  Used by readback row-pitch math.
const BPP: u32 = 4;

struct SurfaceState {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

struct FrameState {
    surface: SurfaceId,
    pipeline: Option<PipelineId>,
    vertex_buffer: Option<BufferId>,
    index_buffer: Option<BufferId>,
    /// `DrawInstanced` records its counts here; `EndFrame` consumes
    /// them when it actually opens the wgpu render pass.  Gen-1
    /// supports exactly one draw per frame; multi-draw lands in I.2.x.
    pending_draw: Option<(u32, u32)>,
}

/// Vertex shape the bridge assumes for all `CreatePipeline` results
/// today: position (vec2) + color (vec3).  Real I.2.x slices will
/// promote this to a registered vertex-layout descriptor in
/// `CreatePipeline`; for Gen-1 we hard-code it so the demo triangle
/// compiles without a layout-negotiation round trip.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DemoVertex {
    pub position: [f32; 2],
    pub color: [f32; 3],
}

const VERTEX_LAYOUT: wgpu::VertexBufferLayout = wgpu::VertexBufferLayout {
    array_stride: core::mem::size_of::<DemoVertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: core::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            shader_location: 1,
        },
    ],
};

/// Host-side renderer for Gen-1.  Owns a `wgpu::Device` + `Queue` and
/// per-resource hash maps keyed by the gos-gfx-protocol handle types.
/// Synchronous `submit` API — wgpu's command encoder is recorded
/// per-frame, then submitted on `EndFrame`.
pub struct WgpuBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surfaces: HashMap<u32, SurfaceState>,
    pipelines: HashMap<u32, wgpu::RenderPipeline>,
    buffers: HashMap<u32, wgpu::Buffer>,
    next_handle: u32,
    frame: Option<FrameState>,
    /// Last `EndFrame` readback (raw RGBA bytes, length = width * height
    /// * 4).  Tests use this for pixel assertions; later slices wire
    /// PNG diff against `test-frames/*.png`.
    last_readback: Option<Vec<u8>>,
}

impl WgpuBackend {
    /// Bring up a headless wgpu context.  `power_preference =
    /// LowPower` so CI / lavapipe / software adapters are preferred
    /// when available; the discrete GPU path is exercised by the
    /// nightly hardware bench (future I.3.0 slice).
    pub fn new_headless() -> Result<Self, GfxError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            },
        ))
        .ok_or(GfxError::DeviceLost)?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("gos-gfx-bridge-host"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        ))
        .map_err(|_| GfxError::DeviceLost)?;
        Ok(Self {
            device,
            queue,
            surfaces: HashMap::new(),
            pipelines: HashMap::new(),
            buffers: HashMap::new(),
            next_handle: 1, // 0 is the protocol's invalid sentinel
            frame: None,
            last_readback: None,
        })
    }

    pub fn last_readback(&self) -> Option<&[u8]> {
        self.last_readback.as_deref()
    }

    fn alloc_handle(&mut self) -> u32 {
        let h = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        h
    }

    pub fn create_surface(&mut self, width: u32, height: u32) -> Result<SurfaceId, GfxError> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gos-gfx-surface"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SURFACE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let id = self.alloc_handle();
        self.surfaces.insert(
            id,
            SurfaceState {
                texture,
                view,
                width,
                height,
            },
        );
        Ok(SurfaceId(id))
    }

    pub fn create_pipeline(&mut self, wgsl: &[u8]) -> Result<PipelineId, GfxError> {
        let source = core::str::from_utf8(wgsl).map_err(|_| GfxError::DecodeFailed)?;
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gos-gfx-pipeline-shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gos-gfx-pipeline-layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gos-gfx-render-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[VERTEX_LAYOUT],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SURFACE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let id = self.alloc_handle();
        self.pipelines.insert(id, pipeline);
        Ok(PipelineId(id))
    }

    pub fn upload_buffer(&mut self, kind: BufferKind, bytes: &[u8]) -> Result<BufferId, GfxError> {
        let usage = match kind {
            BufferKind::Vertex => wgpu::BufferUsages::VERTEX,
            BufferKind::Index => wgpu::BufferUsages::INDEX,
            BufferKind::Instance => wgpu::BufferUsages::VERTEX,
            BufferKind::Uniform => wgpu::BufferUsages::UNIFORM,
        } | wgpu::BufferUsages::COPY_DST;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gos-gfx-buffer"),
            size: bytes.len() as wgpu::BufferAddress,
            usage,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buffer, 0, bytes);
        let id = self.alloc_handle();
        self.buffers.insert(id, buffer);
        Ok(BufferId(id))
    }

    fn begin_frame(&mut self, surface: SurfaceId) -> Result<(), GfxError> {
        if self.frame.is_some() {
            return Err(GfxError::InvalidState);
        }
        if !self.surfaces.contains_key(&surface.0) {
            return Err(GfxError::InvalidHandle);
        }
        self.frame = Some(FrameState {
            surface,
            pipeline: None,
            vertex_buffer: None,
            index_buffer: None,
            pending_draw: None,
        });
        Ok(())
    }

    fn end_frame(&mut self, index_count: u32, instance_count: u32) -> Result<(), GfxError> {
        let frame = self.frame.take().ok_or(GfxError::InvalidState)?;
        let pipeline_id = frame.pipeline.ok_or(GfxError::InvalidState)?;
        let vertex_buffer_id = frame.vertex_buffer.ok_or(GfxError::InvalidState)?;
        let index_buffer_id = frame.index_buffer.ok_or(GfxError::InvalidState)?;
        let surface = self
            .surfaces
            .get(&frame.surface.0)
            .ok_or(GfxError::InvalidHandle)?;
        let pipeline = self
            .pipelines
            .get(&pipeline_id.0)
            .ok_or(GfxError::InvalidHandle)?;
        let vertex_buffer = self
            .buffers
            .get(&vertex_buffer_id.0)
            .ok_or(GfxError::InvalidHandle)?;
        let index_buffer = self
            .buffers
            .get(&index_buffer_id.0)
            .ok_or(GfxError::InvalidHandle)?;

        // wgpu requires the row pitch of texture-to-buffer copies to be a
        // multiple of 256 bytes.  Round up the surface row pitch to the
        // next 256-byte boundary; readback strips the padding back to a
        // tight `width * BPP` row.
        let bytes_per_row = ((surface.width * BPP + 255) / 256) * 256;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gos-gfx-readback"),
            size: (bytes_per_row * surface.height) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gos-gfx-frame-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gos-gfx-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..index_count, 0, 0..instance_count);
        }
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &surface.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(surface.height),
                },
            },
            wgpu::Extent3d {
                width: surface.width,
                height: surface.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        // Block until the staging buffer is mapped (acceptable in a
        // synchronous test path; Phase I.x will move to fence + poll).
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| GfxError::DeviceLost)?
            .map_err(|_| GfxError::DeviceLost)?;
        let view = slice.get_mapped_range();

        // Strip the 256-aligned row padding back to width*BPP.
        let mut out = Vec::with_capacity((surface.width * surface.height * BPP) as usize);
        let row_tight = (surface.width * BPP) as usize;
        for row in 0..surface.height as usize {
            let start = row * bytes_per_row as usize;
            out.extend_from_slice(&view[start..start + row_tight]);
        }
        drop(view);
        staging.unmap();

        self.last_readback = Some(out);
        Ok(())
    }
}

impl RenderBackend for WgpuBackend {
    fn submit(&mut self, cmd: &RenderCommand<'_>) -> Result<(), GfxError> {
        // Resource creates would normally return their handle via a
        // response channel; for Gen-1 the harness drives the backend
        // directly via these convenience methods (create_surface
        // etc.) on top of `submit`, so `submit` only handles the
        // frame-loop commands.  The wire-encoded request/response
        // dance lands when the carrier does.
        match cmd {
            RenderCommand::BeginFrame { surface } => self.begin_frame(*surface),
            RenderCommand::BindPipeline { pipeline } => {
                let frame = self.frame.as_mut().ok_or(GfxError::InvalidState)?;
                if !self.pipelines.contains_key(&pipeline.0) {
                    return Err(GfxError::InvalidHandle);
                }
                frame.pipeline = Some(*pipeline);
                Ok(())
            }
            RenderCommand::BindBuffers {
                vertex,
                instance: _,
                index,
                uniform: _,
            } => {
                let frame = self.frame.as_mut().ok_or(GfxError::InvalidState)?;
                if !self.buffers.contains_key(&vertex.0) {
                    return Err(GfxError::InvalidHandle);
                }
                if !self.buffers.contains_key(&index.0) {
                    return Err(GfxError::InvalidHandle);
                }
                frame.vertex_buffer = Some(*vertex);
                frame.index_buffer = Some(*index);
                Ok(())
            }
            RenderCommand::DrawInstanced {
                index_count,
                instance_count,
            } => {
                // Gen-1 collapses Draw+End: the actual GPU submission
                // happens in EndFrame, where it picks up these counts
                // from the frame state.  Stash them transiently in
                // module-private statics — but Gen-1 just inlines this
                // by letting end_frame take the counts.  The test path
                // calls submit(DrawInstanced) then submit(EndFrame),
                // so we cache here and read in EndFrame.
                let frame = self.frame.as_mut().ok_or(GfxError::InvalidState)?;
                // Stash on the frame so EndFrame sees them.  Add fields
                // lazily via a side channel: we tunnel through the
                // FrameState by transmuting?  No, just add the fields.
                // (See FrameState extension below.)
                frame.pending_draw = Some((*index_count, *instance_count));
                Ok(())
            }
            RenderCommand::EndFrame => {
                let (index_count, instance_count) = self
                    .frame
                    .as_ref()
                    .and_then(|f| f.pending_draw)
                    .ok_or(GfxError::InvalidState)?;
                self.end_frame(index_count, instance_count)
            }
            RenderCommand::CreateSurface { .. }
            | RenderCommand::CreatePipeline { .. }
            | RenderCommand::UploadBuffer { .. }
            | RenderCommand::UploadTexture { .. }
            | RenderCommand::DestroySurface(_)
            | RenderCommand::DestroyPipeline(_)
            | RenderCommand::DestroyBuffer(_)
            | RenderCommand::DestroyTexture(_) => {
                // Resource lifecycle goes through the direct
                // `create_surface` / `create_pipeline` / `upload_buffer`
                // methods today (they return the handle synchronously).
                // The submit() path is reserved for verbs whose
                // response is implicit (frame-loop verbs).
                Err(GfxError::InvalidState)
            }
        }
    }
}
