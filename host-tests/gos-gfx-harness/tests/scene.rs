//! Phase I.2.0 — k-scene translates a SceneSnapshot into a fixed
//! RenderCommand trace shape against a mock RenderBackend.  Asserts
//! both the *sequence* and the *counts* (vertex/index buffer sizes,
//! DrawInstanced index_count = 6 × node_count) so a regression in
//! the layout / packing path fails loudly.
//!
//! No GPU involvement — this is a pure logic test for the translator.
//! The wgpu-backed end-to-end golden frame is `i_1_1_triangle.png`-
//! shaped (Phase I.x slice) and will live in gos-gfx-bridge-host.

use gos_gfx_protocol::{
    BufferId, BufferKind, GfxError, PipelineId, RenderBackend, RenderCommand, SurfaceId,
};
use gos_protocol::VectorAddress;
use k_scene::{
    index_buffer_bytes_for, vertex_buffer_bytes_for, SceneNode, SceneSnapshot, SceneState,
    INDICES_PER_NODE, NODE_QUAD_WGSL,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TraceEntry {
    CreateSurface { width: u32, height: u32 },
    CreatePipeline { shader_len: usize },
    UploadBuffer { kind: BufferKind, len: usize },
    Submit(&'static str), // tag for the matched variant
    DrawInstanced { index_count: u32, instance_count: u32 },
}

struct MockBackend {
    next_handle: u32,
    trace: Vec<TraceEntry>,
}

impl MockBackend {
    fn new() -> Self {
        Self {
            next_handle: 1,
            trace: Vec::new(),
        }
    }

    fn alloc(&mut self) -> u32 {
        let h = self.next_handle;
        self.next_handle += 1;
        h
    }
}

impl RenderBackend for MockBackend {
    fn create_surface(&mut self, width: u32, height: u32) -> Result<SurfaceId, GfxError> {
        self.trace.push(TraceEntry::CreateSurface { width, height });
        Ok(SurfaceId(self.alloc()))
    }
    fn create_pipeline(&mut self, shader: &[u8]) -> Result<PipelineId, GfxError> {
        self.trace.push(TraceEntry::CreatePipeline {
            shader_len: shader.len(),
        });
        Ok(PipelineId(self.alloc()))
    }
    fn upload_buffer(
        &mut self,
        kind: BufferKind,
        bytes: &[u8],
    ) -> Result<BufferId, GfxError> {
        self.trace.push(TraceEntry::UploadBuffer {
            kind,
            len: bytes.len(),
        });
        Ok(BufferId(self.alloc()))
    }
    fn submit(&mut self, cmd: &RenderCommand<'_>) -> Result<(), GfxError> {
        let tag = match cmd {
            RenderCommand::BeginFrame { .. } => "BeginFrame",
            RenderCommand::BindPipeline { .. } => "BindPipeline",
            RenderCommand::BindBuffers { .. } => "BindBuffers",
            RenderCommand::EndFrame => "EndFrame",
            RenderCommand::DrawInstanced {
                index_count,
                instance_count,
            } => {
                self.trace.push(TraceEntry::DrawInstanced {
                    index_count: *index_count,
                    instance_count: *instance_count,
                });
                return Ok(());
            }
            other => {
                panic!("MockBackend does not expect submit({:?})", other);
            }
        };
        self.trace.push(TraceEntry::Submit(tag));
        Ok(())
    }
}

fn build_nodes(n: usize) -> Vec<SceneNode> {
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

#[test]
fn scene_step_produces_expected_trace_for_100_node_grid() {
    let nodes = build_nodes(100);
    let snap = SceneSnapshot {
        nodes: &nodes,
        generation: 1,
    };
    let mut v_scratch = vec![0u8; vertex_buffer_bytes_for(nodes.len())];
    let mut i_scratch = vec![0u8; index_buffer_bytes_for(nodes.len())];

    let mut backend = MockBackend::new();
    let mut scene = SceneState::new();
    scene
        .step(&mut backend, &snap, (800, 600), &mut v_scratch, &mut i_scratch)
        .expect("first step");

    // First-step trace shape (resource bring-up + initial geometry +
    // frame loop).  Buffers allocated twice: once empty (Gen-1 surface-
    // bring-up placeholder), once with real geometry.
    let expected = vec![
        TraceEntry::CreateSurface {
            width: 800,
            height: 600,
        },
        TraceEntry::CreatePipeline {
            shader_len: NODE_QUAD_WGSL.len(),
        },
        TraceEntry::UploadBuffer {
            kind: BufferKind::Vertex,
            len: 4, // placeholder
        },
        TraceEntry::UploadBuffer {
            kind: BufferKind::Index,
            len: 4, // placeholder
        },
        TraceEntry::UploadBuffer {
            kind: BufferKind::Vertex,
            len: vertex_buffer_bytes_for(100), // = 8000
        },
        TraceEntry::UploadBuffer {
            kind: BufferKind::Index,
            len: index_buffer_bytes_for(100), // = 1200
        },
        TraceEntry::Submit("BeginFrame"),
        TraceEntry::Submit("BindPipeline"),
        TraceEntry::Submit("BindBuffers"),
        TraceEntry::DrawInstanced {
            index_count: (100 * INDICES_PER_NODE) as u32, // = 600
            instance_count: 1,
        },
        TraceEntry::Submit("EndFrame"),
    ];
    assert_eq!(backend.trace, expected);
}

#[test]
fn scene_step_skips_geometry_upload_when_generation_unchanged() {
    let nodes = build_nodes(4);
    let snap = SceneSnapshot {
        nodes: &nodes,
        generation: 7,
    };
    let mut v_scratch = vec![0u8; vertex_buffer_bytes_for(nodes.len())];
    let mut i_scratch = vec![0u8; index_buffer_bytes_for(nodes.len())];

    let mut backend = MockBackend::new();
    let mut scene = SceneState::new();
    scene
        .step(&mut backend, &snap, (256, 256), &mut v_scratch, &mut i_scratch)
        .expect("first");
    backend.trace.clear();

    // Second step at the same generation: no new uploads, just the
    // frame loop.
    scene
        .step(&mut backend, &snap, (256, 256), &mut v_scratch, &mut i_scratch)
        .expect("second");
    assert_eq!(
        backend.trace,
        vec![
            TraceEntry::Submit("BeginFrame"),
            TraceEntry::Submit("BindPipeline"),
            TraceEntry::Submit("BindBuffers"),
            TraceEntry::DrawInstanced {
                index_count: (4 * INDICES_PER_NODE) as u32,
                instance_count: 1,
            },
            TraceEntry::Submit("EndFrame"),
        ]
    );
}

#[test]
fn scene_step_re_uploads_when_generation_advances() {
    let nodes = build_nodes(4);
    let mut v_scratch = vec![0u8; vertex_buffer_bytes_for(nodes.len())];
    let mut i_scratch = vec![0u8; index_buffer_bytes_for(nodes.len())];
    let mut backend = MockBackend::new();
    let mut scene = SceneState::new();

    scene
        .step(
            &mut backend,
            &SceneSnapshot {
                nodes: &nodes,
                generation: 1,
            },
            (256, 256),
            &mut v_scratch,
            &mut i_scratch,
        )
        .expect("gen 1");
    backend.trace.clear();

    scene
        .step(
            &mut backend,
            &SceneSnapshot {
                nodes: &nodes,
                generation: 2,
            },
            (256, 256),
            &mut v_scratch,
            &mut i_scratch,
        )
        .expect("gen 2");

    // Generation tick: two fresh uploads (vertex + index), then frame.
    assert!(
        backend.trace.contains(&TraceEntry::UploadBuffer {
            kind: BufferKind::Vertex,
            len: vertex_buffer_bytes_for(4),
        }),
        "expected vertex re-upload, trace = {:?}",
        backend.trace
    );
    assert!(
        backend.trace.contains(&TraceEntry::UploadBuffer {
            kind: BufferKind::Index,
            len: index_buffer_bytes_for(4),
        }),
        "expected index re-upload"
    );
}

#[test]
fn scene_step_emits_only_frame_loop_for_empty_scene() {
    let snap = SceneSnapshot {
        nodes: &[],
        generation: 0,
    };
    let mut v = [0u8; 16];
    let mut i = [0u8; 16];
    let mut backend = MockBackend::new();
    let mut scene = SceneState::new();
    scene
        .step(&mut backend, &snap, (64, 64), &mut v, &mut i)
        .expect("empty scene");

    // Resource bring-up still runs (so a future non-empty step can
    // reuse handles), but no Bind / Draw — just BeginFrame +
    // EndFrame so the host clears the surface.
    let frame_only: Vec<_> = backend
        .trace
        .iter()
        .filter(|e| matches!(e, TraceEntry::Submit(_) | TraceEntry::DrawInstanced { .. }))
        .cloned()
        .collect();
    assert_eq!(
        frame_only,
        vec![
            TraceEntry::Submit("BeginFrame"),
            TraceEntry::Submit("EndFrame"),
        ]
    );
}

#[test]
fn scene_step_rejects_undersized_scratch_with_queue_full() {
    let nodes = build_nodes(10);
    let snap = SceneSnapshot {
        nodes: &nodes,
        generation: 1,
    };
    // Way too small.
    let mut v = [0u8; 4];
    let mut i = [0u8; 4];
    let mut backend = MockBackend::new();
    let mut scene = SceneState::new();
    let err = scene
        .step(&mut backend, &snap, (256, 256), &mut v, &mut i)
        .expect_err("should reject");
    assert_eq!(err, GfxError::QueueFull);
    // No backend calls were made before the size check.
    assert!(backend.trace.is_empty());
}
