// gos-graph-bipartite-harness — V2.37 bipartite-check API tests
//
// Verifies `gos_runtime::graph_bipartite` — BFS 2-coloring on the undirected
// projection of the live directed graph.  A graph is bipartite iff it has no
// odd-length cycle; equivalently, every edge crosses between set A and set B.
// Analogous to `bipartite_check` in graph libraries or testing whether a
// dependency graph can be split into two non-conflicting scheduling tiers.
//
//  1. Empty graph (no nodes) → bipartite (vacuously true).
//  2. Single isolated node → bipartite (no edges).
//  3. Single directed edge A→B → bipartite: {A}, {B}.
//  4. Path A→B→C → bipartite: {A,C} vs {B}.
//  5. Triangle A→B→C→A → NOT bipartite (odd cycle length 3).
//  6. 4-cycle A→B→C→D→A → bipartite: {A,C} vs {B,D}.
//  7. 4-cycle + chord A→C → NOT bipartite (creates odd 3-cycle A-B-C-A).
//  8. Two disconnected bipartite components → bipartite.
//  9. Star: centre→{A,B,C,D} → bipartite: {centre} vs {A,B,C,D}.
// 10. Color assignment: A and C in same set, B in other, for path A→B→C.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const BIP_PLUGIN: PluginId   = PluginId::from_ascii("KL_BIPA_00");
const BIP_EXEC:   ExecutorId = ExecutorId::from_ascii("bip.exec");

const BIP_KEY_A: &str = "bip.alpha";
const BIP_KEY_B: &str = "bip.beta";
const BIP_KEY_C: &str = "bip.gamma";
const BIP_KEY_D: &str = "bip.delta";
const BIP_KEY_E: &str = "bip.epsilon"; // centre for star test
const BIP_KEY_F: &str = "bip.zeta";   // disconnected component node 1
const BIP_KEY_G: &str = "bip.eta";    // disconnected component node 2

const BIP_ID_A: NodeId = derive_node_id(BIP_PLUGIN, BIP_KEY_A);
const BIP_ID_B: NodeId = derive_node_id(BIP_PLUGIN, BIP_KEY_B);
const BIP_ID_C: NodeId = derive_node_id(BIP_PLUGIN, BIP_KEY_C);
const BIP_ID_D: NodeId = derive_node_id(BIP_PLUGIN, BIP_KEY_D);
const BIP_ID_E: NodeId = derive_node_id(BIP_PLUGIN, BIP_KEY_E);
const BIP_ID_F: NodeId = derive_node_id(BIP_PLUGIN, BIP_KEY_F);
const BIP_ID_G: NodeId = derive_node_id(BIP_PLUGIN, BIP_KEY_G);

const BIP_VEC_A: VectorAddress = VectorAddress::new(14, 2, 1, 0);
const BIP_VEC_B: VectorAddress = VectorAddress::new(14, 2, 2, 0);
const BIP_VEC_C: VectorAddress = VectorAddress::new(14, 2, 3, 0);
const BIP_VEC_D: VectorAddress = VectorAddress::new(14, 2, 4, 0);
const BIP_VEC_E: VectorAddress = VectorAddress::new(14, 2, 5, 0);
const BIP_VEC_F: VectorAddress = VectorAddress::new(14, 2, 6, 0);
const BIP_VEC_G: VectorAddress = VectorAddress::new(14, 2, 7, 0);

const BIP_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    BIP_PLUGIN,
    name:         "kl-graph-bipartite-harness",
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
        executor_id: BIP_EXEC,
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
    gos_runtime::discover_plugin(BIP_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(BIP_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn color_for_vec(
    vecs: &[VectorAddress],
    colors: &[u8],
    total: usize,
    target: VectorAddress,
) -> Option<u8> {
    for i in 0..total {
        if vecs[i] == target {
            return Some(colors[i]);
        }
    }
    None
}

// ── 1. Empty graph → bipartite ────────────────────────────────────────────────

#[test]
fn empty_graph_is_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
    assert!(is_bipartite, "empty graph must be bipartite (vacuously)");
}

// ── 2. Single isolated node → bipartite ──────────────────────────────────────

#[test]
fn single_node_is_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 1, "1 node registered");
    assert!(is_bipartite, "single node with no edges must be bipartite");
}

// ── 3. Single directed edge A→B → bipartite ──────────────────────────────────

#[test]
fn single_edge_is_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_edge(BIP_ID_A, BIP_ID_B, "bip.ab.t3");
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert!(is_bipartite, "single edge A->B: bipartite (sets A, B)");
}

// ── 4. Path A→B→C → bipartite ────────────────────────────────────────────────

#[test]
fn path_three_nodes_is_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_node(BIP_VEC_C, BIP_KEY_C, BIP_ID_C, 0xB003);
    add_edge(BIP_ID_A, BIP_ID_B, "bip.ab.t4");
    add_edge(BIP_ID_B, BIP_ID_C, "bip.bc.t4");
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert!(is_bipartite, "path A->B->C: bipartite (set A,C vs set B)");
}

// ── 5. Triangle A→B→C→A → NOT bipartite ─────────────────────────────────────

#[test]
fn triangle_is_not_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_node(BIP_VEC_C, BIP_KEY_C, BIP_ID_C, 0xB003);
    add_edge(BIP_ID_A, BIP_ID_B, "bip.ab.t5");
    add_edge(BIP_ID_B, BIP_ID_C, "bip.bc.t5");
    add_edge(BIP_ID_C, BIP_ID_A, "bip.ca.t5");
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert!(!is_bipartite, "triangle A→B→C→A: odd cycle 3 → NOT bipartite");
}

// ── 6. 4-cycle A→B→C→D→A → bipartite ────────────────────────────────────────

#[test]
fn four_cycle_is_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_node(BIP_VEC_C, BIP_KEY_C, BIP_ID_C, 0xB003);
    add_node(BIP_VEC_D, BIP_KEY_D, BIP_ID_D, 0xB004);
    add_edge(BIP_ID_A, BIP_ID_B, "bip.ab.t6");
    add_edge(BIP_ID_B, BIP_ID_C, "bip.bc.t6");
    add_edge(BIP_ID_C, BIP_ID_D, "bip.cd.t6");
    add_edge(BIP_ID_D, BIP_ID_A, "bip.da.t6");
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert!(is_bipartite, "4-cycle A->B->C->D->A: even cycle bipartite (set A,C vs set B,D)");
}

// ── 7. 4-cycle + chord A→C → NOT bipartite ───────────────────────────────────

#[test]
fn four_cycle_with_chord_is_not_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_node(BIP_VEC_C, BIP_KEY_C, BIP_ID_C, 0xB003);
    add_node(BIP_VEC_D, BIP_KEY_D, BIP_ID_D, 0xB004);
    add_edge(BIP_ID_A, BIP_ID_B, "bip.ab.t7");
    add_edge(BIP_ID_B, BIP_ID_C, "bip.bc.t7");
    add_edge(BIP_ID_C, BIP_ID_D, "bip.cd.t7");
    add_edge(BIP_ID_D, BIP_ID_A, "bip.da.t7");
    add_edge(BIP_ID_A, BIP_ID_C, "bip.ac.t7"); // diagonal creates 3-cycle A-B-C-A
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert!(!is_bipartite, "4-cycle + chord A→C creates odd 3-cycle → NOT bipartite");
}

// ── 8. Two disconnected bipartite components → bipartite ─────────────────────

#[test]
fn disconnected_bipartite_components_are_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Component 1: A→B
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_edge(BIP_ID_A, BIP_ID_B, "bip.ab.t8");
    // Component 2: F→G (isolated from A/B)
    add_node(BIP_VEC_F, BIP_KEY_F, BIP_ID_F, 0xB006);
    add_node(BIP_VEC_G, BIP_KEY_G, BIP_ID_G, 0xB007);
    add_edge(BIP_ID_F, BIP_ID_G, "bip.fg.t8");
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 4, "4 nodes across two components");
    assert!(is_bipartite, "two disconnected bipartite components → bipartite");
}

// ── 9. Star: centre→{A,B,C,D} → bipartite ───────────────────────────────────

#[test]
fn star_graph_is_bipartite() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // E is the centre; A, B, C, D are leaves.
    add_node(BIP_VEC_E, BIP_KEY_E, BIP_ID_E, 0xB005); // centre
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_node(BIP_VEC_C, BIP_KEY_C, BIP_ID_C, 0xB003);
    add_node(BIP_VEC_D, BIP_KEY_D, BIP_ID_D, 0xB004);
    add_edge(BIP_ID_E, BIP_ID_A, "bip.ea.t9");
    add_edge(BIP_ID_E, BIP_ID_B, "bip.eb.t9");
    add_edge(BIP_ID_E, BIP_ID_C, "bip.ec.t9");
    add_edge(BIP_ID_E, BIP_ID_D, "bip.ed.t9");
    let (_vecs, _colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 5, "5 nodes (centre + 4 leaves)");
    assert!(is_bipartite, "star K_1_4: bipartite (centre vs A,B,C,D)");
}

// ── 10. Color assignment correct for path A→B→C ───────────────────────────────

#[test]
fn color_assignment_correct_for_path() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BIP_VEC_A, BIP_KEY_A, BIP_ID_A, 0xB001);
    add_node(BIP_VEC_B, BIP_KEY_B, BIP_ID_B, 0xB002);
    add_node(BIP_VEC_C, BIP_KEY_C, BIP_ID_C, 0xB003);
    add_edge(BIP_ID_A, BIP_ID_B, "bip.ab.t10");
    add_edge(BIP_ID_B, BIP_ID_C, "bip.bc.t10");
    let (vecs, colors, total, is_bipartite) = gos_runtime::graph_bipartite::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert!(is_bipartite, "path A→B→C is bipartite");
    // A and C must have the same color, B must have the opposite color.
    let color_a = color_for_vec(&vecs, &colors, total, BIP_VEC_A)
        .expect("A must be in result");
    let color_b = color_for_vec(&vecs, &colors, total, BIP_VEC_B)
        .expect("B must be in result");
    let color_c = color_for_vec(&vecs, &colors, total, BIP_VEC_C)
        .expect("C must be in result");
    assert_eq!(color_a, color_c, "A and C must share the same color set");
    assert_ne!(color_a, color_b, "A and B must have different colors");
    assert_ne!(color_b, color_c, "B and C must have different colors");
}
