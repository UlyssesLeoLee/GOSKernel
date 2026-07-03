// gos-graph-clustering-harness — V2.61 graph clustering coefficient
//
// Verifies `gos_runtime::graph_clustering` — the Watts-Strogatz global
// clustering coefficient expressed in parts-per-million.
//
// Algorithm: for each node v with >= 2 undirected neighbors, count how many
// pairs among v's neighbors share an edge (in either direction). Then:
//   clustering_ppm = total_triangle_pairs * 1_000_000 / total_pair_triplets
//
// Interpretation:
//   0         = no triangles anywhere in the graph
//   1_000_000 = every pair of common neighbors is connected (perfect clustering)
//
// VectorAddress namespace: L4=37 (graph-clustering harness).
//
// Test matrix:
//  1.  Empty graph: clustering_ppm=0, n=0
//  2.  Single node: no triplets → ppm=0, n=1
//  3.  Two nodes, A→B: no node has >= 2 neighbors → ppm=0
//  4.  Path A→B→C: B has 2 neighbors (A,C) but they're not connected → ppm=0
//  5.  Triangle A→B, A→C, B→C: all three nodes form full triangles → ppm=1_000_000
//  6.  Directed 3-cycle A→B→C→A: undirected neighbors form complete triangle → ppm=1_000_000
//  7.  Star A→B, A→C, A→D, no inner edges: triplets exist but no triangles → ppm=0
//  8.  Partial: A→B, A→C, B→C, A→D → 3/5 triplets are triangles → ppm=600_000
//  9.  Reset clears clustering state
// 10.  graph_clustering does NOT bump graph_epoch

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const GCL_PLUGIN: PluginId   = PluginId::from_ascii("KL_GCLUS_00");
const GCL_EXEC:   ExecutorId = ExecutorId::from_ascii("gcl.exec.00");

// L4=37 (unique to this harness)
const GCL_VEC_A: VectorAddress = VectorAddress::new(37, 1, 1, 0);
const GCL_VEC_B: VectorAddress = VectorAddress::new(37, 1, 2, 0);
const GCL_VEC_C: VectorAddress = VectorAddress::new(37, 1, 3, 0);
const GCL_VEC_D: VectorAddress = VectorAddress::new(37, 1, 4, 0);

const GCL_ID_A: NodeId = derive_node_id(GCL_PLUGIN, "gcl.alpha");
const GCL_ID_B: NodeId = derive_node_id(GCL_PLUGIN, "gcl.beta");
const GCL_ID_C: NodeId = derive_node_id(GCL_PLUGIN, "gcl.gamma");
const GCL_ID_D: NodeId = derive_node_id(GCL_PLUGIN, "gcl.delta");

const GCL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    GCL_PLUGIN,
    name:         "kl-graph-clustering-harness",
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

fn node_spec(key: &'static str, id: NodeId, schema: u64) -> NodeSpec {
    NodeSpec {
        node_id:           id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       GCL_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(GCL_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(GCL_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

// ── 1. Empty graph: clustering_ppm=0, n=0 ────────────────────────────────────

#[test]
fn empty_graph_clustering_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, n) = gos_runtime::graph_clustering();
    assert_eq!(ppm, 0, "empty graph: ppm must be 0");
    assert_eq!(n,   0, "empty graph: n must be 0");
}

// ── 2. Single node: no triplets → ppm=0 ──────────────────────────────────────

#[test]
fn single_node_no_triplets() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    let (ppm, n) = gos_runtime::graph_clustering();
    assert_eq!(ppm, 0, "single node: no triplets, ppm=0");
    assert_eq!(n,   1, "single node: n=1");
}

// ── 3. Two nodes A→B: no node has >= 2 neighbors → ppm=0 ────────────────────

#[test]
fn two_nodes_one_edge_no_triplets() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    let (ppm, n) = gos_runtime::graph_clustering();
    // A has neighbor B (k=1), B has neighbor A (k=1) — no node qualifies
    assert_eq!(ppm, 0, "A→B: no node has >= 2 neighbors, ppm=0");
    assert_eq!(n,   2, "n=2");
}

// ── 4. Path A→B→C: B has 2 neighbors but they're not connected → ppm=0 ──────

#[test]
fn path_abc_no_triangles() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_node(GCL_VEC_C, "gcl.gamma", GCL_ID_C, 3);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    add_edge(GCL_ID_B, GCL_ID_C, "gcl.bc");
    // B has undirected neighbors {A, C}; pair (A,C): no A-C edge → 0 triangles
    let (ppm, n) = gos_runtime::graph_clustering();
    assert_eq!(ppm, 0, "path A→B→C: no triangles, ppm=0");
    assert_eq!(n,   3, "n=3");
}

// ── 5. Triangle A→B, A→C, B→C: every node fully triangulated → ppm=1_000_000

#[test]
fn triangle_full_clustering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_node(GCL_VEC_C, "gcl.gamma", GCL_ID_C, 3);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    add_edge(GCL_ID_A, GCL_ID_C, "gcl.ac");
    add_edge(GCL_ID_B, GCL_ID_C, "gcl.bc");
    // A:{B,C} pair(B,C)→B→C✓, B:{A,C} pair(A,C)→A→C✓, C:{A,B} pair(A,B)→A→B✓
    // total_triangles=3, total_triplets=3 → ppm=1_000_000
    let (ppm, n) = gos_runtime::graph_clustering();
    assert_eq!(ppm, 1_000_000, "triangle A→B, A→C, B→C: ppm=1_000_000");
    assert_eq!(n,   3,         "n=3");
}

// ── 6. Directed 3-cycle A→B→C→A: undirected neighborhood is complete ─────────

#[test]
fn directed_3_cycle_full_clustering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_node(GCL_VEC_C, "gcl.gamma", GCL_ID_C, 3);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    add_edge(GCL_ID_B, GCL_ID_C, "gcl.bc");
    add_edge(GCL_ID_C, GCL_ID_A, "gcl.ca");
    // A: undirected neighbors = {B (out), C (in)}, pair(B,C): B→C ✓
    // B: neighbors = {A (in), C (out)}, pair(A,C): C→A ✓
    // C: neighbors = {A (out), B (in)}, pair(A,B): A→B ✓
    // total_triangles=3, total_triplets=3 → ppm=1_000_000
    let (ppm, n) = gos_runtime::graph_clustering();
    assert_eq!(ppm, 1_000_000, "directed 3-cycle: ppm=1_000_000 (full clustering)");
    assert_eq!(n,   3,         "n=3");
}

// ── 7. Star A→B, A→C, A→D, no inner edges → ppm=0 ───────────────────────────

#[test]
fn star_no_inner_edges_zero_clustering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_node(GCL_VEC_C, "gcl.gamma", GCL_ID_C, 3);
    add_node(GCL_VEC_D, "gcl.delta", GCL_ID_D, 4);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    add_edge(GCL_ID_A, GCL_ID_C, "gcl.ac");
    add_edge(GCL_ID_A, GCL_ID_D, "gcl.ad");
    // A: neighbors={B,C,D}, triplets=3, no B-C/B-D/C-D edges → triangles=0
    // B,C,D: each has only A as neighbor (k=1) → skip
    let (ppm, n) = gos_runtime::graph_clustering();
    assert_eq!(ppm, 0, "star without inner edges: ppm=0");
    assert_eq!(n,   4, "n=4");
}

// ── 8. Partial: A→B, A→C, B→C, A→D → 3/5 triplets → ppm=600_000 ────────────

#[test]
fn partial_clustering_three_fifths() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_node(GCL_VEC_C, "gcl.gamma", GCL_ID_C, 3);
    add_node(GCL_VEC_D, "gcl.delta", GCL_ID_D, 4);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    add_edge(GCL_ID_A, GCL_ID_C, "gcl.ac");
    add_edge(GCL_ID_B, GCL_ID_C, "gcl.bc");
    add_edge(GCL_ID_A, GCL_ID_D, "gcl.ad");
    // A:{B,C,D}, triplets=3: (B,C)→B→C✓, (B,D)→no, (C,D)→no → triangles(A)=1
    // B:{A,C}, triplets=1: (A,C)→A→C✓ → triangles(B)=1
    // C:{A,B}, triplets=1: (A,B)→A→B✓ → triangles(C)=1
    // D:{A}, k=1 → skip
    // total_triplets=5, total_triangles=3, ppm = 3*1_000_000/5 = 600_000
    let (ppm, n) = gos_runtime::graph_clustering();
    assert_eq!(ppm, 600_000, "partial (3/5 triplets triangulated): ppm=600_000");
    assert_eq!(n,   4,       "n=4");
}

// ── 9. Reset clears clustering ────────────────────────────────────────────────

#[test]
fn reset_clears_clustering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_node(GCL_VEC_C, "gcl.gamma", GCL_ID_C, 3);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    add_edge(GCL_ID_A, GCL_ID_C, "gcl.ac");
    add_edge(GCL_ID_B, GCL_ID_C, "gcl.bc");
    let (before, _) = gos_runtime::graph_clustering();
    assert_eq!(before, 1_000_000, "before reset: full clustering");
    reset();
    let (after, n) = gos_runtime::graph_clustering();
    assert_eq!(after, 0, "after reset: ppm=0");
    assert_eq!(n,     0, "after reset: n=0");
}

// ── 10. graph_clustering does NOT bump graph_epoch ───────────────────────────

#[test]
fn graph_clustering_does_not_bump_epoch() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GCL_VEC_A, "gcl.alpha", GCL_ID_A, 1);
    add_node(GCL_VEC_B, "gcl.beta",  GCL_ID_B, 2);
    add_node(GCL_VEC_C, "gcl.gamma", GCL_ID_C, 3);
    add_edge(GCL_ID_A, GCL_ID_B, "gcl.ab");
    add_edge(GCL_ID_B, GCL_ID_C, "gcl.bc");
    let epoch_before = gos_runtime::graph_epoch();
    let _ = gos_runtime::graph_clustering();
    let epoch_after = gos_runtime::graph_epoch();
    assert_eq!(epoch_after, epoch_before, "graph_clustering must not advance graph_epoch");
}
