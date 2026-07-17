// gos-graph-topo49-harness — V3.60 NTRICTC + NHTRICTC + NRSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices49()`:
//   Returns (ntrictc, nhtrictc, nrso, edge_count, node_count)
//   - ntrictc  = NTRICTC(G)  = Σ_v S(v)^23                  (exact u64; S-Tricosic vertex sum)
//   - nhtrictc = NHTRICTC(G) = Σ_{uv∈E} (S_u+S_v)^22        (exact u64; S-Docosic edge-sum)
//   - nrso     = NRSO(G)     = Σ_{uv∈E} (S_u²+S_v²)^17      (exact u64; S-Tetratriacontyl Sombor, α=34)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTRICTC(G) = Σ_v S(v)^23
//     S-Tricosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49).
//     NTRICTC = n·S^23 for S-regular.
//     Overflow: S^23 ≤ 16129^23 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHTRICTC(G) = Σ_{uv∈E} (S_u+S_v)^22
//     S-Docosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49).
//     NHTRICTC = |E|·(2S)^22 = 4194304|E|·S^22 for S-regular.
//     Overflow per edge: (2×16129)^22 → saturating u128 accumulator.
//
//   NRSO(G) = Σ_{uv∈E} (S_u²+S_v²)^17
//     S-Tetratriacontyl Sombor: generalised Sombor SO^α with α=34 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34).
//     (R used: O=α=8 taken; P=α=30 taken; Q=α=32 taken; R follows in sequence)
//     NRSO = |E|·(2S²)^17 = 131072|E|·S^34 for S-regular.
//     Overflow per edge: (2×16129²)^17 → saturating u128 accumulator.
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
//  Graph     NTRICTC(exact)              NHTRICTC(exact)               NRSO(exact)              edges  nodes
//  Empty                  0                            0                         0               0      0
//  1 node                 0                            0                         0               0      1
//  K₂                     2                    4_194_304                   131_072               1      2
//  P₃              25_165_824           35_184_372_088_832     4_503_599_627_370_496              2      3
//  K₃         211_106_232_532_992       u64::MAX(sat.)              u64::MAX(sat.)               3      3
//  K_{1,4}    351_843_720_888_320       u64::MAX(sat.)              u64::MAX(sat.)               4      5
//  P₄            188_303_134_870    136_390_075_424_298_386         u64::MAX(sat.)               3      4
//  K₄          u64::MAX(sat.)          u64::MAX(sat.)               u64::MAX(sat.)               6      4
//  2 isolated             0                            0                         0               0      2
//  K_{2,3}  3_948_651_115_268_014_080  u64::MAX(sat.)               u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTRICTC:  1^23 + 1^23 = 2. ✓
//     NHTRICTC: (1+1)^22 = 2^22 = 4_194_304. ✓
//     NRSO:     (1²+1²)^17 = 2^17 = 131_072. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTRICTC:  3×2^23 = 3×8_388_608 = 25_165_824. ✓
//     NHTRICTC: 2×(2+2)^22 = 2×4^22 = 2×17_592_186_044_416 = 35_184_372_088_832. ✓
//       (4^11=4_194_304; 4^22=(4^11)²=4_194_304²=17_592_186_044_416)
//     NRSO:     2×(4+4)^17 = 2×8^17.
//       8^16=281_474_976_710_656; 8^17=8×281_474_976_710_656=2_251_799_813_685_248
//       2×2_251_799_813_685_248=4_503_599_627_370_496 (fits u64). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTRICTC:  3×4^23 = 3×70_368_744_177_664 = 211_106_232_532_992 (fits u64). ✓
//       (4^22=17_592_186_044_416; 4^23=4×17_592_186_044_416=70_368_744_177_664)
//     NHTRICTC: 3×(4+4)^22 = 3×8^22 → SATURATES (8^21=9_223_372_036_854_775_808; 8^22>>u64::MAX per-edge). ✓
//     NRSO:     3×(16+16)^17 = 3×32^17 → SATURATES (32^17=2^85>>u64::MAX per-edge). ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTRICTC:  5×4^23 = 5×70_368_744_177_664 = 351_843_720_888_320 (fits u64). ✓
//     NHTRICTC: 4×8^22 → SATURATES. ✓
//     NRSO:     4×32^17 → SATURATES (per-edge >> u64::MAX). ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTRICTC:  2^23+3^23+3^23+2^23 = 2×8_388_608+2×94_143_178_827 = 188_303_134_870. ✓
//       (3^22=31_381_059_609; 3^23=3×31_381_059_609=94_143_178_827)
//     NHTRICTC: 5^22+6^22+5^22
//       5^22: 5^21=476_837_158_203_125; 5^22=5×476_837_158_203_125=2_384_185_791_015_625
//       6^22: 6^21=21_936_950_640_377_856; 6^22=6×21_936_950_640_377_856=131_621_703_842_267_136
//       2×2_384_185_791_015_625+131_621_703_842_267_136=4_768_371_582_031_250+131_621_703_842_267_136
//       =136_390_075_424_298_386 (fits u64: 1.36×10^17 < 1.84×10^19). ✓
//     NRSO: 13^17+18^17+13^17
//       13^16=665_416_609_183_179_841; 13^17=13×665_416_609_183_179_841=8_650_415_919_381_337_933
//       18^16=121_439_531_096_594_251_776 >> u64::MAX → 18^17 >> u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTRICTC:  4×9^23 → SATURATES (9^22≈9.85×10^20>>u64::MAX per-vertex). ✓
//     NHTRICTC: 6×18^22 → SATURATES. ✓
//     NRSO:     6×162^17 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTRICTC:  5×6^23 = 5×789_730_223_053_602_816 = 3_948_651_115_268_014_080 (fits u64). ✓
//       (6^22=131_621_703_842_267_136; 6^23=6×131_621_703_842_267_136=789_730_223_053_602_816)
//     NHTRICTC: 6×12^22 → SATURATES (12^22≈9.85×10^24>>u64::MAX per-edge). ✓
//     NRSO:     6×72^17 → SATURATES (72^16>>u64::MAX per-edge). ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTRICTC  = n·S^23                                   for S-regular ✓
//   NHTRICTC = |E|·(2S)^22 = 4194304|E|·S^22           for S-regular ✓
//   NRSO     = |E|·(2S²)^17 = 131072|E|·S^34           for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 4_194_304, 131_072, 1, 2)
//  4.  Path P₃ = A-B-C                   → (25_165_824, 35_184_372_088_832, 4_503_599_627_370_496, 2, 3)
//  5.  Triangle K₃                       → (211_106_232_532_992, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (351_843_720_888_320, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (188_303_134_870, 136_390_075_424_298_386, u64::MAX, 3, 4)
//  8.  Complete K₄                       → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (3_948_651_115_268_014_080, u64::MAX, u64::MAX, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T49_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_49");
const T49_EXEC:   ExecutorId = ExecutorId::from_ascii("t49.exec");

const T49_KEY_A: &str = "t49.alpha";
const T49_KEY_B: &str = "t49.beta";
const T49_KEY_C: &str = "t49.gamma";
const T49_KEY_D: &str = "t49.delta";
const T49_KEY_E: &str = "t49.epsilon";

const T49_ID_A: NodeId = derive_node_id(T49_PLUGIN, T49_KEY_A);
const T49_ID_B: NodeId = derive_node_id(T49_PLUGIN, T49_KEY_B);
const T49_ID_C: NodeId = derive_node_id(T49_PLUGIN, T49_KEY_C);
const T49_ID_D: NodeId = derive_node_id(T49_PLUGIN, T49_KEY_D);
const T49_ID_E: NodeId = derive_node_id(T49_PLUGIN, T49_KEY_E);

// L4=136 namespace for this harness.
const T49_VEC_A: VectorAddress = VectorAddress::new(136, 1, 1, 0);
const T49_VEC_B: VectorAddress = VectorAddress::new(136, 1, 2, 0);
const T49_VEC_C: VectorAddress = VectorAddress::new(136, 1, 3, 0);
const T49_VEC_D: VectorAddress = VectorAddress::new(136, 2, 1, 0);
const T49_VEC_E: VectorAddress = VectorAddress::new(136, 2, 2, 0);

const T49_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T49_PLUGIN,
    name:         "kl-graph-topo49-harness",
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
        executor_id:       T49_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T49_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T49_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       0, "empty: node_count=0");
    assert_eq!(ec,       0, "empty: edge_count=0");
    assert_eq!(ntrictc,  0, "empty: NTRICTC=0");
    assert_eq!(nhtrictc, 0, "empty: NHTRICTC=0");
    assert_eq!(nrso,     0, "empty: NRSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NTRICTC: 0^23=0; NHTRICTC: no edges; NRSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       1, "single: node_count=1");
    assert_eq!(ec,       0, "single: no edges");
    assert_eq!(ntrictc,  0, "single: NTRICTC=0 (S=0; 0^23=0)");
    assert_eq!(nhtrictc, 0, "single: NHTRICTC=0 (no edges)");
    assert_eq!(nrso,     0, "single: NRSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NTRICTC:  1^23+1^23 = 2.
// NHTRICTC: (1+1)^22 = 2^22 = 4_194_304.
// NRSO:     (1²+1²)^17 = 2^17 = 131_072.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);
    add_edge(T49_ID_A, T49_ID_B, "t49.e.ab");

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       2,         "k2: node_count=2");
    assert_eq!(ec,       1,         "k2: edge_count=1");
    assert_eq!(ntrictc,  2,         "k2: NTRICTC=2 (1\u{00b2}\u{00b3}+1\u{00b2}\u{00b3}=2; S-uniform S=1)");
    assert_eq!(nhtrictc, 4_194_304, "k2: NHTRICTC=4_194_304 ((1+1)\u{00b2}\u{00b2}=2\u{00b2}\u{00b2}=4_194_304; S-uniform S=1)");
    assert_eq!(nrso,     131_072,   "k2: NRSO=131_072 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{2077}=2\u{00b9}\u{2077}=131_072; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NTRICTC:  3×2^23 = 3×8_388_608 = 25_165_824.
// NHTRICTC: 2×(2+2)^22 = 2×4^22 = 2×17_592_186_044_416 = 35_184_372_088_832.
// NRSO:     2×(4+4)^17 = 2×8^17 = 2×2_251_799_813_685_248 = 4_503_599_627_370_496.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);
    add_node(T49_VEC_C, T49_KEY_C, T49_ID_C);
    add_edge(T49_ID_A, T49_ID_B, "t49.e.ab");
    add_edge(T49_ID_B, T49_ID_C, "t49.e.bc");

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       3,                     "p3: node_count=3");
    assert_eq!(ec,       2,                     "p3: edge_count=2");
    assert_eq!(ntrictc,  25_165_824,            "p3: NTRICTC=25_165_824 (3\u{00d7}8_388_608; 2\u{00b2}\u{00b3}=8_388_608; S-uniform S=2)");
    assert_eq!(nhtrictc, 35_184_372_088_832,    "p3: NHTRICTC=35_184_372_088_832 (2\u{00d7}17_592_186_044_416; (2+2)\u{00b2}\u{00b2}=4\u{00b2}\u{00b2}=17_592_186_044_416; S-uniform S=2)");
    assert_eq!(nrso,     4_503_599_627_370_496, "p3: NRSO=4_503_599_627_370_496 (2\u{00d7}2_251_799_813_685_248; (4+4)\u{00b9}\u{2077}=8\u{00b9}\u{2077}=2_251_799_813_685_248; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NTRICTC:  3×4^23 = 3×70_368_744_177_664 = 211_106_232_532_992 (fits u64).
// NHTRICTC: 3×(4+4)^22 = 3×8^22 → SATURATES (8^21=9_223_372_036_854_775_808; 8^22>>u64::MAX).
// NRSO:     3×(16+16)^17 = 3×32^17 → SATURATES (32^17=2^85>>u64::MAX per-edge).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);
    add_node(T49_VEC_C, T49_KEY_C, T49_ID_C);
    add_edge(T49_ID_A, T49_ID_B, "t49.e.ab");
    add_edge(T49_ID_B, T49_ID_A, "t49.e.ba");
    add_edge(T49_ID_B, T49_ID_C, "t49.e.bc");
    add_edge(T49_ID_C, T49_ID_B, "t49.e.cb");
    add_edge(T49_ID_A, T49_ID_C, "t49.e.ac");
    add_edge(T49_ID_C, T49_ID_A, "t49.e.ca");

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       3,                    "k3: node_count=3");
    assert_eq!(ec,       3,                    "k3: edge_count=3");
    assert_eq!(ntrictc,  211_106_232_532_992,  "k3: NTRICTC=211_106_232_532_992 (3\u{00d7}70_368_744_177_664; 4\u{00b2}\u{00b3}=70_368_744_177_664; S-uniform S=4)");
    assert_eq!(nhtrictc, u64::MAX,             "k3: NHTRICTC=u64::MAX (3\u{00d7}8\u{00b2}\u{00b2}=3\u{00d7}73_786_976_294_838_206_464 >> u64::MAX; saturated)");
    assert_eq!(nrso,     u64::MAX,             "k3: NRSO=u64::MAX (3\u{00d7}32\u{00b9}\u{2077}=3\u{00d7}2\u{2078}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NTRICTC:  5×4^23 = 5×70_368_744_177_664 = 351_843_720_888_320 (fits u64).
// NHTRICTC: 4×8^22 → SATURATES.
// NRSO:     4×32^17 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);
    add_node(T49_VEC_C, T49_KEY_C, T49_ID_C);
    add_node(T49_VEC_D, T49_KEY_D, T49_ID_D);
    add_node(T49_VEC_E, T49_KEY_E, T49_ID_E);
    add_edge(T49_ID_A, T49_ID_B, "t49.e.ab");
    add_edge(T49_ID_A, T49_ID_C, "t49.e.ac");
    add_edge(T49_ID_A, T49_ID_D, "t49.e.ad");
    add_edge(T49_ID_A, T49_ID_E, "t49.e.ae");

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       5,                   "star: node_count=5");
    assert_eq!(ec,       4,                   "star: edge_count=4");
    assert_eq!(ntrictc,  351_843_720_888_320, "star: NTRICTC=351_843_720_888_320 (5\u{00d7}70_368_744_177_664; same S as K\u{2083})");
    assert_eq!(nhtrictc, u64::MAX,            "star: NHTRICTC=u64::MAX (4\u{00d7}8\u{00b2}\u{00b2} >> u64::MAX; saturated)");
    assert_eq!(nrso,     u64::MAX,            "star: NRSO=u64::MAX (4\u{00d7}32\u{00b9}\u{2077} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NTRICTC:  2^23+3^23+3^23+2^23 = 2×8_388_608+2×94_143_178_827 = 188_303_134_870.
// NHTRICTC: 5^22+6^22+5^22 = 2×2_384_185_791_015_625+131_621_703_842_267_136
//           = 136_390_075_424_298_386 (fits u64).
// NRSO:     13^17+18^17+13^17 → SATURATES (18^17>>u64::MAX per-edge).

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);
    add_node(T49_VEC_C, T49_KEY_C, T49_ID_C);
    add_node(T49_VEC_D, T49_KEY_D, T49_ID_D);
    add_edge(T49_ID_A, T49_ID_B, "t49.e.ab");
    add_edge(T49_ID_B, T49_ID_C, "t49.e.bc");
    add_edge(T49_ID_C, T49_ID_D, "t49.e.cd");

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       4,                          "p4: node_count=4");
    assert_eq!(ec,       3,                          "p4: edge_count=3");
    assert_eq!(ntrictc,  188_303_134_870,            "p4: NTRICTC=188_303_134_870 (2\u{00d7}8_388_608+2\u{00d7}94_143_178_827; 2\u{00b2}\u{00b3}+3\u{00b2}\u{00b3}+3\u{00b2}\u{00b3}+2\u{00b2}\u{00b3})");
    assert_eq!(nhtrictc, 136_390_075_424_298_386,    "p4: NHTRICTC=136_390_075_424_298_386 (2\u{00d7}2_384_185_791_015_625+131_621_703_842_267_136; 5\u{00b2}\u{00b2}+6\u{00b2}\u{00b2}+5\u{00b2}\u{00b2})");
    assert_eq!(nrso,     u64::MAX,                   "p4: NRSO=u64::MAX (18\u{00b9}\u{2077}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NTRICTC:  4×9^23 → SATURATES → u64::MAX.
// NHTRICTC: 6×18^22 → SATURATES → u64::MAX.
// NRSO:     6×162^17 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);
    add_node(T49_VEC_C, T49_KEY_C, T49_ID_C);
    add_node(T49_VEC_D, T49_KEY_D, T49_ID_D);
    add_edge(T49_ID_A, T49_ID_B, "t49.e.ab");
    add_edge(T49_ID_B, T49_ID_A, "t49.e.ba");
    add_edge(T49_ID_A, T49_ID_C, "t49.e.ac");
    add_edge(T49_ID_C, T49_ID_A, "t49.e.ca");
    add_edge(T49_ID_A, T49_ID_D, "t49.e.ad");
    add_edge(T49_ID_D, T49_ID_A, "t49.e.da");
    add_edge(T49_ID_B, T49_ID_C, "t49.e.bc");
    add_edge(T49_ID_C, T49_ID_B, "t49.e.cb");
    add_edge(T49_ID_B, T49_ID_D, "t49.e.bd");
    add_edge(T49_ID_D, T49_ID_B, "t49.e.db");
    add_edge(T49_ID_C, T49_ID_D, "t49.e.cd");
    add_edge(T49_ID_D, T49_ID_C, "t49.e.dc");

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       4,        "k4: node_count=4");
    assert_eq!(ec,       6,        "k4: edge_count=6");
    assert_eq!(ntrictc,  u64::MAX, "k4: NTRICTC=u64::MAX (4\u{00d7}9\u{00b2}\u{00b3} >> u64::MAX; saturated)");
    assert_eq!(nhtrictc, u64::MAX, "k4: NHTRICTC=u64::MAX (6\u{00d7}18\u{00b2}\u{00b2} >> u64::MAX; saturated)");
    assert_eq!(nrso,     u64::MAX, "k4: NRSO=u64::MAX (6\u{00d7}162\u{00b9}\u{2077} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NTRICTC=0; NHTRICTC=0; NRSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       2, "two-iso: node_count=2");
    assert_eq!(ec,       0, "two-iso: edge_count=0");
    assert_eq!(ntrictc,  0, "two-iso: NTRICTC=0");
    assert_eq!(nhtrictc, 0, "two-iso: NHTRICTC=0");
    assert_eq!(nrso,     0, "two-iso: NRSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NTRICTC:  5×6^23 = 5×789_730_223_053_602_816 = 3_948_651_115_268_014_080 (fits u64).
// NHTRICTC: 6×12^22 → SATURATES (12^22>>u64::MAX per-edge).
// NRSO:     6×72^17 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T49_VEC_A, T49_KEY_A, T49_ID_A);
    add_node(T49_VEC_B, T49_KEY_B, T49_ID_B);
    add_node(T49_VEC_C, T49_KEY_C, T49_ID_C);
    add_node(T49_VEC_D, T49_KEY_D, T49_ID_D);
    add_node(T49_VEC_E, T49_KEY_E, T49_ID_E);
    add_edge(T49_ID_A, T49_ID_C, "t49.e.ac");
    add_edge(T49_ID_A, T49_ID_D, "t49.e.ad");
    add_edge(T49_ID_A, T49_ID_E, "t49.e.ae");
    add_edge(T49_ID_B, T49_ID_C, "t49.e.bc");
    add_edge(T49_ID_B, T49_ID_D, "t49.e.bd");
    add_edge(T49_ID_B, T49_ID_E, "t49.e.be");

    let (ntrictc, nhtrictc, nrso, ec, nc) = gos_runtime::graph_topo_indices49();
    assert_eq!(nc,       5,        "k23: node_count=5");
    assert_eq!(ec,       6,        "k23: edge_count=6");
    assert_eq!(ntrictc,  3_948_651_115_268_014_080, "k23: NTRICTC=3_948_651_115_268_014_080 (5\u{00d7}6\u{00b2}\u{00b3}=5\u{00d7}789_730_223_053_602_816; fits u64)");
    assert_eq!(nhtrictc, u64::MAX,                  "k23: NHTRICTC=u64::MAX (6\u{00d7}12\u{00b2}\u{00b2} >> u64::MAX; per-edge saturates)");
    assert_eq!(nrso,     u64::MAX,                  "k23: NRSO=u64::MAX (6\u{00d7}72\u{00b9}\u{2077} >> u64::MAX; per-edge saturates)");
}
