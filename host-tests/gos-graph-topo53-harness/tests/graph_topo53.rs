// gos-graph-topo53-harness — V3.64 NHEPTATC + NHHEPTATC + NXSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices53()`:
//   Returns (nheptatc, nhheptatc, nxso, edge_count, node_count)
//   - nheptatc  = NHEPTATC(G)  = Σ_v S(v)^27                  (exact u64; S-Heptacosic vertex sum)
//   - nhheptatc = NHHEPTATC(G) = Σ_{uv∈E} (S_u+S_v)^26        (exact u64; S-Hexacosic edge-sum)
//   - nxso      = NXSO(G)      = Σ_{uv∈E} (S_u²+S_v²)^21      (exact u64; S-Dotetracontyl Sombor, α=42)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTATC(G) = Σ_v S(v)^27
//     S-Heptacosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53).
//     NHEPTATC = n·S^27 for S-regular.
//     Overflow: S^27 ≤ 16129^27 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHHEPTATC(G) = Σ_{uv∈E} (S_u+S_v)^26
//     S-Hexacosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53).
//     NHHEPTATC = |E|·(2S)^26 = 67108864|E|·S^26 for S-regular.
//     Overflow per edge: (2×16129)^26 → saturating u128 accumulator.
//
//   NXSO(G) = Σ_{uv∈E} (S_u²+S_v²)^21
//     S-Dotetracontyl Sombor: generalised Sombor SO^α with α=42 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42).
//     (W skipped: NWSO already used for S-Weighted Sombor in topo32.)
//     NXSO = |E|·(2S²)^21 = 2097152|E|·S^42 for S-regular.
//     Overflow per edge: (2×16129²)^21 → saturating u128 accumulator.
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
//  Graph     NHEPTATC(exact)              NHHEPTATC(exact)              NXSO(exact)              edges  nodes
//  Empty                  0                             0                         0               0      0
//  1 node                 0                             0                         0               0      1
//  K₂                     2                    67_108_864                 2_097_152               1      2
//  P₃             402_653_184         9_007_199_254_740_992           u64::MAX(sat.)              2      3
//  K₃       54_043_195_528_445_952        u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}  90_071_992_547_409_920        u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄        15_251_463_405_430           u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated             0                             0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTATC:  1^27 + 1^27 = 2. ✓
//     NHHEPTATC: (1+1)^26 = 2^26 = 67_108_864. ✓
//     NXSO:      (1²+1²)^21 = 2^21 = 2_097_152. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTATC:  3×2^27 = 3×134_217_728 = 402_653_184. ✓
//     NHHEPTATC: 2×(2+2)^26 = 2×4^26 = 2×2^52 = 2^53 = 9_007_199_254_740_992. ✓
//       (4^26=2^52=4_503_599_627_370_496; 2×4^26=9_007_199_254_740_992)
//     NXSO:      2×(4+4)^21 = 2×8^21 = 2×2^63 = 2^64 → SATURATES (2^64 > u64::MAX). ✓
//       (8^21=2^63=9_223_372_036_854_775_808 fits u64 per-edge; sum of 2 does not)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTATC:  3×4^27 = 3×2^54 = 3×18_014_398_509_481_984 = 54_043_195_528_445_952 (fits u64). ✓
//       (4^27=2^54; 3×2^54=54_043_195_528_445_952 < 2^64)
//     NHHEPTATC: 3×(4+4)^26 = 3×8^26 = 3×2^78 → SATURATES (per-edge >> u64::MAX). ✓
//     NXSO:      3×(16+16)^21 = 3×32^21 = 3×2^105 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTATC:  5×4^27 = 5×18_014_398_509_481_984 = 90_071_992_547_409_920 (fits u64). ✓
//     NHHEPTATC: 4×8^26 → SATURATES. ✓
//     NXSO:      4×32^21 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTATC:  2^27+3^27+3^27+2^27 = 2×134_217_728+2×7_625_597_484_987.
//       3^27=3^16×3^8×3^3=43_046_721×6_561×27=7_625_597_484_987
//       2×134_217_728+2×7_625_597_484_987=268_435_456+15_251_194_969_974=15_251_463_405_430. ✓
//     NHHEPTATC: (2+3)^26+(3+3)^26+(3+2)^26 = 2×5^26+6^26
//       5^26=1_490_116_119_384_765_625 (fits u64 alone), but 6^26 per-edge >> u64::MAX → SATURATES. ✓
//     NXSO:      13^20+18^20+13^20 — 13^18>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTATC:  4×9^27 → 9^27>>(u64::MAX per-node) → SATURATES. ✓
//     NHHEPTATC: → SATURATES. ✓
//     NXSO:      → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTATC:  5×6^27 → 6^27>>u64::MAX per-node → SATURATES. ✓
//     NHHEPTATC: → SATURATES. ✓
//     NXSO:      → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTATC  = n·S^27                                        for S-regular ✓
//   NHHEPTATC = |E|·(2S)^26 = 67108864|E|·S^26               for S-regular ✓
//   NXSO      = |E|·(2S²)^21 = 2097152|E|·S^42               for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 67_108_864, 2_097_152, 1, 2)
//  4.  Path P₃ = A-B-C                   → (402_653_184, 9_007_199_254_740_992, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (54_043_195_528_445_952, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (90_071_992_547_409_920, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (15_251_463_405_430, u64::MAX, u64::MAX, 3, 4)
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

const T53_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_53");
const T53_EXEC:   ExecutorId = ExecutorId::from_ascii("t53.exec");

const T53_KEY_A: &str = "t53.alpha";
const T53_KEY_B: &str = "t53.beta";
const T53_KEY_C: &str = "t53.gamma";
const T53_KEY_D: &str = "t53.delta";
const T53_KEY_E: &str = "t53.epsilon";

const T53_ID_A: NodeId = derive_node_id(T53_PLUGIN, T53_KEY_A);
const T53_ID_B: NodeId = derive_node_id(T53_PLUGIN, T53_KEY_B);
const T53_ID_C: NodeId = derive_node_id(T53_PLUGIN, T53_KEY_C);
const T53_ID_D: NodeId = derive_node_id(T53_PLUGIN, T53_KEY_D);
const T53_ID_E: NodeId = derive_node_id(T53_PLUGIN, T53_KEY_E);

// L4=140 namespace for this harness.
const T53_VEC_A: VectorAddress = VectorAddress::new(140, 1, 1, 0);
const T53_VEC_B: VectorAddress = VectorAddress::new(140, 1, 2, 0);
const T53_VEC_C: VectorAddress = VectorAddress::new(140, 1, 3, 0);
const T53_VEC_D: VectorAddress = VectorAddress::new(140, 2, 1, 0);
const T53_VEC_E: VectorAddress = VectorAddress::new(140, 2, 2, 0);

const T53_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T53_PLUGIN,
    name:         "kl-graph-topo53-harness",
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
        executor_id:       T53_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T53_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T53_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        0, "empty: node_count=0");
    assert_eq!(ec,        0, "empty: edge_count=0");
    assert_eq!(nheptatc,  0, "empty: NHEPTATC=0");
    assert_eq!(nhheptatc, 0, "empty: NHHEPTATC=0");
    assert_eq!(nxso,      0, "empty: NXSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NHEPTATC: 0^27=0; NHHEPTATC: no edges; NXSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        1, "single: node_count=1");
    assert_eq!(ec,        0, "single: no edges");
    assert_eq!(nheptatc,  0, "single: NHEPTATC=0 (S=0; 0^27=0)");
    assert_eq!(nhheptatc, 0, "single: NHHEPTATC=0 (no edges)");
    assert_eq!(nxso,      0, "single: NXSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NHEPTATC:  1^27+1^27 = 2.
// NHHEPTATC: (1+1)^26 = 2^26 = 67_108_864.
// NXSO:      (1²+1²)^21 = 2^21 = 2_097_152.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);
    add_edge(T53_ID_A, T53_ID_B, "t53.e.ab");

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        2,          "k2: node_count=2");
    assert_eq!(ec,        1,          "k2: edge_count=1");
    assert_eq!(nheptatc,  2,          "k2: NHEPTATC=2 (1\u{00b2}\u{2077}+1\u{00b2}\u{2077}=2; S-uniform S=1)");
    assert_eq!(nhheptatc, 67_108_864, "k2: NHHEPTATC=67_108_864 ((1+1)\u{00b2}\u{2076}=2\u{00b2}\u{2076}=67_108_864; S-uniform S=1)");
    assert_eq!(nxso,      2_097_152,  "k2: NXSO=2_097_152 ((1\u{00b2}+1\u{00b2})\u{00b2}\u{00b9}=2\u{00b2}\u{00b9}=2_097_152; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NHEPTATC:  3×2^27 = 3×134_217_728 = 402_653_184.
// NHHEPTATC: 2×(2+2)^26 = 2×4^26 = 2×4_503_599_627_370_496 = 9_007_199_254_740_992.
// NXSO:      2×(4+4)^21 = 2×8^21 = 2×9_223_372_036_854_775_808 = 2^64 → SATURATES.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);
    add_node(T53_VEC_C, T53_KEY_C, T53_ID_C);
    add_edge(T53_ID_A, T53_ID_B, "t53.e.ab");
    add_edge(T53_ID_B, T53_ID_C, "t53.e.bc");

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        3,                       "p3: node_count=3");
    assert_eq!(ec,        2,                       "p3: edge_count=2");
    assert_eq!(nheptatc,  402_653_184,             "p3: NHEPTATC=402_653_184 (3\u{00d7}134_217_728; 2\u{00b2}\u{2077}=134_217_728; S-uniform S=2)");
    assert_eq!(nhheptatc, 9_007_199_254_740_992,   "p3: NHHEPTATC=9_007_199_254_740_992 (2\u{00d7}4_503_599_627_370_496; (2+2)\u{00b2}\u{2076}=4\u{00b2}\u{2076}=4_503_599_627_370_496; S-uniform S=2)");
    assert_eq!(nxso,      u64::MAX,                "p3: NXSO=u64::MAX (2\u{00d7}8\u{00b2}\u{00b9}=2\u{00d7}2\u{00b6}\u{00b3}=2\u{2076}\u{2074} > u64::MAX; saturated)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHEPTATC:  3×4^27 = 3×2^54 = 54_043_195_528_445_952 (fits u64).
// NHHEPTATC: 3×(4+4)^26 = 3×8^26 = 3×2^78 → SATURATES.
// NXSO:      3×(16+16)^21 = 3×32^21 = 3×2^105 → SATURATES.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);
    add_node(T53_VEC_C, T53_KEY_C, T53_ID_C);
    add_edge(T53_ID_A, T53_ID_B, "t53.e.ab");
    add_edge(T53_ID_B, T53_ID_A, "t53.e.ba");
    add_edge(T53_ID_B, T53_ID_C, "t53.e.bc");
    add_edge(T53_ID_C, T53_ID_B, "t53.e.cb");
    add_edge(T53_ID_A, T53_ID_C, "t53.e.ac");
    add_edge(T53_ID_C, T53_ID_A, "t53.e.ca");

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        3,                            "k3: node_count=3");
    assert_eq!(ec,        3,                            "k3: edge_count=3");
    assert_eq!(nheptatc,  54_043_195_528_445_952,       "k3: NHEPTATC=54_043_195_528_445_952 (3\u{00d7}18_014_398_509_481_984; 4\u{00b2}\u{2077}=2\u{2075}\u{2074}=18_014_398_509_481_984; S-uniform S=4)");
    assert_eq!(nhheptatc, u64::MAX,                     "k3: NHHEPTATC=u64::MAX (3\u{00d7}8\u{00b2}\u{2076}=3\u{00d7}2\u{2077}\u{2078} >> u64::MAX; saturated)");
    assert_eq!(nxso,      u64::MAX,                     "k3: NXSO=u64::MAX (3\u{00d7}32\u{00b2}\u{00b9}=3\u{00d7}2\u{00b9}\u{2070}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NHEPTATC:  5×4^27 = 5×18_014_398_509_481_984 = 90_071_992_547_409_920 (fits u64).
// NHHEPTATC: 4×8^26 → SATURATES.
// NXSO:      4×32^21 → SATURATES.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);
    add_node(T53_VEC_C, T53_KEY_C, T53_ID_C);
    add_node(T53_VEC_D, T53_KEY_D, T53_ID_D);
    add_node(T53_VEC_E, T53_KEY_E, T53_ID_E);
    add_edge(T53_ID_A, T53_ID_B, "t53.e.ab");
    add_edge(T53_ID_A, T53_ID_C, "t53.e.ac");
    add_edge(T53_ID_A, T53_ID_D, "t53.e.ad");
    add_edge(T53_ID_A, T53_ID_E, "t53.e.ae");

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        5,                            "star: node_count=5");
    assert_eq!(ec,        4,                            "star: edge_count=4");
    assert_eq!(nheptatc,  90_071_992_547_409_920,       "star: NHEPTATC=90_071_992_547_409_920 (5\u{00d7}18_014_398_509_481_984; same S as K\u{2083})");
    assert_eq!(nhheptatc, u64::MAX,                     "star: NHHEPTATC=u64::MAX (4\u{00d7}8\u{00b2}\u{2076} >> u64::MAX; saturated)");
    assert_eq!(nxso,      u64::MAX,                     "star: NXSO=u64::MAX (4\u{00d7}32\u{00b2}\u{00b9} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NHEPTATC:  2^27+3^27+3^27+2^27 = 2×134_217_728+2×7_625_597_484_987 = 15_251_463_405_430.
//   (3^27=3^16×3^8×3^3=43_046_721×6_561×27=7_625_597_484_987)
// NHHEPTATC: 5^26+6^26+5^26 — 6^26>>u64::MAX per-edge → SATURATES.
// NXSO:      13^21+18^21+13^21 — 13^18>>u64::MAX per-edge → SATURATES.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);
    add_node(T53_VEC_C, T53_KEY_C, T53_ID_C);
    add_node(T53_VEC_D, T53_KEY_D, T53_ID_D);
    add_edge(T53_ID_A, T53_ID_B, "t53.e.ab");
    add_edge(T53_ID_B, T53_ID_C, "t53.e.bc");
    add_edge(T53_ID_C, T53_ID_D, "t53.e.cd");

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        4,                    "p4: node_count=4");
    assert_eq!(ec,        3,                    "p4: edge_count=3");
    assert_eq!(nheptatc,  15_251_463_405_430,   "p4: NHEPTATC=15_251_463_405_430 (2\u{00d7}134_217_728+2\u{00d7}7_625_597_484_987; 2\u{00b2}\u{2077}+3\u{00b2}\u{2077}+3\u{00b2}\u{2077}+2\u{00b2}\u{2077})");
    assert_eq!(nhheptatc, u64::MAX,             "p4: NHHEPTATC=u64::MAX (6\u{00b2}\u{2076}>>u64::MAX per-edge; saturated)");
    assert_eq!(nxso,      u64::MAX,             "p4: NXSO=u64::MAX (13\u{00b2}\u{00b9}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHEPTATC:  4×9^27 → SATURATES → u64::MAX.
// NHHEPTATC: 6×18^26 → SATURATES → u64::MAX.
// NXSO:      6×162^21 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);
    add_node(T53_VEC_C, T53_KEY_C, T53_ID_C);
    add_node(T53_VEC_D, T53_KEY_D, T53_ID_D);
    add_edge(T53_ID_A, T53_ID_B, "t53.e.ab");
    add_edge(T53_ID_B, T53_ID_A, "t53.e.ba");
    add_edge(T53_ID_A, T53_ID_C, "t53.e.ac");
    add_edge(T53_ID_C, T53_ID_A, "t53.e.ca");
    add_edge(T53_ID_A, T53_ID_D, "t53.e.ad");
    add_edge(T53_ID_D, T53_ID_A, "t53.e.da");
    add_edge(T53_ID_B, T53_ID_C, "t53.e.bc");
    add_edge(T53_ID_C, T53_ID_B, "t53.e.cb");
    add_edge(T53_ID_B, T53_ID_D, "t53.e.bd");
    add_edge(T53_ID_D, T53_ID_B, "t53.e.db");
    add_edge(T53_ID_C, T53_ID_D, "t53.e.cd");
    add_edge(T53_ID_D, T53_ID_C, "t53.e.dc");

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        4,        "k4: node_count=4");
    assert_eq!(ec,        6,        "k4: edge_count=6");
    assert_eq!(nheptatc,  u64::MAX, "k4: NHEPTATC=u64::MAX (4\u{00d7}9\u{00b2}\u{2077} >> u64::MAX; saturated)");
    assert_eq!(nhheptatc, u64::MAX, "k4: NHHEPTATC=u64::MAX (6\u{00d7}18\u{00b2}\u{2076} >> u64::MAX; saturated)");
    assert_eq!(nxso,      u64::MAX, "k4: NXSO=u64::MAX (6\u{00d7}162\u{00b2}\u{00b9} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHEPTATC=0; NHHEPTATC=0; NXSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        2, "two-iso: node_count=2");
    assert_eq!(ec,        0, "two-iso: edge_count=0");
    assert_eq!(nheptatc,  0, "two-iso: NHEPTATC=0");
    assert_eq!(nhheptatc, 0, "two-iso: NHHEPTATC=0");
    assert_eq!(nxso,      0, "two-iso: NXSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NHEPTATC:  5×6^27 → 6^27>>u64::MAX per-node → SATURATES.
// NHHEPTATC: 6×12^26 → SATURATES (12^26>>u64::MAX per-edge).
// NXSO:      6×72^21 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T53_VEC_A, T53_KEY_A, T53_ID_A);
    add_node(T53_VEC_B, T53_KEY_B, T53_ID_B);
    add_node(T53_VEC_C, T53_KEY_C, T53_ID_C);
    add_node(T53_VEC_D, T53_KEY_D, T53_ID_D);
    add_node(T53_VEC_E, T53_KEY_E, T53_ID_E);
    add_edge(T53_ID_A, T53_ID_C, "t53.e.ac");
    add_edge(T53_ID_A, T53_ID_D, "t53.e.ad");
    add_edge(T53_ID_A, T53_ID_E, "t53.e.ae");
    add_edge(T53_ID_B, T53_ID_C, "t53.e.bc");
    add_edge(T53_ID_B, T53_ID_D, "t53.e.bd");
    add_edge(T53_ID_B, T53_ID_E, "t53.e.be");

    let (nheptatc, nhheptatc, nxso, ec, nc) = gos_runtime::graph_topo_indices53();
    assert_eq!(nc,        5,        "k23: node_count=5");
    assert_eq!(ec,        6,        "k23: edge_count=6");
    assert_eq!(nheptatc,  u64::MAX, "k23: NHEPTATC=u64::MAX (5\u{00d7}6\u{00b2}\u{2077}; 6\u{00b2}\u{2077}>>u64::MAX per-node; saturated)");
    assert_eq!(nhheptatc, u64::MAX, "k23: NHHEPTATC=u64::MAX (6\u{00d7}12\u{00b2}\u{2076} >> u64::MAX; per-edge saturates)");
    assert_eq!(nxso,      u64::MAX, "k23: NXSO=u64::MAX (6\u{00d7}72\u{00b2}\u{00b9} >> u64::MAX; per-edge saturates)");
}
