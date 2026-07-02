// gos-graph-health-harness — V2.18 health API tests
//
// Verifies faulted_node_count() and diff_ring_fill() added in V2.18,
// underpinning the new `graph health` shell command.
//
//  1. faulted_node_count returns 0 on empty runtime.
//  2. Freshly registered node is not faulted.
//  3. faulted_count does not exceed proc_count (invariant).
//  4. diff_ring_fill returns 0 on empty runtime.
//  5. Registering a node increases diff_ring_fill above zero.
//  6. diff_ring_fill equals min(diff_total, MAX_DIFF_RING).
//  7. diff_ring_fill is never larger than 128 (MAX_DIFF_RING).
//  8. Multiple node registrations increase diff_ring_fill further.
//  9. faulted_count + healthy_count is consistent with proc_count.
// 10. diff_ring_fill is monotonically non-decreasing across registrations.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const GH_PLUGIN: PluginId  = PluginId::from_ascii("GH_HLTH");
const GH_EXEC:   ExecutorId = ExecutorId::from_ascii("gh.exec");

const GH_KEY_A: &str = "gh.alpha";
const GH_KEY_B: &str = "gh.beta";
const GH_KEY_C: &str = "gh.gamma";
const GH_KEY_D: &str = "gh.delta";
const GH_KEY_E: &str = "gh.epsilon";

const GH_ID_A: NodeId = derive_node_id(GH_PLUGIN, GH_KEY_A);
const GH_ID_B: NodeId = derive_node_id(GH_PLUGIN, GH_KEY_B);
const GH_ID_C: NodeId = derive_node_id(GH_PLUGIN, GH_KEY_C);
const GH_ID_D: NodeId = derive_node_id(GH_PLUGIN, GH_KEY_D);
const GH_ID_E: NodeId = derive_node_id(GH_PLUGIN, GH_KEY_E);

const fn gh_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: GH_EXEC,
        state_schema_hash: 0xAB18,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const GH_SPEC_A: NodeSpec = gh_spec(GH_KEY_A, GH_ID_A);
const GH_SPEC_B: NodeSpec = gh_spec(GH_KEY_B, GH_ID_B);
const GH_SPEC_C: NodeSpec = gh_spec(GH_KEY_C, GH_ID_C);
const GH_SPEC_D: NodeSpec = gh_spec(GH_KEY_D, GH_ID_D);
const GH_SPEC_E: NodeSpec = gh_spec(GH_KEY_E, GH_ID_E);

const GH_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: GH_PLUGIN,
    name: "GH_HLTH",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[GH_SPEC_A, GH_SPEC_B, GH_SPEC_C, GH_SPEC_D, GH_SPEC_E],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

const VEC_A: VectorAddress = VectorAddress::new(6, 1, 0, 1);
const VEC_B: VectorAddress = VectorAddress::new(6, 1, 0, 2);
const VEC_C: VectorAddress = VectorAddress::new(6, 1, 0, 3);
const VEC_D: VectorAddress = VectorAddress::new(6, 1, 0, 4);
const VEC_E: VectorAddress = VectorAddress::new(6, 1, 0, 5);

/// Maximum diff ring capacity (mirrors the constant in gos-runtime).
const MAX_DIFF_RING: usize = 128;

// ── Test 1: empty runtime → faulted_node_count == 0 ─────────────────────────

#[test]
fn empty_faulted_node_count_is_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();

    assert_eq!(
        gos_runtime::faulted_node_count(), 0,
        "empty runtime must report 0 faulted nodes"
    );
}

// ── Test 2: freshly registered node is not faulted ───────────────────────────

#[test]
fn registered_node_is_not_faulted() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_A, GH_SPEC_A);

    assert_eq!(
        gos_runtime::faulted_node_count(), 0,
        "freshly allocated node must not be in Faulted state"
    );
}

// ── Test 3: faulted_count ≤ proc_count (structural invariant) ────────────────

#[test]
fn faulted_count_does_not_exceed_total() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_A, GH_SPEC_A);
    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_B, GH_SPEC_B);
    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_C, GH_SPEC_C);

    let total   = gos_runtime::proc_count();
    let faulted = gos_runtime::faulted_node_count();

    assert!(
        faulted <= total,
        "faulted_node_count ({}) must never exceed proc_count ({})",
        faulted, total
    );
}

// ── Test 4: empty runtime → diff_ring_fill == 0 ──────────────────────────────

#[test]
fn empty_diff_ring_fill_is_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();

    assert_eq!(
        gos_runtime::diff_ring_fill(), 0,
        "fresh runtime must have 0 entries in the diff ring"
    );
}

// ── Test 5: registering a node increases diff_ring_fill ──────────────────────

#[test]
fn register_node_increases_diff_ring_fill() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_A, GH_SPEC_A);

    assert!(
        gos_runtime::diff_ring_fill() > 0,
        "registering a node must push at least one entry into the diff ring"
    );
}

// ── Test 6: diff_ring_fill == min(diff_total, MAX_DIFF_RING) ─────────────────

#[test]
fn diff_ring_fill_equals_min_total_cap() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_A, GH_SPEC_A);
    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_B, GH_SPEC_B);

    let fill  = gos_runtime::diff_ring_fill();
    let total = gos_runtime::diff_total() as usize;
    let expected = total.min(MAX_DIFF_RING);

    assert_eq!(
        fill, expected,
        "diff_ring_fill ({}) must equal min(diff_total={}, MAX_DIFF_RING={})",
        fill, total, MAX_DIFF_RING
    );
}

// ── Test 7: diff_ring_fill ≤ MAX_DIFF_RING always ────────────────────────────

#[test]
fn diff_ring_fill_never_exceeds_max() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    // Register all 5 nodes.
    let vecs  = [VEC_A, VEC_B, VEC_C, VEC_D, VEC_E];
    let specs = [GH_SPEC_A, GH_SPEC_B, GH_SPEC_C, GH_SPEC_D, GH_SPEC_E];
    for (v, s) in vecs.iter().zip(specs.iter()) {
        let _ = gos_runtime::register_node(GH_PLUGIN, *v, *s);
    }

    assert!(
        gos_runtime::diff_ring_fill() <= MAX_DIFF_RING,
        "diff_ring_fill must never exceed MAX_DIFF_RING ({})",
        MAX_DIFF_RING
    );
}

// ── Test 8: multiple registrations further increase diff_ring_fill ────────────

#[test]
fn multiple_registrations_increase_diff_fill() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_A, GH_SPEC_A);
    let fill_1 = gos_runtime::diff_ring_fill();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_B, GH_SPEC_B);
    let fill_2 = gos_runtime::diff_ring_fill();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_C, GH_SPEC_C);
    let fill_3 = gos_runtime::diff_ring_fill();

    assert!(
        fill_1 < fill_2 && fill_2 < fill_3,
        "diff_ring_fill must increase with each additional node registration: {} {} {}",
        fill_1, fill_2, fill_3
    );
}

// ── Test 9: (total - faulted) + faulted == total (partition identity) ─────────

#[test]
fn health_node_counts_consistent() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_A, GH_SPEC_A);
    let _ = gos_runtime::register_node(GH_PLUGIN, VEC_B, GH_SPEC_B);

    let total   = gos_runtime::proc_count();
    let faulted = gos_runtime::faulted_node_count();
    let healthy = total.saturating_sub(faulted);

    assert_eq!(
        healthy + faulted, total,
        "healthy ({}) + faulted ({}) must equal total ({})",
        healthy, faulted, total
    );
}

// ── Test 10: diff_ring_fill is monotonically non-decreasing ──────────────────

#[test]
fn diff_ring_fill_monotonic_with_mutations() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GH_MANIFEST).unwrap();

    let vecs  = [VEC_A, VEC_B, VEC_C, VEC_D, VEC_E];
    let specs = [GH_SPEC_A, GH_SPEC_B, GH_SPEC_C, GH_SPEC_D, GH_SPEC_E];
    let mut last_fill = gos_runtime::diff_ring_fill();

    for (v, s) in vecs.iter().zip(specs.iter()) {
        let _ = gos_runtime::register_node(GH_PLUGIN, *v, *s);
        let fill = gos_runtime::diff_ring_fill();
        assert!(
            fill >= last_fill,
            "diff_ring_fill must be non-decreasing: was {}, now {}",
            last_fill, fill
        );
        last_fill = fill;
    }
}
