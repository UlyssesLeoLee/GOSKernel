// gos-graph-indep-harness -- V2.96 maximum independent set tests
//
// Verifies `gos_runtime::graph_independent_set` — BK on the complement graph.
// A maximum independent set (MIS) is the largest set of nodes with no two
// nodes adjacent.  α(G) is the independence number.
//
// Algorithm: build complement adjacency comp[i] = all_nodes & !adj[i] & !self;
// then run iterative Bron-Kerbosch with Tomita pivot on the complement.
// Max clique in G̅ = max independent set in G.
//
//  1. Empty graph            → α=0, is_count=0.
//  2. Single node            → α=1, is_count=1.
//  3. Two isolated nodes     → α=2, is_count=1 (both form the unique MIS).
//  4. Single edge A-B        → α=1, is_count=2 ({A} and {B} both optimal).
//  5. Triangle K3            → α=1, is_count=3 (each singleton is a MIS).
//  6. Path P4 (A-B-C-D)      → α=2, is_count=3 ({A,C},{A,D},{B,D}).
//  7. K4 complete graph      → α=1, is_count=4 (each node is a MIS).
//  8. Star K_{1,4}           → α=4, is_count=1 (all four leaves form the MIS).
//  9. Bipartite K_{3,3}      → α=3, is_count=2; König's theorem: α=n-ν.
// 10. Cross-check: α(G)+ω(G) and K4 König/complement invariants.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const IS_PLUGIN: PluginId   = PluginId::from_ascii("KL_INDEP_HARN__");
const IS_EXEC:   ExecutorId = ExecutorId::from_ascii("is.exec");

const IS_KEY_A: &str = "is.a";
const IS_KEY_B: &str = "is.b";
const IS_KEY_C: &str = "is.c";
const IS_KEY_D: &str = "is.d";
const IS_KEY_E: &str = "is.e";
const IS_KEY_F: &str = "is.f";

const IS_ID_A: NodeId = derive_node_id(IS_PLUGIN, IS_KEY_A);
const IS_ID_B: NodeId = derive_node_id(IS_PLUGIN, IS_KEY_B);
const IS_ID_C: NodeId = derive_node_id(IS_PLUGIN, IS_KEY_C);
const IS_ID_D: NodeId = derive_node_id(IS_PLUGIN, IS_KEY_D);
const IS_ID_E: NodeId = derive_node_id(IS_PLUGIN, IS_KEY_E);
const IS_ID_F: NodeId = derive_node_id(IS_PLUGIN, IS_KEY_F);

// L4=72 namespace for this harness.
const IS_VEC_A: VectorAddress = VectorAddress::new(72, 1, 1, 0);
const IS_VEC_B: VectorAddress = VectorAddress::new(72, 1, 2, 0);
const IS_VEC_C: VectorAddress = VectorAddress::new(72, 1, 3, 0);
const IS_VEC_D: VectorAddress = VectorAddress::new(72, 1, 4, 0);
const IS_VEC_E: VectorAddress = VectorAddress::new(72, 2, 1, 0);
const IS_VEC_F: VectorAddress = VectorAddress::new(72, 2, 2, 0);

const IS_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    IS_PLUGIN,
    name:         "kl-graph-indep-harness",
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
        executor_id:       IS_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(IS_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(IS_MANIFEST).unwrap();
    g
}

fn is_contains(vecs: &[VectorAddress], count: usize, target: VectorAddress) -> bool {
    vecs[..count].iter().any(|&v| v == target)
}

// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 0, "empty: node_count=0");
    assert_eq!(is_size,    0, "empty: α(G)=0");
    assert_eq!(is_count,   0, "empty: is_count=0");
}

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    let (vecs, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 1, "1 node");
    assert_eq!(is_size,    1, "single node: α=1");
    assert_eq!(is_count,   1, "single node: exactly 1 MIS");
    assert!(is_contains(&vecs, is_size, IS_VEC_A), "A in MIS");
}

#[test]
fn test_03_two_isolated_nodes() {
    // No edge between A and B → both are mutually non-adjacent → {A,B} is the unique MIS.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B);
    let (vecs, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(is_size,    2, "no edge: α=2 (both nodes)");
    assert_eq!(is_count,   1, "unique MIS");
    assert!(is_contains(&vecs, is_size, IS_VEC_A), "A in MIS");
    assert!(is_contains(&vecs, is_size, IS_VEC_B), "B in MIS");
}

#[test]
fn test_04_single_edge() {
    // A→B (undirected A-B).  MIS = {A} or {B} — each of size 1.  Two distinct MISes.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B);
    add_edge(IS_ID_A, IS_ID_B, "ab");
    let (_, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(is_size,    1, "single edge: α=1");
    assert_eq!(is_count,   2, "two distinct MIS: {{A}} and {{B}}");
}

#[test]
fn test_05_triangle_k3() {
    // K3: every pair is adjacent.  Each singleton {X} is a MIS of size 1.  Three MISes.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B);
    add_node(IS_VEC_C, IS_KEY_C, IS_ID_C);
    add_edge(IS_ID_A, IS_ID_B, "ab");
    add_edge(IS_ID_B, IS_ID_C, "bc");
    add_edge(IS_ID_C, IS_ID_A, "ca");
    let (_, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 3, "3 nodes");
    assert_eq!(is_size,    1, "K3: α=1");
    assert_eq!(is_count,   3, "K3: 3 maximum independent sets (each singleton)");
}

#[test]
fn test_06_path_p4() {
    // Path A-B-C-D.  Non-edges: A-C, A-D, B-D.
    // MISes of size 2: {A,C}, {A,D}, {B,D}.  α=2, is_count=3.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B);
    add_node(IS_VEC_C, IS_KEY_C, IS_ID_C);
    add_node(IS_VEC_D, IS_KEY_D, IS_ID_D);
    add_edge(IS_ID_A, IS_ID_B, "ab");
    add_edge(IS_ID_B, IS_ID_C, "bc");
    add_edge(IS_ID_C, IS_ID_D, "cd");
    let (_, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(is_size,    2, "path P4: α=2");
    assert_eq!(is_count,   3, "path P4: 3 distinct MISes");
}

#[test]
fn test_07_k4_complete_graph() {
    // K4: every pair adjacent.  Only singleton MISes of size 1.  Four MISes.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B);
    add_node(IS_VEC_C, IS_KEY_C, IS_ID_C);
    add_node(IS_VEC_D, IS_KEY_D, IS_ID_D);
    add_bidir(IS_ID_A, IS_ID_B, "abf", "abr");
    add_bidir(IS_ID_A, IS_ID_C, "acf", "acr");
    add_bidir(IS_ID_A, IS_ID_D, "adf", "adr");
    add_bidir(IS_ID_B, IS_ID_C, "bcf", "bcr");
    add_bidir(IS_ID_B, IS_ID_D, "bdf", "bdr");
    add_bidir(IS_ID_C, IS_ID_D, "cdf", "cdr");
    let (_, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(is_size,    1, "K4: α=1");
    assert_eq!(is_count,   4, "K4: 4 singleton MISes (one per node)");
}

#[test]
fn test_08_star_k1_4() {
    // Star K_{1,4}: center A, leaves B,C,D,E.
    // Leaves have no edges to each other → {B,C,D,E} is the unique MIS.  α=4.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A); // center
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B); // leaf
    add_node(IS_VEC_C, IS_KEY_C, IS_ID_C); // leaf
    add_node(IS_VEC_D, IS_KEY_D, IS_ID_D); // leaf
    add_node(IS_VEC_E, IS_KEY_E, IS_ID_E); // leaf
    add_bidir(IS_ID_A, IS_ID_B, "abf", "abr");
    add_bidir(IS_ID_A, IS_ID_C, "acf", "acr");
    add_bidir(IS_ID_A, IS_ID_D, "adf", "adr");
    add_bidir(IS_ID_A, IS_ID_E, "aef", "aer");
    let (vecs, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 5, "5 nodes");
    assert_eq!(is_size,    4, "star K_(1,4): α=4 (all leaves)");
    assert_eq!(is_count,   1, "star: unique MIS = {{leaves}}");
    // Center must NOT be in the MIS (adjacent to all leaves)
    assert!(!is_contains(&vecs, is_size, IS_VEC_A), "center A not in MIS");
    // All leaves must be in the MIS
    for v in [IS_VEC_B, IS_VEC_C, IS_VEC_D, IS_VEC_E] {
        assert!(is_contains(&vecs, is_size, v), "leaf in MIS");
    }
}

#[test]
fn test_09_bipartite_k33_konig() {
    // Complete bipartite K_{3,3}: left={A,B,C}, right={D,E,F}.
    // All left↔right edges; no within-partition edges.
    // α(G) = 3 (either full left or full right partition).
    // By König's theorem: α(G) = n − ν(G) = 6 − 3 = 3.
    // Two distinct MISes: {A,B,C} and {D,E,F}.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B);
    add_node(IS_VEC_C, IS_KEY_C, IS_ID_C);
    add_node(IS_VEC_D, IS_KEY_D, IS_ID_D);
    add_node(IS_VEC_E, IS_KEY_E, IS_ID_E);
    add_node(IS_VEC_F, IS_KEY_F, IS_ID_F);
    // All A,B,C × D,E,F cross-edges
    for (l, r, fk, rk) in [
        (IS_ID_A, IS_ID_D, "adf", "adr"),
        (IS_ID_A, IS_ID_E, "aef", "aer"),
        (IS_ID_A, IS_ID_F, "aff", "afr"),
        (IS_ID_B, IS_ID_D, "bdf", "bdr"),
        (IS_ID_B, IS_ID_E, "bef", "ber"),
        (IS_ID_B, IS_ID_F, "bff", "bfr"),
        (IS_ID_C, IS_ID_D, "cdf", "cdr"),
        (IS_ID_C, IS_ID_E, "cef", "cer"),
        (IS_ID_C, IS_ID_F, "cff", "cfr"),
    ] {
        add_bidir(l, r, fk, rk);
    }
    let (_, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    assert_eq!(node_count, 6, "6 nodes");
    assert_eq!(is_size,    3, "K_{{3,3}}: \u{3b1}=3");
    assert_eq!(is_count,   2, "K_{{3,3}}: 2 distinct MIS (left or right partition)");

    // König's theorem cross-check: α = n − max_matching
    let (_, _, match_count, is_bipartite, _) = gos_runtime::graph_bipartite_match::<128>();
    assert!(is_bipartite, "K_{{3,3}} is bipartite");
    assert_eq!(match_count, 3, "K_{{3,3}} has perfect matching ν=3");
    assert_eq!(
        is_size, node_count - match_count,
        "König: \u{3b1}(G) = n \u{2212} \u{3bd}(G) = {node_count} \u{2212} {match_count}"
    );
}

#[test]
fn test_10_cross_check_alpha_omega_k4() {
    // For K4:
    //   α(G) = 1  (no two nodes are non-adjacent)
    //   ω(G) = 4  (the whole K4 is the max clique)
    //
    // Complement of K4 = empty graph → complement has α=1 maximum cliques (each node alone).
    // Ramsey-type bound: α(G) · ω(G) ≥ n (for vertex-transitive graphs; K4 is vertex-transitive).
    // For K4: 1 · 4 = 4 = n ≥ n ✓
    //
    // Also: for any graph α(G) + ω(G) ≤ n + 1 only for perfect graphs.
    // K4 is a perfect graph: α+ω = 1+4 = 5 = n+1.
    //
    // The BK-on-complement result must agree: ω(G̅) = α(G) = 1.
    let _g = setup();
    add_node(IS_VEC_A, IS_KEY_A, IS_ID_A);
    add_node(IS_VEC_B, IS_KEY_B, IS_ID_B);
    add_node(IS_VEC_C, IS_KEY_C, IS_ID_C);
    add_node(IS_VEC_D, IS_KEY_D, IS_ID_D);
    add_bidir(IS_ID_A, IS_ID_B, "xabf", "xabr");
    add_bidir(IS_ID_A, IS_ID_C, "xacf", "xacr");
    add_bidir(IS_ID_A, IS_ID_D, "xadf", "xadr");
    add_bidir(IS_ID_B, IS_ID_C, "xbcf", "xbcr");
    add_bidir(IS_ID_B, IS_ID_D, "xbdf", "xbdr");
    add_bidir(IS_ID_C, IS_ID_D, "xcdf", "xcdr");

    let (_, is_size, is_count, node_count) = gos_runtime::graph_independent_set::<128>();
    let (_, clique_size, _, _)             = gos_runtime::graph_clique::<128>();

    assert_eq!(node_count,  4, "K4: 4 nodes");
    assert_eq!(is_size,     1, "K4: \u{3b1}(G)=1");
    assert_eq!(is_count,    4, "K4: 4 singleton MISes");
    assert_eq!(clique_size, 4, "K4: \u{3c9}(G)=4");

    // α·ω ≥ n (tight for K4)
    assert!(
        is_size * clique_size >= node_count,
        "\u{3b1}\u{b7}\u{3c9} \u{2265} n: {}*{} >= {}", is_size, clique_size, node_count
    );

    // α + ω = n+1 for K4 (perfect graph)
    assert_eq!(
        is_size + clique_size, node_count + 1,
        "perfect graph: \u{3b1}+\u{3c9} = n+1"
    );

    // Output sorted ascending by VectorAddress (trivially true for size-1 IS)
    assert_eq!(is_size, 1, "representative MIS has exactly 1 node");
}
