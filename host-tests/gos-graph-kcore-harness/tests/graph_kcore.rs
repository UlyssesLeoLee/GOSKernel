// gos-graph-kcore-harness — V2.64 k-core decomposition
//
// Verifies `gos_runtime::graph_kcore` — compute the coreness of each node
// (the largest k such that the node is in the k-core) and the graph
// degeneracy (maximum coreness).
//
// Algorithm: Batagelj-Zaversnik iterative peeling.
//   For k = 1, 2, ..., max_degree:
//     Repeatedly remove nodes whose effective undirected degree < k,
//     updating their neighbours' degrees.  Removed nodes get coreness = k-1.
//   Surviving nodes get coreness = max_degree.
//
// Return value: (vecs, coreness, n, max_coreness)
//   vecs[0..n]     — nodes sorted by coreness descending
//   coreness[0..n] — coreness for each node
//   n              — total live node count
//   max_coreness   — graph degeneracy
//
// Key invariants:
//   Empty graph                     → n=0, max_coreness=0
//   Isolated node                   → coreness=0 (not in any 1-core)
//   Path A→B→C                      → coreness=1 for all (each in 1-core, not 2-core)
//   Triangle A-B-C                  → coreness=2 for all
//   Triangle + pendant D-A          → A,B,C coreness=2; D coreness=1
//   K4 complete                     → coreness=3 for all (degeneracy=3)
//   Output sorted descending        → coreness[i] >= coreness[i+1]
//   n matches registered nodes      → correct count
//   Star graph (hub + leaves)       → hub coreness=1, leaves coreness=1 (if ≥2 leaves)
//   Two disjoint triangles          → all coreness=2, degeneracy=2
//
// Test matrix:
//  1.  Empty graph                  → n=0, max_coreness=0
//  2.  Single isolated node         → coreness[0]=0
//  3.  Path A→B→C (3 nodes)        → all coreness=1, degeneracy=1
//  4.  Triangle A-B-C (3-clique)   → all coreness=2, degeneracy=2
//  5.  Triangle + pendant D→A      → A/B/C coreness=2, D coreness=1
//  6.  K4 complete graph            → all coreness=3, degeneracy=3
//  7.  Output sorted descending     → coreness[i] >= coreness[i+1]
//  8.  n matches registered count  → correct for multi-node graph
//  9.  Star: hub + 3 leaves         → all coreness=1 (hub and leaves in 1-core)
// 10.  Two disjoint triangles       → all 6 nodes coreness=2, degeneracy=2

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const KC_PLUGIN: PluginId   = PluginId::from_ascii("KL_KCORE_0");
const KC_EXEC:   ExecutorId = ExecutorId::from_ascii("kcore.exec");

const KC_KEY_A: &str = "kc.alpha";
const KC_KEY_B: &str = "kc.beta";
const KC_KEY_C: &str = "kc.gamma";
const KC_KEY_D: &str = "kc.delta";
const KC_KEY_E: &str = "kc.epsilon";
const KC_KEY_F: &str = "kc.zeta";
const KC_KEY_G: &str = "kc.eta";

const KC_ID_A: NodeId = derive_node_id(KC_PLUGIN, KC_KEY_A);
const KC_ID_B: NodeId = derive_node_id(KC_PLUGIN, KC_KEY_B);
const KC_ID_C: NodeId = derive_node_id(KC_PLUGIN, KC_KEY_C);
const KC_ID_D: NodeId = derive_node_id(KC_PLUGIN, KC_KEY_D);
const KC_ID_E: NodeId = derive_node_id(KC_PLUGIN, KC_KEY_E);
const KC_ID_F: NodeId = derive_node_id(KC_PLUGIN, KC_KEY_F);
const KC_ID_G: NodeId = derive_node_id(KC_PLUGIN, KC_KEY_G);

const KC_VEC_A: VectorAddress = VectorAddress::new(40, 1, 1, 0);
const KC_VEC_B: VectorAddress = VectorAddress::new(40, 1, 2, 0);
const KC_VEC_C: VectorAddress = VectorAddress::new(40, 1, 3, 0);
const KC_VEC_D: VectorAddress = VectorAddress::new(40, 1, 4, 0);
const KC_VEC_E: VectorAddress = VectorAddress::new(40, 1, 5, 0);
const KC_VEC_F: VectorAddress = VectorAddress::new(40, 1, 6, 0);
const KC_VEC_G: VectorAddress = VectorAddress::new(40, 1, 7, 0);

const KC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    KC_PLUGIN,
    name:         "kl-graph-kcore-harness",
    version:      1,
    depends_on:   &[],
    permissions:  &[],
    exports:      &[],
    imports:      &[],
    nodes:        &[],
    edges:        &[],
    signature:    None,
    policy_hash:  [0u8; 16],
};

fn node_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       KC_EXEC,
        state_schema_hash: 0xC0C0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(KC_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(KC_PLUGIN, vec, node_spec(key, id)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    gos_runtime::register_edge(EdgeSpec {
        edge_id:              derive_edge_id(from, to, key),
        from_node:            from,
        to_node:              to,
        edge_type:            RuntimeEdgeType::Signal,
        weight:               1.0,
        acl_mask:             u64::MAX,
        route_policy:         RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding:   None,
        vector_ref:           None,
    }).unwrap();
}

fn run_kcore() -> ([VectorAddress; 128], [u8; 128], usize, u8) {
    gos_runtime::graph_kcore::<128>()
}

fn coreness_for(vecs: &[VectorAddress; 128], core: &[u8; 128], n: usize, target: VectorAddress) -> Option<u8> {
    for i in 0..n {
        if vecs[i] == target { return Some(core[i]); }
    }
    None
}

// ── 1. Empty graph → n=0, max_coreness=0 ─────────────────────────────────────

#[test]
fn empty_graph_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_, _, n, max_core) = run_kcore();
    assert_eq!(n, 0, "empty: n must be 0");
    assert_eq!(max_core, 0, "empty: max_coreness must be 0");
}

// ── 2. Single isolated node → coreness=0 ─────────────────────────────────────

#[test]
fn single_isolated_node_coreness_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);

    let (vecs, core, n, max_core) = run_kcore();
    assert_eq!(n, 1, "single node: n=1");
    assert_eq!(max_core, 0, "isolated node: degeneracy=0");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_A), Some(0),
        "isolated node A: coreness must be 0");
}

// ── 3. Path A→B→C → all coreness=1, degeneracy=1 ─────────────────────────────

#[test]
fn path_graph_coreness_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    // Path: A-B-C (undirected view: A-B, B-C)
    add_edge(KC_ID_A, KC_ID_B, "kc.ab.t3");
    add_edge(KC_ID_B, KC_ID_C, "kc.bc.t3");

    let (vecs, core, n, max_core) = run_kcore();
    assert_eq!(n, 3, "path: 3 nodes");
    assert_eq!(max_core, 1, "path: degeneracy=1");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_A), Some(1), "path: A coreness=1");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_B), Some(1), "path: B coreness=1");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_C), Some(1), "path: C coreness=1");
}

// ── 4. Triangle A-B-C → all coreness=2, degeneracy=2 ─────────────────────────

#[test]
fn triangle_coreness_two() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    add_edge(KC_ID_A, KC_ID_B, "kc.ab.t4");
    add_edge(KC_ID_B, KC_ID_C, "kc.bc.t4");
    add_edge(KC_ID_C, KC_ID_A, "kc.ca.t4");

    let (vecs, core, n, max_core) = run_kcore();
    assert_eq!(n, 3, "triangle: 3 nodes");
    assert_eq!(max_core, 2, "triangle: degeneracy=2");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_A), Some(2), "triangle: A coreness=2");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_B), Some(2), "triangle: B coreness=2");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_C), Some(2), "triangle: C coreness=2");
}

// ── 5. Triangle + pendant D→A → A/B/C coreness=2, D coreness=1 ───────────────

#[test]
fn triangle_plus_pendant_mixed_coreness() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    add_node(KC_VEC_D, KC_KEY_D, KC_ID_D);
    // Triangle
    add_edge(KC_ID_A, KC_ID_B, "kc.ab.t5");
    add_edge(KC_ID_B, KC_ID_C, "kc.bc.t5");
    add_edge(KC_ID_C, KC_ID_A, "kc.ca.t5");
    // Pendant
    add_edge(KC_ID_D, KC_ID_A, "kc.da.t5");

    let (vecs, core, n, max_core) = run_kcore();
    assert_eq!(n, 4, "triangle+pendant: 4 nodes");
    assert_eq!(max_core, 2, "triangle+pendant: degeneracy=2");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_A), Some(2), "A in triangle: coreness=2");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_B), Some(2), "B in triangle: coreness=2");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_C), Some(2), "C in triangle: coreness=2");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_D), Some(1), "D pendant: coreness=1");
}

// ── 6. K4 complete graph → all coreness=3, degeneracy=3 ──────────────────────

#[test]
fn k4_complete_graph_coreness_three() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    add_node(KC_VEC_D, KC_KEY_D, KC_ID_D);
    // All 6 undirected edges of K4
    add_edge(KC_ID_A, KC_ID_B, "kc.ab.t6");
    add_edge(KC_ID_A, KC_ID_C, "kc.ac.t6");
    add_edge(KC_ID_A, KC_ID_D, "kc.ad.t6");
    add_edge(KC_ID_B, KC_ID_C, "kc.bc.t6");
    add_edge(KC_ID_B, KC_ID_D, "kc.bd.t6");
    add_edge(KC_ID_C, KC_ID_D, "kc.cd.t6");

    let (vecs, core, n, max_core) = run_kcore();
    assert_eq!(n, 4, "K4: 4 nodes");
    assert_eq!(max_core, 3, "K4: degeneracy=3");
    for i in 0..n {
        assert_eq!(core[i], 3, "K4: all nodes must have coreness=3 (got {} at {})", core[i], i);
    }
    // All should be 'core' role
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_A), Some(3), "K4: A coreness=3");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_D), Some(3), "K4: D coreness=3");
}

// ── 7. Output sorted descending ───────────────────────────────────────────────

#[test]
fn output_sorted_descending() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Triangle (coreness=2) + pendant (coreness=1) + isolated (coreness=0)
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    add_node(KC_VEC_D, KC_KEY_D, KC_ID_D);
    add_node(KC_VEC_E, KC_KEY_E, KC_ID_E); // isolated
    add_edge(KC_ID_A, KC_ID_B, "kc.ab.t7");
    add_edge(KC_ID_B, KC_ID_C, "kc.bc.t7");
    add_edge(KC_ID_C, KC_ID_A, "kc.ca.t7");
    add_edge(KC_ID_D, KC_ID_A, "kc.da.t7"); // pendant

    let (_, core, n, _) = run_kcore();
    assert_eq!(n, 5, "5 nodes");
    for i in 0..(n.saturating_sub(1)) {
        assert!(
            core[i] >= core[i + 1],
            "output must be sorted descending: core[{}]={} < core[{}]={}",
            i, core[i], i + 1, core[i + 1]
        );
    }
}

// ── 8. n matches registered node count ────────────────────────────────────────

#[test]
fn n_matches_registered_node_count() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    add_node(KC_VEC_D, KC_KEY_D, KC_ID_D);
    add_node(KC_VEC_E, KC_KEY_E, KC_ID_E);
    add_node(KC_VEC_F, KC_KEY_F, KC_ID_F);

    let (_, _, n, _) = run_kcore();
    assert_eq!(n, 6, "n must match 6 registered nodes");
}

// ── 9. Star graph: hub + 3 leaves → all coreness=1 ───────────────────────────

#[test]
fn star_graph_all_coreness_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Hub = A, leaves = B, C, D
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    add_node(KC_VEC_D, KC_KEY_D, KC_ID_D);
    add_edge(KC_ID_A, KC_ID_B, "kc.ab.t9");
    add_edge(KC_ID_A, KC_ID_C, "kc.ac.t9");
    add_edge(KC_ID_A, KC_ID_D, "kc.ad.t9");

    let (vecs, core, n, max_core) = run_kcore();
    assert_eq!(n, 4, "star: 4 nodes");
    // Each leaf has undirected degree 1; hub has degree 3.
    // 1-core: all nodes have degree >= 1 → all are in 1-core.
    // 2-core: hub has degree 3 but leaves have degree 1 < 2 → leaves removed.
    //   After removing leaves, hub has degree 0 < 2 → hub removed.
    // So all coreness = 1, degeneracy = 1.
    assert_eq!(max_core, 1, "star: degeneracy=1");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_A), Some(1), "hub coreness=1");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_B), Some(1), "leaf B coreness=1");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_C), Some(1), "leaf C coreness=1");
    assert_eq!(coreness_for(&vecs, &core, n, KC_VEC_D), Some(1), "leaf D coreness=1");
}

// ── 10. Two disjoint triangles → all 6 nodes coreness=2, degeneracy=2 ─────────

#[test]
fn two_disjoint_triangles_coreness_two() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Triangle 1: A-B-C
    add_node(KC_VEC_A, KC_KEY_A, KC_ID_A);
    add_node(KC_VEC_B, KC_KEY_B, KC_ID_B);
    add_node(KC_VEC_C, KC_KEY_C, KC_ID_C);
    add_edge(KC_ID_A, KC_ID_B, "kc.ab.t10");
    add_edge(KC_ID_B, KC_ID_C, "kc.bc.t10");
    add_edge(KC_ID_C, KC_ID_A, "kc.ca.t10");
    // Triangle 2: D-E-F
    add_node(KC_VEC_D, KC_KEY_D, KC_ID_D);
    add_node(KC_VEC_E, KC_KEY_E, KC_ID_E);
    add_node(KC_VEC_F, KC_KEY_F, KC_ID_F);
    add_edge(KC_ID_D, KC_ID_E, "kc.de.t10");
    add_edge(KC_ID_E, KC_ID_F, "kc.ef.t10");
    add_edge(KC_ID_F, KC_ID_D, "kc.fd.t10");

    let (vecs, core, n, max_core) = run_kcore();
    assert_eq!(n, 6, "two triangles: 6 nodes");
    assert_eq!(max_core, 2, "two triangles: degeneracy=2");
    for target in [KC_VEC_A, KC_VEC_B, KC_VEC_C, KC_VEC_D, KC_VEC_E, KC_VEC_F] {
        assert_eq!(
            coreness_for(&vecs, &core, n, target),
            Some(2),
            "all triangle nodes must have coreness=2"
        );
    }
}
