// gos-graph-topo105-harness — V3.116 NHEPTAENNACTC + NHHEPTAENNACTC + NBVSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices105()`:
//   Returns (nheptaennactc, nhheptaennactc, nbvso, edge_count, node_count)
//   - nheptaennactc  = NHEPTAENNACTC(G) = Σ_v S(v)^79                          (exact u64; S-Heptaennacontic vertex sum)
//   - nhheptaennactc = NHHEPTAENNACTC(G) = Σ_{uv∈E} (S_u+S_v)^78              (exact u64; S-Heptaennacontic edge-sum)
//   - nbvso           = NBVSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^73            (exact u64; S-Variant Sombor, α=146)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTAENNACTC(G) = Σ_v S(v)^79
//     S-Heptaennacontic vertex sum; tenth (and final) of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTAOCTAACTC=Σ S^78 (topo104) → NHEPTAENNACTC=Σ S^79 (topo105).
//     NHEPTAENNACTC = n·S^79 for S-regular.
//     Overflow: S^79 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^79 = s64 × s8 × s4 × s2 × s  (79=64+8+4+2+1; 10 mults total).
//
//   NHHEPTAENNACTC(G) = Σ_{uv∈E} (S_u+S_v)^78
//     S-Heptaennacontic edge-sum; extends NHHEPTAOCTAACTC=Σ(S+S)^77 (topo104).
//     NHHEPTAENNACTC = |E|·(2S)^78 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^78 → saturating u128 accumulator.
//     Implementation: ss^78 = ss64 × ss8 × ss4 × ss2  (78=64+8+4+2; 9 mults total).
//
//   NBVSO(G) = Σ_{uv∈E} (S_u²+S_v²)^73
//     S-Variant Sombor: generalised Sombor SO^α with α=146 on S-variant.
//     22nd of NB series, letter V (after NBUSO α=144 topo104).
//     NBUSO(topo104,α=144) → NBVSO(topo105,α=146).
//     NBVSO = |E|·(2S²)^73 for S-regular.
//     Overflow per edge: (2×16129²)^73 → saturating u128 accumulator.
//     Implementation: s2s^73 = s2s64 × s2s8 × s2s  (73=64+8+1; 8 mults total).
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
//  Graph     NHEPTAENNACTC(exact)       NHHEPTAENNACTC(exact)      NBVSO(exact)               edges  nodes
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
//     NHEPTAENNACTC:  1^79 + 1^79 = 2. ✓
//     NHHEPTAENNACTC: (1+1)^78 = 2^78 ≈ 3.02×10^23 > u64::MAX → SATURATES. ✓
//     NBVSO:          (1²+1²)^73 = 2^73 ≈ 9.44×10^21 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTAENNACTC:  3×2^79 >> u64::MAX → SATURATES. ✓
//     NHHEPTAENNACTC: 2×(4)^78 → SATURATES. ✓
//     NBVSO:          2×(8)^73 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTAENNACTC:  3×4^79 → SATURATES. ✓
//     NHHEPTAENNACTC: 3×8^78 → SATURATES. ✓
//     NBVSO:          3×32^73 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTAENNACTC:  5×4^79 → SATURATES. ✓
//     NHHEPTAENNACTC: 4×8^78 → SATURATES. ✓
//     NBVSO:          4×32^73 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTAENNACTC:  2×2^79 + 2×3^79. 3^79 >> u64::MAX → SATURATES. ✓
//     NHHEPTAENNACTC: 5^78+6^78+5^78 → SATURATES. ✓
//     NBVSO:          13^73+18^73+13^73 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTAENNACTC:  4×9^79 → SATURATES. ✓
//     NHHEPTAENNACTC: 6×18^78 → SATURATES. ✓
//     NBVSO:          6×162^73 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTAENNACTC:  5×6^79 → SATURATES. ✓
//     NHHEPTAENNACTC: 6×12^78 → SATURATES. ✓
//     NBVSO:          6×72^73 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTAENNACTC  = n·S^79                                                                       for S-regular ✓
//   NHHEPTAENNACTC = |E|·(2S)^78 (saturates for |E|≥1,S≥1)                                      for S-regular ✓
//   NBVSO          = |E|·(2S²)^73                                                                 for S-regular ✓
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

const T105_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX105");
const T105_EXEC:   ExecutorId = ExecutorId::from_ascii("t105.exec");

const T105_KEY_A: &str = "t105.alpha";
const T105_KEY_B: &str = "t105.beta";
const T105_KEY_C: &str = "t105.gamma";
const T105_KEY_D: &str = "t105.delta";
const T105_KEY_E: &str = "t105.epsilon";

const T105_ID_A: NodeId = derive_node_id(T105_PLUGIN, T105_KEY_A);
const T105_ID_B: NodeId = derive_node_id(T105_PLUGIN, T105_KEY_B);
const T105_ID_C: NodeId = derive_node_id(T105_PLUGIN, T105_KEY_C);
const T105_ID_D: NodeId = derive_node_id(T105_PLUGIN, T105_KEY_D);
const T105_ID_E: NodeId = derive_node_id(T105_PLUGIN, T105_KEY_E);

// L4=192 namespace for this harness.
const T105_VEC_A: VectorAddress = VectorAddress::new(192, 1, 1, 0);
const T105_VEC_B: VectorAddress = VectorAddress::new(192, 1, 2, 0);
const T105_VEC_C: VectorAddress = VectorAddress::new(192, 1, 3, 0);
const T105_VEC_D: VectorAddress = VectorAddress::new(192, 2, 1, 0);
const T105_VEC_E: VectorAddress = VectorAddress::new(192, 2, 2, 0);

const T105_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T105_PLUGIN,
    name:         "kl-graph-topo105-harness",
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
        executor_id:       T105_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T105_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T105_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nheptaennactc,    0, "empty: NHEPTAENNACTC=0");
    assert_eq!(nhheptaennactc,   0, "empty: NHHEPTAENNACTC=0");
    assert_eq!(nbvso,            0, "empty: NBVSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nheptaennactc,    0, "single: NHEPTAENNACTC=0");
    assert_eq!(nhheptaennactc,   0, "single: NHHEPTAENNACTC=0");
    assert_eq!(nbvso,            0, "single: NBVSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTAENNACTC:  1^79 + 1^79 = 2.
// NHHEPTAENNACTC: (1+1)^78 = 2^78 > u64::MAX → SATURATES.
// NBVSO:          (1²+1²)^73 = 2^73 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);
    add_edge(T105_ID_A, T105_ID_B, "t105.e.ab");

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               2,        "k2: node_count=2");
    assert_eq!(ec,               1,        "k2: edge_count=1");
    assert_eq!(nheptaennactc,    2,        "k2: NHEPTAENNACTC=2 (1^79+1^79=2)");
    assert_eq!(nhheptaennactc,   u64::MAX, "k2: NHHEPTAENNACTC=SAT (2^78>u64::MAX)");
    assert_eq!(nbvso,            u64::MAX, "k2: NBVSO=SAT (2^73>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);
    add_node(T105_VEC_C, T105_KEY_C, T105_ID_C);
    add_edge(T105_ID_A, T105_ID_B, "t105.e.ab");
    add_edge(T105_ID_B, T105_ID_C, "t105.e.bc");

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               3,        "p3: node_count=3");
    assert_eq!(ec,               2,        "p3: edge_count=2");
    assert_eq!(nheptaennactc,    u64::MAX, "p3: NHEPTAENNACTC=SAT (3\u{00d7}2^79>u64)");
    assert_eq!(nhheptaennactc,   u64::MAX, "p3: NHHEPTAENNACTC=SAT (4^78>u64)");
    assert_eq!(nbvso,            u64::MAX, "p3: NBVSO=SAT (8^73>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);
    add_node(T105_VEC_C, T105_KEY_C, T105_ID_C);
    add_edge(T105_ID_A, T105_ID_B, "t105.e.ab");
    add_edge(T105_ID_B, T105_ID_C, "t105.e.bc");
    add_edge(T105_ID_C, T105_ID_A, "t105.e.ca");

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nheptaennactc,    u64::MAX, "k3: NHEPTAENNACTC=SAT");
    assert_eq!(nhheptaennactc,   u64::MAX, "k3: NHHEPTAENNACTC=SAT");
    assert_eq!(nbvso,            u64::MAX, "k3: NBVSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A); // hub
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);
    add_node(T105_VEC_C, T105_KEY_C, T105_ID_C);
    add_node(T105_VEC_D, T105_KEY_D, T105_ID_D);
    add_node(T105_VEC_E, T105_KEY_E, T105_ID_E);
    add_edge(T105_ID_A, T105_ID_B, "t105.e.ab");
    add_edge(T105_ID_A, T105_ID_C, "t105.e.ac");
    add_edge(T105_ID_A, T105_ID_D, "t105.e.ad");
    add_edge(T105_ID_A, T105_ID_E, "t105.e.ae");

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nheptaennactc,    u64::MAX, "k14: NHEPTAENNACTC=SAT");
    assert_eq!(nhheptaennactc,   u64::MAX, "k14: NHHEPTAENNACTC=SAT");
    assert_eq!(nbvso,            u64::MAX, "k14: NBVSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);
    add_node(T105_VEC_C, T105_KEY_C, T105_ID_C);
    add_node(T105_VEC_D, T105_KEY_D, T105_ID_D);
    add_edge(T105_ID_A, T105_ID_B, "t105.e.ab");
    add_edge(T105_ID_B, T105_ID_C, "t105.e.bc");
    add_edge(T105_ID_C, T105_ID_D, "t105.e.cd");

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nheptaennactc,    u64::MAX, "p4: NHEPTAENNACTC=SAT");
    assert_eq!(nhheptaennactc,   u64::MAX, "p4: NHHEPTAENNACTC=SAT");
    assert_eq!(nbvso,            u64::MAX, "p4: NBVSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);
    add_node(T105_VEC_C, T105_KEY_C, T105_ID_C);
    add_node(T105_VEC_D, T105_KEY_D, T105_ID_D);
    add_edge(T105_ID_A, T105_ID_B, "t105.e.ab");
    add_edge(T105_ID_A, T105_ID_C, "t105.e.ac");
    add_edge(T105_ID_A, T105_ID_D, "t105.e.ad");
    add_edge(T105_ID_B, T105_ID_C, "t105.e.bc");
    add_edge(T105_ID_B, T105_ID_D, "t105.e.bd");
    add_edge(T105_ID_C, T105_ID_D, "t105.e.cd");

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nheptaennactc,    u64::MAX, "k4: NHEPTAENNACTC=SAT");
    assert_eq!(nhheptaennactc,   u64::MAX, "k4: NHHEPTAENNACTC=SAT");
    assert_eq!(nbvso,            u64::MAX, "k4: NBVSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               2, "2iso: node_count=2");
    assert_eq!(ec,               0, "2iso: edge_count=0");
    assert_eq!(nheptaennactc,    0, "2iso: NHEPTAENNACTC=0");
    assert_eq!(nhheptaennactc,   0, "2iso: NHHEPTAENNACTC=0");
    assert_eq!(nbvso,            0, "2iso: NBVSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T105_VEC_A, T105_KEY_A, T105_ID_A);
    add_node(T105_VEC_B, T105_KEY_B, T105_ID_B);
    add_node(T105_VEC_C, T105_KEY_C, T105_ID_C);
    add_node(T105_VEC_D, T105_KEY_D, T105_ID_D);
    add_node(T105_VEC_E, T105_KEY_E, T105_ID_E);
    add_edge(T105_ID_A, T105_ID_C, "t105.e.ac");
    add_edge(T105_ID_A, T105_ID_D, "t105.e.ad");
    add_edge(T105_ID_A, T105_ID_E, "t105.e.ae");
    add_edge(T105_ID_B, T105_ID_C, "t105.e.bc");
    add_edge(T105_ID_B, T105_ID_D, "t105.e.bd");
    add_edge(T105_ID_B, T105_ID_E, "t105.e.be");

    let (nheptaennactc, nhheptaennactc, nbvso, ec, nc) = gos_runtime::graph_topo_indices105();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nheptaennactc,    u64::MAX, "k23: NHEPTAENNACTC=SAT");
    assert_eq!(nhheptaennactc,   u64::MAX, "k23: NHHEPTAENNACTC=SAT");
    assert_eq!(nbvso,            u64::MAX, "k23: NBVSO=SAT");
}
