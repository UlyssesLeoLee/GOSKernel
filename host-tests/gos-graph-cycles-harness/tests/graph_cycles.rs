// gos-graph-cycles-harness — V2.32 directed cycle detection API tests
//
// Verifies `gos_runtime::find_graph_cycle` and `gos_runtime::is_cyclic` —
// the iterative 3-color DFS cycle detector added in V2.32.
// Analogous to `tsort` cycle checking or `cargo` circular dependency detection.
//
//  1. Empty graph → no cycle (length 0).
//  2. Single isolated node → no cycle.
//  3. Linear chain A→B→C → no cycle (DAG).
//  4. Self-loop A→A → cycle detected (length >= 2).
//  5. Two-node cycle A→B→A → cycle detected (length >= 3).
//  6. Three-node cycle A→B→C→A → cycle detected (length >= 4).
//  7. Diamond (shared ancestor) A→B, A→C, B→D, C→D → no cycle.
//  8. Mixed: DAG subgraph + isolated cycle component → cycle detected.
//  9. `is_cyclic()` returns false for a pure DAG.
// 10. `is_cyclic()` returns true when a cycle exists.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const GC_PLUGIN: PluginId   = PluginId::from_ascii("KL_GCYCLE_H");
const GC_EXEC:   ExecutorId = ExecutorId::from_ascii("gc.exec");

const GC_KEY_A: &str = "gc.alpha";
const GC_KEY_B: &str = "gc.beta";
const GC_KEY_C: &str = "gc.gamma";
const GC_KEY_D: &str = "gc.delta";
const GC_KEY_E: &str = "gc.epsilon";

const GC_ID_A: NodeId = derive_node_id(GC_PLUGIN, GC_KEY_A);
const GC_ID_B: NodeId = derive_node_id(GC_PLUGIN, GC_KEY_B);
const GC_ID_C: NodeId = derive_node_id(GC_PLUGIN, GC_KEY_C);
const GC_ID_D: NodeId = derive_node_id(GC_PLUGIN, GC_KEY_D);
const GC_ID_E: NodeId = derive_node_id(GC_PLUGIN, GC_KEY_E);

const GC_VEC_A: VectorAddress = VectorAddress::new(11, 4, 1, 0);
const GC_VEC_B: VectorAddress = VectorAddress::new(11, 4, 2, 0);
const GC_VEC_C: VectorAddress = VectorAddress::new(11, 4, 3, 0);
const GC_VEC_D: VectorAddress = VectorAddress::new(11, 4, 4, 0);
const GC_VEC_E: VectorAddress = VectorAddress::new(11, 4, 5, 0);

const GC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    GC_PLUGIN,
    name:         "kl-graph-cycles-harness",
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
        executor_id: GC_EXEC,
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
    gos_runtime::discover_plugin(GC_MANIFEST).unwrap();
    gos_runtime::register_node(GC_PLUGIN, GC_VEC_A, node_spec(GC_KEY_A, GC_ID_A, 0xC001)).unwrap();
    gos_runtime::register_node(GC_PLUGIN, GC_VEC_B, node_spec(GC_KEY_B, GC_ID_B, 0xC002)).unwrap();
    gos_runtime::register_node(GC_PLUGIN, GC_VEC_C, node_spec(GC_KEY_C, GC_ID_C, 0xC003)).unwrap();
    gos_runtime::register_node(GC_PLUGIN, GC_VEC_D, node_spec(GC_KEY_D, GC_ID_D, 0xC004)).unwrap();
    gos_runtime::register_node(GC_PLUGIN, GC_VEC_E, node_spec(GC_KEY_E, GC_ID_E, 0xC005)).unwrap();
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

fn has_cycle() -> bool {
    let (_, len) = gos_runtime::find_graph_cycle::<32>();
    len > 0
}

fn cycle_len() -> usize {
    let (_, len) = gos_runtime::find_graph_cycle::<32>();
    len
}

// ── 1. Empty graph → no cycle ────────────────────────────────────────────────

#[test]
fn empty_graph_no_cycle() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    assert!(!has_cycle(), "empty graph must have no cycle");
    assert_eq!(cycle_len(), 0, "empty graph cycle_len must be 0");
}

// ── 2. Single isolated node → no cycle ───────────────────────────────────────

#[test]
fn single_node_no_cycle() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(GC_MANIFEST).unwrap();
    gos_runtime::register_node(GC_PLUGIN, GC_VEC_A, node_spec(GC_KEY_A, GC_ID_A, 0xC001)).unwrap();
    assert!(!has_cycle(), "single node with no edges must have no cycle");
}

// ── 3. Linear chain A→B→C → no cycle (DAG) ───────────────────────────────────

#[test]
fn linear_chain_is_acyclic() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    add_edge(GC_ID_A, GC_ID_B, "gc.ab");
    add_edge(GC_ID_B, GC_ID_C, "gc.bc");
    assert!(!has_cycle(), "chain A→B→C must be acyclic");
}

// ── 4. Self-loop A→A → cycle detected ────────────────────────────────────────

#[test]
fn self_loop_is_cyclic() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    add_edge(GC_ID_A, GC_ID_A, "gc.aa");
    assert!(has_cycle(), "self-loop A→A must be detected as a cycle");
    assert!(cycle_len() >= 2, "self-loop cycle path must have length >= 2");
}

// ── 5. Two-node cycle A→B→A → cycle detected ─────────────────────────────────

#[test]
fn two_node_cycle_detected() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    add_edge(GC_ID_A, GC_ID_B, "gc.ab2");
    add_edge(GC_ID_B, GC_ID_A, "gc.ba2");
    assert!(has_cycle(), "A→B→A must be detected as a cycle");
    assert!(cycle_len() >= 3, "A→B→A cycle path must have length >= 3");
}

// ── 6. Three-node cycle A→B→C→A → cycle detected ─────────────────────────────

#[test]
fn three_node_cycle_detected() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    add_edge(GC_ID_A, GC_ID_B, "gc.ab3");
    add_edge(GC_ID_B, GC_ID_C, "gc.bc3");
    add_edge(GC_ID_C, GC_ID_A, "gc.ca3");
    assert!(has_cycle(), "A→B→C→A must be detected as a cycle");
    assert!(cycle_len() >= 4, "A→B→C→A cycle path must have length >= 4");
}

// ── 7. Diamond (A→B, A→C, B→D, C→D) → no cycle ──────────────────────────────

#[test]
fn diamond_dag_is_acyclic() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    add_edge(GC_ID_A, GC_ID_B, "gc.ab4");
    add_edge(GC_ID_A, GC_ID_C, "gc.ac4");
    add_edge(GC_ID_B, GC_ID_D, "gc.bd4");
    add_edge(GC_ID_C, GC_ID_D, "gc.cd4");
    assert!(!has_cycle(), "diamond DAG (A→B, A→C, B→D, C→D) must be acyclic");
}

// ── 8. Mixed: DAG subgraph + isolated cycle ────────────────────────────────

#[test]
fn mixed_dag_and_cycle_detected() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    // DAG part: A→B→C (acyclic)
    add_edge(GC_ID_A, GC_ID_B, "gc.ab5");
    add_edge(GC_ID_B, GC_ID_C, "gc.bc5");
    // Isolated cycle: D→E→D
    add_edge(GC_ID_D, GC_ID_E, "gc.de5");
    add_edge(GC_ID_E, GC_ID_D, "gc.ed5");
    assert!(has_cycle(), "graph with an isolated D→E→D cycle must be detected");
}

// ── 9. `is_cyclic()` returns false for pure DAG ──────────────────────────────

#[test]
fn is_cyclic_false_for_dag() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    add_edge(GC_ID_A, GC_ID_B, "gc.ab6");
    add_edge(GC_ID_B, GC_ID_C, "gc.bc6");
    add_edge(GC_ID_C, GC_ID_D, "gc.cd6");
    assert!(!gos_runtime::is_cyclic(), "is_cyclic() must return false for a pure DAG");
}

// ── 10. `is_cyclic()` returns true when cycle exists ─────────────────────────

#[test]
fn is_cyclic_true_when_cycle_exists() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_abcde();
    add_edge(GC_ID_A, GC_ID_B, "gc.ab7");
    add_edge(GC_ID_B, GC_ID_C, "gc.bc7");
    add_edge(GC_ID_C, GC_ID_A, "gc.ca7"); // closes the cycle
    assert!(gos_runtime::is_cyclic(), "is_cyclic() must return true when A→B→C→A exists");
}
