// gos-graph-articulation-harness — V2.85 articulation point API tests
//
// Verifies `gos_runtime::graph_articulation` — Tarjan's iterative disc/low-link
// DFS to identify cut vertices (articulation points) in the undirected projection
// of the live directed kernel graph.
//
// An articulation point is a node whose removal increases the number of connected
// components.  Algorithm: O(V+E), no_std safe, fixed-size stack arrays.
//
// OS analogy: `systemctl list-dependencies --reverse` to identify single-point-of-
// failure kernel subsystems with no redundant dependency path.
//
//  1. Empty graph → art_count=0, node_count=0.
//  2. Single isolated node → art_count=0, node_count=1.
//  3. Single directed edge A→B (undirected: A-B) → 0 articulation points.
//  4. Path A→B→C (undirected chain) → B is cut vertex (art_count=1).
//  5. Triangle A→B→C→A → 0 articulation points (every pair has two paths).
//  6. Star with 4 leaves: centre is the sole cut vertex (art_count=1).
//  7. Two triangles sharing one vertex (bowtie): shared vertex is cut (art_count=1).
//  8. Square (4-cycle) A→B→C→D→A → 0 articulation points.
//  9. Linear chain of 4: A-B-C-D → B and C are cut vertices (art_count=2).
// 10. Two triangles connected by a bridge edge: both bridge endpoints are cut vertices.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const AP_PLUGIN: PluginId   = PluginId::from_ascii("KL_AP01_00");
const AP_EXEC:   ExecutorId = ExecutorId::from_ascii("ap.exec");

const AP_KEY_A: &str = "ap.alpha";
const AP_KEY_B: &str = "ap.beta";
const AP_KEY_C: &str = "ap.gamma";
const AP_KEY_D: &str = "ap.delta";
const AP_KEY_E: &str = "ap.epsilon";
const AP_KEY_F: &str = "ap.zeta";
const AP_KEY_G: &str = "ap.eta";

const AP_ID_A: NodeId = derive_node_id(AP_PLUGIN, AP_KEY_A);
const AP_ID_B: NodeId = derive_node_id(AP_PLUGIN, AP_KEY_B);
const AP_ID_C: NodeId = derive_node_id(AP_PLUGIN, AP_KEY_C);
const AP_ID_D: NodeId = derive_node_id(AP_PLUGIN, AP_KEY_D);
const AP_ID_E: NodeId = derive_node_id(AP_PLUGIN, AP_KEY_E);
const AP_ID_F: NodeId = derive_node_id(AP_PLUGIN, AP_KEY_F);
const AP_ID_G: NodeId = derive_node_id(AP_PLUGIN, AP_KEY_G);

// L4=61 identifies this harness namespace.
const AP_VEC_A: VectorAddress = VectorAddress::new(61, 1, 1, 0);
const AP_VEC_B: VectorAddress = VectorAddress::new(61, 1, 2, 0);
const AP_VEC_C: VectorAddress = VectorAddress::new(61, 1, 3, 0);
const AP_VEC_D: VectorAddress = VectorAddress::new(61, 1, 4, 0);
const AP_VEC_E: VectorAddress = VectorAddress::new(61, 1, 5, 0);
const AP_VEC_F: VectorAddress = VectorAddress::new(61, 1, 6, 0);
const AP_VEC_G: VectorAddress = VectorAddress::new(61, 1, 7, 0);

const AP_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    AP_PLUGIN,
    name:         "kl-graph-articulation-harness",
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
        executor_id:       AP_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(AP_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(AP_MANIFEST).unwrap();
    g
}

// ── Tests ─────────────────────────────────────────────────────────────────────

// 1. Empty graph → art_count=0, node_count=0.
#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 0, "empty graph: node_count=0");
    assert_eq!(art_count, 0, "empty graph: no articulation points");
}

// 2. Single isolated node → art_count=0, node_count=1.
// A single-node graph has no edges; removing the sole node yields 0 components
// (was 1), so it is NOT an articulation point by definition.
#[test]
fn test_02_single_isolated_node() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    let (_vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 1);
    assert_eq!(art_count, 0, "isolated node is never an articulation point");
}

// 3. Two nodes connected by a single directed edge A→B.
// Removing A → {B} isolated: was 1 component ({A,B}), still 1 component ({B}).
// Removing B → {A} isolated: was 1 component, still 1 component ({A}).
// Neither is an articulation point.
#[test]
fn test_03_single_edge_no_cut_vertices() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_edge(AP_ID_A, AP_ID_B, "ab");
    let (_vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 2);
    assert_eq!(art_count, 0, "single edge: no cut vertices");
}

// 4. Path A→B→C (undirected: A-B-C).
// Removing B splits {A} and {C}: B is the sole articulation point.
#[test]
fn test_04_path_three_nodes_middle_is_cut() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_node(AP_VEC_C, AP_KEY_C, AP_ID_C);
    add_edge(AP_ID_A, AP_ID_B, "ab");
    add_edge(AP_ID_B, AP_ID_C, "bc");
    let (vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 3);
    assert_eq!(art_count, 1, "B is the only cut vertex in path A-B-C");
    assert_eq!(vecs[0], AP_VEC_B, "cut vertex should be B");
}

// 5. Triangle A→B→C→A (undirected 3-cycle).
// Every node has two alternative paths to every other; 0 articulation points.
#[test]
fn test_05_triangle_no_cut_vertices() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_node(AP_VEC_C, AP_KEY_C, AP_ID_C);
    add_edge(AP_ID_A, AP_ID_B, "ab");
    add_edge(AP_ID_B, AP_ID_C, "bc");
    add_edge(AP_ID_C, AP_ID_A, "ca");
    let (_vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 3);
    assert_eq!(art_count, 0, "triangle: fully biconnected, no cut vertices");
}

// 6. Star: centre E → leaves A, B, C, D (undirected: E-A, E-B, E-C, E-D).
// Removing E disconnects all leaves into 4 isolated nodes; E is the sole AP.
#[test]
fn test_06_star_centre_is_cut_vertex() {
    let _g = setup();
    add_node(AP_VEC_E, AP_KEY_E, AP_ID_E); // centre
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_node(AP_VEC_C, AP_KEY_C, AP_ID_C);
    add_node(AP_VEC_D, AP_KEY_D, AP_ID_D);
    add_edge(AP_ID_E, AP_ID_A, "ea");
    add_edge(AP_ID_E, AP_ID_B, "eb");
    add_edge(AP_ID_E, AP_ID_C, "ec");
    add_edge(AP_ID_E, AP_ID_D, "ed");
    let (vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 5);
    assert_eq!(art_count, 1, "star: centre is the only cut vertex");
    assert_eq!(vecs[0], AP_VEC_E, "cut vertex is the star centre E");
}

// 7. Bowtie: triangle A-B-C and triangle C-D-E, sharing vertex C.
// Removing C splits {A,B} from {D,E}; C is the sole articulation point.
//   A→B→C→A (left triangle) + C→D→E→C (right triangle)
#[test]
fn test_07_bowtie_shared_vertex_is_cut() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_node(AP_VEC_C, AP_KEY_C, AP_ID_C); // shared apex
    add_node(AP_VEC_D, AP_KEY_D, AP_ID_D);
    add_node(AP_VEC_E, AP_KEY_E, AP_ID_E);
    // Left triangle: A-B-C-A
    add_edge(AP_ID_A, AP_ID_B, "ab");
    add_edge(AP_ID_B, AP_ID_C, "bc");
    add_edge(AP_ID_C, AP_ID_A, "ca");
    // Right triangle: C-D-E-C
    add_edge(AP_ID_C, AP_ID_D, "cd");
    add_edge(AP_ID_D, AP_ID_E, "de");
    add_edge(AP_ID_E, AP_ID_C, "ec");
    let (vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 5);
    assert_eq!(art_count, 1, "bowtie: shared apex C is the only cut vertex");
    assert_eq!(vecs[0], AP_VEC_C, "cut vertex should be C (the shared apex)");
}

// 8. Square (4-cycle): A→B→C→D→A.
// Every pair of adjacent nodes has two disjoint paths; 0 articulation points.
#[test]
fn test_08_square_no_cut_vertices() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_node(AP_VEC_C, AP_KEY_C, AP_ID_C);
    add_node(AP_VEC_D, AP_KEY_D, AP_ID_D);
    add_edge(AP_ID_A, AP_ID_B, "ab");
    add_edge(AP_ID_B, AP_ID_C, "bc");
    add_edge(AP_ID_C, AP_ID_D, "cd");
    add_edge(AP_ID_D, AP_ID_A, "da");
    let (_vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 4);
    assert_eq!(art_count, 0, "square 4-cycle: no articulation points");
}

// 9. Linear chain of 4: A-B-C-D.
// Removing B splits {A} from {C,D}; removing C splits {A,B} from {D}.
// Both B and C are articulation points; A and D are not.
#[test]
fn test_09_chain_four_two_cuts() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_node(AP_VEC_C, AP_KEY_C, AP_ID_C);
    add_node(AP_VEC_D, AP_KEY_D, AP_ID_D);
    add_edge(AP_ID_A, AP_ID_B, "ab");
    add_edge(AP_ID_B, AP_ID_C, "bc");
    add_edge(AP_ID_C, AP_ID_D, "cd");
    let (vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 4);
    assert_eq!(art_count, 2, "chain A-B-C-D: B and C are cut vertices");
    // Results sorted ascending by VectorAddress.as_u64(): B < C in L4=61 namespace.
    assert_eq!(vecs[0], AP_VEC_B, "first cut vertex should be B");
    assert_eq!(vecs[1], AP_VEC_C, "second cut vertex should be C");
}

// 10. Two triangles connected by a bridge edge C→F.
//   Triangle 1: A-B-C-A  (directed: A→B, B→C, C→A)
//   Triangle 2: F-G-E-F  (directed: F→G, G→E, E→F)
//   Bridge:     C→F
// In the undirected projection, C and F are the bridge endpoints and both are
// articulation points (removing C splits {A,B} from {F,G,E}; removing F
// splits {A,B,C} from {G,E}).
#[test]
fn test_10_two_triangles_bridge_endpoints_are_cuts() {
    let _g = setup();
    add_node(AP_VEC_A, AP_KEY_A, AP_ID_A);
    add_node(AP_VEC_B, AP_KEY_B, AP_ID_B);
    add_node(AP_VEC_C, AP_KEY_C, AP_ID_C);
    add_node(AP_VEC_F, AP_KEY_F, AP_ID_F);
    add_node(AP_VEC_G, AP_KEY_G, AP_ID_G);
    add_node(AP_VEC_E, AP_KEY_E, AP_ID_E);
    // Triangle 1: A→B, B→C, C→A
    add_edge(AP_ID_A, AP_ID_B, "ab");
    add_edge(AP_ID_B, AP_ID_C, "bc");
    add_edge(AP_ID_C, AP_ID_A, "ca");
    // Triangle 2: F→G, G→E, E→F
    add_edge(AP_ID_F, AP_ID_G, "fg");
    add_edge(AP_ID_G, AP_ID_E, "ge");
    add_edge(AP_ID_E, AP_ID_F, "ef");
    // Bridge: C→F
    add_edge(AP_ID_C, AP_ID_F, "cf");
    let (vecs, art_count, node_count) = gos_runtime::graph_articulation::<128>();
    assert_eq!(node_count, 6);
    assert_eq!(art_count, 2, "bridge endpoints C and F are both cut vertices");
    // C < F in sort order (L4=61: C has offset 3, F has offset 6).
    assert_eq!(vecs[0], AP_VEC_C, "first cut vertex: C (lower address)");
    assert_eq!(vecs[1], AP_VEC_F, "second cut vertex: F (bridge other end)");
}
