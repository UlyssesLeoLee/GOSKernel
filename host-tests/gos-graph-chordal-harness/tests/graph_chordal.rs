// gos-graph-chordal-harness — V3.04 chordal graph recognition tests
//
// Verifies `gos_runtime::graph_chordal` — LexBFS perfect elimination ordering
// + PEO verification (Fulkerson & Gross 1965 / Rose, Tarjan & Lueker 1976).
//
// A graph is CHORDAL iff every cycle of length ≥ 4 has a chord (an edge
// between non-adjacent cycle vertices).  Equivalently, it admits a Perfect
// Elimination Ordering (PEO): an ordering v₁,…,vₙ where N⁺(vᵢ) is a clique.
//
// Directed edges are treated as undirected (A→B and B→A = one edge A–B).
// Self-loops are excluded.
//
// Key invariants tested:
//   Complete graphs K_n are always chordal.
//   Trees (paths, stars) are always chordal.
//   Chordless k-cycles C_k (k ≥ 4) are NOT chordal.
//   Adding a chord to C_k can make it chordal.
//
//  1. Empty graph                               → chordal (vacuous).
//  2. Single node                               → chordal.
//  3. K₂ (two connected nodes)                 → chordal.
//  4. K₃ (triangle)                            → chordal (no 4+ cycle).
//  5. C₄ (4-cycle, no chord)                   → NOT chordal.
//  6. C₄ + chord (A–C diagonal)                → chordal (splits into K₃∪K₃).
//  7. K₄ (complete on 4 nodes)                 → chordal.
//  8. C₅ (5-cycle, no chord)                   → NOT chordal.
//  9. Path P₅ (A–B–C–D–E, a tree)              → chordal.
// 10. K₅ (complete on 5 nodes)                 → chordal.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const CH_PLUGIN: PluginId   = PluginId::from_ascii("KL_CHORDAL_H___");
const CH_EXEC:   ExecutorId = ExecutorId::from_ascii("ch.exec");

const CH_KEY_A: &str = "ch.a";
const CH_KEY_B: &str = "ch.b";
const CH_KEY_C: &str = "ch.c";
const CH_KEY_D: &str = "ch.d";
const CH_KEY_E: &str = "ch.e";

const CH_ID_A: NodeId = derive_node_id(CH_PLUGIN, CH_KEY_A);
const CH_ID_B: NodeId = derive_node_id(CH_PLUGIN, CH_KEY_B);
const CH_ID_C: NodeId = derive_node_id(CH_PLUGIN, CH_KEY_C);
const CH_ID_D: NodeId = derive_node_id(CH_PLUGIN, CH_KEY_D);
const CH_ID_E: NodeId = derive_node_id(CH_PLUGIN, CH_KEY_E);

// L4=80 namespace for this harness.
const CH_VEC_A: VectorAddress = VectorAddress::new(80, 1, 1, 0);
const CH_VEC_B: VectorAddress = VectorAddress::new(80, 1, 2, 0);
const CH_VEC_C: VectorAddress = VectorAddress::new(80, 1, 3, 0);
const CH_VEC_D: VectorAddress = VectorAddress::new(80, 1, 4, 0);
const CH_VEC_E: VectorAddress = VectorAddress::new(80, 2, 1, 0);

const CH_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    CH_PLUGIN,
    name:         "kl-graph-chordal-harness",
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

fn node_spec(key: &'static str, id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id:           id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       CH_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(CH_PLUGIN, vec, node_spec(key, id)).unwrap();
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

fn setup() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(CH_MANIFEST).unwrap();
    g
}

// ── Test 1: empty graph ───────────────────────────────────────────────────────
// Vacuously chordal: no cycles of any length.

#[test]
fn test_01_empty_graph() {
    let _g = setup();

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 0,  "empty: node_count=0");
    assert!(is_chordal,         "empty: vacuously chordal");
}

// ── Test 2: single node ───────────────────────────────────────────────────────
// Trivially chordal: no cycles possible.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);

    let (vecs, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 1,    "single: node_count=1");
    assert!(is_chordal,           "single: trivially chordal");
    assert_eq!(vecs[0], CH_VEC_A, "single: PEO[0]=A");
}

// ── Test 3: K₂ (two nodes, bidirectional edge) ───────────────────────────────
// No cycle of length ≥ 4 → chordal.

#[test]
fn test_03_k2_two_nodes() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_edge(CH_ID_A, CH_ID_B, "ab");
    add_edge(CH_ID_B, CH_ID_A, "ba");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 2, "K2: node_count=2");
    assert!(is_chordal,        "K2: chordal (shortest cycle=2, below threshold of 4)");
}

// ── Test 4: K₃ (triangle) ────────────────────────────────────────────────────
// Only 3-cycles exist; no 4+ cycle → chordal.

#[test]
fn test_04_k3_triangle() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_node(CH_VEC_C, CH_KEY_C, CH_ID_C);
    // Full undirected triangle via bidirectional edges.
    add_edge(CH_ID_A, CH_ID_B, "ab"); add_edge(CH_ID_B, CH_ID_A, "ba");
    add_edge(CH_ID_B, CH_ID_C, "bc"); add_edge(CH_ID_C, CH_ID_B, "cb");
    add_edge(CH_ID_C, CH_ID_A, "ca"); add_edge(CH_ID_A, CH_ID_C, "ac");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 3, "K3: node_count=3");
    assert!(is_chordal,        "K3: chordal (only 3-cycles, no 4+ cycle)");
}

// ── Test 5: C₄ (4-cycle, no chord) ───────────────────────────────────────────
// A–B–C–D–A with no diagonal edge → NOT chordal.

#[test]
fn test_05_c4_no_chord() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_node(CH_VEC_C, CH_KEY_C, CH_ID_C);
    add_node(CH_VEC_D, CH_KEY_D, CH_ID_D);
    // 4-cycle: A–B–C–D–A (bidirectional edges = undirected cycle).
    add_edge(CH_ID_A, CH_ID_B, "ab"); add_edge(CH_ID_B, CH_ID_A, "ba");
    add_edge(CH_ID_B, CH_ID_C, "bc"); add_edge(CH_ID_C, CH_ID_B, "cb");
    add_edge(CH_ID_C, CH_ID_D, "cd"); add_edge(CH_ID_D, CH_ID_C, "dc");
    add_edge(CH_ID_D, CH_ID_A, "da"); add_edge(CH_ID_A, CH_ID_D, "ad");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 4, "C4: node_count=4");
    assert!(!is_chordal,       "C4: NOT chordal (chordless 4-cycle A–B–C–D–A)");
}

// ── Test 6: C₄ + chord (A–C diagonal) ───────────────────────────────────────
// Adding edge A–C splits the 4-cycle into two triangles → chordal.

#[test]
fn test_06_c4_with_chord() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_node(CH_VEC_C, CH_KEY_C, CH_ID_C);
    add_node(CH_VEC_D, CH_KEY_D, CH_ID_D);
    // 4-cycle edges.
    add_edge(CH_ID_A, CH_ID_B, "ab"); add_edge(CH_ID_B, CH_ID_A, "ba");
    add_edge(CH_ID_B, CH_ID_C, "bc"); add_edge(CH_ID_C, CH_ID_B, "cb");
    add_edge(CH_ID_C, CH_ID_D, "cd"); add_edge(CH_ID_D, CH_ID_C, "dc");
    add_edge(CH_ID_D, CH_ID_A, "da"); add_edge(CH_ID_A, CH_ID_D, "ad");
    // Chord A–C eliminates the chordless 4-cycle.
    add_edge(CH_ID_A, CH_ID_C, "ac"); add_edge(CH_ID_C, CH_ID_A, "ca");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 4, "C4+chord: node_count=4");
    assert!(is_chordal,        "C4+chord: chordal (A–C splits into triangles A–B–C and A–C–D)");
}

// ── Test 7: K₄ (complete graph on 4 nodes) ───────────────────────────────────
// Every pair is connected → every cycle has a chord → chordal.

#[test]
fn test_07_k4_complete() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_node(CH_VEC_C, CH_KEY_C, CH_ID_C);
    add_node(CH_VEC_D, CH_KEY_D, CH_ID_D);
    // All 12 directed edges (= 6 undirected edges).
    add_edge(CH_ID_A, CH_ID_B, "ab"); add_edge(CH_ID_B, CH_ID_A, "ba");
    add_edge(CH_ID_A, CH_ID_C, "ac"); add_edge(CH_ID_C, CH_ID_A, "ca");
    add_edge(CH_ID_A, CH_ID_D, "ad"); add_edge(CH_ID_D, CH_ID_A, "da");
    add_edge(CH_ID_B, CH_ID_C, "bc"); add_edge(CH_ID_C, CH_ID_B, "cb");
    add_edge(CH_ID_B, CH_ID_D, "bd"); add_edge(CH_ID_D, CH_ID_B, "db");
    add_edge(CH_ID_C, CH_ID_D, "cd"); add_edge(CH_ID_D, CH_ID_C, "dc");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 4, "K4: node_count=4");
    assert!(is_chordal,        "K4: chordal (complete graph; every cycle has a chord)");
}

// ── Test 8: C₅ (5-cycle, no chord) ───────────────────────────────────────────
// A–B–C–D–E–A with no chord → NOT chordal (chordless 5-cycle).

#[test]
fn test_08_c5_no_chord() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_node(CH_VEC_C, CH_KEY_C, CH_ID_C);
    add_node(CH_VEC_D, CH_KEY_D, CH_ID_D);
    add_node(CH_VEC_E, CH_KEY_E, CH_ID_E);
    // 5-cycle: A–B–C–D–E–A.
    add_edge(CH_ID_A, CH_ID_B, "ab"); add_edge(CH_ID_B, CH_ID_A, "ba");
    add_edge(CH_ID_B, CH_ID_C, "bc"); add_edge(CH_ID_C, CH_ID_B, "cb");
    add_edge(CH_ID_C, CH_ID_D, "cd"); add_edge(CH_ID_D, CH_ID_C, "dc");
    add_edge(CH_ID_D, CH_ID_E, "de"); add_edge(CH_ID_E, CH_ID_D, "ed");
    add_edge(CH_ID_E, CH_ID_A, "ea"); add_edge(CH_ID_A, CH_ID_E, "ae");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 5, "C5: node_count=5");
    assert!(!is_chordal,       "C5: NOT chordal (chordless 5-cycle)");
}

// ── Test 9: Path P₅ (A–B–C–D–E linear chain) ────────────────────────────────
// A path is a tree; trees are always chordal (no cycles at all).

#[test]
fn test_09_path_p5() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_node(CH_VEC_C, CH_KEY_C, CH_ID_C);
    add_node(CH_VEC_D, CH_KEY_D, CH_ID_D);
    add_node(CH_VEC_E, CH_KEY_E, CH_ID_E);
    // Undirected path via bidirectional edges.
    add_edge(CH_ID_A, CH_ID_B, "ab"); add_edge(CH_ID_B, CH_ID_A, "ba");
    add_edge(CH_ID_B, CH_ID_C, "bc"); add_edge(CH_ID_C, CH_ID_B, "cb");
    add_edge(CH_ID_C, CH_ID_D, "cd"); add_edge(CH_ID_D, CH_ID_C, "dc");
    add_edge(CH_ID_D, CH_ID_E, "de"); add_edge(CH_ID_E, CH_ID_D, "ed");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 5, "P5: node_count=5");
    assert!(is_chordal,        "P5: chordal (path is a tree; no cycles exist)");
}

// ── Test 10: K₅ (complete on 5 nodes) ───────────────────────────────────────
// Every pair connected → every 4+ cycle has a chord → chordal.
// Also validates node_count=5 for a 5-node complete graph.

#[test]
fn test_10_k5_complete() {
    let _g = setup();
    add_node(CH_VEC_A, CH_KEY_A, CH_ID_A);
    add_node(CH_VEC_B, CH_KEY_B, CH_ID_B);
    add_node(CH_VEC_C, CH_KEY_C, CH_ID_C);
    add_node(CH_VEC_D, CH_KEY_D, CH_ID_D);
    add_node(CH_VEC_E, CH_KEY_E, CH_ID_E);
    // All 20 directed edges (= 10 undirected edges).
    add_edge(CH_ID_A, CH_ID_B, "ab"); add_edge(CH_ID_B, CH_ID_A, "ba");
    add_edge(CH_ID_A, CH_ID_C, "ac"); add_edge(CH_ID_C, CH_ID_A, "ca");
    add_edge(CH_ID_A, CH_ID_D, "ad"); add_edge(CH_ID_D, CH_ID_A, "da");
    add_edge(CH_ID_A, CH_ID_E, "ae"); add_edge(CH_ID_E, CH_ID_A, "ea");
    add_edge(CH_ID_B, CH_ID_C, "bc"); add_edge(CH_ID_C, CH_ID_B, "cb");
    add_edge(CH_ID_B, CH_ID_D, "bd"); add_edge(CH_ID_D, CH_ID_B, "db");
    add_edge(CH_ID_B, CH_ID_E, "be"); add_edge(CH_ID_E, CH_ID_B, "eb");
    add_edge(CH_ID_C, CH_ID_D, "cd"); add_edge(CH_ID_D, CH_ID_C, "dc");
    add_edge(CH_ID_C, CH_ID_E, "ce"); add_edge(CH_ID_E, CH_ID_C, "ec");
    add_edge(CH_ID_D, CH_ID_E, "de"); add_edge(CH_ID_E, CH_ID_D, "ed");

    let (_, is_chordal, node_count) = gos_runtime::graph_chordal::<128>();

    assert_eq!(node_count, 5, "K5: node_count=5");
    assert!(is_chordal,        "K5: chordal (complete graph; every pair connected)");
}
