// gos-graph-topo42-harness — V3.53 NSTC + NHPTC + NJSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices42()`:
//   Returns (nstc, nhptc, njso, edge_count, node_count)
//   - nstc  = NSTC(G)  = Σ_v S(v)^16                    (exact u64; S-Hexadecic vertex sum)
//   - nhptc = NHPTC(G) = Σ_{uv∈E} (S_u+S_v)^15          (exact u64; S-Pentadecic edge-sum)
//   - njso  = NJSO(G)  = Σ_{uv∈E} (S_u²+S_v²)^10        (exact u64; S-Eicosic Sombor, α=20)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NSTC(G) = Σ_v S(v)^16
//     S-Hexadecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42).
//     NSTC = n·S^16 for S-regular.
//     Overflow: S^16 ≤ 16129^16 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHPTC(G) = Σ_{uv∈E} (S_u+S_v)^15
//     S-Pentadecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42).
//     NHPTC = |E|·(2S)^15 = 32768|E|·S^15 for S-regular.
//     Overflow per edge: (2×16129)^15 → saturating u128 accumulator.
//
//   NJSO(G) = Σ_{uv∈E} (S_u²+S_v²)^10
//     S-Eicosic Sombor: generalised Sombor SO^α with α=20 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20) — exact, no isqrt.
//     NJSO = |E|·(2S²)^10 = 1024|E|·S^20 for S-regular.
//     Overflow per edge: (2×16129²)^10 → saturating u128 accumulator;
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
//  Graph     NSTC(exact)              NHPTC(exact)                 NJSO(exact)             edges  nodes
//  Empty                0                          0                         0              0      0
//  1 node               0                          0                         0              0      1
//  K₂                   2                     32_768                     1_024              1      2
//  P₃             196_608              2_147_483_648             2_147_483_648              2      3
//  K₃      12_884_901_888        105_553_116_266_496     3_377_699_720_527_872              3      3
//  K_{1,4} 21_474_836_480        140_737_488_355_328     4_503_599_627_370_496              4      5
//  P₄          86_224_514            531_220_140_826         3_846_184_210_322              3      4
//  K₄   7_412_080_755_407_364         u64::MAX(sat.)              u64::MAX(sat.)            6      4
//  2 isolated           0                          0                         0              0      2
//  K_{2,3} 14_105_549_537_280   92_442_129_447_518_208              u64::MAX(sat.)          6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NSTC:  1^16 + 1^16 = 2. ✓
//     NHPTC: (1+1)^15 = 2^15 = 32_768. ✓
//     NJSO:  (1²+1²)^10 = 2^10 = 1_024. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NSTC:  3×2^16 = 3×65_536 = 196_608. ✓
//     NHPTC: 2×(2+2)^15 = 2×4^15 = 2×1_073_741_824 = 2_147_483_648. ✓
//       (4^15=4^8×4^4×4^2×4=65_536×256×16×4=65_536×16_384=1_073_741_824)
//     NJSO:  2×(4+4)^10 = 2×8^10 = 2×1_073_741_824 = 2_147_483_648. ✓
//       (8^10=8^8×8^2=16_777_216×64=1_073_741_824)
//     Coincidence: NHPTC=NJSO=2_147_483_648 on P₃ (4^15=8^10=2^30). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NSTC:  3×4^16 = 3×4_294_967_296 = 12_884_901_888. ✓
//       (4^15=1_073_741_824 (topo41); 4^16=4×1_073_741_824=4_294_967_296)
//     NHPTC: 3×(4+4)^15 = 3×8^15 = 3×35_184_372_088_832 = 105_553_116_266_496. ✓
//       (8^14=4_398_046_511_104 (topo41); 8^15=8×4_398_046_511_104=35_184_372_088_832)
//     NJSO:  3×(16+16)^10 = 3×32^10 = 3×1_125_899_906_842_624 = 3_377_699_720_527_872. ✓
//       (32^9=35_184_372_088_832 (topo41 NIOSO K_{1,4}/4); 32^10=32×35_184_372_088_832=1_125_899_906_842_624)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NSTC:  5×4^16 = 5×4_294_967_296 = 21_474_836_480. ✓
//     NHPTC: 4×8^15 = 4×35_184_372_088_832 = 140_737_488_355_328. ✓
//     NJSO:  4×32^10 = 4×1_125_899_906_842_624 = 4_503_599_627_370_496. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHPTC and NJSO; NSTC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NSTC:  2^16+3^16+3^16+2^16 = 65_536+43_046_721+43_046_721+65_536 = 86_224_514. ✓
//       (3^15=14_348_907 (topo41); 3^16=3×14_348_907=43_046_721)
//     NHPTC: (2+3)^15+(3+3)^15+(3+2)^15 = 5^15+6^15+5^15
//            = 30_517_578_125+470_184_984_576+30_517_578_125 = 531_220_140_826. ✓
//       (5^14=6_103_515_625 (topo41); 5^15=5×6_103_515_625=30_517_578_125)
//       (6^14=78_364_164_096 (topo41); 6^15=6×78_364_164_096=470_184_984_576)
//     NJSO:  13^10+18^10+13^10 = 137_858_491_849+3_570_467_226_624+137_858_491_849 = 3_846_184_210_322. ✓
//       (13^9=10_604_499_373 (topo41); 13^10=13×10_604_499_373=137_858_491_849)
//       (18^9=198_359_290_368 (topo41); 18^10=18×198_359_290_368=3_570_467_226_624)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NSTC:  4×9^16 = 4×1_853_020_188_851_841 = 7_412_080_755_407_364. ✓
//       (9^15=205_891_132_094_649 (topo41); 9^16=9×205_891_132_094_649=1_853_020_188_851_841)
//     NHPTC: 6×18^15 → SATURATES to u64::MAX.
//       (18^14=374_813_367_582_081_024 (topo41); 18^15=18×374_813_367_582_081_024=6_746_640_616_477_458_432)
//       (6×6_746_640_616_477_458_432=40_479_843_698_864_750_592 > u64::MAX) → clamped u64::MAX. ✓
//     NJSO:  6×162^10 → SATURATES to u64::MAX (per-edge 162^10 >> u64::MAX).
//       (162^9=76_848_453_272_063_549_952 (topo41); 162^10=162×76_848_453_272_063_549_952
//        ≈1.245×10^22 >> u64::MAX per-edge → clamped u64::MAX). ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NSTC:  5×6^16 = 5×2_821_109_907_456 = 14_105_549_537_280. ✓
//       (6^15=470_184_984_576 (topo41); 6^16=6×470_184_984_576=2_821_109_907_456)
//     NHPTC: 6×12^15 = 6×15_407_021_574_586_368 = 92_442_129_447_518_208. ✓
//       (12^14=1_283_918_464_548_864 (topo41); 12^15=12×1_283_918_464_548_864=15_407_021_574_586_368)
//       (92_442_129_447_518_208 < u64::MAX) → exact. ✓
//     NJSO:  6×72^10 → SATURATES to u64::MAX.
//       (72^9=51_998_697_814_228_992 (topo41); 72^10=72×51_998_697_814_228_992=3_743_906_402_864_487_424)
//       (6×3_743_906_402_864_487_424=22_463_438_417_186_924_544 > u64::MAX) → clamped u64::MAX. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NSTC  = n·S^16                              for S-regular ✓
//   NHPTC = |E|·(2S)^15 = 32768|E|·S^15        for S-regular ✓
//   NJSO  = |E|·(2S²)^10 = 1024|E|·S^20        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 32_768, 1_024, 1, 2)
//  4.  Path P₃ = A-B-C                   → (196_608, 2_147_483_648, 2_147_483_648, 2, 3)
//  5.  Triangle K₃                       → (12_884_901_888, 105_553_116_266_496, 3_377_699_720_527_872, 3, 3)
//  6.  Star K_{1,4}                      → (21_474_836_480, 140_737_488_355_328, 4_503_599_627_370_496, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (86_224_514, 531_220_140_826, 3_846_184_210_322, 3, 4)
//  8.  Complete K₄                       → (7_412_080_755_407_364, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (14_105_549_537_280, 92_442_129_447_518_208, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T42_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_42");
const T42_EXEC:   ExecutorId = ExecutorId::from_ascii("t42.exec");

const T42_KEY_A: &str = "t42.alpha";
const T42_KEY_B: &str = "t42.beta";
const T42_KEY_C: &str = "t42.gamma";
const T42_KEY_D: &str = "t42.delta";
const T42_KEY_E: &str = "t42.epsilon";

const T42_ID_A: NodeId = derive_node_id(T42_PLUGIN, T42_KEY_A);
const T42_ID_B: NodeId = derive_node_id(T42_PLUGIN, T42_KEY_B);
const T42_ID_C: NodeId = derive_node_id(T42_PLUGIN, T42_KEY_C);
const T42_ID_D: NodeId = derive_node_id(T42_PLUGIN, T42_KEY_D);
const T42_ID_E: NodeId = derive_node_id(T42_PLUGIN, T42_KEY_E);

// L4=129 namespace for this harness.
const T42_VEC_A: VectorAddress = VectorAddress::new(129, 1, 1, 0);
const T42_VEC_B: VectorAddress = VectorAddress::new(129, 1, 2, 0);
const T42_VEC_C: VectorAddress = VectorAddress::new(129, 1, 3, 0);
const T42_VEC_D: VectorAddress = VectorAddress::new(129, 2, 1, 0);
const T42_VEC_E: VectorAddress = VectorAddress::new(129, 2, 2, 0);

const T42_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T42_PLUGIN,
    name:         "kl-graph-topo42-harness",
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
        executor_id:       T42_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T42_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T42_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    0, "empty: node_count=0");
    assert_eq!(ec,    0, "empty: edge_count=0");
    assert_eq!(nstc,  0, "empty: NSTC=0");
    assert_eq!(nhptc, 0, "empty: NHPTC=0");
    assert_eq!(njso,  0, "empty: NJSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NSTC: 0^16=0; NHPTC: no edges; NJSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    1, "single: node_count=1");
    assert_eq!(ec,    0, "single: no edges");
    assert_eq!(nstc,  0, "single: NSTC=0 (S=0; 0^16=0)");
    assert_eq!(nhptc, 0, "single: NHPTC=0 (no edges)");
    assert_eq!(njso,  0, "single: NJSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NSTC:  1^16+1^16 = 2.
// NHPTC: (1+1)^15 = 2^15 = 32_768.
// NJSO:  (1²+1²)^10 = 2^10 = 1_024.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);
    add_edge(T42_ID_A, T42_ID_B, "t42.e.ab");

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    2,      "k2: node_count=2");
    assert_eq!(ec,    1,      "k2: edge_count=1");
    assert_eq!(nstc,  2,      "k2: NSTC=2 (1\u{00b9}\u{2076}+1\u{00b9}\u{2076}=2; S-uniform S=1)");
    assert_eq!(nhptc, 32_768, "k2: NHPTC=32_768 ((1+1)\u{00b9}\u{2075}=2\u{00b9}\u{2075}=32_768; S-uniform S=1)");
    assert_eq!(njso,  1_024,  "k2: NJSO=1_024 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{2070}=2\u{00b9}\u{2070}=1_024; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NSTC:  3×2^16 = 3×65_536 = 196_608.
// NHPTC: 2×(2+2)^15 = 2×4^15 = 2×1_073_741_824 = 2_147_483_648.
// NJSO:  2×(4+4)^10 = 2×8^10 = 2×1_073_741_824 = 2_147_483_648.
// Coincidence: NHPTC=NJSO on P₃ (4^15=8^10=2^30=1_073_741_824).

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);
    add_node(T42_VEC_C, T42_KEY_C, T42_ID_C);
    add_edge(T42_ID_A, T42_ID_B, "t42.e.ab");
    add_edge(T42_ID_B, T42_ID_C, "t42.e.bc");

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    3,             "p3: node_count=3");
    assert_eq!(ec,    2,             "p3: edge_count=2");
    assert_eq!(nstc,  196_608,       "p3: NSTC=196_608 (3\u{00d7}65_536; 2\u{00b9}\u{2076}=65_536; S-uniform S=2)");
    assert_eq!(nhptc, 2_147_483_648, "p3: NHPTC=2_147_483_648 (2\u{00d7}1_073_741_824; (2+2)\u{00b9}\u{2075}=4\u{00b9}\u{2075}=1_073_741_824; S-uniform S=2)");
    assert_eq!(njso,  2_147_483_648, "p3: NJSO=2_147_483_648 (2\u{00d7}1_073_741_824; (4+4)\u{00b9}\u{2070}=8\u{00b9}\u{2070}=1_073_741_824; coincidence NHPTC=NJSO=2^30)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NSTC:  3×4^16 = 3×4_294_967_296 = 12_884_901_888.
// NHPTC: 3×(4+4)^15 = 3×8^15 = 3×35_184_372_088_832 = 105_553_116_266_496.
// NJSO:  3×(16+16)^10 = 3×32^10 = 3×1_125_899_906_842_624 = 3_377_699_720_527_872.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);
    add_node(T42_VEC_C, T42_KEY_C, T42_ID_C);
    add_edge(T42_ID_A, T42_ID_B, "t42.e.ab");
    add_edge(T42_ID_B, T42_ID_A, "t42.e.ba");
    add_edge(T42_ID_B, T42_ID_C, "t42.e.bc");
    add_edge(T42_ID_C, T42_ID_B, "t42.e.cb");
    add_edge(T42_ID_A, T42_ID_C, "t42.e.ac");
    add_edge(T42_ID_C, T42_ID_A, "t42.e.ca");

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    3,                       "k3: node_count=3");
    assert_eq!(ec,    3,                       "k3: edge_count=3");
    assert_eq!(nstc,  12_884_901_888,          "k3: NSTC=12_884_901_888 (3\u{00d7}4_294_967_296; 4\u{00b9}\u{2076}=4_294_967_296; S-uniform S=4)");
    assert_eq!(nhptc, 105_553_116_266_496,     "k3: NHPTC=105_553_116_266_496 (3\u{00d7}35_184_372_088_832; (4+4)\u{00b9}\u{2075}=8\u{00b9}\u{2075}=35_184_372_088_832; S-uniform S=4)");
    assert_eq!(njso,  3_377_699_720_527_872,   "k3: NJSO=3_377_699_720_527_872 (3\u{00d7}1_125_899_906_842_624; (16+16)\u{00b9}\u{2070}=32\u{00b9}\u{2070}=1_125_899_906_842_624; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHPTC and NJSO as K₃; NSTC and totals differ by node/edge count.
// NSTC:  5×4^16 = 5×4_294_967_296 = 21_474_836_480.
// NHPTC: 4×8^15 = 4×35_184_372_088_832 = 140_737_488_355_328.
// NJSO:  4×32^10 = 4×1_125_899_906_842_624 = 4_503_599_627_370_496.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);
    add_node(T42_VEC_C, T42_KEY_C, T42_ID_C);
    add_node(T42_VEC_D, T42_KEY_D, T42_ID_D);
    add_node(T42_VEC_E, T42_KEY_E, T42_ID_E);
    add_edge(T42_ID_A, T42_ID_B, "t42.e.ab");
    add_edge(T42_ID_A, T42_ID_C, "t42.e.ac");
    add_edge(T42_ID_A, T42_ID_D, "t42.e.ad");
    add_edge(T42_ID_A, T42_ID_E, "t42.e.ae");

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    5,                       "star: node_count=5");
    assert_eq!(ec,    4,                       "star: edge_count=4");
    assert_eq!(nstc,  21_474_836_480,          "star: NSTC=21_474_836_480 (5\u{00d7}4_294_967_296; same S as K\u{2083})");
    assert_eq!(nhptc, 140_737_488_355_328,     "star: NHPTC=140_737_488_355_328 (4\u{00d7}35_184_372_088_832; same per-edge as K\u{2083})");
    assert_eq!(njso,  4_503_599_627_370_496,   "star: NJSO=4_503_599_627_370_496 (4\u{00d7}1_125_899_906_842_624; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NSTC:  2^16+3^16+3^16+2^16 = 65_536+43_046_721+43_046_721+65_536 = 86_224_514.
// NHPTC: (2+3)^15+(3+3)^15+(3+2)^15 = 5^15+6^15+5^15
//        = 30_517_578_125+470_184_984_576+30_517_578_125 = 531_220_140_826.
// NJSO:  13^10+18^10+13^10 = 137_858_491_849+3_570_467_226_624+137_858_491_849 = 3_846_184_210_322.
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);
    add_node(T42_VEC_C, T42_KEY_C, T42_ID_C);
    add_node(T42_VEC_D, T42_KEY_D, T42_ID_D);
    add_edge(T42_ID_A, T42_ID_B, "t42.e.ab");
    add_edge(T42_ID_B, T42_ID_C, "t42.e.bc");
    add_edge(T42_ID_C, T42_ID_D, "t42.e.cd");

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    4,                 "p4: node_count=4");
    assert_eq!(ec,    3,                 "p4: edge_count=3");
    assert_eq!(nstc,  86_224_514,        "p4: NSTC=86_224_514 (65_536+43_046_721+43_046_721+65_536; 2\u{00b9}\u{2076}+3\u{00b9}\u{2076}+3\u{00b9}\u{2076}+2\u{00b9}\u{2076})");
    assert_eq!(nhptc, 531_220_140_826,   "p4: NHPTC=531_220_140_826 (30_517_578_125+470_184_984_576+30_517_578_125; 5\u{00b9}\u{2075}+6\u{00b9}\u{2075}+5\u{00b9}\u{2075})");
    assert_eq!(njso,  3_846_184_210_322, "p4: NJSO=3_846_184_210_322 (137_858_491_849+3_570_467_226_624+137_858_491_849; 13\u{00b9}\u{2070}+18\u{00b9}\u{2070}+13\u{00b9}\u{2070})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NSTC:  4×9^16 = 4×1_853_020_188_851_841 = 7_412_080_755_407_364.
// NHPTC: 6×18^15 → SATURATES → u64::MAX.
//   (18^15=6_746_640_616_477_458_432; 6×18^15=40_479_843_698_864_750_592 > u64::MAX)
// NJSO:  6×162^10 → SATURATES → u64::MAX (per-edge already >> u64::MAX).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);
    add_node(T42_VEC_C, T42_KEY_C, T42_ID_C);
    add_node(T42_VEC_D, T42_KEY_D, T42_ID_D);
    add_edge(T42_ID_A, T42_ID_B, "t42.e.ab");
    add_edge(T42_ID_B, T42_ID_A, "t42.e.ba");
    add_edge(T42_ID_A, T42_ID_C, "t42.e.ac");
    add_edge(T42_ID_C, T42_ID_A, "t42.e.ca");
    add_edge(T42_ID_A, T42_ID_D, "t42.e.ad");
    add_edge(T42_ID_D, T42_ID_A, "t42.e.da");
    add_edge(T42_ID_B, T42_ID_C, "t42.e.bc");
    add_edge(T42_ID_C, T42_ID_B, "t42.e.cb");
    add_edge(T42_ID_B, T42_ID_D, "t42.e.bd");
    add_edge(T42_ID_D, T42_ID_B, "t42.e.db");
    add_edge(T42_ID_C, T42_ID_D, "t42.e.cd");
    add_edge(T42_ID_D, T42_ID_C, "t42.e.dc");

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    4,                       "k4: node_count=4");
    assert_eq!(ec,    6,                       "k4: edge_count=6");
    assert_eq!(nstc,  7_412_080_755_407_364,   "k4: NSTC=7_412_080_755_407_364 (4\u{00d7}1_853_020_188_851_841; 9\u{00b9}\u{2076}=1_853_020_188_851_841; S-uniform S=9)");
    assert_eq!(nhptc, u64::MAX,                "k4: NHPTC=u64::MAX (6\u{00d7}18\u{00b9}\u{2075}=40_479_843_698_864_750_592 > u64::MAX; saturated)");
    assert_eq!(njso,  u64::MAX,                "k4: NJSO=u64::MAX (6\u{00d7}162\u{00b9}\u{2070} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NSTC=0; NHPTC=0; NJSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    2, "isolated: node_count=2");
    assert_eq!(ec,    0, "isolated: no edges");
    assert_eq!(nstc,  0, "isolated: NSTC=0 (S=0; 0^16=0)");
    assert_eq!(nhptc, 0, "isolated: NHPTC=0 (no edges)");
    assert_eq!(njso,  0, "isolated: NJSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NSTC:  5×6^16 = 5×2_821_109_907_456 = 14_105_549_537_280.
// NHPTC: 6×12^15 = 6×15_407_021_574_586_368 = 92_442_129_447_518_208.
// NJSO:  6×72^10 = 6×3_743_906_402_864_487_424 = 22_463_438_417_186_924_544 > u64::MAX → u64::MAX.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T42_VEC_A, T42_KEY_A, T42_ID_A);
    add_node(T42_VEC_B, T42_KEY_B, T42_ID_B);
    add_node(T42_VEC_C, T42_KEY_C, T42_ID_C);
    add_node(T42_VEC_D, T42_KEY_D, T42_ID_D);
    add_node(T42_VEC_E, T42_KEY_E, T42_ID_E);
    add_edge(T42_ID_A, T42_ID_C, "t42.e.ac");
    add_edge(T42_ID_C, T42_ID_A, "t42.e.ca");
    add_edge(T42_ID_A, T42_ID_D, "t42.e.ad");
    add_edge(T42_ID_D, T42_ID_A, "t42.e.da");
    add_edge(T42_ID_A, T42_ID_E, "t42.e.ae");
    add_edge(T42_ID_E, T42_ID_A, "t42.e.ea");
    add_edge(T42_ID_B, T42_ID_C, "t42.e.bc");
    add_edge(T42_ID_C, T42_ID_B, "t42.e.cb");
    add_edge(T42_ID_B, T42_ID_D, "t42.e.bd");
    add_edge(T42_ID_D, T42_ID_B, "t42.e.db");
    add_edge(T42_ID_B, T42_ID_E, "t42.e.be");
    add_edge(T42_ID_E, T42_ID_B, "t42.e.eb");

    let (nstc, nhptc, njso, ec, nc) = gos_runtime::graph_topo_indices42();
    assert_eq!(nc,    5,                        "k23: node_count=5");
    assert_eq!(ec,    6,                        "k23: edge_count=6");
    assert_eq!(nstc,  14_105_549_537_280,       "k23: NSTC=14_105_549_537_280 (5\u{00d7}2_821_109_907_456; 6\u{00b9}\u{2076}=2_821_109_907_456; S-uniform S=6)");
    assert_eq!(nhptc, 92_442_129_447_518_208,   "k23: NHPTC=92_442_129_447_518_208 (6\u{00d7}15_407_021_574_586_368; 12\u{00b9}\u{2075}=15_407_021_574_586_368; S-uniform S=6)");
    assert_eq!(njso,  u64::MAX,                 "k23: NJSO=u64::MAX (6\u{00d7}72\u{00b9}\u{2070}=22_463_438_417_186_924_544 > u64::MAX; saturated)");
}
