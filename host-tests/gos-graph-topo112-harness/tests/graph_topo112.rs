// gos-graph-topo112-harness — V3.123 NOCTAHEXACTC + NHOCTAHEXACTC + NBCCSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices112()`:
//   Returns (noctahexactc, nhoctahexactc, nbccso, edge_count, node_count)
//   - noctahexactc  = NOCTAHEXACTC(G)  = Σ_v S(v)^86                          (exact u64; S-Octahexic vertex sum)
//   - nhoctahexactc = NHOCTAHEXACTC(G) = Σ_{uv∈E} (S_u+S_v)^85              (exact u64; S-Octahexic edge-sum)
//   - nbccso        = NBCCSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^80            (exact u64; S-Variant Sombor, α=160)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTAHEXACTC(G) = Σ_v S(v)^86
//     S-Octahexic vertex sum; seventh of the octacontic (80-89) series.
//     Extends: NOCTAPENTACTC=Σ S^85 (topo111) → NOCTAHEXACTC=Σ S^86 (topo112).
//     NOCTAHEXACTC = n·S^86 for S-regular.
//     Overflow: S^86 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^86 = s64 × s16 × s4 × s2  (86=64+16+4+2; 9 mults).
//
//   NHOCTAHEXACTC(G) = Σ_{uv∈E} (S_u+S_v)^85
//     S-Octahexic edge-sum; extends NHOCTAPENTACTC=Σ(S+S)^84 (topo111).
//     NHOCTAHEXACTC = |E|·(2S)^85 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^85 → saturating u128 accumulator.
//     Implementation: ss^85 = ss64 × ss16 × ss4 × ss  (85=64+16+4+1; 9 mults total).
//
//   NBCCSO(G) = Σ_{uv∈E} (S_u²+S_v²)^80
//     S-Variant Sombor: generalised Sombor SO^α with α=160 on S-variant.
//     29th of NB series, letters CC (after NBBSO α=158 topo111).
//     NBBSO(topo111,α=158) → NBCCSO(topo112,α=160).
//     NBCCSO = |E|·(2S²)^80 for S-regular.
//     Overflow per edge: (2×16129²)^80 → saturating u128 accumulator.
//     Implementation: s2s^80 = s2s64 × s2s16  (80=64+16; 7 mults total).
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
//  Graph     NOCTAHEXACTC(exact)       NHOCTAHEXACTC(exact)       NBCCSO(exact)              edges  nodes
//  Empty                    0                           0               0                      0      0
//  1 node                   0                           0               0                      0      1
//  K₂                       2           u64::MAX(sat.)    u64::MAX(sat.)                       1      2
//  P₃             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                  2      3
//  K₃             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                  3      3
//  K_{1,4}        u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                  4      5
//  P₄             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                  3      4
//  K₄             u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                  6      4
//  2 isolated               0                           0               0                      0      2
//  K_{2,3}        u64::MAX(sat.)          u64::MAX(sat.)       u64::MAX(sat.)                  6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTAHEXACTC:   1^86 + 1^86 = 2. ✓
//     NHOCTAHEXACTC:  (1+1)^85 = 2^85 ≈ 3.87×10^25 > u64::MAX → SATURATES. ✓
//     NBCCSO:         (1²+1²)^80 = 2^80 ≈ 1.21×10^24 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTAHEXACTC:   3×2^86 >> u64::MAX → SATURATES. ✓
//     NHOCTAHEXACTC:  2×(4)^85 → SATURATES. ✓
//     NBCCSO:         2×(8)^80 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTAHEXACTC:   3×4^86 → SATURATES. ✓
//     NHOCTAHEXACTC:  3×8^85 → SATURATES. ✓
//     NBCCSO:         3×32^80 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTAHEXACTC:   5×4^86 → SATURATES. ✓
//     NHOCTAHEXACTC:  4×8^85 → SATURATES. ✓
//     NBCCSO:         4×32^80 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTAHEXACTC:   2×2^86 + 2×3^86. 3^86 >> u64::MAX → SATURATES. ✓
//     NHOCTAHEXACTC:  5^85+6^85+5^85 → SATURATES. ✓
//     NBCCSO:         13^80+18^80+13^80 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTAHEXACTC:   4×9^86 → SATURATES. ✓
//     NHOCTAHEXACTC:  6×18^85 → SATURATES. ✓
//     NBCCSO:         6×162^80 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTAHEXACTC:   5×6^86 → SATURATES. ✓
//     NHOCTAHEXACTC:  6×12^85 → SATURATES. ✓
//     NBCCSO:         6×72^80 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTAHEXACTC  = n·S^86                                                                      for S-regular ✓
//   NHOCTAHEXACTC = |E|·(2S)^85 (saturates for |E|≥1,S≥1)                                     for S-regular ✓
//   NBCCSO        = |E|·(2S²)^80                                                                for S-regular ✓
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

const T112_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX112");
const T112_EXEC:   ExecutorId = ExecutorId::from_ascii("t112.exec");

const T112_KEY_A: &str = "t112.alpha";
const T112_KEY_B: &str = "t112.beta";
const T112_KEY_C: &str = "t112.gamma";
const T112_KEY_D: &str = "t112.delta";
const T112_KEY_E: &str = "t112.epsilon";

const T112_ID_A: NodeId = derive_node_id(T112_PLUGIN, T112_KEY_A);
const T112_ID_B: NodeId = derive_node_id(T112_PLUGIN, T112_KEY_B);
const T112_ID_C: NodeId = derive_node_id(T112_PLUGIN, T112_KEY_C);
const T112_ID_D: NodeId = derive_node_id(T112_PLUGIN, T112_KEY_D);
const T112_ID_E: NodeId = derive_node_id(T112_PLUGIN, T112_KEY_E);

// L4=199 namespace for this harness.
const T112_VEC_A: VectorAddress = VectorAddress::new(199, 1, 1, 0);
const T112_VEC_B: VectorAddress = VectorAddress::new(199, 1, 2, 0);
const T112_VEC_C: VectorAddress = VectorAddress::new(199, 1, 3, 0);
const T112_VEC_D: VectorAddress = VectorAddress::new(199, 2, 1, 0);
const T112_VEC_E: VectorAddress = VectorAddress::new(199, 2, 2, 0);

const T112_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T112_PLUGIN,
    name:         "kl-graph-topo112-harness",
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
        executor_id:       T112_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T112_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T112_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             0, "empty: node_count=0");
    assert_eq!(ec,             0, "empty: edge_count=0");
    assert_eq!(noctahexactc,  0, "empty: NOCTAHEXACTC=0");
    assert_eq!(nhoctahexactc, 0, "empty: NHOCTAHEXACTC=0");
    assert_eq!(nbccso,        0, "empty: NBCCSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             1, "single: node_count=1");
    assert_eq!(ec,             0, "single: edge_count=0");
    assert_eq!(noctahexactc,  0, "single: NOCTAHEXACTC=0");
    assert_eq!(nhoctahexactc, 0, "single: NHOCTAHEXACTC=0");
    assert_eq!(nbccso,        0, "single: NBCCSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTAHEXACTC:   1^86 + 1^86 = 2.
// NHOCTAHEXACTC:  (1+1)^85 = 2^85 > u64::MAX → SATURATES.
// NBCCSO:         (1²+1²)^80 = 2^80 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);
    add_edge(T112_ID_A, T112_ID_B, "t112.e.ab");

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             2,        "k2: node_count=2");
    assert_eq!(ec,             1,        "k2: edge_count=1");
    assert_eq!(noctahexactc,  2,        "k2: NOCTAHEXACTC=2 (1^86+1^86=2)");
    assert_eq!(nhoctahexactc, u64::MAX, "k2: NHOCTAHEXACTC=SAT (2^85>u64::MAX)");
    assert_eq!(nbccso,        u64::MAX, "k2: NBCCSO=SAT (2^80>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);
    add_node(T112_VEC_C, T112_KEY_C, T112_ID_C);
    add_edge(T112_ID_A, T112_ID_B, "t112.e.ab");
    add_edge(T112_ID_B, T112_ID_C, "t112.e.bc");

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             3,        "p3: node_count=3");
    assert_eq!(ec,             2,        "p3: edge_count=2");
    assert_eq!(noctahexactc,  u64::MAX, "p3: NOCTAHEXACTC=SAT (3\u{00d7}2^86>u64)");
    assert_eq!(nhoctahexactc, u64::MAX, "p3: NHOCTAHEXACTC=SAT (4^85>u64)");
    assert_eq!(nbccso,        u64::MAX, "p3: NBCCSO=SAT (8^80>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);
    add_node(T112_VEC_C, T112_KEY_C, T112_ID_C);
    add_edge(T112_ID_A, T112_ID_B, "t112.e.ab");
    add_edge(T112_ID_B, T112_ID_C, "t112.e.bc");
    add_edge(T112_ID_C, T112_ID_A, "t112.e.ca");

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(noctahexactc,  u64::MAX, "k3: NOCTAHEXACTC=SAT");
    assert_eq!(nhoctahexactc, u64::MAX, "k3: NHOCTAHEXACTC=SAT");
    assert_eq!(nbccso,        u64::MAX, "k3: NBCCSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A); // hub
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);
    add_node(T112_VEC_C, T112_KEY_C, T112_ID_C);
    add_node(T112_VEC_D, T112_KEY_D, T112_ID_D);
    add_node(T112_VEC_E, T112_KEY_E, T112_ID_E);
    add_edge(T112_ID_A, T112_ID_B, "t112.e.ab");
    add_edge(T112_ID_A, T112_ID_C, "t112.e.ac");
    add_edge(T112_ID_A, T112_ID_D, "t112.e.ad");
    add_edge(T112_ID_A, T112_ID_E, "t112.e.ae");

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(noctahexactc,  u64::MAX, "k14: NOCTAHEXACTC=SAT");
    assert_eq!(nhoctahexactc, u64::MAX, "k14: NHOCTAHEXACTC=SAT");
    assert_eq!(nbccso,        u64::MAX, "k14: NBCCSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);
    add_node(T112_VEC_C, T112_KEY_C, T112_ID_C);
    add_node(T112_VEC_D, T112_KEY_D, T112_ID_D);
    add_edge(T112_ID_A, T112_ID_B, "t112.e.ab");
    add_edge(T112_ID_B, T112_ID_C, "t112.e.bc");
    add_edge(T112_ID_C, T112_ID_D, "t112.e.cd");

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(noctahexactc,  u64::MAX, "p4: NOCTAHEXACTC=SAT");
    assert_eq!(nhoctahexactc, u64::MAX, "p4: NHOCTAHEXACTC=SAT");
    assert_eq!(nbccso,        u64::MAX, "p4: NBCCSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);
    add_node(T112_VEC_C, T112_KEY_C, T112_ID_C);
    add_node(T112_VEC_D, T112_KEY_D, T112_ID_D);
    add_edge(T112_ID_A, T112_ID_B, "t112.e.ab");
    add_edge(T112_ID_A, T112_ID_C, "t112.e.ac");
    add_edge(T112_ID_A, T112_ID_D, "t112.e.ad");
    add_edge(T112_ID_B, T112_ID_C, "t112.e.bc");
    add_edge(T112_ID_B, T112_ID_D, "t112.e.bd");
    add_edge(T112_ID_C, T112_ID_D, "t112.e.cd");

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(noctahexactc,  u64::MAX, "k4: NOCTAHEXACTC=SAT");
    assert_eq!(nhoctahexactc, u64::MAX, "k4: NHOCTAHEXACTC=SAT");
    assert_eq!(nbccso,        u64::MAX, "k4: NBCCSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             2, "2iso: node_count=2");
    assert_eq!(ec,             0, "2iso: edge_count=0");
    assert_eq!(noctahexactc,  0, "2iso: NOCTAHEXACTC=0");
    assert_eq!(nhoctahexactc, 0, "2iso: NHOCTAHEXACTC=0");
    assert_eq!(nbccso,        0, "2iso: NBCCSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T112_VEC_A, T112_KEY_A, T112_ID_A);
    add_node(T112_VEC_B, T112_KEY_B, T112_ID_B);
    add_node(T112_VEC_C, T112_KEY_C, T112_ID_C);
    add_node(T112_VEC_D, T112_KEY_D, T112_ID_D);
    add_node(T112_VEC_E, T112_KEY_E, T112_ID_E);
    add_edge(T112_ID_A, T112_ID_C, "t112.e.ac");
    add_edge(T112_ID_A, T112_ID_D, "t112.e.ad");
    add_edge(T112_ID_A, T112_ID_E, "t112.e.ae");
    add_edge(T112_ID_B, T112_ID_C, "t112.e.bc");
    add_edge(T112_ID_B, T112_ID_D, "t112.e.bd");
    add_edge(T112_ID_B, T112_ID_E, "t112.e.be");

    let (noctahexactc, nhoctahexactc, nbccso, ec, nc) = gos_runtime::graph_topo_indices112();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(noctahexactc,  u64::MAX, "k23: NOCTAHEXACTC=SAT");
    assert_eq!(nhoctahexactc, u64::MAX, "k23: NHOCTAHEXACTC=SAT");
    assert_eq!(nbccso,        u64::MAX, "k23: NBCCSO=SAT");
}
