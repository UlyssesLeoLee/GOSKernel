// gos-graph-topo47-harness — V3.58 NHENTC + NHHENTC + NPSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices47()`:
//   Returns (nhentc, nhhentc, npso, edge_count, node_count)
//   - nhentc  = NHENTC(G)  = Σ_v S(v)^21                  (exact u64; S-Heneicosic vertex sum)
//   - nhhentc = NHHENTC(G) = Σ_{uv∈E} (S_u+S_v)^20        (exact u64; S-Eicosic edge-sum)
//   - npso    = NPSO(G)    = Σ_{uv∈E} (S_u²+S_v²)^15      (exact u64; S-Triacontyl Sombor, α=30)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHENTC(G) = Σ_v S(v)^21
//     S-Heneicosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47).
//     NHENTC = n·S^21 for S-regular.
//     Overflow: S^21 ≤ 16129^21 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHHENTC(G) = Σ_{uv∈E} (S_u+S_v)^20
//     S-Eicosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47).
//     NHHENTC = |E|·(2S)^20 = 1_048_576|E|·S^20 for S-regular.
//     Overflow per edge: (2×16129)^20 → saturating u128 accumulator.
//
//   NPSO(G) = Σ_{uv∈E} (S_u²+S_v²)^15
//     S-Triacontyl Sombor: generalised Sombor SO^α with α=30 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30).
//     (P used instead of O because NOSO=α=8 already taken.)
//     NPSO = |E|·(2S²)^15 = 32768|E|·S^30 for S-regular.
//     Overflow per edge: (2×16129²)^15 → saturating u128 accumulator.
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
//  Graph     NHENTC(exact)              NHHENTC(exact)               NPSO(exact)              edges  nodes
//  Empty                  0                            0                         0               0      0
//  1 node                 0                            0                         0               0      1
//  K₂                     2                    1_048_576                    32_768               1      2
//  P₃               6_291_456            2_199_023_255_552          70_368_744_177_664            2      3
//  K₃        13_194_139_533_312  3_458_764_513_820_540_928          u64::MAX(sat.)               3      3
//  K_{1,4}   21_990_232_555_520  4_611_686_018_427_387_904          u64::MAX(sat.)               4      5
//  P₄            20_924_900_710      3_846_893_303_344_226   6_849_012_402_505_639_946            3      4
//  K₄          u64::MAX(sat.)          u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated             0                            0                         0               0      2
//  K_{2,3}  109_684_753_201_889_280    u64::MAX(sat.)               u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHENTC:  1^21 + 1^21 = 2. ✓
//     NHHENTC: (1+1)^20 = 2^20 = 1_048_576. ✓
//     NPSO:    (1²+1²)^15 = 2^15 = 32_768. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHENTC:  3×2^21 = 3×2_097_152 = 6_291_456. ✓
//     NHHENTC: 2×(2+2)^20 = 2×4^20 = 2×1_099_511_627_776 = 2_199_023_255_552. ✓
//       (4^19=274_877_906_944; 4^20=4×274_877_906_944=1_099_511_627_776)
//     NPSO:    2×(4+4)^15 = 2×8^15 = 2×35_184_372_088_832 = 70_368_744_177_664. ✓
//       (8^14=4_398_046_511_104; 8^15=8×4_398_046_511_104=35_184_372_088_832)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHENTC:  3×4^21 = 3×4_398_046_511_104 = 13_194_139_533_312. ✓
//       (4^20=1_099_511_627_776; 4^21=4×1_099_511_627_776=4_398_046_511_104)
//     NHHENTC: 3×(4+4)^20 = 3×8^20 = 3×1_152_921_504_606_846_976 = 3_458_764_513_820_540_928. ✓
//       (8^19=144_115_188_075_855_872; 8^20=8×144_115_188_075_855_872=1_152_921_504_606_846_976)
//     NPSO:    3×(16+16)^15 = 3×32^15 → SATURATES to u64::MAX. ✓
//       (32^7=34_359_738_368; 32^14≈1.18×10²¹>>u64::MAX; 32^15 saturates per-edge)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHENTC:  5×4^21 = 5×4_398_046_511_104 = 21_990_232_555_520. ✓
//     NHHENTC: 4×8^20 = 4×1_152_921_504_606_846_976 = 4_611_686_018_427_387_904. ✓
//     NPSO:    4×32^15 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHENTC:  2^21+3^21+3^21+2^21 = 2×2_097_152+2×10_460_353_203 = 20_924_900_710. ✓
//       (3^20=3_486_784_401; 3^21=3×3_486_784_401=10_460_353_203)
//     NHHENTC: 5^20+6^20+5^20
//       5^20: 5^16=152_587_890_625; 5^20=152_587_890_625×625=95_367_431_640_625
//       6^20: 6^16=2_821_109_907_456; 6^20=2_821_109_907_456×1_296=3_656_158_440_062_976
//       95_367_431_640_625+3_656_158_440_062_976+95_367_431_640_625 = 3_846_893_303_344_226. ✓
//     NPSO:    13^15+18^15+13^15
//       (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//       13^15: 13^14=3_937_376_385_699_289; 13^15=13×3_937_376_385_699_289=51_185_893_014_090_757
//       18^15: 18^14=374_813_367_582_081_024; 18^15=18×374_813_367_582_081_024=6_746_640_616_477_458_432
//       51_185_893_014_090_757+6_746_640_616_477_458_432+51_185_893_014_090_757 = 6_849_012_402_505_639_946. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHENTC:  4×9^21 → SATURATES to u64::MAX.
//       (9^20≈1.22×10¹⁹; 9^21=9×9^20≈1.09×10²⁰ >> u64::MAX per vertex) ✓
//     NHHENTC: 6×18^20 → SATURATES to u64::MAX.
//       (18^20>>u64::MAX per-edge) ✓
//     NPSO:    6×162^15 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHENTC:  5×6^21 = 5×21_936_950_640_377_856 = 109_684_753_201_889_280 (fits u64). ✓
//       (6^20=3_656_158_440_062_976; 6^21=6×3_656_158_440_062_976=21_936_950_640_377_856;
//        5×21_936_950_640_377_856=109_684_753_201_889_280 < u64::MAX≈1.84×10¹⁹)
//     NHHENTC: 6×12^20 → SATURATES to u64::MAX (12^20>>u64::MAX per-edge). ✓
//     NPSO:    6×72^15 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHENTC  = n·S^21                                for S-regular ✓
//   NHHENTC = |E|·(2S)^20 = 1048576|E|·S^20        for S-regular ✓
//   NPSO    = |E|·(2S²)^15 = 32768|E|·S^30         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 1_048_576, 32_768, 1, 2)
//  4.  Path P₃ = A-B-C                   → (6_291_456, 2_199_023_255_552, 70_368_744_177_664, 2, 3)
//  5.  Triangle K₃                       → (13_194_139_533_312, 3_458_764_513_820_540_928, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (21_990_232_555_520, 4_611_686_018_427_387_904, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (20_924_900_710, 3_846_893_303_344_226, 6_849_012_402_505_639_946, 3, 4)
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

const T47_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_47");
const T47_EXEC:   ExecutorId = ExecutorId::from_ascii("t47.exec");

const T47_KEY_A: &str = "t47.alpha";
const T47_KEY_B: &str = "t47.beta";
const T47_KEY_C: &str = "t47.gamma";
const T47_KEY_D: &str = "t47.delta";
const T47_KEY_E: &str = "t47.epsilon";

const T47_ID_A: NodeId = derive_node_id(T47_PLUGIN, T47_KEY_A);
const T47_ID_B: NodeId = derive_node_id(T47_PLUGIN, T47_KEY_B);
const T47_ID_C: NodeId = derive_node_id(T47_PLUGIN, T47_KEY_C);
const T47_ID_D: NodeId = derive_node_id(T47_PLUGIN, T47_KEY_D);
const T47_ID_E: NodeId = derive_node_id(T47_PLUGIN, T47_KEY_E);

// L4=134 namespace for this harness.
const T47_VEC_A: VectorAddress = VectorAddress::new(134, 1, 1, 0);
const T47_VEC_B: VectorAddress = VectorAddress::new(134, 1, 2, 0);
const T47_VEC_C: VectorAddress = VectorAddress::new(134, 1, 3, 0);
const T47_VEC_D: VectorAddress = VectorAddress::new(134, 2, 1, 0);
const T47_VEC_E: VectorAddress = VectorAddress::new(134, 2, 2, 0);

const T47_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T47_PLUGIN,
    name:         "kl-graph-topo47-harness",
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
        executor_id:       T47_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T47_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T47_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      0, "empty: node_count=0");
    assert_eq!(ec,      0, "empty: edge_count=0");
    assert_eq!(nhentc,  0, "empty: NHENTC=0");
    assert_eq!(nhhentc, 0, "empty: NHHENTC=0");
    assert_eq!(npso,    0, "empty: NPSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NHENTC: 0^21=0; NHHENTC: no edges; NPSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      1, "single: node_count=1");
    assert_eq!(ec,      0, "single: no edges");
    assert_eq!(nhentc,  0, "single: NHENTC=0 (S=0; 0^21=0)");
    assert_eq!(nhhentc, 0, "single: NHHENTC=0 (no edges)");
    assert_eq!(npso,    0, "single: NPSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NHENTC:  1^21+1^21 = 2.
// NHHENTC: (1+1)^20 = 2^20 = 1_048_576.
// NPSO:    (1²+1²)^15 = 2^15 = 32_768.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);
    add_edge(T47_ID_A, T47_ID_B, "t47.e.ab");

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      2,         "k2: node_count=2");
    assert_eq!(ec,      1,         "k2: edge_count=1");
    assert_eq!(nhentc,  2,         "k2: NHENTC=2 (1\u{00b2}\u{00b9}+1\u{00b2}\u{00b9}=2; S-uniform S=1)");
    assert_eq!(nhhentc, 1_048_576, "k2: NHHENTC=1_048_576 ((1+1)\u{00b2}\u{2070}=2\u{00b2}\u{2070}=1_048_576; S-uniform S=1)");
    assert_eq!(npso,    32_768,    "k2: NPSO=32_768 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{2075}=2\u{00b9}\u{2075}=32_768; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NHENTC:  3×2^21 = 3×2_097_152 = 6_291_456.
// NHHENTC: 2×(2+2)^20 = 2×4^20 = 2×1_099_511_627_776 = 2_199_023_255_552.
// NPSO:    2×(4+4)^15 = 2×8^15 = 2×35_184_372_088_832 = 70_368_744_177_664.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);
    add_node(T47_VEC_C, T47_KEY_C, T47_ID_C);
    add_edge(T47_ID_A, T47_ID_B, "t47.e.ab");
    add_edge(T47_ID_B, T47_ID_C, "t47.e.bc");

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      3,                   "p3: node_count=3");
    assert_eq!(ec,      2,                   "p3: edge_count=2");
    assert_eq!(nhentc,  6_291_456,           "p3: NHENTC=6_291_456 (3\u{00d7}2_097_152; 2\u{00b2}\u{00b9}=2_097_152; S-uniform S=2)");
    assert_eq!(nhhentc, 2_199_023_255_552,   "p3: NHHENTC=2_199_023_255_552 (2\u{00d7}1_099_511_627_776; (2+2)\u{00b2}\u{2070}=4\u{00b2}\u{2070}=1_099_511_627_776; S-uniform S=2)");
    assert_eq!(npso,    70_368_744_177_664,  "p3: NPSO=70_368_744_177_664 (2\u{00d7}35_184_372_088_832; (4+4)\u{00b9}\u{2075}=8\u{00b9}\u{2075}=35_184_372_088_832; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHENTC:  3×4^21 = 3×4_398_046_511_104 = 13_194_139_533_312.
// NHHENTC: 3×(4+4)^20 = 3×8^20 = 3×1_152_921_504_606_846_976 = 3_458_764_513_820_540_928.
// NPSO:    3×(16+16)^15 = 3×32^15 → SATURATES to u64::MAX.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);
    add_node(T47_VEC_C, T47_KEY_C, T47_ID_C);
    add_edge(T47_ID_A, T47_ID_B, "t47.e.ab");
    add_edge(T47_ID_B, T47_ID_A, "t47.e.ba");
    add_edge(T47_ID_B, T47_ID_C, "t47.e.bc");
    add_edge(T47_ID_C, T47_ID_B, "t47.e.cb");
    add_edge(T47_ID_A, T47_ID_C, "t47.e.ac");
    add_edge(T47_ID_C, T47_ID_A, "t47.e.ca");

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      3,                          "k3: node_count=3");
    assert_eq!(ec,      3,                          "k3: edge_count=3");
    assert_eq!(nhentc,  13_194_139_533_312,         "k3: NHENTC=13_194_139_533_312 (3\u{00d7}4_398_046_511_104; 4\u{00b2}\u{00b9}=4_398_046_511_104; S-uniform S=4)");
    assert_eq!(nhhentc, 3_458_764_513_820_540_928,  "k3: NHHENTC=3_458_764_513_820_540_928 (3\u{00d7}1_152_921_504_606_846_976; (4+4)\u{00b2}\u{2070}=8\u{00b2}\u{2070}=1_152_921_504_606_846_976; S-uniform S=4)");
    assert_eq!(npso,    u64::MAX,                   "k3: NPSO=u64::MAX (3\u{00d7}32\u{00b9}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NHENTC:  5×4^21 = 5×4_398_046_511_104 = 21_990_232_555_520.
// NHHENTC: 4×8^20 = 4×1_152_921_504_606_846_976 = 4_611_686_018_427_387_904.
// NPSO:    4×32^15 → SATURATES to u64::MAX.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);
    add_node(T47_VEC_C, T47_KEY_C, T47_ID_C);
    add_node(T47_VEC_D, T47_KEY_D, T47_ID_D);
    add_node(T47_VEC_E, T47_KEY_E, T47_ID_E);
    add_edge(T47_ID_A, T47_ID_B, "t47.e.ab");
    add_edge(T47_ID_A, T47_ID_C, "t47.e.ac");
    add_edge(T47_ID_A, T47_ID_D, "t47.e.ad");
    add_edge(T47_ID_A, T47_ID_E, "t47.e.ae");

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      5,                          "star: node_count=5");
    assert_eq!(ec,      4,                          "star: edge_count=4");
    assert_eq!(nhentc,  21_990_232_555_520,         "star: NHENTC=21_990_232_555_520 (5\u{00d7}4_398_046_511_104; same S as K\u{2083})");
    assert_eq!(nhhentc, 4_611_686_018_427_387_904,  "star: NHHENTC=4_611_686_018_427_387_904 (4\u{00d7}1_152_921_504_606_846_976; same per-edge as K\u{2083})");
    assert_eq!(npso,    u64::MAX,                   "star: NPSO=u64::MAX (4\u{00d7}32\u{00b9}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NHENTC:  2^21+3^21+3^21+2^21 = 2×2_097_152+2×10_460_353_203 = 20_924_900_710.
// NHHENTC: 5^20+6^20+5^20
//   = 95_367_431_640_625+3_656_158_440_062_976+95_367_431_640_625 = 3_846_893_303_344_226.
// NPSO:    13^15+18^15+13^15
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//   = 51_185_893_014_090_757+6_746_640_616_477_458_432+51_185_893_014_090_757 = 6_849_012_402_505_639_946.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);
    add_node(T47_VEC_C, T47_KEY_C, T47_ID_C);
    add_node(T47_VEC_D, T47_KEY_D, T47_ID_D);
    add_edge(T47_ID_A, T47_ID_B, "t47.e.ab");
    add_edge(T47_ID_B, T47_ID_C, "t47.e.bc");
    add_edge(T47_ID_C, T47_ID_D, "t47.e.cd");

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      4,                             "p4: node_count=4");
    assert_eq!(ec,      3,                             "p4: edge_count=3");
    assert_eq!(nhentc,  20_924_900_710,                "p4: NHENTC=20_924_900_710 (2\u{00d7}2_097_152+2\u{00d7}10_460_353_203; 2\u{00b2}\u{00b9}+3\u{00b2}\u{00b9}+3\u{00b2}\u{00b9}+2\u{00b2}\u{00b9})");
    assert_eq!(nhhentc, 3_846_893_303_344_226,         "p4: NHHENTC=3_846_893_303_344_226 (95_367_431_640_625+3_656_158_440_062_976+95_367_431_640_625; 5\u{00b2}\u{2070}+6\u{00b2}\u{2070}+5\u{00b2}\u{2070})");
    assert_eq!(npso,    6_849_012_402_505_639_946,     "p4: NPSO=6_849_012_402_505_639_946 (51_185_893_014_090_757+6_746_640_616_477_458_432+51_185_893_014_090_757; 13\u{00b9}\u{2075}+18\u{00b9}\u{2075}+13\u{00b9}\u{2075})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHENTC:  4×9^21 → SATURATES → u64::MAX.
// NHHENTC: 6×18^20 → SATURATES → u64::MAX (per-edge >> u64::MAX).
// NPSO:    6×162^15 → SATURATES → u64::MAX (per-edge already >> u64::MAX).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);
    add_node(T47_VEC_C, T47_KEY_C, T47_ID_C);
    add_node(T47_VEC_D, T47_KEY_D, T47_ID_D);
    add_edge(T47_ID_A, T47_ID_B, "t47.e.ab");
    add_edge(T47_ID_B, T47_ID_A, "t47.e.ba");
    add_edge(T47_ID_A, T47_ID_C, "t47.e.ac");
    add_edge(T47_ID_C, T47_ID_A, "t47.e.ca");
    add_edge(T47_ID_A, T47_ID_D, "t47.e.ad");
    add_edge(T47_ID_D, T47_ID_A, "t47.e.da");
    add_edge(T47_ID_B, T47_ID_C, "t47.e.bc");
    add_edge(T47_ID_C, T47_ID_B, "t47.e.cb");
    add_edge(T47_ID_B, T47_ID_D, "t47.e.bd");
    add_edge(T47_ID_D, T47_ID_B, "t47.e.db");
    add_edge(T47_ID_C, T47_ID_D, "t47.e.cd");
    add_edge(T47_ID_D, T47_ID_C, "t47.e.dc");

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      4,        "k4: node_count=4");
    assert_eq!(ec,      6,        "k4: edge_count=6");
    assert_eq!(nhentc,  u64::MAX, "k4: NHENTC=u64::MAX (4\u{00d7}9\u{00b2}\u{00b9} >> u64::MAX; saturated)");
    assert_eq!(nhhentc, u64::MAX, "k4: NHHENTC=u64::MAX (6\u{00d7}18\u{00b2}\u{2070} >> u64::MAX; saturated)");
    assert_eq!(npso,    u64::MAX, "k4: NPSO=u64::MAX (6\u{00d7}162\u{00b9}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHENTC=0; NHHENTC=0; NPSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      2, "two-iso: node_count=2");
    assert_eq!(ec,      0, "two-iso: edge_count=0");
    assert_eq!(nhentc,  0, "two-iso: NHENTC=0");
    assert_eq!(nhhentc, 0, "two-iso: NHHENTC=0");
    assert_eq!(npso,    0, "two-iso: NPSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NHENTC:  5×6^21 = 5×21_936_950_640_377_856 = 109_684_753_201_889_280 (exact, fits u64).
// NHHENTC: 6×12^20 → SATURATES (12^20>>u64::MAX per-edge).
// NPSO:    6×72^15 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T47_VEC_A, T47_KEY_A, T47_ID_A);
    add_node(T47_VEC_B, T47_KEY_B, T47_ID_B);
    add_node(T47_VEC_C, T47_KEY_C, T47_ID_C);
    add_node(T47_VEC_D, T47_KEY_D, T47_ID_D);
    add_node(T47_VEC_E, T47_KEY_E, T47_ID_E);
    add_edge(T47_ID_A, T47_ID_C, "t47.e.ac");
    add_edge(T47_ID_A, T47_ID_D, "t47.e.ad");
    add_edge(T47_ID_A, T47_ID_E, "t47.e.ae");
    add_edge(T47_ID_B, T47_ID_C, "t47.e.bc");
    add_edge(T47_ID_B, T47_ID_D, "t47.e.bd");
    add_edge(T47_ID_B, T47_ID_E, "t47.e.be");

    let (nhentc, nhhentc, npso, ec, nc) = gos_runtime::graph_topo_indices47();
    assert_eq!(nc,      5,        "k23: node_count=5");
    assert_eq!(ec,      6,        "k23: edge_count=6");
    assert_eq!(nhentc,  109_684_753_201_889_280, "k23: NHENTC=109_684_753_201_889_280 (5\u{00d7}6\u{00b2}\u{00b9}=5\u{00d7}21_936_950_640_377_856; fits u64)");
    assert_eq!(nhhentc, u64::MAX,               "k23: NHHENTC=u64::MAX (6\u{00d7}12\u{00b2}\u{2070} >> u64::MAX; per-edge saturates)");
    assert_eq!(npso,    u64::MAX,               "k23: NPSO=u64::MAX (6\u{00d7}72\u{00b9}\u{2075} >> u64::MAX; per-edge saturates)");
}
