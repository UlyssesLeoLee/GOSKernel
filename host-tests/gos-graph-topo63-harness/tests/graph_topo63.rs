// gos-graph-topo63-harness — V3.74 NHEPTATRIACTC + NHHEPTATRIACTC + NAFSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices63()`:
//   Returns (nheptatriactc, nhheptatriactc, nafso, edge_count, node_count)
//   - nheptatriactc  = NHEPTATRIACTC(G) = Σ_v S(v)^37                   (exact u64; S-Heptatriacontic vertex sum)
//   - nhheptatriactc = NHHEPTATRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^36         (exact u64; S-Hexatriacontic edge-sum)
//   - nafso          = NAFSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^31       (exact u64; S-Hexahexacontyl Sombor, α=62)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTATRIACTC(G) = Σ_v S(v)^37
//     S-Heptatriacontic vertex sum; extends the S-power-vertex series:
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
//       NPENTTRIACTC=Σ S³⁵ (topo61), NHEXATRIACTC=Σ S³⁶ (topo62), NHEPTATRIACTC=Σ S³⁷ (topo63).
//     NHEPTATRIACTC = n·S^37 for S-regular.
//     Overflow: S^37 ≤ 16129^37 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^37 = s32 × s4 × s  (s32 = s16^2 perfect square; s4 = s2^2).
//
//   NHHEPTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^36
//     S-Hexatriacontic edge-sum; extends the S-power-edge series:
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
//       NHHEXATRIACTC=Σ(S+S)³⁵ (topo62), NHHEPTATRIACTC=Σ(S+S)³⁶ (topo63).
//     NHHEPTATRIACTC = |E|·(2S)^36 = 68719476736|E|·S^36 for S-regular.
//     Overflow per edge: (2×16129)^36 → saturating u128 accumulator.
//     Implementation: ss^36 = ss32 × ss4  (ss32 = ss16^2 perfect square; ss4 = ss2^2).
//
//   NAFSO(G) = Σ_{uv∈E} (S_u²+S_v²)^31
//     S-Hexahexacontyl Sombor: generalised Sombor SO^α with α=62 on S-variant.
//     3rd-pass double-letter "AF" (after NAESO α=60, topo62).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60), NAFSO(topo63,α=62).
//     NAFSO = |E|·(2S²)^31 = 2147483648|E|·S^62 for S-regular.
//     Overflow per edge: (2×16129²)^31 → saturating u128 accumulator.
//     Implementation: s2s^31 = s2s16 × s2s8 × s2s4 × s2s2 × s2s.
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
//  Graph     NHEPTATRIACTC(exact)             NHHEPTATRIACTC(exact)         NAFSO(exact)             edges  nodes
//  Empty                      0                              0                        0               0      0
//  1 node                     0                              0                        0               0      1
//  K₂                         2                 68_719_476_736               2_147_483_648               1      2
//  P₃           412_316_860_416              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       900_568_086_659_901_670           u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                 0                              0                        0               0      2
//  K_{2,3}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTATRIACTC:  1^37 + 1^37 = 2. ✓
//     NHHEPTATRIACTC: (1+1)^36 = 2^36 = 68_719_476_736. ✓
//     NAFSO:          (1²+1²)^31 = 2^31 = 2_147_483_648. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTATRIACTC:  3×2^37 = 3×137_438_953_472 = 412_316_860_416. ✓
//     NHHEPTATRIACTC: 2×(2+2)^36 = 2×4^36 = 2×2^72 → SATURATES (4^36=2^72>u64::MAX per-edge). ✓
//     NAFSO:          2×(4+4)^31 = 2×8^31 = 2×2^93 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTATRIACTC:  3×4^37 = 3×2^74 → SATURATES (2^74>u64::MAX per-node). ✓
//     NHHEPTATRIACTC: 3×(4+4)^36 = 3×8^36 = 3×2^108 → SATURATES. ✓
//     NAFSO:          3×(16+16)^31 = 3×32^31 = 3×2^155 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTATRIACTC:  5×4^37 → SATURATES. ✓
//     NHHEPTATRIACTC: 4×8^36 → SATURATES. ✓
//     NAFSO:          4×32^31 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTATRIACTC:  2×2^37 + 2×3^37.
//       3^32=1_853_020_188_851_841; 3^36=3^32×81=150_094_635_296_999_121; 3^37=3×3^36=450_283_905_890_997_363.
//       2×3^37=900_567_811_781_994_726. 2×2^37=274_877_906_944.
//       Total=900_567_811_781_994_726+274_877_906_944=900_568_086_659_901_670. ✓
//     NHHEPTATRIACTC: (2+3)^36+(3+3)^36+(3+2)^36 = 2×5^36+6^36
//       5^36>>u64::MAX per-edge → SATURATES. ✓
//     NAFSO:          2×13^31+18^31 — 13^17>u64::MAX so 13^31>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTATRIACTC:  4×9^37 → SATURATES → u64::MAX. ✓
//     NHHEPTATRIACTC: 6×18^36 → SATURATES. ✓
//     NAFSO:          6×162^31 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTATRIACTC:  5×6^37 → SATURATES → u64::MAX. ✓
//     NHHEPTATRIACTC: 6×12^36 → SATURATES. ✓
//     NAFSO:          6×72^31 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTATRIACTC  = n·S^37                                                    for S-regular ✓
//   NHHEPTATRIACTC = |E|·(2S)^36 = 68719476736|E|·S^36                         for S-regular ✓
//   NAFSO          = |E|·(2S²)^31 = 2147483648|E|·S^62                          for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 68_719_476_736, 2_147_483_648, 1, 2)
//  4.  Path P₃ = A-B-C                   → (412_316_860_416, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (900_567_812_056_872_670, u64::MAX, u64::MAX, 3, 4)
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

const T63_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_63");
const T63_EXEC:   ExecutorId = ExecutorId::from_ascii("t63.exec");

const T63_KEY_A: &str = "t63.alpha";
const T63_KEY_B: &str = "t63.beta";
const T63_KEY_C: &str = "t63.gamma";
const T63_KEY_D: &str = "t63.delta";
const T63_KEY_E: &str = "t63.epsilon";

const T63_ID_A: NodeId = derive_node_id(T63_PLUGIN, T63_KEY_A);
const T63_ID_B: NodeId = derive_node_id(T63_PLUGIN, T63_KEY_B);
const T63_ID_C: NodeId = derive_node_id(T63_PLUGIN, T63_KEY_C);
const T63_ID_D: NodeId = derive_node_id(T63_PLUGIN, T63_KEY_D);
const T63_ID_E: NodeId = derive_node_id(T63_PLUGIN, T63_KEY_E);

// L4=150 namespace for this harness.
const T63_VEC_A: VectorAddress = VectorAddress::new(150, 1, 1, 0);
const T63_VEC_B: VectorAddress = VectorAddress::new(150, 1, 2, 0);
const T63_VEC_C: VectorAddress = VectorAddress::new(150, 1, 3, 0);
const T63_VEC_D: VectorAddress = VectorAddress::new(150, 2, 1, 0);
const T63_VEC_E: VectorAddress = VectorAddress::new(150, 2, 2, 0);

const T63_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T63_PLUGIN,
    name:         "kl-graph-topo63-harness",
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
        executor_id:       T63_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T63_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T63_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(nheptatriactc,     0, "empty: NHEPTATRIACTC=0");
    assert_eq!(nhheptatriactc,    0, "empty: NHHEPTATRIACTC=0");
    assert_eq!(nafso,             0, "empty: NAFSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(nheptatriactc,     0, "single: NHEPTATRIACTC=0");
    assert_eq!(nhheptatriactc,    0, "single: NHHEPTATRIACTC=0");
    assert_eq!(nafso,             0, "single: NAFSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTATRIACTC:  1^37+1^37 = 2.
// NHHEPTATRIACTC: (1+1)^36 = 2^36 = 68_719_476_736.
// NAFSO:          (1²+1²)^31 = 2^31 = 2_147_483_648.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);
    add_edge(T63_ID_A, T63_ID_B, "t63.e.ab");

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                2,               "k2: node_count=2");
    assert_eq!(ec,                1,               "k2: edge_count=1");
    assert_eq!(nheptatriactc,     2,               "k2: NHEPTATRIACTC=2 (1\u{00b3}\u{2077}+1\u{00b3}\u{2077}=2)");
    assert_eq!(nhheptatriactc,    68_719_476_736,  "k2: NHHEPTATRIACTC=68_719_476_736 (2\u{00b3}\u{2076}=2^36)");
    assert_eq!(nafso,             2_147_483_648,   "k2: NAFSO=2_147_483_648 (2\u{00b3}\u{00b9}=2^31)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHEPTATRIACTC:  3×2^37 = 3×137_438_953_472 = 412_316_860_416.
// NHHEPTATRIACTC: 2×(2+2)^36 = 2×4^36 = 2×2^72 → SATURATES (4^36=2^72>u64::MAX per-edge).
// NAFSO:          2×(4+4)^31 = 2×8^31 = 2×2^93 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);
    add_node(T63_VEC_C, T63_KEY_C, T63_ID_C);
    add_edge(T63_ID_A, T63_ID_B, "t63.e.ab");
    add_edge(T63_ID_B, T63_ID_C, "t63.e.bc");

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                3,               "p3: node_count=3");
    assert_eq!(ec,                2,               "p3: edge_count=2");
    assert_eq!(nheptatriactc,     412_316_860_416,  "p3: NHEPTATRIACTC=412_316_860_416 (3\u{00d7}2\u{00b3}\u{2077})");
    assert_eq!(nhheptatriactc,    u64::MAX,         "p3: NHHEPTATRIACTC=u64::MAX (4\u{00b3}\u{2076}=2^72>u64::MAX per-edge; saturated)");
    assert_eq!(nafso,             u64::MAX,         "p3: NAFSO=u64::MAX (8\u{00b3}\u{00b9}=2^93>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHEPTATRIACTC:  3×4^37 = 3×2^74 → SATURATES (2^74>u64::MAX per-node).
// NHHEPTATRIACTC: 3×(4+4)^36 = 3×8^36 = 3×2^108 → SATURATES.
// NAFSO:          3×(16+16)^31 = 3×32^31 = 3×2^155 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);
    add_node(T63_VEC_C, T63_KEY_C, T63_ID_C);
    add_edge(T63_ID_A, T63_ID_B, "t63.e.ab");
    add_edge(T63_ID_B, T63_ID_A, "t63.e.ba");
    add_edge(T63_ID_B, T63_ID_C, "t63.e.bc");
    add_edge(T63_ID_C, T63_ID_B, "t63.e.cb");
    add_edge(T63_ID_A, T63_ID_C, "t63.e.ac");
    add_edge(T63_ID_C, T63_ID_A, "t63.e.ca");

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                3,        "k3: node_count=3");
    assert_eq!(ec,                3,        "k3: edge_count=3");
    assert_eq!(nheptatriactc,     u64::MAX, "k3: NHEPTATRIACTC=u64::MAX (3\u{00d7}4\u{00b3}\u{2077}>>u64::MAX; saturated)");
    assert_eq!(nhheptatriactc,    u64::MAX, "k3: NHHEPTATRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{2076}=3\u{00d7}2^108>>u64::MAX; saturated)");
    assert_eq!(nafso,             u64::MAX, "k3: NAFSO=u64::MAX (3\u{00d7}32\u{00b3}\u{00b9}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NHEPTATRIACTC:  5×4^37 → SATURATES.
// NHHEPTATRIACTC: 4×(4+4)^36 → SATURATES.
// NAFSO:          4×(16+16)^31 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);
    add_node(T63_VEC_C, T63_KEY_C, T63_ID_C);
    add_node(T63_VEC_D, T63_KEY_D, T63_ID_D);
    add_node(T63_VEC_E, T63_KEY_E, T63_ID_E);
    add_edge(T63_ID_A, T63_ID_B, "t63.e.ab");
    add_edge(T63_ID_A, T63_ID_C, "t63.e.ac");
    add_edge(T63_ID_A, T63_ID_D, "t63.e.ad");
    add_edge(T63_ID_A, T63_ID_E, "t63.e.ae");

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                5,        "k14: node_count=5");
    assert_eq!(ec,                4,        "k14: edge_count=4");
    assert_eq!(nheptatriactc,     u64::MAX, "k14: NHEPTATRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{2077}>u64::MAX; saturated)");
    assert_eq!(nhheptatriactc,    u64::MAX, "k14: NHHEPTATRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{2076}>>u64::MAX; saturated)");
    assert_eq!(nafso,             u64::MAX, "k14: NAFSO=u64::MAX (4\u{00d7}32\u{00b3}\u{00b9}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NHEPTATRIACTC:  2×2^37+2×3^37.
//   3^32=1_853_020_188_851_841; 3^36=3^32×81=150_094_635_296_999_121; 3^37=3×3^36=450_283_905_890_997_363.
//   2×3^37=900_567_811_781_994_726. 2×2^37=274_877_906_944.
//   Total=900_567_811_781_994_726+274_877_906_944=900_568_086_659_901_670.
// NHHEPTATRIACTC: (2+3)^36+(3+3)^36+(3+2)^36 = 2×5^36+6^36; 5^36>>u64::MAX per-edge → SATURATES.
// NAFSO:          2×13^31+18^31 — 13^17>u64::MAX so 13^31>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);
    add_node(T63_VEC_C, T63_KEY_C, T63_ID_C);
    add_node(T63_VEC_D, T63_KEY_D, T63_ID_D);
    add_edge(T63_ID_A, T63_ID_B, "t63.e.ab");
    add_edge(T63_ID_B, T63_ID_C, "t63.e.bc");
    add_edge(T63_ID_C, T63_ID_D, "t63.e.cd");

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                4,                           "p4: node_count=4");
    assert_eq!(ec,                3,                           "p4: edge_count=3");
    assert_eq!(nheptatriactc,     900_568_086_659_901_670,     "p4: NHEPTATRIACTC=900_568_086_659_901_670 (2\u{00d7}2\u{00b3}\u{2077}+2\u{00d7}3\u{00b3}\u{2077}; 3\u{00b3}\u{2077}=450_283_905_890_997_363)");
    assert_eq!(nhheptatriactc,    u64::MAX,                    "p4: NHHEPTATRIACTC=u64::MAX (5\u{00b3}\u{2076}>>u64::MAX per-edge; saturated)");
    assert_eq!(nafso,             u64::MAX,                    "p4: NAFSO=u64::MAX (13\u{00b3}\u{00b9}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHEPTATRIACTC:  4×9^37 → SATURATES → u64::MAX.
// NHHEPTATRIACTC: 6×18^36 → SATURATES → u64::MAX.
// NAFSO:          6×162^31 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);
    add_node(T63_VEC_C, T63_KEY_C, T63_ID_C);
    add_node(T63_VEC_D, T63_KEY_D, T63_ID_D);
    add_edge(T63_ID_A, T63_ID_B, "t63.e.ab");
    add_edge(T63_ID_B, T63_ID_A, "t63.e.ba");
    add_edge(T63_ID_A, T63_ID_C, "t63.e.ac");
    add_edge(T63_ID_C, T63_ID_A, "t63.e.ca");
    add_edge(T63_ID_A, T63_ID_D, "t63.e.ad");
    add_edge(T63_ID_D, T63_ID_A, "t63.e.da");
    add_edge(T63_ID_B, T63_ID_C, "t63.e.bc");
    add_edge(T63_ID_C, T63_ID_B, "t63.e.cb");
    add_edge(T63_ID_B, T63_ID_D, "t63.e.bd");
    add_edge(T63_ID_D, T63_ID_B, "t63.e.db");
    add_edge(T63_ID_C, T63_ID_D, "t63.e.cd");
    add_edge(T63_ID_D, T63_ID_C, "t63.e.dc");

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                4,        "k4: node_count=4");
    assert_eq!(ec,                6,        "k4: edge_count=6");
    assert_eq!(nheptatriactc,     u64::MAX, "k4: NHEPTATRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{2077} >> u64::MAX; saturated)");
    assert_eq!(nhheptatriactc,    u64::MAX, "k4: NHHEPTATRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{2076} >> u64::MAX; saturated)");
    assert_eq!(nafso,             u64::MAX, "k4: NAFSO=u64::MAX (6\u{00d7}162\u{00b3}\u{00b9} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHEPTATRIACTC=0; NHHEPTATRIACTC=0; NAFSO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                2, "two-iso: node_count=2");
    assert_eq!(ec,                0, "two-iso: edge_count=0");
    assert_eq!(nheptatriactc,     0, "two-iso: NHEPTATRIACTC=0");
    assert_eq!(nhheptatriactc,    0, "two-iso: NHHEPTATRIACTC=0");
    assert_eq!(nafso,             0, "two-iso: NAFSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NHEPTATRIACTC:  5×6^37 → SATURATES (6^37 >> u64::MAX per-node).
// NHHEPTATRIACTC: 6×12^36 → SATURATES (12^36>>u64::MAX per-edge).
// NAFSO:          6×72^31 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T63_VEC_A, T63_KEY_A, T63_ID_A);
    add_node(T63_VEC_B, T63_KEY_B, T63_ID_B);
    add_node(T63_VEC_C, T63_KEY_C, T63_ID_C);
    add_node(T63_VEC_D, T63_KEY_D, T63_ID_D);
    add_node(T63_VEC_E, T63_KEY_E, T63_ID_E);
    add_edge(T63_ID_A, T63_ID_C, "t63.e.ac");
    add_edge(T63_ID_A, T63_ID_D, "t63.e.ad");
    add_edge(T63_ID_A, T63_ID_E, "t63.e.ae");
    add_edge(T63_ID_B, T63_ID_C, "t63.e.bc");
    add_edge(T63_ID_B, T63_ID_D, "t63.e.bd");
    add_edge(T63_ID_B, T63_ID_E, "t63.e.be");

    let (nheptatriactc, nhheptatriactc, nafso, ec, nc) = gos_runtime::graph_topo_indices63();
    assert_eq!(nc,                5,        "k23: node_count=5");
    assert_eq!(ec,                6,        "k23: edge_count=6");
    assert_eq!(nheptatriactc,     u64::MAX, "k23: NHEPTATRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{2077}; 6\u{00b3}\u{2077}>>u64::MAX per-node; saturated)");
    assert_eq!(nhheptatriactc,    u64::MAX, "k23: NHHEPTATRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{2076} >> u64::MAX; per-edge saturates)");
    assert_eq!(nafso,             u64::MAX, "k23: NAFSO=u64::MAX (6\u{00d7}72\u{00b3}\u{00b9} >> u64::MAX; per-edge saturates)");
}
