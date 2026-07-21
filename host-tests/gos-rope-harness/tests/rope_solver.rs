// gos-rope-harness — Phase R0 Verlet/XPBD rope solver core tests
//
// Verifies `k_rope`: the no_std, zero-allocation physics core that will back
// real (not straight-line) rope rendering in a future fbtest.rs integration
// (Phase R2, see plan/ROPE_PHYSICS_PLAN.md). This harness never touches
// gos-runtime/gos-protocol — k-rope is a pure library crate with zero graph
// coupling by design (Phase R1's edge-type -> material mapping will only
// ever *read* a caller-supplied enum value, never reach into the graph).
//
// Scope note: Phase R0 implements integration + the stretch constraint +
// anchors only. Bend constraints and hard strain-limiting are Phase R1.
// Tests below that touch "how far can a segment stretch" therefore check
// non-divergence, not a hard strain ceiling — that ceiling doesn't exist
// yet by design.
//
//  1. Determinism: identical seed + identical step sequence -> bit-identical trajectory.
//  2. Rest-state stability: an already-at-rest taut rope drifts negligibly over 1000 steps.
//  3. Stretch convergence: a free chain stretched to 2x rest length relaxes back near rest length.
//  4. Non-divergence: a jittered free chain's total length stays bounded (no blow-up).
//  5. Zero-g momentum conservation: free chain, damping=1.0 -> center-of-mass velocity is constant.
//  6. Damping monotonicity: free chain, damping<1.0 -> kinetic energy trends down, no energy spikes.
//  7. Anchor exactness: anchored ends never move, regardless of interior perturbation.
//  8. Slack kappa semantics: rest_length() == slack_kappa * straight-line distance.
//  9. NaN/Inf fuzz: 10,000 steps of random interior impulses never produce non-finite state.
// 10. Rope independence: rope A's motion has zero effect on rope B's trajectory.
// 11. Deactivation freezes: a deactivated rope's particles are frozen; reseeding reactivates it.

use k_rope::{rope_seed, rope_step, xorshift32, RopeEnd, RopeMaterial, RopeState, MAX_ROPES, PARTICLES_PER_ROPE};

fn uniform_materials(mat: RopeMaterial) -> Box<[RopeMaterial; MAX_ROPES]> {
    Box::new([mat; MAX_ROPES])
}

fn stiff_material() -> RopeMaterial {
    RopeMaterial {
        stretch_alpha: 0.0001,
        damping: 0.998,
        ..RopeMaterial::default_const()
    }
}

fn soft_material(damping: f32) -> RopeMaterial {
    RopeMaterial {
        stretch_alpha: 0.01,
        damping,
        slack_kappa: 1.0,
        ..RopeMaterial::default_const()
    }
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

const H_TOTAL: f32 = 1.0 / 60.0;
const SUBSTEPS: u32 = 4;

/// Frees both ends of a freshly-seeded rope (overrides anchor inv_mass to
/// match the interior particles' mass) so tests can observe an untethered
/// chain's own dynamics without anchor pinning interfering.
fn free_both_ends(state: &mut RopeState, rope_id: usize) {
    let interior_inv_mass = state.particle_inv_mass(rope_id, 1);
    state.set_inv_mass(rope_id, 0, interior_inv_mass);
    state.set_inv_mass(rope_id, PARTICLES_PER_ROPE - 1, interior_inv_mass);
}

#[test]
fn determinism_identical_seed_yields_identical_trajectory() {
    let mat = stiff_material();
    let materials = uniform_materials(mat);

    let run = |seed: u32| -> Vec<[f32; 3]> {
        let mut state = Box::new(RopeState::new());
        let mut rng = seed;
        rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng));
        for _ in 0..50 {
            rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
        }
        (0..PARTICLES_PER_ROPE).map(|i| state.particle_pos(0, i)).collect()
    };

    let a = run(0x1234_5678);
    let b = run(0x1234_5678);
    assert_eq!(a, b, "identical seed + identical steps must yield bit-identical trajectories");
}

#[test]
fn rest_state_is_stable_over_1000_steps() {
    // slack_kappa = 1.0 (taut) + rng = None (no jitter) -> particles start
    // exactly on the straight line at exactly the rest spacing, i.e.
    // already at equilibrium.
    let taut = RopeMaterial { slack_kappa: 1.0, ..stiff_material() };
    let materials = uniform_materials(taut);
    let mut state = Box::new(RopeState::new());
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [4.0, 0.0, 0.0], &taut, None);

    let initial: Vec<[f32; 3]> = (0..PARTICLES_PER_ROPE).map(|i| state.particle_pos(0, i)).collect();

    for _ in 0..1000 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
    }

    for i in 0..PARTICLES_PER_ROPE {
        let drift = dist(state.particle_pos(0, i), initial[i]);
        assert!(drift < 1e-3, "particle {i} drifted {drift} from an already-at-rest configuration");
    }
}

#[test]
fn stretched_free_chain_relaxes_toward_rest_length() {
    let mat = soft_material(0.97);
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    let a = [0.0f32, 0.0, 0.0];
    let b = [2.0f32, 0.0, 0.0];
    let mut rng = 0xC0FF_EEu32;
    rope_seed(&mut state, 0, a, b, &mat, Some(&mut rng));
    free_both_ends(&mut state, 0);

    let rest = state.rest_length(0);

    // Double the span: pull each end outward by half the original length.
    state.apply_impulse(0, 0, [-1.0, 0.0, 0.0]);
    state.apply_impulse(0, PARTICLES_PER_ROPE - 1, [1.0, 0.0, 0.0]);

    let stretched_len = state.current_length(0);
    assert!(
        stretched_len > rest * 1.5,
        "sanity check: the pull should have measurably stretched the chain (got {stretched_len}, rest {rest})"
    );

    for _ in 0..300 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
    }

    let settled_len = state.current_length(0);
    let rel_err = (settled_len - rest).abs() / rest;
    assert!(
        rel_err < 0.05,
        "settled length {settled_len} should be within 5% of rest length {rest} (rel_err {rel_err})"
    );
}

#[test]
fn jittered_free_chain_does_not_diverge() {
    let mat = soft_material(0.98);
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    let mut rng = 0x5EED_0001u32;
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng));
    free_both_ends(&mut state, 0);

    // Bounded random displacement of every interior particle — modest
    // relative to the ~0.375 rest segment length. This test is about
    // solver *stability* under perturbation (does the corrective solve
    // ever make things worse than the perturbation itself), not about how
    // large a one-off perturbation the system can absorb — that's a
    // separate, deliberately large-perturbation scenario already covered
    // by `stretched_free_chain_relaxes_toward_rest_length`.
    for i in 1..PARTICLES_PER_ROPE - 1 {
        let dx = (xorshift32(&mut rng) as f32 / u32::MAX as f32 - 0.5) * 0.1;
        let dy = (xorshift32(&mut rng) as f32 / u32::MAX as f32 - 0.5) * 0.1;
        let dz = (xorshift32(&mut rng) as f32 / u32::MAX as f32 - 0.5) * 0.1;
        state.apply_impulse(0, i, [dx, dy, dz]);
    }

    // A single-iteration-per-substep Gauss-Seidel sweep (this solver's
    // approach — substeps over iterations, see rope_step's docs) doesn't
    // guarantee the summed length shrinks on every individual substep:
    // correcting one segment can transiently stretch its neighbor within
    // the same sweep, only to be resolved on a later substep. So this
    // test checks the two properties that actually characterize
    // *divergence* rather than ordinary transient correction dynamics:
    // (a) the state stays finite at every single step (no blow-up), and
    // (b) once transients have had time to settle, length stays bounded
    // near rest length (no long-term drift/instability).
    let rest = state.rest_length(0);
    for step_idx in 0..500 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
        let len = state.current_length(0);
        assert!(len.is_finite(), "step {step_idx}: current_length went non-finite");
        if step_idx >= 50 {
            assert!(
                len < rest * 2.0,
                "step {step_idx}: current_length {len} did not settle near rest {rest} after warm-up"
            );
        }
    }
}

#[test]
fn zero_gravity_momentum_is_conserved_with_no_damping() {
    let mat = RopeMaterial {
        stretch_alpha: 0.001,
        damping: 1.0,
        slack_kappa: 1.1,
        ..RopeMaterial::default_const()
    };
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    let mut rng = 0xABCD_1234u32;
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng));
    free_both_ends(&mut state, 0);

    // Kick one interior particle sideways; total system momentum must not
    // change afterward since XPBD's mass-weighted correction split
    // (Δx_i * w_i = -Δx_j * w_j) is exactly momentum-conserving and
    // damping is disabled (1.0 = lossless).
    state.apply_impulse(0, PARTICLES_PER_ROPE / 2, [0.0, 0.4, 0.0]);
    rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);

    let v0 = state.com_velocity(0, H_TOTAL / SUBSTEPS as f32);
    let mut samples = vec![v0];
    for _ in 0..200 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
        samples.push(state.com_velocity(0, H_TOTAL / SUBSTEPS as f32));
    }

    for (idx, v) in samples.iter().enumerate() {
        let drift = dist(*v, v0);
        assert!(
            drift < 1e-3,
            "sample {idx}: COM velocity {v:?} drifted {drift} from initial {v0:?} under lossless zero-g dynamics"
        );
    }
}

#[test]
fn damping_trends_kinetic_energy_downward() {
    let mat = soft_material(0.95);
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    let mut rng = 0x9999_0001u32;
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng));
    free_both_ends(&mut state, 0);

    state.apply_impulse(0, PARTICLES_PER_ROPE / 2, [0.0, 0.3, 0.0]);

    let h_sub = H_TOTAL / SUBSTEPS as f32;
    let mut samples = Vec::new();
    for _ in 0..20 {
        for _ in 0..10 {
            rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
        }
        samples.push(state.kinetic_energy(0, h_sub));
    }

    for w in samples.windows(2) {
        assert!(
            w[1] <= w[0] * 1.2,
            "kinetic energy spiked from {} to {} between samples (damping should prevent growth)",
            w[0],
            w[1]
        );
    }
    assert!(
        *samples.last().unwrap() < samples[0] * 0.5,
        "kinetic energy should have decayed substantially: first={}, last={}",
        samples[0],
        samples.last().unwrap()
    );
}

#[test]
fn anchors_never_move_regardless_of_interior_perturbation() {
    let mat = soft_material(0.99);
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    let a = [0.0f32, 1.0, -2.0];
    let b = [5.0f32, -1.0, 2.0];
    let mut rng = 0x4242_4242u32;
    rope_seed(&mut state, 0, a, b, &mat, Some(&mut rng));

    let anchor_a = state.particle_pos(0, 0);
    let anchor_b = state.particle_pos(0, PARTICLES_PER_ROPE - 1);

    // Large interior perturbation to stress-test the constraint solver.
    state.apply_impulse(0, PARTICLES_PER_ROPE / 2, [2.0, -1.5, 0.7]);
    for _ in 0..100 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
    }

    assert_eq!(state.particle_pos(0, 0), anchor_a, "anchor A must never move");
    assert_eq!(
        state.particle_pos(0, PARTICLES_PER_ROPE - 1),
        anchor_b,
        "anchor B must never move"
    );
}

#[test]
fn slack_kappa_controls_rest_length_proportionally() {
    let mut state = Box::new(RopeState::new());
    let straight = 4.0f32;
    let kappa = 1.3f32;
    let mat = RopeMaterial { slack_kappa: kappa, ..RopeMaterial::default_const() };
    let mut rng = 0x1111_2222u32;
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [straight, 0.0, 0.0], &mat, Some(&mut rng));

    let expected = straight * kappa;
    let actual = state.rest_length(0);
    assert!(
        (actual - expected).abs() < 1e-3,
        "rest_length {actual} should equal slack_kappa * straight distance = {expected}"
    );
}

#[test]
fn random_impulses_never_produce_nan_or_inf() {
    let mat = soft_material(0.99);
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    // Nonzero seed per the project's xorshift convention (a zero seed
    // would otherwise silently reset via xorshift32's internal guard,
    // masking whether a caller passed a real seed).
    let mut rng = 0x0BAD_F00Du32;
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng));

    for _ in 0..10_000 {
        let pick = 1 + (xorshift32(&mut rng) as usize) % (PARTICLES_PER_ROPE - 2);
        let dx = (xorshift32(&mut rng) as f32 / u32::MAX as f32 - 0.5) * 0.05;
        let dy = (xorshift32(&mut rng) as f32 / u32::MAX as f32 - 0.5) * 0.05;
        let dz = (xorshift32(&mut rng) as f32 / u32::MAX as f32 - 0.5) * 0.05;
        state.apply_impulse(0, pick, [dx, dy, dz]);
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
    }

    for i in 0..PARTICLES_PER_ROPE {
        let p = state.particle_pos(0, i);
        assert!(
            p[0].is_finite() && p[1].is_finite() && p[2].is_finite(),
            "particle {i} became non-finite: {p:?}"
        );
    }
}

#[test]
fn ropes_are_fully_independent() {
    let mat = soft_material(0.98);
    let materials = uniform_materials(mat);

    // Scenario A: rope 0 and rope 1 both seeded and stepped together.
    let mut with_neighbor = Box::new(RopeState::new());
    let mut rng0 = 0xAAAA_0001u32;
    rope_seed(&mut with_neighbor, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng0));
    let mut rng1 = 0xBBBB_0002u32;
    rope_seed(&mut with_neighbor, 1, [10.0, 5.0, -3.0], [12.0, 6.0, -1.0], &mat, Some(&mut rng1));
    free_both_ends(&mut with_neighbor, 0);
    state_perturb_rope0(&mut with_neighbor);
    for _ in 0..80 {
        rope_step(&mut with_neighbor, &materials, H_TOTAL, SUBSTEPS);
    }
    let rope1_with_neighbor: Vec<[f32; 3]> =
        (0..PARTICLES_PER_ROPE).map(|i| with_neighbor.particle_pos(1, i)).collect();

    // Scenario B: only rope 1 exists (rope 0 slot left inactive).
    let mut alone = Box::new(RopeState::new());
    let mut rng1b = 0xBBBB_0002u32;
    rope_seed(&mut alone, 1, [10.0, 5.0, -3.0], [12.0, 6.0, -1.0], &mat, Some(&mut rng1b));
    for _ in 0..80 {
        rope_step(&mut alone, &materials, H_TOTAL, SUBSTEPS);
    }
    let rope1_alone: Vec<[f32; 3]> = (0..PARTICLES_PER_ROPE).map(|i| alone.particle_pos(1, i)).collect();

    assert_eq!(
        rope1_with_neighbor, rope1_alone,
        "rope 1's trajectory must be unaffected by rope 0's presence or motion"
    );
}

fn state_perturb_rope0(state: &mut RopeState) {
    state.apply_impulse(0, PARTICLES_PER_ROPE / 2, [1.0, -0.8, 0.5]);
}

#[test]
fn deactivation_freezes_particles_until_reseeded() {
    let mat = soft_material(0.98);
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    let mut rng = 0x7777_0003u32;
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng));
    free_both_ends(&mut state, 0);
    state.apply_impulse(0, PARTICLES_PER_ROPE / 2, [0.5, 0.5, 0.0]);
    for _ in 0..10 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
    }

    assert!(state.is_active(0));
    state.deactivate(0);
    assert!(!state.is_active(0));

    let frozen: Vec<[f32; 3]> = (0..PARTICLES_PER_ROPE).map(|i| state.particle_pos(0, i)).collect();
    for _ in 0..50 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
    }
    for i in 0..PARTICLES_PER_ROPE {
        assert_eq!(
            state.particle_pos(0, i),
            frozen[i],
            "particle {i} moved after deactivation"
        );
    }

    // Reseeding reactivates the slot.
    let mut rng2 = 0x7777_0004u32;
    rope_seed(&mut state, 0, [1.0, 0.0, 0.0], [4.0, 0.0, 0.0], &mat, Some(&mut rng2));
    assert!(state.is_active(0));
}

#[test]
fn set_anchor_repins_endpoint_and_pulls_neighbor() {
    let mat = soft_material(0.97);
    let materials = uniform_materials(mat);
    let mut state = Box::new(RopeState::new());

    let mut rng = 0x2468_1357u32;
    rope_seed(&mut state, 0, [0.0, 0.0, 0.0], [3.0, 0.0, 0.0], &mat, Some(&mut rng));

    // Move anchor B far away, as Phase R2 will every frame while a node
    // orbits with the camera. set_anchor only touches `pos` (never
    // `prev`), so the anchor itself is repositioned instantly...
    let new_b = [3.0, 4.0, 0.0];
    state.set_anchor(0, RopeEnd::B, new_b);
    assert_eq!(state.particle_pos(0, PARTICLES_PER_ROPE - 1), new_b);

    // ...and the stretch constraint should pull the adjacent interior
    // particle measurably toward the new anchor position over subsequent
    // steps (proving the anchor move actually participates in the solve,
    // not just a cosmetic position write).
    let neighbor_before = state.particle_pos(0, PARTICLES_PER_ROPE - 2);
    let dist_before = dist(neighbor_before, new_b);
    for _ in 0..60 {
        rope_step(&mut state, &materials, H_TOTAL, SUBSTEPS);
    }
    let dist_after = dist(state.particle_pos(0, PARTICLES_PER_ROPE - 2), new_b);
    assert!(
        dist_after < dist_before,
        "neighbor should move closer to the relocated anchor (before {dist_before}, after {dist_after})"
    );

    // The anchor itself must still be exactly where it was placed —
    // moving it doesn't make it movable by the solver.
    assert_eq!(state.particle_pos(0, PARTICLES_PER_ROPE - 1), new_b);
}
