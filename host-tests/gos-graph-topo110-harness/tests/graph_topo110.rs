// gos-graph-topo110-harness — V3.121 NOCTATETRAACTC + NHOCTATETRAACTC + NBAASO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices110()`:
//   Returns (noctatetraactc, nhoctatetraactc, nbaaso, edge_count, node_count)
//   - noctatetraactc  = NOCTATETRAACTC(G)  = Σ_v S(v)^84                          (exact u64; S-Octatetracontic vertex sum)
//   - nhoctatetraactc = NHOCTATETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^83              (exact u64; S-Octatetracontic edge-sum)
//   - nbaaso          = NBAASO(G)          = Σ_{uv∈E} (S_u²+S_v²)^78            (exact u64; S-Variant Sombor, α=156)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTATETRAACTC(G) = Σ_v S(v)^84
//     S-Octatetracontic vertex sum; fifth of the octacontic (80-89) series.
//     Extends: NOCTATRIACTC=Σ S^83 (topo109) → NOCTATETRAACTC=Σ S^84 (topo110).
//     NOCTATETRAACTC = n·S^84 for S-regular.
//     Overflow: S^84 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^84 = s64 × s16 × s4  (84=64+16+4; 8 mults).
//
//   NHOCTATETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^83
//     S-Octatetracontic edge-sum; extends NHOCTATRIACTC=Σ(S+S)^82 (topo109).
//     NHOCTATETRAACTC = |E|·(2S)^83 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^83 → saturating u128 accumulator.
//     Implementation: ss^83 = ss64 × ss16 × ss2 × ss  (83=64+16+2+1; 9 mults total).
//
//   NBAASO(G) = Σ_{uv∈E} (S_u²+S_v²)^78
//     S-Variant Sombor: generalised Sombor SO^α with α=156 on S-variant.
//     27th of NB series, letters AA (after NBZSO α=154 topo109).
//     NBZSO(topo109,α=154) → NBAASO(topo110,α=156).
//     NBAASO = |E|·(2S²)^78 for S-regular.
//     Overflow per edge: (2×16129²)^78 → saturating u128 accumulator.
//     Implementation: s2s^78 = s2s64 × s2s8 × s2s4 × s2s2  (78=64+8+4+2; 9 mults total).
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
//  Graph     NOCTATETRAACTC(exact)     NHOCTATETRAACTC(exact)     NBAASO(exact)              edges  nodes
//  Empty                     0                          0                 0                     0      0
//  1 node                    0                          0                 0                     0      1
//  K₂                        2          u64::MAX(sat.)    u64::MAX(sat.)                       1      2
//  P₃             u64::MAX(sat.)         u64::MAX(sat.)        u64::MAX(sat.)                  2      3
//  K₃             u64::MAX(sat.)         u64::MAX(sat.)        u64::MAX(sat.)                  3      3
//  K_{1,4}        u64::MAX(sat.)         u64::MAX(sat.)        u64::MAX(sat.)                  4      5
//  P₄             u64::MAX(sat.)         u64::MAX(sat.)        u64::MAX(sat.)                  3      4
//  K₄             u64::MAX(sat.)         u64::MAX(sat.)        u64::MAX(sat.)                  6      4
//  2 isolated                0                          0                 0                     0      2
//  K_{2,3}        u64::MAX(sat.)         u64::MAX(sat.)        u64::MAX(sat.)                  6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTATETRAACTC:   1^84 + 1^84 = 2. ✓
//     NHOCTATETRAACTC:  (1+1)^83 = 2^83 ≈ 9.67×10^24 > u64::MAX → SATURATES. ✓
//     NBAASO:           (1²+1²)^78 = 2^78 ≈ 3.02×10^23 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTATETRAACTC:   3×2^84 >> u64::MAX → SATURATES. ✓
//     NHOCTATETRAACTC:  2×(4)^83 → SATURATES. ✓
//     NBAASO:           2×(8)^78 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTATETRAACTC:   3×4^84 → SATURATES. ✓
//     NHOCTATETRAACTC:  3×8^83 → SATURATES. ✓
//     NBAASO:           3×32^78 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTATETRAACTC:   5×4^84 → SATURATES. ✓
//     NHOCTATETRAACTC:  4×8^83 → SATURATES. ✓
//     NBAASO:           4×32^78 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTATETRAACTC:   2×2^84 + 2×3^84. 3^84 >> u64::MAX → SATURATES. ✓
//     NHOCTATETRAACTC:  5^83+6^83+5^83 → SATURATES. ✓
//     NBAASO:           13^78+18^78+13^78 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTATETRAACTC:   4×9^84 → SATURATES. ✓
//     NHOCTATETRAACTC:  6×18^83 → SATURATES. ✓
//     NBAASO:           6×162^78 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTATETRAACTC:   5×6^84 → SATURATES. ✓
//     NHOCTATETRAACTC:  6×12^83 → SATURATES. ✓
//     NBAASO:           6×72^78 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTATETRAACTC  = n·S^84                                                                     for S-regular ✓
//   NHOCTATETRAACTC = |E|·(2S)^83 (saturates for |E|≥1,S≥1)                                    for S-regular ✓
//   NBAASO          = |E|·(2S²)^78                                                               for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, u64::MAX, u64::MAX, 1, 2)
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

const T110_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX110");
const T110_EXEC:   ExecutorId = ExecutorId::from_ascii("t110.exec");

const T110_KEY_A: &str = "t110.alpha";
const T110_KEY_B: &str = "t110.beta";
const T110_KEY_C: &str = "t110.gamma";
const T110_KEY_D: &str = "t110.delta";
const T110_KEY_E: &str = "t110.epsilon";

const T110_ID_A: NodeId = derive_node_id(T110_PLUGIN, T110_KEY_A);
const T110_ID_B: NodeId = derive_node_id(T110_PLUGIN, T110_KEY_B);
const T110_ID_C: NodeId = derive_node_id(T110_PLUGIN, T110_KEY_C);
const T110_ID_D: NodeId = derive_node_id(T110_PLUGIN, T110_KEY_D);
const T110_ID_E: NodeId = derive_node_id(T110_PLUGIN, T110_KEY_E);

// L4=197 namespace for this harness.
const T110_VEC_A: VectorAddress = VectorAddress::new(197, 1, 1, 0);
const T110_VEC_B: VectorAddress = VectorAddress::new(197, 1, 2, 0);
const T110_VEC_C: VectorAddress = VectorAddress::new(197, 1, 3, 0);
const T110_VEC_D: VectorAddress = VectorAddress::new(197, 2, 1, 0);
const T110_VEC_E: VectorAddress = VectorAddress::new(197, 2, 2, 0);

const T110_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T110_PLUGIN,
    name:         "kl-graph-topo110-harness",
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
        executor_id:       T110_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T110_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T110_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 0, "empty: node_count=0");
    assert_eq!(ec,                 0, "empty: edge_count=0");
    assert_eq!(noctatetraactc,    0, "empty: NOCTATETRAACTC=0");
    assert_eq!(nhoctatetraactc,   0, "empty: NHOCTATETRAACTC=0");
    assert_eq!(nbaaso,            0, "empty: NBAASO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 1, "single: node_count=1");
    assert_eq!(ec,                 0, "single: edge_count=0");
    assert_eq!(noctatetraactc,    0, "single: NOCTATETRAACTC=0");
    assert_eq!(nhoctatetraactc,   0, "single: NHOCTATETRAACTC=0");
    assert_eq!(nbaaso,            0, "single: NBAASO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTATETRAACTC:   1^84 + 1^84 = 2.
// NHOCTATETRAACTC:  (1+1)^83 = 2^83 > u64::MAX → SATURATES.
// NBAASO:           (1²+1²)^78 = 2^78 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);
    add_edge(T110_ID_A, T110_ID_B, "t110.e.ab");

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 2,        "k2: node_count=2");
    assert_eq!(ec,                 1,        "k2: edge_count=1");
    assert_eq!(noctatetraactc,    2,        "k2: NOCTATETRAACTC=2 (1^84+1^84=2)");
    assert_eq!(nhoctatetraactc,   u64::MAX, "k2: NHOCTATETRAACTC=SAT (2^83>u64::MAX)");
    assert_eq!(nbaaso,            u64::MAX, "k2: NBAASO=SAT (2^78>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);
    add_node(T110_VEC_C, T110_KEY_C, T110_ID_C);
    add_edge(T110_ID_A, T110_ID_B, "t110.e.ab");
    add_edge(T110_ID_B, T110_ID_C, "t110.e.bc");

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 3,        "p3: node_count=3");
    assert_eq!(ec,                 2,        "p3: edge_count=2");
    assert_eq!(noctatetraactc,    u64::MAX, "p3: NOCTATETRAACTC=SAT (3\u{00d7}2^84>u64)");
    assert_eq!(nhoctatetraactc,   u64::MAX, "p3: NHOCTATETRAACTC=SAT (4^83>u64)");
    assert_eq!(nbaaso,            u64::MAX, "p3: NBAASO=SAT (8^78>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);
    add_node(T110_VEC_C, T110_KEY_C, T110_ID_C);
    add_edge(T110_ID_A, T110_ID_B, "t110.e.ab");
    add_edge(T110_ID_B, T110_ID_C, "t110.e.bc");
    add_edge(T110_ID_C, T110_ID_A, "t110.e.ca");

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 3,        "k3: node_count=3");
    assert_eq!(ec,                 3,        "k3: edge_count=3");
    assert_eq!(noctatetraactc,    u64::MAX, "k3: NOCTATETRAACTC=SAT");
    assert_eq!(nhoctatetraactc,   u64::MAX, "k3: NHOCTATETRAACTC=SAT");
    assert_eq!(nbaaso,            u64::MAX, "k3: NBAASO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A); // hub
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);
    add_node(T110_VEC_C, T110_KEY_C, T110_ID_C);
    add_node(T110_VEC_D, T110_KEY_D, T110_ID_D);
    add_node(T110_VEC_E, T110_KEY_E, T110_ID_E);
    add_edge(T110_ID_A, T110_ID_B, "t110.e.ab");
    add_edge(T110_ID_A, T110_ID_C, "t110.e.ac");
    add_edge(T110_ID_A, T110_ID_D, "t110.e.ad");
    add_edge(T110_ID_A, T110_ID_E, "t110.e.ae");

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 5,        "k14: node_count=5");
    assert_eq!(ec,                 4,        "k14: edge_count=4");
    assert_eq!(noctatetraactc,    u64::MAX, "k14: NOCTATETRAACTC=SAT");
    assert_eq!(nhoctatetraactc,   u64::MAX, "k14: NHOCTATETRAACTC=SAT");
    assert_eq!(nbaaso,            u64::MAX, "k14: NBAASO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);
    add_node(T110_VEC_C, T110_KEY_C, T110_ID_C);
    add_node(T110_VEC_D, T110_KEY_D, T110_ID_D);
    add_edge(T110_ID_A, T110_ID_B, "t110.e.ab");
    add_edge(T110_ID_B, T110_ID_C, "t110.e.bc");
    add_edge(T110_ID_C, T110_ID_D, "t110.e.cd");

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 4,        "p4: node_count=4");
    assert_eq!(ec,                 3,        "p4: edge_count=3");
    assert_eq!(noctatetraactc,    u64::MAX, "p4: NOCTATETRAACTC=SAT");
    assert_eq!(nhoctatetraactc,   u64::MAX, "p4: NHOCTATETRAACTC=SAT");
    assert_eq!(nbaaso,            u64::MAX, "p4: NBAASO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);
    add_node(T110_VEC_C, T110_KEY_C, T110_ID_C);
    add_node(T110_VEC_D, T110_KEY_D, T110_ID_D);
    add_edge(T110_ID_A, T110_ID_B, "t110.e.ab");
    add_edge(T110_ID_A, T110_ID_C, "t110.e.ac");
    add_edge(T110_ID_A, T110_ID_D, "t110.e.ad");
    add_edge(T110_ID_B, T110_ID_C, "t110.e.bc");
    add_edge(T110_ID_B, T110_ID_D, "t110.e.bd");
    add_edge(T110_ID_C, T110_ID_D, "t110.e.cd");

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 4,        "k4: node_count=4");
    assert_eq!(ec,                 6,        "k4: edge_count=6");
    assert_eq!(noctatetraactc,    u64::MAX, "k4: NOCTATETRAACTC=SAT");
    assert_eq!(nhoctatetraactc,   u64::MAX, "k4: NHOCTATETRAACTC=SAT");
    assert_eq!(nbaaso,            u64::MAX, "k4: NBAASO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 2, "2iso: node_count=2");
    assert_eq!(ec,                 0, "2iso: edge_count=0");
    assert_eq!(noctatetraactc,    0, "2iso: NOCTATETRAACTC=0");
    assert_eq!(nhoctatetraactc,   0, "2iso: NHOCTATETRAACTC=0");
    assert_eq!(nbaaso,            0, "2iso: NBAASO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T110_VEC_A, T110_KEY_A, T110_ID_A);
    add_node(T110_VEC_B, T110_KEY_B, T110_ID_B);
    add_node(T110_VEC_C, T110_KEY_C, T110_ID_C);
    add_node(T110_VEC_D, T110_KEY_D, T110_ID_D);
    add_node(T110_VEC_E, T110_KEY_E, T110_ID_E);
    add_edge(T110_ID_A, T110_ID_C, "t110.e.ac");
    add_edge(T110_ID_A, T110_ID_D, "t110.e.ad");
    add_edge(T110_ID_A, T110_ID_E, "t110.e.ae");
    add_edge(T110_ID_B, T110_ID_C, "t110.e.bc");
    add_edge(T110_ID_B, T110_ID_D, "t110.e.bd");
    add_edge(T110_ID_B, T110_ID_E, "t110.e.be");

    let (noctatetraactc, nhoctatetraactc, nbaaso, ec, nc) = gos_runtime::graph_topo_indices110();
    assert_eq!(nc,                 5,        "k23: node_count=5");
    assert_eq!(ec,                 6,        "k23: edge_count=6");
    assert_eq!(noctatetraactc,    u64::MAX, "k23: NOCTATETRAACTC=SAT");
    assert_eq!(nhoctatetraactc,   u64::MAX, "k23: NHOCTATETRAACTC=SAT");
    assert_eq!(nbaaso,            u64::MAX, "k23: NBAASO=SAT");
}
