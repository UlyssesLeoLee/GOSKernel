// gos-graph-topo55-harness — V3.66 NNONATC + NHNONATC + NZSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices55()`:
//   Returns (nnonatc, nhnonatc, nzso, edge_count, node_count)
//   - nnonatc  = NNONATC(G)  = Σ_v S(v)^29                   (exact u64; S-Nonacosic vertex sum)
//   - nhnonatc = NHNONATC(G) = Σ_{uv∈E} (S_u+S_v)^28         (exact u64; S-Octacosic edge-sum)
//   - nzso     = NZSO(G)     = Σ_{uv∈E} (S_u²+S_v²)^23       (exact u64; S-Hexatetracontyl Sombor, α=46)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NNONATC(G) = Σ_v S(v)^29
//     S-Nonacosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52), NHEPTATC=Σ S²⁷ (topo53), NOCTATC=Σ S²⁸ (topo54),
//       NNONATC=Σ S²⁹ (topo55).
//     NNONATC = n·S^29 for S-regular.
//     Overflow: S^29 ≤ 16129^29 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHNONATC(G) = Σ_{uv∈E} (S_u+S_v)^28
//     S-Octacosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52),
//       NHHEPTATC=Σ(S+S)²⁶ (topo53), NHOCTATC=Σ(S+S)²⁷ (topo54), NHNONATC=Σ(S+S)²⁸ (topo55).
//     NHNONATC = |E|·(2S)^28 = 268435456|E|·S^28 for S-regular.
//     Overflow per edge: (2×16129)^28 → saturating u128 accumulator.
//
//   NZSO(G) = Σ_{uv∈E} (S_u²+S_v²)^23
//     S-Hexatetracontyl Sombor: generalised Sombor SO^α with α=46 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40), NXSO(topo53,α=42), NYSO(topo54,α=44), NZSO(topo55,α=46).
//     NZSO = |E|·(2S²)^23 = 8388608|E|·S^46 for S-regular.
//     Overflow per edge: (2×16129²)^23 → saturating u128 accumulator.
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
//  Graph     NNONATC(exact)                NHNONATC(exact)               NZSO(exact)              edges  nodes
//  Empty                   0                             0                         0               0      0
//  1 node                  0                             0                         0               0      1
//  K₂                      2                   268_435_456                 8_388_608               1      2
//  P₃           1_610_612_736       144_115_188_075_855_872           u64::MAX(sat.)              2      3
//  K₃     864_691_128_455_135_232       u64::MAX(sat.)              u64::MAX(sat.)              3      3
//  K_{1,4} 1_441_151_880_758_558_720    u64::MAX(sat.)              u64::MAX(sat.)              4      5
//  P₄       137_261_828_471_590          u64::MAX(sat.)              u64::MAX(sat.)              3      4
//  K₄          u64::MAX(sat.)            u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated              0                             0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)             u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NNONATC:  1^29 + 1^29 = 2. ✓
//     NHNONATC: (1+1)^28 = 2^28 = 268_435_456. ✓
//     NZSO:     (1²+1²)^23 = 2^23 = 8_388_608. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NNONATC:  3×2^29 = 3×536_870_912 = 1_610_612_736. ✓
//     NHNONATC: 2×(2+2)^28 = 2×4^28 = 2×2^56 = 2^57 = 144_115_188_075_855_872. ✓
//       (4^28=2^56=72_057_594_037_927_936; 2×4^28=144_115_188_075_855_872)
//     NZSO:     2×(4+4)^23 = 2×8^23 = 2×2^69 → SATURATES (8^23=2^69>u64::MAX per-edge). ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NNONATC:  3×4^29 = 3×2^58 = 3×288_230_376_151_711_744 = 864_691_128_455_135_232 (fits u64). ✓
//       (4^29=4^28×4=72_057_594_037_927_936×4=288_230_376_151_711_744; 3×2^58=864_691_128_455_135_232 < 2^64)
//     NHNONATC: 3×(4+4)^28 = 3×8^28 = 3×2^84 → SATURATES (per-edge >> u64::MAX). ✓
//     NZSO:     3×(16+16)^23 = 3×32^23 = 3×2^115 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NNONATC:  5×4^29 = 5×288_230_376_151_711_744 = 1_441_151_880_758_558_720 (fits u64). ✓
//     NHNONATC: 4×8^28 → SATURATES. ✓
//     NZSO:     4×32^23 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NNONATC:  2^29+3^29+3^29+2^29 = 2×536_870_912+2×68_630_377_364_883.
//       3^29=3^28×3=22_876_792_454_961×3=68_630_377_364_883
//       2×536_870_912+2×68_630_377_364_883=1_073_741_824+137_260_754_729_766=137_261_828_471_590. ✓
//     NHNONATC: (2+3)^28+(3+3)^28+(3+2)^28 = 2×5^28+6^28
//       5^28>>u64::MAX per-edge → SATURATES. ✓
//     NZSO:     13^23+18^23+13^23 — 13^23>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NNONATC:  4×9^29 → 9^29>>u64::MAX per-node → SATURATES. ✓
//     NHNONATC: → SATURATES. ✓
//     NZSO:     → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NNONATC:  5×6^29 → 6^29>>u64::MAX per-node → SATURATES. ✓
//     NHNONATC: → SATURATES. ✓
//     NZSO:     → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NNONATC  = n·S^29                                         for S-regular ✓
//   NHNONATC = |E|·(2S)^28 = 268435456|E|·S^28               for S-regular ✓
//   NZSO     = |E|·(2S²)^23 = 8388608|E|·S^46                for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 268_435_456, 8_388_608, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_610_612_736, 144_115_188_075_855_872, u64::MAX, 2, 3)
//  5.  Triangle K₃                       → (864_691_128_455_135_232, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (1_441_151_880_758_558_720, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (137_261_828_471_590, u64::MAX, u64::MAX, 3, 4)
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

const T55_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_55");
const T55_EXEC:   ExecutorId = ExecutorId::from_ascii("t55.exec");

const T55_KEY_A: &str = "t55.alpha";
const T55_KEY_B: &str = "t55.beta";
const T55_KEY_C: &str = "t55.gamma";
const T55_KEY_D: &str = "t55.delta";
const T55_KEY_E: &str = "t55.epsilon";

const T55_ID_A: NodeId = derive_node_id(T55_PLUGIN, T55_KEY_A);
const T55_ID_B: NodeId = derive_node_id(T55_PLUGIN, T55_KEY_B);
const T55_ID_C: NodeId = derive_node_id(T55_PLUGIN, T55_KEY_C);
const T55_ID_D: NodeId = derive_node_id(T55_PLUGIN, T55_KEY_D);
const T55_ID_E: NodeId = derive_node_id(T55_PLUGIN, T55_KEY_E);

// L4=142 namespace for this harness.
const T55_VEC_A: VectorAddress = VectorAddress::new(142, 1, 1, 0);
const T55_VEC_B: VectorAddress = VectorAddress::new(142, 1, 2, 0);
const T55_VEC_C: VectorAddress = VectorAddress::new(142, 1, 3, 0);
const T55_VEC_D: VectorAddress = VectorAddress::new(142, 2, 1, 0);
const T55_VEC_E: VectorAddress = VectorAddress::new(142, 2, 2, 0);

const T55_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T55_PLUGIN,
    name:         "kl-graph-topo55-harness",
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
        executor_id:       T55_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T55_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T55_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       0, "empty: node_count=0");
    assert_eq!(ec,       0, "empty: edge_count=0");
    assert_eq!(nnonatc,  0, "empty: NNONATC=0");
    assert_eq!(nhnonatc, 0, "empty: NHNONATC=0");
    assert_eq!(nzso,     0, "empty: NZSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NNONATC: 0^29=0; NHNONATC: no edges; NZSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       1, "single: node_count=1");
    assert_eq!(ec,       0, "single: no edges");
    assert_eq!(nnonatc,  0, "single: NNONATC=0 (S=0; 0^29=0)");
    assert_eq!(nhnonatc, 0, "single: NHNONATC=0 (no edges)");
    assert_eq!(nzso,     0, "single: NZSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NNONATC:  1^29+1^29 = 2.
// NHNONATC: (1+1)^28 = 2^28 = 268_435_456.
// NZSO:     (1²+1²)^23 = 2^23 = 8_388_608.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);
    add_edge(T55_ID_A, T55_ID_B, "t55.e.ab");

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       2,           "k2: node_count=2");
    assert_eq!(ec,       1,           "k2: edge_count=1");
    assert_eq!(nnonatc,  2,           "k2: NNONATC=2 (1\u{00b2}\u{2079}+1\u{00b2}\u{2079}=2; S-uniform S=1)");
    assert_eq!(nhnonatc, 268_435_456, "k2: NHNONATC=268_435_456 ((1+1)\u{00b2}\u{2078}=2\u{00b2}\u{2078}=268_435_456; S-uniform S=1)");
    assert_eq!(nzso,     8_388_608,   "k2: NZSO=8_388_608 ((1\u{00b2}+1\u{00b2})\u{00b2}\u{00b3}=2\u{00b2}\u{00b3}=8_388_608; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NNONATC:  3×2^29 = 3×536_870_912 = 1_610_612_736.
// NHNONATC: 2×(2+2)^28 = 2×4^28 = 2×72_057_594_037_927_936 = 144_115_188_075_855_872.
// NZSO:     2×(4+4)^23 = 2×8^23 = 2×2^69 → SATURATES (8^23=2^69>u64::MAX per-edge).

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);
    add_node(T55_VEC_C, T55_KEY_C, T55_ID_C);
    add_edge(T55_ID_A, T55_ID_B, "t55.e.ab");
    add_edge(T55_ID_B, T55_ID_C, "t55.e.bc");

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       3,                         "p3: node_count=3");
    assert_eq!(ec,       2,                         "p3: edge_count=2");
    assert_eq!(nnonatc,  1_610_612_736,              "p3: NNONATC=1_610_612_736 (3\u{00d7}536_870_912; 2\u{00b2}\u{2079}=536_870_912; S-uniform S=2)");
    assert_eq!(nhnonatc, 144_115_188_075_855_872,    "p3: NHNONATC=144_115_188_075_855_872 (2\u{00d7}72_057_594_037_927_936; (2+2)\u{00b2}\u{2078}=4\u{00b2}\u{2078}=2\u{2075}\u{2076}=72_057_594_037_927_936; S-uniform S=2)");
    assert_eq!(nzso,     u64::MAX,                   "p3: NZSO=u64::MAX (2\u{00d7}8\u{00b2}\u{00b3}=2\u{00d7}2\u{2076}\u{2079}=2\u{2077}\u{2070} > u64::MAX; per-edge 8\u{00b2}\u{00b3}=2\u{2076}\u{2079}>u64::MAX; saturated)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NNONATC:  3×4^29 = 3×2^58 = 864_691_128_455_135_232 (fits u64).
// NHNONATC: 3×(4+4)^28 = 3×8^28 = 3×2^84 → SATURATES.
// NZSO:     3×(16+16)^23 = 3×32^23 = 3×2^115 → SATURATES.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);
    add_node(T55_VEC_C, T55_KEY_C, T55_ID_C);
    add_edge(T55_ID_A, T55_ID_B, "t55.e.ab");
    add_edge(T55_ID_B, T55_ID_A, "t55.e.ba");
    add_edge(T55_ID_B, T55_ID_C, "t55.e.bc");
    add_edge(T55_ID_C, T55_ID_B, "t55.e.cb");
    add_edge(T55_ID_A, T55_ID_C, "t55.e.ac");
    add_edge(T55_ID_C, T55_ID_A, "t55.e.ca");

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       3,                              "k3: node_count=3");
    assert_eq!(ec,       3,                              "k3: edge_count=3");
    assert_eq!(nnonatc,  864_691_128_455_135_232,        "k3: NNONATC=864_691_128_455_135_232 (3\u{00d7}288_230_376_151_711_744; 4\u{00b2}\u{2079}=2\u{2075}\u{2078}=288_230_376_151_711_744; S-uniform S=4)");
    assert_eq!(nhnonatc, u64::MAX,                       "k3: NHNONATC=u64::MAX (3\u{00d7}8\u{00b2}\u{2078}=3\u{00d7}2\u{2078}\u{2074} >> u64::MAX; per-edge saturates)");
    assert_eq!(nzso,     u64::MAX,                       "k3: NZSO=u64::MAX (3\u{00d7}32\u{00b2}\u{00b3}=3\u{00d7}2\u{00b9}\u{00b9}\u{2075} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NNONATC:  5×4^29 = 5×288_230_376_151_711_744 = 1_441_151_880_758_558_720 (fits u64).
// NHNONATC: 4×8^28 → SATURATES.
// NZSO:     4×32^23 → SATURATES.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);
    add_node(T55_VEC_C, T55_KEY_C, T55_ID_C);
    add_node(T55_VEC_D, T55_KEY_D, T55_ID_D);
    add_node(T55_VEC_E, T55_KEY_E, T55_ID_E);
    add_edge(T55_ID_A, T55_ID_B, "t55.e.ab");
    add_edge(T55_ID_A, T55_ID_C, "t55.e.ac");
    add_edge(T55_ID_A, T55_ID_D, "t55.e.ad");
    add_edge(T55_ID_A, T55_ID_E, "t55.e.ae");

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       5,                              "star: node_count=5");
    assert_eq!(ec,       4,                              "star: edge_count=4");
    assert_eq!(nnonatc,  1_441_151_880_758_558_720,      "star: NNONATC=1_441_151_880_758_558_720 (5\u{00d7}288_230_376_151_711_744; same S as K\u{2083})");
    assert_eq!(nhnonatc, u64::MAX,                       "star: NHNONATC=u64::MAX (4\u{00d7}8\u{00b2}\u{2078} >> u64::MAX; per-edge saturates)");
    assert_eq!(nzso,     u64::MAX,                       "star: NZSO=u64::MAX (4\u{00d7}32\u{00b2}\u{00b3} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NNONATC:  2^29+3^29+3^29+2^29 = 2×536_870_912+2×68_630_377_364_883 = 137_261_828_471_590.
//   (3^29=3^28×3=22_876_792_454_961×3=68_630_377_364_883)
// NHNONATC: 5^28+6^28+5^28 — 5^28>>u64::MAX per-edge → SATURATES.
// NZSO:     13^23+18^23+13^23 — 13^23>>u64::MAX per-edge → SATURATES.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);
    add_node(T55_VEC_C, T55_KEY_C, T55_ID_C);
    add_node(T55_VEC_D, T55_KEY_D, T55_ID_D);
    add_edge(T55_ID_A, T55_ID_B, "t55.e.ab");
    add_edge(T55_ID_B, T55_ID_C, "t55.e.bc");
    add_edge(T55_ID_C, T55_ID_D, "t55.e.cd");

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       4,                      "p4: node_count=4");
    assert_eq!(ec,       3,                      "p4: edge_count=3");
    assert_eq!(nnonatc,  137_261_828_471_590,    "p4: NNONATC=137_261_828_471_590 (2\u{00d7}536_870_912+2\u{00d7}68_630_377_364_883; 2\u{00b2}\u{2079}+3\u{00b2}\u{2079}+3\u{00b2}\u{2079}+2\u{00b2}\u{2079})");
    assert_eq!(nhnonatc, u64::MAX,               "p4: NHNONATC=u64::MAX (5\u{00b2}\u{2078}>>u64::MAX per-edge; saturated)");
    assert_eq!(nzso,     u64::MAX,               "p4: NZSO=u64::MAX (13\u{00b2}\u{00b3}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NNONATC:  4×9^29 → SATURATES → u64::MAX.
// NHNONATC: 6×18^28 → SATURATES → u64::MAX.
// NZSO:     6×162^23 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);
    add_node(T55_VEC_C, T55_KEY_C, T55_ID_C);
    add_node(T55_VEC_D, T55_KEY_D, T55_ID_D);
    add_edge(T55_ID_A, T55_ID_B, "t55.e.ab");
    add_edge(T55_ID_B, T55_ID_A, "t55.e.ba");
    add_edge(T55_ID_A, T55_ID_C, "t55.e.ac");
    add_edge(T55_ID_C, T55_ID_A, "t55.e.ca");
    add_edge(T55_ID_A, T55_ID_D, "t55.e.ad");
    add_edge(T55_ID_D, T55_ID_A, "t55.e.da");
    add_edge(T55_ID_B, T55_ID_C, "t55.e.bc");
    add_edge(T55_ID_C, T55_ID_B, "t55.e.cb");
    add_edge(T55_ID_B, T55_ID_D, "t55.e.bd");
    add_edge(T55_ID_D, T55_ID_B, "t55.e.db");
    add_edge(T55_ID_C, T55_ID_D, "t55.e.cd");
    add_edge(T55_ID_D, T55_ID_C, "t55.e.dc");

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       4,        "k4: node_count=4");
    assert_eq!(ec,       6,        "k4: edge_count=6");
    assert_eq!(nnonatc,  u64::MAX, "k4: NNONATC=u64::MAX (4\u{00d7}9\u{00b2}\u{2079} >> u64::MAX; saturated)");
    assert_eq!(nhnonatc, u64::MAX, "k4: NHNONATC=u64::MAX (6\u{00d7}18\u{00b2}\u{2078} >> u64::MAX; saturated)");
    assert_eq!(nzso,     u64::MAX, "k4: NZSO=u64::MAX (6\u{00d7}162\u{00b2}\u{00b3} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NNONATC=0; NHNONATC=0; NZSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       2, "two-iso: node_count=2");
    assert_eq!(ec,       0, "two-iso: edge_count=0");
    assert_eq!(nnonatc,  0, "two-iso: NNONATC=0");
    assert_eq!(nhnonatc, 0, "two-iso: NHNONATC=0");
    assert_eq!(nzso,     0, "two-iso: NZSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NNONATC:  5×6^29 → 6^29>>u64::MAX per-node → SATURATES.
// NHNONATC: 6×12^28 → SATURATES (12^28>>u64::MAX per-edge).
// NZSO:     6×72^23 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T55_VEC_A, T55_KEY_A, T55_ID_A);
    add_node(T55_VEC_B, T55_KEY_B, T55_ID_B);
    add_node(T55_VEC_C, T55_KEY_C, T55_ID_C);
    add_node(T55_VEC_D, T55_KEY_D, T55_ID_D);
    add_node(T55_VEC_E, T55_KEY_E, T55_ID_E);
    add_edge(T55_ID_A, T55_ID_C, "t55.e.ac");
    add_edge(T55_ID_A, T55_ID_D, "t55.e.ad");
    add_edge(T55_ID_A, T55_ID_E, "t55.e.ae");
    add_edge(T55_ID_B, T55_ID_C, "t55.e.bc");
    add_edge(T55_ID_B, T55_ID_D, "t55.e.bd");
    add_edge(T55_ID_B, T55_ID_E, "t55.e.be");

    let (nnonatc, nhnonatc, nzso, ec, nc) = gos_runtime::graph_topo_indices55();
    assert_eq!(nc,       5,        "k23: node_count=5");
    assert_eq!(ec,       6,        "k23: edge_count=6");
    assert_eq!(nnonatc,  u64::MAX, "k23: NNONATC=u64::MAX (5\u{00d7}6\u{00b2}\u{2079}; 6\u{00b2}\u{2079}>>u64::MAX per-node; saturated)");
    assert_eq!(nhnonatc, u64::MAX, "k23: NHNONATC=u64::MAX (6\u{00d7}12\u{00b2}\u{2078} >> u64::MAX; per-edge saturates)");
    assert_eq!(nzso,     u64::MAX, "k23: NZSO=u64::MAX (6\u{00d7}72\u{00b2}\u{00b3} >> u64::MAX; per-edge saturates)");
}
