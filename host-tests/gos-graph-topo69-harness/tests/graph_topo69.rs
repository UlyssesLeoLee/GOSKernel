// gos-graph-topo69-harness — V3.80 NTRITETRAACTC + NHTRITETRAACTC + NALSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices69()`:
//   Returns (ntritetraactc, nhtritetraactc, nalso, edge_count, node_count)
//   - ntritetraactc  = NTRITETRAACTC(G) = Σ_v S(v)^43                   (exact u64; S-Tritetracontic vertex sum)
//   - nhtritetraactc = NHTRITETRAACTC(G)= Σ_{uv∈E} (S_u+S_v)^42         (exact u64; S-Dotetracontic edge-sum)
//   - nalso           = NALSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^37       (exact u64; S-Tritetracontyl Sombor, α=74)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTRITETRAACTC(G) = Σ_v S(v)^43
//     S-Tritetracontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56), NHENTRIACTC=Σ S³¹ (topo57),
//       NDOTRIACTC=Σ S³² (topo58), NTRITRIACTC=Σ S³³ (topo59), NTETRTRIACTC=Σ S³⁴ (topo60),
//       NPENTTRIACTC=Σ S³⁵ (topo61), NHEXATRIACTC=Σ S³⁶ (topo62), NHEPTATRIACTC=Σ S³⁷ (topo63),
//       NOCTATRIACTC=Σ S³⁸ (topo64), NNONATRIACTC=Σ S³⁹ (topo65), NTETRAACTC=Σ S⁴⁰ (topo66),
//       NHENTETRAACTC=Σ S⁴¹ (topo67), NDOTETRAACTC=Σ S⁴² (topo68), NTRITETRAACTC=Σ S⁴³ (topo69).
//     NTRITETRAACTC = n·S^43 for S-regular.
//     Overflow: S^43 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^43 = s32 × s8 × s2 × s  (s32=s16^2; s8=s4^2; s2=s×s; 43=32+8+2+1).
//
//   NHTRITETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^42
//     S-Dotetracontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53), NHOCTATC=Σ(S+S)²⁷ (topo54), NHNONATC=Σ(S+S)²⁸ (topo55),
//       NHTRIACTC=Σ(S+S)²⁹ (topo56), NHHENTRIACTC=Σ(S+S)³⁰ (topo57),
//       NHDOTRIACTC=Σ(S+S)³¹ (topo58), NHTRITRIACTC=Σ(S+S)³² (topo59),
//       NHTETRTRIACTC=Σ(S+S)³³ (topo60), NHPENTTRIACTC=Σ(S+S)³⁴ (topo61),
//       NHHEXATRIACTC=Σ(S+S)³⁵ (topo62), NHHEPTATRIACTC=Σ(S+S)³⁶ (topo63),
//       NHOCTATRIACTC=Σ(S+S)³⁷ (topo64), NHNONATRIACTC=Σ(S+S)³⁸ (topo65),
//       NHTETRAACTC=Σ(S+S)³⁹ (topo66), NHHENTETRAACTC=Σ(S+S)⁴⁰ (topo67),
//       NHDOTETRAACTC=Σ(S+S)⁴¹ (topo68), NHTRITETRAACTC=Σ(S+S)⁴² (topo69).
//     NHTRITETRAACTC = |E|·(2S)^42 = 4398046511104|E|·S^42 for S-regular.
//     Overflow per edge: (2×16129)^42 → saturating u128 accumulator.
//     Implementation: ss^42 = ss32 × ss8 × ss2  (ss32=ss16^2; ss8=ss4^2; 42=32+8+2).
//
//   NALSO(G) = Σ_{uv∈E} (S_u²+S_v²)^37
//     S-Tritetracontyl Sombor: generalised Sombor SO^α with α=74 on S-variant.
//     3rd-pass double-letter "AL" (after NAKSO α=72, topo68).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60), NAFSO(topo63,α=62),
//     NAGSO(topo64,α=64), NAHSO(topo65,α=66), NAISO(topo66,α=68), NAJSO(topo67,α=70),
//     NAKSO(topo68,α=72), NALSO(topo69,α=74).
//     NALSO = |E|·(2S²)^37 = 137438953472|E|·S^74 for S-regular.
//     Overflow per edge: (2×16129²)^37 → saturating u128 accumulator.
//     Implementation: s2s^37 = s2s32 × s2s4 × s2s  (s2s32=s2s16^2; s2s4=s2s2^2; 37=32+4+1).
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
//  Graph     NTRITETRAACTC(exact)             NHTRITETRAACTC(exact)        NALSO(exact)             edges  nodes
//  Empty                      0                               0                        0               0      0
//  1 node                     0                               0                        0               0      1
//  K₂                         2               4_398_046_511_104             137_438_953_472               1      2
//  P₃        26_388_279_066_624              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                 0                               0                        0               0      2
//  K_{2,3}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTRITETRAACTC:  1^43 + 1^43 = 2. ✓
//     NHTRITETRAACTC: (1+1)^42 = 2^42 = 4_398_046_511_104. ✓
//     NALSO:           (1²+1²)^37 = 2^37 = 137_438_953_472. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTRITETRAACTC:  3×2^43 = 3×8_796_093_022_208 = 26_388_279_066_624. ✓
//     NHTRITETRAACTC: 2×(2+2)^42 = 2×4^42 = 2×2^84 → SATURATES. ✓
//     NALSO:           2×(4+4)^37 = 2×8^37 = 2×2^111 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTRITETRAACTC:  3×4^43 = 3×2^86 → SATURATES. ✓
//     NHTRITETRAACTC: 3×8^42 → SATURATES. ✓
//     NALSO:           3×32^37 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTRITETRAACTC:  5×4^43 → SATURATES. ✓
//     NHTRITETRAACTC: 4×8^42 → SATURATES. ✓
//     NALSO:           4×32^37 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTRITETRAACTC:  2×2^43 + 2×3^43.
//       3^41=36_472_996_377_170_786_403>u64::MAX; 3^43>>u64::MAX → SATURATES. ✓
//     NHTRITETRAACTC: 2×5^42 + 6^42 → each term >> u64::MAX → SATURATES. ✓
//     NALSO:           2×13^37 + 18^37 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTRITETRAACTC:  4×9^43 → SATURATES. ✓
//     NHTRITETRAACTC: 6×18^42 → SATURATES. ✓
//     NALSO:           6×162^37 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTRITETRAACTC:  5×6^43 → SATURATES. ✓
//     NHTRITETRAACTC: 6×12^42 → SATURATES. ✓
//     NALSO:           6×72^37 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTRITETRAACTC  = n·S^43                                                          for S-regular ✓
//   NHTRITETRAACTC = |E|·(2S)^42 = 4398046511104|E|·S^42                             for S-regular ✓
//   NALSO          = |E|·(2S²)^37 = 137438953472|E|·S^74                             for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 4_398_046_511_104, 137_438_953_472, 1, 2)
//  4.  Path P₃ = A-B-C                   → (26_388_279_066_624, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
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

const T69_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_69");
const T69_EXEC:   ExecutorId = ExecutorId::from_ascii("t69.exec");

const T69_KEY_A: &str = "t69.alpha";
const T69_KEY_B: &str = "t69.beta";
const T69_KEY_C: &str = "t69.gamma";
const T69_KEY_D: &str = "t69.delta";
const T69_KEY_E: &str = "t69.epsilon";

const T69_ID_A: NodeId = derive_node_id(T69_PLUGIN, T69_KEY_A);
const T69_ID_B: NodeId = derive_node_id(T69_PLUGIN, T69_KEY_B);
const T69_ID_C: NodeId = derive_node_id(T69_PLUGIN, T69_KEY_C);
const T69_ID_D: NodeId = derive_node_id(T69_PLUGIN, T69_KEY_D);
const T69_ID_E: NodeId = derive_node_id(T69_PLUGIN, T69_KEY_E);

// L4=156 namespace for this harness.
const T69_VEC_A: VectorAddress = VectorAddress::new(156, 1, 1, 0);
const T69_VEC_B: VectorAddress = VectorAddress::new(156, 1, 2, 0);
const T69_VEC_C: VectorAddress = VectorAddress::new(156, 1, 3, 0);
const T69_VEC_D: VectorAddress = VectorAddress::new(156, 2, 1, 0);
const T69_VEC_E: VectorAddress = VectorAddress::new(156, 2, 2, 0);

const T69_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T69_PLUGIN,
    name:         "kl-graph-topo69-harness",
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
        executor_id:       T69_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T69_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T69_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(ntritetraactc,    0, "empty: NTRITETRAACTC=0");
    assert_eq!(nhtritetraactc,   0, "empty: NHTRITETRAACTC=0");
    assert_eq!(nalso,            0, "empty: NALSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(ntritetraactc,    0, "single: NTRITETRAACTC=0");
    assert_eq!(nhtritetraactc,   0, "single: NHTRITETRAACTC=0");
    assert_eq!(nalso,            0, "single: NALSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NTRITETRAACTC:  1^43+1^43 = 2.
// NHTRITETRAACTC: (1+1)^42 = 2^42 = 4_398_046_511_104.
// NALSO:           (1²+1²)^37 = 2^37 = 137_438_953_472.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);
    add_edge(T69_ID_A, T69_ID_B, "t69.e.ab");

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,               2,                   "k2: node_count=2");
    assert_eq!(ec,               1,                   "k2: edge_count=1");
    assert_eq!(ntritetraactc,    2,                   "k2: NTRITETRAACTC=2 (1\u{2074}\u{00b3}+1\u{2074}\u{00b3}=2)");
    assert_eq!(nhtritetraactc,   4_398_046_511_104,   "k2: NHTRITETRAACTC=4_398_046_511_104 (2\u{2074}\u{00b2}=2^42)");
    assert_eq!(nalso,            137_438_953_472,     "k2: NALSO=137_438_953_472 (2\u{00b3}\u{2077}=2^37)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NTRITETRAACTC:  3×2^43 = 3×8_796_093_022_208 = 26_388_279_066_624.
// NHTRITETRAACTC: 2×(2+2)^42 = 2×4^42 = 2×2^84 → SATURATES.
// NALSO:           2×(4+4)^37 = 2×8^37 = 2×2^111 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);
    add_node(T69_VEC_C, T69_KEY_C, T69_ID_C);
    add_edge(T69_ID_A, T69_ID_B, "t69.e.ab");
    add_edge(T69_ID_B, T69_ID_C, "t69.e.bc");

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,               3,                    "p3: node_count=3");
    assert_eq!(ec,               2,                    "p3: edge_count=2");
    assert_eq!(ntritetraactc,    26_388_279_066_624,   "p3: NTRITETRAACTC=26_388_279_066_624 (3\u{00d7}2\u{2074}\u{00b3})");
    assert_eq!(nhtritetraactc,   u64::MAX,             "p3: NHTRITETRAACTC=SAT (4\u{2074}\u{00b2}>u64)");
    assert_eq!(nalso,            u64::MAX,             "p3: NALSO=SAT (8\u{00b3}\u{2077}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);
    add_node(T69_VEC_C, T69_KEY_C, T69_ID_C);
    add_edge(T69_ID_A, T69_ID_B, "t69.e.ab");
    add_edge(T69_ID_B, T69_ID_C, "t69.e.bc");
    add_edge(T69_ID_C, T69_ID_A, "t69.e.ca");

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(ntritetraactc,  u64::MAX, "k3: NTRITETRAACTC=SAT");
    assert_eq!(nhtritetraactc, u64::MAX, "k3: NHTRITETRAACTC=SAT");
    assert_eq!(nalso,          u64::MAX, "k3: NALSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A); // hub
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);
    add_node(T69_VEC_C, T69_KEY_C, T69_ID_C);
    add_node(T69_VEC_D, T69_KEY_D, T69_ID_D);
    add_node(T69_VEC_E, T69_KEY_E, T69_ID_E);
    add_edge(T69_ID_A, T69_ID_B, "t69.e.ab");
    add_edge(T69_ID_A, T69_ID_C, "t69.e.ac");
    add_edge(T69_ID_A, T69_ID_D, "t69.e.ad");
    add_edge(T69_ID_A, T69_ID_E, "t69.e.ae");

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(ntritetraactc,  u64::MAX, "k14: NTRITETRAACTC=SAT");
    assert_eq!(nhtritetraactc, u64::MAX, "k14: NHTRITETRAACTC=SAT");
    assert_eq!(nalso,          u64::MAX, "k14: NALSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NTRITETRAACTC: 2×2^43 + 2×3^43.  3^41>u64::MAX → SATURATES.
// NHTRITETRAACTC: 5^42+6^42+5^42 → SATURATES.
// NALSO: 13^37+18^37+13^37 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);
    add_node(T69_VEC_C, T69_KEY_C, T69_ID_C);
    add_node(T69_VEC_D, T69_KEY_D, T69_ID_D);
    add_edge(T69_ID_A, T69_ID_B, "t69.e.ab");
    add_edge(T69_ID_B, T69_ID_C, "t69.e.bc");
    add_edge(T69_ID_C, T69_ID_D, "t69.e.cd");

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(ntritetraactc,  u64::MAX, "p4: NTRITETRAACTC=SAT");
    assert_eq!(nhtritetraactc, u64::MAX, "p4: NHTRITETRAACTC=SAT");
    assert_eq!(nalso,          u64::MAX, "p4: NALSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);
    add_node(T69_VEC_C, T69_KEY_C, T69_ID_C);
    add_node(T69_VEC_D, T69_KEY_D, T69_ID_D);
    add_edge(T69_ID_A, T69_ID_B, "t69.e.ab");
    add_edge(T69_ID_A, T69_ID_C, "t69.e.ac");
    add_edge(T69_ID_A, T69_ID_D, "t69.e.ad");
    add_edge(T69_ID_B, T69_ID_C, "t69.e.bc");
    add_edge(T69_ID_B, T69_ID_D, "t69.e.bd");
    add_edge(T69_ID_C, T69_ID_D, "t69.e.cd");

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(ntritetraactc,  u64::MAX, "k4: NTRITETRAACTC=SAT");
    assert_eq!(nhtritetraactc, u64::MAX, "k4: NHTRITETRAACTC=SAT");
    assert_eq!(nalso,          u64::MAX, "k4: NALSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,               2, "isolated: node_count=2");
    assert_eq!(ec,               0, "isolated: edge_count=0");
    assert_eq!(ntritetraactc,    0, "isolated: NTRITETRAACTC=0");
    assert_eq!(nhtritetraactc,   0, "isolated: NHTRITETRAACTC=0");
    assert_eq!(nalso,            0, "isolated: NALSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NTRITETRAACTC=5×6^43 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T69_VEC_A, T69_KEY_A, T69_ID_A);
    add_node(T69_VEC_B, T69_KEY_B, T69_ID_B);
    add_node(T69_VEC_C, T69_KEY_C, T69_ID_C);
    add_node(T69_VEC_D, T69_KEY_D, T69_ID_D);
    add_node(T69_VEC_E, T69_KEY_E, T69_ID_E);
    add_edge(T69_ID_A, T69_ID_C, "t69.e.ac");
    add_edge(T69_ID_A, T69_ID_D, "t69.e.ad");
    add_edge(T69_ID_A, T69_ID_E, "t69.e.ae");
    add_edge(T69_ID_B, T69_ID_C, "t69.e.bc");
    add_edge(T69_ID_B, T69_ID_D, "t69.e.bd");
    add_edge(T69_ID_B, T69_ID_E, "t69.e.be");

    let (ntritetraactc, nhtritetraactc, nalso, ec, nc) = gos_runtime::graph_topo_indices69();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(ntritetraactc,  u64::MAX, "k23: NTRITETRAACTC=SAT (5\u{00d7}6\u{2074}\u{00b3})");
    assert_eq!(nhtritetraactc, u64::MAX, "k23: NHTRITETRAACTC=SAT");
    assert_eq!(nalso,          u64::MAX, "k23: NALSO=SAT");
}
