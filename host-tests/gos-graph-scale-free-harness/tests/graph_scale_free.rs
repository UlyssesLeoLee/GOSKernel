// gos-graph-scale-free-harness — V2.78 degree heterogeneity index API tests
//
// Verifies `gos_runtime::graph_scale_free`.
//
// κ = ⟨k²⟩/⟨k⟩  (undirected degree heterogeneity index)
// Returns (kappa_ppm, max_degree, avg_degree_ppm, node_count, m_undir).
//
//  1. Empty graph                       → all zeros
//  2. Single isolated node              → kappa=0 (no edges), max_k=0
//  3. Two isolated nodes                → kappa=0, max_k=0
//  4. Two nodes, one edge (A→B)        → k_A=1, k_B=1; kappa=1, avg=1
//  5. K3 regular (bidirected triangle) → k_v=2 all; kappa=2, avg=2
//  6. K4 complete (bidirected)         → k_v=3 all; kappa=3, avg=3
//  7. Star A→{B,C,D} undirected deg:  → hub k=3, spokes k=1; kappa > avg (heterogeneous)
//  8. Path A→B→C→D (directed chain)   → asymmetric degrees
//  9. K4 + isolated node               → kappa unchanged structure; max_k same
// 10. Star with 5 spokes               → scale-free signature: kappa >> avg

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

// L4=54 identifies this harness namespace in the VectorAddress space.
const ELF_VEC_A: VectorAddress = VectorAddress::new(54, 1, 1, 0);
const ELF_VEC_B: VectorAddress = VectorAddress::new(54, 1, 2, 0);
const ELF_VEC_C: VectorAddress = VectorAddress::new(54, 1, 3, 0);
const ELF_VEC_D: VectorAddress = VectorAddress::new(54, 1, 4, 0);
const ELF_VEC_E: VectorAddress = VectorAddress::new(54, 1, 5, 0);
const ELF_VEC_F: VectorAddress = VectorAddress::new(54, 1, 6, 0);
const ELF_VEC_G: VectorAddress = VectorAddress::new(54, 1, 7, 0);

const ELF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ELF_PLUGIN,
    name:         "kl-graph-scale-free-harness",
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
fn empty_graph_all_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     0, "empty: node_count=0");
    assert_eq!(m,     0, "empty: m_undir=0");
    assert_eq!(kappa, 0, "empty: kappa=0");
    assert_eq!(max_k, 0, "empty: max_degree=0");
    assert_eq!(avg_k, 0, "empty: avg_degree=0");
}

// ── 2. Single isolated node ───────────────────────────────────────────────────

#[test]
fn single_node_no_edges() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     1, "1 node");
    assert_eq!(m,     0, "no edges");
    assert_eq!(kappa, 0, "no edges: kappa=0");
    assert_eq!(max_k, 0, "no edges: max_k=0");
    assert_eq!(avg_k, 0, "no edges: avg_k=0");
}

// ── 3. Two isolated nodes ─────────────────────────────────────────────────────

#[test]
fn two_isolated_nodes() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     2, "2 nodes");
    assert_eq!(m,     0, "no edges");
    assert_eq!(kappa, 0, "no edges: kappa=0");
    assert_eq!(max_k, 0, "max_k=0");
    assert_eq!(avg_k, 0, "avg_k=0");
}

// ── 4. Two nodes, one directed edge (A→B) ────────────────────────────────────
//
// Undirected degree: A=1, B=1.
// sum_k=2, sum_k2=2, kappa=sum_k2/sum_k=1, avg_k=2/2=1.
// kappa_ppm = 1_000_000, avg_degree_ppm = 1_000_000.

#[test]
fn two_nodes_one_edge_regular() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_edge(ELF_ID_A, ELF_ID_B, "sf.ab.t4");
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,       2,         "2 nodes");
    assert_eq!(m,       1,         "1 undirected edge");
    assert_eq!(max_k,   1,         "max_k=1");
    // kappa = sum_k2/sum_k = 2/2 = 1 → ppm = 1_000_000
    assert_eq!(kappa,   1_000_000, "kappa=1.0 (regular)");
    // avg = sum_k/n = 2/2 = 1 → ppm = 1_000_000
    assert_eq!(avg_k,   1_000_000, "avg_k=1.0");
}

// ── 5. K3 bidirected triangle ─────────────────────────────────────────────────
//
// All 3 nodes have undirected degree 2.
// sum_k=6, sum_k2=12, kappa=12/6=2, avg_k=6/3=2.
// kappa_ppm = 2_000_000, avg_degree_ppm = 2_000_000 (homogeneous: kappa = avg).

#[test]
fn bidirected_triangle_regular() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "sf.ab.t5"); add_edge(ELF_ID_B, ELF_ID_A, "sf.ba.t5");
    add_edge(ELF_ID_B, ELF_ID_C, "sf.bc.t5"); add_edge(ELF_ID_C, ELF_ID_B, "sf.cb.t5");
    add_edge(ELF_ID_A, ELF_ID_C, "sf.ac.t5"); add_edge(ELF_ID_C, ELF_ID_A, "sf.ca.t5");
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     3,         "3 nodes");
    assert_eq!(m,     3,         "3 undirected edges");
    assert_eq!(max_k, 2,         "max_k=2");
    assert_eq!(kappa, 2_000_000, "kappa=2.0 (regular: kappa=avg_k)");
    assert_eq!(avg_k, 2_000_000, "avg_k=2.0");
}

// ── 6. K4 complete (bidirected) ───────────────────────────────────────────────
//
// All 4 nodes have undirected degree 3.
// sum_k=12, sum_k2=36, kappa=36/12=3, avg_k=12/4=3.
// Regular graph → kappa = avg_k.

#[test]
fn complete_k4_regular() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sf.ab.t6"); add_edge(ELF_ID_B, ELF_ID_A, "sf.ba.t6");
    add_edge(ELF_ID_A, ELF_ID_C, "sf.ac.t6"); add_edge(ELF_ID_C, ELF_ID_A, "sf.ca.t6");
    add_edge(ELF_ID_A, ELF_ID_D, "sf.ad.t6"); add_edge(ELF_ID_D, ELF_ID_A, "sf.da.t6");
    add_edge(ELF_ID_B, ELF_ID_C, "sf.bc.t6"); add_edge(ELF_ID_C, ELF_ID_B, "sf.cb.t6");
    add_edge(ELF_ID_B, ELF_ID_D, "sf.bd.t6"); add_edge(ELF_ID_D, ELF_ID_B, "sf.db.t6");
    add_edge(ELF_ID_C, ELF_ID_D, "sf.cd.t6"); add_edge(ELF_ID_D, ELF_ID_C, "sf.dc.t6");
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     4,         "4 nodes");
    assert_eq!(m,     6,         "6 undirected edges");
    assert_eq!(max_k, 3,         "max_k=3");
    assert_eq!(kappa, 3_000_000, "kappa=3.0 (regular: kappa=avg_k)");
    assert_eq!(avg_k, 3_000_000, "avg_k=3.0");
}

// ── 7. Star A→{B,C,D} ────────────────────────────────────────────────────────
//
// Directed: A→B, A→C, A→D.
// Undirected degrees: A=3 (hub), B=1, C=1, D=1 (spokes).
// sum_k=6, sum_k2=9+1+1+1=12, kappa=12/6=2, avg_k=6/4=1.5.
// kappa_ppm=2_000_000 > avg_degree_ppm=1_500_000: heterogeneous (kappa > avg_k).

#[test]
fn star_graph_heterogeneous() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sf.ab.t7");
    add_edge(ELF_ID_A, ELF_ID_C, "sf.ac.t7");
    add_edge(ELF_ID_A, ELF_ID_D, "sf.ad.t7");
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     4,         "4 nodes");
    assert_eq!(m,     3,         "3 undirected edges");
    assert_eq!(max_k, 3,         "hub degree=3");
    // sum_k2=9+1+1+1=12, sum_k=6, kappa=12/6=2 → 2_000_000
    assert_eq!(kappa, 2_000_000, "kappa=2.0");
    // avg_k=6/4=1.5 → 1_500_000
    assert_eq!(avg_k, 1_500_000, "avg_k=1.5");
    // Star: kappa > avg_k → heterogeneous.
    assert!(kappa > avg_k, "star is heterogeneous: kappa > avg_k");
}

// ── 8. Directed path A→B→C→D ─────────────────────────────────────────────────
//
// Undirected degrees: A=1, B=2, C=2, D=1.
// sum_k=6, sum_k2=1+4+4+1=10, kappa=10/6≈1.666..., avg_k=6/4=1.5.
// kappa_ppm = 10_000_000/6 = 1_666_666, avg_degree_ppm = 1_500_000.

#[test]
fn directed_path_abcd() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sf.ab.t8");
    add_edge(ELF_ID_B, ELF_ID_C, "sf.bc.t8");
    add_edge(ELF_ID_C, ELF_ID_D, "sf.cd.t8");
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     4,         "4 nodes");
    assert_eq!(m,     3,         "3 undirected edges");
    assert_eq!(max_k, 2,         "internal nodes have degree 2");
    // kappa = 10_000_000/6 = 1_666_666
    assert!(kappa >= 1_666_000 && kappa <= 1_667_000,
        "kappa_ppm={kappa} expected ≈1_666_666");
    assert_eq!(avg_k, 1_500_000, "avg_k=1.5");
}

// ── 9. K4 + isolated node ─────────────────────────────────────────────────────
//
// K4 bidirected (6 undirected edges) + isolated node E.
// Undirected degrees: A=3, B=3, C=3, D=3, E=0.
// sum_k=12, sum_k2=9+9+9+9+0=36, kappa=36/12=3, avg_k=12/5=2.4.
// kappa_ppm=3_000_000, avg_degree_ppm=2_400_000.
// kappa > avg_k → heterogeneous (isolated node dilutes avg but not kappa).

#[test]
fn k4_plus_isolated_heterogeneous() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_node(ELF_VEC_E, ELF_KEY_E, ELF_ID_E, 0xF005);   // isolated
    add_edge(ELF_ID_A, ELF_ID_B, "sf.ab.t9"); add_edge(ELF_ID_B, ELF_ID_A, "sf.ba.t9");
    add_edge(ELF_ID_A, ELF_ID_C, "sf.ac.t9"); add_edge(ELF_ID_C, ELF_ID_A, "sf.ca.t9");
    add_edge(ELF_ID_A, ELF_ID_D, "sf.ad.t9"); add_edge(ELF_ID_D, ELF_ID_A, "sf.da.t9");
    add_edge(ELF_ID_B, ELF_ID_C, "sf.bc.t9"); add_edge(ELF_ID_C, ELF_ID_B, "sf.cb.t9");
    add_edge(ELF_ID_B, ELF_ID_D, "sf.bd.t9"); add_edge(ELF_ID_D, ELF_ID_B, "sf.db.t9");
    add_edge(ELF_ID_C, ELF_ID_D, "sf.cd.t9"); add_edge(ELF_ID_D, ELF_ID_C, "sf.dc.t9");
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     5,         "5 nodes (K4 + isolated)");
    assert_eq!(m,     6,         "6 undirected edges");
    assert_eq!(max_k, 3,         "K4 nodes have degree 3");
    assert_eq!(kappa, 3_000_000, "kappa=3.0 (isolated node: sum_k2/sum_k=36/12)");
    assert_eq!(avg_k, 2_400_000, "avg_k=2.4 (12/5)");
    assert!(kappa > avg_k, "heterogeneous: kappa > avg_k");
}

// ── 10. Hub-and-spoke star with 6 spokes (heterogeneous / scale-free seed) ────
//
// A is hub connected to B,C,D,E,F,G (directed: A→B…A→G).
// Undirected degrees: A=6 (hub), B–G=1 each (spokes).
// n=7, m=6, sum_k=12, sum_k2=36+6×1=42.
// kappa = 42/12 = 3.5 → kappa_ppm = 3_500_000.
// avg_k = 12/7 ≈ 1.714 → avg_degree_ppm ≈ 1_714_285.
// kappa > 2×avg_k: 3_500_000 > 3_428_570 → "heterogeneous" threshold passes.
//
// A 6-spoke star is the minimum needed to cross the kappa > 2×avg_k boundary
// (n≥6 spokes needed: (n+1)²/(4n) > 2 ↔ n > 5.83).

#[test]
fn large_star_heterogeneous_signature() {
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
    add_edge(ELF_ID_A, ELF_ID_B, "sf.ab.t10");
    add_edge(ELF_ID_A, ELF_ID_C, "sf.ac.t10");
    add_edge(ELF_ID_A, ELF_ID_D, "sf.ad.t10");
    add_edge(ELF_ID_A, ELF_ID_E, "sf.ae.t10");
    add_edge(ELF_ID_A, ELF_ID_F, "sf.af.t10");
    add_edge(ELF_ID_A, ELF_ID_G, "sf.ag.t10");
    let (kappa, max_k, avg_k, n, m) = gos_runtime::graph_scale_free();
    assert_eq!(n,     7, "7 nodes (hub + 6 spokes)");
    assert_eq!(m,     6, "6 undirected edges");
    assert_eq!(max_k, 6, "hub degree=6");
    // sum_k2=36+6=42, sum_k=12, kappa=42/12=3.5 → 3_500_000
    assert_eq!(kappa, 3_500_000, "kappa=3.5");
    // avg_k=12/7 → avg_degree_ppm ≈ 1_714_285
    assert!(avg_k >= 1_714_000 && avg_k <= 1_715_000,
        "avg_k_ppm={avg_k} expected ≈1_714_285");
    // Heterogeneous signature: kappa > 2 × avg_k (needs ≥6 spokes).
    assert!(kappa as u64 > avg_k as u64 * 2,
        "kappa={kappa} should exceed 2*avg_k≈{} (heterogeneous signature)", avg_k * 2);
}
