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
    crc32_ieee, read_bridge_frame, write_bridge_frame, BridgeCommandTag, BufferId, GfxError,
    PipelineId, PresentMode, RenderBackend, RenderCommand, SurfaceId, TextureId,
    BRIDGE_FRAME_HEADER_BYTES, GFX_ABI_VERSION, GFX_ABI_VERSION_MAJOR, GFX_ABI_VERSION_MINOR,
    GFX_ABI_VERSION_PATCH, GFX_FRAME_MAGIC,
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

// ── Phase I.1.0 — bridge transport round-trip ─────────────────────

/// Spec-baseline CRC32 known-answer for `b"123456789"`.  If `crc32_ieee`
/// ever silently shifts polynomial / direction the bridge resync
/// breaks; this test catches it independently of the frame round-trip.
#[test]
fn crc32_ieee_matches_known_answer_for_baseline_input() {
    assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32_ieee(b""), 0);
}

/// I.1.0.1 — encode + decode a `BeginFrame { surface: 0x1234_5678 }`
/// through the wire format; assert structural details (magic bytes
/// appear at offset 0, surface id round-trips, total length matches).
#[test]
fn begin_frame_round_trips_through_wire_format() {
    let mut buf = [0u8; 32];
    let cmd_in = RenderCommand::BeginFrame {
        surface: SurfaceId(0x1234_5678),
    };
    let written = write_bridge_frame(&mut buf, &cmd_in).expect("encode");
    assert_eq!(written, BRIDGE_FRAME_HEADER_BYTES + 4);
    // Magic + version sanity at the byte level — the host decoder uses
    // these as the resync anchors, so they must be exactly here.
    assert_eq!(&buf[0..4], &GFX_FRAME_MAGIC.to_le_bytes());
    assert_eq!(&buf[4..8], &GFX_ABI_VERSION.to_le_bytes());
    assert_eq!(buf[8], BridgeCommandTag::BeginFrame as u8);

    let (cmd_out, consumed) = read_bridge_frame(&buf).expect("decode");
    assert_eq!(consumed, written);
    match cmd_out {
        RenderCommand::BeginFrame { surface } => assert_eq!(surface.0, 0x1234_5678),
        other => panic!("decoded wrong variant: {:?}", other),
    }
}

/// I.1.0.2 — EndFrame is the simpler shape (empty payload); confirms
/// payload_len=0 path doesn't trip the strict length checks.
#[test]
fn end_frame_round_trips_through_wire_format() {
    let mut buf = [0u8; BRIDGE_FRAME_HEADER_BYTES];
    let written = write_bridge_frame(&mut buf, &RenderCommand::EndFrame).expect("encode");
    assert_eq!(written, BRIDGE_FRAME_HEADER_BYTES);
    let (cmd_out, consumed) = read_bridge_frame(&buf).expect("decode");
    assert_eq!(consumed, BRIDGE_FRAME_HEADER_BYTES);
    assert!(matches!(cmd_out, RenderCommand::EndFrame));
}

/// I.1.0.3 — A single bit flip in the header must surface as
/// `DecodeFailed`, not panic and not pass through to the backend.  The
/// host decoder's resync depends on this strictness — silently
/// accepting a corrupt frame would dispatch garbage state to Vulkan.
#[test]
fn corrupted_header_is_rejected_with_decode_failed() {
    let mut buf = [0u8; BRIDGE_FRAME_HEADER_BYTES + 4];
    write_bridge_frame(
        &mut buf,
        &RenderCommand::BeginFrame {
            surface: SurfaceId(7),
        },
    )
    .expect("encode");
    // Flip a bit in the magic.
    buf[0] ^= 0x01;
    assert_eq!(read_bridge_frame(&buf), Err(GfxError::DecodeFailed));

    // Repair magic, corrupt CRC slot instead.
    buf[0] ^= 0x01;
    buf[12] ^= 0x80;
    assert_eq!(read_bridge_frame(&buf), Err(GfxError::DecodeFailed));

    // Repair CRC, flip the version field — must also be rejected
    // (Gen-1 is strict-equal).
    buf[12] ^= 0x80;
    buf[4] ^= 0x01;
    assert_eq!(read_bridge_frame(&buf), Err(GfxError::DecodeFailed));
}

/// I.1.0.4 — End-to-end: encoder drives a stub `RenderBackend` via a
/// Vec<u8> carrier, demonstrating the carrier-agnostic transport works
/// without any real hypervisor escape.  Same test shape will hold when
/// I.1.x swaps Vec<u8> for a shared-memory ring.
#[test]
fn encoder_drives_stub_backend_through_byte_carrier() {
    /// Stub backend tracks the order and shape of every frame it
    /// receives; tests assert against the trace.
    struct StubBackend {
        log: Vec<RenderCommand<'static>>,
    }
    impl RenderBackend for StubBackend {
        fn submit(&mut self, cmd: &RenderCommand<'_>) -> Result<(), GfxError> {
            // The Gen-1 commands we exercise here are all `Copy` /
            // owned, so we can safely materialize into 'static.  When
            // I.1.1 lands borrowed-slice variants, the backend will
            // copy the slice into its own arena before recording.
            let owned: RenderCommand<'static> = match *cmd {
                RenderCommand::BeginFrame { surface } => RenderCommand::BeginFrame { surface },
                RenderCommand::EndFrame => RenderCommand::EndFrame,
                other => panic!("stub doesn't handle {:?} yet", other),
            };
            self.log.push(owned);
            Ok(())
        }
    }

    // Carrier: a Vec<u8> playing the role of the future shared-memory
    // ring.  Encoder appends; decoder slices from the front.
    let mut carrier: Vec<u8> = Vec::new();
    let mut staging = [0u8; 32];

    for cmd in [
        RenderCommand::BeginFrame {
            surface: SurfaceId(42),
        },
        RenderCommand::EndFrame,
    ] {
        let n = write_bridge_frame(&mut staging, &cmd).expect("encode");
        carrier.extend_from_slice(&staging[..n]);
    }

    let mut backend = StubBackend { log: Vec::new() };
    let mut cursor = 0;
    while cursor < carrier.len() {
        let (cmd, consumed) = read_bridge_frame(&carrier[cursor..]).expect("decode");
        backend.submit(&cmd).expect("submit");
        cursor += consumed;
    }
    assert_eq!(cursor, carrier.len(), "carrier fully consumed");
    assert_eq!(backend.log.len(), 2);
    assert!(matches!(
        backend.log[0],
        RenderCommand::BeginFrame { surface: SurfaceId(42) }
    ));
    assert!(matches!(backend.log[1], RenderCommand::EndFrame));
}

/// I.1.0.5 — Variants we haven't wired the encoder for yet return
/// `InvalidState` rather than silently emitting a malformed frame.
/// This is the contract that lets k-scene fail loudly during the
/// I.2.0 bringup if it tries to emit (say) a CreatePipeline before
/// I.2.0's slice lands.
#[test]
fn encoder_refuses_variants_not_yet_in_scope() {
    let mut buf = [0u8; 64];
    let err = write_bridge_frame(
        &mut buf,
        &RenderCommand::DrawInstanced {
            index_count: 36,
            instance_count: 5000,
        },
    );
    assert_eq!(err, Err(GfxError::InvalidState));
}
