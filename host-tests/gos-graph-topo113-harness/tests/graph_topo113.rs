// gos-graph-topo113-harness — V3.124 NOCTAHEPTACTC + NHOCTAHEPTACTC + NBDDSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices113()`:
//   Returns (noctaheptactc, nhoctaheptactc, nbddso, edge_count, node_count)
//   - noctaheptactc  = NOCTAHEPTACTC(G)  = Σ_v S(v)^87                          (exact u64; S-Octaheptic vertex sum)
//   - nhoctaheptactc = NHOCTAHEPTACTC(G) = Σ_{uv∈E} (S_u+S_v)^86              (exact u64; S-Octaheptic edge-sum)
//   - nbddso         = NBDDSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^81            (exact u64; S-Variant Sombor, α=162)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTAHEPTACTC(G) = Σ_v S(v)^87
//     S-Octaheptic vertex sum; eighth of the octacontic (80-89) series.
//     Extends: NOCTAHEXACTC=Σ S^86 (topo112) → NOCTAHEPTACTC=Σ S^87 (topo113).
//     NOCTAHEPTACTC = n·S^87 for S-regular.
//     Overflow: S^87 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^87 = s64 × s16 × s4 × s2 × s  (87=64+16+4+2+1; 10 mults).
//
//   NHOCTAHEPTACTC(G) = Σ_{uv∈E} (S_u+S_v)^86
//     S-Octaheptic edge-sum; extends NHOCTAHEXACTC=Σ(S+S)^85 (topo112).
//     NHOCTAHEPTACTC = |E|·(2S)^86 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^86 → saturating u128 accumulator.
//     Implementation: ss^86 = ss64 × ss16 × ss4 × ss2  (86=64+16+4+2; 9 mults total).
//
//   NBDDSO(G) = Σ_{uv∈E} (S_u²+S_v²)^81
//     S-Variant Sombor: generalised Sombor SO^α with α=162 on S-variant.
//     30th of NB series, letters DD (after NBCCSO α=160 topo112).
//     NBCCSO(topo112,α=160) → NBDDSO(topo113,α=162).
//     NBDDSO = |E|·(2S²)^81 for S-regular.
//     Overflow per edge: (2×16129²)^81 → saturating u128 accumulator.
//     Implementation: s2s^81 = s2s64 × s2s16 × s2s  (81=64+16+1; 8 mults total).
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
//  Graph     NOCTAHEPTACTC(exact)      NHOCTAHEPTACTC(exact)      NBDDSO(exact)              edges  nodes
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
//     NOCTAHEPTACTC:   1^87 + 1^87 = 2. ✓
//     NHOCTAHEPTACTC:  (1+1)^86 = 2^86 ≈ 7.73×10^25 > u64::MAX → SATURATES. ✓
//     NBDDSO:          (1²+1²)^81 = 2^81 ≈ 2.42×10^24 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTAHEPTACTC:   3×2^87 >> u64::MAX → SATURATES. ✓
//     NHOCTAHEPTACTC:  2×(4)^86 → SATURATES. ✓
//     NBDDSO:          2×(8)^81 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTAHEPTACTC:   3×4^87 → SATURATES. ✓
//     NHOCTAHEPTACTC:  3×8^86 → SATURATES. ✓
//     NBDDSO:          3×32^81 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTAHEPTACTC:   5×4^87 → SATURATES. ✓
//     NHOCTAHEPTACTC:  4×8^86 → SATURATES. ✓
//     NBDDSO:          4×32^81 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTAHEPTACTC:   2×2^87 + 2×3^87. 3^87 >> u64::MAX → SATURATES. ✓
//     NHOCTAHEPTACTC:  5^86+6^86+5^86 → SATURATES. ✓
//     NBDDSO:          13^81+18^81+13^81 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTAHEPTACTC:   4×9^87 → SATURATES. ✓
//     NHOCTAHEPTACTC:  6×18^86 → SATURATES. ✓
//     NBDDSO:          6×162^81 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTAHEPTACTC:   5×6^87 → SATURATES. ✓
//     NHOCTAHEPTACTC:  6×12^86 → SATURATES. ✓
//     NBDDSO:          6×72^81 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTAHEPTACTC  = n·S^87                                                                      for S-regular ✓
//   NHOCTAHEPTACTC = |E|·(2S)^86 (saturates for |E|≥1,S≥1)                                     for S-regular ✓
//   NBDDSO         = |E|·(2S²)^81                                                                for S-regular ✓
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

const T113_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX113");
const T113_EXEC:   ExecutorId = ExecutorId::from_ascii("t113.exec");

const T113_KEY_A: &str = "t113.alpha";
const T113_KEY_B: &str = "t113.beta";
const T113_KEY_C: &str = "t113.gamma";
const T113_KEY_D: &str = "t113.delta";
const T113_KEY_E: &str = "t113.epsilon";

const T113_ID_A: NodeId = derive_node_id(T113_PLUGIN, T113_KEY_A);
const T113_ID_B: NodeId = derive_node_id(T113_PLUGIN, T113_KEY_B);
const T113_ID_C: NodeId = derive_node_id(T113_PLUGIN, T113_KEY_C);
const T113_ID_D: NodeId = derive_node_id(T113_PLUGIN, T113_KEY_D);
const T113_ID_E: NodeId = derive_node_id(T113_PLUGIN, T113_KEY_E);

// L4=200 namespace for this harness.
const T113_VEC_A: VectorAddress = VectorAddress::new(200, 1, 1, 0);
const T113_VEC_B: VectorAddress = VectorAddress::new(200, 1, 2, 0);
const T113_VEC_C: VectorAddress = VectorAddress::new(200, 1, 3, 0);
const T113_VEC_D: VectorAddress = VectorAddress::new(200, 2, 1, 0);
const T113_VEC_E: VectorAddress = VectorAddress::new(200, 2, 2, 0);

const T113_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T113_PLUGIN,
    name:         "kl-graph-topo113-harness",
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
        executor_id:       T113_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T113_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T113_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(noctaheptactc,  0, "empty: NOCTAHEPTACTC=0");
    assert_eq!(nhoctaheptactc, 0, "empty: NHOCTAHEPTACTC=0");
    assert_eq!(nbddso,         0, "empty: NBDDSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(noctaheptactc,  0, "single: NOCTAHEPTACTC=0");
    assert_eq!(nhoctaheptactc, 0, "single: NHOCTAHEPTACTC=0");
    assert_eq!(nbddso,         0, "single: NBDDSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTAHEPTACTC:   1^87 + 1^87 = 2.
// NHOCTAHEPTACTC:  (1+1)^86 = 2^86 > u64::MAX → SATURATES.
// NBDDSO:          (1²+1²)^81 = 2^81 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);
    add_edge(T113_ID_A, T113_ID_B, "t113.e.ab");

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              2,        "k2: node_count=2");
    assert_eq!(ec,              1,        "k2: edge_count=1");
    assert_eq!(noctaheptactc,  2,        "k2: NOCTAHEPTACTC=2 (1^87+1^87=2)");
    assert_eq!(nhoctaheptactc, u64::MAX, "k2: NHOCTAHEPTACTC=SAT (2^86>u64::MAX)");
    assert_eq!(nbddso,         u64::MAX, "k2: NBDDSO=SAT (2^81>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);
    add_node(T113_VEC_C, T113_KEY_C, T113_ID_C);
    add_edge(T113_ID_A, T113_ID_B, "t113.e.ab");
    add_edge(T113_ID_B, T113_ID_C, "t113.e.bc");

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              3,        "p3: node_count=3");
    assert_eq!(ec,              2,        "p3: edge_count=2");
    assert_eq!(noctaheptactc,  u64::MAX, "p3: NOCTAHEPTACTC=SAT (3\u{00d7}2^87>u64)");
    assert_eq!(nhoctaheptactc, u64::MAX, "p3: NHOCTAHEPTACTC=SAT (4^86>u64)");
    assert_eq!(nbddso,         u64::MAX, "p3: NBDDSO=SAT (8^81>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);
    add_node(T113_VEC_C, T113_KEY_C, T113_ID_C);
    add_edge(T113_ID_A, T113_ID_B, "t113.e.ab");
    add_edge(T113_ID_B, T113_ID_C, "t113.e.bc");
    add_edge(T113_ID_C, T113_ID_A, "t113.e.ca");

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(noctaheptactc,  u64::MAX, "k3: NOCTAHEPTACTC=SAT");
    assert_eq!(nhoctaheptactc, u64::MAX, "k3: NHOCTAHEPTACTC=SAT");
    assert_eq!(nbddso,         u64::MAX, "k3: NBDDSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A); // hub
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);
    add_node(T113_VEC_C, T113_KEY_C, T113_ID_C);
    add_node(T113_VEC_D, T113_KEY_D, T113_ID_D);
    add_node(T113_VEC_E, T113_KEY_E, T113_ID_E);
    add_edge(T113_ID_A, T113_ID_B, "t113.e.ab");
    add_edge(T113_ID_A, T113_ID_C, "t113.e.ac");
    add_edge(T113_ID_A, T113_ID_D, "t113.e.ad");
    add_edge(T113_ID_A, T113_ID_E, "t113.e.ae");

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(noctaheptactc,  u64::MAX, "k14: NOCTAHEPTACTC=SAT");
    assert_eq!(nhoctaheptactc, u64::MAX, "k14: NHOCTAHEPTACTC=SAT");
    assert_eq!(nbddso,         u64::MAX, "k14: NBDDSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);
    add_node(T113_VEC_C, T113_KEY_C, T113_ID_C);
    add_node(T113_VEC_D, T113_KEY_D, T113_ID_D);
    add_edge(T113_ID_A, T113_ID_B, "t113.e.ab");
    add_edge(T113_ID_B, T113_ID_C, "t113.e.bc");
    add_edge(T113_ID_C, T113_ID_D, "t113.e.cd");

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(noctaheptactc,  u64::MAX, "p4: NOCTAHEPTACTC=SAT");
    assert_eq!(nhoctaheptactc, u64::MAX, "p4: NHOCTAHEPTACTC=SAT");
    assert_eq!(nbddso,         u64::MAX, "p4: NBDDSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);
    add_node(T113_VEC_C, T113_KEY_C, T113_ID_C);
    add_node(T113_VEC_D, T113_KEY_D, T113_ID_D);
    add_edge(T113_ID_A, T113_ID_B, "t113.e.ab");
    add_edge(T113_ID_A, T113_ID_C, "t113.e.ac");
    add_edge(T113_ID_A, T113_ID_D, "t113.e.ad");
    add_edge(T113_ID_B, T113_ID_C, "t113.e.bc");
    add_edge(T113_ID_B, T113_ID_D, "t113.e.bd");
    add_edge(T113_ID_C, T113_ID_D, "t113.e.cd");

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(noctaheptactc,  u64::MAX, "k4: NOCTAHEPTACTC=SAT");
    assert_eq!(nhoctaheptactc, u64::MAX, "k4: NHOCTAHEPTACTC=SAT");
    assert_eq!(nbddso,         u64::MAX, "k4: NBDDSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              2, "2iso: node_count=2");
    assert_eq!(ec,              0, "2iso: edge_count=0");
    assert_eq!(noctaheptactc,  0, "2iso: NOCTAHEPTACTC=0");
    assert_eq!(nhoctaheptactc, 0, "2iso: NHOCTAHEPTACTC=0");
    assert_eq!(nbddso,         0, "2iso: NBDDSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T113_VEC_A, T113_KEY_A, T113_ID_A);
    add_node(T113_VEC_B, T113_KEY_B, T113_ID_B);
    add_node(T113_VEC_C, T113_KEY_C, T113_ID_C);
    add_node(T113_VEC_D, T113_KEY_D, T113_ID_D);
    add_node(T113_VEC_E, T113_KEY_E, T113_ID_E);
    add_edge(T113_ID_A, T113_ID_C, "t113.e.ac");
    add_edge(T113_ID_A, T113_ID_D, "t113.e.ad");
    add_edge(T113_ID_A, T113_ID_E, "t113.e.ae");
    add_edge(T113_ID_B, T113_ID_C, "t113.e.bc");
    add_edge(T113_ID_B, T113_ID_D, "t113.e.bd");
    add_edge(T113_ID_B, T113_ID_E, "t113.e.be");

    let (noctaheptactc, nhoctaheptactc, nbddso, ec, nc) = gos_runtime::graph_topo_indices113();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(noctaheptactc,  u64::MAX, "k23: NOCTAHEPTACTC=SAT");
    assert_eq!(nhoctaheptactc, u64::MAX, "k23: NHOCTAHEPTACTC=SAT");
    assert_eq!(nbddso,         u64::MAX, "k23: NBDDSO=SAT");
}
