// gos-graph-topo107-harness — V3.118 NOCTAMONOACTC + NHOCTAMONOACTC + NBXSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices107()`:
//   Returns (noctamonoactc, nhoctamonoactc, nbxso, edge_count, node_count)
//   - noctamonoactc  = NOCTAMONOACTC(G)  = Σ_v S(v)^81                        (exact u64; S-Octamonocontic vertex sum)
//   - nhoctamonoactc = NHOCTAMONOACTC(G) = Σ_{uv∈E} (S_u+S_v)^80            (exact u64; S-Octamonocontic edge-sum)
//   - nbxso           = NBXSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^75          (exact u64; S-Variant Sombor, α=150)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTAMONOACTC(G) = Σ_v S(v)^81
//     S-Octamonocontic vertex sum; second of the octacontic (80-89) series.
//     Extends: NOCTAACTC=Σ S^80 (topo106) → NOCTAMONOACTC=Σ S^81 (topo107).
//     NOCTAMONOACTC = n·S^81 for S-regular.
//     Overflow: S^81 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^81 = s64 × s16 × s  (81=64+16+1; 8 mults total).
//
//   NHOCTAMONOACTC(G) = Σ_{uv∈E} (S_u+S_v)^80
//     S-Octamonocontic edge-sum; extends NHOCTAACTC=Σ(S+S)^79 (topo106).
//     NHOCTAMONOACTC = |E|·(2S)^80 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^80 → saturating u128 accumulator.
//     Implementation: ss^80 = ss64 × ss16  (80=64+16; 7 mults total).
//
//   NBXSO(G) = Σ_{uv∈E} (S_u²+S_v²)^75
//     S-Variant Sombor: generalised Sombor SO^α with α=150 on S-variant.
//     24th of NB series, letter X (after NBWSO α=148 topo106).
//     NBWSO(topo106,α=148) → NBXSO(topo107,α=150).
//     NBXSO = |E|·(2S²)^75 for S-regular.
//     Overflow per edge: (2×16129²)^75 → saturating u128 accumulator.
//     Implementation: s2s^75 = s2s64 × s2s8 × s2s2 × s2s  (75=64+8+2+1; 9 mults total).
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
//  Graph     NOCTAMONOACTC(exact)       NHOCTAMONOACTC(exact)      NBXSO(exact)               edges  nodes
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
//     NOCTAMONOACTC:   1^81 + 1^81 = 2. ✓
//     NHOCTAMONOACTC:  (1+1)^80 = 2^80 ≈ 1.21×10^24 > u64::MAX → SATURATES. ✓
//     NBXSO:           (1²+1²)^75 = 2^75 ≈ 3.78×10^22 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTAMONOACTC:   3×2^81 >> u64::MAX → SATURATES. ✓
//     NHOCTAMONOACTC:  2×(4)^80 → SATURATES. ✓
//     NBXSO:           2×(8)^75 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTAMONOACTC:   3×4^81 → SATURATES. ✓
//     NHOCTAMONOACTC:  3×8^80 → SATURATES. ✓
//     NBXSO:           3×32^75 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTAMONOACTC:   5×4^81 → SATURATES. ✓
//     NHOCTAMONOACTC:  4×8^80 → SATURATES. ✓
//     NBXSO:           4×32^75 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTAMONOACTC:   2×2^81 + 2×3^81. 3^81 >> u64::MAX → SATURATES. ✓
//     NHOCTAMONOACTC:  5^80+6^80+5^80 → SATURATES. ✓
//     NBXSO:           13^75+18^75+13^75 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTAMONOACTC:   4×9^81 → SATURATES. ✓
//     NHOCTAMONOACTC:  6×18^80 → SATURATES. ✓
//     NBXSO:           6×162^75 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTAMONOACTC:   5×6^81 → SATURATES. ✓
//     NHOCTAMONOACTC:  6×12^80 → SATURATES. ✓
//     NBXSO:           6×72^75 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTAMONOACTC  = n·S^81                                                                     for S-regular ✓
//   NHOCTAMONOACTC = |E|·(2S)^80 (saturates for |E|≥1,S≥1)                                    for S-regular ✓
//   NBXSO          = |E|·(2S²)^75                                                               for S-regular ✓
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

const T107_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX107");
const T107_EXEC:   ExecutorId = ExecutorId::from_ascii("t107.exec");

const T107_KEY_A: &str = "t107.alpha";
const T107_KEY_B: &str = "t107.beta";
const T107_KEY_C: &str = "t107.gamma";
const T107_KEY_D: &str = "t107.delta";
const T107_KEY_E: &str = "t107.epsilon";

const T107_ID_A: NodeId = derive_node_id(T107_PLUGIN, T107_KEY_A);
const T107_ID_B: NodeId = derive_node_id(T107_PLUGIN, T107_KEY_B);
const T107_ID_C: NodeId = derive_node_id(T107_PLUGIN, T107_KEY_C);
const T107_ID_D: NodeId = derive_node_id(T107_PLUGIN, T107_KEY_D);
const T107_ID_E: NodeId = derive_node_id(T107_PLUGIN, T107_KEY_E);

// L4=194 namespace for this harness.
const T107_VEC_A: VectorAddress = VectorAddress::new(194, 1, 1, 0);
const T107_VEC_B: VectorAddress = VectorAddress::new(194, 1, 2, 0);
const T107_VEC_C: VectorAddress = VectorAddress::new(194, 1, 3, 0);
const T107_VEC_D: VectorAddress = VectorAddress::new(194, 2, 1, 0);
const T107_VEC_E: VectorAddress = VectorAddress::new(194, 2, 2, 0);

const T107_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T107_PLUGIN,
    name:         "kl-graph-topo107-harness",
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
        executor_id:       T107_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T107_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T107_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(noctamonoactc,   0, "empty: NOCTAMONOACTC=0");
    assert_eq!(nhoctamonoactc,  0, "empty: NHOCTAMONOACTC=0");
    assert_eq!(nbxso,           0, "empty: NBXSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(noctamonoactc,   0, "single: NOCTAMONOACTC=0");
    assert_eq!(nhoctamonoactc,  0, "single: NHOCTAMONOACTC=0");
    assert_eq!(nbxso,           0, "single: NBXSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTAMONOACTC:   1^81 + 1^81 = 2.
// NHOCTAMONOACTC:  (1+1)^80 = 2^80 > u64::MAX → SATURATES.
// NBXSO:           (1²+1²)^75 = 2^75 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);
    add_edge(T107_ID_A, T107_ID_B, "t107.e.ab");

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              2,        "k2: node_count=2");
    assert_eq!(ec,              1,        "k2: edge_count=1");
    assert_eq!(noctamonoactc,   2,        "k2: NOCTAMONOACTC=2 (1^81+1^81=2)");
    assert_eq!(nhoctamonoactc,  u64::MAX, "k2: NHOCTAMONOACTC=SAT (2^80>u64::MAX)");
    assert_eq!(nbxso,           u64::MAX, "k2: NBXSO=SAT (2^75>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);
    add_node(T107_VEC_C, T107_KEY_C, T107_ID_C);
    add_edge(T107_ID_A, T107_ID_B, "t107.e.ab");
    add_edge(T107_ID_B, T107_ID_C, "t107.e.bc");

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              3,        "p3: node_count=3");
    assert_eq!(ec,              2,        "p3: edge_count=2");
    assert_eq!(noctamonoactc,   u64::MAX, "p3: NOCTAMONOACTC=SAT (3\u{00d7}2^81>u64)");
    assert_eq!(nhoctamonoactc,  u64::MAX, "p3: NHOCTAMONOACTC=SAT (4^80>u64)");
    assert_eq!(nbxso,           u64::MAX, "p3: NBXSO=SAT (8^75>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);
    add_node(T107_VEC_C, T107_KEY_C, T107_ID_C);
    add_edge(T107_ID_A, T107_ID_B, "t107.e.ab");
    add_edge(T107_ID_B, T107_ID_C, "t107.e.bc");
    add_edge(T107_ID_C, T107_ID_A, "t107.e.ca");

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(noctamonoactc,   u64::MAX, "k3: NOCTAMONOACTC=SAT");
    assert_eq!(nhoctamonoactc,  u64::MAX, "k3: NHOCTAMONOACTC=SAT");
    assert_eq!(nbxso,           u64::MAX, "k3: NBXSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A); // hub
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);
    add_node(T107_VEC_C, T107_KEY_C, T107_ID_C);
    add_node(T107_VEC_D, T107_KEY_D, T107_ID_D);
    add_node(T107_VEC_E, T107_KEY_E, T107_ID_E);
    add_edge(T107_ID_A, T107_ID_B, "t107.e.ab");
    add_edge(T107_ID_A, T107_ID_C, "t107.e.ac");
    add_edge(T107_ID_A, T107_ID_D, "t107.e.ad");
    add_edge(T107_ID_A, T107_ID_E, "t107.e.ae");

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(noctamonoactc,   u64::MAX, "k14: NOCTAMONOACTC=SAT");
    assert_eq!(nhoctamonoactc,  u64::MAX, "k14: NHOCTAMONOACTC=SAT");
    assert_eq!(nbxso,           u64::MAX, "k14: NBXSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);
    add_node(T107_VEC_C, T107_KEY_C, T107_ID_C);
    add_node(T107_VEC_D, T107_KEY_D, T107_ID_D);
    add_edge(T107_ID_A, T107_ID_B, "t107.e.ab");
    add_edge(T107_ID_B, T107_ID_C, "t107.e.bc");
    add_edge(T107_ID_C, T107_ID_D, "t107.e.cd");

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(noctamonoactc,   u64::MAX, "p4: NOCTAMONOACTC=SAT");
    assert_eq!(nhoctamonoactc,  u64::MAX, "p4: NHOCTAMONOACTC=SAT");
    assert_eq!(nbxso,           u64::MAX, "p4: NBXSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);
    add_node(T107_VEC_C, T107_KEY_C, T107_ID_C);
    add_node(T107_VEC_D, T107_KEY_D, T107_ID_D);
    add_edge(T107_ID_A, T107_ID_B, "t107.e.ab");
    add_edge(T107_ID_A, T107_ID_C, "t107.e.ac");
    add_edge(T107_ID_A, T107_ID_D, "t107.e.ad");
    add_edge(T107_ID_B, T107_ID_C, "t107.e.bc");
    add_edge(T107_ID_B, T107_ID_D, "t107.e.bd");
    add_edge(T107_ID_C, T107_ID_D, "t107.e.cd");

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(noctamonoactc,   u64::MAX, "k4: NOCTAMONOACTC=SAT");
    assert_eq!(nhoctamonoactc,  u64::MAX, "k4: NHOCTAMONOACTC=SAT");
    assert_eq!(nbxso,           u64::MAX, "k4: NBXSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              2, "2iso: node_count=2");
    assert_eq!(ec,              0, "2iso: edge_count=0");
    assert_eq!(noctamonoactc,   0, "2iso: NOCTAMONOACTC=0");
    assert_eq!(nhoctamonoactc,  0, "2iso: NHOCTAMONOACTC=0");
    assert_eq!(nbxso,           0, "2iso: NBXSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T107_VEC_A, T107_KEY_A, T107_ID_A);
    add_node(T107_VEC_B, T107_KEY_B, T107_ID_B);
    add_node(T107_VEC_C, T107_KEY_C, T107_ID_C);
    add_node(T107_VEC_D, T107_KEY_D, T107_ID_D);
    add_node(T107_VEC_E, T107_KEY_E, T107_ID_E);
    add_edge(T107_ID_A, T107_ID_C, "t107.e.ac");
    add_edge(T107_ID_A, T107_ID_D, "t107.e.ad");
    add_edge(T107_ID_A, T107_ID_E, "t107.e.ae");
    add_edge(T107_ID_B, T107_ID_C, "t107.e.bc");
    add_edge(T107_ID_B, T107_ID_D, "t107.e.bd");
    add_edge(T107_ID_B, T107_ID_E, "t107.e.be");

    let (noctamonoactc, nhoctamonoactc, nbxso, ec, nc) = gos_runtime::graph_topo_indices107();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(noctamonoactc,   u64::MAX, "k23: NOCTAMONOACTC=SAT");
    assert_eq!(nhoctamonoactc,  u64::MAX, "k23: NHOCTAMONOACTC=SAT");
    assert_eq!(nbxso,           u64::MAX, "k23: NBXSO=SAT");
}
