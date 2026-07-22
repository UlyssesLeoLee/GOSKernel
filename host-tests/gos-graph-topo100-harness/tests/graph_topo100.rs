// gos-graph-topo100-harness — V3.111 NHEPTATETRAACTC + NHHEPTATETRAACTC + NBQSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices100()`:
//   Returns (nheptatetraactc, nhheptatetraactc, nbqso, edge_count, node_count)
//   - nheptatetraactc  = NHEPTATETRAACTC(G) = Σ_v S(v)^74                         (exact u64; S-Heptatetracontic vertex sum)
//   - nhheptatetraactc = NHHEPTATETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^73             (exact u64; S-Heptatetracontic edge-sum)
//   - nbqso            = NBQSO(G)            = Σ_{uv∈E} (S_u²+S_v²)^68           (exact u64; S-Variant Sombor, α=136)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTATETRAACTC(G) = Σ_v S(v)^74
//     S-Heptatetracontic vertex sum; fifth of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTATRIACTC=Σ S^73 (topo99) → NHEPTATETRAACTC=Σ S^74 (topo100).
//     NHEPTATETRAACTC = n·S^74 for S-regular.
//     Overflow: S^74 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^74 = s64 × s8 × s2  (74=64+8+2; 8 mults total).
//
//   NHHEPTATETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^73
//     S-Heptatetracontic edge-sum; extends NHHEPTATRIACTC=Σ(S+S)^72 (topo99).
//     NHHEPTATETRAACTC = |E|·(2S)^73 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^73 → saturating u128 accumulator.
//     Implementation: ss^73 = ss64 × ss8 × ss  (73=64+8+1; 8 mults total).
//
//   NBQSO(G) = Σ_{uv∈E} (S_u²+S_v²)^68
//     S-Variant Sombor: generalised Sombor SO^α with α=136 on S-variant.
//     17th of NB series, letter Q (after NBPSO α=134 topo99).
//     NBPSO(topo99,α=134) → NBQSO(topo100,α=136).
//     NBQSO = |E|·(2S²)^68 for S-regular.
//     Overflow per edge: (2×16129²)^68 → saturating u128 accumulator.
//     Implementation: s2s^68 = s2s64 × s2s4  (68=64+4; 7 mults total — efficient!).
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
//  Graph     NHEPTATETRAACTC(exact)    NHHEPTATETRAACTC(exact)    NBQSO(exact)               edges  nodes
//  Empty                      0                           0                   0                   0      0
//  1 node                     0                           0                   0                   0      1
//  K₂                         2           u64::MAX(sat.)      u64::MAX(sat.)                    1      2
//  P₃              u64::MAX(sat.)          u64::MAX(sat.)           u64::MAX(sat.)               2      3
//  K₃              u64::MAX(sat.)          u64::MAX(sat.)           u64::MAX(sat.)               3      3
//  K_{1,4}         u64::MAX(sat.)          u64::MAX(sat.)           u64::MAX(sat.)               4      5
//  P₄              u64::MAX(sat.)          u64::MAX(sat.)           u64::MAX(sat.)               3      4
//  K₄              u64::MAX(sat.)          u64::MAX(sat.)           u64::MAX(sat.)               6      4
//  2 isolated                 0                           0                   0                   0      2
//  K_{2,3}         u64::MAX(sat.)          u64::MAX(sat.)           u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTATETRAACTC:  1^74 + 1^74 = 2. ✓
//     NHHEPTATETRAACTC: (1+1)^73 = 2^73 ≈ 9.44×10^21 > u64::MAX → SATURATES. ✓
//     NBQSO:            (1²+1²)^68 = 2^68 ≈ 2.95×10^20 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTATETRAACTC:  3×2^74 >> u64::MAX → SATURATES. ✓
//     NHHEPTATETRAACTC: 2×(4)^73 → SATURATES. ✓
//     NBQSO:            2×(8)^68 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTATETRAACTC:  3×4^74 → SATURATES. ✓
//     NHHEPTATETRAACTC: 3×8^73 → SATURATES. ✓
//     NBQSO:            3×32^68 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTATETRAACTC:  5×4^74 → SATURATES. ✓
//     NHHEPTATETRAACTC: 4×8^73 → SATURATES. ✓
//     NBQSO:            4×32^68 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTATETRAACTC:  2×2^74 + 2×3^74. 3^74 >> u64::MAX → SATURATES. ✓
//     NHHEPTATETRAACTC: 5^73+6^73+5^73 → SATURATES. ✓
//     NBQSO:            13^68+18^68+13^68 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTATETRAACTC:  4×9^74 → SATURATES. ✓
//     NHHEPTATETRAACTC: 6×18^73 → SATURATES. ✓
//     NBQSO:            6×162^68 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTATETRAACTC:  5×6^74 → SATURATES. ✓
//     NHHEPTATETRAACTC: 6×12^73 → SATURATES. ✓
//     NBQSO:            6×72^68 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTATETRAACTC  = n·S^74                                                                          for S-regular ✓
//   NHHEPTATETRAACTC = |E|·(2S)^73 (saturates for |E|≥1,S≥1)                                         for S-regular ✓
//   NBQSO            = |E|·(2S²)^68                                                                    for S-regular ✓
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

const T100_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX100");
const T100_EXEC:   ExecutorId = ExecutorId::from_ascii("t100.exec");

const T100_KEY_A: &str = "t100.alpha";
const T100_KEY_B: &str = "t100.beta";
const T100_KEY_C: &str = "t100.gamma";
const T100_KEY_D: &str = "t100.delta";
const T100_KEY_E: &str = "t100.epsilon";

const T100_ID_A: NodeId = derive_node_id(T100_PLUGIN, T100_KEY_A);
const T100_ID_B: NodeId = derive_node_id(T100_PLUGIN, T100_KEY_B);
const T100_ID_C: NodeId = derive_node_id(T100_PLUGIN, T100_KEY_C);
const T100_ID_D: NodeId = derive_node_id(T100_PLUGIN, T100_KEY_D);
const T100_ID_E: NodeId = derive_node_id(T100_PLUGIN, T100_KEY_E);

// L4=187 namespace for this harness.
const T100_VEC_A: VectorAddress = VectorAddress::new(187, 1, 1, 0);
const T100_VEC_B: VectorAddress = VectorAddress::new(187, 1, 2, 0);
const T100_VEC_C: VectorAddress = VectorAddress::new(187, 1, 3, 0);
const T100_VEC_D: VectorAddress = VectorAddress::new(187, 2, 1, 0);
const T100_VEC_E: VectorAddress = VectorAddress::new(187, 2, 2, 0);

const T100_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T100_PLUGIN,
    name:         "kl-graph-topo100-harness",
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
        executor_id:       T100_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T100_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T100_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(nheptatetraactc,   0, "empty: NHEPTATETRAACTC=0");
    assert_eq!(nhheptatetraactc,  0, "empty: NHHEPTATETRAACTC=0");
    assert_eq!(nbqso,             0, "empty: NBQSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(nheptatetraactc,   0, "single: NHEPTATETRAACTC=0");
    assert_eq!(nhheptatetraactc,  0, "single: NHHEPTATETRAACTC=0");
    assert_eq!(nbqso,             0, "single: NBQSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTATETRAACTC:  1^74 + 1^74 = 2.
// NHHEPTATETRAACTC: (1+1)^73 = 2^73 > u64::MAX → SATURATES.
// NBQSO:            (1²+1²)^68 = 2^68 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);
    add_edge(T100_ID_A, T100_ID_B, "t100.e.ab");

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                2,         "k2: node_count=2");
    assert_eq!(ec,                1,         "k2: edge_count=1");
    assert_eq!(nheptatetraactc,   2,         "k2: NHEPTATETRAACTC=2 (1^74+1^74=2)");
    assert_eq!(nhheptatetraactc,  u64::MAX,  "k2: NHHEPTATETRAACTC=SAT (2^73>u64::MAX)");
    assert_eq!(nbqso,             u64::MAX,  "k2: NBQSO=SAT (2^68>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);
    add_node(T100_VEC_C, T100_KEY_C, T100_ID_C);
    add_edge(T100_ID_A, T100_ID_B, "t100.e.ab");
    add_edge(T100_ID_B, T100_ID_C, "t100.e.bc");

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                3,         "p3: node_count=3");
    assert_eq!(ec,                2,         "p3: edge_count=2");
    assert_eq!(nheptatetraactc,   u64::MAX,  "p3: NHEPTATETRAACTC=SAT (3\u{00d7}2^74>u64)");
    assert_eq!(nhheptatetraactc,  u64::MAX,  "p3: NHHEPTATETRAACTC=SAT (4^73>u64)");
    assert_eq!(nbqso,             u64::MAX,  "p3: NBQSO=SAT (8^68>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);
    add_node(T100_VEC_C, T100_KEY_C, T100_ID_C);
    add_edge(T100_ID_A, T100_ID_B, "t100.e.ab");
    add_edge(T100_ID_B, T100_ID_C, "t100.e.bc");
    add_edge(T100_ID_C, T100_ID_A, "t100.e.ca");

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                3,        "k3: node_count=3");
    assert_eq!(ec,                3,        "k3: edge_count=3");
    assert_eq!(nheptatetraactc,   u64::MAX, "k3: NHEPTATETRAACTC=SAT");
    assert_eq!(nhheptatetraactc,  u64::MAX, "k3: NHHEPTATETRAACTC=SAT");
    assert_eq!(nbqso,             u64::MAX, "k3: NBQSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A); // hub
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);
    add_node(T100_VEC_C, T100_KEY_C, T100_ID_C);
    add_node(T100_VEC_D, T100_KEY_D, T100_ID_D);
    add_node(T100_VEC_E, T100_KEY_E, T100_ID_E);
    add_edge(T100_ID_A, T100_ID_B, "t100.e.ab");
    add_edge(T100_ID_A, T100_ID_C, "t100.e.ac");
    add_edge(T100_ID_A, T100_ID_D, "t100.e.ad");
    add_edge(T100_ID_A, T100_ID_E, "t100.e.ae");

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                5,        "k14: node_count=5");
    assert_eq!(ec,                4,        "k14: edge_count=4");
    assert_eq!(nheptatetraactc,   u64::MAX, "k14: NHEPTATETRAACTC=SAT");
    assert_eq!(nhheptatetraactc,  u64::MAX, "k14: NHHEPTATETRAACTC=SAT");
    assert_eq!(nbqso,             u64::MAX, "k14: NBQSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);
    add_node(T100_VEC_C, T100_KEY_C, T100_ID_C);
    add_node(T100_VEC_D, T100_KEY_D, T100_ID_D);
    add_edge(T100_ID_A, T100_ID_B, "t100.e.ab");
    add_edge(T100_ID_B, T100_ID_C, "t100.e.bc");
    add_edge(T100_ID_C, T100_ID_D, "t100.e.cd");

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                4,        "p4: node_count=4");
    assert_eq!(ec,                3,        "p4: edge_count=3");
    assert_eq!(nheptatetraactc,   u64::MAX, "p4: NHEPTATETRAACTC=SAT");
    assert_eq!(nhheptatetraactc,  u64::MAX, "p4: NHHEPTATETRAACTC=SAT");
    assert_eq!(nbqso,             u64::MAX, "p4: NBQSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);
    add_node(T100_VEC_C, T100_KEY_C, T100_ID_C);
    add_node(T100_VEC_D, T100_KEY_D, T100_ID_D);
    add_edge(T100_ID_A, T100_ID_B, "t100.e.ab");
    add_edge(T100_ID_A, T100_ID_C, "t100.e.ac");
    add_edge(T100_ID_A, T100_ID_D, "t100.e.ad");
    add_edge(T100_ID_B, T100_ID_C, "t100.e.bc");
    add_edge(T100_ID_B, T100_ID_D, "t100.e.bd");
    add_edge(T100_ID_C, T100_ID_D, "t100.e.cd");

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                4,        "k4: node_count=4");
    assert_eq!(ec,                6,        "k4: edge_count=6");
    assert_eq!(nheptatetraactc,   u64::MAX, "k4: NHEPTATETRAACTC=SAT");
    assert_eq!(nhheptatetraactc,  u64::MAX, "k4: NHHEPTATETRAACTC=SAT");
    assert_eq!(nbqso,             u64::MAX, "k4: NBQSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                2, "2iso: node_count=2");
    assert_eq!(ec,                0, "2iso: edge_count=0");
    assert_eq!(nheptatetraactc,   0, "2iso: NHEPTATETRAACTC=0");
    assert_eq!(nhheptatetraactc,  0, "2iso: NHHEPTATETRAACTC=0");
    assert_eq!(nbqso,             0, "2iso: NBQSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T100_VEC_A, T100_KEY_A, T100_ID_A);
    add_node(T100_VEC_B, T100_KEY_B, T100_ID_B);
    add_node(T100_VEC_C, T100_KEY_C, T100_ID_C);
    add_node(T100_VEC_D, T100_KEY_D, T100_ID_D);
    add_node(T100_VEC_E, T100_KEY_E, T100_ID_E);
    add_edge(T100_ID_A, T100_ID_C, "t100.e.ac");
    add_edge(T100_ID_A, T100_ID_D, "t100.e.ad");
    add_edge(T100_ID_A, T100_ID_E, "t100.e.ae");
    add_edge(T100_ID_B, T100_ID_C, "t100.e.bc");
    add_edge(T100_ID_B, T100_ID_D, "t100.e.bd");
    add_edge(T100_ID_B, T100_ID_E, "t100.e.be");

    let (nheptatetraactc, nhheptatetraactc, nbqso, ec, nc) = gos_runtime::graph_topo_indices100();
    assert_eq!(nc,                5,        "k23: node_count=5");
    assert_eq!(ec,                6,        "k23: edge_count=6");
    assert_eq!(nheptatetraactc,   u64::MAX, "k23: NHEPTATETRAACTC=SAT");
    assert_eq!(nhheptatetraactc,  u64::MAX, "k23: NHHEPTATETRAACTC=SAT");
    assert_eq!(nbqso,             u64::MAX, "k23: NBQSO=SAT");
}
