// gos-graph-topo48-harness — V3.59 NDOCTC + NHDOCTC + NQSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices48()`:
//   Returns (ndoctc, nhdoctc, nqso, edge_count, node_count)
//   - ndoctc  = NDOCTC(G)  = Σ_v S(v)^22                  (exact u64; S-Docosic vertex sum)
//   - nhdoctc = NHDOCTC(G) = Σ_{uv∈E} (S_u+S_v)^21        (exact u64; S-Heneicosic edge-sum)
//   - nqso    = NQSO(G)    = Σ_{uv∈E} (S_u²+S_v²)^16      (exact u64; S-Dotriacontyl Sombor, α=32)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NDOCTC(G) = Σ_v S(v)^22
//     S-Docosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48).
//     NDOCTC = n·S^22 for S-regular.
//     Overflow: S^22 ≤ 16129^22 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHDOCTC(G) = Σ_{uv∈E} (S_u+S_v)^21
//     S-Heneicosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48).
//     NHDOCTC = |E|·(2S)^21 = 2097152|E|·S^21 for S-regular.
//     Overflow per edge: (2×16129)^21 → saturating u128 accumulator.
//
//   NQSO(G) = Σ_{uv∈E} (S_u²+S_v²)^16
//     S-Dotriacontyl Sombor: generalised Sombor SO^α with α=32 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32).
//     (Q used: O=α=8 taken; P=α=30 taken; Q follows in sequence)
//     NQSO = |E|·(2S²)^16 = 65536|E|·S^32 for S-regular.
//     Overflow per edge: (2×16129²)^16 → saturating u128 accumulator.
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
//  Graph     NDOCTC(exact)              NHDOCTC(exact)               NQSO(exact)              edges  nodes
//  Empty                  0                            0                         0               0      0
//  1 node                 0                            0                         0               0      1
//  K₂                     2                    2_097_152                    65_536               1      2
//  P₃              12_582_912            8_796_093_022_208         562_949_953_421_312            2      3
//  K₃        52_776_558_133_248         u64::MAX(sat.)              u64::MAX(sat.)               3      3
//  K_{1,4}   87_960_930_222_080         u64::MAX(sat.)              u64::MAX(sat.)               4      5
//  P₄            62_770_507_826     22_890_624_956_784_106          u64::MAX(sat.)               3      4
//  K₄          u64::MAX(sat.)          u64::MAX(sat.)               u64::MAX(sat.)               6      4
//  2 isolated             0                            0                         0               0      2
//  K_{2,3}  658_108_519_211_335_680    u64::MAX(sat.)               u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NDOCTC:  1^22 + 1^22 = 2. ✓
//     NHDOCTC: (1+1)^21 = 2^21 = 2_097_152. ✓
//     NQSO:    (1²+1²)^16 = 2^16 = 65_536. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NDOCTC:  3×2^22 = 3×4_194_304 = 12_582_912. ✓
//     NHDOCTC: 2×(2+2)^21 = 2×4^21 = 2×4_398_046_511_104 = 8_796_093_022_208. ✓
//       (4^20=1_099_511_627_776; 4^21=4×1_099_511_627_776=4_398_046_511_104)
//     NQSO:    2×(4+4)^16 = 2×8^16 = 2×281_474_976_710_656 = 562_949_953_421_312. ✓
//       (8^8=16_777_216; 8^16=16_777_216^2=281_474_976_710_656)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NDOCTC:  3×4^22 = 3×17_592_186_044_416 = 52_776_558_133_248. ✓
//       (4^21=4_398_046_511_104; 4^22=4×4_398_046_511_104=17_592_186_044_416)
//     NHDOCTC: 3×(4+4)^21 = 3×8^21 = 3×9_223_372_036_854_775_808 = 27_670_116_110_564_327_424 >> u64::MAX → SAT. ✓
//       (8^20=1_152_921_504_606_846_976; 8^21=8×1_152_921_504_606_846_976=9_223_372_036_854_775_808)
//     NQSO:    3×(16+16)^16 = 3×32^16 → SATURATES (32^16=2^80>>u64::MAX per-edge). ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NDOCTC:  5×4^22 = 5×17_592_186_044_416 = 87_960_930_222_080. ✓
//     NHDOCTC: 4×8^21 = 4×9_223_372_036_854_775_808 = 36_893_488_147_419_103_232 >> u64::MAX → SAT. ✓
//     NQSO:    4×32^16 → SATURATES (per-edge >> u64::MAX). ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NDOCTC:  2^22+3^22+3^22+2^22 = 2×4_194_304+2×31_381_059_609 = 62_770_507_826. ✓
//       (3^21=10_460_353_203; 3^22=3×10_460_353_203=31_381_059_609)
//     NHDOCTC: 5^21+6^21+5^21
//       5^21: 5^20=95_367_431_640_625; 5^21=5×95_367_431_640_625=476_837_158_203_125
//       6^21: 6^20=3_656_158_440_062_976; 6^21=6×3_656_158_440_062_976=21_936_950_640_377_856
//       953_674_316_406_250+21_936_950_640_377_856+953_674_316_406_250=22_890_624_956_784_106 (fits u64). ✓
//       (total < u64::MAX=18_446_744_073_709_551_615? NO: 22_890... > 18_446... → wait)
//       Actually 22_890_624_956_784_106 < 18_446_744_073_709_551_615? Let me check:
//       22_890_624... × 10^15 vs 18_446_744... × 10^15 → 22_890 > 18_446 → NO!
//       Wait: 22_890_624_956_784_106 in full:
//         = 2.2890... × 10^16
//       u64::MAX = 1.8446... × 10^19
//       So 2.29×10^16 < 1.84×10^19 → YES it fits! ✓
//     NQSO:    13^16+18^16+13^16
//       13^16: 13^8=815_730_721; 13^16=815_730_721^2=665_416_609_183_179_841 (fits u64)
//       18^16: 18^8=11_019_960_576; 18^16=11_019_960_576^2=121_439_531_096_594_251_776 >> u64::MAX
//       Total >> u64::MAX → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NDOCTC:  4×9^22 → SATURATES (9^22>>u64::MAX per vertex). ✓
//     NHDOCTC: 6×18^22 → SATURATES (18^22>>u64::MAX per-edge). ✓
//     NQSO:    6×162^16 → SATURATES (162^16>>u64::MAX per-edge). ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NDOCTC:  5×6^22 = 5×131_621_703_842_267_136 = 658_108_519_211_335_680 (fits u64). ✓
//       (6^21=21_936_950_640_377_856; 6^22=6×21_936_950_640_377_856=131_621_703_842_267_136;
//        5×131_621_703_842_267_136=658_108_519_211_335_680 < u64::MAX≈1.84×10¹⁹)
//     NHDOCTC: 6×12^21 → SATURATES (12^21>>u64::MAX per-edge). ✓
//     NQSO:    6×72^16 → SATURATES (72^16>>u64::MAX per-edge). ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NDOCTC  = n·S^22                                  for S-regular ✓
//   NHDOCTC = |E|·(2S)^21 = 2097152|E|·S^21          for S-regular ✓
//   NQSO    = |E|·(2S²)^16 = 65536|E|·S^32           for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 2_097_152, 65_536, 1, 2)
//  4.  Path P₃ = A-B-C                   → (12_582_912, 8_796_093_022_208, 562_949_953_421_312, 2, 3)
//  5.  Triangle K₃                       → (52_776_558_133_248, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (87_960_930_222_080, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (62_770_507_826, 22_890_624_956_784_106, u64::MAX, 3, 4)
//  8.  Complete K₄                       → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (658_108_519_211_335_680, u64::MAX, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T48_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_48");
const T48_EXEC:   ExecutorId = ExecutorId::from_ascii("t48.exec");

const T48_KEY_A: &str = "t48.alpha";
const T48_KEY_B: &str = "t48.beta";
const T48_KEY_C: &str = "t48.gamma";
const T48_KEY_D: &str = "t48.delta";
const T48_KEY_E: &str = "t48.epsilon";

const T48_ID_A: NodeId = derive_node_id(T48_PLUGIN, T48_KEY_A);
const T48_ID_B: NodeId = derive_node_id(T48_PLUGIN, T48_KEY_B);
const T48_ID_C: NodeId = derive_node_id(T48_PLUGIN, T48_KEY_C);
const T48_ID_D: NodeId = derive_node_id(T48_PLUGIN, T48_KEY_D);
const T48_ID_E: NodeId = derive_node_id(T48_PLUGIN, T48_KEY_E);

// L4=135 namespace for this harness.
const T48_VEC_A: VectorAddress = VectorAddress::new(135, 1, 1, 0);
const T48_VEC_B: VectorAddress = VectorAddress::new(135, 1, 2, 0);
const T48_VEC_C: VectorAddress = VectorAddress::new(135, 1, 3, 0);
const T48_VEC_D: VectorAddress = VectorAddress::new(135, 2, 1, 0);
const T48_VEC_E: VectorAddress = VectorAddress::new(135, 2, 2, 0);

const T48_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T48_PLUGIN,
    name:         "kl-graph-topo48-harness",
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
        executor_id:       T48_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T48_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T48_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      0, "empty: node_count=0");
    assert_eq!(ec,      0, "empty: edge_count=0");
    assert_eq!(ndoctc,  0, "empty: NDOCTC=0");
    assert_eq!(nhdoctc, 0, "empty: NHDOCTC=0");
    assert_eq!(nqso,    0, "empty: NQSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NDOCTC: 0^22=0; NHDOCTC: no edges; NQSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      1, "single: node_count=1");
    assert_eq!(ec,      0, "single: no edges");
    assert_eq!(ndoctc,  0, "single: NDOCTC=0 (S=0; 0^22=0)");
    assert_eq!(nhdoctc, 0, "single: NHDOCTC=0 (no edges)");
    assert_eq!(nqso,    0, "single: NQSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NDOCTC:  1^22+1^22 = 2.
// NHDOCTC: (1+1)^21 = 2^21 = 2_097_152.
// NQSO:    (1²+1²)^16 = 2^16 = 65_536.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);
    add_edge(T48_ID_A, T48_ID_B, "t48.e.ab");

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      2,         "k2: node_count=2");
    assert_eq!(ec,      1,         "k2: edge_count=1");
    assert_eq!(ndoctc,  2,         "k2: NDOCTC=2 (1\u{00b2}\u{00b2}+1\u{00b2}\u{00b2}=2; S-uniform S=1)");
    assert_eq!(nhdoctc, 2_097_152, "k2: NHDOCTC=2_097_152 ((1+1)\u{00b2}\u{00b9}=2\u{00b2}\u{00b9}=2_097_152; S-uniform S=1)");
    assert_eq!(nqso,    65_536,    "k2: NQSO=65_536 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{2076}=2\u{00b9}\u{2076}=65_536; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NDOCTC:  3×2^22 = 3×4_194_304 = 12_582_912.
// NHDOCTC: 2×(2+2)^21 = 2×4^21 = 2×4_398_046_511_104 = 8_796_093_022_208.
// NQSO:    2×(4+4)^16 = 2×8^16 = 2×281_474_976_710_656 = 562_949_953_421_312.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);
    add_node(T48_VEC_C, T48_KEY_C, T48_ID_C);
    add_edge(T48_ID_A, T48_ID_B, "t48.e.ab");
    add_edge(T48_ID_B, T48_ID_C, "t48.e.bc");

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      3,                    "p3: node_count=3");
    assert_eq!(ec,      2,                    "p3: edge_count=2");
    assert_eq!(ndoctc,  12_582_912,           "p3: NDOCTC=12_582_912 (3\u{00d7}4_194_304; 2\u{00b2}\u{00b2}=4_194_304; S-uniform S=2)");
    assert_eq!(nhdoctc, 8_796_093_022_208,    "p3: NHDOCTC=8_796_093_022_208 (2\u{00d7}4_398_046_511_104; (2+2)\u{00b2}\u{00b9}=4\u{00b2}\u{00b9}=4_398_046_511_104; S-uniform S=2)");
    assert_eq!(nqso,    562_949_953_421_312,  "p3: NQSO=562_949_953_421_312 (2\u{00d7}281_474_976_710_656; (4+4)\u{00b9}\u{2076}=8\u{00b9}\u{2076}=281_474_976_710_656; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NDOCTC:  3×4^22 = 3×17_592_186_044_416 = 52_776_558_133_248.
// NHDOCTC: 3×(4+4)^21 = 3×8^21 = 3×9_223_372_036_854_775_808 → SATURATES.
// NQSO:    3×(16+16)^16 = 3×32^16 → SATURATES (32^16=2^80>>u64::MAX per-edge).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);
    add_node(T48_VEC_C, T48_KEY_C, T48_ID_C);
    add_edge(T48_ID_A, T48_ID_B, "t48.e.ab");
    add_edge(T48_ID_B, T48_ID_A, "t48.e.ba");
    add_edge(T48_ID_B, T48_ID_C, "t48.e.bc");
    add_edge(T48_ID_C, T48_ID_B, "t48.e.cb");
    add_edge(T48_ID_A, T48_ID_C, "t48.e.ac");
    add_edge(T48_ID_C, T48_ID_A, "t48.e.ca");

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      3,                    "k3: node_count=3");
    assert_eq!(ec,      3,                    "k3: edge_count=3");
    assert_eq!(ndoctc,  52_776_558_133_248,   "k3: NDOCTC=52_776_558_133_248 (3\u{00d7}17_592_186_044_416; 4\u{00b2}\u{00b2}=17_592_186_044_416; S-uniform S=4)");
    assert_eq!(nhdoctc, u64::MAX,             "k3: NHDOCTC=u64::MAX (3\u{00d7}8\u{00b2}\u{00b9}=3\u{00d7}9_223_372_036_854_775_808 >> u64::MAX; saturated)");
    assert_eq!(nqso,    u64::MAX,             "k3: NQSO=u64::MAX (3\u{00d7}32\u{00b9}\u{2076} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NDOCTC:  5×4^22 = 5×17_592_186_044_416 = 87_960_930_222_080.
// NHDOCTC: 4×8^21 = 4×9_223_372_036_854_775_808 → SATURATES.
// NQSO:    4×32^16 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);
    add_node(T48_VEC_C, T48_KEY_C, T48_ID_C);
    add_node(T48_VEC_D, T48_KEY_D, T48_ID_D);
    add_node(T48_VEC_E, T48_KEY_E, T48_ID_E);
    add_edge(T48_ID_A, T48_ID_B, "t48.e.ab");
    add_edge(T48_ID_A, T48_ID_C, "t48.e.ac");
    add_edge(T48_ID_A, T48_ID_D, "t48.e.ad");
    add_edge(T48_ID_A, T48_ID_E, "t48.e.ae");

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      5,                   "star: node_count=5");
    assert_eq!(ec,      4,                   "star: edge_count=4");
    assert_eq!(ndoctc,  87_960_930_222_080,  "star: NDOCTC=87_960_930_222_080 (5\u{00d7}17_592_186_044_416; same S as K\u{2083})");
    assert_eq!(nhdoctc, u64::MAX,            "star: NHDOCTC=u64::MAX (4\u{00d7}8\u{00b2}\u{00b9} >> u64::MAX; saturated)");
    assert_eq!(nqso,    u64::MAX,            "star: NQSO=u64::MAX (4\u{00d7}32\u{00b9}\u{2076} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NDOCTC:  2^22+3^22+3^22+2^22 = 2×4_194_304+2×31_381_059_609 = 62_770_507_826.
// NHDOCTC: 5^21+6^21+5^21
//   = 476_837_158_203_125+21_936_950_640_377_856+476_837_158_203_125 = 22_890_624_956_784_106.
// NQSO:    13^16+18^16+13^16 → SATURATES (18^16≈1.21×10²⁰ >> u64::MAX per-edge).

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);
    add_node(T48_VEC_C, T48_KEY_C, T48_ID_C);
    add_node(T48_VEC_D, T48_KEY_D, T48_ID_D);
    add_edge(T48_ID_A, T48_ID_B, "t48.e.ab");
    add_edge(T48_ID_B, T48_ID_C, "t48.e.bc");
    add_edge(T48_ID_C, T48_ID_D, "t48.e.cd");

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      4,                          "p4: node_count=4");
    assert_eq!(ec,      3,                          "p4: edge_count=3");
    assert_eq!(ndoctc,  62_770_507_826,             "p4: NDOCTC=62_770_507_826 (2\u{00d7}4_194_304+2\u{00d7}31_381_059_609; 2\u{00b2}\u{00b2}+3\u{00b2}\u{00b2}+3\u{00b2}\u{00b2}+2\u{00b2}\u{00b2})");
    assert_eq!(nhdoctc, 22_890_624_956_784_106,     "p4: NHDOCTC=22_890_624_956_784_106 (476_837_158_203_125+21_936_950_640_377_856+476_837_158_203_125; 5\u{00b2}\u{00b9}+6\u{00b2}\u{00b9}+5\u{00b2}\u{00b9})");
    assert_eq!(nqso,    u64::MAX,                   "p4: NQSO=u64::MAX (18\u{00b9}\u{2076}\u{2248}1.21\u{00d7}10\u{00b2}\u{2070} >> u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NDOCTC:  4×9^22 → SATURATES → u64::MAX.
// NHDOCTC: 6×18^21 → SATURATES → u64::MAX.
// NQSO:    6×162^16 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);
    add_node(T48_VEC_C, T48_KEY_C, T48_ID_C);
    add_node(T48_VEC_D, T48_KEY_D, T48_ID_D);
    add_edge(T48_ID_A, T48_ID_B, "t48.e.ab");
    add_edge(T48_ID_B, T48_ID_A, "t48.e.ba");
    add_edge(T48_ID_A, T48_ID_C, "t48.e.ac");
    add_edge(T48_ID_C, T48_ID_A, "t48.e.ca");
    add_edge(T48_ID_A, T48_ID_D, "t48.e.ad");
    add_edge(T48_ID_D, T48_ID_A, "t48.e.da");
    add_edge(T48_ID_B, T48_ID_C, "t48.e.bc");
    add_edge(T48_ID_C, T48_ID_B, "t48.e.cb");
    add_edge(T48_ID_B, T48_ID_D, "t48.e.bd");
    add_edge(T48_ID_D, T48_ID_B, "t48.e.db");
    add_edge(T48_ID_C, T48_ID_D, "t48.e.cd");
    add_edge(T48_ID_D, T48_ID_C, "t48.e.dc");

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      4,        "k4: node_count=4");
    assert_eq!(ec,      6,        "k4: edge_count=6");
    assert_eq!(ndoctc,  u64::MAX, "k4: NDOCTC=u64::MAX (4\u{00d7}9\u{00b2}\u{00b2} >> u64::MAX; saturated)");
    assert_eq!(nhdoctc, u64::MAX, "k4: NHDOCTC=u64::MAX (6\u{00d7}18\u{00b2}\u{00b9} >> u64::MAX; saturated)");
    assert_eq!(nqso,    u64::MAX, "k4: NQSO=u64::MAX (6\u{00d7}162\u{00b9}\u{2076} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NDOCTC=0; NHDOCTC=0; NQSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      2, "two-iso: node_count=2");
    assert_eq!(ec,      0, "two-iso: edge_count=0");
    assert_eq!(ndoctc,  0, "two-iso: NDOCTC=0");
    assert_eq!(nhdoctc, 0, "two-iso: NHDOCTC=0");
    assert_eq!(nqso,    0, "two-iso: NQSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NDOCTC:  5×6^22 = 5×131_621_703_842_267_136 = 658_108_519_211_335_680 (exact, fits u64).
// NHDOCTC: 6×12^21 → SATURATES (12^21>>u64::MAX per-edge).
// NQSO:    6×72^16 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T48_VEC_A, T48_KEY_A, T48_ID_A);
    add_node(T48_VEC_B, T48_KEY_B, T48_ID_B);
    add_node(T48_VEC_C, T48_KEY_C, T48_ID_C);
    add_node(T48_VEC_D, T48_KEY_D, T48_ID_D);
    add_node(T48_VEC_E, T48_KEY_E, T48_ID_E);
    add_edge(T48_ID_A, T48_ID_C, "t48.e.ac");
    add_edge(T48_ID_A, T48_ID_D, "t48.e.ad");
    add_edge(T48_ID_A, T48_ID_E, "t48.e.ae");
    add_edge(T48_ID_B, T48_ID_C, "t48.e.bc");
    add_edge(T48_ID_B, T48_ID_D, "t48.e.bd");
    add_edge(T48_ID_B, T48_ID_E, "t48.e.be");

    let (ndoctc, nhdoctc, nqso, ec, nc) = gos_runtime::graph_topo_indices48();
    assert_eq!(nc,      5,        "k23: node_count=5");
    assert_eq!(ec,      6,        "k23: edge_count=6");
    assert_eq!(ndoctc,  658_108_519_211_335_680, "k23: NDOCTC=658_108_519_211_335_680 (5\u{00d7}6\u{00b2}\u{00b2}=5\u{00d7}131_621_703_842_267_136; fits u64)");
    assert_eq!(nhdoctc, u64::MAX,               "k23: NHDOCTC=u64::MAX (6\u{00d7}12\u{00b2}\u{00b9} >> u64::MAX; per-edge saturates)");
    assert_eq!(nqso,    u64::MAX,               "k23: NQSO=u64::MAX (6\u{00d7}72\u{00b9}\u{2076} >> u64::MAX; per-edge saturates)");
}
