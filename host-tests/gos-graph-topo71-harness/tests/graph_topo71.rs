// gos-graph-topo71-harness — V3.82 NPENTETRAACTC + NHPENTETRAACTC + NANSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices71()`:
//   Returns (npentetraactc, nhpentetraactc, nanso, edge_count, node_count)
//   - npentetraactc  = NPENTETRAACTC(G) = Σ_v S(v)^45                   (exact u64; S-Pentatetracontic vertex sum)
//   - nhpentetraactc = NHPENTETRAACTC(G)= Σ_{uv∈E} (S_u+S_v)^44         (exact u64; S-Tetratetracontic edge-sum)
//   - nanso           = NANSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^39       (exact u64; S-Pentatetracontyl Sombor, α=78)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NPENTETRAACTC(G) = Σ_v S(v)^45
//     S-Pentatetracontic vertex sum; extends the S-power-vertex series:
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
//       NHENTETRAACTC=Σ S⁴¹ (topo67), NDOTETRAACTC=Σ S⁴² (topo68), NTRITETRAACTC=Σ S⁴³ (topo69),
//       NTETRATETRAACTC=Σ S⁴⁴ (topo70), NPENTETRAACTC=Σ S⁴⁵ (topo71).
//     NPENTETRAACTC = n·S^45 for S-regular.
//     Overflow: S^45 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^45 = s32 × s8 × s4 × s  (s32=s16^2; s8=s4^2; 45=32+8+4+1; 4 mults).
//
//   NHPENTETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^44
//     S-Tetratetracontic edge-sum; extends the S-power-edge series:
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
//       NHDOTETRAACTC=Σ(S+S)⁴¹ (topo68), NHTRITETRAACTC=Σ(S+S)⁴² (topo69),
//       NHTETRATETRAACTC=Σ(S+S)⁴³ (topo70), NHPENTETRAACTC=Σ(S+S)⁴⁴ (topo71).
//     NHPENTETRAACTC = |E|·(2S)^44 = 17592186044416|E|·S^44 for S-regular.
//     Overflow per edge: (2×16129)^44 → saturating u128 accumulator.
//     Implementation: ss^44 = ss32 × ss8 × ss4  (ss32=ss16^2; ss8=ss4^2; 44=32+8+4; 3 mults — efficient!).
//
//   NANSO(G) = Σ_{uv∈E} (S_u²+S_v²)^39
//     S-Pentatetracontyl Sombor: generalised Sombor SO^α with α=78 on S-variant.
//     3rd-pass double-letter "AN" (after NAMSO α=76, topo70).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60), NAFSO(topo63,α=62),
//     NAGSO(topo64,α=64), NAHSO(topo65,α=66), NAISO(topo66,α=68), NAJSO(topo67,α=70),
//     NAKSO(topo68,α=72), NALSO(topo69,α=74), NAMSO(topo70,α=76), NANSO(topo71,α=78).
//     NANSO = |E|·(2S²)^39 = 549755813888|E|·S^78 for S-regular.
//     Overflow per edge: (2×16129²)^39 → saturating u128 accumulator.
//     Implementation: s2s^39 = s2s32 × s2s4 × s2s2 × s2s  (s2s32=s2s16^2; s2s4=s2s2^2; 39=32+4+2+1; 4 mults).
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
//  Graph     NPENTETRAACTC(exact)             NHPENTETRAACTC(exact)        NANSO(exact)             edges  nodes
//  Empty                        0                               0                        0               0      0
//  1 node                       0                               0                        0               0      1
//  K₂                           2              17_592_186_044_416             549_755_813_888               1      2
//  P₃          105_553_116_266_496              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃               u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}          u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄               u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄               u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                   0                               0                        0               0      2
//  K_{2,3}          u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NPENTETRAACTC:  1^45 + 1^45 = 2. ✓
//     NHPENTETRAACTC: (1+1)^44 = 2^44 = 17_592_186_044_416. ✓
//     NANSO:           (1²+1²)^39 = 2^39 = 549_755_813_888. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NPENTETRAACTC:  3×2^45 = 3×35_184_372_088_832 = 105_553_116_266_496. ✓
//     NHPENTETRAACTC: 2×(2+2)^44 = 2×4^44 = 2×2^88 → SATURATES. ✓
//     NANSO:           2×(4+4)^39 = 2×8^39 = 2×2^117 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NPENTETRAACTC:  3×4^45 = 3×2^90 → SATURATES. ✓
//     NHPENTETRAACTC: 3×8^44 → SATURATES. ✓
//     NANSO:           3×32^39 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NPENTETRAACTC:  5×4^45 → SATURATES. ✓
//     NHPENTETRAACTC: 4×8^44 → SATURATES. ✓
//     NANSO:           4×32^39 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NPENTETRAACTC:  2×2^45 + 2×3^45.
//       3^44>u64::MAX → SATURATES. ✓
//     NHPENTETRAACTC: 2×5^44 + 6^44 → each term >> u64::MAX → SATURATES. ✓
//     NANSO:           2×13^39 + 18^39 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NPENTETRAACTC:  4×9^45 → SATURATES. ✓
//     NHPENTETRAACTC: 6×18^44 → SATURATES. ✓
//     NANSO:           6×162^39 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NPENTETRAACTC:  5×6^45 → SATURATES. ✓
//     NHPENTETRAACTC: 6×12^44 → SATURATES. ✓
//     NANSO:           6×72^39 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NPENTETRAACTC  = n·S^45                                                          for S-regular ✓
//   NHPENTETRAACTC = |E|·(2S)^44 = 17592186044416|E|·S^44                            for S-regular ✓
//   NANSO          = |E|·(2S²)^39 = 549755813888|E|·S^78                             for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 17_592_186_044_416, 549_755_813_888, 1, 2)
//  4.  Path P₃ = A-B-C                   → (105_553_116_266_496, u64::MAX, u64::MAX, 2, 3)
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

const T71_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_71");
const T71_EXEC:   ExecutorId = ExecutorId::from_ascii("t71.exec");

const T71_KEY_A: &str = "t71.alpha";
const T71_KEY_B: &str = "t71.beta";
const T71_KEY_C: &str = "t71.gamma";
const T71_KEY_D: &str = "t71.delta";
const T71_KEY_E: &str = "t71.epsilon";

const T71_ID_A: NodeId = derive_node_id(T71_PLUGIN, T71_KEY_A);
const T71_ID_B: NodeId = derive_node_id(T71_PLUGIN, T71_KEY_B);
const T71_ID_C: NodeId = derive_node_id(T71_PLUGIN, T71_KEY_C);
const T71_ID_D: NodeId = derive_node_id(T71_PLUGIN, T71_KEY_D);
const T71_ID_E: NodeId = derive_node_id(T71_PLUGIN, T71_KEY_E);

// L4=158 namespace for this harness.
const T71_VEC_A: VectorAddress = VectorAddress::new(158, 1, 1, 0);
const T71_VEC_B: VectorAddress = VectorAddress::new(158, 1, 2, 0);
const T71_VEC_C: VectorAddress = VectorAddress::new(158, 1, 3, 0);
const T71_VEC_D: VectorAddress = VectorAddress::new(158, 2, 1, 0);
const T71_VEC_E: VectorAddress = VectorAddress::new(158, 2, 2, 0);

const T71_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T71_PLUGIN,
    name:         "kl-graph-topo71-harness",
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
        executor_id:       T71_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T71_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T71_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(npentetraactc,    0, "empty: NPENTETRAACTC=0");
    assert_eq!(nhpentetraactc,   0, "empty: NHPENTETRAACTC=0");
    assert_eq!(nanso,            0, "empty: NANSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(npentetraactc,    0, "single: NPENTETRAACTC=0");
    assert_eq!(nhpentetraactc,   0, "single: NHPENTETRAACTC=0");
    assert_eq!(nanso,            0, "single: NANSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NPENTETRAACTC:  1^45+1^45 = 2.
// NHPENTETRAACTC: (1+1)^44 = 2^44 = 17_592_186_044_416.
// NANSO:           (1²+1²)^39 = 2^39 = 549_755_813_888.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);
    add_edge(T71_ID_A, T71_ID_B, "t71.e.ab");

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,               2,                    "k2: node_count=2");
    assert_eq!(ec,               1,                    "k2: edge_count=1");
    assert_eq!(npentetraactc,    2,                    "k2: NPENTETRAACTC=2 (1\u{2074}\u{2075}+1\u{2074}\u{2075}=2)");
    assert_eq!(nhpentetraactc,   17_592_186_044_416,   "k2: NHPENTETRAACTC=17_592_186_044_416 (2\u{2074}\u{2074}=2^44)");
    assert_eq!(nanso,            549_755_813_888,      "k2: NANSO=549_755_813_888 (2\u{00b3}\u{2079}=2^39)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NPENTETRAACTC:  3×2^45 = 3×35_184_372_088_832 = 105_553_116_266_496.
// NHPENTETRAACTC: 2×(2+2)^44 = 2×4^44 = 2×2^88 → SATURATES.
// NANSO:           2×(4+4)^39 = 2×8^39 = 2×2^117 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);
    add_node(T71_VEC_C, T71_KEY_C, T71_ID_C);
    add_edge(T71_ID_A, T71_ID_B, "t71.e.ab");
    add_edge(T71_ID_B, T71_ID_C, "t71.e.bc");

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,               3,                      "p3: node_count=3");
    assert_eq!(ec,               2,                      "p3: edge_count=2");
    assert_eq!(npentetraactc,    105_553_116_266_496,    "p3: NPENTETRAACTC=105_553_116_266_496 (3\u{00d7}2\u{2074}\u{2075})");
    assert_eq!(nhpentetraactc,   u64::MAX,               "p3: NHPENTETRAACTC=SAT (4\u{2074}\u{2074}>u64)");
    assert_eq!(nanso,            u64::MAX,               "p3: NANSO=SAT (8\u{00b3}\u{2079}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);
    add_node(T71_VEC_C, T71_KEY_C, T71_ID_C);
    add_edge(T71_ID_A, T71_ID_B, "t71.e.ab");
    add_edge(T71_ID_B, T71_ID_C, "t71.e.bc");
    add_edge(T71_ID_C, T71_ID_A, "t71.e.ca");

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(npentetraactc,   u64::MAX, "k3: NPENTETRAACTC=SAT");
    assert_eq!(nhpentetraactc,  u64::MAX, "k3: NHPENTETRAACTC=SAT");
    assert_eq!(nanso,           u64::MAX, "k3: NANSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A); // hub
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);
    add_node(T71_VEC_C, T71_KEY_C, T71_ID_C);
    add_node(T71_VEC_D, T71_KEY_D, T71_ID_D);
    add_node(T71_VEC_E, T71_KEY_E, T71_ID_E);
    add_edge(T71_ID_A, T71_ID_B, "t71.e.ab");
    add_edge(T71_ID_A, T71_ID_C, "t71.e.ac");
    add_edge(T71_ID_A, T71_ID_D, "t71.e.ad");
    add_edge(T71_ID_A, T71_ID_E, "t71.e.ae");

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(npentetraactc,   u64::MAX, "k14: NPENTETRAACTC=SAT");
    assert_eq!(nhpentetraactc,  u64::MAX, "k14: NHPENTETRAACTC=SAT");
    assert_eq!(nanso,           u64::MAX, "k14: NANSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NPENTETRAACTC: 2×2^45 + 2×3^45.  3^44>u64::MAX → SATURATES.
// NHPENTETRAACTC: 5^44+6^44+5^44 → SATURATES.
// NANSO: 13^39+18^39+13^39 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);
    add_node(T71_VEC_C, T71_KEY_C, T71_ID_C);
    add_node(T71_VEC_D, T71_KEY_D, T71_ID_D);
    add_edge(T71_ID_A, T71_ID_B, "t71.e.ab");
    add_edge(T71_ID_B, T71_ID_C, "t71.e.bc");
    add_edge(T71_ID_C, T71_ID_D, "t71.e.cd");

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(npentetraactc,   u64::MAX, "p4: NPENTETRAACTC=SAT");
    assert_eq!(nhpentetraactc,  u64::MAX, "p4: NHPENTETRAACTC=SAT");
    assert_eq!(nanso,           u64::MAX, "p4: NANSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);
    add_node(T71_VEC_C, T71_KEY_C, T71_ID_C);
    add_node(T71_VEC_D, T71_KEY_D, T71_ID_D);
    add_edge(T71_ID_A, T71_ID_B, "t71.e.ab");
    add_edge(T71_ID_A, T71_ID_C, "t71.e.ac");
    add_edge(T71_ID_A, T71_ID_D, "t71.e.ad");
    add_edge(T71_ID_B, T71_ID_C, "t71.e.bc");
    add_edge(T71_ID_B, T71_ID_D, "t71.e.bd");
    add_edge(T71_ID_C, T71_ID_D, "t71.e.cd");

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(npentetraactc,   u64::MAX, "k4: NPENTETRAACTC=SAT");
    assert_eq!(nhpentetraactc,  u64::MAX, "k4: NHPENTETRAACTC=SAT");
    assert_eq!(nanso,           u64::MAX, "k4: NANSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,               2, "isolated: node_count=2");
    assert_eq!(ec,               0, "isolated: edge_count=0");
    assert_eq!(npentetraactc,    0, "isolated: NPENTETRAACTC=0");
    assert_eq!(nhpentetraactc,   0, "isolated: NHPENTETRAACTC=0");
    assert_eq!(nanso,            0, "isolated: NANSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NPENTETRAACTC=5×6^45 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T71_VEC_A, T71_KEY_A, T71_ID_A);
    add_node(T71_VEC_B, T71_KEY_B, T71_ID_B);
    add_node(T71_VEC_C, T71_KEY_C, T71_ID_C);
    add_node(T71_VEC_D, T71_KEY_D, T71_ID_D);
    add_node(T71_VEC_E, T71_KEY_E, T71_ID_E);
    add_edge(T71_ID_A, T71_ID_C, "t71.e.ac");
    add_edge(T71_ID_A, T71_ID_D, "t71.e.ad");
    add_edge(T71_ID_A, T71_ID_E, "t71.e.ae");
    add_edge(T71_ID_B, T71_ID_C, "t71.e.bc");
    add_edge(T71_ID_B, T71_ID_D, "t71.e.bd");
    add_edge(T71_ID_B, T71_ID_E, "t71.e.be");

    let (npentetraactc, nhpentetraactc, nanso, ec, nc) = gos_runtime::graph_topo_indices71();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(npentetraactc,   u64::MAX, "k23: NPENTETRAACTC=SAT (5\u{00d7}6\u{2074}\u{2075})");
    assert_eq!(nhpentetraactc,  u64::MAX, "k23: NHPENTETRAACTC=SAT");
    assert_eq!(nanso,           u64::MAX, "k23: NANSO=SAT");
}
