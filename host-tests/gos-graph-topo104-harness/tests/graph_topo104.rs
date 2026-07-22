// gos-graph-topo104-harness — V3.115 NHEPTAOCTAACTC + NHHEPTAOCTAACTC + NBUSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices104()`:
//   Returns (nheptaoctaactc, nhheptaoctaactc, nbuso, edge_count, node_count)
//   - nheptaoctaactc  = NHEPTAOCTAACTC(G) = Σ_v S(v)^78                         (exact u64; S-Heptaoctacontic vertex sum)
//   - nhheptaoctaactc = NHHEPTAOCTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^77             (exact u64; S-Heptaoctacontic edge-sum)
//   - nbuso            = NBUSO(G)           = Σ_{uv∈E} (S_u²+S_v²)^72           (exact u64; S-Variant Sombor, α=144)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTAOCTAACTC(G) = Σ_v S(v)^78
//     S-Heptaoctacontic vertex sum; ninth of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTAHEPTAACTC=Σ S^77 (topo103) → NHEPTAOCTAACTC=Σ S^78 (topo104).
//     NHEPTAOCTAACTC = n·S^78 for S-regular.
//     Overflow: S^78 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^78 = s64 × s8 × s4 × s2  (78=64+8+4+2; 9 mults total).
//
//   NHHEPTAOCTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^77
//     S-Heptaoctacontic edge-sum; extends NHHEPTAHEPTAACTC=Σ(S+S)^76 (topo103).
//     NHHEPTAOCTAACTC = |E|·(2S)^77 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^77 → saturating u128 accumulator.
//     Implementation: ss^77 = ss64 × ss8 × ss4 × ss  (77=64+8+4+1; 9 mults total).
//
//   NBUSO(G) = Σ_{uv∈E} (S_u²+S_v²)^72
//     S-Variant Sombor: generalised Sombor SO^α with α=144 on S-variant.
//     21st of NB series, letter U (after NBTSO α=142 topo103).
//     NBTSO(topo103,α=142) → NBUSO(topo104,α=144).
//     NBUSO = |E|·(2S²)^72 for S-regular.
//     Overflow per edge: (2×16129²)^72 → saturating u128 accumulator.
//     Implementation: s2s^72 = s2s64 × s2s8  (72=64+8; 7 mults total).
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
//  Graph     NHEPTAOCTAACTC(exact)      NHHEPTAOCTAACTC(exact)     NBUSO(exact)               edges  nodes
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
//     NHEPTAOCTAACTC:  1^78 + 1^78 = 2. ✓
//     NHHEPTAOCTAACTC: (1+1)^77 = 2^77 ≈ 1.51×10^23 > u64::MAX → SATURATES. ✓
//     NBUSO:           (1²+1²)^72 = 2^72 ≈ 4.72×10^21 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTAOCTAACTC:  3×2^78 >> u64::MAX → SATURATES. ✓
//     NHHEPTAOCTAACTC: 2×(4)^77 → SATURATES. ✓
//     NBUSO:           2×(8)^72 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTAOCTAACTC:  3×4^78 → SATURATES. ✓
//     NHHEPTAOCTAACTC: 3×8^77 → SATURATES. ✓
//     NBUSO:           3×32^72 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTAOCTAACTC:  5×4^78 → SATURATES. ✓
//     NHHEPTAOCTAACTC: 4×8^77 → SATURATES. ✓
//     NBUSO:           4×32^72 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTAOCTAACTC:  2×2^78 + 2×3^78. 3^78 >> u64::MAX → SATURATES. ✓
//     NHHEPTAOCTAACTC: 5^77+6^77+5^77 → SATURATES. ✓
//     NBUSO:           13^72+18^72+13^72 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTAOCTAACTC:  4×9^78 → SATURATES. ✓
//     NHHEPTAOCTAACTC: 6×18^77 → SATURATES. ✓
//     NBUSO:           6×162^72 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTAOCTAACTC:  5×6^78 → SATURATES. ✓
//     NHHEPTAOCTAACTC: 6×12^77 → SATURATES. ✓
//     NBUSO:           6×72^72 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTAOCTAACTC  = n·S^78                                                                        for S-regular ✓
//   NHHEPTAOCTAACTC = |E|·(2S)^77 (saturates for |E|≥1,S≥1)                                       for S-regular ✓
//   NBUSO           = |E|·(2S²)^72                                                                  for S-regular ✓
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

const T104_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX104");
const T104_EXEC:   ExecutorId = ExecutorId::from_ascii("t104.exec");

const T104_KEY_A: &str = "t104.alpha";
const T104_KEY_B: &str = "t104.beta";
const T104_KEY_C: &str = "t104.gamma";
const T104_KEY_D: &str = "t104.delta";
const T104_KEY_E: &str = "t104.epsilon";

const T104_ID_A: NodeId = derive_node_id(T104_PLUGIN, T104_KEY_A);
const T104_ID_B: NodeId = derive_node_id(T104_PLUGIN, T104_KEY_B);
const T104_ID_C: NodeId = derive_node_id(T104_PLUGIN, T104_KEY_C);
const T104_ID_D: NodeId = derive_node_id(T104_PLUGIN, T104_KEY_D);
const T104_ID_E: NodeId = derive_node_id(T104_PLUGIN, T104_KEY_E);

// L4=191 namespace for this harness.
const T104_VEC_A: VectorAddress = VectorAddress::new(191, 1, 1, 0);
const T104_VEC_B: VectorAddress = VectorAddress::new(191, 1, 2, 0);
const T104_VEC_C: VectorAddress = VectorAddress::new(191, 1, 3, 0);
const T104_VEC_D: VectorAddress = VectorAddress::new(191, 2, 1, 0);
const T104_VEC_E: VectorAddress = VectorAddress::new(191, 2, 2, 0);

const T104_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T104_PLUGIN,
    name:         "kl-graph-topo104-harness",
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
        executor_id:       T104_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T104_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T104_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nheptaoctaactc,   0, "empty: NHEPTAOCTAACTC=0");
    assert_eq!(nhheptaoctaactc,  0, "empty: NHHEPTAOCTAACTC=0");
    assert_eq!(nbuso,            0, "empty: NBUSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nheptaoctaactc,   0, "single: NHEPTAOCTAACTC=0");
    assert_eq!(nhheptaoctaactc,  0, "single: NHHEPTAOCTAACTC=0");
    assert_eq!(nbuso,            0, "single: NBUSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTAOCTAACTC:  1^78 + 1^78 = 2.
// NHHEPTAOCTAACTC: (1+1)^77 = 2^77 > u64::MAX → SATURATES.
// NBUSO:           (1²+1²)^72 = 2^72 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);
    add_edge(T104_ID_A, T104_ID_B, "t104.e.ab");

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               2,        "k2: node_count=2");
    assert_eq!(ec,               1,        "k2: edge_count=1");
    assert_eq!(nheptaoctaactc,   2,        "k2: NHEPTAOCTAACTC=2 (1^78+1^78=2)");
    assert_eq!(nhheptaoctaactc,  u64::MAX, "k2: NHHEPTAOCTAACTC=SAT (2^77>u64::MAX)");
    assert_eq!(nbuso,            u64::MAX, "k2: NBUSO=SAT (2^72>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);
    add_node(T104_VEC_C, T104_KEY_C, T104_ID_C);
    add_edge(T104_ID_A, T104_ID_B, "t104.e.ab");
    add_edge(T104_ID_B, T104_ID_C, "t104.e.bc");

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               3,        "p3: node_count=3");
    assert_eq!(ec,               2,        "p3: edge_count=2");
    assert_eq!(nheptaoctaactc,   u64::MAX, "p3: NHEPTAOCTAACTC=SAT (3\u{00d7}2^78>u64)");
    assert_eq!(nhheptaoctaactc,  u64::MAX, "p3: NHHEPTAOCTAACTC=SAT (4^77>u64)");
    assert_eq!(nbuso,            u64::MAX, "p3: NBUSO=SAT (8^72>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);
    add_node(T104_VEC_C, T104_KEY_C, T104_ID_C);
    add_edge(T104_ID_A, T104_ID_B, "t104.e.ab");
    add_edge(T104_ID_B, T104_ID_C, "t104.e.bc");
    add_edge(T104_ID_C, T104_ID_A, "t104.e.ca");

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nheptaoctaactc,   u64::MAX, "k3: NHEPTAOCTAACTC=SAT");
    assert_eq!(nhheptaoctaactc,  u64::MAX, "k3: NHHEPTAOCTAACTC=SAT");
    assert_eq!(nbuso,            u64::MAX, "k3: NBUSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A); // hub
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);
    add_node(T104_VEC_C, T104_KEY_C, T104_ID_C);
    add_node(T104_VEC_D, T104_KEY_D, T104_ID_D);
    add_node(T104_VEC_E, T104_KEY_E, T104_ID_E);
    add_edge(T104_ID_A, T104_ID_B, "t104.e.ab");
    add_edge(T104_ID_A, T104_ID_C, "t104.e.ac");
    add_edge(T104_ID_A, T104_ID_D, "t104.e.ad");
    add_edge(T104_ID_A, T104_ID_E, "t104.e.ae");

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nheptaoctaactc,   u64::MAX, "k14: NHEPTAOCTAACTC=SAT");
    assert_eq!(nhheptaoctaactc,  u64::MAX, "k14: NHHEPTAOCTAACTC=SAT");
    assert_eq!(nbuso,            u64::MAX, "k14: NBUSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);
    add_node(T104_VEC_C, T104_KEY_C, T104_ID_C);
    add_node(T104_VEC_D, T104_KEY_D, T104_ID_D);
    add_edge(T104_ID_A, T104_ID_B, "t104.e.ab");
    add_edge(T104_ID_B, T104_ID_C, "t104.e.bc");
    add_edge(T104_ID_C, T104_ID_D, "t104.e.cd");

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nheptaoctaactc,   u64::MAX, "p4: NHEPTAOCTAACTC=SAT");
    assert_eq!(nhheptaoctaactc,  u64::MAX, "p4: NHHEPTAOCTAACTC=SAT");
    assert_eq!(nbuso,            u64::MAX, "p4: NBUSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);
    add_node(T104_VEC_C, T104_KEY_C, T104_ID_C);
    add_node(T104_VEC_D, T104_KEY_D, T104_ID_D);
    add_edge(T104_ID_A, T104_ID_B, "t104.e.ab");
    add_edge(T104_ID_A, T104_ID_C, "t104.e.ac");
    add_edge(T104_ID_A, T104_ID_D, "t104.e.ad");
    add_edge(T104_ID_B, T104_ID_C, "t104.e.bc");
    add_edge(T104_ID_B, T104_ID_D, "t104.e.bd");
    add_edge(T104_ID_C, T104_ID_D, "t104.e.cd");

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nheptaoctaactc,   u64::MAX, "k4: NHEPTAOCTAACTC=SAT");
    assert_eq!(nhheptaoctaactc,  u64::MAX, "k4: NHHEPTAOCTAACTC=SAT");
    assert_eq!(nbuso,            u64::MAX, "k4: NBUSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               2, "2iso: node_count=2");
    assert_eq!(ec,               0, "2iso: edge_count=0");
    assert_eq!(nheptaoctaactc,   0, "2iso: NHEPTAOCTAACTC=0");
    assert_eq!(nhheptaoctaactc,  0, "2iso: NHHEPTAOCTAACTC=0");
    assert_eq!(nbuso,            0, "2iso: NBUSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T104_VEC_A, T104_KEY_A, T104_ID_A);
    add_node(T104_VEC_B, T104_KEY_B, T104_ID_B);
    add_node(T104_VEC_C, T104_KEY_C, T104_ID_C);
    add_node(T104_VEC_D, T104_KEY_D, T104_ID_D);
    add_node(T104_VEC_E, T104_KEY_E, T104_ID_E);
    add_edge(T104_ID_A, T104_ID_C, "t104.e.ac");
    add_edge(T104_ID_A, T104_ID_D, "t104.e.ad");
    add_edge(T104_ID_A, T104_ID_E, "t104.e.ae");
    add_edge(T104_ID_B, T104_ID_C, "t104.e.bc");
    add_edge(T104_ID_B, T104_ID_D, "t104.e.bd");
    add_edge(T104_ID_B, T104_ID_E, "t104.e.be");

    let (nheptaoctaactc, nhheptaoctaactc, nbuso, ec, nc) = gos_runtime::graph_topo_indices104();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nheptaoctaactc,   u64::MAX, "k23: NHEPTAOCTAACTC=SAT");
    assert_eq!(nhheptaoctaactc,  u64::MAX, "k23: NHHEPTAOCTAACTC=SAT");
    assert_eq!(nbuso,            u64::MAX, "k23: NBUSO=SAT");
}
