// gos-graph-topo41-harness — V3.52 NPTC + NHQTC + NIOSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices41()`:
//   Returns (nptc, nhqtc, nioso, edge_count, node_count)
//   - nptc  = NPTC(G)  = Σ_v S(v)^15                    (exact u64; S-Pentadecic vertex sum)
//   - nhqtc = NHQTC(G) = Σ_{uv∈E} (S_u+S_v)^14          (exact u64; S-Tetradecic edge-sum)
//   - nioso = NIOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^9         (exact u64; S-Octadecic Sombor, α=18)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NPTC(G) = Σ_v S(v)^15
//     S-Pentadecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41).
//     NPTC = n·S^15 for S-regular.
//     Overflow: S^15 ≤ 16129^15 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHQTC(G) = Σ_{uv∈E} (S_u+S_v)^14
//     S-Tetradecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41).
//     NHQTC = |E|·(2S)^14 = 16384|E|·S^14 for S-regular.
//     Overflow per edge: (2×16129)^14 → saturating u128 accumulator.
//
//   NIOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^9
//     S-Octadecic Sombor: generalised Sombor SO^α with α=18 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18) — exact, no isqrt.
//     NIOSO = |E|·(2S²)^9 = 512|E|·S^18 for S-regular.
//     Overflow per edge: (2×16129²)^9 → saturating u128 accumulator;
//     K₄ (S=9) saturates → u64::MAX.
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
//  Graph    NPTC(exact)          NHQTC(exact)            NIOSO(exact)        edges  nodes
//  Empty              0                     0                       0             0      0
//  1 node             0                     0                       0             0      1
//  K₂                 2                16_384                     512             1      2
//  P₃            98_304           536_870_912             268_435_456             2      3
//  K₃     3_221_225_472    13_194_139_533_312     105_553_116_266_496             3      3
//  K_{1,4} 5_368_709_120   17_592_186_044_416     140_737_488_355_328             4      5
//  P₄        28_763_350        90_571_195_346         219_568_289_114             3      4
//  K₄   823_564_528_378_596  2_248_880_205_492_486_144  u64::MAX(sat.)            6      4
//  2 isolated         0                     0                       0             0      2
//  K_{2,3} 2_350_924_922_880  7_703_510_787_293_184  311_992_186_885_373_952      6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NPTC:  1^15 + 1^15 = 2. ✓
//     NHQTC: (1+1)^14 = 2^14 = 16_384. ✓
//     NIOSO: (1²+1²)^9 = 2^9 = 512. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NPTC:  3×2^15 = 3×32_768 = 98_304. ✓
//     NHQTC: 2×(2+2)^14 = 2×4^14 = 2×268_435_456 = 536_870_912. ✓
//       (4^7=16_384; 4^14=16_384²=268_435_456)
//     NIOSO: 2×(4+4)^9 = 2×8^9 = 2×134_217_728 = 268_435_456. ✓
//       (8^8=16_777_216; 8^9=134_217_728)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NPTC:  3×4^15 = 3×1_073_741_824 = 3_221_225_472. ✓
//       (4^8=65_536; 4^14=268_435_456 (topo40); 4^15=268_435_456×4=1_073_741_824)
//     NHQTC: 3×(4+4)^14 = 3×8^14 = 3×4_398_046_511_104 = 13_194_139_533_312. ✓
//       (8^13=549_755_813_888 (topo40); 8^14=549_755_813_888×8=4_398_046_511_104)
//     NIOSO: 3×(16+16)^9 = 3×32^9 = 3×35_184_372_088_832 = 105_553_116_266_496. ✓
//       (32^8=1_099_511_627_776 (topo40); 32^9=1_099_511_627_776×32=35_184_372_088_832)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NPTC:  5×4^15 = 5×1_073_741_824 = 5_368_709_120. ✓
//     NHQTC: 4×8^14 = 4×4_398_046_511_104 = 17_592_186_044_416. ✓
//     NIOSO: 4×32^9 = 4×35_184_372_088_832 = 140_737_488_355_328. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHQTC and NIOSO; NPTC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NPTC:  2^15+3^15+3^15+2^15 = 32_768+14_348_907+14_348_907+32_768 = 28_763_350. ✓
//       (2^15=32_768; 3^14=4_782_969 (topo40); 3^15=4_782_969×3=14_348_907)
//     NHQTC: (2+3)^14+(3+3)^14+(3+2)^14 = 5^14+6^14+5^14
//            = 6_103_515_625+78_364_164_096+6_103_515_625 = 90_571_195_346. ✓
//       (5^7=78_125; 5^14=78_125²=6_103_515_625)
//       (6^14=78_364_164_096 (topo40))
//     NIOSO: 13^9+18^9+13^9 = 10_604_499_373+198_359_290_368+10_604_499_373 = 219_568_289_114. ✓
//       (S_A²+S_B²=4+9=13; 13^8=815_730_721 (topo40); 13^9=815_730_721×13=10_604_499_373)
//       (S_B²+S_C²=9+9=18; 18^8=11_019_960_576 (topo40); 18^9=11_019_960_576×18=198_359_290_368)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NPTC:  4×9^15 = 4×205_891_132_094_649 = 823_564_528_378_596. ✓
//       (9^14=22_876_792_454_961 (topo40); 9^15=22_876_792_454_961×9=205_891_132_094_649)
//     NHQTC: 6×18^14 = 6×374_813_367_582_081_024 = 2_248_880_205_492_486_144. ✓
//       (18^13=20_822_964_865_671_168 (topo40); 18^14=18×20_822_964_865_671_168=374_813_367_582_081_024)
//     NIOSO: 6×162^9 — SATURATES to u64::MAX.
//       (162^8=474_373_168_346_071_296 (topo40); 162^9=474_373_168_346_071_296×162=76_848_453_272_063_549_952)
//       76_848_453_272_063_549_952 > u64::MAX (18_446_744_073_709_551_615) → per-edge already saturates.
//       6×76_848_453_272_063_549_952 in u128 = 461_090_719_632_381_299_712 → clamped to u64::MAX. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NPTC:  5×6^15 = 5×470_184_984_576 = 2_350_924_922_880. ✓
//       (6^14=78_364_164_096 (topo40); 6^15=78_364_164_096×6=470_184_984_576)
//     NHQTC: 6×12^14 = 6×1_283_918_464_548_864 = 7_703_510_787_293_184. ✓
//       (12^13=106_993_205_379_072 (topo40); 12^14=106_993_205_379_072×12=1_283_918_464_548_864)
//     NIOSO: 6×72^9 = 6×51_998_697_814_228_992 = 311_992_186_885_373_952. ✓
//       (72^8=722_204_136_308_736 (topo40); 72^9=722_204_136_308_736×72=51_998_697_814_228_992)
//
// S-REGULAR FORMULA VERIFICATION:
//   NPTC  = n·S^15                            for S-regular ✓
//   NHQTC = |E|·(2S)^14 = 16384|E|·S^14      for S-regular ✓
//   NIOSO = |E|·(2S²)^9 = 512|E|·S^18        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 16_384, 512, 1, 2)
//  4.  Path P₃ = A-B-C                   → (98_304, 536_870_912, 268_435_456, 2, 3)
//  5.  Triangle K₃                       → (3_221_225_472, 13_194_139_533_312, 105_553_116_266_496, 3, 3)
//  6.  Star K_{1,4}                      → (5_368_709_120, 17_592_186_044_416, 140_737_488_355_328, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (28_763_350, 90_571_195_346, 219_568_289_114, 3, 4)
//  8.  Complete K₄                       → (823_564_528_378_596, 2_248_880_205_492_486_144, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (2_350_924_922_880, 7_703_510_787_293_184, 311_992_186_885_373_952, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T41_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_41");
const T41_EXEC:   ExecutorId = ExecutorId::from_ascii("t41.exec");

const T41_KEY_A: &str = "t41.alpha";
const T41_KEY_B: &str = "t41.beta";
const T41_KEY_C: &str = "t41.gamma";
const T41_KEY_D: &str = "t41.delta";
const T41_KEY_E: &str = "t41.epsilon";

const T41_ID_A: NodeId = derive_node_id(T41_PLUGIN, T41_KEY_A);
const T41_ID_B: NodeId = derive_node_id(T41_PLUGIN, T41_KEY_B);
const T41_ID_C: NodeId = derive_node_id(T41_PLUGIN, T41_KEY_C);
const T41_ID_D: NodeId = derive_node_id(T41_PLUGIN, T41_KEY_D);
const T41_ID_E: NodeId = derive_node_id(T41_PLUGIN, T41_KEY_E);

// L4=128 namespace for this harness.
const T41_VEC_A: VectorAddress = VectorAddress::new(128, 1, 1, 0);
const T41_VEC_B: VectorAddress = VectorAddress::new(128, 1, 2, 0);
const T41_VEC_C: VectorAddress = VectorAddress::new(128, 1, 3, 0);
const T41_VEC_D: VectorAddress = VectorAddress::new(128, 2, 1, 0);
const T41_VEC_E: VectorAddress = VectorAddress::new(128, 2, 2, 0);

const T41_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T41_PLUGIN,
    name:         "kl-graph-topo41-harness",
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
        executor_id:       T41_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T41_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T41_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    0, "empty: node_count=0");
    assert_eq!(ec,    0, "empty: edge_count=0");
    assert_eq!(nptc,  0, "empty: NPTC=0");
    assert_eq!(nhqtc, 0, "empty: NHQTC=0");
    assert_eq!(nioso, 0, "empty: NIOSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NPTC: 0^15=0; NHQTC: no edges; NIOSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    1, "single: node_count=1");
    assert_eq!(ec,    0, "single: no edges");
    assert_eq!(nptc,  0, "single: NPTC=0 (S=0; 0^15=0)");
    assert_eq!(nhqtc, 0, "single: NHQTC=0 (no edges)");
    assert_eq!(nioso, 0, "single: NIOSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NPTC:  1^15+1^15 = 2.
// NHQTC: (1+1)^14 = 2^14 = 16_384.
// NIOSO: (1²+1²)^9 = 2^9 = 512.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);
    add_edge(T41_ID_A, T41_ID_B, "t41.e.ab");

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    2,      "k2: node_count=2");
    assert_eq!(ec,    1,      "k2: edge_count=1");
    assert_eq!(nptc,  2,      "k2: NPTC=2 (1\u{00b9}\u{2075}+1\u{00b9}\u{2075}=2; S-uniform S=1)");
    assert_eq!(nhqtc, 16_384, "k2: NHQTC=16_384 ((1+1)\u{00b9}\u{2074}=2\u{00b9}\u{2074}=16_384; S-uniform S=1)");
    assert_eq!(nioso, 512,    "k2: NIOSO=512 ((1\u{00b2}+1\u{00b2})\u{2079}=2\u{2079}=512; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NPTC:  3×2^15 = 3×32_768 = 98_304.
// NHQTC: 2×(2+2)^14 = 2×4^14 = 2×268_435_456 = 536_870_912.
// NIOSO: 2×(4+4)^9 = 2×8^9 = 2×134_217_728 = 268_435_456.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);
    add_node(T41_VEC_C, T41_KEY_C, T41_ID_C);
    add_edge(T41_ID_A, T41_ID_B, "t41.e.ab");
    add_edge(T41_ID_B, T41_ID_C, "t41.e.bc");

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    3,           "p3: node_count=3");
    assert_eq!(ec,    2,           "p3: edge_count=2");
    assert_eq!(nptc,  98_304,      "p3: NPTC=98_304 (3\u{00d7}32_768; 2\u{00b9}\u{2075}=32_768; S-uniform S=2)");
    assert_eq!(nhqtc, 536_870_912, "p3: NHQTC=536_870_912 (2\u{00d7}268_435_456; (2+2)\u{00b9}\u{2074}=4\u{00b9}\u{2074}=268_435_456; S-uniform S=2)");
    assert_eq!(nioso, 268_435_456, "p3: NIOSO=268_435_456 (2\u{00d7}134_217_728; (4+4)\u{2079}=8\u{2079}=134_217_728; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NPTC:  3×4^15 = 3×1_073_741_824 = 3_221_225_472.
// NHQTC: 3×(4+4)^14 = 3×8^14 = 3×4_398_046_511_104 = 13_194_139_533_312.
// NIOSO: 3×(16+16)^9 = 3×32^9 = 3×35_184_372_088_832 = 105_553_116_266_496.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);
    add_node(T41_VEC_C, T41_KEY_C, T41_ID_C);
    add_edge(T41_ID_A, T41_ID_B, "t41.e.ab");
    add_edge(T41_ID_B, T41_ID_A, "t41.e.ba");
    add_edge(T41_ID_B, T41_ID_C, "t41.e.bc");
    add_edge(T41_ID_C, T41_ID_B, "t41.e.cb");
    add_edge(T41_ID_A, T41_ID_C, "t41.e.ac");
    add_edge(T41_ID_C, T41_ID_A, "t41.e.ca");

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    3,                    "k3: node_count=3");
    assert_eq!(ec,    3,                    "k3: edge_count=3");
    assert_eq!(nptc,  3_221_225_472,        "k3: NPTC=3_221_225_472 (3\u{00d7}1_073_741_824; 4\u{00b9}\u{2075}=1_073_741_824; S-uniform S=4)");
    assert_eq!(nhqtc, 13_194_139_533_312,   "k3: NHQTC=13_194_139_533_312 (3\u{00d7}4_398_046_511_104; (4+4)\u{00b9}\u{2074}=8\u{00b9}\u{2074}=4_398_046_511_104; S-uniform S=4)");
    assert_eq!(nioso, 105_553_116_266_496,  "k3: NIOSO=105_553_116_266_496 (3\u{00d7}35_184_372_088_832; (16+16)\u{2079}=32\u{2079}=35_184_372_088_832; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHQTC and NIOSO as K₃; NPTC and totals differ by node/edge count.
// NPTC:  5×4^15 = 5×1_073_741_824 = 5_368_709_120.
// NHQTC: 4×8^14 = 4×4_398_046_511_104 = 17_592_186_044_416.
// NIOSO: 4×32^9 = 4×35_184_372_088_832 = 140_737_488_355_328.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);
    add_node(T41_VEC_C, T41_KEY_C, T41_ID_C);
    add_node(T41_VEC_D, T41_KEY_D, T41_ID_D);
    add_node(T41_VEC_E, T41_KEY_E, T41_ID_E);
    add_edge(T41_ID_A, T41_ID_B, "t41.e.ab");
    add_edge(T41_ID_A, T41_ID_C, "t41.e.ac");
    add_edge(T41_ID_A, T41_ID_D, "t41.e.ad");
    add_edge(T41_ID_A, T41_ID_E, "t41.e.ae");

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    5,                    "star: node_count=5");
    assert_eq!(ec,    4,                    "star: edge_count=4");
    assert_eq!(nptc,  5_368_709_120,        "star: NPTC=5_368_709_120 (5\u{00d7}1_073_741_824; same S as K\u{2083})");
    assert_eq!(nhqtc, 17_592_186_044_416,   "star: NHQTC=17_592_186_044_416 (4\u{00d7}4_398_046_511_104; same per-edge as K\u{2083})");
    assert_eq!(nioso, 140_737_488_355_328,  "star: NIOSO=140_737_488_355_328 (4\u{00d7}35_184_372_088_832; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NPTC:  2^15+3^15+3^15+2^15 = 32_768+14_348_907+14_348_907+32_768 = 28_763_350.
// NHQTC: (2+3)^14+(3+3)^14+(3+2)^14 = 5^14+6^14+5^14
//        = 6_103_515_625+78_364_164_096+6_103_515_625 = 90_571_195_346.
// NIOSO: 13^9+18^9+13^9 = 10_604_499_373+198_359_290_368+10_604_499_373 = 219_568_289_114.
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);
    add_node(T41_VEC_C, T41_KEY_C, T41_ID_C);
    add_node(T41_VEC_D, T41_KEY_D, T41_ID_D);
    add_edge(T41_ID_A, T41_ID_B, "t41.e.ab");
    add_edge(T41_ID_B, T41_ID_C, "t41.e.bc");
    add_edge(T41_ID_C, T41_ID_D, "t41.e.cd");

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    4,               "p4: node_count=4");
    assert_eq!(ec,    3,               "p4: edge_count=3");
    assert_eq!(nptc,  28_763_350,      "p4: NPTC=28_763_350 (32_768+14_348_907+14_348_907+32_768; 2\u{00b9}\u{2075}+3\u{00b9}\u{2075}+3\u{00b9}\u{2075}+2\u{00b9}\u{2075})");
    assert_eq!(nhqtc, 90_571_195_346,  "p4: NHQTC=90_571_195_346 (6_103_515_625+78_364_164_096+6_103_515_625; 5\u{00b9}\u{2074}+6\u{00b9}\u{2074}+5\u{00b9}\u{2074})");
    assert_eq!(nioso, 219_568_289_114, "p4: NIOSO=219_568_289_114 (10_604_499_373+198_359_290_368+10_604_499_373; 13\u{2079}+18\u{2079}+13\u{2079})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NPTC:  4×9^15 = 4×205_891_132_094_649 = 823_564_528_378_596.
// NHQTC: 6×18^14 = 6×374_813_367_582_081_024 = 2_248_880_205_492_486_144.
// NIOSO: 6×162^9 → SATURATES → u64::MAX.
//   (162^9 = 76_848_453_272_063_549_952 > u64::MAX per-edge; 6× in u128 also > u64::MAX)

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);
    add_node(T41_VEC_C, T41_KEY_C, T41_ID_C);
    add_node(T41_VEC_D, T41_KEY_D, T41_ID_D);
    add_edge(T41_ID_A, T41_ID_B, "t41.e.ab");
    add_edge(T41_ID_B, T41_ID_A, "t41.e.ba");
    add_edge(T41_ID_A, T41_ID_C, "t41.e.ac");
    add_edge(T41_ID_C, T41_ID_A, "t41.e.ca");
    add_edge(T41_ID_A, T41_ID_D, "t41.e.ad");
    add_edge(T41_ID_D, T41_ID_A, "t41.e.da");
    add_edge(T41_ID_B, T41_ID_C, "t41.e.bc");
    add_edge(T41_ID_C, T41_ID_B, "t41.e.cb");
    add_edge(T41_ID_B, T41_ID_D, "t41.e.bd");
    add_edge(T41_ID_D, T41_ID_B, "t41.e.db");
    add_edge(T41_ID_C, T41_ID_D, "t41.e.cd");
    add_edge(T41_ID_D, T41_ID_C, "t41.e.dc");

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    4,                         "k4: node_count=4");
    assert_eq!(ec,    6,                         "k4: edge_count=6");
    assert_eq!(nptc,  823_564_528_378_596,        "k4: NPTC=823_564_528_378_596 (4\u{00d7}205_891_132_094_649; 9\u{00b9}\u{2075}=205_891_132_094_649; S-uniform S=9)");
    assert_eq!(nhqtc, 2_248_880_205_492_486_144,  "k4: NHQTC=2_248_880_205_492_486_144 (6\u{00d7}374_813_367_582_081_024; 18\u{00b9}\u{2074}=374_813_367_582_081_024; S-uniform S=9)");
    assert_eq!(nioso, u64::MAX,                   "k4: NIOSO=u64::MAX (6\u{00d7}162\u{2079} saturates; 162\u{2079}=76_848_453_272_063_549_952 > u64::MAX per-edge)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NPTC=0; NHQTC=0; NIOSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    2, "isolated: node_count=2");
    assert_eq!(ec,    0, "isolated: no edges");
    assert_eq!(nptc,  0, "isolated: NPTC=0 (S=0; 0^15=0)");
    assert_eq!(nhqtc, 0, "isolated: NHQTC=0 (no edges)");
    assert_eq!(nioso, 0, "isolated: NIOSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NPTC:  5×6^15 = 5×470_184_984_576 = 2_350_924_922_880.
// NHQTC: 6×12^14 = 6×1_283_918_464_548_864 = 7_703_510_787_293_184.
// NIOSO: 6×72^9 = 6×51_998_697_814_228_992 = 311_992_186_885_373_952.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T41_VEC_A, T41_KEY_A, T41_ID_A);
    add_node(T41_VEC_B, T41_KEY_B, T41_ID_B);
    add_node(T41_VEC_C, T41_KEY_C, T41_ID_C);
    add_node(T41_VEC_D, T41_KEY_D, T41_ID_D);
    add_node(T41_VEC_E, T41_KEY_E, T41_ID_E);
    add_edge(T41_ID_A, T41_ID_C, "t41.e.ac");
    add_edge(T41_ID_C, T41_ID_A, "t41.e.ca");
    add_edge(T41_ID_A, T41_ID_D, "t41.e.ad");
    add_edge(T41_ID_D, T41_ID_A, "t41.e.da");
    add_edge(T41_ID_A, T41_ID_E, "t41.e.ae");
    add_edge(T41_ID_E, T41_ID_A, "t41.e.ea");
    add_edge(T41_ID_B, T41_ID_C, "t41.e.bc");
    add_edge(T41_ID_C, T41_ID_B, "t41.e.cb");
    add_edge(T41_ID_B, T41_ID_D, "t41.e.bd");
    add_edge(T41_ID_D, T41_ID_B, "t41.e.db");
    add_edge(T41_ID_B, T41_ID_E, "t41.e.be");
    add_edge(T41_ID_E, T41_ID_B, "t41.e.eb");

    let (nptc, nhqtc, nioso, ec, nc) = gos_runtime::graph_topo_indices41();
    assert_eq!(nc,    5,                       "k23: node_count=5");
    assert_eq!(ec,    6,                       "k23: edge_count=6");
    assert_eq!(nptc,  2_350_924_922_880,       "k23: NPTC=2_350_924_922_880 (5\u{00d7}470_184_984_576; 6\u{00b9}\u{2075}=470_184_984_576; S-uniform S=6)");
    assert_eq!(nhqtc, 7_703_510_787_293_184,   "k23: NHQTC=7_703_510_787_293_184 (6\u{00d7}1_283_918_464_548_864; 12\u{00b9}\u{2074}=1_283_918_464_548_864; S-uniform S=6)");
    assert_eq!(nioso, 311_992_186_885_373_952, "k23: NIOSO=311_992_186_885_373_952 (6\u{00d7}51_998_697_814_228_992; 72\u{2079}=51_998_697_814_228_992; S-uniform S=6)");
}
