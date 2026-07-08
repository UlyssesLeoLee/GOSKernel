#![no_std]

//! Phase R0 — Verlet/XPBD rope solver core.
//!
//! Pure, `#![no_std]`, zero-allocation physics library. This crate owns no
//! graph state and no global statics — it is a library crate in the sense
//! `tools/verify-graph-architecture.ps1` enforces (no builtin-plugin
//! descriptor, no `NodeCell`/`PluginEntry`/`try_mount_cell`), analogous to
//! `k-fat32`. All
//! mutable state lives in a caller-owned `RopeState` value; callers (e.g. a
//! future `fbtest.rs` integration in Phase R2) decide where that value lives
//! and how it is synchronized. This keeps rope physics fully decoupled from
//! `gos-runtime`'s graph — R1 will only ever *read* `RuntimeEdgeType` as a
//! plain caller-supplied value to pick a `RopeMaterial`, never reach into the
//! graph itself.
//!
//! Solver: Extended Position-Based Dynamics (XPBD, Müller et al. 2016).
//! Chosen over explicit spring-mass because position-level projection is
//! unconditionally stable under the large, jittery timesteps a software
//! rasterizer running at 8-13 FPS produces, and `compliance` (`alpha`) is a
//! physical quantity independent of substep count or framerate — the same
//! material behaves the same way regardless of how ragged the frame timing
//! is. Phase R0 implemented integration + the stretch constraint + anchors
//! (anchors are simply particles with `inv_mass = 0`, so no separate anchor
//! constraint pass is needed). Phase R1 (this revision) adds the bend
//! constraint, hard strain limiting, and the edge-type -> material mapping
//! (see [`material`]).

mod material;
pub use material::{
    material_for_edge_type, MATERIAL_CAPABILITY, MATERIAL_COMMUNICATION, MATERIAL_REFERENCE,
    MATERIAL_STRUCTURAL,
};

/// Particles per rope. 8 segments gives a visibly-curved polyline without
/// growing the per-rope solve cost too far past what a straight 2-segment
/// "rope" already costs to rasterize.
pub const PARTICLES_PER_ROPE: usize = 9;
/// Segments per rope (one less than particle count).
pub const SEGMENTS_PER_ROPE: usize = PARTICLES_PER_ROPE - 1;
/// Mirrors `fbtest.rs::MAXE` — one rope per graph edge slot.
pub const MAX_ROPES: usize = 512;
/// Total particle capacity across all rope slots.
pub const MAX_PARTICLES: usize = MAX_ROPES * PARTICLES_PER_ROPE;

/// World-space 3D vector. Plain `[f32; 3]` (not a wrapper struct) so callers
/// can pass coordinates straight from existing `(f32, f32, f32)` position
/// arrays without a conversion layer.
pub type Vec3 = [f32; 3];

const ZERO3: Vec3 = [0.0, 0.0, 0.0];

fn v_add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn v_sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn v_scale(a: Vec3, s: f32) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn v_dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn v_cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn v_len(a: Vec3) -> f32 {
    libm::sqrtf(v_dot(a, a))
}
fn v_normalize_or(a: Vec3, fallback: Vec3) -> Vec3 {
    let len = v_len(a);
    if len > 1e-6 {
        v_scale(a, 1.0 / len)
    } else {
        fallback
    }
}

/// xorshift32 PRNG step. Mirrors the "never zero" idiom used throughout
/// `gos-runtime` (e.g. `graph_*` randomized-walk functions) so rope jitter
/// and impulse fuzzing are reproducible from an explicit caller-owned seed —
/// no hidden global RNG state, no `static mut`.
pub fn xorshift32(state: &mut u32) -> u32 {
    let mut x = if *state == 0 { 0x9E37_79B9 } else { *state };
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// Map a xorshift32 draw to `[-1.0, 1.0]`.
fn xorshift_unit(state: &mut u32) -> f32 {
    let x = xorshift32(state);
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Physical parameters for one rope. A single struct spans Phase R0 (only
/// `linear_density`, `stretch_alpha`, `damping`, `slack_kappa` are consumed
/// by `rope_step` today) through Phase R1 (`bend_alpha`, `max_strain` gate
/// the bend-constraint and strain-limiting passes) so the public API doesn't
/// break when R1 lands — `radius` is render-only, read by the future R2
/// integration, never by the solver itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RopeMaterial {
    /// kg/m. Together with rest length this sets each interior particle's
    /// mass, which sets how strongly external impulses move it.
    pub linear_density: f32,
    /// XPBD compliance for the stretch constraint. 0 = rigid; larger = more
    /// elastic. Units: m/N (inverse stiffness), consumed as `alpha / h^2`.
    pub stretch_alpha: f32,
    /// XPBD compliance for the bend constraint (Phase R1).
    pub bend_alpha: f32,
    /// Per-substep velocity multiplier applied during integration.
    /// `1.0` = lossless; realistic vacuum/near-vacuum ropes sit close to
    /// but under `1.0` (material self-damping only — no air drag in vacuum).
    pub damping: f32,
    /// Rest length = `slack_kappa * straight_line_distance`. `1.0` = taut;
    /// `>1.0` lets the rope hang slack and drift in zero-g.
    pub slack_kappa: f32,
    /// Hard per-segment strain ceiling as a fraction of rest length
    /// (Phase R1 strain-limiting pass).
    pub max_strain: f32,
    /// Visual/collision radius. Unused by the solver; carried here so
    /// materials are a single source of truth for the renderer too.
    pub radius: f32,
}

impl RopeMaterial {
    /// A reasonable default: fairly taut, gently damped, near-rigid.
    /// Individual phases (R1's edge-type table) build on top of this.
    pub const fn default_const() -> Self {
        Self {
            linear_density: 1.0,
            stretch_alpha: 0.0001,
            bend_alpha: 0.02,
            damping: 0.998,
            slack_kappa: 1.15,
            max_strain: 0.08,
            radius: 0.02,
        }
    }
}

impl Default for RopeMaterial {
    fn default() -> Self {
        Self::default_const()
    }
}

/// All rope particle state. Zero-allocation: every array is fixed-size and
/// the whole struct can live in a caller's static or on the stack. No
/// interior mutability, no globals — callers own an instance and pass
/// `&mut` into the free functions below (matches `gos-runtime`'s
/// pass-slices style rather than introducing a second state-ownership
/// pattern into the codebase).
pub struct RopeState {
    pos: [Vec3; MAX_PARTICLES],
    prev: [Vec3; MAX_PARTICLES],
    inv_mass: [f32; MAX_PARTICLES],
    active: [bool; MAX_ROPES],
    seg_rest: [f32; MAX_ROPES],
}

impl RopeState {
    pub const fn new() -> Self {
        Self {
            pos: [ZERO3; MAX_PARTICLES],
            prev: [ZERO3; MAX_PARTICLES],
            inv_mass: [0.0; MAX_PARTICLES],
            active: [false; MAX_ROPES],
            seg_rest: [0.0; MAX_ROPES],
        }
    }

    #[inline]
    fn base(rope_id: usize) -> usize {
        rope_id * PARTICLES_PER_ROPE
    }

    pub fn is_active(&self, rope_id: usize) -> bool {
        self.active[rope_id]
    }

    pub fn deactivate(&mut self, rope_id: usize) {
        self.active[rope_id] = false;
    }

    pub fn particle_pos(&self, rope_id: usize, i: usize) -> Vec3 {
        self.pos[Self::base(rope_id) + i]
    }

    pub fn particle_prev(&self, rope_id: usize, i: usize) -> Vec3 {
        self.prev[Self::base(rope_id) + i]
    }

    pub fn particle_inv_mass(&self, rope_id: usize, i: usize) -> f32 {
        self.inv_mass[Self::base(rope_id) + i]
    }

    /// Overrides one particle's inverse mass. Exposed for tests that need a
    /// rope with both ends free (no anchors) — e.g. a momentum-conservation
    /// check, where `rope_seed`'s default anchored ends would pin exactly
    /// the endpoints whose combined motion the test wants to observe.
    pub fn set_inv_mass(&mut self, rope_id: usize, i: usize, inv_mass: f32) {
        self.inv_mass[Self::base(rope_id) + i] = inv_mass;
    }

    /// Directly displaces a particle's position without touching `prev`.
    /// Because XPBD's implicit velocity is `(pos - prev) / h`, this is
    /// exactly a position-space impulse — used by anchor updates (Phase R2:
    /// move a rope's endpoint every frame as its node orbits) and by
    /// Phase R3's "pluck" interaction.
    pub fn apply_impulse(&mut self, rope_id: usize, i: usize, delta: Vec3) {
        let idx = Self::base(rope_id) + i;
        self.pos[idx] = v_add(self.pos[idx], delta);
    }

    /// Moves an anchor (particle 0 or `PARTICLES_PER_ROPE - 1`) to a new
    /// world position. Anchors have `inv_mass == 0` so the integration pass
    /// never moves them on its own and never reads `prev` for them either —
    /// setting `pos` alone is sufficient; neighboring stretch constraints
    /// read `pos` directly. Call every frame regardless of whether the
    /// rope's topology changed (Phase R2's epoch-aware rebuild only calls
    /// `rope_seed` for genuinely new edges; surviving edges just get their
    /// anchors moved here, preserving momentum in the interior particles).
    pub fn set_anchor(&mut self, rope_id: usize, end: RopeEnd, pos: Vec3) {
        let idx = match end {
            RopeEnd::A => Self::base(rope_id),
            RopeEnd::B => Self::base(rope_id) + PARTICLES_PER_ROPE - 1,
        };
        self.pos[idx] = pos;
    }

    /// Sum of current (possibly stretched/slack) segment lengths.
    pub fn current_length(&self, rope_id: usize) -> f32 {
        let base = Self::base(rope_id);
        let mut total = 0.0;
        for s in 0..SEGMENTS_PER_ROPE {
            total += v_len(v_sub(self.pos[base + s + 1], self.pos[base + s]));
        }
        total
    }

    /// Total rest length (`SEGMENTS_PER_ROPE * seg_rest`).
    pub fn rest_length(&self, rope_id: usize) -> f32 {
        self.seg_rest[rope_id] * SEGMENTS_PER_ROPE as f32
    }

    /// Mass-weighted center-of-mass velocity estimate over particles with
    /// nonzero inverse mass (anchors, being infinite-mass by convention,
    /// are excluded — they are externally driven, not part of the free
    /// system whose momentum should be conserved). `h` is the substep
    /// duration used by the most recent `rope_step` call; pass the same
    /// value for a physically-scaled velocity, or `1.0` to compare relative
    /// magnitudes only.
    pub fn com_velocity(&self, rope_id: usize, h: f32) -> Vec3 {
        let base = Self::base(rope_id);
        let mut mom = ZERO3;
        let mut mass_total = 0.0f32;
        for i in 0..PARTICLES_PER_ROPE {
            let idx = base + i;
            let inv_m = self.inv_mass[idx];
            if inv_m <= 0.0 {
                continue;
            }
            let mass = 1.0 / inv_m;
            let vel = v_scale(v_sub(self.pos[idx], self.prev[idx]), 1.0 / h.max(1e-9));
            mom = v_add(mom, v_scale(vel, mass));
            mass_total += mass;
        }
        if mass_total > 1e-9 {
            v_scale(mom, 1.0 / mass_total)
        } else {
            ZERO3
        }
    }

    /// Total kinetic energy over particles with nonzero inverse mass, in
    /// the same relative units as `com_velocity` (pass a consistent `h`
    /// across calls being compared).
    pub fn kinetic_energy(&self, rope_id: usize, h: f32) -> f32 {
        let base = Self::base(rope_id);
        let mut ke = 0.0f32;
        for i in 0..PARTICLES_PER_ROPE {
            let idx = base + i;
            let inv_m = self.inv_mass[idx];
            if inv_m <= 0.0 {
                continue;
            }
            let mass = 1.0 / inv_m;
            let vel = v_scale(v_sub(self.pos[idx], self.prev[idx]), 1.0 / h.max(1e-9));
            ke += 0.5 * mass * v_dot(vel, vel);
        }
        ke
    }
}

impl Default for RopeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Which end of a rope (particle `0` or `PARTICLES_PER_ROPE - 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RopeEnd {
    A,
    B,
}

/// Seeds (or reseeds) `rope_id` between world points `a` and `b`. Rest
/// length is `slack_kappa * |b - a|`; when there is slack, particles are
/// laid out along a shallow parabolic bow perpendicular to the `a->b` axis
/// rather than collinear, because an exactly-collinear initial state is a
/// degenerate zero-gradient starting condition for the future bend
/// constraint (Phase R1) — better to never produce it than to special-case
/// it later. Pass `rng = None` for an exact, jitter-free straight-line seed
/// (used by determinism/conservation tests); pass `Some(seed)` for normal
/// use, which also adds a tiny per-particle asymmetry so a taut
/// (`slack_kappa == 1.0`) rope isn't perfectly planar either.
pub fn rope_seed(
    state: &mut RopeState,
    rope_id: usize,
    a: Vec3,
    b: Vec3,
    material: &RopeMaterial,
    mut rng: Option<&mut u32>,
) {
    let straight = v_sub(b, a);
    let straight_len = v_len(straight);
    let rest_total = (straight_len * material.slack_kappa).max(0.0);
    let seg_rest = rest_total / SEGMENTS_PER_ROPE as f32;
    state.seg_rest[rope_id] = seg_rest;
    state.active[rope_id] = true;

    let interior_count = PARTICLES_PER_ROPE.saturating_sub(2).max(1);
    let rest_total_safe = rest_total.max(1e-6);
    let mass_per_particle = material.linear_density * rest_total_safe / interior_count as f32;
    let inv_mass_interior = if mass_per_particle > 1e-9 {
        1.0 / mass_per_particle
    } else {
        0.0
    };

    let dir = v_normalize_or(straight, [1.0, 0.0, 0.0]);
    // Arbitrary perpendicular: cross with world-up unless nearly parallel
    // to it, in which case cross with world-X instead.
    let up_hint = if libm::fabsf(dir[1]) < 0.99 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let perp = v_normalize_or(v_cross(dir, up_hint), [0.0, 0.0, 1.0]);
    let bow_amt = (rest_total - straight_len).max(0.0) * 0.5;

    let base = RopeState::base(rope_id);
    for i in 0..PARTICLES_PER_ROPE {
        let t = i as f32 / (PARTICLES_PER_ROPE - 1) as f32;
        let lin = v_add(a, v_scale(straight, t));
        let bow_shape = 4.0 * t * (1.0 - t); // parabola, 0 at ends, 1 at t=0.5
        let jitter = match rng.as_deref_mut() {
            Some(seed) => xorshift_unit(seed) * 1e-4,
            None => 0.0,
        };
        let p = v_add(lin, v_scale(perp, bow_amt * bow_shape + jitter));
        let idx = base + i;
        state.pos[idx] = p;
        state.prev[idx] = p;
        state.inv_mass[idx] = if i == 0 || i == PARTICLES_PER_ROPE - 1 {
            0.0
        } else {
            inv_mass_interior
        };
    }
}

/// XPBD distance-constraint projection between particles `i` and `j`
/// (rest length `rest`, compliance `alpha`, substep duration `h`). Returns
/// early (no-op) when both particles are anchored (`wsum == 0`, division by
/// it would be undefined) or when they are coincident (normalization would
/// be undefined) — both are legitimate transient states, not errors.
fn xpbd_distance(state: &mut RopeState, i: usize, j: usize, rest: f32, alpha: f32, h: f32) {
    let wi = state.inv_mass[i];
    let wj = state.inv_mass[j];
    let wsum = wi + wj;
    if wsum <= 0.0 {
        return;
    }
    let d = v_sub(state.pos[j], state.pos[i]);
    let len = v_len(d);
    if len < 1e-9 {
        return;
    }
    let n = v_scale(d, 1.0 / len);
    let c = len - rest;
    let alpha_tilde = alpha / (h * h);
    let dlambda = -c / (wsum + alpha_tilde);
    let corr = v_scale(n, dlambda);
    if wi > 0.0 {
        state.pos[i] = v_sub(state.pos[i], v_scale(corr, wi));
    }
    if wj > 0.0 {
        state.pos[j] = v_add(state.pos[j], v_scale(corr, wj));
    }
}

/// Runs `substeps` XPBD substeps covering total duration `h_total`.
/// Substepping (rather than more constraint-solver iterations per single
/// large step) is what gives XPBD its framerate-independent stiffness —
/// each substep sees a small, well-conditioned `alpha / h_substep^2`.
/// `materials[rope_id]` supplies that rope's parameters; inactive ropes
/// (`is_active(rope_id) == false`) are skipped entirely.
pub fn rope_step(
    state: &mut RopeState,
    materials: &[RopeMaterial; MAX_ROPES],
    h_total: f32,
    substeps: u32,
) {
    let n = substeps.max(1);
    let h = h_total / n as f32;
    for _ in 0..n {
        substep(state, materials, h);
    }
}

fn substep(state: &mut RopeState, materials: &[RopeMaterial; MAX_ROPES], h: f32) {
    // Integrate: implicit-velocity Verlet, zero external acceleration
    // (zero-g universe — the whole point of this crate's environment
    // model). Anchors (inv_mass == 0) are skipped: they're moved
    // exclusively via `set_anchor`/`apply_impulse`, never by integration.
    for rope_id in 0..MAX_ROPES {
        if !state.active[rope_id] {
            continue;
        }
        let damp = materials[rope_id].damping;
        let base = RopeState::base(rope_id);
        for i in 0..PARTICLES_PER_ROPE {
            let idx = base + i;
            if state.inv_mass[idx] <= 0.0 {
                continue;
            }
            let cur = state.pos[idx];
            let vel = v_scale(v_sub(cur, state.prev[idx]), damp);
            state.prev[idx] = cur;
            state.pos[idx] = v_add(cur, vel);
        }
    }

    // Stretch constraints: neighbor-to-neighbor distance = one rest segment.
    for rope_id in 0..MAX_ROPES {
        if !state.active[rope_id] {
            continue;
        }
        let alpha = materials[rope_id].stretch_alpha;
        let rest = state.seg_rest[rope_id];
        let base = RopeState::base(rope_id);
        for s in 0..SEGMENTS_PER_ROPE {
            xpbd_distance(state, base + s, base + s + 1, rest, alpha, h);
        }
    }

    // Bend constraints: particle i and i+2 held at twice the rest segment
    // length (a straight-line approximation of the true two-segment
    // distance — cheaper than a real dihedral-angle constraint, adequate
    // for a visual rope). Larger `bend_alpha` = floppier; near-zero =
    // resists folding sharply.
    for rope_id in 0..MAX_ROPES {
        if !state.active[rope_id] {
            continue;
        }
        let alpha = materials[rope_id].bend_alpha;
        let rest = state.seg_rest[rope_id] * 2.0;
        let base = RopeState::base(rope_id);
        if PARTICLES_PER_ROPE < 3 {
            continue;
        }
        for s in 0..PARTICLES_PER_ROPE - 2 {
            xpbd_distance(state, base + s, base + s + 2, rest, alpha, h);
        }
    }

    // Strain limiting: hard post-pass ceiling on each segment's length,
    // applied after the compliant constraints above so it always wins —
    // this is what actually bounds worst-case stretch under a large
    // impulse (the stretch constraint's `alpha` alone only *discourages*
    // stretch, it doesn't cap it).
    for rope_id in 0..MAX_ROPES {
        if !state.active[rope_id] {
            continue;
        }
        let rest = state.seg_rest[rope_id];
        let max_len = rest * (1.0 + materials[rope_id].max_strain);
        let base = RopeState::base(rope_id);
        for s in 0..SEGMENTS_PER_ROPE {
            clamp_distance(state, base + s, base + s + 1, max_len);
        }
    }
}

/// Hard distance ceiling between particles `i` and `j`: if their current
/// distance exceeds `max_len`, pulls them back to exactly `max_len` apart
/// (mass-weighted split, same convention as [`xpbd_distance`]). Unlike
/// [`xpbd_distance`] this has no compliance term — it is a rigid clamp,
/// intentionally: strain limiting is meant to be a hard ceiling regardless
/// of material softness, not a softer version of the stretch constraint.
fn clamp_distance(state: &mut RopeState, i: usize, j: usize, max_len: f32) {
    let wi = state.inv_mass[i];
    let wj = state.inv_mass[j];
    let wsum = wi + wj;
    if wsum <= 0.0 {
        return;
    }
    let d = v_sub(state.pos[j], state.pos[i]);
    let len = v_len(d);
    if len <= max_len || len < 1e-9 {
        return;
    }
    let n = v_scale(d, 1.0 / len);
    let excess = len - max_len;
    let corr = v_scale(n, excess);
    if wi > 0.0 {
        state.pos[i] = v_add(state.pos[i], v_scale(corr, wi / wsum));
    }
    if wj > 0.0 {
        state.pos[j] = v_sub(state.pos[j], v_scale(corr, wj / wsum));
    }
}
