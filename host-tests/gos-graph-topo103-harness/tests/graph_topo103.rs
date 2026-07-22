// gos-graph-topo103-harness — V3.114 NHEPTAHEPTAACTC + NHHEPTAHEPTAACTC + NBTSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices103()`:
//   Returns (nheptaheptaactc, nhheptaheptaactc, nbtso, edge_count, node_count)
//   - nheptaheptaactc  = NHEPTAHEPTAACTC(G) = Σ_v S(v)^77                         (exact u64; S-Heptaheptacontic vertex sum)
//   - nhheptaheptaactc = NHHEPTAHEPTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^76             (exact u64; S-Heptaheptacontic edge-sum)
//   - nbtso            = NBTSO(G)            = Σ_{uv∈E} (S_u²+S_v²)^71           (exact u64; S-Variant Sombor, α=142)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTAHEPTAACTC(G) = Σ_v S(v)^77
//     S-Heptaheptacontic vertex sum; eighth of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTAHEXAACTC=Σ S^76 (topo102) → NHEPTAHEPTAACTC=Σ S^77 (topo103).
//     NHEPTAHEPTAACTC = n·S^77 for S-regular.
//     Overflow: S^77 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^77 = s64 × s8 × s4 × s  (77=64+8+4+1; 9 mults total).
//
//   NHHEPTAHEPTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^76
//     S-Heptaheptacontic edge-sum; extends NHHEPTAHEXAACTC=Σ(S+S)^75 (topo102).
//     NHHEPTAHEPTAACTC = |E|·(2S)^76 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^76 → saturating u128 accumulator.
//     Implementation: ss^76 = ss64 × ss8 × ss4  (76=64+8+4; 8 mults total).
//
//   NBTSO(G) = Σ_{uv∈E} (S_u²+S_v²)^71
//     S-Variant Sombor: generalised Sombor SO^α with α=142 on S-variant.
//     20th of NB series, letter T (after NBSSO α=140 topo102).
//     NBSSO(topo102,α=140) → NBTSO(topo103,α=142).
//     NBTSO = |E|·(2S²)^71 for S-regular.
//     Overflow per edge: (2×16129²)^71 → saturating u128 accumulator.
//     Implementation: s2s^71 = s2s64 × s2s4 × s2s2 × s2s  (71=64+4+2+1; 9 mults total).
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
//  Graph     NHEPTAHEPTAACTC(exact)     NHHEPTAHEPTAACTC(exact)    NBTSO(exact)               edges  nodes
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
//     NHEPTAHEPTAACTC:  1^77 + 1^77 = 2. ✓
//     NHHEPTAHEPTAACTC: (1+1)^76 = 2^76 ≈ 7.56×10^22 > u64::MAX → SATURATES. ✓
//     NBTSO:            (1²+1²)^71 = 2^71 ≈ 2.36×10^21 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTAHEPTAACTC:  3×2^77 >> u64::MAX → SATURATES. ✓
//     NHHEPTAHEPTAACTC: 2×(4)^76 → SATURATES. ✓
//     NBTSO:            2×(8)^71 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTAHEPTAACTC:  3×4^77 → SATURATES. ✓
//     NHHEPTAHEPTAACTC: 3×8^76 → SATURATES. ✓
//     NBTSO:            3×32^71 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTAHEPTAACTC:  5×4^77 → SATURATES. ✓
//     NHHEPTAHEPTAACTC: 4×8^76 → SATURATES. ✓
//     NBTSO:            4×32^71 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTAHEPTAACTC:  2×2^77 + 2×3^77. 3^77 >> u64::MAX → SATURATES. ✓
//     NHHEPTAHEPTAACTC: 5^76+6^76+5^76 → SATURATES. ✓
//     NBTSO:            13^71+18^71+13^71 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTAHEPTAACTC:  4×9^77 → SATURATES. ✓
//     NHHEPTAHEPTAACTC: 6×18^76 → SATURATES. ✓
//     NBTSO:            6×162^71 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTAHEPTAACTC:  5×6^77 → SATURATES. ✓
//     NHHEPTAHEPTAACTC: 6×12^76 → SATURATES. ✓
//     NBTSO:            6×72^71 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTAHEPTAACTC  = n·S^77                                                                        for S-regular ✓
//   NHHEPTAHEPTAACTC = |E|·(2S)^76 (saturates for |E|≥1,S≥1)                                       for S-regular ✓
//   NBTSO            = |E|·(2S²)^71                                                                  for S-regular ✓
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

const T103_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX103");
const T103_EXEC:   ExecutorId = ExecutorId::from_ascii("t103.exec");

const T103_KEY_A: &str = "t103.alpha";
const T103_KEY_B: &str = "t103.beta";
const T103_KEY_C: &str = "t103.gamma";
const T103_KEY_D: &str = "t103.delta";
const T103_KEY_E: &str = "t103.epsilon";

const T103_ID_A: NodeId = derive_node_id(T103_PLUGIN, T103_KEY_A);
const T103_ID_B: NodeId = derive_node_id(T103_PLUGIN, T103_KEY_B);
const T103_ID_C: NodeId = derive_node_id(T103_PLUGIN, T103_KEY_C);
const T103_ID_D: NodeId = derive_node_id(T103_PLUGIN, T103_KEY_D);
const T103_ID_E: NodeId = derive_node_id(T103_PLUGIN, T103_KEY_E);

// L4=190 namespace for this harness.
const T103_VEC_A: VectorAddress = VectorAddress::new(190, 1, 1, 0);
const T103_VEC_B: VectorAddress = VectorAddress::new(190, 1, 2, 0);
const T103_VEC_C: VectorAddress = VectorAddress::new(190, 1, 3, 0);
const T103_VEC_D: VectorAddress = VectorAddress::new(190, 2, 1, 0);
const T103_VEC_E: VectorAddress = VectorAddress::new(190, 2, 2, 0);

const T103_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T103_PLUGIN,
    name:         "kl-graph-topo103-harness",
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
        executor_id:       T103_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T103_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T103_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(nheptaheptaactc,   0, "empty: NHEPTAHEPTAACTC=0");
    assert_eq!(nhheptaheptaactc,  0, "empty: NHHEPTAHEPTAACTC=0");
    assert_eq!(nbtso,             0, "empty: NBTSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(nheptaheptaactc,   0, "single: NHEPTAHEPTAACTC=0");
    assert_eq!(nhheptaheptaactc,  0, "single: NHHEPTAHEPTAACTC=0");
    assert_eq!(nbtso,             0, "single: NBTSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTAHEPTAACTC:  1^77 + 1^77 = 2.
// NHHEPTAHEPTAACTC: (1+1)^76 = 2^76 > u64::MAX → SATURATES.
// NBTSO:            (1²+1²)^71 = 2^71 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);
    add_edge(T103_ID_A, T103_ID_B, "t103.e.ab");

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                2,         "k2: node_count=2");
    assert_eq!(ec,                1,         "k2: edge_count=1");
    assert_eq!(nheptaheptaactc,   2,         "k2: NHEPTAHEPTAACTC=2 (1^77+1^77=2)");
    assert_eq!(nhheptaheptaactc,  u64::MAX,  "k2: NHHEPTAHEPTAACTC=SAT (2^76>u64::MAX)");
    assert_eq!(nbtso,             u64::MAX,  "k2: NBTSO=SAT (2^71>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);
    add_node(T103_VEC_C, T103_KEY_C, T103_ID_C);
    add_edge(T103_ID_A, T103_ID_B, "t103.e.ab");
    add_edge(T103_ID_B, T103_ID_C, "t103.e.bc");

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                3,         "p3: node_count=3");
    assert_eq!(ec,                2,         "p3: edge_count=2");
    assert_eq!(nheptaheptaactc,   u64::MAX,  "p3: NHEPTAHEPTAACTC=SAT (3\u{00d7}2^77>u64)");
    assert_eq!(nhheptaheptaactc,  u64::MAX,  "p3: NHHEPTAHEPTAACTC=SAT (4^76>u64)");
    assert_eq!(nbtso,             u64::MAX,  "p3: NBTSO=SAT (8^71>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);
    add_node(T103_VEC_C, T103_KEY_C, T103_ID_C);
    add_edge(T103_ID_A, T103_ID_B, "t103.e.ab");
    add_edge(T103_ID_B, T103_ID_C, "t103.e.bc");
    add_edge(T103_ID_C, T103_ID_A, "t103.e.ca");

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                3,        "k3: node_count=3");
    assert_eq!(ec,                3,        "k3: edge_count=3");
    assert_eq!(nheptaheptaactc,   u64::MAX, "k3: NHEPTAHEPTAACTC=SAT");
    assert_eq!(nhheptaheptaactc,  u64::MAX, "k3: NHHEPTAHEPTAACTC=SAT");
    assert_eq!(nbtso,             u64::MAX, "k3: NBTSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A); // hub
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);
    add_node(T103_VEC_C, T103_KEY_C, T103_ID_C);
    add_node(T103_VEC_D, T103_KEY_D, T103_ID_D);
    add_node(T103_VEC_E, T103_KEY_E, T103_ID_E);
    add_edge(T103_ID_A, T103_ID_B, "t103.e.ab");
    add_edge(T103_ID_A, T103_ID_C, "t103.e.ac");
    add_edge(T103_ID_A, T103_ID_D, "t103.e.ad");
    add_edge(T103_ID_A, T103_ID_E, "t103.e.ae");

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                5,        "k14: node_count=5");
    assert_eq!(ec,                4,        "k14: edge_count=4");
    assert_eq!(nheptaheptaactc,   u64::MAX, "k14: NHEPTAHEPTAACTC=SAT");
    assert_eq!(nhheptaheptaactc,  u64::MAX, "k14: NHHEPTAHEPTAACTC=SAT");
    assert_eq!(nbtso,             u64::MAX, "k14: NBTSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);
    add_node(T103_VEC_C, T103_KEY_C, T103_ID_C);
    add_node(T103_VEC_D, T103_KEY_D, T103_ID_D);
    add_edge(T103_ID_A, T103_ID_B, "t103.e.ab");
    add_edge(T103_ID_B, T103_ID_C, "t103.e.bc");
    add_edge(T103_ID_C, T103_ID_D, "t103.e.cd");

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                4,        "p4: node_count=4");
    assert_eq!(ec,                3,        "p4: edge_count=3");
    assert_eq!(nheptaheptaactc,   u64::MAX, "p4: NHEPTAHEPTAACTC=SAT");
    assert_eq!(nhheptaheptaactc,  u64::MAX, "p4: NHHEPTAHEPTAACTC=SAT");
    assert_eq!(nbtso,             u64::MAX, "p4: NBTSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);
    add_node(T103_VEC_C, T103_KEY_C, T103_ID_C);
    add_node(T103_VEC_D, T103_KEY_D, T103_ID_D);
    add_edge(T103_ID_A, T103_ID_B, "t103.e.ab");
    add_edge(T103_ID_A, T103_ID_C, "t103.e.ac");
    add_edge(T103_ID_A, T103_ID_D, "t103.e.ad");
    add_edge(T103_ID_B, T103_ID_C, "t103.e.bc");
    add_edge(T103_ID_B, T103_ID_D, "t103.e.bd");
    add_edge(T103_ID_C, T103_ID_D, "t103.e.cd");

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                4,        "k4: node_count=4");
    assert_eq!(ec,                6,        "k4: edge_count=6");
    assert_eq!(nheptaheptaactc,   u64::MAX, "k4: NHEPTAHEPTAACTC=SAT");
    assert_eq!(nhheptaheptaactc,  u64::MAX, "k4: NHHEPTAHEPTAACTC=SAT");
    assert_eq!(nbtso,             u64::MAX, "k4: NBTSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                2, "2iso: node_count=2");
    assert_eq!(ec,                0, "2iso: edge_count=0");
    assert_eq!(nheptaheptaactc,   0, "2iso: NHEPTAHEPTAACTC=0");
    assert_eq!(nhheptaheptaactc,  0, "2iso: NHHEPTAHEPTAACTC=0");
    assert_eq!(nbtso,             0, "2iso: NBTSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T103_VEC_A, T103_KEY_A, T103_ID_A);
    add_node(T103_VEC_B, T103_KEY_B, T103_ID_B);
    add_node(T103_VEC_C, T103_KEY_C, T103_ID_C);
    add_node(T103_VEC_D, T103_KEY_D, T103_ID_D);
    add_node(T103_VEC_E, T103_KEY_E, T103_ID_E);
    add_edge(T103_ID_A, T103_ID_C, "t103.e.ac");
    add_edge(T103_ID_A, T103_ID_D, "t103.e.ad");
    add_edge(T103_ID_A, T103_ID_E, "t103.e.ae");
    add_edge(T103_ID_B, T103_ID_C, "t103.e.bc");
    add_edge(T103_ID_B, T103_ID_D, "t103.e.bd");
    add_edge(T103_ID_B, T103_ID_E, "t103.e.be");

    let (nheptaheptaactc, nhheptaheptaactc, nbtso, ec, nc) = gos_runtime::graph_topo_indices103();
    assert_eq!(nc,                5,        "k23: node_count=5");
    assert_eq!(ec,                6,        "k23: edge_count=6");
    assert_eq!(nheptaheptaactc,   u64::MAX, "k23: NHEPTAHEPTAACTC=SAT");
    assert_eq!(nhheptaheptaactc,  u64::MAX, "k23: NHHEPTAHEPTAACTC=SAT");
    assert_eq!(nbtso,             u64::MAX, "k23: NBTSO=SAT");
}
