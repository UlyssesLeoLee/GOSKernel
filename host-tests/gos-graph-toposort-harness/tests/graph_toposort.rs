// gos-graph-toposort-harness — V2.33 topological sort API tests
//
// Verifies `gos_runtime::graph_toposort` — the Kahn's BFS topological sort
// added in V2.33.  Analogous to `tsort(1)` / cmake dependency ordering.
//
//  1. Empty graph → length 0, is_dag true.
//  2. Single node, no edges → length 1, is_dag true, node emitted.
//  3. Linear chain A→B→C → length 3, is_dag true, order is A,B,C.
//  4. Two-node cycle A→B→A → is_dag false.
//  5. Diamond DAG (A→B, A→C, B→D, C→D) → is_dag true, length 4.
//  6. Diamond DAG: A precedes B and C in output.
//  7. Diamond DAG: D is last in output.
//  8. Self-loop node emitted even when self-loop exists.
//  9. Multiple disconnected chains are all emitted (is_dag true).
// 10. Cyclic graph: partial length < total node count.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const TS_PLUGIN: PluginId   = PluginId::from_ascii("KL_TSORT_H");
const TS_EXEC:   ExecutorId = ExecutorId::from_ascii("ts.exec");

const TS_KEY_A: &str = "ts.alpha";
const TS_KEY_B: &str = "ts.beta";
const TS_KEY_C: &str = "ts.gamma";
const TS_KEY_D: &str = "ts.delta";
const TS_KEY_E: &str = "ts.epsilon";

const TS_ID_A: NodeId = derive_node_id(TS_PLUGIN, TS_KEY_A);
const TS_ID_B: NodeId = derive_node_id(TS_PLUGIN, TS_KEY_B);
const TS_ID_C: NodeId = derive_node_id(TS_PLUGIN, TS_KEY_C);
const TS_ID_D: NodeId = derive_node_id(TS_PLUGIN, TS_KEY_D);
const TS_ID_E: NodeId = derive_node_id(TS_PLUGIN, TS_KEY_E);

const TS_VEC_A: VectorAddress = VectorAddress::new(12, 1, 1, 0);
const TS_VEC_B: VectorAddress = VectorAddress::new(12, 1, 2, 0);
const TS_VEC_C: VectorAddress = VectorAddress::new(12, 1, 3, 0);
const TS_VEC_D: VectorAddress = VectorAddress::new(12, 1, 4, 0);
const TS_VEC_E: VectorAddress = VectorAddress::new(12, 1, 5, 0);

const TS_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    TS_PLUGIN,
    name:         "kl-graph-toposort-harness",
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

fn node_spec(key: &'static str, node_id: NodeId, schema: u64) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: TS_EXEC,
        state_schema_hash: schema,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_abcde() {
    gos_runtime::discover_plugin(TS_MANIFEST).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_A, node_spec(TS_KEY_A, TS_ID_A, 0xD001)).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_B, node_spec(TS_KEY_B, TS_ID_B, 0xD002)).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_C, node_spec(TS_KEY_C, TS_ID_C, 0xD003)).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_D, node_spec(TS_KEY_D, TS_ID_D, 0xD004)).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_E, node_spec(TS_KEY_E, TS_ID_E, 0xD005)).unwrap();
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

fn toposort() -> (Vec<VectorAddress>, bool) {
    let (order, len, is_dag) = gos_runtime::graph_toposort::<128>();
    (order[..len].to_vec(), is_dag)
}

// ── 1. Empty graph → length 0, is_dag true ───────────────────────────────────

#[test]
fn empty_graph_toposort_is_empty_dag() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (order, is_dag) = toposort();
    assert!(is_dag, "empty graph must be a DAG");
    assert_eq!(order.len(), 0, "empty graph toposort must have length 0");
}

// ── 2. Single node, no edges → emitted, is_dag true ─────────────────────────

#[test]
fn single_node_toposort() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(TS_MANIFEST).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_A, node_spec(TS_KEY_A, TS_ID_A, 0xD001)).unwrap();
    let (order, is_dag) = toposort();
    assert!(is_dag, "single node must be a DAG");
    assert_eq!(order.len(), 1, "single node toposort must have length 1");
    assert_eq!(order[0], TS_VEC_A, "single node must be emitted");
}

// ── 3. Linear chain A→B→C → order A, B, C; is_dag true ──────────────────────

#[test]
fn linear_chain_toposort_order() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    // Register only A, B, C so no isolated nodes skew the ordering.
    gos_runtime::discover_plugin(TS_MANIFEST).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_A, node_spec(TS_KEY_A, TS_ID_A, 0xD001)).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_B, node_spec(TS_KEY_B, TS_ID_B, 0xD002)).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_C, node_spec(TS_KEY_C, TS_ID_C, 0xD003)).unwrap();
    add_edge(TS_ID_A, TS_ID_B, "ts.ab");
    add_edge(TS_ID_B, TS_ID_C, "ts.bc");
    let (order, is_dag) = toposort();
    assert!(is_dag, "linear chain A→B→C must be a DAG");
    assert_eq!(order.len(), 3, "chain A→B→C must emit exactly 3 nodes");
    // In Kahn's algorithm on A→B→C, A is the sole source; order must be A,B,C.
    let pos_a = order.iter().position(|&v| v == TS_VEC_A).expect("A must appear");
    let pos_b = order.iter().position(|&v| v == TS_VEC_B).expect("B must appear");
    let pos_c = order.iter().position(|&v| v == TS_VEC_C).expect("C must appear");
    assert!(pos_a < pos_b, "A must precede B");
    assert!(pos_b < pos_c, "B must precede C");
}

// ── 4. Two-node cycle A→B→A → is_dag false ───────────────────────────────────

#[test]
fn two_node_cycle_is_not_dag() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    add_edge(TS_ID_A, TS_ID_B, "ts.ab2");
    add_edge(TS_ID_B, TS_ID_A, "ts.ba2");
    let (_, is_dag) = toposort();
    assert!(!is_dag, "A→B→A cycle must not be a DAG");
}

// ── 5. Diamond DAG → is_dag true, length 4 ───────────────────────────────────

#[test]
fn diamond_dag_toposort_is_dag() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    add_edge(TS_ID_A, TS_ID_B, "ts.ab3");
    add_edge(TS_ID_A, TS_ID_C, "ts.ac3");
    add_edge(TS_ID_B, TS_ID_D, "ts.bd3");
    add_edge(TS_ID_C, TS_ID_D, "ts.cd3");
    let (order, is_dag) = toposort();
    assert!(is_dag, "diamond DAG must be a DAG");
    // 4 nodes are reachable from the diamond; E is isolated (also a DAG node).
    assert_eq!(order.len(), 5, "all 5 nodes must appear in toposort");
}

// ── 6. Diamond DAG: A precedes B and C ───────────────────────────────────────

#[test]
fn diamond_dag_a_precedes_b_and_c() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    add_edge(TS_ID_A, TS_ID_B, "ts.ab4");
    add_edge(TS_ID_A, TS_ID_C, "ts.ac4");
    add_edge(TS_ID_B, TS_ID_D, "ts.bd4");
    add_edge(TS_ID_C, TS_ID_D, "ts.cd4");
    let (order, is_dag) = toposort();
    assert!(is_dag);
    let pos_a = order.iter().position(|&v| v == TS_VEC_A).expect("A must appear");
    let pos_b = order.iter().position(|&v| v == TS_VEC_B).expect("B must appear");
    let pos_c = order.iter().position(|&v| v == TS_VEC_C).expect("C must appear");
    assert!(pos_a < pos_b, "A must precede B in diamond");
    assert!(pos_a < pos_c, "A must precede C in diamond");
}

// ── 7. Diamond DAG: D is last among A/B/C/D ──────────────────────────────────

#[test]
fn diamond_dag_d_is_last() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    add_edge(TS_ID_A, TS_ID_B, "ts.ab5");
    add_edge(TS_ID_A, TS_ID_C, "ts.ac5");
    add_edge(TS_ID_B, TS_ID_D, "ts.bd5");
    add_edge(TS_ID_C, TS_ID_D, "ts.cd5");
    let (order, is_dag) = toposort();
    assert!(is_dag);
    let pos_b = order.iter().position(|&v| v == TS_VEC_B).expect("B must appear");
    let pos_c = order.iter().position(|&v| v == TS_VEC_C).expect("C must appear");
    let pos_d = order.iter().position(|&v| v == TS_VEC_D).expect("D must appear");
    assert!(pos_b < pos_d, "B must precede D");
    assert!(pos_c < pos_d, "C must precede D");
}

// ── 8. Self-loop node still emitted ──────────────────────────────────────────

#[test]
fn self_loop_node_still_emitted() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    gos_runtime::discover_plugin(TS_MANIFEST).unwrap();
    gos_runtime::register_node(TS_PLUGIN, TS_VEC_A, node_spec(TS_KEY_A, TS_ID_A, 0xD001)).unwrap();
    // Self-loop: A→A (excluded from in-degree, so A still has in-degree 0)
    add_edge(TS_ID_A, TS_ID_A, "ts.aa");
    let (order, is_dag) = toposort();
    assert!(is_dag, "single node with only a self-loop must be treated as a DAG");
    assert_eq!(order.len(), 1, "self-loop node must be emitted");
    assert_eq!(order[0], TS_VEC_A, "self-loop node must be in output");
}

// ── 9. Two disconnected chains are both fully emitted ────────────────────────

#[test]
fn disconnected_chains_all_emitted() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    // Chain 1: A→B
    add_edge(TS_ID_A, TS_ID_B, "ts.ab6");
    // Chain 2: C→D→E
    add_edge(TS_ID_C, TS_ID_D, "ts.cd6");
    add_edge(TS_ID_D, TS_ID_E, "ts.de6");
    let (order, is_dag) = toposort();
    assert!(is_dag, "two disconnected chains must be a DAG");
    assert_eq!(order.len(), 5, "all 5 nodes must be emitted across two chains");
    // A before B; C before D before E
    let pos_a = order.iter().position(|&v| v == TS_VEC_A).expect("A");
    let pos_b = order.iter().position(|&v| v == TS_VEC_B).expect("B");
    let pos_c = order.iter().position(|&v| v == TS_VEC_C).expect("C");
    let pos_d = order.iter().position(|&v| v == TS_VEC_D).expect("D");
    let pos_e = order.iter().position(|&v| v == TS_VEC_E).expect("E");
    assert!(pos_a < pos_b);
    assert!(pos_c < pos_d);
    assert!(pos_d < pos_e);
}

// ── 10. Cyclic graph: partial length < total node count ──────────────────────

#[test]
fn cyclic_graph_partial_sort() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_abcde();
    // Cycle: A→B→C→A  (all 3 nodes stuck in cycle, in-degree never hits 0)
    add_edge(TS_ID_A, TS_ID_B, "ts.ab7");
    add_edge(TS_ID_B, TS_ID_C, "ts.bc7");
    add_edge(TS_ID_C, TS_ID_A, "ts.ca7");
    // D and E are isolated — they should still appear in the partial sort.
    let (order, is_dag) = toposort();
    assert!(!is_dag, "cyclic graph must not be a DAG");
    // A, B, C are stuck; D and E have in-degree 0 and must be emitted.
    assert_eq!(order.len(), 2, "only the 2 isolated nodes must be emitted");
    assert!(order.contains(&TS_VEC_D), "D must appear in partial toposort");
    assert!(order.contains(&TS_VEC_E), "E must appear in partial toposort");
}
