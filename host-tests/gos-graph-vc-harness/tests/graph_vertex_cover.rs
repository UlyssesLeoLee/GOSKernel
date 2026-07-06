// gos-graph-vc-harness -- V2.97 minimum vertex cover tests
//
// Verifies `gos_runtime::graph_vertex_cover` — minimum vertex cover τ(G).
//
// Bipartite graphs: exact via König's theorem (τ = ν = max matching size).
// General graphs: 2-approximation via greedy maximal matching.
//
// Key invariants tested:
//   Gallai (bipartite): α(G) + τ(G) = n
//   König  (bipartite): τ(G) = ν(G) = max bipartite matching size
//   Cover validity: every edge has at least one endpoint in the cover
//
//  1. Empty graph            → cover=[], τ=0, exact=true.
//  2. Single isolated node   → cover=[], τ=0, exact=true (no edges to cover).
//  3. Single edge A-B        → cover=1 node, τ=1, exact=true (bipartite).
//  4. Path P4 (A-B-C-D)      → τ=2, exact=true; König cover {B,D} or {A,C}.
//  5. Triangle K3            → τ=2, exact=false (2-approx); cover validity check.
//  6. K4 complete graph      → τ≥3, exact=false (2-approx); cover validity check.
//  7. Star K_{1,4}           → τ=1, cover={center}, exact=true (bipartite).
//  8. K_{3,3} bipartite      → τ=3, exact=true; König: τ=ν=3.
//  9. Gallai cross-check P4  → α+τ=n using graph_independent_set.
// 10. König cross-check K33  → τ(G)=ν(G) using graph_bipartite_match.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const VC_PLUGIN: PluginId   = PluginId::from_ascii("KL_VC_HARNESS__");
const VC_EXEC:   ExecutorId = ExecutorId::from_ascii("vc.exec");

const VC_KEY_A: &str = "vc.a";
const VC_KEY_B: &str = "vc.b";
const VC_KEY_C: &str = "vc.c";
const VC_KEY_D: &str = "vc.d";
const VC_KEY_E: &str = "vc.e";
const VC_KEY_F: &str = "vc.f";

const VC_ID_A: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_A);
const VC_ID_B: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_B);
const VC_ID_C: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_C);
const VC_ID_D: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_D);
const VC_ID_E: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_E);
const VC_ID_F: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_F);

// L4=73 namespace for this harness.
const VC_VEC_A: VectorAddress = VectorAddress::new(73, 1, 1, 0);
const VC_VEC_B: VectorAddress = VectorAddress::new(73, 1, 2, 0);
const VC_VEC_C: VectorAddress = VectorAddress::new(73, 1, 3, 0);
const VC_VEC_D: VectorAddress = VectorAddress::new(73, 1, 4, 0);
const VC_VEC_E: VectorAddress = VectorAddress::new(73, 2, 1, 0);
const VC_VEC_F: VectorAddress = VectorAddress::new(73, 2, 2, 0);

const VC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    VC_PLUGIN,
    name:         "kl-graph-vc-harness",
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
        executor_id:       VC_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(VC_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(VC_MANIFEST).unwrap();
    g
}

fn cover_contains(vecs: &[VectorAddress], count: usize, target: VectorAddress) -> bool {
    vecs[..count].iter().any(|&v| v == target)
}

// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count,  0, "empty: node_count=0");
    assert_eq!(cover_size,  0, "empty: \u{3c4}=0");
    assert!(is_exact,          "empty: trivially exact (no edges)");
}

#[test]
fn test_02_single_isolated_node() {
    // One node, no edges: the empty set covers all (zero) edges.
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    let (_, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count, 1, "1 node");
    assert_eq!(cover_size, 0, "isolated node: \u{3c4}=0 (no edges)");
    assert!(is_exact,          "bipartite (no edges)");
}

#[test]
fn test_03_single_edge() {
    // A→B: one undirected edge.  Min cover = {A} or {B}, size 1.
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_edge(VC_ID_A, VC_ID_B, "ab");
    let (vecs, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count, 2, "2 nodes");
    assert_eq!(cover_size, 1, "single edge: \u{3c4}=1");
    assert!(is_exact, "single edge: bipartite, exact");
    // The one cover vertex must be A or B.
    let has_a = cover_contains(&vecs, cover_size, VC_VEC_A);
    let has_b = cover_contains(&vecs, cover_size, VC_VEC_B);
    assert!(has_a || has_b, "cover must contain A or B");
    assert!(!(has_a && has_b), "cover must contain exactly one of A,B (size=1)");
}

#[test]
fn test_04_path_p4() {
    // Path A-B-C-D (undirected).  Bipartite (2-colorable).
    // Min vertex cover size = 2.  Valid covers: {A,C}, {B,D}, {B,C}.
    // König gives one valid cover of size 2.
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_bidir(VC_ID_A, VC_ID_B, "abf", "abr");
    add_bidir(VC_ID_B, VC_ID_C, "bcf", "bcr");
    add_bidir(VC_ID_C, VC_ID_D, "cdf", "cdr");
    let (vecs, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert_eq!(cover_size, 2, "path P4: \u{3c4}=2");
    assert!(is_exact, "path P4 is bipartite: exact");

    // Verify every edge has at least one endpoint in the cover.
    let ca = cover_contains(&vecs, cover_size, VC_VEC_A);
    let cb = cover_contains(&vecs, cover_size, VC_VEC_B);
    let cc = cover_contains(&vecs, cover_size, VC_VEC_C);
    let cd = cover_contains(&vecs, cover_size, VC_VEC_D);
    assert!(ca || cb, "edge A-B covered: A={ca} B={cb}");
    assert!(cb || cc, "edge B-C covered: B={cb} C={cc}");
    assert!(cc || cd, "edge C-D covered: C={cc} D={cd}");
}

#[test]
fn test_05_triangle_k3() {
    // K3: non-bipartite (odd cycle).  τ(K3)=2.
    // Greedy 2-approx must return a valid cover (size ≤ 4 = 2*2).
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_bidir(VC_ID_A, VC_ID_B, "abf", "abr");
    add_bidir(VC_ID_B, VC_ID_C, "bcf", "bcr");
    add_bidir(VC_ID_C, VC_ID_A, "caf", "car");
    let (vecs, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count, 3, "3 nodes");
    assert!(!is_exact, "K3 is not bipartite: approximate");
    // 2-approx: cover_size ≤ 2 * τ(K3) = 2 * 2 = 4
    assert!(cover_size <= 4, "2-approx bound: cover_size={cover_size} \u{2264} 4");
    // Must be a valid cover: every edge covered.
    let ca = cover_contains(&vecs, cover_size, VC_VEC_A);
    let cb = cover_contains(&vecs, cover_size, VC_VEC_B);
    let cc = cover_contains(&vecs, cover_size, VC_VEC_C);
    assert!(ca || cb, "edge A-B covered");
    assert!(cb || cc, "edge B-C covered");
    assert!(cc || ca, "edge C-A covered");
}

#[test]
fn test_06_k4_complete_graph() {
    // K4: non-bipartite.  τ(K4)=3.  Greedy gives size ≤ 2*3=6 (≤4 in practice).
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_bidir(VC_ID_A, VC_ID_B, "abf", "abr");
    add_bidir(VC_ID_A, VC_ID_C, "acf", "acr");
    add_bidir(VC_ID_A, VC_ID_D, "adf", "adr");
    add_bidir(VC_ID_B, VC_ID_C, "bcf", "bcr");
    add_bidir(VC_ID_B, VC_ID_D, "bdf", "bdr");
    add_bidir(VC_ID_C, VC_ID_D, "cdf", "cdr");
    let (vecs, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count, 4, "4 nodes");
    assert!(!is_exact, "K4 is not bipartite: approximate");
    // Valid cover: all 6 edges covered.
    let ca = cover_contains(&vecs, cover_size, VC_VEC_A);
    let cb = cover_contains(&vecs, cover_size, VC_VEC_B);
    let cc = cover_contains(&vecs, cover_size, VC_VEC_C);
    let cd = cover_contains(&vecs, cover_size, VC_VEC_D);
    assert!(ca || cb, "A-B covered");
    assert!(ca || cc, "A-C covered");
    assert!(ca || cd, "A-D covered");
    assert!(cb || cc, "B-C covered");
    assert!(cb || cd, "B-D covered");
    assert!(cc || cd, "C-D covered");
    // 2-approx bound: τ(K4)=3, so cover_size ≤ 6.
    assert!(cover_size <= 6, "2-approx: cover_size={cover_size} \u{2264} 6");
}

#[test]
fn test_07_star_k1_4() {
    // Star K_{1,4}: center A, leaves B,C,D,E.
    // Bipartite; max matching = 1 (only the center can match).
    // τ(K_{1,4}) = 1 — only the center covers all edges.
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A); // center
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_node(VC_VEC_E, VC_KEY_E, VC_ID_E);
    add_bidir(VC_ID_A, VC_ID_B, "abf", "abr");
    add_bidir(VC_ID_A, VC_ID_C, "acf", "acr");
    add_bidir(VC_ID_A, VC_ID_D, "adf", "adr");
    add_bidir(VC_ID_A, VC_ID_E, "aef", "aer");
    let (vecs, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count, 5, "5 nodes");
    assert_eq!(cover_size, 1, "star K_{{1,4}}: \u{3c4}=1 (center covers all)");
    assert!(is_exact, "star is bipartite: exact");
    assert!(
        cover_contains(&vecs, cover_size, VC_VEC_A),
        "cover must be the center A"
    );
}

#[test]
fn test_08_k33_bipartite_exact() {
    // Complete bipartite K_{3,3}: left={A,B,C}, right={D,E,F}.
    // Max bipartite matching ν=3 (perfect matching).
    // König: τ = ν = 3.  The cover is one entire side.
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_node(VC_VEC_E, VC_KEY_E, VC_ID_E);
    add_node(VC_VEC_F, VC_KEY_F, VC_ID_F);
    for (l, r, fk, rk) in [
        (VC_ID_A, VC_ID_D, "adf", "adr"),
        (VC_ID_A, VC_ID_E, "aef", "aer"),
        (VC_ID_A, VC_ID_F, "aff", "afr"),
        (VC_ID_B, VC_ID_D, "bdf", "bdr"),
        (VC_ID_B, VC_ID_E, "bef", "ber"),
        (VC_ID_B, VC_ID_F, "bff", "bfr"),
        (VC_ID_C, VC_ID_D, "cdf", "cdr"),
        (VC_ID_C, VC_ID_E, "cef", "cer"),
        (VC_ID_C, VC_ID_F, "cff", "cfr"),
    ] {
        add_bidir(l, r, fk, rk);
    }
    let (vecs, cover_size, is_exact, node_count) = gos_runtime::graph_vertex_cover::<128>();
    assert_eq!(node_count, 6, "6 nodes");
    assert_eq!(cover_size, 3, "K_{{3,3}}: \u{3c4}=3");
    assert!(is_exact, "K_{{3,3}} bipartite: exact via K\u{f6}nig");

    // Every edge must be covered.
    for (av, bv, name) in [
        (VC_VEC_A, VC_VEC_D, "A-D"), (VC_VEC_A, VC_VEC_E, "A-E"), (VC_VEC_A, VC_VEC_F, "A-F"),
        (VC_VEC_B, VC_VEC_D, "B-D"), (VC_VEC_B, VC_VEC_E, "B-E"), (VC_VEC_B, VC_VEC_F, "B-F"),
        (VC_VEC_C, VC_VEC_D, "C-D"), (VC_VEC_C, VC_VEC_E, "C-E"), (VC_VEC_C, VC_VEC_F, "C-F"),
    ] {
        assert!(
            cover_contains(&vecs, cover_size, av) || cover_contains(&vecs, cover_size, bv),
            "edge {name} not covered"
        );
    }
}

#[test]
fn test_09_gallai_cross_check_p4() {
    // Gallai's theorem (bipartite): α(G) + τ(G) = n.
    // For P4: α=2, τ=2, n=4 → 2+2=4 ✓
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_bidir(VC_ID_A, VC_ID_B, "abf", "abr");
    add_bidir(VC_ID_B, VC_ID_C, "bcf", "bcr");
    add_bidir(VC_ID_C, VC_ID_D, "cdf", "cdr");

    let (_, cover_size, is_exact, node_count) =
        gos_runtime::graph_vertex_cover::<128>();
    let (_, is_size, _, _) =
        gos_runtime::graph_independent_set::<128>();

    assert!(is_exact, "P4 is bipartite");
    assert_eq!(
        cover_size + is_size, node_count,
        "Gallai: \u{3c4}+\u{3b1}=n  {cover_size}+{is_size}={node_count}"
    );
}

#[test]
fn test_10_konig_cross_check_k33() {
    // König's theorem: τ(G) = ν(G) for bipartite G.
    // For K_{3,3}: both τ and ν = 3.
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_node(VC_VEC_E, VC_KEY_E, VC_ID_E);
    add_node(VC_VEC_F, VC_KEY_F, VC_ID_F);
    for (l, r, fk, rk) in [
        (VC_ID_A, VC_ID_D, "xadf", "xadr"),
        (VC_ID_A, VC_ID_E, "xaef", "xaer"),
        (VC_ID_A, VC_ID_F, "xaff", "xafr"),
        (VC_ID_B, VC_ID_D, "xbdf", "xbdr"),
        (VC_ID_B, VC_ID_E, "xbef", "xber"),
        (VC_ID_B, VC_ID_F, "xbff", "xbfr"),
        (VC_ID_C, VC_ID_D, "xcdf", "xcdr"),
        (VC_ID_C, VC_ID_E, "xcef", "xcer"),
        (VC_ID_C, VC_ID_F, "xcff", "xcfr"),
    ] {
        add_bidir(l, r, fk, rk);
    }

    let (_, cover_size, is_exact, _) =
        gos_runtime::graph_vertex_cover::<128>();
    let (_, _, match_count, is_bipartite, _) =
        gos_runtime::graph_bipartite_match::<128>();

    assert!(is_exact,      "K_{{3,3}} is bipartite");
    assert!(is_bipartite,  "bipartite match confirms bipartite");
    assert_eq!(cover_size, 3, "K_{{3,3}}: \u{3c4}=3");
    assert_eq!(match_count, 3, "K_{{3,3}}: \u{3bd}=3 (perfect matching)");
    assert_eq!(
        cover_size, match_count,
        "K\u{f6}nig: \u{3c4}(G)=\u{3bd}(G) for bipartite: {cover_size}={match_count}"
    );
}
