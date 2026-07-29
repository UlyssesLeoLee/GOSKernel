// gos-graph-topo109-harness — V3.120 NOCTATRIACTC + NHOCTATRIACTC + NBZSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices109()`:
//   Returns (noctatriactc, nhoctatriactc, nbzso, edge_count, node_count)
//   - noctatriactc  = NOCTATRIACTC(G)  = Σ_v S(v)^83                          (exact u64; S-Octatricontic vertex sum)
//   - nhoctatriactc = NHOCTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^82              (exact u64; S-Octatricontic edge-sum)
//   - nbzso          = NBZSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^77            (exact u64; S-Variant Sombor, α=154)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTATRIACTC(G) = Σ_v S(v)^83
//     S-Octatricontic vertex sum; fourth of the octacontic (80-89) series.
//     Extends: NOCTADIACTC=Σ S^82 (topo108) → NOCTATRIACTC=Σ S^83 (topo109).
//     NOCTATRIACTC = n·S^83 for S-regular.
//     Overflow: S^83 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^83 = s64 × s16 × s2 × s  (83=64+16+2+1; 9 mults).
//
//   NHOCTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^82
//     S-Octatricontic edge-sum; extends NHOCTADIACTC=Σ(S+S)^81 (topo108).
//     NHOCTATRIACTC = |E|·(2S)^82 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^82 → saturating u128 accumulator.
//     Implementation: ss^82 = ss64 × ss16 × ss2  (82=64+16+2; 8 mults total).
//
//   NBZSO(G) = Σ_{uv∈E} (S_u²+S_v²)^77
//     S-Variant Sombor: generalised Sombor SO^α with α=154 on S-variant.
//     26th of NB series, letter Z (after NBYSO α=152 topo108).
//     NBYSO(topo108,α=152) → NBZSO(topo109,α=154).
//     NBZSO = |E|·(2S²)^77 for S-regular.
//     Overflow per edge: (2×16129²)^77 → saturating u128 accumulator.
//     Implementation: s2s^77 = s2s64 × s2s8 × s2s4 × s2s  (77=64+8+4+1; 9 mults total).
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
//  Graph     NOCTATRIACTC(exact)        NHOCTATRIACTC(exact)       NBZSO(exact)               edges  nodes
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
//     NOCTATRIACTC:   1^83 + 1^83 = 2. ✓
//     NHOCTATRIACTC:  (1+1)^82 = 2^82 ≈ 4.84×10^24 > u64::MAX → SATURATES. ✓
//     NBZSO:          (1²+1²)^77 = 2^77 ≈ 1.51×10^23 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTATRIACTC:   3×2^83 >> u64::MAX → SATURATES. ✓
//     NHOCTATRIACTC:  2×(4)^82 → SATURATES. ✓
//     NBZSO:          2×(8)^77 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTATRIACTC:   3×4^83 → SATURATES. ✓
//     NHOCTATRIACTC:  3×8^82 → SATURATES. ✓
//     NBZSO:          3×32^77 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTATRIACTC:   5×4^83 → SATURATES. ✓
//     NHOCTATRIACTC:  4×8^82 → SATURATES. ✓
//     NBZSO:          4×32^77 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTATRIACTC:   2×2^83 + 2×3^83. 3^83 >> u64::MAX → SATURATES. ✓
//     NHOCTATRIACTC:  5^82+6^82+5^82 → SATURATES. ✓
//     NBZSO:          13^77+18^77+13^77 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTATRIACTC:   4×9^83 → SATURATES. ✓
//     NHOCTATRIACTC:  6×18^82 → SATURATES. ✓
//     NBZSO:          6×162^77 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTATRIACTC:   5×6^83 → SATURATES. ✓
//     NHOCTATRIACTC:  6×12^82 → SATURATES. ✓
//     NBZSO:          6×72^77 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTATRIACTC  = n·S^83                                                                      for S-regular ✓
//   NHOCTATRIACTC = |E|·(2S)^82 (saturates for |E|≥1,S≥1)                                     for S-regular ✓
//   NBZSO         = |E|·(2S²)^77                                                                for S-regular ✓
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

const T109_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX109");
const T109_EXEC:   ExecutorId = ExecutorId::from_ascii("t109.exec");

const T109_KEY_A: &str = "t109.alpha";
const T109_KEY_B: &str = "t109.beta";
const T109_KEY_C: &str = "t109.gamma";
const T109_KEY_D: &str = "t109.delta";
const T109_KEY_E: &str = "t109.epsilon";

const T109_ID_A: NodeId = derive_node_id(T109_PLUGIN, T109_KEY_A);
const T109_ID_B: NodeId = derive_node_id(T109_PLUGIN, T109_KEY_B);
const T109_ID_C: NodeId = derive_node_id(T109_PLUGIN, T109_KEY_C);
const T109_ID_D: NodeId = derive_node_id(T109_PLUGIN, T109_KEY_D);
const T109_ID_E: NodeId = derive_node_id(T109_PLUGIN, T109_KEY_E);

// L4=196 namespace for this harness.
const T109_VEC_A: VectorAddress = VectorAddress::new(196, 1, 1, 0);
const T109_VEC_B: VectorAddress = VectorAddress::new(196, 1, 2, 0);
const T109_VEC_C: VectorAddress = VectorAddress::new(196, 1, 3, 0);
const T109_VEC_D: VectorAddress = VectorAddress::new(196, 2, 1, 0);
const T109_VEC_E: VectorAddress = VectorAddress::new(196, 2, 2, 0);

const T109_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T109_PLUGIN,
    name:         "kl-graph-topo109-harness",
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
        executor_id:       T109_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T109_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T109_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(noctatriactc,   0, "empty: NOCTATRIACTC=0");
    assert_eq!(nhoctatriactc,  0, "empty: NHOCTATRIACTC=0");
    assert_eq!(nbzso,          0, "empty: NBZSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(noctatriactc,   0, "single: NOCTATRIACTC=0");
    assert_eq!(nhoctatriactc,  0, "single: NHOCTATRIACTC=0");
    assert_eq!(nbzso,          0, "single: NBZSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTATRIACTC:   1^83 + 1^83 = 2.
// NHOCTATRIACTC:  (1+1)^82 = 2^82 > u64::MAX → SATURATES.
// NBZSO:          (1²+1²)^77 = 2^77 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);
    add_edge(T109_ID_A, T109_ID_B, "t109.e.ab");

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              2,        "k2: node_count=2");
    assert_eq!(ec,              1,        "k2: edge_count=1");
    assert_eq!(noctatriactc,   2,        "k2: NOCTATRIACTC=2 (1^83+1^83=2)");
    assert_eq!(nhoctatriactc,  u64::MAX, "k2: NHOCTATRIACTC=SAT (2^82>u64::MAX)");
    assert_eq!(nbzso,          u64::MAX, "k2: NBZSO=SAT (2^77>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);
    add_node(T109_VEC_C, T109_KEY_C, T109_ID_C);
    add_edge(T109_ID_A, T109_ID_B, "t109.e.ab");
    add_edge(T109_ID_B, T109_ID_C, "t109.e.bc");

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              3,        "p3: node_count=3");
    assert_eq!(ec,              2,        "p3: edge_count=2");
    assert_eq!(noctatriactc,   u64::MAX, "p3: NOCTATRIACTC=SAT (3\u{00d7}2^83>u64)");
    assert_eq!(nhoctatriactc,  u64::MAX, "p3: NHOCTATRIACTC=SAT (4^82>u64)");
    assert_eq!(nbzso,          u64::MAX, "p3: NBZSO=SAT (8^77>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);
    add_node(T109_VEC_C, T109_KEY_C, T109_ID_C);
    add_edge(T109_ID_A, T109_ID_B, "t109.e.ab");
    add_edge(T109_ID_B, T109_ID_C, "t109.e.bc");
    add_edge(T109_ID_C, T109_ID_A, "t109.e.ca");

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(noctatriactc,   u64::MAX, "k3: NOCTATRIACTC=SAT");
    assert_eq!(nhoctatriactc,  u64::MAX, "k3: NHOCTATRIACTC=SAT");
    assert_eq!(nbzso,          u64::MAX, "k3: NBZSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A); // hub
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);
    add_node(T109_VEC_C, T109_KEY_C, T109_ID_C);
    add_node(T109_VEC_D, T109_KEY_D, T109_ID_D);
    add_node(T109_VEC_E, T109_KEY_E, T109_ID_E);
    add_edge(T109_ID_A, T109_ID_B, "t109.e.ab");
    add_edge(T109_ID_A, T109_ID_C, "t109.e.ac");
    add_edge(T109_ID_A, T109_ID_D, "t109.e.ad");
    add_edge(T109_ID_A, T109_ID_E, "t109.e.ae");

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(noctatriactc,   u64::MAX, "k14: NOCTATRIACTC=SAT");
    assert_eq!(nhoctatriactc,  u64::MAX, "k14: NHOCTATRIACTC=SAT");
    assert_eq!(nbzso,          u64::MAX, "k14: NBZSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);
    add_node(T109_VEC_C, T109_KEY_C, T109_ID_C);
    add_node(T109_VEC_D, T109_KEY_D, T109_ID_D);
    add_edge(T109_ID_A, T109_ID_B, "t109.e.ab");
    add_edge(T109_ID_B, T109_ID_C, "t109.e.bc");
    add_edge(T109_ID_C, T109_ID_D, "t109.e.cd");

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(noctatriactc,   u64::MAX, "p4: NOCTATRIACTC=SAT");
    assert_eq!(nhoctatriactc,  u64::MAX, "p4: NHOCTATRIACTC=SAT");
    assert_eq!(nbzso,          u64::MAX, "p4: NBZSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);
    add_node(T109_VEC_C, T109_KEY_C, T109_ID_C);
    add_node(T109_VEC_D, T109_KEY_D, T109_ID_D);
    add_edge(T109_ID_A, T109_ID_B, "t109.e.ab");
    add_edge(T109_ID_A, T109_ID_C, "t109.e.ac");
    add_edge(T109_ID_A, T109_ID_D, "t109.e.ad");
    add_edge(T109_ID_B, T109_ID_C, "t109.e.bc");
    add_edge(T109_ID_B, T109_ID_D, "t109.e.bd");
    add_edge(T109_ID_C, T109_ID_D, "t109.e.cd");

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(noctatriactc,   u64::MAX, "k4: NOCTATRIACTC=SAT");
    assert_eq!(nhoctatriactc,  u64::MAX, "k4: NHOCTATRIACTC=SAT");
    assert_eq!(nbzso,          u64::MAX, "k4: NBZSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              2, "2iso: node_count=2");
    assert_eq!(ec,              0, "2iso: edge_count=0");
    assert_eq!(noctatriactc,   0, "2iso: NOCTATRIACTC=0");
    assert_eq!(nhoctatriactc,  0, "2iso: NHOCTATRIACTC=0");
    assert_eq!(nbzso,          0, "2iso: NBZSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T109_VEC_A, T109_KEY_A, T109_ID_A);
    add_node(T109_VEC_B, T109_KEY_B, T109_ID_B);
    add_node(T109_VEC_C, T109_KEY_C, T109_ID_C);
    add_node(T109_VEC_D, T109_KEY_D, T109_ID_D);
    add_node(T109_VEC_E, T109_KEY_E, T109_ID_E);
    add_edge(T109_ID_A, T109_ID_C, "t109.e.ac");
    add_edge(T109_ID_A, T109_ID_D, "t109.e.ad");
    add_edge(T109_ID_A, T109_ID_E, "t109.e.ae");
    add_edge(T109_ID_B, T109_ID_C, "t109.e.bc");
    add_edge(T109_ID_B, T109_ID_D, "t109.e.bd");
    add_edge(T109_ID_B, T109_ID_E, "t109.e.be");

    let (noctatriactc, nhoctatriactc, nbzso, ec, nc) = gos_runtime::graph_topo_indices109();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(noctatriactc,   u64::MAX, "k23: NOCTATRIACTC=SAT");
    assert_eq!(nhoctatriactc,  u64::MAX, "k23: NHOCTATRIACTC=SAT");
    assert_eq!(nbzso,          u64::MAX, "k23: NBZSO=SAT");
}
