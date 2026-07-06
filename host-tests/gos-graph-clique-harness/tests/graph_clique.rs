// gos-graph-clique-harness -- V2.95 maximum clique tests
//
// Verifies `gos_runtime::graph_clique` — iterative Bron-Kerbosch with Tomita
// pivot.  Returns the clique number ω(G), a representative max clique, and the
// total count of distinct maximum-size cliques.
//
//  1. Empty graph            → clique_size=0, clique_count=0.
//  2. Single isolated node   → clique_size=1, clique_count=1.
//  3. Two isolated nodes     → clique_size=1, clique_count=2.
//  4. Single edge A-B        → clique_size=2, clique_count=1.
//  5. Triangle (K3)          → clique_size=3, clique_count=1.
//  6. K4 (complete graph)    → clique_size=4, clique_count=1.
//  7. Two disjoint triangles → clique_size=3, clique_count=2.
//  8. Diamond (K4 − 1 edge)  → clique_size=3, clique_count=2.
//  9. Path of 4 nodes        → clique_size=2, clique_count=3.
// 10. Cross-check ω(G) vs k-core and k-truss for K4:
//      ω=4, max_kcore=3, max_truss=4; invariant ω ≥ max_kcore satisfied.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const CL_PLUGIN: PluginId   = PluginId::from_ascii("KL_CLIQUE_HARN_");
const CL_EXEC:   ExecutorId = ExecutorId::from_ascii("cl.exec");

const CL_KEY_A: &str = "cl.a";
const CL_KEY_B: &str = "cl.b";
const CL_KEY_C: &str = "cl.c";
const CL_KEY_D: &str = "cl.d";
const CL_KEY_E: &str = "cl.e";
const CL_KEY_F: &str = "cl.f";

const CL_ID_A: NodeId = derive_node_id(CL_PLUGIN, CL_KEY_A);
const CL_ID_B: NodeId = derive_node_id(CL_PLUGIN, CL_KEY_B);
const CL_ID_C: NodeId = derive_node_id(CL_PLUGIN, CL_KEY_C);
const CL_ID_D: NodeId = derive_node_id(CL_PLUGIN, CL_KEY_D);
const CL_ID_E: NodeId = derive_node_id(CL_PLUGIN, CL_KEY_E);
const CL_ID_F: NodeId = derive_node_id(CL_PLUGIN, CL_KEY_F);

// L4=71 namespace for this harness.
const CL_VEC_A: VectorAddress = VectorAddress::new(71, 1, 1, 0);
const CL_VEC_B: VectorAddress = VectorAddress::new(71, 1, 2, 0);
const CL_VEC_C: VectorAddress = VectorAddress::new(71, 1, 3, 0);
const CL_VEC_D: VectorAddress = VectorAddress::new(71, 1, 4, 0);
const CL_VEC_E: VectorAddress = VectorAddress::new(71, 2, 1, 0);
const CL_VEC_F: VectorAddress = VectorAddress::new(71, 2, 2, 0);

const CL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    CL_PLUGIN,
    name:         "kl-graph-clique-harness",
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
        executor_id:       CL_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(CL_PLUGIN, vec, node_spec(key, id)).unwrap();
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

/// Add an undirected edge (both directions).
fn add_bidir(a: NodeId, b: NodeId, fwd: &'static str, rev: &'static str) {
    add_edge(a, b, fwd);
    add_edge(b, a, rev);
}

fn setup() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(CL_MANIFEST).unwrap();
    g
}

/// Returns true iff `target` appears in `vecs[0..count]`.
fn clique_contains(vecs: &[VectorAddress], count: usize, target: VectorAddress) -> bool {
    vecs[..count].iter().any(|&v| v == target)
}

// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   0, "empty: node_count=0");
    assert_eq!(clique_size,  0, "empty: clique_size=0");
    assert_eq!(clique_count, 0, "empty: clique_count=0");
}

#[test]
fn test_02_single_isolated_node() {
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    let (vecs, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   1, "1 node");
    assert_eq!(clique_size,  1, "single node: max clique = A, size=1");
    assert_eq!(clique_count, 1, "single node: exactly 1 maximum clique");
    assert!(clique_contains(&vecs, clique_size, CL_VEC_A), "A in clique");
}

#[test]
fn test_03_two_isolated_nodes() {
    // No edges → every node is its own maximum clique of size 1.
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    let (_, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   2, "2 nodes");
    assert_eq!(clique_size,  1, "no edges: max clique size=1");
    assert_eq!(clique_count, 2, "2 isolated nodes → 2 maximum cliques");
}

#[test]
fn test_04_single_edge() {
    // A→B (undirected: A-B).  Maximum clique = {A, B}, size=2.
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    add_edge(CL_ID_A, CL_ID_B, "ab");
    let (vecs, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   2, "2 nodes");
    assert_eq!(clique_size,  2, "single edge: clique=A-B, size=2");
    assert_eq!(clique_count, 1, "exactly 1 maximum clique");
    assert!(clique_contains(&vecs, clique_size, CL_VEC_A), "A in clique");
    assert!(clique_contains(&vecs, clique_size, CL_VEC_B), "B in clique");
}

#[test]
fn test_05_triangle_k3() {
    // Directed 3-cycle A→B→C→A → undirected triangle A-B-C.
    // Maximum clique = {A, B, C}, size=3.  Exactly 1 max clique.
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    add_node(CL_VEC_C, CL_KEY_C, CL_ID_C);
    add_edge(CL_ID_A, CL_ID_B, "ab");
    add_edge(CL_ID_B, CL_ID_C, "bc");
    add_edge(CL_ID_C, CL_ID_A, "ca");
    let (vecs, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   3, "3 nodes");
    assert_eq!(clique_size,  3, "triangle: clique size=3");
    assert_eq!(clique_count, 1, "K3: exactly 1 max clique");
    assert!(clique_contains(&vecs, clique_size, CL_VEC_A), "A in clique");
    assert!(clique_contains(&vecs, clique_size, CL_VEC_B), "B in clique");
    assert!(clique_contains(&vecs, clique_size, CL_VEC_C), "C in clique");
    // Output sorted ascending by VectorAddress
    for i in 1..clique_size {
        assert!(vecs[i - 1].as_u64() <= vecs[i].as_u64(), "output sorted");
    }
}

#[test]
fn test_06_k4_complete_graph() {
    // K4: A-B, A-C, A-D, B-C, B-D, C-D (fully connected).
    // Maximum clique = {A, B, C, D}, size=4.  Exactly 1 max clique.
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    add_node(CL_VEC_C, CL_KEY_C, CL_ID_C);
    add_node(CL_VEC_D, CL_KEY_D, CL_ID_D);
    add_bidir(CL_ID_A, CL_ID_B, "abf", "abr");
    add_bidir(CL_ID_A, CL_ID_C, "acf", "acr");
    add_bidir(CL_ID_A, CL_ID_D, "adf", "adr");
    add_bidir(CL_ID_B, CL_ID_C, "bcf", "bcr");
    add_bidir(CL_ID_B, CL_ID_D, "bdf", "bdr");
    add_bidir(CL_ID_C, CL_ID_D, "cdf", "cdr");
    let (vecs, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   4, "4 nodes");
    assert_eq!(clique_size,  4, "K4: clique size=4");
    assert_eq!(clique_count, 1, "K4: exactly 1 max clique");
    for vec in [CL_VEC_A, CL_VEC_B, CL_VEC_C, CL_VEC_D] {
        assert!(clique_contains(&vecs, clique_size, vec), "all nodes in clique");
    }
}

#[test]
fn test_07_two_disjoint_triangles() {
    // Triangle 1: A-B-C-A.  Triangle 2: D-E-F-D.
    // Maximum clique size = 3.  Two distinct max cliques.
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    add_node(CL_VEC_C, CL_KEY_C, CL_ID_C);
    add_node(CL_VEC_D, CL_KEY_D, CL_ID_D);
    add_node(CL_VEC_E, CL_KEY_E, CL_ID_E);
    add_node(CL_VEC_F, CL_KEY_F, CL_ID_F);
    // Triangle 1
    add_edge(CL_ID_A, CL_ID_B, "ab");
    add_edge(CL_ID_B, CL_ID_C, "bc");
    add_edge(CL_ID_C, CL_ID_A, "ca");
    // Triangle 2
    add_edge(CL_ID_D, CL_ID_E, "de");
    add_edge(CL_ID_E, CL_ID_F, "ef");
    add_edge(CL_ID_F, CL_ID_D, "fd");
    let (_, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   6, "6 nodes");
    assert_eq!(clique_size,  3, "two disjoint triangles: max clique size=3");
    assert_eq!(clique_count, 2, "two disjoint triangles: 2 maximum cliques");
}

#[test]
fn test_08_diamond_k4_minus_one_edge() {
    // Diamond: K4 with edge C-D removed.  Edges: A-B, A-C, A-D, B-C, B-D.
    // Two triangles: {A,B,C} and {A,B,D}.  Max clique size=3, count=2.
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    add_node(CL_VEC_C, CL_KEY_C, CL_ID_C);
    add_node(CL_VEC_D, CL_KEY_D, CL_ID_D);
    add_bidir(CL_ID_A, CL_ID_B, "abf", "abr");
    add_bidir(CL_ID_A, CL_ID_C, "acf", "acr");
    add_bidir(CL_ID_A, CL_ID_D, "adf", "adr");
    add_bidir(CL_ID_B, CL_ID_C, "bcf", "bcr");
    add_bidir(CL_ID_B, CL_ID_D, "bdf", "bdr");
    // C-D edge intentionally absent
    let (vecs, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,   4, "4 nodes");
    assert_eq!(clique_size,  3, "diamond: max clique size=3");
    assert_eq!(clique_count, 2, "diamond: 2 triangles = 2 max cliques");
    // A and B must be in the representative clique (shared by both triangles)
    assert!(clique_contains(&vecs, clique_size, CL_VEC_A), "A in rep clique");
    assert!(clique_contains(&vecs, clique_size, CL_VEC_B), "B in rep clique");
}

#[test]
fn test_09_path_of_four_nodes() {
    // Path A→B→C→D (directed, treated as undirected A-B-C-D).
    // Edges: A-B, B-C, C-D.  Max cliques (size=2): {A,B}, {B,C}, {C,D}.
    // clique_size=2, clique_count=3.
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    add_node(CL_VEC_C, CL_KEY_C, CL_ID_C);
    add_node(CL_VEC_D, CL_KEY_D, CL_ID_D);
    add_edge(CL_ID_A, CL_ID_B, "ab");
    add_edge(CL_ID_B, CL_ID_C, "bc");
    add_edge(CL_ID_C, CL_ID_D, "cd");
    let (_, clique_size, clique_count, node_count) = gos_runtime::graph_clique::<128>();
    assert_eq!(node_count,  4, "4 nodes");
    assert_eq!(clique_size, 2, "path: max clique size=2");
    assert_eq!(clique_count, 3, "path of 4: 3 maximum cliques (each edge)");
}

#[test]
fn test_10_cross_check_clique_vs_kcore_ktruss_on_k4() {
    // For K4:
    //   max_kcore   = 3  (every node has 3 neighbours in the k-core)
    //   max_truss   = 4  (every edge is in 2 triangles)
    //   omega(G)    = 4  (the whole K4 is the max clique)
    //
    // Invariants:
    //   omega(G) >= max_kcore   (4 >= 3) ✓
    //   omega(G) >= max_truss - 1 (4 >= 3) ✓
    //   For K_n: omega = n, max_kcore = n-1, max_truss = n (tight).
    let _g = setup();
    add_node(CL_VEC_A, CL_KEY_A, CL_ID_A);
    add_node(CL_VEC_B, CL_KEY_B, CL_ID_B);
    add_node(CL_VEC_C, CL_KEY_C, CL_ID_C);
    add_node(CL_VEC_D, CL_KEY_D, CL_ID_D);
    add_bidir(CL_ID_A, CL_ID_B, "xabf", "xabr");
    add_bidir(CL_ID_A, CL_ID_C, "xacf", "xacr");
    add_bidir(CL_ID_A, CL_ID_D, "xadf", "xadr");
    add_bidir(CL_ID_B, CL_ID_C, "xbcf", "xbcr");
    add_bidir(CL_ID_B, CL_ID_D, "xbdf", "xbdr");
    add_bidir(CL_ID_C, CL_ID_D, "xcdf", "xcdr");

    let (_, clique_size, clique_count, _) = gos_runtime::graph_clique::<128>();
    let (_, kcore_vals, kc_n, max_core)  = gos_runtime::graph_kcore::<128>();
    let (_, _, tr_n, max_truss)          = gos_runtime::graph_truss::<128>();

    assert_eq!(kc_n,        4, "kcore: 4 nodes");
    assert_eq!(tr_n,        4, "truss: 4 nodes");
    assert_eq!(clique_size, 4, "K4: omega=4");
    assert_eq!(clique_count, 1, "K4: exactly 1 max clique");
    assert_eq!(max_core,   3u8, "K4: max_kcore=3");
    assert_eq!(max_truss,  4u8, "K4: max_truss=4");

    assert!(
        clique_size >= max_core as usize,
        "omega(G) >= max_kcore: {clique_size} >= {max_core}"
    );
    assert!(
        clique_size + 1 >= max_truss as usize,
        "omega(G) >= max_truss-1: {clique_size} >= {}", max_truss - 1
    );
    // For K_n: omega = n = max_truss = max_kcore + 1
    assert_eq!(
        clique_size, max_truss as usize,
        "K4: omega == max_truss (K_n tight invariant)"
    );
    assert_eq!(
        clique_size, max_core as usize + 1,
        "K4: omega == max_kcore + 1 (K_n tight invariant)"
    );
    // All nodes in K4 have the same coreness=3
    for i in 0..kc_n {
        assert_eq!(kcore_vals[i], 3, "node[{i}] kcore=3");
    }
}
