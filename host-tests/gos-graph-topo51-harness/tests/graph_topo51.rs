// gos-graph-topo51-harness — V3.62 NPENTTC + NHPENTTC + NUSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices51()`:
//   Returns (npenttc, nhpenttc, nuso, edge_count, node_count)
//   - npenttc  = NPENTTC(G)  = Σ_v S(v)^25                  (exact u64; S-Pentacosic vertex sum)
//   - nhpenttc = NHPENTTC(G) = Σ_{uv∈E} (S_u+S_v)^24        (exact u64; S-Tetracosic edge-sum)
//   - nuso     = NUSO(G)     = Σ_{uv∈E} (S_u²+S_v²)^19      (exact u64; S-Octatriacontyl Sombor, α=38)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NPENTTC(G) = Σ_v S(v)^25
//     S-Pentacosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51).
//     NPENTTC = n·S^25 for S-regular.
//     Overflow: S^25 ≤ 16129^25 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHPENTTC(G) = Σ_{uv∈E} (S_u+S_v)^24
//     S-Tetracosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51).
//     NHPENTTC = |E|·(2S)^24 = 16777216|E|·S^24 for S-regular.
//     Overflow per edge: (2×16129)^24 → saturating u128 accumulator.
//
//   NUSO(G) = Σ_{uv∈E} (S_u²+S_v²)^19
//     S-Octatriacontyl Sombor: generalised Sombor SO^α with α=38 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38).
//     (T skipped: NTSO already used for α=10)
//     NUSO = |E|·(2S²)^19 = 524288|E|·S^38 for S-regular.
//     Overflow per edge: (2×16129²)^19 → saturating u128 accumulator.
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
//  Graph     NPENTTC(exact)              NHPENTTC(exact)               NUSO(exact)              edges  nodes
//  Empty                  0                            0                         0               0      0
//  1 node                 0                            0                         0               0      1
//  K₂                     2                   16_777_216                   524_288               1      2
//  P₃             100_663_296          562_949_953_421_312   288_230_376_151_711_744             2      3
//  K₃       3_377_699_720_527_872       u64::MAX(sat.)              u64::MAX(sat.)               3      3
//  K_{1,4}  5_629_499_534_213_120       u64::MAX(sat.)              u64::MAX(sat.)               4      5
//  P₄         1_694_644_327_750    4_857_590_627_872_398_146         u64::MAX(sat.)               3      4
//  K₄          u64::MAX(sat.)          u64::MAX(sat.)               u64::MAX(sat.)               6      4
//  2 isolated             0                            0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)           u64::MAX(sat.)               u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NPENTTC:  1^25 + 1^25 = 2. ✓
//     NHPENTTC: (1+1)^24 = 2^24 = 16_777_216. ✓
//     NUSO:     (1²+1²)^19 = 2^19 = 524_288. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NPENTTC:  3×2^25 = 3×33_554_432 = 100_663_296. ✓
//     NHPENTTC: 2×(2+2)^24 = 2×4^24 = 2×281_474_976_710_656 = 562_949_953_421_312. ✓
//       (4^12=16_777_216; 4^24=16_777_216^2=281_474_976_710_656)
//     NUSO:     2×(4+4)^19 = 2×8^19.
//       8^16=281_474_976_710_656; 8^19=8^16×8^2×8=281_474_976_710_656×64×8
//       =281_474_976_710_656×512=144_115_188_075_855_872
//       2×144_115_188_075_855_872=288_230_376_151_711_744 (fits u64). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NPENTTC:  3×4^25 = 3×2^50 = 3×1_125_899_906_842_624 = 3_377_699_720_527_872 (fits u64). ✓
//       (4^12=16_777_216; 4^24=281_474_976_710_656; 4^25=4^24×4=1_125_899_906_842_624)
//     NHPENTTC: 3×(4+4)^24 = 3×8^24 = 3×2^72 → SATURATES.
//       (8^24=2^72≈4.72×10^21 >> u64::MAX per-edge). ✓
//     NUSO:     3×(16+16)^19 = 3×32^19 = 3×2^95 → SATURATES (per-edge >> u64::MAX). ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NPENTTC:  5×4^25 = 5×1_125_899_906_842_624 = 5_629_499_534_213_120 (fits u64). ✓
//     NHPENTTC: 4×8^24 → SATURATES. ✓
//     NUSO:     4×32^19 → SATURATES (per-edge >> u64::MAX). ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NPENTTC:  2^25+3^25+3^25+2^25 = 2×33_554_432+2×847_288_609_443.
//       3^25=3^16×3^8×3^1=43_046_721×6_561×3=43_046_721×19_683=847_288_609_443
//       2×33_554_432+2×847_288_609_443=67_108_864+1_694_577_218_886=1_694_644_327_750. ✓
//     NHPENTTC: 5^24+6^24+5^24
//       5^24: 5^16=152_587_890_625; 5^24=5^16×5^8=152_587_890_625×390_625=59_604_644_775_390_625
//       6^24: 6^12=2_176_782_336; 6^24=2_176_782_336^2=4_738_381_338_321_616_896
//       2×59_604_644_775_390_625+4_738_381_338_321_616_896
//       =119_209_289_550_781_250+4_738_381_338_321_616_896=4_857_590_627_872_398_146 (fits u64). ✓
//     NUSO:     13^19+18^19+13^19
//       13^16=665_416_609_183_179_841; 13^19=13^16×13^2×13=13^16×2197 >> u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NPENTTC:  4×9^25 → SATURATES (9^12=282_429_536_481; 9^24>>u64::MAX per-vertex). ✓
//     NHPENTTC: 6×18^24 → SATURATES. ✓
//     NUSO:     6×162^19 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NPENTTC:  5×6^25. 6^25=6^24×6=4_738_381_338_321_616_896×6=28_430_288_029_929_701_376>u64::MAX → SATURATES. ✓
//     NHPENTTC: 6×12^24 → SATURATES (12^24>>u64::MAX per-edge). ✓
//     NUSO:     6×72^19 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NPENTTC  = n·S^25                                     for S-regular ✓
//   NHPENTTC = |E|·(2S)^24 = 16777216|E|·S^24            for S-regular ✓
//   NUSO     = |E|·(2S²)^19 = 524288|E|·S^38             for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 16_777_216, 524_288, 1, 2)
//  4.  Path P₃ = A-B-C                   → (100_663_296, 562_949_953_421_312, 288_230_376_151_711_744, 2, 3)
//  5.  Triangle K₃                       → (3_377_699_720_527_872, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (5_629_499_534_213_120, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (1_694_644_327_750, 4_857_590_627_872_398_146, u64::MAX, 3, 4)
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

const T51_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_51");
const T51_EXEC:   ExecutorId = ExecutorId::from_ascii("t51.exec");

const T51_KEY_A: &str = "t51.alpha";
const T51_KEY_B: &str = "t51.beta";
const T51_KEY_C: &str = "t51.gamma";
const T51_KEY_D: &str = "t51.delta";
const T51_KEY_E: &str = "t51.epsilon";

const T51_ID_A: NodeId = derive_node_id(T51_PLUGIN, T51_KEY_A);
const T51_ID_B: NodeId = derive_node_id(T51_PLUGIN, T51_KEY_B);
const T51_ID_C: NodeId = derive_node_id(T51_PLUGIN, T51_KEY_C);
const T51_ID_D: NodeId = derive_node_id(T51_PLUGIN, T51_KEY_D);
const T51_ID_E: NodeId = derive_node_id(T51_PLUGIN, T51_KEY_E);

// L4=138 namespace for this harness.
const T51_VEC_A: VectorAddress = VectorAddress::new(138, 1, 1, 0);
const T51_VEC_B: VectorAddress = VectorAddress::new(138, 1, 2, 0);
const T51_VEC_C: VectorAddress = VectorAddress::new(138, 1, 3, 0);
const T51_VEC_D: VectorAddress = VectorAddress::new(138, 2, 1, 0);
const T51_VEC_E: VectorAddress = VectorAddress::new(138, 2, 2, 0);

const T51_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T51_PLUGIN,
    name:         "kl-graph-topo51-harness",
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
        executor_id:       T51_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T51_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T51_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       0, "empty: node_count=0");
    assert_eq!(ec,       0, "empty: edge_count=0");
    assert_eq!(npenttc,  0, "empty: NPENTTC=0");
    assert_eq!(nhpenttc, 0, "empty: NHPENTTC=0");
    assert_eq!(nuso,     0, "empty: NUSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NPENTTC: 0^25=0; NHPENTTC: no edges; NUSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       1, "single: node_count=1");
    assert_eq!(ec,       0, "single: no edges");
    assert_eq!(npenttc,  0, "single: NPENTTC=0 (S=0; 0^25=0)");
    assert_eq!(nhpenttc, 0, "single: NHPENTTC=0 (no edges)");
    assert_eq!(nuso,     0, "single: NUSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NPENTTC:  1^25+1^25 = 2.
// NHPENTTC: (1+1)^24 = 2^24 = 16_777_216.
// NUSO:     (1²+1²)^19 = 2^19 = 524_288.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);
    add_edge(T51_ID_A, T51_ID_B, "t51.e.ab");

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       2,          "k2: node_count=2");
    assert_eq!(ec,       1,          "k2: edge_count=1");
    assert_eq!(npenttc,  2,          "k2: NPENTTC=2 (1\u{00b2}\u{2075}+1\u{00b2}\u{2075}=2; S-uniform S=1)");
    assert_eq!(nhpenttc, 16_777_216, "k2: NHPENTTC=16_777_216 ((1+1)\u{00b2}\u{2074}=2\u{00b2}\u{2074}=16_777_216; S-uniform S=1)");
    assert_eq!(nuso,     524_288,    "k2: NUSO=524_288 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{2079}=2\u{00b9}\u{2079}=524_288; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NPENTTC:  3×2^25 = 3×33_554_432 = 100_663_296.
// NHPENTTC: 2×(2+2)^24 = 2×4^24 = 2×281_474_976_710_656 = 562_949_953_421_312.
// NUSO:     2×(4+4)^19 = 2×8^19 = 2×144_115_188_075_855_872 = 288_230_376_151_711_744.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);
    add_node(T51_VEC_C, T51_KEY_C, T51_ID_C);
    add_edge(T51_ID_A, T51_ID_B, "t51.e.ab");
    add_edge(T51_ID_B, T51_ID_C, "t51.e.bc");

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       3,                          "p3: node_count=3");
    assert_eq!(ec,       2,                          "p3: edge_count=2");
    assert_eq!(npenttc,  100_663_296,                "p3: NPENTTC=100_663_296 (3\u{00d7}33_554_432; 2\u{00b2}\u{2075}=33_554_432; S-uniform S=2)");
    assert_eq!(nhpenttc, 562_949_953_421_312,        "p3: NHPENTTC=562_949_953_421_312 (2\u{00d7}281_474_976_710_656; (2+2)\u{00b2}\u{2074}=4\u{00b2}\u{2074}=281_474_976_710_656; S-uniform S=2)");
    assert_eq!(nuso,     288_230_376_151_711_744,    "p3: NUSO=288_230_376_151_711_744 (2\u{00d7}144_115_188_075_855_872; (4+4)\u{00b9}\u{2079}=8\u{00b9}\u{2079}=144_115_188_075_855_872; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NPENTTC:  3×4^25 = 3×2^50 = 3_377_699_720_527_872 (fits u64).
// NHPENTTC: 3×(4+4)^24 = 3×8^24 = 3×2^72 → SATURATES (2^72≈4.72×10^21>>u64::MAX per-edge).
// NUSO:     3×(16+16)^19 = 3×32^19 = 3×2^95 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);
    add_node(T51_VEC_C, T51_KEY_C, T51_ID_C);
    add_edge(T51_ID_A, T51_ID_B, "t51.e.ab");
    add_edge(T51_ID_B, T51_ID_A, "t51.e.ba");
    add_edge(T51_ID_B, T51_ID_C, "t51.e.bc");
    add_edge(T51_ID_C, T51_ID_B, "t51.e.cb");
    add_edge(T51_ID_A, T51_ID_C, "t51.e.ac");
    add_edge(T51_ID_C, T51_ID_A, "t51.e.ca");

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       3,                      "k3: node_count=3");
    assert_eq!(ec,       3,                      "k3: edge_count=3");
    assert_eq!(npenttc,  3_377_699_720_527_872,  "k3: NPENTTC=3_377_699_720_527_872 (3\u{00d7}1_125_899_906_842_624; 4\u{00b2}\u{2075}=2\u{2075}\u{2070}=1_125_899_906_842_624; S-uniform S=4)");
    assert_eq!(nhpenttc, u64::MAX,               "k3: NHPENTTC=u64::MAX (3\u{00d7}8\u{00b2}\u{2074}=3\u{00d7}2\u{2077}\u{00b2} >> u64::MAX; saturated)");
    assert_eq!(nuso,     u64::MAX,               "k3: NUSO=u64::MAX (3\u{00d7}32\u{00b9}\u{2079}=3\u{00d7}2\u{2079}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NPENTTC:  5×4^25 = 5×1_125_899_906_842_624 = 5_629_499_534_213_120 (fits u64).
// NHPENTTC: 4×8^24 → SATURATES.
// NUSO:     4×32^19 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);
    add_node(T51_VEC_C, T51_KEY_C, T51_ID_C);
    add_node(T51_VEC_D, T51_KEY_D, T51_ID_D);
    add_node(T51_VEC_E, T51_KEY_E, T51_ID_E);
    add_edge(T51_ID_A, T51_ID_B, "t51.e.ab");
    add_edge(T51_ID_A, T51_ID_C, "t51.e.ac");
    add_edge(T51_ID_A, T51_ID_D, "t51.e.ad");
    add_edge(T51_ID_A, T51_ID_E, "t51.e.ae");

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       5,                      "star: node_count=5");
    assert_eq!(ec,       4,                      "star: edge_count=4");
    assert_eq!(npenttc,  5_629_499_534_213_120,  "star: NPENTTC=5_629_499_534_213_120 (5\u{00d7}1_125_899_906_842_624; same S as K\u{2083})");
    assert_eq!(nhpenttc, u64::MAX,               "star: NHPENTTC=u64::MAX (4\u{00d7}8\u{00b2}\u{2074} >> u64::MAX; saturated)");
    assert_eq!(nuso,     u64::MAX,               "star: NUSO=u64::MAX (4\u{00d7}32\u{00b9}\u{2079} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NPENTTC:  2^25+3^25+3^25+2^25 = 2×33_554_432+2×847_288_609_443 = 1_694_644_327_750.
// NHPENTTC: 5^24+6^24+5^24 = 2×59_604_644_775_390_625+4_738_381_338_321_616_896
//           = 4_857_590_627_872_398_146 (fits u64).
// NUSO:     13^19+18^19+13^19 → SATURATES (13^19>>u64::MAX per-edge).

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);
    add_node(T51_VEC_C, T51_KEY_C, T51_ID_C);
    add_node(T51_VEC_D, T51_KEY_D, T51_ID_D);
    add_edge(T51_ID_A, T51_ID_B, "t51.e.ab");
    add_edge(T51_ID_B, T51_ID_C, "t51.e.bc");
    add_edge(T51_ID_C, T51_ID_D, "t51.e.cd");

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       4,                           "p4: node_count=4");
    assert_eq!(ec,       3,                           "p4: edge_count=3");
    assert_eq!(npenttc,  1_694_644_327_750,           "p4: NPENTTC=1_694_644_327_750 (2\u{00d7}33_554_432+2\u{00d7}847_288_609_443; 2\u{00b2}\u{2075}+3\u{00b2}\u{2075}+3\u{00b2}\u{2075}+2\u{00b2}\u{2075})");
    assert_eq!(nhpenttc, 4_857_590_627_872_398_146,   "p4: NHPENTTC=4_857_590_627_872_398_146 (2\u{00d7}59_604_644_775_390_625+4_738_381_338_321_616_896; 5\u{00b2}\u{2074}+6\u{00b2}\u{2074}+5\u{00b2}\u{2074})");
    assert_eq!(nuso,     u64::MAX,                    "p4: NUSO=u64::MAX (13\u{00b9}\u{2079}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NPENTTC:  4×9^25 → SATURATES → u64::MAX.
// NHPENTTC: 6×18^24 → SATURATES → u64::MAX.
// NUSO:     6×162^19 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);
    add_node(T51_VEC_C, T51_KEY_C, T51_ID_C);
    add_node(T51_VEC_D, T51_KEY_D, T51_ID_D);
    add_edge(T51_ID_A, T51_ID_B, "t51.e.ab");
    add_edge(T51_ID_B, T51_ID_A, "t51.e.ba");
    add_edge(T51_ID_A, T51_ID_C, "t51.e.ac");
    add_edge(T51_ID_C, T51_ID_A, "t51.e.ca");
    add_edge(T51_ID_A, T51_ID_D, "t51.e.ad");
    add_edge(T51_ID_D, T51_ID_A, "t51.e.da");
    add_edge(T51_ID_B, T51_ID_C, "t51.e.bc");
    add_edge(T51_ID_C, T51_ID_B, "t51.e.cb");
    add_edge(T51_ID_B, T51_ID_D, "t51.e.bd");
    add_edge(T51_ID_D, T51_ID_B, "t51.e.db");
    add_edge(T51_ID_C, T51_ID_D, "t51.e.cd");
    add_edge(T51_ID_D, T51_ID_C, "t51.e.dc");

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       4,        "k4: node_count=4");
    assert_eq!(ec,       6,        "k4: edge_count=6");
    assert_eq!(npenttc,  u64::MAX, "k4: NPENTTC=u64::MAX (4\u{00d7}9\u{00b2}\u{2075} >> u64::MAX; saturated)");
    assert_eq!(nhpenttc, u64::MAX, "k4: NHPENTTC=u64::MAX (6\u{00d7}18\u{00b2}\u{2074} >> u64::MAX; saturated)");
    assert_eq!(nuso,     u64::MAX, "k4: NUSO=u64::MAX (6\u{00d7}162\u{00b9}\u{2079} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NPENTTC=0; NHPENTTC=0; NUSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       2, "two-iso: node_count=2");
    assert_eq!(ec,       0, "two-iso: edge_count=0");
    assert_eq!(npenttc,  0, "two-iso: NPENTTC=0");
    assert_eq!(nhpenttc, 0, "two-iso: NHPENTTC=0");
    assert_eq!(nuso,     0, "two-iso: NUSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NPENTTC:  5×6^25 = 5×28_430_288_029_929_701_376 > u64::MAX → SATURATES.
// NHPENTTC: 6×12^24 → SATURATES (12^24>>u64::MAX per-edge).
// NUSO:     6×72^19 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T51_VEC_A, T51_KEY_A, T51_ID_A);
    add_node(T51_VEC_B, T51_KEY_B, T51_ID_B);
    add_node(T51_VEC_C, T51_KEY_C, T51_ID_C);
    add_node(T51_VEC_D, T51_KEY_D, T51_ID_D);
    add_node(T51_VEC_E, T51_KEY_E, T51_ID_E);
    add_edge(T51_ID_A, T51_ID_C, "t51.e.ac");
    add_edge(T51_ID_A, T51_ID_D, "t51.e.ad");
    add_edge(T51_ID_A, T51_ID_E, "t51.e.ae");
    add_edge(T51_ID_B, T51_ID_C, "t51.e.bc");
    add_edge(T51_ID_B, T51_ID_D, "t51.e.bd");
    add_edge(T51_ID_B, T51_ID_E, "t51.e.be");

    let (npenttc, nhpenttc, nuso, ec, nc) = gos_runtime::graph_topo_indices51();
    assert_eq!(nc,       5,        "k23: node_count=5");
    assert_eq!(ec,       6,        "k23: edge_count=6");
    assert_eq!(npenttc,  u64::MAX, "k23: NPENTTC=u64::MAX (5\u{00d7}6\u{00b2}\u{2075}=5\u{00d7}28_430_288_029_929_701_376>u64::MAX; saturated)");
    assert_eq!(nhpenttc, u64::MAX, "k23: NHPENTTC=u64::MAX (6\u{00d7}12\u{00b2}\u{2074} >> u64::MAX; per-edge saturates)");
    assert_eq!(nuso,     u64::MAX, "k23: NUSO=u64::MAX (6\u{00d7}72\u{00b9}\u{2079} >> u64::MAX; per-edge saturates)");
}
