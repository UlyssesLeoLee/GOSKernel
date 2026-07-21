// gos-graph-topo84-harness — V3.95 NOCTOPENTAACTC + NHOCTOPENTAACTC + NBASO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices84()`:
//   Returns (noctopentaactc, nhoctopentaactc, nbaso, edge_count, node_count)
//   - noctopentaactc  = NOCTOPENTAACTC(G)  = Σ_v S(v)^58                   (exact u64; S-Octopentacontic vertex sum)
//   - nhoctopentaactc = NHOCTOPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^57         (exact u64; S-Heptapentacontic edge-sum)
//   - nbaso           = NBASO(G)           = Σ_{uv∈E} (S_u²+S_v²)^52       (exact u64; S-Variant Sombor, α=104)
//   - edge_count      = undirected non-self-loop edges
//   - node_count      = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTOPENTAACTC(G) = Σ_v S(v)^58
//     S-Octopentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEPTPENTAACTC=Σ S⁵⁷ (topo83), NOCTOPENTAACTC=Σ S⁵⁸ (topo84). Ninth of the pentacontic (50-59) series.
//     NOCTOPENTAACTC = n·S^58 for S-regular.
//     Overflow: S^58 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^58 = s32 × s16 × s8 × s2  (58=32+16+8+2; 4 mults).
//
//   NHOCTOPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^57
//     S-Heptapentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEPTPENTAACTC=Σ(S+S)⁵⁶ (topo83),
//       NHOCTOPENTAACTC=Σ(S+S)⁵⁷ (topo84).
//     NHOCTOPENTAACTC = |E|·(2S)^57 = 144115188075855872|E|·S^57 for S-regular.
//     Overflow per edge: (2×16129)^57 → saturating u128 accumulator.
//     Implementation: ss^57 = ss32 × ss16 × ss8 × ss  (57=32+16+8+1; 4 mults).
//
//   NBASO(G) = Σ_{uv∈E} (S_u²+S_v²)^52
//     S-Variant Sombor: generalised Sombor SO^α with α=104 on S-variant.
//     4th-pass double-letter "BA" (after NAZSO α=102, topo83; first of NB series).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NAZSO(topo83,α=102), NBASO(topo84,α=104).
//     NBASO = |E|·(2S²)^52 = 4503599627370496|E|·S^104 for S-regular.
//     Overflow per edge: (2×16129²)^52 → saturating u128 accumulator.
//     Implementation: s2s^52 = s2s32 × s2s16 × s2s4  (52=32+16+4; 3 mults — efficient!).
//
// S VALUES PER GRAPH:
//   K₂        : S(A)=S(B)=1
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2    → S-uniform S=2
//   K₃        : S(each)=4            → S-uniform S=4
//   K_{1,4}   : S(hub)=4, S(leaf)=4  → S-uniform S=4
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3 → mixed S
//   K₄        : S(each)=9            → S-uniform S=9
//   K_{2,3}   : S(all)=6             → S-uniform S=6
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph     NOCTOPENTAACTC(exact)            NHOCTOPENTAACTC(exact)      NBASO(exact)              edges  nodes
//  Empty                       0                             0                      0                0      0
//  1 node                      0                             0                      0                0      1
//  K₂                          2           144_115_188_075_855_872    4_503_599_627_370_496              1      2
//  P₃       864_691_128_455_135_232              u64::MAX(sat.)             u64::MAX(sat.)            2      3
//  K₃              u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            3      3
//  K_{1,4}         u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            4      5
//  P₄              u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            3      4
//  K₄              u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            6      4
//  2 isolated                  0                             0                      0                0      2
//  K_{2,3}         u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTOPENTAACTC:  1^58 + 1^58 = 2. ✓
//     NHOCTOPENTAACTC: (1+1)^57 = 2^57 = 144_115_188_075_855_872. ✓
//     NBASO:           (1²+1²)^52 = 2^52 = 4_503_599_627_370_496. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTOPENTAACTC:  3×2^58 = 3×288_230_376_151_711_744 = 864_691_128_455_135_232. ✓
//     NHOCTOPENTAACTC: 2×(2+2)^57 = 2×4^57 = 2×2^114 → SATURATES. ✓
//     NBASO:           2×(4+4)^52 = 2×8^52 = 2×2^156 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTOPENTAACTC:  3×4^58 = 3×2^116 → SATURATES. ✓
//     NHOCTOPENTAACTC: 3×8^57 → SATURATES. ✓
//     NBASO:           3×32^52 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTOPENTAACTC:  5×4^58 → SATURATES. ✓
//     NHOCTOPENTAACTC: 4×8^57 → SATURATES. ✓
//     NBASO:           4×32^52 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTOPENTAACTC:  2×2^58 + 2×3^58.  3^41>u64::MAX → 3^58 >> u64::MAX → SATURATES. ✓
//     NHOCTOPENTAACTC: 5^57+6^57+5^57 → each term >> u64::MAX → SATURATES. ✓
//     NBASO:           13^52+18^52+13^52 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTOPENTAACTC:  4×9^58 → SATURATES. ✓
//     NHOCTOPENTAACTC: 6×18^57 → SATURATES. ✓
//     NBASO:           6×162^52 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTOPENTAACTC:  5×6^58 → SATURATES. ✓
//     NHOCTOPENTAACTC: 6×12^57 → SATURATES. ✓
//     NBASO:           6×72^52 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTOPENTAACTC  = n·S^58                                                                           for S-regular ✓
//   NHOCTOPENTAACTC = |E|·(2S)^57 = 144115188075855872|E|·S^57                                        for S-regular ✓
//   NBASO           = |E|·(2S²)^52 = 4503599627370496|E|·S^104                                        for S-regular ✓
//   Note: s2s^52 = s2s32×s2s16×s2s4 is efficient (52=32+16+4, three powers of 2, only 3 mults)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 144_115_188_075_855_872, 4_503_599_627_370_496, 1, 2)
//  4.  Path P₃ = A-B-C                   → (864_691_128_455_135_232, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
//  8.  Complete K₄                       → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (u64::MAX, u64::MAX, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T84_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_84");
const T84_EXEC:   ExecutorId = ExecutorId::from_ascii("t84.exec");

const T84_KEY_A: &str = "t84.alpha";
const T84_KEY_B: &str = "t84.beta";
const T84_KEY_C: &str = "t84.gamma";
const T84_KEY_D: &str = "t84.delta";
const T84_KEY_E: &str = "t84.epsilon";

const T84_ID_A: NodeId = derive_node_id(T84_PLUGIN, T84_KEY_A);
const T84_ID_B: NodeId = derive_node_id(T84_PLUGIN, T84_KEY_B);
const T84_ID_C: NodeId = derive_node_id(T84_PLUGIN, T84_KEY_C);
const T84_ID_D: NodeId = derive_node_id(T84_PLUGIN, T84_KEY_D);
const T84_ID_E: NodeId = derive_node_id(T84_PLUGIN, T84_KEY_E);

// L4=171 namespace for this harness.
const T84_VEC_A: VectorAddress = VectorAddress::new(171, 1, 1, 0);
const T84_VEC_B: VectorAddress = VectorAddress::new(171, 1, 2, 0);
const T84_VEC_C: VectorAddress = VectorAddress::new(171, 1, 3, 0);
const T84_VEC_D: VectorAddress = VectorAddress::new(171, 2, 1, 0);
const T84_VEC_E: VectorAddress = VectorAddress::new(171, 2, 2, 0);

const T84_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T84_PLUGIN,
    name:         "kl-graph-topo84-harness",
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
        executor_id:       T84_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T84_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T84_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(noctopentaactc,    0, "empty: NOCTOPENTAACTC=0");
    assert_eq!(nhoctopentaactc,   0, "empty: NHOCTOPENTAACTC=0");
    assert_eq!(nbaso,             0, "empty: NBASO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(noctopentaactc,    0, "single: NOCTOPENTAACTC=0");
    assert_eq!(nhoctopentaactc,   0, "single: NHOCTOPENTAACTC=0");
    assert_eq!(nbaso,             0, "single: NBASO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTOPENTAACTC:  1^58+1^58 = 2.
// NHOCTOPENTAACTC: (1+1)^57 = 2^57 = 144_115_188_075_855_872.
// NBASO:           (1²+1²)^52 = 2^52 = 4_503_599_627_370_496.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);
    add_edge(T84_ID_A, T84_ID_B, "t84.e.ab");

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,                2,                           "k2: node_count=2");
    assert_eq!(ec,                1,                           "k2: edge_count=1");
    assert_eq!(noctopentaactc,    2,                           "k2: NOCTOPENTAACTC=2 (1\u{2075}\u{2078}+1\u{2075}\u{2078}=2)");
    assert_eq!(nhoctopentaactc,   144_115_188_075_855_872,     "k2: NHOCTOPENTAACTC=144_115_188_075_855_872 (2\u{2075}\u{2077}=2^57)");
    assert_eq!(nbaso,             4_503_599_627_370_496,       "k2: NBASO=4_503_599_627_370_496 (2\u{2075}\u{00b2}=2^52)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NOCTOPENTAACTC:  3×2^58 = 3×288_230_376_151_711_744 = 864_691_128_455_135_232.
// NHOCTOPENTAACTC: 2×(2+2)^57 = 2×4^57 = 2×2^114 → SATURATES.
// NBASO:           2×(4+4)^52 = 2×8^52 = 2×2^156 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);
    add_node(T84_VEC_C, T84_KEY_C, T84_ID_C);
    add_edge(T84_ID_A, T84_ID_B, "t84.e.ab");
    add_edge(T84_ID_B, T84_ID_C, "t84.e.bc");

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,                3,                           "p3: node_count=3");
    assert_eq!(ec,                2,                           "p3: edge_count=2");
    assert_eq!(noctopentaactc,    864_691_128_455_135_232,     "p3: NOCTOPENTAACTC=864_691_128_455_135_232 (3\u{00d7}2\u{2075}\u{2078})");
    assert_eq!(nhoctopentaactc,   u64::MAX,                    "p3: NHOCTOPENTAACTC=SAT (4\u{2075}\u{2077}>u64)");
    assert_eq!(nbaso,             u64::MAX,                    "p3: NBASO=SAT (8\u{2075}\u{00b2}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);
    add_node(T84_VEC_C, T84_KEY_C, T84_ID_C);
    add_edge(T84_ID_A, T84_ID_B, "t84.e.ab");
    add_edge(T84_ID_B, T84_ID_C, "t84.e.bc");
    add_edge(T84_ID_C, T84_ID_A, "t84.e.ca");

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(noctopentaactc,   u64::MAX, "k3: NOCTOPENTAACTC=SAT");
    assert_eq!(nhoctopentaactc,  u64::MAX, "k3: NHOCTOPENTAACTC=SAT");
    assert_eq!(nbaso,            u64::MAX, "k3: NBASO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A); // hub
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);
    add_node(T84_VEC_C, T84_KEY_C, T84_ID_C);
    add_node(T84_VEC_D, T84_KEY_D, T84_ID_D);
    add_node(T84_VEC_E, T84_KEY_E, T84_ID_E);
    add_edge(T84_ID_A, T84_ID_B, "t84.e.ab");
    add_edge(T84_ID_A, T84_ID_C, "t84.e.ac");
    add_edge(T84_ID_A, T84_ID_D, "t84.e.ad");
    add_edge(T84_ID_A, T84_ID_E, "t84.e.ae");

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(noctopentaactc,   u64::MAX, "k14: NOCTOPENTAACTC=SAT");
    assert_eq!(nhoctopentaactc,  u64::MAX, "k14: NHOCTOPENTAACTC=SAT");
    assert_eq!(nbaso,            u64::MAX, "k14: NBASO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NOCTOPENTAACTC:  2×2^58 + 2×3^58.  3^41>u64::MAX → SATURATES.
// NHOCTOPENTAACTC: 5^57+6^57+5^57 → SATURATES.
// NBASO:           13^52+18^52+13^52 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);
    add_node(T84_VEC_C, T84_KEY_C, T84_ID_C);
    add_node(T84_VEC_D, T84_KEY_D, T84_ID_D);
    add_edge(T84_ID_A, T84_ID_B, "t84.e.ab");
    add_edge(T84_ID_B, T84_ID_C, "t84.e.bc");
    add_edge(T84_ID_C, T84_ID_D, "t84.e.cd");

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(noctopentaactc,   u64::MAX, "p4: NOCTOPENTAACTC=SAT");
    assert_eq!(nhoctopentaactc,  u64::MAX, "p4: NHOCTOPENTAACTC=SAT");
    assert_eq!(nbaso,            u64::MAX, "p4: NBASO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);
    add_node(T84_VEC_C, T84_KEY_C, T84_ID_C);
    add_node(T84_VEC_D, T84_KEY_D, T84_ID_D);
    add_edge(T84_ID_A, T84_ID_B, "t84.e.ab");
    add_edge(T84_ID_A, T84_ID_C, "t84.e.ac");
    add_edge(T84_ID_A, T84_ID_D, "t84.e.ad");
    add_edge(T84_ID_B, T84_ID_C, "t84.e.bc");
    add_edge(T84_ID_B, T84_ID_D, "t84.e.bd");
    add_edge(T84_ID_C, T84_ID_D, "t84.e.cd");

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(noctopentaactc,   u64::MAX, "k4: NOCTOPENTAACTC=SAT");
    assert_eq!(nhoctopentaactc,  u64::MAX, "k4: NHOCTOPENTAACTC=SAT");
    assert_eq!(nbaso,            u64::MAX, "k4: NBASO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,                2, "isolated: node_count=2");
    assert_eq!(ec,                0, "isolated: edge_count=0");
    assert_eq!(noctopentaactc,    0, "isolated: NOCTOPENTAACTC=0");
    assert_eq!(nhoctopentaactc,   0, "isolated: NHOCTOPENTAACTC=0");
    assert_eq!(nbaso,             0, "isolated: NBASO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NOCTOPENTAACTC=5×6^58 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T84_VEC_A, T84_KEY_A, T84_ID_A);
    add_node(T84_VEC_B, T84_KEY_B, T84_ID_B);
    add_node(T84_VEC_C, T84_KEY_C, T84_ID_C);
    add_node(T84_VEC_D, T84_KEY_D, T84_ID_D);
    add_node(T84_VEC_E, T84_KEY_E, T84_ID_E);
    add_edge(T84_ID_A, T84_ID_C, "t84.e.ac");
    add_edge(T84_ID_A, T84_ID_D, "t84.e.ad");
    add_edge(T84_ID_A, T84_ID_E, "t84.e.ae");
    add_edge(T84_ID_B, T84_ID_C, "t84.e.bc");
    add_edge(T84_ID_B, T84_ID_D, "t84.e.bd");
    add_edge(T84_ID_B, T84_ID_E, "t84.e.be");

    let (noctopentaactc, nhoctopentaactc, nbaso, ec, nc) = gos_runtime::graph_topo_indices84();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(noctopentaactc,   u64::MAX, "k23: NOCTOPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{2078})");
    assert_eq!(nhoctopentaactc,  u64::MAX, "k23: NHOCTOPENTAACTC=SAT");
    assert_eq!(nbaso,            u64::MAX, "k23: NBASO=SAT");
}
