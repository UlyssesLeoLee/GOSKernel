// gos-graph-girth-harness — V2.69 graph girth (shortest directed cycle)
//
// Verifies `gos_runtime::graph_girth()` — BFS-based detection of the shortest
// directed cycle in the live graph.
//
// Return value: (girth: u32, is_acyclic: bool, node_count: usize)
//   girth = 1       → self-loop
//   girth = 2       → mutual pair A↔B
//   girth = k       → shortest directed cycle uses k edges
//   girth = u32::MAX → no directed cycle (DAG); is_acyclic = true
//
// Test matrix:
//  1.  Empty graph                             → (u32::MAX, true,  0)
//  2.  Three isolated nodes, no edges          → (u32::MAX, true,  3)
//  3.  Linear chain A→B→C (DAG)               → (u32::MAX, true,  3)
//  4.  Single self-loop A→A                   → (1,        false, 1)
//  5.  Chain A→B→C plus self-loop A→A         → (1,        false, 3)
//  6.  Mutual pair A↔B                        → (2,        false, 2)
//  7.  Triangle A→B→C→A                       → (3,        false, 3)
//  8.  Triangle A→B→C→A plus extra node D→A   → (3,        false, 4)
//  9.  C4 unidirectional: A→B→C→D→A           → (4,        false, 4)
// 10.  Disjoint triangle + mutual pair D↔E    → (2,        false, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const GT_PLUGIN: PluginId   = PluginId::from_ascii("KL_GIRTH_0000");
const GT_EXEC:   ExecutorId = ExecutorId::from_ascii("girth.exec000");

const GT_KEY_A: &str = "gt.alpha";
const GT_KEY_B: &str = "gt.beta";
const GT_KEY_C: &str = "gt.gamma";
const GT_KEY_D: &str = "gt.delta";
const GT_KEY_E: &str = "gt.epsilon";

const GT_ID_A: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_A);
const GT_ID_B: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_B);
const GT_ID_C: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_C);
const GT_ID_D: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_D);
const GT_ID_E: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_E);

const GT_VEC_A: VectorAddress = VectorAddress::new(45, 1, 1, 0);
const GT_VEC_B: VectorAddress = VectorAddress::new(45, 1, 2, 0);
const GT_VEC_C: VectorAddress = VectorAddress::new(45, 1, 3, 0);
const GT_VEC_D: VectorAddress = VectorAddress::new(45, 1, 4, 0);
const GT_VEC_E: VectorAddress = VectorAddress::new(45, 1, 5, 0);

const GT_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    GT_PLUGIN,
    name:         "kl-graph-girth-harness",
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
        executor_id:       GT_EXEC,
        state_schema_hash: 0xB869,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(GT_PLUGIN, vec, node_spec(key, id)).unwrap();
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

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_is_acyclic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      u32::MAX, "empty: no cycle possible");
    assert!(is_acyclic,              "empty: must be acyclic");
    assert_eq!(node_count, 0,        "empty: 0 nodes");
}

// ── 2. Three isolated nodes, no edges ────────────────────────────────────────

#[test]
fn three_isolated_nodes_no_edges_acyclic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_node(GT_VEC_C, GT_KEY_C, GT_ID_C);

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      u32::MAX, "isolated nodes: no edges, no cycle");
    assert!(is_acyclic,              "3 isolated nodes: acyclic");
    assert_eq!(node_count, 3,        "3 live nodes");
}

// ── 3. Linear chain A→B→C (DAG) ──────────────────────────────────────────────

#[test]
fn linear_chain_abc_dag_acyclic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_node(GT_VEC_C, GT_KEY_C, GT_ID_C);
    add_edge(GT_ID_A, GT_ID_B, "gt.ab.t3");
    add_edge(GT_ID_B, GT_ID_C, "gt.bc.t3");

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      u32::MAX, "linear chain: DAG, no directed cycle");
    assert!(is_acyclic,              "A\u{2192}B\u{2192}C is acyclic");
    assert_eq!(node_count, 3,        "3 live nodes");
}

// ── 4. Single self-loop A→A ───────────────────────────────────────────────────

#[test]
fn self_loop_girth_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_edge(GT_ID_A, GT_ID_A, "gt.aa.t4");

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      1,     "self-loop: girth = 1");
    assert!(!is_acyclic,          "self-loop: not acyclic");
    assert_eq!(node_count, 1,     "1 live node");
}

// ── 5. Chain A→B→C plus self-loop A→A ────────────────────────────────────────
//
// The self-loop dominates: girth = 1 even though the chain contributes no cycle.

#[test]
fn chain_with_self_loop_girth_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_node(GT_VEC_C, GT_KEY_C, GT_ID_C);
    add_edge(GT_ID_A, GT_ID_B, "gt.ab.t5");
    add_edge(GT_ID_B, GT_ID_C, "gt.bc.t5");
    add_edge(GT_ID_A, GT_ID_A, "gt.aa.t5"); // self-loop

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      1,  "self-loop present: girth = 1");
    assert!(!is_acyclic,       "not acyclic");
    assert_eq!(node_count, 3,  "3 live nodes");
}

// ── 6. Mutual pair A↔B ────────────────────────────────────────────────────────
//
// A→B and B→A form a directed 2-cycle: girth = 2.

#[test]
fn mutual_pair_girth_two() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_edge(GT_ID_A, GT_ID_B, "gt.ab.t6");
    add_edge(GT_ID_B, GT_ID_A, "gt.ba.t6");

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      2,  "mutual pair A\u{2194}B: girth = 2");
    assert!(!is_acyclic,       "not acyclic");
    assert_eq!(node_count, 2,  "2 live nodes");
}

// ── 7. Triangle A→B→C→A ──────────────────────────────────────────────────────
//
// Shortest directed cycle is length 3 (the triangle itself).

#[test]
fn triangle_girth_three() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_node(GT_VEC_C, GT_KEY_C, GT_ID_C);
    add_edge(GT_ID_A, GT_ID_B, "gt.ab.t7");
    add_edge(GT_ID_B, GT_ID_C, "gt.bc.t7");
    add_edge(GT_ID_C, GT_ID_A, "gt.ca.t7");

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      3,  "A\u{2192}B\u{2192}C\u{2192}A: girth = 3");
    assert!(!is_acyclic,       "not acyclic");
    assert_eq!(node_count, 3,  "3 live nodes");
}

// ── 8. Triangle A→B→C→A plus extra node D→A ─────────────────────────────────
//
// D contributes only an incoming edge to A; the triangle's girth = 3 is unchanged.
// D→A does not form a cycle (there is no path A→...→D).

#[test]
fn triangle_plus_extra_node_girth_three() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_node(GT_VEC_C, GT_KEY_C, GT_ID_C);
    add_node(GT_VEC_D, GT_KEY_D, GT_ID_D);
    add_edge(GT_ID_A, GT_ID_B, "gt.ab.t8");
    add_edge(GT_ID_B, GT_ID_C, "gt.bc.t8");
    add_edge(GT_ID_C, GT_ID_A, "gt.ca.t8");
    add_edge(GT_ID_D, GT_ID_A, "gt.da.t8"); // D feeds into cycle, but no return

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      3,  "triangle still dominates: girth = 3");
    assert!(!is_acyclic,       "not acyclic (triangle present)");
    assert_eq!(node_count, 4,  "4 live nodes");
}

// ── 9. C4 unidirectional cycle A→B→C→D→A ────────────────────────────────────
//
// Only one directed 4-cycle, no shorter cycle.
// Girth = 4.

#[test]
fn c4_cycle_girth_four() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_node(GT_VEC_C, GT_KEY_C, GT_ID_C);
    add_node(GT_VEC_D, GT_KEY_D, GT_ID_D);
    add_edge(GT_ID_A, GT_ID_B, "gt.ab.t9");
    add_edge(GT_ID_B, GT_ID_C, "gt.bc.t9");
    add_edge(GT_ID_C, GT_ID_D, "gt.cd.t9");
    add_edge(GT_ID_D, GT_ID_A, "gt.da.t9");

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      4,  "C4 cycle: girth = 4");
    assert!(!is_acyclic,       "not acyclic");
    assert_eq!(node_count, 4,  "4 live nodes");
}

// ── 10. Disjoint triangle A→B→C→A plus mutual pair D↔E ──────────────────────
//
// Triangle has girth 3; mutual pair D↔E has girth 2.
// Minimum = 2 (from the mutual pair).

#[test]
fn disjoint_triangle_and_mutual_pair_girth_two() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GT_VEC_A, GT_KEY_A, GT_ID_A);
    add_node(GT_VEC_B, GT_KEY_B, GT_ID_B);
    add_node(GT_VEC_C, GT_KEY_C, GT_ID_C);
    add_node(GT_VEC_D, GT_KEY_D, GT_ID_D);
    add_node(GT_VEC_E, GT_KEY_E, GT_ID_E);
    // Triangle component
    add_edge(GT_ID_A, GT_ID_B, "gt.ab.t10");
    add_edge(GT_ID_B, GT_ID_C, "gt.bc.t10");
    add_edge(GT_ID_C, GT_ID_A, "gt.ca.t10");
    // Mutual pair component (girth = 2, shorter than triangle)
    add_edge(GT_ID_D, GT_ID_E, "gt.de.t10");
    add_edge(GT_ID_E, GT_ID_D, "gt.ed.t10");

    let (girth, is_acyclic, node_count) = gos_runtime::graph_girth();
    assert_eq!(girth,      2,  "mutual pair D\u{2194}E gives minimum girth = 2");
    assert!(!is_acyclic,       "not acyclic");
    assert_eq!(node_count, 5,  "5 live nodes");
}
