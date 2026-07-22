// gos-graph-topo106-harness — V3.117 NOCTAACTC + NHOCTAACTC + NBWSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices106()`:
//   Returns (noctaactc, nhoctaactc, nbwso, edge_count, node_count)
//   - noctaactc  = NOCTAACTC(G)  = Σ_v S(v)^80                          (exact u64; S-Octacontic vertex sum)
//   - nhoctaactc = NHOCTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^79              (exact u64; S-Octacontic edge-sum)
//   - nbwso       = NBWSO(G)      = Σ_{uv∈E} (S_u²+S_v²)^74            (exact u64; S-Variant Sombor, α=148)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTAACTC(G) = Σ_v S(v)^80
//     S-Octacontic vertex sum; first of the octacontic (80-89) series.
//     Extends: NHEPTAENNACTC=Σ S^79 (topo105) → NOCTAACTC=Σ S^80 (topo106).
//     NOCTAACTC = n·S^80 for S-regular.
//     Overflow: S^80 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^80 = s64 × s16  (80=64+16; 7 mults total).
//
//   NHOCTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^79
//     S-Octacontic edge-sum; extends NHHEPTAENNACTC=Σ(S+S)^78 (topo105).
//     NHOCTAACTC = |E|·(2S)^79 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^79 → saturating u128 accumulator.
//     Implementation: ss^79 = ss64 × ss8 × ss4 × ss2 × ss  (79=64+8+4+2+1; 10 mults total).
//
//   NBWSO(G) = Σ_{uv∈E} (S_u²+S_v²)^74
//     S-Variant Sombor: generalised Sombor SO^α with α=148 on S-variant.
//     23rd of NB series, letter W (after NBVSO α=146 topo105).
//     NBVSO(topo105,α=146) → NBWSO(topo106,α=148).
//     NBWSO = |E|·(2S²)^74 for S-regular.
//     Overflow per edge: (2×16129²)^74 → saturating u128 accumulator.
//     Implementation: s2s^74 = s2s64 × s2s8 × s2s2  (74=64+8+2; 9 mults total).
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
//  Graph     NOCTAACTC(exact)           NHOCTAACTC(exact)          NBWSO(exact)               edges  nodes
//  Empty                     0                           0                 0                     0      0
//  1 node                    0                           0                 0                     0      1
//  K₂                        2           u64::MAX(sat.)    u64::MAX(sat.)                       1      2
//  P₃             u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 2      3
//  K₃             u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 3      3
//  K_{1,4}        u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 4      5
//  P₄             u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 3      4
//  K₄             u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 6      4
//  2 isolated                0                           0                 0                     0      2
//  K_{2,3}        u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTAACTC:   1^80 + 1^80 = 2. ✓
//     NHOCTAACTC:  (1+1)^79 = 2^79 ≈ 6.04×10^23 > u64::MAX → SATURATES. ✓
//     NBWSO:       (1²+1²)^74 = 2^74 ≈ 1.89×10^22 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTAACTC:   3×2^80 >> u64::MAX → SATURATES. ✓
//     NHOCTAACTC:  2×(4)^79 → SATURATES. ✓
//     NBWSO:       2×(8)^74 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTAACTC:   3×4^80 → SATURATES. ✓
//     NHOCTAACTC:  3×8^79 → SATURATES. ✓
//     NBWSO:       3×32^74 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTAACTC:   5×4^80 → SATURATES. ✓
//     NHOCTAACTC:  4×8^79 → SATURATES. ✓
//     NBWSO:       4×32^74 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTAACTC:   2×2^80 + 2×3^80. 3^80 >> u64::MAX → SATURATES. ✓
//     NHOCTAACTC:  5^79+6^79+5^79 → SATURATES. ✓
//     NBWSO:       13^74+18^74+13^74 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTAACTC:   4×9^80 → SATURATES. ✓
//     NHOCTAACTC:  6×18^79 → SATURATES. ✓
//     NBWSO:       6×162^74 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTAACTC:   5×6^80 → SATURATES. ✓
//     NHOCTAACTC:  6×12^79 → SATURATES. ✓
//     NBWSO:       6×72^74 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTAACTC  = n·S^80                                                                         for S-regular ✓
//   NHOCTAACTC = |E|·(2S)^79 (saturates for |E|≥1,S≥1)                                        for S-regular ✓
//   NBWSO      = |E|·(2S²)^74                                                                   for S-regular ✓
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

const T106_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX106");
const T106_EXEC:   ExecutorId = ExecutorId::from_ascii("t106.exec");

const T106_KEY_A: &str = "t106.alpha";
const T106_KEY_B: &str = "t106.beta";
const T106_KEY_C: &str = "t106.gamma";
const T106_KEY_D: &str = "t106.delta";
const T106_KEY_E: &str = "t106.epsilon";

const T106_ID_A: NodeId = derive_node_id(T106_PLUGIN, T106_KEY_A);
const T106_ID_B: NodeId = derive_node_id(T106_PLUGIN, T106_KEY_B);
const T106_ID_C: NodeId = derive_node_id(T106_PLUGIN, T106_KEY_C);
const T106_ID_D: NodeId = derive_node_id(T106_PLUGIN, T106_KEY_D);
const T106_ID_E: NodeId = derive_node_id(T106_PLUGIN, T106_KEY_E);

// L4=193 namespace for this harness.
const T106_VEC_A: VectorAddress = VectorAddress::new(193, 1, 1, 0);
const T106_VEC_B: VectorAddress = VectorAddress::new(193, 1, 2, 0);
const T106_VEC_C: VectorAddress = VectorAddress::new(193, 1, 3, 0);
const T106_VEC_D: VectorAddress = VectorAddress::new(193, 2, 1, 0);
const T106_VEC_E: VectorAddress = VectorAddress::new(193, 2, 2, 0);

const T106_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T106_PLUGIN,
    name:         "kl-graph-topo106-harness",
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
        executor_id:       T106_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T106_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T106_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          0, "empty: node_count=0");
    assert_eq!(ec,          0, "empty: edge_count=0");
    assert_eq!(noctaactc,   0, "empty: NOCTAACTC=0");
    assert_eq!(nhoctaactc,  0, "empty: NHOCTAACTC=0");
    assert_eq!(nbwso,       0, "empty: NBWSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          1, "single: node_count=1");
    assert_eq!(ec,          0, "single: edge_count=0");
    assert_eq!(noctaactc,   0, "single: NOCTAACTC=0");
    assert_eq!(nhoctaactc,  0, "single: NHOCTAACTC=0");
    assert_eq!(nbwso,       0, "single: NBWSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTAACTC:   1^80 + 1^80 = 2.
// NHOCTAACTC:  (1+1)^79 = 2^79 > u64::MAX → SATURATES.
// NBWSO:       (1²+1²)^74 = 2^74 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);
    add_edge(T106_ID_A, T106_ID_B, "t106.e.ab");

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          2,        "k2: node_count=2");
    assert_eq!(ec,          1,        "k2: edge_count=1");
    assert_eq!(noctaactc,   2,        "k2: NOCTAACTC=2 (1^80+1^80=2)");
    assert_eq!(nhoctaactc,  u64::MAX, "k2: NHOCTAACTC=SAT (2^79>u64::MAX)");
    assert_eq!(nbwso,       u64::MAX, "k2: NBWSO=SAT (2^74>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);
    add_node(T106_VEC_C, T106_KEY_C, T106_ID_C);
    add_edge(T106_ID_A, T106_ID_B, "t106.e.ab");
    add_edge(T106_ID_B, T106_ID_C, "t106.e.bc");

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          3,        "p3: node_count=3");
    assert_eq!(ec,          2,        "p3: edge_count=2");
    assert_eq!(noctaactc,   u64::MAX, "p3: NOCTAACTC=SAT (3\u{00d7}2^80>u64)");
    assert_eq!(nhoctaactc,  u64::MAX, "p3: NHOCTAACTC=SAT (4^79>u64)");
    assert_eq!(nbwso,       u64::MAX, "p3: NBWSO=SAT (8^74>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);
    add_node(T106_VEC_C, T106_KEY_C, T106_ID_C);
    add_edge(T106_ID_A, T106_ID_B, "t106.e.ab");
    add_edge(T106_ID_B, T106_ID_C, "t106.e.bc");
    add_edge(T106_ID_C, T106_ID_A, "t106.e.ca");

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          3,        "k3: node_count=3");
    assert_eq!(ec,          3,        "k3: edge_count=3");
    assert_eq!(noctaactc,   u64::MAX, "k3: NOCTAACTC=SAT");
    assert_eq!(nhoctaactc,  u64::MAX, "k3: NHOCTAACTC=SAT");
    assert_eq!(nbwso,       u64::MAX, "k3: NBWSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A); // hub
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);
    add_node(T106_VEC_C, T106_KEY_C, T106_ID_C);
    add_node(T106_VEC_D, T106_KEY_D, T106_ID_D);
    add_node(T106_VEC_E, T106_KEY_E, T106_ID_E);
    add_edge(T106_ID_A, T106_ID_B, "t106.e.ab");
    add_edge(T106_ID_A, T106_ID_C, "t106.e.ac");
    add_edge(T106_ID_A, T106_ID_D, "t106.e.ad");
    add_edge(T106_ID_A, T106_ID_E, "t106.e.ae");

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          5,        "k14: node_count=5");
    assert_eq!(ec,          4,        "k14: edge_count=4");
    assert_eq!(noctaactc,   u64::MAX, "k14: NOCTAACTC=SAT");
    assert_eq!(nhoctaactc,  u64::MAX, "k14: NHOCTAACTC=SAT");
    assert_eq!(nbwso,       u64::MAX, "k14: NBWSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);
    add_node(T106_VEC_C, T106_KEY_C, T106_ID_C);
    add_node(T106_VEC_D, T106_KEY_D, T106_ID_D);
    add_edge(T106_ID_A, T106_ID_B, "t106.e.ab");
    add_edge(T106_ID_B, T106_ID_C, "t106.e.bc");
    add_edge(T106_ID_C, T106_ID_D, "t106.e.cd");

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          4,        "p4: node_count=4");
    assert_eq!(ec,          3,        "p4: edge_count=3");
    assert_eq!(noctaactc,   u64::MAX, "p4: NOCTAACTC=SAT");
    assert_eq!(nhoctaactc,  u64::MAX, "p4: NHOCTAACTC=SAT");
    assert_eq!(nbwso,       u64::MAX, "p4: NBWSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);
    add_node(T106_VEC_C, T106_KEY_C, T106_ID_C);
    add_node(T106_VEC_D, T106_KEY_D, T106_ID_D);
    add_edge(T106_ID_A, T106_ID_B, "t106.e.ab");
    add_edge(T106_ID_A, T106_ID_C, "t106.e.ac");
    add_edge(T106_ID_A, T106_ID_D, "t106.e.ad");
    add_edge(T106_ID_B, T106_ID_C, "t106.e.bc");
    add_edge(T106_ID_B, T106_ID_D, "t106.e.bd");
    add_edge(T106_ID_C, T106_ID_D, "t106.e.cd");

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          4,        "k4: node_count=4");
    assert_eq!(ec,          6,        "k4: edge_count=6");
    assert_eq!(noctaactc,   u64::MAX, "k4: NOCTAACTC=SAT");
    assert_eq!(nhoctaactc,  u64::MAX, "k4: NHOCTAACTC=SAT");
    assert_eq!(nbwso,       u64::MAX, "k4: NBWSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          2, "2iso: node_count=2");
    assert_eq!(ec,          0, "2iso: edge_count=0");
    assert_eq!(noctaactc,   0, "2iso: NOCTAACTC=0");
    assert_eq!(nhoctaactc,  0, "2iso: NHOCTAACTC=0");
    assert_eq!(nbwso,       0, "2iso: NBWSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T106_VEC_A, T106_KEY_A, T106_ID_A);
    add_node(T106_VEC_B, T106_KEY_B, T106_ID_B);
    add_node(T106_VEC_C, T106_KEY_C, T106_ID_C);
    add_node(T106_VEC_D, T106_KEY_D, T106_ID_D);
    add_node(T106_VEC_E, T106_KEY_E, T106_ID_E);
    add_edge(T106_ID_A, T106_ID_C, "t106.e.ac");
    add_edge(T106_ID_A, T106_ID_D, "t106.e.ad");
    add_edge(T106_ID_A, T106_ID_E, "t106.e.ae");
    add_edge(T106_ID_B, T106_ID_C, "t106.e.bc");
    add_edge(T106_ID_B, T106_ID_D, "t106.e.bd");
    add_edge(T106_ID_B, T106_ID_E, "t106.e.be");

    let (noctaactc, nhoctaactc, nbwso, ec, nc) = gos_runtime::graph_topo_indices106();
    assert_eq!(nc,          5,        "k23: node_count=5");
    assert_eq!(ec,          6,        "k23: edge_count=6");
    assert_eq!(noctaactc,   u64::MAX, "k23: NOCTAACTC=SAT");
    assert_eq!(nhoctaactc,  u64::MAX, "k23: NHOCTAACTC=SAT");
    assert_eq!(nbwso,       u64::MAX, "k23: NBWSO=SAT");
}
