// gos-graph-density-harness — V2.59 graph density metric
//
// Verifies `gos_runtime::graph_density` — the ratio E / (N*(N-1)) for a
// directed graph, expressed in parts-per-million (ppm).
//
// density_ppm interpretation:
//   0           = empty / single-node graph (undefined)
//   500_000     = 50% — half of all directed pairs are connected
//   1_000_000   = 100% — complete directed graph (K_n)
//
// Formula: density_ppm = E * 1_000_000 / (N * (N-1))
//
// VectorAddress namespace: L4=35 (graph-density harness).
//
// Test matrix:
//  1.  Empty graph: density_ppm=0, n=0, e=0
//  2.  Single node, no edges: density_ppm=0 (< 2 nodes), n=1, e=0
//  3.  Two nodes, 0 edges: density_ppm=0, n=2, e=0
//  4.  Two nodes, 1 edge (A→B): density_ppm=500_000 (50%), n=2, e=1
//  5.  Two nodes, 2 edges (A↔B complete): density_ppm=1_000_000 (100%)
//  6.  Four nodes, 4 edges: density_ppm=333_333 (≈33.33%), n=4, e=4
//  7.  Four nodes, 12 edges (complete K4): density_ppm=1_000_000 (100%)
//  8.  Reset clears density: after reset ppm=0, n=0, e=0
//  9.  Path A→B→C (n=3, e=2): density_ppm=333_333 (2/6≈33.33%)
// 10.  graph_density does NOT bump graph_epoch

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const GD_PLUGIN: PluginId   = PluginId::from_ascii("KL_GDENS_00");
const GD_EXEC:   ExecutorId = ExecutorId::from_ascii("gdens.exec0");

// L4=35 (unique to this harness)
const GD_VEC_A: VectorAddress = VectorAddress::new(35, 1, 1, 0);
const GD_VEC_B: VectorAddress = VectorAddress::new(35, 1, 2, 0);
const GD_VEC_C: VectorAddress = VectorAddress::new(35, 1, 3, 0);
const GD_VEC_D: VectorAddress = VectorAddress::new(35, 1, 4, 0);

const GD_ID_A: NodeId = derive_node_id(GD_PLUGIN, "gd.alpha");
const GD_ID_B: NodeId = derive_node_id(GD_PLUGIN, "gd.beta");
const GD_ID_C: NodeId = derive_node_id(GD_PLUGIN, "gd.gamma");
const GD_ID_D: NodeId = derive_node_id(GD_PLUGIN, "gd.delta");

const GD_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    GD_PLUGIN,
    name:         "kl-graph-density-harness",
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
        executor_id:       GD_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(GD_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(GD_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

// ── 1. Empty graph: density_ppm=0, n=0, e=0 ──────────────────────────────────

#[test]
fn empty_graph_density_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (ppm, n, e) = gos_runtime::graph_density();
    assert_eq!(ppm, 0, "empty graph: density_ppm must be 0");
    assert_eq!(n,   0, "empty graph: n must be 0");
    assert_eq!(e,   0, "empty graph: e must be 0");
}

// ── 2. Single node, no edges: density undefined → ppm=0 ──────────────────────

#[test]
fn single_node_density_undefined_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    let (ppm, n, e) = gos_runtime::graph_density();
    assert_eq!(ppm, 0, "single node: density undefined, must be 0");
    assert_eq!(n,   1, "single node: n must be 1");
    assert_eq!(e,   0, "single node: e must be 0");
}

// ── 3. Two nodes, 0 edges: density=0 ─────────────────────────────────────────

#[test]
fn two_nodes_no_edges_density_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    let (ppm, n, e) = gos_runtime::graph_density();
    assert_eq!(ppm, 0,   "two nodes, no edges: density=0");
    assert_eq!(n,   2,   "n must be 2");
    assert_eq!(e,   0,   "e must be 0");
}

// ── 4. Two nodes, 1 edge (A→B): density=50% = 500_000 ppm ───────────────────

#[test]
fn two_nodes_one_edge_density_fifty_percent() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    add_edge(GD_ID_A, GD_ID_B, "gd.ab");
    let (ppm, n, e) = gos_runtime::graph_density();
    // max = 2*(2-1) = 2; density = 1/2 = 500_000 ppm
    assert_eq!(ppm, 500_000, "A→B: density must be 500_000 ppm (50%)");
    assert_eq!(n,   2,       "n=2");
    assert_eq!(e,   1,       "e=1");
}

// ── 5. Two nodes, 2 edges (complete K2): density=100% = 1_000_000 ppm ────────

#[test]
fn two_nodes_two_edges_complete_graph_density_100() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    add_edge(GD_ID_A, GD_ID_B, "gd.ab");
    add_edge(GD_ID_B, GD_ID_A, "gd.ba");
    let (ppm, n, e) = gos_runtime::graph_density();
    assert_eq!(ppm, 1_000_000, "K2: density must be 1_000_000 ppm (100%)");
    assert_eq!(n,   2,         "n=2");
    assert_eq!(e,   2,         "e=2");
}

// ── 6. Four nodes, 4 edges: density≈33.33% = 333_333 ppm ────────────────────

#[test]
fn four_nodes_four_edges_density_33pct() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    add_node(GD_VEC_C, "gd.gamma", GD_ID_C, 3);
    add_node(GD_VEC_D, "gd.delta", GD_ID_D, 4);
    add_edge(GD_ID_A, GD_ID_B, "gd.ab");
    add_edge(GD_ID_B, GD_ID_C, "gd.bc");
    add_edge(GD_ID_C, GD_ID_D, "gd.cd");
    add_edge(GD_ID_D, GD_ID_A, "gd.da");
    let (ppm, n, e) = gos_runtime::graph_density();
    // max = 4*(4-1) = 12; density = 4/12 = 333_333 ppm (truncated)
    assert_eq!(ppm, 333_333, "4 nodes, 4 edges: density must be 333_333 ppm (≈33.33%)");
    assert_eq!(n,   4,       "n=4");
    assert_eq!(e,   4,       "e=4");
}

// ── 7. Complete K4 (12 edges): density=100% = 1_000_000 ppm ─────────────────

#[test]
fn complete_k4_density_100_pct() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    add_node(GD_VEC_C, "gd.gamma", GD_ID_C, 3);
    add_node(GD_VEC_D, "gd.delta", GD_ID_D, 4);
    let nodes = [(GD_ID_A, "a"), (GD_ID_B, "b"), (GD_ID_C, "c"), (GD_ID_D, "d")];
    for (from, fk) in nodes {
        for (to, tk) in nodes {
            if from != to {
                let key: &'static str = Box::leak(format!("gd.{fk}{tk}").into_boxed_str());
                add_edge(from, to, key);
            }
        }
    }
    let (ppm, n, e) = gos_runtime::graph_density();
    assert_eq!(ppm, 1_000_000, "K4 complete: density must be 1_000_000 ppm (100%)");
    assert_eq!(n,   4,         "n=4");
    assert_eq!(e,   12,        "K4 has 4*(4-1)=12 directed edges");
}

// ── 8. Reset clears density ───────────────────────────────────────────────────

#[test]
fn reset_clears_density() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    add_edge(GD_ID_A, GD_ID_B, "gd.ab");
    let (before_ppm, ..) = gos_runtime::graph_density();
    assert_ne!(before_ppm, 0, "before reset: density must be nonzero");
    reset();
    let (ppm, n, e) = gos_runtime::graph_density();
    assert_eq!(ppm, 0, "after reset: density must be 0");
    assert_eq!(n,   0, "after reset: n must be 0");
    assert_eq!(e,   0, "after reset: e must be 0");
}

// ── 9. Path A→B→C (n=3, e=2): density≈33.33% = 333_333 ppm ─────────────────

#[test]
fn three_node_path_density_33pct() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    add_node(GD_VEC_C, "gd.gamma", GD_ID_C, 3);
    add_edge(GD_ID_A, GD_ID_B, "gd.ab");
    add_edge(GD_ID_B, GD_ID_C, "gd.bc");
    let (ppm, n, e) = gos_runtime::graph_density();
    // max = 3*(3-1) = 6; density = 2/6 = 333_333 ppm (truncated)
    assert_eq!(ppm, 333_333, "A→B→C path: density must be 333_333 ppm (≈33.33%)");
    assert_eq!(n,   3,       "n=3");
    assert_eq!(e,   2,       "e=2");
}

// ── 10. graph_density does NOT bump graph_epoch ───────────────────────────────

#[test]
fn graph_density_does_not_bump_epoch() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GD_VEC_A, "gd.alpha", GD_ID_A, 1);
    add_node(GD_VEC_B, "gd.beta",  GD_ID_B, 2);
    add_edge(GD_ID_A, GD_ID_B, "gd.ab");
    let epoch_before = gos_runtime::graph_epoch();
    let _ = gos_runtime::graph_density();
    let epoch_after = gos_runtime::graph_epoch();
    assert_eq!(epoch_after, epoch_before, "graph_density must not advance graph_epoch");
}
