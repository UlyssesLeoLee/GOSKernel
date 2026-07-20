// gos-graph-topo62-harness — V3.73 NHEXATRIACTC + NHHEXATRIACTC + NAESO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices62()`:
//   Returns (nhexatriactc, nhhexatriactc, naeso, edge_count, node_count)
//   - nhexatriactc  = NHEXATRIACTC(G) = Σ_v S(v)^36                   (exact u64; S-Hexatriacontic vertex sum)
//   - nhhexatriactc = NHHEXATRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^35         (exact u64; S-Pentatriacontic edge-sum)
//   - naeso         = NAESO(G)        = Σ_{uv∈E} (S_u²+S_v²)^30       (exact u64; S-Hexacontyl Sombor, α=60)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXATRIACTC(G) = Σ_v S(v)^36
//     S-Hexatriacontic vertex sum; extends the S-power-vertex series:
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
//       NPENTTRIACTC=Σ S³⁵ (topo61), NHEXATRIACTC=Σ S³⁶ (topo62).
//     NHEXATRIACTC = n·S^36 for S-regular.
//     Overflow: S^36 ≤ 16129^36 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^36 = s32 × s4  (s32 = s16^2 perfect square; s4 = s2^2).
//
//   NHHEXATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^35
//     S-Pentatriacontic edge-sum; extends the S-power-edge series:
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
//       NHHEXATRIACTC=Σ(S+S)³⁵ (topo62).
//     NHHEXATRIACTC = |E|·(2S)^35 = 34359738368|E|·S^35 for S-regular.
//     Overflow per edge: (2×16129)^35 → saturating u128 accumulator.
//     Implementation: ss^35 = ss32 × ss2 × ss  (ss32 = ss16^2 perfect square).
//
//   NAESO(G) = Σ_{uv∈E} (S_u²+S_v²)^30
//     S-Hexacontyl Sombor: generalised Sombor SO^α with α=60 on S-variant.
//     3rd-pass double-letter "AE" (after NADSO α=58, topo61).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60).
//     NAESO = |E|·(2S²)^30 = 1073741824|E|·S^60 for S-regular.
//     Overflow per edge: (2×16129²)^30 → saturating u128 accumulator.
//     Implementation: s2s^30 = s2s16 × s2s8 × s2s4 × s2s2.
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
//  Graph     NHEXATRIACTC(exact)              NHHEXATRIACTC(exact)          NAESO(exact)             edges  nodes
//  Empty                      0                              0                        0               0      0
//  1 node                     0                              0                        0               0      1
//  K₂                         2                 34_359_738_368               1_073_741_824               1      2
//  P₃           206_158_430_208              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       300_189_408_032_951_714           u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                 0                              0                        0               0      2
//  K_{2,3}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXATRIACTC:  1^36 + 1^36 = 2. ✓
//     NHHEXATRIACTC: (1+1)^35 = 2^35 = 34_359_738_368. ✓
//     NAESO:         (1²+1²)^30 = 2^30 = 1_073_741_824. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXATRIACTC:  3×2^36 = 3×68_719_476_736 = 206_158_430_208. ✓
//     NHHEXATRIACTC: 2×(2+2)^35 = 2×4^35 = 2×2^70 → SATURATES (4^35=2^70>u64::MAX per-edge). ✓
//     NAESO:         2×(4+4)^30 = 2×8^30 = 2×2^90 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXATRIACTC:  3×4^36 = 3×2^72 → SATURATES (2^72>u64::MAX per-node). ✓
//     NHHEXATRIACTC: 3×(4+4)^35 = 3×8^35 = 3×2^105 → SATURATES. ✓
//     NAESO:         3×(16+16)^30 = 3×32^30 = 3×2^150 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXATRIACTC:  5×4^36 → SATURATES. ✓
//     NHHEXATRIACTC: 4×8^35 → SATURATES. ✓
//     NAESO:         4×32^30 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXATRIACTC:  2×2^36 + 2×3^36 = 2^37 + 2×3^36.
//       3^32=1_853_020_188_851_841; 3^36=3^32×81=150_094_635_296_999_121; 2×3^36=300_189_270_593_998_242.
//       2^37=137_438_953_472. Total=300_189_270_593_998_242+137_438_953_472=300_189_408_032_951_714. ✓
//     NHHEXATRIACTC: (2+3)^35+(3+3)^35+(3+2)^35 = 2×5^35+6^35
//       5^35>>u64::MAX per-edge → SATURATES. ✓
//     NAESO:        (4+9)^30+(9+9)^30+(9+4)^30 → 13^30>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXATRIACTC:  4×9^36 → SATURATES → u64::MAX. ✓
//     NHHEXATRIACTC: 6×18^35 → SATURATES. ✓
//     NAESO:         6×162^30 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXATRIACTC:  5×6^36 → SATURATES → u64::MAX. ✓
//     NHHEXATRIACTC: 6×12^35 → SATURATES. ✓
//     NAESO:         6×72^30 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXATRIACTC  = n·S^36                                                    for S-regular ✓
//   NHHEXATRIACTC = |E|·(2S)^35 = 34359738368|E|·S^35                         for S-regular ✓
//   NAESO         = |E|·(2S²)^30 = 1073741824|E|·S^60                          for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 34_359_738_368, 1_073_741_824, 1, 2)
//  4.  Path P₃ = A-B-C                   → (206_158_430_208, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (300_189_408_032_951_714, u64::MAX, u64::MAX, 3, 4)
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

const T62_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_62");
const T62_EXEC:   ExecutorId = ExecutorId::from_ascii("t62.exec");

const T62_KEY_A: &str = "t62.alpha";
const T62_KEY_B: &str = "t62.beta";
const T62_KEY_C: &str = "t62.gamma";
const T62_KEY_D: &str = "t62.delta";
const T62_KEY_E: &str = "t62.epsilon";

const T62_ID_A: NodeId = derive_node_id(T62_PLUGIN, T62_KEY_A);
const T62_ID_B: NodeId = derive_node_id(T62_PLUGIN, T62_KEY_B);
const T62_ID_C: NodeId = derive_node_id(T62_PLUGIN, T62_KEY_C);
const T62_ID_D: NodeId = derive_node_id(T62_PLUGIN, T62_KEY_D);
const T62_ID_E: NodeId = derive_node_id(T62_PLUGIN, T62_KEY_E);

// L4=149 namespace for this harness.
const T62_VEC_A: VectorAddress = VectorAddress::new(149, 1, 1, 0);
const T62_VEC_B: VectorAddress = VectorAddress::new(149, 1, 2, 0);
const T62_VEC_C: VectorAddress = VectorAddress::new(149, 1, 3, 0);
const T62_VEC_D: VectorAddress = VectorAddress::new(149, 2, 1, 0);
const T62_VEC_E: VectorAddress = VectorAddress::new(149, 2, 2, 0);

const T62_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T62_PLUGIN,
    name:         "kl-graph-topo62-harness",
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
        executor_id:       T62_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T62_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T62_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nhexatriactc,     0, "empty: NHEXATRIACTC=0");
    assert_eq!(nhhexatriactc,    0, "empty: NHHEXATRIACTC=0");
    assert_eq!(naeso,            0, "empty: NAESO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nhexatriactc,     0, "single: NHEXATRIACTC=0");
    assert_eq!(nhhexatriactc,    0, "single: NHHEXATRIACTC=0");
    assert_eq!(naeso,            0, "single: NAESO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXATRIACTC:  1^36+1^36 = 2.
// NHHEXATRIACTC: (1+1)^35 = 2^35 = 34_359_738_368.
// NAESO:         (1²+1²)^30 = 2^30 = 1_073_741_824.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);
    add_edge(T62_ID_A, T62_ID_B, "t62.e.ab");

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               2,               "k2: node_count=2");
    assert_eq!(ec,               1,               "k2: edge_count=1");
    assert_eq!(nhexatriactc,     2,               "k2: NHEXATRIACTC=2 (1\u{00b3}\u{2076}+1\u{00b3}\u{2076}=2)");
    assert_eq!(nhhexatriactc,    34_359_738_368,  "k2: NHHEXATRIACTC=34_359_738_368 (2\u{00b3}\u{2075}=2^35)");
    assert_eq!(naeso,            1_073_741_824,   "k2: NAESO=1_073_741_824 (2\u{00b3}\u{2070}=2^30)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHEXATRIACTC:  3×2^36 = 3×68_719_476_736 = 206_158_430_208.
// NHHEXATRIACTC: 2×(2+2)^35 = 2×4^35 = 2×2^70 → SATURATES (4^35=2^70>u64::MAX per-edge).
// NAESO:         2×(4+4)^30 = 2×8^30 = 2×2^90 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);
    add_node(T62_VEC_C, T62_KEY_C, T62_ID_C);
    add_edge(T62_ID_A, T62_ID_B, "t62.e.ab");
    add_edge(T62_ID_B, T62_ID_C, "t62.e.bc");

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               3,               "p3: node_count=3");
    assert_eq!(ec,               2,               "p3: edge_count=2");
    assert_eq!(nhexatriactc,     206_158_430_208,  "p3: NHEXATRIACTC=206_158_430_208 (3\u{00d7}2\u{00b3}\u{2076})");
    assert_eq!(nhhexatriactc,    u64::MAX,         "p3: NHHEXATRIACTC=u64::MAX (4\u{00b3}\u{2075}=2^70>u64::MAX per-edge; saturated)");
    assert_eq!(naeso,            u64::MAX,         "p3: NAESO=u64::MAX (8\u{00b3}\u{2070}=2^90>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHEXATRIACTC:  3×4^36 = 3×2^72 → SATURATES (2^72>u64::MAX per-node).
// NHHEXATRIACTC: 3×(4+4)^35 = 3×8^35 = 3×2^105 → SATURATES.
// NAESO:         3×(16+16)^30 = 3×32^30 = 3×2^150 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);
    add_node(T62_VEC_C, T62_KEY_C, T62_ID_C);
    add_edge(T62_ID_A, T62_ID_B, "t62.e.ab");
    add_edge(T62_ID_B, T62_ID_A, "t62.e.ba");
    add_edge(T62_ID_B, T62_ID_C, "t62.e.bc");
    add_edge(T62_ID_C, T62_ID_B, "t62.e.cb");
    add_edge(T62_ID_A, T62_ID_C, "t62.e.ac");
    add_edge(T62_ID_C, T62_ID_A, "t62.e.ca");

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nhexatriactc,     u64::MAX, "k3: NHEXATRIACTC=u64::MAX (3\u{00d7}4\u{00b3}\u{2076}>>u64::MAX; saturated)");
    assert_eq!(nhhexatriactc,    u64::MAX, "k3: NHHEXATRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{2075}=3\u{00d7}2^105>>u64::MAX; saturated)");
    assert_eq!(naeso,            u64::MAX, "k3: NAESO=u64::MAX (3\u{00d7}32\u{00b3}\u{2070}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NHEXATRIACTC:  5×4^36 → SATURATES.
// NHHEXATRIACTC: 4×(4+4)^35 → SATURATES.
// NAESO:         4×(16+16)^30 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);
    add_node(T62_VEC_C, T62_KEY_C, T62_ID_C);
    add_node(T62_VEC_D, T62_KEY_D, T62_ID_D);
    add_node(T62_VEC_E, T62_KEY_E, T62_ID_E);
    add_edge(T62_ID_A, T62_ID_B, "t62.e.ab");
    add_edge(T62_ID_A, T62_ID_C, "t62.e.ac");
    add_edge(T62_ID_A, T62_ID_D, "t62.e.ad");
    add_edge(T62_ID_A, T62_ID_E, "t62.e.ae");

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nhexatriactc,     u64::MAX, "k14: NHEXATRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{2076}>u64::MAX; saturated)");
    assert_eq!(nhhexatriactc,    u64::MAX, "k14: NHHEXATRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{2075}>>u64::MAX; saturated)");
    assert_eq!(naeso,            u64::MAX, "k14: NAESO=u64::MAX (4\u{00d7}32\u{00b3}\u{2070}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NHEXATRIACTC:  2×2^36+2×3^36 = 2^37 + 2×3^36.
//   3^32=1_853_020_188_851_841; 3^36=3^32×81=150_094_635_296_999_121; 2×3^36=300_189_270_593_998_242.
//   2^37=137_438_953_472. Total=300_189_270_593_998_242+137_438_953_472=300_189_408_032_951_714.
// NHHEXATRIACTC: (2+3)^35+(3+3)^35+(3+2)^35 = 2×5^35+6^35; 5^35>>u64::MAX per-edge → SATURATES.
// NAESO:         13^30+18^30+13^30 — 13^30>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);
    add_node(T62_VEC_C, T62_KEY_C, T62_ID_C);
    add_node(T62_VEC_D, T62_KEY_D, T62_ID_D);
    add_edge(T62_ID_A, T62_ID_B, "t62.e.ab");
    add_edge(T62_ID_B, T62_ID_C, "t62.e.bc");
    add_edge(T62_ID_C, T62_ID_D, "t62.e.cd");

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               4,                           "p4: node_count=4");
    assert_eq!(ec,               3,                           "p4: edge_count=3");
    assert_eq!(nhexatriactc,     300_189_408_032_951_714,     "p4: NHEXATRIACTC=300_189_408_032_951_714 (2\u{00d7}2\u{00b3}\u{2076}+2\u{00d7}3\u{00b3}\u{2076}; 3\u{00b3}\u{2076}=150_094_635_296_999_121)");
    assert_eq!(nhhexatriactc,    u64::MAX,                    "p4: NHHEXATRIACTC=u64::MAX (5\u{00b3}\u{2075}>>u64::MAX per-edge; saturated)");
    assert_eq!(naeso,            u64::MAX,                    "p4: NAESO=u64::MAX (13\u{00b3}\u{2070}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHEXATRIACTC:  4×9^36 → SATURATES → u64::MAX.
// NHHEXATRIACTC: 6×18^35 → SATURATES → u64::MAX.
// NAESO:         6×162^30 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);
    add_node(T62_VEC_C, T62_KEY_C, T62_ID_C);
    add_node(T62_VEC_D, T62_KEY_D, T62_ID_D);
    add_edge(T62_ID_A, T62_ID_B, "t62.e.ab");
    add_edge(T62_ID_B, T62_ID_A, "t62.e.ba");
    add_edge(T62_ID_A, T62_ID_C, "t62.e.ac");
    add_edge(T62_ID_C, T62_ID_A, "t62.e.ca");
    add_edge(T62_ID_A, T62_ID_D, "t62.e.ad");
    add_edge(T62_ID_D, T62_ID_A, "t62.e.da");
    add_edge(T62_ID_B, T62_ID_C, "t62.e.bc");
    add_edge(T62_ID_C, T62_ID_B, "t62.e.cb");
    add_edge(T62_ID_B, T62_ID_D, "t62.e.bd");
    add_edge(T62_ID_D, T62_ID_B, "t62.e.db");
    add_edge(T62_ID_C, T62_ID_D, "t62.e.cd");
    add_edge(T62_ID_D, T62_ID_C, "t62.e.dc");

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nhexatriactc,     u64::MAX, "k4: NHEXATRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{2076} >> u64::MAX; saturated)");
    assert_eq!(nhhexatriactc,    u64::MAX, "k4: NHHEXATRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{2075} >> u64::MAX; saturated)");
    assert_eq!(naeso,            u64::MAX, "k4: NAESO=u64::MAX (6\u{00d7}162\u{00b3}\u{2070} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHEXATRIACTC=0; NHHEXATRIACTC=0; NAESO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               2, "two-iso: node_count=2");
    assert_eq!(ec,               0, "two-iso: edge_count=0");
    assert_eq!(nhexatriactc,     0, "two-iso: NHEXATRIACTC=0");
    assert_eq!(nhhexatriactc,    0, "two-iso: NHHEXATRIACTC=0");
    assert_eq!(naeso,            0, "two-iso: NAESO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NHEXATRIACTC:  5×6^36 → SATURATES (6^36 >> u64::MAX per-node).
// NHHEXATRIACTC: 6×12^35 → SATURATES (12^35>>u64::MAX per-edge).
// NAESO:         6×72^30 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T62_VEC_A, T62_KEY_A, T62_ID_A);
    add_node(T62_VEC_B, T62_KEY_B, T62_ID_B);
    add_node(T62_VEC_C, T62_KEY_C, T62_ID_C);
    add_node(T62_VEC_D, T62_KEY_D, T62_ID_D);
    add_node(T62_VEC_E, T62_KEY_E, T62_ID_E);
    add_edge(T62_ID_A, T62_ID_C, "t62.e.ac");
    add_edge(T62_ID_A, T62_ID_D, "t62.e.ad");
    add_edge(T62_ID_A, T62_ID_E, "t62.e.ae");
    add_edge(T62_ID_B, T62_ID_C, "t62.e.bc");
    add_edge(T62_ID_B, T62_ID_D, "t62.e.bd");
    add_edge(T62_ID_B, T62_ID_E, "t62.e.be");

    let (nhexatriactc, nhhexatriactc, naeso, ec, nc) = gos_runtime::graph_topo_indices62();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nhexatriactc,     u64::MAX, "k23: NHEXATRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{2076}; 6\u{00b3}\u{2076}>>u64::MAX per-node; saturated)");
    assert_eq!(nhhexatriactc,    u64::MAX, "k23: NHHEXATRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{2075} >> u64::MAX; per-edge saturates)");
    assert_eq!(naeso,            u64::MAX, "k23: NAESO=u64::MAX (6\u{00d7}72\u{00b3}\u{2070} >> u64::MAX; per-edge saturates)");
}
