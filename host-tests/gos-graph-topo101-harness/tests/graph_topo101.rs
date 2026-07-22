// gos-graph-topo101-harness — V3.112 NHEPTAPENTACTC + NHHEPTAPENTACTC + NBRSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices101()`:
//   Returns (nheptapentactc, nhheptapentactc, nbrso, edge_count, node_count)
//   - nheptapentactc  = NHEPTAPENTACTC(G) = Σ_v S(v)^75                         (exact u64; S-Heptapentacontic vertex sum)
//   - nhheptapentactc = NHHEPTAPENTACTC(G) = Σ_{uv∈E} (S_u+S_v)^74             (exact u64; S-Heptapentacontic edge-sum)
//   - nbrso           = NBRSO(G)           = Σ_{uv∈E} (S_u²+S_v²)^69           (exact u64; S-Variant Sombor, α=138)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTAPENTACTC(G) = Σ_v S(v)^75
//     S-Heptapentacontic vertex sum; sixth of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTATETRAACTC=Σ S^74 (topo100) → NHEPTAPENTACTC=Σ S^75 (topo101).
//     NHEPTAPENTACTC = n·S^75 for S-regular.
//     Overflow: S^75 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^75 = s64 × s8 × s2 × s  (75=64+8+2+1; 9 mults total).
//
//   NHHEPTAPENTACTC(G) = Σ_{uv∈E} (S_u+S_v)^74
//     S-Heptapentacontic edge-sum; extends NHHEPTATETRAACTC=Σ(S+S)^73 (topo100).
//     NHHEPTAPENTACTC = |E|·(2S)^74 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^74 → saturating u128 accumulator.
//     Implementation: ss^74 = ss64 × ss8 × ss2  (74=64+8+2; 8 mults total).
//
//   NBRSO(G) = Σ_{uv∈E} (S_u²+S_v²)^69
//     S-Variant Sombor: generalised Sombor SO^α with α=138 on S-variant.
//     18th of NB series, letter R (after NBQSO α=136 topo100).
//     NBQSO(topo100,α=136) → NBRSO(topo101,α=138).
//     NBRSO = |E|·(2S²)^69 for S-regular.
//     Overflow per edge: (2×16129²)^69 → saturating u128 accumulator.
//     Implementation: s2s^69 = s2s64 × s2s4 × s2s  (69=64+4+1; 8 mults total).
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
//  Graph     NHEPTAPENTACTC(exact)     NHHEPTAPENTACTC(exact)     NBRSO(exact)               edges  nodes
//  Empty                      0                           0                  0                    0      0
//  1 node                     0                           0                  0                    0      1
//  K₂                         2           u64::MAX(sat.)     u64::MAX(sat.)                     1      2
//  P₃              u64::MAX(sat.)          u64::MAX(sat.)          u64::MAX(sat.)                2      3
//  K₃              u64::MAX(sat.)          u64::MAX(sat.)          u64::MAX(sat.)                3      3
//  K_{1,4}         u64::MAX(sat.)          u64::MAX(sat.)          u64::MAX(sat.)                4      5
//  P₄              u64::MAX(sat.)          u64::MAX(sat.)          u64::MAX(sat.)                3      4
//  K₄              u64::MAX(sat.)          u64::MAX(sat.)          u64::MAX(sat.)                6      4
//  2 isolated                 0                           0                  0                    0      2
//  K_{2,3}         u64::MAX(sat.)          u64::MAX(sat.)          u64::MAX(sat.)                6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTAPENTACTC:  1^75 + 1^75 = 2. ✓
//     NHHEPTAPENTACTC: (1+1)^74 = 2^74 ≈ 1.89×10^22 > u64::MAX → SATURATES. ✓
//     NBRSO:           (1²+1²)^69 = 2^69 ≈ 5.90×10^20 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTAPENTACTC:  3×2^75 >> u64::MAX → SATURATES. ✓
//     NHHEPTAPENTACTC: 2×(4)^74 → SATURATES. ✓
//     NBRSO:           2×(8)^69 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTAPENTACTC:  3×4^75 → SATURATES. ✓
//     NHHEPTAPENTACTC: 3×8^74 → SATURATES. ✓
//     NBRSO:           3×32^69 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTAPENTACTC:  5×4^75 → SATURATES. ✓
//     NHHEPTAPENTACTC: 4×8^74 → SATURATES. ✓
//     NBRSO:           4×32^69 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTAPENTACTC:  2×2^75 + 2×3^75. 3^75 >> u64::MAX → SATURATES. ✓
//     NHHEPTAPENTACTC: 5^74+6^74+5^74 → SATURATES. ✓
//     NBRSO:           13^69+18^69+13^69 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTAPENTACTC:  4×9^75 → SATURATES. ✓
//     NHHEPTAPENTACTC: 6×18^74 → SATURATES. ✓
//     NBRSO:           6×162^69 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTAPENTACTC:  5×6^75 → SATURATES. ✓
//     NHHEPTAPENTACTC: 6×12^74 → SATURATES. ✓
//     NBRSO:           6×72^69 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTAPENTACTC  = n·S^75                                                                          for S-regular ✓
//   NHHEPTAPENTACTC = |E|·(2S)^74 (saturates for |E|≥1,S≥1)                                         for S-regular ✓
//   NBRSO           = |E|·(2S²)^69                                                                    for S-regular ✓
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

const T101_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX101");
const T101_EXEC:   ExecutorId = ExecutorId::from_ascii("t101.exec");

const T101_KEY_A: &str = "t101.alpha";
const T101_KEY_B: &str = "t101.beta";
const T101_KEY_C: &str = "t101.gamma";
const T101_KEY_D: &str = "t101.delta";
const T101_KEY_E: &str = "t101.epsilon";

const T101_ID_A: NodeId = derive_node_id(T101_PLUGIN, T101_KEY_A);
const T101_ID_B: NodeId = derive_node_id(T101_PLUGIN, T101_KEY_B);
const T101_ID_C: NodeId = derive_node_id(T101_PLUGIN, T101_KEY_C);
const T101_ID_D: NodeId = derive_node_id(T101_PLUGIN, T101_KEY_D);
const T101_ID_E: NodeId = derive_node_id(T101_PLUGIN, T101_KEY_E);

// L4=188 namespace for this harness.
const T101_VEC_A: VectorAddress = VectorAddress::new(188, 1, 1, 0);
const T101_VEC_B: VectorAddress = VectorAddress::new(188, 1, 2, 0);
const T101_VEC_C: VectorAddress = VectorAddress::new(188, 1, 3, 0);
const T101_VEC_D: VectorAddress = VectorAddress::new(188, 2, 1, 0);
const T101_VEC_E: VectorAddress = VectorAddress::new(188, 2, 2, 0);

const T101_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T101_PLUGIN,
    name:         "kl-graph-topo101-harness",
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
        executor_id:       T101_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T101_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T101_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nheptapentactc,   0, "empty: NHEPTAPENTACTC=0");
    assert_eq!(nhheptapentactc,  0, "empty: NHHEPTAPENTACTC=0");
    assert_eq!(nbrso,            0, "empty: NBRSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nheptapentactc,   0, "single: NHEPTAPENTACTC=0");
    assert_eq!(nhheptapentactc,  0, "single: NHHEPTAPENTACTC=0");
    assert_eq!(nbrso,            0, "single: NBRSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTAPENTACTC:  1^75 + 1^75 = 2.
// NHHEPTAPENTACTC: (1+1)^74 = 2^74 > u64::MAX → SATURATES.
// NBRSO:           (1²+1²)^69 = 2^69 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);
    add_edge(T101_ID_A, T101_ID_B, "t101.e.ab");

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               2,         "k2: node_count=2");
    assert_eq!(ec,               1,         "k2: edge_count=1");
    assert_eq!(nheptapentactc,   2,         "k2: NHEPTAPENTACTC=2 (1^75+1^75=2)");
    assert_eq!(nhheptapentactc,  u64::MAX,  "k2: NHHEPTAPENTACTC=SAT (2^74>u64::MAX)");
    assert_eq!(nbrso,            u64::MAX,  "k2: NBRSO=SAT (2^69>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);
    add_node(T101_VEC_C, T101_KEY_C, T101_ID_C);
    add_edge(T101_ID_A, T101_ID_B, "t101.e.ab");
    add_edge(T101_ID_B, T101_ID_C, "t101.e.bc");

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               3,         "p3: node_count=3");
    assert_eq!(ec,               2,         "p3: edge_count=2");
    assert_eq!(nheptapentactc,   u64::MAX,  "p3: NHEPTAPENTACTC=SAT (3\u{00d7}2^75>u64)");
    assert_eq!(nhheptapentactc,  u64::MAX,  "p3: NHHEPTAPENTACTC=SAT (4^74>u64)");
    assert_eq!(nbrso,            u64::MAX,  "p3: NBRSO=SAT (8^69>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);
    add_node(T101_VEC_C, T101_KEY_C, T101_ID_C);
    add_edge(T101_ID_A, T101_ID_B, "t101.e.ab");
    add_edge(T101_ID_B, T101_ID_C, "t101.e.bc");
    add_edge(T101_ID_C, T101_ID_A, "t101.e.ca");

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nheptapentactc,   u64::MAX, "k3: NHEPTAPENTACTC=SAT");
    assert_eq!(nhheptapentactc,  u64::MAX, "k3: NHHEPTAPENTACTC=SAT");
    assert_eq!(nbrso,            u64::MAX, "k3: NBRSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A); // hub
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);
    add_node(T101_VEC_C, T101_KEY_C, T101_ID_C);
    add_node(T101_VEC_D, T101_KEY_D, T101_ID_D);
    add_node(T101_VEC_E, T101_KEY_E, T101_ID_E);
    add_edge(T101_ID_A, T101_ID_B, "t101.e.ab");
    add_edge(T101_ID_A, T101_ID_C, "t101.e.ac");
    add_edge(T101_ID_A, T101_ID_D, "t101.e.ad");
    add_edge(T101_ID_A, T101_ID_E, "t101.e.ae");

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nheptapentactc,   u64::MAX, "k14: NHEPTAPENTACTC=SAT");
    assert_eq!(nhheptapentactc,  u64::MAX, "k14: NHHEPTAPENTACTC=SAT");
    assert_eq!(nbrso,            u64::MAX, "k14: NBRSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);
    add_node(T101_VEC_C, T101_KEY_C, T101_ID_C);
    add_node(T101_VEC_D, T101_KEY_D, T101_ID_D);
    add_edge(T101_ID_A, T101_ID_B, "t101.e.ab");
    add_edge(T101_ID_B, T101_ID_C, "t101.e.bc");
    add_edge(T101_ID_C, T101_ID_D, "t101.e.cd");

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nheptapentactc,   u64::MAX, "p4: NHEPTAPENTACTC=SAT");
    assert_eq!(nhheptapentactc,  u64::MAX, "p4: NHHEPTAPENTACTC=SAT");
    assert_eq!(nbrso,            u64::MAX, "p4: NBRSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);
    add_node(T101_VEC_C, T101_KEY_C, T101_ID_C);
    add_node(T101_VEC_D, T101_KEY_D, T101_ID_D);
    add_edge(T101_ID_A, T101_ID_B, "t101.e.ab");
    add_edge(T101_ID_A, T101_ID_C, "t101.e.ac");
    add_edge(T101_ID_A, T101_ID_D, "t101.e.ad");
    add_edge(T101_ID_B, T101_ID_C, "t101.e.bc");
    add_edge(T101_ID_B, T101_ID_D, "t101.e.bd");
    add_edge(T101_ID_C, T101_ID_D, "t101.e.cd");

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nheptapentactc,   u64::MAX, "k4: NHEPTAPENTACTC=SAT");
    assert_eq!(nhheptapentactc,  u64::MAX, "k4: NHHEPTAPENTACTC=SAT");
    assert_eq!(nbrso,            u64::MAX, "k4: NBRSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               2, "2iso: node_count=2");
    assert_eq!(ec,               0, "2iso: edge_count=0");
    assert_eq!(nheptapentactc,   0, "2iso: NHEPTAPENTACTC=0");
    assert_eq!(nhheptapentactc,  0, "2iso: NHHEPTAPENTACTC=0");
    assert_eq!(nbrso,            0, "2iso: NBRSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T101_VEC_A, T101_KEY_A, T101_ID_A);
    add_node(T101_VEC_B, T101_KEY_B, T101_ID_B);
    add_node(T101_VEC_C, T101_KEY_C, T101_ID_C);
    add_node(T101_VEC_D, T101_KEY_D, T101_ID_D);
    add_node(T101_VEC_E, T101_KEY_E, T101_ID_E);
    add_edge(T101_ID_A, T101_ID_C, "t101.e.ac");
    add_edge(T101_ID_A, T101_ID_D, "t101.e.ad");
    add_edge(T101_ID_A, T101_ID_E, "t101.e.ae");
    add_edge(T101_ID_B, T101_ID_C, "t101.e.bc");
    add_edge(T101_ID_B, T101_ID_D, "t101.e.bd");
    add_edge(T101_ID_B, T101_ID_E, "t101.e.be");

    let (nheptapentactc, nhheptapentactc, nbrso, ec, nc) = gos_runtime::graph_topo_indices101();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nheptapentactc,   u64::MAX, "k23: NHEPTAPENTACTC=SAT");
    assert_eq!(nhheptapentactc,  u64::MAX, "k23: NHHEPTAPENTACTC=SAT");
    assert_eq!(nbrso,            u64::MAX, "k23: NBRSO=SAT");
}
