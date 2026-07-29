// gos-graph-topo114-harness — V3.125 NOCTAOCTACTC + NHOCTAOCTACTC + NBEESO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices114()`:
//   Returns (noctaoctactc, nhoctaoctactc, nbeeso, edge_count, node_count)
//   - noctaoctactc  = NOCTAOCTACTC(G)  = Σ_v S(v)^88                          (exact u64; S-Octaoctocontic vertex sum)
//   - nhoctaoctactc = NHOCTAOCTACTC(G) = Σ_{uv∈E} (S_u+S_v)^87              (exact u64; S-Octaoctocontic edge-sum)
//   - nbeeso         = NBEESO(G)        = Σ_{uv∈E} (S_u²+S_v²)^82            (exact u64; S-Variant Sombor, α=164)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NOCTAOCTACTC(G) = Σ_v S(v)^88
//     S-Octaoctocontic vertex sum; ninth of the octacontic (80-89) series.
//     Extends: NOCTAHEPTACTC=Σ S^87 (topo113) → NOCTAOCTACTC=Σ S^88 (topo114).
//     NOCTAOCTACTC = n·S^88 for S-regular.
//     Overflow: S^88 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^88 = s64 × s16 × s8  (88=64+16+8; 8 mults).
//
//   NHOCTAOCTACTC(G) = Σ_{uv∈E} (S_u+S_v)^87
//     S-Octaoctocontic edge-sum; extends NHOCTAHEPTACTC=Σ(S+S)^86 (topo113).
//     NHOCTAOCTACTC = |E|·(2S)^87 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^87 → saturating u128 accumulator.
//     Implementation: ss^87 = ss64 × ss16 × ss4 × ss2 × ss  (87=64+16+4+2+1; 10 mults total).
//
//   NBEESO(G) = Σ_{uv∈E} (S_u²+S_v²)^82
//     S-Variant Sombor: generalised Sombor SO^α with α=164 on S-variant.
//     31st of NB series, letters EE (after NBDDSO α=162 topo113).
//     NBDDSO(topo113,α=162) → NBEESO(topo114,α=164).
//     NBEESO = |E|·(2S²)^82 for S-regular.
//     Overflow per edge: (2×16129²)^82 → saturating u128 accumulator.
//     Implementation: s2s^82 = s2s64 × s2s16 × s2s2  (82=64+16+2; 8 mults total).
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
//  Graph     NOCTAOCTACTC(exact)       NHOCTAOCTACTC(exact)       NBEESO(exact)              edges  nodes
//  Empty                    0                            0               0                      0      0
//  1 node                   0                            0               0                      0      1
//  K₂                       2            u64::MAX(sat.)    u64::MAX(sat.)                       1      2
//  P₃             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  2      3
//  K₃             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  3      3
//  K_{1,4}        u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  4      5
//  P₄             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  3      4
//  K₄             u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  6      4
//  2 isolated               0                            0               0                      0      2
//  K_{2,3}        u64::MAX(sat.)           u64::MAX(sat.)       u64::MAX(sat.)                  6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NOCTAOCTACTC:    1^88 + 1^88 = 2. ✓
//     NHOCTAOCTACTC:   (1+1)^87 = 2^87 ≈ 1.55×10^26 > u64::MAX → SATURATES. ✓
//     NBEESO:          (1²+1²)^82 = 2^82 ≈ 4.84×10^24 > u64::MAX → SATURATES. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NOCTAOCTACTC:    3×2^88 >> u64::MAX → SATURATES. ✓
//     NHOCTAOCTACTC:   2×(4)^87 → SATURATES. ✓
//     NBEESO:          2×(8)^82 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NOCTAOCTACTC:    3×4^88 → SATURATES. ✓
//     NHOCTAOCTACTC:   3×8^87 → SATURATES. ✓
//     NBEESO:          3×32^82 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NOCTAOCTACTC:    5×4^88 → SATURATES. ✓
//     NHOCTAOCTACTC:   4×8^87 → SATURATES. ✓
//     NBEESO:          4×32^82 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NOCTAOCTACTC:    2×2^88 + 2×3^88. 3^88 >> u64::MAX → SATURATES. ✓
//     NHOCTAOCTACTC:   5^87+6^87+5^87 → SATURATES. ✓
//     NBEESO:          13^82+18^82+13^82 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NOCTAOCTACTC:    4×9^88 → SATURATES. ✓
//     NHOCTAOCTACTC:   6×18^87 → SATURATES. ✓
//     NBEESO:          6×162^82 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NOCTAOCTACTC:    5×6^88 → SATURATES. ✓
//     NHOCTAOCTACTC:   6×12^87 → SATURATES. ✓
//     NBEESO:          6×72^82 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NOCTAOCTACTC  = n·S^88                                                                       for S-regular ✓
//   NHOCTAOCTACTC = |E|·(2S)^87 (saturates for |E|≥1,S≥1)                                      for S-regular ✓
//   NBEESO        = |E|·(2S²)^82                                                                 for S-regular ✓
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

const T114_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX114");
const T114_EXEC:   ExecutorId = ExecutorId::from_ascii("t114.exec");

const T114_KEY_A: &str = "t114.alpha";
const T114_KEY_B: &str = "t114.beta";
const T114_KEY_C: &str = "t114.gamma";
const T114_KEY_D: &str = "t114.delta";
const T114_KEY_E: &str = "t114.epsilon";

const T114_ID_A: NodeId = derive_node_id(T114_PLUGIN, T114_KEY_A);
const T114_ID_B: NodeId = derive_node_id(T114_PLUGIN, T114_KEY_B);
const T114_ID_C: NodeId = derive_node_id(T114_PLUGIN, T114_KEY_C);
const T114_ID_D: NodeId = derive_node_id(T114_PLUGIN, T114_KEY_D);
const T114_ID_E: NodeId = derive_node_id(T114_PLUGIN, T114_KEY_E);

// L4=201 namespace for this harness.
const T114_VEC_A: VectorAddress = VectorAddress::new(201, 1, 1, 0);
const T114_VEC_B: VectorAddress = VectorAddress::new(201, 1, 2, 0);
const T114_VEC_C: VectorAddress = VectorAddress::new(201, 1, 3, 0);
const T114_VEC_D: VectorAddress = VectorAddress::new(201, 2, 1, 0);
const T114_VEC_E: VectorAddress = VectorAddress::new(201, 2, 2, 0);

const T114_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T114_PLUGIN,
    name:         "kl-graph-topo114-harness",
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
        executor_id:       T114_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T114_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T114_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             0, "empty: node_count=0");
    assert_eq!(ec,             0, "empty: edge_count=0");
    assert_eq!(noctaoctactc,  0, "empty: NOCTAOCTACTC=0");
    assert_eq!(nhoctaoctactc, 0, "empty: NHOCTAOCTACTC=0");
    assert_eq!(nbeeso,        0, "empty: NBEESO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             1, "single: node_count=1");
    assert_eq!(ec,             0, "single: edge_count=0");
    assert_eq!(noctaoctactc,  0, "single: NOCTAOCTACTC=0");
    assert_eq!(nhoctaoctactc, 0, "single: NHOCTAOCTACTC=0");
    assert_eq!(nbeeso,        0, "single: NBEESO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NOCTAOCTACTC:    1^88 + 1^88 = 2.
// NHOCTAOCTACTC:   (1+1)^87 = 2^87 > u64::MAX → SATURATES.
// NBEESO:          (1²+1²)^82 = 2^82 > u64::MAX → SATURATES.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);
    add_edge(T114_ID_A, T114_ID_B, "t114.e.ab");

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             2,        "k2: node_count=2");
    assert_eq!(ec,             1,        "k2: edge_count=1");
    assert_eq!(noctaoctactc,  2,        "k2: NOCTAOCTACTC=2 (1^88+1^88=2)");
    assert_eq!(nhoctaoctactc, u64::MAX, "k2: NHOCTAOCTACTC=SAT (2^87>u64::MAX)");
    assert_eq!(nbeeso,        u64::MAX, "k2: NBEESO=SAT (2^82>u64::MAX)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);
    add_node(T114_VEC_C, T114_KEY_C, T114_ID_C);
    add_edge(T114_ID_A, T114_ID_B, "t114.e.ab");
    add_edge(T114_ID_B, T114_ID_C, "t114.e.bc");

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             3,        "p3: node_count=3");
    assert_eq!(ec,             2,        "p3: edge_count=2");
    assert_eq!(noctaoctactc,  u64::MAX, "p3: NOCTAOCTACTC=SAT (3\u{00d7}2^88>u64)");
    assert_eq!(nhoctaoctactc, u64::MAX, "p3: NHOCTAOCTACTC=SAT (4^87>u64)");
    assert_eq!(nbeeso,        u64::MAX, "p3: NBEESO=SAT (8^82>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);
    add_node(T114_VEC_C, T114_KEY_C, T114_ID_C);
    add_edge(T114_ID_A, T114_ID_B, "t114.e.ab");
    add_edge(T114_ID_B, T114_ID_C, "t114.e.bc");
    add_edge(T114_ID_C, T114_ID_A, "t114.e.ca");

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(noctaoctactc,  u64::MAX, "k3: NOCTAOCTACTC=SAT");
    assert_eq!(nhoctaoctactc, u64::MAX, "k3: NHOCTAOCTACTC=SAT");
    assert_eq!(nbeeso,        u64::MAX, "k3: NBEESO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A); // hub
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);
    add_node(T114_VEC_C, T114_KEY_C, T114_ID_C);
    add_node(T114_VEC_D, T114_KEY_D, T114_ID_D);
    add_node(T114_VEC_E, T114_KEY_E, T114_ID_E);
    add_edge(T114_ID_A, T114_ID_B, "t114.e.ab");
    add_edge(T114_ID_A, T114_ID_C, "t114.e.ac");
    add_edge(T114_ID_A, T114_ID_D, "t114.e.ad");
    add_edge(T114_ID_A, T114_ID_E, "t114.e.ae");

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(noctaoctactc,  u64::MAX, "k14: NOCTAOCTACTC=SAT");
    assert_eq!(nhoctaoctactc, u64::MAX, "k14: NHOCTAOCTACTC=SAT");
    assert_eq!(nbeeso,        u64::MAX, "k14: NBEESO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);
    add_node(T114_VEC_C, T114_KEY_C, T114_ID_C);
    add_node(T114_VEC_D, T114_KEY_D, T114_ID_D);
    add_edge(T114_ID_A, T114_ID_B, "t114.e.ab");
    add_edge(T114_ID_B, T114_ID_C, "t114.e.bc");
    add_edge(T114_ID_C, T114_ID_D, "t114.e.cd");

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(noctaoctactc,  u64::MAX, "p4: NOCTAOCTACTC=SAT");
    assert_eq!(nhoctaoctactc, u64::MAX, "p4: NHOCTAOCTACTC=SAT");
    assert_eq!(nbeeso,        u64::MAX, "p4: NBEESO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);
    add_node(T114_VEC_C, T114_KEY_C, T114_ID_C);
    add_node(T114_VEC_D, T114_KEY_D, T114_ID_D);
    add_edge(T114_ID_A, T114_ID_B, "t114.e.ab");
    add_edge(T114_ID_A, T114_ID_C, "t114.e.ac");
    add_edge(T114_ID_A, T114_ID_D, "t114.e.ad");
    add_edge(T114_ID_B, T114_ID_C, "t114.e.bc");
    add_edge(T114_ID_B, T114_ID_D, "t114.e.bd");
    add_edge(T114_ID_C, T114_ID_D, "t114.e.cd");

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(noctaoctactc,  u64::MAX, "k4: NOCTAOCTACTC=SAT");
    assert_eq!(nhoctaoctactc, u64::MAX, "k4: NHOCTAOCTACTC=SAT");
    assert_eq!(nbeeso,        u64::MAX, "k4: NBEESO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             2, "2iso: node_count=2");
    assert_eq!(ec,             0, "2iso: edge_count=0");
    assert_eq!(noctaoctactc,  0, "2iso: NOCTAOCTACTC=0");
    assert_eq!(nhoctaoctactc, 0, "2iso: NHOCTAOCTACTC=0");
    assert_eq!(nbeeso,        0, "2iso: NBEESO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T114_VEC_A, T114_KEY_A, T114_ID_A);
    add_node(T114_VEC_B, T114_KEY_B, T114_ID_B);
    add_node(T114_VEC_C, T114_KEY_C, T114_ID_C);
    add_node(T114_VEC_D, T114_KEY_D, T114_ID_D);
    add_node(T114_VEC_E, T114_KEY_E, T114_ID_E);
    add_edge(T114_ID_A, T114_ID_C, "t114.e.ac");
    add_edge(T114_ID_A, T114_ID_D, "t114.e.ad");
    add_edge(T114_ID_A, T114_ID_E, "t114.e.ae");
    add_edge(T114_ID_B, T114_ID_C, "t114.e.bc");
    add_edge(T114_ID_B, T114_ID_D, "t114.e.bd");
    add_edge(T114_ID_B, T114_ID_E, "t114.e.be");

    let (noctaoctactc, nhoctaoctactc, nbeeso, ec, nc) = gos_runtime::graph_topo_indices114();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(noctaoctactc,  u64::MAX, "k23: NOCTAOCTACTC=SAT");
    assert_eq!(nhoctaoctactc, u64::MAX, "k23: NHOCTAOCTACTC=SAT");
    assert_eq!(nbeeso,        u64::MAX, "k23: NBEESO=SAT");
}
