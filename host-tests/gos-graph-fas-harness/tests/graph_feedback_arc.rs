// gos-graph-fas-harness -- V2.91 feedback arc set tests
//
// Verifies `gos_runtime::graph_feedback_arc` — iterative DFS 3-colouring
// (UNVISITED / IN_STACK / DONE) that identifies back-edges forming a valid
// feedback arc set.  Removing all returned arcs leaves the graph acyclic.
//
//  1. Empty graph              -> arc_count=0, node_count=0.
//  2. Single node, no edges    -> arc_count=0 (trivially acyclic).
//  3. Self-loop A->A           -> arc_count=1, arc is A->A.
//  4. 2-cycle A->B->A          -> arc_count=1 (one back-edge).
//  5. DAG chain A->B->C        -> arc_count=0 (no cycles).
//  6. Diamond DAG A->{B,C}->D  -> arc_count=0 (no cycles).
//  7. Triangle A->B->C->A      -> arc_count=1.
//  8. Two independent cycles   -> arc_count=2.
//  9. Disconnected: DAG + cycle -> arc_count reflects only cycle component.
// 10. DAG status cross-check: arc_count==0 iff graph_dag_layers reports is_dag.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const FAS_PLUGIN: PluginId   = PluginId::from_ascii("KL_FASARC_H_");
const FAS_EXEC:   ExecutorId = ExecutorId::from_ascii("fas.exec");

const FAS_KEY_A: &str = "fas.alpha";
const FAS_KEY_B: &str = "fas.beta";
const FAS_KEY_C: &str = "fas.gamma";
const FAS_KEY_D: &str = "fas.delta";

const FAS_ID_A: NodeId = derive_node_id(FAS_PLUGIN, FAS_KEY_A);
const FAS_ID_B: NodeId = derive_node_id(FAS_PLUGIN, FAS_KEY_B);
const FAS_ID_C: NodeId = derive_node_id(FAS_PLUGIN, FAS_KEY_C);
const FAS_ID_D: NodeId = derive_node_id(FAS_PLUGIN, FAS_KEY_D);

// L4=67 namespace for this harness.
const FAS_VEC_A: VectorAddress = VectorAddress::new(67, 1, 1, 0);
const FAS_VEC_B: VectorAddress = VectorAddress::new(67, 1, 2, 0);
const FAS_VEC_C: VectorAddress = VectorAddress::new(67, 1, 3, 0);
const FAS_VEC_D: VectorAddress = VectorAddress::new(67, 1, 4, 0);

const FAS_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    FAS_PLUGIN,
    name:         "kl-graph-fas-harness",
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
        executor_id:       FAS_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(FAS_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(FAS_MANIFEST).unwrap();
    g
}

/// Check whether a particular arc (from→to) appears in the output.
fn has_arc(
    from_vecs: &[VectorAddress],
    to_vecs:   &[VectorAddress],
    count:     usize,
    from:      VectorAddress,
    to:        VectorAddress,
) -> bool {
    for i in 0..count {
        if from_vecs[i] == from && to_vecs[i] == to { return true; }
    }
    false
}

// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_, _, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 0, "empty graph: 0 nodes");
    assert_eq!(arc_count,  0, "empty graph: 0 feedback arcs");
}

#[test]
fn test_02_single_node_no_edges() {
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    let (_, _, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 1);
    assert_eq!(arc_count,  0, "isolated node has no feedback arcs");
}

#[test]
fn test_03_self_loop() {
    // A -> A: a self-loop is trivially a directed cycle of length 1.
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    add_edge(FAS_ID_A, FAS_ID_A, "aa");
    let (from_vecs, to_vecs, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 1);
    assert_eq!(arc_count,  1, "self-loop is one feedback arc");
    assert!(
        has_arc(&from_vecs, &to_vecs, arc_count, FAS_VEC_A, FAS_VEC_A),
        "self-loop A->A must appear"
    );
}

#[test]
fn test_04_two_cycle() {
    // A -> B -> A: 2-node directed cycle.  DFS from A pushes B; B sees A on
    // the stack — one back-edge.
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
    add_edge(FAS_ID_A, FAS_ID_B, "ab");
    add_edge(FAS_ID_B, FAS_ID_A, "ba");
    let (_, _, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 2);
    assert_eq!(arc_count,  1, "2-cycle produces exactly 1 back-edge");
}

#[test]
fn test_05_dag_chain_no_arcs() {
    // A -> B -> C: acyclic chain.  DFS finds no back-edges.
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
    add_node(FAS_VEC_C, FAS_KEY_C, FAS_ID_C);
    add_edge(FAS_ID_A, FAS_ID_B, "ab");
    add_edge(FAS_ID_B, FAS_ID_C, "bc");
    let (_, _, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 3);
    assert_eq!(arc_count,  0, "DAG chain: no feedback arcs");
}

#[test]
fn test_06_diamond_dag_no_arcs() {
    // A -> B, A -> C, B -> D, C -> D: diamond DAG.  No cycles.
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
    add_node(FAS_VEC_C, FAS_KEY_C, FAS_ID_C);
    add_node(FAS_VEC_D, FAS_KEY_D, FAS_ID_D);
    add_edge(FAS_ID_A, FAS_ID_B, "ab");
    add_edge(FAS_ID_A, FAS_ID_C, "ac");
    add_edge(FAS_ID_B, FAS_ID_D, "bd");
    add_edge(FAS_ID_C, FAS_ID_D, "cd");
    let (_, _, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 4);
    assert_eq!(arc_count,  0, "diamond DAG: no feedback arcs");
}

#[test]
fn test_07_triangle_cycle() {
    // A -> B -> C -> A: 3-node triangle.  DFS reaches C, C sees A on stack.
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
    add_node(FAS_VEC_C, FAS_KEY_C, FAS_ID_C);
    add_edge(FAS_ID_A, FAS_ID_B, "ab");
    add_edge(FAS_ID_B, FAS_ID_C, "bc");
    add_edge(FAS_ID_C, FAS_ID_A, "ca");
    let (from_vecs, to_vecs, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 3);
    assert_eq!(arc_count,  1, "triangle: exactly 1 back-edge");
    // The back-edge closes the triangle: C->A (C sees A on stack).
    assert!(
        has_arc(&from_vecs, &to_vecs, arc_count, FAS_VEC_C, FAS_VEC_A),
        "back-edge of triangle is C->A"
    );
}

#[test]
fn test_08_two_independent_cycles() {
    // Cycle-1: A -> B -> A
    // Cycle-2: C -> D -> C
    // Both are independent 2-cycles; each contributes 1 back-edge.
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
    add_node(FAS_VEC_C, FAS_KEY_C, FAS_ID_C);
    add_node(FAS_VEC_D, FAS_KEY_D, FAS_ID_D);
    add_edge(FAS_ID_A, FAS_ID_B, "ab");
    add_edge(FAS_ID_B, FAS_ID_A, "ba");
    add_edge(FAS_ID_C, FAS_ID_D, "cd");
    add_edge(FAS_ID_D, FAS_ID_C, "dc");
    let (_, _, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 4);
    assert_eq!(arc_count,  2, "two independent cycles -> 2 feedback arcs");
}

#[test]
fn test_09_disconnected_dag_plus_cycle() {
    // Component-1: A -> B (DAG, no arcs).
    // Component-2: C -> D -> C (2-cycle, 1 arc).
    let _g = setup();
    add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
    add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
    add_node(FAS_VEC_C, FAS_KEY_C, FAS_ID_C);
    add_node(FAS_VEC_D, FAS_KEY_D, FAS_ID_D);
    add_edge(FAS_ID_A, FAS_ID_B, "ab");
    add_edge(FAS_ID_C, FAS_ID_D, "cd");
    add_edge(FAS_ID_D, FAS_ID_C, "dc");
    let (_, _, arc_count, node_count) =
        gos_runtime::graph_feedback_arc::<512>();
    assert_eq!(node_count, 4, "four nodes total");
    assert_eq!(arc_count,  1, "only the cycle component contributes a feedback arc");
}

#[test]
fn test_10_consistency_with_dag_layers() {
    // Cross-check: arc_count == 0 iff graph_dag_layers reports is_dag == true.
    // Test on three graphs: a DAG, a cyclic graph, and an empty graph.

    // Case A: DAG (chain A->B->C).
    {
        let _g = setup();
        add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
        add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
        add_node(FAS_VEC_C, FAS_KEY_C, FAS_ID_C);
        add_edge(FAS_ID_A, FAS_ID_B, "ab");
        add_edge(FAS_ID_B, FAS_ID_C, "bc");
        let (_, _, fas_count, _) = gos_runtime::graph_feedback_arc::<512>();
        let (_, _, _, _, is_dag) = gos_runtime::graph_dag_layers::<128>();
        assert!(is_dag, "chain A->B->C is a DAG");
        assert_eq!(fas_count, 0, "DAG => no feedback arcs");
    }

    // Case B: cyclic (A->B->A).
    {
        let _g = setup();
        add_node(FAS_VEC_A, FAS_KEY_A, FAS_ID_A);
        add_node(FAS_VEC_B, FAS_KEY_B, FAS_ID_B);
        add_edge(FAS_ID_A, FAS_ID_B, "ab");
        add_edge(FAS_ID_B, FAS_ID_A, "ba");
        let (_, _, fas_count, _) = gos_runtime::graph_feedback_arc::<512>();
        let (_, _, _, _, is_dag) = gos_runtime::graph_dag_layers::<128>();
        assert!(!is_dag, "A->B->A is cyclic");
        assert!(fas_count > 0, "cyclic => at least one feedback arc");
    }

    // Case C: empty graph.
    {
        let _g = setup();
        let (_, _, fas_count, _) = gos_runtime::graph_feedback_arc::<512>();
        let (_, _, _, _, is_dag) = gos_runtime::graph_dag_layers::<128>();
        assert!(is_dag, "empty graph is vacuously a DAG");
        assert_eq!(fas_count, 0, "empty graph => no feedback arcs");
    }
}
