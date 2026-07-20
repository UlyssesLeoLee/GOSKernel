// gos-graph-topo60-harness — V3.71 NTETRTRIACTC + NHTETRTRIACTC + NACSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices60()`:
//   Returns (ntetrtriactc, nhtetrtriactc, nacso, edge_count, node_count)
//   - ntetrtriactc  = NTETRTRIACTC(G) = Σ_v S(v)^34                   (exact u64; S-Tetratriacontic vertex sum)
//   - nhtetrtriactc = NHTETRTRIACTC(G)= Σ_{uv∈E} (S_u+S_v)^33         (exact u64; S-Tritriacontic edge-sum)
//   - nacso         = NACSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^28       (exact u64; S-Hexapentacontyl Sombor, α=56)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTETRTRIACTC(G) = Σ_v S(v)^34
//     S-Tetratriacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55), NTRIACTC=Σ S³⁰ (topo56), NHENTRIACTC=Σ S³¹ (topo57),
//       NDOTRIACTC=Σ S³² (topo58), NTRITRIACTC=Σ S³³ (topo59), NTETRTRIACTC=Σ S³⁴ (topo60).
//     NTETRTRIACTC = n·S^34 for S-regular.
//     Overflow: S^34 ≤ 16129^34 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^34 = s16 × s16 × s2  (s^32 as perfect square, multiply by s^2).
//
//   NHTETRTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^33
//     S-Tritriacontic edge-sum; extends the S-power-edge series:
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
//       NHTETRTRIACTC=Σ(S+S)³³ (topo60).
//     NHTETRTRIACTC = |E|·(2S)^33 = 8589934592|E|·S^33 for S-regular.
//     Overflow per edge: (2×16129)^33 → saturating u128 accumulator.
//     Implementation: ss^33 = ss16 × ss16 × ss  (ss^32 as perfect square, then × ss).
//
//   NACSO(G) = Σ_{uv∈E} (S_u²+S_v²)^28
//     S-Hexapentacontyl Sombor: generalised Sombor SO^α with α=56 on S-variant.
//     3rd-pass double-letter "AC" (after NAASO α=52, topo58; NABSO α=54, topo59).
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46),
//     NASO(topo56,α=48), NBSO(topo57,α=50), NAASO(topo58,α=52), NABSO(topo59,α=54),
//     NACSO(topo60,α=56).
//     NACSO = |E|·(2S²)^28 = 268435456|E|·S^56 for S-regular.
//     Overflow per edge: (2×16129²)^28 → saturating u128 accumulator.
//     Implementation: s2s^28 = s2s16 × s2s8 × s2s4.
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
//  Graph     NTETRTRIACTC(exact)            NHTETRTRIACTC(exact)          NACSO(exact)             edges  nodes
//  Empty                   0                             0                        0               0      0
//  1 node                  0                             0                        0               0      1
//  K₂                      2                   8_589_934_592               268_435_456               1      2
//  P₃          51_539_607_552            u64::MAX(sat.)              u64::MAX(sat.)              2      3
//  K₃          u64::MAX(sat.)            u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}     u64::MAX(sat.)            u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       33_354_397_759_071_506        u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated              0                             0                        0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTETRTRIACTC:  1^34 + 1^34 = 2. ✓
//     NHTETRTRIACTC: (1+1)^33 = 2^33 = 8_589_934_592. ✓
//     NACSO:         (1²+1²)^28 = 2^28 = 268_435_456. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTETRTRIACTC:  3×2^34 = 3×17_179_869_184 = 51_539_607_552. ✓
//     NHTETRTRIACTC: 2×(2+2)^33 = 2×4^33 = 2×2^66 → SATURATES (4^33=2^66>u64::MAX per-edge). ✓
//     NACSO:         2×(4+4)^28 = 2×8^28 = 2×2^84 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTETRTRIACTC:  3×4^34 = 3×2^68 → SATURATES (2^68>u64::MAX per-node). ✓
//     NHTETRTRIACTC: 3×(4+4)^33 = 3×8^33 = 3×2^99 → SATURATES. ✓
//     NACSO:         3×(16+16)^28 = 3×32^28 = 3×2^140 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTETRTRIACTC:  5×4^34 → SATURATES. ✓
//     NHTETRTRIACTC: 4×8^33 → SATURATES. ✓
//     NACSO:         4×32^28 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTETRTRIACTC:  2×2^34 + 2×3^34 = 2^35 + 2×3^34.
//       3^32=1_853_020_188_851_841; 3^34=3^32×9=16_677_181_699_666_569; 2×3^34=33_354_363_399_333_138.
//       2^35=34_359_738_368. Total=33_354_363_399_333_138+34_359_738_368=33_354_397_759_071_506. ✓
//     NHTETRTRIACTC: (2+3)^33+(3+3)^33+(3+2)^33 = 2×5^33+6^33
//       5^28>u64::MAX per-edge → SATURATES. ✓
//     NACSO:        (4+9)^28+(9+9)^28+(9+4)^28 → 13^28>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTETRTRIACTC:  4×9^34 → SATURATES → u64::MAX. ✓
//     NHTETRTRIACTC: 6×18^33 → SATURATES. ✓
//     NACSO:         6×162^28 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTETRTRIACTC:  5×6^34 → SATURATES → u64::MAX. ✓
//     NHTETRTRIACTC: 6×12^33 → SATURATES. ✓
//     NACSO:         6×72^28 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTETRTRIACTC  = n·S^34                                                  for S-regular ✓
//   NHTETRTRIACTC = |E|·(2S)^33 = 8589934592|E|·S^33                        for S-regular ✓
//   NACSO         = |E|·(2S²)^28 = 268435456|E|·S^56                         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 8_589_934_592, 268_435_456, 1, 2)
//  4.  Path P₃ = A-B-C                   → (51_539_607_552, u64::MAX, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (33_354_397_759_071_506, u64::MAX, u64::MAX, 3, 4)
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

const T60_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_60");
const T60_EXEC:   ExecutorId = ExecutorId::from_ascii("t60.exec");

const T60_KEY_A: &str = "t60.alpha";
const T60_KEY_B: &str = "t60.beta";
const T60_KEY_C: &str = "t60.gamma";
const T60_KEY_D: &str = "t60.delta";
const T60_KEY_E: &str = "t60.epsilon";

const T60_ID_A: NodeId = derive_node_id(T60_PLUGIN, T60_KEY_A);
const T60_ID_B: NodeId = derive_node_id(T60_PLUGIN, T60_KEY_B);
const T60_ID_C: NodeId = derive_node_id(T60_PLUGIN, T60_KEY_C);
const T60_ID_D: NodeId = derive_node_id(T60_PLUGIN, T60_KEY_D);
const T60_ID_E: NodeId = derive_node_id(T60_PLUGIN, T60_KEY_E);

// L4=147 namespace for this harness.
const T60_VEC_A: VectorAddress = VectorAddress::new(147, 1, 1, 0);
const T60_VEC_B: VectorAddress = VectorAddress::new(147, 1, 2, 0);
const T60_VEC_C: VectorAddress = VectorAddress::new(147, 1, 3, 0);
const T60_VEC_D: VectorAddress = VectorAddress::new(147, 2, 1, 0);
const T60_VEC_E: VectorAddress = VectorAddress::new(147, 2, 2, 0);

const T60_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T60_PLUGIN,
    name:         "kl-graph-topo60-harness",
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
        executor_id:       T60_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T60_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T60_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(ntetrtriactc,     0, "empty: NTETRTRIACTC=0");
    assert_eq!(nhtetrtriactc,    0, "empty: NHTETRTRIACTC=0");
    assert_eq!(nacso,            0, "empty: NACSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(ntetrtriactc,     0, "single: NTETRTRIACTC=0");
    assert_eq!(nhtetrtriactc,    0, "single: NHTETRTRIACTC=0");
    assert_eq!(nacso,            0, "single: NACSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NTETRTRIACTC:  1^34+1^34 = 2.
// NHTETRTRIACTC: (1+1)^33 = 2^33 = 8_589_934_592.
// NACSO:         (1²+1²)^28 = 2^28 = 268_435_456.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);
    add_edge(T60_ID_A, T60_ID_B, "t60.e.ab");

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               2,             "k2: node_count=2");
    assert_eq!(ec,               1,             "k2: edge_count=1");
    assert_eq!(ntetrtriactc,     2,             "k2: NTETRTRIACTC=2 (1\u{00b3}\u{2074}+1\u{00b3}\u{2074}=2)");
    assert_eq!(nhtetrtriactc,    8_589_934_592, "k2: NHTETRTRIACTC=8_589_934_592 (2\u{00b3}\u{00b3}=2^33)");
    assert_eq!(nacso,            268_435_456,   "k2: NACSO=268_435_456 (2\u{00b2}\u{2078}=2^28)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NTETRTRIACTC:  3×2^34 = 3×17_179_869_184 = 51_539_607_552.
// NHTETRTRIACTC: 2×(2+2)^33 = 2×4^33 = 2×2^66 → SATURATES (4^33=2^66>u64::MAX per-edge).
// NACSO:         2×(4+4)^28 = 2×8^28 = 2×2^84 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);
    add_node(T60_VEC_C, T60_KEY_C, T60_ID_C);
    add_edge(T60_ID_A, T60_ID_B, "t60.e.ab");
    add_edge(T60_ID_B, T60_ID_C, "t60.e.bc");

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               3,               "p3: node_count=3");
    assert_eq!(ec,               2,               "p3: edge_count=2");
    assert_eq!(ntetrtriactc,     51_539_607_552,  "p3: NTETRTRIACTC=51_539_607_552 (3\u{00d7}2\u{00b3}\u{2074})");
    assert_eq!(nhtetrtriactc,    u64::MAX,        "p3: NHTETRTRIACTC=u64::MAX (4\u{00b3}\u{00b3}=2^66>u64::MAX per-edge; saturated)");
    assert_eq!(nacso,            u64::MAX,        "p3: NACSO=u64::MAX (8\u{00b2}\u{2078}=2^84>u64::MAX per-edge; saturated)");
}

// ── Test 5: Triangle K₃ ─────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NTETRTRIACTC:  3×4^34 → SATURATES (4^34=2^68>u64::MAX per-node).
// NHTETRTRIACTC: 3×(4+4)^33 = 3×8^33 = 3×2^99 → SATURATES.
// NACSO:         3×(16+16)^28 = 3×32^28 = 3×2^140 → SATURATES.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);
    add_node(T60_VEC_C, T60_KEY_C, T60_ID_C);
    add_edge(T60_ID_A, T60_ID_B, "t60.e.ab");
    add_edge(T60_ID_B, T60_ID_A, "t60.e.ba");
    add_edge(T60_ID_B, T60_ID_C, "t60.e.bc");
    add_edge(T60_ID_C, T60_ID_B, "t60.e.cb");
    add_edge(T60_ID_A, T60_ID_C, "t60.e.ac");
    add_edge(T60_ID_C, T60_ID_A, "t60.e.ca");

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(ntetrtriactc,     u64::MAX, "k3: NTETRTRIACTC=u64::MAX (3\u{00d7}4\u{00b3}\u{2074}>>u64::MAX; saturated)");
    assert_eq!(nhtetrtriactc,    u64::MAX, "k3: NHTETRTRIACTC=u64::MAX (3\u{00d7}8\u{00b3}\u{00b3}=3\u{00d7}2^99>>u64::MAX; saturated)");
    assert_eq!(nacso,            u64::MAX, "k3: NACSO=u64::MAX (3\u{00d7}32\u{00b2}\u{2078}>>u64::MAX; saturated)");
}

// ── Test 6: Star K_{1,4} ────────────────────────────────────────────────────
// Center A: d=4. Leaves B,C,D,E: d=1.
// S(center)=4×1=4. S(leaf)=1×4=4. S-uniform S=4. 4 edges, 5 nodes.
// NTETRTRIACTC:  5×4^34 → SATURATES.
// NHTETRTRIACTC: 4×(4+4)^33 → SATURATES.
// NACSO:         4×(16+16)^28 → SATURATES.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);
    add_node(T60_VEC_C, T60_KEY_C, T60_ID_C);
    add_node(T60_VEC_D, T60_KEY_D, T60_ID_D);
    add_node(T60_VEC_E, T60_KEY_E, T60_ID_E);
    add_edge(T60_ID_A, T60_ID_B, "t60.e.ab");
    add_edge(T60_ID_A, T60_ID_C, "t60.e.ac");
    add_edge(T60_ID_A, T60_ID_D, "t60.e.ad");
    add_edge(T60_ID_A, T60_ID_E, "t60.e.ae");

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(ntetrtriactc,     u64::MAX, "k14: NTETRTRIACTC=u64::MAX (5\u{00d7}4\u{00b3}\u{2074}>u64::MAX; saturated)");
    assert_eq!(nhtetrtriactc,    u64::MAX, "k14: NHTETRTRIACTC=u64::MAX (4\u{00d7}8\u{00b3}\u{00b3}>>u64::MAX; saturated)");
    assert_eq!(nacso,            u64::MAX, "k14: NACSO=u64::MAX (4\u{00d7}32\u{00b2}\u{2078}>>u64::MAX; saturated)");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1. S: S(A)=2,S(B)=3,S(C)=3,S(D)=2. 3 edges, 4 nodes.
// NTETRTRIACTC:  2×2^34+2×3^34 = 2^35 + 2×3^34.
//   3^32=1_853_020_188_851_841; 3^34=3^32×9=16_677_181_699_666_569; 2×3^34=33_354_363_399_333_138.
//   2^35=34_359_738_368. Total=33_354_363_399_333_138+34_359_738_368=33_354_397_759_071_506.
// NHTETRTRIACTC: (2+3)^33+(3+3)^33+(3+2)^33 = 2×5^33+6^33; 5^28>u64::MAX per-edge → SATURATES.
// NACSO:         13^28+18^28+13^28 — 13^28>>u64::MAX per-edge → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);
    add_node(T60_VEC_C, T60_KEY_C, T60_ID_C);
    add_node(T60_VEC_D, T60_KEY_D, T60_ID_D);
    add_edge(T60_ID_A, T60_ID_B, "t60.e.ab");
    add_edge(T60_ID_B, T60_ID_C, "t60.e.bc");
    add_edge(T60_ID_C, T60_ID_D, "t60.e.cd");

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               4,                          "p4: node_count=4");
    assert_eq!(ec,               3,                          "p4: edge_count=3");
    assert_eq!(ntetrtriactc,     33_354_397_759_071_506,     "p4: NTETRTRIACTC=33_354_397_759_071_506 (2\u{00d7}2\u{00b3}\u{2074}+2\u{00d7}3\u{00b3}\u{2074}; 3\u{00b3}\u{2074}=16_677_181_699_666_569)");
    assert_eq!(nhtetrtriactc,    u64::MAX,                   "p4: NHTETRTRIACTC=u64::MAX (5\u{00b3}\u{00b3}>>u64::MAX per-edge; saturated)");
    assert_eq!(nacso,            u64::MAX,                   "p4: NACSO=u64::MAX (13\u{00b2}\u{2078}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NTETRTRIACTC:  4×9^34 → SATURATES → u64::MAX.
// NHTETRTRIACTC: 6×18^33 → SATURATES → u64::MAX.
// NACSO:         6×162^28 → SATURATES → u64::MAX.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);
    add_node(T60_VEC_C, T60_KEY_C, T60_ID_C);
    add_node(T60_VEC_D, T60_KEY_D, T60_ID_D);
    add_edge(T60_ID_A, T60_ID_B, "t60.e.ab");
    add_edge(T60_ID_B, T60_ID_A, "t60.e.ba");
    add_edge(T60_ID_A, T60_ID_C, "t60.e.ac");
    add_edge(T60_ID_C, T60_ID_A, "t60.e.ca");
    add_edge(T60_ID_A, T60_ID_D, "t60.e.ad");
    add_edge(T60_ID_D, T60_ID_A, "t60.e.da");
    add_edge(T60_ID_B, T60_ID_C, "t60.e.bc");
    add_edge(T60_ID_C, T60_ID_B, "t60.e.cb");
    add_edge(T60_ID_B, T60_ID_D, "t60.e.bd");
    add_edge(T60_ID_D, T60_ID_B, "t60.e.db");
    add_edge(T60_ID_C, T60_ID_D, "t60.e.cd");
    add_edge(T60_ID_D, T60_ID_C, "t60.e.dc");

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(ntetrtriactc,     u64::MAX, "k4: NTETRTRIACTC=u64::MAX (4\u{00d7}9\u{00b3}\u{2074} >> u64::MAX; saturated)");
    assert_eq!(nhtetrtriactc,    u64::MAX, "k4: NHTETRTRIACTC=u64::MAX (6\u{00d7}18\u{00b3}\u{00b3} >> u64::MAX; saturated)");
    assert_eq!(nacso,            u64::MAX, "k4: NACSO=u64::MAX (6\u{00d7}162\u{00b2}\u{2078} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NTETRTRIACTC=0; NHTETRTRIACTC=0; NACSO=0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               2, "two-iso: node_count=2");
    assert_eq!(ec,               0, "two-iso: edge_count=0");
    assert_eq!(ntetrtriactc,     0, "two-iso: NTETRTRIACTC=0");
    assert_eq!(nhtetrtriactc,    0, "two-iso: NHTETRTRIACTC=0");
    assert_eq!(nacso,            0, "two-iso: NACSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NTETRTRIACTC:  5×6^34 → SATURATES (6^34 >> u64::MAX per-node).
// NHTETRTRIACTC: 6×12^33 → SATURATES (12^33>>u64::MAX per-edge).
// NACSO:         6×72^28 → SATURATES (per-edge >> u64::MAX).
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T60_VEC_A, T60_KEY_A, T60_ID_A);
    add_node(T60_VEC_B, T60_KEY_B, T60_ID_B);
    add_node(T60_VEC_C, T60_KEY_C, T60_ID_C);
    add_node(T60_VEC_D, T60_KEY_D, T60_ID_D);
    add_node(T60_VEC_E, T60_KEY_E, T60_ID_E);
    add_edge(T60_ID_A, T60_ID_C, "t60.e.ac");
    add_edge(T60_ID_A, T60_ID_D, "t60.e.ad");
    add_edge(T60_ID_A, T60_ID_E, "t60.e.ae");
    add_edge(T60_ID_B, T60_ID_C, "t60.e.bc");
    add_edge(T60_ID_B, T60_ID_D, "t60.e.bd");
    add_edge(T60_ID_B, T60_ID_E, "t60.e.be");

    let (ntetrtriactc, nhtetrtriactc, nacso, ec, nc) = gos_runtime::graph_topo_indices60();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(ntetrtriactc,     u64::MAX, "k23: NTETRTRIACTC=u64::MAX (5\u{00d7}6\u{00b3}\u{2074}; 6\u{00b3}\u{2074}>>u64::MAX per-node; saturated)");
    assert_eq!(nhtetrtriactc,    u64::MAX, "k23: NHTETRTRIACTC=u64::MAX (6\u{00d7}12\u{00b3}\u{00b3} >> u64::MAX; per-edge saturates)");
    assert_eq!(nacso,            u64::MAX, "k23: NACSO=u64::MAX (6\u{00d7}72\u{00b2}\u{2078} >> u64::MAX; per-edge saturates)");
}
