// gos-graph-topo38-harness — V3.49 NDoC + NHUC + NDSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices38()`:
//   Returns (ndoc, nhuc, ndso, edge_count, node_count)
//   - ndoc = NDoC(G) = Σ_v S(v)^12                    (exact u64; S-Dodecic vertex sum)
//   - nhuc = NHUC(G) = Σ_{uv∈E} (S_u+S_v)^11          (exact u64; S-Undecic edge-sum)
//   - ndso = NDSO(G) = Σ_{uv∈E} (S_u²+S_v²)^6         (exact u64; S-Duodecic Sombor, α=12)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NDoC(G) = Σ_v S(v)^12
//     S-Dodecic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36), NUC=Σ S¹¹ (topo37), NDoC=Σ S¹² (topo38).
//     NDoC = n·S^12 for S-regular.
//     Overflow: S^12 ≤ 16129^12 ≈ 3.8×10^49 > u128::MAX → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHUC(G) = Σ_{uv∈E} (S_u+S_v)^11
//     S-Undecic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36), NHDC=Σ(S+S)¹⁰ (topo37),
//       NHUC=Σ(S+S)¹¹ (topo38).
//     NHUC = |E|·(2S)^11 = 2048|E|S^11 for S-regular.
//     Overflow per edge: (2×16129)^11 ≈ 9.0×10^48 > u128::MAX → saturating u128 accumulator.
//
//   NDSO(G) = Σ_{uv∈E} (S_u²+S_v²)^6
//     S-Duodecic Sombor: generalised Sombor SO^α with α=12 on S-variant.
//     NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
//     NFSO(topo34)=Σ(S²+S²)^2 (α=4), NHSO(topo35)=Σ(S²+S²)^3 (α=6),
//     NOSO(topo36)=Σ(S²+S²)^4 (α=8), NTSO(topo37)=Σ(S²+S²)^5 (α=10),
//     NDSO(topo38)=Σ(S²+S²)^6 (α=12) — exact, no isqrt.
//     NDSO = |E|·(2S²)^6 = 64|E|S^12 for S-regular.
//     Overflow per edge: (2×16129²)^6 ≈ 6.1×10^52 > u128::MAX → saturating u128 accumulator.
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
//  Graph       NDoC(exact)              NHUC(exact)               NDSO(exact)       edges  nodes
//  Empty                    0                       0                        0           0      0
//  1 node                   0                       0                        0           0      1
//  K₂                       2                   2_048                       64           1      2
//  P₃                  12_288               8_388_608                  524_288           2      3
//  K₃              50_331_648          25_769_803_776            3_221_225_472           3      3
//  K_{1,4}         83_886_080          34_359_738_368            4_294_967_296           4      5
//  P₄               1_071_074             460_453_306               43_665_842           3      4
//  K₄       1_129_718_145_924     385_610_460_475_392      108_452_942_008_704           6      4
//  2 isolated               0                       0                        0           0      2
//  K_{2,3}     10_883_911_680       4_458_050_224_128          835_884_417_024           6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NDoC: 1^12 + 1^12 = 2. ✓
//     NHUC: (1+1)^11 = 2^11 = 2_048. ✓
//     NDSO: (1²+1²)^6 = 2^6 = 64. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NDoC: 3×2^12 = 3×4_096 = 12_288. ✓
//     NHUC: 2×(2+2)^11 = 2×4^11 = 2×4_194_304 = 8_388_608. ✓
//     NDSO: 2×(4+4)^6 = 2×8^6 = 2×262_144 = 524_288. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NDoC: 3×4^12 = 3×16_777_216 = 50_331_648. ✓
//     NHUC: 3×(4+4)^11 = 3×8^11 = 3×8_589_934_592 = 25_769_803_776. ✓
//     NDSO: 3×(16+16)^6 = 3×32^6 = 3×1_073_741_824 = 3_221_225_472. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NDoC: 5×4^12 = 5×16_777_216 = 83_886_080. ✓
//     NHUC: 4×8^11 = 4×8_589_934_592 = 34_359_738_368. ✓
//     NDSO: 4×32^6 = 4×1_073_741_824 = 4_294_967_296. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHUC and NDSO; NDoC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NDoC: 2^12+3^12+3^12+2^12 = 4_096+531_441+531_441+4_096 = 1_071_074. ✓
//     NHUC: 5^11+6^11+5^11 = 48_828_125+362_797_056+48_828_125 = 460_453_306. ✓
//       (5^11=48_828_125; 6^11=362_797_056)
//     NDSO: 13^6+18^6+13^6 = 4_826_809+34_012_224+4_826_809 = 43_665_842. ✓
//       (S_A²+S_B²=4+9=13; 13^6=4_826_809; S_B²+S_C²=9+9=18; 18^6=34_012_224)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NDoC: 4×9^12 = 4×282_429_536_481 = 1_129_718_145_924. ✓
//       (9^11=31_381_059_609; 9^12=31_381_059_609×9=282_429_536_481)
//     NHUC: 6×18^11 = 6×64_268_410_079_232 = 385_610_460_475_392. ✓
//       (18^10=3_570_467_226_624; 18^11=3_570_467_226_624×18=64_268_410_079_232)
//     NDSO: 6×162^6 = 6×18_075_490_334_784 = 108_452_942_008_704. ✓
//       (162^5=111_577_100_832; 162^6=111_577_100_832×162=18_075_490_334_784)
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NDoC: 5×6^12 = 5×2_176_782_336 = 10_883_911_680. ✓
//       (6^11=362_797_056; 6^12=362_797_056×6=2_176_782_336)
//     NHUC: 6×12^11 = 6×743_008_370_688 = 4_458_050_224_128. ✓
//       (12^10=61_917_364_224; 12^11=61_917_364_224×12=743_008_370_688)
//     NDSO: 6×72^6 = 6×139_314_069_504 = 835_884_417_024. ✓
//       (72^5=1_934_917_632; 72^6=1_934_917_632×72=139_314_069_504)
//
// S-REGULAR FORMULA VERIFICATION:
//   NDoC = n·S^12                         for S-regular ✓
//   NHUC = |E|·(2S)^11 = 2048|E|·S^11    for S-regular ✓
//   NDSO = |E|·(2S²)^6 = 64|E|·S^12      for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 2_048, 64, 1, 2)
//  4.  Path P₃ = A-B-C                   → (12_288, 8_388_608, 524_288, 2, 3)
//  5.  Triangle K₃                       → (50_331_648, 25_769_803_776, 3_221_225_472, 3, 3)
//  6.  Star K_{1,4}                      → (83_886_080, 34_359_738_368, 4_294_967_296, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (1_071_074, 460_453_306, 43_665_842, 3, 4)
//  8.  Complete K₄                       → (1_129_718_145_924, 385_610_460_475_392, 108_452_942_008_704, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (10_883_911_680, 4_458_050_224_128, 835_884_417_024, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T38_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_38");
const T38_EXEC:   ExecutorId = ExecutorId::from_ascii("t38.exec");

const T38_KEY_A: &str = "t38.alpha";
const T38_KEY_B: &str = "t38.beta";
const T38_KEY_C: &str = "t38.gamma";
const T38_KEY_D: &str = "t38.delta";
const T38_KEY_E: &str = "t38.epsilon";

const T38_ID_A: NodeId = derive_node_id(T38_PLUGIN, T38_KEY_A);
const T38_ID_B: NodeId = derive_node_id(T38_PLUGIN, T38_KEY_B);
const T38_ID_C: NodeId = derive_node_id(T38_PLUGIN, T38_KEY_C);
const T38_ID_D: NodeId = derive_node_id(T38_PLUGIN, T38_KEY_D);
const T38_ID_E: NodeId = derive_node_id(T38_PLUGIN, T38_KEY_E);

// L4=125 namespace for this harness.
const T38_VEC_A: VectorAddress = VectorAddress::new(125, 1, 1, 0);
const T38_VEC_B: VectorAddress = VectorAddress::new(125, 1, 2, 0);
const T38_VEC_C: VectorAddress = VectorAddress::new(125, 1, 3, 0);
const T38_VEC_D: VectorAddress = VectorAddress::new(125, 2, 1, 0);
const T38_VEC_E: VectorAddress = VectorAddress::new(125, 2, 2, 0);

const T38_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T38_PLUGIN,
    name:         "kl-graph-topo38-harness",
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
        executor_id:       T38_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T38_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T38_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(ndoc, 0, "empty: NDoC=0");
    assert_eq!(nhuc, 0, "empty: NHUC=0");
    assert_eq!(ndso, 0, "empty: NDSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NDoC: 0^12=0; NHUC: no edges; NDSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(ndoc, 0, "single: NDoC=0 (S=0; 0^12=0)");
    assert_eq!(nhuc, 0, "single: NHUC=0 (no edges)");
    assert_eq!(ndso, 0, "single: NDSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NDoC: 1^12+1^12 = 2.
// NHUC: (1+1)^11 = 2^11 = 2_048.
// NDSO: (1²+1²)^6 = 2^6 = 64.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);
    add_edge(T38_ID_A, T38_ID_B, "t38.e.ab");

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   2,     "k2: node_count=2");
    assert_eq!(ec,   1,     "k2: edge_count=1");
    assert_eq!(ndoc, 2,     "k2: NDoC=2 (1\u{00b9}\u{00b2}+1\u{00b9}\u{00b2}=2; S-uniform S=1)");
    assert_eq!(nhuc, 2_048, "k2: NHUC=2_048 ((1+1)\u{00b9}\u{00b9}=2\u{00b9}\u{00b9}=2_048; S-uniform S=1)");
    assert_eq!(ndso, 64,    "k2: NDSO=64 ((1\u{00b2}+1\u{00b2})\u{2076}=2\u{2076}=64; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NDoC: 3×2^12 = 3×4_096 = 12_288.
// NHUC: 2×(2+2)^11 = 2×4^11 = 2×4_194_304 = 8_388_608.
// NDSO: 2×(4+4)^6 = 2×8^6 = 2×262_144 = 524_288.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);
    add_node(T38_VEC_C, T38_KEY_C, T38_ID_C);
    add_edge(T38_ID_A, T38_ID_B, "t38.e.ab");
    add_edge(T38_ID_B, T38_ID_C, "t38.e.bc");

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(ndoc, 12_288,    "p3: NDoC=12_288 (3\u{00d7}4_096; 2\u{00b9}\u{00b2}=4_096; S-uniform S=2)");
    assert_eq!(nhuc, 8_388_608, "p3: NHUC=8_388_608 (2\u{00d7}4_194_304; (2+2)\u{00b9}\u{00b9}=4\u{00b9}\u{00b9}=4_194_304; S-uniform S=2)");
    assert_eq!(ndso, 524_288,   "p3: NDSO=524_288 (2\u{00d7}262_144; (4+4)\u{2076}=8\u{2076}=262_144; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NDoC: 3×4^12 = 3×16_777_216 = 50_331_648.
// NHUC: 3×(4+4)^11 = 3×8^11 = 3×8_589_934_592 = 25_769_803_776.
// NDSO: 3×(16+16)^6 = 3×32^6 = 3×1_073_741_824 = 3_221_225_472.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);
    add_node(T38_VEC_C, T38_KEY_C, T38_ID_C);
    add_edge(T38_ID_A, T38_ID_B, "t38.e.ab");
    add_edge(T38_ID_B, T38_ID_A, "t38.e.ba");
    add_edge(T38_ID_B, T38_ID_C, "t38.e.bc");
    add_edge(T38_ID_C, T38_ID_B, "t38.e.cb");
    add_edge(T38_ID_A, T38_ID_C, "t38.e.ac");
    add_edge(T38_ID_C, T38_ID_A, "t38.e.ca");

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   3,              "k3: node_count=3");
    assert_eq!(ec,   3,              "k3: edge_count=3");
    assert_eq!(ndoc, 50_331_648,     "k3: NDoC=50_331_648 (3\u{00d7}16_777_216; 4\u{00b9}\u{00b2}=16_777_216; S-uniform S=4)");
    assert_eq!(nhuc, 25_769_803_776, "k3: NHUC=25_769_803_776 (3\u{00d7}8_589_934_592; (4+4)\u{00b9}\u{00b9}=8\u{00b9}\u{00b9}=8_589_934_592; S-uniform S=4)");
    assert_eq!(ndso, 3_221_225_472,  "k3: NDSO=3_221_225_472 (3\u{00d7}1_073_741_824; (16+16)\u{2076}=32\u{2076}=1_073_741_824; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHUC (8_589_934_592) and NDSO (1_073_741_824) as K₃; NDoC and totals differ.
// NDoC: 5×4^12 = 5×16_777_216 = 83_886_080.
// NHUC: 4×8^11 = 4×8_589_934_592 = 34_359_738_368.
// NDSO: 4×32^6 = 4×1_073_741_824 = 4_294_967_296.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);
    add_node(T38_VEC_C, T38_KEY_C, T38_ID_C);
    add_node(T38_VEC_D, T38_KEY_D, T38_ID_D);
    add_node(T38_VEC_E, T38_KEY_E, T38_ID_E);
    add_edge(T38_ID_A, T38_ID_B, "t38.e.ab");
    add_edge(T38_ID_A, T38_ID_C, "t38.e.ac");
    add_edge(T38_ID_A, T38_ID_D, "t38.e.ad");
    add_edge(T38_ID_A, T38_ID_E, "t38.e.ae");

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   5,              "star: node_count=5");
    assert_eq!(ec,   4,              "star: edge_count=4");
    assert_eq!(ndoc, 83_886_080,     "star: NDoC=83_886_080 (5\u{00d7}16_777_216; same S as K\u{2083})");
    assert_eq!(nhuc, 34_359_738_368, "star: NHUC=34_359_738_368 (4\u{00d7}8_589_934_592; same per-edge as K\u{2083})");
    assert_eq!(ndso, 4_294_967_296,  "star: NDSO=4_294_967_296 (4\u{00d7}1_073_741_824; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NDoC: 2^12+3^12+3^12+2^12 = 4_096+531_441+531_441+4_096 = 1_071_074.
// NHUC: (2+3)^11+(3+3)^11+(3+2)^11 = 5^11+6^11+5^11 = 48_828_125+362_797_056+48_828_125 = 460_453_306.
// NDSO: 13^6+18^6+13^6 = 4_826_809+34_012_224+4_826_809 = 43_665_842.
//   (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);
    add_node(T38_VEC_C, T38_KEY_C, T38_ID_C);
    add_node(T38_VEC_D, T38_KEY_D, T38_ID_D);
    add_edge(T38_ID_A, T38_ID_B, "t38.e.ab");
    add_edge(T38_ID_B, T38_ID_C, "t38.e.bc");
    add_edge(T38_ID_C, T38_ID_D, "t38.e.cd");

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   4,           "p4: node_count=4");
    assert_eq!(ec,   3,           "p4: edge_count=3");
    assert_eq!(ndoc, 1_071_074,   "p4: NDoC=1_071_074 (4_096+531_441+531_441+4_096; 2\u{00b9}\u{00b2}+3\u{00b9}\u{00b2}+3\u{00b9}\u{00b2}+2\u{00b9}\u{00b2})");
    assert_eq!(nhuc, 460_453_306, "p4: NHUC=460_453_306 (48_828_125+362_797_056+48_828_125; 5\u{00b9}\u{00b9}+6\u{00b9}\u{00b9}+5\u{00b9}\u{00b9})");
    assert_eq!(ndso, 43_665_842,  "p4: NDSO=43_665_842 (4_826_809+34_012_224+4_826_809; 13\u{2076}+18\u{2076}+13\u{2076})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NDoC: 4×9^12 = 4×282_429_536_481 = 1_129_718_145_924.
// NHUC: 6×18^11 = 6×64_268_410_079_232 = 385_610_460_475_392.
// NDSO: 6×162^6 = 6×18_075_490_334_784 = 108_452_942_008_704.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);
    add_node(T38_VEC_C, T38_KEY_C, T38_ID_C);
    add_node(T38_VEC_D, T38_KEY_D, T38_ID_D);
    add_edge(T38_ID_A, T38_ID_B, "t38.e.ab");
    add_edge(T38_ID_B, T38_ID_A, "t38.e.ba");
    add_edge(T38_ID_A, T38_ID_C, "t38.e.ac");
    add_edge(T38_ID_C, T38_ID_A, "t38.e.ca");
    add_edge(T38_ID_A, T38_ID_D, "t38.e.ad");
    add_edge(T38_ID_D, T38_ID_A, "t38.e.da");
    add_edge(T38_ID_B, T38_ID_C, "t38.e.bc");
    add_edge(T38_ID_C, T38_ID_B, "t38.e.cb");
    add_edge(T38_ID_B, T38_ID_D, "t38.e.bd");
    add_edge(T38_ID_D, T38_ID_B, "t38.e.db");
    add_edge(T38_ID_C, T38_ID_D, "t38.e.cd");
    add_edge(T38_ID_D, T38_ID_C, "t38.e.dc");

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   4,                       "k4: node_count=4");
    assert_eq!(ec,   6,                       "k4: edge_count=6");
    assert_eq!(ndoc, 1_129_718_145_924,       "k4: NDoC=1_129_718_145_924 (4\u{00d7}282_429_536_481; 9\u{00b9}\u{00b2}=282_429_536_481; S-uniform S=9)");
    assert_eq!(nhuc, 385_610_460_475_392,     "k4: NHUC=385_610_460_475_392 (6\u{00d7}64_268_410_079_232; 18\u{00b9}\u{00b9}=64_268_410_079_232; S-uniform S=9)");
    assert_eq!(ndso, 108_452_942_008_704,     "k4: NDSO=108_452_942_008_704 (6\u{00d7}18_075_490_334_784; 162\u{2076}=18_075_490_334_784; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NDoC=0; NHUC=0; NDSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(ndoc, 0, "isolated: NDoC=0 (S=0; 0^12=0)");
    assert_eq!(nhuc, 0, "isolated: NHUC=0 (no edges)");
    assert_eq!(ndso, 0, "isolated: NDSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NDoC: 5×6^12 = 5×2_176_782_336 = 10_883_911_680.
// NHUC: 6×12^11 = 6×743_008_370_688 = 4_458_050_224_128.
// NDSO: 6×72^6 = 6×139_314_069_504 = 835_884_417_024.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T38_VEC_A, T38_KEY_A, T38_ID_A);
    add_node(T38_VEC_B, T38_KEY_B, T38_ID_B);
    add_node(T38_VEC_C, T38_KEY_C, T38_ID_C);
    add_node(T38_VEC_D, T38_KEY_D, T38_ID_D);
    add_node(T38_VEC_E, T38_KEY_E, T38_ID_E);
    add_edge(T38_ID_A, T38_ID_C, "t38.e.ac");
    add_edge(T38_ID_C, T38_ID_A, "t38.e.ca");
    add_edge(T38_ID_A, T38_ID_D, "t38.e.ad");
    add_edge(T38_ID_D, T38_ID_A, "t38.e.da");
    add_edge(T38_ID_A, T38_ID_E, "t38.e.ae");
    add_edge(T38_ID_E, T38_ID_A, "t38.e.ea");
    add_edge(T38_ID_B, T38_ID_C, "t38.e.bc");
    add_edge(T38_ID_C, T38_ID_B, "t38.e.cb");
    add_edge(T38_ID_B, T38_ID_D, "t38.e.bd");
    add_edge(T38_ID_D, T38_ID_B, "t38.e.db");
    add_edge(T38_ID_B, T38_ID_E, "t38.e.be");
    add_edge(T38_ID_E, T38_ID_B, "t38.e.eb");

    let (ndoc, nhuc, ndso, ec, nc) = gos_runtime::graph_topo_indices38();
    assert_eq!(nc,   5,                "k23: node_count=5");
    assert_eq!(ec,   6,                "k23: edge_count=6");
    assert_eq!(ndoc, 10_883_911_680,   "k23: NDoC=10_883_911_680 (5\u{00d7}2_176_782_336; 6\u{00b9}\u{00b2}=2_176_782_336; S-uniform S=6)");
    assert_eq!(nhuc, 4_458_050_224_128,"k23: NHUC=4_458_050_224_128 (6\u{00d7}743_008_370_688; 12\u{00b9}\u{00b9}=743_008_370_688; S-uniform S=6)");
    assert_eq!(ndso, 835_884_417_024,  "k23: NDSO=835_884_417_024 (6\u{00d7}139_314_069_504; 72\u{2076}=139_314_069_504; S-uniform S=6)");
}
