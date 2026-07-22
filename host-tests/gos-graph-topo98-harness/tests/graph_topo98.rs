// gos-graph-topo98-harness — V3.109 NHEPTADIACTC + NHHEPTADIACTC + NBOSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices98()`:
//   Returns (nheptadiactc, nhheptadiactc, nboso, edge_count, node_count)
//   - nheptadiactc  = NHEPTADIACTC(G) = Σ_v S(v)^72                        (exact u64; S-Heptadicontic vertex sum)
//   - nhheptadiactc = NHHEPTADIACTC(G) = Σ_{uv∈E} (S_u+S_v)^71            (exact u64; S-Heptadicontic edge-sum)
//   - nboso          = NBOSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^66         (exact u64; S-Variant Sombor, α=132)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTADIACTC(G) = Σ_v S(v)^72
//     S-Heptadicontic vertex sum; third of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTAENACTC=Σ S^71 (topo97) → NHEPTADIACTC=Σ S^72 (topo98).
//     NHEPTADIACTC = n·S^72 for S-regular.
//     Overflow: S^72 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^72 = s64 × s8  (72=64+8; 7 mults total — both powers of 2, efficient!).
//
//   NHHEPTADIACTC(G) = Σ_{uv∈E} (S_u+S_v)^71
//     S-Heptadicontic edge-sum; extends NHHEPTAENACTC=Σ(S+S)^70 (topo97).
//     NHHEPTADIACTC = |E|·(2S)^71 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^71 → saturating u128 accumulator.
//     Implementation: ss^71 = ss64 × ss4 × ss2 × ss  (71=64+4+2+1; 9 mults total).
//
//   NBOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^66
//     S-Variant Sombor: generalised Sombor SO^α with α=132 on S-variant.
//     15th of NB series, letter O (after NBNSO α=130 topo97).
//     NBNSO(topo97,α=130) → NBOSO(topo98,α=132).
//     NBOSO = |E|·(2S²)^66 for S-regular.
//     Overflow per edge: (2×16129²)^66 → saturating u128 accumulator.
//     Implementation: s2s^66 = s2s64 × s2s2  (66=64+2; 7 mults total — efficient!).
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
//  Graph     NHEPTADIACTC(exact)        NHHEPTADIACTC(exact)       NBOSO(exact)               edges  nodes
//  Empty                      0                             0                   0                   0      0
//  1 node                     0                             0                   0                   0      1
//  K₂                         2             u64::MAX(sat.)      u64::MAX(sat.)                    1      2
//  P₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               2      3
//  K₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               3      3
//  K_{1,4}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               4      5
//  P₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               3      4
//  K₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               6      4
//  2 isolated                 0                             0                   0                   0      2
//  K_{2,3}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTADIACTC:  1^72 + 1^72 = 2. ✓
//     NHHEPTADIACTC: (1+1)^71 = 2^71 ≈ 2.36×10^21 > u64::MAX → SATURATES. ✓
//     NBOSO:         (1²+1²)^66 = 2^66 ≈ 7.38×10^19 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTADIACTC:  3×2^72 >> u64::MAX → SATURATES. ✓
//     NHHEPTADIACTC: 2×(4)^71 → SATURATES. ✓
//     NBOSO:         2×(8)^66 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTADIACTC:  3×4^72 → SATURATES. ✓
//     NHHEPTADIACTC: 3×8^71 → SATURATES. ✓
//     NBOSO:         3×32^66 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTADIACTC:  5×4^72 → SATURATES. ✓
//     NHHEPTADIACTC: 4×8^71 → SATURATES. ✓
//     NBOSO:         4×32^66 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTADIACTC:  2×2^72 + 2×3^72. 3^72 >> u64::MAX → SATURATES. ✓
//     NHHEPTADIACTC: 5^71+6^71+5^71 → SATURATES. ✓
//     NBOSO:         13^66+18^66+13^66 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTADIACTC:  4×9^72 → SATURATES. ✓
//     NHHEPTADIACTC: 6×18^71 → SATURATES. ✓
//     NBOSO:         6×162^66 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTADIACTC:  5×6^72 → SATURATES. ✓
//     NHHEPTADIACTC: 6×12^71 → SATURATES. ✓
//     NBOSO:         6×72^66 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTADIACTC  = n·S^72                                                                          for S-regular ✓
//   NHHEPTADIACTC = |E|·(2S)^71 (saturates for |E|≥1,S≥1)                                         for S-regular ✓
//   NBOSO         = |E|·(2S²)^66                                                                    for S-regular ✓
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

const T98_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_98");
const T98_EXEC:   ExecutorId = ExecutorId::from_ascii("t98.exec");

const T98_KEY_A: &str = "t98.alpha";
const T98_KEY_B: &str = "t98.beta";
const T98_KEY_C: &str = "t98.gamma";
const T98_KEY_D: &str = "t98.delta";
const T98_KEY_E: &str = "t98.epsilon";

const T98_ID_A: NodeId = derive_node_id(T98_PLUGIN, T98_KEY_A);
const T98_ID_B: NodeId = derive_node_id(T98_PLUGIN, T98_KEY_B);
const T98_ID_C: NodeId = derive_node_id(T98_PLUGIN, T98_KEY_C);
const T98_ID_D: NodeId = derive_node_id(T98_PLUGIN, T98_KEY_D);
const T98_ID_E: NodeId = derive_node_id(T98_PLUGIN, T98_KEY_E);

// L4=185 namespace for this harness.
const T98_VEC_A: VectorAddress = VectorAddress::new(185, 1, 1, 0);
const T98_VEC_B: VectorAddress = VectorAddress::new(185, 1, 2, 0);
const T98_VEC_C: VectorAddress = VectorAddress::new(185, 1, 3, 0);
const T98_VEC_D: VectorAddress = VectorAddress::new(185, 2, 1, 0);
const T98_VEC_E: VectorAddress = VectorAddress::new(185, 2, 2, 0);

const T98_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T98_PLUGIN,
    name:         "kl-graph-topo98-harness",
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
        executor_id:       T98_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T98_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T98_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             0, "empty: node_count=0");
    assert_eq!(ec,             0, "empty: edge_count=0");
    assert_eq!(nheptadiactc,   0, "empty: NHEPTADIACTC=0");
    assert_eq!(nhheptadiactc,  0, "empty: NHHEPTADIACTC=0");
    assert_eq!(nboso,          0, "empty: NBOSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             1, "single: node_count=1");
    assert_eq!(ec,             0, "single: edge_count=0");
    assert_eq!(nheptadiactc,   0, "single: NHEPTADIACTC=0");
    assert_eq!(nhheptadiactc,  0, "single: NHHEPTADIACTC=0");
    assert_eq!(nboso,          0, "single: NBOSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTADIACTC:  1^72 + 1^72 = 2.
// NHHEPTADIACTC: (1+1)^71 = 2^71 > u64::MAX → SATURATES.
// NBOSO:         (1²+1²)^66 = 2^66 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);
    add_edge(T98_ID_A, T98_ID_B, "t98.e.ab");

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             2,         "k2: node_count=2");
    assert_eq!(ec,             1,         "k2: edge_count=1");
    assert_eq!(nheptadiactc,   2,         "k2: NHEPTADIACTC=2 (1^72+1^72=2)");
    assert_eq!(nhheptadiactc,  u64::MAX,  "k2: NHHEPTADIACTC=SAT (2^71>u64::MAX)");
    assert_eq!(nboso,          u64::MAX,  "k2: NBOSO=SAT (2^66>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);
    add_node(T98_VEC_C, T98_KEY_C, T98_ID_C);
    add_edge(T98_ID_A, T98_ID_B, "t98.e.ab");
    add_edge(T98_ID_B, T98_ID_C, "t98.e.bc");

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             3,         "p3: node_count=3");
    assert_eq!(ec,             2,         "p3: edge_count=2");
    assert_eq!(nheptadiactc,   u64::MAX,  "p3: NHEPTADIACTC=SAT (3\u{00d7}2^72>u64)");
    assert_eq!(nhheptadiactc,  u64::MAX,  "p3: NHHEPTADIACTC=SAT (4^71>u64)");
    assert_eq!(nboso,          u64::MAX,  "p3: NBOSO=SAT (8^66>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);
    add_node(T98_VEC_C, T98_KEY_C, T98_ID_C);
    add_edge(T98_ID_A, T98_ID_B, "t98.e.ab");
    add_edge(T98_ID_B, T98_ID_C, "t98.e.bc");
    add_edge(T98_ID_C, T98_ID_A, "t98.e.ca");

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(nheptadiactc,   u64::MAX, "k3: NHEPTADIACTC=SAT");
    assert_eq!(nhheptadiactc,  u64::MAX, "k3: NHHEPTADIACTC=SAT");
    assert_eq!(nboso,          u64::MAX, "k3: NBOSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A); // hub
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);
    add_node(T98_VEC_C, T98_KEY_C, T98_ID_C);
    add_node(T98_VEC_D, T98_KEY_D, T98_ID_D);
    add_node(T98_VEC_E, T98_KEY_E, T98_ID_E);
    add_edge(T98_ID_A, T98_ID_B, "t98.e.ab");
    add_edge(T98_ID_A, T98_ID_C, "t98.e.ac");
    add_edge(T98_ID_A, T98_ID_D, "t98.e.ad");
    add_edge(T98_ID_A, T98_ID_E, "t98.e.ae");

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(nheptadiactc,   u64::MAX, "k14: NHEPTADIACTC=SAT");
    assert_eq!(nhheptadiactc,  u64::MAX, "k14: NHHEPTADIACTC=SAT");
    assert_eq!(nboso,          u64::MAX, "k14: NBOSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);
    add_node(T98_VEC_C, T98_KEY_C, T98_ID_C);
    add_node(T98_VEC_D, T98_KEY_D, T98_ID_D);
    add_edge(T98_ID_A, T98_ID_B, "t98.e.ab");
    add_edge(T98_ID_B, T98_ID_C, "t98.e.bc");
    add_edge(T98_ID_C, T98_ID_D, "t98.e.cd");

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(nheptadiactc,   u64::MAX, "p4: NHEPTADIACTC=SAT");
    assert_eq!(nhheptadiactc,  u64::MAX, "p4: NHHEPTADIACTC=SAT");
    assert_eq!(nboso,          u64::MAX, "p4: NBOSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);
    add_node(T98_VEC_C, T98_KEY_C, T98_ID_C);
    add_node(T98_VEC_D, T98_KEY_D, T98_ID_D);
    add_edge(T98_ID_A, T98_ID_B, "t98.e.ab");
    add_edge(T98_ID_A, T98_ID_C, "t98.e.ac");
    add_edge(T98_ID_A, T98_ID_D, "t98.e.ad");
    add_edge(T98_ID_B, T98_ID_C, "t98.e.bc");
    add_edge(T98_ID_B, T98_ID_D, "t98.e.bd");
    add_edge(T98_ID_C, T98_ID_D, "t98.e.cd");

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(nheptadiactc,   u64::MAX, "k4: NHEPTADIACTC=SAT");
    assert_eq!(nhheptadiactc,  u64::MAX, "k4: NHHEPTADIACTC=SAT");
    assert_eq!(nboso,          u64::MAX, "k4: NBOSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             2, "2iso: node_count=2");
    assert_eq!(ec,             0, "2iso: edge_count=0");
    assert_eq!(nheptadiactc,   0, "2iso: NHEPTADIACTC=0");
    assert_eq!(nhheptadiactc,  0, "2iso: NHHEPTADIACTC=0");
    assert_eq!(nboso,          0, "2iso: NBOSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T98_VEC_A, T98_KEY_A, T98_ID_A);
    add_node(T98_VEC_B, T98_KEY_B, T98_ID_B);
    add_node(T98_VEC_C, T98_KEY_C, T98_ID_C);
    add_node(T98_VEC_D, T98_KEY_D, T98_ID_D);
    add_node(T98_VEC_E, T98_KEY_E, T98_ID_E);
    add_edge(T98_ID_A, T98_ID_C, "t98.e.ac");
    add_edge(T98_ID_A, T98_ID_D, "t98.e.ad");
    add_edge(T98_ID_A, T98_ID_E, "t98.e.ae");
    add_edge(T98_ID_B, T98_ID_C, "t98.e.bc");
    add_edge(T98_ID_B, T98_ID_D, "t98.e.bd");
    add_edge(T98_ID_B, T98_ID_E, "t98.e.be");

    let (nheptadiactc, nhheptadiactc, nboso, ec, nc) = gos_runtime::graph_topo_indices98();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(nheptadiactc,   u64::MAX, "k23: NHEPTADIACTC=SAT");
    assert_eq!(nhheptadiactc,  u64::MAX, "k23: NHHEPTADIACTC=SAT");
    assert_eq!(nboso,          u64::MAX, "k23: NBOSO=SAT");
}
