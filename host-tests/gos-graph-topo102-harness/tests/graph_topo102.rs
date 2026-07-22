// gos-graph-topo102-harness — V3.113 NHEPTAHEXAACTC + NHHEPTAHEXAACTC + NBSSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices102()`:
//   Returns (nheptahexaactc, nhheptahexaactc, nbsso, edge_count, node_count)
//   - nheptahexaactc  = NHEPTAHEXAACTC(G) = Σ_v S(v)^76                         (exact u64; S-Heptahexacontic vertex sum)
//   - nhheptahexaactc = NHHEPTAHEXAACTC(G) = Σ_{uv∈E} (S_u+S_v)^75             (exact u64; S-Heptahexacontic edge-sum)
//   - nbsso           = NBSSO(G)           = Σ_{uv∈E} (S_u²+S_v²)^70           (exact u64; S-Variant Sombor, α=140)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTAHEXAACTC(G) = Σ_v S(v)^76
//     S-Heptahexacontic vertex sum; seventh of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTAPENTACTC=Σ S^75 (topo101) → NHEPTAHEXAACTC=Σ S^76 (topo102).
//     NHEPTAHEXAACTC = n·S^76 for S-regular.
//     Overflow: S^76 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^76 = s64 × s8 × s4  (76=64+8+4; 8 mults total).
//
//   NHHEPTAHEXAACTC(G) = Σ_{uv∈E} (S_u+S_v)^75
//     S-Heptahexacontic edge-sum; extends NHHEPTAPENTACTC=Σ(S+S)^74 (topo101).
//     NHHEPTAHEXAACTC = |E|·(2S)^75 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^75 → saturating u128 accumulator.
//     Implementation: ss^75 = ss64 × ss8 × ss2 × ss  (75=64+8+2+1; 9 mults total).
//
//   NBSSO(G) = Σ_{uv∈E} (S_u²+S_v²)^70
//     S-Variant Sombor: generalised Sombor SO^α with α=140 on S-variant.
//     19th of NB series, letter S (after NBRSO α=138 topo101).
//     NBRSO(topo101,α=138) → NBSSO(topo102,α=140).
//     NBSSO = |E|·(2S²)^70 for S-regular.
//     Overflow per edge: (2×16129²)^70 → saturating u128 accumulator.
//     Implementation: s2s^70 = s2s64 × s2s4 × s2s2  (70=64+4+2; 8 mults total).
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
//  Graph     NHEPTAHEXAACTC(exact)     NHHEPTAHEXAACTC(exact)     NBSSO(exact)               edges  nodes
//  Empty                      0                           0                 0                     0      0
//  1 node                     0                           0                 0                     0      1
//  K₂                         2           u64::MAX(sat.)    u64::MAX(sat.)                      1      2
//  P₃              u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 2      3
//  K₃              u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 3      3
//  K_{1,4}         u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 4      5
//  P₄              u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 3      4
//  K₄              u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 6      4
//  2 isolated                 0                           0                 0                     0      2
//  K_{2,3}         u64::MAX(sat.)          u64::MAX(sat.)         u64::MAX(sat.)                 6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTAHEXAACTC:  1^76 + 1^76 = 2. ✓
//     NHHEPTAHEXAACTC: (1+1)^75 = 2^75 ≈ 3.78×10^22 > u64::MAX → SATURATES. ✓
//     NBSSO:           (1²+1²)^70 = 2^70 ≈ 1.18×10^21 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTAHEXAACTC:  3×2^76 >> u64::MAX → SATURATES. ✓
//     NHHEPTAHEXAACTC: 2×(4)^75 → SATURATES. ✓
//     NBSSO:           2×(8)^70 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTAHEXAACTC:  3×4^76 → SATURATES. ✓
//     NHHEPTAHEXAACTC: 3×8^75 → SATURATES. ✓
//     NBSSO:           3×32^70 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTAHEXAACTC:  5×4^76 → SATURATES. ✓
//     NHHEPTAHEXAACTC: 4×8^75 → SATURATES. ✓
//     NBSSO:           4×32^70 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTAHEXAACTC:  2×2^76 + 2×3^76. 3^76 >> u64::MAX → SATURATES. ✓
//     NHHEPTAHEXAACTC: 5^75+6^75+5^75 → SATURATES. ✓
//     NBSSO:           13^70+18^70+13^70 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTAHEXAACTC:  4×9^76 → SATURATES. ✓
//     NHHEPTAHEXAACTC: 6×18^75 → SATURATES. ✓
//     NBSSO:           6×162^70 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTAHEXAACTC:  5×6^76 → SATURATES. ✓
//     NHHEPTAHEXAACTC: 6×12^75 → SATURATES. ✓
//     NBSSO:           6×72^70 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTAHEXAACTC  = n·S^76                                                                         for S-regular ✓
//   NHHEPTAHEXAACTC = |E|·(2S)^75 (saturates for |E|≥1,S≥1)                                        for S-regular ✓
//   NBSSO           = |E|·(2S²)^70                                                                   for S-regular ✓
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

const T102_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX102");
const T102_EXEC:   ExecutorId = ExecutorId::from_ascii("t102.exec");

const T102_KEY_A: &str = "t102.alpha";
const T102_KEY_B: &str = "t102.beta";
const T102_KEY_C: &str = "t102.gamma";
const T102_KEY_D: &str = "t102.delta";
const T102_KEY_E: &str = "t102.epsilon";

const T102_ID_A: NodeId = derive_node_id(T102_PLUGIN, T102_KEY_A);
const T102_ID_B: NodeId = derive_node_id(T102_PLUGIN, T102_KEY_B);
const T102_ID_C: NodeId = derive_node_id(T102_PLUGIN, T102_KEY_C);
const T102_ID_D: NodeId = derive_node_id(T102_PLUGIN, T102_KEY_D);
const T102_ID_E: NodeId = derive_node_id(T102_PLUGIN, T102_KEY_E);

// L4=189 namespace for this harness.
const T102_VEC_A: VectorAddress = VectorAddress::new(189, 1, 1, 0);
const T102_VEC_B: VectorAddress = VectorAddress::new(189, 1, 2, 0);
const T102_VEC_C: VectorAddress = VectorAddress::new(189, 1, 3, 0);
const T102_VEC_D: VectorAddress = VectorAddress::new(189, 2, 1, 0);
const T102_VEC_E: VectorAddress = VectorAddress::new(189, 2, 2, 0);

const T102_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T102_PLUGIN,
    name:         "kl-graph-topo102-harness",
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
        executor_id:       T102_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T102_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T102_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nheptahexaactc,   0, "empty: NHEPTAHEXAACTC=0");
    assert_eq!(nhheptahexaactc,  0, "empty: NHHEPTAHEXAACTC=0");
    assert_eq!(nbsso,            0, "empty: NBSSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nheptahexaactc,   0, "single: NHEPTAHEXAACTC=0");
    assert_eq!(nhheptahexaactc,  0, "single: NHHEPTAHEXAACTC=0");
    assert_eq!(nbsso,            0, "single: NBSSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTAHEXAACTC:  1^76 + 1^76 = 2.
// NHHEPTAHEXAACTC: (1+1)^75 = 2^75 > u64::MAX → SATURATES.
// NBSSO:           (1²+1²)^70 = 2^70 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);
    add_edge(T102_ID_A, T102_ID_B, "t102.e.ab");

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               2,         "k2: node_count=2");
    assert_eq!(ec,               1,         "k2: edge_count=1");
    assert_eq!(nheptahexaactc,   2,         "k2: NHEPTAHEXAACTC=2 (1^76+1^76=2)");
    assert_eq!(nhheptahexaactc,  u64::MAX,  "k2: NHHEPTAHEXAACTC=SAT (2^75>u64::MAX)");
    assert_eq!(nbsso,            u64::MAX,  "k2: NBSSO=SAT (2^70>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);
    add_node(T102_VEC_C, T102_KEY_C, T102_ID_C);
    add_edge(T102_ID_A, T102_ID_B, "t102.e.ab");
    add_edge(T102_ID_B, T102_ID_C, "t102.e.bc");

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               3,         "p3: node_count=3");
    assert_eq!(ec,               2,         "p3: edge_count=2");
    assert_eq!(nheptahexaactc,   u64::MAX,  "p3: NHEPTAHEXAACTC=SAT (3\u{00d7}2^76>u64)");
    assert_eq!(nhheptahexaactc,  u64::MAX,  "p3: NHHEPTAHEXAACTC=SAT (4^75>u64)");
    assert_eq!(nbsso,            u64::MAX,  "p3: NBSSO=SAT (8^70>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);
    add_node(T102_VEC_C, T102_KEY_C, T102_ID_C);
    add_edge(T102_ID_A, T102_ID_B, "t102.e.ab");
    add_edge(T102_ID_B, T102_ID_C, "t102.e.bc");
    add_edge(T102_ID_C, T102_ID_A, "t102.e.ca");

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nheptahexaactc,   u64::MAX, "k3: NHEPTAHEXAACTC=SAT");
    assert_eq!(nhheptahexaactc,  u64::MAX, "k3: NHHEPTAHEXAACTC=SAT");
    assert_eq!(nbsso,            u64::MAX, "k3: NBSSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A); // hub
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);
    add_node(T102_VEC_C, T102_KEY_C, T102_ID_C);
    add_node(T102_VEC_D, T102_KEY_D, T102_ID_D);
    add_node(T102_VEC_E, T102_KEY_E, T102_ID_E);
    add_edge(T102_ID_A, T102_ID_B, "t102.e.ab");
    add_edge(T102_ID_A, T102_ID_C, "t102.e.ac");
    add_edge(T102_ID_A, T102_ID_D, "t102.e.ad");
    add_edge(T102_ID_A, T102_ID_E, "t102.e.ae");

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nheptahexaactc,   u64::MAX, "k14: NHEPTAHEXAACTC=SAT");
    assert_eq!(nhheptahexaactc,  u64::MAX, "k14: NHHEPTAHEXAACTC=SAT");
    assert_eq!(nbsso,            u64::MAX, "k14: NBSSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);
    add_node(T102_VEC_C, T102_KEY_C, T102_ID_C);
    add_node(T102_VEC_D, T102_KEY_D, T102_ID_D);
    add_edge(T102_ID_A, T102_ID_B, "t102.e.ab");
    add_edge(T102_ID_B, T102_ID_C, "t102.e.bc");
    add_edge(T102_ID_C, T102_ID_D, "t102.e.cd");

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nheptahexaactc,   u64::MAX, "p4: NHEPTAHEXAACTC=SAT");
    assert_eq!(nhheptahexaactc,  u64::MAX, "p4: NHHEPTAHEXAACTC=SAT");
    assert_eq!(nbsso,            u64::MAX, "p4: NBSSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);
    add_node(T102_VEC_C, T102_KEY_C, T102_ID_C);
    add_node(T102_VEC_D, T102_KEY_D, T102_ID_D);
    add_edge(T102_ID_A, T102_ID_B, "t102.e.ab");
    add_edge(T102_ID_A, T102_ID_C, "t102.e.ac");
    add_edge(T102_ID_A, T102_ID_D, "t102.e.ad");
    add_edge(T102_ID_B, T102_ID_C, "t102.e.bc");
    add_edge(T102_ID_B, T102_ID_D, "t102.e.bd");
    add_edge(T102_ID_C, T102_ID_D, "t102.e.cd");

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nheptahexaactc,   u64::MAX, "k4: NHEPTAHEXAACTC=SAT");
    assert_eq!(nhheptahexaactc,  u64::MAX, "k4: NHHEPTAHEXAACTC=SAT");
    assert_eq!(nbsso,            u64::MAX, "k4: NBSSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               2, "2iso: node_count=2");
    assert_eq!(ec,               0, "2iso: edge_count=0");
    assert_eq!(nheptahexaactc,   0, "2iso: NHEPTAHEXAACTC=0");
    assert_eq!(nhheptahexaactc,  0, "2iso: NHHEPTAHEXAACTC=0");
    assert_eq!(nbsso,            0, "2iso: NBSSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T102_VEC_A, T102_KEY_A, T102_ID_A);
    add_node(T102_VEC_B, T102_KEY_B, T102_ID_B);
    add_node(T102_VEC_C, T102_KEY_C, T102_ID_C);
    add_node(T102_VEC_D, T102_KEY_D, T102_ID_D);
    add_node(T102_VEC_E, T102_KEY_E, T102_ID_E);
    add_edge(T102_ID_A, T102_ID_C, "t102.e.ac");
    add_edge(T102_ID_A, T102_ID_D, "t102.e.ad");
    add_edge(T102_ID_A, T102_ID_E, "t102.e.ae");
    add_edge(T102_ID_B, T102_ID_C, "t102.e.bc");
    add_edge(T102_ID_B, T102_ID_D, "t102.e.bd");
    add_edge(T102_ID_B, T102_ID_E, "t102.e.be");

    let (nheptahexaactc, nhheptahexaactc, nbsso, ec, nc) = gos_runtime::graph_topo_indices102();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nheptahexaactc,   u64::MAX, "k23: NHEPTAHEXAACTC=SAT");
    assert_eq!(nhheptahexaactc,  u64::MAX, "k23: NHHEPTAHEXAACTC=SAT");
    assert_eq!(nbsso,            u64::MAX, "k23: NBSSO=SAT");
}
