// gos-graph-eulerian-harness — V2.87 Eulerian path/circuit detection tests
//
// Verifies `gos_runtime::graph_eulerian` — O(V+E) detection of whether the
// directed live kernel graph admits:
//   • Eulerian circuit: closed walk visiting every edge exactly once.
//     Conditions: weakly connected + all nodes have in_degree == out_degree.
//   • Eulerian path: open walk visiting every edge exactly once.
//     Conditions: weakly connected + exactly one node with out-in=1 (start),
//     exactly one with in-out=1 (end), all others balanced.
//
// Isolated nodes are excluded from degree and connectivity checks.
// Vacuous case (no edges): has_circuit=true.
//
// OS analogy: can a maintenance daemon visit every IPC channel exactly once
// and return to base (circuit), or perform a complete single-pass audit
// (path)?  Equivalent to the "Chinese postman" / "route inspection" problem
// applied to the kernel dependency graph.
//
//  1. Empty graph → has_circuit=true (vacuous), has_path=false.
//  2. Single isolated node (no edges) → has_circuit=true (vacuous).
//  3. Directed triangle A→B→C→A → Eulerian circuit (every node balanced).
//  4. Single directed edge A→B → Eulerian path (start=A, end=B).
//  5. Path A→B→C → Eulerian path (start=A, end=C).
//  6. Anti-parallel pair A→B + B→A → Eulerian circuit.
//  7. Two disconnected edges A→B, C→D → neither (disconnected).
//  8. Square 4-cycle A→B→C→D→A → Eulerian circuit.
//  9. Lollipop: triangle A→B→C→A plus tail C→D → Eulerian path (start=C, end=D).
// 10. Imbalanced star hub→A + hub→B + C→hub → neither (hub has out=2, in=1; imbalance 1 each side but no path start/end pairing with rest balanced).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const EL_PLUGIN: PluginId   = PluginId::from_ascii("KL_EL01_00");
const EL_EXEC:   ExecutorId = ExecutorId::from_ascii("el.exec");

const EL_KEY_A: &str = "el.alpha";
const EL_KEY_B: &str = "el.beta";
const EL_KEY_C: &str = "el.gamma";
const EL_KEY_D: &str = "el.delta";
const EL_KEY_H: &str = "el.hub";

const EL_ID_A: NodeId = derive_node_id(EL_PLUGIN, EL_KEY_A);
const EL_ID_B: NodeId = derive_node_id(EL_PLUGIN, EL_KEY_B);
const EL_ID_C: NodeId = derive_node_id(EL_PLUGIN, EL_KEY_C);
const EL_ID_D: NodeId = derive_node_id(EL_PLUGIN, EL_KEY_D);
const EL_ID_H: NodeId = derive_node_id(EL_PLUGIN, EL_KEY_H);

// L4=63 identifies this harness namespace.
const EL_VEC_A: VectorAddress = VectorAddress::new(63, 1, 1, 0);
const EL_VEC_B: VectorAddress = VectorAddress::new(63, 1, 2, 0);
const EL_VEC_C: VectorAddress = VectorAddress::new(63, 1, 3, 0);
const EL_VEC_D: VectorAddress = VectorAddress::new(63, 1, 4, 0);
const EL_VEC_H: VectorAddress = VectorAddress::new(63, 1, 5, 0);

const EL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    EL_PLUGIN,
    name:         "kl-graph-eulerian-harness",
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
        executor_id:       EL_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(EL_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(EL_MANIFEST).unwrap();
    g
}

// 1. Empty graph: no nodes, no edges → vacuous Eulerian circuit.
#[test]
fn test_01_empty_graph_circuit() {
    let _g = setup();
    let (has_circuit, has_path, _sv, _ev, node_count) = gos_runtime::graph_eulerian();
    assert_eq!(node_count, 0, "empty: no nodes");
    assert!(has_circuit,  "empty: vacuous circuit");
    assert!(!has_path,    "empty: no path");
}

// 2. Single isolated node, no edges → vacuous circuit (isolated node excluded).
#[test]
fn test_02_isolated_node_circuit() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    let (has_circuit, has_path, _sv, _ev, node_count) = gos_runtime::graph_eulerian();
    assert_eq!(node_count, 1);
    assert!(has_circuit, "isolated node: still vacuous circuit (no edges)");
    assert!(!has_path);
}

// 3. Directed triangle A→B→C→A: every node has in=out=1 → Eulerian circuit.
#[test]
fn test_03_triangle_circuit() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_node(EL_VEC_C, EL_KEY_C, EL_ID_C);
    add_edge(EL_ID_A, EL_ID_B, "ab");
    add_edge(EL_ID_B, EL_ID_C, "bc");
    add_edge(EL_ID_C, EL_ID_A, "ca");
    let (has_circuit, has_path, _sv, _ev, node_count) = gos_runtime::graph_eulerian();
    assert_eq!(node_count, 3);
    assert!(has_circuit, "triangle: Eulerian circuit");
    assert!(!has_path,   "triangle: not merely a path");
}

// 4. Single directed edge A→B: A has out=1,in=0 (+1) and B has out=0,in=1 (-1)
//    → Eulerian path, start=A, end=B.
#[test]
fn test_04_single_edge_path() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_edge(EL_ID_A, EL_ID_B, "ab");
    let (has_circuit, has_path, start_vec, end_vec, node_count) =
        gos_runtime::graph_eulerian();
    assert_eq!(node_count, 2);
    assert!(!has_circuit, "single edge: not a circuit");
    assert!(has_path,     "single edge: Eulerian path");
    assert_eq!(start_vec, EL_VEC_A, "start must be A (out-in=1)");
    assert_eq!(end_vec,   EL_VEC_B, "end must be B (in-out=1)");
}

// 5. Path A→B→C: A has out=1,in=0; C has in=1,out=0; B is balanced.
//    → Eulerian path, start=A, end=C.
#[test]
fn test_05_three_node_path() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_node(EL_VEC_C, EL_KEY_C, EL_ID_C);
    add_edge(EL_ID_A, EL_ID_B, "ab");
    add_edge(EL_ID_B, EL_ID_C, "bc");
    let (has_circuit, has_path, start_vec, end_vec, node_count) =
        gos_runtime::graph_eulerian();
    assert_eq!(node_count, 3);
    assert!(!has_circuit);
    assert!(has_path, "A-B-C: Eulerian path");
    assert_eq!(start_vec, EL_VEC_A, "start=A");
    assert_eq!(end_vec,   EL_VEC_C, "end=C");
}

// 6. Anti-parallel pair A→B + B→A: both nodes have in=out=1 → Eulerian circuit.
#[test]
fn test_06_antiparallel_circuit() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_edge(EL_ID_A, EL_ID_B, "ab");
    add_edge(EL_ID_B, EL_ID_A, "ba");
    let (has_circuit, has_path, _sv, _ev, node_count) = gos_runtime::graph_eulerian();
    assert_eq!(node_count, 2);
    assert!(has_circuit, "anti-parallel: Eulerian circuit");
    assert!(!has_path);
}

// 7. Two disconnected directed edges A→B and C→D: degree conditions say "path"
//    but the graph is not weakly connected → neither.
#[test]
fn test_07_disconnected_neither() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_node(EL_VEC_C, EL_KEY_C, EL_ID_C);
    add_node(EL_VEC_D, EL_KEY_D, EL_ID_D);
    add_edge(EL_ID_A, EL_ID_B, "ab");
    add_edge(EL_ID_C, EL_ID_D, "cd");
    let (has_circuit, has_path, _sv, _ev, node_count) = gos_runtime::graph_eulerian();
    assert_eq!(node_count, 4);
    assert!(!has_circuit, "disconnected: no circuit");
    assert!(!has_path,    "disconnected: no path (not weakly connected)");
}

// 8. Square 4-cycle A→B→C→D→A: every node balanced → Eulerian circuit.
#[test]
fn test_08_square_circuit() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_node(EL_VEC_C, EL_KEY_C, EL_ID_C);
    add_node(EL_VEC_D, EL_KEY_D, EL_ID_D);
    add_edge(EL_ID_A, EL_ID_B, "ab");
    add_edge(EL_ID_B, EL_ID_C, "bc");
    add_edge(EL_ID_C, EL_ID_D, "cd");
    add_edge(EL_ID_D, EL_ID_A, "da");
    let (has_circuit, has_path, _sv, _ev, node_count) = gos_runtime::graph_eulerian();
    assert_eq!(node_count, 4);
    assert!(has_circuit, "4-cycle: Eulerian circuit");
    assert!(!has_path);
}

// 9. Lollipop: triangle A→B→C→A plus tail C→D.
//    C gets +1 out_degree extra → out_C=2, in_C=1 → diff=+1 (start).
//    D gets in_D=1, out_D=0 → diff=-1 (end).
//    A and B are balanced (in=out=1).
//    → Eulerian path, start=C, end=D.
#[test]
fn test_09_lollipop_path() {
    let _g = setup();
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_node(EL_VEC_C, EL_KEY_C, EL_ID_C);
    add_node(EL_VEC_D, EL_KEY_D, EL_ID_D);
    // Triangle
    add_edge(EL_ID_A, EL_ID_B, "ab");
    add_edge(EL_ID_B, EL_ID_C, "bc");
    add_edge(EL_ID_C, EL_ID_A, "ca");
    // Tail
    add_edge(EL_ID_C, EL_ID_D, "cd");
    let (has_circuit, has_path, start_vec, end_vec, node_count) =
        gos_runtime::graph_eulerian();
    assert_eq!(node_count, 4);
    assert!(!has_circuit, "lollipop: C has extra out-edge, not a circuit");
    assert!(has_path, "lollipop: Eulerian path exists");
    assert_eq!(start_vec, EL_VEC_C, "start=C (out-in=2-1=1)");
    assert_eq!(end_vec,   EL_VEC_D, "end=D (in-out=1-0=1)");
}

// 10. Imbalanced: hub→A + hub→B + C→hub (hub: out=2, in=1 → diff=+1).
//     A: in=1, out=0 → diff=-1.
//     B: in=1, out=0 → diff=-1.
//     C: out=1, in=0 → diff=+1.
//     Two start candidates (hub, C) and two end candidates (A, B) → not a valid path.
//     → neither.
#[test]
fn test_10_two_starts_two_ends_neither() {
    let _g = setup();
    add_node(EL_VEC_H, EL_KEY_H, EL_ID_H);
    add_node(EL_VEC_A, EL_KEY_A, EL_ID_A);
    add_node(EL_VEC_B, EL_KEY_B, EL_ID_B);
    add_node(EL_VEC_C, EL_KEY_C, EL_ID_C);
    add_edge(EL_ID_H, EL_ID_A, "ha"); // hub→A
    add_edge(EL_ID_H, EL_ID_B, "hb"); // hub→B
    add_edge(EL_ID_C, EL_ID_H, "ch"); // C→hub
    let (has_circuit, has_path, _sv, _ev, node_count) = gos_runtime::graph_eulerian();
    assert_eq!(node_count, 4);
    assert!(!has_circuit, "two-start/two-end: not a circuit");
    assert!(!has_path,    "two-start/two-end: not a valid path (two start candidates)");
}
