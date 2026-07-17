// gos-graph-topo52-harness — V3.63 NHEXATC + NHHEXATC + NVSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices52()`:
//   Returns (nhexatc, nhhexatc, nvso, edge_count, node_count)
//   - nhexatc  = NHEXATC(G)  = Σ_v S(v)^26                  (exact u64; S-Hexacosic vertex sum)
//   - nhhexatc = NHHEXATC(G) = Σ_{uv∈E} (S_u+S_v)^25        (exact u64; S-Pentacosic edge-sum)
//   - nvso     = NVSO(G)     = Σ_{uv∈E} (S_u²+S_v²)^20      (exact u64; S-Tetracontyl Sombor, α=40)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXATC(G) = Σ_v S(v)^26
//     S-Hexacosic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38), NTC=Σ S¹³ (topo39),
//       NQTC=Σ S¹⁴ (topo40), NPTC=Σ S¹⁵ (topo41), NSTC=Σ S¹⁶ (topo42),
//       NHEPTC=Σ S¹⁷ (topo43), NOCTC=Σ S¹⁸ (topo44), NNONTC=Σ S¹⁹ (topo45),
//       NEICTC=Σ S²⁰ (topo46), NHENTC=Σ S²¹ (topo47), NDOCTC=Σ S²² (topo48),
//       NTRICTC=Σ S²³ (topo49), NTETRTC=Σ S²⁴ (topo50), NPENTTC=Σ S²⁵ (topo51),
//       NHEXATC=Σ S²⁶ (topo52).
//     NHEXATC = n·S^26 for S-regular.
//     Overflow: S^26 ≤ 16129^26 → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHHEXATC(G) = Σ_{uv∈E} (S_u+S_v)^25
//     S-Pentacosic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38), NHDOC=Σ(S+S)¹² (topo39), NHTC=Σ(S+S)¹³ (topo40),
//       NHQTC=Σ(S+S)¹⁴ (topo41), NHPTC=Σ(S+S)¹⁵ (topo42), NHSTC=Σ(S+S)¹⁶ (topo43),
//       NHOCTC=Σ(S+S)¹⁷ (topo44), NHNONTC=Σ(S+S)¹⁸ (topo45), NHEICTC=Σ(S+S)¹⁹ (topo46),
//       NHHENTC=Σ(S+S)²⁰ (topo47), NHDOCTC=Σ(S+S)²¹ (topo48), NHTRICTC=Σ(S+S)²² (topo49),
//       NHTETRTC=Σ(S+S)²³ (topo50), NHPENTTC=Σ(S+S)²⁴ (topo51), NHHEXATC=Σ(S+S)²⁵ (topo52).
//     NHHEXATC = |E|·(2S)^25 = 33554432|E|·S^25 for S-regular.
//     Overflow per edge: (2×16129)^25 → saturating u128 accumulator.
//
//   NVSO(G) = Σ_{uv∈E} (S_u²+S_v²)^20
//     S-Tetracontyl Sombor: generalised Sombor SO^α with α=40 on S-variant.
//     NSO(topo21,α=1), NCSO(topo33,α=3), NFSO(topo34,α=4), NHSO(topo35,α=6),
//     NOSO(topo36,α=8), NTSO(topo37,α=10), NDSO(topo38,α=12), NESO(topo39,α=14),
//     NGSO(topo40,α=16), NIOSO(topo41,α=18), NJSO(topo42,α=20), NKSO(topo43,α=22),
//     NLSO(topo44,α=24), NMSO(topo45,α=26), NNSO(topo46,α=28), NPSO(topo47,α=30),
//     NQSO(topo48,α=32), NRSO(topo49,α=34), NSSO(topo50,α=36), NUSO(topo51,α=38),
//     NVSO(topo52,α=40).
//     NVSO = |E|·(2S²)^20 = 1048576|E|·S^40 for S-regular.
//     Overflow per edge: (2×16129²)^20 → saturating u128 accumulator.
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
//  Graph     NHEXATC(exact)              NHHEXATC(exact)               NVSO(exact)              edges  nodes
//  Empty                  0                            0                         0               0      0
//  1 node                 0                            0                         0               0      1
//  K₂                     2                   33_554_432                 1_048_576               1      2
//  P₃             201_326_592        2_251_799_813_685_248   2_305_843_009_213_693_952            2      3
//  K₃       13_510_798_882_111_488       u64::MAX(sat.)              u64::MAX(sat.)               3      3
//  K_{1,4}  22_517_998_136_852_480       u64::MAX(sat.)              u64::MAX(sat.)               4      5
//  P₄         5_083_865_874_386          u64::MAX(sat.)              u64::MAX(sat.)               3      4
//  K₄          u64::MAX(sat.)          u64::MAX(sat.)               u64::MAX(sat.)               6      4
//  2 isolated             0                            0                         0               0      2
//  K_{2,3}    u64::MAX(sat.)           u64::MAX(sat.)               u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXATC:  1^26 + 1^26 = 2. ✓
//     NHHEXATC: (1+1)^25 = 2^25 = 33_554_432. ✓
//     NVSO:     (1²+1²)^20 = 2^20 = 1_048_576. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXATC:  3×2^26 = 3×67_108_864 = 201_326_592. ✓
//     NHHEXATC: 2×(2+2)^25 = 2×4^25 = 2×2^50 = 2^51 = 2_251_799_813_685_248. ✓
//       (4^25=2^50=1_125_899_906_842_624; 2×4^25=2_251_799_813_685_248)
//     NVSO:     2×(4+4)^20 = 2×8^20 = 2×2^60 = 2^61 = 2_305_843_009_213_693_952. ✓
//       (8^20=2^60=1_152_921_504_606_846_976; 2×8^20=2^61=2_305_843_009_213_693_952)
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXATC:  3×4^26 = 3×2^52 = 3×4_503_599_627_370_496 = 13_510_798_882_111_488 (fits u64). ✓
//       (4^26=2^52; 3×2^52=13_510_798_882_111_488 < 2^64)
//     NHHEXATC: 3×(4+4)^25 = 3×8^25 = 3×2^75 → SATURATES.
//       (8^25=2^75≈3.78×10^22 >> u64::MAX per-edge). ✓
//     NVSO:     3×(16+16)^20 = 3×32^20 = 3×2^100 → SATURATES (per-edge >> u64::MAX). ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXATC:  5×4^26 = 5×4_503_599_627_370_496 = 22_517_998_136_852_480 (fits u64). ✓
//     NHHEXATC: 4×8^25 → SATURATES. ✓
//     NVSO:     4×32^20 → SATURATES (per-edge >> u64::MAX). ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXATC:  2^26+3^26+3^26+2^26 = 2×67_108_864+2×2_541_865_828_329.
//       3^26=3^16×3^8×3^2=43_046_721×6_561×9=2_541_865_828_329
//       2×67_108_864+2×2_541_865_828_329=134_217_728+5_083_731_656_658=5_083_865_874_386. ✓
//     NHHEXATC: 5^25+6^25+5^25
//       5^25=5^16×5^8×5=152_587_890_625×390_625×5=298_023_223_876_953_125 >> u64::MAX per-edge → SATURATES. ✓
//     NVSO:     13^20+18^20+13^20
//       13^20=(13^10)^2; 13^10=137_858_491_849; (137_858_491_849)^2>>u64::MAX per-edge → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXATC:  4×9^26 → SATURATES (9^26=(9^13)^2=(2_541_865_828_329)^2>>u64::MAX). ✓
//     NHHEXATC: 6×18^25 → SATURATES. ✓
//     NVSO:     6×162^20 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXATC:  5×6^26. 6^24=4_738_381_338_321_616_896; 6^26=6^24×36>>u64::MAX → SATURATES. ✓
//     NHHEXATC: 6×12^25 → SATURATES (12^25>>u64::MAX per-edge). ✓
//     NVSO:     6×72^20 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXATC  = n·S^26                                      for S-regular ✓
//   NHHEXATC = |E|·(2S)^25 = 33554432|E|·S^25             for S-regular ✓
//   NVSO     = |E|·(2S²)^20 = 1048576|E|·S^40             for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 33_554_432, 1_048_576, 1, 2)
//  4.  Path P₃ = A-B-C                   → (201_326_592, 2_251_799_813_685_248, 2_305_843_009_213_693_952, 2, 3)
//  5.  Triangle K₃                       → (13_510_798_882_111_488, u64::MAX, u64::MAX, 3, 3)
//  6.  Star K_{1,4}                      → (22_517_998_136_852_480, u64::MAX, u64::MAX, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (5_083_865_874_386, u64::MAX, u64::MAX, 3, 4)
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

const T52_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_52");
const T52_EXEC:   ExecutorId = ExecutorId::from_ascii("t52.exec");

const T52_KEY_A: &str = "t52.alpha";
const T52_KEY_B: &str = "t52.beta";
const T52_KEY_C: &str = "t52.gamma";
const T52_KEY_D: &str = "t52.delta";
const T52_KEY_E: &str = "t52.epsilon";

const T52_ID_A: NodeId = derive_node_id(T52_PLUGIN, T52_KEY_A);
const T52_ID_B: NodeId = derive_node_id(T52_PLUGIN, T52_KEY_B);
const T52_ID_C: NodeId = derive_node_id(T52_PLUGIN, T52_KEY_C);
const T52_ID_D: NodeId = derive_node_id(T52_PLUGIN, T52_KEY_D);
const T52_ID_E: NodeId = derive_node_id(T52_PLUGIN, T52_KEY_E);

// L4=139 namespace for this harness.
const T52_VEC_A: VectorAddress = VectorAddress::new(139, 1, 1, 0);
const T52_VEC_B: VectorAddress = VectorAddress::new(139, 1, 2, 0);
const T52_VEC_C: VectorAddress = VectorAddress::new(139, 1, 3, 0);
const T52_VEC_D: VectorAddress = VectorAddress::new(139, 2, 1, 0);
const T52_VEC_E: VectorAddress = VectorAddress::new(139, 2, 2, 0);

const T52_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T52_PLUGIN,
    name:         "kl-graph-topo52-harness",
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
        executor_id:       T52_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T52_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T52_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       0, "empty: node_count=0");
    assert_eq!(ec,       0, "empty: edge_count=0");
    assert_eq!(nhexatc,  0, "empty: NHEXATC=0");
    assert_eq!(nhhexatc, 0, "empty: NHHEXATC=0");
    assert_eq!(nvso,     0, "empty: NVSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NHEXATC: 0^26=0; NHHEXATC: no edges; NVSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       1, "single: node_count=1");
    assert_eq!(ec,       0, "single: no edges");
    assert_eq!(nhexatc,  0, "single: NHEXATC=0 (S=0; 0^26=0)");
    assert_eq!(nhhexatc, 0, "single: NHHEXATC=0 (no edges)");
    assert_eq!(nvso,     0, "single: NVSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NHEXATC:  1^26+1^26 = 2.
// NHHEXATC: (1+1)^25 = 2^25 = 33_554_432.
// NVSO:     (1²+1²)^20 = 2^20 = 1_048_576.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);
    add_edge(T52_ID_A, T52_ID_B, "t52.e.ab");

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       2,          "k2: node_count=2");
    assert_eq!(ec,       1,          "k2: edge_count=1");
    assert_eq!(nhexatc,  2,          "k2: NHEXATC=2 (1\u{00b2}\u{2076}+1\u{00b2}\u{2076}=2; S-uniform S=1)");
    assert_eq!(nhhexatc, 33_554_432, "k2: NHHEXATC=33_554_432 ((1+1)\u{00b2}\u{2075}=2\u{00b2}\u{2075}=33_554_432; S-uniform S=1)");
    assert_eq!(nvso,     1_048_576,  "k2: NVSO=1_048_576 ((1\u{00b2}+1\u{00b2})\u{00b2}\u{2070}=2\u{00b2}\u{2070}=1_048_576; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NHEXATC:  3×2^26 = 3×67_108_864 = 201_326_592.
// NHHEXATC: 2×(2+2)^25 = 2×4^25 = 2×1_125_899_906_842_624 = 2_251_799_813_685_248.
// NVSO:     2×(4+4)^20 = 2×8^20 = 2×1_152_921_504_606_846_976 = 2_305_843_009_213_693_952.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);
    add_node(T52_VEC_C, T52_KEY_C, T52_ID_C);
    add_edge(T52_ID_A, T52_ID_B, "t52.e.ab");
    add_edge(T52_ID_B, T52_ID_C, "t52.e.bc");

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       3,                            "p3: node_count=3");
    assert_eq!(ec,       2,                            "p3: edge_count=2");
    assert_eq!(nhexatc,  201_326_592,                  "p3: NHEXATC=201_326_592 (3\u{00d7}67_108_864; 2\u{00b2}\u{2076}=67_108_864; S-uniform S=2)");
    assert_eq!(nhhexatc, 2_251_799_813_685_248,        "p3: NHHEXATC=2_251_799_813_685_248 (2\u{00d7}1_125_899_906_842_624; (2+2)\u{00b2}\u{2075}=4\u{00b2}\u{2075}=1_125_899_906_842_624; S-uniform S=2)");
    assert_eq!(nvso,     2_305_843_009_213_693_952,    "p3: NVSO=2_305_843_009_213_693_952 (2\u{00d7}1_152_921_504_606_846_976; (4+4)\u{00b2}\u{2070}=8\u{00b2}\u{2070}=1_152_921_504_606_846_976; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NHEXATC:  3×4^26 = 3×2^52 = 13_510_798_882_111_488 (fits u64).
// NHHEXATC: 3×(4+4)^25 = 3×8^25 = 3×2^75 → SATURATES (2^75≈3.78×10^22>>u64::MAX per-edge).
// NVSO:     3×(16+16)^20 = 3×32^20 = 3×2^100 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);
    add_node(T52_VEC_C, T52_KEY_C, T52_ID_C);
    add_edge(T52_ID_A, T52_ID_B, "t52.e.ab");
    add_edge(T52_ID_B, T52_ID_A, "t52.e.ba");
    add_edge(T52_ID_B, T52_ID_C, "t52.e.bc");
    add_edge(T52_ID_C, T52_ID_B, "t52.e.cb");
    add_edge(T52_ID_A, T52_ID_C, "t52.e.ac");
    add_edge(T52_ID_C, T52_ID_A, "t52.e.ca");

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       3,                         "k3: node_count=3");
    assert_eq!(ec,       3,                         "k3: edge_count=3");
    assert_eq!(nhexatc,  13_510_798_882_111_488,    "k3: NHEXATC=13_510_798_882_111_488 (3\u{00d7}4_503_599_627_370_496; 4\u{00b2}\u{2076}=2\u{2075}\u{00b2}=4_503_599_627_370_496; S-uniform S=4)");
    assert_eq!(nhhexatc, u64::MAX,                  "k3: NHHEXATC=u64::MAX (3\u{00d7}8\u{00b2}\u{2075}=3\u{00d7}2\u{2077}\u{2075} >> u64::MAX; saturated)");
    assert_eq!(nvso,     u64::MAX,                  "k3: NVSO=u64::MAX (3\u{00d7}32\u{00b2}\u{2070}=3\u{00d7}2\u{00b9}\u{2070}\u{2070} >> u64::MAX; per-edge already saturates)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// NHEXATC:  5×4^26 = 5×4_503_599_627_370_496 = 22_517_998_136_852_480 (fits u64).
// NHHEXATC: 4×8^25 → SATURATES.
// NVSO:     4×32^20 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);
    add_node(T52_VEC_C, T52_KEY_C, T52_ID_C);
    add_node(T52_VEC_D, T52_KEY_D, T52_ID_D);
    add_node(T52_VEC_E, T52_KEY_E, T52_ID_E);
    add_edge(T52_ID_A, T52_ID_B, "t52.e.ab");
    add_edge(T52_ID_A, T52_ID_C, "t52.e.ac");
    add_edge(T52_ID_A, T52_ID_D, "t52.e.ad");
    add_edge(T52_ID_A, T52_ID_E, "t52.e.ae");

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       5,                         "star: node_count=5");
    assert_eq!(ec,       4,                         "star: edge_count=4");
    assert_eq!(nhexatc,  22_517_998_136_852_480,    "star: NHEXATC=22_517_998_136_852_480 (5\u{00d7}4_503_599_627_370_496; same S as K\u{2083})");
    assert_eq!(nhhexatc, u64::MAX,                  "star: NHHEXATC=u64::MAX (4\u{00d7}8\u{00b2}\u{2075} >> u64::MAX; saturated)");
    assert_eq!(nvso,     u64::MAX,                  "star: NVSO=u64::MAX (4\u{00d7}32\u{00b2}\u{2070} >> u64::MAX; per-edge already saturates)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NHEXATC:  2^26+3^26+3^26+2^26 = 2×67_108_864+2×2_541_865_828_329 = 5_083_865_874_386.
//   (3^26=3^16×3^8×3^2=43_046_721×6_561×9=2_541_865_828_329)
// NHHEXATC: 5^25+6^25+5^25 — 5^25>>u64::MAX per-edge → SATURATES.
// NVSO:     13^20+18^20+13^20 — 13^20>>u64::MAX per-edge → SATURATES.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);
    add_node(T52_VEC_C, T52_KEY_C, T52_ID_C);
    add_node(T52_VEC_D, T52_KEY_D, T52_ID_D);
    add_edge(T52_ID_A, T52_ID_B, "t52.e.ab");
    add_edge(T52_ID_B, T52_ID_C, "t52.e.bc");
    add_edge(T52_ID_C, T52_ID_D, "t52.e.cd");

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       4,                   "p4: node_count=4");
    assert_eq!(ec,       3,                   "p4: edge_count=3");
    assert_eq!(nhexatc,  5_083_865_874_386,   "p4: NHEXATC=5_083_865_874_386 (2\u{00d7}67_108_864+2\u{00d7}2_541_865_828_329; 2\u{00b2}\u{2076}+3\u{00b2}\u{2076}+3\u{00b2}\u{2076}+2\u{00b2}\u{2076})");
    assert_eq!(nhhexatc, u64::MAX,            "p4: NHHEXATC=u64::MAX (5\u{00b2}\u{2075}>>u64::MAX per-edge; saturated)");
    assert_eq!(nvso,     u64::MAX,            "p4: NVSO=u64::MAX (13\u{00b2}\u{2070}>>u64::MAX per-edge; saturated)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NHEXATC:  4×9^26 → SATURATES → u64::MAX.
// NHHEXATC: 6×18^25 → SATURATES → u64::MAX.
// NVSO:     6×162^20 → SATURATES → u64::MAX.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);
    add_node(T52_VEC_C, T52_KEY_C, T52_ID_C);
    add_node(T52_VEC_D, T52_KEY_D, T52_ID_D);
    add_edge(T52_ID_A, T52_ID_B, "t52.e.ab");
    add_edge(T52_ID_B, T52_ID_A, "t52.e.ba");
    add_edge(T52_ID_A, T52_ID_C, "t52.e.ac");
    add_edge(T52_ID_C, T52_ID_A, "t52.e.ca");
    add_edge(T52_ID_A, T52_ID_D, "t52.e.ad");
    add_edge(T52_ID_D, T52_ID_A, "t52.e.da");
    add_edge(T52_ID_B, T52_ID_C, "t52.e.bc");
    add_edge(T52_ID_C, T52_ID_B, "t52.e.cb");
    add_edge(T52_ID_B, T52_ID_D, "t52.e.bd");
    add_edge(T52_ID_D, T52_ID_B, "t52.e.db");
    add_edge(T52_ID_C, T52_ID_D, "t52.e.cd");
    add_edge(T52_ID_D, T52_ID_C, "t52.e.dc");

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       4,        "k4: node_count=4");
    assert_eq!(ec,       6,        "k4: edge_count=6");
    assert_eq!(nhexatc,  u64::MAX, "k4: NHEXATC=u64::MAX (4\u{00d7}9\u{00b2}\u{2076} >> u64::MAX; saturated)");
    assert_eq!(nhhexatc, u64::MAX, "k4: NHHEXATC=u64::MAX (6\u{00d7}18\u{00b2}\u{2075} >> u64::MAX; saturated)");
    assert_eq!(nvso,     u64::MAX, "k4: NVSO=u64::MAX (6\u{00d7}162\u{00b2}\u{2070} >> u64::MAX; per-edge already saturates)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NHEXATC=0; NHHEXATC=0; NVSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       2, "two-iso: node_count=2");
    assert_eq!(ec,       0, "two-iso: edge_count=0");
    assert_eq!(nhexatc,  0, "two-iso: NHEXATC=0");
    assert_eq!(nhhexatc, 0, "two-iso: NHHEXATC=0");
    assert_eq!(nvso,     0, "two-iso: NVSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Left {A,B} deg=3, right {C,D,E} deg=2. S(all)=6. 6 edges, 5 nodes.
// NHEXATC:  5×6^26 → 6^26=6^24×36>>u64::MAX → SATURATES.
// NHHEXATC: 6×12^25 → SATURATES (12^25>>u64::MAX per-edge).
// NVSO:     6×72^20 → SATURATES (per-edge >> u64::MAX).

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T52_VEC_A, T52_KEY_A, T52_ID_A);
    add_node(T52_VEC_B, T52_KEY_B, T52_ID_B);
    add_node(T52_VEC_C, T52_KEY_C, T52_ID_C);
    add_node(T52_VEC_D, T52_KEY_D, T52_ID_D);
    add_node(T52_VEC_E, T52_KEY_E, T52_ID_E);
    add_edge(T52_ID_A, T52_ID_C, "t52.e.ac");
    add_edge(T52_ID_A, T52_ID_D, "t52.e.ad");
    add_edge(T52_ID_A, T52_ID_E, "t52.e.ae");
    add_edge(T52_ID_B, T52_ID_C, "t52.e.bc");
    add_edge(T52_ID_B, T52_ID_D, "t52.e.bd");
    add_edge(T52_ID_B, T52_ID_E, "t52.e.be");

    let (nhexatc, nhhexatc, nvso, ec, nc) = gos_runtime::graph_topo_indices52();
    assert_eq!(nc,       5,        "k23: node_count=5");
    assert_eq!(ec,       6,        "k23: edge_count=6");
    assert_eq!(nhexatc,  u64::MAX, "k23: NHEXATC=u64::MAX (5\u{00d7}6\u{00b2}\u{2076}; 6\u{00b2}\u{2074}\u{00d7}36>>u64::MAX; saturated)");
    assert_eq!(nhhexatc, u64::MAX, "k23: NHHEXATC=u64::MAX (6\u{00d7}12\u{00b2}\u{2075} >> u64::MAX; per-edge saturates)");
    assert_eq!(nvso,     u64::MAX, "k23: NVSO=u64::MAX (6\u{00d7}72\u{00b2}\u{2070} >> u64::MAX; per-edge saturates)");
}
