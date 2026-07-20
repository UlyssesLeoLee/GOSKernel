// gos-graph-topo67-harness — V3.78 NHENTETRAACTC + NHHENTETRAACTC + NAJSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices67()`:
//   Returns (nhentetraactc, nhhentetraactc, najso, edge_count, node_count)
//   - nhentetraactc  = NHENTETRAACTC(G) = Σ_v S(v)^41                   (exact u64; S-Hentetracontic vertex sum)
//   - nhhentetraactc = NHHENTETRAACTC(G)= Σ_{uv∈E} (S_u+S_v)^40         (exact u64; S-Tetracontic edge-sum)
//   - najso           = NAJSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^35       (exact u64; S-Tetracontyl Sombor, α=70)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHENTETRAACTC(G) = Σ_v S(v)^41
//     S-Hentetracontic vertex sum; extends the S-power-vertex series:
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
//       NHENTETRAACTC=Σ S⁴¹ (topo67).
//     NHENTETRAACTC = n·S^41 for S-regular.
//     Overflow: S^41 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^41 = s32 × s8 × s  (s32=s16^2; s8=s4^2; 41=32+8+1).
//
//   NHHENTETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^40
//     S-Tetracontic edge-sum; extends the S-power-edge series:
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
//       NHTETRAACTC=Σ(S+S)³⁹ (topo66), NHHENTETRAACTC=Σ(S+S)⁴⁰ (topo67).
//     NHHENTETRAACTC = |E|·(2S)^40 = 1099511627776|E|·S^40 for S-regular.
//     Overflow per edge: (2×16129)^40 → saturating u128 accumulator.
//     Implementation: ss^40 = ss32 × ss8  (ss32=ss16^2; ss8=ss4^2; 40=32+8, two powers of 2).
//
//   NAJSO(G) = Σ_{uv∈E} (S_u²+S_v²)^35
//     S-Tetracontyl Sombor: generalised Sombor SO^α with α=70 on S-variant.
//     3rd-pass double-letter "AJ" (after NAISO α=68, topo66).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56), NADSO(topo61,α=58), NAESO(topo62,α=60), NAFSO(topo63,α=62),
//     NAGSO(topo64,α=64), NAHSO(topo65,α=66), NAISO(topo66,α=68), NAJSO(topo67,α=70).
//     NAJSO = |E|·(2S²)^35 = 34359738368|E|·S^70 for S-regular.
//     Overflow per edge: (2×16129²)^35 → saturating u128 accumulator.
//     Implementation: s2s^35 = s2s32 × s2s2 × s2s  (s2s32=s2s16^2; 35=32+2+1).
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
//  Graph     NHENTETRAACTC(exact)             NHHENTETRAACTC(exact)        NAJSO(exact)             edges  nodes
//  Empty                      0                               0                        0               0      0
//  1 node                     0                               0                        0               0      1
//  K₂                         2               1_099_511_627_776             34_359_738_368               1      2
//  P₃         6_597_069_766_656              u64::MAX(sat.)              u64::MAX(sat.)              2      3
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
//     NHENTETRAACTC:  1^41 + 1^41 = 2. ✓
//     NHHENTETRAACTC: (1+1)^40 = 2^40 = 1_099_511_627_776. ✓
//     NAJSO:           (1²+1²)^35 = 2^35 = 34_359_738_368. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHENTETRAACTC:  3×2^41 = 3×2_199_023_255_552 = 6_597_069_766_656. ✓
//     NHHENTETRAACTC: 2×(2+2)^40 = 2×4^40 = 2×2^80 → SATURATES (4^40=2^80>u64::MAX per-edge). ✓
//     NAJSO:           2×(4+4)^35 = 2×8^35 = 2×2^105 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHENTETRAACTC:  3×4^41 = 3×2^82 → SATURATES (2^82>u64::MAX per-node). ✓
//     NHHENTETRAACTC: 3×(4+4)^40 = 3×8^40 = 3×2^120 → SATURATES. ✓
//     NAJSO:           3×(16+16)^35 = 3×32^35 = 3×2^175 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHENTETRAACTC:  5×4^41 → SATURATES. ✓
//     NHHENTETRAACTC: 4×8^40 → SATURATES. ✓
//     NAJSO:           4×32^35 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHENTETRAACTC:  2×2^41 + 2×3^41.
//       3^41=36_472_996_377_170_786_403. 2×3^41 > u64::MAX → SATURATES. ✓
//     NHHENTETRAACTC: (2+3)^40+(3+3)^40+(3+2)^40 = 2×5^40+6^40; 5^40>>u64::MAX per-edge → SATURATES. ✓
//     NAJSO:           2×13^35+18^35; 13^35>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHENTETRAACTC:  4×9^41 → SATURATES → u64::MAX. ✓
//     NHHENTETRAACTC: 6×18^40 → SATURATES. ✓
//     NAJSO:           6×162^35 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHENTETRAACTC:  5×6^41 → SATURATES → u64::MAX. ✓
//     NHHENTETRAACTC: 6×12^40 → SATURATES. ✓
//     NAJSO:           6×72^35 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHENTETRAACTC  = n·S^41                                                      for S-regular ✓
//   NHHENTETRAACTC = |E|·(2S)^40 = 1099511627776|E|·S^40                         for S-regular ✓
//   NAJSO          = |E|·(2S²)^35 = 34359738368|E|·S^70                           for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 1_099_511_627_776, 34_359_738_368, 1, 2)
//  4.  Path P₃ = A-B-C                   → (6_597_069_766_656, u64::MAX, u64::MAX, 2, 3)
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

const T67_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_67");
const T67_EXEC:   ExecutorId = ExecutorId::from_ascii("t67.exec");

const T67_KEY_A: &str = "t67.alpha";
const T67_KEY_B: &str = "t67.beta";
const T67_KEY_C: &str = "t67.gamma";
const T67_KEY_D: &str = "t67.delta";
const T67_KEY_E: &str = "t67.epsilon";

const T67_ID_A: NodeId = derive_node_id(T67_PLUGIN, T67_KEY_A);
const T67_ID_B: NodeId = derive_node_id(T67_PLUGIN, T67_KEY_B);
const T67_ID_C: NodeId = derive_node_id(T67_PLUGIN, T67_KEY_C);
const T67_ID_D: NodeId = derive_node_id(T67_PLUGIN, T67_KEY_D);
const T67_ID_E: NodeId = derive_node_id(T67_PLUGIN, T67_KEY_E);

// L4=154 namespace for this harness.
const T67_VEC_A: VectorAddress = VectorAddress::new(154, 1, 1, 0);
const T67_VEC_B: VectorAddress = VectorAddress::new(154, 1, 2, 0);
const T67_VEC_C: VectorAddress = VectorAddress::new(154, 1, 3, 0);
const T67_VEC_D: VectorAddress = VectorAddress::new(154, 2, 1, 0);
const T67_VEC_E: VectorAddress = VectorAddress::new(154, 2, 2, 0);

const T67_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T67_PLUGIN,
    name:         "kl-graph-topo67-harness",
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
        executor_id:       T67_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T67_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T67_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nhentetraactc,   0, "empty: NHENTETRAACTC=0");
    assert_eq!(nhhentetraactc,  0, "empty: NHHENTETRAACTC=0");
    assert_eq!(najso,           0, "empty: NAJSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nhentetraactc,   0, "single: NHENTETRAACTC=0");
    assert_eq!(nhhentetraactc,  0, "single: NHHENTETRAACTC=0");
    assert_eq!(najso,           0, "single: NAJSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHENTETRAACTC:  1^41+1^41 = 2.
// NHHENTETRAACTC: (1+1)^40 = 2^40 = 1_099_511_627_776.
// NAJSO:           (1²+1²)^35 = 2^35 = 34_359_738_368.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);
    add_edge(T67_ID_A, T67_ID_B, "t67.e.ab");

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               2,                    "k2: node_count=2");
    assert_eq!(ec,               1,                    "k2: edge_count=1");
    assert_eq!(nhentetraactc,   2,                    "k2: NHENTETRAACTC=2 (1^41+1^41=2)");
    assert_eq!(nhhentetraactc,  1_099_511_627_776,    "k2: NHHENTETRAACTC=1_099_511_627_776 (2^40)");
    assert_eq!(najso,           34_359_738_368,       "k2: NAJSO=34_359_738_368 (2^35)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHENTETRAACTC:  3×2^41 = 3×2_199_023_255_552 = 6_597_069_766_656.
// NHHENTETRAACTC: 2×(2+2)^40 = 2×4^40 = 2×2^80 → SATURATES (4^40=2^80>u64::MAX per-edge).
// NAJSO:           2×(4+4)^35 = 2×8^35 = 2×2^105 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);
    add_node(T67_VEC_C, T67_KEY_C, T67_ID_C);
    add_edge(T67_ID_A, T67_ID_B, "t67.e.ab");
    add_edge(T67_ID_B, T67_ID_C, "t67.e.bc");

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               3,                      "p3: node_count=3");
    assert_eq!(ec,               2,                      "p3: edge_count=2");
    assert_eq!(nhentetraactc,   6_597_069_766_656,       "p3: NHENTETRAACTC=6_597_069_766_656 (3\u{00d7}2^41)");
    assert_eq!(nhhentetraactc,  u64::MAX,                "p3: NHHENTETRAACTC=u64::MAX (4^40=2^80>u64::MAX per-edge; saturated)");
    assert_eq!(najso,           u64::MAX,                "p3: NAJSO=u64::MAX (8^35=2^105>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHENTETRAACTC:  3×4^41 = 3×2^82 → SATURATES (2^82>u64::MAX per-node).
// NHHENTETRAACTC: 3×(4+4)^40 = 3×8^40 = 3×2^120 → SATURATES.
// NAJSO:           3×(16+16)^35 = 3×32^35 = 3×2^175 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);
    add_node(T67_VEC_C, T67_KEY_C, T67_ID_C);
    add_edge(T67_ID_A, T67_ID_B, "t67.e.ab");
    add_edge(T67_ID_B, T67_ID_A, "t67.e.ba");
    add_edge(T67_ID_B, T67_ID_C, "t67.e.bc");
    add_edge(T67_ID_C, T67_ID_B, "t67.e.cb");
    add_edge(T67_ID_A, T67_ID_C, "t67.e.ac");
    add_edge(T67_ID_C, T67_ID_A, "t67.e.ca");

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nhentetraactc,   u64::MAX, "k3: NHENTETRAACTC=u64::MAX (3\u{00d7}4^41=3\u{00d7}2^82>>u64::MAX; saturated)");
    assert_eq!(nhhentetraactc,  u64::MAX, "k3: NHHENTETRAACTC=u64::MAX (3\u{00d7}8^40=3\u{00d7}2^120>>u64::MAX; saturated)");
    assert_eq!(najso,           u64::MAX, "k3: NAJSO=u64::MAX (3\u{00d7}32^35>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NHENTETRAACTC:  5×4^41 → SATURATES.
// NHHENTETRAACTC: 4×(4+4)^40 → SATURATES.
// NAJSO:           4×(16+16)^35 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);
    add_node(T67_VEC_C, T67_KEY_C, T67_ID_C);
    add_node(T67_VEC_D, T67_KEY_D, T67_ID_D);
    add_node(T67_VEC_E, T67_KEY_E, T67_ID_E);
    add_edge(T67_ID_A, T67_ID_B, "t67.e.ab");
    add_edge(T67_ID_A, T67_ID_C, "t67.e.ac");
    add_edge(T67_ID_A, T67_ID_D, "t67.e.ad");
    add_edge(T67_ID_A, T67_ID_E, "t67.e.ae");

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nhentetraactc,   u64::MAX, "k14: NHENTETRAACTC=u64::MAX (5\u{00d7}4^41>u64::MAX; saturated)");
    assert_eq!(nhhentetraactc,  u64::MAX, "k14: NHHENTETRAACTC=u64::MAX (4\u{00d7}8^40>>u64::MAX; saturated)");
    assert_eq!(najso,           u64::MAX, "k14: NAJSO=u64::MAX (4\u{00d7}32^35>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NHENTETRAACTC:  2×2^41 + 2×3^41.
//   3^41=36_472_996_377_170_786_403. 2×3^41 > u64::MAX → SATURATES.
// NHHENTETRAACTC: 2×5^40+6^40; 5^40>>u64::MAX per-edge → SATURATES.
// NAJSO:           2×13^35+18^35; 13^35>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);
    add_node(T67_VEC_C, T67_KEY_C, T67_ID_C);
    add_node(T67_VEC_D, T67_KEY_D, T67_ID_D);
    add_edge(T67_ID_A, T67_ID_B, "t67.e.ab");
    add_edge(T67_ID_B, T67_ID_C, "t67.e.bc");
    add_edge(T67_ID_C, T67_ID_D, "t67.e.cd");

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nhentetraactc,   u64::MAX, "p4: NHENTETRAACTC=u64::MAX (2\u{00d7}3^41>>u64::MAX; saturated)");
    assert_eq!(nhhentetraactc,  u64::MAX, "p4: NHHENTETRAACTC=u64::MAX (5^40>>u64::MAX per-edge; saturated)");
    assert_eq!(najso,           u64::MAX, "p4: NAJSO=u64::MAX (13^35>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHENTETRAACTC:  4×9^41 → SATURATES → u64::MAX.
// NHHENTETRAACTC: 6×18^40 → SATURATES → u64::MAX.
// NAJSO:           6×162^35 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);
    add_node(T67_VEC_C, T67_KEY_C, T67_ID_C);
    add_node(T67_VEC_D, T67_KEY_D, T67_ID_D);
    add_edge(T67_ID_A, T67_ID_B, "t67.e.ab");
    add_edge(T67_ID_B, T67_ID_A, "t67.e.ba");
    add_edge(T67_ID_A, T67_ID_C, "t67.e.ac");
    add_edge(T67_ID_C, T67_ID_A, "t67.e.ca");
    add_edge(T67_ID_A, T67_ID_D, "t67.e.ad");
    add_edge(T67_ID_D, T67_ID_A, "t67.e.da");
    add_edge(T67_ID_B, T67_ID_C, "t67.e.bc");
    add_edge(T67_ID_C, T67_ID_B, "t67.e.cb");
    add_edge(T67_ID_B, T67_ID_D, "t67.e.bd");
    add_edge(T67_ID_D, T67_ID_B, "t67.e.db");
    add_edge(T67_ID_C, T67_ID_D, "t67.e.cd");
    add_edge(T67_ID_D, T67_ID_C, "t67.e.dc");

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nhentetraactc,   u64::MAX, "k4: NHENTETRAACTC=u64::MAX (4\u{00d7}9^41 >> u64::MAX; saturated)");
    assert_eq!(nhhentetraactc,  u64::MAX, "k4: NHHENTETRAACTC=u64::MAX (6\u{00d7}18^40 >> u64::MAX; saturated)");
    assert_eq!(najso,           u64::MAX, "k4: NAJSO=u64::MAX (6\u{00d7}162^35 >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHENTETRAACTC=0; NHHENTETRAACTC=0; NAJSO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               2, "two-iso: node_count=2");
    assert_eq!(ec,               0, "two-iso: edge_count=0");
    assert_eq!(nhentetraactc,   0, "two-iso: NHENTETRAACTC=0");
    assert_eq!(nhhentetraactc,  0, "two-iso: NHHENTETRAACTC=0");
    assert_eq!(najso,           0, "two-iso: NAJSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NHENTETRAACTC:  5×6^41 → SATURATES (6^41 >> u64::MAX per-node).
// NHHENTETRAACTC: 6×12^40 → SATURATES (12^40>>u64::MAX per-edge).
// NAJSO:           6×72^35 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T67_VEC_A, T67_KEY_A, T67_ID_A);
    add_node(T67_VEC_B, T67_KEY_B, T67_ID_B);
    add_node(T67_VEC_C, T67_KEY_C, T67_ID_C);
    add_node(T67_VEC_D, T67_KEY_D, T67_ID_D);
    add_node(T67_VEC_E, T67_KEY_E, T67_ID_E);
    add_edge(T67_ID_A, T67_ID_C, "t67.e.ac");
    add_edge(T67_ID_A, T67_ID_D, "t67.e.ad");
    add_edge(T67_ID_A, T67_ID_E, "t67.e.ae");
    add_edge(T67_ID_B, T67_ID_C, "t67.e.bc");
    add_edge(T67_ID_B, T67_ID_D, "t67.e.bd");
    add_edge(T67_ID_B, T67_ID_E, "t67.e.be");

    let (nhentetraactc, nhhentetraactc, najso, ec, nc) = gos_runtime::graph_topo_indices67();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nhentetraactc,   u64::MAX, "k23: NHENTETRAACTC=u64::MAX (5\u{00d7}6^41; per-node saturates)");
    assert_eq!(nhhentetraactc,  u64::MAX, "k23: NHHENTETRAACTC=u64::MAX (6\u{00d7}12^40 >> u64::MAX; per-edge saturates)");
    assert_eq!(najso,           u64::MAX, "k23: NAJSO=u64::MAX (6\u{00d7}72^35 >> u64::MAX; per-edge saturates)");
}
