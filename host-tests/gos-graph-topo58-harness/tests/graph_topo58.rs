// gos-graph-topo58-harness — V3.69 NDOTRIACTC + NHDOTRIACTC + NAASO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices58()`:
//   Returns (ndotriactc, nhdotriactc, naaso, edge_count, node_count)
//   - ndotriactc  = NDOTRIACTC(G) = Σ_v S(v)^32                   (exact u64; S-Dotriacontic vertex sum)
//   - nhdotriactc = NHDOTRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^31         (exact u64; S-Hentriacontic edge-sum)
//   - naaso       = NAASO(G)      = Σ_{uv∈E} (S_u²+S_v²)^26       (exact u64; S-Dopentecontyl Sombor, α=52)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NDOTRIACTC(G) = Σ_v S(v)^32
//     S-Dotriacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56), NHENTRIACTC=Σ S³¹ (topo57),
//       NDOTRIACTC=Σ S³² (topo58).
//     NDOTRIACTC = n·S^32 for S-regular.
//     Overflow: S^32 ≤ 16129^32 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^32 = s16 × s16  (perfect square).
//
//   NHDOTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^31
//     S-Hentriacontic edge-sum; extends the S-power-edge series:
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
//       NHDOTRIACTC=Σ(S+S)³¹ (topo58).
//     NHDOTRIACTC = |E|·(2S)^31 = 2147483648|E|·S^31 for S-regular.
//     Overflow per edge: (2×16129)^31 → saturating u128 accumulator.
//
//   NAASO(G) = Σ_{uv∈E} (S_u²+S_v²)^26
//     S-Dopentecontyl Sombor: generalised Sombor SO^α with α=52 on S-variant.
//     Single-letter alphabet exhausted after NBSO (α=50, topo57); 3rd-pass double-letter "AA".
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52).
//     NAASO = |E|·(2S²)^26 = 67108864|E|·S^52 for S-regular.
//     Overflow per edge: (2×16129²)^26 → saturating u128 accumulator.
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
//  Graph     NDOTRIACTC(exact)              NHDOTRIACTC(exact)            NAASO(exact)             edges  nodes
//  Empty                   0                             0                         0               0      0
//  1 node                  0                             0                         0               0      1
//  K₂                      2                   2_147_483_648                67_108_864               1      2
//  P₃          12_884_901_888       9_223_372_036_854_775_808           u64::MAX(sat.)              2      3
//  K₃          u64::MAX(sat.)            u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}     u64::MAX(sat.)            u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       3_706_048_967_638_274         u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated              0                             0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NDOTRIACTC:  1^32 + 1^32 = 2. ✓
//     NHDOTRIACTC: (1+1)^31 = 2^31 = 2_147_483_648. ✓
//     NAASO:       (1²+1²)^26 = 2^26 = 67_108_864. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NDOTRIACTC:  3×2^32 = 3×4_294_967_296 = 12_884_901_888. ✓
//     NHDOTRIACTC: 2×(2+2)^31 = 2×4^31 = 2×2^62 = 2^63 = 9_223_372_036_854_775_808. ✓
//       (4^31=2^62=4_611_686_018_427_387_904; 2×2^62=2^63<u64::MAX=2^64-1)
//     NAASO:       2×(4+4)^26 = 2×8^26 = 2×2^78 → SATURATES (8^26=2^78>u64::MAX per-edge). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NDOTRIACTC:  3×4^32 = 3×2^64 → SATURATES (4^32=2^64>u64::MAX=2^64-1 per-node). ✓
//     NHDOTRIACTC: 3×(4+4)^31 = 3×8^31 = 3×2^93 → SATURATES. ✓
//     NAASO:       3×(16+16)^26 = 3×32^26 = 3×2^130 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NDOTRIACTC:  5×4^32 → SATURATES. ✓
//     NHDOTRIACTC: 4×8^31 → SATURATES. ✓
//     NAASO:       4×32^26 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NDOTRIACTC:  2×2^32 + 2×3^32 = 2^33 + 2×3^32.
//       3^32 = (3^16)^2 = 43_046_721^2 = 1_853_020_188_851_841.
//       2×3^32 = 3_706_040_377_703_682.
//       2^33 = 8_589_934_592.
//       Total = 3_706_040_377_703_682 + 8_589_934_592 = 3_706_048_967_638_274. ✓
//     NHDOTRIACTC: (2+3)^31+(3+3)^31+(3+2)^31 = 2×5^31+6^31
//       5^28>u64::MAX per-edge → SATURATES. ✓
//     NAASO:       (4+9)^26+(9+9)^26+(9+4)^26 → 13^26>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NDOTRIACTC:  4×9^32 → SATURATES → u64::MAX. ✓
//     NHDOTRIACTC: 6×18^31 → SATURATES. ✓
//     NAASO:       → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NDOTRIACTC:  5×6^32 → SATURATES → u64::MAX. ✓
//     NHDOTRIACTC: 6×12^31 → SATURATES. ✓
//     NAASO:       6×72^26 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NDOTRIACTC   = n·S^32                                                  for S-regular ✓
//   NHDOTRIACTC  = |E|·(2S)^31 = 2147483648|E|·S^31                        for S-regular ✓
//   NAASO        = |E|·(2S²)^26 = 67108864|E|·S^52                          for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 2_147_483_648, 67_108_864, 1, 2)
//  4.  Path P₃ = A-B-C                   → (12_884_901_888, 9_223_372_036_854_775_808, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (3_706_048_967_638_274, u64::MAX, u64::MAX, 3, 4)
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

const T58_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_58");
const T58_EXEC:   ExecutorId = ExecutorId::from_ascii("t58.exec");

const T58_KEY_A: &str = "t58.alpha";
const T58_KEY_B: &str = "t58.beta";
const T58_KEY_C: &str = "t58.gamma";
const T58_KEY_D: &str = "t58.delta";
const T58_KEY_E: &str = "t58.epsilon";

const T58_ID_A: NodeId = derive_node_id(T58_PLUGIN, T58_KEY_A);
const T58_ID_B: NodeId = derive_node_id(T58_PLUGIN, T58_KEY_B);
const T58_ID_C: NodeId = derive_node_id(T58_PLUGIN, T58_KEY_C);
const T58_ID_D: NodeId = derive_node_id(T58_PLUGIN, T58_KEY_D);
const T58_ID_E: NodeId = derive_node_id(T58_PLUGIN, T58_KEY_E);

// L4=145 namespace for this harness.
const T58_VEC_A: VectorAddress = VectorAddress::new(145, 1, 1, 0);
const T58_VEC_B: VectorAddress = VectorAddress::new(145, 1, 2, 0);
const T58_VEC_C: VectorAddress = VectorAddress::new(145, 1, 3, 0);
const T58_VEC_D: VectorAddress = VectorAddress::new(145, 2, 1, 0);
const T58_VEC_E: VectorAddress = VectorAddress::new(145, 2, 2, 0);

const T58_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T58_PLUGIN,
    name:         "kl-graph-topo58-harness",
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
        executor_id:       T58_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T58_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T58_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           0, "empty: node_count=0");
    assert_eq!(ec,           0, "empty: edge_count=0");
    assert_eq!(ndotriactc,   0, "empty: NDOTRIACTC=0");
    assert_eq!(nhdotriactc,  0, "empty: NHDOTRIACTC=0");
    assert_eq!(naaso,        0, "empty: NAASO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           1, "single: node_count=1");
    assert_eq!(ec,           0, "single: edge_count=0");
    assert_eq!(ndotriactc,   0, "single: NDOTRIACTC=0");
    assert_eq!(nhdotriactc,  0, "single: NHDOTRIACTC=0");
    assert_eq!(naaso,        0, "single: NAASO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NDOTRIACTC:  1^32+1^32 = 2.
// NHDOTRIACTC: (1+1)^31 = 2^31 = 2_147_483_648.
// NAASO:       (1²+1²)^26 = 2^26 = 67_108_864.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);
    add_edge(T58_ID_A, T58_ID_B, "t58.e.ab");

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           2,             "k2: node_count=2");
    assert_eq!(ec,           1,             "k2: edge_count=1");
    assert_eq!(ndotriactc,   2,             "k2: NDOTRIACTC=2 (1\u{00b3}\u{00b2}+1\u{00b3}\u{00b2}=2)");
    assert_eq!(nhdotriactc,  2_147_483_648, "k2: NHDOTRIACTC=2_147_483_648 (2\u{00b3}\u{00b9}=2^31)");
    assert_eq!(naaso,        67_108_864,    "k2: NAASO=67_108_864 (2\u{00b2}\u{2076}=2^26)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NDOTRIACTC:  3×2^32 = 3×4_294_967_296 = 12_884_901_888.
// NHDOTRIACTC: 2×(2+2)^31 = 2×4^31 = 2×2^62 = 2^63 = 9_223_372_036_854_775_808.
// NAASO:       2×(4+4)^26 = 2×8^26 = 2×2^78 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);
    add_node(T58_VEC_C, T58_KEY_C, T58_ID_C);
    add_edge(T58_ID_A, T58_ID_B, "t58.e.ab");
    add_edge(T58_ID_B, T58_ID_C, "t58.e.bc");

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           3,                            "p3: node_count=3");
    assert_eq!(ec,           2,                            "p3: edge_count=2");
    assert_eq!(ndotriactc,   12_884_901_888,               "p3: NDOTRIACTC=12_884_901_888 (3\u{00d7}2\u{00b3}\u{00b2})");
    assert_eq!(nhdotriactc,  9_223_372_036_854_775_808,    "p3: NHDOTRIACTC=9_223_372_036_854_775_808 (2\u{00d7}4\u{00b3}\u{00b9}=2^63)");
    assert_eq!(naaso,        u64::MAX,                     "p3: NAASO=u64::MAX (8\u{00b2}\u{2076}=2^78>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NDOTRIACTC:  3×4^32 = 3×2^64 → SATURATES (4^32=2^64>u64::MAX=2^64-1 per-node).
// NHDOTRIACTC: 3×(4+4)^31 = 3×8^31 = 3×2^93 → SATURATES.
// NAASO:       3×(16+16)^26 = 3×32^26 = 3×2^130 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);
    add_node(T58_VEC_C, T58_KEY_C, T58_ID_C);
    add_edge(T58_ID_A, T58_ID_B, "t58.e.ab");
    add_edge(T58_ID_B, T58_ID_A, "t58.e.ba");
    add_edge(T58_ID_B, T58_ID_C, "t58.e.bc");
    add_edge(T58_ID_C, T58_ID_B, "t58.e.cb");
    add_edge(T58_ID_A, T58_ID_C, "t58.e.ac");
    add_edge(T58_ID_C, T58_ID_A, "t58.e.ca");

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           3,        "k3: node_count=3");
    assert_eq!(ec,           3,        "k3: edge_count=3");
    assert_eq!(ndotriactc,   u64::MAX, "k3: NDOTRIACTC=u64::MAX (3\u{00d7}4\u{00b3}\u{00b2}=3\u{00d7}2^64>>u64::MAX; saturated)");
    assert_eq!(nhdotriactc,  u64::MAX, "k3: NHDOTRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{00b9}=3\u{00d7}2^93>>u64::MAX; saturated)");
    assert_eq!(naaso,        u64::MAX, "k3: NAASO=u64::MAX (3\u{00d7}32\u{00b2}\u{2076}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NDOTRIACTC:  5×4^32 → SATURATES.
// NHDOTRIACTC: 4×(4+4)^31 → SATURATES.
// NAASO:       4×(16+16)^26 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);
    add_node(T58_VEC_C, T58_KEY_C, T58_ID_C);
    add_node(T58_VEC_D, T58_KEY_D, T58_ID_D);
    add_node(T58_VEC_E, T58_KEY_E, T58_ID_E);
    add_edge(T58_ID_A, T58_ID_B, "t58.e.ab");
    add_edge(T58_ID_A, T58_ID_C, "t58.e.ac");
    add_edge(T58_ID_A, T58_ID_D, "t58.e.ad");
    add_edge(T58_ID_A, T58_ID_E, "t58.e.ae");

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           5,        "k14: node_count=5");
    assert_eq!(ec,           4,        "k14: edge_count=4");
    assert_eq!(ndotriactc,   u64::MAX, "k14: NDOTRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{00b2}>u64::MAX; saturated)");
    assert_eq!(nhdotriactc,  u64::MAX, "k14: NHDOTRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{00b9}>>u64::MAX; saturated)");
    assert_eq!(naaso,        u64::MAX, "k14: NAASO=u64::MAX (4\u{00d7}32\u{00b2}\u{2076}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NDOTRIACTC:  2×2^32+2×3^32 = 8_589_934_592 + 2×1_853_020_188_851_841 = 3_706_048_967_638_274.
//   3^32=(3^16)^2=43_046_721^2=1_853_020_188_851_841; 2×3^32=3_706_040_377_703_682; +8_589_934_592=3_706_048_967_638_274.
// NHDOTRIACTC: (2+3)^31+(3+3)^31+(3+2)^31 = 2×5^31+6^31; 5^28>u64::MAX → SATURATES.
// NAASO:       13^26+18^26+13^26 — 13^26>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);
    add_node(T58_VEC_C, T58_KEY_C, T58_ID_C);
    add_node(T58_VEC_D, T58_KEY_D, T58_ID_D);
    add_edge(T58_ID_A, T58_ID_B, "t58.e.ab");
    add_edge(T58_ID_B, T58_ID_C, "t58.e.bc");
    add_edge(T58_ID_C, T58_ID_D, "t58.e.cd");

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           4,                      "p4: node_count=4");
    assert_eq!(ec,           3,                      "p4: edge_count=3");
    assert_eq!(ndotriactc,   3_706_048_967_638_274,  "p4: NDOTRIACTC=3_706_048_967_638_274 (2\u{00d7}2\u{00b3}\u{00b2}+2\u{00d7}3\u{00b3}\u{00b2}; 3\u{00b3}\u{00b2}=1_853_020_188_851_841)");
    assert_eq!(nhdotriactc,  u64::MAX,               "p4: NHDOTRIACTC=u64::MAX (5\u{00b3}\u{00b9}>u64::MAX per-edge; saturated)");
    assert_eq!(naaso,        u64::MAX,               "p4: NAASO=u64::MAX (13\u{00b2}\u{2076}>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NDOTRIACTC:  4×9^32 → SATURATES → u64::MAX.
// NHDOTRIACTC: 6×18^31 → SATURATES → u64::MAX.
// NAASO:       6×162^26 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);
    add_node(T58_VEC_C, T58_KEY_C, T58_ID_C);
    add_node(T58_VEC_D, T58_KEY_D, T58_ID_D);
    add_edge(T58_ID_A, T58_ID_B, "t58.e.ab");
    add_edge(T58_ID_B, T58_ID_A, "t58.e.ba");
    add_edge(T58_ID_A, T58_ID_C, "t58.e.ac");
    add_edge(T58_ID_C, T58_ID_A, "t58.e.ca");
    add_edge(T58_ID_A, T58_ID_D, "t58.e.ad");
    add_edge(T58_ID_D, T58_ID_A, "t58.e.da");
    add_edge(T58_ID_B, T58_ID_C, "t58.e.bc");
    add_edge(T58_ID_C, T58_ID_B, "t58.e.cb");
    add_edge(T58_ID_B, T58_ID_D, "t58.e.bd");
    add_edge(T58_ID_D, T58_ID_B, "t58.e.db");
    add_edge(T58_ID_C, T58_ID_D, "t58.e.cd");
    add_edge(T58_ID_D, T58_ID_C, "t58.e.dc");

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           4,        "k4: node_count=4");
    assert_eq!(ec,           6,        "k4: edge_count=6");
    assert_eq!(ndotriactc,   u64::MAX, "k4: NDOTRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{00b2} >> u64::MAX; saturated)");
    assert_eq!(nhdotriactc,  u64::MAX, "k4: NHDOTRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{00b9} >> u64::MAX; saturated)");
    assert_eq!(naaso,        u64::MAX, "k4: NAASO=u64::MAX (6\u{00d7}162\u{00b2}\u{2076} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NDOTRIACTC=0; NHDOTRIACTC=0; NAASO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           2, "two-iso: node_count=2");
    assert_eq!(ec,           0, "two-iso: edge_count=0");
    assert_eq!(ndotriactc,   0, "two-iso: NDOTRIACTC=0");
    assert_eq!(nhdotriactc,  0, "two-iso: NHDOTRIACTC=0");
    assert_eq!(naaso,        0, "two-iso: NAASO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NDOTRIACTC:  5×6^32 → SATURATES (6^32 >> u64::MAX per-node).
// NHDOTRIACTC: 6×12^31 → SATURATES (12^31>>u64::MAX per-edge).
// NAASO:       6×72^26 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T58_VEC_A, T58_KEY_A, T58_ID_A);
    add_node(T58_VEC_B, T58_KEY_B, T58_ID_B);
    add_node(T58_VEC_C, T58_KEY_C, T58_ID_C);
    add_node(T58_VEC_D, T58_KEY_D, T58_ID_D);
    add_node(T58_VEC_E, T58_KEY_E, T58_ID_E);
    add_edge(T58_ID_A, T58_ID_C, "t58.e.ac");
    add_edge(T58_ID_A, T58_ID_D, "t58.e.ad");
    add_edge(T58_ID_A, T58_ID_E, "t58.e.ae");
    add_edge(T58_ID_B, T58_ID_C, "t58.e.bc");
    add_edge(T58_ID_B, T58_ID_D, "t58.e.bd");
    add_edge(T58_ID_B, T58_ID_E, "t58.e.be");

    let (ndotriactc, nhdotriactc, naaso, ec, nc) = gos_runtime::graph_topo_indices58();
    assert_eq!(nc,           5,        "k23: node_count=5");
    assert_eq!(ec,           6,        "k23: edge_count=6");
    assert_eq!(ndotriactc,   u64::MAX, "k23: NDOTRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{00b2}; 6\u{00b3}\u{00b2}>>u64::MAX per-node; saturated)");
    assert_eq!(nhdotriactc,  u64::MAX, "k23: NHDOTRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{00b9} >> u64::MAX; per-edge saturates)");
    assert_eq!(naaso,        u64::MAX, "k23: NAASO=u64::MAX (6\u{00d7}72\u{00b2}\u{2076} >> u64::MAX; per-edge saturates)");
}
