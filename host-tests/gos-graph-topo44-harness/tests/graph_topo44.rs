// gos-graph-topo44-harness — V3.55 NOCTC + NHOCTC + NLSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices44()`:
//   Returns (noctc, nhoctc, nlso, edge_count, node_count)
//   - noctc  = NOCTC(G)  = Σ_v S(v)^18                   (exact u64; S-Octadecic vertex sum)
//   - nhoctc = NHOCTC(G) = Σ_{uv∈E} (S_u+S_v)^17         (exact u64; S-Heptadecic edge-sum)
//   - nlso   = NLSO(G)   = Σ_{uv∈E} (S_u²+S_v²)^12       (exact u64; S-Tetracosic Sombor, α=24)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTC(G) = Σ_v S(v)^18
//     S-Octadecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44).
//     NOCTC = n·S^18 for S-regular.
//     Overflow: S^18 ≤ 16129^18 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHOCTC(G) = Σ_{uv∈E} (S_u+S_v)^17
//     S-Heptadecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44).
//     NHOCTC = |E|·(2S)^17 = 131072|E|·S^17 for S-regular.
//     Overflow per edge: (2×16129)^17 → saturating u128 accumulator.
//
//   NLSO(G) = Σ_{uv∈E} (S_u²+S_v²)^12
//     S-Tetracosic Sombor: generalised Sombor SO^α with α=24 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24) — exact, no isqrt.
//     NLSO = |E|·(2S²)^12 = 4096|E|·S^24 for S-regular.
//     Overflow per edge: (2×16129²)^12 → saturating u128 accumulator;
//     K₄ (S=9) and K_{2,3} (S=6) saturate → u64::MAX.
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
//  Graph     NOCTC(exact)               NHOCTC(exact)                 NLSO(exact)             edges  nodes
//  Empty                  0                           0                          0              0      0
//  1 node                 0                           0                          0              0      1
//  K₂                     2                     131_072                      4_096              1      2
//  P₃                786_432              34_359_738_368             137_438_953_472             2      3
//  K₃          206_158_430_208       6_755_399_441_055_744   3_458_764_513_820_540_928            3      3
//  K_{1,4}     343_597_383_680       9_007_199_254_740_992   4_611_686_018_427_387_904            4      5
//  P₄             775_365_266          18_452_538_350_986       1_203_427_551_671_138             3      4
//  K₄    600_378_541_187_996_484        u64::MAX(sat.)               u64::MAX(sat.)             6      4
//  2 isolated             0                           0                          0              0      2
//  K_{2,3}     507_799_783_342_080  13_311_666_640_442_621_952        u64::MAX(sat.)            6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTC:  1^18 + 1^18 = 2. ✓
//     NHOCTC: (1+1)^17 = 2^17 = 131_072. ✓
//     NLSO:   (1²+1²)^12 = 2^12 = 4_096. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTC:  3×2^18 = 3×262_144 = 786_432. ✓
//       (2^17=131_072; 2^18=2×131_072=262_144)
//     NHOCTC: 2×(2+2)^17 = 2×4^17 = 2×17_179_869_184 = 34_359_738_368. ✓
//       (4^16=4_294_967_296; 4^17=4×4_294_967_296=17_179_869_184)
//     NLSO:   2×(4+4)^12 = 2×8^12 = 2×68_719_476_736 = 137_438_953_472. ✓
//       (8^11=8_589_934_592; 8^12=8×8_589_934_592=68_719_476_736)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTC:  3×4^18 = 3×68_719_476_736 = 206_158_430_208. ✓
//       (4^17=17_179_869_184; 4^18=4×17_179_869_184=68_719_476_736)
//     NHOCTC: 3×(4+4)^17 = 3×8^17 = 3×2_251_799_813_685_248 = 6_755_399_441_055_744. ✓
//       (8^16=281_474_976_710_656; 8^17=8×281_474_976_710_656=2_251_799_813_685_248)
//     NLSO:   3×(16+16)^12 = 3×32^12 = 3×1_152_921_504_606_846_976 = 3_458_764_513_820_540_928. ✓
//       (32^11=36_028_797_018_963_968; 32^12=32×36_028_797_018_963_968=1_152_921_504_606_846_976)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTC:  5×4^18 = 5×68_719_476_736 = 343_597_383_680. ✓
//     NHOCTC: 4×8^17 = 4×2_251_799_813_685_248 = 9_007_199_254_740_992. ✓
//     NLSO:   4×32^12 = 4×1_152_921_504_606_846_976 = 4_611_686_018_427_387_904. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHOCTC and NLSO; NOCTC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTC:  2^18+3^18+3^18+2^18 = 262_144+387_420_489+387_420_489+262_144 = 775_365_266. ✓
//       (3^17=129_140_163; 3^18=3×129_140_163=387_420_489)
//     NHOCTC: 5^17+6^17+5^17
//       5^17: 5^16=152_587_890_625; 5^17=5×152_587_890_625=762_939_453_125
//       6^17: 6^16=2_821_109_907_456; 6^17=6×2_821_109_907_456=16_926_659_444_736
//       762_939_453_125+16_926_659_444_736+762_939_453_125 = 18_452_538_350_986. ✓
//     NLSO:   13^12+18^12+13^12
//       (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//       13^12: 13^11=1_792_160_394_037; 13^12=13×1_792_160_394_037=23_298_085_122_481
//       18^12: 18^11=64_268_410_079_232; 18^12=18×64_268_410_079_232=1_156_831_381_426_176
//       23_298_085_122_481+1_156_831_381_426_176+23_298_085_122_481 = 1_203_427_551_671_138. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTC:  4×9^18 = 4×150_094_635_296_999_121 = 600_378_541_187_996_484 (fits u64). ✓
//       (9^17=16_677_181_699_666_569; 9^18=9×16_677_181_699_666_569=150_094_635_296_999_121)
//     NHOCTC: 6×18^17 → SATURATES to u64::MAX.
//       (18^16≈1.214×10^20 > u64::MAX per-edge; 18^17 >> u64::MAX) ✓
//     NLSO:   6×162^12 → SATURATES to u64::MAX (per-edge >> u64::MAX).
//       (162^11 >> u64::MAX per-edge in topo43; 162^12 >> u64::MAX per-edge) ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTC:  5×6^18 = 5×101_559_956_668_416 = 507_799_783_342_080 (fits u64). ✓
//       (6^17=16_926_659_444_736; 6^18=6×16_926_659_444_736=101_559_956_668_416)
//     NHOCTC: 6×12^17 = 6×2_218_611_106_740_436_992 = 13_311_666_640_442_621_952 (fits u64). ✓
//       (12^16=184_884_258_895_036_416; 12^17=12×184_884_258_895_036_416=2_218_611_106_740_436_992)
//       (13_311_666_640_442_621_952 < u64::MAX=18_446_744_073_709_551_615) ✓
//     NLSO:   6×72^12 → SATURATES to u64::MAX.
//       (72^11≈3.743×10^20 > u64::MAX per-edge in topo43; 72^12 >> u64::MAX per-edge) ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTC  = n·S^18                               for S-regular ✓
//   NHOCTC = |E|·(2S)^17 = 131072|E|·S^17        for S-regular ✓
//   NLSO   = |E|·(2S²)^12 = 4096|E|·S^24         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 131_072, 4_096, 1, 2)
//  4.  Path P₃ = A-B-C                   → (786_432, 34_359_738_368, 137_438_953_472, 2, 3)
//  5.  Triangle K₃                       → (206_158_430_208, 6_755_399_441_055_744, 3_458_764_513_820_540_928, 3, 3)
//  6.  Star K_{1,4}                      → (343_597_383_680, 9_007_199_254_740_992, 4_611_686_018_427_387_904, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (775_365_266, 18_452_538_350_986, 1_203_427_551_671_138, 3, 4)
//  8.  Complete K₄                       → (600_378_541_187_996_484, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (507_799_783_342_080, 13_311_666_640_442_621_952, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T44_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_44");
const T44_EXEC:   ExecutorId = ExecutorId::from_ascii("t44.exec");

const T44_KEY_A: &str = "t44.alpha";
const T44_KEY_B: &str = "t44.beta";
const T44_KEY_C: &str = "t44.gamma";
const T44_KEY_D: &str = "t44.delta";
const T44_KEY_E: &str = "t44.epsilon";

const T44_ID_A: NodeId = derive_node_id(T44_PLUGIN, T44_KEY_A);
const T44_ID_B: NodeId = derive_node_id(T44_PLUGIN, T44_KEY_B);
const T44_ID_C: NodeId = derive_node_id(T44_PLUGIN, T44_KEY_C);
const T44_ID_D: NodeId = derive_node_id(T44_PLUGIN, T44_KEY_D);
const T44_ID_E: NodeId = derive_node_id(T44_PLUGIN, T44_KEY_E);

// L4=131 namespace for this harness.
const T44_VEC_A: VectorAddress = VectorAddress::new(131, 1, 1, 0);
const T44_VEC_B: VectorAddress = VectorAddress::new(131, 1, 2, 0);
const T44_VEC_C: VectorAddress = VectorAddress::new(131, 1, 3, 0);
const T44_VEC_D: VectorAddress = VectorAddress::new(131, 2, 1, 0);
const T44_VEC_E: VectorAddress = VectorAddress::new(131, 2, 2, 0);

const T44_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T44_PLUGIN,
    name:         "kl-graph-topo44-harness",
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
        executor_id:       T44_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T44_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T44_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     0, "empty: node_count=0");
    assert_eq!(ec,     0, "empty: edge_count=0");
    assert_eq!(noctc,  0, "empty: NOCTC=0");
    assert_eq!(nhoctc, 0, "empty: NHOCTC=0");
    assert_eq!(nlso,   0, "empty: NLSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NOCTC: 0^18=0; NHOCTC: no edges; NLSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     1, "single: node_count=1");
    assert_eq!(ec,     0, "single: no edges");
    assert_eq!(noctc,  0, "single: NOCTC=0 (S=0; 0^18=0)");
    assert_eq!(nhoctc, 0, "single: NHOCTC=0 (no edges)");
    assert_eq!(nlso,   0, "single: NLSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NOCTC:  1^18+1^18 = 2.
// NHOCTC: (1+1)^17 = 2^17 = 131_072.
// NLSO:   (1²+1²)^12 = 2^12 = 4_096.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);
    add_edge(T44_ID_A, T44_ID_B, "t44.e.ab");

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     2,       "k2: node_count=2");
    assert_eq!(ec,     1,       "k2: edge_count=1");
    assert_eq!(noctc,  2,       "k2: NOCTC=2 (1\u{00b9}\u{2078}+1\u{00b9}\u{2078}=2; S-uniform S=1)");
    assert_eq!(nhoctc, 131_072, "k2: NHOCTC=131_072 ((1+1)\u{00b9}\u{2077}=2\u{00b9}\u{2077}=131_072; S-uniform S=1)");
    assert_eq!(nlso,   4_096,   "k2: NLSO=4_096 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{00b2}=2\u{00b9}\u{00b2}=4_096; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NOCTC:  3×2^18 = 3×262_144 = 786_432.
// NHOCTC: 2×(2+2)^17 = 2×4^17 = 2×17_179_869_184 = 34_359_738_368.
// NLSO:   2×(4+4)^12 = 2×8^12 = 2×68_719_476_736 = 137_438_953_472.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);
    add_node(T44_VEC_C, T44_KEY_C, T44_ID_C);
    add_edge(T44_ID_A, T44_ID_B, "t44.e.ab");
    add_edge(T44_ID_B, T44_ID_C, "t44.e.bc");

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     3,               "p3: node_count=3");
    assert_eq!(ec,     2,               "p3: edge_count=2");
    assert_eq!(noctc,  786_432,         "p3: NOCTC=786_432 (3\u{00d7}262_144; 2\u{00b9}\u{2078}=262_144; S-uniform S=2)");
    assert_eq!(nhoctc, 34_359_738_368,  "p3: NHOCTC=34_359_738_368 (2\u{00d7}17_179_869_184; (2+2)\u{00b9}\u{2077}=4\u{00b9}\u{2077}=17_179_869_184; S-uniform S=2)");
    assert_eq!(nlso,   137_438_953_472, "p3: NLSO=137_438_953_472 (2\u{00d7}68_719_476_736; (4+4)\u{00b9}\u{00b2}=8\u{00b9}\u{00b2}=68_719_476_736; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NOCTC:  3×4^18 = 3×68_719_476_736 = 206_158_430_208.
// NHOCTC: 3×(4+4)^17 = 3×8^17 = 3×2_251_799_813_685_248 = 6_755_399_441_055_744.
// NLSO:   3×(16+16)^12 = 3×32^12 = 3×1_152_921_504_606_846_976 = 3_458_764_513_820_540_928.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);
    add_node(T44_VEC_C, T44_KEY_C, T44_ID_C);
    add_edge(T44_ID_A, T44_ID_B, "t44.e.ab");
    add_edge(T44_ID_B, T44_ID_A, "t44.e.ba");
    add_edge(T44_ID_B, T44_ID_C, "t44.e.bc");
    add_edge(T44_ID_C, T44_ID_B, "t44.e.cb");
    add_edge(T44_ID_A, T44_ID_C, "t44.e.ac");
    add_edge(T44_ID_C, T44_ID_A, "t44.e.ca");

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     3,                          "k3: node_count=3");
    assert_eq!(ec,     3,                          "k3: edge_count=3");
    assert_eq!(noctc,  206_158_430_208,            "k3: NOCTC=206_158_430_208 (3\u{00d7}68_719_476_736; 4\u{00b9}\u{2078}=68_719_476_736; S-uniform S=4)");
    assert_eq!(nhoctc, 6_755_399_441_055_744,      "k3: NHOCTC=6_755_399_441_055_744 (3\u{00d7}2_251_799_813_685_248; (4+4)\u{00b9}\u{2077}=8\u{00b9}\u{2077}=2_251_799_813_685_248; S-uniform S=4)");
    assert_eq!(nlso,   3_458_764_513_820_540_928,  "k3: NLSO=3_458_764_513_820_540_928 (3\u{00d7}1_152_921_504_606_846_976; (16+16)\u{00b9}\u{00b2}=32\u{00b9}\u{00b2}=1_152_921_504_606_846_976; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHOCTC and NLSO as K₃; NOCTC and totals differ by node/edge count.
// NOCTC:  5×4^18 = 5×68_719_476_736 = 343_597_383_680.
// NHOCTC: 4×8^17 = 4×2_251_799_813_685_248 = 9_007_199_254_740_992.
// NLSO:   4×32^12 = 4×1_152_921_504_606_846_976 = 4_611_686_018_427_387_904.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);
    add_node(T44_VEC_C, T44_KEY_C, T44_ID_C);
    add_node(T44_VEC_D, T44_KEY_D, T44_ID_D);
    add_node(T44_VEC_E, T44_KEY_E, T44_ID_E);
    add_edge(T44_ID_A, T44_ID_B, "t44.e.ab");
    add_edge(T44_ID_A, T44_ID_C, "t44.e.ac");
    add_edge(T44_ID_A, T44_ID_D, "t44.e.ad");
    add_edge(T44_ID_A, T44_ID_E, "t44.e.ae");

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     5,                          "star: node_count=5");
    assert_eq!(ec,     4,                          "star: edge_count=4");
    assert_eq!(noctc,  343_597_383_680,            "star: NOCTC=343_597_383_680 (5\u{00d7}68_719_476_736; same S as K\u{2083})");
    assert_eq!(nhoctc, 9_007_199_254_740_992,      "star: NHOCTC=9_007_199_254_740_992 (4\u{00d7}2_251_799_813_685_248; same per-edge as K\u{2083})");
    assert_eq!(nlso,   4_611_686_018_427_387_904,  "star: NLSO=4_611_686_018_427_387_904 (4\u{00d7}1_152_921_504_606_846_976; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NOCTC:  2^18+3^18+3^18+2^18 = 262_144+387_420_489+387_420_489+262_144 = 775_365_266.
// NHOCTC: 5^17+6^17+5^17
//   = 762_939_453_125+16_926_659_444_736+762_939_453_125 = 18_452_538_350_986.
// NLSO:   13^12+18^12+13^12
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//   = 23_298_085_122_481+1_156_831_381_426_176+23_298_085_122_481 = 1_203_427_551_671_138.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);
    add_node(T44_VEC_C, T44_KEY_C, T44_ID_C);
    add_node(T44_VEC_D, T44_KEY_D, T44_ID_D);
    add_edge(T44_ID_A, T44_ID_B, "t44.e.ab");
    add_edge(T44_ID_B, T44_ID_C, "t44.e.bc");
    add_edge(T44_ID_C, T44_ID_D, "t44.e.cd");

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     4,                    "p4: node_count=4");
    assert_eq!(ec,     3,                    "p4: edge_count=3");
    assert_eq!(noctc,  775_365_266,          "p4: NOCTC=775_365_266 (262_144+387_420_489+387_420_489+262_144; 2\u{00b9}\u{2078}+3\u{00b9}\u{2078}+3\u{00b9}\u{2078}+2\u{00b9}\u{2078})");
    assert_eq!(nhoctc, 18_452_538_350_986,   "p4: NHOCTC=18_452_538_350_986 (762_939_453_125+16_926_659_444_736+762_939_453_125; 5\u{00b9}\u{2077}+6\u{00b9}\u{2077}+5\u{00b9}\u{2077})");
    assert_eq!(nlso,   1_203_427_551_671_138,"p4: NLSO=1_203_427_551_671_138 (23_298_085_122_481+1_156_831_381_426_176+23_298_085_122_481; 13\u{00b9}\u{00b2}+18\u{00b9}\u{00b2}+13\u{00b9}\u{00b2})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NOCTC:  4×9^18 = 4×150_094_635_296_999_121 = 600_378_541_187_996_484 (fits u64).
// NHOCTC: 6×18^17 → SATURATES → u64::MAX.
//   (18^16≈1.214×10^20 > u64::MAX per-edge)
// NLSO:   6×162^12 → SATURATES → u64::MAX (per-edge already >> u64::MAX).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);
    add_node(T44_VEC_C, T44_KEY_C, T44_ID_C);
    add_node(T44_VEC_D, T44_KEY_D, T44_ID_D);
    add_edge(T44_ID_A, T44_ID_B, "t44.e.ab");
    add_edge(T44_ID_B, T44_ID_A, "t44.e.ba");
    add_edge(T44_ID_A, T44_ID_C, "t44.e.ac");
    add_edge(T44_ID_C, T44_ID_A, "t44.e.ca");
    add_edge(T44_ID_A, T44_ID_D, "t44.e.ad");
    add_edge(T44_ID_D, T44_ID_A, "t44.e.da");
    add_edge(T44_ID_B, T44_ID_C, "t44.e.bc");
    add_edge(T44_ID_C, T44_ID_B, "t44.e.cb");
    add_edge(T44_ID_B, T44_ID_D, "t44.e.bd");
    add_edge(T44_ID_D, T44_ID_B, "t44.e.db");
    add_edge(T44_ID_C, T44_ID_D, "t44.e.cd");
    add_edge(T44_ID_D, T44_ID_C, "t44.e.dc");

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     4,                        "k4: node_count=4");
    assert_eq!(ec,     6,                        "k4: edge_count=6");
    assert_eq!(noctc,  600_378_541_187_996_484,  "k4: NOCTC=600_378_541_187_996_484 (4\u{00d7}150_094_635_296_999_121; 9\u{00b9}\u{2078}=150_094_635_296_999_121; S-uniform S=9; fits u64)");
    assert_eq!(nhoctc, u64::MAX,                 "k4: NHOCTC=u64::MAX (6\u{00d7}18\u{00b9}\u{2077} >> u64::MAX; saturated)");
    assert_eq!(nlso,   u64::MAX,                 "k4: NLSO=u64::MAX (6\u{00d7}162\u{00b9}\u{00b2} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NOCTC=0; NHOCTC=0; NLSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     2, "isolated: node_count=2");
    assert_eq!(ec,     0, "isolated: no edges");
    assert_eq!(noctc,  0, "isolated: NOCTC=0 (S=0; 0^18=0)");
    assert_eq!(nhoctc, 0, "isolated: NHOCTC=0 (no edges)");
    assert_eq!(nlso,   0, "isolated: NLSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NOCTC:  5×6^18 = 5×101_559_956_668_416 = 507_799_783_342_080.
// NHOCTC: 6×12^17 = 6×2_218_611_106_740_436_992 = 13_311_666_640_442_621_952.
// NLSO:   6×72^12 ≈ >> u64::MAX → u64::MAX.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T44_VEC_A, T44_KEY_A, T44_ID_A);
    add_node(T44_VEC_B, T44_KEY_B, T44_ID_B);
    add_node(T44_VEC_C, T44_KEY_C, T44_ID_C);
    add_node(T44_VEC_D, T44_KEY_D, T44_ID_D);
    add_node(T44_VEC_E, T44_KEY_E, T44_ID_E);
    add_edge(T44_ID_A, T44_ID_C, "t44.e.ac");
    add_edge(T44_ID_C, T44_ID_A, "t44.e.ca");
    add_edge(T44_ID_A, T44_ID_D, "t44.e.ad");
    add_edge(T44_ID_D, T44_ID_A, "t44.e.da");
    add_edge(T44_ID_A, T44_ID_E, "t44.e.ae");
    add_edge(T44_ID_E, T44_ID_A, "t44.e.ea");
    add_edge(T44_ID_B, T44_ID_C, "t44.e.bc");
    add_edge(T44_ID_C, T44_ID_B, "t44.e.cb");
    add_edge(T44_ID_B, T44_ID_D, "t44.e.bd");
    add_edge(T44_ID_D, T44_ID_B, "t44.e.db");
    add_edge(T44_ID_B, T44_ID_E, "t44.e.be");
    add_edge(T44_ID_E, T44_ID_B, "t44.e.eb");

    let (noctc, nhoctc, nlso, ec, nc) = gos_runtime::graph_topo_indices44();
    assert_eq!(nc,     5,                            "k23: node_count=5");
    assert_eq!(ec,     6,                            "k23: edge_count=6");
    assert_eq!(noctc,  507_799_783_342_080,          "k23: NOCTC=507_799_783_342_080 (5\u{00d7}101_559_956_668_416; 6\u{00b9}\u{2078}=101_559_956_668_416; S-uniform S=6)");
    assert_eq!(nhoctc, 13_311_666_640_442_621_952,   "k23: NHOCTC=13_311_666_640_442_621_952 (6\u{00d7}2_218_611_106_740_436_992; 12\u{00b9}\u{2077}=2_218_611_106_740_436_992; S-uniform S=6)");
    assert_eq!(nlso,   u64::MAX,                     "k23: NLSO=u64::MAX (6\u{00d7}72\u{00b9}\u{00b2} >> u64::MAX; saturated)");
}
