// gos-graph-power-law-harness — V2.80 power-law exponent MLE API tests
//
// Verifies `gos_runtime::graph_power_law`.
//
// γ̂ = 1 + n_fit × [Σ_{k_i ≥ 1} ln(k_i)]^{-1}  (Clauset–Newman–Shalizi 2009)
// Returns (gamma_ppm, n_fit, node_count).
//
// LN_TABLE constants used in calculations (ln(k) × 1_000_000):
//   LN[1]=0  LN[2]=693_147  LN[3]=1_098_612  LN[4]=1_386_294  LN[6]=1_791_759
//
//  1. Empty graph                        → (0, 0, 0)
//  2. Single isolated node               → (0, 0, 1) [k=0, n_fit=0]
//  3. Two nodes, one edge (A→B)          → k={1,1}; sum_ln=0 → gamma=0 (undefined)
//  4. Bidirected triangle K3             → k={2,2,2}; gamma ≈ 2.44
//  5. Complete K4 bidirected             → k={3,3,3,3}; gamma ≈ 1.91
//  6. Directed chain A→B→C→D            → k={1,2,2,1}; gamma ≈ 3.89
//  7. Star hub→3 spokes                  → k={3,1,1,1}; sum_ln=LN[3]; gamma ≈ 4.64
//  8. Star hub→6 spokes                  → k={6,1,1,1,1,1,1}; gamma ≈ 4.91
//  9. K4 + isolated node E               → n_fit=4; gamma same as K4 ≈ 1.91
// 10. Mixed-degree graph with isolated   → gamma ∈ [2, 3] range

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const ELF_PLUGIN: PluginId   = PluginId::from_ascii("KL_ELF0_00");
const ELF_EXEC:   ExecutorId = ExecutorId::from_ascii("elf.exec");

const ELF_KEY_A: &str = "elf.alpha";
const ELF_KEY_B: &str = "elf.beta";
const ELF_KEY_C: &str = "elf.gamma";
const ELF_KEY_D: &str = "elf.delta";
const ELF_KEY_E: &str = "elf.epsilon";
const ELF_KEY_F: &str = "elf.zeta";
const ELF_KEY_G: &str = "elf.eta";

const ELF_ID_A: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_A);
const ELF_ID_B: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_B);
const ELF_ID_C: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_C);
const ELF_ID_D: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_D);
const ELF_ID_E: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_E);
const ELF_ID_F: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_F);
const ELF_ID_G: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_G);

// L4=56 identifies this harness namespace in the VectorAddress space.
const ELF_VEC_A: VectorAddress = VectorAddress::new(56, 1, 1, 0);
const ELF_VEC_B: VectorAddress = VectorAddress::new(56, 1, 2, 0);
const ELF_VEC_C: VectorAddress = VectorAddress::new(56, 1, 3, 0);
const ELF_VEC_D: VectorAddress = VectorAddress::new(56, 1, 4, 0);
const ELF_VEC_E: VectorAddress = VectorAddress::new(56, 1, 5, 0);
const ELF_VEC_F: VectorAddress = VectorAddress::new(56, 1, 6, 0);
const ELF_VEC_G: VectorAddress = VectorAddress::new(56, 1, 7, 0);

const ELF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ELF_PLUGIN,
    name:         "kl-graph-power-law-harness",
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

fn reset() {
    gos_runtime::reset();
}

fn register_plugin() {
    gos_runtime::discover_plugin(ELF_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(ELF_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     0, "empty: node_count=0");
    assert_eq!(n_fit, 0, "empty: n_fit=0");
    assert_eq!(gamma, 0, "empty: gamma=0 (undefined)");
}

// ── 2. Single isolated node (k=0, excluded from fit) ─────────────────────────

#[test]
fn single_isolated_node_undefined() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     1, "1 node");
    assert_eq!(n_fit, 0, "isolated: not included in fit");
    assert_eq!(gamma, 0, "isolated: gamma=0 (undefined)");
}

// ── 3. Two nodes, one directed edge — both have k=1 (sum_ln=0 → undefined) ───
//
// k_A=1, k_B=1; LN[1]=0, so sum_ln_ppm=0.
// MLE is degenerate: γ̂ → ∞ is reported as 0.

#[test]
fn two_nodes_k_one_undefined() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t3");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     2, "2 nodes");
    assert_eq!(n_fit, 2, "both have k=1 (non-isolated)");
    assert_eq!(gamma, 0, "sum_ln=0: MLE undefined (all k=1)");
}

// ── 4. Bidirected triangle K3 ─────────────────────────────────────────────────
//
// k = {2, 2, 2}; sum_ln = 3 × LN[2] = 3 × 693_147 = 2_079_441
// gamma = 1_000_000 + 3 × 10^12 / 2_079_441 = 1_000_000 + 1_442_695 = 2_442_695

#[test]
fn bidirected_triangle_gamma_approx_2_44() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t4"); add_edge(ELF_ID_B, ELF_ID_A, "pl.ba.t4");
    add_edge(ELF_ID_B, ELF_ID_C, "pl.bc.t4"); add_edge(ELF_ID_C, ELF_ID_B, "pl.cb.t4");
    add_edge(ELF_ID_A, ELF_ID_C, "pl.ac.t4"); add_edge(ELF_ID_C, ELF_ID_A, "pl.ca.t4");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     3, "3 nodes");
    assert_eq!(n_fit, 3, "all 3 have k=2 ≥ 1");
    // sum_ln = 3 × 693_147 = 2_079_441; gamma = 1_000_000 + 1_442_695 = 2_442_695
    assert!(gamma >= 2_442_000 && gamma <= 2_443_000,
        "gamma_ppm={gamma} expected ≈2_442_695");
}

// ── 5. Complete K4 bidirected ─────────────────────────────────────────────────
//
// k = {3, 3, 3, 3}; sum_ln = 4 × LN[3] = 4 × 1_098_612 = 4_394_448
// gamma = 1_000_000 + 4 × 10^12 / 4_394_448 = 1_000_000 + 910_467 = 1_910_467

#[test]
fn complete_k4_gamma_approx_1_91() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t5"); add_edge(ELF_ID_B, ELF_ID_A, "pl.ba.t5");
    add_edge(ELF_ID_A, ELF_ID_C, "pl.ac.t5"); add_edge(ELF_ID_C, ELF_ID_A, "pl.ca.t5");
    add_edge(ELF_ID_A, ELF_ID_D, "pl.ad.t5"); add_edge(ELF_ID_D, ELF_ID_A, "pl.da.t5");
    add_edge(ELF_ID_B, ELF_ID_C, "pl.bc.t5"); add_edge(ELF_ID_C, ELF_ID_B, "pl.cb.t5");
    add_edge(ELF_ID_B, ELF_ID_D, "pl.bd.t5"); add_edge(ELF_ID_D, ELF_ID_B, "pl.db.t5");
    add_edge(ELF_ID_C, ELF_ID_D, "pl.cd.t5"); add_edge(ELF_ID_D, ELF_ID_C, "pl.dc.t5");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     4, "4 nodes");
    assert_eq!(n_fit, 4, "all 4 have k=3 ≥ 1");
    // sum_ln = 4 × 1_098_612 = 4_394_448; gamma = 1_000_000 + 910_467 = 1_910_467
    assert!(gamma >= 1_910_000 && gamma <= 1_911_000,
        "gamma_ppm={gamma} expected ≈1_910_467");
}

// ── 6. Directed chain A→B→C→D ────────────────────────────────────────────────
//
// Undirected degrees: A=1, B=2, C=2, D=1.
// sum_ln = LN[1] + LN[2] + LN[2] + LN[1] = 0 + 693_147 + 693_147 + 0 = 1_386_294
// gamma = 1_000_000 + 4 × 10^12 / 1_386_294 = 1_000_000 + 2_885_390 = 3_885_390

#[test]
fn directed_chain_abcd_gamma_approx_3_89() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t6");
    add_edge(ELF_ID_B, ELF_ID_C, "pl.bc.t6");
    add_edge(ELF_ID_C, ELF_ID_D, "pl.cd.t6");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     4, "4 nodes");
    assert_eq!(n_fit, 4, "all 4 have k ≥ 1");
    // sum_ln = 1_386_294; gamma ≈ 3_885_390
    assert!(gamma >= 3_884_000 && gamma <= 3_886_000,
        "gamma_ppm={gamma} expected ≈3_885_390");
}

// ── 7. Star: hub A→{B, C, D} (hub k=3, spokes k=1) ──────────────────────────
//
// sum_ln = LN[3] + 3×LN[1] = 1_098_612 + 0 = 1_098_612
// gamma = 1_000_000 + 4 × 10^12 / 1_098_612 = 1_000_000 + 3_640_957 = 4_640_957

#[test]
fn star_3spokes_gamma_approx_4_64() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);   // hub
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t7");
    add_edge(ELF_ID_A, ELF_ID_C, "pl.ac.t7");
    add_edge(ELF_ID_A, ELF_ID_D, "pl.ad.t7");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     4, "4 nodes");
    assert_eq!(n_fit, 4, "all 4 have k ≥ 1");
    // sum_ln = 1_098_612 (hub only contributes); gamma ≈ 4_640_957
    assert!(gamma >= 4_640_000 && gamma <= 4_642_000,
        "gamma_ppm={gamma} expected ≈4_640_957");
}

// ── 8. Star: hub A→{B,C,D,E,F,G} (hub k=6, 6 spokes k=1) ───────────────────
//
// sum_ln = LN[6] + 6×LN[1] = 1_791_759 + 0 = 1_791_759
// gamma = 1_000_000 + 7 × 10^12 / 1_791_759 = 1_000_000 + 3_906_775 = 4_906_775

#[test]
fn star_6spokes_gamma_approx_4_91() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);   // hub
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_node(ELF_VEC_E, ELF_KEY_E, ELF_ID_E, 0xF005);
    add_node(ELF_VEC_F, ELF_KEY_F, ELF_ID_F, 0xF006);
    add_node(ELF_VEC_G, ELF_KEY_G, ELF_ID_G, 0xF007);
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t8");
    add_edge(ELF_ID_A, ELF_ID_C, "pl.ac.t8");
    add_edge(ELF_ID_A, ELF_ID_D, "pl.ad.t8");
    add_edge(ELF_ID_A, ELF_ID_E, "pl.ae.t8");
    add_edge(ELF_ID_A, ELF_ID_F, "pl.af.t8");
    add_edge(ELF_ID_A, ELF_ID_G, "pl.ag.t8");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     7, "7 nodes (hub + 6 spokes)");
    assert_eq!(n_fit, 7, "all 7 have k ≥ 1");
    // sum_ln = 1_791_759; gamma ≈ 4_906_775
    assert!(gamma >= 4_905_000 && gamma <= 4_908_000,
        "gamma_ppm={gamma} expected ≈4_906_775");
}

// ── 9. K4 bidirected + isolated node E ───────────────────────────────────────
//
// k: A=3, B=3, C=3, D=3, E=0 (isolated, excluded from fit).
// n=5, n_fit=4 (E excluded); gamma = same as K4 alone.

#[test]
fn k4_plus_isolated_n_fit_excludes_isolated() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_node(ELF_VEC_E, ELF_KEY_E, ELF_ID_E, 0xF005);   // isolated
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t9"); add_edge(ELF_ID_B, ELF_ID_A, "pl.ba.t9");
    add_edge(ELF_ID_A, ELF_ID_C, "pl.ac.t9"); add_edge(ELF_ID_C, ELF_ID_A, "pl.ca.t9");
    add_edge(ELF_ID_A, ELF_ID_D, "pl.ad.t9"); add_edge(ELF_ID_D, ELF_ID_A, "pl.da.t9");
    add_edge(ELF_ID_B, ELF_ID_C, "pl.bc.t9"); add_edge(ELF_ID_C, ELF_ID_B, "pl.cb.t9");
    add_edge(ELF_ID_B, ELF_ID_D, "pl.bd.t9"); add_edge(ELF_ID_D, ELF_ID_B, "pl.db.t9");
    add_edge(ELF_ID_C, ELF_ID_D, "pl.cd.t9"); add_edge(ELF_ID_D, ELF_ID_C, "pl.dc.t9");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     5, "5 nodes (K4 + isolated E)");
    assert_eq!(n_fit, 4, "isolated E excluded from fit");
    // Same as K4: gamma ≈ 1_910_467
    assert!(gamma >= 1_910_000 && gamma <= 1_911_000,
        "gamma_ppm={gamma} expected ≈1_910_467 (same as K4)");
}

// ── 10. Mixed-degree graph: realistic gamma in [2, 3] ─────────────────────────
//
// Graph: hub A (k=4) connected to B,C,D,E; B–C bidirected (B,C each k=2);
//        D,E each k=1 (spoke only); F isolated (k=0).
// Edges: A→B, B→A, A→C, C→A, B→C, C→B, A→D, A→E.
// k: A=4, B=2, C=2, D=1, E=1, F=0.
// sum_ln = LN[4] + LN[2] + LN[2] + LN[1] + LN[1]
//        = 1_386_294 + 693_147 + 693_147 + 0 + 0 = 2_772_588
// n_fit = 5 (F excluded)
// gamma = 1_000_000 + 5 × 10^12 / 2_772_588 = 1_000_000 + 1_803_369 = 2_803_369

#[test]
fn mixed_degree_gamma_in_powerlaw_range() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);   // hub k=4
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);   // k=2
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);   // k=2
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);   // k=1
    add_node(ELF_VEC_E, ELF_KEY_E, ELF_ID_E, 0xF005);   // k=1
    add_node(ELF_VEC_F, ELF_KEY_F, ELF_ID_F, 0xF006);   // isolated k=0
    add_edge(ELF_ID_A, ELF_ID_B, "pl.ab.t10"); add_edge(ELF_ID_B, ELF_ID_A, "pl.ba.t10");
    add_edge(ELF_ID_A, ELF_ID_C, "pl.ac.t10"); add_edge(ELF_ID_C, ELF_ID_A, "pl.ca.t10");
    add_edge(ELF_ID_B, ELF_ID_C, "pl.bc.t10"); add_edge(ELF_ID_C, ELF_ID_B, "pl.cb.t10");
    add_edge(ELF_ID_A, ELF_ID_D, "pl.ad.t10");
    add_edge(ELF_ID_A, ELF_ID_E, "pl.ae.t10");
    let (gamma, n_fit, n) = gos_runtime::graph_power_law();
    assert_eq!(n,     6, "6 nodes (5 connected + 1 isolated)");
    assert_eq!(n_fit, 5, "F (isolated) excluded from fit");
    // sum_ln = 2_772_588; gamma ≈ 2_803_369
    assert!(gamma >= 2_802_000 && gamma <= 2_805_000,
        "gamma_ppm={gamma} expected ≈2_803_369");
    // Confirm gamma is in typical power-law range [2, 3].
    assert!(gamma >= 2_000_000 && gamma <= 3_000_000,
        "gamma={gamma} should be in [2, 3] ppm range for this mixed graph");
}
