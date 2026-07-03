// gos-graph-rich-club-harness — V2.68 graph rich-club coefficient
//
// Verifies `gos_runtime::graph_rich_club(k)` — fraction of edges present
// among "rich" nodes (those with undirected degree > k), relative to the
// maximum possible:
//
//   ρ(k) = E_{>k} / [N_{>k} × (N_{>k} − 1) / 2]
//
// Integer arithmetic:
//   ρ_ppm(k) = E_{>k} × 2_000_000 / (N_{>k} × (N_{>k} − 1))
//
// Directed edges treated as undirected; self-loops excluded.
//
// Return value: (rich_club_ppm, rich_node_count, edges_among_rich)
//   1_000_000 → rich nodes form a clique (max possible density).
//   0         → no rich nodes, fewer than 2 rich nodes, or no edges among them.
//
// Test matrix:
//  1.  Empty graph (k=0)                              → (0, 0, 0)
//  2.  Three isolated nodes (k=0)                     → (0, 0, 0)
//  3.  Star hub→B,C,D — hub has degree 3, leaves 1
//        k=0: all 4 nodes are rich (deg > 0)          → (1_000_000, 4, ... wait)
//        Actually: k=0 means deg > 0. Hub deg=3, B,C,D deg=1, so all 4 have deg>0.
//        Max edges among 4: 4*3/2=6. Edges: 3 (hub-B, hub-C, hub-D). 3*2=6/12=500_000.
//        k=1: deg > 1. Only hub (deg=3). N_rich=1 < 2 → (0, 1, 0)
//        k=2: deg > 2. Only hub (deg=3). N_rich=1 < 2 → (0, 1, 0)
//  4.  K4 complete graph (k=2)                        → (1_000_000, 4, 6)
//  5.  K4 complete graph (k=0) — all 4 are rich       → (1_000_000, 4, 6)
//  6.  Two K3 cliques disconnected (k=1)
//        All 6 nodes have degree 2 > 1; 6 undirected edges, all intra-rich.
//        N_rich=6: max edges = 6*5/2=15; E_rich=6.
//        ρ_ppm = 6*2_000_000/(6*5) = 12_000_000/30 = 400_000.
//  7.  Path A→B→C→D (k=0): all 4 have degree ≥ 1
//        deg: A=1, B=2, C=2, D=1. All > 0.
//        E_rich = 3 edges (undirected A-B, B-C, C-D).
//        ρ_ppm = 3*2_000_000/(4*3) = 6_000_000/12 = 500_000.
//  8.  Path A→B→C→D (k=1): deg>1 → B(2), C(2). N_rich=2.
//        Edge among rich: B-C (1 edge). Max=1. ρ_ppm = 1*2_000_000/(2*1) = 1_000_000.
//  9.  Mutual pair A↔B + C-D single (k=0)
//        Undirected degrees: A=1, B=1, C=1, D=1. All > 0. N_rich=4.
//        E_rich=2 (A-B and C-D). Max=6. ρ_ppm=2*2_000_000/(4*3)=4_000_000/12=333_333.
// 10.  K3 + isolated node D (k=1)
//        A,B,C each have undirected degree 2 > 1; D isolated (degree 0). N_rich=3.
//        E_rich=3 (K3 edges). Max=3. ρ_ppm=1_000_000.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const RC_PLUGIN: PluginId   = PluginId::from_ascii("KL_RICHCL_000");
const RC_EXEC:   ExecutorId = ExecutorId::from_ascii("richcl.exec00");

const RC_KEY_A: &str = "rc.alpha";
const RC_KEY_B: &str = "rc.beta";
const RC_KEY_C: &str = "rc.gamma";
const RC_KEY_D: &str = "rc.delta";

const RC_ID_A: NodeId = derive_node_id(RC_PLUGIN, RC_KEY_A);
const RC_ID_B: NodeId = derive_node_id(RC_PLUGIN, RC_KEY_B);
const RC_ID_C: NodeId = derive_node_id(RC_PLUGIN, RC_KEY_C);
const RC_ID_D: NodeId = derive_node_id(RC_PLUGIN, RC_KEY_D);

const RC_VEC_A: VectorAddress = VectorAddress::new(44, 1, 1, 0);
const RC_VEC_B: VectorAddress = VectorAddress::new(44, 1, 2, 0);
const RC_VEC_C: VectorAddress = VectorAddress::new(44, 1, 3, 0);
const RC_VEC_D: VectorAddress = VectorAddress::new(44, 1, 4, 0);

const RC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    RC_PLUGIN,
    name:         "kl-graph-rich-club-harness",
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

fn node_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       RC_EXEC,
        state_schema_hash: 0xB868,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(RC_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(RC_PLUGIN, vec, node_spec(key, id)).unwrap();
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

fn run_rich_club(k: u8) -> (u32, usize, usize) {
    gos_runtime::graph_rich_club(k)
}

// ── 1. Empty graph (k=0) ──────────────────────────────────────────────────────

#[test]
fn empty_graph_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (rho, n_rich, e_rich) = run_rich_club(0);
    assert_eq!(rho,    0, "empty: ρ_ppm must be 0");
    assert_eq!(n_rich, 0, "empty: no rich nodes");
    assert_eq!(e_rich, 0, "empty: no edges");
}

// ── 2. Three isolated nodes (k=0) — no edges → no rich nodes ─────────────────
//
// Isolated nodes have degree 0. With k=0, rich means degree > 0.
// None qualify → N_rich = 0.

#[test]
fn three_isolated_nodes_k0_no_rich() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);

    let (rho, n_rich, e_rich) = run_rich_club(0);
    assert_eq!(rho,    0, "isolated nodes, k=0: no nodes have degree > 0");
    assert_eq!(n_rich, 0, "0 rich nodes");
    assert_eq!(e_rich, 0, "0 edges among rich");
}

// ── 3. Star A→B,C,D — hub degree 3, leaves degree 1 (k=0) ───────────────────
//
// All 4 nodes have degree > 0 → N_rich=4.
// Undirected edges: {A,B}, {A,C}, {A,D} — only 3 of the 6 possible.
// ρ_ppm = 3 × 2_000_000 / (4 × 3) = 6_000_000 / 12 = 500_000.

#[test]
fn star_k0_all_nodes_rich() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A); // hub
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D);
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t3");
    add_edge(RC_ID_A, RC_ID_C, "rc.ac.t3");
    add_edge(RC_ID_A, RC_ID_D, "rc.ad.t3");

    let (rho, n_rich, e_rich) = run_rich_club(0);
    assert_eq!(n_rich, 4,       "star k=0: all 4 nodes have degree > 0");
    assert_eq!(e_rich, 3,       "3 undirected edges among rich nodes");
    assert_eq!(rho,    500_000, "ρ_ppm = 3×2M/(4×3) = 500_000");
}

// ── 4. Star A→B,C,D — k=1: only hub (deg=3 > 1), leaves (deg=1) excluded ────
//
// N_rich = 1 (only hub). N_rich < 2 → undefined → (0, 1, 0).

#[test]
fn star_k1_only_hub_rich_undefined() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D);
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t4");
    add_edge(RC_ID_A, RC_ID_C, "rc.ac.t4");
    add_edge(RC_ID_A, RC_ID_D, "rc.ad.t4");

    let (rho, n_rich, e_rich) = run_rich_club(1);
    assert_eq!(n_rich, 1, "star k=1: only hub qualifies (deg=3 > 1)");
    assert_eq!(e_rich, 0, "no edges among a single rich node");
    assert_eq!(rho,    0, "N_rich < 2: ρ undefined, returns 0");
}

// ── 5. K4 complete graph (k=2) — all 4 have degree 3 > 2 ────────────────────
//
// N_rich=4; E_rich=6 (all edges in K4); max=6.
// ρ_ppm = 6 × 2_000_000 / (4 × 3) = 12_000_000 / 12 = 1_000_000.

#[test]
fn k4_complete_k2_all_rich_clique() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D);
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t5");
    add_edge(RC_ID_A, RC_ID_C, "rc.ac.t5");
    add_edge(RC_ID_A, RC_ID_D, "rc.ad.t5");
    add_edge(RC_ID_B, RC_ID_C, "rc.bc.t5");
    add_edge(RC_ID_B, RC_ID_D, "rc.bd.t5");
    add_edge(RC_ID_C, RC_ID_D, "rc.cd.t5");

    let (rho, n_rich, e_rich) = run_rich_club(2);
    assert_eq!(n_rich, 4,         "K4 k=2: all 4 nodes have degree 3 > 2");
    assert_eq!(e_rich, 6,         "all 6 K4 edges are among rich nodes");
    assert_eq!(rho,    1_000_000, "\u{03c1}_ppm = 1_000_000 (K4 is a clique)");
}

// ── 6. K4 complete graph (k=0) — every node has degree > 0 ──────────────────

#[test]
fn k4_complete_k0_clique() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D);
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t6");
    add_edge(RC_ID_A, RC_ID_C, "rc.ac.t6");
    add_edge(RC_ID_A, RC_ID_D, "rc.ad.t6");
    add_edge(RC_ID_B, RC_ID_C, "rc.bc.t6");
    add_edge(RC_ID_B, RC_ID_D, "rc.bd.t6");
    add_edge(RC_ID_C, RC_ID_D, "rc.cd.t6");

    let (rho, n_rich, e_rich) = run_rich_club(0);
    assert_eq!(n_rich, 4,         "K4 k=0: all 4 have degree > 0");
    assert_eq!(e_rich, 6,         "all 6 edges among rich nodes");
    assert_eq!(rho,    1_000_000, "\u{03c1}_ppm = 1_000_000 (K4 clique)");
}

// ── 7. Path A→B→C→D (k=0) — all 4 nodes have degree ≥ 1 ────────────────────
//
// Undirected degrees: A=1, B=2, C=2, D=1. All > 0 → N_rich=4.
// E_rich = 3 undirected edges (A-B, B-C, C-D). Max = 4×3/2 = 6.
// ρ_ppm = 3 × 2_000_000 / (4 × 3) = 6_000_000 / 12 = 500_000.

#[test]
fn path_abcd_k0_four_rich_half_dense() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D);
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t7");
    add_edge(RC_ID_B, RC_ID_C, "rc.bc.t7");
    add_edge(RC_ID_C, RC_ID_D, "rc.cd.t7");

    let (rho, n_rich, e_rich) = run_rich_club(0);
    assert_eq!(n_rich, 4,       "path k=0: all 4 nodes have degree > 0");
    assert_eq!(e_rich, 3,       "3 undirected edges in the path");
    assert_eq!(rho,    500_000, "\u{03c1}_ppm = 3\u{00d7}2M/(4\u{00d7}3) = 500_000");
}

// ── 8. Path A→B→C→D (k=1) — only interior nodes B, C qualify ────────────────
//
// B has degree 2 > 1; C has degree 2 > 1. A and D have degree 1 (not rich).
// N_rich=2; E_rich=1 (edge B-C). Max=1. ρ_ppm = 1 × 2_000_000 / (2 × 1) = 1_000_000.

#[test]
fn path_abcd_k1_interior_nodes_form_clique() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D);
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t8");
    add_edge(RC_ID_B, RC_ID_C, "rc.bc.t8");
    add_edge(RC_ID_C, RC_ID_D, "rc.cd.t8");

    let (rho, n_rich, e_rich) = run_rich_club(1);
    assert_eq!(n_rich, 2,         "path k=1: only B (deg=2) and C (deg=2) qualify");
    assert_eq!(e_rich, 1,         "1 undirected edge B-C among rich nodes");
    assert_eq!(rho,    1_000_000, "\u{03c1}_ppm = 1\u{00d7}2M/(2\u{00d7}1) = 1_000_000");
}

// ── 9. Mutual pair A↔B + separate pair C→D (k=0) ────────────────────────────
//
// A↔B counts as 1 undirected edge (deduplicated). C→D counts as 1.
// Degrees: A=1, B=1, C=1, D=1. All > 0. N_rich=4.
// E_rich=2. Max=6. ρ_ppm = 2 × 2_000_000 / (4 × 3) = 4_000_000/12 = 333_333.

#[test]
fn mutual_pair_plus_separate_k0() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D);
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t9");
    add_edge(RC_ID_B, RC_ID_A, "rc.ba.t9");
    add_edge(RC_ID_C, RC_ID_D, "rc.cd.t9");

    let (rho, n_rich, e_rich) = run_rich_club(0);
    assert_eq!(n_rich, 4,       "k=0: all 4 have degree \u{2265} 1");
    assert_eq!(e_rich, 2,       "2 undirected edges (A\u{2194}B deduplicated + C\u{2192}D)");
    assert_eq!(rho,    333_333, "\u{03c1}_ppm = 2\u{00d7}2M/(4\u{00d7}3) = 333_333");
}

// ── 10. K3 + isolated D (k=1) — only K3 nodes qualify ───────────────────────
//
// A,B,C each have undirected degree 2 > 1. D is isolated (degree 0).
// N_rich=3; E_rich=3 (A-B, B-C, A-C). Max = 3×2/2 = 3.
// ρ_ppm = 3 × 2_000_000 / (3 × 2) = 6_000_000 / 6 = 1_000_000.

#[test]
fn k3_plus_isolated_d_k1_clique() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(RC_VEC_A, RC_KEY_A, RC_ID_A);
    add_node(RC_VEC_B, RC_KEY_B, RC_ID_B);
    add_node(RC_VEC_C, RC_KEY_C, RC_ID_C);
    add_node(RC_VEC_D, RC_KEY_D, RC_ID_D); // isolated
    // K3 triangle (directed, one direction per pair)
    add_edge(RC_ID_A, RC_ID_B, "rc.ab.t10");
    add_edge(RC_ID_B, RC_ID_C, "rc.bc.t10");
    add_edge(RC_ID_A, RC_ID_C, "rc.ac.t10");

    let (rho, n_rich, e_rich) = run_rich_club(1);
    assert_eq!(n_rich, 3,         "k=1: A, B, C each have degree 2 > 1; D isolated");
    assert_eq!(e_rich, 3,         "3 edges among K3 nodes (all three pairs)");
    assert_eq!(rho,    1_000_000, "K3 is a clique among rich nodes: \u{03c1}=1");
}
