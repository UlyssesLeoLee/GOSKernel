// gos-graph-topo111-harness — V3.122 NOCTAPENTACTC + NHOCTAPENTACTC + NBBSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices111()`:
//   Returns (noctapentactc, nhoctapentactc, nbbso, edge_count, node_count)
//   - noctapentactc  = NOCTAPENTACTC(G)  = Σ_v S(v)^85                          (exact u64; S-Octapentic vertex sum)
//   - nhoctapentactc = NHOCTAPENTACTC(G) = Σ_{uv∈E} (S_u+S_v)^84              (exact u64; S-Octapentic edge-sum)
//   - nbbso          = NBBSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^79            (exact u64; S-Variant Sombor, α=158)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTAPENTACTC(G) = Σ_v S(v)^85
//     S-Octapentic vertex sum; sixth of the octacontic (80-89) series.
//     Extends: NOCTATETRAACTC=Σ S^84 (topo110) → NOCTAPENTACTC=Σ S^85 (topo111).
//     NOCTAPENTACTC = n·S^85 for S-regular.
//     Overflow: S^85 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^85 = s64 × s16 × s4 × s  (85=64+16+4+1; 9 mults).
//
//   NHOCTAPENTACTC(G) = Σ_{uv∈E} (S_u+S_v)^84
//     S-Octapentic edge-sum; extends NHOCTATETRAACTC=Σ(S+S)^83 (topo110).
//     NHOCTAPENTACTC = |E|·(2S)^84 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^84 → saturating u128 accumulator.
//     Implementation: ss^84 = ss64 × ss16 × ss4  (84=64+16+4; 8 mults total).
//
//   NBBSO(G) = Σ_{uv∈E} (S_u²+S_v²)^79
//     S-Variant Sombor: generalised Sombor SO^α with α=158 on S-variant.
//     28th of NB series, letters BB (after NBAASO α=156 topo110).
//     NBAASO(topo110,α=156) → NBBSO(topo111,α=158).
//     NBBSO = |E|·(2S²)^79 for S-regular.
//     Overflow per edge: (2×16129²)^79 → saturating u128 accumulator.
//     Implementation: s2s^79 = s2s64 × s2s8 × s2s4 × s2s2 × s2s  (79=64+8+4+2+1; 10 mults total).
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
//  Graph     NOCTAPENTACTC(exact)      NHOCTAPENTACTC(exact)      NBBSO(exact)               edges  nodes
//  Empty                     0                           0                0                     0      0
//  1 node                    0                           0                0                     0      1
//  K₂                        2           u64::MAX(sat.)    u64::MAX(sat.)                       1      2
//  P₃             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                   2      3
//  K₃             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                   3      3
//  K_{1,4}        u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                   4      5
//  P₄             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                   3      4
//  K₄             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                   6      4
//  2 isolated                0                           0                0                     0      2
//  K_{2,3}        u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                   6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTAPENTACTC:   1^85 + 1^85 = 2. ✓
//     NHOCTAPENTACTC:  (1+1)^84 = 2^84 ≈ 1.93×10^25 > u64::MAX → SATURATES. ✓
//     NBBSO:           (1²+1²)^79 = 2^79 ≈ 6.04×10^23 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTAPENTACTC:   3×2^85 >> u64::MAX → SATURATES. ✓
//     NHOCTAPENTACTC:  2×(4)^84 → SATURATES. ✓
//     NBBSO:           2×(8)^79 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTAPENTACTC:   3×4^85 → SATURATES. ✓
//     NHOCTAPENTACTC:  3×8^84 → SATURATES. ✓
//     NBBSO:           3×32^79 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTAPENTACTC:   5×4^85 → SATURATES. ✓
//     NHOCTAPENTACTC:  4×8^84 → SATURATES. ✓
//     NBBSO:           4×32^79 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTAPENTACTC:   2×2^85 + 2×3^85. 3^85 >> u64::MAX → SATURATES. ✓
//     NHOCTAPENTACTC:  5^84+6^84+5^84 → SATURATES. ✓
//     NBBSO:           13^79+18^79+13^79 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTAPENTACTC:   4×9^85 → SATURATES. ✓
//     NHOCTAPENTACTC:  6×18^84 → SATURATES. ✓
//     NBBSO:           6×162^79 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTAPENTACTC:   5×6^85 → SATURATES. ✓
//     NHOCTAPENTACTC:  6×12^84 → SATURATES. ✓
//     NBBSO:           6×72^79 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTAPENTACTC  = n·S^85                                                                      for S-regular ✓
//   NHOCTAPENTACTC = |E|·(2S)^84 (saturates for |E|≥1,S≥1)                                     for S-regular ✓
//   NBBSO          = |E|·(2S²)^79                                                                for S-regular ✓
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

const T111_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX111");
const T111_EXEC:   ExecutorId = ExecutorId::from_ascii("t111.exec");

const T111_KEY_A: &str = "t111.alpha";
const T111_KEY_B: &str = "t111.beta";
const T111_KEY_C: &str = "t111.gamma";
const T111_KEY_D: &str = "t111.delta";
const T111_KEY_E: &str = "t111.epsilon";

const T111_ID_A: NodeId = derive_node_id(T111_PLUGIN, T111_KEY_A);
const T111_ID_B: NodeId = derive_node_id(T111_PLUGIN, T111_KEY_B);
const T111_ID_C: NodeId = derive_node_id(T111_PLUGIN, T111_KEY_C);
const T111_ID_D: NodeId = derive_node_id(T111_PLUGIN, T111_KEY_D);
const T111_ID_E: NodeId = derive_node_id(T111_PLUGIN, T111_KEY_E);

// L4=198 namespace for this harness.
const T111_VEC_A: VectorAddress = VectorAddress::new(198, 1, 1, 0);
const T111_VEC_B: VectorAddress = VectorAddress::new(198, 1, 2, 0);
const T111_VEC_C: VectorAddress = VectorAddress::new(198, 1, 3, 0);
const T111_VEC_D: VectorAddress = VectorAddress::new(198, 2, 1, 0);
const T111_VEC_E: VectorAddress = VectorAddress::new(198, 2, 2, 0);

const T111_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T111_PLUGIN,
    name:         "kl-graph-topo111-harness",
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
        executor_id:       T111_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T111_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T111_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(noctapentactc,   0, "empty: NOCTAPENTACTC=0");
    assert_eq!(nhoctapentactc,  0, "empty: NHOCTAPENTACTC=0");
    assert_eq!(nbbso,           0, "empty: NBBSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(noctapentactc,   0, "single: NOCTAPENTACTC=0");
    assert_eq!(nhoctapentactc,  0, "single: NHOCTAPENTACTC=0");
    assert_eq!(nbbso,           0, "single: NBBSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTAPENTACTC:   1^85 + 1^85 = 2.
// NHOCTAPENTACTC:  (1+1)^84 = 2^84 > u64::MAX → SATURATES.
// NBBSO:           (1²+1²)^79 = 2^79 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);
    add_edge(T111_ID_A, T111_ID_B, "t111.e.ab");

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               2,        "k2: node_count=2");
    assert_eq!(ec,               1,        "k2: edge_count=1");
    assert_eq!(noctapentactc,   2,        "k2: NOCTAPENTACTC=2 (1^85+1^85=2)");
    assert_eq!(nhoctapentactc,  u64::MAX, "k2: NHOCTAPENTACTC=SAT (2^84>u64::MAX)");
    assert_eq!(nbbso,           u64::MAX, "k2: NBBSO=SAT (2^79>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);
    add_node(T111_VEC_C, T111_KEY_C, T111_ID_C);
    add_edge(T111_ID_A, T111_ID_B, "t111.e.ab");
    add_edge(T111_ID_B, T111_ID_C, "t111.e.bc");

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               3,        "p3: node_count=3");
    assert_eq!(ec,               2,        "p3: edge_count=2");
    assert_eq!(noctapentactc,   u64::MAX, "p3: NOCTAPENTACTC=SAT (3\u{00d7}2^85>u64)");
    assert_eq!(nhoctapentactc,  u64::MAX, "p3: NHOCTAPENTACTC=SAT (4^84>u64)");
    assert_eq!(nbbso,           u64::MAX, "p3: NBBSO=SAT (8^79>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);
    add_node(T111_VEC_C, T111_KEY_C, T111_ID_C);
    add_edge(T111_ID_A, T111_ID_B, "t111.e.ab");
    add_edge(T111_ID_B, T111_ID_C, "t111.e.bc");
    add_edge(T111_ID_C, T111_ID_A, "t111.e.ca");

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(noctapentactc,   u64::MAX, "k3: NOCTAPENTACTC=SAT");
    assert_eq!(nhoctapentactc,  u64::MAX, "k3: NHOCTAPENTACTC=SAT");
    assert_eq!(nbbso,           u64::MAX, "k3: NBBSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A); // hub
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);
    add_node(T111_VEC_C, T111_KEY_C, T111_ID_C);
    add_node(T111_VEC_D, T111_KEY_D, T111_ID_D);
    add_node(T111_VEC_E, T111_KEY_E, T111_ID_E);
    add_edge(T111_ID_A, T111_ID_B, "t111.e.ab");
    add_edge(T111_ID_A, T111_ID_C, "t111.e.ac");
    add_edge(T111_ID_A, T111_ID_D, "t111.e.ad");
    add_edge(T111_ID_A, T111_ID_E, "t111.e.ae");

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(noctapentactc,   u64::MAX, "k14: NOCTAPENTACTC=SAT");
    assert_eq!(nhoctapentactc,  u64::MAX, "k14: NHOCTAPENTACTC=SAT");
    assert_eq!(nbbso,           u64::MAX, "k14: NBBSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);
    add_node(T111_VEC_C, T111_KEY_C, T111_ID_C);
    add_node(T111_VEC_D, T111_KEY_D, T111_ID_D);
    add_edge(T111_ID_A, T111_ID_B, "t111.e.ab");
    add_edge(T111_ID_B, T111_ID_C, "t111.e.bc");
    add_edge(T111_ID_C, T111_ID_D, "t111.e.cd");

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(noctapentactc,   u64::MAX, "p4: NOCTAPENTACTC=SAT");
    assert_eq!(nhoctapentactc,  u64::MAX, "p4: NHOCTAPENTACTC=SAT");
    assert_eq!(nbbso,           u64::MAX, "p4: NBBSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);
    add_node(T111_VEC_C, T111_KEY_C, T111_ID_C);
    add_node(T111_VEC_D, T111_KEY_D, T111_ID_D);
    add_edge(T111_ID_A, T111_ID_B, "t111.e.ab");
    add_edge(T111_ID_A, T111_ID_C, "t111.e.ac");
    add_edge(T111_ID_A, T111_ID_D, "t111.e.ad");
    add_edge(T111_ID_B, T111_ID_C, "t111.e.bc");
    add_edge(T111_ID_B, T111_ID_D, "t111.e.bd");
    add_edge(T111_ID_C, T111_ID_D, "t111.e.cd");

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(noctapentactc,   u64::MAX, "k4: NOCTAPENTACTC=SAT");
    assert_eq!(nhoctapentactc,  u64::MAX, "k4: NHOCTAPENTACTC=SAT");
    assert_eq!(nbbso,           u64::MAX, "k4: NBBSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               2, "2iso: node_count=2");
    assert_eq!(ec,               0, "2iso: edge_count=0");
    assert_eq!(noctapentactc,   0, "2iso: NOCTAPENTACTC=0");
    assert_eq!(nhoctapentactc,  0, "2iso: NHOCTAPENTACTC=0");
    assert_eq!(nbbso,           0, "2iso: NBBSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T111_VEC_A, T111_KEY_A, T111_ID_A);
    add_node(T111_VEC_B, T111_KEY_B, T111_ID_B);
    add_node(T111_VEC_C, T111_KEY_C, T111_ID_C);
    add_node(T111_VEC_D, T111_KEY_D, T111_ID_D);
    add_node(T111_VEC_E, T111_KEY_E, T111_ID_E);
    add_edge(T111_ID_A, T111_ID_C, "t111.e.ac");
    add_edge(T111_ID_A, T111_ID_D, "t111.e.ad");
    add_edge(T111_ID_A, T111_ID_E, "t111.e.ae");
    add_edge(T111_ID_B, T111_ID_C, "t111.e.bc");
    add_edge(T111_ID_B, T111_ID_D, "t111.e.bd");
    add_edge(T111_ID_B, T111_ID_E, "t111.e.be");

    let (noctapentactc, nhoctapentactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices111();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(noctapentactc,   u64::MAX, "k23: NOCTAPENTACTC=SAT");
    assert_eq!(nhoctapentactc,  u64::MAX, "k23: NHOCTAPENTACTC=SAT");
    assert_eq!(nbbso,           u64::MAX, "k23: NBBSO=SAT");
}
