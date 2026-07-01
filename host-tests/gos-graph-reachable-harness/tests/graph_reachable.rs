// gos-graph-reachable-harness — V2.36 transitive reachability API tests
//
// Verifies `gos_runtime::graph_reachable` — iterative DFS returning all nodes
// reachable from a given vector via directed edges (excluding the source).
// Analogous to `systemctl list-dependencies --all`, `cargo tree -p`, or
// `ldd --recursive`.
//
//  1. Source not registered → 0 reachable.
//  2. Single isolated node → 0 reachable (no outbound edges).
//  3. A→B: reachable from A = {B}.
//  4. A→B→C: reachable from A = {B, C}.
//  5. Fan-out A→B, A→C: reachable from A = {B, C}.
//  6. Cycle A→B→A: reachable from A = {B} (no infinite loop).
//  7. Triangle A→B→C→A: reachable from A = {B, C}.
//  8. Chain A→B→C→D: reachable from B = {C, D} (not A).
//  9. Disconnected: A→B isolated from C→D; reachable from A = {B}, not C or D.
// 10. Output sorted ascending by VectorAddress.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const REACH_PLUGIN: PluginId   = PluginId::from_ascii("KL_REACH_00");
const REACH_EXEC:   ExecutorId = ExecutorId::from_ascii("reach.exec");

const REACH_KEY_A: &str = "reach.alpha";
const REACH_KEY_B: &str = "reach.beta";
const REACH_KEY_C: &str = "reach.gamma";
const REACH_KEY_D: &str = "reach.delta";
const REACH_KEY_E: &str = "reach.epsilon";

const REACH_ID_A: NodeId = derive_node_id(REACH_PLUGIN, REACH_KEY_A);
const REACH_ID_B: NodeId = derive_node_id(REACH_PLUGIN, REACH_KEY_B);
const REACH_ID_C: NodeId = derive_node_id(REACH_PLUGIN, REACH_KEY_C);
const REACH_ID_D: NodeId = derive_node_id(REACH_PLUGIN, REACH_KEY_D);
const REACH_ID_E: NodeId = derive_node_id(REACH_PLUGIN, REACH_KEY_E);

const REACH_VEC_A: VectorAddress = VectorAddress::new(15, 1, 1, 0);
const REACH_VEC_B: VectorAddress = VectorAddress::new(15, 1, 2, 0);
const REACH_VEC_C: VectorAddress = VectorAddress::new(15, 1, 3, 0);
const REACH_VEC_D: VectorAddress = VectorAddress::new(15, 1, 4, 0);
const REACH_VEC_E: VectorAddress = VectorAddress::new(15, 1, 5, 0);

const REACH_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    REACH_PLUGIN,
    name:         "kl-graph-reachable-harness",
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
        executor_id: REACH_EXEC,
        state_schema_hash: schema,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_plugin() {
    gos_runtime::discover_plugin(REACH_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(REACH_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn reachable(from: VectorAddress) -> Vec<VectorAddress> {
    let (nodes, len) = gos_runtime::graph_reachable::<128>(from);
    nodes[..len].to_vec()
}

fn contains(set: &[VectorAddress], vec: VectorAddress) -> bool {
    set.iter().any(|&v| v == vec)
}

// ── 1. Source not registered → 0 reachable ───────────────────────────────────

#[test]
fn unregistered_source_returns_empty() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 0, "unregistered source: expect 0 reachable");
}

// ── 2. Single isolated node → 0 reachable ────────────────────────────────────

#[test]
fn isolated_node_returns_empty() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 0, "isolated node: no outbound edges → 0 reachable");
}

// ── 3. A→B: reachable from A = {B} ───────────────────────────────────────────

#[test]
fn single_edge_reaches_one_node() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t3");
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 1, "A→B: 1 reachable node");
    assert!(contains(&result, REACH_VEC_B), "B must be reachable from A");
    assert!(!contains(&result, REACH_VEC_A), "source A must not appear in reachable set");
}

// ── 4. A→B→C: reachable from A = {B, C} ─────────────────────────────────────

#[test]
fn chain_reaches_transitive_node() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_node(REACH_VEC_C, REACH_KEY_C, REACH_ID_C, 0xA003);
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t4");
    add_edge(REACH_ID_B, REACH_ID_C, "reach.bc.t4");
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 2, "A→B→C: 2 reachable from A");
    assert!(contains(&result, REACH_VEC_B), "B reachable from A");
    assert!(contains(&result, REACH_VEC_C), "C reachable transitively from A");
}

// ── 5. Fan-out A→B, A→C: reachable from A = {B, C} ──────────────────────────

#[test]
fn fan_out_reaches_both_children() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_node(REACH_VEC_C, REACH_KEY_C, REACH_ID_C, 0xA003);
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t5");
    add_edge(REACH_ID_A, REACH_ID_C, "reach.ac.t5");
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 2, "fan-out A→B, A→C: 2 reachable");
    assert!(contains(&result, REACH_VEC_B), "B reachable from A");
    assert!(contains(&result, REACH_VEC_C), "C reachable from A");
}

// ── 6. Cycle A→B→A: reachable from A = {B} (no infinite loop) ───────────────

#[test]
fn cycle_does_not_loop_forever() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t6a");
    add_edge(REACH_ID_B, REACH_ID_A, "reach.ba.t6b");
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 1, "cycle A↔B: only B reachable from A");
    assert!(contains(&result, REACH_VEC_B), "B reachable from A");
    assert!(!contains(&result, REACH_VEC_A), "A (source) must not appear");
}

// ── 7. Triangle A→B→C→A: reachable from A = {B, C} ──────────────────────────

#[test]
fn triangle_reaches_all_other_members() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_node(REACH_VEC_C, REACH_KEY_C, REACH_ID_C, 0xA003);
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t7");
    add_edge(REACH_ID_B, REACH_ID_C, "reach.bc.t7");
    add_edge(REACH_ID_C, REACH_ID_A, "reach.ca.t7");
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 2, "triangle A→B→C→A: 2 reachable from A");
    assert!(contains(&result, REACH_VEC_B), "B reachable from A");
    assert!(contains(&result, REACH_VEC_C), "C reachable from A");
    assert!(!contains(&result, REACH_VEC_A), "A not in reachable set");
}

// ── 8. Chain A→B→C→D: reachable from B = {C, D}, not A ──────────────────────

#[test]
fn reachable_from_midpoint_excludes_predecessor() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_node(REACH_VEC_C, REACH_KEY_C, REACH_ID_C, 0xA003);
    add_node(REACH_VEC_D, REACH_KEY_D, REACH_ID_D, 0xA004);
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t8");
    add_edge(REACH_ID_B, REACH_ID_C, "reach.bc.t8");
    add_edge(REACH_ID_C, REACH_ID_D, "reach.cd.t8");
    let result = reachable(REACH_VEC_B);
    assert_eq!(result.len(), 2, "reachable from B: C and D");
    assert!(contains(&result, REACH_VEC_C), "C reachable from B");
    assert!(contains(&result, REACH_VEC_D), "D reachable from B");
    assert!(!contains(&result, REACH_VEC_A), "A not reachable from B (no back-edge)");
    assert!(!contains(&result, REACH_VEC_B), "B (source) not in reachable set");
}

// ── 9. Disconnected: A→B vs C→D; reachable from A = {B} only ────────────────

#[test]
fn disconnected_components_not_reached() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_node(REACH_VEC_C, REACH_KEY_C, REACH_ID_C, 0xA003);
    add_node(REACH_VEC_D, REACH_KEY_D, REACH_ID_D, 0xA004);
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t9");
    add_edge(REACH_ID_C, REACH_ID_D, "reach.cd.t9");
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 1, "only B reachable from A");
    assert!(contains(&result, REACH_VEC_B), "B reachable from A");
    assert!(!contains(&result, REACH_VEC_C), "C not reachable from A");
    assert!(!contains(&result, REACH_VEC_D), "D not reachable from A");
}

// ── 10. Output sorted ascending by VectorAddress ─────────────────────────────

#[test]
fn reachable_output_sorted_ascending() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Register nodes in reverse order so insertion sort has real work to do.
    add_node(REACH_VEC_E, REACH_KEY_E, REACH_ID_E, 0xA005);
    add_node(REACH_VEC_D, REACH_KEY_D, REACH_ID_D, 0xA004);
    add_node(REACH_VEC_C, REACH_KEY_C, REACH_ID_C, 0xA003);
    add_node(REACH_VEC_B, REACH_KEY_B, REACH_ID_B, 0xA002);
    add_node(REACH_VEC_A, REACH_KEY_A, REACH_ID_A, 0xA001);
    // A fans out to B, C, D, E.
    add_edge(REACH_ID_A, REACH_ID_E, "reach.ae.t10");
    add_edge(REACH_ID_A, REACH_ID_D, "reach.ad.t10");
    add_edge(REACH_ID_A, REACH_ID_C, "reach.ac.t10");
    add_edge(REACH_ID_A, REACH_ID_B, "reach.ab.t10");
    let result = reachable(REACH_VEC_A);
    assert_eq!(result.len(), 4, "4 reachable from A");
    // Verify ascending order.
    for i in 1..result.len() {
        assert!(
            result[i - 1].as_u64() < result[i].as_u64(),
            "output not sorted: result[{}]={:?} >= result[{}]={:?}",
            i - 1, result[i - 1], i, result[i]
        );
    }
    // Verify membership.
    assert!(contains(&result, REACH_VEC_B), "B in result");
    assert!(contains(&result, REACH_VEC_C), "C in result");
    assert!(contains(&result, REACH_VEC_D), "D in result");
    assert!(contains(&result, REACH_VEC_E), "E in result");
}
