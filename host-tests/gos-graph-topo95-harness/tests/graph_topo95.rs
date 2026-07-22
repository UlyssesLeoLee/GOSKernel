// gos-graph-topo95-harness — V3.106 NHEXAENNACTC + NHHEXAENNACTC + NBLSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices95()`:
//   Returns (nhexaennactc, nhhexaennactc, nblso, edge_count, node_count)
//   - nhexaennactc  = NHEXAENNACTC(G) = Σ_v S(v)^69                      (exact u64; S-Hexaennacontic vertex sum)
//   - nhhexaennactc = NHHEXAENNACTC(G) = Σ_{uv∈E} (S_u+S_v)^68           (exact u64; S-Hexaennacontic edge-sum)
//   - nblso         = NBLSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^63         (exact u64; S-Variant Sombor, α=126)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXAENNACTC(G) = Σ_v S(v)^69
//     S-Hexaennacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXAOCTACTC=Σ S⁶⁸ (topo94), NHEXAENNACTC=Σ S⁶⁹ (topo95).
//     TENTH (last) of the hexacontic (60-69) series. Completes the series.
//     NHEXAENNACTC = n·S^69 for S-regular.
//     Overflow: S^69 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^69 = s64 × s4 × s  (69=64+4+1; 8 mults total).
//
//   NHHEXAENNACTC(G) = Σ_{uv∈E} (S_u+S_v)^68
//     S-Hexaennacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXAOCTACTC=Σ(S+S)⁶⁷ (topo94),
//       NHHEXAENNACTC=Σ(S+S)⁶⁸ (topo95).
//     NHHEXAENNACTC = |E|·(2S)^68 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^68 → saturating u128 accumulator.
//     Implementation: ss^68 = ss64 × ss4  (68=64+4; 7 mults).
//
//   NBLSO(G) = Σ_{uv∈E} (S_u²+S_v²)^63
//     S-Variant Sombor: generalised Sombor SO^α with α=126 on S-variant.
//     12th of NB series, letter L (after NBKSO α=124 topo94).
//     NSO(topo21,α=1),..., NBKSO(topo94,α=124), NBLSO(topo95,α=126).
//     NBLSO = |E|·(2S²)^63 for S-regular.
//     Overflow per edge: (2×16129²)^63 → saturating u128 accumulator.
//     Implementation: s2s^63 = s2s32 × s2s16 × s2s8 × s2s4 × s2s2 × s2s  (63=32+16+8+4+2+1; 6 mults).
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
//  Graph     NHEXAENNACTC(exact)          NHHEXAENNACTC(exact)         NBLSO(exact)               edges  nodes
//  Empty                      0                             0                   0                   0      0
//  1 node                     0                             0                   0                   0      1
//  K₂                         2              u64::MAX(sat.)     9_223_372_036_854_775_808           1      2
//  P₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               2      3
//  K₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               3      3
//  K_{1,4}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               4      5
//  P₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               3      4
//  K₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               6      4
//  2 isolated                 0                             0                   0                   0      2
//  K_{2,3}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXAENNACTC:  1^69 + 1^69 = 2. ✓
//     NHHEXAENNACTC: (1+1)^68 = 2^68 = 295_147_905_179_352_825_856 > u64::MAX → SATURATES. ✓
//     NBLSO:         (1²+1²)^63 = 2^63 = 9_223_372_036_854_775_808. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXAENNACTC:  3×2^69 >> u64::MAX → SATURATES. ✓
//     NHHEXAENNACTC: 2×(4)^68 → SATURATES. ✓
//     NBLSO:         2×(8)^63 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXAENNACTC:  3×4^69 → SATURATES. ✓
//     NHHEXAENNACTC: 3×8^68 → SATURATES. ✓
//     NBLSO:         3×32^63 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXAENNACTC:  5×4^69 → SATURATES. ✓
//     NHHEXAENNACTC: 4×8^68 → SATURATES. ✓
//     NBLSO:         4×32^63 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXAENNACTC:  2×2^69 + 2×3^69. 3^69 >> u64::MAX → SATURATES. ✓
//     NHHEXAENNACTC: 5^68+6^68+5^68 → SATURATES. ✓
//     NBLSO:         13^63+18^63+13^63 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXAENNACTC:  4×9^69 → SATURATES. ✓
//     NHHEXAENNACTC: 6×18^68 → SATURATES. ✓
//     NBLSO:         6×162^63 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXAENNACTC:  5×6^69 → SATURATES. ✓
//     NHHEXAENNACTC: 6×12^68 → SATURATES. ✓
//     NBLSO:         6×72^63 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXAENNACTC  = n·S^69                                                                              for S-regular ✓
//   NHHEXAENNACTC = |E|·(2S)^68 (saturates for |E|≥1,S≥1)                                             for S-regular ✓
//   NBLSO         = |E|·(2S²)^63                                                                        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, u64::MAX, 9_223_372_036_854_775_808, 1, 2)
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

const T95_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_95");
const T95_EXEC:   ExecutorId = ExecutorId::from_ascii("t95.exec");

const T95_KEY_A: &str = "t95.alpha";
const T95_KEY_B: &str = "t95.beta";
const T95_KEY_C: &str = "t95.gamma";
const T95_KEY_D: &str = "t95.delta";
const T95_KEY_E: &str = "t95.epsilon";

const T95_ID_A: NodeId = derive_node_id(T95_PLUGIN, T95_KEY_A);
const T95_ID_B: NodeId = derive_node_id(T95_PLUGIN, T95_KEY_B);
const T95_ID_C: NodeId = derive_node_id(T95_PLUGIN, T95_KEY_C);
const T95_ID_D: NodeId = derive_node_id(T95_PLUGIN, T95_KEY_D);
const T95_ID_E: NodeId = derive_node_id(T95_PLUGIN, T95_KEY_E);

// L4=182 namespace for this harness.
const T95_VEC_A: VectorAddress = VectorAddress::new(182, 1, 1, 0);
const T95_VEC_B: VectorAddress = VectorAddress::new(182, 1, 2, 0);
const T95_VEC_C: VectorAddress = VectorAddress::new(182, 1, 3, 0);
const T95_VEC_D: VectorAddress = VectorAddress::new(182, 2, 1, 0);
const T95_VEC_E: VectorAddress = VectorAddress::new(182, 2, 2, 0);

const T95_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T95_PLUGIN,
    name:         "kl-graph-topo95-harness",
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
        executor_id:       T95_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T95_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T95_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(nhexaennactc,   0, "empty: NHEXAENNACTC=0");
    assert_eq!(nhhexaennactc,  0, "empty: NHHEXAENNACTC=0");
    assert_eq!(nblso,          0, "empty: NBLSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(nhexaennactc,   0, "single: NHEXAENNACTC=0");
    assert_eq!(nhhexaennactc,  0, "single: NHHEXAENNACTC=0");
    assert_eq!(nblso,          0, "single: NBLSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXAENNACTC:  1^69 + 1^69 = 2.
// NHHEXAENNACTC: (1+1)^68 = 2^68 = 295_147_905_179_352_825_856 > u64::MAX → SATURATES.
// NBLSO:         (1²+1²)^63 = 2^63 = 9_223_372_036_854_775_808.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);
    add_edge(T95_ID_A, T95_ID_B, "t95.e.ab");

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             2,                           "k2: node_count=2");
    assert_eq!(ec,             1,                           "k2: edge_count=1");
    assert_eq!(nhexaennactc,   2,                           "k2: NHEXAENNACTC=2 (1^69+1^69=2)");
    assert_eq!(nhhexaennactc,  u64::MAX,                    "k2: NHHEXAENNACTC=SAT (2^68>u64::MAX)");
    assert_eq!(nblso,          9_223_372_036_854_775_808,   "k2: NBLSO=9_223_372_036_854_775_808 (2^63)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);
    add_node(T95_VEC_C, T95_KEY_C, T95_ID_C);
    add_edge(T95_ID_A, T95_ID_B, "t95.e.ab");
    add_edge(T95_ID_B, T95_ID_C, "t95.e.bc");

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             3,         "p3: node_count=3");
    assert_eq!(ec,             2,         "p3: edge_count=2");
    assert_eq!(nhexaennactc,   u64::MAX,  "p3: NHEXAENNACTC=SAT (3\u{00d7}2^69>u64)");
    assert_eq!(nhhexaennactc,  u64::MAX,  "p3: NHHEXAENNACTC=SAT (4^68>u64)");
    assert_eq!(nblso,          u64::MAX,  "p3: NBLSO=SAT (8^63>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);
    add_node(T95_VEC_C, T95_KEY_C, T95_ID_C);
    add_edge(T95_ID_A, T95_ID_B, "t95.e.ab");
    add_edge(T95_ID_B, T95_ID_C, "t95.e.bc");
    add_edge(T95_ID_C, T95_ID_A, "t95.e.ca");

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(nhexaennactc,   u64::MAX, "k3: NHEXAENNACTC=SAT");
    assert_eq!(nhhexaennactc,  u64::MAX, "k3: NHHEXAENNACTC=SAT");
    assert_eq!(nblso,          u64::MAX, "k3: NBLSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A); // hub
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);
    add_node(T95_VEC_C, T95_KEY_C, T95_ID_C);
    add_node(T95_VEC_D, T95_KEY_D, T95_ID_D);
    add_node(T95_VEC_E, T95_KEY_E, T95_ID_E);
    add_edge(T95_ID_A, T95_ID_B, "t95.e.ab");
    add_edge(T95_ID_A, T95_ID_C, "t95.e.ac");
    add_edge(T95_ID_A, T95_ID_D, "t95.e.ad");
    add_edge(T95_ID_A, T95_ID_E, "t95.e.ae");

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(nhexaennactc,   u64::MAX, "k14: NHEXAENNACTC=SAT");
    assert_eq!(nhhexaennactc,  u64::MAX, "k14: NHHEXAENNACTC=SAT");
    assert_eq!(nblso,          u64::MAX, "k14: NBLSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);
    add_node(T95_VEC_C, T95_KEY_C, T95_ID_C);
    add_node(T95_VEC_D, T95_KEY_D, T95_ID_D);
    add_edge(T95_ID_A, T95_ID_B, "t95.e.ab");
    add_edge(T95_ID_B, T95_ID_C, "t95.e.bc");
    add_edge(T95_ID_C, T95_ID_D, "t95.e.cd");

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(nhexaennactc,   u64::MAX, "p4: NHEXAENNACTC=SAT");
    assert_eq!(nhhexaennactc,  u64::MAX, "p4: NHHEXAENNACTC=SAT");
    assert_eq!(nblso,          u64::MAX, "p4: NBLSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);
    add_node(T95_VEC_C, T95_KEY_C, T95_ID_C);
    add_node(T95_VEC_D, T95_KEY_D, T95_ID_D);
    add_edge(T95_ID_A, T95_ID_B, "t95.e.ab");
    add_edge(T95_ID_A, T95_ID_C, "t95.e.ac");
    add_edge(T95_ID_A, T95_ID_D, "t95.e.ad");
    add_edge(T95_ID_B, T95_ID_C, "t95.e.bc");
    add_edge(T95_ID_B, T95_ID_D, "t95.e.bd");
    add_edge(T95_ID_C, T95_ID_D, "t95.e.cd");

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(nhexaennactc,   u64::MAX, "k4: NHEXAENNACTC=SAT");
    assert_eq!(nhhexaennactc,  u64::MAX, "k4: NHHEXAENNACTC=SAT");
    assert_eq!(nblso,          u64::MAX, "k4: NBLSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             2, "2iso: node_count=2");
    assert_eq!(ec,             0, "2iso: edge_count=0");
    assert_eq!(nhexaennactc,   0, "2iso: NHEXAENNACTC=0");
    assert_eq!(nhhexaennactc,  0, "2iso: NHHEXAENNACTC=0");
    assert_eq!(nblso,          0, "2iso: NBLSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T95_VEC_A, T95_KEY_A, T95_ID_A);
    add_node(T95_VEC_B, T95_KEY_B, T95_ID_B);
    add_node(T95_VEC_C, T95_KEY_C, T95_ID_C);
    add_node(T95_VEC_D, T95_KEY_D, T95_ID_D);
    add_node(T95_VEC_E, T95_KEY_E, T95_ID_E);
    add_edge(T95_ID_A, T95_ID_C, "t95.e.ac");
    add_edge(T95_ID_A, T95_ID_D, "t95.e.ad");
    add_edge(T95_ID_A, T95_ID_E, "t95.e.ae");
    add_edge(T95_ID_B, T95_ID_C, "t95.e.bc");
    add_edge(T95_ID_B, T95_ID_D, "t95.e.bd");
    add_edge(T95_ID_B, T95_ID_E, "t95.e.be");

    let (nhexaennactc, nhhexaennactc, nblso, ec, nc) = gos_runtime::graph_topo_indices95();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(nhexaennactc,   u64::MAX, "k23: NHEXAENNACTC=SAT");
    assert_eq!(nhhexaennactc,  u64::MAX, "k23: NHHEXAENNACTC=SAT");
    assert_eq!(nblso,          u64::MAX, "k23: NBLSO=SAT");
}
