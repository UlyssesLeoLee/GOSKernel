// gos-graph-topo65-harness — V3.76 NNONATRIACTC + NHNONATRIACTC + NAHSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices65()`:
//   Returns (nnonatriactc, nhnonatriactc, nahso, edge_count, node_count)
//   - nnonatriactc  = NNONATRIACTC(G) = Σ_v S(v)^39                   (exact u64; S-Nonatriacontic vertex sum)
//   - nhnonatriactc = NHNONATRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^38         (exact u64; S-Octatriacontic edge-sum)
//   - nahso         = NAHSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^33       (exact u64; S-Hexahexacontyl Sombor, α=66)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NNONATRIACTC(G) = Σ_v S(v)^39
//     S-Nonatriacontic vertex sum; extends the S-power-vertex series:
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
//       NOCTATRIACTC=Σ S³⁸ (topo64), NNONATRIACTC=Σ S³⁹ (topo65).
//     NNONATRIACTC = n·S^39 for S-regular.
//     Overflow: S^39 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^39 = s32 × s4 × s2 × s  (s32=s16^2; s4=s2^2; 39=32+4+2+1).
//
//   NHNONATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^38
//     S-Octatriacontic edge-sum; extends the S-power-edge series:
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
//       NHOCTATRIACTC=Σ(S+S)³⁷ (topo64), NHNONATRIACTC=Σ(S+S)³⁸ (topo65).
//     NHNONATRIACTC = |E|·(2S)^38 = 274877906944|E|·S^38 for S-regular.
//     Overflow per edge: (2×16129)^38 → saturating u128 accumulator.
//     Implementation: ss^38 = ss32 × ss4 × ss2  (ss32=ss16^2; ss4=ss2^2; 38=32+4+2).
//
//   NAHSO(G) = Σ_{uv∈E} (S_u²+S_v²)^33
//     S-Hexahexacontyl Sombor: generalised Sombor SO^α with α=66 on S-variant.
//     3rd-pass double-letter "AH" (after NAGSO α=64, topo64).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60), NAFSO(topo63,α=62),
//     NAGSO(topo64,α=64), NAHSO(topo65,α=66).
//     NAHSO = |E|·(2S²)^33 = 8589934592|E|·S^66 for S-regular.
//     Overflow per edge: (2×16129²)^33 → saturating u128 accumulator.
//     Implementation: s2s^33 = s2s32 × s2s  (s2s32=s2s16^2; 33=32+1).
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
//  Graph     NNONATRIACTC(exact)              NHNONATRIACTC(exact)         NAHSO(exact)             edges  nodes
//  Empty                      0                               0                        0               0      0
//  1 node                     0                               0                        0               0      1
//  K₂                         2                 274_877_906_944               8_589_934_592               1      2
//  P₃         1_649_267_441_664              u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄     8_105_111_405_549_580_310           u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄             u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      4
//  2 isolated                 0                               0                        0               0      2
//  K_{2,3}        u64::MAX(sat.)              u64::MAX(sat.)              u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NNONATRIACTC:  1^39 + 1^39 = 2. ✓
//     NHNONATRIACTC: (1+1)^38 = 2^38 = 274_877_906_944. ✓
//     NAHSO:          (1²+1²)^33 = 2^33 = 8_589_934_592. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NNONATRIACTC:  3×2^39 = 3×549_755_813_888 = 1_649_267_441_664. ✓
//     NHNONATRIACTC: 2×(2+2)^38 = 2×4^38 = 2×2^76 → SATURATES (4^38=2^76>u64::MAX per-edge). ✓
//     NAHSO:          2×(4+4)^33 = 2×8^33 = 2×2^99 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NNONATRIACTC:  3×4^39 = 3×2^78 → SATURATES (2^78>u64::MAX per-node). ✓
//     NHNONATRIACTC: 3×(4+4)^38 = 3×8^38 = 3×2^114 → SATURATES. ✓
//     NAHSO:          3×(16+16)^33 = 3×32^33 = 3×2^165 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NNONATRIACTC:  5×4^39 → SATURATES. ✓
//     NHNONATRIACTC: 4×8^38 → SATURATES. ✓
//     NAHSO:          4×32^33 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NNONATRIACTC:  2×2^39 + 2×3^39.
//       3^38=1_350_851_717_672_992_089; 3^39=3^38×3=4_052_555_153_018_976_267.
//       2×3^39=8_105_110_306_037_952_534. 2×2^39=1_099_511_627_776.
//       Total=8_105_110_306_037_952_534+1_099_511_627_776=8_105_111_405_549_580_310. ✓
//     NHNONATRIACTC: (2+3)^38+(3+3)^38+(3+2)^38 = 2×5^38+6^38
//       5^38>>u64::MAX per-edge → SATURATES. ✓
//     NAHSO:          2×13^33+18^33 — 13^33>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NNONATRIACTC:  4×9^39 → SATURATES → u64::MAX. ✓
//     NHNONATRIACTC: 6×18^38 → SATURATES. ✓
//     NAHSO:          6×162^33 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NNONATRIACTC:  5×6^39 → SATURATES → u64::MAX. ✓
//     NHNONATRIACTC: 6×12^38 → SATURATES. ✓
//     NAHSO:          6×72^33 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NNONATRIACTC  = n·S^39                                                     for S-regular ✓
//   NHNONATRIACTC = |E|·(2S)^38 = 274877906944|E|·S^38                          for S-regular ✓
//   NAHSO         = |E|·(2S²)^33 = 8589934592|E|·S^66                           for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 274_877_906_944, 8_589_934_592, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_649_267_441_664, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (8_105_111_405_549_580_310, u64::MAX, u64::MAX, 3, 4)
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

const T65_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_65");
const T65_EXEC:   ExecutorId = ExecutorId::from_ascii("t65.exec");

const T65_KEY_A: &str = "t65.alpha";
const T65_KEY_B: &str = "t65.beta";
const T65_KEY_C: &str = "t65.gamma";
const T65_KEY_D: &str = "t65.delta";
const T65_KEY_E: &str = "t65.epsilon";

const T65_ID_A: NodeId = derive_node_id(T65_PLUGIN, T65_KEY_A);
const T65_ID_B: NodeId = derive_node_id(T65_PLUGIN, T65_KEY_B);
const T65_ID_C: NodeId = derive_node_id(T65_PLUGIN, T65_KEY_C);
const T65_ID_D: NodeId = derive_node_id(T65_PLUGIN, T65_KEY_D);
const T65_ID_E: NodeId = derive_node_id(T65_PLUGIN, T65_KEY_E);

// L4=152 namespace for this harness.
const T65_VEC_A: VectorAddress = VectorAddress::new(152, 1, 1, 0);
const T65_VEC_B: VectorAddress = VectorAddress::new(152, 1, 2, 0);
const T65_VEC_C: VectorAddress = VectorAddress::new(152, 1, 3, 0);
const T65_VEC_D: VectorAddress = VectorAddress::new(152, 2, 1, 0);
const T65_VEC_E: VectorAddress = VectorAddress::new(152, 2, 2, 0);

const T65_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T65_PLUGIN,
    name:         "kl-graph-topo65-harness",
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
        executor_id:       T65_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T65_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T65_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nnonatriactc,     0, "empty: NNONATRIACTC=0");
    assert_eq!(nhnonatriactc,    0, "empty: NHNONATRIACTC=0");
    assert_eq!(nahso,            0, "empty: NAHSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nnonatriactc,     0, "single: NNONATRIACTC=0");
    assert_eq!(nhnonatriactc,    0, "single: NHNONATRIACTC=0");
    assert_eq!(nahso,            0, "single: NAHSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NNONATRIACTC:  1^39+1^39 = 2.
// NHNONATRIACTC: (1+1)^38 = 2^38 = 274_877_906_944.
// NAHSO:          (1²+1²)^33 = 2^33 = 8_589_934_592.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);
    add_edge(T65_ID_A, T65_ID_B, "t65.e.ab");

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               2,               "k2: node_count=2");
    assert_eq!(ec,               1,               "k2: edge_count=1");
    assert_eq!(nnonatriactc,     2,               "k2: NNONATRIACTC=2 (1\u{00b3}\u{2079}+1\u{00b3}\u{2079}=2)");
    assert_eq!(nhnonatriactc,    274_877_906_944, "k2: NHNONATRIACTC=274_877_906_944 (2\u{00b3}\u{2078}=2^38)");
    assert_eq!(nahso,            8_589_934_592,   "k2: NAHSO=8_589_934_592 (2\u{00b3}\u{00b3}=2^33)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NNONATRIACTC:  3×2^39 = 3×549_755_813_888 = 1_649_267_441_664.
// NHNONATRIACTC: 2×(2+2)^38 = 2×4^38 = 2×2^76 → SATURATES (4^38=2^76>u64::MAX per-edge).
// NAHSO:          2×(4+4)^33 = 2×8^33 = 2×2^99 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);
    add_node(T65_VEC_C, T65_KEY_C, T65_ID_C);
    add_edge(T65_ID_A, T65_ID_B, "t65.e.ab");
    add_edge(T65_ID_B, T65_ID_C, "t65.e.bc");

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               3,                 "p3: node_count=3");
    assert_eq!(ec,               2,                 "p3: edge_count=2");
    assert_eq!(nnonatriactc,     1_649_267_441_664,  "p3: NNONATRIACTC=1_649_267_441_664 (3\u{00d7}2\u{00b3}\u{2079})");
    assert_eq!(nhnonatriactc,    u64::MAX,            "p3: NHNONATRIACTC=u64::MAX (4\u{00b3}\u{2078}=2^76>u64::MAX per-edge; saturated)");
    assert_eq!(nahso,            u64::MAX,            "p3: NAHSO=u64::MAX (8\u{00b3}\u{00b3}=2^99>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NNONATRIACTC:  3×4^39 = 3×2^78 → SATURATES (2^78>u64::MAX per-node).
// NHNONATRIACTC: 3×(4+4)^38 = 3×8^38 = 3×2^114 → SATURATES.
// NAHSO:          3×(16+16)^33 = 3×32^33 = 3×2^165 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);
    add_node(T65_VEC_C, T65_KEY_C, T65_ID_C);
    add_edge(T65_ID_A, T65_ID_B, "t65.e.ab");
    add_edge(T65_ID_B, T65_ID_A, "t65.e.ba");
    add_edge(T65_ID_B, T65_ID_C, "t65.e.bc");
    add_edge(T65_ID_C, T65_ID_B, "t65.e.cb");
    add_edge(T65_ID_A, T65_ID_C, "t65.e.ac");
    add_edge(T65_ID_C, T65_ID_A, "t65.e.ca");

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nnonatriactc,     u64::MAX, "k3: NNONATRIACTC=u64::MAX (3\u{00d7}4\u{00b3}\u{2079}>>u64::MAX; saturated)");
    assert_eq!(nhnonatriactc,    u64::MAX, "k3: NHNONATRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{2078}=3\u{00d7}2^114>>u64::MAX; saturated)");
    assert_eq!(nahso,            u64::MAX, "k3: NAHSO=u64::MAX (3\u{00d7}32\u{00b3}\u{00b3}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NNONATRIACTC:  5×4^39 → SATURATES.
// NHNONATRIACTC: 4×(4+4)^38 → SATURATES.
// NAHSO:          4×(16+16)^33 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);
    add_node(T65_VEC_C, T65_KEY_C, T65_ID_C);
    add_node(T65_VEC_D, T65_KEY_D, T65_ID_D);
    add_node(T65_VEC_E, T65_KEY_E, T65_ID_E);
    add_edge(T65_ID_A, T65_ID_B, "t65.e.ab");
    add_edge(T65_ID_A, T65_ID_C, "t65.e.ac");
    add_edge(T65_ID_A, T65_ID_D, "t65.e.ad");
    add_edge(T65_ID_A, T65_ID_E, "t65.e.ae");

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nnonatriactc,     u64::MAX, "k14: NNONATRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{2079}>u64::MAX; saturated)");
    assert_eq!(nhnonatriactc,    u64::MAX, "k14: NHNONATRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{2078}>>u64::MAX; saturated)");
    assert_eq!(nahso,            u64::MAX, "k14: NAHSO=u64::MAX (4\u{00d7}32\u{00b3}\u{00b3}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NNONATRIACTC:  2×2^39+2×3^39.
//   3^38=1_350_851_717_672_992_089; 3^39=3^38×3=4_052_555_153_018_976_267.
//   2×3^39=8_105_110_306_037_952_534. 2×2^39=1_099_511_627_776.
//   Total=8_105_110_306_037_952_534+1_099_511_627_776=8_105_111_405_549_580_310.
// NHNONATRIACTC: (2+3)^38+(3+3)^38+(3+2)^38 = 2×5^38+6^38; 5^38>>u64::MAX per-edge → SATURATES.
// NAHSO:          2×13^33+18^33 — 13^33>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);
    add_node(T65_VEC_C, T65_KEY_C, T65_ID_C);
    add_node(T65_VEC_D, T65_KEY_D, T65_ID_D);
    add_edge(T65_ID_A, T65_ID_B, "t65.e.ab");
    add_edge(T65_ID_B, T65_ID_C, "t65.e.bc");
    add_edge(T65_ID_C, T65_ID_D, "t65.e.cd");

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               4,                             "p4: node_count=4");
    assert_eq!(ec,               3,                             "p4: edge_count=3");
    assert_eq!(nnonatriactc,     8_105_111_405_549_580_310,     "p4: NNONATRIACTC=8_105_111_405_549_580_310 (2\u{00d7}2\u{00b3}\u{2079}+2\u{00d7}3\u{00b3}\u{2079}; 3\u{00b3}\u{2079}=4_052_555_153_018_976_267)");
    assert_eq!(nhnonatriactc,    u64::MAX,                      "p4: NHNONATRIACTC=u64::MAX (5\u{00b3}\u{2078}>>u64::MAX per-edge; saturated)");
    assert_eq!(nahso,            u64::MAX,                      "p4: NAHSO=u64::MAX (13\u{00b3}\u{00b3}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NNONATRIACTC:  4×9^39 → SATURATES → u64::MAX.
// NHNONATRIACTC: 6×18^38 → SATURATES → u64::MAX.
// NAHSO:          6×162^33 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);
    add_node(T65_VEC_C, T65_KEY_C, T65_ID_C);
    add_node(T65_VEC_D, T65_KEY_D, T65_ID_D);
    add_edge(T65_ID_A, T65_ID_B, "t65.e.ab");
    add_edge(T65_ID_B, T65_ID_A, "t65.e.ba");
    add_edge(T65_ID_A, T65_ID_C, "t65.e.ac");
    add_edge(T65_ID_C, T65_ID_A, "t65.e.ca");
    add_edge(T65_ID_A, T65_ID_D, "t65.e.ad");
    add_edge(T65_ID_D, T65_ID_A, "t65.e.da");
    add_edge(T65_ID_B, T65_ID_C, "t65.e.bc");
    add_edge(T65_ID_C, T65_ID_B, "t65.e.cb");
    add_edge(T65_ID_B, T65_ID_D, "t65.e.bd");
    add_edge(T65_ID_D, T65_ID_B, "t65.e.db");
    add_edge(T65_ID_C, T65_ID_D, "t65.e.cd");
    add_edge(T65_ID_D, T65_ID_C, "t65.e.dc");

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nnonatriactc,     u64::MAX, "k4: NNONATRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{2079} >> u64::MAX; saturated)");
    assert_eq!(nhnonatriactc,    u64::MAX, "k4: NHNONATRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{2078} >> u64::MAX; saturated)");
    assert_eq!(nahso,            u64::MAX, "k4: NAHSO=u64::MAX (6\u{00d7}162\u{00b3}\u{00b3} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NNONATRIACTC=0; NHNONATRIACTC=0; NAHSO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               2, "two-iso: node_count=2");
    assert_eq!(ec,               0, "two-iso: edge_count=0");
    assert_eq!(nnonatriactc,     0, "two-iso: NNONATRIACTC=0");
    assert_eq!(nhnonatriactc,    0, "two-iso: NHNONATRIACTC=0");
    assert_eq!(nahso,            0, "two-iso: NAHSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NNONATRIACTC:  5×6^39 → SATURATES (6^39 >> u64::MAX per-node).
// NHNONATRIACTC: 6×12^38 → SATURATES (12^38>>u64::MAX per-edge).
// NAHSO:          6×72^33 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T65_VEC_A, T65_KEY_A, T65_ID_A);
    add_node(T65_VEC_B, T65_KEY_B, T65_ID_B);
    add_node(T65_VEC_C, T65_KEY_C, T65_ID_C);
    add_node(T65_VEC_D, T65_KEY_D, T65_ID_D);
    add_node(T65_VEC_E, T65_KEY_E, T65_ID_E);
    add_edge(T65_ID_A, T65_ID_C, "t65.e.ac");
    add_edge(T65_ID_A, T65_ID_D, "t65.e.ad");
    add_edge(T65_ID_A, T65_ID_E, "t65.e.ae");
    add_edge(T65_ID_B, T65_ID_C, "t65.e.bc");
    add_edge(T65_ID_B, T65_ID_D, "t65.e.bd");
    add_edge(T65_ID_B, T65_ID_E, "t65.e.be");

    let (nnonatriactc, nhnonatriactc, nahso, ec, nc) = gos_runtime::graph_topo_indices65();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nnonatriactc,     u64::MAX, "k23: NNONATRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{2079}; 6\u{00b3}\u{2079}>>u64::MAX per-node; saturated)");
    assert_eq!(nhnonatriactc,    u64::MAX, "k23: NHNONATRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{2078} >> u64::MAX; per-edge saturates)");
    assert_eq!(nahso,            u64::MAX, "k23: NAHSO=u64::MAX (6\u{00d7}72\u{00b3}\u{00b3} >> u64::MAX; per-edge saturates)");
}
