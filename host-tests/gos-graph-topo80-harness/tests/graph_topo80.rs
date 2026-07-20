// gos-graph-topo80-harness — V3.91 NTETRAPENTAACTC + NHTETRAPENTAACTC + NAWSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices80()`:
//   Returns (ntetrapentaactc, nhtetrapentaactc, nawso, edge_count, node_count)
//   - ntetrapentaactc  = NTETRAPENTAACTC(G)  = Σ_v S(v)^54                   (exact u64; S-Tetrapentacontic vertex sum)
//   - nhtetrapentaactc = NHTETRAPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^53         (exact u64; S-Tripentacontic edge-sum)
//   - nawso            = NAWSO(G)            = Σ_{uv∈E} (S_u²+S_v²)^48       (exact u64; S-Variant Sombor, α=96)
//   - edge_count       = undirected non-self-loop edges
//   - node_count       = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTETRAPENTAACTC(G) = Σ_v S(v)^54
//     S-Tetrapentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NTRIPENTAACTC=Σ S⁵³ (topo79), NTETRAPENTAACTC=Σ S⁵⁴ (topo80). Fifth of the pentacontic (50-59) series.
//     NTETRAPENTAACTC = n·S^54 for S-regular.
//     Overflow: S^54 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^54 = s32 × s16 × s4 × s2  (54=32+16+4+2; 4 mults).
//
//   NHTETRAPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^53
//     S-Tripentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHTRIPENTAACTC=Σ(S+S)⁵² (topo79),
//       NHTETRAPENTAACTC=Σ(S+S)⁵³ (topo80).
//     NHTETRAPENTAACTC = |E|·(2S)^53 = 9007199254740992|E|·S^53 for S-regular.
//     Overflow per edge: (2×16129)^53 → saturating u128 accumulator.
//     Implementation: ss^53 = ss32 × ss16 × ss4 × ss  (53=32+16+4+1; 4 mults).
//
//   NAWSO(G) = Σ_{uv∈E} (S_u²+S_v²)^48
//     S-Variant Sombor: generalised Sombor SO^α with α=96 on S-variant.
//     3rd-pass double-letter "AW" (after NAVSO α=94, topo79).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NAVSO(topo79,α=94), NAWSO(topo80,α=96).
//     NAWSO = |E|·(2S²)^48 = 281474976710656|E|·S^96 for S-regular.
//     Overflow per edge: (2×16129²)^48 → saturating u128 accumulator.
//     Implementation: s2s^48 = s2s32 × s2s16  (48=32+16; 2 mults — very efficient!).
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
//  Graph     NTETRAPENTAACTC(exact)          NHTETRAPENTAACTC(exact)        NAWSO(exact)              edges  nodes
//  Empty                    0                               0                         0                0      0
//  1 node                   0                               0                         0                0      1
//  K₂                       2             9_007_199_254_740_992         281_474_976_710_656               1      2
//  P₃     54_043_195_528_445_952               u64::MAX(sat.)               u64::MAX(sat.)              2      3
//  K₃           u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)              3      3
//  K_{1,4}      u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)              4      5
//  P₄           u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)              3      4
//  K₄           u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)              6      4
//  2 isolated               0                               0                         0                0      2
//  K_{2,3}      u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)              6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NTETRAPENTAACTC:  1^54 + 1^54 = 2. ✓
//     NHTETRAPENTAACTC: (1+1)^53 = 2^53 = 9_007_199_254_740_992. ✓
//     NAWSO:            (1²+1²)^48 = 2^48 = 281_474_976_710_656. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTETRAPENTAACTC:  3×2^54 = 3×18_014_398_509_481_984 = 54_043_195_528_445_952. ✓
//     NHTETRAPENTAACTC: 2×(2+2)^53 = 2×4^53 = 2×2^106 → SATURATES. ✓
//     NAWSO:            2×(4+4)^48 = 2×8^48 = 2×2^144 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTETRAPENTAACTC:  3×4^54 = 3×2^108 → SATURATES. ✓
//     NHTETRAPENTAACTC: 3×8^53 → SATURATES. ✓
//     NAWSO:            3×32^48 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTETRAPENTAACTC:  5×4^54 → SATURATES. ✓
//     NHTETRAPENTAACTC: 4×8^53 → SATURATES. ✓
//     NAWSO:            4×32^48 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTETRAPENTAACTC:  2×2^54 + 2×3^54. 3^54>>u64::MAX → SATURATES. ✓
//     NHTETRAPENTAACTC: 5^53+6^53+5^53 → each term >> u64::MAX → SATURATES. ✓
//     NAWSO:            13^48+18^48+13^48 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTETRAPENTAACTC:  4×9^54 → SATURATES. ✓
//     NHTETRAPENTAACTC: 6×18^53 → SATURATES. ✓
//     NAWSO:            6×162^48 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTETRAPENTAACTC:  5×6^54 → SATURATES. ✓
//     NHTETRAPENTAACTC: 6×12^53 → SATURATES. ✓
//     NAWSO:            6×72^48 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTETRAPENTAACTC   = n·S^54                                                               for S-regular ✓
//   NHTETRAPENTAACTC  = |E|·(2S)^53 = 9007199254740992|E|·S^53                              for S-regular ✓
//   NAWSO             = |E|·(2S²)^48 = 281474976710656|E|·S^96                              for S-regular ✓
//   Note: s2s^48 = s2s32 × s2s16 is very efficient (48=32+16, sum of two powers of 2, 1 final mult)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 9_007_199_254_740_992, 281_474_976_710_656, 1, 2)
//  4.  Path P₃ = A-B-C                   → (54_043_195_528_445_952, u64::MAX, u64::MAX, 2, 3)
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

const T80_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_80");
const T80_EXEC:   ExecutorId = ExecutorId::from_ascii("t80.exec");

const T80_KEY_A: &str = "t80.alpha";
const T80_KEY_B: &str = "t80.beta";
const T80_KEY_C: &str = "t80.gamma";
const T80_KEY_D: &str = "t80.delta";
const T80_KEY_E: &str = "t80.epsilon";

const T80_ID_A: NodeId = derive_node_id(T80_PLUGIN, T80_KEY_A);
const T80_ID_B: NodeId = derive_node_id(T80_PLUGIN, T80_KEY_B);
const T80_ID_C: NodeId = derive_node_id(T80_PLUGIN, T80_KEY_C);
const T80_ID_D: NodeId = derive_node_id(T80_PLUGIN, T80_KEY_D);
const T80_ID_E: NodeId = derive_node_id(T80_PLUGIN, T80_KEY_E);

// L4=167 namespace for this harness.
const T80_VEC_A: VectorAddress = VectorAddress::new(167, 1, 1, 0);
const T80_VEC_B: VectorAddress = VectorAddress::new(167, 1, 2, 0);
const T80_VEC_C: VectorAddress = VectorAddress::new(167, 1, 3, 0);
const T80_VEC_D: VectorAddress = VectorAddress::new(167, 2, 1, 0);
const T80_VEC_E: VectorAddress = VectorAddress::new(167, 2, 2, 0);

const T80_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T80_PLUGIN,
    name:         "kl-graph-topo80-harness",
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
        executor_id:       T80_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T80_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T80_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(ntetrapentaactc,   0, "empty: NTETRAPENTAACTC=0");
    assert_eq!(nhtetrapentaactc,  0, "empty: NHTETRAPENTAACTC=0");
    assert_eq!(nawso,             0, "empty: NAWSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(ntetrapentaactc,   0, "single: NTETRAPENTAACTC=0");
    assert_eq!(nhtetrapentaactc,  0, "single: NHTETRAPENTAACTC=0");
    assert_eq!(nawso,             0, "single: NAWSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NTETRAPENTAACTC:  1^54+1^54 = 2.
// NHTETRAPENTAACTC: (1+1)^53 = 2^53 = 9_007_199_254_740_992.
// NAWSO:            (1²+1²)^48 = 2^48 = 281_474_976_710_656.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);
    add_edge(T80_ID_A, T80_ID_B, "t80.e.ab");

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                2,                        "k2: node_count=2");
    assert_eq!(ec,                1,                        "k2: edge_count=1");
    assert_eq!(ntetrapentaactc,   2,                        "k2: NTETRAPENTAACTC=2 (1\u{2075}\u{2074}+1\u{2075}\u{2074}=2)");
    assert_eq!(nhtetrapentaactc,  9_007_199_254_740_992,    "k2: NHTETRAPENTAACTC=9_007_199_254_740_992 (2\u{2075}\u{00b3}=2^53)");
    assert_eq!(nawso,             281_474_976_710_656,      "k2: NAWSO=281_474_976_710_656 (2\u{2074}\u{2078}=2^48)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NTETRAPENTAACTC:  3×2^54 = 3×18_014_398_509_481_984 = 54_043_195_528_445_952.
// NHTETRAPENTAACTC: 2×(2+2)^53 = 2×4^53 = 2×2^106 → SATURATES.
// NAWSO:            2×(4+4)^48 = 2×8^48 = 2×2^144 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);
    add_node(T80_VEC_C, T80_KEY_C, T80_ID_C);
    add_edge(T80_ID_A, T80_ID_B, "t80.e.ab");
    add_edge(T80_ID_B, T80_ID_C, "t80.e.bc");

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                3,                           "p3: node_count=3");
    assert_eq!(ec,                2,                           "p3: edge_count=2");
    assert_eq!(ntetrapentaactc,   54_043_195_528_445_952,      "p3: NTETRAPENTAACTC=54_043_195_528_445_952 (3\u{00d7}2\u{2075}\u{2074})");
    assert_eq!(nhtetrapentaactc,  u64::MAX,                    "p3: NHTETRAPENTAACTC=SAT (4\u{2075}\u{00b3}>u64)");
    assert_eq!(nawso,             u64::MAX,                    "p3: NAWSO=SAT (8\u{2074}\u{2078}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);
    add_node(T80_VEC_C, T80_KEY_C, T80_ID_C);
    add_edge(T80_ID_A, T80_ID_B, "t80.e.ab");
    add_edge(T80_ID_B, T80_ID_C, "t80.e.bc");
    add_edge(T80_ID_C, T80_ID_A, "t80.e.ca");

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                3,        "k3: node_count=3");
    assert_eq!(ec,                3,        "k3: edge_count=3");
    assert_eq!(ntetrapentaactc,   u64::MAX, "k3: NTETRAPENTAACTC=SAT");
    assert_eq!(nhtetrapentaactc,  u64::MAX, "k3: NHTETRAPENTAACTC=SAT");
    assert_eq!(nawso,             u64::MAX, "k3: NAWSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A); // hub
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);
    add_node(T80_VEC_C, T80_KEY_C, T80_ID_C);
    add_node(T80_VEC_D, T80_KEY_D, T80_ID_D);
    add_node(T80_VEC_E, T80_KEY_E, T80_ID_E);
    add_edge(T80_ID_A, T80_ID_B, "t80.e.ab");
    add_edge(T80_ID_A, T80_ID_C, "t80.e.ac");
    add_edge(T80_ID_A, T80_ID_D, "t80.e.ad");
    add_edge(T80_ID_A, T80_ID_E, "t80.e.ae");

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                5,        "k14: node_count=5");
    assert_eq!(ec,                4,        "k14: edge_count=4");
    assert_eq!(ntetrapentaactc,   u64::MAX, "k14: NTETRAPENTAACTC=SAT");
    assert_eq!(nhtetrapentaactc,  u64::MAX, "k14: NHTETRAPENTAACTC=SAT");
    assert_eq!(nawso,             u64::MAX, "k14: NAWSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NTETRAPENTAACTC:  2×2^54 + 2×3^54.  3^54>>u64::MAX → SATURATES.
// NHTETRAPENTAACTC: 5^53+6^53+5^53 → SATURATES.
// NAWSO:            13^48+18^48+13^48 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);
    add_node(T80_VEC_C, T80_KEY_C, T80_ID_C);
    add_node(T80_VEC_D, T80_KEY_D, T80_ID_D);
    add_edge(T80_ID_A, T80_ID_B, "t80.e.ab");
    add_edge(T80_ID_B, T80_ID_C, "t80.e.bc");
    add_edge(T80_ID_C, T80_ID_D, "t80.e.cd");

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                4,        "p4: node_count=4");
    assert_eq!(ec,                3,        "p4: edge_count=3");
    assert_eq!(ntetrapentaactc,   u64::MAX, "p4: NTETRAPENTAACTC=SAT");
    assert_eq!(nhtetrapentaactc,  u64::MAX, "p4: NHTETRAPENTAACTC=SAT");
    assert_eq!(nawso,             u64::MAX, "p4: NAWSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);
    add_node(T80_VEC_C, T80_KEY_C, T80_ID_C);
    add_node(T80_VEC_D, T80_KEY_D, T80_ID_D);
    add_edge(T80_ID_A, T80_ID_B, "t80.e.ab");
    add_edge(T80_ID_A, T80_ID_C, "t80.e.ac");
    add_edge(T80_ID_A, T80_ID_D, "t80.e.ad");
    add_edge(T80_ID_B, T80_ID_C, "t80.e.bc");
    add_edge(T80_ID_B, T80_ID_D, "t80.e.bd");
    add_edge(T80_ID_C, T80_ID_D, "t80.e.cd");

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                4,        "k4: node_count=4");
    assert_eq!(ec,                6,        "k4: edge_count=6");
    assert_eq!(ntetrapentaactc,   u64::MAX, "k4: NTETRAPENTAACTC=SAT");
    assert_eq!(nhtetrapentaactc,  u64::MAX, "k4: NHTETRAPENTAACTC=SAT");
    assert_eq!(nawso,             u64::MAX, "k4: NAWSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                2, "isolated: node_count=2");
    assert_eq!(ec,                0, "isolated: edge_count=0");
    assert_eq!(ntetrapentaactc,   0, "isolated: NTETRAPENTAACTC=0");
    assert_eq!(nhtetrapentaactc,  0, "isolated: NHTETRAPENTAACTC=0");
    assert_eq!(nawso,             0, "isolated: NAWSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NTETRAPENTAACTC=5×6^54 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T80_VEC_A, T80_KEY_A, T80_ID_A);
    add_node(T80_VEC_B, T80_KEY_B, T80_ID_B);
    add_node(T80_VEC_C, T80_KEY_C, T80_ID_C);
    add_node(T80_VEC_D, T80_KEY_D, T80_ID_D);
    add_node(T80_VEC_E, T80_KEY_E, T80_ID_E);
    add_edge(T80_ID_A, T80_ID_C, "t80.e.ac");
    add_edge(T80_ID_A, T80_ID_D, "t80.e.ad");
    add_edge(T80_ID_A, T80_ID_E, "t80.e.ae");
    add_edge(T80_ID_B, T80_ID_C, "t80.e.bc");
    add_edge(T80_ID_B, T80_ID_D, "t80.e.bd");
    add_edge(T80_ID_B, T80_ID_E, "t80.e.be");

    let (ntetrapentaactc, nhtetrapentaactc, nawso, ec, nc) = gos_runtime::graph_topo_indices80();
    assert_eq!(nc,                5,        "k23: node_count=5");
    assert_eq!(ec,                6,        "k23: edge_count=6");
    assert_eq!(ntetrapentaactc,   u64::MAX, "k23: NTETRAPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{2074})");
    assert_eq!(nhtetrapentaactc,  u64::MAX, "k23: NHTETRAPENTAACTC=SAT");
    assert_eq!(nawso,             u64::MAX, "k23: NAWSO=SAT");
}
