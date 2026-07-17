// gos-graph-topo46-harness — V3.57 NEICTC + NHEICTC + NNSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices46()`:
//   Returns (neictc, nheictc, nnso, edge_count, node_count)
//   - neictc  = NEICTC(G)  = Σ_v S(v)^20                  (exact u64; S-Eicosic vertex sum)
//   - nheictc = NHEICTC(G) = Σ_{uv∈E} (S_u+S_v)^19        (exact u64; S-Nonadecic edge-sum)
//   - nnso    = NNSO(G)    = Σ_{uv∈E} (S_u²+S_v²)^14      (exact u64; S-Octacosic Sombor, α=28)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NEICTC(G) = Σ_v S(v)^20
//     S-Eicosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46).
//     NEICTC = n·S^20 for S-regular.
//     Overflow: S^20 ≤ 16129^20 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHEICTC(G) = Σ_{uv∈E} (S_u+S_v)^19
//     S-Nonadecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46).
//     NHEICTC = |E|·(2S)^19 = 524288|E|·S^19 for S-regular.
//     Overflow per edge: (2×16129)^19 → saturating u128 accumulator.
//
//   NNSO(G) = Σ_{uv∈E} (S_u²+S_v²)^14
//     S-Octacosic Sombor: generalised Sombor SO^α with α=28 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28) — exact, no isqrt.
//     NNSO = |E|·(2S²)^14 = 16384|E|·S^28 for S-regular.
//     Overflow per edge: (2×16129²)^14 → saturating u128 accumulator;
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
//  Graph     NEICTC(exact)              NHEICTC(exact)               NNSO(exact)              edges  nodes
//  Empty                  0                            0                         0               0      0
//  1 node                 0                            0                         0               0      1
//  K₂                     2                      524_288                    16_384               1      2
//  P₃               3_145_728              549_755_813_888           8_796_093_022_208            2      3
//  K₃         3_298_534_883_328   432_345_564_227_567_616          u64::MAX(sat.)               3      3
//  K_{1,4}    5_497_558_138_880   576_460_752_303_423_488          u64::MAX(sat.)               4      5
//  P₄             6_975_665_954       647_506_712_666_746     382_688_120_353_479_602            3      4
//  K₄          u64::MAX(sat.)          u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated             0                            0                         0               0      2
//  K_{2,3}   18_280_792_200_314_880    u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NEICTC:  1^20 + 1^20 = 2. ✓
//     NHEICTC: (1+1)^19 = 2^19 = 524_288. ✓
//     NNSO:    (1²+1²)^14 = 2^14 = 16_384. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NEICTC:  3×2^20 = 3×1_048_576 = 3_145_728. ✓
//       (2^19=524_288; 2^20=2×524_288=1_048_576)
//     NHEICTC: 2×(2+2)^19 = 2×4^19 = 2×274_877_906_944 = 549_755_813_888. ✓
//       (4^18=68_719_476_736; 4^19=4×68_719_476_736=274_877_906_944)
//     NNSO:    2×(4+4)^14 = 2×8^14 = 2×4_398_046_511_104 = 8_796_093_022_208. ✓
//       (8^13=549_755_813_888; 8^14=8×549_755_813_888=4_398_046_511_104)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NEICTC:  3×4^20 = 3×1_099_511_627_776 = 3_298_534_883_328. ✓
//       (4^19=274_877_906_944; 4^20=4×274_877_906_944=1_099_511_627_776)
//     NHEICTC: 3×(4+4)^19 = 3×8^19 = 3×144_115_188_075_855_872 = 432_345_564_227_567_616. ✓
//       (8^18=18_014_398_509_481_984; 8^19=8×18_014_398_509_481_984=144_115_188_075_855_872)
//     NNSO:    3×(16+16)^14 = 3×32^14 → SATURATES to u64::MAX. ✓
//       (32^7=34_359_738_368; 32^14=34_359_738_368^2≈1.18×10²¹ >> u64::MAX)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NEICTC:  5×4^20 = 5×1_099_511_627_776 = 5_497_558_138_880. ✓
//     NHEICTC: 4×8^19 = 4×144_115_188_075_855_872 = 576_460_752_303_423_488. ✓
//     NNSO:    4×32^14 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHEICTC and per-edge NNSO basis.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NEICTC:  2^20+3^20+3^20+2^20 = 1_048_576+3_486_784_401+3_486_784_401+1_048_576 = 6_975_665_954. ✓
//       (3^19=1_162_261_467; 3^20=3×1_162_261_467=3_486_784_401)
//     NHEICTC: 5^19+6^19+5^19
//       5^19: 5^18=3_814_697_265_625; 5^19=5×3_814_697_265_625=19_073_486_328_125
//       6^19: 6^18=101_559_956_668_416; 6^19=6×101_559_956_668_416=609_359_740_010_496
//       19_073_486_328_125+609_359_740_010_496+19_073_486_328_125 = 647_506_712_666_746. ✓
//     NNSO:    13^14+18^14+13^14
//       (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//       13^14: 13^13=302_875_106_592_253; 13^14=13×302_875_106_592_253=3_937_376_385_699_289
//       18^14: 18^13=20_822_964_865_671_168; 18^14=18×20_822_964_865_671_168=374_813_367_582_081_024
//       3_937_376_385_699_289+374_813_367_582_081_024+3_937_376_385_699_289 = 382_688_120_353_479_602. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NEICTC:  4×9^20 = 4×12_157_665_459_056_928_801 → SATURATES to u64::MAX.
//       (9^19=1_350_851_717_672_992_089; 9^20=9×1_350_851_717_672_992_089=12_157_665_459_056_928_801;
//        4×12_157_665_459_056_928_801=48_630_661_836_227_715_204 > u64::MAX) ✓
//     NHEICTC: 6×18^19 → SATURATES to u64::MAX.
//       (18^16>u64::MAX per-edge; 18^19>>u64::MAX) ✓
//     NNSO:    6×162^14 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NEICTC:  5×6^20 = 5×3_656_158_440_062_976 = 18_280_792_200_314_880 (fits u64). ✓
//       (6^19=609_359_740_010_496; 6^20=6×609_359_740_010_496=3_656_158_440_062_976)
//     NHEICTC: 6×12^19 → SATURATES to u64::MAX.
//       (12^18=26_623_333_280_885_243_904 > u64::MAX per-edge; 12^19>>u64::MAX) ✓
//     NNSO:    6×72^14 → SATURATES to u64::MAX (per-edge >> u64::MAX). ✓
//       (72^7≈1.003×10¹³; 72^14≈10²⁶ >> u64::MAX per-edge)
//
// S-REGULAR FORMULA VERIFICATION:
//   NEICTC  = n·S^20                               for S-regular ✓
//   NHEICTC = |E|·(2S)^19 = 524288|E|·S^19        for S-regular ✓
//   NNSO    = |E|·(2S²)^14 = 16384|E|·S^28        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 524_288, 16_384, 1, 2)
//  4.  Path P₃ = A-B-C                   → (3_145_728, 549_755_813_888, 8_796_093_022_208, 2, 3)
//  5.  Triangle K₃                       → (3_298_534_883_328, 432_345_564_227_567_616, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (5_497_558_138_880, 576_460_752_303_423_488, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (6_975_665_954, 647_506_712_666_746, 382_688_120_353_479_602, 3, 4)
//  8.  Complete K₄                       → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (18_280_792_200_314_880, u64::MAX, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T46_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_46");
const T46_EXEC:   ExecutorId = ExecutorId::from_ascii("t46.exec");

const T46_KEY_A: &str = "t46.alpha";
const T46_KEY_B: &str = "t46.beta";
const T46_KEY_C: &str = "t46.gamma";
const T46_KEY_D: &str = "t46.delta";
const T46_KEY_E: &str = "t46.epsilon";

const T46_ID_A: NodeId = derive_node_id(T46_PLUGIN, T46_KEY_A);
const T46_ID_B: NodeId = derive_node_id(T46_PLUGIN, T46_KEY_B);
const T46_ID_C: NodeId = derive_node_id(T46_PLUGIN, T46_KEY_C);
const T46_ID_D: NodeId = derive_node_id(T46_PLUGIN, T46_KEY_D);
const T46_ID_E: NodeId = derive_node_id(T46_PLUGIN, T46_KEY_E);

// L4=133 namespace for this harness.
const T46_VEC_A: VectorAddress = VectorAddress::new(133, 1, 1, 0);
const T46_VEC_B: VectorAddress = VectorAddress::new(133, 1, 2, 0);
const T46_VEC_C: VectorAddress = VectorAddress::new(133, 1, 3, 0);
const T46_VEC_D: VectorAddress = VectorAddress::new(133, 2, 1, 0);
const T46_VEC_E: VectorAddress = VectorAddress::new(133, 2, 2, 0);

const T46_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T46_PLUGIN,
    name:         "kl-graph-topo46-harness",
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
        executor_id:       T46_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T46_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T46_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      0, "empty: node_count=0");
    assert_eq!(ec,      0, "empty: edge_count=0");
    assert_eq!(neictc,  0, "empty: NEICTC=0");
    assert_eq!(nheictc, 0, "empty: NHEICTC=0");
    assert_eq!(nnso,    0, "empty: NNSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NEICTC: 0^20=0; NHEICTC: no edges; NNSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      1, "single: node_count=1");
    assert_eq!(ec,      0, "single: no edges");
    assert_eq!(neictc,  0, "single: NEICTC=0 (S=0; 0^20=0)");
    assert_eq!(nheictc, 0, "single: NHEICTC=0 (no edges)");
    assert_eq!(nnso,    0, "single: NNSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NEICTC:  1^20+1^20 = 2.
// NHEICTC: (1+1)^19 = 2^19 = 524_288.
// NNSO:    (1²+1²)^14 = 2^14 = 16_384.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);
    add_edge(T46_ID_A, T46_ID_B, "t46.e.ab");

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      2,       "k2: node_count=2");
    assert_eq!(ec,      1,       "k2: edge_count=1");
    assert_eq!(neictc,  2,       "k2: NEICTC=2 (1\u{00b2}\u{2070}+1\u{00b2}\u{2070}=2; S-uniform S=1)");
    assert_eq!(nheictc, 524_288, "k2: NHEICTC=524_288 ((1+1)\u{00b9}\u{2079}=2\u{00b9}\u{2079}=524_288; S-uniform S=1)");
    assert_eq!(nnso,    16_384,  "k2: NNSO=16_384 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{2074}=2\u{00b9}\u{2074}=16_384; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NEICTC:  3×2^20 = 3×1_048_576 = 3_145_728.
// NHEICTC: 2×(2+2)^19 = 2×4^19 = 2×274_877_906_944 = 549_755_813_888.
// NNSO:    2×(4+4)^14 = 2×8^14 = 2×4_398_046_511_104 = 8_796_093_022_208.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);
    add_node(T46_VEC_C, T46_KEY_C, T46_ID_C);
    add_edge(T46_ID_A, T46_ID_B, "t46.e.ab");
    add_edge(T46_ID_B, T46_ID_C, "t46.e.bc");

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      3,                 "p3: node_count=3");
    assert_eq!(ec,      2,                 "p3: edge_count=2");
    assert_eq!(neictc,  3_145_728,         "p3: NEICTC=3_145_728 (3\u{00d7}1_048_576; 2\u{00b2}\u{2070}=1_048_576; S-uniform S=2)");
    assert_eq!(nheictc, 549_755_813_888,   "p3: NHEICTC=549_755_813_888 (2\u{00d7}274_877_906_944; (2+2)\u{00b9}\u{2079}=4\u{00b9}\u{2079}=274_877_906_944; S-uniform S=2)");
    assert_eq!(nnso,    8_796_093_022_208, "p3: NNSO=8_796_093_022_208 (2\u{00d7}4_398_046_511_104; (4+4)\u{00b9}\u{2074}=8\u{00b9}\u{2074}=4_398_046_511_104; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NEICTC:  3×4^20 = 3×1_099_511_627_776 = 3_298_534_883_328.
// NHEICTC: 3×(4+4)^19 = 3×8^19 = 3×144_115_188_075_855_872 = 432_345_564_227_567_616.
// NNSO:    3×(16+16)^14 = 3×32^14 → SATURATES to u64::MAX.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);
    add_node(T46_VEC_C, T46_KEY_C, T46_ID_C);
    add_edge(T46_ID_A, T46_ID_B, "t46.e.ab");
    add_edge(T46_ID_B, T46_ID_A, "t46.e.ba");
    add_edge(T46_ID_B, T46_ID_C, "t46.e.bc");
    add_edge(T46_ID_C, T46_ID_B, "t46.e.cb");
    add_edge(T46_ID_A, T46_ID_C, "t46.e.ac");
    add_edge(T46_ID_C, T46_ID_A, "t46.e.ca");

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      3,                         "k3: node_count=3");
    assert_eq!(ec,      3,                         "k3: edge_count=3");
    assert_eq!(neictc,  3_298_534_883_328,         "k3: NEICTC=3_298_534_883_328 (3\u{00d7}1_099_511_627_776; 4\u{00b2}\u{2070}=1_099_511_627_776; S-uniform S=4)");
    assert_eq!(nheictc, 432_345_564_227_567_616,   "k3: NHEICTC=432_345_564_227_567_616 (3\u{00d7}144_115_188_075_855_872; (4+4)\u{00b9}\u{2079}=8\u{00b9}\u{2079}=144_115_188_075_855_872; S-uniform S=4)");
    assert_eq!(nnso,    u64::MAX,                  "k3: NNSO=u64::MAX (3\u{00d7}32\u{00b9}\u{2074} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NEICTC:  5×4^20 = 5×1_099_511_627_776 = 5_497_558_138_880.
// NHEICTC: 4×8^19 = 4×144_115_188_075_855_872 = 576_460_752_303_423_488.
// NNSO:    4×32^14 → SATURATES to u64::MAX.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);
    add_node(T46_VEC_C, T46_KEY_C, T46_ID_C);
    add_node(T46_VEC_D, T46_KEY_D, T46_ID_D);
    add_node(T46_VEC_E, T46_KEY_E, T46_ID_E);
    add_edge(T46_ID_A, T46_ID_B, "t46.e.ab");
    add_edge(T46_ID_A, T46_ID_C, "t46.e.ac");
    add_edge(T46_ID_A, T46_ID_D, "t46.e.ad");
    add_edge(T46_ID_A, T46_ID_E, "t46.e.ae");

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      5,                         "star: node_count=5");
    assert_eq!(ec,      4,                         "star: edge_count=4");
    assert_eq!(neictc,  5_497_558_138_880,         "star: NEICTC=5_497_558_138_880 (5\u{00d7}1_099_511_627_776; same S as K\u{2083})");
    assert_eq!(nheictc, 576_460_752_303_423_488,   "star: NHEICTC=576_460_752_303_423_488 (4\u{00d7}144_115_188_075_855_872; same per-edge as K\u{2083})");
    assert_eq!(nnso,    u64::MAX,                  "star: NNSO=u64::MAX (4\u{00d7}32\u{00b9}\u{2074} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NEICTC:  2^20+3^20+3^20+2^20 = 1_048_576+3_486_784_401+3_486_784_401+1_048_576 = 6_975_665_954.
// NHEICTC: 5^19+6^19+5^19
//   = 19_073_486_328_125+609_359_740_010_496+19_073_486_328_125 = 647_506_712_666_746.
// NNSO:    13^14+18^14+13^14
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//   = 3_937_376_385_699_289+374_813_367_582_081_024+3_937_376_385_699_289 = 382_688_120_353_479_602.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);
    add_node(T46_VEC_C, T46_KEY_C, T46_ID_C);
    add_node(T46_VEC_D, T46_KEY_D, T46_ID_D);
    add_edge(T46_ID_A, T46_ID_B, "t46.e.ab");
    add_edge(T46_ID_B, T46_ID_C, "t46.e.bc");
    add_edge(T46_ID_C, T46_ID_D, "t46.e.cd");

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      4,                           "p4: node_count=4");
    assert_eq!(ec,      3,                           "p4: edge_count=3");
    assert_eq!(neictc,  6_975_665_954,               "p4: NEICTC=6_975_665_954 (1_048_576+3_486_784_401+3_486_784_401+1_048_576; 2\u{00b2}\u{2070}+3\u{00b2}\u{2070}+3\u{00b2}\u{2070}+2\u{00b2}\u{2070})");
    assert_eq!(nheictc, 647_506_712_666_746,         "p4: NHEICTC=647_506_712_666_746 (19_073_486_328_125+609_359_740_010_496+19_073_486_328_125; 5\u{00b9}\u{2079}+6\u{00b9}\u{2079}+5\u{00b9}\u{2079})");
    assert_eq!(nnso,    382_688_120_353_479_602,     "p4: NNSO=382_688_120_353_479_602 (3_937_376_385_699_289+374_813_367_582_081_024+3_937_376_385_699_289; 13\u{00b9}\u{2074}+18\u{00b9}\u{2074}+13\u{00b9}\u{2074})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NEICTC:  4×9^20 = 4×12_157_665_459_056_928_801 → SATURATES → u64::MAX.
//   (9^19=1_350_851_717_672_992_089; 9^20=9×1_350_851_717_672_992_089=12_157_665_459_056_928_801;
//    4×12_157_665_459_056_928_801 > u64::MAX)
// NHEICTC: 6×18^19 → SATURATES → u64::MAX.
//   (18^16>u64::MAX per-edge)
// NNSO:    6×162^14 → SATURATES → u64::MAX (per-edge already >> u64::MAX).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);
    add_node(T46_VEC_C, T46_KEY_C, T46_ID_C);
    add_node(T46_VEC_D, T46_KEY_D, T46_ID_D);
    add_edge(T46_ID_A, T46_ID_B, "t46.e.ab");
    add_edge(T46_ID_B, T46_ID_A, "t46.e.ba");
    add_edge(T46_ID_A, T46_ID_C, "t46.e.ac");
    add_edge(T46_ID_C, T46_ID_A, "t46.e.ca");
    add_edge(T46_ID_A, T46_ID_D, "t46.e.ad");
    add_edge(T46_ID_D, T46_ID_A, "t46.e.da");
    add_edge(T46_ID_B, T46_ID_C, "t46.e.bc");
    add_edge(T46_ID_C, T46_ID_B, "t46.e.cb");
    add_edge(T46_ID_B, T46_ID_D, "t46.e.bd");
    add_edge(T46_ID_D, T46_ID_B, "t46.e.db");
    add_edge(T46_ID_C, T46_ID_D, "t46.e.cd");
    add_edge(T46_ID_D, T46_ID_C, "t46.e.dc");

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      4,        "k4: node_count=4");
    assert_eq!(ec,      6,        "k4: edge_count=6");
    assert_eq!(neictc,  u64::MAX, "k4: NEICTC=u64::MAX (4\u{00d7}9\u{00b2}\u{2070} >> u64::MAX; saturated)");
    assert_eq!(nheictc, u64::MAX, "k4: NHEICTC=u64::MAX (6\u{00d7}18\u{00b9}\u{2079} >> u64::MAX; saturated)");
    assert_eq!(nnso,    u64::MAX, "k4: NNSO=u64::MAX (6\u{00d7}162\u{00b9}\u{2074} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NEICTC=0; NHEICTC=0; NNSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      2, "isolated: node_count=2");
    assert_eq!(ec,      0, "isolated: no edges");
    assert_eq!(neictc,  0, "isolated: NEICTC=0 (S=0; 0^20=0)");
    assert_eq!(nheictc, 0, "isolated: NHEICTC=0 (no edges)");
    assert_eq!(nnso,    0, "isolated: NNSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NEICTC:  5×6^20 = 5×3_656_158_440_062_976 = 18_280_792_200_314_880.
// NHEICTC: 6×12^19 → SATURATES to u64::MAX.
//   (12^18=26_623_333_280_885_243_904 > u64::MAX per-edge)
// NNSO:    6×72^14 → SATURATES to u64::MAX (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T46_VEC_A, T46_KEY_A, T46_ID_A);
    add_node(T46_VEC_B, T46_KEY_B, T46_ID_B);
    add_node(T46_VEC_C, T46_KEY_C, T46_ID_C);
    add_node(T46_VEC_D, T46_KEY_D, T46_ID_D);
    add_node(T46_VEC_E, T46_KEY_E, T46_ID_E);
    add_edge(T46_ID_A, T46_ID_C, "t46.e.ac");
    add_edge(T46_ID_C, T46_ID_A, "t46.e.ca");
    add_edge(T46_ID_A, T46_ID_D, "t46.e.ad");
    add_edge(T46_ID_D, T46_ID_A, "t46.e.da");
    add_edge(T46_ID_A, T46_ID_E, "t46.e.ae");
    add_edge(T46_ID_E, T46_ID_A, "t46.e.ea");
    add_edge(T46_ID_B, T46_ID_C, "t46.e.bc");
    add_edge(T46_ID_C, T46_ID_B, "t46.e.cb");
    add_edge(T46_ID_B, T46_ID_D, "t46.e.bd");
    add_edge(T46_ID_D, T46_ID_B, "t46.e.db");
    add_edge(T46_ID_B, T46_ID_E, "t46.e.be");
    add_edge(T46_ID_E, T46_ID_B, "t46.e.eb");

    let (neictc, nheictc, nnso, ec, nc) = gos_runtime::graph_topo_indices46();
    assert_eq!(nc,      5,                         "k23: node_count=5");
    assert_eq!(ec,      6,                         "k23: edge_count=6");
    assert_eq!(neictc,  18_280_792_200_314_880,    "k23: NEICTC=18_280_792_200_314_880 (5\u{00d7}3_656_158_440_062_976; 6\u{00b2}\u{2070}=3_656_158_440_062_976; S-uniform S=6)");
    assert_eq!(nheictc, u64::MAX,                  "k23: NHEICTC=u64::MAX (6\u{00d7}12\u{00b9}\u{2079}=6\u{00d7}(26_623_333_280_885_243_904\u{00d7}12) >> u64::MAX; per-edge saturates)");
    assert_eq!(nnso,    u64::MAX,                  "k23: NNSO=u64::MAX (6\u{00d7}72\u{00b9}\u{2074} >> u64::MAX; per-edge saturates)");
}
