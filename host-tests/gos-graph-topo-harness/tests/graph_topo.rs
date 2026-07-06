// gos-graph-topo-harness — V3.12 SC + GA + AZI degree-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices()`:
//   Returns (sc_ppm, ga_ppm, azi_milli, edge_count, node_count)
//   - sc_ppm    = SC × 10^6 where SC  = Σ_{uv∈E} 1/√(deg(u)+deg(v))         (Zhou 2009)
//   - ga_ppm    = GA × 10^6 where GA  = Σ_{uv∈E} 2√(deg(u)·deg(v))/(deg(u)+deg(v)) (Vukičević 2009)
//   - azi_milli = AZI×1000  where AZI = Σ_{uv∈E,q>0} (deg(u)·deg(v)/(deg(u)+deg(v)−2))³
//                                                                              (Furtula 2010)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// Integer precision:
//   SC:  contribution = floor(10^12 / isqrt_ppm(s)) where s = da+db, isqrt_ppm = floor(√s × 10^6)
//   GA:  contribution = 2 × isqrt_ppm(p) / s        where p = da×db  (error ≤ 1 ppm per edge)
//   AZI: contribution = p³ × 1000 / q³              where q = s-2    (exact integer for q > 0)
//
// GA = |E| × 10^6 iff graph is regular (all degrees equal); pendant-pendant (q=0) skipped in AZI.
//
// Key analytical values:
//
//  Graph       SC_ppm    GA_ppm    AZI_milli   edges   notes
//  Empty             0         0          0      0
//  1 node            0         0          0      0    no edges
//  Edge A-B    707_107 1_000_000          0      1    1/√2; GA=1 exact; AZI: q=0 skip
//  Path P₃   1_154_700 1_885_616     16_000      2    SC=2/√3; GA=2×2√2/(3); AZI=2×8/1
//  Triangle K₃ 1_500_000 3_000_000   24_000      3    SC=3/2=1.5 exact; GA=|E|=3; AZI=3×64/8
//  Star K_{1,4}1_788_852 3_200_000    9_480      4    SC=4/√5; GA=4×4/5; AZI=4×64/27
//  Path P₄   1_654_700 2_885_616     24_000      3    mixed degrees
//  Complete K₄ 2_449_488 6_000_000   68_340      6    SC=6/√6; GA=|E|=6 (regular); AZI=6×729/64
//  2 isolated        0         0          0      0    no edges
//  K_{2,3}   2_683_278 5_878_770     48_000      6    bipartite, non-regular cross-check
//
// Tests (10):
//  1.  Empty graph                       → (0, 0, 0, 0, 0)
//  2.  Single isolated node              → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B          → (707_107, 1_000_000, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_154_700, 1_885_616, 16_000, 2, 3)
//  5.  Triangle K₃                       → (1_500_000, 3_000_000, 24_000, 3, 3)
//  6.  Star K_{1,4}                      → (1_788_852, 3_200_000, 9_480, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (1_654_700, 2_885_616, 24_000, 3, 4)
//  8.  Complete K₄                       → (2_449_488, 6_000_000, 68_340, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (2_683_278, 5_878_770, 48_000, 6, 5)
//      Cross-checks: GA < |E|×10^6 (non-regular); AZI/edge=8_000 same as K₃ (both q=da+db-2=2)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const TI_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IDX");
const TI_EXEC:   ExecutorId = ExecutorId::from_ascii("ti.exec");

const TI_KEY_A: &str = "ti.alpha";
const TI_KEY_B: &str = "ti.beta";
const TI_KEY_C: &str = "ti.gamma";
const TI_KEY_D: &str = "ti.delta";
const TI_KEY_E: &str = "ti.epsilon";

const TI_ID_A: NodeId = derive_node_id(TI_PLUGIN, TI_KEY_A);
const TI_ID_B: NodeId = derive_node_id(TI_PLUGIN, TI_KEY_B);
const TI_ID_C: NodeId = derive_node_id(TI_PLUGIN, TI_KEY_C);
const TI_ID_D: NodeId = derive_node_id(TI_PLUGIN, TI_KEY_D);
const TI_ID_E: NodeId = derive_node_id(TI_PLUGIN, TI_KEY_E);

// L4=88 namespace for this harness.
const TI_VEC_A: VectorAddress = VectorAddress::new(88, 1, 1, 0);
const TI_VEC_B: VectorAddress = VectorAddress::new(88, 1, 2, 0);
const TI_VEC_C: VectorAddress = VectorAddress::new(88, 1, 3, 0);
const TI_VEC_D: VectorAddress = VectorAddress::new(88, 2, 1, 0);
const TI_VEC_E: VectorAddress = VectorAddress::new(88, 2, 2, 0);

const TI_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    TI_PLUGIN,
    name:         "kl-graph-topo-harness",
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
        executor_id:       TI_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(TI_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    let g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    gos_runtime::reset();
    gos_runtime::discover_plugin(TI_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices and counts are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(sc,  0, "empty: SC=0");
    assert_eq!(ga,  0, "empty: GA=0");
    assert_eq!(azi, 0, "empty: AZI=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with degree 0. No edges, so all indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(sc,  0, "single: SC=0");
    assert_eq!(ga,  0, "single: GA=0");
    assert_eq!(azi, 0, "single: AZI=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B. Both nodes have degree 1. s=2, p=1, q=0.
// SC: isqrt_ppm(2)=1_414_213 → floor(10^12/1_414_213) = 707_107.
// GA: 2 × isqrt_ppm(1) / 2 = 2 × 1_000_000 / 2 = 1_000_000 (GA=1 exact, 2√1/(1+1)=1).
// AZI: q = 1+1-2 = 0 → skip pendant-pendant edge. AZI=0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);
    add_edge(TI_ID_A, TI_ID_B, "ab");

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(sc,  707_107,   "edge: SC_ppm=707_107 (=1/√2×10^6, floor)");
    assert_eq!(ga,  1_000_000, "edge: GA_ppm=1_000_000 (GA=1 exact for A-B)");
    assert_eq!(azi, 0,         "edge: AZI=0 (pendant-pendant, q=0 skip)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1. Edges: A-B and B-C.
//
// A-B: da=1, db=2; s=3, p=2, q=1.
//   SC: isqrt_ppm(3)=1_732_050 → floor(10^12/1_732_050) = 577_350
//   GA: 2 × isqrt_ppm(2) / 3 = 2×1_414_213/3 = 2_828_426/3 = 942_808 (floor)
//   AZI: p=2,q=1 → 2³×1000/1 = 8_000
// B-C: same as A-B.
// Totals: SC=1_154_700, GA=1_885_616, AZI=16_000.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);
    add_node(TI_VEC_C, TI_KEY_C, TI_ID_C);
    add_edge(TI_ID_A, TI_ID_B, "ab");
    add_edge(TI_ID_B, TI_ID_C, "bc");

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  3,         "P₃: node_count=3");
    assert_eq!(ec,  2,         "P₃: edge_count=2");
    assert_eq!(sc,  1_154_700, "P₃: SC_ppm=1_154_700 (2×577_350)");
    assert_eq!(ga,  1_885_616, "P₃: GA_ppm=1_885_616 (2×942_808)");
    assert_eq!(azi, 16_000,    "P₃: AZI_milli=16_000 (2×8_000)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// Degrees: all 2. 3 undirected edges. s=4, p=4, q=2.
// SC per edge: isqrt_ppm(4)=2_000_000 → 10^12/2_000_000 = 500_000.
// GA per edge: 2×isqrt_ppm(4)/4 = 2×2_000_000/4 = 1_000_000 (GA=1 per edge, exact).
//   GA total = 3_000_000 = |E|×10^6 ✓ (regular graph invariant)
// AZI per edge: p=4,q=2 → 4³×1000/2³ = 64_000/8 = 8_000. Total: 24_000.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);
    add_node(TI_VEC_C, TI_KEY_C, TI_ID_C);
    add_edge(TI_ID_A, TI_ID_B, "ab");
    add_edge(TI_ID_B, TI_ID_C, "bc");
    add_edge(TI_ID_C, TI_ID_A, "ca");

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  3,         "K₃: node_count=3");
    assert_eq!(ec,  3,         "K₃: edge_count=3");
    assert_eq!(sc,  1_500_000, "K₃: SC_ppm=1_500_000 (3×500_000=3/2 exact)");
    assert_eq!(ga,  3_000_000, "K₃: GA_ppm=3_000_000 (=|E|×10^6; regular graph)");
    assert_eq!(azi, 24_000,    "K₃: AZI_milli=24_000 (3×8_000)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center deg=4, 4 leaves deg=1. 4 edges. For each edge: da=4,db=1; s=5,p=4,q=3.
// SC per edge: isqrt_ppm(5)=2_236_067 → floor(10^12/2_236_067) = 447_213.
//   Total: 4×447_213 = 1_788_852.
// GA per edge: 2×isqrt_ppm(4)/5 = 2×2_000_000/5 = 800_000 (GA=4/5 per edge).
//   Total: 4×800_000 = 3_200_000.
// AZI per edge: p=4,q=3 → 4³×1000/3³ = 64_000/27 = 2_370 (floor). Total: 4×2_370 = 9_480.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A); // center
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);
    add_node(TI_VEC_C, TI_KEY_C, TI_ID_C);
    add_node(TI_VEC_D, TI_KEY_D, TI_ID_D);
    add_node(TI_VEC_E, TI_KEY_E, TI_ID_E);
    add_edge(TI_ID_A, TI_ID_B, "ab");
    add_edge(TI_ID_A, TI_ID_C, "ac");
    add_edge(TI_ID_A, TI_ID_D, "ad");
    add_edge(TI_ID_A, TI_ID_E, "ae");

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  5,         "K_{{1,4}}: node_count=5");
    assert_eq!(ec,  4,         "K_{{1,4}}: edge_count=4");
    assert_eq!(sc,  1_788_852, "K_{{1,4}}: SC_ppm=1_788_852 (4×447_213)");
    assert_eq!(ga,  3_200_000, "K_{{1,4}}: GA_ppm=3_200_000 (4×800_000; GA=3.2)");
    assert_eq!(azi, 9_480,     "K_{{1,4}}: AZI_milli=9_480 (4×2_370; floor 64/27×1000)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1. 3 edges: A-B, B-C, C-D.
//
// A-B: da=1,db=2; s=3,p=2,q=1. SC=577_350. GA=942_808. AZI=8_000.
// B-C: da=2,db=2; s=4,p=4,q=2. SC=500_000. GA=1_000_000. AZI=8_000.
// C-D: da=2,db=1; s=3,p=2,q=1. SC=577_350. GA=942_808. AZI=8_000.
// Totals: SC=1_654_700, GA=2_885_616, AZI=24_000.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);
    add_node(TI_VEC_C, TI_KEY_C, TI_ID_C);
    add_node(TI_VEC_D, TI_KEY_D, TI_ID_D);
    add_edge(TI_ID_A, TI_ID_B, "ab");
    add_edge(TI_ID_B, TI_ID_C, "bc");
    add_edge(TI_ID_C, TI_ID_D, "cd");

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  4,         "P₄: node_count=4");
    assert_eq!(ec,  3,         "P₄: edge_count=3");
    assert_eq!(sc,  1_654_700, "P₄: SC_ppm=1_654_700");
    assert_eq!(ga,  2_885_616, "P₄: GA_ppm=2_885_616");
    assert_eq!(azi, 24_000,    "P₄: AZI_milli=24_000 (3×8_000)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All degrees = 3. 6 undirected edges. s=6, p=9, q=4.
// SC per edge: isqrt_ppm(6)=2_449_489 → floor(10^12/2_449_489) = 408_248.
//   Total: 6×408_248 = 2_449_488.
// GA per edge: 2×isqrt_ppm(9)/6 = 2×3_000_000/6 = 1_000_000 (GA=1 per edge, exact).
//   Total: 6_000_000 = |E|×10^6 ✓ (regular graph invariant)
// AZI per edge: p=9,q=4 → 9³×1000/4³ = 729_000/64 = 11_390 (floor). Total: 6×11_390 = 68_340.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);
    add_node(TI_VEC_C, TI_KEY_C, TI_ID_C);
    add_node(TI_VEC_D, TI_KEY_D, TI_ID_D);
    add_edge(TI_ID_A, TI_ID_B, "ab");
    add_edge(TI_ID_A, TI_ID_C, "ac");
    add_edge(TI_ID_A, TI_ID_D, "ad");
    add_edge(TI_ID_B, TI_ID_C, "bc");
    add_edge(TI_ID_B, TI_ID_D, "bd");
    add_edge(TI_ID_C, TI_ID_D, "cd");

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  4,         "K₄: node_count=4");
    assert_eq!(ec,  6,         "K₄: edge_count=6");
    assert_eq!(sc,  2_449_488, "K₄: SC_ppm=2_449_488 (6×408_248=6/√6×10^6)");
    assert_eq!(ga,  6_000_000, "K₄: GA_ppm=6_000_000 (=|E|×10^6; regular graph)");
    assert_eq!(azi, 68_340,    "K₄: AZI_milli=68_340 (6×11_390; floor 729/64×1000)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Two nodes, no edges. All indices are 0, node_count=2.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  2, "2 isolated: node_count=2");
    assert_eq!(ec,  0, "2 isolated: no edges");
    assert_eq!(sc,  0, "2 isolated: SC=0");
    assert_eq!(ga,  0, "2 isolated: GA=0");
    assert_eq!(azi, 0, "2 isolated: AZI=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Side-A: A(deg=3), B(deg=3). Side-B: C(deg=2), D(deg=2), E(deg=2). 6 edges.
// All edges: da=3, db=2; s=5, p=6, q=3.
//
// SC per edge: isqrt_ppm(5)=2_236_067 → floor(10^12/2_236_067) = 447_213.
//   Total: 6×447_213 = 2_683_278.
// GA per edge: 2×isqrt_ppm(6)/5 = 2×2_449_489/5 = 4_898_978/5 = 979_795 (floor).
//   Total: 6×979_795 = 5_878_770.
//   Note: GA < |E|×10^6 = 6_000_000 since K_{2,3} is not regular. ✓
// AZI per edge: p=6,q=3 → 6³×1000/3³ = 216_000/27 = 8_000 (exact).
//   Total: 6×8_000 = 48_000.
//   Cross-check: AZI/edge=8_000 same as K₃ (K₃: p=4,q=2 → 64/8=8) ✓
//
// GA cross-check: 12√6/5 ≈ 5.87877; GA_ppm ≈ 5_878_770. ✓ (within 6 ppm of exact)

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    // Side-A: TI_VEC_A (deg=3), TI_VEC_B (deg=3)
    // Side-B: TI_VEC_C (deg=2), TI_VEC_D (deg=2), TI_VEC_E (deg=2)
    add_node(TI_VEC_A, TI_KEY_A, TI_ID_A);
    add_node(TI_VEC_B, TI_KEY_B, TI_ID_B);
    add_node(TI_VEC_C, TI_KEY_C, TI_ID_C);
    add_node(TI_VEC_D, TI_KEY_D, TI_ID_D);
    add_node(TI_VEC_E, TI_KEY_E, TI_ID_E);
    add_edge(TI_ID_A, TI_ID_C, "ac");
    add_edge(TI_ID_A, TI_ID_D, "ad");
    add_edge(TI_ID_A, TI_ID_E, "ae");
    add_edge(TI_ID_B, TI_ID_C, "bc");
    add_edge(TI_ID_B, TI_ID_D, "bd");
    add_edge(TI_ID_B, TI_ID_E, "be");

    let (sc, ga, azi, ec, nc) = gos_runtime::graph_topo_indices();
    assert_eq!(nc,  5,         "K_{{2,3}}: node_count=5");
    assert_eq!(ec,  6,         "K_{{2,3}}: edge_count=6");
    assert_eq!(sc,  2_683_278, "K_{{2,3}}: SC_ppm=2_683_278 (6×447_213)");
    assert_eq!(ga,  5_878_770, "K_{{2,3}}: GA_ppm=5_878_770 (6×979_795; non-regular: GA<|E|)");
    assert_eq!(azi, 48_000,    "K_{{2,3}}: AZI_milli=48_000 (6×8_000; AZI/edge=8_000=K₃)");

    // Invariant: regular graph ↔ GA_ppm == ec × 1_000_000
    assert_ne!(ga, ec as u64 * 1_000_000,
        "K_{{2,3}} is not regular so GA_ppm must differ from |E|×10^6");
}
