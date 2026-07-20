// gos-graph-topo59-harness — V3.70 NTRITRIACTC + NHTRITRIACTC + NABSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices59()`:
//   Returns (ntritriactc, nhtritriactc, nabso, edge_count, node_count)
//   - ntritriactc  = NTRITRIACTC(G) = Σ_v S(v)^33                   (exact u64; S-Tritriacontic vertex sum)
//   - nhtritriactc = NHTRITRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^32         (exact u64; S-Dotriacontic edge-sum)
//   - nabso        = NABSO(G)       = Σ_{uv∈E} (S_u²+S_v²)^27       (exact u64; S-Dopentatecontyl Sombor, α=54)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTRITRIACTC(G) = Σ_v S(v)^33
//     S-Tritriacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56), NHENTRIACTC=Σ S³¹ (topo57),
//       NDOTRIACTC=Σ S³² (topo58), NTRITRIACTC=Σ S³³ (topo59).
//     NTRITRIACTC = n·S^33 for S-regular.
//     Overflow: S^33 ≤ 16129^33 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^33 = s16 × s16 × s  (s^32 as perfect square, multiply by s).
//
//   NHTRITRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^32
//     S-Dotriacontic edge-sum; extends the S-power-edge series:
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
//       NHDOTRIACTC=Σ(S+S)³¹ (topo58), NHTRITRIACTC=Σ(S+S)³² (topo59).
//     NHTRITRIACTC = |E|·(2S)^32 = 4294967296|E|·S^32 for S-regular.
//     Overflow per edge: (2×16129)^32 → saturating u128 accumulator.
//     Implementation: ss^32 = ss16 × ss16  (perfect square, simplest form).
//
//   NABSO(G) = Σ_{uv∈E} (S_u²+S_v²)^27
//     S-Dopentatecontyl Sombor: generalised Sombor SO^α with α=54 on S-variant.
//     3rd-pass double-letter "AB" (after NAASO α=52, topo58).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54).
//     NABSO = |E|·(2S²)^27 = 134217728|E|·S^54 for S-regular.
//     Overflow per edge: (2×16129²)^27 → saturating u128 accumulator.
//     Implementation: s2s^27 = s2s16 × s2s8 × s2s2 × s2s.
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
//  Graph     NTRITRIACTC(exact)             NHTRITRIACTC(exact)           NABSO(exact)             edges  nodes
//  Empty                   0                             0                        0               0      0
//  1 node                  0                             0                        0               0      1
//  K₂                      2                   4_294_967_296               134_217_728               1      2
//  P₃          25_769_803_776            u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃          u64::MAX(sat.)            u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}     u64::MAX(sat.)            u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       11_118_138_312_980_230        u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated              0                             0                        0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTRITRIACTC:  1^33 + 1^33 = 2. ✓
//     NHTRITRIACTC: (1+1)^32 = 2^32 = 4_294_967_296. ✓
//     NABSO:        (1²+1²)^27 = 2^27 = 134_217_728. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTRITRIACTC:  3×2^33 = 3×8_589_934_592 = 25_769_803_776. ✓
//     NHTRITRIACTC: 2×(2+2)^32 = 2×4^32 = 2×2^64 → SATURATES (4^32=2^64>u64::MAX per-edge). ✓
//     NABSO:        2×(4+4)^27 = 2×8^27 = 2×2^81 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTRITRIACTC:  3×4^33 = 3×4^32×4 = 3×2^64×4 → SATURATES (4^32=2^64>u64::MAX per-node). ✓
//     NHTRITRIACTC: 3×(4+4)^32 = 3×8^32 = 3×2^96 → SATURATES. ✓
//     NABSO:        3×(16+16)^27 = 3×32^27 = 3×2^135 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTRITRIACTC:  5×4^33 → SATURATES. ✓
//     NHTRITRIACTC: 4×8^32 → SATURATES. ✓
//     NABSO:        4×32^27 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTRITRIACTC:  2×2^33 + 2×3^33 = 2^34 + 2×3^33.
//       3^32 = (3^16)^2 = 43_046_721^2 = 1_853_020_188_851_841.
//       3^33 = 3×3^32 = 5_559_060_566_555_523.
//       2×3^33 = 11_118_121_133_111_046.
//       2^34 = 17_179_869_184.
//       Total = 11_118_121_133_111_046 + 17_179_869_184 = 11_118_138_312_980_230. ✓
//     NHTRITRIACTC: (2+3)^32+(3+3)^32+(3+2)^32 = 2×5^32+6^32
//       5^32 = (5^16)^2 = 152_587_890_625^2 >> u64::MAX per-edge → SATURATES. ✓
//     NABSO:       (4+9)^27+(9+9)^27+(9+4)^27 → 13^27 >> u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTRITRIACTC:  4×9^33 → SATURATES → u64::MAX. ✓
//     NHTRITRIACTC: 6×18^32 → SATURATES. ✓
//     NABSO:        → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTRITRIACTC:  5×6^33 → SATURATES → u64::MAX. ✓
//     NHTRITRIACTC: 6×12^32 → SATURATES. ✓
//     NABSO:        6×72^27 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTRITRIACTC   = n·S^33                                                  for S-regular ✓
//   NHTRITRIACTC  = |E|·(2S)^32 = 4294967296|E|·S^32                        for S-regular ✓
//   NABSO         = |E|·(2S²)^27 = 134217728|E|·S^54                         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 4_294_967_296, 134_217_728, 1, 2)
//  4.  Path P₃ = A-B-C                   → (25_769_803_776, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (11_118_138_312_980_230, u64::MAX, u64::MAX, 3, 4)
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

const T59_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_59");
const T59_EXEC:   ExecutorId = ExecutorId::from_ascii("t59.exec");

const T59_KEY_A: &str = "t59.alpha";
const T59_KEY_B: &str = "t59.beta";
const T59_KEY_C: &str = "t59.gamma";
const T59_KEY_D: &str = "t59.delta";
const T59_KEY_E: &str = "t59.epsilon";

const T59_ID_A: NodeId = derive_node_id(T59_PLUGIN, T59_KEY_A);
const T59_ID_B: NodeId = derive_node_id(T59_PLUGIN, T59_KEY_B);
const T59_ID_C: NodeId = derive_node_id(T59_PLUGIN, T59_KEY_C);
const T59_ID_D: NodeId = derive_node_id(T59_PLUGIN, T59_KEY_D);
const T59_ID_E: NodeId = derive_node_id(T59_PLUGIN, T59_KEY_E);

// L4=146 namespace for this harness.
const T59_VEC_A: VectorAddress = VectorAddress::new(146, 1, 1, 0);
const T59_VEC_B: VectorAddress = VectorAddress::new(146, 1, 2, 0);
const T59_VEC_C: VectorAddress = VectorAddress::new(146, 1, 3, 0);
const T59_VEC_D: VectorAddress = VectorAddress::new(146, 2, 1, 0);
const T59_VEC_E: VectorAddress = VectorAddress::new(146, 2, 2, 0);

const T59_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T59_PLUGIN,
    name:         "kl-graph-topo59-harness",
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
        executor_id:       T59_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T59_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T59_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             0, "empty: node_count=0");
    assert_eq!(ec,             0, "empty: edge_count=0");
    assert_eq!(ntritriactc,    0, "empty: NTRITRIACTC=0");
    assert_eq!(nhtritriactc,   0, "empty: NHTRITRIACTC=0");
    assert_eq!(nabso,          0, "empty: NABSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             1, "single: node_count=1");
    assert_eq!(ec,             0, "single: edge_count=0");
    assert_eq!(ntritriactc,    0, "single: NTRITRIACTC=0");
    assert_eq!(nhtritriactc,   0, "single: NHTRITRIACTC=0");
    assert_eq!(nabso,          0, "single: NABSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NTRITRIACTC:  1^33+1^33 = 2.
// NHTRITRIACTC: (1+1)^32 = 2^32 = 4_294_967_296.
// NABSO:        (1²+1²)^27 = 2^27 = 134_217_728.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);
    add_edge(T59_ID_A, T59_ID_B, "t59.e.ab");

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             2,             "k2: node_count=2");
    assert_eq!(ec,             1,             "k2: edge_count=1");
    assert_eq!(ntritriactc,    2,             "k2: NTRITRIACTC=2 (1\u{00b3}\u{00b3}+1\u{00b3}\u{00b3}=2)");
    assert_eq!(nhtritriactc,   4_294_967_296, "k2: NHTRITRIACTC=4_294_967_296 (2\u{00b3}\u{00b2}=2^32)");
    assert_eq!(nabso,          134_217_728,   "k2: NABSO=134_217_728 (2\u{00b2}\u{2077}=2^27)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NTRITRIACTC:  3×2^33 = 3×8_589_934_592 = 25_769_803_776.
// NHTRITRIACTC: 2×(2+2)^32 = 2×4^32 = 2×2^64 → SATURATES (4^32=2^64>u64::MAX per-edge).
// NABSO:        2×(4+4)^27 = 2×8^27 = 2×2^81 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);
    add_node(T59_VEC_C, T59_KEY_C, T59_ID_C);
    add_edge(T59_ID_A, T59_ID_B, "t59.e.ab");
    add_edge(T59_ID_B, T59_ID_C, "t59.e.bc");

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             3,               "p3: node_count=3");
    assert_eq!(ec,             2,               "p3: edge_count=2");
    assert_eq!(ntritriactc,    25_769_803_776,  "p3: NTRITRIACTC=25_769_803_776 (3\u{00d7}2\u{00b3}\u{00b3})");
    assert_eq!(nhtritriactc,   u64::MAX,        "p3: NHTRITRIACTC=u64::MAX (4\u{00b3}\u{00b2}=2^64>u64::MAX per-edge; saturated)");
    assert_eq!(nabso,          u64::MAX,        "p3: NABSO=u64::MAX (8\u{00b2}\u{2077}=2^81>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NTRITRIACTC:  3×4^33 → SATURATES (4^32=2^64>u64::MAX per-node).
// NHTRITRIACTC: 3×(4+4)^32 = 3×8^32 = 3×2^96 → SATURATES.
// NABSO:        3×(16+16)^27 = 3×32^27 = 3×2^135 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);
    add_node(T59_VEC_C, T59_KEY_C, T59_ID_C);
    add_edge(T59_ID_A, T59_ID_B, "t59.e.ab");
    add_edge(T59_ID_B, T59_ID_A, "t59.e.ba");
    add_edge(T59_ID_B, T59_ID_C, "t59.e.bc");
    add_edge(T59_ID_C, T59_ID_B, "t59.e.cb");
    add_edge(T59_ID_A, T59_ID_C, "t59.e.ac");
    add_edge(T59_ID_C, T59_ID_A, "t59.e.ca");

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(ntritriactc,    u64::MAX, "k3: NTRITRIACTC=u64::MAX (3\u{00d7}4\u{00b3}\u{00b3}>>u64::MAX; saturated)");
    assert_eq!(nhtritriactc,   u64::MAX, "k3: NHTRITRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{00b2}=3\u{00d7}2^96>>u64::MAX; saturated)");
    assert_eq!(nabso,          u64::MAX, "k3: NABSO=u64::MAX (3\u{00d7}32\u{00b2}\u{2077}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NTRITRIACTC:  5×4^33 → SATURATES.
// NHTRITRIACTC: 4×(4+4)^32 → SATURATES.
// NABSO:        4×(16+16)^27 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);
    add_node(T59_VEC_C, T59_KEY_C, T59_ID_C);
    add_node(T59_VEC_D, T59_KEY_D, T59_ID_D);
    add_node(T59_VEC_E, T59_KEY_E, T59_ID_E);
    add_edge(T59_ID_A, T59_ID_B, "t59.e.ab");
    add_edge(T59_ID_A, T59_ID_C, "t59.e.ac");
    add_edge(T59_ID_A, T59_ID_D, "t59.e.ad");
    add_edge(T59_ID_A, T59_ID_E, "t59.e.ae");

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(ntritriactc,    u64::MAX, "k14: NTRITRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{00b3}>u64::MAX; saturated)");
    assert_eq!(nhtritriactc,   u64::MAX, "k14: NHTRITRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{00b2}>>u64::MAX; saturated)");
    assert_eq!(nabso,          u64::MAX, "k14: NABSO=u64::MAX (4\u{00d7}32\u{00b2}\u{2077}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NTRITRIACTC:  2×2^33+2×3^33 = 2^34 + 2×3^33.
//   3^32=1_853_020_188_851_841; 3^33=3×3^32=5_559_060_566_555_523; 2×3^33=11_118_121_133_111_046.
//   2^34=17_179_869_184. Total=11_118_138_312_980_230.
// NHTRITRIACTC: (2+3)^32+(3+3)^32+(3+2)^32 = 2×5^32+6^32; 5^32>>u64::MAX per-edge → SATURATES.
// NABSO:        13^27+18^27+13^27 — 13^27>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);
    add_node(T59_VEC_C, T59_KEY_C, T59_ID_C);
    add_node(T59_VEC_D, T59_KEY_D, T59_ID_D);
    add_edge(T59_ID_A, T59_ID_B, "t59.e.ab");
    add_edge(T59_ID_B, T59_ID_C, "t59.e.bc");
    add_edge(T59_ID_C, T59_ID_D, "t59.e.cd");

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             4,                        "p4: node_count=4");
    assert_eq!(ec,             3,                        "p4: edge_count=3");
    assert_eq!(ntritriactc,    11_118_138_312_980_230,   "p4: NTRITRIACTC=11_118_138_312_980_230 (2\u{00d7}2\u{00b3}\u{00b3}+2\u{00d7}3\u{00b3}\u{00b3}; 3\u{00b3}\u{00b3}=5_559_060_566_555_523)");
    assert_eq!(nhtritriactc,   u64::MAX,                 "p4: NHTRITRIACTC=u64::MAX (5\u{00b3}\u{00b2}>>u64::MAX per-edge; saturated)");
    assert_eq!(nabso,          u64::MAX,                 "p4: NABSO=u64::MAX (13\u{00b2}\u{2077}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NTRITRIACTC:  4×9^33 → SATURATES → u64::MAX.
// NHTRITRIACTC: 6×18^32 → SATURATES → u64::MAX.
// NABSO:        6×162^27 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);
    add_node(T59_VEC_C, T59_KEY_C, T59_ID_C);
    add_node(T59_VEC_D, T59_KEY_D, T59_ID_D);
    add_edge(T59_ID_A, T59_ID_B, "t59.e.ab");
    add_edge(T59_ID_B, T59_ID_A, "t59.e.ba");
    add_edge(T59_ID_A, T59_ID_C, "t59.e.ac");
    add_edge(T59_ID_C, T59_ID_A, "t59.e.ca");
    add_edge(T59_ID_A, T59_ID_D, "t59.e.ad");
    add_edge(T59_ID_D, T59_ID_A, "t59.e.da");
    add_edge(T59_ID_B, T59_ID_C, "t59.e.bc");
    add_edge(T59_ID_C, T59_ID_B, "t59.e.cb");
    add_edge(T59_ID_B, T59_ID_D, "t59.e.bd");
    add_edge(T59_ID_D, T59_ID_B, "t59.e.db");
    add_edge(T59_ID_C, T59_ID_D, "t59.e.cd");
    add_edge(T59_ID_D, T59_ID_C, "t59.e.dc");

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(ntritriactc,    u64::MAX, "k4: NTRITRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{00b3} >> u64::MAX; saturated)");
    assert_eq!(nhtritriactc,   u64::MAX, "k4: NHTRITRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{00b2} >> u64::MAX; saturated)");
    assert_eq!(nabso,          u64::MAX, "k4: NABSO=u64::MAX (6\u{00d7}162\u{00b2}\u{2077} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NTRITRIACTC=0; NHTRITRIACTC=0; NABSO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             2, "two-iso: node_count=2");
    assert_eq!(ec,             0, "two-iso: edge_count=0");
    assert_eq!(ntritriactc,    0, "two-iso: NTRITRIACTC=0");
    assert_eq!(nhtritriactc,   0, "two-iso: NHTRITRIACTC=0");
    assert_eq!(nabso,          0, "two-iso: NABSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NTRITRIACTC:  5×6^33 → SATURATES (6^33 >> u64::MAX per-node).
// NHTRITRIACTC: 6×12^32 → SATURATES (12^32>>u64::MAX per-edge).
// NABSO:        6×72^27 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T59_VEC_A, T59_KEY_A, T59_ID_A);
    add_node(T59_VEC_B, T59_KEY_B, T59_ID_B);
    add_node(T59_VEC_C, T59_KEY_C, T59_ID_C);
    add_node(T59_VEC_D, T59_KEY_D, T59_ID_D);
    add_node(T59_VEC_E, T59_KEY_E, T59_ID_E);
    add_edge(T59_ID_A, T59_ID_C, "t59.e.ac");
    add_edge(T59_ID_A, T59_ID_D, "t59.e.ad");
    add_edge(T59_ID_A, T59_ID_E, "t59.e.ae");
    add_edge(T59_ID_B, T59_ID_C, "t59.e.bc");
    add_edge(T59_ID_B, T59_ID_D, "t59.e.bd");
    add_edge(T59_ID_B, T59_ID_E, "t59.e.be");

    let (ntritriactc, nhtritriactc, nabso, ec, nc) = gos_runtime::graph_topo_indices59();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(ntritriactc,    u64::MAX, "k23: NTRITRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{00b3}; 6\u{00b3}\u{00b3}>>u64::MAX per-node; saturated)");
    assert_eq!(nhtritriactc,   u64::MAX, "k23: NHTRITRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{00b2} >> u64::MAX; per-edge saturates)");
    assert_eq!(nabso,          u64::MAX, "k23: NABSO=u64::MAX (6\u{00d7}72\u{00b2}\u{2077} >> u64::MAX; per-edge saturates)");
}
