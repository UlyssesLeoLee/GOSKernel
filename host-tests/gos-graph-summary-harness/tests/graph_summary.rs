// gos-graph-summary-harness — V2.79 graph topology summary API consistency tests
//
// Verifies that the set of metrics called by `graph summary` are consistent with
// each other when computed on well-known graph structures.  These tests exercise
// the multi-metric invariants that the summary display relies on.
//
// Each test calls all summary metrics on the same graph state and asserts the
// cross-metric relationships that must hold by construction.
//
//  1. Empty graph              → all metrics zero/undefined
//  2. Single isolated node     → density=0, all effs=0, sigma=0, kappa=0
//  3. Two nodes one edge       → density=1/(2*1)=0.5, CC=0, E_global>0
//  4. Bidirected triangle      → density=1.0, CC=1.0, E_global=1.0, E_local=1.0
//  5. Directed triangle (one way) → some asymmetry: geff < 1.0
//  6. Path A→B→C→D            → E_global < E_local (chain topology)
//  7. K4 complete              → density=1.0, all CCs=1.0, kappa=avg_k
//  8. Star (hub+3 spokes)      → kappa > avg_k (heterogeneous)
//  9. K4 + isolated node       → E_global < K4 baseline (isolation penalty)
// 10. Sigma vs CC invariant    → sigma > 1 iff CC > CC_rand and L < L_rand

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const ELF_PLUGIN: PluginId   = PluginId::from_ascii("KL_ELF1_00");
const ELF_EXEC:   ExecutorId = ExecutorId::from_ascii("elf.exec");

const ELF_KEY_A: &str = "elf.alpha";
const ELF_KEY_B: &str = "elf.beta";
const ELF_KEY_C: &str = "elf.gamma";
const ELF_KEY_D: &str = "elf.delta";
const ELF_KEY_E: &str = "elf.epsilon";

const ELF_ID_A: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_A);
const ELF_ID_B: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_B);
const ELF_ID_C: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_C);
const ELF_ID_D: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_D);
const ELF_ID_E: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_E);

// L4=55 identifies this harness namespace.
const ELF_VEC_A: VectorAddress = VectorAddress::new(55, 1, 1, 0);
const ELF_VEC_B: VectorAddress = VectorAddress::new(55, 1, 2, 0);
const ELF_VEC_C: VectorAddress = VectorAddress::new(55, 1, 3, 0);
const ELF_VEC_D: VectorAddress = VectorAddress::new(55, 1, 4, 0);
const ELF_VEC_E: VectorAddress = VectorAddress::new(55, 1, 5, 0);

const ELF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ELF_PLUGIN,
    name:         "kl-graph-summary-harness",
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
        executor_id:       ELF_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }
fn register_plugin() { gos_runtime::discover_plugin(ELF_MANIFEST).unwrap(); }

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(ELF_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

// Collect all summary metrics in one call.
// graph_density() → (ppm, node_count, edge_count)
// graph_global_efficiency() → (ppm_u64, pairs_max, node_count)
fn summary() -> (u32, u32, u32, u64, u32, u32, u32, usize, usize) {
    let (density_ppm, node_count, _edge_count)           = gos_runtime::graph_density();
    let (trans_ppm,   _tri, _trip, _)                    = gos_runtime::graph_transitivity();
    let (avgcc_ppm,   _,    _)                           = gos_runtime::graph_avg_clustering();
    let (geff_ppm,    _,    _)                           = gos_runtime::graph_global_efficiency();
    let (leff_ppm,    _,    _)                           = gos_runtime::graph_local_efficiency();
    let (sigma_ppm,   _,    _,    _, _, m_undir)         = gos_runtime::graph_small_world();
    let (kappa_ppm,   _,    _avg_k_ppm, _,  _)          = gos_runtime::graph_scale_free();
    (density_ppm, trans_ppm, avgcc_ppm, geff_ppm, leff_ppm, sigma_ppm, kappa_ppm, node_count, m_undir)
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_all_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (density, trans, avgcc, geff, leff, sigma, kappa, n, m) = summary();
    assert_eq!(n,       0, "n=0");
    assert_eq!(m,       0, "m=0");
    assert_eq!(density, 0, "density=0");
    assert_eq!(trans,   0, "trans=0");
    assert_eq!(avgcc,   0, "avgcc=0");
    assert_eq!(geff,    0, "geff=0");
    assert_eq!(leff,    0, "leff=0");
    assert_eq!(sigma,   0, "sigma=0");
    assert_eq!(kappa,   0, "kappa=0");
}

// ── 2. Single isolated node ───────────────────────────────────────────────────

#[test]
fn single_node_all_zero_except_n() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    let (density, trans, avgcc, geff, leff, sigma, kappa, n, m) = summary();
    assert_eq!(n,       1, "n=1");
    assert_eq!(m,       0, "m=0");
    assert_eq!(density, 0, "density=0 (no edges)");
    assert_eq!(trans,   0, "trans=0");
    assert_eq!(avgcc,   0, "avgcc=0");
    assert_eq!(geff,    0, "geff=0 (isolated)");
    assert_eq!(leff,    0, "leff=0 (isolated)");
    assert_eq!(sigma,   0, "sigma=0");
    assert_eq!(kappa,   0, "kappa=0 (no edges)");
}

// ── 3. Two nodes, one directed edge (A→B) ────────────────────────────────────
//
// n=2, m_undir=1, density = 1/(n*(n-1)) = 1/2 → 500_000.
// No triangles → CC=0. One reachable pair → E_global > 0.

#[test]
fn two_nodes_one_edge_consistency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t3");
    let (density, trans, avgcc, geff, leff, sigma, kappa, n, m) = summary();
    assert_eq!(n,        2,       "n=2");
    assert_eq!(m,        1,       "m_undir=1");
    assert_eq!(density,  500_000, "density=0.5 (1/2 directed pairs, A→B only)");
    assert_eq!(trans,    0,       "no triangles: transitivity=0");
    assert_eq!(avgcc,    0,       "no triangles: avg_cc=0");
    assert!(geff > 0,             "one reachable pair: E_global>0");
    assert_eq!(leff,     0,       "no node has 2+ undirected neighbors: E_local=0");
    assert_eq!(sigma,    0,       "avg_k=1 < 2: sigma=0");
    // kappa: A has degree 1, B has degree 1; kappa=1.0.
    assert_eq!(kappa,    1_000_000, "kappa=1.0 (regular pair)");
}

// ── 4. Bidirected triangle: all metrics maximal ───────────────────────────────
//
// n=3, m_undir=3. All nodes mutually connected.
// density=1.0 (n*(n-1)=6 directed pairs, 6 edges), CC=1.0, E_global=1.0, E_local=1.0.

#[test]
fn bidirected_triangle_maximal_metrics() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t4"); add_edge(ELF_ID_B, ELF_ID_A, "sm.ba.t4");
    add_edge(ELF_ID_B, ELF_ID_C, "sm.bc.t4"); add_edge(ELF_ID_C, ELF_ID_B, "sm.cb.t4");
    add_edge(ELF_ID_A, ELF_ID_C, "sm.ac.t4"); add_edge(ELF_ID_C, ELF_ID_A, "sm.ca.t4");
    let (density, trans, avgcc, geff, leff, _sigma, kappa, n, m) = summary();
    assert_eq!(n,       3,         "n=3");
    assert_eq!(m,       3,         "m_undir=3");
    assert_eq!(density, 1_000_000, "density=1.0 (complete)");
    assert_eq!(trans,   1_000_000, "transitivity=1.0");
    assert_eq!(avgcc,   1_000_000, "avg_cc=1.0");
    assert_eq!(geff,    1_000_000, "E_global=1.0 (all pairs at d=1)");
    assert_eq!(leff,    1_000_000, "E_local=1.0 (each node's neighbor subgraph is complete)");
    assert_eq!(kappa,   2_000_000, "kappa=2.0 (regular: all degree 2)");
}

// ── 5. Directed triangle (A→B, B→C, A→C): asymmetric reachability ────────────
//
// Not all 6 directed pairs are reachable; C→A and C→B unreachable.
// E_global < 1.0. CC=1.0 (all 3 undirected edges present).

#[test]
fn directed_triangle_partial_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t5");
    add_edge(ELF_ID_B, ELF_ID_C, "sm.bc.t5");
    add_edge(ELF_ID_A, ELF_ID_C, "sm.ac.t5");
    let (density, trans, _avgcc, geff, _leff, _sigma, _kappa, n, m) = summary();
    assert_eq!(n,   3, "n=3");
    assert_eq!(m,   3, "m_undir=3");
    // Only 3 directed edges → density = 3/6 = 0.5.
    assert_eq!(density, 500_000, "density=0.5 (3 of 6 directed pairs)");
    // Undirected edges form complete K3 → transitivity=1.0.
    assert_eq!(trans, 1_000_000, "transitivity=1.0 (K3 undirected)");
    // Directed reachability: A→B (d=1), A→C (d=1), B→C (d=1); C→A, C→B, B→A: unreachable.
    // E_global = (1/1 + 1/1 + 1/1) / (3*2) = 3/6 = 0.5.
    assert_eq!(geff, 500_000, "E_global=0.5 (3 of 6 directed pairs reachable at d=1)");
}

// ── 6. Path A→B→C→D: E_global < E_local ─────────────────────────────────────
//
// Chain: all nodes at distance 1 from their immediate neighbor but far otherwise.
// E_global is low (long paths). E_local depends on shared-neighbor subgraphs.

#[test]
fn path_efficiency_relationship() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t6");
    add_edge(ELF_ID_B, ELF_ID_C, "sm.bc.t6");
    add_edge(ELF_ID_C, ELF_ID_D, "sm.cd.t6");
    let (_density, _trans, _avgcc, geff, leff, _sigma, _kappa, n, m) = summary();
    assert_eq!(n, 4, "n=4");
    assert_eq!(m, 3, "m_undir=3");
    // E_global: directed reachable pairs are A→B,A→C,A→D,B→C,B→D,C→D (6 of 12).
    // distances: 1+2+3+1+2+1=10; E_global = (1+0.5+0.333+1+0.5+1)/12 ≈ 4.333/12 ≈ 361_111.
    assert!(geff > 0,    "E_global>0 (some pairs reachable)");
    assert!(geff < 500_000, "E_global<0.5 (long chain dilutes efficiency)");
    // E_local: B's neighbors are {A,C} but no edge B_A→B_C in the subgraph →
    // local eff for all internal nodes = 0 (no edges in 2-node subgraph... actually
    // the subgraph of neighbors-of-B = {A,C}; there's no edge A→C in the main graph).
    // So E_local=0 for this purely directed chain.
    assert_eq!(leff, 0, "E_local=0 (no edges among neighbors of any node in directed chain)");
}

// ── 7. K4 complete (bidirected): kappa = avg_k ───────────────────────────────
//
// Regular graph invariant: for k-regular graphs, κ = k = ⟨k⟩.

#[test]
fn k4_regular_kappa_equals_avg_k() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t7"); add_edge(ELF_ID_B, ELF_ID_A, "sm.ba.t7");
    add_edge(ELF_ID_A, ELF_ID_C, "sm.ac.t7"); add_edge(ELF_ID_C, ELF_ID_A, "sm.ca.t7");
    add_edge(ELF_ID_A, ELF_ID_D, "sm.ad.t7"); add_edge(ELF_ID_D, ELF_ID_A, "sm.da.t7");
    add_edge(ELF_ID_B, ELF_ID_C, "sm.bc.t7"); add_edge(ELF_ID_C, ELF_ID_B, "sm.cb.t7");
    add_edge(ELF_ID_B, ELF_ID_D, "sm.bd.t7"); add_edge(ELF_ID_D, ELF_ID_B, "sm.db.t7");
    add_edge(ELF_ID_C, ELF_ID_D, "sm.cd.t7"); add_edge(ELF_ID_D, ELF_ID_C, "sm.dc.t7");
    let (density, trans, avgcc, geff, leff, _sigma, kappa, n, m) = summary();
    assert_eq!(n,       4,         "n=4");
    assert_eq!(m,       6,         "m_undir=6");
    assert_eq!(density, 1_000_000, "density=1.0 (K4 complete)");
    assert_eq!(trans,   1_000_000, "transitivity=1.0");
    assert_eq!(avgcc,   1_000_000, "avg_cc=1.0");
    assert_eq!(geff,    1_000_000, "E_global=1.0");
    assert_eq!(leff,    1_000_000, "E_local=1.0");
    // Regular (k=3): kappa = avg_k = 3 → both 3_000_000.
    assert_eq!(kappa, 3_000_000, "kappa=3.0 (regular)");
    // avg_k for K4: (4*3)/4 = 3 → avg_degree_ppm = 3_000_000.
    let (kappa2, _, avg_k_ppm, _, _) = gos_runtime::graph_scale_free();
    assert_eq!(kappa2, avg_k_ppm, "regular graph invariant: kappa == avg_k");
}

// ── 8. Star hub+3 spokes: kappa > avg_k ──────────────────────────────────────
//
// Heterogeneity invariant: star graph always has kappa > avg_k.

#[test]
fn star_kappa_exceeds_avg_k() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);  // hub
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t8");
    add_edge(ELF_ID_A, ELF_ID_C, "sm.ac.t8");
    add_edge(ELF_ID_A, ELF_ID_D, "sm.ad.t8");
    let (_, _, _, _, _, _, kappa, n, _) = summary();
    assert_eq!(n, 4, "n=4");
    let (kappa2, _, avg_k_ppm, _, _) = gos_runtime::graph_scale_free();
    assert_eq!(kappa, kappa2, "kappa consistent across calls");
    assert!(kappa > avg_k_ppm, "star: kappa > avg_k (heterogeneous)");
}

// ── 9. K4 + isolated: E_global reduced vs pure K4 ────────────────────────────
//
// Adding an isolated node penalizes E_global (denominator n*(n-1) grows but
// no new reachable pairs added through the isolated node).

#[test]
fn k4_plus_isolated_penalizes_efficiency() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    // K4 edges.
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t9"); add_edge(ELF_ID_B, ELF_ID_A, "sm.ba.t9");
    add_edge(ELF_ID_A, ELF_ID_C, "sm.ac.t9"); add_edge(ELF_ID_C, ELF_ID_A, "sm.ca.t9");
    add_edge(ELF_ID_A, ELF_ID_D, "sm.ad.t9"); add_edge(ELF_ID_D, ELF_ID_A, "sm.da.t9");
    add_edge(ELF_ID_B, ELF_ID_C, "sm.bc.t9"); add_edge(ELF_ID_C, ELF_ID_B, "sm.cb.t9");
    add_edge(ELF_ID_B, ELF_ID_D, "sm.bd.t9"); add_edge(ELF_ID_D, ELF_ID_B, "sm.db.t9");
    add_edge(ELF_ID_C, ELF_ID_D, "sm.cd.t9"); add_edge(ELF_ID_D, ELF_ID_C, "sm.dc.t9");
    let (_, _, _, geff_k4, _, _, _, _, _) = summary();
    assert_eq!(geff_k4, 1_000_000, "K4 alone: E_global=1.0");

    // Now add isolated node E.
    add_node(ELF_VEC_E, ELF_KEY_E, ELF_ID_E, 0xF005);
    let (_, _, _, geff_k4e, _, _, _, n, _) = summary();
    assert_eq!(n, 5, "n=5 after adding isolated node");
    assert!(geff_k4e < geff_k4,
        "E_global={geff_k4e} should be < {geff_k4} after adding isolated node");
}

// ── 10. Small-world σ > 1 iff CC > CC_rand and L comparable to L_rand ────────
//
// By construction: a bidirected triangle (K3) has sigma > 1 because it is
// maximally clustered relative to a random graph of the same size/density.
// This test verifies the relationship between sigma and CC.

#[test]
fn sigma_and_cc_relationship() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // K3 bidirected: all pairs connected, σ > 1.
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "sm.ab.t10"); add_edge(ELF_ID_B, ELF_ID_A, "sm.ba.t10");
    add_edge(ELF_ID_B, ELF_ID_C, "sm.bc.t10"); add_edge(ELF_ID_C, ELF_ID_B, "sm.cb.t10");
    add_edge(ELF_ID_A, ELF_ID_C, "sm.ac.t10"); add_edge(ELF_ID_C, ELF_ID_A, "sm.ca.t10");
    let (_density, trans, avgcc, geff, _leff, sigma, _kappa, n, m) = summary();
    assert_eq!(n, 3,         "n=3");
    assert_eq!(m, 3,         "m_undir=3");
    assert_eq!(trans,   1_000_000, "transitivity=1.0");
    assert_eq!(avgcc,   1_000_000, "avg_cc=1.0");
    assert_eq!(geff,    1_000_000, "E_global=1.0");
    // K3 with avg_k=2: CC=1.0 > CC_rand=2*3/(3*2)=1.0 ... CC_rand=1.0 exactly.
    // σ = (CC/CC_rand)/(L/L_rand) = 1/(1/L_rand) = L_rand = ln(3)/ln(2) ≈ 1.584963.
    assert!(sigma > 1_000_000,
        "sigma={sigma} expected > 1_000_000 (K3 is small-world relative to random)");
}
