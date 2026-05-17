#![no_std]

//! Phase I.2.0 — graph-to-render-command translator.
//!
//! `k-scene` is the lens that turns the live runtime graph (nodes +
//! edges + audit envelopes) into a `RenderCommand` stream a backend
//! can execute.  Today it ships the *node* half (one coloured quad
//! per graph node, arranged in a grid in NDC) — the I.2.1 slice adds
//! edge bezier line strips, I.2.2 wires a camera, I.2.3 the theme +
//! HUD overlay.
//!
//! Layering:
//!
//!   ```text
//!   gos-runtime audit ring  ──┐
//!                              ├─> SceneSnapshot ──> k-scene::step()
//!   gos-runtime node table   ──┘                         │
//!                                                         ▼
//!                                          RenderBackend (wgpu / virtio-gpu /
//!                                                        future carrier)
//!   ```
//!
//! This crate is intentionally **stateless about the runtime**: the
//! caller (kernel boot loop or test harness) builds a `SceneSnapshot`
//! and passes it in.  That lets gfx-harness unit-test the translator
//! against a mock backend without standing up the gos-runtime
//! singleton, and keeps the kernel-side wiring trivial when a Phase
//! I.x carrier lands.
//!
//! Memory ownership: `k-scene` is `no_std` + no-alloc.  Vertex and
//! index byte buffers are *caller-owned* slices passed into `step`;
//! the caller decides whether they live in a static array, a Phase
//! G.4 GPU-mem grant, or a `Vec<u8>` in a test.  k-scene never
//! allocates.

use gos_gfx_protocol::{
    BufferId, BufferKind, GfxError, PipelineId, PipelineKind, PresentMode, RenderBackend,
    RenderCommand, SurfaceId,
};
use gos_protocol::VectorAddress;

/// One node in the scene.  Caller is free to derive `color` from a
/// theme (theme.wabi vs theme.shoji) and `vector` from the runtime
/// node table; k-scene treats both as opaque inputs.
#[derive(Debug, Clone, Copy)]
pub struct SceneNode {
    /// Stable identity used by the grid layout to keep the same node
    /// at the same screen cell across frames.  Not interpreted further
    /// in Gen-1; the I.x force-directed slice will read it as a seed.
    pub vector: VectorAddress,
    pub color: [f32; 3],
}

/// Caller's view of the live runtime state at a single tick.  Empty
/// `nodes` is legal — the renderer just emits a clear frame.
#[derive(Debug, Clone, Copy)]
pub struct SceneSnapshot<'a> {
    pub nodes: &'a [SceneNode],
    /// `gos_runtime::graph_generation()`.  Used to skip the geometry
    /// rebuild when the graph hasn't changed between frames.
    pub generation: u64,
}

/// Bytes per vertex on the wire: vec2 position + vec3 color, both
/// `f32`.  Matches the layout the bridge-host shader expects.
pub const BYTES_PER_VERTEX: usize = 4 * 2 + 4 * 3;
/// Each node is rendered as one quad (4 verts, 6 indices).
pub const VERTICES_PER_NODE: usize = 4;
pub const INDICES_PER_NODE: usize = 6;
/// u16 indices.
pub const BYTES_PER_INDEX: usize = 2;

/// Caller scratch sizing: vertex/index buffer in bytes for an N-node
/// scene.  Multiply by your `MAX_NODES` and stamp into a static array.
pub const fn vertex_buffer_bytes_for(max_nodes: usize) -> usize {
    max_nodes * VERTICES_PER_NODE * BYTES_PER_VERTEX
}
pub const fn index_buffer_bytes_for(max_nodes: usize) -> usize {
    max_nodes * INDICES_PER_NODE * BYTES_PER_INDEX
}

/// WGSL shader for the Gen-1 node quad.  Position is already in NDC;
/// the fragment stage just passes the per-vertex colour through.  The
/// future I.2.1 edge slice ships a separate pipeline with its own
/// shader — `CreatePipeline` is per-pipeline-kind on the wire.
pub const NODE_QUAD_WGSL: &str = r#"
struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) color:    vec3<f32>,
};
struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       color:         vec3<f32>,
};
@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    return out;
}
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
"#;

/// Half-size of each node quad in NDC.  Scales with the grid density
/// in `step`; Gen-1 hard-codes a comfortable visual default.
const NODE_HALF_SIZE: f32 = 0.04;

/// Resource handles allocated on the first `step` call.  Surface is
/// resized lazily; pipeline is one-shot (shader source is constant).
struct Handles {
    surface: SurfaceId,
    surface_dims: (u32, u32),
    pipeline: PipelineId,
    vertex_buffer: BufferId,
    index_buffer: BufferId,
}

/// Per-scene long-lived state.  Construct once at boot, then call
/// `step` from the render loop.
pub struct SceneState {
    handles: Option<Handles>,
    /// Last `SceneSnapshot.generation` we rebuilt geometry for.
    /// Initial sentinel: `u64::MAX` (any first observation differs).
    last_generation: u64,
    /// Last observed node count (geometry rebuild also fires if this
    /// changes even at the same generation, which shouldn't happen
    /// in practice but is cheap insurance).
    last_node_count: usize,
}

impl Default for SceneState {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneState {
    pub const fn new() -> Self {
        Self {
            handles: None,
            last_generation: u64::MAX,
            last_node_count: usize::MAX,
        }
    }

    /// Drive one frame.
    ///
    /// First call allocates surface + pipeline + vertex/index buffers.
    /// Subsequent calls reuse them and only re-upload geometry when
    /// `snapshot.generation` or `nodes.len()` has changed.
    ///
    /// `vertex_scratch` and `index_scratch` are caller-owned byte
    /// buffers used to lay out geometry before upload.  They must be
    /// at least `vertex_buffer_bytes_for(nodes.len())` /
    /// `index_buffer_bytes_for(nodes.len())` long; smaller slices
    /// return `GfxError::QueueFull` (signalling "scratch too small").
    pub fn step<B: RenderBackend>(
        &mut self,
        backend: &mut B,
        snapshot: &SceneSnapshot<'_>,
        viewport: (u32, u32),
        vertex_scratch: &mut [u8],
        index_scratch: &mut [u8],
    ) -> Result<(), GfxError> {
        let needed_v = vertex_buffer_bytes_for(snapshot.nodes.len());
        let needed_i = index_buffer_bytes_for(snapshot.nodes.len());
        if vertex_scratch.len() < needed_v || index_scratch.len() < needed_i {
            return Err(GfxError::QueueFull);
        }

        // 1. Resource bring-up on first step (or surface resize).
        let need_alloc = self
            .handles
            .as_ref()
            .map(|h| h.surface_dims != viewport)
            .unwrap_or(true);
        if need_alloc {
            // Tear down a prior surface on resize.  Pipeline + buffers
            // survive because their contents are size-independent.
            let prior_pipeline = self.handles.as_ref().map(|h| h.pipeline);
            let prior_vbuf = self.handles.as_ref().map(|h| h.vertex_buffer);
            let prior_ibuf = self.handles.as_ref().map(|h| h.index_buffer);

            let surface = backend.create_surface(viewport.0, viewport.1)?;

            let pipeline = match prior_pipeline {
                Some(p) => p,
                None => backend.create_pipeline(NODE_QUAD_WGSL.as_bytes())?,
            };
            // Vertex / index buffers: allocate empty placeholders so
            // the bind step always has SOMETHING to bind, even if
            // the snapshot has zero nodes.  Real geometry uploads
            // below.  Buffer length must respect COPY_BUFFER_ALIGNMENT
            // (4 B); zero-length isn't legal in wgpu, so pad to 4.
            let vertex_buffer = match prior_vbuf {
                Some(b) => b,
                None => backend.upload_buffer(BufferKind::Vertex, &[0u8; 4])?,
            };
            let index_buffer = match prior_ibuf {
                Some(b) => b,
                None => backend.upload_buffer(BufferKind::Index, &[0u8; 4])?,
            };

            self.handles = Some(Handles {
                surface,
                surface_dims: viewport,
                pipeline,
                vertex_buffer,
                index_buffer,
            });
        }

        // 2. Geometry rebuild on generation / count change.  Each
        // rebuild uploads BOTH buffers fresh so the indices and
        // vertices stay in lockstep.
        let geom_dirty = snapshot.generation != self.last_generation
            || snapshot.nodes.len() != self.last_node_count;
        if geom_dirty && !snapshot.nodes.is_empty() {
            let v_len = write_quad_vertices(snapshot.nodes, vertex_scratch);
            let i_len = write_quad_indices(snapshot.nodes.len(), index_scratch);

            let new_vbuf = backend.upload_buffer(BufferKind::Vertex, &vertex_scratch[..v_len])?;
            let new_ibuf = backend.upload_buffer(BufferKind::Index, &index_scratch[..i_len])?;

            if let Some(h) = self.handles.as_mut() {
                h.vertex_buffer = new_vbuf;
                h.index_buffer = new_ibuf;
            }
            self.last_generation = snapshot.generation;
            self.last_node_count = snapshot.nodes.len();
        } else if snapshot.nodes.is_empty() {
            self.last_generation = snapshot.generation;
            self.last_node_count = 0;
        }

        // 3. Frame loop.  Even on an empty scene we still emit a
        // BeginFrame/EndFrame so the host clears the surface.
        let handles = self.handles.as_ref().expect("allocated above");
        backend.submit(&RenderCommand::BeginFrame {
            surface: handles.surface,
        })?;
        if !snapshot.nodes.is_empty() {
            backend.submit(&RenderCommand::BindPipeline {
                pipeline: handles.pipeline,
            })?;
            backend.submit(&RenderCommand::BindBuffers {
                vertex: handles.vertex_buffer,
                instance: handles.vertex_buffer, // unused in Gen-1
                index: handles.index_buffer,
                uniform: handles.vertex_buffer, // unused in Gen-1
            })?;
            backend.submit(&RenderCommand::DrawInstanced {
                index_count: (snapshot.nodes.len() * INDICES_PER_NODE) as u32,
                instance_count: 1,
            })?;
        }
        backend.submit(&RenderCommand::EndFrame)?;
        Ok(())
    }
}

// ── Layout + geometry helpers ──────────────────────────────────────

/// Integer square root using a tiny long-division-style search.  No
/// f32 dependency keeps us friendly to targets without an FPU
/// (eventually `k-scene` will compile to a kernel-side plugin and
/// FPU access varies by ring / context).
fn isqrt_ceil(n: usize) -> usize {
    let mut s: usize = 1;
    while s * s < n {
        s += 1;
    }
    s
}

/// Map a graph node's grid cell to its NDC centre.  Spread covers
/// (-0.9, +0.9) in both axes; with `NODE_HALF_SIZE = 0.04` the
/// largest grids stay comfortably inside the clip volume.
fn node_centre(index: usize, total: usize) -> (f32, f32) {
    let cols = isqrt_ceil(total).max(1);
    let row = index / cols;
    let col = index % cols;
    let rows = (total + cols - 1) / cols;
    let span = 1.8_f32;
    let cx = if cols == 1 {
        0.0
    } else {
        -0.9 + (col as f32 / (cols - 1) as f32) * span
    };
    let cy = if rows <= 1 {
        0.0
    } else {
        -0.9 + (row as f32 / (rows - 1) as f32) * span
    };
    (cx, cy)
}

/// Write `nodes.len()` quads into `out` as little-endian f32 bytes,
/// matching `BYTES_PER_VERTEX`.  Returns bytes written.
fn write_quad_vertices(nodes: &[SceneNode], out: &mut [u8]) -> usize {
    let mut off = 0;
    for (i, node) in nodes.iter().enumerate() {
        let (cx, cy) = node_centre(i, nodes.len());
        let h = NODE_HALF_SIZE;
        // 4 verts: bottom-left, bottom-right, top-right, top-left
        let verts: [[f32; 2]; 4] = [
            [cx - h, cy - h],
            [cx + h, cy - h],
            [cx + h, cy + h],
            [cx - h, cy + h],
        ];
        for v in verts {
            write_f32_le(&mut out[off..off + 4], v[0]);
            off += 4;
            write_f32_le(&mut out[off..off + 4], v[1]);
            off += 4;
            write_f32_le(&mut out[off..off + 4], node.color[0]);
            off += 4;
            write_f32_le(&mut out[off..off + 4], node.color[1]);
            off += 4;
            write_f32_le(&mut out[off..off + 4], node.color[2]);
            off += 4;
        }
    }
    off
}

/// Write `node_count` quad-index sets into `out` as little-endian u16
/// bytes.  Layout per quad: [base, base+1, base+2, base, base+2, base+3].
fn write_quad_indices(node_count: usize, out: &mut [u8]) -> usize {
    let mut off = 0;
    for i in 0..node_count {
        let base = (i * VERTICES_PER_NODE) as u16;
        for ix in [base, base + 1, base + 2, base, base + 2, base + 3] {
            out[off..off + 2].copy_from_slice(&ix.to_le_bytes());
            off += 2;
        }
    }
    off
}

fn write_f32_le(out: &mut [u8], value: f32) {
    out.copy_from_slice(&value.to_le_bytes());
}

// Compile-time sanity: the public PipelineKind / PresentMode names are
// re-exported via gos_gfx_protocol — keep them imported (and used) so
// rust-analyzer doesn't flag the import path as dead when downstream
// callers eventually wire them.  Not all are referenced yet today.
const _: () = {
    let _ = PipelineKind::NodeInstance as u8;
    let _ = PresentMode::Immediate as u8;
};

