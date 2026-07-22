// gos-graph-topo99-harness — V3.110 NHEPTATRIACTC + NHHEPTATRIACTC + NBPSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices99()`:
//   Returns (nheptatriactc, nhheptatriactc, nbpso, edge_count, node_count)
//   - nheptatriactc  = NHEPTATRIACTC(G) = Σ_v S(v)^73                         (exact u64; S-Heptatricontic vertex sum)
//   - nhheptatriactc = NHHEPTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^72             (exact u64; S-Heptatricontic edge-sum)
//   - nbpso           = NBPSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^67          (exact u64; S-Variant Sombor, α=134)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTATRIACTC(G) = Σ_v S(v)^73
//     S-Heptatricontic vertex sum; fourth of the heptacontic (70-79) series.
//     Extends heptacontic: NHEPTADIACTC=Σ S^72 (topo98) → NHEPTATRIACTC=Σ S^73 (topo99).
//     NHEPTATRIACTC = n·S^73 for S-regular.
//     Overflow: S^73 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^73 = s64 × s8 × s  (73=64+8+1; 8 mults total).
//
//   NHHEPTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^72
//     S-Heptatricontic edge-sum; extends NHHEPTADIACTC=Σ(S+S)^71 (topo98).
//     NHHEPTATRIACTC = |E|·(2S)^72 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^72 → saturating u128 accumulator.
//     Implementation: ss^72 = ss64 × ss8  (72=64+8; 7 mults total — efficient!).
//
//   NBPSO(G) = Σ_{uv∈E} (S_u²+S_v²)^67
//     S-Variant Sombor: generalised Sombor SO^α with α=134 on S-variant.
//     16th of NB series, letter P (after NBOSO α=132 topo98).
//     NBOSO(topo98,α=132) → NBPSO(topo99,α=134).
//     NBPSO = |E|·(2S²)^67 for S-regular.
//     Overflow per edge: (2×16129²)^67 → saturating u128 accumulator.
//     Implementation: s2s^67 = s2s64 × s2s2 × s2s  (67=64+2+1; 8 mults total).
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
//  Graph     NHEPTATRIACTC(exact)       NHHEPTATRIACTC(exact)      NBPSO(exact)               edges  nodes
//  Empty                      0                            0                   0                   0      0
//  1 node                     0                            0                   0                   0      1
//  K₂                         2            u64::MAX(sat.)      u64::MAX(sat.)                    1      2
//  P₃              u64::MAX(sat.)           u64::MAX(sat.)           u64::MAX(sat.)               2      3
//  K₃              u64::MAX(sat.)           u64::MAX(sat.)           u64::MAX(sat.)               3      3
//  K_{1,4}         u64::MAX(sat.)           u64::MAX(sat.)           u64::MAX(sat.)               4      5
//  P₄              u64::MAX(sat.)           u64::MAX(sat.)           u64::MAX(sat.)               3      4
//  K₄              u64::MAX(sat.)           u64::MAX(sat.)           u64::MAX(sat.)               6      4
//  2 isolated                 0                            0                   0                   0      2
//  K_{2,3}         u64::MAX(sat.)           u64::MAX(sat.)           u64::MAX(sat.)               6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEPTATRIACTC:  1^73 + 1^73 = 2. ✓
//     NHHEPTATRIACTC: (1+1)^72 = 2^72 ≈ 4.72×10^21 > u64::MAX → SATURATES. ✓
//     NBPSO:          (1²+1²)^67 = 2^67 ≈ 1.47×10^20 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTATRIACTC:  3×2^73 >> u64::MAX → SATURATES. ✓
//     NHHEPTATRIACTC: 2×(4)^72 → SATURATES. ✓
//     NBPSO:          2×(8)^67 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTATRIACTC:  3×4^73 → SATURATES. ✓
//     NHHEPTATRIACTC: 3×8^72 → SATURATES. ✓
//     NBPSO:          3×32^67 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTATRIACTC:  5×4^73 → SATURATES. ✓
//     NHHEPTATRIACTC: 4×8^72 → SATURATES. ✓
//     NBPSO:          4×32^67 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTATRIACTC:  2×2^73 + 2×3^73. 3^73 >> u64::MAX → SATURATES. ✓
//     NHHEPTATRIACTC: 5^72+6^72+5^72 → SATURATES. ✓
//     NBPSO:          13^67+18^67+13^67 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTATRIACTC:  4×9^73 → SATURATES. ✓
//     NHHEPTATRIACTC: 6×18^72 → SATURATES. ✓
//     NBPSO:          6×162^67 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTATRIACTC:  5×6^73 → SATURATES. ✓
//     NHHEPTATRIACTC: 6×12^72 → SATURATES. ✓
//     NBPSO:          6×72^67 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTATRIACTC  = n·S^73                                                                          for S-regular ✓
//   NHHEPTATRIACTC = |E|·(2S)^72 (saturates for |E|≥1,S≥1)                                         for S-regular ✓
//   NBPSO          = |E|·(2S²)^67                                                                    for S-regular ✓
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

const T99_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_99");
const T99_EXEC:   ExecutorId = ExecutorId::from_ascii("t99.exec");

const T99_KEY_A: &str = "t99.alpha";
const T99_KEY_B: &str = "t99.beta";
const T99_KEY_C: &str = "t99.gamma";
const T99_KEY_D: &str = "t99.delta";
const T99_KEY_E: &str = "t99.epsilon";

const T99_ID_A: NodeId = derive_node_id(T99_PLUGIN, T99_KEY_A);
const T99_ID_B: NodeId = derive_node_id(T99_PLUGIN, T99_KEY_B);
const T99_ID_C: NodeId = derive_node_id(T99_PLUGIN, T99_KEY_C);
const T99_ID_D: NodeId = derive_node_id(T99_PLUGIN, T99_KEY_D);
const T99_ID_E: NodeId = derive_node_id(T99_PLUGIN, T99_KEY_E);

// L4=186 namespace for this harness.
const T99_VEC_A: VectorAddress = VectorAddress::new(186, 1, 1, 0);
const T99_VEC_B: VectorAddress = VectorAddress::new(186, 1, 2, 0);
const T99_VEC_C: VectorAddress = VectorAddress::new(186, 1, 3, 0);
const T99_VEC_D: VectorAddress = VectorAddress::new(186, 2, 1, 0);
const T99_VEC_E: VectorAddress = VectorAddress::new(186, 2, 2, 0);

const T99_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T99_PLUGIN,
    name:         "kl-graph-topo99-harness",
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
        executor_id:       T99_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T99_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T99_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(nheptatriactc,   0, "empty: NHEPTATRIACTC=0");
    assert_eq!(nhheptatriactc,  0, "empty: NHHEPTATRIACTC=0");
    assert_eq!(nbpso,           0, "empty: NBPSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(nheptatriactc,   0, "single: NHEPTATRIACTC=0");
    assert_eq!(nhheptatriactc,  0, "single: NHHEPTATRIACTC=0");
    assert_eq!(nbpso,           0, "single: NBPSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTATRIACTC:  1^73 + 1^73 = 2.
// NHHEPTATRIACTC: (1+1)^72 = 2^72 > u64::MAX → SATURATES.
// NBPSO:          (1²+1²)^67 = 2^67 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);
    add_edge(T99_ID_A, T99_ID_B, "t99.e.ab");

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              2,         "k2: node_count=2");
    assert_eq!(ec,              1,         "k2: edge_count=1");
    assert_eq!(nheptatriactc,   2,         "k2: NHEPTATRIACTC=2 (1^73+1^73=2)");
    assert_eq!(nhheptatriactc,  u64::MAX,  "k2: NHHEPTATRIACTC=SAT (2^72>u64::MAX)");
    assert_eq!(nbpso,           u64::MAX,  "k2: NBPSO=SAT (2^67>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);
    add_node(T99_VEC_C, T99_KEY_C, T99_ID_C);
    add_edge(T99_ID_A, T99_ID_B, "t99.e.ab");
    add_edge(T99_ID_B, T99_ID_C, "t99.e.bc");

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              3,         "p3: node_count=3");
    assert_eq!(ec,              2,         "p3: edge_count=2");
    assert_eq!(nheptatriactc,   u64::MAX,  "p3: NHEPTATRIACTC=SAT (3\u{00d7}2^73>u64)");
    assert_eq!(nhheptatriactc,  u64::MAX,  "p3: NHHEPTATRIACTC=SAT (4^72>u64)");
    assert_eq!(nbpso,           u64::MAX,  "p3: NBPSO=SAT (8^67>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);
    add_node(T99_VEC_C, T99_KEY_C, T99_ID_C);
    add_edge(T99_ID_A, T99_ID_B, "t99.e.ab");
    add_edge(T99_ID_B, T99_ID_C, "t99.e.bc");
    add_edge(T99_ID_C, T99_ID_A, "t99.e.ca");

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(nheptatriactc,   u64::MAX, "k3: NHEPTATRIACTC=SAT");
    assert_eq!(nhheptatriactc,  u64::MAX, "k3: NHHEPTATRIACTC=SAT");
    assert_eq!(nbpso,           u64::MAX, "k3: NBPSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A); // hub
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);
    add_node(T99_VEC_C, T99_KEY_C, T99_ID_C);
    add_node(T99_VEC_D, T99_KEY_D, T99_ID_D);
    add_node(T99_VEC_E, T99_KEY_E, T99_ID_E);
    add_edge(T99_ID_A, T99_ID_B, "t99.e.ab");
    add_edge(T99_ID_A, T99_ID_C, "t99.e.ac");
    add_edge(T99_ID_A, T99_ID_D, "t99.e.ad");
    add_edge(T99_ID_A, T99_ID_E, "t99.e.ae");

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(nheptatriactc,   u64::MAX, "k14: NHEPTATRIACTC=SAT");
    assert_eq!(nhheptatriactc,  u64::MAX, "k14: NHHEPTATRIACTC=SAT");
    assert_eq!(nbpso,           u64::MAX, "k14: NBPSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);
    add_node(T99_VEC_C, T99_KEY_C, T99_ID_C);
    add_node(T99_VEC_D, T99_KEY_D, T99_ID_D);
    add_edge(T99_ID_A, T99_ID_B, "t99.e.ab");
    add_edge(T99_ID_B, T99_ID_C, "t99.e.bc");
    add_edge(T99_ID_C, T99_ID_D, "t99.e.cd");

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(nheptatriactc,   u64::MAX, "p4: NHEPTATRIACTC=SAT");
    assert_eq!(nhheptatriactc,  u64::MAX, "p4: NHHEPTATRIACTC=SAT");
    assert_eq!(nbpso,           u64::MAX, "p4: NBPSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);
    add_node(T99_VEC_C, T99_KEY_C, T99_ID_C);
    add_node(T99_VEC_D, T99_KEY_D, T99_ID_D);
    add_edge(T99_ID_A, T99_ID_B, "t99.e.ab");
    add_edge(T99_ID_A, T99_ID_C, "t99.e.ac");
    add_edge(T99_ID_A, T99_ID_D, "t99.e.ad");
    add_edge(T99_ID_B, T99_ID_C, "t99.e.bc");
    add_edge(T99_ID_B, T99_ID_D, "t99.e.bd");
    add_edge(T99_ID_C, T99_ID_D, "t99.e.cd");

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(nheptatriactc,   u64::MAX, "k4: NHEPTATRIACTC=SAT");
    assert_eq!(nhheptatriactc,  u64::MAX, "k4: NHHEPTATRIACTC=SAT");
    assert_eq!(nbpso,           u64::MAX, "k4: NBPSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              2, "2iso: node_count=2");
    assert_eq!(ec,              0, "2iso: edge_count=0");
    assert_eq!(nheptatriactc,   0, "2iso: NHEPTATRIACTC=0");
    assert_eq!(nhheptatriactc,  0, "2iso: NHHEPTATRIACTC=0");
    assert_eq!(nbpso,           0, "2iso: NBPSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T99_VEC_A, T99_KEY_A, T99_ID_A);
    add_node(T99_VEC_B, T99_KEY_B, T99_ID_B);
    add_node(T99_VEC_C, T99_KEY_C, T99_ID_C);
    add_node(T99_VEC_D, T99_KEY_D, T99_ID_D);
    add_node(T99_VEC_E, T99_KEY_E, T99_ID_E);
    add_edge(T99_ID_A, T99_ID_C, "t99.e.ac");
    add_edge(T99_ID_A, T99_ID_D, "t99.e.ad");
    add_edge(T99_ID_A, T99_ID_E, "t99.e.ae");
    add_edge(T99_ID_B, T99_ID_C, "t99.e.bc");
    add_edge(T99_ID_B, T99_ID_D, "t99.e.bd");
    add_edge(T99_ID_B, T99_ID_E, "t99.e.be");

    let (nheptatriactc, nhheptatriactc, nbpso, ec, nc) = gos_runtime::graph_topo_indices99();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(nheptatriactc,   u64::MAX, "k23: NHEPTATRIACTC=SAT");
    assert_eq!(nhheptatriactc,  u64::MAX, "k23: NHHEPTATRIACTC=SAT");
    assert_eq!(nbpso,           u64::MAX, "k23: NBPSO=SAT");
}
