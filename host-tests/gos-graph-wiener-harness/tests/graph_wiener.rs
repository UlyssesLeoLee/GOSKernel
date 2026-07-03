// gos-graph-wiener-harness — V2.70 graph Wiener index
//
// Verifies `gos_runtime::graph_wiener()` — BFS-based sum of all pairwise
// directed shortest-path distances in the live graph.
//
// Return value: (wiener_index: u64, reachable_pairs: usize, node_count: usize)
//   wiener_index    = Σ_{u≠v, d(u,v)<∞} d(u,v)   (directed BFS, unweighted)
//   reachable_pairs = count of ordered pairs (u,v) with u≠v and d(u,v)<∞
//   node_count      = live nodes in the graph
//
// Test matrix:
//  1.  Empty graph                            → (0, 0, 0)
//  2.  Single node, no edges                 → (0, 0, 1)
//  3.  Two isolated nodes, no edges          → (0, 0, 2)
//  4.  A→B (single directed edge)            → (1, 1, 2)
//  5.  A→B→C (directed chain)               → (4, 3, 3)
//  6.  Triangle A→B→C→A                     → (9, 6, 3)
//  7.  Complete directed K3 (all 6 edges)    → (6, 6, 3)
//  8.  Single node with self-loop A→A        → (0, 0, 1)
//  9.  Disconnected: A→B plus isolated C    → (1, 1, 3)
// 10.  Mutual pair A↔B plus chain C→D       → (3, 3, 4)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const GW_PLUGIN: PluginId   = PluginId::from_ascii("KL_WIENR_0000");
const GW_EXEC:   ExecutorId = ExecutorId::from_ascii("wiener.exec00");

const GW_KEY_A: &str = "gw.alpha";
const GW_KEY_B: &str = "gw.beta";
const GW_KEY_C: &str = "gw.gamma";
const GW_KEY_D: &str = "gw.delta";

const GW_ID_A: NodeId = derive_node_id(GW_PLUGIN, GW_KEY_A);
const GW_ID_B: NodeId = derive_node_id(GW_PLUGIN, GW_KEY_B);
const GW_ID_C: NodeId = derive_node_id(GW_PLUGIN, GW_KEY_C);
const GW_ID_D: NodeId = derive_node_id(GW_PLUGIN, GW_KEY_D);

const GW_VEC_A: VectorAddress = VectorAddress::new(46, 1, 1, 0);
const GW_VEC_B: VectorAddress = VectorAddress::new(46, 1, 2, 0);
const GW_VEC_C: VectorAddress = VectorAddress::new(46, 1, 3, 0);
const GW_VEC_D: VectorAddress = VectorAddress::new(46, 1, 4, 0);

const GW_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    GW_PLUGIN,
    name:         "kl-graph-wiener-harness",
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
        executor_id:       GW_EXEC,
        state_schema_hash: 0xB870,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(GW_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(GW_PLUGIN, vec, node_spec(key, id)).unwrap();
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

// ── 1. Empty graph ─────────────────────────────────────────────────────────────

#[test]
fn empty_graph_wiener_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     0, "empty: W=0");
    assert_eq!(pairs, 0, "empty: 0 reachable pairs");
    assert_eq!(nodes, 0, "empty: 0 nodes");
}

// ── 2. Single node, no edges ──────────────────────────────────────────────────

#[test]
fn single_node_no_edges_wiener_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     0, "single node: W=0 (no pairs)");
    assert_eq!(pairs, 0, "single node: 0 reachable pairs");
    assert_eq!(nodes, 1, "1 live node");
}

// ── 3. Two isolated nodes, no edges ──────────────────────────────────────────

#[test]
fn two_isolated_nodes_wiener_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_node(GW_VEC_B, GW_KEY_B, GW_ID_B);

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     0, "no edges: W=0");
    assert_eq!(pairs, 0, "no directed paths: 0 reachable pairs");
    assert_eq!(nodes, 2, "2 live nodes");
}

// ── 4. A→B (single directed edge) ────────────────────────────────────────────
//
// d(A,B)=1; d(B,A)=∞ (no reverse edge).
// W=1, reachable_pairs=1.

#[test]
fn single_edge_wiener_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_node(GW_VEC_B, GW_KEY_B, GW_ID_B);
    add_edge(GW_ID_A, GW_ID_B, "gw.ab.t4");

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     1, "A\u{2192}B: W=1");
    assert_eq!(pairs, 1, "only (A,B) reachable");
    assert_eq!(nodes, 2, "2 live nodes");
}

// ── 5. A→B→C (directed chain) ────────────────────────────────────────────────
//
// d(A,B)=1, d(A,C)=2, d(B,C)=1; no reverse paths.
// W = 1+2+1 = 4, reachable_pairs = 3.

#[test]
fn chain_abc_wiener_four() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_node(GW_VEC_B, GW_KEY_B, GW_ID_B);
    add_node(GW_VEC_C, GW_KEY_C, GW_ID_C);
    add_edge(GW_ID_A, GW_ID_B, "gw.ab.t5");
    add_edge(GW_ID_B, GW_ID_C, "gw.bc.t5");

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    // d(A,B)=1, d(A,C)=2, d(B,C)=1 → W=4
    assert_eq!(w,     4, "A\u{2192}B\u{2192}C: W=4");
    assert_eq!(pairs, 3, "3 reachable ordered pairs");
    assert_eq!(nodes, 3, "3 live nodes");
}

// ── 6. Triangle A→B→C→A ──────────────────────────────────────────────────────
//
// All nodes mutually reachable via the cycle:
//   d(A,B)=1, d(A,C)=2, d(B,C)=1, d(B,A)=2, d(C,A)=1, d(C,B)=2
// W = 1+2+1+2+1+2 = 9, reachable_pairs = 6.

#[test]
fn triangle_wiener_nine() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_node(GW_VEC_B, GW_KEY_B, GW_ID_B);
    add_node(GW_VEC_C, GW_KEY_C, GW_ID_C);
    add_edge(GW_ID_A, GW_ID_B, "gw.ab.t6");
    add_edge(GW_ID_B, GW_ID_C, "gw.bc.t6");
    add_edge(GW_ID_C, GW_ID_A, "gw.ca.t6");

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     9, "triangle A\u{2192}B\u{2192}C\u{2192}A: W=9");
    assert_eq!(pairs, 6, "all 6 ordered pairs reachable in 3-node cycle");
    assert_eq!(nodes, 3, "3 live nodes");
}

// ── 7. Complete directed K3 (all 6 directed edges A↔B, A↔C, B↔C) ────────────
//
// Every pair has d=1 in both directions.
// W = 6×1 = 6, reachable_pairs = 6.

#[test]
fn complete_k3_wiener_six() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_node(GW_VEC_B, GW_KEY_B, GW_ID_B);
    add_node(GW_VEC_C, GW_KEY_C, GW_ID_C);
    add_edge(GW_ID_A, GW_ID_B, "gw.ab.t7");
    add_edge(GW_ID_B, GW_ID_A, "gw.ba.t7");
    add_edge(GW_ID_A, GW_ID_C, "gw.ac.t7");
    add_edge(GW_ID_C, GW_ID_A, "gw.ca.t7");
    add_edge(GW_ID_B, GW_ID_C, "gw.bc.t7");
    add_edge(GW_ID_C, GW_ID_B, "gw.cb.t7");

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     6, "K3 complete: all distances = 1, W=6");
    assert_eq!(pairs, 6, "all 6 ordered pairs distance=1");
    assert_eq!(nodes, 3, "3 live nodes");
}

// ── 8. Single node with self-loop A→A ────────────────────────────────────────
//
// Self-loops do not create pairwise distances (u≠v required).
// W=0, reachable_pairs=0.

#[test]
fn self_loop_only_wiener_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_edge(GW_ID_A, GW_ID_A, "gw.aa.t8");

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     0, "self-loop: no u\u{2260}v pairs, W=0");
    assert_eq!(pairs, 0, "self-loop: 0 reachable pairs");
    assert_eq!(nodes, 1, "1 live node");
}

// ── 9. Disconnected: A→B plus isolated C ─────────────────────────────────────
//
// d(A,B)=1; C is unreachable from/to A and B.
// W=1, reachable_pairs=1, node_count=3.

#[test]
fn disconnected_ab_plus_c_wiener_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_node(GW_VEC_B, GW_KEY_B, GW_ID_B);
    add_node(GW_VEC_C, GW_KEY_C, GW_ID_C);
    add_edge(GW_ID_A, GW_ID_B, "gw.ab.t9");

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     1, "A\u{2192}B plus isolated C: W=1");
    assert_eq!(pairs, 1, "only (A,B) reachable");
    assert_eq!(nodes, 3, "3 live nodes");
}

// ── 10. Mutual pair A↔B plus chain C→D ───────────────────────────────────────
//
// Component 1: A↔B → d(A,B)=1, d(B,A)=1
// Component 2: C→D → d(C,D)=1
// W = 1+1+1 = 3, reachable_pairs = 3.

#[test]
fn mutual_pair_plus_chain_wiener_three() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(GW_VEC_A, GW_KEY_A, GW_ID_A);
    add_node(GW_VEC_B, GW_KEY_B, GW_ID_B);
    add_node(GW_VEC_C, GW_KEY_C, GW_ID_C);
    add_node(GW_VEC_D, GW_KEY_D, GW_ID_D);
    // Mutual pair
    add_edge(GW_ID_A, GW_ID_B, "gw.ab.t10");
    add_edge(GW_ID_B, GW_ID_A, "gw.ba.t10");
    // Chain
    add_edge(GW_ID_C, GW_ID_D, "gw.cd.t10");

    let (w, pairs, nodes) = gos_runtime::graph_wiener();
    assert_eq!(w,     3, "A\u{2194}B + C\u{2192}D: W=3");
    assert_eq!(pairs, 3, "3 reachable ordered pairs");
    assert_eq!(nodes, 4, "4 live nodes");
}
