// gos-graph-topo93-harness — V3.104 NHEXAHEPTACTC + NHHEXAHEPTACTC + NBJSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices93()`:
//   Returns (nhexaheptactc, nhhexaheptactc, nbjso, edge_count, node_count)
//   - nhexaheptactc  = NHEXAHEPTACTC(G) = Σ_v S(v)^67                      (exact u64; S-Hexaheptacontic vertex sum)
//   - nhhexaheptactc = NHHEXAHEPTACTC(G) = Σ_{uv∈E} (S_u+S_v)^66           (exact u64; S-Hexaheptacontic edge-sum)
//   - nbjso          = NBJSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^61         (exact u64; S-Variant Sombor, α=122)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXAHEPTACTC(G) = Σ_v S(v)^67
//     S-Hexaheptacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXAHEXAACTC=Σ S⁶⁶ (topo92), NHEXAHEPTACTC=Σ S⁶⁷ (topo93).
//     Eighth of the hexacontic (60-69) series.
//     NHEXAHEPTACTC = n·S^67 for S-regular.
//     Overflow: S^67 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^67 = s64 × s2 × s  (67=64+2+1; 8 mults total).
//
//   NHHEXAHEPTACTC(G) = Σ_{uv∈E} (S_u+S_v)^66
//     S-Hexaheptacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXAHEXAACTC=Σ(S+S)⁶⁵ (topo92),
//       NHHEXAHEPTACTC=Σ(S+S)⁶⁶ (topo93).
//     NHHEXAHEPTACTC = |E|·(2S)^66 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^66 → saturating u128 accumulator.
//     Implementation: ss^66 = ss64 × ss2  (66=64+2; 7 mults).
//
//   NBJSO(G) = Σ_{uv∈E} (S_u²+S_v²)^61
//     S-Variant Sombor: generalised Sombor SO^α with α=122 on S-variant.
//     10th of NB series, letter J (after NBISOS α=120 topo92).
//     NSO(topo21,α=1),..., NBISOS(topo92,α=120), NBJSO(topo93,α=122).
//     NBJSO = |E|·(2S²)^61 for S-regular.
//     Overflow per edge: (2×16129²)^61 → saturating u128 accumulator.
//     Implementation: s2s^61 = s2s32 × s2s16 × s2s8 × s2s4 × s2s  (61=32+16+8+4+1; 5 mults).
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
//  Graph     NHEXAHEPTACTC(exact)         NHHEXAHEPTACTC(exact)        NBJSO(exact)               edges  nodes
//  Empty                      0                             0                    0                  0      0
//  1 node                     0                             0                    0                  0      1
//  K₂                         2              u64::MAX(sat.)     2_305_843_009_213_693_952           1      2
//  P₃              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)              2      3
//  K₃              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)              3      3
//  K_{1,4}         u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)              4      5
//  P₄              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)              3      4
//  K₄              u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)              6      4
//  2 isolated                 0                             0                    0                  0      2
//  K_{2,3}         u64::MAX(sat.)            u64::MAX(sat.)             u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXAHEPTACTC:  1^67 + 1^67 = 2. ✓
//     NHHEXAHEPTACTC: (1+1)^66 = 2^66 = 73_786_976_294_838_206_464 > u64::MAX → SATURATES. ✓
//     NBJSO:          (1²+1²)^61 = 2^61 = 2_305_843_009_213_693_952. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXAHEPTACTC:  3×2^67 >> u64::MAX → SATURATES. ✓
//     NHHEXAHEPTACTC: 2×(4)^66 → SATURATES. ✓
//     NBJSO:          2×(8)^61 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXAHEPTACTC:  3×4^67 → SATURATES. ✓
//     NHHEXAHEPTACTC: 3×8^66 → SATURATES. ✓
//     NBJSO:          3×32^61 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXAHEPTACTC:  5×4^67 → SATURATES. ✓
//     NHHEXAHEPTACTC: 4×8^66 → SATURATES. ✓
//     NBJSO:          4×32^61 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXAHEPTACTC:  2×2^67 + 2×3^67. 3^67 >> u64::MAX → SATURATES. ✓
//     NHHEXAHEPTACTC: 5^66+6^66+5^66 → SATURATES. ✓
//     NBJSO:          13^61+18^61+13^61 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXAHEPTACTC:  4×9^67 → SATURATES. ✓
//     NHHEXAHEPTACTC: 6×18^66 → SATURATES. ✓
//     NBJSO:          6×162^61 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXAHEPTACTC:  5×6^67 → SATURATES. ✓
//     NHHEXAHEPTACTC: 6×12^66 → SATURATES. ✓
//     NBJSO:          6×72^61 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXAHEPTACTC  = n·S^67                                                                              for S-regular ✓
//   NHHEXAHEPTACTC = |E|·(2S)^66 (saturates for |E|≥1,S≥1)                                             for S-regular ✓
//   NBJSO          = |E|·(2S²)^61                                                                        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, u64::MAX, 2_305_843_009_213_693_952, 1, 2)
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

const T93_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_93");
const T93_EXEC:   ExecutorId = ExecutorId::from_ascii("t93.exec");

const T93_KEY_A: &str = "t93.alpha";
const T93_KEY_B: &str = "t93.beta";
const T93_KEY_C: &str = "t93.gamma";
const T93_KEY_D: &str = "t93.delta";
const T93_KEY_E: &str = "t93.epsilon";

const T93_ID_A: NodeId = derive_node_id(T93_PLUGIN, T93_KEY_A);
const T93_ID_B: NodeId = derive_node_id(T93_PLUGIN, T93_KEY_B);
const T93_ID_C: NodeId = derive_node_id(T93_PLUGIN, T93_KEY_C);
const T93_ID_D: NodeId = derive_node_id(T93_PLUGIN, T93_KEY_D);
const T93_ID_E: NodeId = derive_node_id(T93_PLUGIN, T93_KEY_E);

// L4=180 namespace for this harness.
const T93_VEC_A: VectorAddress = VectorAddress::new(180, 1, 1, 0);
const T93_VEC_B: VectorAddress = VectorAddress::new(180, 1, 2, 0);
const T93_VEC_C: VectorAddress = VectorAddress::new(180, 1, 3, 0);
const T93_VEC_D: VectorAddress = VectorAddress::new(180, 2, 1, 0);
const T93_VEC_E: VectorAddress = VectorAddress::new(180, 2, 2, 0);

const T93_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T93_PLUGIN,
    name:         "kl-graph-topo93-harness",
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
        executor_id:       T93_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T93_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T93_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               0, "empty: node_count=0");
    assert_eq!(ec,               0, "empty: edge_count=0");
    assert_eq!(nhexaheptactc,    0, "empty: NHEXAHEPTACTC=0");
    assert_eq!(nhhexaheptactc,   0, "empty: NHHEXAHEPTACTC=0");
    assert_eq!(nbjso,            0, "empty: NBJSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               1, "single: node_count=1");
    assert_eq!(ec,               0, "single: edge_count=0");
    assert_eq!(nhexaheptactc,    0, "single: NHEXAHEPTACTC=0");
    assert_eq!(nhhexaheptactc,   0, "single: NHHEXAHEPTACTC=0");
    assert_eq!(nbjso,            0, "single: NBJSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXAHEPTACTC:  1^67 + 1^67 = 2.
// NHHEXAHEPTACTC: (1+1)^66 = 2^66 = 73_786_976_294_838_206_464 > u64::MAX → SATURATES.
// NBJSO:          (1²+1²)^61 = 2^61 = 2_305_843_009_213_693_952.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);
    add_edge(T93_ID_A, T93_ID_B, "t93.e.ab");

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               2,                           "k2: node_count=2");
    assert_eq!(ec,               1,                           "k2: edge_count=1");
    assert_eq!(nhexaheptactc,    2,                           "k2: NHEXAHEPTACTC=2 (1^67+1^67=2)");
    assert_eq!(nhhexaheptactc,   u64::MAX,                    "k2: NHHEXAHEPTACTC=SAT (2^66>u64::MAX)");
    assert_eq!(nbjso,            2_305_843_009_213_693_952,   "k2: NBJSO=2_305_843_009_213_693_952 (2^61)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);
    add_node(T93_VEC_C, T93_KEY_C, T93_ID_C);
    add_edge(T93_ID_A, T93_ID_B, "t93.e.ab");
    add_edge(T93_ID_B, T93_ID_C, "t93.e.bc");

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               3,         "p3: node_count=3");
    assert_eq!(ec,               2,         "p3: edge_count=2");
    assert_eq!(nhexaheptactc,    u64::MAX,  "p3: NHEXAHEPTACTC=SAT (3\u{00d7}2^67>u64)");
    assert_eq!(nhhexaheptactc,   u64::MAX,  "p3: NHHEXAHEPTACTC=SAT (4^66>u64)");
    assert_eq!(nbjso,            u64::MAX,  "p3: NBJSO=SAT (8^61>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);
    add_node(T93_VEC_C, T93_KEY_C, T93_ID_C);
    add_edge(T93_ID_A, T93_ID_B, "t93.e.ab");
    add_edge(T93_ID_B, T93_ID_C, "t93.e.bc");
    add_edge(T93_ID_C, T93_ID_A, "t93.e.ca");

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nhexaheptactc,    u64::MAX, "k3: NHEXAHEPTACTC=SAT");
    assert_eq!(nhhexaheptactc,   u64::MAX, "k3: NHHEXAHEPTACTC=SAT");
    assert_eq!(nbjso,            u64::MAX, "k3: NBJSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A); // hub
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);
    add_node(T93_VEC_C, T93_KEY_C, T93_ID_C);
    add_node(T93_VEC_D, T93_KEY_D, T93_ID_D);
    add_node(T93_VEC_E, T93_KEY_E, T93_ID_E);
    add_edge(T93_ID_A, T93_ID_B, "t93.e.ab");
    add_edge(T93_ID_A, T93_ID_C, "t93.e.ac");
    add_edge(T93_ID_A, T93_ID_D, "t93.e.ad");
    add_edge(T93_ID_A, T93_ID_E, "t93.e.ae");

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nhexaheptactc,    u64::MAX, "k14: NHEXAHEPTACTC=SAT");
    assert_eq!(nhhexaheptactc,   u64::MAX, "k14: NHHEXAHEPTACTC=SAT");
    assert_eq!(nbjso,            u64::MAX, "k14: NBJSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// S(A)=2,S(B)=3,S(C)=3,S(D)=2; 3 edges, 4 nodes. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);
    add_node(T93_VEC_C, T93_KEY_C, T93_ID_C);
    add_node(T93_VEC_D, T93_KEY_D, T93_ID_D);
    add_edge(T93_ID_A, T93_ID_B, "t93.e.ab");
    add_edge(T93_ID_B, T93_ID_C, "t93.e.bc");
    add_edge(T93_ID_C, T93_ID_D, "t93.e.cd");

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nhexaheptactc,    u64::MAX, "p4: NHEXAHEPTACTC=SAT");
    assert_eq!(nhhexaheptactc,   u64::MAX, "p4: NHHEXAHEPTACTC=SAT");
    assert_eq!(nbjso,            u64::MAX, "p4: NBJSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);
    add_node(T93_VEC_C, T93_KEY_C, T93_ID_C);
    add_node(T93_VEC_D, T93_KEY_D, T93_ID_D);
    add_edge(T93_ID_A, T93_ID_B, "t93.e.ab");
    add_edge(T93_ID_A, T93_ID_C, "t93.e.ac");
    add_edge(T93_ID_A, T93_ID_D, "t93.e.ad");
    add_edge(T93_ID_B, T93_ID_C, "t93.e.bc");
    add_edge(T93_ID_B, T93_ID_D, "t93.e.bd");
    add_edge(T93_ID_C, T93_ID_D, "t93.e.cd");

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nhexaheptactc,    u64::MAX, "k4: NHEXAHEPTACTC=SAT");
    assert_eq!(nhhexaheptactc,   u64::MAX, "k4: NHHEXAHEPTACTC=SAT");
    assert_eq!(nbjso,            u64::MAX, "k4: NBJSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → all indices 0; 2 nodes.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               2, "2iso: node_count=2");
    assert_eq!(ec,               0, "2iso: edge_count=0");
    assert_eq!(nhexaheptactc,    0, "2iso: NHEXAHEPTACTC=0");
    assert_eq!(nhhexaheptactc,   0, "2iso: NHHEXAHEPTACTC=0");
    assert_eq!(nbjso,            0, "2iso: NBJSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform (all nodes), 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_bipartite_k23() {
    let _g = setup();
    // Left: A, B  |  Right: C, D, E
    add_node(T93_VEC_A, T93_KEY_A, T93_ID_A);
    add_node(T93_VEC_B, T93_KEY_B, T93_ID_B);
    add_node(T93_VEC_C, T93_KEY_C, T93_ID_C);
    add_node(T93_VEC_D, T93_KEY_D, T93_ID_D);
    add_node(T93_VEC_E, T93_KEY_E, T93_ID_E);
    add_edge(T93_ID_A, T93_ID_C, "t93.e.ac");
    add_edge(T93_ID_A, T93_ID_D, "t93.e.ad");
    add_edge(T93_ID_A, T93_ID_E, "t93.e.ae");
    add_edge(T93_ID_B, T93_ID_C, "t93.e.bc");
    add_edge(T93_ID_B, T93_ID_D, "t93.e.bd");
    add_edge(T93_ID_B, T93_ID_E, "t93.e.be");

    let (nhexaheptactc, nhhexaheptactc, nbjso, ec, nc) = gos_runtime::graph_topo_indices93();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nhexaheptactc,    u64::MAX, "k23: NHEXAHEPTACTC=SAT");
    assert_eq!(nhhexaheptactc,   u64::MAX, "k23: NHHEXAHEPTACTC=SAT");
    assert_eq!(nbjso,            u64::MAX, "k23: NBJSO=SAT");
}
