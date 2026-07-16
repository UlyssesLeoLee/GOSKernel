// gos-graph-topo25-harness — V3.36 NHM₂ + NAG + NABS (Neighborhood S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices25()`:
//   Returns (nhm2, nag_ppm, nabs_ppm, edge_count, node_count)
//   - nhm2     = NHM₂(G) = Σ_{uv∈E} (S_u·S_v)²                         (exact u64; S-HM₂)
//   - nag_ppm  = NAG(G) × 10^6 = Σ_{uv∈E} floor((S_u+S_v)·10^6/(2√(S_u·S_v)))  (floor ppm; S-AG)
//   - nabs_ppm = NABS(G) × 10^6 = Σ_{uv∈E} floor(√((S_u+S_v-2)/(S_u+S_v))·10^6) (floor ppm; S-ABS)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHM₂(G) = Σ_{uv∈E} (S_u·S_v)²           (S-analogue of HM₂; Das & Trinajstić 2011)
//   NAG(G)  = Σ_{uv∈E} (S_u+S_v)/(2√(S_u·S_v))  (S-analogue of AG ratio; Zheng et al. 2020)
//   NABS(G) = Σ_{uv∈E} √((S_u+S_v-2)/(S_u+S_v))  (S-analogue of ABS; Chen et al. 2022)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NHM₂ per edge  = (S_u·S_v)²                                  [exact u64; u128 accumulator]
//   NAG  per edge  = floor(ssum·10^12 / (2·isqrt128(sp·10^12)))  [floor ppm; ssum≤32258 < u64::MAX]
//   NABS per edge  = isqrt64((ssum-2)·10^12 / ssum)              [floor ppm; (ssum-2)·10^12 < u64::MAX]
//
// KEY INVARIANTS:
//   NAG ≥ |E|×10^6 always (AM≥GM: (S_u+S_v)/2 ≥ √(S_u·S_v)); equal iff S-uniform graph.
//   NABS = 0 only when S_u+S_v=2 for every edge (only K₂: both endpoints have S=1).
//   K₃ and K_{1,4} coincide on NAG and NABS per edge (S-uniform S=4 coincidence, ssum=8).
//   K₄ and K_{2,3} both give NAG=|E|×10^6=6_000_000 (both S-uniform, |E|=6), but NHM₂
//   and NABS differ (S=9 vs S=6).
//
// S VALUES PER GRAPH (same S-variant as topo18/topo21–topo25):
//   K₂        : S(A)=S(B)=1              → ssum=2, sp=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2        → S-uniform S=2
//   K₃        : S(each)=4               → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4     → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S values
//   K₄        : S(each)=9               → S-uniform S=9
//   K_{2,3}   : S(all)=6                → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph       NHM₂     NAG(ppm)    NABS(ppm)  edges  nodes
//  Empty          0            0            0      0      0
//  1 node         0            0            0      0      1
//  Edge K₂        1    1_000_000            0      1      2
//  Path P₃       32    2_000_000    1_414_212      2      3
//  Triangle K₃  768    3_000_000    2_598_075      3      3
//  Star K_{1,4} 1024    4_000_000    3_464_100      4      5
//  Path P₄      153    3_041_242    2_365_688      3      4
//  Complete K₄ 39366    6_000_000    5_656_854      6      4
//  2 isolated     0            0            0      0      2
//  K_{2,3}     7776    6_000_000    5_477_220      6      5
//
// Derivations:
//
//   K₂ (S_A=S_B=1, ssum=2, sp=1):
//     NHM₂: (1×1)²=1. ✓
//     NAG:  isqrt128(1×10^12)=10^6; floor(2×10^12/(2×10^6))=10^6=1_000_000. ✓
//     NABS: isqrt64((2-2)×10^12/2)=isqrt64(0)=0. ✓
//
//   P₃ (S-uniform S=2, 2 edges; ssum=4, sp=4):
//     NHM₂ per edge: (2×2)²=16. Total: 32. ✓
//     NAG  per edge: isqrt128(4×10^12)=2×10^6=2_000_000; floor(4×10^12/(2×2_000_000))=1_000_000. Total: 2_000_000. ✓
//       (S-uniform: NAG=|E|×10^6 exactly)
//     NABS per edge: isqrt64((4-2)×10^12/4)=isqrt64(500_000_000_000)=707_106. Total: 1_414_212. ✓
//
//   K₃ (S-uniform S=4, 3 edges; ssum=8, sp=16):
//     NHM₂ per edge: (4×4)²=256. Total: 768. ✓
//     NAG  per edge: isqrt128(16×10^12)=4×10^6; floor(8×10^12/(2×4_000_000))=1_000_000. Total: 3_000_000. ✓
//     NABS per edge: isqrt64(6×10^12/8)=isqrt64(750_000_000_000)=866_025. Total: 2_598_075. ✓
//       (√(6/8)×10^6=√(3/4)×10^6; 866_025²=749_999_300_625 ≤ 750_000_000_000 < 866_026²=750_001_032_676)
//
//   K_{1,4} (S-uniform S=4, 4 edges; same per-edge values as K₃):
//     NHM₂: 4×256=1024. ✓   NAG: 4×1_000_000=4_000_000. ✓   NABS: 4×866_025=3_464_100. ✓
//     (S-uniform coincidence: K₃ and K_{1,4} share S=4, same per-edge values)
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2):
//     Edge A-B (sa=2,sb=3, ssum=5, sp=6):
//       NHM₂ = (2×3)²=36.
//       NAG:  isqrt128(6×10^12)=2_449_489; floor(5×10^12/(2×2_449_489))=floor(5×10^12/4_898_978)=1_020_621.
//             (5×10^12/4_898_978: 1_020_621×4_898_978=4_999_999_825_038 ≤ 5×10^12 < 1_020_622×4_898_978)
//       NABS: isqrt64(3×10^12/5)=isqrt64(600_000_000_000)=774_596.
//             (774_596²=599_999_011_216 ≤ 600_000_000_000 < 774_597²=600_000_560_409)
//     Edge B-C (sa=3,sb=3, ssum=6, sp=9):
//       NHM₂ = (3×3)²=81.
//       NAG:  isqrt128(9×10^12)=3_000_000; floor(6×10^12/(2×3_000_000))=1_000_000. (exact, S-uniform edge)
//       NABS: isqrt64(4×10^12/6)=isqrt64(666_666_666_666)=816_496.
//             (816_496²=666_665_718_016 ≤ 666_666_666_666 < 816_497²=666_667_351_009)
//     Edge C-D: same as A-B by symmetry.
//     NHM₂ total = 36+81+36=153. ✓
//     NAG  total = 1_020_621+1_000_000+1_020_621=3_041_242. ✓
//     NABS total = 774_596+816_496+774_596=2_365_688. ✓
//
//   K₄ (S-uniform S=9, 6 edges; ssum=18, sp=81):
//     NHM₂ per edge: (9×9)²=6561. Total: 39366. ✓
//     NAG  per edge: isqrt128(81×10^12)=9_000_000 (exact); floor(18×10^12/18_000_000)=1_000_000. Total: 6_000_000. ✓
//     NABS per edge: isqrt64(16×10^12/18)=isqrt64(888_888_888_888)=942_809. Total: 5_656_854. ✓
//       (942_809²=888_888_810_481 ≤ 888_888_888_888 < 942_810²=888_890_696_100)
//
//   K_{2,3} (S-uniform S=6, 6 edges; ssum=12, sp=36):
//     NHM₂ per edge: (6×6)²=1296. Total: 7776. ✓
//     NAG  per edge: isqrt128(36×10^12)=6_000_000 (exact); floor(12×10^12/12_000_000)=1_000_000. Total: 6_000_000. ✓
//       (Note: K₄ and K_{2,3} both NAG=6_000_000 since |E|=6 and both S-uniform)
//     NABS per edge: isqrt64(10×10^12/12)=isqrt64(833_333_333_333)=912_870. Total: 5_477_220. ✓
//       (912_870²=833_331_636_900 ≤ 833_333_333_333 < 912_871²=833_333_462_641)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (1, 1_000_000, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (32, 2_000_000, 1_414_212, 2, 3)
//  5.  Triangle K₃                       → (768, 3_000_000, 2_598_075, 3, 3)
//  6.  Star K_{1,4}                      → (1024, 4_000_000, 3_464_100, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (153, 3_041_242, 2_365_688, 3, 4)
//  8.  Complete K₄                       → (39366, 6_000_000, 5_656_854, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (7776, 6_000_000, 5_477_220, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T25_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_25");
const T25_EXEC:   ExecutorId = ExecutorId::from_ascii("t25.exec");

const T25_KEY_A: &str = "t25.alpha";
const T25_KEY_B: &str = "t25.beta";
const T25_KEY_C: &str = "t25.gamma";
const T25_KEY_D: &str = "t25.delta";
const T25_KEY_E: &str = "t25.epsilon";

const T25_ID_A: NodeId = derive_node_id(T25_PLUGIN, T25_KEY_A);
const T25_ID_B: NodeId = derive_node_id(T25_PLUGIN, T25_KEY_B);
const T25_ID_C: NodeId = derive_node_id(T25_PLUGIN, T25_KEY_C);
const T25_ID_D: NodeId = derive_node_id(T25_PLUGIN, T25_KEY_D);
const T25_ID_E: NodeId = derive_node_id(T25_PLUGIN, T25_KEY_E);

// L4=112 namespace for this harness.
const T25_VEC_A: VectorAddress = VectorAddress::new(112, 1, 1, 0);
const T25_VEC_B: VectorAddress = VectorAddress::new(112, 1, 2, 0);
const T25_VEC_C: VectorAddress = VectorAddress::new(112, 1, 3, 0);
const T25_VEC_D: VectorAddress = VectorAddress::new(112, 2, 1, 0);
const T25_VEC_E: VectorAddress = VectorAddress::new(112, 2, 2, 0);

const T25_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T25_PLUGIN,
    name:         "kl-graph-topo25-harness",
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
        executor_id:       T25_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T25_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T25_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nhm2, 0, "empty: NHM\u{2082}=0");
    assert_eq!(nag,  0, "empty: NAG=0");
    assert_eq!(nabs, 0, "empty: NABS=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nhm2, 0, "single: NHM\u{2082}=0 (no edges)");
    assert_eq!(nag,  0, "single: NAG=0 (no edges)");
    assert_eq!(nabs, 0, "single: NABS=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. ssum=2, sp=1.
// NHM₂: (1×1)²=1.
// NAG:  floor(2×10^12/(2×isqrt128(10^12)))=floor(2×10^12/(2×10^6))=1_000_000.
// NABS: isqrt64((2-2)×10^12/2)=isqrt64(0)=0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);
    add_edge(T25_ID_A, T25_ID_B, "t25.e.ab");

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nhm2, 1,         "k2: NHM\u{2082}=1 ((1\u{00d7}1)\u{00b2}=1; S=1)");
    assert_eq!(nag,  1_000_000, "k2: NAG=1_000_000 (S-uniform S=1; AM=GM for equal S)");
    assert_eq!(nabs, 0,         "k2: NABS=0 (\u{221a}((1+1-2)/(1+1))=\u{221a}0=0; S\u{2081}+S\u{2082}=2)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. ssum=4, sp=4.
// NHM₂ per edge: (2×2)²=16. Total: 32.
// NAG  per edge: isqrt128(4×10^12)=2_000_000; floor(4×10^12/4_000_000)=1_000_000. Total: 2_000_000.
// NABS per edge: isqrt64(2×10^12/4)=isqrt64(500_000_000_000)=707_106. Total: 1_414_212.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);
    add_node(T25_VEC_C, T25_KEY_C, T25_ID_C);
    add_edge(T25_ID_A, T25_ID_B, "t25.e.ab");
    add_edge(T25_ID_B, T25_ID_C, "t25.e.bc");

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(nhm2, 32,        "p3: NHM\u{2082}=32 (2\u{00d7}16; (2\u{00d7}2)\u{00b2}=16; S-uniform S=2)");
    assert_eq!(nag,  2_000_000, "p3: NAG=2_000_000 (2\u{00d7}1_000_000; S-uniform\u{2192}|E|\u{00d7}10\u{2076})");
    assert_eq!(nabs, 1_414_212, "p3: NABS=1_414_212 (2\u{00d7}707_106; isqrt64(500_000_000_000)=707_106)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. ssum=8, sp=16.
// NHM₂ per edge: (4×4)²=256. Total: 768.
// NAG  per edge: isqrt128(16×10^12)=4_000_000; floor(8×10^12/8_000_000)=1_000_000. Total: 3_000_000.
// NABS per edge: isqrt64(6×10^12/8)=isqrt64(750_000_000_000)=866_025. Total: 2_598_075.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);
    add_node(T25_VEC_C, T25_KEY_C, T25_ID_C);
    add_edge(T25_ID_A, T25_ID_B, "t25.e.ab");
    add_edge(T25_ID_B, T25_ID_A, "t25.e.ba");
    add_edge(T25_ID_B, T25_ID_C, "t25.e.bc");
    add_edge(T25_ID_C, T25_ID_B, "t25.e.cb");
    add_edge(T25_ID_A, T25_ID_C, "t25.e.ac");
    add_edge(T25_ID_C, T25_ID_A, "t25.e.ca");

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   3,         "k3: node_count=3");
    assert_eq!(ec,   3,         "k3: edge_count=3");
    assert_eq!(nhm2, 768,       "k3: NHM\u{2082}=768 (3\u{00d7}256; (4\u{00d7}4)\u{00b2}=256; S-uniform S=4)");
    assert_eq!(nag,  3_000_000, "k3: NAG=3_000_000 (3\u{00d7}1_000_000; S-uniform\u{2192}|E|\u{00d7}10\u{2076})");
    assert_eq!(nabs, 2_598_075, "k3: NABS=2_598_075 (3\u{00d7}866_025; isqrt64(750_000_000_000)=866_025)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4×1=4, S(leaf)=deg(hub)=4. S-uniform S=4.
// Same per-edge values as K₃ (S-uniform S=4 coincidence), but 4 edges.
// NHM₂: 4×256=1024.   NAG: 4×1_000_000=4_000_000.   NABS: 4×866_025=3_464_100.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);
    add_node(T25_VEC_C, T25_KEY_C, T25_ID_C);
    add_node(T25_VEC_D, T25_KEY_D, T25_ID_D);
    add_node(T25_VEC_E, T25_KEY_E, T25_ID_E);
    add_edge(T25_ID_A, T25_ID_B, "t25.e.ab");
    add_edge(T25_ID_A, T25_ID_C, "t25.e.ac");
    add_edge(T25_ID_A, T25_ID_D, "t25.e.ad");
    add_edge(T25_ID_A, T25_ID_E, "t25.e.ae");

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   5,         "star: node_count=5");
    assert_eq!(ec,   4,         "star: edge_count=4");
    assert_eq!(nhm2, 1024,      "star: NHM\u{2082}=1024 (4\u{00d7}256; S-uniform S=4 same as K\u{2083})");
    assert_eq!(nag,  4_000_000, "star: NAG=4_000_000 (4\u{00d7}1_000_000; S-uniform S=4 coincidence)");
    assert_eq!(nabs, 3_464_100, "star: NABS=3_464_100 (4\u{00d7}866_025; S-uniform S=4 coincidence)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=3, S(C)=deg(B)+deg(D)=3, S(D)=deg(C)=2.
// Edge A-B (sa=2,sb=3, ssum=5, sp=6):
//   NHM₂=(2×3)²=36; NAG=1_020_621; NABS=774_596.
// Edge B-C (sa=3,sb=3, ssum=6, sp=9):
//   NHM₂=(3×3)²=81; NAG=1_000_000 (exact, S-uniform edge); NABS=816_496.
// Edge C-D: same as A-B.
// NHM₂=153. NAG=3_041_242. NABS=2_365_688.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);
    add_node(T25_VEC_C, T25_KEY_C, T25_ID_C);
    add_node(T25_VEC_D, T25_KEY_D, T25_ID_D);
    add_edge(T25_ID_A, T25_ID_B, "t25.e.ab");
    add_edge(T25_ID_B, T25_ID_C, "t25.e.bc");
    add_edge(T25_ID_C, T25_ID_D, "t25.e.cd");

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(nhm2, 153,       "p4: NHM\u{2082}=153 (36+81+36; (2\u{00d7}3)\u{00b2}+(3\u{00d7}3)\u{00b2}+(3\u{00d7}2)\u{00b2})");
    assert_eq!(nag,  3_041_242, "p4: NAG=3_041_242 (1_020_621+1_000_000+1_020_621; floor(5e12/4_898_978)=1_020_621)");
    assert_eq!(nabs, 2_365_688, "p4: NABS=2_365_688 (774_596+816_496+774_596; mixed S=2,3)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. ssum=18, sp=81.
// NHM₂ per edge: (9×9)²=6561. Total: 39366.
// NAG  per edge: isqrt128(81×10^12)=9_000_000 (exact); floor(18×10^12/18_000_000)=1_000_000. Total: 6_000_000.
// NABS per edge: isqrt64(16×10^12/18)=isqrt64(888_888_888_888)=942_809. Total: 5_656_854.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);
    add_node(T25_VEC_C, T25_KEY_C, T25_ID_C);
    add_node(T25_VEC_D, T25_KEY_D, T25_ID_D);
    add_edge(T25_ID_A, T25_ID_B, "t25.e.ab");
    add_edge(T25_ID_B, T25_ID_A, "t25.e.ba");
    add_edge(T25_ID_A, T25_ID_C, "t25.e.ac");
    add_edge(T25_ID_C, T25_ID_A, "t25.e.ca");
    add_edge(T25_ID_A, T25_ID_D, "t25.e.ad");
    add_edge(T25_ID_D, T25_ID_A, "t25.e.da");
    add_edge(T25_ID_B, T25_ID_C, "t25.e.bc");
    add_edge(T25_ID_C, T25_ID_B, "t25.e.cb");
    add_edge(T25_ID_B, T25_ID_D, "t25.e.bd");
    add_edge(T25_ID_D, T25_ID_B, "t25.e.db");
    add_edge(T25_ID_C, T25_ID_D, "t25.e.cd");
    add_edge(T25_ID_D, T25_ID_C, "t25.e.dc");

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   4,         "k4: node_count=4");
    assert_eq!(ec,   6,         "k4: edge_count=6");
    assert_eq!(nhm2, 39366,     "k4: NHM\u{2082}=39366 (6\u{00d7}6561; (9\u{00d7}9)\u{00b2}=6561; S-uniform S=9)");
    assert_eq!(nag,  6_000_000, "k4: NAG=6_000_000 (6\u{00d7}1_000_000; S-uniform\u{2192}|E|\u{00d7}10\u{2076})");
    assert_eq!(nabs, 5_656_854, "k4: NABS=5_656_854 (6\u{00d7}942_809; isqrt64(888_888_888_888)=942_809)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nhm2, 0, "isolated: NHM\u{2082}=0 (no edges)");
    assert_eq!(nag,  0, "isolated: NAG=0 (no edges)");
    assert_eq!(nabs, 0, "isolated: NABS=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. ssum=12, sp=36.
// NHM₂ per edge: (6×6)²=1296. Total: 7776.
// NAG  per edge: isqrt128(36×10^12)=6_000_000 (exact); floor(12×10^12/12_000_000)=1_000_000. Total: 6_000_000.
//   (Note: K₄ and K_{2,3} both NAG=6_000_000; same |E|=6 and both S-uniform, regardless of S value)
// NABS per edge: isqrt64(10×10^12/12)=isqrt64(833_333_333_333)=912_870. Total: 5_477_220.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T25_VEC_A, T25_KEY_A, T25_ID_A);
    add_node(T25_VEC_B, T25_KEY_B, T25_ID_B);
    add_node(T25_VEC_C, T25_KEY_C, T25_ID_C);
    add_node(T25_VEC_D, T25_KEY_D, T25_ID_D);
    add_node(T25_VEC_E, T25_KEY_E, T25_ID_E);
    add_edge(T25_ID_A, T25_ID_C, "t25.e.ac");
    add_edge(T25_ID_C, T25_ID_A, "t25.e.ca");
    add_edge(T25_ID_A, T25_ID_D, "t25.e.ad");
    add_edge(T25_ID_D, T25_ID_A, "t25.e.da");
    add_edge(T25_ID_A, T25_ID_E, "t25.e.ae");
    add_edge(T25_ID_E, T25_ID_A, "t25.e.ea");
    add_edge(T25_ID_B, T25_ID_C, "t25.e.bc");
    add_edge(T25_ID_C, T25_ID_B, "t25.e.cb");
    add_edge(T25_ID_B, T25_ID_D, "t25.e.bd");
    add_edge(T25_ID_D, T25_ID_B, "t25.e.db");
    add_edge(T25_ID_B, T25_ID_E, "t25.e.be");
    add_edge(T25_ID_E, T25_ID_B, "t25.e.eb");

    let (nhm2, nag, nabs, ec, nc) = gos_runtime::graph_topo_indices25();
    assert_eq!(nc,   5,         "k23: node_count=5");
    assert_eq!(ec,   6,         "k23: edge_count=6");
    assert_eq!(nhm2, 7776,      "k23: NHM\u{2082}=7776 (6\u{00d7}1296; (6\u{00d7}6)\u{00b2}=1296; S-uniform S=6)");
    assert_eq!(nag,  6_000_000, "k23: NAG=6_000_000 (6\u{00d7}1_000_000; S-uniform; same as K\u{2084} NAG)");
    assert_eq!(nabs, 5_477_220, "k23: NABS=5_477_220 (6\u{00d7}912_870; isqrt64(833_333_333_333)=912_870)");
}
