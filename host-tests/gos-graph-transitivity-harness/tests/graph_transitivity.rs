// gos-graph-transitivity-harness — V2.63 global graph transitivity
//
// Global transitivity = total_triangles * 1_000_000 / total_triplets (ppm).
//
// Relationship to graph_clustering (V2.61):
//   Both use the same global ratio formula (total_triangles / total_triplets).
//   graph_transitivity additionally exposes the raw triangle and triplet counts,
//   making it useful for structural audits and as a building block for other metrics.
//
// A triplet centered at v is an unordered pair of distinct undirected neighbors
// of v.  A triplet is "closed" (a triangle) when the two neighbors are also
// adjacent to each other.  Each actual triangle contributes 3 triplets (one per
// vertex) to total_triplets and 3 closed triplets to total_triangles.
//
// Therefore:  transitivity = total_triangles / total_triplets
//                           = 3T / open_triplets  (T = true triangle count)
//
// Test matrix:
//  1.  Empty graph                     → transitivity=0  triplets=0
//  2.  Single isolated node            → transitivity=0  triplets=0
//  3.  Two isolated nodes (no edges)   → transitivity=0  triplets=0
//  4.  K₂ (single edge, no triangle)  → transitivity=0  triplets=0  (no node has ≥2 nbrs)
//  5.  Path A→B→C (open triplet)       → transitivity=0  (1 triplet, 0 triangles)
//  6.  K₃ triangle (all edges)         → transitivity=1_000_000 ppm (100%)
//  7.  K₄ complete graph               → transitivity=1_000_000 ppm (100%)
//  8.  Diamond: K₃ + one extra node    → transitivity < 1_000_000 (mixed)
//  9.  ppm matches graph_clustering; raw counts (triangles, triplets) exposed
// 10.  Transitivity is pure-read: does NOT bump graph_epoch

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const TR_PLUGIN: PluginId   = PluginId::from_ascii("KL_TRANS_00");
const TR_EXEC:   ExecutorId = ExecutorId::from_ascii("trans.exec0");

// L4=39 (unique to this harness)
const TR_VEC_A: VectorAddress = VectorAddress::new(39, 1, 1, 0);
const TR_VEC_B: VectorAddress = VectorAddress::new(39, 1, 2, 0);
const TR_VEC_C: VectorAddress = VectorAddress::new(39, 1, 3, 0);
const TR_VEC_D: VectorAddress = VectorAddress::new(39, 1, 4, 0);

const TR_ID_A: NodeId = derive_node_id(TR_PLUGIN, "tr.alpha");
const TR_ID_B: NodeId = derive_node_id(TR_PLUGIN, "tr.beta");
const TR_ID_C: NodeId = derive_node_id(TR_PLUGIN, "tr.gamma");
const TR_ID_D: NodeId = derive_node_id(TR_PLUGIN, "tr.delta");

const TR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    TR_PLUGIN,
    name:         "kl-graph-transitivity-harness",
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

fn node_spec(key: &'static str, minor: u8) -> NodeSpec {
    NodeSpec {
        node_id:           derive_node_id(TR_PLUGIN, key),
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       TR_EXEC,
        state_schema_hash: minor as u64,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(TR_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, minor: u8) {
    gos_runtime::register_node(TR_PLUGIN, vec, node_spec(key, minor)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    let spec = EdgeSpec {
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
    };
    gos_runtime::register_edge(spec).unwrap();
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_transitivity_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, triangles, triplets, n) = gos_runtime::graph_transitivity();
    assert_eq!(ppm, 0,       "empty: ppm must be 0");
    assert_eq!(triangles, 0, "empty: triangles must be 0");
    assert_eq!(triplets, 0,  "empty: triplets must be 0");
    assert_eq!(n, 0,         "empty: node_count must be 0");
}

// ── 2. Single isolated node ───────────────────────────────────────────────────

#[test]
fn single_node_transitivity_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    let (ppm, triangles, triplets, n) = gos_runtime::graph_transitivity();
    assert_eq!(ppm, 0,       "single node: ppm must be 0");
    assert_eq!(triangles, 0, "single node: triangles=0");
    assert_eq!(triplets, 0,  "single node: triplets=0 (no node has >=2 nbrs)");
    assert_eq!(n, 1,         "single node: n=1");
}

// ── 3. Two isolated nodes ─────────────────────────────────────────────────────

#[test]
fn two_isolated_nodes_transitivity_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    let (ppm, _, triplets, _) = gos_runtime::graph_transitivity();
    assert_eq!(ppm, 0,      "two isolated: ppm=0");
    assert_eq!(triplets, 0, "two isolated: no triplets");
}

// ── 4. K₂: one edge, no triangle ─────────────────────────────────────────────

#[test]
fn k2_has_no_triplets_and_zero_transitivity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    let (ppm, _, triplets, _) = gos_runtime::graph_transitivity();
    assert_eq!(ppm, 0,      "K2: no node has >=2 nbrs → ppm=0");
    assert_eq!(triplets, 0, "K2: triplets=0");
}

// ── 5. Open path A→B→C (one open triplet) ────────────────────────────────────

#[test]
fn open_path_abc_has_one_triplet_zero_triangles() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    add_node(TR_VEC_C, "tr.gamma", 3);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    // B has 2 undirected neighbors (A and C) → 1 triplet, A-C not connected
    let (ppm, triangles, triplets, _) = gos_runtime::graph_transitivity();
    assert_eq!(triangles, 0, "open path: 0 closed triangles");
    assert_eq!(triplets,  1, "open path: 1 open triplet (centered at B)");
    assert_eq!(ppm, 0,       "open path: transitivity=0");
}

// ── 6. K₃ triangle: all three edges present ──────────────────────────────────

#[test]
fn k3_triangle_has_full_transitivity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    add_node(TR_VEC_C, "tr.gamma", 3);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_A, TR_ID_C, "ac");
    // Each of A, B, C has 2 undirected neighbors → 3 triplets total.
    // Each triplet is closed → 3 closed triangles.
    let (ppm, triangles, triplets, _) = gos_runtime::graph_transitivity();
    assert_eq!(triangles, 3,         "K3: 3 closed triplets (one per vertex)");
    assert_eq!(triplets,  3,         "K3: 3 total triplets");
    assert_eq!(ppm, 1_000_000,       "K3: transitivity=100%");
}

// ── 7. K₄ complete graph ──────────────────────────────────────────────────────

#[test]
fn k4_complete_graph_has_full_transitivity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    add_node(TR_VEC_C, "tr.gamma", 3);
    add_node(TR_VEC_D, "tr.delta", 4);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_A, TR_ID_C, "ac");
    add_edge(TR_ID_A, TR_ID_D, "ad");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_B, TR_ID_D, "bd");
    add_edge(TR_ID_C, TR_ID_D, "cd");
    // Each node has 3 undirected neighbors → C(3,2)=3 triplets per node → 12 total.
    // Every pair of neighbors in K4 is connected → 12 closed triplets.
    let (ppm, triangles, triplets, _) = gos_runtime::graph_transitivity();
    assert_eq!(triplets,  12,        "K4: 12 total triplets");
    assert_eq!(triangles, 12,        "K4: 12 closed triplets");
    assert_eq!(ppm, 1_000_000,       "K4: transitivity=100%");
}

// ── 8. Diamond: K₃ + extra node D connected to only one K₃ vertex ────────────
//
// Graph: A-B, B-C, A-C (triangle), B-D (extra edge)
// Triplets: A has nbrs B,C → 1 triplet (closed).
//           B has nbrs A,C,D → C(3,2)=3 triplets: (A,C) closed, (A,D) open, (C,D) open.
//           C has nbrs A,B → 1 triplet (A,B) closed.
//           D has nbr B only → 0 triplets.
// Totals: 5 triplets, 3 closed → transitivity = 3/5 = 600_000 ppm.

#[test]
fn diamond_has_partial_transitivity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    add_node(TR_VEC_C, "tr.gamma", 3);
    add_node(TR_VEC_D, "tr.delta", 4);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_A, TR_ID_C, "ac");
    add_edge(TR_ID_B, TR_ID_D, "bd");
    let (ppm, triangles, triplets, _) = gos_runtime::graph_transitivity();
    assert_eq!(triplets,  5,         "diamond: 5 triplets");
    assert_eq!(triangles, 3,         "diamond: 3 closed triplets");
    assert_eq!(ppm, 600_000,         "diamond: transitivity=60%");
}

// ── 9. graph_transitivity exposes raw counts that graph_clustering does not ───
//
// Both graph_clustering and graph_transitivity compute the same ppm ratio
// (total_triangles / total_triplets over the undirected projection).
// graph_transitivity additionally returns the raw triangle and triplet counts,
// making it useful for structural audits and composition with other metrics.
//
// This test verifies ppm agreement and that the raw counts are correct for the
// partial (diamond-variant) topology: A→B, A→C, B→C, A→D → 3/5 triplets closed.

#[test]
fn transitivity_ppm_matches_clustering_and_exposes_raw_counts() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    add_node(TR_VEC_C, "tr.gamma", 3);
    add_node(TR_VEC_D, "tr.delta", 4);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_A, TR_ID_C, "ac");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_A, TR_ID_D, "ad");
    let (trans_ppm, triangles, triplets, _) = gos_runtime::graph_transitivity();
    let (clust_ppm, _) = gos_runtime::graph_clustering();
    // Both use the same global ratio formula
    assert_eq!(trans_ppm, clust_ppm,   "transitivity ppm must equal clustering ppm");
    assert_eq!(trans_ppm, 600_000,     "3/5 triplets closed → 60%");
    // Raw counts only available from graph_transitivity
    assert_eq!(triplets,  5,           "raw triplets=5");
    assert_eq!(triangles, 3,           "raw triangles=3");
}

// ── 10. Transitivity is pure-read: does NOT bump graph_epoch ─────────────────

#[test]
fn transitivity_does_not_bump_epoch() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(TR_VEC_A, "tr.alpha", 1);
    add_node(TR_VEC_B, "tr.beta",  2);
    add_node(TR_VEC_C, "tr.gamma", 3);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_A, TR_ID_C, "ac");
    let epoch_before = gos_runtime::graph_epoch();
    let _ = gos_runtime::graph_transitivity();
    let epoch_after  = gos_runtime::graph_epoch();
    assert_eq!(epoch_before, epoch_after,
        "graph_transitivity must not bump graph_epoch (pure-read)");
}
