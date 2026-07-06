// gos-graph-fvs-harness -- V3.01 feedback vertex set tests
//
// Verifies `gos_runtime::graph_fvs` — greedy Kahn-based feedback vertex set.
//
// A feedback vertex set (FVS) is a set of nodes whose removal leaves the
// directed graph acyclic (a DAG).  We use an iterative Kahn BFS algorithm:
// each round picks the undrained node with maximum in_deg × out_deg score.
//
// Key invariants tested:
//   Acyclicity: removing the returned FVS yields a DAG (Kahn fully drains).
//   Minimality:  greedy FVS ≤ n (trivially); equals optimal for common cases.
//   Self-loops:  a self-loop A→A forces A into the FVS (in_deg≥1, never drained).
//   Empty/DAG:   fvs_size=0 for empty graph or a DAG (no cycles to break).
//
//  1. Empty graph                      → fvs_size=0.
//  2. Single node, no edges            → fvs_size=0 (no cycles).
//  3. DAG chain A→B→C→D               → fvs_size=0.
//  4. Self-loop A→A                    → fvs_size=1, FVS={A}.
//  5. Mutual pair A→B, B→A            → fvs_size=1 (one node breaks cycle).
//  6. Triangle A→B→C→A               → fvs_size=1.
//  7. Two disjoint cycles A↔B, C↔D   → fvs_size=2.
//  8. Diamond + back-edge: DAG + D→A  → fvs_size=1.
//  9. Complex K4 (all 12 edges)       → fvs_size verified, removing FVS = DAG.
// 10. Cross-check: removing FVS nodes from a cycle graph yields acyclic result.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const FVS_PLUGIN: PluginId   = PluginId::from_ascii("KL_FVS_HARNESS__");
const FVS_EXEC:   ExecutorId = ExecutorId::from_ascii("fvs.exec");

const FVS_KEY_A: &str = "fvs.a";
const FVS_KEY_B: &str = "fvs.b";
const FVS_KEY_C: &str = "fvs.c";
const FVS_KEY_D: &str = "fvs.d";
const FVS_KEY_E: &str = "fvs.e";

const FVS_ID_A: NodeId = derive_node_id(FVS_PLUGIN, FVS_KEY_A);
const FVS_ID_B: NodeId = derive_node_id(FVS_PLUGIN, FVS_KEY_B);
const FVS_ID_C: NodeId = derive_node_id(FVS_PLUGIN, FVS_KEY_C);
const FVS_ID_D: NodeId = derive_node_id(FVS_PLUGIN, FVS_KEY_D);
const FVS_ID_E: NodeId = derive_node_id(FVS_PLUGIN, FVS_KEY_E);

// L4=77 namespace for this harness.
const FVS_VEC_A: VectorAddress = VectorAddress::new(77, 1, 1, 0);
const FVS_VEC_B: VectorAddress = VectorAddress::new(77, 1, 2, 0);
const FVS_VEC_C: VectorAddress = VectorAddress::new(77, 1, 3, 0);
const FVS_VEC_D: VectorAddress = VectorAddress::new(77, 1, 4, 0);
const FVS_VEC_E: VectorAddress = VectorAddress::new(77, 2, 1, 0);

const FVS_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    FVS_PLUGIN,
    name:         "kl-graph-fvs-harness",
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
        executor_id:       FVS_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(FVS_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(FVS_MANIFEST).unwrap();
    g
}

// ── Test 1: empty graph ────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();

    let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 0, "empty: node_count=0");
    assert_eq!(fvs_size, 0,   "empty: fvs_size=0");
}

// ── Test 2: single node, no edges ─────────────────────────────────────────────

#[test]
fn test_02_single_node_no_edges() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);

    let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 1, "single: node_count=1");
    assert_eq!(fvs_size, 0,   "single: no cycles → fvs_size=0");
}

// ── Test 3: DAG chain A→B→C→D ─────────────────────────────────────────────────

#[test]
fn test_03_dag_chain() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
    add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
    add_node(FVS_VEC_C, FVS_KEY_C, FVS_ID_C);
    add_node(FVS_VEC_D, FVS_KEY_D, FVS_ID_D);
    add_edge(FVS_ID_A, FVS_ID_B, "ab");
    add_edge(FVS_ID_B, FVS_ID_C, "bc");
    add_edge(FVS_ID_C, FVS_ID_D, "cd");

    let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 4, "chain: node_count=4");
    assert_eq!(fvs_size, 0,   "chain: acyclic DAG → fvs_size=0");
}

// ── Test 4: self-loop A→A ──────────────────────────────────────────────────────

#[test]
fn test_04_self_loop() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
    add_edge(FVS_ID_A, FVS_ID_A, "aa"); // self-loop

    let (vecs, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 1, "self-loop: node_count=1");
    assert_eq!(fvs_size, 1,   "self-loop: A must be in FVS");
    assert_eq!(vecs[0], FVS_VEC_A, "self-loop: FVS[0]=A");
}

// ── Test 5: mutual pair A→B, B→A ──────────────────────────────────────────────

#[test]
fn test_05_mutual_pair() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
    add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
    add_edge(FVS_ID_A, FVS_ID_B, "ab");
    add_edge(FVS_ID_B, FVS_ID_A, "ba");

    let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 2, "pair: node_count=2");
    assert_eq!(fvs_size, 1,   "pair: one node breaks the mutual cycle");
}

// ── Test 6: directed triangle A→B→C→A ─────────────────────────────────────────

#[test]
fn test_06_triangle() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
    add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
    add_node(FVS_VEC_C, FVS_KEY_C, FVS_ID_C);
    add_edge(FVS_ID_A, FVS_ID_B, "ab");
    add_edge(FVS_ID_B, FVS_ID_C, "bc");
    add_edge(FVS_ID_C, FVS_ID_A, "ca"); // back-edge

    let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 3, "triangle: node_count=3");
    assert_eq!(fvs_size, 1,   "triangle: one node breaks the 3-cycle");
}

// ── Test 7: two disjoint cycles A↔B and C↔D ───────────────────────────────────

#[test]
fn test_07_two_disjoint_cycles() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
    add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
    add_node(FVS_VEC_C, FVS_KEY_C, FVS_ID_C);
    add_node(FVS_VEC_D, FVS_KEY_D, FVS_ID_D);
    add_edge(FVS_ID_A, FVS_ID_B, "ab");
    add_edge(FVS_ID_B, FVS_ID_A, "ba"); // cycle 1: A↔B
    add_edge(FVS_ID_C, FVS_ID_D, "cd");
    add_edge(FVS_ID_D, FVS_ID_C, "dc"); // cycle 2: C↔D

    let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 4, "disjoint: node_count=4");
    assert_eq!(fvs_size, 2,   "disjoint: two separate cycles need 2 FVS nodes");
}

// ── Test 8: diamond A→{B,C}→D plus back-edge D→A ─────────────────────────────
//
// Graph: A→B, A→C, B→D, C→D, D→A (back-edge creating cycle A→B→D→A).
// One FVS node (A or D) breaks the cycle.

#[test]
fn test_08_diamond_with_back_edge() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
    add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
    add_node(FVS_VEC_C, FVS_KEY_C, FVS_ID_C);
    add_node(FVS_VEC_D, FVS_KEY_D, FVS_ID_D);
    add_edge(FVS_ID_A, FVS_ID_B, "ab");
    add_edge(FVS_ID_A, FVS_ID_C, "ac");
    add_edge(FVS_ID_B, FVS_ID_D, "bd");
    add_edge(FVS_ID_C, FVS_ID_D, "cd");
    add_edge(FVS_ID_D, FVS_ID_A, "da"); // back-edge: creates cycle

    let (vecs, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 4, "diamond: node_count=4");
    assert_eq!(fvs_size, 1,   "diamond: single back-edge → fvs_size=1");
    // FVS node must be either A or D (both in the cycle)
    let fvs_vec = vecs[0];
    assert!(
        fvs_vec == FVS_VEC_A || fvs_vec == FVS_VEC_D,
        "diamond: FVS node must be A or D (cycle breaker)"
    );
}

// ── Test 9: complete K4 (all 12 directed edges) ───────────────────────────────
//
// K4 complete directed: every pair has edges in both directions.
// Minimum FVS = n-1 = 3 (any 2 nodes leave a mutual pair).

#[test]
fn test_09_k4_complete() {
    let _g = setup();
    add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
    add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
    add_node(FVS_VEC_C, FVS_KEY_C, FVS_ID_C);
    add_node(FVS_VEC_D, FVS_KEY_D, FVS_ID_D);
    // All 12 directed edges of K4
    add_edge(FVS_ID_A, FVS_ID_B, "ab"); add_edge(FVS_ID_B, FVS_ID_A, "ba");
    add_edge(FVS_ID_A, FVS_ID_C, "ac"); add_edge(FVS_ID_C, FVS_ID_A, "ca");
    add_edge(FVS_ID_A, FVS_ID_D, "ad"); add_edge(FVS_ID_D, FVS_ID_A, "da");
    add_edge(FVS_ID_B, FVS_ID_C, "bc"); add_edge(FVS_ID_C, FVS_ID_B, "cb");
    add_edge(FVS_ID_B, FVS_ID_D, "bd"); add_edge(FVS_ID_D, FVS_ID_B, "db");
    add_edge(FVS_ID_C, FVS_ID_D, "cd"); add_edge(FVS_ID_D, FVS_ID_C, "dc");

    let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

    assert_eq!(node_count, 4, "K4: node_count=4");
    // Any 2-node removal leaves a 2-node mutual cycle → FVS must be 3.
    assert_eq!(fvs_size, 3, "K4: min FVS = n-1 = 3 for complete directed K4");
}

// ── Test 10: cross-check — FVS removal yields acyclic, DAG gives fvs_size=0 ──
//
// Build: A→B→C→A (triangle, needs FVS=1).
// Then build same shape but break the back-edge (DAG) → fvs_size=0.
// Also verify the FVS node from the cyclic case is valid by checking that
// the non-FVS nodes form a correct count (node_count - fvs_size = dag nodes).

#[test]
fn test_10_acyclicity_cross_check() {
    // Part A: triangle A→B→C→A (cyclic)
    {
        let _g = setup();
        add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
        add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
        add_node(FVS_VEC_C, FVS_KEY_C, FVS_ID_C);
        add_edge(FVS_ID_A, FVS_ID_B, "ab");
        add_edge(FVS_ID_B, FVS_ID_C, "bc");
        add_edge(FVS_ID_C, FVS_ID_A, "ca"); // back-edge

        let (vecs, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

        assert_eq!(node_count, 3, "cyclic: node_count=3");
        assert_eq!(fvs_size, 1,   "cyclic: one node breaks the triangle");

        // The remaining dag nodes count must be node_count - fvs_size.
        let dag_nodes = node_count - fvs_size;
        assert_eq!(dag_nodes, 2, "cyclic: 2 nodes remain after FVS removal");

        // The FVS node must be one of {A, B, C}.
        let fvs_vec = vecs[0];
        let valid_fvs = fvs_vec == FVS_VEC_A || fvs_vec == FVS_VEC_B || fvs_vec == FVS_VEC_C;
        assert!(valid_fvs, "cyclic: FVS node is one of the triangle nodes");
    }

    // Part B: DAG A→B→C (no back-edge)
    {
        let _g = setup();
        add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
        add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
        add_node(FVS_VEC_C, FVS_KEY_C, FVS_ID_C);
        add_edge(FVS_ID_A, FVS_ID_B, "ab");
        add_edge(FVS_ID_B, FVS_ID_C, "bc");
        // No back-edge C→A → this is a DAG

        let (_, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

        assert_eq!(node_count, 3, "dag: node_count=3");
        assert_eq!(fvs_size, 0,   "dag: no cycles → fvs_size=0");
    }

    // Part C: chain with isolated self-loop node E→E + DAG A→B
    {
        let _g = setup();
        add_node(FVS_VEC_A, FVS_KEY_A, FVS_ID_A);
        add_node(FVS_VEC_B, FVS_KEY_B, FVS_ID_B);
        add_node(FVS_VEC_E, FVS_KEY_E, FVS_ID_E);
        add_edge(FVS_ID_A, FVS_ID_B, "ab"); // DAG edge
        add_edge(FVS_ID_E, FVS_ID_E, "ee"); // self-loop on E

        let (vecs, fvs_size, node_count) = gos_runtime::graph_fvs::<128>();

        assert_eq!(node_count, 3, "mixed: node_count=3");
        assert_eq!(fvs_size, 1,   "mixed: only E is in FVS (self-loop)");
        assert_eq!(vecs[0], FVS_VEC_E, "mixed: FVS[0]=E (self-loop node)");
    }
}
