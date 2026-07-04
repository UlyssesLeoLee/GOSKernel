// gos-graph-dag-longest-harness — V2.88 DAG longest path / critical path tests
//
// Verifies `gos_runtime::graph_dag_longest` — Kahn BFS + distance DP to find
// the longest directed path (critical path) in the graph's DAG projection.
// Analogous to `systemd-analyze critical-chain` for graph-native subsystems.
//
//  1. Empty graph            → is_dag=true, path_hops=0, start/end=zero.
//  2. Single isolated node   → is_dag=true, path_hops=0 (no edges).
//  3. Single self-loop A→A   → is_dag=false (self-loop is a cycle).
//  4. Linear chain A→B→C→D  → is_dag=true, path_hops=3, start=A, end=D.
//  5. Diamond A→B, A→C, B→D, C→D → is_dag=true, path_hops=2, end=D.
//  6. Two independent chains A→B and C→D→E → is_dag=true, path_hops=2.
//  7. Directed 3-cycle A→B→C→A → is_dag=false, path_hops=0.
//  8. DAG with shortcut A→B→C + A→C → is_dag=true, path_hops=2, end=C.
//  9. Star fan-out A→B, A→C, A→D, A→E → is_dag=true, path_hops=1.
// 10. Chain of 5 hops A→B→C→D→E→F → is_dag=true, path_hops=5.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const DL_PLUGIN: PluginId   = PluginId::from_ascii("KL_DAGLONG_H");
const DL_EXEC:   ExecutorId = ExecutorId::from_ascii("dl.exec");

const DL_KEY_A: &str = "dl.alpha";
const DL_KEY_B: &str = "dl.beta";
const DL_KEY_C: &str = "dl.gamma";
const DL_KEY_D: &str = "dl.delta";
const DL_KEY_E: &str = "dl.epsilon";
const DL_KEY_F: &str = "dl.zeta";

const DL_ID_A: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_A);
const DL_ID_B: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_B);
const DL_ID_C: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_C);
const DL_ID_D: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_D);
const DL_ID_E: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_E);
const DL_ID_F: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_F);

// L4=64 namespace for this harness.
const DL_VEC_A: VectorAddress = VectorAddress::new(64, 1, 1, 0);
const DL_VEC_B: VectorAddress = VectorAddress::new(64, 1, 2, 0);
const DL_VEC_C: VectorAddress = VectorAddress::new(64, 1, 3, 0);
const DL_VEC_D: VectorAddress = VectorAddress::new(64, 1, 4, 0);
const DL_VEC_E: VectorAddress = VectorAddress::new(64, 1, 5, 0);
const DL_VEC_F: VectorAddress = VectorAddress::new(64, 1, 6, 0);

const DL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    DL_PLUGIN,
    name:         "kl-graph-dag-longest-harness",
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
        executor_id:       DL_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(DL_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(DL_MANIFEST).unwrap();
    g
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Test 1: empty graph → is_dag=true, path_hops=0.
#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (hops, is_dag, sv, ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag,   "empty graph must be a DAG");
    assert_eq!(hops, 0, "empty graph has no path hops");
    assert_eq!(nc,   0, "node count must be 0");
    let zero = VectorAddress::new(0, 0, 0, 0);
    assert_eq!(sv, zero);
    assert_eq!(ev, zero);
}

/// Test 2: single isolated node → is_dag=true, path_hops=0.
#[test]
fn test_02_single_isolated_node() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);

    let (hops, is_dag, sv, ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag, "single isolated node is a DAG");
    assert_eq!(hops, 0, "no edges → no hops");
    assert_eq!(nc, 1);
    let zero = VectorAddress::new(0, 0, 0, 0);
    assert_eq!(sv, zero);
    assert_eq!(ev, zero);
}

/// Test 3: single self-loop A→A → is_dag=false.
#[test]
fn test_03_self_loop() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_edge(DL_ID_A, DL_ID_A, "aa");

    let (hops, is_dag, _sv, _ev, nc) = gos_runtime::graph_dag_longest();
    assert!(!is_dag, "self-loop must make is_dag=false");
    assert_eq!(hops, 0, "cyclic graph returns path_hops=0");
    assert_eq!(nc, 1);
}

/// Test 4: linear chain A→B→C→D → path_hops=3, start=A, end=D.
#[test]
fn test_04_linear_chain() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_B, DL_ID_C, "bc");
    add_edge(DL_ID_C, DL_ID_D, "cd");

    let (hops, is_dag, sv, ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag, "linear chain is a DAG");
    assert_eq!(hops, 3, "A->B->C->D is 3 hops");
    assert_eq!(nc, 4);
    assert_eq!(sv, DL_VEC_A, "start must be A");
    assert_eq!(ev, DL_VEC_D, "end must be D");
}

/// Test 5: diamond A→B, A→C, B→D, C→D → path_hops=2, end=D, start=A.
#[test]
fn test_05_diamond() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_A, DL_ID_C, "ac");
    add_edge(DL_ID_B, DL_ID_D, "bd");
    add_edge(DL_ID_C, DL_ID_D, "cd");

    let (hops, is_dag, sv, ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag, "diamond is a DAG");
    assert_eq!(hops, 2, "diamond longest path is 2 hops (A->B->D or A->C->D)");
    assert_eq!(nc, 4);
    assert_eq!(ev, DL_VEC_D, "end node must be D (the sink)");
    assert_eq!(sv, DL_VEC_A, "start node must be A (the source)");
}

/// Test 6: two independent chains A→B and C→D→E → path_hops=2.
#[test]
fn test_06_two_independent_chains() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_node(DL_VEC_E, DL_KEY_E, DL_ID_E);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_C, DL_ID_D, "cd");
    add_edge(DL_ID_D, DL_ID_E, "de");

    let (hops, is_dag, _sv, ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag, "two chains is a DAG");
    assert_eq!(hops, 2, "longest chain is C->D->E = 2 hops");
    assert_eq!(nc, 5);
    assert_eq!(ev, DL_VEC_E, "longest chain ends at E");
}

/// Test 7: directed 3-cycle A→B→C→A → is_dag=false.
#[test]
fn test_07_directed_cycle() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_B, DL_ID_C, "bc");
    add_edge(DL_ID_C, DL_ID_A, "ca");

    let (hops, is_dag, _sv, _ev, nc) = gos_runtime::graph_dag_longest();
    assert!(!is_dag, "3-cycle is not a DAG");
    assert_eq!(hops, 0, "cyclic graph returns path_hops=0");
    assert_eq!(nc, 3);
}

/// Test 8: DAG with shortcut A→B→C + A→C → path_hops=2 (longer path A→B→C wins).
#[test]
fn test_08_shortcut_dag() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_B, DL_ID_C, "bc");
    add_edge(DL_ID_A, DL_ID_C, "ac"); // shortcut

    let (hops, is_dag, sv, ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag, "shortcut DAG is acyclic");
    assert_eq!(hops, 2, "longest path A->B->C = 2 hops (not 1 via shortcut)");
    assert_eq!(nc, 3);
    assert_eq!(sv, DL_VEC_A, "start must be A");
    assert_eq!(ev, DL_VEC_C, "end must be C");
}

/// Test 9: star fan-out A→B, A→C, A→D, A→E → path_hops=1, start=A.
#[test]
fn test_09_star_fan_out() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_node(DL_VEC_E, DL_KEY_E, DL_ID_E);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_A, DL_ID_C, "ac");
    add_edge(DL_ID_A, DL_ID_D, "ad");
    add_edge(DL_ID_A, DL_ID_E, "ae");

    let (hops, is_dag, sv, _ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag, "star is a DAG");
    assert_eq!(hops, 1, "star: all paths are exactly 1 hop from A");
    assert_eq!(nc, 5);
    assert_eq!(sv, DL_VEC_A, "start must always be A (the hub)");
}

/// Test 10: chain of 5 hops A→B→C→D→E→F → path_hops=5, start=A, end=F.
#[test]
fn test_10_five_hop_chain() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_node(DL_VEC_E, DL_KEY_E, DL_ID_E);
    add_node(DL_VEC_F, DL_KEY_F, DL_ID_F);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_B, DL_ID_C, "bc");
    add_edge(DL_ID_C, DL_ID_D, "cd");
    add_edge(DL_ID_D, DL_ID_E, "de");
    add_edge(DL_ID_E, DL_ID_F, "ef");

    let (hops, is_dag, sv, ev, nc) = gos_runtime::graph_dag_longest();
    assert!(is_dag, "6-node chain is a DAG");
    assert_eq!(hops, 5, "A->B->C->D->E->F = 5 hops");
    assert_eq!(nc, 6);
    assert_eq!(sv, DL_VEC_A, "start must be A");
    assert_eq!(ev, DL_VEC_F, "end must be F");
}
