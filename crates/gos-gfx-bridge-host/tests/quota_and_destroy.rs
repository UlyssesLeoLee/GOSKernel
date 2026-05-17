//! Phase I.1.2 — quota enforcement + destroy-then-reuse handle
//! safety, exercised against the live `WgpuBackend`.
//!
//! Both contracts are part of the Phase I product acceptance list
//! (PHASE_I_GRAPHICS.md I.3 items #5 device-lost recovery and #8
//! quota execution) — testing here so a Phase I.x slice can't
//! regress them without a CI trip.

use gos_gfx_bridge_host::{GfxQuota, WgpuBackend};
use gos_gfx_protocol::{
    BufferId, BufferKind, GfxError, PipelineId, RenderBackend, RenderCommand, SurfaceId,
};

const TRIVIAL_WGSL: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
"#;

fn fresh_backend() -> Option<WgpuBackend> {
    match WgpuBackend::new_headless() {
        Ok(b) => Some(b),
        Err(_) => {
            eprintln!("quota_and_destroy: no usable wgpu adapter on this host, skipping");
            None
        }
    }
}

#[test]
fn quota_caps_each_resource_kind_independently() {
    let Some(mut backend) = fresh_backend() else { return };
    // Cap surfaces at 1, everything else at 2.
    backend.set_quota(GfxQuota {
        max_surfaces: Some(1),
        max_pipelines: Some(2),
        max_buffers: Some(2),
        max_textures: Some(0),
    });

    // First surface: allowed.
    let _s0 = backend.create_surface(8, 8).expect("first surface");
    // Second surface: must be rejected with QuotaExceeded (not panic,
    // not silently allow; wgpu state is unchanged).
    match backend.create_surface(8, 8) {
        Err(GfxError::QuotaExceeded) => {}
        other => panic!("expected QuotaExceeded, got {:?}", other),
    }
    let (s, p, b, _t) = backend.live_counts();
    assert_eq!((s, p, b), (1, 0, 0), "no resource leaked into the cap");

    // Two pipelines, two buffers should each succeed; the third is over cap.
    let _p0 = backend.create_pipeline(TRIVIAL_WGSL.as_bytes()).expect("p0");
    let _p1 = backend.create_pipeline(TRIVIAL_WGSL.as_bytes()).expect("p1");
    assert_eq!(
        backend.create_pipeline(TRIVIAL_WGSL.as_bytes()),
        Err(GfxError::QuotaExceeded)
    );

    // 4 bytes is the minimum that respects COPY_BUFFER_ALIGNMENT.
    let payload = [0u8; 4];
    let _b0 = backend
        .upload_buffer(BufferKind::Vertex, &payload)
        .expect("b0");
    let _b1 = backend
        .upload_buffer(BufferKind::Vertex, &payload)
        .expect("b1");
    assert_eq!(
        backend.upload_buffer(BufferKind::Vertex, &payload),
        Err(GfxError::QuotaExceeded)
    );
}

#[test]
fn destroy_then_create_recovers_quota_slot() {
    let Some(mut backend) = fresh_backend() else { return };
    backend.set_quota(GfxQuota::all(1));

    let s0 = backend.create_surface(8, 8).expect("s0");
    // At-cap rejection before destroy.
    assert_eq!(backend.create_surface(8, 8), Err(GfxError::QuotaExceeded));

    // Destroy frees the slot.
    backend.destroy_surface(s0).expect("destroy_surface");
    let (live_s, _, _, _) = backend.live_counts();
    assert_eq!(live_s, 0, "destroy must drop the slot");

    // Now a fresh allocation should succeed.
    let _s1 = backend.create_surface(8, 8).expect("post-destroy allocation");
}

#[test]
fn reuse_of_destroyed_handle_returns_invalid_handle() {
    let Some(mut backend) = fresh_backend() else { return };
    let s0 = backend.create_surface(8, 8).expect("create");
    let p0 = backend.create_pipeline(TRIVIAL_WGSL.as_bytes()).expect("p");
    let b0 = backend
        .upload_buffer(BufferKind::Vertex, &[0u8; 4])
        .expect("b");

    backend.destroy_surface(s0).expect("destroy_surface");
    backend.destroy_pipeline(p0).expect("destroy_pipeline");
    backend.destroy_buffer(b0).expect("destroy_buffer");

    // Double-destroy: same handle, second call -> InvalidHandle.
    assert_eq!(backend.destroy_surface(s0), Err(GfxError::InvalidHandle));
    assert_eq!(backend.destroy_pipeline(p0), Err(GfxError::InvalidHandle));
    assert_eq!(backend.destroy_buffer(b0), Err(GfxError::InvalidHandle));

    // submit() path also rejects: BeginFrame on a stale surface, then
    // BindPipeline / BindBuffers on stale ids.  Each should bail with
    // InvalidHandle rather than panic on a missing HashMap entry.
    assert_eq!(
        backend.submit(&RenderCommand::BeginFrame { surface: s0 }),
        Err(GfxError::InvalidHandle)
    );

    // Issue a fresh surface to set up a frame state, then check stale
    // pipeline / buffer handles short-circuit BindPipeline /
    // BindBuffers without contaminating the frame.
    let s1 = backend.create_surface(8, 8).expect("fresh");
    backend
        .submit(&RenderCommand::BeginFrame { surface: s1 })
        .expect("BeginFrame fresh");
    assert_eq!(
        backend.submit(&RenderCommand::BindPipeline { pipeline: p0 }),
        Err(GfxError::InvalidHandle)
    );
    assert_eq!(
        backend.submit(&RenderCommand::BindBuffers {
            vertex: b0,
            instance: b0,
            index: b0,
            uniform: b0,
        }),
        Err(GfxError::InvalidHandle)
    );
}

#[test]
fn destroying_active_surface_mid_frame_is_rejected() {
    let Some(mut backend) = fresh_backend() else { return };
    let s = backend.create_surface(8, 8).expect("surface");
    backend
        .submit(&RenderCommand::BeginFrame { surface: s })
        .expect("BeginFrame");
    // Tearing down the surface mid-frame would leave the encoder
    // pointing at a freed view — must be InvalidState, not silent.
    assert_eq!(backend.destroy_surface(s), Err(GfxError::InvalidState));
    // Surface still in the table.
    let (live_s, _, _, _) = backend.live_counts();
    assert_eq!(live_s, 1);
}

#[test]
fn destroy_texture_on_unknown_handle_returns_invalid_handle() {
    let Some(mut backend) = fresh_backend() else { return };
    // Gen-1 never allocates textures, so any DestroyTexture is by
    // construction a bogus handle.
    use gos_gfx_protocol::TextureId;
    assert_eq!(
        backend.submit(&RenderCommand::DestroyTexture(TextureId(99))),
        Err(GfxError::InvalidHandle)
    );
    let _ = backend;
    let _ = BufferId::INVALID;
    let _ = PipelineId::INVALID;
    let _ = SurfaceId::INVALID;
}
