// gos-graph-topo115-harness — V3.126 NOCTAENNACTC + NHOCTAENNACTC + NBFFSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices115()`:
//   Returns (noctaennactc, nhoctaennactc, nbffso, edge_count, node_count)
//   - noctaennactc  = NOCTAENNACTC(G)  = Σ_v S(v)^89                          (exact u64; S-Octaennacontic vertex sum)
//   - nhoctaennactc = NHOCTAENNACTC(G) = Σ_{uv∈E} (S_u+S_v)^88              (exact u64; S-Octaennacontic edge-sum)
//   - nbffso         = NBFFSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^83            (exact u64; S-Variant Sombor, α=166)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTAENNACTC(G) = Σ_v S(v)^89
//     S-Octaennacontic vertex sum; tenth and FINAL of the octacontic (80-89) series.
//     Extends: NOCTAOCTACTC=Σ S^88 (topo114) → NOCTAENNACTC=Σ S^89 (topo115).
//     NOCTAENNACTC = n·S^89 for S-regular.
//     Overflow: S^89 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^89 = s64 × s16 × s8 × s  (89=64+16+8+1; 9 mults).
//
//   NHOCTAENNACTC(G) = Σ_{uv∈E} (S_u+S_v)^88
//     S-Octaennacontic edge-sum; extends NHOCTAOCTACTC=Σ(S+S)^87 (topo114).
//     NHOCTAENNACTC = |E|·(2S)^88 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^88 → saturating u128 accumulator.
//     Implementation: ss^88 = ss64 × ss16 × ss8  (88=64+16+8; 8 mults total).
//
//   NBFFSO(G) = Σ_{uv∈E} (S_u²+S_v²)^83
//     S-Variant Sombor: generalised Sombor SO^α with α=166 on S-variant.
//     32nd of NB series, letters FF (after NBEESO α=164 topo114).
//     NBEESO(topo114,α=164) → NBFFSO(topo115,α=166).
//     NBFFSO = |E|·(2S²)^83 for S-regular.
//     Overflow per edge: (2×16129²)^83 → saturating u128 accumulator.
//     Implementation: s2s^83 = s2s64 × s2s16 × s2s2 × s2s  (83=64+16+2+1; 9 mults total).
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
//  Graph     NOCTAENNACTC(exact)       NHOCTAENNACTC(exact)       NBFFSO(exact)              edges  nodes
//  Empty                    0                            0               0                      0      0
//  1 node                   0                            0               0                      0      1
//  K₂                       2            u64::MAX(sat.)    u64::MAX(sat.)                       1      2
//  P₃             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  2      3
//  K₃             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  3      3
//  K_{1,4}        u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  4      5
//  P₄             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  3      4
//  K₄             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  6      4
//  2 isolated               0                            0               0                      0      2
//  K_{2,3}        u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTAENNACTC:    1^89 + 1^89 = 2. ✓
//     NHOCTAENNACTC:   (1+1)^88 = 2^88 ≈ 3.09×10^26 > u64::MAX → SATURATES. ✓
//     NBFFSO:          (1²+1²)^83 = 2^83 ≈ 9.67×10^24 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTAENNACTC:    3×2^89 >> u64::MAX → SATURATES. ✓
//     NHOCTAENNACTC:   2×(4)^88 → SATURATES. ✓
//     NBFFSO:          2×(8)^83 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTAENNACTC:    3×4^89 → SATURATES. ✓
//     NHOCTAENNACTC:   3×8^88 → SATURATES. ✓
//     NBFFSO:          3×32^83 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTAENNACTC:    5×4^89 → SATURATES. ✓
//     NHOCTAENNACTC:   4×8^88 → SATURATES. ✓
//     NBFFSO:          4×32^83 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTAENNACTC:    2×2^89 + 2×3^89. 3^89 >> u64::MAX → SATURATES. ✓
//     NHOCTAENNACTC:   5^88+6^88+5^88 → SATURATES. ✓
//     NBFFSO:          13^83+18^83+13^83 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTAENNACTC:    4×9^89 → SATURATES. ✓
//     NHOCTAENNACTC:   6×18^88 → SATURATES. ✓
//     NBFFSO:          6×162^83 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTAENNACTC:    5×6^89 → SATURATES. ✓
//     NHOCTAENNACTC:   6×12^88 → SATURATES. ✓
//     NBFFSO:          6×72^83 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTAENNACTC  = n·S^89                                                                       for S-regular ✓
//   NHOCTAENNACTC = |E|·(2S)^88 (saturates for |E|≥1,S≥1)                                      for S-regular ✓
//   NBFFSO        = |E|·(2S²)^83                                                                 for S-regular ✓
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

const T115_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX115");
const T115_EXEC:   ExecutorId = ExecutorId::from_ascii("t115.exec");

const T115_KEY_A: &str = "t115.alpha";
const T115_KEY_B: &str = "t115.beta";
const T115_KEY_C: &str = "t115.gamma";
const T115_KEY_D: &str = "t115.delta";
const T115_KEY_E: &str = "t115.epsilon";

const T115_ID_A: NodeId = derive_node_id(T115_PLUGIN, T115_KEY_A);
const T115_ID_B: NodeId = derive_node_id(T115_PLUGIN, T115_KEY_B);
const T115_ID_C: NodeId = derive_node_id(T115_PLUGIN, T115_KEY_C);
const T115_ID_D: NodeId = derive_node_id(T115_PLUGIN, T115_KEY_D);
const T115_ID_E: NodeId = derive_node_id(T115_PLUGIN, T115_KEY_E);

// L4=202 namespace for this harness.
const T115_VEC_A: VectorAddress = VectorAddress::new(202, 1, 1, 0);
const T115_VEC_B: VectorAddress = VectorAddress::new(202, 1, 2, 0);
const T115_VEC_C: VectorAddress = VectorAddress::new(202, 1, 3, 0);
const T115_VEC_D: VectorAddress = VectorAddress::new(202, 2, 1, 0);
const T115_VEC_E: VectorAddress = VectorAddress::new(202, 2, 2, 0);

const T115_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T115_PLUGIN,
    name:         "kl-graph-topo115-harness",
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
        executor_id:       T115_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T115_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T115_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(noctaennactc,   0, "empty: NOCTAENNACTC=0");
    assert_eq!(nhoctaennactc,  0, "empty: NHOCTAENNACTC=0");
    assert_eq!(nbffso,         0, "empty: NBFFSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(noctaennactc,   0, "single: NOCTAENNACTC=0");
    assert_eq!(nhoctaennactc,  0, "single: NHOCTAENNACTC=0");
    assert_eq!(nbffso,         0, "single: NBFFSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTAENNACTC:    1^89 + 1^89 = 2.
// NHOCTAENNACTC:   (1+1)^88 = 2^88 > u64::MAX → SATURATES.
// NBFFSO:          (1²+1²)^83 = 2^83 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);
    add_edge(T115_ID_A, T115_ID_B, "t115.e.ab");

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              2,        "k2: node_count=2");
    assert_eq!(ec,              1,        "k2: edge_count=1");
    assert_eq!(noctaennactc,   2,        "k2: NOCTAENNACTC=2 (1^89+1^89=2)");
    assert_eq!(nhoctaennactc,  u64::MAX, "k2: NHOCTAENNACTC=SAT (2^88>u64::MAX)");
    assert_eq!(nbffso,         u64::MAX, "k2: NBFFSO=SAT (2^83>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);
    add_node(T115_VEC_C, T115_KEY_C, T115_ID_C);
    add_edge(T115_ID_A, T115_ID_B, "t115.e.ab");
    add_edge(T115_ID_B, T115_ID_C, "t115.e.bc");

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              3,        "p3: node_count=3");
    assert_eq!(ec,              2,        "p3: edge_count=2");
    assert_eq!(noctaennactc,   u64::MAX, "p3: NOCTAENNACTC=SAT (3\u{00d7}2^89>u64)");
    assert_eq!(nhoctaennactc,  u64::MAX, "p3: NHOCTAENNACTC=SAT (4^88>u64)");
    assert_eq!(nbffso,         u64::MAX, "p3: NBFFSO=SAT (8^83>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);
    add_node(T115_VEC_C, T115_KEY_C, T115_ID_C);
    add_edge(T115_ID_A, T115_ID_B, "t115.e.ab");
    add_edge(T115_ID_B, T115_ID_C, "t115.e.bc");
    add_edge(T115_ID_C, T115_ID_A, "t115.e.ca");

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(noctaennactc,   u64::MAX, "k3: NOCTAENNACTC=SAT");
    assert_eq!(nhoctaennactc,  u64::MAX, "k3: NHOCTAENNACTC=SAT");
    assert_eq!(nbffso,         u64::MAX, "k3: NBFFSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A); // hub
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);
    add_node(T115_VEC_C, T115_KEY_C, T115_ID_C);
    add_node(T115_VEC_D, T115_KEY_D, T115_ID_D);
    add_node(T115_VEC_E, T115_KEY_E, T115_ID_E);
    add_edge(T115_ID_A, T115_ID_B, "t115.e.ab");
    add_edge(T115_ID_A, T115_ID_C, "t115.e.ac");
    add_edge(T115_ID_A, T115_ID_D, "t115.e.ad");
    add_edge(T115_ID_A, T115_ID_E, "t115.e.ae");

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(noctaennactc,   u64::MAX, "k14: NOCTAENNACTC=SAT");
    assert_eq!(nhoctaennactc,  u64::MAX, "k14: NHOCTAENNACTC=SAT");
    assert_eq!(nbffso,         u64::MAX, "k14: NBFFSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);
    add_node(T115_VEC_C, T115_KEY_C, T115_ID_C);
    add_node(T115_VEC_D, T115_KEY_D, T115_ID_D);
    add_edge(T115_ID_A, T115_ID_B, "t115.e.ab");
    add_edge(T115_ID_B, T115_ID_C, "t115.e.bc");
    add_edge(T115_ID_C, T115_ID_D, "t115.e.cd");

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(noctaennactc,   u64::MAX, "p4: NOCTAENNACTC=SAT");
    assert_eq!(nhoctaennactc,  u64::MAX, "p4: NHOCTAENNACTC=SAT");
    assert_eq!(nbffso,         u64::MAX, "p4: NBFFSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);
    add_node(T115_VEC_C, T115_KEY_C, T115_ID_C);
    add_node(T115_VEC_D, T115_KEY_D, T115_ID_D);
    add_edge(T115_ID_A, T115_ID_B, "t115.e.ab");
    add_edge(T115_ID_A, T115_ID_C, "t115.e.ac");
    add_edge(T115_ID_A, T115_ID_D, "t115.e.ad");
    add_edge(T115_ID_B, T115_ID_C, "t115.e.bc");
    add_edge(T115_ID_B, T115_ID_D, "t115.e.bd");
    add_edge(T115_ID_C, T115_ID_D, "t115.e.cd");

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(noctaennactc,   u64::MAX, "k4: NOCTAENNACTC=SAT");
    assert_eq!(nhoctaennactc,  u64::MAX, "k4: NHOCTAENNACTC=SAT");
    assert_eq!(nbffso,         u64::MAX, "k4: NBFFSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              2, "2iso: node_count=2");
    assert_eq!(ec,              0, "2iso: edge_count=0");
    assert_eq!(noctaennactc,   0, "2iso: NOCTAENNACTC=0");
    assert_eq!(nhoctaennactc,  0, "2iso: NHOCTAENNACTC=0");
    assert_eq!(nbffso,         0, "2iso: NBFFSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T115_VEC_A, T115_KEY_A, T115_ID_A);
    add_node(T115_VEC_B, T115_KEY_B, T115_ID_B);
    add_node(T115_VEC_C, T115_KEY_C, T115_ID_C);
    add_node(T115_VEC_D, T115_KEY_D, T115_ID_D);
    add_node(T115_VEC_E, T115_KEY_E, T115_ID_E);
    add_edge(T115_ID_A, T115_ID_C, "t115.e.ac");
    add_edge(T115_ID_A, T115_ID_D, "t115.e.ad");
    add_edge(T115_ID_A, T115_ID_E, "t115.e.ae");
    add_edge(T115_ID_B, T115_ID_C, "t115.e.bc");
    add_edge(T115_ID_B, T115_ID_D, "t115.e.bd");
    add_edge(T115_ID_B, T115_ID_E, "t115.e.be");

    let (noctaennactc, nhoctaennactc, nbffso, ec, nc) = gos_runtime::graph_topo_indices115();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(noctaennactc,   u64::MAX, "k23: NOCTAENNACTC=SAT");
    assert_eq!(nhoctaennactc,  u64::MAX, "k23: NHOCTAENNACTC=SAT");
    assert_eq!(nbffso,         u64::MAX, "k23: NBFFSO=SAT");
}
