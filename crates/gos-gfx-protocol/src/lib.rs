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
