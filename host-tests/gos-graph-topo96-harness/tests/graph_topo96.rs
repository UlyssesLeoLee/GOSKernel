// gos-graph-topo96-harness — V3.107 NHEPTAACTC + NHHEPTAACTC + NBMSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices96()`:
//   Returns (nheptaactc, nhheptaactc, nbmso, edge_count, node_count)
//   - nheptaactc  = NHEPTAACTC(G) = Σ_v S(v)^70                        (exact u64; S-Heptacontic vertex sum)
//   - nhheptaactc = NHHEPTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^69            (exact u64; S-Heptacontic edge-sum)
//   - nbmso        = NBMSO(G)       = Σ_{uv∈E} (S_u²+S_v²)^64          (exact u64; S-Variant Sombor, α=128)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTAACTC(G) = Σ_v S(v)^70
//     S-Heptacontic vertex sum; first of the heptacontic (70-79) series.
//     Extends hexacontic: NHEXAENNACTC=Σ S⁶⁹ (topo95) → NHEPTAACTC=Σ S⁷⁰ (topo96).
//     NHEPTAACTC = n·S^70 for S-regular.
//     Overflow: S^70 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^70 = s64 × s4 × s2  (70=64+4+2; 8 mults total).
//
//   NHHEPTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^69
//     S-Heptacontic edge-sum; extends NHHEXAENNACTC=Σ(S+S)^68 (topo95).
//     NHHEPTAACTC = |E|·(2S)^69 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^69 → saturating u128 accumulator.
//     Implementation: ss^69 = ss64 × ss4 × ss  (69=64+4+1; 8 mults).
//
//   NBMSO(G) = Σ_{uv∈E} (S_u²+S_v²)^64
//     S-Variant Sombor: generalised Sombor SO^α with α=128 on S-variant.
//     13th of NB series, letter M (after NBLSO α=126 topo95).
//     NBLSO(topo95,α=126) → NBMSO(topo96,α=128).
//     NBMSO = |E|·(2S²)^64 for S-regular.
//     Overflow per edge: (2×16129²)^64 → saturating u128 accumulator.
//     Implementation: s2s^64 = s2s32 × s2s32  (64=32+32; 7 mults total).
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
//  Graph     NHEPTAACTC(exact)          NHHEPTAACTC(exact)         NBMSO(exact)               edges  nodes
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
//     NHEPTAACTC:  1^70 + 1^70 = 2. ✓
//     NHHEPTAACTC: (1+1)^69 = 2^69 = 590_295_810_358_705_651_712 > u64::MAX → SATURATES. ✓
//     NBMSO:       (1²+1²)^64 = 2^64 = 18_446_744_073_709_551_616 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTAACTC:  3×2^70 >> u64::MAX → SATURATES. ✓
//     NHHEPTAACTC: 2×(4)^69 → SATURATES. ✓
//     NBMSO:       2×(8)^64 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTAACTC:  3×4^70 → SATURATES. ✓
//     NHHEPTAACTC: 3×8^69 → SATURATES. ✓
//     NBMSO:       3×32^64 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTAACTC:  5×4^70 → SATURATES. ✓
//     NHHEPTAACTC: 4×8^69 → SATURATES. ✓
//     NBMSO:       4×32^64 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTAACTC:  2×2^70 + 2×3^70. 3^70 >> u64::MAX → SATURATES. ✓
//     NHHEPTAACTC: 5^69+6^69+5^69 → SATURATES. ✓
//     NBMSO:       13^64+18^64+13^64 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTAACTC:  4×9^70 → SATURATES. ✓
//     NHHEPTAACTC: 6×18^69 → SATURATES. ✓
//     NBMSO:       6×162^64 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTAACTC:  5×6^70 → SATURATES. ✓
//     NHHEPTAACTC: 6×12^69 → SATURATES. ✓
//     NBMSO:       6×72^64 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTAACTC  = n·S^70                                                                              for S-regular ✓
//   NHHEPTAACTC = |E|·(2S)^69 (saturates for |E|≥1,S≥1)                                             for S-regular ✓
//   NBMSO       = |E|·(2S²)^64                                                                        for S-regular ✓
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

const T96_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_96");
const T96_EXEC:   ExecutorId = ExecutorId::from_ascii("t96.exec");

const T96_KEY_A: &str = "t96.alpha";
const T96_KEY_B: &str = "t96.beta";
const T96_KEY_C: &str = "t96.gamma";
const T96_KEY_D: &str = "t96.delta";
const T96_KEY_E: &str = "t96.epsilon";

const T96_ID_A: NodeId = derive_node_id(T96_PLUGIN, T96_KEY_A);
const T96_ID_B: NodeId = derive_node_id(T96_PLUGIN, T96_KEY_B);
const T96_ID_C: NodeId = derive_node_id(T96_PLUGIN, T96_KEY_C);
const T96_ID_D: NodeId = derive_node_id(T96_PLUGIN, T96_KEY_D);
const T96_ID_E: NodeId = derive_node_id(T96_PLUGIN, T96_KEY_E);

// L4=183 namespace for this harness.
const T96_VEC_A: VectorAddress = VectorAddress::new(183, 1, 1, 0);
const T96_VEC_B: VectorAddress = VectorAddress::new(183, 1, 2, 0);
const T96_VEC_C: VectorAddress = VectorAddress::new(183, 1, 3, 0);
const T96_VEC_D: VectorAddress = VectorAddress::new(183, 2, 1, 0);
const T96_VEC_E: VectorAddress = VectorAddress::new(183, 2, 2, 0);

const T96_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T96_PLUGIN,
    name:         "kl-graph-topo96-harness",
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
        executor_id:       T96_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T96_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T96_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           0, "empty: node_count=0");
    assert_eq!(ec,           0, "empty: edge_count=0");
    assert_eq!(nheptaactc,   0, "empty: NHEPTAACTC=0");
    assert_eq!(nhheptaactc,  0, "empty: NHHEPTAACTC=0");
    assert_eq!(nbmso,        0, "empty: NBMSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           1, "single: node_count=1");
    assert_eq!(ec,           0, "single: edge_count=0");
    assert_eq!(nheptaactc,   0, "single: NHEPTAACTC=0");
    assert_eq!(nhheptaactc,  0, "single: NHHEPTAACTC=0");
    assert_eq!(nbmso,        0, "single: NBMSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTAACTC:  1^70 + 1^70 = 2.
// NHHEPTAACTC: (1+1)^69 = 2^69 > u64::MAX → SATURATES.
// NBMSO:       (1²+1²)^64 = 2^64 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);
    add_edge(T96_ID_A, T96_ID_B, "t96.e.ab");

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           2,         "k2: node_count=2");
    assert_eq!(ec,           1,         "k2: edge_count=1");
    assert_eq!(nheptaactc,   2,         "k2: NHEPTAACTC=2 (1^70+1^70=2)");
    assert_eq!(nhheptaactc,  u64::MAX,  "k2: NHHEPTAACTC=SAT (2^69>u64::MAX)");
    assert_eq!(nbmso,        u64::MAX,  "k2: NBMSO=SAT (2^64>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);
    add_node(T96_VEC_C, T96_KEY_C, T96_ID_C);
    add_edge(T96_ID_A, T96_ID_B, "t96.e.ab");
    add_edge(T96_ID_B, T96_ID_C, "t96.e.bc");

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           3,         "p3: node_count=3");
    assert_eq!(ec,           2,         "p3: edge_count=2");
    assert_eq!(nheptaactc,   u64::MAX,  "p3: NHEPTAACTC=SAT (3\u{00d7}2^70>u64)");
    assert_eq!(nhheptaactc,  u64::MAX,  "p3: NHHEPTAACTC=SAT (4^69>u64)");
    assert_eq!(nbmso,        u64::MAX,  "p3: NBMSO=SAT (8^64>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);
    add_node(T96_VEC_C, T96_KEY_C, T96_ID_C);
    add_edge(T96_ID_A, T96_ID_B, "t96.e.ab");
    add_edge(T96_ID_B, T96_ID_C, "t96.e.bc");
    add_edge(T96_ID_C, T96_ID_A, "t96.e.ca");

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           3,        "k3: node_count=3");
    assert_eq!(ec,           3,        "k3: edge_count=3");
    assert_eq!(nheptaactc,   u64::MAX, "k3: NHEPTAACTC=SAT");
    assert_eq!(nhheptaactc,  u64::MAX, "k3: NHHEPTAACTC=SAT");
    assert_eq!(nbmso,        u64::MAX, "k3: NBMSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A); // hub
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);
    add_node(T96_VEC_C, T96_KEY_C, T96_ID_C);
    add_node(T96_VEC_D, T96_KEY_D, T96_ID_D);
    add_node(T96_VEC_E, T96_KEY_E, T96_ID_E);
    add_edge(T96_ID_A, T96_ID_B, "t96.e.ab");
    add_edge(T96_ID_A, T96_ID_C, "t96.e.ac");
    add_edge(T96_ID_A, T96_ID_D, "t96.e.ad");
    add_edge(T96_ID_A, T96_ID_E, "t96.e.ae");

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           5,        "k14: node_count=5");
    assert_eq!(ec,           4,        "k14: edge_count=4");
    assert_eq!(nheptaactc,   u64::MAX, "k14: NHEPTAACTC=SAT");
    assert_eq!(nhheptaactc,  u64::MAX, "k14: NHHEPTAACTC=SAT");
    assert_eq!(nbmso,        u64::MAX, "k14: NBMSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);
    add_node(T96_VEC_C, T96_KEY_C, T96_ID_C);
    add_node(T96_VEC_D, T96_KEY_D, T96_ID_D);
    add_edge(T96_ID_A, T96_ID_B, "t96.e.ab");
    add_edge(T96_ID_B, T96_ID_C, "t96.e.bc");
    add_edge(T96_ID_C, T96_ID_D, "t96.e.cd");

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           4,        "p4: node_count=4");
    assert_eq!(ec,           3,        "p4: edge_count=3");
    assert_eq!(nheptaactc,   u64::MAX, "p4: NHEPTAACTC=SAT");
    assert_eq!(nhheptaactc,  u64::MAX, "p4: NHHEPTAACTC=SAT");
    assert_eq!(nbmso,        u64::MAX, "p4: NBMSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);
    add_node(T96_VEC_C, T96_KEY_C, T96_ID_C);
    add_node(T96_VEC_D, T96_KEY_D, T96_ID_D);
    add_edge(T96_ID_A, T96_ID_B, "t96.e.ab");
    add_edge(T96_ID_A, T96_ID_C, "t96.e.ac");
    add_edge(T96_ID_A, T96_ID_D, "t96.e.ad");
    add_edge(T96_ID_B, T96_ID_C, "t96.e.bc");
    add_edge(T96_ID_B, T96_ID_D, "t96.e.bd");
    add_edge(T96_ID_C, T96_ID_D, "t96.e.cd");

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           4,        "k4: node_count=4");
    assert_eq!(ec,           6,        "k4: edge_count=6");
    assert_eq!(nheptaactc,   u64::MAX, "k4: NHEPTAACTC=SAT");
    assert_eq!(nhheptaactc,  u64::MAX, "k4: NHHEPTAACTC=SAT");
    assert_eq!(nbmso,        u64::MAX, "k4: NBMSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           2, "2iso: node_count=2");
    assert_eq!(ec,           0, "2iso: edge_count=0");
    assert_eq!(nheptaactc,   0, "2iso: NHEPTAACTC=0");
    assert_eq!(nhheptaactc,  0, "2iso: NHHEPTAACTC=0");
    assert_eq!(nbmso,        0, "2iso: NBMSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T96_VEC_A, T96_KEY_A, T96_ID_A);
    add_node(T96_VEC_B, T96_KEY_B, T96_ID_B);
    add_node(T96_VEC_C, T96_KEY_C, T96_ID_C);
    add_node(T96_VEC_D, T96_KEY_D, T96_ID_D);
    add_node(T96_VEC_E, T96_KEY_E, T96_ID_E);
    add_edge(T96_ID_A, T96_ID_C, "t96.e.ac");
    add_edge(T96_ID_A, T96_ID_D, "t96.e.ad");
    add_edge(T96_ID_A, T96_ID_E, "t96.e.ae");
    add_edge(T96_ID_B, T96_ID_C, "t96.e.bc");
    add_edge(T96_ID_B, T96_ID_D, "t96.e.bd");
    add_edge(T96_ID_B, T96_ID_E, "t96.e.be");

    let (nheptaactc, nhheptaactc, nbmso, ec, nc) = gos_runtime::graph_topo_indices96();
    assert_eq!(nc,           5,        "k23: node_count=5");
    assert_eq!(ec,           6,        "k23: edge_count=6");
    assert_eq!(nheptaactc,   u64::MAX, "k23: NHEPTAACTC=SAT");
    assert_eq!(nhheptaactc,  u64::MAX, "k23: NHHEPTAACTC=SAT");
    assert_eq!(nbmso,        u64::MAX, "k23: NBMSO=SAT");
}
