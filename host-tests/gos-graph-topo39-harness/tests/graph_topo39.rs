// gos-graph-topo39-harness — V3.50 NTC + NHDOC + NESO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices39()`:
//   Returns (ntc, nhdoc, neso, edge_count, node_count)
//   - ntc   = NTC(G)   = Σ_v S(v)^13                   (exact u64; S-Tridecic vertex sum)
//   - nhdoc = NHDOC(G) = Σ_{uv∈E} (S_u+S_v)^12         (exact u64; S-Dodecic edge-sum)
//   - neso  = NESO(G)  = Σ_{uv∈E} (S_u²+S_v²)^7        (exact u64; S-Tetradecic Sombor, α=14)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTC(G) = Σ_v S(v)^13
//     S-Tridecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39).
//     NTC = n·S^13 for S-regular.
//     Overflow: S^13 ≤ 16129^13 ≈ 6.1×10^53 > u128::MAX → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHDOC(G) = Σ_{uv∈E} (S_u+S_v)^12
//     S-Dodecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39).
//     NHDOC = |E|·(2S)^12 = 4096|E|S^12 for S-regular.
//     Overflow per edge: (2×16129)^12 ≈ 2.9×10^52 > u128::MAX → saturating u128 accumulator.
//
//   NESO(G) = Σ_{uv∈E} (S_u²+S_v²)^7
//     S-Tetradecic Sombor: generalised Sombor SO^α with α=14 on S-variant.
//     NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
//     NFSO(topo34)=Σ(S²+S²)^2 (α=4), NHSO(topo35)=Σ(S²+S²)^3 (α=6),
//     NOSO(topo36)=Σ(S²+S²)^4 (α=8), NTSO(topo37)=Σ(S²+S²)^5 (α=10),
//     NDSO(topo38)=Σ(S²+S²)^6 (α=12), NESO(topo39)=Σ(S²+S²)^7 (α=14) — exact, no isqrt.
//     NESO = |E|·(2S²)^7 = 128|E|S^14 for S-regular.
//     Overflow per edge: (2×16129²)^7 ≈ 2.7×10^61 > u128::MAX → saturating u128 accumulator.
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
//  Graph       NTC(exact)               NHDOC(exact)              NESO(exact)       edges  nodes
//  Empty                    0                       0                        0           0      0
//  1 node                   0                       0                        0           0      1
//  K₂                       2                   4_096                      128           1      2
//  P₃                  24_576              33_554_432                4_194_304           2      3
//  K₃             201_326_592         206_158_430_208          103_079_215_104           3      3
//  K_{1,4}        335_544_320         274_877_906_944          137_438_953_472           4      5
//  P₄               3_205_030           2_665_063_586              737_717_066           3      4
//  K₄      10_167_463_313_316   6_940_988_288_557_056   17_569_376_605_410_048           6      4
//  2 isolated               0                       0                        0           0      2
//  K_{2,3}     65_303_470_080      53_496_602_689_536       60_183_678_025_728           6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTC:   1^13 + 1^13 = 2. ✓
//     NHDOC: (1+1)^12 = 2^12 = 4_096. ✓
//     NESO:  (1²+1²)^7 = 2^7 = 128. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTC:   3×2^13 = 3×8_192 = 24_576. ✓
//     NHDOC: 2×(2+2)^12 = 2×4^12 = 2×16_777_216 = 33_554_432. ✓
//     NESO:  2×(4+4)^7 = 2×8^7 = 2×2_097_152 = 4_194_304. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTC:   3×4^13 = 3×67_108_864 = 201_326_592. ✓
//       (4^12=16_777_216; 4^13=16_777_216×4=67_108_864)
//     NHDOC: 3×(4+4)^12 = 3×8^12 = 3×68_719_476_736 = 206_158_430_208. ✓
//       (8^11=8_589_934_592; 8^12=8_589_934_592×8=68_719_476_736)
//     NESO:  3×(16+16)^7 = 3×32^7 = 3×34_359_738_368 = 103_079_215_104. ✓
//       (32^6=1_073_741_824; 32^7=1_073_741_824×32=34_359_738_368)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTC:   5×4^13 = 5×67_108_864 = 335_544_320. ✓
//     NHDOC: 4×8^12 = 4×68_719_476_736 = 274_877_906_944. ✓
//     NESO:  4×32^7 = 4×34_359_738_368 = 137_438_953_472. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHDOC and NESO; NTC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTC:   2^13+3^13+3^13+2^13 = 8_192+1_594_323+1_594_323+8_192 = 3_205_030. ✓
//       (3^12=531_441; 3^13=531_441×3=1_594_323)
//     NHDOC: 5^12+6^12+5^12 = 244_140_625+2_176_782_336+244_140_625 = 2_665_063_586. ✓
//       (5^11=48_828_125; 5^12=48_828_125×5=244_140_625)
//       (6^11=362_797_056; 6^12=362_797_056×6=2_176_782_336)
//     NESO:  13^7+18^7+13^7 = 62_748_517+612_220_032+62_748_517 = 737_717_066. ✓
//       (S_A²+S_B²=4+9=13; 13^6=4_826_809; 13^7=4_826_809×13=62_748_517)
//       (S_B²+S_C²=9+9=18; 18^6=34_012_224; 18^7=34_012_224×18=612_220_032)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTC:   4×9^13 = 4×2_541_865_828_329 = 10_167_463_313_316. ✓
//       (9^12=282_429_536_481; 9^13=282_429_536_481×9=2_541_865_828_329)
//     NHDOC: 6×18^12 = 6×1_156_831_381_426_176 = 6_940_988_288_557_056. ✓
//       (18^11=64_268_410_079_232; 18^12=64_268_410_079_232×18=1_156_831_381_426_176)
//     NESO:  6×162^7 = 6×2_928_229_434_235_008 = 17_569_376_605_410_048. ✓
//       (162^6=18_075_490_334_784; 162^7=18_075_490_334_784×162=2_928_229_434_235_008)
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTC:   5×6^13 = 5×13_060_694_016 = 65_303_470_080. ✓
//       (6^12=2_176_782_336; 6^13=2_176_782_336×6=13_060_694_016)
//     NHDOC: 6×12^12 = 6×8_916_100_448_256 = 53_496_602_689_536. ✓
//       (12^11=743_008_370_688; 12^12=743_008_370_688×12=8_916_100_448_256)
//     NESO:  6×72^7 = 6×10_030_613_004_288 = 60_183_678_025_728. ✓
//       (72^6=139_314_069_504; 72^7=139_314_069_504×72=10_030_613_004_288)
//
// S-REGULAR FORMULA VERIFICATION:
//   NTC   = n·S^13                           for S-regular ✓
//   NHDOC = |E|·(2S)^12 = 4096|E|·S^12      for S-regular ✓
//   NESO  = |E|·(2S²)^7 = 128|E|·S^14       for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 4_096, 128, 1, 2)
//  4.  Path P₃ = A-B-C                   → (24_576, 33_554_432, 4_194_304, 2, 3)
//  5.  Triangle K₃                       → (201_326_592, 206_158_430_208, 103_079_215_104, 3, 3)
//  6.  Star K_{1,4}                      → (335_544_320, 274_877_906_944, 137_438_953_472, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (3_205_030, 2_665_063_586, 737_717_066, 3, 4)
//  8.  Complete K₄                       → (10_167_463_313_316, 6_940_988_288_557_056, 17_569_376_605_410_048, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (65_303_470_080, 53_496_602_689_536, 60_183_678_025_728, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T39_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_39");
const T39_EXEC:   ExecutorId = ExecutorId::from_ascii("t39.exec");

const T39_KEY_A: &str = "t39.alpha";
const T39_KEY_B: &str = "t39.beta";
const T39_KEY_C: &str = "t39.gamma";
const T39_KEY_D: &str = "t39.delta";
const T39_KEY_E: &str = "t39.epsilon";

const T39_ID_A: NodeId = derive_node_id(T39_PLUGIN, T39_KEY_A);
const T39_ID_B: NodeId = derive_node_id(T39_PLUGIN, T39_KEY_B);
const T39_ID_C: NodeId = derive_node_id(T39_PLUGIN, T39_KEY_C);
const T39_ID_D: NodeId = derive_node_id(T39_PLUGIN, T39_KEY_D);
const T39_ID_E: NodeId = derive_node_id(T39_PLUGIN, T39_KEY_E);

// L4=126 namespace for this harness.
const T39_VEC_A: VectorAddress = VectorAddress::new(126, 1, 1, 0);
const T39_VEC_B: VectorAddress = VectorAddress::new(126, 1, 2, 0);
const T39_VEC_C: VectorAddress = VectorAddress::new(126, 1, 3, 0);
const T39_VEC_D: VectorAddress = VectorAddress::new(126, 2, 1, 0);
const T39_VEC_E: VectorAddress = VectorAddress::new(126, 2, 2, 0);

const T39_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T39_PLUGIN,
    name:         "kl-graph-topo39-harness",
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
        executor_id:       T39_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T39_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T39_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    0, "empty: node_count=0");
    assert_eq!(ec,    0, "empty: edge_count=0");
    assert_eq!(ntc,   0, "empty: NTC=0");
    assert_eq!(nhdoc, 0, "empty: NHDOC=0");
    assert_eq!(neso,  0, "empty: NESO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NTC: 0^13=0; NHDOC: no edges; NESO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    1, "single: node_count=1");
    assert_eq!(ec,    0, "single: no edges");
    assert_eq!(ntc,   0, "single: NTC=0 (S=0; 0^13=0)");
    assert_eq!(nhdoc, 0, "single: NHDOC=0 (no edges)");
    assert_eq!(neso,  0, "single: NESO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NTC:   1^13+1^13 = 2.
// NHDOC: (1+1)^12 = 2^12 = 4_096.
// NESO:  (1²+1²)^7 = 2^7 = 128.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);
    add_edge(T39_ID_A, T39_ID_B, "t39.e.ab");

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    2,     "k2: node_count=2");
    assert_eq!(ec,    1,     "k2: edge_count=1");
    assert_eq!(ntc,   2,     "k2: NTC=2 (1\u{00b9}\u{00b3}+1\u{00b9}\u{00b3}=2; S-uniform S=1)");
    assert_eq!(nhdoc, 4_096, "k2: NHDOC=4_096 ((1+1)\u{00b9}\u{00b2}=2\u{00b9}\u{00b2}=4_096; S-uniform S=1)");
    assert_eq!(neso,  128,   "k2: NESO=128 ((1\u{00b2}+1\u{00b2})\u{2077}=2\u{2077}=128; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NTC:   3×2^13 = 3×8_192 = 24_576.
// NHDOC: 2×(2+2)^12 = 2×4^12 = 2×16_777_216 = 33_554_432.
// NESO:  2×(4+4)^7 = 2×8^7 = 2×2_097_152 = 4_194_304.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);
    add_node(T39_VEC_C, T39_KEY_C, T39_ID_C);
    add_edge(T39_ID_A, T39_ID_B, "t39.e.ab");
    add_edge(T39_ID_B, T39_ID_C, "t39.e.bc");

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    3,          "p3: node_count=3");
    assert_eq!(ec,    2,          "p3: edge_count=2");
    assert_eq!(ntc,   24_576,     "p3: NTC=24_576 (3\u{00d7}8_192; 2\u{00b9}\u{00b3}=8_192; S-uniform S=2)");
    assert_eq!(nhdoc, 33_554_432, "p3: NHDOC=33_554_432 (2\u{00d7}16_777_216; (2+2)\u{00b9}\u{00b2}=4\u{00b9}\u{00b2}=16_777_216; S-uniform S=2)");
    assert_eq!(neso,  4_194_304,  "p3: NESO=4_194_304 (2\u{00d7}2_097_152; (4+4)\u{2077}=8\u{2077}=2_097_152; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NTC:   3×4^13 = 3×67_108_864 = 201_326_592.
// NHDOC: 3×(4+4)^12 = 3×8^12 = 3×68_719_476_736 = 206_158_430_208.
// NESO:  3×(16+16)^7 = 3×32^7 = 3×34_359_738_368 = 103_079_215_104.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);
    add_node(T39_VEC_C, T39_KEY_C, T39_ID_C);
    add_edge(T39_ID_A, T39_ID_B, "t39.e.ab");
    add_edge(T39_ID_B, T39_ID_A, "t39.e.ba");
    add_edge(T39_ID_B, T39_ID_C, "t39.e.bc");
    add_edge(T39_ID_C, T39_ID_B, "t39.e.cb");
    add_edge(T39_ID_A, T39_ID_C, "t39.e.ac");
    add_edge(T39_ID_C, T39_ID_A, "t39.e.ca");

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    3,               "k3: node_count=3");
    assert_eq!(ec,    3,               "k3: edge_count=3");
    assert_eq!(ntc,   201_326_592,     "k3: NTC=201_326_592 (3\u{00d7}67_108_864; 4\u{00b9}\u{00b3}=67_108_864; S-uniform S=4)");
    assert_eq!(nhdoc, 206_158_430_208, "k3: NHDOC=206_158_430_208 (3\u{00d7}68_719_476_736; (4+4)\u{00b9}\u{00b2}=8\u{00b9}\u{00b2}=68_719_476_736; S-uniform S=4)");
    assert_eq!(neso,  103_079_215_104, "k3: NESO=103_079_215_104 (3\u{00d7}34_359_738_368; (16+16)\u{2077}=32\u{2077}=34_359_738_368; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHDOC (68_719_476_736) and NESO (34_359_738_368) as K₃; NTC and totals differ.
// NTC:   5×4^13 = 5×67_108_864 = 335_544_320.
// NHDOC: 4×8^12 = 4×68_719_476_736 = 274_877_906_944.
// NESO:  4×32^7 = 4×34_359_738_368 = 137_438_953_472.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);
    add_node(T39_VEC_C, T39_KEY_C, T39_ID_C);
    add_node(T39_VEC_D, T39_KEY_D, T39_ID_D);
    add_node(T39_VEC_E, T39_KEY_E, T39_ID_E);
    add_edge(T39_ID_A, T39_ID_B, "t39.e.ab");
    add_edge(T39_ID_A, T39_ID_C, "t39.e.ac");
    add_edge(T39_ID_A, T39_ID_D, "t39.e.ad");
    add_edge(T39_ID_A, T39_ID_E, "t39.e.ae");

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    5,               "star: node_count=5");
    assert_eq!(ec,    4,               "star: edge_count=4");
    assert_eq!(ntc,   335_544_320,     "star: NTC=335_544_320 (5\u{00d7}67_108_864; same S as K\u{2083})");
    assert_eq!(nhdoc, 274_877_906_944, "star: NHDOC=274_877_906_944 (4\u{00d7}68_719_476_736; same per-edge as K\u{2083})");
    assert_eq!(neso,  137_438_953_472, "star: NESO=137_438_953_472 (4\u{00d7}34_359_738_368; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NTC:   2^13+3^13+3^13+2^13 = 8_192+1_594_323+1_594_323+8_192 = 3_205_030.
// NHDOC: (2+3)^12+(3+3)^12+(3+2)^12 = 5^12+6^12+5^12 = 244_140_625+2_176_782_336+244_140_625 = 2_665_063_586.
// NESO:  13^7+18^7+13^7 = 62_748_517+612_220_032+62_748_517 = 737_717_066.
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);
    add_node(T39_VEC_C, T39_KEY_C, T39_ID_C);
    add_node(T39_VEC_D, T39_KEY_D, T39_ID_D);
    add_edge(T39_ID_A, T39_ID_B, "t39.e.ab");
    add_edge(T39_ID_B, T39_ID_C, "t39.e.bc");
    add_edge(T39_ID_C, T39_ID_D, "t39.e.cd");

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    4,             "p4: node_count=4");
    assert_eq!(ec,    3,             "p4: edge_count=3");
    assert_eq!(ntc,   3_205_030,     "p4: NTC=3_205_030 (8_192+1_594_323+1_594_323+8_192; 2\u{00b9}\u{00b3}+3\u{00b9}\u{00b3}+3\u{00b9}\u{00b3}+2\u{00b9}\u{00b3})");
    assert_eq!(nhdoc, 2_665_063_586, "p4: NHDOC=2_665_063_586 (244_140_625+2_176_782_336+244_140_625; 5\u{00b9}\u{00b2}+6\u{00b9}\u{00b2}+5\u{00b9}\u{00b2})");
    assert_eq!(neso,  737_717_066,   "p4: NESO=737_717_066 (62_748_517+612_220_032+62_748_517; 13\u{2077}+18\u{2077}+13\u{2077})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NTC:   4×9^13 = 4×2_541_865_828_329 = 10_167_463_313_316.
// NHDOC: 6×18^12 = 6×1_156_831_381_426_176 = 6_940_988_288_557_056.
// NESO:  6×162^7 = 6×2_928_229_434_235_008 = 17_569_376_605_410_048.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);
    add_node(T39_VEC_C, T39_KEY_C, T39_ID_C);
    add_node(T39_VEC_D, T39_KEY_D, T39_ID_D);
    add_edge(T39_ID_A, T39_ID_B, "t39.e.ab");
    add_edge(T39_ID_B, T39_ID_A, "t39.e.ba");
    add_edge(T39_ID_A, T39_ID_C, "t39.e.ac");
    add_edge(T39_ID_C, T39_ID_A, "t39.e.ca");
    add_edge(T39_ID_A, T39_ID_D, "t39.e.ad");
    add_edge(T39_ID_D, T39_ID_A, "t39.e.da");
    add_edge(T39_ID_B, T39_ID_C, "t39.e.bc");
    add_edge(T39_ID_C, T39_ID_B, "t39.e.cb");
    add_edge(T39_ID_B, T39_ID_D, "t39.e.bd");
    add_edge(T39_ID_D, T39_ID_B, "t39.e.db");
    add_edge(T39_ID_C, T39_ID_D, "t39.e.cd");
    add_edge(T39_ID_D, T39_ID_C, "t39.e.dc");

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    4,                        "k4: node_count=4");
    assert_eq!(ec,    6,                        "k4: edge_count=6");
    assert_eq!(ntc,   10_167_463_313_316,       "k4: NTC=10_167_463_313_316 (4\u{00d7}2_541_865_828_329; 9\u{00b9}\u{00b3}=2_541_865_828_329; S-uniform S=9)");
    assert_eq!(nhdoc, 6_940_988_288_557_056,    "k4: NHDOC=6_940_988_288_557_056 (6\u{00d7}1_156_831_381_426_176; 18\u{00b9}\u{00b2}=1_156_831_381_426_176; S-uniform S=9)");
    assert_eq!(neso,  17_569_376_605_410_048,   "k4: NESO=17_569_376_605_410_048 (6\u{00d7}2_928_229_434_235_008; 162\u{2077}=2_928_229_434_235_008; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NTC=0; NHDOC=0; NESO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    2, "isolated: node_count=2");
    assert_eq!(ec,    0, "isolated: no edges");
    assert_eq!(ntc,   0, "isolated: NTC=0 (S=0; 0^13=0)");
    assert_eq!(nhdoc, 0, "isolated: NHDOC=0 (no edges)");
    assert_eq!(neso,  0, "isolated: NESO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NTC:   5×6^13 = 5×13_060_694_016 = 65_303_470_080.
// NHDOC: 6×12^12 = 6×8_916_100_448_256 = 53_496_602_689_536.
// NESO:  6×72^7 = 6×10_030_613_004_288 = 60_183_678_025_728.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T39_VEC_A, T39_KEY_A, T39_ID_A);
    add_node(T39_VEC_B, T39_KEY_B, T39_ID_B);
    add_node(T39_VEC_C, T39_KEY_C, T39_ID_C);
    add_node(T39_VEC_D, T39_KEY_D, T39_ID_D);
    add_node(T39_VEC_E, T39_KEY_E, T39_ID_E);
    add_edge(T39_ID_A, T39_ID_C, "t39.e.ac");
    add_edge(T39_ID_C, T39_ID_A, "t39.e.ca");
    add_edge(T39_ID_A, T39_ID_D, "t39.e.ad");
    add_edge(T39_ID_D, T39_ID_A, "t39.e.da");
    add_edge(T39_ID_A, T39_ID_E, "t39.e.ae");
    add_edge(T39_ID_E, T39_ID_A, "t39.e.ea");
    add_edge(T39_ID_B, T39_ID_C, "t39.e.bc");
    add_edge(T39_ID_C, T39_ID_B, "t39.e.cb");
    add_edge(T39_ID_B, T39_ID_D, "t39.e.bd");
    add_edge(T39_ID_D, T39_ID_B, "t39.e.db");
    add_edge(T39_ID_B, T39_ID_E, "t39.e.be");
    add_edge(T39_ID_E, T39_ID_B, "t39.e.eb");

    let (ntc, nhdoc, neso, ec, nc) = gos_runtime::graph_topo_indices39();
    assert_eq!(nc,    5,                   "k23: node_count=5");
    assert_eq!(ec,    6,                   "k23: edge_count=6");
    assert_eq!(ntc,   65_303_470_080,      "k23: NTC=65_303_470_080 (5\u{00d7}13_060_694_016; 6\u{00b9}\u{00b3}=13_060_694_016; S-uniform S=6)");
    assert_eq!(nhdoc, 53_496_602_689_536,  "k23: NHDOC=53_496_602_689_536 (6\u{00d7}8_916_100_448_256; 12\u{00b9}\u{00b2}=8_916_100_448_256; S-uniform S=6)");
    assert_eq!(neso,  60_183_678_025_728,  "k23: NESO=60_183_678_025_728 (6\u{00d7}10_030_613_004_288; 72\u{2077}=10_030_613_004_288; S-uniform S=6)");
}
