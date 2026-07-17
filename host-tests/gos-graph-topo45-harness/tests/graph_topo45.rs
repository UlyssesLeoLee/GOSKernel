// gos-graph-topo45-harness — V3.56 NNONTC + NHNONTC + NMSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices45()`:
//   Returns (nnontc, nhnontc, nmso, edge_count, node_count)
//   - nnontc  = NNONTC(G)  = Σ_v S(v)^19                  (exact u64; S-Nonadecic vertex sum)
//   - nhnontc = NHNONTC(G) = Σ_{uv∈E} (S_u+S_v)^18        (exact u64; S-Octadecic edge-sum)
//   - nmso    = NMSO(G)    = Σ_{uv∈E} (S_u²+S_v²)^13      (exact u64; S-Hexacosic Sombor, α=26)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NNONTC(G) = Σ_v S(v)^19
//     S-Nonadecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45).
//     NNONTC = n·S^19 for S-regular.
//     Overflow: S^19 ≤ 16129^19 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHNONTC(G) = Σ_{uv∈E} (S_u+S_v)^18
//     S-Octadecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45).
//     NHNONTC = |E|·(2S)^18 = 262144|E|·S^18 for S-regular.
//     Overflow per edge: (2×16129)^18 → saturating u128 accumulator.
//
//   NMSO(G) = Σ_{uv∈E} (S_u²+S_v²)^13
//     S-Hexacosic Sombor: generalised Sombor SO^α with α=26 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26) — exact, no isqrt.
//     NMSO = |E|·(2S²)^13 = 8192|E|·S^26 for S-regular.
//     Overflow per edge: (2×16129²)^13 → saturating u128 accumulator;
//     K₃ (S=4), K_{1,4} (S=4), K₄ (S=9) and K_{2,3} (S=6) all saturate → u64::MAX.
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
//  Graph     NNONTC(exact)              NHNONTC(exact)               NMSO(exact)             edges  nodes
//  Empty                  0                            0                         0              0      0
//  1 node                 0                            0                         0              0      1
//  K₂                     2                      262_144                     8_192              1      2
//  P₃               1_572_864              137_438_953_472           1_099_511_627_776           2      3
//  K₃         824_633_720_832       54_043_195_528_445_952          u64::MAX(sat.)              3      3
//  K_{1,4}  1_374_389_534_720       72_057_594_037_927_936          u64::MAX(sat.)              4      5
//  P₄           2_325_571_510          109_189_351_199_666      21_428_715_078_855_674           3      4
//  K₄  5_403_406_870_691_968_356       u64::MAX(sat.)                u64::MAX(sat.)             6      4
//  2 isolated             0                            0                         0              0      2
//  K_{2,3}    3_046_798_700_052_480       u64::MAX(sat.)                u64::MAX(sat.)          6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NNONTC:  1^19 + 1^19 = 2. ✓
//     NHNONTC: (1+1)^18 = 2^18 = 262_144. ✓
//     NMSO:    (1²+1²)^13 = 2^13 = 8_192. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NNONTC:  3×2^19 = 3×524_288 = 1_572_864. ✓
//       (2^18=262_144; 2^19=2×262_144=524_288)
//     NHNONTC: 2×(2+2)^18 = 2×4^18 = 2×68_719_476_736 = 137_438_953_472. ✓
//       (4^17=17_179_869_184; 4^18=4×17_179_869_184=68_719_476_736)
//     NMSO:    2×(4+4)^13 = 2×8^13 = 2×549_755_813_888 = 1_099_511_627_776. ✓
//       (8^12=68_719_476_736; 8^13=8×68_719_476_736=549_755_813_888)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NNONTC:  3×4^19 = 3×274_877_906_944 = 824_633_720_832. ✓
//       (4^18=68_719_476_736; 4^19=4×68_719_476_736=274_877_906_944)
//     NHNONTC: 3×(4+4)^18 = 3×8^18 = 3×18_014_398_509_481_984 = 54_043_195_528_445_952. ✓
//       (8^17=2_251_799_813_685_248; 8^18=8×2_251_799_813_685_248=18_014_398_509_481_984)
//     NMSO:    3×(16+16)^13 = 3×32^13 = 3×36_893_488_147_419_103_232 → SATURATES to u64::MAX. ✓
//       (32^12=1_152_921_504_606_846_976; 32^13=32×1_152_921_504_606_846_976=36_893_488_147_419_103_232 >> u64::MAX)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NNONTC:  5×4^19 = 5×274_877_906_944 = 1_374_389_534_720. ✓
//     NHNONTC: 4×8^18 = 4×18_014_398_509_481_984 = 72_057_594_037_927_936. ✓
//     NMSO:    4×32^13 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHNONTC and per-edge NMSO basis.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NNONTC:  2^19+3^19+3^19+2^19 = 524_288+1_162_261_467+1_162_261_467+524_288 = 2_325_571_510. ✓
//       (3^18=387_420_489; 3^19=3×387_420_489=1_162_261_467)
//     NHNONTC: 5^18+6^18+5^18
//       5^18: 5^17=762_939_453_125; 5^18=5×762_939_453_125=3_814_697_265_625
//       6^18: 6^17=16_926_659_444_736; 6^18=6×16_926_659_444_736=101_559_956_668_416
//       3_814_697_265_625+101_559_956_668_416+3_814_697_265_625 = 109_189_351_199_666. ✓
//     NMSO:    13^13+18^13+13^13
//       (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//       13^13: 13^12=23_298_085_122_481; 13^13=13×23_298_085_122_481=302_875_106_592_253
//       18^13: 18^12=1_156_831_381_426_176; 18^13=18×1_156_831_381_426_176=20_822_964_865_671_168
//       302_875_106_592_253+20_822_964_865_671_168+302_875_106_592_253 = 21_428_715_078_855_674. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NNONTC:  4×9^19 = 4×1_350_851_717_672_992_089 = 5_403_406_870_691_968_356 (fits u64). ✓
//       (9^18=150_094_635_296_999_121; 9^19=9×150_094_635_296_999_121=1_350_851_717_672_992_089)
//     NHNONTC: 6×18^18 → SATURATES to u64::MAX.
//       (18^16=121_439_529_476_697_931_776 > u64::MAX per-edge; 18^18 >> u64::MAX) ✓
//     NMSO:    6×162^13 → SATURATES to u64::MAX (per-edge >> u64::MAX).
//       (162^13 >> u64::MAX; 32^13 already >> u64::MAX at S=4) ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NNONTC:  5×6^19 = 5×609_359_740_010_496 = 3_046_798_700_052_480 (fits u64). ✓
//       (6^18=101_559_956_668_416; 6^19=6×101_559_956_668_416=609_359_740_010_496)
//     NHNONTC: 6×12^18 → SATURATES to u64::MAX.
//       (12^17=2_218_611_106_740_436_992; 12^18=12×2_218_611_106_740_436_992=26_623_333_280_885_243_904 > u64::MAX per-edge) ✓
//     NMSO:    6×72^13 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//       (72^6=139_314_069_504; 72^12≈1.94×10^22 >> u64::MAX per-edge)
//
// S-REGULAR FORMULA VERIFICATION:
//   NNONTC  = n·S^19                               for S-regular ✓
//   NHNONTC = |E|·(2S)^18 = 262144|E|·S^18        for S-regular ✓
//   NMSO    = |E|·(2S²)^13 = 8192|E|·S^26         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 262_144, 8_192, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_572_864, 137_438_953_472, 1_099_511_627_776, 2, 3)
//  5.  Triangle K₃                       → (824_633_720_832, 54_043_195_528_445_952, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (1_374_389_534_720, 72_057_594_037_927_936, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (2_325_571_510, 109_189_351_199_666, 21_428_715_078_855_674, 3, 4)
//  8.  Complete K₄                       → (5_403_406_870_691_968_356, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (3_046_798_700_052_480, u64::MAX, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T45_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_45");
const T45_EXEC:   ExecutorId = ExecutorId::from_ascii("t45.exec");

const T45_KEY_A: &str = "t45.alpha";
const T45_KEY_B: &str = "t45.beta";
const T45_KEY_C: &str = "t45.gamma";
const T45_KEY_D: &str = "t45.delta";
const T45_KEY_E: &str = "t45.epsilon";

const T45_ID_A: NodeId = derive_node_id(T45_PLUGIN, T45_KEY_A);
const T45_ID_B: NodeId = derive_node_id(T45_PLUGIN, T45_KEY_B);
const T45_ID_C: NodeId = derive_node_id(T45_PLUGIN, T45_KEY_C);
const T45_ID_D: NodeId = derive_node_id(T45_PLUGIN, T45_KEY_D);
const T45_ID_E: NodeId = derive_node_id(T45_PLUGIN, T45_KEY_E);

// L4=132 namespace for this harness.
const T45_VEC_A: VectorAddress = VectorAddress::new(132, 1, 1, 0);
const T45_VEC_B: VectorAddress = VectorAddress::new(132, 1, 2, 0);
const T45_VEC_C: VectorAddress = VectorAddress::new(132, 1, 3, 0);
const T45_VEC_D: VectorAddress = VectorAddress::new(132, 2, 1, 0);
const T45_VEC_E: VectorAddress = VectorAddress::new(132, 2, 2, 0);

const T45_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T45_PLUGIN,
    name:         "kl-graph-topo45-harness",
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
        executor_id:       T45_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T45_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T45_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      0, "empty: node_count=0");
    assert_eq!(ec,      0, "empty: edge_count=0");
    assert_eq!(nnontc,  0, "empty: NNONTC=0");
    assert_eq!(nhnontc, 0, "empty: NHNONTC=0");
    assert_eq!(nmso,    0, "empty: NMSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NNONTC: 0^19=0; NHNONTC: no edges; NMSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      1, "single: node_count=1");
    assert_eq!(ec,      0, "single: no edges");
    assert_eq!(nnontc,  0, "single: NNONTC=0 (S=0; 0^19=0)");
    assert_eq!(nhnontc, 0, "single: NHNONTC=0 (no edges)");
    assert_eq!(nmso,    0, "single: NMSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NNONTC:  1^19+1^19 = 2.
// NHNONTC: (1+1)^18 = 2^18 = 262_144.
// NMSO:    (1²+1²)^13 = 2^13 = 8_192.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);
    add_edge(T45_ID_A, T45_ID_B, "t45.e.ab");

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      2,       "k2: node_count=2");
    assert_eq!(ec,      1,       "k2: edge_count=1");
    assert_eq!(nnontc,  2,       "k2: NNONTC=2 (1\u{00b9}\u{2079}+1\u{00b9}\u{2079}=2; S-uniform S=1)");
    assert_eq!(nhnontc, 262_144, "k2: NHNONTC=262_144 ((1+1)\u{00b9}\u{2078}=2\u{00b9}\u{2078}=262_144; S-uniform S=1)");
    assert_eq!(nmso,    8_192,   "k2: NMSO=8_192 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{00b3}=2\u{00b9}\u{00b3}=8_192; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NNONTC:  3×2^19 = 3×524_288 = 1_572_864.
// NHNONTC: 2×(2+2)^18 = 2×4^18 = 2×68_719_476_736 = 137_438_953_472.
// NMSO:    2×(4+4)^13 = 2×8^13 = 2×549_755_813_888 = 1_099_511_627_776.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);
    add_node(T45_VEC_C, T45_KEY_C, T45_ID_C);
    add_edge(T45_ID_A, T45_ID_B, "t45.e.ab");
    add_edge(T45_ID_B, T45_ID_C, "t45.e.bc");

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      3,                 "p3: node_count=3");
    assert_eq!(ec,      2,                 "p3: edge_count=2");
    assert_eq!(nnontc,  1_572_864,         "p3: NNONTC=1_572_864 (3\u{00d7}524_288; 2\u{00b9}\u{2079}=524_288; S-uniform S=2)");
    assert_eq!(nhnontc, 137_438_953_472,   "p3: NHNONTC=137_438_953_472 (2\u{00d7}68_719_476_736; (2+2)\u{00b9}\u{2078}=4\u{00b9}\u{2078}=68_719_476_736; S-uniform S=2)");
    assert_eq!(nmso,    1_099_511_627_776, "p3: NMSO=1_099_511_627_776 (2\u{00d7}549_755_813_888; (4+4)\u{00b9}\u{00b3}=8\u{00b9}\u{00b3}=549_755_813_888; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NNONTC:  3×4^19 = 3×274_877_906_944 = 824_633_720_832.
// NHNONTC: 3×(4+4)^18 = 3×8^18 = 3×18_014_398_509_481_984 = 54_043_195_528_445_952.
// NMSO:    3×(16+16)^13 = 3×32^13 → SATURATES to u64::MAX.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);
    add_node(T45_VEC_C, T45_KEY_C, T45_ID_C);
    add_edge(T45_ID_A, T45_ID_B, "t45.e.ab");
    add_edge(T45_ID_B, T45_ID_A, "t45.e.ba");
    add_edge(T45_ID_B, T45_ID_C, "t45.e.bc");
    add_edge(T45_ID_C, T45_ID_B, "t45.e.cb");
    add_edge(T45_ID_A, T45_ID_C, "t45.e.ac");
    add_edge(T45_ID_C, T45_ID_A, "t45.e.ca");

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      3,                       "k3: node_count=3");
    assert_eq!(ec,      3,                       "k3: edge_count=3");
    assert_eq!(nnontc,  824_633_720_832,         "k3: NNONTC=824_633_720_832 (3\u{00d7}274_877_906_944; 4\u{00b9}\u{2079}=274_877_906_944; S-uniform S=4)");
    assert_eq!(nhnontc, 54_043_195_528_445_952,  "k3: NHNONTC=54_043_195_528_445_952 (3\u{00d7}18_014_398_509_481_984; (4+4)\u{00b9}\u{2078}=8\u{00b9}\u{2078}=18_014_398_509_481_984; S-uniform S=4)");
    assert_eq!(nmso,    u64::MAX,                "k3: NMSO=u64::MAX (3\u{00d7}32\u{00b9}\u{00b3} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NNONTC:  5×4^19 = 5×274_877_906_944 = 1_374_389_534_720.
// NHNONTC: 4×8^18 = 4×18_014_398_509_481_984 = 72_057_594_037_927_936.
// NMSO:    4×32^13 → SATURATES to u64::MAX.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);
    add_node(T45_VEC_C, T45_KEY_C, T45_ID_C);
    add_node(T45_VEC_D, T45_KEY_D, T45_ID_D);
    add_node(T45_VEC_E, T45_KEY_E, T45_ID_E);
    add_edge(T45_ID_A, T45_ID_B, "t45.e.ab");
    add_edge(T45_ID_A, T45_ID_C, "t45.e.ac");
    add_edge(T45_ID_A, T45_ID_D, "t45.e.ad");
    add_edge(T45_ID_A, T45_ID_E, "t45.e.ae");

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      5,                       "star: node_count=5");
    assert_eq!(ec,      4,                       "star: edge_count=4");
    assert_eq!(nnontc,  1_374_389_534_720,       "star: NNONTC=1_374_389_534_720 (5\u{00d7}274_877_906_944; same S as K\u{2083})");
    assert_eq!(nhnontc, 72_057_594_037_927_936,  "star: NHNONTC=72_057_594_037_927_936 (4\u{00d7}18_014_398_509_481_984; same per-edge as K\u{2083})");
    assert_eq!(nmso,    u64::MAX,                "star: NMSO=u64::MAX (4\u{00d7}32\u{00b9}\u{00b3} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NNONTC:  2^19+3^19+3^19+2^19 = 524_288+1_162_261_467+1_162_261_467+524_288 = 2_325_571_510.
// NHNONTC: 5^18+6^18+5^18
//   = 3_814_697_265_625+101_559_956_668_416+3_814_697_265_625 = 109_189_351_199_666.
// NMSO:    13^13+18^13+13^13
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//   = 302_875_106_592_253+20_822_964_865_671_168+302_875_106_592_253 = 21_428_715_078_855_674.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);
    add_node(T45_VEC_C, T45_KEY_C, T45_ID_C);
    add_node(T45_VEC_D, T45_KEY_D, T45_ID_D);
    add_edge(T45_ID_A, T45_ID_B, "t45.e.ab");
    add_edge(T45_ID_B, T45_ID_C, "t45.e.bc");
    add_edge(T45_ID_C, T45_ID_D, "t45.e.cd");

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      4,                        "p4: node_count=4");
    assert_eq!(ec,      3,                        "p4: edge_count=3");
    assert_eq!(nnontc,  2_325_571_510,            "p4: NNONTC=2_325_571_510 (524_288+1_162_261_467+1_162_261_467+524_288; 2\u{00b9}\u{2079}+3\u{00b9}\u{2079}+3\u{00b9}\u{2079}+2\u{00b9}\u{2079})");
    assert_eq!(nhnontc, 109_189_351_199_666,      "p4: NHNONTC=109_189_351_199_666 (3_814_697_265_625+101_559_956_668_416+3_814_697_265_625; 5\u{00b9}\u{2078}+6\u{00b9}\u{2078}+5\u{00b9}\u{2078})");
    assert_eq!(nmso,    21_428_715_078_855_674,   "p4: NMSO=21_428_715_078_855_674 (302_875_106_592_253+20_822_964_865_671_168+302_875_106_592_253; 13\u{00b9}\u{00b3}+18\u{00b9}\u{00b3}+13\u{00b9}\u{00b3})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NNONTC:  4×9^19 = 4×1_350_851_717_672_992_089 = 5_403_406_870_691_968_356 (fits u64).
// NHNONTC: 6×18^18 → SATURATES → u64::MAX.
//   (18^16=121_439_529_476_697_931_776 > u64::MAX per-edge)
// NMSO:    6×162^13 → SATURATES → u64::MAX (per-edge already >> u64::MAX).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);
    add_node(T45_VEC_C, T45_KEY_C, T45_ID_C);
    add_node(T45_VEC_D, T45_KEY_D, T45_ID_D);
    add_edge(T45_ID_A, T45_ID_B, "t45.e.ab");
    add_edge(T45_ID_B, T45_ID_A, "t45.e.ba");
    add_edge(T45_ID_A, T45_ID_C, "t45.e.ac");
    add_edge(T45_ID_C, T45_ID_A, "t45.e.ca");
    add_edge(T45_ID_A, T45_ID_D, "t45.e.ad");
    add_edge(T45_ID_D, T45_ID_A, "t45.e.da");
    add_edge(T45_ID_B, T45_ID_C, "t45.e.bc");
    add_edge(T45_ID_C, T45_ID_B, "t45.e.cb");
    add_edge(T45_ID_B, T45_ID_D, "t45.e.bd");
    add_edge(T45_ID_D, T45_ID_B, "t45.e.db");
    add_edge(T45_ID_C, T45_ID_D, "t45.e.cd");
    add_edge(T45_ID_D, T45_ID_C, "t45.e.dc");

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      4,                           "k4: node_count=4");
    assert_eq!(ec,      6,                           "k4: edge_count=6");
    assert_eq!(nnontc,  5_403_406_870_691_968_356,   "k4: NNONTC=5_403_406_870_691_968_356 (4\u{00d7}1_350_851_717_672_992_089; 9\u{00b9}\u{2079}=1_350_851_717_672_992_089; S-uniform S=9; fits u64)");
    assert_eq!(nhnontc, u64::MAX,                    "k4: NHNONTC=u64::MAX (6\u{00d7}18\u{00b9}\u{2078} >> u64::MAX; saturated)");
    assert_eq!(nmso,    u64::MAX,                    "k4: NMSO=u64::MAX (6\u{00d7}162\u{00b9}\u{00b3} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NNONTC=0; NHNONTC=0; NMSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      2, "isolated: node_count=2");
    assert_eq!(ec,      0, "isolated: no edges");
    assert_eq!(nnontc,  0, "isolated: NNONTC=0 (S=0; 0^19=0)");
    assert_eq!(nhnontc, 0, "isolated: NHNONTC=0 (no edges)");
    assert_eq!(nmso,    0, "isolated: NMSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NNONTC:  5×6^19 = 5×609_359_740_010_496 = 3_046_798_700_052_480.
// NHNONTC: 6×12^18 → SATURATES to u64::MAX.
//   (12^18=26_623_333_280_885_243_904 > u64::MAX per-edge)
// NMSO:    6×72^13 → SATURATES to u64::MAX (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T45_VEC_A, T45_KEY_A, T45_ID_A);
    add_node(T45_VEC_B, T45_KEY_B, T45_ID_B);
    add_node(T45_VEC_C, T45_KEY_C, T45_ID_C);
    add_node(T45_VEC_D, T45_KEY_D, T45_ID_D);
    add_node(T45_VEC_E, T45_KEY_E, T45_ID_E);
    add_edge(T45_ID_A, T45_ID_C, "t45.e.ac");
    add_edge(T45_ID_C, T45_ID_A, "t45.e.ca");
    add_edge(T45_ID_A, T45_ID_D, "t45.e.ad");
    add_edge(T45_ID_D, T45_ID_A, "t45.e.da");
    add_edge(T45_ID_A, T45_ID_E, "t45.e.ae");
    add_edge(T45_ID_E, T45_ID_A, "t45.e.ea");
    add_edge(T45_ID_B, T45_ID_C, "t45.e.bc");
    add_edge(T45_ID_C, T45_ID_B, "t45.e.cb");
    add_edge(T45_ID_B, T45_ID_D, "t45.e.bd");
    add_edge(T45_ID_D, T45_ID_B, "t45.e.db");
    add_edge(T45_ID_B, T45_ID_E, "t45.e.be");
    add_edge(T45_ID_E, T45_ID_B, "t45.e.eb");

    let (nnontc, nhnontc, nmso, ec, nc) = gos_runtime::graph_topo_indices45();
    assert_eq!(nc,      5,                       "k23: node_count=5");
    assert_eq!(ec,      6,                       "k23: edge_count=6");
    assert_eq!(nnontc,  3_046_798_700_052_480,   "k23: NNONTC=3_046_798_700_052_480 (5\u{00d7}609_359_740_010_496; 6\u{00b9}\u{2079}=609_359_740_010_496; S-uniform S=6)");
    assert_eq!(nhnontc, u64::MAX,                "k23: NHNONTC=u64::MAX (6\u{00d7}12\u{00b9}\u{2078}=6\u{00d7}26_623_333_280_885_243_904 >> u64::MAX; per-edge saturates)");
    assert_eq!(nmso,    u64::MAX,                "k23: NMSO=u64::MAX (6\u{00d7}72\u{00b9}\u{00b3} >> u64::MAX; per-edge saturates)");
}
