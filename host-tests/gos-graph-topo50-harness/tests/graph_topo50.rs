// gos-graph-topo50-harness — V3.61 NTETRTC + NHTETRTC + NSSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices50()`:
//   Returns (ntetrtc, nhtetrtc, nsso, edge_count, node_count)
//   - ntetrtc  = NTETRTC(G)  = Σ_v S(v)^24                  (exact u64; S-Tetracosic vertex sum)
//   - nhtetrtc = NHTETRTC(G) = Σ_{uv∈E} (S_u+S_v)^23        (exact u64; S-Tricosic edge-sum)
//   - nsso     = NSSO(G)     = Σ_{uv∈E} (S_u²+S_v²)^18      (exact u64; S-Hexatriacontyl Sombor, α=36)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTETRTC(G) = Σ_v S(v)^24
//     S-Tetracosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50).
//     NTETRTC = n·S^24 for S-regular.
//     Overflow: S^24 ≤ 16129^24 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHTETRTC(G) = Σ_{uv∈E} (S_u+S_v)^23
//     S-Tricosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50).
//     NHTETRTC = |E|·(2S)^23 = 8388608|E|·S^23 for S-regular.
//     Overflow per edge: (2×16129)^23 → saturating u128 accumulator.
//
//   NSSO(G) = Σ_{uv∈E} (S_u²+S_v²)^18
//     S-Hexatriacontyl Sombor: generalised Sombor SO^α with α=36 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36).
//     (S follows R in alphabetical sequence of middle letters)
//     NSSO = |E|·(2S²)^18 = 262144|E|·S^36 for S-regular.
//     Overflow per edge: (2×16129²)^18 → saturating u128 accumulator.
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
//  Graph     NTETRTC(exact)              NHTETRTC(exact)               NSSO(exact)              edges  nodes
//  Empty                  0                            0                         0               0      0
//  1 node                 0                            0                         0               0      1
//  K₂                     2                    8_388_608                   262_144               1      2
//  P₃              50_331_648          140_737_488_355_328    36_028_797_018_963_968              2      3
//  K₃         844_424_930_131_968       u64::MAX(sat.)              u64::MAX(sat.)               3      3
//  K_{1,4}  1_407_374_883_553_280       u64::MAX(sat.)              u64::MAX(sat.)               4      5
//  P₄            564_892_627_394    813_572_080_963_759_066         u64::MAX(sat.)               3      4
//  K₄          u64::MAX(sat.)          u64::MAX(sat.)               u64::MAX(sat.)               6      4
//  2 isolated             0                            0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)           u64::MAX(sat.)               u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTETRTC:  1^24 + 1^24 = 2. ✓
//     NHTETRTC: (1+1)^23 = 2^23 = 8_388_608. ✓
//     NSSO:     (1²+1²)^18 = 2^18 = 262_144. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTETRTC:  3×2^24 = 3×16_777_216 = 50_331_648. ✓
//     NHTETRTC: 2×(2+2)^23 = 2×4^23 = 2×70_368_744_177_664 = 140_737_488_355_328. ✓
//       (4^10=1_048_576; 4^20=1_048_576^2=1_099_511_627_776; 4^23=4^20×4^3=1_099_511_627_776×64=70_368_744_177_664)
//     NSSO:     2×(4+4)^18 = 2×8^18.
//       8^16=281_474_976_710_656; 8^18=8^16×8^2=281_474_976_710_656×64=18_014_398_509_481_984
//       2×18_014_398_509_481_984=36_028_797_018_963_968 (fits u64). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTETRTC:  3×4^24 = 3×281_474_976_710_656 = 844_424_930_131_968 (fits u64). ✓
//       (4^12=16_777_216; 4^24=16_777_216^2=281_474_976_710_656)
//     NHTETRTC: 3×(4+4)^23 = 3×8^23 → SATURATES.
//       (8^20=1_152_921_504_606_846_976; 8^23=8^20×8^3>>u64::MAX per-edge). ✓
//     NSSO:     3×(16+16)^18 = 3×32^18 → SATURATES (32^18=2^90>>u64::MAX per-edge). ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTETRTC:  5×4^24 = 5×281_474_976_710_656 = 1_407_374_883_553_280 (fits u64). ✓
//     NHTETRTC: 4×8^23 → SATURATES. ✓
//     NSSO:     4×32^18 → SATURATES (per-edge >> u64::MAX). ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTETRTC:  2^24+3^24+3^24+2^24 = 2×16_777_216+2×282_429_536_481 = 564_892_627_394. ✓
//       (3^12=531_441; 3^24=531_441^2=282_429_536_481)
//     NHTETRTC: 5^23+6^23+5^23
//       5^22=2_384_185_791_015_625; 5^23=5×2_384_185_791_015_625=11_920_928_955_078_125
//       6^22=131_621_703_842_267_136; 6^23=6×131_621_703_842_267_136=789_730_223_053_602_816
//       2×11_920_928_955_078_125+789_730_223_053_602_816=813_572_080_963_759_066 (fits u64). ✓
//     NSSO: 13^18+18^18+13^18
//       13^16=665_416_609_183_179_841; 13^18=13^16×169 >> u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTETRTC:  4×9^24 → SATURATES (9^12=282_429_536_481; 9^24=282_429_536_481^2>>u64::MAX per-vertex). ✓
//     NHTETRTC: 6×18^23 → SATURATES. ✓
//     NSSO:     6×162^18 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTETRTC:  5×6^24 = 5×4_738_381_338_321_616_896 = 23_691_906_691_608_084_480 > u64::MAX → SATURATES. ✓
//       (6^12=2_176_782_336; 6^24=2_176_782_336^2=4_738_381_338_321_616_896; 5× overflows)
//     NHTETRTC: 6×12^23 → SATURATES (12^22>>u64::MAX per-edge). ✓
//     NSSO:     6×72^18 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTETRTC  = n·S^24                                    for S-regular ✓
//   NHTETRTC = |E|·(2S)^23 = 8388608|E|·S^23            for S-regular ✓
//   NSSO     = |E|·(2S²)^18 = 262144|E|·S^36            for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 8_388_608, 262_144, 1, 2)
//  4.  Path P₃ = A-B-C                   → (50_331_648, 140_737_488_355_328, 36_028_797_018_963_968, 2, 3)
//  5.  Triangle K₃                       → (844_424_930_131_968, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (1_407_374_883_553_280, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (564_892_627_394, 813_572_080_963_759_066, u64::MAX, 3, 4)
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

const T50_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_50");
const T50_EXEC:   ExecutorId = ExecutorId::from_ascii("t50.exec");

const T50_KEY_A: &str = "t50.alpha";
const T50_KEY_B: &str = "t50.beta";
const T50_KEY_C: &str = "t50.gamma";
const T50_KEY_D: &str = "t50.delta";
const T50_KEY_E: &str = "t50.epsilon";

const T50_ID_A: NodeId = derive_node_id(T50_PLUGIN, T50_KEY_A);
const T50_ID_B: NodeId = derive_node_id(T50_PLUGIN, T50_KEY_B);
const T50_ID_C: NodeId = derive_node_id(T50_PLUGIN, T50_KEY_C);
const T50_ID_D: NodeId = derive_node_id(T50_PLUGIN, T50_KEY_D);
const T50_ID_E: NodeId = derive_node_id(T50_PLUGIN, T50_KEY_E);

// L4=137 namespace for this harness.
const T50_VEC_A: VectorAddress = VectorAddress::new(137, 1, 1, 0);
const T50_VEC_B: VectorAddress = VectorAddress::new(137, 1, 2, 0);
const T50_VEC_C: VectorAddress = VectorAddress::new(137, 1, 3, 0);
const T50_VEC_D: VectorAddress = VectorAddress::new(137, 2, 1, 0);
const T50_VEC_E: VectorAddress = VectorAddress::new(137, 2, 2, 0);

const T50_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T50_PLUGIN,
    name:         "kl-graph-topo50-harness",
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
        executor_id:       T50_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T50_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T50_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       0, "empty: node_count=0");
    assert_eq!(ec,       0, "empty: edge_count=0");
    assert_eq!(ntetrtc,  0, "empty: NTETRTC=0");
    assert_eq!(nhtetrtc, 0, "empty: NHTETRTC=0");
    assert_eq!(nsso,     0, "empty: NSSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NTETRTC: 0^24=0; NHTETRTC: no edges; NSSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       1, "single: node_count=1");
    assert_eq!(ec,       0, "single: no edges");
    assert_eq!(ntetrtc,  0, "single: NTETRTC=0 (S=0; 0^24=0)");
    assert_eq!(nhtetrtc, 0, "single: NHTETRTC=0 (no edges)");
    assert_eq!(nsso,     0, "single: NSSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NTETRTC:  1^24+1^24 = 2.
// NHTETRTC: (1+1)^23 = 2^23 = 8_388_608.
// NSSO:     (1²+1²)^18 = 2^18 = 262_144.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);
    add_edge(T50_ID_A, T50_ID_B, "t50.e.ab");

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       2,         "k2: node_count=2");
    assert_eq!(ec,       1,         "k2: edge_count=1");
    assert_eq!(ntetrtc,  2,         "k2: NTETRTC=2 (1\u{00b2}\u{2074}+1\u{00b2}\u{2074}=2; S-uniform S=1)");
    assert_eq!(nhtetrtc, 8_388_608, "k2: NHTETRTC=8_388_608 ((1+1)\u{00b2}\u{00b3}=2\u{00b2}\u{00b3}=8_388_608; S-uniform S=1)");
    assert_eq!(nsso,     262_144,   "k2: NSSO=262_144 ((1\u{00b2}+1\u{00b2})\u{00b9}\u{2078}=2\u{00b9}\u{2078}=262_144; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NTETRTC:  3×2^24 = 3×16_777_216 = 50_331_648.
// NHTETRTC: 2×(2+2)^23 = 2×4^23 = 2×70_368_744_177_664 = 140_737_488_355_328.
// NSSO:     2×(4+4)^18 = 2×8^18 = 2×18_014_398_509_481_984 = 36_028_797_018_963_968.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);
    add_node(T50_VEC_C, T50_KEY_C, T50_ID_C);
    add_edge(T50_ID_A, T50_ID_B, "t50.e.ab");
    add_edge(T50_ID_B, T50_ID_C, "t50.e.bc");

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       3,                      "p3: node_count=3");
    assert_eq!(ec,       2,                      "p3: edge_count=2");
    assert_eq!(ntetrtc,  50_331_648,             "p3: NTETRTC=50_331_648 (3\u{00d7}16_777_216; 2\u{00b2}\u{2074}=16_777_216; S-uniform S=2)");
    assert_eq!(nhtetrtc, 140_737_488_355_328,    "p3: NHTETRTC=140_737_488_355_328 (2\u{00d7}70_368_744_177_664; (2+2)\u{00b2}\u{00b3}=4\u{00b2}\u{00b3}=70_368_744_177_664; S-uniform S=2)");
    assert_eq!(nsso,     36_028_797_018_963_968, "p3: NSSO=36_028_797_018_963_968 (2\u{00d7}18_014_398_509_481_984; (4+4)\u{00b9}\u{2078}=8\u{00b9}\u{2078}=18_014_398_509_481_984; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NTETRTC:  3×4^24 = 3×281_474_976_710_656 = 844_424_930_131_968 (fits u64).
// NHTETRTC: 3×(4+4)^23 = 3×8^23 → SATURATES (8^20=1_152_921_504_606_846_976; 8^23>>u64::MAX).
// NSSO:     3×(16+16)^18 = 3×32^18 → SATURATES (32^18=2^90>>u64::MAX per-edge).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);
    add_node(T50_VEC_C, T50_KEY_C, T50_ID_C);
    add_edge(T50_ID_A, T50_ID_B, "t50.e.ab");
    add_edge(T50_ID_B, T50_ID_A, "t50.e.ba");
    add_edge(T50_ID_B, T50_ID_C, "t50.e.bc");
    add_edge(T50_ID_C, T50_ID_B, "t50.e.cb");
    add_edge(T50_ID_A, T50_ID_C, "t50.e.ac");
    add_edge(T50_ID_C, T50_ID_A, "t50.e.ca");

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       3,                    "k3: node_count=3");
    assert_eq!(ec,       3,                    "k3: edge_count=3");
    assert_eq!(ntetrtc,  844_424_930_131_968,  "k3: NTETRTC=844_424_930_131_968 (3\u{00d7}281_474_976_710_656; 4\u{00b2}\u{2074}=281_474_976_710_656; S-uniform S=4)");
    assert_eq!(nhtetrtc, u64::MAX,             "k3: NHTETRTC=u64::MAX (3\u{00d7}8\u{00b2}\u{00b3} >> u64::MAX; saturated)");
    assert_eq!(nsso,     u64::MAX,             "k3: NSSO=u64::MAX (3\u{00d7}32\u{00b9}\u{2078}=3\u{00d7}2\u{2079}\u{2070} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NTETRTC:  5×4^24 = 5×281_474_976_710_656 = 1_407_374_883_553_280 (fits u64).
// NHTETRTC: 4×8^23 → SATURATES.
// NSSO:     4×32^18 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);
    add_node(T50_VEC_C, T50_KEY_C, T50_ID_C);
    add_node(T50_VEC_D, T50_KEY_D, T50_ID_D);
    add_node(T50_VEC_E, T50_KEY_E, T50_ID_E);
    add_edge(T50_ID_A, T50_ID_B, "t50.e.ab");
    add_edge(T50_ID_A, T50_ID_C, "t50.e.ac");
    add_edge(T50_ID_A, T50_ID_D, "t50.e.ad");
    add_edge(T50_ID_A, T50_ID_E, "t50.e.ae");

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       5,                     "star: node_count=5");
    assert_eq!(ec,       4,                     "star: edge_count=4");
    assert_eq!(ntetrtc,  1_407_374_883_553_280, "star: NTETRTC=1_407_374_883_553_280 (5\u{00d7}281_474_976_710_656; same S as K\u{2083})");
    assert_eq!(nhtetrtc, u64::MAX,              "star: NHTETRTC=u64::MAX (4\u{00d7}8\u{00b2}\u{00b3} >> u64::MAX; saturated)");
    assert_eq!(nsso,     u64::MAX,              "star: NSSO=u64::MAX (4\u{00d7}32\u{00b9}\u{2078} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NTETRTC:  2^24+3^24+3^24+2^24 = 2×16_777_216+2×282_429_536_481 = 564_892_627_394.
// NHTETRTC: 5^23+6^23+5^23 = 2×11_920_928_955_078_125+789_730_223_053_602_816
//           = 813_572_080_963_759_066 (fits u64).
// NSSO:     13^18+18^18+13^18 → SATURATES (13^16×169 >> u64::MAX per-edge).

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);
    add_node(T50_VEC_C, T50_KEY_C, T50_ID_C);
    add_node(T50_VEC_D, T50_KEY_D, T50_ID_D);
    add_edge(T50_ID_A, T50_ID_B, "t50.e.ab");
    add_edge(T50_ID_B, T50_ID_C, "t50.e.bc");
    add_edge(T50_ID_C, T50_ID_D, "t50.e.cd");

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       4,                          "p4: node_count=4");
    assert_eq!(ec,       3,                          "p4: edge_count=3");
    assert_eq!(ntetrtc,  564_892_627_394,            "p4: NTETRTC=564_892_627_394 (2\u{00d7}16_777_216+2\u{00d7}282_429_536_481; 2\u{00b2}\u{2074}+3\u{00b2}\u{2074}+3\u{00b2}\u{2074}+2\u{00b2}\u{2074})");
    assert_eq!(nhtetrtc, 813_572_080_963_759_066,    "p4: NHTETRTC=813_572_080_963_759_066 (2\u{00d7}11_920_928_955_078_125+789_730_223_053_602_816; 5\u{00b2}\u{00b3}+6\u{00b2}\u{00b3}+5\u{00b2}\u{00b3})");
    assert_eq!(nsso,     u64::MAX,                   "p4: NSSO=u64::MAX (13\u{00b9}\u{2078}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NTETRTC:  4×9^24 → SATURATES → u64::MAX.
// NHTETRTC: 6×18^23 → SATURATES → u64::MAX.
// NSSO:     6×162^18 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);
    add_node(T50_VEC_C, T50_KEY_C, T50_ID_C);
    add_node(T50_VEC_D, T50_KEY_D, T50_ID_D);
    add_edge(T50_ID_A, T50_ID_B, "t50.e.ab");
    add_edge(T50_ID_B, T50_ID_A, "t50.e.ba");
    add_edge(T50_ID_A, T50_ID_C, "t50.e.ac");
    add_edge(T50_ID_C, T50_ID_A, "t50.e.ca");
    add_edge(T50_ID_A, T50_ID_D, "t50.e.ad");
    add_edge(T50_ID_D, T50_ID_A, "t50.e.da");
    add_edge(T50_ID_B, T50_ID_C, "t50.e.bc");
    add_edge(T50_ID_C, T50_ID_B, "t50.e.cb");
    add_edge(T50_ID_B, T50_ID_D, "t50.e.bd");
    add_edge(T50_ID_D, T50_ID_B, "t50.e.db");
    add_edge(T50_ID_C, T50_ID_D, "t50.e.cd");
    add_edge(T50_ID_D, T50_ID_C, "t50.e.dc");

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       4,        "k4: node_count=4");
    assert_eq!(ec,       6,        "k4: edge_count=6");
    assert_eq!(ntetrtc,  u64::MAX, "k4: NTETRTC=u64::MAX (4\u{00d7}9\u{00b2}\u{2074} >> u64::MAX; saturated)");
    assert_eq!(nhtetrtc, u64::MAX, "k4: NHTETRTC=u64::MAX (6\u{00d7}18\u{00b2}\u{00b3} >> u64::MAX; saturated)");
    assert_eq!(nsso,     u64::MAX, "k4: NSSO=u64::MAX (6\u{00d7}162\u{00b9}\u{2078} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NTETRTC=0; NHTETRTC=0; NSSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       2, "two-iso: node_count=2");
    assert_eq!(ec,       0, "two-iso: edge_count=0");
    assert_eq!(ntetrtc,  0, "two-iso: NTETRTC=0");
    assert_eq!(nhtetrtc, 0, "two-iso: NHTETRTC=0");
    assert_eq!(nsso,     0, "two-iso: NSSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NTETRTC:  5×6^24 = 5×4_738_381_338_321_616_896 = 23_691_906_691_608_084_480 > u64::MAX → SATURATES.
// NHTETRTC: 6×12^23 → SATURATES (12^22>>u64::MAX per-edge).
// NSSO:     6×72^18 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T50_VEC_A, T50_KEY_A, T50_ID_A);
    add_node(T50_VEC_B, T50_KEY_B, T50_ID_B);
    add_node(T50_VEC_C, T50_KEY_C, T50_ID_C);
    add_node(T50_VEC_D, T50_KEY_D, T50_ID_D);
    add_node(T50_VEC_E, T50_KEY_E, T50_ID_E);
    add_edge(T50_ID_A, T50_ID_C, "t50.e.ac");
    add_edge(T50_ID_A, T50_ID_D, "t50.e.ad");
    add_edge(T50_ID_A, T50_ID_E, "t50.e.ae");
    add_edge(T50_ID_B, T50_ID_C, "t50.e.bc");
    add_edge(T50_ID_B, T50_ID_D, "t50.e.bd");
    add_edge(T50_ID_B, T50_ID_E, "t50.e.be");

    let (ntetrtc, nhtetrtc, nsso, ec, nc) = gos_runtime::graph_topo_indices50();
    assert_eq!(nc,       5,        "k23: node_count=5");
    assert_eq!(ec,       6,        "k23: edge_count=6");
    assert_eq!(ntetrtc,  u64::MAX, "k23: NTETRTC=u64::MAX (5\u{00d7}6\u{00b2}\u{2074}=5\u{00d7}4_738_381_338_321_616_896=23_691_906_691_608_084_480>u64::MAX; saturated)");
    assert_eq!(nhtetrtc, u64::MAX, "k23: NHTETRTC=u64::MAX (6\u{00d7}12\u{00b2}\u{00b3} >> u64::MAX; per-edge saturates)");
    assert_eq!(nsso,     u64::MAX, "k23: NSSO=u64::MAX (6\u{00d7}72\u{00b9}\u{2078} >> u64::MAX; per-edge saturates)");
}
