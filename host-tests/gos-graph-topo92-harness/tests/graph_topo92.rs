// gos-graph-topo92-harness — V3.103 NHEXAHEXAACTC + NHHEXAHEXAACTC + NBISOS (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices92()`:
//   Returns (nhexahexaactc, nhhexahexaactc, nbisos, edge_count, node_count)
//   - nhexahexaactc  = NHEXAHEXAACTC(G) = Σ_v S(v)^66                     (exact u64; S-Hexahexacontic vertex sum)
//   - nhhexahexaactc = NHHEXAHEXAACTC(G) = Σ_{uv∈E} (S_u+S_v)^65          (exact u64; S-Hexahexacontic edge-sum)
//   - nbisos         = NBISOS(G)         = Σ_{uv∈E} (S_u²+S_v²)^60        (exact u64; S-Variant Sombor, α=120)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXAHEXAACTC(G) = Σ_v S(v)^66
//     S-Hexahexacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXAPENTACTC=Σ S⁶⁵ (topo91), NHEXAHEXAACTC=Σ S⁶⁶ (topo92).
//     Seventh of the hexacontic (60-69) series.
//     NHEXAHEXAACTC = n·S^66 for S-regular.
//     Overflow: S^66 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^66 = s64 × s2  (66=64+2; s2=s×s, s64=s32×s32; 7 mults total).
//
//   NHHEXAHEXAACTC(G) = Σ_{uv∈E} (S_u+S_v)^65
//     S-Hexahexacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXAPENTACTC=Σ(S+S)⁶⁴ (topo91),
//       NHHEXAHEXAACTC=Σ(S+S)⁶⁵ (topo92).
//     NHHEXAHEXAACTC = |E|·(2S)^65 for S-regular (saturates for |E|≥1,S≥1).
//     Overflow per edge: (2×16129)^65 → saturating u128 accumulator.
//     Implementation: ss^65 = ss64 × ss  (65=64+1; ss64=ss32×ss32; 7 mults).
//
//   NBISOS(G) = Σ_{uv∈E} (S_u²+S_v²)^60
//     S-Variant Sombor: generalised Sombor SO^α with α=120 on S-variant.
//     9th of NB series, letter I (after NBHSO α=118 topo91).
//     NSO(topo21,α=1),..., NBHSO(topo91,α=118), NBISOS(topo92,α=120).
//     NBISOS = |E|·(2S²)^60 for S-regular.
//     Overflow per edge: (2×16129²)^60 → saturating u128 accumulator.
//     Implementation: s2s^60 = s2s32 × s2s16 × s2s8 × s2s4  (60=32+16+8+4; 4 mults — EFFICIENT).
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
//  Graph     NHEXAHEXAACTC(exact)         NHHEXAHEXAACTC(exact)        NBISOS(exact)              edges  nodes
//  Empty                      0                             0                    0                  0      0
//  1 node                     0                             0                    0                  0      1
//  K₂                         2              u64::MAX(sat.)     1_152_921_504_606_846_976           1      2
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
//     NHEXAHEXAACTC:  1^66 + 1^66 = 2. ✓
//     NHHEXAHEXAACTC: (1+1)^65 = 2^65 = 36_893_488_147_419_103_232 > u64::MAX → SATURATES. ✓
//     NBISOS:         (1²+1²)^60 = 2^60 = 1_152_921_504_606_846_976. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXAHEXAACTC:  3×2^66 = 3×2^66 > u64::MAX → SATURATES. ✓
//     NHHEXAHEXAACTC: 2×(2+2)^65 = 2×4^65 → SATURATES. ✓
//     NBISOS:         2×(4+4)^60 = 2×8^60 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXAHEXAACTC:  3×4^66 → SATURATES. ✓
//     NHHEXAHEXAACTC: 3×8^65 → SATURATES. ✓
//     NBISOS:         3×32^60 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXAHEXAACTC:  5×4^66 → SATURATES. ✓
//     NHHEXAHEXAACTC: 4×8^65 → SATURATES. ✓
//     NBISOS:         4×32^60 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXAHEXAACTC:  2×2^66 + 2×3^66. 3^66 >> u64::MAX → SATURATES. ✓
//     NHHEXAHEXAACTC: 5^65+6^65+5^65 → SATURATES. ✓
//     NBISOS:         13^60+18^60+13^60 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXAHEXAACTC:  4×9^66 → SATURATES. ✓
//     NHHEXAHEXAACTC: 6×18^65 → SATURATES. ✓
//     NBISOS:         6×162^60 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXAHEXAACTC:  5×6^66 → SATURATES. ✓
//     NHHEXAHEXAACTC: 6×12^65 → SATURATES. ✓
//     NBISOS:         6×72^60 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXAHEXAACTC  = n·S^66                                                                              for S-regular ✓
//   NHHEXAHEXAACTC = |E|·(2S)^65 (saturates for |E|≥1,S≥1)                                             for S-regular ✓
//   NBISOS         = |E|·(2S²)^60                                                                        for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, u64::MAX, 1_152_921_504_606_846_976, 1, 2)
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

const T92_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_92");
const T92_EXEC:   ExecutorId = ExecutorId::from_ascii("t92.exec");

const T92_KEY_A: &str = "t92.alpha";
const T92_KEY_B: &str = "t92.beta";
const T92_KEY_C: &str = "t92.gamma";
const T92_KEY_D: &str = "t92.delta";
const T92_KEY_E: &str = "t92.epsilon";

const T92_ID_A: NodeId = derive_node_id(T92_PLUGIN, T92_KEY_A);
const T92_ID_B: NodeId = derive_node_id(T92_PLUGIN, T92_KEY_B);
const T92_ID_C: NodeId = derive_node_id(T92_PLUGIN, T92_KEY_C);
const T92_ID_D: NodeId = derive_node_id(T92_PLUGIN, T92_KEY_D);
const T92_ID_E: NodeId = derive_node_id(T92_PLUGIN, T92_KEY_E);

// L4=179 namespace for this harness.
const T92_VEC_A: VectorAddress = VectorAddress::new(179, 1, 1, 0);
const T92_VEC_B: VectorAddress = VectorAddress::new(179, 1, 2, 0);
const T92_VEC_C: VectorAddress = VectorAddress::new(179, 1, 3, 0);
const T92_VEC_D: VectorAddress = VectorAddress::new(179, 2, 1, 0);
const T92_VEC_E: VectorAddress = VectorAddress::new(179, 2, 2, 0);

const T92_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T92_PLUGIN,
    name:         "kl-graph-topo92-harness",
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
        executor_id:       T92_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T92_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T92_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(nhexahexaactc,   0, "empty: NHEXAHEXAACTC=0");
    assert_eq!(nhhexahexaactc,  0, "empty: NHHEXAHEXAACTC=0");
    assert_eq!(nbisos,          0, "empty: NBISOS=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(nhexahexaactc,   0, "single: NHEXAHEXAACTC=0");
    assert_eq!(nhhexahexaactc,  0, "single: NHHEXAHEXAACTC=0");
    assert_eq!(nbisos,          0, "single: NBISOS=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXAHEXAACTC:  1^66+1^66 = 2.
// NHHEXAHEXAACTC: (1+1)^65 = 2^65 = 36_893_488_147_419_103_232 > u64::MAX → SATURATES.
// NBISOS:         (1²+1²)^60 = 2^60 = 1_152_921_504_606_846_976.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);
    add_edge(T92_ID_A, T92_ID_B, "t92.e.ab");

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              2,                           "k2: node_count=2");
    assert_eq!(ec,              1,                           "k2: edge_count=1");
    assert_eq!(nhexahexaactc,   2,                           "k2: NHEXAHEXAACTC=2 (1^66+1^66=2)");
    assert_eq!(nhhexahexaactc,  u64::MAX,                    "k2: NHHEXAHEXAACTC=SAT (2^65>u64::MAX)");
    assert_eq!(nbisos,          1_152_921_504_606_846_976,   "k2: NBISOS=1_152_921_504_606_846_976 (2^60)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);
    add_node(T92_VEC_C, T92_KEY_C, T92_ID_C);
    add_edge(T92_ID_A, T92_ID_B, "t92.e.ab");
    add_edge(T92_ID_B, T92_ID_C, "t92.e.bc");

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              3,         "p3: node_count=3");
    assert_eq!(ec,              2,         "p3: edge_count=2");
    assert_eq!(nhexahexaactc,   u64::MAX,  "p3: NHEXAHEXAACTC=SAT (3\u{00d7}2^66>u64)");
    assert_eq!(nhhexahexaactc,  u64::MAX,  "p3: NHHEXAHEXAACTC=SAT (4^65>u64)");
    assert_eq!(nbisos,          u64::MAX,  "p3: NBISOS=SAT (8^60>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);
    add_node(T92_VEC_C, T92_KEY_C, T92_ID_C);
    add_edge(T92_ID_A, T92_ID_B, "t92.e.ab");
    add_edge(T92_ID_B, T92_ID_C, "t92.e.bc");
    add_edge(T92_ID_C, T92_ID_A, "t92.e.ca");

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(nhexahexaactc,   u64::MAX, "k3: NHEXAHEXAACTC=SAT");
    assert_eq!(nhhexahexaactc,  u64::MAX, "k3: NHHEXAHEXAACTC=SAT");
    assert_eq!(nbisos,          u64::MAX, "k3: NBISOS=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A); // hub
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);
    add_node(T92_VEC_C, T92_KEY_C, T92_ID_C);
    add_node(T92_VEC_D, T92_KEY_D, T92_ID_D);
    add_node(T92_VEC_E, T92_KEY_E, T92_ID_E);
    add_edge(T92_ID_A, T92_ID_B, "t92.e.ab");
    add_edge(T92_ID_A, T92_ID_C, "t92.e.ac");
    add_edge(T92_ID_A, T92_ID_D, "t92.e.ad");
    add_edge(T92_ID_A, T92_ID_E, "t92.e.ae");

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(nhexahexaactc,   u64::MAX, "k14: NHEXAHEXAACTC=SAT");
    assert_eq!(nhhexahexaactc,  u64::MAX, "k14: NHHEXAHEXAACTC=SAT");
    assert_eq!(nbisos,          u64::MAX, "k14: NBISOS=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// S(A)=2, S(B)=3, S(C)=3, S(D)=2. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);
    add_node(T92_VEC_C, T92_KEY_C, T92_ID_C);
    add_node(T92_VEC_D, T92_KEY_D, T92_ID_D);
    add_edge(T92_ID_A, T92_ID_B, "t92.e.ab");
    add_edge(T92_ID_B, T92_ID_C, "t92.e.bc");
    add_edge(T92_ID_C, T92_ID_D, "t92.e.cd");

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(nhexahexaactc,   u64::MAX, "p4: NHEXAHEXAACTC=SAT");
    assert_eq!(nhhexahexaactc,  u64::MAX, "p4: NHHEXAHEXAACTC=SAT");
    assert_eq!(nbisos,          u64::MAX, "p4: NBISOS=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);
    add_node(T92_VEC_C, T92_KEY_C, T92_ID_C);
    add_node(T92_VEC_D, T92_KEY_D, T92_ID_D);
    add_edge(T92_ID_A, T92_ID_B, "t92.e.ab");
    add_edge(T92_ID_A, T92_ID_C, "t92.e.ac");
    add_edge(T92_ID_A, T92_ID_D, "t92.e.ad");
    add_edge(T92_ID_B, T92_ID_C, "t92.e.bc");
    add_edge(T92_ID_B, T92_ID_D, "t92.e.bd");
    add_edge(T92_ID_C, T92_ID_D, "t92.e.cd");

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(nhexahexaactc,   u64::MAX, "k4: NHEXAHEXAACTC=SAT");
    assert_eq!(nhhexahexaactc,  u64::MAX, "k4: NHHEXAHEXAACTC=SAT");
    assert_eq!(nbisos,          u64::MAX, "k4: NBISOS=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              2, "isolated: node_count=2");
    assert_eq!(ec,              0, "isolated: edge_count=0");
    assert_eq!(nhexahexaactc,   0, "isolated: NHEXAHEXAACTC=0");
    assert_eq!(nhhexahexaactc,  0, "isolated: NHHEXAHEXAACTC=0");
    assert_eq!(nbisos,          0, "isolated: NBISOS=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform, 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T92_VEC_A, T92_KEY_A, T92_ID_A);
    add_node(T92_VEC_B, T92_KEY_B, T92_ID_B);
    add_node(T92_VEC_C, T92_KEY_C, T92_ID_C);
    add_node(T92_VEC_D, T92_KEY_D, T92_ID_D);
    add_node(T92_VEC_E, T92_KEY_E, T92_ID_E);
    add_edge(T92_ID_A, T92_ID_C, "t92.e.ac");
    add_edge(T92_ID_A, T92_ID_D, "t92.e.ad");
    add_edge(T92_ID_A, T92_ID_E, "t92.e.ae");
    add_edge(T92_ID_B, T92_ID_C, "t92.e.bc");
    add_edge(T92_ID_B, T92_ID_D, "t92.e.bd");
    add_edge(T92_ID_B, T92_ID_E, "t92.e.be");

    let (nhexahexaactc, nhhexahexaactc, nbisos, ec, nc) = gos_runtime::graph_topo_indices92();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(nhexahexaactc,   u64::MAX, "k23: NHEXAHEXAACTC=SAT (5\u{00d7}6^66)");
    assert_eq!(nhhexahexaactc,  u64::MAX, "k23: NHHEXAHEXAACTC=SAT");
    assert_eq!(nbisos,          u64::MAX, "k23: NBISOS=SAT");
}
