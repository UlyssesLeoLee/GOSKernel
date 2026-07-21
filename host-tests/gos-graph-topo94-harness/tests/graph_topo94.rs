// gos-graph-topo94-harness — V3.105 NHEXAOCTACTC + NHHEXAOCTACTC + NBKSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices94()`:
//   Returns (nhexaoctactc, nhhexaoctactc, nbkso, edge_count, node_count)
//   - nhexaoctactc  = NHEXAOCTACTC(G) = Σ_v S(v)^68                      (exact u64; S-Hexaoctatontic vertex sum)
//   - nhhexaoctactc = NHHEXAOCTACTC(G) = Σ_{uv∈E} (S_u+S_v)^67           (exact u64; S-Hexaoctatontic edge-sum)
//   - nbkso         = NBKSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^62         (exact u64; S-Variant Sombor, α=124)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXAOCTACTC(G) = Σ_v S(v)^68
//     S-Hexaoctatontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXAHEPTACTC=Σ S⁶⁷ (topo93), NHEXAOCTACTC=Σ S⁶⁸ (topo94).
//     Ninth of the hexacontic (60-69) series.
//     NHEXAOCTACTC = n·S^68 for S-regular.
//     Overflow: S^68 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^68 = s64 × s4  (68=64+4; 7 mults total).
//
//   NHHEXAOCTACTC(G) = Σ_{uv∈E} (S_u+S_v)^67
//     S-Hexaoctatontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXAHEPTACTC=Σ(S+S)⁶⁶ (topo93),
//       NHHEXAOCTACTC=Σ(S+S)⁶⁷ (topo94).
//     NHHEXAOCTACTC = |E|·(2S)^67 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^67 → saturating u128 accumulator.
//     Implementation: ss^67 = ss64 × ss2 × ss  (67=64+2+1; 8 mults).
//
//   NBKSO(G) = Σ_{uv∈E} (S_u²+S_v²)^62
//     S-Variant Sombor: generalised Sombor SO^α with α=124 on S-variant.
//     11th of NB series, letter K (after NBJSO α=122 topo93).
//     NSO(topo21,α=1),..., NBJSO(topo93,α=122), NBKSO(topo94,α=124).
//     NBKSO = |E|·(2S²)^62 for S-regular.
//     Overflow per edge: (2×16129²)^62 → saturating u128 accumulator.
//     Implementation: s2s^62 = s2s32 × s2s16 × s2s8 × s2s4 × s2s2  (62=32+16+8+4+2; 5 mults).
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
//  Graph     NHEXAOCTACTC(exact)          NHHEXAOCTACTC(exact)         NBKSO(exact)               edges  nodes
//  Empty                      0                             0                   0                   0      0
//  1 node                     0                             0                   0                   0      1
//  K₂                         2              u64::MAX(sat.)     4_611_686_018_427_387_904           1      2
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
//     NHEXAOCTACTC:  1^68 + 1^68 = 2. ✓
//     NHHEXAOCTACTC: (1+1)^67 = 2^67 = 147_573_952_589_676_412_928 > u64::MAX → SATURATES. ✓
//     NBKSO:         (1²+1²)^62 = 2^62 = 4_611_686_018_427_387_904. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXAOCTACTC:  3×2^68 >> u64::MAX → SATURATES. ✓
//     NHHEXAOCTACTC: 2×(4)^67 → SATURATES. ✓
//     NBKSO:         2×(8)^62 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXAOCTACTC:  3×4^68 → SATURATES. ✓
//     NHHEXAOCTACTC: 3×8^67 → SATURATES. ✓
//     NBKSO:         3×32^62 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXAOCTACTC:  5×4^68 → SATURATES. ✓
//     NHHEXAOCTACTC: 4×8^67 → SATURATES. ✓
//     NBKSO:         4×32^62 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXAOCTACTC:  2×2^68 + 2×3^68. 3^68 >> u64::MAX → SATURATES. ✓
//     NHHEXAOCTACTC: 5^67+6^67+5^67 → SATURATES. ✓
//     NBKSO:         13^62+18^62+13^62 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXAOCTACTC:  4×9^68 → SATURATES. ✓
//     NHHEXAOCTACTC: 6×18^67 → SATURATES. ✓
//     NBKSO:         6×162^62 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXAOCTACTC:  5×6^68 → SATURATES. ✓
//     NHHEXAOCTACTC: 6×12^67 → SATURATES. ✓
//     NBKSO:         6×72^62 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXAOCTACTC  = n·S^68                                                                              for S-regular ✓
//   NHHEXAOCTACTC = |E|·(2S)^67 (saturates for |E|≥1,S≥1)                                             for S-regular ✓
//   NBKSO         = |E|·(2S²)^62                                                                        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, u64::MAX, 4_611_686_018_427_387_904, 1, 2)
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

const T94_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_94");
const T94_EXEC:   ExecutorId = ExecutorId::from_ascii("t94.exec");

const T94_KEY_A: &str = "t94.alpha";
const T94_KEY_B: &str = "t94.beta";
const T94_KEY_C: &str = "t94.gamma";
const T94_KEY_D: &str = "t94.delta";
const T94_KEY_E: &str = "t94.epsilon";

const T94_ID_A: NodeId = derive_node_id(T94_PLUGIN, T94_KEY_A);
const T94_ID_B: NodeId = derive_node_id(T94_PLUGIN, T94_KEY_B);
const T94_ID_C: NodeId = derive_node_id(T94_PLUGIN, T94_KEY_C);
const T94_ID_D: NodeId = derive_node_id(T94_PLUGIN, T94_KEY_D);
const T94_ID_E: NodeId = derive_node_id(T94_PLUGIN, T94_KEY_E);

// L4=181 namespace for this harness.
const T94_VEC_A: VectorAddress = VectorAddress::new(181, 1, 1, 0);
const T94_VEC_B: VectorAddress = VectorAddress::new(181, 1, 2, 0);
const T94_VEC_C: VectorAddress = VectorAddress::new(181, 1, 3, 0);
const T94_VEC_D: VectorAddress = VectorAddress::new(181, 2, 1, 0);
const T94_VEC_E: VectorAddress = VectorAddress::new(181, 2, 2, 0);

const T94_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T94_PLUGIN,
    name:         "kl-graph-topo94-harness",
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
        executor_id:       T94_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T94_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T94_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(nhexaoctactc,   0, "empty: NHEXAOCTACTC=0");
    assert_eq!(nhhexaoctactc,  0, "empty: NHHEXAOCTACTC=0");
    assert_eq!(nbkso,          0, "empty: NBKSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(nhexaoctactc,   0, "single: NHEXAOCTACTC=0");
    assert_eq!(nhhexaoctactc,  0, "single: NHHEXAOCTACTC=0");
    assert_eq!(nbkso,          0, "single: NBKSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXAOCTACTC:  1^68 + 1^68 = 2.
// NHHEXAOCTACTC: (1+1)^67 = 2^67 = 147_573_952_589_676_412_928 > u64::MAX → SATURATES.
// NBKSO:         (1²+1²)^62 = 2^62 = 4_611_686_018_427_387_904.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);
    add_edge(T94_ID_A, T94_ID_B, "t94.e.ab");

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             2,                           "k2: node_count=2");
    assert_eq!(ec,             1,                           "k2: edge_count=1");
    assert_eq!(nhexaoctactc,   2,                           "k2: NHEXAOCTACTC=2 (1^68+1^68=2)");
    assert_eq!(nhhexaoctactc,  u64::MAX,                    "k2: NHHEXAOCTACTC=SAT (2^67>u64::MAX)");
    assert_eq!(nbkso,          4_611_686_018_427_387_904,   "k2: NBKSO=4_611_686_018_427_387_904 (2^62)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);
    add_node(T94_VEC_C, T94_KEY_C, T94_ID_C);
    add_edge(T94_ID_A, T94_ID_B, "t94.e.ab");
    add_edge(T94_ID_B, T94_ID_C, "t94.e.bc");

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             3,         "p3: node_count=3");
    assert_eq!(ec,             2,         "p3: edge_count=2");
    assert_eq!(nhexaoctactc,   u64::MAX,  "p3: NHEXAOCTACTC=SAT (3\u{00d7}2^68>u64)");
    assert_eq!(nhhexaoctactc,  u64::MAX,  "p3: NHHEXAOCTACTC=SAT (4^67>u64)");
    assert_eq!(nbkso,          u64::MAX,  "p3: NBKSO=SAT (8^62>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);
    add_node(T94_VEC_C, T94_KEY_C, T94_ID_C);
    add_edge(T94_ID_A, T94_ID_B, "t94.e.ab");
    add_edge(T94_ID_B, T94_ID_C, "t94.e.bc");
    add_edge(T94_ID_C, T94_ID_A, "t94.e.ca");

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(nhexaoctactc,   u64::MAX, "k3: NHEXAOCTACTC=SAT");
    assert_eq!(nhhexaoctactc,  u64::MAX, "k3: NHHEXAOCTACTC=SAT");
    assert_eq!(nbkso,          u64::MAX, "k3: NBKSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A); // hub
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);
    add_node(T94_VEC_C, T94_KEY_C, T94_ID_C);
    add_node(T94_VEC_D, T94_KEY_D, T94_ID_D);
    add_node(T94_VEC_E, T94_KEY_E, T94_ID_E);
    add_edge(T94_ID_A, T94_ID_B, "t94.e.ab");
    add_edge(T94_ID_A, T94_ID_C, "t94.e.ac");
    add_edge(T94_ID_A, T94_ID_D, "t94.e.ad");
    add_edge(T94_ID_A, T94_ID_E, "t94.e.ae");

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(nhexaoctactc,   u64::MAX, "k14: NHEXAOCTACTC=SAT");
    assert_eq!(nhhexaoctactc,  u64::MAX, "k14: NHHEXAOCTACTC=SAT");
    assert_eq!(nbkso,          u64::MAX, "k14: NBKSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);
    add_node(T94_VEC_C, T94_KEY_C, T94_ID_C);
    add_node(T94_VEC_D, T94_KEY_D, T94_ID_D);
    add_edge(T94_ID_A, T94_ID_B, "t94.e.ab");
    add_edge(T94_ID_B, T94_ID_C, "t94.e.bc");
    add_edge(T94_ID_C, T94_ID_D, "t94.e.cd");

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(nhexaoctactc,   u64::MAX, "p4: NHEXAOCTACTC=SAT");
    assert_eq!(nhhexaoctactc,  u64::MAX, "p4: NHHEXAOCTACTC=SAT");
    assert_eq!(nbkso,          u64::MAX, "p4: NBKSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);
    add_node(T94_VEC_C, T94_KEY_C, T94_ID_C);
    add_node(T94_VEC_D, T94_KEY_D, T94_ID_D);
    add_edge(T94_ID_A, T94_ID_B, "t94.e.ab");
    add_edge(T94_ID_A, T94_ID_C, "t94.e.ac");
    add_edge(T94_ID_A, T94_ID_D, "t94.e.ad");
    add_edge(T94_ID_B, T94_ID_C, "t94.e.bc");
    add_edge(T94_ID_B, T94_ID_D, "t94.e.bd");
    add_edge(T94_ID_C, T94_ID_D, "t94.e.cd");

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(nhexaoctactc,   u64::MAX, "k4: NHEXAOCTACTC=SAT");
    assert_eq!(nhhexaoctactc,  u64::MAX, "k4: NHHEXAOCTACTC=SAT");
    assert_eq!(nbkso,          u64::MAX, "k4: NBKSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             2, "2iso: node_count=2");
    assert_eq!(ec,             0, "2iso: edge_count=0");
    assert_eq!(nhexaoctactc,   0, "2iso: NHEXAOCTACTC=0");
    assert_eq!(nhhexaoctactc,  0, "2iso: NHHEXAOCTACTC=0");
    assert_eq!(nbkso,          0, "2iso: NBKSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T94_VEC_A, T94_KEY_A, T94_ID_A);
    add_node(T94_VEC_B, T94_KEY_B, T94_ID_B);
    add_node(T94_VEC_C, T94_KEY_C, T94_ID_C);
    add_node(T94_VEC_D, T94_KEY_D, T94_ID_D);
    add_node(T94_VEC_E, T94_KEY_E, T94_ID_E);
    add_edge(T94_ID_A, T94_ID_C, "t94.e.ac");
    add_edge(T94_ID_A, T94_ID_D, "t94.e.ad");
    add_edge(T94_ID_A, T94_ID_E, "t94.e.ae");
    add_edge(T94_ID_B, T94_ID_C, "t94.e.bc");
    add_edge(T94_ID_B, T94_ID_D, "t94.e.bd");
    add_edge(T94_ID_B, T94_ID_E, "t94.e.be");

    let (nhexaoctactc, nhhexaoctactc, nbkso, ec, nc) = gos_runtime::graph_topo_indices94();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(nhexaoctactc,   u64::MAX, "k23: NHEXAOCTACTC=SAT");
    assert_eq!(nhhexaoctactc,  u64::MAX, "k23: NHHEXAOCTACTC=SAT");
    assert_eq!(nbkso,          u64::MAX, "k23: NBKSO=SAT");
}
