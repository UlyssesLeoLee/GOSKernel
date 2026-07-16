// gos-graph-topo43-harness — V3.54 NHEPTC + NHSTC + NKSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices43()`:
//   Returns (nheptc, nhstc, nkso, edge_count, node_count)
//   - nheptc = NHEPTC(G) = Σ_v S(v)^17                   (exact u64; S-Heptadecic vertex sum)
//   - nhstc  = NHSTC(G)  = Σ_{uv∈E} (S_u+S_v)^16         (exact u64; S-Hexadecic edge-sum)
//   - nkso   = NKSO(G)   = Σ_{uv∈E} (S_u²+S_v²)^11       (exact u64; S-Docosic Sombor, α=22)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTC(G) = Σ_v S(v)^17
//     S-Heptadecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43).
//     NHEPTC = n·S^17 for S-regular.
//     Overflow: S^17 ≤ 16129^17 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHSTC(G) = Σ_{uv∈E} (S_u+S_v)^16
//     S-Hexadecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43).
//     NHSTC = |E|·(2S)^16 = 65536|E|·S^16 for S-regular.
//     Overflow per edge: (2×16129)^16 → saturating u128 accumulator.
//
//   NKSO(G) = Σ_{uv∈E} (S_u²+S_v²)^11
//     S-Docosic Sombor: generalised Sombor SO^α with α=22 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22)
//     — exact, no isqrt.
//     NKSO = |E|·(2S²)^11 = 2048|E|·S^22 for S-regular.
//     Overflow per edge: (2×16129²)^11 → saturating u128 accumulator;
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
//  Graph     NHEPTC(exact)              NHSTC(exact)                  NKSO(exact)             edges  nodes
//  Empty                  0                           0                          0              0      0
//  1 node                 0                           0                          0              0      1
//  K₂                     2                      65_536                      2_048              1      2
//  P₃                393_216               8_589_934_592              17_179_869_184             2      3
//  K₃          51_539_607_552         844_424_930_131_968     108_086_391_056_891_904             3      3
//  K_{1,4}     85_899_345_920       1_125_899_906_842_624     144_115_188_075_855_872             4      5
//  P₄             258_542_470           3_126_285_688_706          67_852_730_867_306             3      4
//  K₄    66_708_726_798_666_276          u64::MAX(sat.)               u64::MAX(sat.)             6      4
//  2 isolated             0                           0                          0              0      2
//  K_{2,3}     84_633_297_223_680   1_109_305_553_370_218_496          u64::MAX(sat.)            6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTC: 1^17 + 1^17 = 2. ✓
//     NHSTC:  (1+1)^16 = 2^16 = 65_536. ✓
//     NKSO:   (1²+1²)^11 = 2^11 = 2_048. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTC: 3×2^17 = 3×131_072 = 393_216. ✓
//       (2^16=65_536 (topo42); 2^17=2×65_536=131_072)
//     NHSTC:  2×(2+2)^16 = 2×4^16 = 2×4_294_967_296 = 8_589_934_592. ✓
//       (4^16=4×4^15=4×1_073_741_824=4_294_967_296)
//     NKSO:   2×(4+4)^11 = 2×8^11 = 2×8_589_934_592 = 17_179_869_184. ✓
//       (8^10=1_073_741_824 (topo42); 8^11=8×1_073_741_824=8_589_934_592)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTC: 3×4^17 = 3×17_179_869_184 = 51_539_607_552. ✓
//       (4^16=4_294_967_296 (topo42); 4^17=4×4_294_967_296=17_179_869_184)
//     NHSTC:  3×(4+4)^16 = 3×8^16 = 3×281_474_976_710_656 = 844_424_930_131_968. ✓
//       (8^15=35_184_372_088_832 (topo42); 8^16=8×35_184_372_088_832=281_474_976_710_656)
//     NKSO:   3×(16+16)^11 = 3×32^11 = 3×36_028_797_018_963_968 = 108_086_391_056_891_904. ✓
//       (32^10=1_125_899_906_842_624 (topo42); 32^11=32×1_125_899_906_842_624=36_028_797_018_963_968)
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTC: 5×4^17 = 5×17_179_869_184 = 85_899_345_920. ✓
//     NHSTC:  4×8^16 = 4×281_474_976_710_656 = 1_125_899_906_842_624. ✓
//     NKSO:   4×32^11 = 4×36_028_797_018_963_968 = 144_115_188_075_855_872. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHSTC and NKSO; NHEPTC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTC: 2^17+3^17+3^17+2^17 = 131_072+129_140_163+129_140_163+131_072 = 258_542_470. ✓
//       (3^16=43_046_721 (topo42); 3^17=3×43_046_721=129_140_163)
//     NHSTC:  (2+3)^16+(3+3)^16+(3+2)^16 = 5^16+6^16+5^16
//             = 152_587_890_625+2_821_109_907_456+152_587_890_625 = 3_126_285_688_706. ✓
//       (5^15=30_517_578_125 (topo42); 5^16=5×30_517_578_125=152_587_890_625)
//       (6^15=470_184_984_576 (topo42); 6^16=6×470_184_984_576=2_821_109_907_456)
//     NKSO:   13^11+18^11+13^11 = 1_792_160_394_037+64_268_410_079_232+1_792_160_394_037
//             = 67_852_730_867_306. ✓
//       (13^10=137_858_491_849 (topo42); 13^11=13×137_858_491_849=1_792_160_394_037)
//       (18^10=3_570_467_226_624 (topo42); 18^11=18×3_570_467_226_624=64_268_410_079_232)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTC: 4×9^17 = 4×16_677_181_699_666_569 = 66_708_726_798_666_276. ✓
//       (9^16=1_853_020_188_851_841 (topo42); 9^17=9×1_853_020_188_851_841=16_677_181_699_666_569)
//       (66_708_726_798_666_276 < u64::MAX) → exact. ✓
//     NHSTC: 6×18^16 → SATURATES to u64::MAX.
//       (18^15=6_746_640_616_477_458_432 (topo42); 18^16=18×6_746_640_616_477_458_432
//        ≈1.214×10^20 > u64::MAX per-edge → clamped u64::MAX). ✓
//     NKSO:  6×162^11 → SATURATES to u64::MAX (per-edge >> u64::MAX).
//       (162^10 >> u64::MAX (topo42); 162^11 >> u64::MAX per-edge). ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTC: 5×6^17 = 5×16_926_659_444_736 = 84_633_297_223_680. ✓
//       (6^16=2_821_109_907_456 (topo42); 6^17=6×2_821_109_907_456=16_926_659_444_736)
//     NHSTC: 6×12^16 = 6×184_884_258_895_036_416 = 1_109_305_553_370_218_496. ✓
//       (12^15=15_407_021_574_586_368 (topo42); 12^16=12×15_407_021_574_586_368=184_884_258_895_036_416)
//       (1_109_305_553_370_218_496 < u64::MAX) → exact. ✓
//     NKSO:  6×72^11 → SATURATES to u64::MAX.
//       (72^10=3_743_906_402_864_487_424 (topo42); 72^11=72×3_743_906_402_864_487_424
//        ≈2.695×10^20 > u64::MAX; 6×72^11 >> u64::MAX) → clamped u64::MAX. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTC = n·S^17                              for S-regular ✓
//   NHSTC  = |E|·(2S)^16 = 65536|E|·S^16        for S-regular ✓
//   NKSO   = |E|·(2S²)^11 = 2048|E|·S^22        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 65_536, 2_048, 1, 2)
//  4.  Path P₃ = A-B-C                   → (393_216, 8_589_934_592, 17_179_869_184, 2, 3)
//  5.  Triangle K₃                       → (51_539_607_552, 844_424_930_131_968, 108_086_391_056_891_904, 3, 3)
//  6.  Star K_{1,4}                      → (85_899_345_920, 1_125_899_906_842_624, 144_115_188_075_855_872, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (258_542_470, 3_126_285_688_706, 67_852_730_867_306, 3, 4)
//  8.  Complete K₄                       → (66_708_726_798_666_276, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (84_633_297_223_680, 1_109_305_553_370_218_496, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T43_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_43");
const T43_EXEC:   ExecutorId = ExecutorId::from_ascii("t43.exec");

const T43_KEY_A: &str = "t43.alpha";
const T43_KEY_B: &str = "t43.beta";
const T43_KEY_C: &str = "t43.gamma";
const T43_KEY_D: &str = "t43.delta";
const T43_KEY_E: &str = "t43.epsilon";

const T43_ID_A: NodeId = derive_node_id(T43_PLUGIN, T43_KEY_A);
const T43_ID_B: NodeId = derive_node_id(T43_PLUGIN, T43_KEY_B);
const T43_ID_C: NodeId = derive_node_id(T43_PLUGIN, T43_KEY_C);
const T43_ID_D: NodeId = derive_node_id(T43_PLUGIN, T43_KEY_D);
const T43_ID_E: NodeId = derive_node_id(T43_PLUGIN, T43_KEY_E);

// L4=130 namespace for this harness.
const T43_VEC_A: VectorAddress = VectorAddress::new(130, 1, 1, 0);
const T43_VEC_B: VectorAddress = VectorAddress::new(130, 1, 2, 0);
const T43_VEC_C: VectorAddress = VectorAddress::new(130, 1, 3, 0);
const T43_VEC_D: VectorAddress = VectorAddress::new(130, 2, 1, 0);
const T43_VEC_E: VectorAddress = VectorAddress::new(130, 2, 2, 0);

const T43_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T43_PLUGIN,
    name:         "kl-graph-topo43-harness",
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
        executor_id:       T43_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T43_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T43_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     0, "empty: node_count=0");
    assert_eq!(ec,     0, "empty: edge_count=0");
    assert_eq!(nheptc, 0, "empty: NHEPTC=0");
    assert_eq!(nhstc,  0, "empty: NHSTC=0");
    assert_eq!(nkso,   0, "empty: NKSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NHEPTC: 0^17=0; NHSTC: no edges; NKSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     1, "single: node_count=1");
    assert_eq!(ec,     0, "single: no edges");
    assert_eq!(nheptc, 0, "single: NHEPTC=0 (S=0; 0^17=0)");
    assert_eq!(nhstc,  0, "single: NHSTC=0 (no edges)");
    assert_eq!(nkso,   0, "single: NKSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NHEPTC: 1^17+1^17 = 2.
// NHSTC:  (1+1)^16 = 2^16 = 65_536.
// NKSO:   (1²+1²)^11 = 2^11 = 2_048.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);
    add_edge(T43_ID_A, T43_ID_B, "t43.e.ab");

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     2,      "k2: node_count=2");
    assert_eq!(ec,     1,      "k2: edge_count=1");
    assert_eq!(nheptc, 2,      "k2: NHEPTC=2 (1\u{00b9}\u{2077}+1\u{00b9}\u{2077}=2; S-uniform S=1)");
    assert_eq!(nhstc,  65_536, "k2: NHSTC=65_536 ((1+1)\u{00b9}\u{2076}=2\u{00b9}\u{2076}=65_536; S-uniform S=1)");
    assert_eq!(nkso,   2_048,  "k2: NKSO=2_048 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{00b9}=2\u{00b9}\u{00b9}=2_048; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NHEPTC: 3×2^17 = 3×131_072 = 393_216.
// NHSTC:  2×(2+2)^16 = 2×4^16 = 2×4_294_967_296 = 8_589_934_592.
// NKSO:   2×(4+4)^11 = 2×8^11 = 2×8_589_934_592 = 17_179_869_184.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);
    add_node(T43_VEC_C, T43_KEY_C, T43_ID_C);
    add_edge(T43_ID_A, T43_ID_B, "t43.e.ab");
    add_edge(T43_ID_B, T43_ID_C, "t43.e.bc");

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     3,              "p3: node_count=3");
    assert_eq!(ec,     2,              "p3: edge_count=2");
    assert_eq!(nheptc, 393_216,        "p3: NHEPTC=393_216 (3\u{00d7}131_072; 2\u{00b9}\u{2077}=131_072; S-uniform S=2)");
    assert_eq!(nhstc,  8_589_934_592,  "p3: NHSTC=8_589_934_592 (2\u{00d7}4_294_967_296; (2+2)\u{00b9}\u{2076}=4\u{00b9}\u{2076}=4_294_967_296; S-uniform S=2)");
    assert_eq!(nkso,   17_179_869_184, "p3: NKSO=17_179_869_184 (2\u{00d7}8_589_934_592; (4+4)\u{00b9}\u{00b9}=8\u{00b9}\u{00b9}=8_589_934_592; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHEPTC: 3×4^17 = 3×17_179_869_184 = 51_539_607_552.
// NHSTC:  3×(4+4)^16 = 3×8^16 = 3×281_474_976_710_656 = 844_424_930_131_968.
// NKSO:   3×(16+16)^11 = 3×32^11 = 3×36_028_797_018_963_968 = 108_086_391_056_891_904.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);
    add_node(T43_VEC_C, T43_KEY_C, T43_ID_C);
    add_edge(T43_ID_A, T43_ID_B, "t43.e.ab");
    add_edge(T43_ID_B, T43_ID_A, "t43.e.ba");
    add_edge(T43_ID_B, T43_ID_C, "t43.e.bc");
    add_edge(T43_ID_C, T43_ID_B, "t43.e.cb");
    add_edge(T43_ID_A, T43_ID_C, "t43.e.ac");
    add_edge(T43_ID_C, T43_ID_A, "t43.e.ca");

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     3,                         "k3: node_count=3");
    assert_eq!(ec,     3,                         "k3: edge_count=3");
    assert_eq!(nheptc, 51_539_607_552,            "k3: NHEPTC=51_539_607_552 (3\u{00d7}17_179_869_184; 4\u{00b9}\u{2077}=17_179_869_184; S-uniform S=4)");
    assert_eq!(nhstc,  844_424_930_131_968,       "k3: NHSTC=844_424_930_131_968 (3\u{00d7}281_474_976_710_656; (4+4)\u{00b9}\u{2076}=8\u{00b9}\u{2076}=281_474_976_710_656; S-uniform S=4)");
    assert_eq!(nkso,   108_086_391_056_891_904,   "k3: NKSO=108_086_391_056_891_904 (3\u{00d7}36_028_797_018_963_968; (16+16)\u{00b9}\u{00b9}=32\u{00b9}\u{00b9}=36_028_797_018_963_968; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHSTC and NKSO as K₃; NHEPTC and totals differ by node/edge count.
// NHEPTC: 5×4^17 = 5×17_179_869_184 = 85_899_345_920.
// NHSTC:  4×8^16 = 4×281_474_976_710_656 = 1_125_899_906_842_624.
// NKSO:   4×32^11 = 4×36_028_797_018_963_968 = 144_115_188_075_855_872.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);
    add_node(T43_VEC_C, T43_KEY_C, T43_ID_C);
    add_node(T43_VEC_D, T43_KEY_D, T43_ID_D);
    add_node(T43_VEC_E, T43_KEY_E, T43_ID_E);
    add_edge(T43_ID_A, T43_ID_B, "t43.e.ab");
    add_edge(T43_ID_A, T43_ID_C, "t43.e.ac");
    add_edge(T43_ID_A, T43_ID_D, "t43.e.ad");
    add_edge(T43_ID_A, T43_ID_E, "t43.e.ae");

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     5,                         "star: node_count=5");
    assert_eq!(ec,     4,                         "star: edge_count=4");
    assert_eq!(nheptc, 85_899_345_920,            "star: NHEPTC=85_899_345_920 (5\u{00d7}17_179_869_184; same S as K\u{2083})");
    assert_eq!(nhstc,  1_125_899_906_842_624,     "star: NHSTC=1_125_899_906_842_624 (4\u{00d7}281_474_976_710_656; same per-edge as K\u{2083})");
    assert_eq!(nkso,   144_115_188_075_855_872,   "star: NKSO=144_115_188_075_855_872 (4\u{00d7}36_028_797_018_963_968; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NHEPTC: 2^17+3^17+3^17+2^17 = 131_072+129_140_163+129_140_163+131_072 = 258_542_470.
// NHSTC:  (2+3)^16+(3+3)^16+(3+2)^16 = 5^16+6^16+5^16
//         = 152_587_890_625+2_821_109_907_456+152_587_890_625 = 3_126_285_688_706.
// NKSO:   13^11+18^11+13^11 = 1_792_160_394_037+64_268_410_079_232+1_792_160_394_037
//         = 67_852_730_867_306.
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);
    add_node(T43_VEC_C, T43_KEY_C, T43_ID_C);
    add_node(T43_VEC_D, T43_KEY_D, T43_ID_D);
    add_edge(T43_ID_A, T43_ID_B, "t43.e.ab");
    add_edge(T43_ID_B, T43_ID_C, "t43.e.bc");
    add_edge(T43_ID_C, T43_ID_D, "t43.e.cd");

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     4,                  "p4: node_count=4");
    assert_eq!(ec,     3,                  "p4: edge_count=3");
    assert_eq!(nheptc, 258_542_470,        "p4: NHEPTC=258_542_470 (131_072+129_140_163+129_140_163+131_072; 2\u{00b9}\u{2077}+3\u{00b9}\u{2077}+3\u{00b9}\u{2077}+2\u{00b9}\u{2077})");
    assert_eq!(nhstc,  3_126_285_688_706,  "p4: NHSTC=3_126_285_688_706 (152_587_890_625+2_821_109_907_456+152_587_890_625; 5\u{00b9}\u{2076}+6\u{00b9}\u{2076}+5\u{00b9}\u{2076})");
    assert_eq!(nkso,   67_852_730_867_306, "p4: NKSO=67_852_730_867_306 (1_792_160_394_037+64_268_410_079_232+1_792_160_394_037; 13\u{00b9}\u{00b9}+18\u{00b9}\u{00b9}+13\u{00b9}\u{00b9})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHEPTC: 4×9^17 = 4×16_677_181_699_666_569 = 66_708_726_798_666_276 (fits u64).
// NHSTC:  6×18^16 → SATURATES → u64::MAX.
//   (18^16≈1.214×10^20 > u64::MAX per-edge)
// NKSO:   6×162^11 → SATURATES → u64::MAX (per-edge already >> u64::MAX).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);
    add_node(T43_VEC_C, T43_KEY_C, T43_ID_C);
    add_node(T43_VEC_D, T43_KEY_D, T43_ID_D);
    add_edge(T43_ID_A, T43_ID_B, "t43.e.ab");
    add_edge(T43_ID_B, T43_ID_A, "t43.e.ba");
    add_edge(T43_ID_A, T43_ID_C, "t43.e.ac");
    add_edge(T43_ID_C, T43_ID_A, "t43.e.ca");
    add_edge(T43_ID_A, T43_ID_D, "t43.e.ad");
    add_edge(T43_ID_D, T43_ID_A, "t43.e.da");
    add_edge(T43_ID_B, T43_ID_C, "t43.e.bc");
    add_edge(T43_ID_C, T43_ID_B, "t43.e.cb");
    add_edge(T43_ID_B, T43_ID_D, "t43.e.bd");
    add_edge(T43_ID_D, T43_ID_B, "t43.e.db");
    add_edge(T43_ID_C, T43_ID_D, "t43.e.cd");
    add_edge(T43_ID_D, T43_ID_C, "t43.e.dc");

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     4,                        "k4: node_count=4");
    assert_eq!(ec,     6,                        "k4: edge_count=6");
    assert_eq!(nheptc, 66_708_726_798_666_276,   "k4: NHEPTC=66_708_726_798_666_276 (4\u{00d7}16_677_181_699_666_569; 9\u{00b9}\u{2077}=16_677_181_699_666_569; S-uniform S=9; fits u64)");
    assert_eq!(nhstc,  u64::MAX,                 "k4: NHSTC=u64::MAX (6\u{00d7}18\u{00b9}\u{2076}\u{2248}7.28\u{00d7}10^20 > u64::MAX; saturated)");
    assert_eq!(nkso,   u64::MAX,                 "k4: NKSO=u64::MAX (6\u{00d7}162\u{00b9}\u{00b9} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHEPTC=0; NHSTC=0; NKSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     2, "isolated: node_count=2");
    assert_eq!(ec,     0, "isolated: no edges");
    assert_eq!(nheptc, 0, "isolated: NHEPTC=0 (S=0; 0^17=0)");
    assert_eq!(nhstc,  0, "isolated: NHSTC=0 (no edges)");
    assert_eq!(nkso,   0, "isolated: NKSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NHEPTC: 5×6^17 = 5×16_926_659_444_736 = 84_633_297_223_680.
// NHSTC:  6×12^16 = 6×184_884_258_895_036_416 = 1_109_305_553_370_218_496.
// NKSO:   6×72^11 ≈ 2.695×10^20 × 6 >> u64::MAX → u64::MAX.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T43_VEC_A, T43_KEY_A, T43_ID_A);
    add_node(T43_VEC_B, T43_KEY_B, T43_ID_B);
    add_node(T43_VEC_C, T43_KEY_C, T43_ID_C);
    add_node(T43_VEC_D, T43_KEY_D, T43_ID_D);
    add_node(T43_VEC_E, T43_KEY_E, T43_ID_E);
    add_edge(T43_ID_A, T43_ID_C, "t43.e.ac");
    add_edge(T43_ID_C, T43_ID_A, "t43.e.ca");
    add_edge(T43_ID_A, T43_ID_D, "t43.e.ad");
    add_edge(T43_ID_D, T43_ID_A, "t43.e.da");
    add_edge(T43_ID_A, T43_ID_E, "t43.e.ae");
    add_edge(T43_ID_E, T43_ID_A, "t43.e.ea");
    add_edge(T43_ID_B, T43_ID_C, "t43.e.bc");
    add_edge(T43_ID_C, T43_ID_B, "t43.e.cb");
    add_edge(T43_ID_B, T43_ID_D, "t43.e.bd");
    add_edge(T43_ID_D, T43_ID_B, "t43.e.db");
    add_edge(T43_ID_B, T43_ID_E, "t43.e.be");
    add_edge(T43_ID_E, T43_ID_B, "t43.e.eb");

    let (nheptc, nhstc, nkso, ec, nc) = gos_runtime::graph_topo_indices43();
    assert_eq!(nc,     5,                          "k23: node_count=5");
    assert_eq!(ec,     6,                          "k23: edge_count=6");
    assert_eq!(nheptc, 84_633_297_223_680,         "k23: NHEPTC=84_633_297_223_680 (5\u{00d7}16_926_659_444_736; 6\u{00b9}\u{2077}=16_926_659_444_736; S-uniform S=6)");
    assert_eq!(nhstc,  1_109_305_553_370_218_496,  "k23: NHSTC=1_109_305_553_370_218_496 (6\u{00d7}184_884_258_895_036_416; 12\u{00b9}\u{2076}=184_884_258_895_036_416; S-uniform S=6)");
    assert_eq!(nkso,   u64::MAX,                   "k23: NKSO=u64::MAX (6\u{00d7}72\u{00b9}\u{00b9}\u{2248}1.617\u{00d7}10^21 > u64::MAX; saturated)");
}
