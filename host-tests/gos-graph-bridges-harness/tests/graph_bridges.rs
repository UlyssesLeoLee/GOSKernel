// gos-graph-bridges-harness — V2.86 bridge (cut-edge) detection tests
//
// Verifies `gos_runtime::graph_bridges` — iterative Tarjan disc/low-link DFS
// to identify all cut edges (bridges) in the undirected projection of the live
// directed kernel graph.
//
// A bridge is an edge whose removal increases the number of connected components.
// Bridge condition: low[child] > disc[parent] (strictly greater than).
// Parent tracked by edge-index (not parent-slot) so that two anti-parallel
// directed edges A→B + B→A correctly form a single undirected path, not a bridge.
//
// OS analogy: a single uplink between a leaf switch and the core — its failure
// partitions the routing fabric (like a NIC whose removal isolates a subnet).
//
//  1. Empty graph → bridge_count=0, node_count=0.
//  2. Single isolated node → bridge_count=0, node_count=1.
//  3. Two nodes, one directed edge A→B → 1 bridge (A-B).
//  4. Triangle A→B→C→A → 0 bridges (all edges have a cycle backup).
//  5. Path A→B→C (chain) → 2 bridges: A-B and B-C.
//  6. Anti-parallel pair: A→B and B→A → 0 bridges (two directed = one undirected path).
//  7. Star: centre→4 leaves → 4 bridges (all spokes).
//  8. Square (4-cycle) A→B→C→D→A → 0 bridges.
//  9. Two triangles joined by a bridge edge C→F → 1 bridge (C-F).
// 10. Linear chain A→B→C→D → 3 bridges: A-B, B-C, C-D.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const BR_PLUGIN: PluginId   = PluginId::from_ascii("KL_BR01_00");
const BR_EXEC:   ExecutorId = ExecutorId::from_ascii("br.exec");

const BR_KEY_A: &str = "br.alpha";
const BR_KEY_B: &str = "br.beta";
const BR_KEY_C: &str = "br.gamma";
const BR_KEY_D: &str = "br.delta";
const BR_KEY_E: &str = "br.epsilon";
const BR_KEY_F: &str = "br.zeta";
const BR_KEY_G: &str = "br.eta";
const BR_KEY_H: &str = "br.theta";

const BR_ID_A: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_A);
const BR_ID_B: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_B);
const BR_ID_C: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_C);
const BR_ID_D: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_D);
const BR_ID_E: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_E);
const BR_ID_F: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_F);
const BR_ID_G: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_G);
const BR_ID_H: NodeId = derive_node_id(BR_PLUGIN, BR_KEY_H);

// L4=62 identifies this harness namespace.
const BR_VEC_A: VectorAddress = VectorAddress::new(62, 1, 1, 0);
const BR_VEC_B: VectorAddress = VectorAddress::new(62, 1, 2, 0);
const BR_VEC_C: VectorAddress = VectorAddress::new(62, 1, 3, 0);
const BR_VEC_D: VectorAddress = VectorAddress::new(62, 1, 4, 0);
const BR_VEC_E: VectorAddress = VectorAddress::new(62, 1, 5, 0);
const BR_VEC_F: VectorAddress = VectorAddress::new(62, 1, 6, 0);
const BR_VEC_G: VectorAddress = VectorAddress::new(62, 1, 7, 0);
const BR_VEC_H: VectorAddress = VectorAddress::new(62, 1, 8, 0);

const BR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    BR_PLUGIN,
    name:         "kl-graph-bridges-harness",
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
        executor_id:       BR_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(BR_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(BR_MANIFEST).unwrap();
    g
}

// ── Tests ─────────────────────────────────────────────────────────────────────

// 1. Empty graph → bridge_count=0, node_count=0.
#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_f, _t, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 0, "empty: node_count=0");
    assert_eq!(bridge_count, 0, "empty: no bridges");
}

// 2. Single isolated node → no edges → no bridges.
#[test]
fn test_02_single_isolated_node() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    let (_f, _t, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 1);
    assert_eq!(bridge_count, 0, "isolated node: no edges → no bridges");
}

// 3. A→B: single directed edge is a bridge (removing it isolates both nodes).
#[test]
fn test_03_single_edge_is_bridge() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_edge(BR_ID_A, BR_ID_B, "ab");
    let (from, to, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 2);
    assert_eq!(bridge_count, 1, "single edge is a bridge");
    // Canonicalized: smaller as_u64() first; A < B in L4=62 namespace.
    assert_eq!(from[0], BR_VEC_A, "bridge from: A");
    assert_eq!(to[0],   BR_VEC_B, "bridge to:   B");
}

// 4. Triangle A→B→C→A: every edge has an alternate path → 0 bridges.
#[test]
fn test_04_triangle_no_bridges() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_node(BR_VEC_C, BR_KEY_C, BR_ID_C);
    add_edge(BR_ID_A, BR_ID_B, "ab");
    add_edge(BR_ID_B, BR_ID_C, "bc");
    add_edge(BR_ID_C, BR_ID_A, "ca");
    let (_f, _t, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 3);
    assert_eq!(bridge_count, 0, "triangle: 2-edge-connected, no bridges");
}

// 5. Path A→B→C: both edges are bridges.
//    Removing A-B isolates A from {B,C}; removing B-C isolates {A,B} from C.
#[test]
fn test_05_path_three_nodes_two_bridges() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_node(BR_VEC_C, BR_KEY_C, BR_ID_C);
    add_edge(BR_ID_A, BR_ID_B, "ab");
    add_edge(BR_ID_B, BR_ID_C, "bc");
    let (from, to, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 3);
    assert_eq!(bridge_count, 2, "path A-B-C: two bridges");
    // Sorted by (from.as_u64(), to.as_u64()); A<B<C in L4=62.
    assert_eq!(from[0], BR_VEC_A, "first bridge from: A");
    assert_eq!(to[0],   BR_VEC_B, "first bridge to:   B");
    assert_eq!(from[1], BR_VEC_B, "second bridge from: B");
    assert_eq!(to[1],   BR_VEC_C, "second bridge to:   C");
}

// 6. Anti-parallel pair A→B + B→A: undirected projection is a single path A-B.
//    The par_ei guard skips only the exact tree-edge; the reverse edge is a
//    back-edge that sets low[B]=disc[A], so low[B] <= disc[A] → NOT a bridge.
#[test]
fn test_06_antiparallel_not_a_bridge() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_edge(BR_ID_A, BR_ID_B, "ab");
    add_edge(BR_ID_B, BR_ID_A, "ba");
    let (_f, _t, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 2);
    assert_eq!(bridge_count, 0, "anti-parallel A\u{2194}B: the reverse edge acts as a back-edge, no bridge");
}

// 7. Star: centre H → leaves A, B, C, D (4 spokes).
//    All spokes are bridges — removing any leaf's edge isolates that leaf.
#[test]
fn test_07_star_all_spokes_are_bridges() {
    let _g = setup();
    add_node(BR_VEC_H, BR_KEY_H, BR_ID_H); // centre
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_node(BR_VEC_C, BR_KEY_C, BR_ID_C);
    add_node(BR_VEC_D, BR_KEY_D, BR_ID_D);
    add_edge(BR_ID_H, BR_ID_A, "ha");
    add_edge(BR_ID_H, BR_ID_B, "hb");
    add_edge(BR_ID_H, BR_ID_C, "hc");
    add_edge(BR_ID_H, BR_ID_D, "hd");
    let (_f, _t, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 5);
    assert_eq!(bridge_count, 4, "star: every spoke is a bridge");
}

// 8. Square 4-cycle A→B→C→D→A: every edge has an alternate 3-hop path → 0 bridges.
#[test]
fn test_08_square_no_bridges() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_node(BR_VEC_C, BR_KEY_C, BR_ID_C);
    add_node(BR_VEC_D, BR_KEY_D, BR_ID_D);
    add_edge(BR_ID_A, BR_ID_B, "ab");
    add_edge(BR_ID_B, BR_ID_C, "bc");
    add_edge(BR_ID_C, BR_ID_D, "cd");
    add_edge(BR_ID_D, BR_ID_A, "da");
    let (_f, _t, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 4);
    assert_eq!(bridge_count, 0, "square 4-cycle: 2-edge-connected, no bridges");
}

// 9. Two triangles joined by a single bridge C→F:
//    Triangle 1: A→B, B→C, C→A  (biconnected — no bridges inside)
//    Triangle 2: F→G, G→E, E→F  (biconnected — no bridges inside)
//    Bridge:     C→F             (exactly 1 bridge)
#[test]
fn test_09_two_triangles_one_bridge() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_node(BR_VEC_C, BR_KEY_C, BR_ID_C);
    add_node(BR_VEC_F, BR_KEY_F, BR_ID_F);
    add_node(BR_VEC_G, BR_KEY_G, BR_ID_G);
    add_node(BR_VEC_E, BR_KEY_E, BR_ID_E);
    // Triangle 1
    add_edge(BR_ID_A, BR_ID_B, "ab");
    add_edge(BR_ID_B, BR_ID_C, "bc");
    add_edge(BR_ID_C, BR_ID_A, "ca");
    // Triangle 2
    add_edge(BR_ID_F, BR_ID_G, "fg");
    add_edge(BR_ID_G, BR_ID_E, "ge");
    add_edge(BR_ID_E, BR_ID_F, "ef");
    // Bridge
    add_edge(BR_ID_C, BR_ID_F, "cf");
    let (from, to, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 6);
    assert_eq!(bridge_count, 1, "exactly one bridge: C\u{2500}\u{2500}F");
    // Canonical order: C(offset=3) < F(offset=6) → from=C, to=F.
    assert_eq!(from[0], BR_VEC_C, "bridge from: C");
    assert_eq!(to[0],   BR_VEC_F, "bridge to:   F");
}

// 10. Linear chain A→B→C→D: all 3 edges are bridges.
//     Sorted: (A-B), (B-C), (C-D) in ascending address order.
#[test]
fn test_10_chain_four_nodes_three_bridges() {
    let _g = setup();
    add_node(BR_VEC_A, BR_KEY_A, BR_ID_A);
    add_node(BR_VEC_B, BR_KEY_B, BR_ID_B);
    add_node(BR_VEC_C, BR_KEY_C, BR_ID_C);
    add_node(BR_VEC_D, BR_KEY_D, BR_ID_D);
    add_edge(BR_ID_A, BR_ID_B, "ab");
    add_edge(BR_ID_B, BR_ID_C, "bc");
    add_edge(BR_ID_C, BR_ID_D, "cd");
    let (from, to, bridge_count, node_count) = gos_runtime::graph_bridges::<128>();
    assert_eq!(node_count, 4);
    assert_eq!(bridge_count, 3, "chain A-B-C-D: three bridges");
    assert_eq!(from[0], BR_VEC_A, "bridge 0 from: A");
    assert_eq!(to[0],   BR_VEC_B, "bridge 0 to:   B");
    assert_eq!(from[1], BR_VEC_B, "bridge 1 from: B");
    assert_eq!(to[1],   BR_VEC_C, "bridge 1 to:   C");
    assert_eq!(from[2], BR_VEC_C, "bridge 2 from: C");
    assert_eq!(to[2],   BR_VEC_D, "bridge 2 to:   D");
}
