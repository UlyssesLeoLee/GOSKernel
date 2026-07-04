// gos-graph-small-world-harness — V2.77 graph small-world coefficient API tests
//
// Verifies `gos_runtime::graph_small_world`.
//
// σ = (CC / CC_rand) / (L / L_rand)
// CC_rand ≈ 2m / (n·(n−1)),  L_rand ≈ ln(n)/ln(⟨k⟩),  ⟨k⟩ = 2m/n (integer)
// Returns (sigma_ppm, cc_ppm, l_ppm, l_rand_ppm, node_count, m_undir).
//
//  1. Empty graph                  → (0, 0, 0, 0, 0, 0)
//  2. Single node                  → sigma=0, node_count=1, m_undir=0
//  3. Two isolated nodes           → sigma=0 (no directed paths → L=0)
//  4. Two nodes, one directed edge → sigma=0 (avg_k=1 < 2)
//  5. Star A→{B,C,D}              → sigma=0 (avg_k=1 < 2)
//  6. Path A→B→C                  → sigma=0 (avg_k=1 < 2)
//  7. Directed triangle A→B,B→C,A→C → sigma≈1_584_963 (σ=ln(3)/ln(2))
//  8. Bidirected triangle (6 edges)  → sigma same as test 7 (same n,m,CC,L)
//  9. Bidirected K4 (12 edges)       → sigma≈1_261_859 (σ=ln(4)/ln(3))
// 10. Bidirected K4 + isolated node  → sigma≈3_095_904 (diluted CC, larger L_rand)

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

const ELF_ID_A: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_A);
const ELF_ID_B: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_B);
const ELF_ID_C: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_C);
const ELF_ID_D: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_D);
const ELF_ID_E: NodeId = derive_node_id(ELF_PLUGIN, ELF_KEY_E);

// L4=53 identifies this harness namespace in the VectorAddress space.
const ELF_VEC_A: VectorAddress = VectorAddress::new(53, 1, 1, 0);
const ELF_VEC_B: VectorAddress = VectorAddress::new(53, 1, 2, 0);
const ELF_VEC_C: VectorAddress = VectorAddress::new(53, 1, 3, 0);
const ELF_VEC_D: VectorAddress = VectorAddress::new(53, 1, 4, 0);
const ELF_VEC_E: VectorAddress = VectorAddress::new(53, 1, 5, 0);

const ELF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ELF_PLUGIN,
    name:         "kl-graph-small-world-harness",
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
fn empty_graph_zero_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (sigma, cc, l, l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 0,       "empty: node_count=0");
    assert_eq!(m, 0,       "empty: m_undir=0");
    assert_eq!(sigma, 0,   "empty: sigma=0");
    assert_eq!(cc, 0,      "empty: cc=0");
    assert_eq!(l, 0,       "empty: l=0");
    assert_eq!(l_rand, 0,  "empty: l_rand=0");
}

// ── 2. Single node ────────────────────────────────────────────────────────────

#[test]
fn single_node_zero_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    let (sigma, _cc, _l, _l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 1,     "1 node");
    assert_eq!(m, 0,     "no edges");
    assert_eq!(sigma, 0, "n<2: sigma=0");
}

// ── 3. Two isolated nodes ─────────────────────────────────────────────────────

#[test]
fn two_isolated_nodes_zero_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    // No edges → no directed paths → L=0 → sigma undefined.
    let (sigma, _cc, l, _l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 2,     "2 nodes");
    assert_eq!(m, 0,     "no edges");
    assert_eq!(l, 0,     "no paths: L=0");
    assert_eq!(sigma, 0, "L=0: sigma=0");
}

// ── 4. Two nodes, one directed edge ──────────────────────────────────────────

#[test]
fn two_nodes_one_edge_zero_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_edge(ELF_ID_A, ELF_ID_B, "sw.ab.t4");
    // avg_k = 2*1/2 = 1 < 2 → sigma=0.
    let (sigma, _cc, _l, l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 2,        "2 nodes");
    assert_eq!(m, 1,        "1 undirected edge");
    assert_eq!(l_rand, 0,   "avg_k=1: L_rand undefined");
    assert_eq!(sigma, 0,    "avg_k<2: sigma=0");
}

// ── 5. Star A→{B,C,D}: avg_k=1 ───────────────────────────────────────────────

#[test]
fn star_graph_zero_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sw.ab.t5");
    add_edge(ELF_ID_A, ELF_ID_C, "sw.ac.t5");
    add_edge(ELF_ID_A, ELF_ID_D, "sw.ad.t5");
    // avg_k = 2*3/4 = 1 < 2 → sigma=0.
    let (sigma, _cc, _l, l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 4,        "4 nodes");
    assert_eq!(m, 3,        "3 undirected edges");
    assert_eq!(l_rand, 0,   "avg_k=1: L_rand undefined");
    assert_eq!(sigma, 0,    "avg_k<2: sigma=0");
}

// ── 6. Path A→B→C: avg_k=1 ───────────────────────────────────────────────────

#[test]
fn path_three_nodes_zero_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "sw.ab.t6");
    add_edge(ELF_ID_B, ELF_ID_C, "sw.bc.t6");
    // avg_k = 2*2/3 = 1 < 2 → sigma=0.
    let (sigma, _cc, _l, l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 3,        "3 nodes");
    assert_eq!(m, 2,        "2 undirected edges");
    assert_eq!(l_rand, 0,   "avg_k=1: L_rand undefined");
    assert_eq!(sigma, 0,    "avg_k<2: sigma=0");
}

// ── 7. Directed triangle A→B, B→C, A→C ──────────────────────────────────────
//
// n=3, m_undir=3, avg_k=2, CC=1.0, L=1.0.
// CC_rand = 2*3/(3*2) = 1.0.
// L_rand = ln(3)/ln(2) ≈ 1.584963.
// σ = (1/1) / (1/1.584963) = 1.584963 → sigma_ppm ≈ 1_584_963.

#[test]
fn directed_triangle_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "sw.ab.t7");
    add_edge(ELF_ID_B, ELF_ID_C, "sw.bc.t7");
    add_edge(ELF_ID_A, ELF_ID_C, "sw.ac.t7");
    let (sigma, cc, l, _l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 3,            "3 nodes");
    assert_eq!(m, 3,            "3 undirected edges");
    assert_eq!(cc, 1_000_000,   "CC=1.0 (all pairs form triangles)");
    assert_eq!(l, 1_000_000,    "L=1.0 (all 3 reachable pairs at distance 1)");
    // sigma_ppm = ln(3)/ln(2) * 1_000_000 ≈ 1_584_963 (table-based)
    assert!(sigma >= 1_583_000 && sigma <= 1_587_000,
        "sigma_ppm={sigma} expected ≈1_584_963");
}

// ── 8. Bidirected triangle (6 edges): same σ as directed triangle ─────────────
//
// Invariant: adding reverse edges does not change the small-world coefficient
// when the undirected edge count and average path length are preserved.
// n=3, m_undir=3 (same 3 undirected pairs), CC=1.0, L=1.0 (all 6 pairs at d=1).

#[test]
fn bidirected_triangle_same_sigma_as_directed() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_edge(ELF_ID_A, ELF_ID_B, "sw.ab.t8");
    add_edge(ELF_ID_B, ELF_ID_A, "sw.ba.t8");
    add_edge(ELF_ID_B, ELF_ID_C, "sw.bc.t8");
    add_edge(ELF_ID_C, ELF_ID_B, "sw.cb.t8");
    add_edge(ELF_ID_A, ELF_ID_C, "sw.ac.t8");
    add_edge(ELF_ID_C, ELF_ID_A, "sw.ca.t8");
    let (sigma, cc, l, _l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 3,            "3 nodes");
    assert_eq!(m, 3,            "3 undirected edges (bidirected → same m_undir)");
    assert_eq!(cc, 1_000_000,   "CC=1.0");
    assert_eq!(l, 1_000_000,    "L=1.0 (6/6 pairs at d=1)");
    // Same σ as directed triangle (same n, m_undir, CC, L).
    assert!(sigma >= 1_583_000 && sigma <= 1_587_000,
        "sigma_ppm={sigma} expected ≈1_584_963 (same as directed triangle)");
}

// ── 9. Bidirected K4 (12 directed edges, 6 undirected) ───────────────────────
//
// n=4, m_undir=6, avg_k=3, CC=1.0, L=1.0.
// CC_rand = 2*6/(4*3) = 1.0.
// L_rand = ln(4)/ln(3) ≈ 1.261860.
// σ ≈ 1.261860 → sigma_ppm ≈ 1_261_860.

#[test]
fn bidirected_k4_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_edge(ELF_ID_A, ELF_ID_B, "sw.ab.t9"); add_edge(ELF_ID_B, ELF_ID_A, "sw.ba.t9");
    add_edge(ELF_ID_A, ELF_ID_C, "sw.ac.t9"); add_edge(ELF_ID_C, ELF_ID_A, "sw.ca.t9");
    add_edge(ELF_ID_A, ELF_ID_D, "sw.ad.t9"); add_edge(ELF_ID_D, ELF_ID_A, "sw.da.t9");
    add_edge(ELF_ID_B, ELF_ID_C, "sw.bc.t9"); add_edge(ELF_ID_C, ELF_ID_B, "sw.cb.t9");
    add_edge(ELF_ID_B, ELF_ID_D, "sw.bd.t9"); add_edge(ELF_ID_D, ELF_ID_B, "sw.db.t9");
    add_edge(ELF_ID_C, ELF_ID_D, "sw.cd.t9"); add_edge(ELF_ID_D, ELF_ID_C, "sw.dc.t9");
    let (sigma, cc, l, _l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 4,            "4 nodes");
    assert_eq!(m, 6,            "6 undirected edges");
    assert_eq!(cc, 1_000_000,   "CC=1.0 (complete clique)");
    assert_eq!(l, 1_000_000,    "L=1.0 (all 12 pairs at d=1)");
    // sigma_ppm = ln(4)/ln(3) × 1_000_000 ≈ 1_261_860.
    assert!(sigma >= 1_259_000 && sigma <= 1_264_000,
        "sigma_ppm={sigma} expected ≈1_261_860");
}

// ── 10. Bidirected K4 + isolated node (n=5) ──────────────────────────────────
//
// avg_k = 2*6/5 = 2,  CC diluted to 800_000 (isolated E contributes 0).
// L=1.0 (only K4 pairs reachable).  CC_rand = 2*6/(5*4) = 600_000.
// L_rand = ln(5)/ln(2) ≈ 2.321928.
// σ = (CC/CC_rand) / (L/L_rand) = (0.8/0.6) / (1/2.321928) ≈ 3.095904.
// sigma_ppm ≈ 3_095_904.

#[test]
fn bidirected_k4_plus_isolated_sigma() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ELF_VEC_A, ELF_KEY_A, ELF_ID_A, 0xF001);
    add_node(ELF_VEC_B, ELF_KEY_B, ELF_ID_B, 0xF002);
    add_node(ELF_VEC_C, ELF_KEY_C, ELF_ID_C, 0xF003);
    add_node(ELF_VEC_D, ELF_KEY_D, ELF_ID_D, 0xF004);
    add_node(ELF_VEC_E, ELF_KEY_E, ELF_ID_E, 0xF005);  // isolated
    add_edge(ELF_ID_A, ELF_ID_B, "sw.ab.t10"); add_edge(ELF_ID_B, ELF_ID_A, "sw.ba.t10");
    add_edge(ELF_ID_A, ELF_ID_C, "sw.ac.t10"); add_edge(ELF_ID_C, ELF_ID_A, "sw.ca.t10");
    add_edge(ELF_ID_A, ELF_ID_D, "sw.ad.t10"); add_edge(ELF_ID_D, ELF_ID_A, "sw.da.t10");
    add_edge(ELF_ID_B, ELF_ID_C, "sw.bc.t10"); add_edge(ELF_ID_C, ELF_ID_B, "sw.cb.t10");
    add_edge(ELF_ID_B, ELF_ID_D, "sw.bd.t10"); add_edge(ELF_ID_D, ELF_ID_B, "sw.db.t10");
    add_edge(ELF_ID_C, ELF_ID_D, "sw.cd.t10"); add_edge(ELF_ID_D, ELF_ID_C, "sw.dc.t10");
    let (sigma, cc, l, _l_rand, n, m) = gos_runtime::graph_small_world();
    assert_eq!(n, 5,            "5 nodes (K4 + isolated E)");
    assert_eq!(m, 6,            "6 undirected edges");
    assert_eq!(cc, 800_000,     "CC=0.8 (K4 nodes each=1.0, E=0, avg over 5)");
    assert_eq!(l, 1_000_000,    "L=1.0 (only K4 pairs count; W=12, pairs=12)");
    // sigma ≈ 3_095_904 (table-based).
    assert!(sigma >= 3_080_000 && sigma <= 3_110_000,
        "sigma_ppm={sigma} expected ≈3_095_904");
}
