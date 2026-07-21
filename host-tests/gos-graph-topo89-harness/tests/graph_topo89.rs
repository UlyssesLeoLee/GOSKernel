// gos-graph-topo89-harness — V3.100 NHEXATRIACTC + NHHEXATRIACTC + NBFSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices89()`:
//   Returns (nhexatriactc, nhhexatriactc, nbfso, edge_count, node_count)
//   - nhexatriactc  = NHEXATRIACTC(G) = Σ_v S(v)^63                   (exact u64; S-Hexatricontic vertex sum)
//   - nhhexatriactc = NHHEXATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^62        (exact u64; S-Hexatricontic edge-sum)
//   - nbfso         = NBFSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^57      (exact u64; S-Variant Sombor, α=114)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXATRIACTC(G) = Σ_v S(v)^63
//     S-Hexatricontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXADYACTC=Σ S⁶² (topo88), NHEXATRIACTC=Σ S⁶³ (topo89).
//     Fourth of the hexacontic (60-69) series.
//     NHEXATRIACTC = n·S^63 for S-regular.
//     Overflow: S^63 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^63 = s32 × s16 × s8 × s4 × s2 × s  (63=32+16+8+4+2+1; 6 mults).
//
//   NHHEXATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^62
//     S-Hexatricontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXADYACTC=Σ(S+S)⁶¹ (topo88),
//       NHHEXATRIACTC=Σ(S+S)⁶² (topo89).
//     NHHEXATRIACTC = |E|·(2S)^62 = 4611686018427387904|E|·S^62 for S-regular.
//     Overflow per edge: (2×16129)^62 → saturating u128 accumulator.
//     Implementation: ss^62 = ss32 × ss16 × ss8 × ss4 × ss2  (62=32+16+8+4+2; 5 mults).
//
//   NBFSO(G) = Σ_{uv∈E} (S_u²+S_v²)^57
//     S-Variant Sombor: generalised Sombor SO^α with α=114 on S-variant.
//     6th of NB series, letter F (after NBESO α=112 topo88).
//     NSO(topo21,α=1),..., NBESO(topo88,α=112), NBFSO(topo89,α=114).
//     NBFSO = |E|·(2S²)^57 = 144115188075855872|E|·S^114 for S-regular.
//     Overflow per edge: (2×16129²)^57 → saturating u128 accumulator.
//     Implementation: s2s^57 = s2s32 × s2s16 × s2s8 × s2s  (57=32+16+8+1; 4 mults).
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
//  Graph     NHEXATRIACTC(exact)           NHHEXATRIACTC(exact)        NBFSO(exact)          edges  nodes
//  Empty                    0                              0                    0               0      0
//  1 node                   0                              0                    0               0      1
//  K₂                       2           4_611_686_018_427_387_904  144_115_188_075_855_872      1      2
//  P₃              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)          2      3
//  K₃              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)          3      3
//  K_{1,4}         u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)          4      5
//  P₄              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)          3      4
//  K₄              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)          6      4
//  2 isolated               0                              0                    0               0      2
//  K_{2,3}         u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)          6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXATRIACTC:  1^63 + 1^63 = 2. ✓
//     NHHEXATRIACTC: (1+1)^62 = 2^62 = 4_611_686_018_427_387_904. ✓
//     NBFSO:         (1²+1²)^57 = 2^57 = 144_115_188_075_855_872. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXATRIACTC:  3×2^63 = 3×9_223_372_036_854_775_808 = 27_670_116_110_564_327_424 > u64::MAX → SATURATES. ✓
//     NHHEXATRIACTC: 2×(2+2)^62 = 2×4^62 = 2×2^124 → SATURATES. ✓
//     NBFSO:         2×(4+4)^57 = 2×8^57 = 2×2^171 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXATRIACTC:  3×4^63 = 3×2^126 → SATURATES. ✓
//     NHHEXATRIACTC: 3×8^62 → SATURATES. ✓
//     NBFSO:         3×32^57 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXATRIACTC:  5×4^63 → SATURATES. ✓
//     NHHEXATRIACTC: 4×8^62 → SATURATES. ✓
//     NBFSO:         4×32^57 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXATRIACTC:  2×2^63 + 2×3^63. 3^63 >> u64::MAX → SATURATES. ✓
//     NHHEXATRIACTC: 5^62+6^62+5^62 → each term >> u64::MAX → SATURATES. ✓
//     NBFSO:         13^57+18^57+13^57 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXATRIACTC:  4×9^63 → SATURATES. ✓
//     NHHEXATRIACTC: 6×18^62 → SATURATES. ✓
//     NBFSO:         6×162^57 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXATRIACTC:  5×6^63 → SATURATES. ✓
//     NHHEXATRIACTC: 6×12^62 → SATURATES. ✓
//     NBFSO:         6×72^57 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXATRIACTC  = n·S^63                                                                            for S-regular ✓
//   NHHEXATRIACTC = |E|·(2S)^62 = 4611686018427387904|E|·S^62                                        for S-regular ✓
//   NBFSO         = |E|·(2S²)^57 = 144115188075855872|E|·S^114                                       for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 4_611_686_018_427_387_904, 144_115_188_075_855_872, 1, 2)
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

const T89_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_89");
const T89_EXEC:   ExecutorId = ExecutorId::from_ascii("t89.exec");

const T89_KEY_A: &str = "t89.alpha";
const T89_KEY_B: &str = "t89.beta";
const T89_KEY_C: &str = "t89.gamma";
const T89_KEY_D: &str = "t89.delta";
const T89_KEY_E: &str = "t89.epsilon";

const T89_ID_A: NodeId = derive_node_id(T89_PLUGIN, T89_KEY_A);
const T89_ID_B: NodeId = derive_node_id(T89_PLUGIN, T89_KEY_B);
const T89_ID_C: NodeId = derive_node_id(T89_PLUGIN, T89_KEY_C);
const T89_ID_D: NodeId = derive_node_id(T89_PLUGIN, T89_KEY_D);
const T89_ID_E: NodeId = derive_node_id(T89_PLUGIN, T89_KEY_E);

// L4=176 namespace for this harness.
const T89_VEC_A: VectorAddress = VectorAddress::new(176, 1, 1, 0);
const T89_VEC_B: VectorAddress = VectorAddress::new(176, 1, 2, 0);
const T89_VEC_C: VectorAddress = VectorAddress::new(176, 1, 3, 0);
const T89_VEC_D: VectorAddress = VectorAddress::new(176, 2, 1, 0);
const T89_VEC_E: VectorAddress = VectorAddress::new(176, 2, 2, 0);

const T89_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T89_PLUGIN,
    name:         "kl-graph-topo89-harness",
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
        executor_id:       T89_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T89_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T89_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(nhexatriactc,    0, "empty: NHEXATRIACTC=0");
    assert_eq!(nhhexatriactc,   0, "empty: NHHEXATRIACTC=0");
    assert_eq!(nbfso,           0, "empty: NBFSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(nhexatriactc,    0, "single: NHEXATRIACTC=0");
    assert_eq!(nhhexatriactc,   0, "single: NHHEXATRIACTC=0");
    assert_eq!(nbfso,           0, "single: NBFSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXATRIACTC:  1^63+1^63 = 2.
// NHHEXATRIACTC: (1+1)^62 = 2^62 = 4_611_686_018_427_387_904.
// NBFSO:         (1²+1²)^57 = 2^57 = 144_115_188_075_855_872.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);
    add_edge(T89_ID_A, T89_ID_B, "t89.e.ab");

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,              2,                           "k2: node_count=2");
    assert_eq!(ec,              1,                           "k2: edge_count=1");
    assert_eq!(nhexatriactc,    2,                           "k2: NHEXATRIACTC=2 (1^63+1^63=2)");
    assert_eq!(nhhexatriactc,   4_611_686_018_427_387_904,   "k2: NHHEXATRIACTC=4_611_686_018_427_387_904 (2^62)");
    assert_eq!(nbfso,           144_115_188_075_855_872,     "k2: NBFSO=144_115_188_075_855_872 (2^57)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHEXATRIACTC:  3×2^63 = 27_670_116_110_564_327_424 > u64::MAX → SATURATES.
// NHHEXATRIACTC: 2×4^62 → SATURATES.
// NBFSO:         2×8^57 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);
    add_node(T89_VEC_C, T89_KEY_C, T89_ID_C);
    add_edge(T89_ID_A, T89_ID_B, "t89.e.ab");
    add_edge(T89_ID_B, T89_ID_C, "t89.e.bc");

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,              3,         "p3: node_count=3");
    assert_eq!(ec,              2,         "p3: edge_count=2");
    assert_eq!(nhexatriactc,    u64::MAX,  "p3: NHEXATRIACTC=SAT (3\u{00d7}2^63>u64)");
    assert_eq!(nhhexatriactc,   u64::MAX,  "p3: NHHEXATRIACTC=SAT (4^62>u64)");
    assert_eq!(nbfso,           u64::MAX,  "p3: NBFSO=SAT (8^57>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);
    add_node(T89_VEC_C, T89_KEY_C, T89_ID_C);
    add_edge(T89_ID_A, T89_ID_B, "t89.e.ab");
    add_edge(T89_ID_B, T89_ID_C, "t89.e.bc");
    add_edge(T89_ID_C, T89_ID_A, "t89.e.ca");

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(nhexatriactc,   u64::MAX, "k3: NHEXATRIACTC=SAT");
    assert_eq!(nhhexatriactc,  u64::MAX, "k3: NHHEXATRIACTC=SAT");
    assert_eq!(nbfso,          u64::MAX, "k3: NBFSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A); // hub
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);
    add_node(T89_VEC_C, T89_KEY_C, T89_ID_C);
    add_node(T89_VEC_D, T89_KEY_D, T89_ID_D);
    add_node(T89_VEC_E, T89_KEY_E, T89_ID_E);
    add_edge(T89_ID_A, T89_ID_B, "t89.e.ab");
    add_edge(T89_ID_A, T89_ID_C, "t89.e.ac");
    add_edge(T89_ID_A, T89_ID_D, "t89.e.ad");
    add_edge(T89_ID_A, T89_ID_E, "t89.e.ae");

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(nhexatriactc,   u64::MAX, "k14: NHEXATRIACTC=SAT");
    assert_eq!(nhhexatriactc,  u64::MAX, "k14: NHHEXATRIACTC=SAT");
    assert_eq!(nbfso,          u64::MAX, "k14: NBFSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NHEXATRIACTC:  2×2^63 + 2×3^63. 3^63 >> u64::MAX → SATURATES.
// NHHEXATRIACTC: 5^62+6^62+5^62 → SATURATES.
// NBFSO:         13^57+18^57+13^57 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);
    add_node(T89_VEC_C, T89_KEY_C, T89_ID_C);
    add_node(T89_VEC_D, T89_KEY_D, T89_ID_D);
    add_edge(T89_ID_A, T89_ID_B, "t89.e.ab");
    add_edge(T89_ID_B, T89_ID_C, "t89.e.bc");
    add_edge(T89_ID_C, T89_ID_D, "t89.e.cd");

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(nhexatriactc,   u64::MAX, "p4: NHEXATRIACTC=SAT");
    assert_eq!(nhhexatriactc,  u64::MAX, "p4: NHHEXATRIACTC=SAT");
    assert_eq!(nbfso,          u64::MAX, "p4: NBFSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);
    add_node(T89_VEC_C, T89_KEY_C, T89_ID_C);
    add_node(T89_VEC_D, T89_KEY_D, T89_ID_D);
    add_edge(T89_ID_A, T89_ID_B, "t89.e.ab");
    add_edge(T89_ID_A, T89_ID_C, "t89.e.ac");
    add_edge(T89_ID_A, T89_ID_D, "t89.e.ad");
    add_edge(T89_ID_B, T89_ID_C, "t89.e.bc");
    add_edge(T89_ID_B, T89_ID_D, "t89.e.bd");
    add_edge(T89_ID_C, T89_ID_D, "t89.e.cd");

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(nhexatriactc,   u64::MAX, "k4: NHEXATRIACTC=SAT");
    assert_eq!(nhhexatriactc,  u64::MAX, "k4: NHHEXATRIACTC=SAT");
    assert_eq!(nbfso,          u64::MAX, "k4: NBFSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,              2, "isolated: node_count=2");
    assert_eq!(ec,              0, "isolated: edge_count=0");
    assert_eq!(nhexatriactc,    0, "isolated: NHEXATRIACTC=0");
    assert_eq!(nhhexatriactc,   0, "isolated: NHHEXATRIACTC=0");
    assert_eq!(nbfso,           0, "isolated: NBFSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform → all saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T89_VEC_A, T89_KEY_A, T89_ID_A);
    add_node(T89_VEC_B, T89_KEY_B, T89_ID_B);
    add_node(T89_VEC_C, T89_KEY_C, T89_ID_C);
    add_node(T89_VEC_D, T89_KEY_D, T89_ID_D);
    add_node(T89_VEC_E, T89_KEY_E, T89_ID_E);
    add_edge(T89_ID_A, T89_ID_C, "t89.e.ac");
    add_edge(T89_ID_A, T89_ID_D, "t89.e.ad");
    add_edge(T89_ID_A, T89_ID_E, "t89.e.ae");
    add_edge(T89_ID_B, T89_ID_C, "t89.e.bc");
    add_edge(T89_ID_B, T89_ID_D, "t89.e.bd");
    add_edge(T89_ID_B, T89_ID_E, "t89.e.be");

    let (nhexatriactc, nhhexatriactc, nbfso, ec, nc) = gos_runtime::graph_topo_indices89();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(nhexatriactc,   u64::MAX, "k23: NHEXATRIACTC=SAT (5\u{00d7}6^63)");
    assert_eq!(nhhexatriactc,  u64::MAX, "k23: NHHEXATRIACTC=SAT");
    assert_eq!(nbfso,          u64::MAX, "k23: NBFSO=SAT");
}
