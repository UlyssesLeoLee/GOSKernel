// gos-graph-modularity-harness — V2.67 graph modularity
//
// Verifies `gos_runtime::graph_modularity` — Newman–Girvan modularity Q of
// the LPA community partition.
//
// Algorithm: runs Label Propagation (same as `graph_community`) then evaluates
//   Q = Σ_c [ L_c / m  −  (d_c / (2m))² ]
// where m = undirected edge count, L_c = intra-community undirected edges,
// d_c = sum of undirected degrees of nodes in community c.
// Directed edges treated as undirected; self-loops excluded.
//
// Integer arithmetic:  Q_ppm = (4m·ΣL_c − Σd_c²) × 1_000_000 / (4m²)
//
// Return value: (modularity_ppm, community_count, undirected_edge_count, node_count)
//   0         → single community or no edges (Q = 0)
//   500_000   → two equal disconnected cliques (Q = 0.5, theoretical benchmark)
//   1_000_000 → perfect partition (Q = 1, not achievable with natural graphs)
//
// Test matrix:
//  1.  Empty graph                          → (0, 0, 0, 0)
//  2.  Three isolated nodes, no edges       → (0, 3, 0, 3)
//  3.  Single directed edge A→B             → (0, 1, 1, 2)
//  4.  Directed triangle A→B→C→A           → (0, 1, 3, 3)
//  5.  K4 complete graph                    → (0, 1, 6, 4)
//  6.  Two isolated pairs {A–B} {C–D}      → (500_000, 2, 2, 4)
//  7.  Two K3 cliques (disconnected)        → (500_000, 2, 6, 6)
//  8.  K3 + K2 (disconnected)               → (375_000, 2, 4, 5)
//  9.  Mutual pair A↔B + isolated node C    → (0, 2, 1, 3)
// 10.  Star hub→B,C,D (one community)       → (0, 1, 3, 4)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MOD_PLUGIN: PluginId   = PluginId::from_ascii("KL_MODULA_000");
const MOD_EXEC:   ExecutorId = ExecutorId::from_ascii("modq.exec0_0");

const MOD_KEY_A: &str = "mod.alpha";
const MOD_KEY_B: &str = "mod.beta";
const MOD_KEY_C: &str = "mod.gamma";
const MOD_KEY_D: &str = "mod.delta";
const MOD_KEY_E: &str = "mod.epsilon";
const MOD_KEY_F: &str = "mod.zeta";

const MOD_ID_A: NodeId = derive_node_id(MOD_PLUGIN, MOD_KEY_A);
const MOD_ID_B: NodeId = derive_node_id(MOD_PLUGIN, MOD_KEY_B);
const MOD_ID_C: NodeId = derive_node_id(MOD_PLUGIN, MOD_KEY_C);
const MOD_ID_D: NodeId = derive_node_id(MOD_PLUGIN, MOD_KEY_D);
const MOD_ID_E: NodeId = derive_node_id(MOD_PLUGIN, MOD_KEY_E);
const MOD_ID_F: NodeId = derive_node_id(MOD_PLUGIN, MOD_KEY_F);

const MOD_VEC_A: VectorAddress = VectorAddress::new(43, 1, 1, 0);
const MOD_VEC_B: VectorAddress = VectorAddress::new(43, 1, 2, 0);
const MOD_VEC_C: VectorAddress = VectorAddress::new(43, 1, 3, 0);
const MOD_VEC_D: VectorAddress = VectorAddress::new(43, 1, 4, 0);
const MOD_VEC_E: VectorAddress = VectorAddress::new(43, 1, 5, 0);
const MOD_VEC_F: VectorAddress = VectorAddress::new(43, 1, 6, 0);

const MOD_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    MOD_PLUGIN,
    name:         "kl-graph-modularity-harness",
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
        executor_id:       MOD_EXEC,
        state_schema_hash: 0xA767,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(MOD_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(MOD_PLUGIN, vec, node_spec(key, id)).unwrap();
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

fn run_modq() -> (i32, usize, usize, usize) {
    gos_runtime::graph_modularity()
}

// ── 1. Empty graph ─────────────────────────────────────────────────────────────

#[test]
fn empty_graph_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     0, "empty: Q_ppm must be 0");
    assert_eq!(comms, 0, "empty: no communities");
    assert_eq!(edges, 0, "empty: no undirected edges");
    assert_eq!(nodes, 0, "empty: no nodes");
}

// ── 2. Three isolated nodes, no edges ─────────────────────────────────────────
//
// No edges → m=0 early return; each node forms its own community (3 total).

#[test]
fn three_isolated_nodes_no_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     0, "isolated nodes: Q=0 (no edges)");
    assert_eq!(comms, 3, "each isolated node is its own community");
    assert_eq!(edges, 0, "no undirected edges");
    assert_eq!(nodes, 3, "3 live nodes");
}

// ── 3. Single directed edge A→B ────────────────────────────────────────────────
//
// A and B merge into one community via LPA. Single undirected edge {A,B}.
// Q = L_0/m − (d_0/(2m))² = 1/1 − (2/2)² = 1 − 1 = 0.

#[test]
fn single_edge_single_community_q_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t3");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     0, "A→B: LPA merges both, Q=0");
    assert_eq!(comms, 1, "one community (A and B merged)");
    assert_eq!(edges, 1, "1 undirected edge A-B");
    assert_eq!(nodes, 2, "2 nodes");
}

// ── 4. Directed triangle A→B→C→A — one community, Q=0 ────────────────────────
//
// Undirected projection: K3 ({A,B}, {B,C}, {A,C}). LPA converges to 1 community.
// Q = 3/3 − (6/6)² = 1 − 1 = 0.

#[test]
fn directed_triangle_single_community_q_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t4");
    add_edge(MOD_ID_B, MOD_ID_C, "mod.bc.t4");
    add_edge(MOD_ID_C, MOD_ID_A, "mod.ca.t4");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     0, "directed triangle: 1 community, Q=0");
    assert_eq!(comms, 1, "LPA merges all three nodes");
    assert_eq!(edges, 3, "3 undirected edges (K3 projection)");
    assert_eq!(nodes, 3, "3 nodes");
}

// ── 5. K4 complete graph (6 undirected edges) ─────────────────────────────────
//
// All 4 nodes mutually connected → LPA gives 1 community.
// Q = 6/6 − (12/12)² = 1 − 1 = 0.
//
// Edges (directed, no reverses): A→B, A→C, A→D, B→C, B→D, C→D.

#[test]
fn k4_complete_single_community_q_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);
    add_node(MOD_VEC_D, MOD_KEY_D, MOD_ID_D);
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t5");
    add_edge(MOD_ID_A, MOD_ID_C, "mod.ac.t5");
    add_edge(MOD_ID_A, MOD_ID_D, "mod.ad.t5");
    add_edge(MOD_ID_B, MOD_ID_C, "mod.bc.t5");
    add_edge(MOD_ID_B, MOD_ID_D, "mod.bd.t5");
    add_edge(MOD_ID_C, MOD_ID_D, "mod.cd.t5");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     0, "K4: 1 community, Q=0");
    assert_eq!(comms, 1, "K4 is fully connected → one community");
    assert_eq!(edges, 6, "K4 has 6 undirected edges");
    assert_eq!(nodes, 4, "4 nodes");
}

// ── 6. Two isolated pairs {A–B} and {C–D} — Q = 0.5 ─────────────────────────
//
// Two disconnected K2 components. LPA: {A,B} → community 1, {C,D} → community 2.
// m=2; L_0=1, d_0=2; L_1=1, d_1=2.
// Q_ppm = (4·2·2 − (4+4)) × 10⁶ / (4·4) = (16−8)·10⁶/16 = 500_000.

#[test]
fn two_isolated_pairs_q_500000() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);
    add_node(MOD_VEC_D, MOD_KEY_D, MOD_ID_D);
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t6");
    add_edge(MOD_ID_C, MOD_ID_D, "mod.cd.t6");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     500_000, "two isolated pairs: Q = 0.5 → 500_000 ppm");
    assert_eq!(comms, 2,       "2 communities: {{A,B}} and {{C,D}}");
    assert_eq!(edges, 2,       "2 undirected edges");
    assert_eq!(nodes, 4,       "4 nodes");
}

// ── 7. Two K3 cliques disconnected — Q = 0.5 ─────────────────────────────────
//
// {A,B,C} and {D,E,F} each form a K3 with no cross edges.
// LPA: 2 communities. m=6; L_0=L_1=3; d_0=d_1=6.
// Q_ppm = (4·6·6 − (36+36)) × 10⁶ / (4·36) = (144−72)·10⁶/144 = 500_000.
//
// Edges per clique (directed, one direction per pair):
//   K3_1: A→B, B→C, A→C   K3_2: D→E, E→F, D→F

#[test]
fn two_k3_cliques_q_500000() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);
    add_node(MOD_VEC_D, MOD_KEY_D, MOD_ID_D);
    add_node(MOD_VEC_E, MOD_KEY_E, MOD_ID_E);
    add_node(MOD_VEC_F, MOD_KEY_F, MOD_ID_F);
    // K3 clique 1
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t7");
    add_edge(MOD_ID_B, MOD_ID_C, "mod.bc.t7");
    add_edge(MOD_ID_A, MOD_ID_C, "mod.ac.t7");
    // K3 clique 2
    add_edge(MOD_ID_D, MOD_ID_E, "mod.de.t7");
    add_edge(MOD_ID_E, MOD_ID_F, "mod.ef.t7");
    add_edge(MOD_ID_D, MOD_ID_F, "mod.df.t7");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     500_000, "two K3 cliques: Q = 0.5 → 500_000 ppm");
    assert_eq!(comms, 2,       "2 communities: {{A,B,C}} and {{D,E,F}}");
    assert_eq!(edges, 6,       "6 undirected edges (3 per clique)");
    assert_eq!(nodes, 6,       "6 nodes");
}

// ── 8. K3 clique + K2 pair (disconnected) — Q = 375_000 ─────────────────────
//
// {A,B,C} K3 (3 edges) + {D,E} pair (1 edge); 4 undirected edges total.
// LPA: {A,B,C} community 1, {D,E} community 2.
// m=4; L_0=3 d_0=6 d_0²=36; L_1=1 d_1=2 d_1²=4; Σd²=40.
// Q_ppm = (4·4·4 − 40) × 10⁶ / (4·16) = 24·10⁶/64 = 375_000.

#[test]
fn k3_plus_k2_q_375000() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);
    add_node(MOD_VEC_D, MOD_KEY_D, MOD_ID_D);
    add_node(MOD_VEC_E, MOD_KEY_E, MOD_ID_E);
    // K3
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t8");
    add_edge(MOD_ID_B, MOD_ID_C, "mod.bc.t8");
    add_edge(MOD_ID_A, MOD_ID_C, "mod.ac.t8");
    // K2
    add_edge(MOD_ID_D, MOD_ID_E, "mod.de.t8");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     375_000, "K3+K2 disconnected: Q_ppm=375_000");
    assert_eq!(comms, 2,       "2 communities: {{A,B,C}} and {{D,E}}");
    assert_eq!(edges, 4,       "4 undirected edges (3+1)");
    assert_eq!(nodes, 5,       "5 nodes");
}

// ── 9. Mutual pair A↔B + isolated node C ─────────────────────────────────────
//
// Directed edges A→B and B→A count as ONE undirected edge {A,B} (m=1).
// LPA: {A,B} form one community; C is isolated (its own community).
// d_{AB} = 2, d_{AB}² = 4; d_C = 0, d_C² = 0; Σd² = 4.
// Q_ppm = (4·1·1 − 4) × 10⁶ / (4·1) = 0 — C adds a second community but
// contributes no edges or degree, so Q is unchanged from single-edge case.

#[test]
fn mutual_pair_plus_isolated_two_comms_q_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A);
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t9");
    add_edge(MOD_ID_B, MOD_ID_A, "mod.ba.t9");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     0, "A\u{2194}B + isolated C: Q=0 (C has no edges/degree)");
    assert_eq!(comms, 2, "2 communities: {{A,B}} and {{C}}");
    assert_eq!(edges, 1, "A\u{2194}B = 1 undirected edge (pair deduplicated)");
    assert_eq!(nodes, 3, "3 nodes");
}

// ── 10. Star: hub A → B, C, D (all leaves, one community) ─────────────────────
//
// Hub has degree 3; each leaf has degree 1. LPA: all merge into one community
// (hub's label propagates to all leaves in first iteration).
// m=3; L_0=3; d_0=3+1+1+1=6; d_0²=36.
// Q_ppm = (4·3·3 − 36) × 10⁶ / (4·9) = (36−36)·10⁶/36 = 0.

#[test]
fn star_graph_single_community_q_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MOD_VEC_A, MOD_KEY_A, MOD_ID_A); // hub
    add_node(MOD_VEC_B, MOD_KEY_B, MOD_ID_B);
    add_node(MOD_VEC_C, MOD_KEY_C, MOD_ID_C);
    add_node(MOD_VEC_D, MOD_KEY_D, MOD_ID_D);
    add_edge(MOD_ID_A, MOD_ID_B, "mod.ab.t10");
    add_edge(MOD_ID_A, MOD_ID_C, "mod.ac.t10");
    add_edge(MOD_ID_A, MOD_ID_D, "mod.ad.t10");

    let (q, comms, edges, nodes) = run_modq();
    assert_eq!(q,     0, "star graph: 1 community, Q=0");
    assert_eq!(comms, 1, "LPA merges hub and all leaves");
    assert_eq!(edges, 3, "3 undirected spoke edges");
    assert_eq!(nodes, 4, "4 nodes (hub + 3 leaves)");
}
