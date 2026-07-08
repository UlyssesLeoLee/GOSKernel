//! Phase R1 — material catalog and edge-type -> material mapping.
//!
//! The mapping is driven by `RuntimeEdgeType::lower()` (`gos-protocol`'s
//! ADR-001 edge algebra), not by a hand-written per-name table: every named
//! edge lowers to `EdgeBits{refer,send,bind,grant}`, and those four bits are
//! exactly the axes this module classifies on. This is a single source of
//! truth rather than a second, parallel taxonomy that could drift from the
//! constitution — a future named edge (should ADR-001's set ever grow)
//! gets a sensible material automatically from its bits, no k-rope change
//! required. `refer` is unconditionally true for every named edge (the
//! refer-floor invariant), so only `grant`/`bind`/`send` need classifying.
//!
//! This is the only place in `k-rope` that names `gos-protocol` — the
//! solver core (`lib.rs`) never does, and this module only ever reads a
//! caller-supplied `RuntimeEdgeType` value, never touches graph state.

use crate::RopeMaterial;
use gos_protocol::RuntimeEdgeType;

/// Capability-bearing edges (`grant` bit set: `Call`, `Use`). The strongest
/// constraint in the edge algebra — rendered as a taut steel cable that
/// barely sways, matching how little "slack" a capability grant has.
pub const MATERIAL_CAPABILITY: RopeMaterial = RopeMaterial {
    linear_density: 2.0,
    stretch_alpha: 0.00002,
    bend_alpha: 0.005,
    damping: 0.999,
    slack_kappa: 1.03,
    max_strain: 0.02,
    radius: 0.035,
};

/// Structural/ownership edges (`bind` set, `grant` unset: `Mount`,
/// `Spawn`). Firm but visibly more elastic than a capability edge.
pub const MATERIAL_STRUCTURAL: RopeMaterial = RopeMaterial {
    linear_density: 1.2,
    stretch_alpha: 0.0005,
    bend_alpha: 0.02,
    damping: 0.997,
    slack_kappa: 1.12,
    max_strain: 0.05,
    radius: 0.026,
};

/// Communication edges (`send` set, `bind`/`grant` unset: `Signal`,
/// `Return`, `Sync`, `Stream`). A soft data line with visible slack and
/// liveliness — data is actively flowing through it.
pub const MATERIAL_COMMUNICATION: RopeMaterial = RopeMaterial {
    linear_density: 0.6,
    stretch_alpha: 0.004,
    bend_alpha: 0.06,
    damping: 0.992,
    slack_kappa: 1.25,
    max_strain: 0.10,
    radius: 0.016,
};

/// Weak-reference edges (`refer` only: `Depend`). The thinnest, softest
/// material — biggest slack, drifts the most freely in zero-g.
pub const MATERIAL_REFERENCE: RopeMaterial = RopeMaterial {
    linear_density: 0.25,
    stretch_alpha: 0.02,
    bend_alpha: 0.15,
    damping: 0.985,
    slack_kappa: 1.35,
    max_strain: 0.18,
    radius: 0.008,
};

/// Maps a named edge to its rope material via the edge algebra's own
/// primitive bits (see module docs). Total over all 9
/// [`RuntimeEdgeType`] variants — every arm of `lower()`'s bit
/// combinations is covered by exactly one of the four `if`/`else`
/// branches below, since `grant`/`bind`/`send` are the only bits that
/// vary among named edges.
pub fn material_for_edge_type(edge_type: RuntimeEdgeType) -> RopeMaterial {
    let bits = edge_type.lower().bits;
    if bits.grant {
        MATERIAL_CAPABILITY
    } else if bits.bind {
        MATERIAL_STRUCTURAL
    } else if bits.send {
        MATERIAL_COMMUNICATION
    } else {
        MATERIAL_REFERENCE
    }
}
