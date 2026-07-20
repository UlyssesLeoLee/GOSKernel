// gos-graph-topo64-harness — V3.75 NOCTATRIACTC + NHOCTATRIACTC + NAGSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices64()`:
//   Returns (noctatriactc, nhoctatriactc, nagso, edge_count, node_count)
//   - noctatriactc  = NOCTATRIACTC(G) = Σ_v S(v)^38                   (exact u64; S-Octatriacontic vertex sum)
//   - nhoctatriactc = NHOCTATRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^37         (exact u64; S-Heptatriacontic edge-sum)
//   - nagso         = NAGSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^32       (exact u64; S-Tetrahexacontyl Sombor, α=64)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTATRIACTC(G) = Σ_v S(v)^38
//     S-Octatriacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56), NHENTRIACTC=Σ S³¹ (topo57),
//       NDOTRIACTC=Σ S³² (topo58), NTRITRIACTC=Σ S³³ (topo59), NTETRTRIACTC=Σ S³⁴ (topo60),
//       NPENTTRIACTC=Σ S³⁵ (topo61), NHEXATRIACTC=Σ S³⁶ (topo62), NHEPTATRIACTC=Σ S³⁷ (topo63),
//       NOCTATRIACTC=Σ S³⁸ (topo64).
//     NOCTATRIACTC = n·S^38 for S-regular.
//     Overflow: S^38 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^38 = s32 × s4 × s2  (s32=s16^2 perfect square; s4=s2^2; 38=32+4+2).
//
//   NHOCTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^37
//     S-Heptatriacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53), NHOCTATC=Σ(S+S)²⁷ (topo54), NHNONATC=Σ(S+S)²⁸ (topo55),
//       NHTRIACTC=Σ(S+S)²⁹ (topo56), NHHENTRIACTC=Σ(S+S)³⁰ (topo57),
//       NHDOTRIACTC=Σ(S+S)³¹ (topo58), NHTRITRIACTC=Σ(S+S)³² (topo59),
//       NHTETRTRIACTC=Σ(S+S)³³ (topo60), NHPENTTRIACTC=Σ(S+S)³⁴ (topo61),
//       NHHEXATRIACTC=Σ(S+S)³⁵ (topo62), NHHEPTATRIACTC=Σ(S+S)³⁶ (topo63),
//       NHOCTATRIACTC=Σ(S+S)³⁷ (topo64).
//     NHOCTATRIACTC = |E|·(2S)^37 = 137438953472|E|·S^37 for S-regular.
//     Overflow per edge: (2×16129)^37 → saturating u128 accumulator.
//     Implementation: ss^37 = ss32 × ss4 × ss  (ss32=ss16^2; ss4=ss2^2; 37=32+4+1).
//
//   NAGSO(G) = Σ_{uv∈E} (S_u²+S_v²)^32
//     S-Tetrahexacontyl Sombor: generalised Sombor SO^α with α=64 on S-variant.
//     3rd-pass double-letter "AG" (after NAFSO α=62, topo63).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60), NAFSO(topo63,α=62),
//     NAGSO(topo64,α=64).
//     NAGSO = |E|·(2S²)^32 = 4294967296|E|·S^64 for S-regular.
//     Overflow per edge: (2×16129²)^32 → saturating u128 accumulator.
//     Implementation: s2s^32 = s2s16 × s2s16  (perfect square; 32=16+16).
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
//  Graph     NOCTATRIACTC(exact)              NHOCTATRIACTC(exact)         NAGSO(exact)             edges  nodes
//  Empty                      0                               0                        0               0      0
//  1 node                     0                               0                        0               0      1
//  K₂                         2                 137_438_953_472               4_294_967_296               1      2
//  P₃           824_633_720_832              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄     2_701_703_985_101_798_066           u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                 0                               0                        0               0      2
//  K_{2,3}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTATRIACTC:  1^38 + 1^38 = 2. ✓
//     NHOCTATRIACTC: (1+1)^37 = 2^37 = 137_438_953_472. ✓
//     NAGSO:          (1²+1²)^32 = 2^32 = 4_294_967_296. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTATRIACTC:  3×2^38 = 3×274_877_906_944 = 824_633_720_832. ✓
//     NHOCTATRIACTC: 2×(2+2)^37 = 2×4^37 = 2×2^74 → SATURATES (4^37=2^74>u64::MAX per-edge). ✓
//     NAGSO:          2×(4+4)^32 = 2×8^32 = 2×2^96 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTATRIACTC:  3×4^38 = 3×2^76 → SATURATES (2^76>u64::MAX per-node). ✓
//     NHOCTATRIACTC: 3×(4+4)^37 = 3×8^37 = 3×2^111 → SATURATES. ✓
//     NAGSO:          3×(16+16)^32 = 3×32^32 = 3×2^160 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTATRIACTC:  5×4^38 → SATURATES. ✓
//     NHOCTATRIACTC: 4×8^37 → SATURATES. ✓
//     NAGSO:          4×32^32 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTATRIACTC:  2×2^38 + 2×3^38.
//       3^32=1_853_020_188_851_841; 3^36=3^32×81=150_094_635_296_999_121; 3^38=3^36×9=1_350_851_717_672_992_089.
//       2×3^38=2_701_703_435_345_984_178. 2×2^38=549_755_813_888.
//       Total=2_701_703_435_345_984_178+549_755_813_888=2_701_703_985_101_798_066. ✓
//     NHOCTATRIACTC: (2+3)^37+(3+3)^37+(3+2)^37 = 2×5^37+6^37
//       5^37>>u64::MAX per-edge → SATURATES. ✓
//     NAGSO:          2×13^32+18^32 — 13^32>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTATRIACTC:  4×9^38 → SATURATES → u64::MAX. ✓
//     NHOCTATRIACTC: 6×18^37 → SATURATES. ✓
//     NAGSO:          6×162^32 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTATRIACTC:  5×6^38 → SATURATES → u64::MAX. ✓
//     NHOCTATRIACTC: 6×12^37 → SATURATES. ✓
//     NAGSO:          6×72^32 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTATRIACTC  = n·S^38                                                     for S-regular ✓
//   NHOCTATRIACTC = |E|·(2S)^37 = 137438953472|E|·S^37                          for S-regular ✓
//   NAGSO         = |E|·(2S²)^32 = 4294967296|E|·S^64                           for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 137_438_953_472, 4_294_967_296, 1, 2)
//  4.  Path P₃ = A-B-C                   → (824_633_720_832, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (2_701_703_985_101_798_066, u64::MAX, u64::MAX, 3, 4)
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

const T64_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_64");
const T64_EXEC:   ExecutorId = ExecutorId::from_ascii("t64.exec");

const T64_KEY_A: &str = "t64.alpha";
const T64_KEY_B: &str = "t64.beta";
const T64_KEY_C: &str = "t64.gamma";
const T64_KEY_D: &str = "t64.delta";
const T64_KEY_E: &str = "t64.epsilon";

const T64_ID_A: NodeId = derive_node_id(T64_PLUGIN, T64_KEY_A);
const T64_ID_B: NodeId = derive_node_id(T64_PLUGIN, T64_KEY_B);
const T64_ID_C: NodeId = derive_node_id(T64_PLUGIN, T64_KEY_C);
const T64_ID_D: NodeId = derive_node_id(T64_PLUGIN, T64_KEY_D);
const T64_ID_E: NodeId = derive_node_id(T64_PLUGIN, T64_KEY_E);

// L4=151 namespace for this harness.
const T64_VEC_A: VectorAddress = VectorAddress::new(151, 1, 1, 0);
const T64_VEC_B: VectorAddress = VectorAddress::new(151, 1, 2, 0);
const T64_VEC_C: VectorAddress = VectorAddress::new(151, 1, 3, 0);
const T64_VEC_D: VectorAddress = VectorAddress::new(151, 2, 1, 0);
const T64_VEC_E: VectorAddress = VectorAddress::new(151, 2, 2, 0);

const T64_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T64_PLUGIN,
    name:         "kl-graph-topo64-harness",
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
        executor_id:       T64_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T64_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T64_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(noctatriactc,     0, "empty: NOCTATRIACTC=0");
    assert_eq!(nhoctatriactc,    0, "empty: NHOCTATRIACTC=0");
    assert_eq!(nagso,            0, "empty: NAGSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(noctatriactc,     0, "single: NOCTATRIACTC=0");
    assert_eq!(nhoctatriactc,    0, "single: NHOCTATRIACTC=0");
    assert_eq!(nagso,            0, "single: NAGSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTATRIACTC:  1^38+1^38 = 2.
// NHOCTATRIACTC: (1+1)^37 = 2^37 = 137_438_953_472.
// NAGSO:          (1²+1²)^32 = 2^32 = 4_294_967_296.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);
    add_edge(T64_ID_A, T64_ID_B, "t64.e.ab");

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               2,               "k2: node_count=2");
    assert_eq!(ec,               1,               "k2: edge_count=1");
    assert_eq!(noctatriactc,     2,               "k2: NOCTATRIACTC=2 (1\u{00b3}\u{2078}+1\u{00b3}\u{2078}=2)");
    assert_eq!(nhoctatriactc,    137_438_953_472, "k2: NHOCTATRIACTC=137_438_953_472 (2\u{00b3}\u{2077}=2^37)");
    assert_eq!(nagso,            4_294_967_296,   "k2: NAGSO=4_294_967_296 (2\u{00b3}\u{00b2}=2^32)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NOCTATRIACTC:  3×2^38 = 3×274_877_906_944 = 824_633_720_832.
// NHOCTATRIACTC: 2×(2+2)^37 = 2×4^37 = 2×2^74 → SATURATES (4^37=2^74>u64::MAX per-edge).
// NAGSO:          2×(4+4)^32 = 2×8^32 = 2×2^96 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);
    add_node(T64_VEC_C, T64_KEY_C, T64_ID_C);
    add_edge(T64_ID_A, T64_ID_B, "t64.e.ab");
    add_edge(T64_ID_B, T64_ID_C, "t64.e.bc");

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               3,               "p3: node_count=3");
    assert_eq!(ec,               2,               "p3: edge_count=2");
    assert_eq!(noctatriactc,     824_633_720_832,  "p3: NOCTATRIACTC=824_633_720_832 (3\u{00d7}2\u{00b3}\u{2078})");
    assert_eq!(nhoctatriactc,    u64::MAX,         "p3: NHOCTATRIACTC=u64::MAX (4\u{00b3}\u{2077}=2^74>u64::MAX per-edge; saturated)");
    assert_eq!(nagso,            u64::MAX,         "p3: NAGSO=u64::MAX (8\u{00b3}\u{00b2}=2^96>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NOCTATRIACTC:  3×4^38 = 3×2^76 → SATURATES (2^76>u64::MAX per-node).
// NHOCTATRIACTC: 3×(4+4)^37 = 3×8^37 = 3×2^111 → SATURATES.
// NAGSO:          3×(16+16)^32 = 3×32^32 = 3×2^160 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);
    add_node(T64_VEC_C, T64_KEY_C, T64_ID_C);
    add_edge(T64_ID_A, T64_ID_B, "t64.e.ab");
    add_edge(T64_ID_B, T64_ID_A, "t64.e.ba");
    add_edge(T64_ID_B, T64_ID_C, "t64.e.bc");
    add_edge(T64_ID_C, T64_ID_B, "t64.e.cb");
    add_edge(T64_ID_A, T64_ID_C, "t64.e.ac");
    add_edge(T64_ID_C, T64_ID_A, "t64.e.ca");

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(noctatriactc,     u64::MAX, "k3: NOCTATRIACTC=u64::MAX (3\u{00d7}4\u{00b3}\u{2078}>>u64::MAX; saturated)");
    assert_eq!(nhoctatriactc,    u64::MAX, "k3: NHOCTATRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{2077}=3\u{00d7}2^111>>u64::MAX; saturated)");
    assert_eq!(nagso,            u64::MAX, "k3: NAGSO=u64::MAX (3\u{00d7}32\u{00b3}\u{00b2}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NOCTATRIACTC:  5×4^38 → SATURATES.
// NHOCTATRIACTC: 4×(4+4)^37 → SATURATES.
// NAGSO:          4×(16+16)^32 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);
    add_node(T64_VEC_C, T64_KEY_C, T64_ID_C);
    add_node(T64_VEC_D, T64_KEY_D, T64_ID_D);
    add_node(T64_VEC_E, T64_KEY_E, T64_ID_E);
    add_edge(T64_ID_A, T64_ID_B, "t64.e.ab");
    add_edge(T64_ID_A, T64_ID_C, "t64.e.ac");
    add_edge(T64_ID_A, T64_ID_D, "t64.e.ad");
    add_edge(T64_ID_A, T64_ID_E, "t64.e.ae");

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(noctatriactc,     u64::MAX, "k14: NOCTATRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{2078}>u64::MAX; saturated)");
    assert_eq!(nhoctatriactc,    u64::MAX, "k14: NHOCTATRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{2077}>>u64::MAX; saturated)");
    assert_eq!(nagso,            u64::MAX, "k14: NAGSO=u64::MAX (4\u{00d7}32\u{00b3}\u{00b2}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NOCTATRIACTC:  2×2^38+2×3^38.
//   3^32=1_853_020_188_851_841; 3^36=3^32×81=150_094_635_296_999_121; 3^38=3^36×9=1_350_851_717_672_992_089.
//   2×3^38=2_701_703_435_345_984_178. 2×2^38=549_755_813_888.
//   Total=2_701_703_435_345_984_178+549_755_813_888=2_701_703_985_101_798_066.
// NHOCTATRIACTC: (2+3)^37+(3+3)^37+(3+2)^37 = 2×5^37+6^37; 5^37>>u64::MAX per-edge → SATURATES.
// NAGSO:          2×13^32+18^32 — 13^32>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);
    add_node(T64_VEC_C, T64_KEY_C, T64_ID_C);
    add_node(T64_VEC_D, T64_KEY_D, T64_ID_D);
    add_edge(T64_ID_A, T64_ID_B, "t64.e.ab");
    add_edge(T64_ID_B, T64_ID_C, "t64.e.bc");
    add_edge(T64_ID_C, T64_ID_D, "t64.e.cd");

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               4,                             "p4: node_count=4");
    assert_eq!(ec,               3,                             "p4: edge_count=3");
    assert_eq!(noctatriactc,     2_701_703_985_101_798_066,     "p4: NOCTATRIACTC=2_701_703_985_101_798_066 (2\u{00d7}2\u{00b3}\u{2078}+2\u{00d7}3\u{00b3}\u{2078}; 3\u{00b3}\u{2078}=1_350_851_717_672_992_089)");
    assert_eq!(nhoctatriactc,    u64::MAX,                      "p4: NHOCTATRIACTC=u64::MAX (5\u{00b3}\u{2077}>>u64::MAX per-edge; saturated)");
    assert_eq!(nagso,            u64::MAX,                      "p4: NAGSO=u64::MAX (13\u{00b3}\u{00b2}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NOCTATRIACTC:  4×9^38 → SATURATES → u64::MAX.
// NHOCTATRIACTC: 6×18^37 → SATURATES → u64::MAX.
// NAGSO:          6×162^32 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);
    add_node(T64_VEC_C, T64_KEY_C, T64_ID_C);
    add_node(T64_VEC_D, T64_KEY_D, T64_ID_D);
    add_edge(T64_ID_A, T64_ID_B, "t64.e.ab");
    add_edge(T64_ID_B, T64_ID_A, "t64.e.ba");
    add_edge(T64_ID_A, T64_ID_C, "t64.e.ac");
    add_edge(T64_ID_C, T64_ID_A, "t64.e.ca");
    add_edge(T64_ID_A, T64_ID_D, "t64.e.ad");
    add_edge(T64_ID_D, T64_ID_A, "t64.e.da");
    add_edge(T64_ID_B, T64_ID_C, "t64.e.bc");
    add_edge(T64_ID_C, T64_ID_B, "t64.e.cb");
    add_edge(T64_ID_B, T64_ID_D, "t64.e.bd");
    add_edge(T64_ID_D, T64_ID_B, "t64.e.db");
    add_edge(T64_ID_C, T64_ID_D, "t64.e.cd");
    add_edge(T64_ID_D, T64_ID_C, "t64.e.dc");

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(noctatriactc,     u64::MAX, "k4: NOCTATRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{2078} >> u64::MAX; saturated)");
    assert_eq!(nhoctatriactc,    u64::MAX, "k4: NHOCTATRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{2077} >> u64::MAX; saturated)");
    assert_eq!(nagso,            u64::MAX, "k4: NAGSO=u64::MAX (6\u{00d7}162\u{00b3}\u{00b2} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NOCTATRIACTC=0; NHOCTATRIACTC=0; NAGSO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               2, "two-iso: node_count=2");
    assert_eq!(ec,               0, "two-iso: edge_count=0");
    assert_eq!(noctatriactc,     0, "two-iso: NOCTATRIACTC=0");
    assert_eq!(nhoctatriactc,    0, "two-iso: NHOCTATRIACTC=0");
    assert_eq!(nagso,            0, "two-iso: NAGSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NOCTATRIACTC:  5×6^38 → SATURATES (6^38 >> u64::MAX per-node).
// NHOCTATRIACTC: 6×12^37 → SATURATES (12^37>>u64::MAX per-edge).
// NAGSO:          6×72^32 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T64_VEC_A, T64_KEY_A, T64_ID_A);
    add_node(T64_VEC_B, T64_KEY_B, T64_ID_B);
    add_node(T64_VEC_C, T64_KEY_C, T64_ID_C);
    add_node(T64_VEC_D, T64_KEY_D, T64_ID_D);
    add_node(T64_VEC_E, T64_KEY_E, T64_ID_E);
    add_edge(T64_ID_A, T64_ID_C, "t64.e.ac");
    add_edge(T64_ID_A, T64_ID_D, "t64.e.ad");
    add_edge(T64_ID_A, T64_ID_E, "t64.e.ae");
    add_edge(T64_ID_B, T64_ID_C, "t64.e.bc");
    add_edge(T64_ID_B, T64_ID_D, "t64.e.bd");
    add_edge(T64_ID_B, T64_ID_E, "t64.e.be");

    let (noctatriactc, nhoctatriactc, nagso, ec, nc) = gos_runtime::graph_topo_indices64();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(noctatriactc,     u64::MAX, "k23: NOCTATRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{2078}; 6\u{00b3}\u{2078}>>u64::MAX per-node; saturated)");
    assert_eq!(nhoctatriactc,    u64::MAX, "k23: NHOCTATRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{2077} >> u64::MAX; per-edge saturates)");
    assert_eq!(nagso,            u64::MAX, "k23: NAGSO=u64::MAX (6\u{00d7}72\u{00b3}\u{00b2} >> u64::MAX; per-edge saturates)");
}
