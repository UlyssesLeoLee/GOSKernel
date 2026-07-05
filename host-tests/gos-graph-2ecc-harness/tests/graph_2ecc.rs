// gos-graph-2ecc-harness -- V2.93 2-edge-connected components tests
//
// Verifies `gos_runtime::graph_2ecc` — Tarjan bridge-detection +
// BFS on non-bridge undirected edges.
//
//  1. Empty graph              -> comp_count=0, node_count=0.
//  2. Single isolated node     -> comp_count=1, node_count=1.
//  3. Single directed edge A→B -> comp_count=2 (A-B is a bridge; {A} and {B}).
//  4. Triangle (directed cycle) A→B→C→A -> comp_count=1 (no bridges).
//  5. Path of 4 nodes A→B→C→D -> comp_count=4 (every edge is a bridge).
//  6. Two triangles joined by one bridge edge -> comp_count=2.
//  7. Two triangles sharing one edge -> comp_count=1 (shared edge not a bridge).
//  8. Star (A→B, A→C, A→D, A→E) -> comp_count=5 (all edges are bridges).
//  9. Two disconnected triangles -> comp_count=2.
// 10. Cross-check: comp_count == bridge_count for path + 1 cross-check with
//     graph_bridges().

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const EC_PLUGIN: PluginId   = PluginId::from_ascii("KL_2ECC_HARN_");
const EC_EXEC:   ExecutorId = ExecutorId::from_ascii("ec.exec");

const EC_KEY_A: &str = "ec.a";
const EC_KEY_B: &str = "ec.b";
const EC_KEY_C: &str = "ec.c";
const EC_KEY_D: &str = "ec.d";
const EC_KEY_E: &str = "ec.e";
const EC_KEY_F: &str = "ec.f";
const EC_KEY_G: &str = "ec.g";
const EC_KEY_H: &str = "ec.h";

const EC_ID_A: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_A);
const EC_ID_B: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_B);
const EC_ID_C: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_C);
const EC_ID_D: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_D);
const EC_ID_E: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_E);
const EC_ID_F: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_F);
const EC_ID_G: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_G);
const EC_ID_H: NodeId = derive_node_id(EC_PLUGIN, EC_KEY_H);

// L4=69 namespace for this harness.
const EC_VEC_A: VectorAddress = VectorAddress::new(69, 1, 1, 0);
const EC_VEC_B: VectorAddress = VectorAddress::new(69, 1, 2, 0);
const EC_VEC_C: VectorAddress = VectorAddress::new(69, 1, 3, 0);
const EC_VEC_D: VectorAddress = VectorAddress::new(69, 1, 4, 0);
const EC_VEC_E: VectorAddress = VectorAddress::new(69, 1, 5, 0);
const EC_VEC_F: VectorAddress = VectorAddress::new(69, 2, 1, 0);
const EC_VEC_G: VectorAddress = VectorAddress::new(69, 2, 2, 0);
const EC_VEC_H: VectorAddress = VectorAddress::new(69, 2, 3, 0);

const EC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    EC_PLUGIN,
    name:         "kl-graph-2ecc-harness",
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
        executor_id:       EC_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(EC_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(EC_MANIFEST).unwrap();
    g
}

/// Returns the component ID assigned to `target`, or None if not found.
fn comp_of(vecs: &[VectorAddress], comp_ids: &[u8], count: usize, target: VectorAddress) -> Option<u8> {
    for i in 0..count {
        if vecs[i] == target { return Some(comp_ids[i]); }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_, _, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 0, "empty graph: no nodes");
    assert_eq!(comp_count, 0, "empty graph: no components");
}

#[test]
fn test_02_single_isolated_node() {
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 1, "one node");
    assert_eq!(comp_count, 1, "isolated node is its own 2ECC");
    assert_eq!(
        comp_of(&vecs, &comp_ids, node_count, EC_VEC_A),
        Some(0),
        "isolated A is in component 0"
    );
}

#[test]
fn test_03_single_directed_edge_is_bridge() {
    // A → B: the single undirected edge A-B is a bridge.
    // So A and B are in separate 2ECCs: {A} and {B}.
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(comp_count, 2, "A-B is a bridge: 2 singleton 2ECCs");
    let ca = comp_of(&vecs, &comp_ids, node_count, EC_VEC_A).expect("A must appear");
    let cb = comp_of(&vecs, &comp_ids, node_count, EC_VEC_B).expect("B must appear");
    assert_ne!(ca, cb, "A and B must be in different components");
}

#[test]
fn test_04_triangle_no_bridges() {
    // A → B → C → A: directed 3-cycle.  No bridge (any edge removal still
    // leaves the other two on a connected path through the third).
    // All three nodes should be in one 2ECC.
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_A, "ca");
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 3, "3 nodes");
    assert_eq!(comp_count, 1, "triangle: all 3 nodes in one 2ECC");
    let ca = comp_of(&vecs, &comp_ids, node_count, EC_VEC_A).unwrap();
    let cb = comp_of(&vecs, &comp_ids, node_count, EC_VEC_B).unwrap();
    let cc = comp_of(&vecs, &comp_ids, node_count, EC_VEC_C).unwrap();
    assert_eq!(ca, cb, "A and B in same component");
    assert_eq!(cb, cc, "B and C in same component");
}

#[test]
fn test_05_path_of_four_all_bridges() {
    // A → B → C → D: a path.  Every edge is a bridge.
    // Result: 4 singleton 2ECCs, all with distinct comp IDs.
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_D, "cd");
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(comp_count, 4, "path of 4: every edge is a bridge -> 4 singleton 2ECCs");
    let ca = comp_of(&vecs, &comp_ids, node_count, EC_VEC_A).unwrap();
    let cb = comp_of(&vecs, &comp_ids, node_count, EC_VEC_B).unwrap();
    let cc = comp_of(&vecs, &comp_ids, node_count, EC_VEC_C).unwrap();
    let cd = comp_of(&vecs, &comp_ids, node_count, EC_VEC_D).unwrap();
    assert_ne!(ca, cb, "A and B in different components");
    assert_ne!(cb, cc, "B and C in different components");
    assert_ne!(cc, cd, "C and D in different components");
    assert_ne!(ca, cc, "A and C in different components");
}

#[test]
fn test_06_two_triangles_joined_by_bridge() {
    // Triangle {A,B,C}: A→B, B→C, C→A.
    // Triangle {D,E,F}: D→E, E→F, F→D.
    // Bridge: C→D (connects the two triangles).
    // Expected: 2 components — {A,B,C} and {D,E,F}.
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_F, EC_KEY_F, EC_ID_F);
    add_node(EC_VEC_G, EC_KEY_G, EC_ID_G);
    add_node(EC_VEC_H, EC_KEY_H, EC_ID_H);
    // Triangle 1
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_A, "ca");
    // Triangle 2
    add_edge(EC_ID_F, EC_ID_G, "fg");
    add_edge(EC_ID_G, EC_ID_H, "gh");
    add_edge(EC_ID_H, EC_ID_F, "hf");
    // Bridge between the two triangles
    add_edge(EC_ID_C, EC_ID_F, "cf");
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 6, "6 nodes");
    assert_eq!(comp_count, 2, "two triangles + one bridge = 2 components");
    let ca = comp_of(&vecs, &comp_ids, node_count, EC_VEC_A).unwrap();
    let cb = comp_of(&vecs, &comp_ids, node_count, EC_VEC_B).unwrap();
    let cc = comp_of(&vecs, &comp_ids, node_count, EC_VEC_C).unwrap();
    let cf = comp_of(&vecs, &comp_ids, node_count, EC_VEC_F).unwrap();
    let cg = comp_of(&vecs, &comp_ids, node_count, EC_VEC_G).unwrap();
    let ch = comp_of(&vecs, &comp_ids, node_count, EC_VEC_H).unwrap();
    assert_eq!(ca, cb, "A,B same component");
    assert_eq!(cb, cc, "B,C same component");
    assert_eq!(cf, cg, "F,G same component");
    assert_eq!(cg, ch, "G,H same component");
    assert_ne!(ca, cf, "Triangle-1 and Triangle-2 are different components");
}

#[test]
fn test_07_two_triangles_sharing_one_edge() {
    // A-B-C-A (triangle 1) and A-B-D-A (triangle 2) share edge A-B.
    // Because A-B is on two independent cycles, it is NOT a bridge.
    // All four nodes A,B,C,D should be in one single 2ECC.
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    // Triangle 1: A→B, B→C, C→A
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_A, "ca");
    // Triangle 2 shares A-B; add B→D and D→A
    add_edge(EC_ID_B, EC_ID_D, "bd");
    add_edge(EC_ID_D, EC_ID_A, "da");
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(comp_count, 1, "shared edge A-B is on two cycles: no bridge, one 2ECC");
    let ca = comp_of(&vecs, &comp_ids, node_count, EC_VEC_A).unwrap();
    let cb = comp_of(&vecs, &comp_ids, node_count, EC_VEC_B).unwrap();
    let cc = comp_of(&vecs, &comp_ids, node_count, EC_VEC_C).unwrap();
    let cd = comp_of(&vecs, &comp_ids, node_count, EC_VEC_D).unwrap();
    assert_eq!(ca, cb, "A and B in same 2ECC");
    assert_eq!(ca, cc, "A and C in same 2ECC");
    assert_eq!(ca, cd, "A and D in same 2ECC");
}

#[test]
fn test_08_star_all_bridges() {
    // Star: center A, leaves B,C,D,E.  Edges: A→B, A→C, A→D, A→E.
    // Every spoke is a bridge.  Result: 5 singleton 2ECCs.
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    add_node(EC_VEC_E, EC_KEY_E, EC_ID_E);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_A, EC_ID_C, "ac");
    add_edge(EC_ID_A, EC_ID_D, "ad");
    add_edge(EC_ID_A, EC_ID_E, "ae");
    let (_, _, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 5, "5 nodes");
    assert_eq!(comp_count, 5, "star: every spoke is a bridge -> 5 singleton 2ECCs");
}

#[test]
fn test_09_two_disconnected_triangles() {
    // {A,B,C} and {D,E,F} are completely disconnected triangles.
    // Expected: 2 components (each triangle is one 2ECC).
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    add_node(EC_VEC_E, EC_KEY_E, EC_ID_E);
    add_node(EC_VEC_F, EC_KEY_F, EC_ID_F);
    // Triangle 1
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_A, "ca");
    // Triangle 2
    add_edge(EC_ID_D, EC_ID_E, "de");
    add_edge(EC_ID_E, EC_ID_F, "ef");
    add_edge(EC_ID_F, EC_ID_D, "fd");
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();
    assert_eq!(node_count, 6, "6 nodes");
    assert_eq!(comp_count, 2, "two disconnected triangles: 2 components");
    let ca = comp_of(&vecs, &comp_ids, node_count, EC_VEC_A).unwrap();
    let cd = comp_of(&vecs, &comp_ids, node_count, EC_VEC_D).unwrap();
    assert_ne!(ca, cd, "triangle-1 and triangle-2 are in different components");
    // All of triangle-1 same component
    let cb = comp_of(&vecs, &comp_ids, node_count, EC_VEC_B).unwrap();
    let cc = comp_of(&vecs, &comp_ids, node_count, EC_VEC_C).unwrap();
    assert_eq!(ca, cb, "A,B same triangle component");
    assert_eq!(ca, cc, "A,C same triangle component");
    // All of triangle-2 same component
    let ce = comp_of(&vecs, &comp_ids, node_count, EC_VEC_E).unwrap();
    let cf = comp_of(&vecs, &comp_ids, node_count, EC_VEC_F).unwrap();
    assert_eq!(cd, ce, "D,E same triangle component");
    assert_eq!(cd, cf, "D,F same triangle component");
}

#[test]
fn test_10_cross_check_with_graph_bridges() {
    // For a path A→B→C→D:
    //   - graph_bridges() should return 3 bridges (A-B, B-C, C-D).
    //   - graph_2ecc() should return 4 components (one per node).
    //   - Invariant: for a connected graph, comp_count = bridge_count + 1.
    // Also verify node membership: each node is in a unique component.
    let _g = setup();
    add_node(EC_VEC_A, EC_KEY_A, EC_ID_A);
    add_node(EC_VEC_B, EC_KEY_B, EC_ID_B);
    add_node(EC_VEC_C, EC_KEY_C, EC_ID_C);
    add_node(EC_VEC_D, EC_KEY_D, EC_ID_D);
    add_edge(EC_ID_A, EC_ID_B, "ab");
    add_edge(EC_ID_B, EC_ID_C, "bc");
    add_edge(EC_ID_C, EC_ID_D, "cd");

    let (_, _, bridge_count, _) = gos_runtime::graph_bridges::<128>();
    let (vecs, comp_ids, node_count, comp_count) = gos_runtime::graph_2ecc::<128>();

    assert_eq!(bridge_count, 3, "path of 4: 3 bridges");
    assert_eq!(node_count,  4, "4 nodes");
    assert_eq!(comp_count,  4, "4 singleton 2ECCs");
    // For a connected graph: comp_count == bridge_count + 1
    assert_eq!(
        comp_count, bridge_count + 1,
        "connected graph invariant: comp_count = bridge_count + 1"
    );
    // All 4 nodes have distinct component IDs
    let ca = comp_of(&vecs, &comp_ids, node_count, EC_VEC_A).unwrap();
    let cb = comp_of(&vecs, &comp_ids, node_count, EC_VEC_B).unwrap();
    let cc = comp_of(&vecs, &comp_ids, node_count, EC_VEC_C).unwrap();
    let cd = comp_of(&vecs, &comp_ids, node_count, EC_VEC_D).unwrap();
    let ids = [ca, cb, cc, cd];
    for i in 0..4 {
        for j in (i + 1)..4 {
            assert_ne!(ids[i], ids[j], "nodes[{i}] and [{j}] must be in different components");
        }
    }
}
