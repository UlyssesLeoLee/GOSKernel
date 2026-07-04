// gos-graph-summary2-harness — V2.81 γ̂ in graph summary cross-metric invariants
//
// Verifies γ̂ (power-law exponent MLE) is consistent with κ (degree heterogeneity)
// and other summary metrics across well-known graph structures.
//
//  1. Empty graph              → gamma=0 (no nodes)
//  2. Single isolated node     → gamma=0 (n_fit=0, no non-isolated nodes)
//  3. Two nodes, one edge      → gamma=0 (all k=1, MLE degenerate)
//  4. Bidirected triangle      → gamma ∈ [1,3] (all k=2, typical WS graph)
//  5. K4 complete (bidirected) → gamma ∈ [1,3] (all k=3, regular)
//  6. K4 + isolated            → n_fit unchanged, gamma unchanged vs pure K4
//  7. Directed path A→B→C→D   → gamma ∈ (3,4] (mixed k=1,2 distribution)
//  8. Directed star (hub+3)    → gamma > 4 (hub k=3, leaves k=1; not power-law)
//  9. Regular graph invariant  → gamma in [1,3] correlates with kappa ≈ avg_k
// 10. n_fit = node_count - isolated_count invariant

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const ELF_PLUGIN: PluginId   = PluginId::from_ascii("KL_ELF1_01");
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

// L4=57 identifies this harness namespace.
const ELF_VEC_A: VectorAddress = VectorAddress::new(57, 1, 1, 0);
const ELF_VEC_B: VectorAddress = VectorAddress::new(57, 1, 2, 0);
const ELF_VEC_C: VectorAddress = VectorAddress::new(57, 1, 3, 0);
const ELF_VEC_D: VectorAddress = VectorAddress::new(57, 1, 4, 0);
const ELF_VEC_E: VectorAddress = VectorAddress::new(57, 1, 5, 0);

const ELF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ELF_PLUGIN,
    name:         "kl-graph-summary2-harness",
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

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_gamma_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 0, "n=0");
    assert_eq!(n_fit,      0, "n_fit=0");
    assert_eq!(gamma_ppm,  0, "gamma=0 (empty graph)");
}

// ── 2. Single isolated node ───────────────────────────────────────────────────

#[test]
fn single_isolated_gamma_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 1, "n=1");
    assert_eq!(n_fit,      0, "n_fit=0 (isolated: k=0 excluded)");
    assert_eq!(gamma_ppm,  0, "gamma=0 (no non-isolated nodes)");
}

// ── 3. Two nodes, one directed edge A→B → gamma=0 (all k=1, degenerate MLE) ──
//
// Both A and B have undirected degree 1. ln(1)=0, so sum_ln=0 → MLE degenerate.

#[test]
fn two_nodes_one_edge_gamma_degenerate() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t3");
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 2, "n=2");
    assert_eq!(n_fit,      2, "n_fit=2 (both have k=1)");
    assert_eq!(gamma_ppm,  0, "gamma=0 (all k=1: sum_ln=0, MLE undefined)");
}

// ── 4. Bidirected triangle → gamma ∈ [1,3] ───────────────────────────────────
//
// Each node has undirected degree 2.
// sum_ln = 3×ln(2) = 3×693_147 = 2_079_441 (ppm)
// gamma_ppm = 1_000_000 + 3×10^12/2_079_441 = 1_000_000 + 1_442_695 = 2_442_695

#[test]
fn bidirected_triangle_gamma_in_power_law_range() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t4"); add_edge(ELF_ID_B, ELF_ID_A, "s2.ba.t4");
    add_edge(ELF_ID_B, ELF_ID_C, "s2.bc.t4"); add_edge(ELF_ID_C, ELF_ID_B, "s2.cb.t4");
    add_edge(ELF_ID_A, ELF_ID_C, "s2.ac.t4"); add_edge(ELF_ID_C, ELF_ID_A, "s2.ca.t4");
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 3, "n=3");
    assert_eq!(n_fit,      3, "n_fit=3 (all k=2)");
    // gamma = 1 + 3/sum(ln(2)) = 1 + 3/2.079441 ≈ 2.443
    assert_eq!(gamma_ppm, 2_442_695, "gamma_ppm ≈ 2.443 for K3 bidirected");
    assert!(gamma_ppm > 1_000_000 && gamma_ppm <= 3_000_000,
        "gamma={gamma_ppm} should be in [1,3] range");
}

// ── 5. K4 complete (bidirected) → gamma ∈ [1,3] ─────────────────────────────
//
// Each node has undirected degree 3.
// sum_ln = 4×ln(3) = 4×1_098_612 = 4_394_448
// gamma_ppm = 1_000_000 + 4×10^12/4_394_448 = 1_000_000 + 910_228 = 1_910_228

#[test]
fn k4_complete_gamma_in_power_law_range() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t5"); add_edge(ELF_ID_B, ELF_ID_A, "s2.ba.t5");
    add_edge(ELF_ID_A, ELF_ID_C, "s2.ac.t5"); add_edge(ELF_ID_C, ELF_ID_A, "s2.ca.t5");
    add_edge(ELF_ID_A, ELF_ID_D, "s2.ad.t5"); add_edge(ELF_ID_D, ELF_ID_A, "s2.da.t5");
    add_edge(ELF_ID_B, ELF_ID_C, "s2.bc.t5"); add_edge(ELF_ID_C, ELF_ID_B, "s2.cb.t5");
    add_edge(ELF_ID_B, ELF_ID_D, "s2.bd.t5"); add_edge(ELF_ID_D, ELF_ID_B, "s2.db.t5");
    add_edge(ELF_ID_C, ELF_ID_D, "s2.cd.t5"); add_edge(ELF_ID_D, ELF_ID_C, "s2.dc.t5");
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 4, "n=4");
    assert_eq!(n_fit,      4, "n_fit=4 (all k=3)");
    // gamma = 1 + 4/sum(ln(3)) = 1 + 4/4.394448 ≈ 1.910
    assert_eq!(gamma_ppm, 1_910_239, "gamma_ppm ≈ 1.910 for K4 bidirected");
    assert!(gamma_ppm > 1_000_000 && gamma_ppm <= 3_000_000,
        "gamma={gamma_ppm} should be in [1,3] range");
}

// ── 6. K4 + isolated: n_fit unchanged, gamma unchanged ───────────────────────
//
// Adding an isolated node increments node_count but not n_fit (k=0 excluded).
// gamma_ppm must stay the same because isolated nodes don't contribute to MLE.

#[test]
fn k4_plus_isolated_nfit_and_gamma_unchanged() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t6"); add_edge(ELF_ID_B, ELF_ID_A, "s2.ba.t6");
    add_edge(ELF_ID_A, ELF_ID_C, "s2.ac.t6"); add_edge(ELF_ID_C, ELF_ID_A, "s2.ca.t6");
    add_edge(ELF_ID_A, ELF_ID_D, "s2.ad.t6"); add_edge(ELF_ID_D, ELF_ID_A, "s2.da.t6");
    add_edge(ELF_ID_B, ELF_ID_C, "s2.bc.t6"); add_edge(ELF_ID_C, ELF_ID_B, "s2.cb.t6");
    add_edge(ELF_ID_B, ELF_ID_D, "s2.bd.t6"); add_edge(ELF_ID_D, ELF_ID_B, "s2.db.t6");
    add_edge(ELF_ID_C, ELF_ID_D, "s2.cd.t6"); add_edge(ELF_ID_D, ELF_ID_C, "s2.dc.t6");
    let (gamma_k4, n_fit_k4, n_k4) = gos_runtime::graph_power_law();
    assert_eq!(n_k4,       4, "K4: n=4");
    assert_eq!(n_fit_k4,   4, "K4: n_fit=4");

    // Add isolated node E.
    add_node(ELF_VEC_E, ELF_KEY_E, ELF_ID_E, 0xF005);
    let (gamma_k4e, n_fit_k4e, n_k4e) = gos_runtime::graph_power_law();
    assert_eq!(n_k4e,     5, "K4+iso: n=5");
    assert_eq!(n_fit_k4e, 4, "K4+iso: n_fit=4 unchanged (isolated excluded)");
    assert_eq!(gamma_k4e, gamma_k4,
        "gamma unchanged after adding isolated node: {gamma_k4e} vs {gamma_k4}");
}

// ── 7. Directed path A→B→C→D → gamma ∈ (3,4] ────────────────────────────────
//
// Undirected degrees: A=1, B=2, C=2, D=1.
// sum_ln = 0 + ln(2) + ln(2) + 0 = 2×693_147 = 1_386_294
// gamma_ppm = 1_000_000 + 4×10^12/1_386_294 = 1_000_000 + 2_885_390 = 3_885_390

#[test]
fn directed_path_gamma_steep_tail() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t7");
    add_edge(ELF_ID_B, ELF_ID_C, "s2.bc.t7");
    add_edge(ELF_ID_C, ELF_ID_D, "s2.cd.t7");
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 4, "n=4");
    assert_eq!(n_fit,      4, "n_fit=4 (all k≥1)");
    // gamma = 1 + 4/(2×ln2) = 1 + 4/1.386294 ≈ 3.885
    assert_eq!(gamma_ppm, 3_885_390, "gamma_ppm ≈ 3.885 for directed path");
    assert!(gamma_ppm > 3_000_000 && gamma_ppm <= 4_000_000,
        "gamma={gamma_ppm} should be in (3,4] range (steep tail)");
}

// ── 8. Directed star (hub A→B,A→C,A→D) → gamma > 4 ─────────────────────────
//
// Undirected degrees: A=3, B=1, C=1, D=1.
// sum_ln = ln(3) + 0 + 0 + 0 = 1_098_612
// gamma_ppm = 1_000_000 + 4×10^12/1_098_612 = 1_000_000 + 3_640_871 = 4_640_871
// This is >4: not consistent with power-law degree distribution.

#[test]
fn directed_star_gamma_not_power_law() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);  // hub
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t8");
    add_edge(ELF_ID_A, ELF_ID_C, "s2.ac.t8");
    add_edge(ELF_ID_A, ELF_ID_D, "s2.ad.t8");
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 4, "n=4");
    assert_eq!(n_fit,      4, "n_fit=4 (all k≥1)");
    // gamma = 1 + 4/ln(3) ≈ 4.641
    assert_eq!(gamma_ppm, 4_640_957, "gamma_ppm ≈ 4.641 for directed star");
    assert!(gamma_ppm > 4_000_000,
        "gamma={gamma_ppm} should be >4 for small star (not power-law)");
}

// ── 9. Regular graph (K4): gamma ∈ [1,3] correlates with kappa = avg_k ───────
//
// For regular graphs κ = ⟨k⟩ (homogeneous), and γ̂ ∈ [1,3] indicates
// the degree distribution is compatible with a power-law tail.
// Together: regular + low γ confirms a tightly-coupled graph.

#[test]
fn regular_k4_gamma_and_kappa_correlated() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t9"); add_edge(ELF_ID_B, ELF_ID_A, "s2.ba.t9");
    add_edge(ELF_ID_A, ELF_ID_C, "s2.ac.t9"); add_edge(ELF_ID_C, ELF_ID_A, "s2.ca.t9");
    add_edge(ELF_ID_A, ELF_ID_D, "s2.ad.t9"); add_edge(ELF_ID_D, ELF_ID_A, "s2.da.t9");
    add_edge(ELF_ID_B, ELF_ID_C, "s2.bc.t9"); add_edge(ELF_ID_C, ELF_ID_B, "s2.cb.t9");
    add_edge(ELF_ID_B, ELF_ID_D, "s2.bd.t9"); add_edge(ELF_ID_D, ELF_ID_B, "s2.db.t9");
    add_edge(ELF_ID_C, ELF_ID_D, "s2.cd.t9"); add_edge(ELF_ID_D, ELF_ID_C, "s2.dc.t9");
    let (kappa_ppm, _, avg_k_ppm, _, _) = gos_runtime::graph_scale_free();
    let (gamma_ppm, _, _)               = gos_runtime::graph_power_law();
    // Regular graph invariant: kappa = avg_k.
    assert_eq!(kappa_ppm, avg_k_ppm, "K4 regular: kappa == avg_k");
    // γ̂ ∈ [1,3] for K4 (gamma ≈ 1.910).
    assert!(gamma_ppm > 1_000_000 && gamma_ppm <= 3_000_000,
        "gamma={gamma_ppm} should be in [1,3] for K4 (compatible with power-law)");
}

// ── 10. n_fit = node_count - isolated_count (structural invariant) ─────────────
//
// n_fit counts nodes with undirected degree ≥ 1 (non-isolated).
// node_count counts all alive nodes.
// Difference = isolated_count.

#[test]
fn nfit_equals_node_count_minus_isolated() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Connect A↔B, leave C and D isolated.
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);  // isolated
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);  // isolated
    add_edge(ELF_ID_A, ELF_ID_B, "s2.ab.t10");
    add_edge(ELF_ID_B, ELF_ID_A, "s2.ba.t10");
    let (gamma_ppm, n_fit, node_count) = gos_runtime::graph_power_law();
    assert_eq!(node_count, 4, "n=4 total");
    assert_eq!(n_fit,      2, "n_fit=2 (only A and B have k≥1)");
    // A and B both have k=1 (mutual pair bidirected) → sum_ln=0 → gamma=0 (degenerate).
    assert_eq!(gamma_ppm,  0, "gamma=0 (A and B both k=1; MLE degenerate)");
    // n_fit + isolated = node_count.
    let isolated_count = node_count - n_fit;
    assert_eq!(isolated_count, 2, "2 isolated nodes (C, D)");
}
