// gos-graph-topo54-harness — V3.65 NOCTATC + NHOCTATC + NYSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices54()`:
//   Returns (noctatc, nhoctatc, nyso, edge_count, node_count)
//   - noctatc  = NOCTATC(G)  = Σ_v S(v)^28                  (exact u64; S-Octacosic vertex sum)
//   - nhoctatc = NHOCTATC(G) = Σ_{uv∈E} (S_u+S_v)^27        (exact u64; S-Heptacosic edge-sum)
//   - nyso     = NYSO(G)     = Σ_{uv∈E} (S_u²+S_v²)^22      (exact u64; S-Tetratetracontyl Sombor, α=44)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTATC(G) = Σ_v S(v)^28
//     S-Octacosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54).
//     NOCTATC = n·S^28 for S-regular.
//     Overflow: S^28 ≤ 16129^28 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHOCTATC(G) = Σ_{uv∈E} (S_u+S_v)^27
//     S-Heptacosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53), NHOCTATC=Σ(S+S)²⁷ (topo54).
//     NHOCTATC = |E|·(2S)^27 = 134217728|E|·S^27 for S-regular.
//     Overflow per edge: (2×16129)^27 → saturating u128 accumulator.
//
//   NYSO(G) = Σ_{uv∈E} (S_u²+S_v²)^22
//     S-Tetratetracontyl Sombor: generalised Sombor SO^α with α=44 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44).
//     (W skipped: NWSO already used for S-Weighted Sombor in topo32.)
//     NYSO = |E|·(2S²)^22 = 4194304|E|·S^44 for S-regular.
//     Overflow per edge: (2×16129²)^22 → saturating u128 accumulator.
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
//  Graph     NOCTATC(exact)               NHOCTATC(exact)               NYSO(exact)              edges  nodes
//  Empty                  0                             0                         0               0      0
//  1 node                 0                             0                         0               0      1
//  K₂                     2                   134_217_728                 4_194_304               1      2
//  P₃             805_306_368        36_028_797_018_963_968           u64::MAX(sat.)              2      3
//  K₃       216_172_782_113_783_808      u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4}  360_287_970_189_639_680      u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄        45_754_121_780_834          u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated             0                             0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTATC:  1^28 + 1^28 = 2. ✓
//     NHOCTATC: (1+1)^27 = 2^27 = 134_217_728. ✓
//     NYSO:     (1²+1²)^22 = 2^22 = 4_194_304. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTATC:  3×2^28 = 3×268_435_456 = 805_306_368. ✓
//     NHOCTATC: 2×(2+2)^27 = 2×4^27 = 2×2^54 = 2^55 = 36_028_797_018_963_968. ✓
//       (4^27=2^54=18_014_398_509_481_984; 2×4^27=36_028_797_018_963_968)
//     NYSO:     2×(4+4)^22 = 2×8^22 = 2×2^66 → SATURATES (8^22=2^66>u64::MAX per-edge). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTATC:  3×4^28 = 3×2^56 = 3×72_057_594_037_927_936 = 216_172_782_113_783_808 (fits u64). ✓
//       (4^28=2^56=72_057_594_037_927_936; 3×2^56=216_172_782_113_783_808 < 2^64)
//     NHOCTATC: 3×(4+4)^27 = 3×8^27 = 3×2^81 → SATURATES (per-edge >> u64::MAX). ✓
//     NYSO:     3×(16+16)^22 = 3×32^22 = 3×2^110 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTATC:  5×4^28 = 5×72_057_594_037_927_936 = 360_287_970_189_639_680 (fits u64). ✓
//     NHOCTATC: 4×8^27 → SATURATES. ✓
//     NYSO:     4×32^22 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTATC:  2^28+3^28+3^28+2^28 = 2×268_435_456+2×22_876_792_454_961.
//       3^28=3^16×3^8×3^4=43_046_721×6_561×81=22_876_792_454_961
//       2×268_435_456+2×22_876_792_454_961=536_870_912+45_753_584_909_922=45_754_121_780_834. ✓
//     NHOCTATC: (2+3)^27+(3+3)^27+(3+2)^27 = 2×5^27+6^27
//       6^27=6^24×6^3; 6^24≈4.74×10^18 (fits u64); 6^27>>u64::MAX per-edge → SATURATES. ✓
//     NYSO:     13^22+18^22+13^22 — 13^22>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTATC:  4×9^28 → 9^28>>u64::MAX per-node → SATURATES. ✓
//     NHOCTATC: → SATURATES. ✓
//     NYSO:     → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTATC:  5×6^28 → 6^28>>u64::MAX per-node → SATURATES. ✓
//     NHOCTATC: → SATURATES. ✓
//     NYSO:     → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTATC  = n·S^28                                         for S-regular ✓
//   NHOCTATC = |E|·(2S)^27 = 134217728|E|·S^27               for S-regular ✓
//   NYSO     = |E|·(2S²)^22 = 4194304|E|·S^44                for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 134_217_728, 4_194_304, 1, 2)
//  4.  Path P₃ = A-B-C                   → (805_306_368, 36_028_797_018_963_968, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (216_172_782_113_783_808, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (360_287_970_189_639_680, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (45_754_121_780_834, u64::MAX, u64::MAX, 3, 4)
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

const T54_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_54");
const T54_EXEC:   ExecutorId = ExecutorId::from_ascii("t54.exec");

const T54_KEY_A: &str = "t54.alpha";
const T54_KEY_B: &str = "t54.beta";
const T54_KEY_C: &str = "t54.gamma";
const T54_KEY_D: &str = "t54.delta";
const T54_KEY_E: &str = "t54.epsilon";

const T54_ID_A: NodeId = derive_node_id(T54_PLUGIN, T54_KEY_A);
const T54_ID_B: NodeId = derive_node_id(T54_PLUGIN, T54_KEY_B);
const T54_ID_C: NodeId = derive_node_id(T54_PLUGIN, T54_KEY_C);
const T54_ID_D: NodeId = derive_node_id(T54_PLUGIN, T54_KEY_D);
const T54_ID_E: NodeId = derive_node_id(T54_PLUGIN, T54_KEY_E);

// L4=141 namespace for this harness.
const T54_VEC_A: VectorAddress = VectorAddress::new(141, 1, 1, 0);
const T54_VEC_B: VectorAddress = VectorAddress::new(141, 1, 2, 0);
const T54_VEC_C: VectorAddress = VectorAddress::new(141, 1, 3, 0);
const T54_VEC_D: VectorAddress = VectorAddress::new(141, 2, 1, 0);
const T54_VEC_E: VectorAddress = VectorAddress::new(141, 2, 2, 0);

const T54_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T54_PLUGIN,
    name:         "kl-graph-topo54-harness",
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
        executor_id:       T54_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T54_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T54_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       0, "empty: node_count=0");
    assert_eq!(ec,       0, "empty: edge_count=0");
    assert_eq!(noctatc,  0, "empty: NOCTATC=0");
    assert_eq!(nhoctatc, 0, "empty: NHOCTATC=0");
    assert_eq!(nyso,     0, "empty: NYSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NOCTATC: 0^28=0; NHOCTATC: no edges; NYSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       1, "single: node_count=1");
    assert_eq!(ec,       0, "single: no edges");
    assert_eq!(noctatc,  0, "single: NOCTATC=0 (S=0; 0^28=0)");
    assert_eq!(nhoctatc, 0, "single: NHOCTATC=0 (no edges)");
    assert_eq!(nyso,     0, "single: NYSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NOCTATC:  1^28+1^28 = 2.
// NHOCTATC: (1+1)^27 = 2^27 = 134_217_728.
// NYSO:     (1²+1²)^22 = 2^22 = 4_194_304.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);
    add_edge(T54_ID_A, T54_ID_B, "t54.e.ab");

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       2,           "k2: node_count=2");
    assert_eq!(ec,       1,           "k2: edge_count=1");
    assert_eq!(noctatc,  2,           "k2: NOCTATC=2 (1\u{00b2}\u{2078}+1\u{00b2}\u{2078}=2; S-uniform S=1)");
    assert_eq!(nhoctatc, 134_217_728, "k2: NHOCTATC=134_217_728 ((1+1)\u{00b2}\u{2077}=2\u{00b2}\u{2077}=134_217_728; S-uniform S=1)");
    assert_eq!(nyso,     4_194_304,   "k2: NYSO=4_194_304 ((1\u{00b2}+1\u{00b2})\u{00b2}\u{00b2}=2\u{00b2}\u{00b2}=4_194_304; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NOCTATC:  3×2^28 = 3×268_435_456 = 805_306_368.
// NHOCTATC: 2×(2+2)^27 = 2×4^27 = 2×18_014_398_509_481_984 = 36_028_797_018_963_968.
// NYSO:     2×(4+4)^22 = 2×8^22 = 2×2^66 → SATURATES (8^22=2^66>u64::MAX per-edge).

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);
    add_node(T54_VEC_C, T54_KEY_C, T54_ID_C);
    add_edge(T54_ID_A, T54_ID_B, "t54.e.ab");
    add_edge(T54_ID_B, T54_ID_C, "t54.e.bc");

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       3,                        "p3: node_count=3");
    assert_eq!(ec,       2,                        "p3: edge_count=2");
    assert_eq!(noctatc,  805_306_368,              "p3: NOCTATC=805_306_368 (3\u{00d7}268_435_456; 2\u{00b2}\u{2078}=268_435_456; S-uniform S=2)");
    assert_eq!(nhoctatc, 36_028_797_018_963_968,   "p3: NHOCTATC=36_028_797_018_963_968 (2\u{00d7}18_014_398_509_481_984; (2+2)\u{00b2}\u{2077}=4\u{00b2}\u{2077}=4\u{00d7}4\u{00b2}\u{2076}=18_014_398_509_481_984; S-uniform S=2)");
    assert_eq!(nyso,     u64::MAX,                 "p3: NYSO=u64::MAX (2\u{00d7}8\u{00b2}\u{00b2}=2\u{00d7}2\u{2076}\u{2076}=2\u{2076}\u{2077} > u64::MAX; per-edge 8\u{00b2}\u{00b2}=2\u{2076}\u{2076}>u64::MAX; saturated)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NOCTATC:  3×4^28 = 3×2^56 = 216_172_782_113_783_808 (fits u64).
// NHOCTATC: 3×(4+4)^27 = 3×8^27 = 3×2^81 → SATURATES.
// NYSO:     3×(16+16)^22 = 3×32^22 = 3×2^110 → SATURATES.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);
    add_node(T54_VEC_C, T54_KEY_C, T54_ID_C);
    add_edge(T54_ID_A, T54_ID_B, "t54.e.ab");
    add_edge(T54_ID_B, T54_ID_A, "t54.e.ba");
    add_edge(T54_ID_B, T54_ID_C, "t54.e.bc");
    add_edge(T54_ID_C, T54_ID_B, "t54.e.cb");
    add_edge(T54_ID_A, T54_ID_C, "t54.e.ac");
    add_edge(T54_ID_C, T54_ID_A, "t54.e.ca");

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       3,                            "k3: node_count=3");
    assert_eq!(ec,       3,                            "k3: edge_count=3");
    assert_eq!(noctatc,  216_172_782_113_783_808,      "k3: NOCTATC=216_172_782_113_783_808 (3\u{00d7}72_057_594_037_927_936; 4\u{00b2}\u{2078}=2\u{2075}\u{2076}=72_057_594_037_927_936; S-uniform S=4)");
    assert_eq!(nhoctatc, u64::MAX,                     "k3: NHOCTATC=u64::MAX (3\u{00d7}8\u{00b2}\u{2077}=3\u{00d7}2\u{2078}\u{00b9} >> u64::MAX; per-edge saturates)");
    assert_eq!(nyso,     u64::MAX,                     "k3: NYSO=u64::MAX (3\u{00d7}32\u{00b2}\u{00b2}=3\u{00d7}2\u{00b9}\u{00b9}\u{2070} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NOCTATC:  5×4^28 = 5×72_057_594_037_927_936 = 360_287_970_189_639_680 (fits u64).
// NHOCTATC: 4×8^27 → SATURATES.
// NYSO:     4×32^22 → SATURATES.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);
    add_node(T54_VEC_C, T54_KEY_C, T54_ID_C);
    add_node(T54_VEC_D, T54_KEY_D, T54_ID_D);
    add_node(T54_VEC_E, T54_KEY_E, T54_ID_E);
    add_edge(T54_ID_A, T54_ID_B, "t54.e.ab");
    add_edge(T54_ID_A, T54_ID_C, "t54.e.ac");
    add_edge(T54_ID_A, T54_ID_D, "t54.e.ad");
    add_edge(T54_ID_A, T54_ID_E, "t54.e.ae");

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       5,                            "star: node_count=5");
    assert_eq!(ec,       4,                            "star: edge_count=4");
    assert_eq!(noctatc,  360_287_970_189_639_680,      "star: NOCTATC=360_287_970_189_639_680 (5\u{00d7}72_057_594_037_927_936; same S as K\u{2083})");
    assert_eq!(nhoctatc, u64::MAX,                     "star: NHOCTATC=u64::MAX (4\u{00d7}8\u{00b2}\u{2077} >> u64::MAX; per-edge saturates)");
    assert_eq!(nyso,     u64::MAX,                     "star: NYSO=u64::MAX (4\u{00d7}32\u{00b2}\u{00b2} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NOCTATC:  2^28+3^28+3^28+2^28 = 2×268_435_456+2×22_876_792_454_961 = 45_754_121_780_834.
//   (3^28=3^16×3^8×3^4=43_046_721×6_561×81=22_876_792_454_961)
// NHOCTATC: 5^27+6^27+5^27 — 6^27>>u64::MAX per-edge → SATURATES.
// NYSO:     13^22+18^22+13^22 — 13^22>>u64::MAX per-edge → SATURATES.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);
    add_node(T54_VEC_C, T54_KEY_C, T54_ID_C);
    add_node(T54_VEC_D, T54_KEY_D, T54_ID_D);
    add_edge(T54_ID_A, T54_ID_B, "t54.e.ab");
    add_edge(T54_ID_B, T54_ID_C, "t54.e.bc");
    add_edge(T54_ID_C, T54_ID_D, "t54.e.cd");

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       4,                    "p4: node_count=4");
    assert_eq!(ec,       3,                    "p4: edge_count=3");
    assert_eq!(noctatc,  45_754_121_780_834,   "p4: NOCTATC=45_754_121_780_834 (2\u{00d7}268_435_456+2\u{00d7}22_876_792_454_961; 2\u{00b2}\u{2078}+3\u{00b2}\u{2078}+3\u{00b2}\u{2078}+2\u{00b2}\u{2078})");
    assert_eq!(nhoctatc, u64::MAX,             "p4: NHOCTATC=u64::MAX (6\u{00b2}\u{2077}>>u64::MAX per-edge; saturated)");
    assert_eq!(nyso,     u64::MAX,             "p4: NYSO=u64::MAX (13\u{00b2}\u{00b2}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NOCTATC:  4×9^28 → SATURATES → u64::MAX.
// NHOCTATC: 6×18^27 → SATURATES → u64::MAX.
// NYSO:     6×162^22 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);
    add_node(T54_VEC_C, T54_KEY_C, T54_ID_C);
    add_node(T54_VEC_D, T54_KEY_D, T54_ID_D);
    add_edge(T54_ID_A, T54_ID_B, "t54.e.ab");
    add_edge(T54_ID_B, T54_ID_A, "t54.e.ba");
    add_edge(T54_ID_A, T54_ID_C, "t54.e.ac");
    add_edge(T54_ID_C, T54_ID_A, "t54.e.ca");
    add_edge(T54_ID_A, T54_ID_D, "t54.e.ad");
    add_edge(T54_ID_D, T54_ID_A, "t54.e.da");
    add_edge(T54_ID_B, T54_ID_C, "t54.e.bc");
    add_edge(T54_ID_C, T54_ID_B, "t54.e.cb");
    add_edge(T54_ID_B, T54_ID_D, "t54.e.bd");
    add_edge(T54_ID_D, T54_ID_B, "t54.e.db");
    add_edge(T54_ID_C, T54_ID_D, "t54.e.cd");
    add_edge(T54_ID_D, T54_ID_C, "t54.e.dc");

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       4,        "k4: node_count=4");
    assert_eq!(ec,       6,        "k4: edge_count=6");
    assert_eq!(noctatc,  u64::MAX, "k4: NOCTATC=u64::MAX (4\u{00d7}9\u{00b2}\u{2078} >> u64::MAX; saturated)");
    assert_eq!(nhoctatc, u64::MAX, "k4: NHOCTATC=u64::MAX (6\u{00d7}18\u{00b2}\u{2077} >> u64::MAX; saturated)");
    assert_eq!(nyso,     u64::MAX, "k4: NYSO=u64::MAX (6\u{00d7}162\u{00b2}\u{00b2} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NOCTATC=0; NHOCTATC=0; NYSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       2, "two-iso: node_count=2");
    assert_eq!(ec,       0, "two-iso: edge_count=0");
    assert_eq!(noctatc,  0, "two-iso: NOCTATC=0");
    assert_eq!(nhoctatc, 0, "two-iso: NHOCTATC=0");
    assert_eq!(nyso,     0, "two-iso: NYSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NOCTATC:  5×6^28 → 6^28>>u64::MAX per-node → SATURATES.
// NHOCTATC: 6×12^27 → SATURATES (12^27>>u64::MAX per-edge).
// NYSO:     6×72^22 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T54_VEC_A, T54_KEY_A, T54_ID_A);
    add_node(T54_VEC_B, T54_KEY_B, T54_ID_B);
    add_node(T54_VEC_C, T54_KEY_C, T54_ID_C);
    add_node(T54_VEC_D, T54_KEY_D, T54_ID_D);
    add_node(T54_VEC_E, T54_KEY_E, T54_ID_E);
    add_edge(T54_ID_A, T54_ID_C, "t54.e.ac");
    add_edge(T54_ID_A, T54_ID_D, "t54.e.ad");
    add_edge(T54_ID_A, T54_ID_E, "t54.e.ae");
    add_edge(T54_ID_B, T54_ID_C, "t54.e.bc");
    add_edge(T54_ID_B, T54_ID_D, "t54.e.bd");
    add_edge(T54_ID_B, T54_ID_E, "t54.e.be");

    let (noctatc, nhoctatc, nyso, ec, nc) = gos_runtime::graph_topo_indices54();
    assert_eq!(nc,       5,        "k23: node_count=5");
    assert_eq!(ec,       6,        "k23: edge_count=6");
    assert_eq!(noctatc,  u64::MAX, "k23: NOCTATC=u64::MAX (5\u{00d7}6\u{00b2}\u{2078}; 6\u{00b2}\u{2078}>>u64::MAX per-node; saturated)");
    assert_eq!(nhoctatc, u64::MAX, "k23: NHOCTATC=u64::MAX (6\u{00d7}12\u{00b2}\u{2077} >> u64::MAX; per-edge saturates)");
    assert_eq!(nyso,     u64::MAX, "k23: NYSO=u64::MAX (6\u{00d7}72\u{00b2}\u{00b2} >> u64::MAX; per-edge saturates)");
}
