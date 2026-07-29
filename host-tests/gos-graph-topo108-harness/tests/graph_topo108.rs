// gos-graph-topo108-harness — V3.119 NOCTADIACTC + NHOCTADIACTC + NBYSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices108()`:
//   Returns (noctadiactc, nhoctadiactc, nbyso, edge_count, node_count)
//   - noctadiactc  = NOCTADIACTC(G)  = Σ_v S(v)^82                          (exact u64; S-Octadicontic vertex sum)
//   - nhoctadiactc = NHOCTADIACTC(G) = Σ_{uv∈E} (S_u+S_v)^81              (exact u64; S-Octadicontic edge-sum)
//   - nbyso         = NBYSO(G)        = Σ_{uv∈E} (S_u²+S_v²)^76            (exact u64; S-Variant Sombor, α=152)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTADIACTC(G) = Σ_v S(v)^82
//     S-Octadicontic vertex sum; third of the octacontic (80-89) series.
//     Extends: NOCTAMONOACTC=Σ S^81 (topo107) → NOCTADIACTC=Σ S^82 (topo108).
//     NOCTADIACTC = n·S^82 for S-regular.
//     Overflow: S^82 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^82 = s64 × s16 × s2  (82=64+16+2; 8 mults total).
//
//   NHOCTADIACTC(G) = Σ_{uv∈E} (S_u+S_v)^81
//     S-Octadicontic edge-sum; extends NHOCTAMONOACTC=Σ(S+S)^80 (topo107).
//     NHOCTADIACTC = |E|·(2S)^81 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^81 → saturating u128 accumulator.
//     Implementation: ss^81 = ss64 × ss16 × ss  (81=64+16+1; 8 mults total).
//
//   NBYSO(G) = Σ_{uv∈E} (S_u²+S_v²)^76
//     S-Variant Sombor: generalised Sombor SO^α with α=152 on S-variant.
//     25th of NB series, letter Y (after NBXSO α=150 topo107).
//     NBXSO(topo107,α=150) → NBYSO(topo108,α=152).
//     NBYSO = |E|·(2S²)^76 for S-regular.
//     Overflow per edge: (2×16129²)^76 → saturating u128 accumulator.
//     Implementation: s2s^76 = s2s64 × s2s8 × s2s4  (76=64+8+4; 8 mults total).
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
//  Graph     NOCTADIACTC(exact)         NHOCTADIACTC(exact)        NBYSO(exact)               edges  nodes
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
//     NOCTADIACTC:   1^82 + 1^82 = 2. ✓
//     NHOCTADIACTC:  (1+1)^81 = 2^81 ≈ 2.42×10^24 > u64::MAX → SATURATES. ✓
//     NBYSO:         (1²+1²)^76 = 2^76 ≈ 7.56×10^22 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTADIACTC:   3×2^82 >> u64::MAX → SATURATES. ✓
//     NHOCTADIACTC:  2×(4)^81 → SATURATES. ✓
//     NBYSO:         2×(8)^76 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTADIACTC:   3×4^82 → SATURATES. ✓
//     NHOCTADIACTC:  3×8^81 → SATURATES. ✓
//     NBYSO:         3×32^76 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTADIACTC:   5×4^82 → SATURATES. ✓
//     NHOCTADIACTC:  4×8^81 → SATURATES. ✓
//     NBYSO:         4×32^76 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTADIACTC:   2×2^82 + 2×3^82. 3^82 >> u64::MAX → SATURATES. ✓
//     NHOCTADIACTC:  5^81+6^81+5^81 → SATURATES. ✓
//     NBYSO:         13^76+18^76+13^76 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTADIACTC:   4×9^82 → SATURATES. ✓
//     NHOCTADIACTC:  6×18^81 → SATURATES. ✓
//     NBYSO:         6×162^76 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTADIACTC:   5×6^82 → SATURATES. ✓
//     NHOCTADIACTC:  6×12^81 → SATURATES. ✓
//     NBYSO:         6×72^76 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTADIACTC  = n·S^82                                                                       for S-regular ✓
//   NHOCTADIACTC = |E|·(2S)^81 (saturates for |E|≥1,S≥1)                                      for S-regular ✓
//   NBYSO        = |E|·(2S²)^76                                                                 for S-regular ✓
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

const T108_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX108");
const T108_EXEC:   ExecutorId = ExecutorId::from_ascii("t108.exec");

const T108_KEY_A: &str = "t108.alpha";
const T108_KEY_B: &str = "t108.beta";
const T108_KEY_C: &str = "t108.gamma";
const T108_KEY_D: &str = "t108.delta";
const T108_KEY_E: &str = "t108.epsilon";

const T108_ID_A: NodeId = derive_node_id(T108_PLUGIN, T108_KEY_A);
const T108_ID_B: NodeId = derive_node_id(T108_PLUGIN, T108_KEY_B);
const T108_ID_C: NodeId = derive_node_id(T108_PLUGIN, T108_KEY_C);
const T108_ID_D: NodeId = derive_node_id(T108_PLUGIN, T108_KEY_D);
const T108_ID_E: NodeId = derive_node_id(T108_PLUGIN, T108_KEY_E);

// L4=195 namespace for this harness.
const T108_VEC_A: VectorAddress = VectorAddress::new(195, 1, 1, 0);
const T108_VEC_B: VectorAddress = VectorAddress::new(195, 1, 2, 0);
const T108_VEC_C: VectorAddress = VectorAddress::new(195, 1, 3, 0);
const T108_VEC_D: VectorAddress = VectorAddress::new(195, 2, 1, 0);
const T108_VEC_E: VectorAddress = VectorAddress::new(195, 2, 2, 0);

const T108_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T108_PLUGIN,
    name:         "kl-graph-topo108-harness",
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
        executor_id:       T108_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T108_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T108_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             0, "empty: node_count=0");
    assert_eq!(ec,             0, "empty: edge_count=0");
    assert_eq!(noctadiactc,   0, "empty: NOCTADIACTC=0");
    assert_eq!(nhoctadiactc,  0, "empty: NHOCTADIACTC=0");
    assert_eq!(nbyso,         0, "empty: NBYSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             1, "single: node_count=1");
    assert_eq!(ec,             0, "single: edge_count=0");
    assert_eq!(noctadiactc,   0, "single: NOCTADIACTC=0");
    assert_eq!(nhoctadiactc,  0, "single: NHOCTADIACTC=0");
    assert_eq!(nbyso,         0, "single: NBYSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTADIACTC:   1^82 + 1^82 = 2.
// NHOCTADIACTC:  (1+1)^81 = 2^81 > u64::MAX → SATURATES.
// NBYSO:         (1²+1²)^76 = 2^76 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);
    add_edge(T108_ID_A, T108_ID_B, "t108.e.ab");

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             2,        "k2: node_count=2");
    assert_eq!(ec,             1,        "k2: edge_count=1");
    assert_eq!(noctadiactc,   2,        "k2: NOCTADIACTC=2 (1^82+1^82=2)");
    assert_eq!(nhoctadiactc,  u64::MAX, "k2: NHOCTADIACTC=SAT (2^81>u64::MAX)");
    assert_eq!(nbyso,         u64::MAX, "k2: NBYSO=SAT (2^76>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);
    add_node(T108_VEC_C, T108_KEY_C, T108_ID_C);
    add_edge(T108_ID_A, T108_ID_B, "t108.e.ab");
    add_edge(T108_ID_B, T108_ID_C, "t108.e.bc");

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             3,        "p3: node_count=3");
    assert_eq!(ec,             2,        "p3: edge_count=2");
    assert_eq!(noctadiactc,   u64::MAX, "p3: NOCTADIACTC=SAT (3\u{00d7}2^82>u64)");
    assert_eq!(nhoctadiactc,  u64::MAX, "p3: NHOCTADIACTC=SAT (4^81>u64)");
    assert_eq!(nbyso,         u64::MAX, "p3: NBYSO=SAT (8^76>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);
    add_node(T108_VEC_C, T108_KEY_C, T108_ID_C);
    add_edge(T108_ID_A, T108_ID_B, "t108.e.ab");
    add_edge(T108_ID_B, T108_ID_C, "t108.e.bc");
    add_edge(T108_ID_C, T108_ID_A, "t108.e.ca");

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(noctadiactc,   u64::MAX, "k3: NOCTADIACTC=SAT");
    assert_eq!(nhoctadiactc,  u64::MAX, "k3: NHOCTADIACTC=SAT");
    assert_eq!(nbyso,         u64::MAX, "k3: NBYSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A); // hub
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);
    add_node(T108_VEC_C, T108_KEY_C, T108_ID_C);
    add_node(T108_VEC_D, T108_KEY_D, T108_ID_D);
    add_node(T108_VEC_E, T108_KEY_E, T108_ID_E);
    add_edge(T108_ID_A, T108_ID_B, "t108.e.ab");
    add_edge(T108_ID_A, T108_ID_C, "t108.e.ac");
    add_edge(T108_ID_A, T108_ID_D, "t108.e.ad");
    add_edge(T108_ID_A, T108_ID_E, "t108.e.ae");

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(noctadiactc,   u64::MAX, "k14: NOCTADIACTC=SAT");
    assert_eq!(nhoctadiactc,  u64::MAX, "k14: NHOCTADIACTC=SAT");
    assert_eq!(nbyso,         u64::MAX, "k14: NBYSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);
    add_node(T108_VEC_C, T108_KEY_C, T108_ID_C);
    add_node(T108_VEC_D, T108_KEY_D, T108_ID_D);
    add_edge(T108_ID_A, T108_ID_B, "t108.e.ab");
    add_edge(T108_ID_B, T108_ID_C, "t108.e.bc");
    add_edge(T108_ID_C, T108_ID_D, "t108.e.cd");

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(noctadiactc,   u64::MAX, "p4: NOCTADIACTC=SAT");
    assert_eq!(nhoctadiactc,  u64::MAX, "p4: NHOCTADIACTC=SAT");
    assert_eq!(nbyso,         u64::MAX, "p4: NBYSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);
    add_node(T108_VEC_C, T108_KEY_C, T108_ID_C);
    add_node(T108_VEC_D, T108_KEY_D, T108_ID_D);
    add_edge(T108_ID_A, T108_ID_B, "t108.e.ab");
    add_edge(T108_ID_A, T108_ID_C, "t108.e.ac");
    add_edge(T108_ID_A, T108_ID_D, "t108.e.ad");
    add_edge(T108_ID_B, T108_ID_C, "t108.e.bc");
    add_edge(T108_ID_B, T108_ID_D, "t108.e.bd");
    add_edge(T108_ID_C, T108_ID_D, "t108.e.cd");

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(noctadiactc,   u64::MAX, "k4: NOCTADIACTC=SAT");
    assert_eq!(nhoctadiactc,  u64::MAX, "k4: NHOCTADIACTC=SAT");
    assert_eq!(nbyso,         u64::MAX, "k4: NBYSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             2, "2iso: node_count=2");
    assert_eq!(ec,             0, "2iso: edge_count=0");
    assert_eq!(noctadiactc,   0, "2iso: NOCTADIACTC=0");
    assert_eq!(nhoctadiactc,  0, "2iso: NHOCTADIACTC=0");
    assert_eq!(nbyso,         0, "2iso: NBYSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T108_VEC_A, T108_KEY_A, T108_ID_A);
    add_node(T108_VEC_B, T108_KEY_B, T108_ID_B);
    add_node(T108_VEC_C, T108_KEY_C, T108_ID_C);
    add_node(T108_VEC_D, T108_KEY_D, T108_ID_D);
    add_node(T108_VEC_E, T108_KEY_E, T108_ID_E);
    add_edge(T108_ID_A, T108_ID_C, "t108.e.ac");
    add_edge(T108_ID_A, T108_ID_D, "t108.e.ad");
    add_edge(T108_ID_A, T108_ID_E, "t108.e.ae");
    add_edge(T108_ID_B, T108_ID_C, "t108.e.bc");
    add_edge(T108_ID_B, T108_ID_D, "t108.e.bd");
    add_edge(T108_ID_B, T108_ID_E, "t108.e.be");

    let (noctadiactc, nhoctadiactc, nbyso, ec, nc) = gos_runtime::graph_topo_indices108();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(noctadiactc,   u64::MAX, "k23: NOCTADIACTC=SAT");
    assert_eq!(nhoctadiactc,  u64::MAX, "k23: NHOCTADIACTC=SAT");
    assert_eq!(nbyso,         u64::MAX, "k23: NBYSO=SAT");
}
