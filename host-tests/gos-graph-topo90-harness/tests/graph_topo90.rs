// gos-graph-topo90-harness — V3.101 NHEXATETRAACTC + NHHEXATETRAACTC + NBGSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices90()`:
//   Returns (nhexatetraactc, nhhexatetraactc, nbgso, edge_count, node_count)
//   - nhexatetraactc  = NHEXATETRAACTC(G) = Σ_v S(v)^64                    (exact u64; S-Hexacontictetradic vertex sum)
//   - nhhexatetraactc = NHHEXATETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^63         (exact u64; S-Hexacontictetradic edge-sum)
//   - nbgso           = NBGSO(G)           = Σ_{uv∈E} (S_u²+S_v²)^58       (exact u64; S-Variant Sombor, α=116)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXATETRAACTC(G) = Σ_v S(v)^64
//     S-Hexacontictetradic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXATRIACTC=Σ S⁶³ (topo89), NHEXATETRAACTC=Σ S⁶⁴ (topo90).
//     Fifth of the hexacontic (60-69) series.
//     NHEXATETRAACTC = n·S^64 for S-regular.
//     Overflow: S^64 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^64 = s32 × s32  (64=32+32; 1 final mult; 6 squarings total).
//
//   NHHEXATETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^63
//     S-Hexacontictetradic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXATRIACTC=Σ(S+S)⁶² (topo89),
//       NHHEXATETRAACTC=Σ(S+S)⁶³ (topo90).
//     NHHEXATETRAACTC = |E|·(2S)^63 = 9223372036854775808|E|·S^63 for S-regular.
//     Overflow per edge: (2×16129)^63 → saturating u128 accumulator.
//     Implementation: ss^63 = ss32 × ss16 × ss8 × ss4 × ss2 × ss  (63=32+16+8+4+2+1; 6 mults).
//
//   NBGSO(G) = Σ_{uv∈E} (S_u²+S_v²)^58
//     S-Variant Sombor: generalised Sombor SO^α with α=116 on S-variant.
//     7th of NB series, letter G (after NBFSO α=114 topo89).
//     NSO(topo21,α=1),..., NBFSO(topo89,α=114), NBGSO(topo90,α=116).
//     NBGSO = |E|·(2S²)^58 = 288230376151711744|E|·S^116 for S-regular.
//     Overflow per edge: (2×16129²)^58 → saturating u128 accumulator.
//     Implementation: s2s^58 = s2s32 × s2s16 × s2s8 × s2s2  (58=32+16+8+2; 4 mults).
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
//  Graph     NHEXATETRAACTC(exact)        NHHEXATETRAACTC(exact)       NBGSO(exact)           edges  nodes
//  Empty                      0                             0                   0               0      0
//  1 node                     0                             0                   0               0      1
//  K₂                         2         9_223_372_036_854_775_808  288_230_376_151_711_744      1      2
//  P₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)           2      3
//  K₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)           3      3
//  K_{1,4}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)           4      5
//  P₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)           3      4
//  K₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)           6      4
//  2 isolated                 0                             0                   0               0      2
//  K_{2,3}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)           6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXATETRAACTC:  1^64 + 1^64 = 2. ✓
//     NHHEXATETRAACTC: (1+1)^63 = 2^63 = 9_223_372_036_854_775_808. ✓
//     NBGSO:           (1²+1²)^58 = 2^58 = 288_230_376_151_711_744. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXATETRAACTC:  3×2^64 = 3×2^64 → 3×2^64 > u64::MAX → SATURATES. ✓
//     NHHEXATETRAACTC: 2×(2+2)^63 = 2×4^63 = 2×2^126 → SATURATES. ✓
//     NBGSO:           2×(4+4)^58 = 2×8^58 = 2×2^174 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXATETRAACTC:  3×4^64 = 3×2^128 → SATURATES. ✓
//     NHHEXATETRAACTC: 3×8^63 → SATURATES. ✓
//     NBGSO:           3×32^58 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXATETRAACTC:  5×4^64 → SATURATES. ✓
//     NHHEXATETRAACTC: 4×8^63 → SATURATES. ✓
//     NBGSO:           4×32^58 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXATETRAACTC:  2×2^64 + 2×3^64. 3^64 >> u64::MAX → SATURATES. ✓
//     NHHEXATETRAACTC: 5^63+6^63+5^63 → SATURATES. ✓
//     NBGSO:           13^58+18^58+13^58 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXATETRAACTC:  4×9^64 → SATURATES. ✓
//     NHHEXATETRAACTC: 6×18^63 → SATURATES. ✓
//     NBGSO:           6×162^58 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXATETRAACTC:  5×6^64 → SATURATES. ✓
//     NHHEXATETRAACTC: 6×12^63 → SATURATES. ✓
//     NBGSO:           6×72^58 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXATETRAACTC  = n·S^64                                                                              for S-regular ✓
//   NHHEXATETRAACTC = |E|·(2S)^63 = 9223372036854775808|E|·S^63                                          for S-regular ✓
//   NBGSO           = |E|·(2S²)^58 = 288230376151711744|E|·S^116                                         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 9_223_372_036_854_775_808, 288_230_376_151_711_744, 1, 2)
//  4.  Path P₃ = A-B-C                   → (u64::MAX, u64::MAX, u64::MAX, 2, 3)
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

const T90_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_90");
const T90_EXEC:   ExecutorId = ExecutorId::from_ascii("t90.exec");

const T90_KEY_A: &str = "t90.alpha";
const T90_KEY_B: &str = "t90.beta";
const T90_KEY_C: &str = "t90.gamma";
const T90_KEY_D: &str = "t90.delta";
const T90_KEY_E: &str = "t90.epsilon";

const T90_ID_A: NodeId = derive_node_id(T90_PLUGIN, T90_KEY_A);
const T90_ID_B: NodeId = derive_node_id(T90_PLUGIN, T90_KEY_B);
const T90_ID_C: NodeId = derive_node_id(T90_PLUGIN, T90_KEY_C);
const T90_ID_D: NodeId = derive_node_id(T90_PLUGIN, T90_KEY_D);
const T90_ID_E: NodeId = derive_node_id(T90_PLUGIN, T90_KEY_E);

// L4=177 namespace for this harness.
const T90_VEC_A: VectorAddress = VectorAddress::new(177, 1, 1, 0);
const T90_VEC_B: VectorAddress = VectorAddress::new(177, 1, 2, 0);
const T90_VEC_C: VectorAddress = VectorAddress::new(177, 1, 3, 0);
const T90_VEC_D: VectorAddress = VectorAddress::new(177, 2, 1, 0);
const T90_VEC_E: VectorAddress = VectorAddress::new(177, 2, 2, 0);

const T90_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T90_PLUGIN,
    name:         "kl-graph-topo90-harness",
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
        executor_id:       T90_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T90_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T90_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(nhexatetraactc,    0, "empty: NHEXATETRAACTC=0");
    assert_eq!(nhhexatetraactc,   0, "empty: NHHEXATETRAACTC=0");
    assert_eq!(nbgso,             0, "empty: NBGSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(nhexatetraactc,    0, "single: NHEXATETRAACTC=0");
    assert_eq!(nhhexatetraactc,   0, "single: NHHEXATETRAACTC=0");
    assert_eq!(nbgso,             0, "single: NBGSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXATETRAACTC:  1^64+1^64 = 2.
// NHHEXATETRAACTC: (1+1)^63 = 2^63 = 9_223_372_036_854_775_808.
// NBGSO:           (1²+1²)^58 = 2^58 = 288_230_376_151_711_744.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);
    add_edge(T90_ID_A, T90_ID_B, "t90.e.ab");

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,                2,                           "k2: node_count=2");
    assert_eq!(ec,                1,                           "k2: edge_count=1");
    assert_eq!(nhexatetraactc,    2,                           "k2: NHEXATETRAACTC=2 (1^64+1^64=2)");
    assert_eq!(nhhexatetraactc,   9_223_372_036_854_775_808,   "k2: NHHEXATETRAACTC=9_223_372_036_854_775_808 (2^63)");
    assert_eq!(nbgso,             288_230_376_151_711_744,     "k2: NBGSO=288_230_376_151_711_744 (2^58)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);
    add_node(T90_VEC_C, T90_KEY_C, T90_ID_C);
    add_edge(T90_ID_A, T90_ID_B, "t90.e.ab");
    add_edge(T90_ID_B, T90_ID_C, "t90.e.bc");

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,                3,         "p3: node_count=3");
    assert_eq!(ec,                2,         "p3: edge_count=2");
    assert_eq!(nhexatetraactc,    u64::MAX,  "p3: NHEXATETRAACTC=SAT (3\u{00d7}2^64>u64)");
    assert_eq!(nhhexatetraactc,   u64::MAX,  "p3: NHHEXATETRAACTC=SAT (4^63>u64)");
    assert_eq!(nbgso,             u64::MAX,  "p3: NBGSO=SAT (8^58>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);
    add_node(T90_VEC_C, T90_KEY_C, T90_ID_C);
    add_edge(T90_ID_A, T90_ID_B, "t90.e.ab");
    add_edge(T90_ID_B, T90_ID_C, "t90.e.bc");
    add_edge(T90_ID_C, T90_ID_A, "t90.e.ca");

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nhexatetraactc,   u64::MAX, "k3: NHEXATETRAACTC=SAT");
    assert_eq!(nhhexatetraactc,  u64::MAX, "k3: NHHEXATETRAACTC=SAT");
    assert_eq!(nbgso,            u64::MAX, "k3: NBGSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A); // hub
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);
    add_node(T90_VEC_C, T90_KEY_C, T90_ID_C);
    add_node(T90_VEC_D, T90_KEY_D, T90_ID_D);
    add_node(T90_VEC_E, T90_KEY_E, T90_ID_E);
    add_edge(T90_ID_A, T90_ID_B, "t90.e.ab");
    add_edge(T90_ID_A, T90_ID_C, "t90.e.ac");
    add_edge(T90_ID_A, T90_ID_D, "t90.e.ad");
    add_edge(T90_ID_A, T90_ID_E, "t90.e.ae");

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nhexatetraactc,   u64::MAX, "k14: NHEXATETRAACTC=SAT");
    assert_eq!(nhhexatetraactc,  u64::MAX, "k14: NHHEXATETRAACTC=SAT");
    assert_eq!(nbgso,            u64::MAX, "k14: NBGSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// S(A)=2, S(B)=3, S(C)=3, S(D)=2. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);
    add_node(T90_VEC_C, T90_KEY_C, T90_ID_C);
    add_node(T90_VEC_D, T90_KEY_D, T90_ID_D);
    add_edge(T90_ID_A, T90_ID_B, "t90.e.ab");
    add_edge(T90_ID_B, T90_ID_C, "t90.e.bc");
    add_edge(T90_ID_C, T90_ID_D, "t90.e.cd");

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nhexatetraactc,   u64::MAX, "p4: NHEXATETRAACTC=SAT");
    assert_eq!(nhhexatetraactc,  u64::MAX, "p4: NHHEXATETRAACTC=SAT");
    assert_eq!(nbgso,            u64::MAX, "p4: NBGSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);
    add_node(T90_VEC_C, T90_KEY_C, T90_ID_C);
    add_node(T90_VEC_D, T90_KEY_D, T90_ID_D);
    add_edge(T90_ID_A, T90_ID_B, "t90.e.ab");
    add_edge(T90_ID_A, T90_ID_C, "t90.e.ac");
    add_edge(T90_ID_A, T90_ID_D, "t90.e.ad");
    add_edge(T90_ID_B, T90_ID_C, "t90.e.bc");
    add_edge(T90_ID_B, T90_ID_D, "t90.e.bd");
    add_edge(T90_ID_C, T90_ID_D, "t90.e.cd");

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nhexatetraactc,   u64::MAX, "k4: NHEXATETRAACTC=SAT");
    assert_eq!(nhhexatetraactc,  u64::MAX, "k4: NHHEXATETRAACTC=SAT");
    assert_eq!(nbgso,            u64::MAX, "k4: NBGSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,                2, "isolated: node_count=2");
    assert_eq!(ec,                0, "isolated: edge_count=0");
    assert_eq!(nhexatetraactc,    0, "isolated: NHEXATETRAACTC=0");
    assert_eq!(nhhexatetraactc,   0, "isolated: NHHEXATETRAACTC=0");
    assert_eq!(nbgso,             0, "isolated: NBGSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform, 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T90_VEC_A, T90_KEY_A, T90_ID_A);
    add_node(T90_VEC_B, T90_KEY_B, T90_ID_B);
    add_node(T90_VEC_C, T90_KEY_C, T90_ID_C);
    add_node(T90_VEC_D, T90_KEY_D, T90_ID_D);
    add_node(T90_VEC_E, T90_KEY_E, T90_ID_E);
    add_edge(T90_ID_A, T90_ID_C, "t90.e.ac");
    add_edge(T90_ID_A, T90_ID_D, "t90.e.ad");
    add_edge(T90_ID_A, T90_ID_E, "t90.e.ae");
    add_edge(T90_ID_B, T90_ID_C, "t90.e.bc");
    add_edge(T90_ID_B, T90_ID_D, "t90.e.bd");
    add_edge(T90_ID_B, T90_ID_E, "t90.e.be");

    let (nhexatetraactc, nhhexatetraactc, nbgso, ec, nc) = gos_runtime::graph_topo_indices90();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nhexatetraactc,   u64::MAX, "k23: NHEXATETRAACTC=SAT (5\u{00d7}6^64)");
    assert_eq!(nhhexatetraactc,  u64::MAX, "k23: NHHEXATETRAACTC=SAT");
    assert_eq!(nbgso,            u64::MAX, "k23: NBGSO=SAT");
}
