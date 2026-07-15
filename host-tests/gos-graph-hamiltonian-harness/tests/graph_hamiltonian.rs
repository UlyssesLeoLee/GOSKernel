// gos-graph-hamiltonian-harness — V3.03 Hamiltonian path/circuit tests
//
// Verifies `gos_runtime::graph_hamiltonian` — directed Hamiltonian path/circuit
// detection via iterative backtracking DFS with dead-end pruning.
//
// A Hamiltonian PATH visits every vertex exactly once.
// A Hamiltonian CIRCUIT visits every vertex exactly once and returns to the start.
// Contrast with Eulerian (V2.87) which visits every EDGE once.
//
// Key invariants tested:
//   has_circuit ⇒ has_path (circuit is a special case of path)
//   path_len == node_count when has_path; 0 when !has_path
//   Directed: A→B does NOT imply B→A
//   Single node: trivially has_circuit (degenerate case)
//
//  1. Empty graph                              → no path, no circuit.
//  2. Single node                              → trivially has_circuit.
//  3. Two nodes, no edges                      → no path.
//  4. A→B (directed, one way)                  → has_path, no circuit.
//  5. A↔B (both directions)                    → has_circuit.
//  6. Directed path A→B→C                      → has_path, no circuit.
//  7. Directed triangle A→B→C→A               → has_circuit.
//  8. K4 complete directed graph               → has_circuit.
//  9. Diamond: A→B, A→C, B→D, C→D            → has_path (A→B→D impossible but A→B→D? check)
// 10. Disconnected: two isolated pairs          → no path (no global Ham path).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const HM_PLUGIN: PluginId   = PluginId::from_ascii("KL_HAMILT_H____");
const HM_EXEC:   ExecutorId = ExecutorId::from_ascii("hm.exec");

const HM_KEY_A: &str = "hm.a";
const HM_KEY_B: &str = "hm.b";
const HM_KEY_C: &str = "hm.c";
const HM_KEY_D: &str = "hm.d";

const HM_ID_A: NodeId = derive_node_id(HM_PLUGIN, HM_KEY_A);
const HM_ID_B: NodeId = derive_node_id(HM_PLUGIN, HM_KEY_B);
const HM_ID_C: NodeId = derive_node_id(HM_PLUGIN, HM_KEY_C);
const HM_ID_D: NodeId = derive_node_id(HM_PLUGIN, HM_KEY_D);

// L4=79 namespace for this harness.
const HM_VEC_A: VectorAddress = VectorAddress::new(79, 1, 1, 0);
const HM_VEC_B: VectorAddress = VectorAddress::new(79, 1, 2, 0);
const HM_VEC_C: VectorAddress = VectorAddress::new(79, 1, 3, 0);
const HM_VEC_D: VectorAddress = VectorAddress::new(79, 1, 4, 0);

const HM_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    HM_PLUGIN,
    name:         "kl-graph-hamiltonian-harness",
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
        executor_id:       HM_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(HM_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(HM_MANIFEST).unwrap();
    g
}

// ── Test 1: empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();

    let (_, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count,  0, "empty: node_count=0");
    assert_eq!(path_len,    0, "empty: path_len=0");
    assert!(!has_path,        "empty: no path");
    assert!(!has_circuit,     "empty: no circuit");
}

// ── Test 2: single node ───────────────────────────────────────────────────────
// A single node trivially satisfies the Hamiltonian condition.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);

    let (vecs, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 1, "single: node_count=1");
    assert_eq!(path_len,   1, "single: path_len=1");
    assert!(has_path,         "single: trivially has_path");
    assert!(has_circuit,      "single: trivially has_circuit");
    assert_eq!(vecs[0], HM_VEC_A, "single: path is [A]");
}

// ── Test 3: two nodes, no edges ───────────────────────────────────────────────
// No directed edges → no path possible.

#[test]
fn test_03_two_nodes_no_edges() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);

    let (_, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 2, "no-edge: node_count=2");
    assert_eq!(path_len,   0, "no-edge: path_len=0");
    assert!(!has_path,        "no-edge: no Ham path");
    assert!(!has_circuit,     "no-edge: no Ham circuit");
}

// ── Test 4: A→B (directed, one direction only) ───────────────────────────────
// Has a Hamiltonian path A→B, but no circuit (no edge B→A).

#[test]
fn test_04_directed_ab_one_way() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);
    add_edge(HM_ID_A, HM_ID_B, "ab");

    let (vecs, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 2, "one-way: node_count=2");
    assert_eq!(path_len,   2, "one-way: path_len=2 (Ham path found)");
    assert!(has_path,         "one-way: has_path (A→B visits both nodes)");
    assert!(!has_circuit,     "one-way: no circuit (B has no edge back to A)");
    // The path must contain both A and B
    let path_vecs: std::collections::HashSet<_> = vecs[0..2].iter().copied().collect();
    assert!(path_vecs.contains(&HM_VEC_A), "one-way: A in path");
    assert!(path_vecs.contains(&HM_VEC_B), "one-way: B in path");
}

// ── Test 5: A↔B (bidirectional) ──────────────────────────────────────────────
// Both A→B and B→A exist → Hamiltonian circuit A→B→A.

#[test]
fn test_05_bidirectional_ab() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);
    add_edge(HM_ID_A, HM_ID_B, "ab");
    add_edge(HM_ID_B, HM_ID_A, "ba");

    let (_, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 2, "bidir: node_count=2");
    assert_eq!(path_len,   2, "bidir: path_len=2");
    assert!(has_path,         "bidir: has_path");
    assert!(has_circuit,      "bidir: has_circuit (A→B→A)");
}

// ── Test 6: directed path A→B→C ──────────────────────────────────────────────
// Linear chain: Hamiltonian path A→B→C exists; no circuit (C has no edge to A or B).

#[test]
fn test_06_directed_path_abc() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);
    add_node(HM_VEC_C, HM_KEY_C, HM_ID_C);
    add_edge(HM_ID_A, HM_ID_B, "ab");
    add_edge(HM_ID_B, HM_ID_C, "bc");

    let (vecs, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 3, "path3: node_count=3");
    assert_eq!(path_len,   3, "path3: path_len=3");
    assert!(has_path,         "path3: has_path (A→B→C)");
    assert!(!has_circuit,     "path3: no circuit");
    // All three nodes must appear in path
    let path_vecs: std::collections::HashSet<_> = vecs[0..3].iter().copied().collect();
    assert!(path_vecs.contains(&HM_VEC_A), "path3: A in path");
    assert!(path_vecs.contains(&HM_VEC_B), "path3: B in path");
    assert!(path_vecs.contains(&HM_VEC_C), "path3: C in path");
}

// ── Test 7: directed triangle A→B→C→A ────────────────────────────────────────
// Directed 3-cycle: Hamiltonian circuit.

#[test]
fn test_07_directed_triangle() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);
    add_node(HM_VEC_C, HM_KEY_C, HM_ID_C);
    add_edge(HM_ID_A, HM_ID_B, "ab");
    add_edge(HM_ID_B, HM_ID_C, "bc");
    add_edge(HM_ID_C, HM_ID_A, "ca");

    let (_, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 3, "triangle: node_count=3");
    assert_eq!(path_len,   3, "triangle: path_len=3");
    assert!(has_path,         "triangle: has_path");
    assert!(has_circuit,      "triangle: has_circuit (A→B→C→A)");
}

// ── Test 8: K4 complete directed graph ───────────────────────────────────────
// All 12 directed edges among 4 nodes → Hamiltonian circuit exists.

#[test]
fn test_08_k4_complete_directed() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);
    add_node(HM_VEC_C, HM_KEY_C, HM_ID_C);
    add_node(HM_VEC_D, HM_KEY_D, HM_ID_D);
    add_edge(HM_ID_A, HM_ID_B, "ab"); add_edge(HM_ID_B, HM_ID_A, "ba");
    add_edge(HM_ID_A, HM_ID_C, "ac"); add_edge(HM_ID_C, HM_ID_A, "ca");
    add_edge(HM_ID_A, HM_ID_D, "ad"); add_edge(HM_ID_D, HM_ID_A, "da");
    add_edge(HM_ID_B, HM_ID_C, "bc"); add_edge(HM_ID_C, HM_ID_B, "cb");
    add_edge(HM_ID_B, HM_ID_D, "bd"); add_edge(HM_ID_D, HM_ID_B, "db");
    add_edge(HM_ID_C, HM_ID_D, "cd"); add_edge(HM_ID_D, HM_ID_C, "dc");

    let (_, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 4, "K4: node_count=4");
    assert_eq!(path_len,   4, "K4: path_len=4");
    assert!(has_path,         "K4: has_path");
    assert!(has_circuit,      "K4: has_circuit (complete directed graph)");
}

// ── Test 9: diamond graph A→B, A→C, B→D, C→D ────────────────────────────────
// Diamond (fork-join): A→B→D and A→C→D are paths but each skips one node.
// A Hamiltonian path requires visiting A, B, C, D all in sequence.
// Possible: A→B→? → C doesn't follow B, and A→C→? → B doesn't follow C.
// With only these 4 edges: from B you can only go to D; from C you can only go to D.
// So after A, you pick B or C. If B: B→D skips C. If C: C→D skips B.
// Therefore NO Hamiltonian path in this pure diamond.

#[test]
fn test_09_diamond_no_ham_path() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);
    add_node(HM_VEC_C, HM_KEY_C, HM_ID_C);
    add_node(HM_VEC_D, HM_KEY_D, HM_ID_D);
    add_edge(HM_ID_A, HM_ID_B, "ab"); // A forks to B
    add_edge(HM_ID_A, HM_ID_C, "ac"); // A forks to C
    add_edge(HM_ID_B, HM_ID_D, "bd"); // B joins at D
    add_edge(HM_ID_C, HM_ID_D, "cd"); // C joins at D

    let (_, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 4, "diamond: node_count=4");
    // In this pure diamond, no Ham path visits A,B,C,D all in one chain.
    assert_eq!(path_len,   0, "diamond: no Ham path (fork-join skips a branch)");
    assert!(!has_path,        "diamond: !has_path");
    assert!(!has_circuit,     "diamond: !has_circuit");
}

// ── Test 10: disconnected graph — no Ham path possible ───────────────────────
// Two isolated pairs A↔B and C↔D: each pair has a Ham circuit internally,
// but no Ham path spanning all 4 nodes (disconnected).

#[test]
fn test_10_disconnected_no_ham_path() {
    let _g = setup();
    add_node(HM_VEC_A, HM_KEY_A, HM_ID_A);
    add_node(HM_VEC_B, HM_KEY_B, HM_ID_B);
    add_node(HM_VEC_C, HM_KEY_C, HM_ID_C);
    add_node(HM_VEC_D, HM_KEY_D, HM_ID_D);
    // Pair 1: A↔B
    add_edge(HM_ID_A, HM_ID_B, "ab");
    add_edge(HM_ID_B, HM_ID_A, "ba");
    // Pair 2: C↔D (isolated from A,B)
    add_edge(HM_ID_C, HM_ID_D, "cd");
    add_edge(HM_ID_D, HM_ID_C, "dc");

    let (_, path_len, has_circuit, has_path, node_count) =
        gos_runtime::graph_hamiltonian::<128>();

    assert_eq!(node_count, 4, "disconnected: node_count=4");
    assert_eq!(path_len,   0, "disconnected: no global Ham path");
    assert!(!has_path,        "disconnected: !has_path");
    assert!(!has_circuit,     "disconnected: !has_circuit");
}
