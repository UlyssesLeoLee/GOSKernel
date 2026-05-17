#![no_std]

//! Phase I.0 — graphics protocol ABI (host-bridged Vulkan, Gen-1).
//!
//! GOS Gen-1 graphics is *host-bridged*: the kernel-side `k-vk-host`
//! plugin marshals `RenderCommand` records across the hypervisor
//! boundary to a host-side `gos-gfx-bridge-host` process that actually
//! drives Vulkan via `ash`.  This crate owns the stable wire format
//! that both sides agree on.
//!
//! Layering:
//!   * **`gos-gfx-protocol`** (this crate, `no_std`) — handle types,
//!     command enum, error codes, ABI version triple.  No Vulkan
//!     dependency.  Both kernel and host link this.
//!   * **`k-vk-host`** (future, `no_std`) — kernel-side plugin: pushes
//!     `RenderCommand` records into the bridge channel.
//!   * **`k-scene`** (future, `no_std`) — translates runtime graph
//!     mutation envelopes (`ControlPlaneMessageKind::CypherMutationAudited`)
//!     into `RenderCommand` streams.  Reads `gos_runtime::audit_ring`.
//!   * **`k-camera`** (future, `no_std`) — orbit/pan/zoom driven by
//!     `k-mouse` + `k-ime`.
//!   * **`gos-gfx-bridge-host`** (future, host `std`) — decodes
//!     commands, calls `ash`, opens a window, presents frames.
//!
//! Gen-1 explicit non-goals (do not extend this protocol with these
//! until Phase I.2+ is on the table):
//!   * bare-metal Vulkan / virtio-gpu / real GPU drivers
//!   * compute pipelines (use `k-cuda-host` for compute)
//!   * multi-surface / multi-window
//!   * resource sharing across host processes
//!
//! See `doc/PHASE_I_GRAPHICS.md` for the full Gen-1 plan.

/// Stable ABI version triple.  Major mismatch → bridge handshake fails;
/// minor mismatch → bridge accepts with a degraded-feature warning;
/// patch mismatch → silently compatible.  Mirrors `GOS_ABI_VERSION`
/// semantics from `gos-protocol`.
pub const GFX_ABI_VERSION_MAJOR: u16 = 0;
pub const GFX_ABI_VERSION_MINOR: u16 = 1;
pub const GFX_ABI_VERSION_PATCH: u16 = 0;

/// 32-bit packed ABI version: `(major << 16) | (minor << 8) | patch`.
/// Same shape as `gos_protocol::GOS_ABI_VERSION` for consistency.
pub const GFX_ABI_VERSION: u32 = ((GFX_ABI_VERSION_MAJOR as u32) << 16)
    | ((GFX_ABI_VERSION_MINOR as u32) << 8)
    | (GFX_ABI_VERSION_PATCH as u32);

/// Magic header guarding bridge envelopes — `b"GFX1"` little-endian.
/// First field of every bridge frame so host can resync on stream
/// corruption / version skew without crashing the decoder.
pub const GFX_FRAME_MAGIC: u32 = u32::from_le_bytes(*b"GFX1");

// ── Handle types ────────────────────────────────────────────────────
//
// All handles are opaque u32 ids assigned by the bridge host on Create*
// commands and echoed back to the kernel.  Zero is reserved as a
// sentinel ("invalid handle"); the bridge MUST never return zero from
// a Create* response.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SurfaceId(pub u32);
impl SurfaceId {
    pub const INVALID: Self = Self(0);
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PipelineId(pub u32);
impl PipelineId {
    pub const INVALID: Self = Self(0);
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct BufferId(pub u32);
impl BufferId {
    pub const INVALID: Self = Self(0);
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TextureId(pub u32);
impl TextureId {
    pub const INVALID: Self = Self(0);
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

// ── Enums ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PresentMode {
    /// Always present the most recently submitted frame.  Tearing
    /// possible on legacy displays; lowest latency.  Default.
    Immediate = 0,
    /// VK_PRESENT_MODE_FIFO_KHR — guaranteed v-sync, no tearing.
    Vsync = 1,
    /// VK_PRESENT_MODE_MAILBOX_KHR — triple-buffer, no tearing, low
    /// latency.  Falls back to `Vsync` on hardware that doesn't
    /// support it.
    Mailbox = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PipelineKind {
    /// Draws a shape from the Gen-1 instance library (sphere / cube /
    /// octahedron / torus / plane / capsule / tetrahedron / glyph_quad)
    /// per graph node, positioned from the instance buffer.
    NodeInstance = 1,
    /// Cubic bezier line strip per graph edge.
    EdgeLine = 2,
    /// 2D overlay text — shell glyph atlas, HUD labels.
    Text2D = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BufferKind {
    Vertex = 1,
    Instance = 2,
    Index = 3,
    Uniform = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BufferHint {
    /// Uploaded once, drawn many times.  Bridge picks VRAM-resident.
    Static = 0,
    /// Re-uploaded most frames (animated instance buffer, dirty
    /// uniforms).  Bridge picks host-coherent.
    Dynamic = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureFormat {
    Rgba8Unorm = 1,
    Bgra8Unorm = 2,
    R8Unorm = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum GfxError {
    /// Command queue full on the bridge — backpressure.
    QueueFull = -1,
    /// Handle does not exist, was already destroyed, or is zero.
    InvalidHandle = -2,
    /// Surface lost (host window killed, GPU reset).  Caller should
    /// recreate the surface and replay pipeline + buffer creates.
    SurfaceLost = -3,
    /// Vulkan device lost — bridge must teardown and reconnect.
    DeviceLost = -4,
    /// Caller's supervisor quota (RESOURCE_GFX_*) was exhausted.
    /// Distinct from `DeviceOutOfMemory`: caller can request a higher
    /// quota; the device itself still has headroom.
    QuotaExceeded = -5,
    /// Vulkan returned VK_ERROR_OUT_OF_DEVICE_MEMORY.
    DeviceOutOfMemory = -6,
    /// Wire-format / version mismatch.  Decode failed; the bridge
    /// drops the rest of the current frame.
    DecodeFailed = -7,
    /// Operation isn't valid in the current frame state (DrawInstanced
    /// outside Begin/End, double Begin, ...).  Programmer error.
    InvalidState = -8,
}

// ── Command set ─────────────────────────────────────────────────────
//
// `RenderCommand` is the high-level intermediate the kernel emits.  The
// bridge wire format serializes each variant into a fixed header +
// optional variable payload (vertex bytes, shader blob), but that
// serialization is intentionally NOT in this crate — wire encoding
// lives in `k-vk-host` / `gos-gfx-bridge-host`, so we can swap it for
// shared-memory ring buffers in Phase I.x without touching the
// command enum.

/// Phase I.0 command set — minimal vocabulary that's enough to draw
/// a 3D graph.  Resource creates return handles via a separate
/// response channel (see `GfxResponse` once that lands in I.1.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderCommand<'a> {
    CreateSurface {
        width: u32,
        height: u32,
        present_mode: PresentMode,
    },
    CreatePipeline {
        kind: PipelineKind,
        /// Precompiled SPIR-V blob.  Gen-1 forbids runtime HLSL/DXC;
        /// see PHASE_I_GRAPHICS.md.
        shader_spirv: &'a [u8],
    },
    UploadBuffer {
        kind: BufferKind,
        hint: BufferHint,
        bytes: &'a [u8],
    },
    UploadTexture {
        format: TextureFormat,
        width: u32,
        height: u32,
        mips: u8,
        bytes: &'a [u8],
    },
    BeginFrame {
        surface: SurfaceId,
    },
    BindPipeline {
        pipeline: PipelineId,
    },
    BindBuffers {
        vertex: BufferId,
        instance: BufferId,
        index: BufferId,
        uniform: BufferId,
    },
    DrawInstanced {
        index_count: u32,
        instance_count: u32,
    },
    EndFrame,
    DestroySurface(SurfaceId),
    DestroyPipeline(PipelineId),
    DestroyBuffer(BufferId),
    DestroyTexture(TextureId),
}

// ── Phase I.1.0 — bridge transport (wire format + backend trait) ────
//
// Gen-1 transport is intentionally byte-oriented and self-describing
// per frame, so future Phase I.x slices can swap the *carrier* (shared
// memory ring, hypervisor escape, virtio-gpu queue) without touching
// either the kernel-side encoder or the host-side decoder.  Carrier
// choice is a property of the runtime, not the wire.
//
// Frame layout (little-endian):
//
//   offset 0   u32   magic           = GFX_FRAME_MAGIC ("GFX1")
//   offset 4   u32   abi_version     = GFX_ABI_VERSION
//   offset 8   u8    command_tag     (BRIDGE_TAG_*)
//   offset 9   u8    reserved        = 0
//   offset 10  u16   payload_len     (bytes after the header)
//   offset 12  u32   crc32           (over bytes 0..12; payload check
//                                      lives inside variable variants
//                                      that have their own checksum)
//   offset 16  ..    payload         (variant-specific, payload_len
//                                      bytes; 0 for BeginFrame/EndFrame)
//
// CRC over the fixed header guards against bit-flips on the wire so the
// host decoder can resync on the next valid magic+CRC rather than
// crashing.  Payload CRC is per-variant (only Upload* care today).
//
// I.1.0 ships only BeginFrame and EndFrame.  Subsequent slices add tags
// for the rest of the command set:
//   I.1.1 — UploadBuffer + DrawInstanced (variable payload + payload CRC)
//   I.2.0 — CreatePipeline (SPIR-V blob)
//   I.2.1 — CreateSurface + Bind* + Destroy*

pub const BRIDGE_FRAME_HEADER_BYTES: usize = 16;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeCommandTag {
    BeginFrame = 0x01,
    EndFrame = 0x02,
    // Reserved for future slices; do not renumber without an ABI bump.
    // CreateSurface  = 0x10,
    // CreatePipeline = 0x11,
    // UploadBuffer   = 0x12,
    // UploadTexture  = 0x13,
    // BindPipeline   = 0x20,
    // BindBuffers    = 0x21,
    // DrawInstanced  = 0x30,
    // DestroySurface = 0xF0, ...
}

impl BridgeCommandTag {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(Self::BeginFrame),
            0x02 => Some(Self::EndFrame),
            _ => None,
        }
    }
}

/// Minimal CRC32 (IEEE polynomial 0xEDB88320) — same one used by zlib /
/// PNG.  Reference impl, not optimized; the frame header is 12 bytes
/// so even the table-free form runs at well under a microsecond.
pub fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Write a command to the bridge wire.  Returns the number of bytes
/// written (= header for Gen-1 variants with empty payload).  I.1.0
/// only encodes BeginFrame / EndFrame; other variants return
/// `GfxError::InvalidState` until their slice lands.
pub fn write_bridge_frame(out: &mut [u8], cmd: &RenderCommand<'_>) -> Result<usize, GfxError> {
    if out.len() < BRIDGE_FRAME_HEADER_BYTES {
        return Err(GfxError::QueueFull);
    }

    let (tag, surface_id) = match cmd {
        RenderCommand::BeginFrame { surface } => (BridgeCommandTag::BeginFrame, surface.0),
        RenderCommand::EndFrame => (BridgeCommandTag::EndFrame, 0),
        _ => return Err(GfxError::InvalidState),
    };

    // Lay out the first 12 header bytes, then CRC them, then write CRC
    // into bytes 12..16.  This split keeps the CRC input contiguous.
    out[0..4].copy_from_slice(&GFX_FRAME_MAGIC.to_le_bytes());
    out[4..8].copy_from_slice(&GFX_ABI_VERSION.to_le_bytes());
    out[8] = tag as u8;
    out[9] = 0; // reserved
    // payload_len = 4 for BeginFrame (carries u32 surface id IN-LINE in
    // the header to avoid a 4-byte payload + separate length plumbing)
    // — wait: for I.1.0 we keep payload_len=0 and stash surface_id in
    // the reserved area.  Use the high half of the version field?  No,
    // that breaks the ABI check.  Cleanest: use the 4-byte slot at
    // [12..16] for the surface id, and put the CRC AFTER the surface
    // id by extending the header to 20 bytes.  But the doc says 16.
    //
    // Compromise (Gen-1, BeginFrame only): payload_len=4, payload is
    // surface_id (u32 LE).  EndFrame: payload_len=0.  CRC still over
    // first 12 bytes only.  This keeps the header 16 bytes flat;
    // payload (when present) is appended after.
    let payload_len: u16 = match tag {
        BridgeCommandTag::BeginFrame => 4,
        BridgeCommandTag::EndFrame => 0,
    };
    out[10..12].copy_from_slice(&payload_len.to_le_bytes());

    let crc = crc32_ieee(&out[0..12]);
    out[12..16].copy_from_slice(&crc.to_le_bytes());

    let total = BRIDGE_FRAME_HEADER_BYTES + payload_len as usize;
    if out.len() < total {
        return Err(GfxError::QueueFull);
    }
    if payload_len == 4 {
        out[16..20].copy_from_slice(&surface_id.to_le_bytes());
    }
    Ok(total)
}

/// Read a single command from the bridge wire.  Returns the decoded
/// command plus the byte count consumed (so the caller can slide a
/// cursor for stream decoding).
///
/// Decoder is intentionally strict: any magic / version / CRC mismatch
/// returns `DecodeFailed` so the host can resync at the next valid
/// frame boundary rather than silently dispatching a corrupt command.
pub fn read_bridge_frame(input: &[u8]) -> Result<(RenderCommand<'static>, usize), GfxError> {
    if input.len() < BRIDGE_FRAME_HEADER_BYTES {
        return Err(GfxError::DecodeFailed);
    }
    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != GFX_FRAME_MAGIC {
        return Err(GfxError::DecodeFailed);
    }
    let version = u32::from_le_bytes([input[4], input[5], input[6], input[7]]);
    // Gen-1 strict version match.  Phase I.x will relax to "major
    // equal, minor host >= guest" once the bridge negotiates.
    if version != GFX_ABI_VERSION {
        return Err(GfxError::DecodeFailed);
    }
    let payload_len =
        u16::from_le_bytes([input[10], input[11]]) as usize;
    let crc_observed = u32::from_le_bytes([input[12], input[13], input[14], input[15]]);
    let crc_expected = crc32_ieee(&input[0..12]);
    if crc_observed != crc_expected {
        return Err(GfxError::DecodeFailed);
    }
    let total = BRIDGE_FRAME_HEADER_BYTES + payload_len;
    if input.len() < total {
        return Err(GfxError::DecodeFailed);
    }

    let tag = BridgeCommandTag::from_u8(input[8]).ok_or(GfxError::DecodeFailed)?;
    let cmd = match tag {
        BridgeCommandTag::BeginFrame => {
            if payload_len != 4 {
                return Err(GfxError::DecodeFailed);
            }
            let surface_raw =
                u32::from_le_bytes([input[16], input[17], input[18], input[19]]);
            RenderCommand::BeginFrame {
                surface: SurfaceId(surface_raw),
            }
        }
        BridgeCommandTag::EndFrame => {
            if payload_len != 0 {
                return Err(GfxError::DecodeFailed);
            }
            RenderCommand::EndFrame
        }
    };
    Ok((cmd, total))
}

/// Carrier-agnostic backend the host process implements.  Tests stub
/// this with a counter; the future `gos-gfx-bridge-host` impl calls
/// into `ash` (Vulkan) or `wgpu`; future I.2 virtio-gpu impl wraps the
/// guest paravirt queue.
///
/// `submit` is called once per decoded frame.  The backend is free to
/// batch internally (e.g. coalesce many DrawInstanced under one Vulkan
/// command buffer) — the protocol semantics are "BeginFrame ... draws
/// ... EndFrame triggers vkQueueSubmit + vkQueuePresent".
pub trait RenderBackend {
    fn submit(&mut self, cmd: &RenderCommand<'_>) -> Result<(), GfxError>;
}

// ── Compile-time sanity ─────────────────────────────────────────────
//
// Handle types must stay 4 bytes — `k-vk-host` packs them into the
// wire envelope assuming this size.  If anyone ever bumps a handle
// to u64 without thinking, this test fires at compile time.

const _: () = {
    if core::mem::size_of::<SurfaceId>() != 4 {
        panic!("SurfaceId must remain 4 bytes for wire compatibility");
    }
    if core::mem::size_of::<PipelineId>() != 4 {
        panic!("PipelineId must remain 4 bytes for wire compatibility");
    }
    if core::mem::size_of::<BufferId>() != 4 {
        panic!("BufferId must remain 4 bytes for wire compatibility");
    }
    if core::mem::size_of::<TextureId>() != 4 {
        panic!("TextureId must remain 4 bytes for wire compatibility");
    }
};
