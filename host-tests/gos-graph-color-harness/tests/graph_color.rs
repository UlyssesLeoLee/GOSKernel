// gos-graph-color-harness — V2.47 Welsh-Powell greedy graph coloring tests
//
// Verifies `gos_runtime::graph_color` — greedy chromatic coloring of the
// undirected projection of the GOS kernel graph.
//
// Algorithm summary (Welsh-Powell greedy coloring):
//   1. Compute total (undirected) degree for every live node.
//   2. Sort nodes in descending degree order.
//   3. Iterate in that order; assign each node the smallest color index (≥0)
//      not already used by any directly adjacent (undirected) neighbor.
//
// Return value: (vecs, colors, node_count, chromatic_number)
//   vecs[i]             — node vector in descending-degree order
//   colors[i]           — color index (u8, 0-based)
//   node_count          — total live nodes
//   chromatic_number    — number of distinct colors used (max_color + 1)
//
// Key invariants:
//   Empty graph              → node_count=0, chromatic_number=0
//   Single node              → chromatic_number=1, color=0
//   K₂ (single edge A→B)    → chromatic_number=2 (A and B get different colors)
//   Path A→B→C              → chromatic_number=2 (alternating)
//   K₃ (triangle)           → chromatic_number=3 (each vertex a different color)
//   Bipartite graph          → chromatic_number≤2
//   Two isolated nodes       → chromatic_number=1 (no edges → color 0 for all)
//
// Test matrix:
//  1.  Empty graph                   → node_count=0, chromatic=0
//  2.  Single isolated node          → chromatic=1, color=0
//  3.  Two isolated nodes (no edges) → chromatic=1, all colors=0
//  4.  K₂: single edge A→B          → chromatic=2, A≠B colors
//  5.  Path A→B→C                   → chromatic=2, alternating colors
//  6.  K₃ triangle                  → chromatic=3, all different colors
//  7.  K₄ complete graph            → chromatic=4, all different colors
//  8.  Bipartite K_{2,2}            → chromatic=2 (≤2)
//  9.  Validity: no two adjacent nodes share a color
// 10.  Descending degree order: highest-degree node first in output

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const COL_PLUGIN: PluginId   = PluginId::from_ascii("KL_COL_00");
const COL_EXEC:   ExecutorId = ExecutorId::from_ascii("col.exec0");

const COL_KEY_A: &str = "col.alpha";
const COL_KEY_B: &str = "col.beta";
const COL_KEY_C: &str = "col.gamma";
const COL_KEY_D: &str = "col.delta";
const COL_ID_A: NodeId = derive_node_id(COL_PLUGIN, COL_KEY_A);
const COL_ID_B: NodeId = derive_node_id(COL_PLUGIN, COL_KEY_B);
const COL_ID_C: NodeId = derive_node_id(COL_PLUGIN, COL_KEY_C);
const COL_ID_D: NodeId = derive_node_id(COL_PLUGIN, COL_KEY_D);

const COL_VEC_A: VectorAddress = VectorAddress::new(24, 1, 1, 0);
const COL_VEC_B: VectorAddress = VectorAddress::new(24, 1, 2, 0);
const COL_VEC_C: VectorAddress = VectorAddress::new(24, 1, 3, 0);
const COL_VEC_D: VectorAddress = VectorAddress::new(24, 1, 4, 0);

const COL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    COL_PLUGIN,
    name:         "kl-graph-color-harness",
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
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       COL_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(COL_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(COL_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn color_of(
    vecs: &[VectorAddress], colors: &[u8], total: usize, target: VectorAddress,
) -> Option<u8> {
    for i in 0..total { if vecs[i] == target { return Some(colors[i]); } }
    None
}

// ── 1. Empty graph → node_count=0, chromatic_number=0 ───────────────────────

#[test]
fn empty_graph_no_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 0, "empty: 0 nodes");
    assert_eq!(chromatic, 0, "empty: chromatic number = 0");
}

// ── 2. Single isolated node → chromatic=1, color=0 ──────────────────────────

#[test]
fn single_node_color_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 1, "1 node");
    assert_eq!(chromatic, 1, "1 node → chromatic = 1");
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A present");
    assert_eq!(ca, 0, "single node must be color 0");
}

// ── 3. Two isolated nodes (no edges) → chromatic=1, both color 0 ────────────

#[test]
fn two_isolated_nodes_same_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(chromatic, 1, "no edges → chromatic = 1");
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A present");
    let cb = color_of(&vecs, &colors, total, COL_VEC_B).expect("B present");
    assert_eq!(ca, 0, "isolated A: color 0");
    assert_eq!(cb, 0, "isolated B: color 0 (no conflict)");
}

// ── 4. K₂: single edge A→B → chromatic=2, A and B different colors ──────────

#[test]
fn k2_two_colors() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    add_edge(COL_ID_A, COL_ID_B, "col.ab.t4");
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(chromatic, 2, "K₂ requires 2 colors");
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A present");
    let cb = color_of(&vecs, &colors, total, COL_VEC_B).expect("B present");
    assert_ne!(ca, cb, "adjacent A and B must have different colors");
}

// ── 5. Path A→B→C → chromatic=2 (alternating) ────────────────────────────────

#[test]
fn path_abc_two_colors() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    add_node(COL_VEC_C, COL_KEY_C, COL_ID_C, 0xC003);
    add_edge(COL_ID_A, COL_ID_B, "col.ab.t5");
    add_edge(COL_ID_B, COL_ID_C, "col.bc.t5");
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert!(chromatic <= 2, "path graph is bipartite → chromatic ≤ 2");
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A present");
    let cb = color_of(&vecs, &colors, total, COL_VEC_B).expect("B present");
    let cc = color_of(&vecs, &colors, total, COL_VEC_C).expect("C present");
    assert_ne!(ca, cb, "A and B adjacent → different colors");
    assert_ne!(cb, cc, "B and C adjacent → different colors");
}

// ── 6. K₃ triangle → chromatic=3 ─────────────────────────────────────────────

#[test]
fn triangle_three_colors() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    add_node(COL_VEC_C, COL_KEY_C, COL_ID_C, 0xC003);
    // Full triangle: A↔B↔C↔A (directed but treated undirected for coloring)
    add_edge(COL_ID_A, COL_ID_B, "col.ab.t6");
    add_edge(COL_ID_B, COL_ID_C, "col.bc.t6");
    add_edge(COL_ID_C, COL_ID_A, "col.ca.t6");
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(chromatic, 3, "K₃ requires 3 colors");
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A present");
    let cb = color_of(&vecs, &colors, total, COL_VEC_B).expect("B present");
    let cc = color_of(&vecs, &colors, total, COL_VEC_C).expect("C present");
    assert_ne!(ca, cb, "A-B adjacent");
    assert_ne!(cb, cc, "B-C adjacent");
    assert_ne!(ca, cc, "A-C adjacent");
}

// ── 7. K₄ complete graph → chromatic=4 ───────────────────────────────────────

#[test]
fn k4_four_colors() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    add_node(COL_VEC_C, COL_KEY_C, COL_ID_C, 0xC003);
    add_node(COL_VEC_D, COL_KEY_D, COL_ID_D, 0xC004);
    // All six directed edges (symmetric = complete undirected K₄)
    add_edge(COL_ID_A, COL_ID_B, "col.ab.t7");
    add_edge(COL_ID_A, COL_ID_C, "col.ac.t7");
    add_edge(COL_ID_A, COL_ID_D, "col.ad.t7");
    add_edge(COL_ID_B, COL_ID_C, "col.bc.t7");
    add_edge(COL_ID_B, COL_ID_D, "col.bd.t7");
    add_edge(COL_ID_C, COL_ID_D, "col.cd.t7");
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert_eq!(chromatic, 4, "K₄ requires 4 colors");
    // All four colors must be distinct.
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A");
    let cb = color_of(&vecs, &colors, total, COL_VEC_B).expect("B");
    let cc = color_of(&vecs, &colors, total, COL_VEC_C).expect("C");
    let cd = color_of(&vecs, &colors, total, COL_VEC_D).expect("D");
    let mut all = [ca, cb, cc, cd];
    all.sort_unstable();
    assert_eq!(all, [0, 1, 2, 3], "K₄: all four color indices 0-3 used");
}

// ── 8. Bipartite K_{2,2}: chromatic ≤ 2 ─────────────────────────────────────

#[test]
fn bipartite_k22_two_colors() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Set 1: A, B  /  Set 2: C, D
    // Edges only cross sets: A→C, A→D, B→C, B→D
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    add_node(COL_VEC_C, COL_KEY_C, COL_ID_C, 0xC003);
    add_node(COL_VEC_D, COL_KEY_D, COL_ID_D, 0xC004);
    add_edge(COL_ID_A, COL_ID_C, "col.ac.t8");
    add_edge(COL_ID_A, COL_ID_D, "col.ad.t8");
    add_edge(COL_ID_B, COL_ID_C, "col.bc.t8");
    add_edge(COL_ID_B, COL_ID_D, "col.bd.t8");
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert!(chromatic <= 2, "bipartite graph chromatic ≤ 2; got {chromatic}");
    // Cross-set adjacency: A≠C, A≠D, B≠C, B≠D.
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A");
    let cb = color_of(&vecs, &colors, total, COL_VEC_B).expect("B");
    let cc = color_of(&vecs, &colors, total, COL_VEC_C).expect("C");
    let cd = color_of(&vecs, &colors, total, COL_VEC_D).expect("D");
    assert_ne!(ca, cc, "A-C adjacent");
    assert_ne!(ca, cd, "A-D adjacent");
    assert_ne!(cb, cc, "B-C adjacent");
    assert_ne!(cb, cd, "B-D adjacent");
}

// ── 9. Validity: no two adjacent nodes share a color (any topology) ──────────

#[test]
fn validity_no_adjacent_same_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Mixed graph: A→B, B→C, C→A (triangle) + D isolated
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    add_node(COL_VEC_C, COL_KEY_C, COL_ID_C, 0xC003);
    add_node(COL_VEC_D, COL_KEY_D, COL_ID_D, 0xC004);
    add_edge(COL_ID_A, COL_ID_B, "col.ab.t9");
    add_edge(COL_ID_B, COL_ID_C, "col.bc.t9");
    add_edge(COL_ID_C, COL_ID_A, "col.ca.t9");
    // D is isolated — no edges.
    let (vecs, colors, total, _chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 4);

    // Verify every directed edge's endpoints have different colors.
    let edges_to_check = [
        (COL_VEC_A, COL_VEC_B),
        (COL_VEC_B, COL_VEC_C),
        (COL_VEC_C, COL_VEC_A),
    ];
    for (fv, tv) in edges_to_check {
        let cf = color_of(&vecs, &colors, total, fv).expect("from node present");
        let ct = color_of(&vecs, &colors, total, tv).expect("to node present");
        assert_ne!(cf, ct, "adjacent nodes ({fv:?}→{tv:?}) must have different colors");
    }
}

// ── 10. Descending degree order: highest-degree node appears first ────────────

#[test]
fn descending_degree_order() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Star graph: B is center (degree 3 undirected), A/C/D are leaves (degree 1).
    // Edges: B→A, B→C, B→D.
    add_node(COL_VEC_A, COL_KEY_A, COL_ID_A, 0xC001);
    add_node(COL_VEC_B, COL_KEY_B, COL_ID_B, 0xC002);
    add_node(COL_VEC_C, COL_KEY_C, COL_ID_C, 0xC003);
    add_node(COL_VEC_D, COL_KEY_D, COL_ID_D, 0xC004);
    add_edge(COL_ID_B, COL_ID_A, "col.ba.t10");
    add_edge(COL_ID_B, COL_ID_C, "col.bc.t10");
    add_edge(COL_ID_B, COL_ID_D, "col.bd.t10");
    let (vecs, colors, total, chromatic) = gos_runtime::graph_color::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert_eq!(chromatic, 2, "star graph K_{{1,3}} is bipartite → 2 colors");
    // The center node B (highest degree = 3) must appear at index 0.
    assert_eq!(vecs[0], COL_VEC_B, "center node B must be first (highest degree)");
    // B gets color 0 (first processed); leaves A, C, D must get a different color.
    let cb = colors[0];
    assert_eq!(cb, 0, "center B: color 0 (first processed)");
    let ca = color_of(&vecs, &colors, total, COL_VEC_A).expect("A");
    let cc = color_of(&vecs, &colors, total, COL_VEC_C).expect("C");
    let cd = color_of(&vecs, &colors, total, COL_VEC_D).expect("D");
    assert_ne!(ca, cb, "leaf A ≠ center B color");
    assert_ne!(cc, cb, "leaf C ≠ center B color");
    assert_ne!(cd, cb, "leaf D ≠ center B color");
    // Leaves are not adjacent to each other → they can share a color.
    assert_eq!(ca, cc, "non-adjacent leaves A and C may share a color");
    assert_eq!(ca, cd, "non-adjacent leaves A and D may share a color");
}
