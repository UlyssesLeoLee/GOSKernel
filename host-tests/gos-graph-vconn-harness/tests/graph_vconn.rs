// gos-graph-vconn-harness — V3.07 vertex connectivity tests
//
// Verifies `gos_runtime::graph_vertex_connectivity` — Even (1975) algorithm
// for computing κ(G), the minimum vertex cut of the undirected projection of
// the directed kernel graph.
//
// κ(G) = minimum number of vertices whose removal disconnects G (or leaves
// it trivial).  Whitney invariant: κ(G) ≤ κ'(G) ≤ δ(G).
//
// Algorithm (Even 1975 / Menger 1927):
//   1. Build undirected projection; check connectivity (κ=0 if disconnected).
//   2. Check K_n (κ = n-1 shortcut).
//   3. Fix s = min-degree vertex; for each non-neighbour t compute max
//      vertex-disjoint s-t paths via Edmonds-Karp on node-split network.
//   4. κ = min result (≤ δ).
//
// Tests (10):
//  1.  Empty graph                               → κ=0.
//  2.  Single isolated node                      → κ=0.
//  3.  Single directed edge A→B (K_2)            → κ=1.
//  4.  Path A→B→C (undirected)                   → κ=1 (removing B disconnects).
//  5.  Directed cycle A→B→C→D→A (C_4 undirected)→ κ=2.
//  6.  K_4 (4 nodes, one-direction edges)        → κ=3 = n-1.
//  7.  Star K_{1,4} (centre→4 leaves)            → κ=1 (removing centre).
//  8.  Hourglass (two triangles sharing A)       → κ=1 (A is cut vertex).
//  9.  Disconnected A→B ∥ C→D                    → κ=0.
// 10.  Whitney invariant: κ ≤ δ  (C_4 and K_{3,3}).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const VC_PLUGIN: PluginId   = PluginId::from_ascii("KL_GRAPH_VCON_H");
const VC_EXEC:   ExecutorId = ExecutorId::from_ascii("vconn.exec");

const VC_KEY_A: &str = "vconn.a";
const VC_KEY_B: &str = "vconn.b";
const VC_KEY_C: &str = "vconn.c";
const VC_KEY_D: &str = "vconn.d";
const VC_KEY_E: &str = "vconn.e";
const VC_KEY_F: &str = "vconn.f";

const VC_ID_A: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_A);
const VC_ID_B: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_B);
const VC_ID_C: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_C);
const VC_ID_D: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_D);
const VC_ID_E: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_E);
const VC_ID_F: NodeId = derive_node_id(VC_PLUGIN, VC_KEY_F);

// L4=83 namespace for this harness.
const VC_VEC_A: VectorAddress = VectorAddress::new(83, 1, 1, 0);
const VC_VEC_B: VectorAddress = VectorAddress::new(83, 1, 2, 0);
const VC_VEC_C: VectorAddress = VectorAddress::new(83, 1, 3, 0);
const VC_VEC_D: VectorAddress = VectorAddress::new(83, 2, 1, 0);
const VC_VEC_E: VectorAddress = VectorAddress::new(83, 2, 2, 0);
const VC_VEC_F: VectorAddress = VectorAddress::new(83, 2, 3, 0);

const VC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    VC_PLUGIN,
    name:         "kl-graph-vconn-harness",
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

fn setup() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(VC_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (_, nc, kappa, _) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,    0, "empty: nc=0");
    assert_eq!(kappa, 0, "empty: κ=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,      1, "single node: nc=1");
    assert_eq!(kappa,   0, "single node: κ=0");
    assert_eq!(min_deg, 0, "single node: δ=0");
}

// ── Test 3: K_2 — single edge A→B ────────────────────────────────────────────

#[test]
fn test_03_k2_single_edge() {
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_edge(VC_ID_A, VC_ID_B, "ab");

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,      2, "K_2: nc=2");
    assert_eq!(kappa,   1, "K_2: κ=1 = n-1");
    assert_eq!(min_deg, 1, "K_2: δ=1");
    assert!(kappa <= min_deg, "Whitney: κ≤δ");
}

// ── Test 4: Path A→B→C — κ=1 (removing B disconnects) ───────────────────────

#[test]
fn test_04_path_abc_kappa1() {
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_edge(VC_ID_A, VC_ID_B, "ab");
    add_edge(VC_ID_B, VC_ID_C, "bc");

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,    3,  "path A→B→C: nc=3");
    assert_eq!(kappa, 1,  "path A→B→C: κ=1 (B is cut vertex)");
    assert!(kappa <= min_deg, "Whitney: κ≤δ");
    assert!(min_deg >= 1, "path: δ≥1");
}

// ── Test 5: C_4 — directed cycle A→B→C→D→A, κ=2 ─────────────────────────────

#[test]
fn test_05_cycle_c4_kappa2() {
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_edge(VC_ID_A, VC_ID_B, "ab");
    add_edge(VC_ID_B, VC_ID_C, "bc");
    add_edge(VC_ID_C, VC_ID_D, "cd");
    add_edge(VC_ID_D, VC_ID_A, "da");

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,      4, "C_4: nc=4");
    assert_eq!(kappa,   2, "C_4: κ=2 (need 2 removals to disconnect)");
    assert_eq!(min_deg, 2, "C_4: δ=2 (each node has degree 2)");
    assert!(kappa <= min_deg, "Whitney: κ≤δ");
}

// ── Test 6: K_4 — complete, κ=3 = n-1 ────────────────────────────────────────

#[test]
fn test_06_k4_complete_kappa3() {
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    // One-direction edges — undirected projection is K_4.
    add_edge(VC_ID_A, VC_ID_B, "ab");
    add_edge(VC_ID_A, VC_ID_C, "ac");
    add_edge(VC_ID_A, VC_ID_D, "ad");
    add_edge(VC_ID_B, VC_ID_C, "bc");
    add_edge(VC_ID_B, VC_ID_D, "bd");
    add_edge(VC_ID_C, VC_ID_D, "cd");

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,      4, "K_4: nc=4");
    assert_eq!(kappa,   3, "K_4: κ=3 = n-1 (complete graph)");
    assert_eq!(min_deg, 3, "K_4: δ=3");
    assert!(kappa <= min_deg, "Whitney: κ≤δ");
}

// ── Test 7: Star K_{1,4} — centre→leaves, κ=1 ────────────────────────────────

#[test]
fn test_07_star_k14_kappa1() {
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A); // centre
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_node(VC_VEC_E, VC_KEY_E, VC_ID_E);
    add_edge(VC_ID_A, VC_ID_B, "ab");
    add_edge(VC_ID_A, VC_ID_C, "ac");
    add_edge(VC_ID_A, VC_ID_D, "ad");
    add_edge(VC_ID_A, VC_ID_E, "ae");

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,      5, "K_{{1,4}}: nc=5");
    assert_eq!(kappa,   1, "K_{{1,4}}: \u{03ba}=1 (removing centre disconnects)");
    assert_eq!(min_deg, 1, "K_{{1,4}}: \u{03b4}=1 (leaves have degree 1)");
    assert!(kappa <= min_deg, "Whitney: κ≤δ");
}

// ── Test 8: Hourglass — two triangles sharing A, κ=1 ─────────────────────────

#[test]
fn test_08_hourglass_kappa1() {
    let _g = setup();
    // Triangle ABC + Triangle ADE, sharing vertex A.
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_node(VC_VEC_E, VC_KEY_E, VC_ID_E);
    // Triangle ABC (bidirected for symmetric undirected projection)
    add_edge(VC_ID_A, VC_ID_B, "ab"); add_edge(VC_ID_B, VC_ID_A, "ba");
    add_edge(VC_ID_B, VC_ID_C, "bc"); add_edge(VC_ID_C, VC_ID_B, "cb");
    add_edge(VC_ID_A, VC_ID_C, "ac"); add_edge(VC_ID_C, VC_ID_A, "ca");
    // Triangle ADE
    add_edge(VC_ID_A, VC_ID_D, "ad"); add_edge(VC_ID_D, VC_ID_A, "da");
    add_edge(VC_ID_D, VC_ID_E, "de"); add_edge(VC_ID_E, VC_ID_D, "ed");
    add_edge(VC_ID_A, VC_ID_E, "ae"); add_edge(VC_ID_E, VC_ID_A, "ea");

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,      5, "hourglass: nc=5");
    assert_eq!(kappa,   1, "hourglass: κ=1 (A is the only cut vertex)");
    assert_eq!(min_deg, 2, "hourglass: δ=2 (B,C,D,E each have degree 2)");
    assert!(kappa <= min_deg, "Whitney: κ≤δ");
}

// ── Test 9: Disconnected A→B ∥ C→D — κ=0 ────────────────────────────────────

#[test]
fn test_09_disconnected_kappa0() {
    let _g = setup();
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A);
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B);
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C);
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D);
    add_edge(VC_ID_A, VC_ID_B, "ab");
    add_edge(VC_ID_C, VC_ID_D, "cd");

    let (_, nc, kappa, _) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,    4, "disconnected: nc=4");
    assert_eq!(kappa, 0, "disconnected: κ=0");
}

// ── Test 10: Whitney invariant — K_{3,3} κ=3=δ ───────────────────────────────

#[test]
fn test_10_whitney_k33() {
    let _g = setup();
    // K_{3,3}: bipartite complete, A1/A2/A3 vs B1/B2/B3.
    // Use A=A1, B=A2, C=A3, D=B1, E=B2, F=B3 in our node constants.
    add_node(VC_VEC_A, VC_KEY_A, VC_ID_A); // A1
    add_node(VC_VEC_B, VC_KEY_B, VC_ID_B); // A2
    add_node(VC_VEC_C, VC_KEY_C, VC_ID_C); // A3
    add_node(VC_VEC_D, VC_KEY_D, VC_ID_D); // B1
    add_node(VC_VEC_E, VC_KEY_E, VC_ID_E); // B2
    add_node(VC_VEC_F, VC_KEY_F, VC_ID_F); // B3
    // All A_i → B_j edges (one direction; undirected projection = K_{3,3})
    add_edge(VC_ID_A, VC_ID_D, "a-b1");
    add_edge(VC_ID_A, VC_ID_E, "a-b2");
    add_edge(VC_ID_A, VC_ID_F, "a-b3");
    add_edge(VC_ID_B, VC_ID_D, "a2-b1");
    add_edge(VC_ID_B, VC_ID_E, "a2-b2");
    add_edge(VC_ID_B, VC_ID_F, "a2-b3");
    add_edge(VC_ID_C, VC_ID_D, "a3-b1");
    add_edge(VC_ID_C, VC_ID_E, "a3-b2");
    add_edge(VC_ID_C, VC_ID_F, "a3-b3");

    let (_, nc, kappa, min_deg) = gos_runtime::graph_vertex_connectivity::<64>();
    assert_eq!(nc,      6, "K_{{3,3}}: nc=6");
    assert_eq!(kappa,   3, "K_{{3,3}}: \u{03ba}=3 (must remove all of one side)");
    assert_eq!(min_deg, 3, "K_{{3,3}}: \u{03b4}=3 (all nodes have degree 3)");
    // Whitney invariant: κ ≤ δ
    assert!(kappa <= min_deg, "Whitney: κ≤δ for K_{{3,3}}");
    // For K_{3,3} specifically: κ = δ (vertex-connectivity-tight graph)
    assert_eq!(kappa, min_deg, "K_{{3,3}} is vertex-connectivity-tight: κ=δ");
}
