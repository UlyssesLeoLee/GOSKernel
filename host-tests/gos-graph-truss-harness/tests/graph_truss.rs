// gos-graph-truss-harness -- V2.94 k-truss decomposition tests
//
// Verifies `gos_runtime::graph_truss` — edge-peeling truss decomposition.
// Trussness of a node = max k such that the node has an incident edge in the
// k-truss (the maximal subgraph where every edge is in ≥ k-2 triangles).
//
//  1. Empty graph              → node_count=0, max_trussness=0.
//  2. Single isolated node     → trussness=0.
//  3. Single directed edge A→B → trussness=2 for both (edge, no triangle).
//  4. Triangle A→B→C→A        → trussness=3 for all (each edge has 1 triangle).
//  5. K4 (fully bidirectional) → trussness=4 for all (each edge has 2 triangles).
//  6. Two triangles sharing edge A-B → trussness=3 for all nodes.
//  7. Path A→B→C (no triangles) → trussness=2 for all nodes with edges.
//  8. Star (no triangles)       → trussness=2 for all nodes.
//  9. Mixed: triangle + isolated node → triangle nodes=3, isolated=0.
// 10. Cross-check k-truss vs k-core: for K4, max_truss=4 ≥ max_core+1 = 4.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const TR_PLUGIN: PluginId   = PluginId::from_ascii("KL_TRUSS_HARN_");
const TR_EXEC:   ExecutorId = ExecutorId::from_ascii("tr.exec");

const TR_KEY_A: &str = "tr.a";
const TR_KEY_B: &str = "tr.b";
const TR_KEY_C: &str = "tr.c";
const TR_KEY_D: &str = "tr.d";
const TR_KEY_E: &str = "tr.e";

const TR_ID_A: NodeId = derive_node_id(TR_PLUGIN, TR_KEY_A);
const TR_ID_B: NodeId = derive_node_id(TR_PLUGIN, TR_KEY_B);
const TR_ID_C: NodeId = derive_node_id(TR_PLUGIN, TR_KEY_C);
const TR_ID_D: NodeId = derive_node_id(TR_PLUGIN, TR_KEY_D);
const TR_ID_E: NodeId = derive_node_id(TR_PLUGIN, TR_KEY_E);

// L4=70 namespace for this harness.
const TR_VEC_A: VectorAddress = VectorAddress::new(70, 1, 1, 0);
const TR_VEC_B: VectorAddress = VectorAddress::new(70, 1, 2, 0);
const TR_VEC_C: VectorAddress = VectorAddress::new(70, 1, 3, 0);
const TR_VEC_D: VectorAddress = VectorAddress::new(70, 1, 4, 0);
const TR_VEC_E: VectorAddress = VectorAddress::new(70, 1, 5, 0);

const TR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    TR_PLUGIN,
    name:         "kl-graph-truss-harness",
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
        executor_id:       TR_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(TR_PLUGIN, vec, node_spec(key, id)).unwrap();
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

fn add_bidir(a: NodeId, b: NodeId, fwd: &'static str, rev: &'static str) {
    add_edge(a, b, fwd);
    add_edge(b, a, rev);
}

fn setup() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(TR_MANIFEST).unwrap();
    g
}

/// Returns the trussness of `target`, or None if not found.
fn truss_of(vecs: &[VectorAddress], truss: &[u8], count: usize, target: VectorAddress) -> Option<u8> {
    for i in 0..count {
        if vecs[i] == target { return Some(truss[i]); }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_, _, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 0, "empty graph: no nodes");
    assert_eq!(max_truss, 0, "empty graph: max_trussness=0");
}

#[test]
fn test_02_single_isolated_node() {
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 1, "1 node");
    assert_eq!(max_truss, 0, "isolated node: max_trussness=0");
    assert_eq!(
        truss_of(&vecs, &truss, node_count, TR_VEC_A),
        Some(0),
        "isolated A: trussness=0"
    );
}

#[test]
fn test_03_single_directed_edge_no_triangle() {
    // A→B: one directed edge, no triangles possible.
    // Both nodes have an edge but support=0, so trussness=2 (2-truss baseline).
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(max_truss, 2, "single edge: max_trussness=2");
    let ta = truss_of(&vecs, &truss, node_count, TR_VEC_A).expect("A must appear");
    let tb = truss_of(&vecs, &truss, node_count, TR_VEC_B).expect("B must appear");
    assert_eq!(ta, 2, "A trussness=2 (edge, no triangle)");
    assert_eq!(tb, 2, "B trussness=2 (edge, no triangle)");
}

#[test]
fn test_04_triangle_trussness_three() {
    // Directed 3-cycle A→B→C→A.  Undirected edges: A-B, B-C, A-C.
    // Each edge has exactly 1 triangle (with the third node).
    // k=3 (thresh=1): all edges have support=1 ≥ 1. They are in the 3-truss.
    // k=4 (thresh=2): all edges have support=1 < 2. Removed. Trussness = 3.
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_node(TR_VEC_C, TR_KEY_C, TR_ID_C);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_C, TR_ID_A, "ca");
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 3, "3 nodes");
    assert_eq!(max_truss, 3, "triangle: max_trussness=3");
    let ta = truss_of(&vecs, &truss, node_count, TR_VEC_A).unwrap();
    let tb = truss_of(&vecs, &truss, node_count, TR_VEC_B).unwrap();
    let tc = truss_of(&vecs, &truss, node_count, TR_VEC_C).unwrap();
    assert_eq!(ta, 3, "A trussness=3");
    assert_eq!(tb, 3, "B trussness=3");
    assert_eq!(tc, 3, "C trussness=3");
}

#[test]
fn test_05_k4_trussness_four() {
    // K4: fully bidirectional complete graph on A,B,C,D.
    // Each undirected edge has 2 common neighbors (the other 2 nodes), so support=2.
    // k=3 (thresh=1): all sup=2 ≥ 1. No removals.
    // k=4 (thresh=2): all sup=2 ≥ 2. No removals. Survive → trussness=4.
    // k=5 (thresh=3): all sup=2 < 3. Removed. Trussness stays at 4 from prev round.
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_node(TR_VEC_C, TR_KEY_C, TR_ID_C);
    add_node(TR_VEC_D, TR_KEY_D, TR_ID_D);
    add_bidir(TR_ID_A, TR_ID_B, "abf", "abr");
    add_bidir(TR_ID_A, TR_ID_C, "acf", "acr");
    add_bidir(TR_ID_A, TR_ID_D, "adf", "adr");
    add_bidir(TR_ID_B, TR_ID_C, "bcf", "bcr");
    add_bidir(TR_ID_B, TR_ID_D, "bdf", "bdr");
    add_bidir(TR_ID_C, TR_ID_D, "cdf", "cdr");
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(max_truss, 4, "K4: max_trussness=4 (each edge in 2 triangles)");
    for (vec, label) in [(TR_VEC_A, "A"), (TR_VEC_B, "B"), (TR_VEC_C, "C"), (TR_VEC_D, "D")] {
        let t = truss_of(&vecs, &truss, node_count, vec).unwrap();
        assert_eq!(t, 4, "{label} trussness=4");
    }
}

#[test]
fn test_06_two_triangles_sharing_edge() {
    // Triangle ABC (A-B, B-C, A-C) and triangle ABD (A-D, B-D).
    // Edge A-B is in 2 triangles; all others in 1.
    // But after cascade peeling, ALL edges end up with trussness=3.
    // (A-C, B-C, A-D, B-D have support=1; removing them reduces A-B's support to 0,
    //  so A-B also gets removed at k=4 level with trussness=3.)
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_node(TR_VEC_C, TR_KEY_C, TR_ID_C);
    add_node(TR_VEC_D, TR_KEY_D, TR_ID_D);
    // Triangle ABC
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_C, TR_ID_A, "ca");
    // Triangle ABD (shares A-B)
    add_edge(TR_ID_A, TR_ID_D, "ad");
    add_edge(TR_ID_D, TR_ID_B, "db");
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(max_truss, 3, "two triangles sharing edge: max_trussness=3");
    for (vec, label) in [(TR_VEC_A, "A"), (TR_VEC_B, "B"), (TR_VEC_C, "C"), (TR_VEC_D, "D")] {
        let t = truss_of(&vecs, &truss, node_count, vec).unwrap();
        assert_eq!(t, 3, "{label} trussness=3 (triangle but not 4-truss)");
    }
}

#[test]
fn test_07_path_no_triangles() {
    // Path A→B→C: edges A-B and B-C, no triangles.
    // All edges have support=0 → trussness=2.
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_node(TR_VEC_C, TR_KEY_C, TR_ID_C);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 3, "3 nodes");
    assert_eq!(max_truss, 2, "path of 3: max_trussness=2 (edges, no triangles)");
    let ta = truss_of(&vecs, &truss, node_count, TR_VEC_A).unwrap();
    let tb = truss_of(&vecs, &truss, node_count, TR_VEC_B).unwrap();
    let tc = truss_of(&vecs, &truss, node_count, TR_VEC_C).unwrap();
    assert_eq!(ta, 2, "A trussness=2");
    assert_eq!(tb, 2, "B trussness=2");
    assert_eq!(tc, 2, "C trussness=2");
}

#[test]
fn test_08_star_no_triangles() {
    // Star: center A, leaves B,C,D.  All edges have support=0 → trussness=2.
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_node(TR_VEC_C, TR_KEY_C, TR_ID_C);
    add_node(TR_VEC_D, TR_KEY_D, TR_ID_D);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_A, TR_ID_C, "ac");
    add_edge(TR_ID_A, TR_ID_D, "ad");
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(max_truss, 2, "star: max_trussness=2 (no triangles)");
    for (vec, label) in [(TR_VEC_A, "A"), (TR_VEC_B, "B"), (TR_VEC_C, "C"), (TR_VEC_D, "D")] {
        let t = truss_of(&vecs, &truss, node_count, vec).unwrap();
        assert_eq!(t, 2, "{label} trussness=2");
    }
}

#[test]
fn test_09_triangle_plus_isolated_node() {
    // Triangle A→B→C→A plus isolated node E.
    // Triangle nodes: trussness=3. Isolated E: trussness=0.
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_node(TR_VEC_C, TR_KEY_C, TR_ID_C);
    add_node(TR_VEC_E, TR_KEY_E, TR_ID_E);
    add_edge(TR_ID_A, TR_ID_B, "ab");
    add_edge(TR_ID_B, TR_ID_C, "bc");
    add_edge(TR_ID_C, TR_ID_A, "ca");
    let (vecs, truss, node_count, max_truss) = gos_runtime::graph_truss::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(max_truss, 3, "triangle+isolated: max_trussness=3");
    let ta = truss_of(&vecs, &truss, node_count, TR_VEC_A).unwrap();
    let tb = truss_of(&vecs, &truss, node_count, TR_VEC_B).unwrap();
    let tc = truss_of(&vecs, &truss, node_count, TR_VEC_C).unwrap();
    let te = truss_of(&vecs, &truss, node_count, TR_VEC_E).unwrap();
    assert_eq!(ta, 3, "A trussness=3");
    assert_eq!(tb, 3, "B trussness=3");
    assert_eq!(tc, 3, "C trussness=3");
    assert_eq!(te, 0, "E trussness=0 (isolated)");
}

#[test]
fn test_10_truss_strictly_finer_than_kcore() {
    // K4 has k-core max = 3 (every node has degree 3 = max_core).
    // K4 has k-truss max = 4 (every edge in 2 triangles).
    // Invariant: max_trussness == max_coreness + 1 for K4.
    // Also: output is sorted by trussness descending (all equal here → sort by vector).
    let _g = setup();
    add_node(TR_VEC_A, TR_KEY_A, TR_ID_A);
    add_node(TR_VEC_B, TR_KEY_B, TR_ID_B);
    add_node(TR_VEC_C, TR_KEY_C, TR_ID_C);
    add_node(TR_VEC_D, TR_KEY_D, TR_ID_D);
    add_bidir(TR_ID_A, TR_ID_B, "xabf", "xabr");
    add_bidir(TR_ID_A, TR_ID_C, "xacf", "xacr");
    add_bidir(TR_ID_A, TR_ID_D, "xadf", "xadr");
    add_bidir(TR_ID_B, TR_ID_C, "xbcf", "xbcr");
    add_bidir(TR_ID_B, TR_ID_D, "xbdf", "xbdr");
    add_bidir(TR_ID_C, TR_ID_D, "xcdf", "xcdr");

    let (_, kcore_vals, kc_n, max_core) = gos_runtime::graph_kcore::<128>();
    let (vecs, truss_vals, tr_n, max_truss) = gos_runtime::graph_truss::<128>();

    assert_eq!(kc_n, 4, "kcore: 4 nodes");
    assert_eq!(tr_n, 4, "truss: 4 nodes");
    assert_eq!(max_core, 3, "K4 max_coreness=3");
    assert_eq!(max_truss, 4, "K4 max_trussness=4");
    assert!(
        max_truss as usize == max_core as usize + 1,
        "K4: max_truss == max_core + 1 (truss strictly finer than core)"
    );
    // All nodes have equal trussness=4; output sorted ascending by vector as_u64().
    assert_eq!(tr_n, 4);
    for i in 0..tr_n {
        assert_eq!(truss_vals[i], 4, "node[{i}] trussness=4");
        assert_eq!(kcore_vals[i], 3, "node[{i}] coreness=3");
    }
    // Ascending vector order tie-break
    for i in 1..tr_n {
        assert!(
            vecs[i - 1].as_u64() <= vecs[i].as_u64(),
            "output sorted ascending by vector (equal trussness tie-break)"
        );
    }
}
