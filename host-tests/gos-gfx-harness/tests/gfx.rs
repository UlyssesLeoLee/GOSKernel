//! Phase I.0 — gos-gfx-protocol harness scaffold.
//!
//! Today this file owns only the smoke tests against the ABI version
//! triple, handle sentinel semantics, and command-enum size invariants.
//! Each Phase I slice adds one focused test class:
//!
//!   * I.1.0 — bridge transport: round-trip a BeginFrame / EndFrame
//!     pair through a stub `RenderBackend` and assert the bridge frame
//!     magic survives.
//!   * I.1.1 — UploadBuffer + DrawInstanced single-triangle golden
//!     frame (lavapipe-backed, output PNG).
//!   * I.2.x — `k-scene` mutation->command translation goldens.
//!   * I.3.x — perf bench, fuzz, device-lost recovery.
//!
//! Per the slice contract in PHASE_I_GRAPHICS.md, every Phase I PR
//! MUST land its own harness test alongside the kernel-side change.
//! No harness = no merge.

use gos_gfx_protocol::{
    BufferId, GfxError, PipelineId, PresentMode, RenderCommand, SurfaceId, TextureId,
    GFX_ABI_VERSION, GFX_ABI_VERSION_MAJOR, GFX_ABI_VERSION_MINOR, GFX_ABI_VERSION_PATCH,
    GFX_FRAME_MAGIC,
};

// I.0.1 — ABI version triple packs identically on both sides.
#[test]
fn abi_version_packs_consistently() {
    let packed = ((GFX_ABI_VERSION_MAJOR as u32) << 16)
        | ((GFX_ABI_VERSION_MINOR as u32) << 8)
        | (GFX_ABI_VERSION_PATCH as u32);
    assert_eq!(packed, GFX_ABI_VERSION);
}

// I.0.2 — Frame magic is `b"GFX1"` little-endian.  Bridge resync depends
// on this value being stable across crate versions until a major bump.
#[test]
fn frame_magic_matches_ascii_marker() {
    assert_eq!(GFX_FRAME_MAGIC, u32::from_le_bytes(*b"GFX1"));
}

// I.0.3 — Handle sentinel semantics.  Zero MUST be invalid; the bridge
// is forbidden from ever returning it from a Create* response.  k-scene
// uses `is_valid()` as a cheap "did this Create succeed?" check before
// emitting bind commands.
#[test]
fn handle_zero_is_invalid_for_every_handle_kind() {
    assert!(!SurfaceId(0).is_valid());
    assert!(!PipelineId(0).is_valid());
    assert!(!BufferId(0).is_valid());
    assert!(!TextureId(0).is_valid());
    assert!(SurfaceId(1).is_valid());
    assert!(PipelineId(1).is_valid());
    assert!(BufferId(1).is_valid());
    assert!(TextureId(1).is_valid());
    assert_eq!(SurfaceId::INVALID.0, 0);
    assert_eq!(PipelineId::INVALID.0, 0);
    assert_eq!(BufferId::INVALID.0, 0);
    assert_eq!(TextureId::INVALID.0, 0);
}

// I.0.4 — Command enum carries the expected variants for the Gen-1
// minimum vocabulary.  Existence/spelling test only; semantics are
// tested per-slice once a real backend lands.
#[test]
fn render_command_covers_gen1_minimum_vocabulary() {
    let _ = RenderCommand::CreateSurface {
        width: 1920,
        height: 1080,
        present_mode: PresentMode::Mailbox,
    };
    let _ = RenderCommand::BeginFrame {
        surface: SurfaceId(1),
    };
    let _ = RenderCommand::DrawInstanced {
        index_count: 36,
        instance_count: 5000,
    };
    let _ = RenderCommand::EndFrame;
    let _ = RenderCommand::DestroySurface(SurfaceId(1));
}

// I.0.5 — Error codes are stable negative integers.  k-vk-host stamps
// these directly into the kernel-side return path; the bridge MUST NOT
// renumber them without a major ABI bump.
#[test]
fn error_codes_are_stable() {
    assert_eq!(GfxError::QueueFull as i32, -1);
    assert_eq!(GfxError::InvalidHandle as i32, -2);
    assert_eq!(GfxError::SurfaceLost as i32, -3);
    assert_eq!(GfxError::DeviceLost as i32, -4);
    assert_eq!(GfxError::QuotaExceeded as i32, -5);
    assert_eq!(GfxError::DeviceOutOfMemory as i32, -6);
    assert_eq!(GfxError::DecodeFailed as i32, -7);
    assert_eq!(GfxError::InvalidState as i32, -8);
}
