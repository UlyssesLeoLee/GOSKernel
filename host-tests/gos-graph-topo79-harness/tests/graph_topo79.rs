// gos-graph-topo79-harness — V3.90 NTRIPENTAACTC + NHTRIPENTAACTC + NAVSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices79()`:
//   Returns (ntripentaactc, nhtripentaactc, navso, edge_count, node_count)
//   - ntripentaactc  = NTRIPENTAACTC(G)  = Σ_v S(v)^53                   (exact u64; S-Tripentacontic vertex sum)
//   - nhtripentaactc = NHTRIPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^52         (exact u64; S-Dopentacontic edge-sum)
//   - navso          = NAVSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^47       (exact u64; S-Variant Sombor, α=94)
//   - edge_count     = undirected non-self-loop edges
//   - node_count     = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NTRIPENTAACTC(G) = Σ_v S(v)^53
//     S-Tripentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NDOPENTAACTC=Σ S⁵² (topo78), NTRIPENTAACTC=Σ S⁵³ (topo79). Fourth of the pentacontic (50-59) series.
//     NTRIPENTAACTC = n·S^53 for S-regular.
//     Overflow: S^53 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^53 = s32 × s16 × s4 × s  (53=32+16+4+1; 4 mults).
//
//   NHTRIPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^52
//     S-Dopentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHDOPENTAACTC=Σ(S+S)⁵¹ (topo78),
//       NHTRIPENTAACTC=Σ(S+S)⁵² (topo79).
//     NHTRIPENTAACTC = |E|·(2S)^52 = 4503599627370496|E|·S^52 for S-regular.
//     Overflow per edge: (2×16129)^52 → saturating u128 accumulator.
//     Implementation: ss^52 = ss32 × ss16 × ss4  (ss32=ss16^2; 52=32+16+4; 3 mults — efficient!).
//
//   NAVSO(G) = Σ_{uv∈E} (S_u²+S_v²)^47
//     S-Variant Sombor: generalised Sombor SO^α with α=94 on S-variant.
//     3rd-pass double-letter "AV" (after NAUSO α=92, topo78).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NAUSO(topo78,α=92), NAVSO(topo79,α=94).
//     NAVSO = |E|·(2S²)^47 = 140737488355328|E|·S^94 for S-regular.
//     Overflow per edge: (2×16129²)^47 → saturating u128 accumulator.
//     Implementation: s2s^47 = s2s32 × s2s8 × s2s4 × s2s2 × s2s  (47=32+8+4+2+1; 5 mults).
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
//  Graph     NTRIPENTAACTC(exact)           NHTRIPENTAACTC(exact)          NAVSO(exact)              edges  nodes
//  Empty                    0                               0                         0                0      0
//  1 node                   0                               0                         0                0      1
//  K₂                       2             4_503_599_627_370_496         140_737_488_355_328               1      2
//  P₃     27_021_597_764_222_976               u64::MAX(sat.)               u64::MAX(sat.)              2      3
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
//     NTRIPENTAACTC:  1^53 + 1^53 = 2. ✓
//     NHTRIPENTAACTC: (1+1)^52 = 2^52 = 4_503_599_627_370_496. ✓
//     NAVSO:          (1²+1²)^47 = 2^47 = 140_737_488_355_328. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NTRIPENTAACTC:  3×2^53 = 3×9_007_199_254_740_992 = 27_021_597_764_222_976. ✓
//     NHTRIPENTAACTC: 2×(2+2)^52 = 2×4^52 = 2×2^104 → SATURATES. ✓
//     NAVSO:          2×(4+4)^47 = 2×8^47 = 2×2^141 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NTRIPENTAACTC:  3×4^53 = 3×2^106 → SATURATES. ✓
//     NHTRIPENTAACTC: 3×8^52 → SATURATES. ✓
//     NAVSO:          3×32^47 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NTRIPENTAACTC:  5×4^53 → SATURATES. ✓
//     NHTRIPENTAACTC: 4×8^52 → SATURATES. ✓
//     NAVSO:          4×32^47 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NTRIPENTAACTC:  2×2^53 + 2×3^53. 3^53>>u64::MAX → SATURATES. ✓
//     NHTRIPENTAACTC: 5^52+6^52+5^52 → each term >> u64::MAX → SATURATES. ✓
//     NAVSO:          13^47+18^47+13^47 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NTRIPENTAACTC:  4×9^53 → SATURATES. ✓
//     NHTRIPENTAACTC: 6×18^52 → SATURATES. ✓
//     NAVSO:          6×162^47 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NTRIPENTAACTC:  5×6^53 → SATURATES. ✓
//     NHTRIPENTAACTC: 6×12^52 → SATURATES. ✓
//     NAVSO:          6×72^47 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NTRIPENTAACTC   = n·S^53                                                              for S-regular ✓
//   NHTRIPENTAACTC  = |E|·(2S)^52 = 4503599627370496|E|·S^52                             for S-regular ✓
//   NAVSO           = |E|·(2S²)^47 = 140737488355328|E|·S^94                             for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 4_503_599_627_370_496, 140_737_488_355_328, 1, 2)
//  4.  Path P₃ = A-B-C                   → (27_021_597_764_222_976, u64::MAX, u64::MAX, 2, 3)
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

const T79_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_79");
const T79_EXEC:   ExecutorId = ExecutorId::from_ascii("t79.exec");

const T79_KEY_A: &str = "t79.alpha";
const T79_KEY_B: &str = "t79.beta";
const T79_KEY_C: &str = "t79.gamma";
const T79_KEY_D: &str = "t79.delta";
const T79_KEY_E: &str = "t79.epsilon";

const T79_ID_A: NodeId = derive_node_id(T79_PLUGIN, T79_KEY_A);
const T79_ID_B: NodeId = derive_node_id(T79_PLUGIN, T79_KEY_B);
const T79_ID_C: NodeId = derive_node_id(T79_PLUGIN, T79_KEY_C);
const T79_ID_D: NodeId = derive_node_id(T79_PLUGIN, T79_KEY_D);
const T79_ID_E: NodeId = derive_node_id(T79_PLUGIN, T79_KEY_E);

// L4=166 namespace for this harness.
const T79_VEC_A: VectorAddress = VectorAddress::new(166, 1, 1, 0);
const T79_VEC_B: VectorAddress = VectorAddress::new(166, 1, 2, 0);
const T79_VEC_C: VectorAddress = VectorAddress::new(166, 1, 3, 0);
const T79_VEC_D: VectorAddress = VectorAddress::new(166, 2, 1, 0);
const T79_VEC_E: VectorAddress = VectorAddress::new(166, 2, 2, 0);

const T79_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T79_PLUGIN,
    name:         "kl-graph-topo79-harness",
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
        executor_id:       T79_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T79_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T79_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(ntripentaactc,   0, "empty: NTRIPENTAACTC=0");
    assert_eq!(nhtripentaactc,  0, "empty: NHTRIPENTAACTC=0");
    assert_eq!(navso,           0, "empty: NAVSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(ntripentaactc,   0, "single: NTRIPENTAACTC=0");
    assert_eq!(nhtripentaactc,  0, "single: NHTRIPENTAACTC=0");
    assert_eq!(navso,           0, "single: NAVSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NTRIPENTAACTC:  1^53+1^53 = 2.
// NHTRIPENTAACTC: (1+1)^52 = 2^52 = 4_503_599_627_370_496.
// NAVSO:          (1²+1²)^47 = 2^47 = 140_737_488_355_328.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);
    add_edge(T79_ID_A, T79_ID_B, "t79.e.ab");

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              2,                       "k2: node_count=2");
    assert_eq!(ec,              1,                       "k2: edge_count=1");
    assert_eq!(ntripentaactc,   2,                       "k2: NTRIPENTAACTC=2 (1\u{2075}\u{00b3}+1\u{2075}\u{00b3}=2)");
    assert_eq!(nhtripentaactc,  4_503_599_627_370_496,   "k2: NHTRIPENTAACTC=4_503_599_627_370_496 (2\u{2075}\u{00b2}=2^52)");
    assert_eq!(navso,           140_737_488_355_328,     "k2: NAVSO=140_737_488_355_328 (2\u{2074}\u{2077}=2^47)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NTRIPENTAACTC:  3×2^53 = 3×9_007_199_254_740_992 = 27_021_597_764_222_976.
// NHTRIPENTAACTC: 2×(2+2)^52 = 2×4^52 = 2×2^104 → SATURATES.
// NAVSO:          2×(4+4)^47 = 2×8^47 = 2×2^141 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);
    add_node(T79_VEC_C, T79_KEY_C, T79_ID_C);
    add_edge(T79_ID_A, T79_ID_B, "t79.e.ab");
    add_edge(T79_ID_B, T79_ID_C, "t79.e.bc");

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              3,                          "p3: node_count=3");
    assert_eq!(ec,              2,                          "p3: edge_count=2");
    assert_eq!(ntripentaactc,   27_021_597_764_222_976,     "p3: NTRIPENTAACTC=27_021_597_764_222_976 (3\u{00d7}2\u{2075}\u{00b3})");
    assert_eq!(nhtripentaactc,  u64::MAX,                   "p3: NHTRIPENTAACTC=SAT (4\u{2075}\u{00b2}>u64)");
    assert_eq!(navso,           u64::MAX,                   "p3: NAVSO=SAT (8\u{2074}\u{2077}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);
    add_node(T79_VEC_C, T79_KEY_C, T79_ID_C);
    add_edge(T79_ID_A, T79_ID_B, "t79.e.ab");
    add_edge(T79_ID_B, T79_ID_C, "t79.e.bc");
    add_edge(T79_ID_C, T79_ID_A, "t79.e.ca");

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(ntripentaactc,   u64::MAX, "k3: NTRIPENTAACTC=SAT");
    assert_eq!(nhtripentaactc,  u64::MAX, "k3: NHTRIPENTAACTC=SAT");
    assert_eq!(navso,           u64::MAX, "k3: NAVSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A); // hub
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);
    add_node(T79_VEC_C, T79_KEY_C, T79_ID_C);
    add_node(T79_VEC_D, T79_KEY_D, T79_ID_D);
    add_node(T79_VEC_E, T79_KEY_E, T79_ID_E);
    add_edge(T79_ID_A, T79_ID_B, "t79.e.ab");
    add_edge(T79_ID_A, T79_ID_C, "t79.e.ac");
    add_edge(T79_ID_A, T79_ID_D, "t79.e.ad");
    add_edge(T79_ID_A, T79_ID_E, "t79.e.ae");

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(ntripentaactc,   u64::MAX, "k14: NTRIPENTAACTC=SAT");
    assert_eq!(nhtripentaactc,  u64::MAX, "k14: NHTRIPENTAACTC=SAT");
    assert_eq!(navso,           u64::MAX, "k14: NAVSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NTRIPENTAACTC:  2×2^53 + 2×3^53.  3^53>>u64::MAX → SATURATES.
// NHTRIPENTAACTC: 5^52+6^52+5^52 → SATURATES.
// NAVSO:          13^47+18^47+13^47 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);
    add_node(T79_VEC_C, T79_KEY_C, T79_ID_C);
    add_node(T79_VEC_D, T79_KEY_D, T79_ID_D);
    add_edge(T79_ID_A, T79_ID_B, "t79.e.ab");
    add_edge(T79_ID_B, T79_ID_C, "t79.e.bc");
    add_edge(T79_ID_C, T79_ID_D, "t79.e.cd");

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(ntripentaactc,   u64::MAX, "p4: NTRIPENTAACTC=SAT");
    assert_eq!(nhtripentaactc,  u64::MAX, "p4: NHTRIPENTAACTC=SAT");
    assert_eq!(navso,           u64::MAX, "p4: NAVSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);
    add_node(T79_VEC_C, T79_KEY_C, T79_ID_C);
    add_node(T79_VEC_D, T79_KEY_D, T79_ID_D);
    add_edge(T79_ID_A, T79_ID_B, "t79.e.ab");
    add_edge(T79_ID_A, T79_ID_C, "t79.e.ac");
    add_edge(T79_ID_A, T79_ID_D, "t79.e.ad");
    add_edge(T79_ID_B, T79_ID_C, "t79.e.bc");
    add_edge(T79_ID_B, T79_ID_D, "t79.e.bd");
    add_edge(T79_ID_C, T79_ID_D, "t79.e.cd");

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(ntripentaactc,   u64::MAX, "k4: NTRIPENTAACTC=SAT");
    assert_eq!(nhtripentaactc,  u64::MAX, "k4: NHTRIPENTAACTC=SAT");
    assert_eq!(navso,           u64::MAX, "k4: NAVSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              2, "isolated: node_count=2");
    assert_eq!(ec,              0, "isolated: edge_count=0");
    assert_eq!(ntripentaactc,   0, "isolated: NTRIPENTAACTC=0");
    assert_eq!(nhtripentaactc,  0, "isolated: NHTRIPENTAACTC=0");
    assert_eq!(navso,           0, "isolated: NAVSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NTRIPENTAACTC=5×6^53 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T79_VEC_A, T79_KEY_A, T79_ID_A);
    add_node(T79_VEC_B, T79_KEY_B, T79_ID_B);
    add_node(T79_VEC_C, T79_KEY_C, T79_ID_C);
    add_node(T79_VEC_D, T79_KEY_D, T79_ID_D);
    add_node(T79_VEC_E, T79_KEY_E, T79_ID_E);
    add_edge(T79_ID_A, T79_ID_C, "t79.e.ac");
    add_edge(T79_ID_A, T79_ID_D, "t79.e.ad");
    add_edge(T79_ID_A, T79_ID_E, "t79.e.ae");
    add_edge(T79_ID_B, T79_ID_C, "t79.e.bc");
    add_edge(T79_ID_B, T79_ID_D, "t79.e.bd");
    add_edge(T79_ID_B, T79_ID_E, "t79.e.be");

    let (ntripentaactc, nhtripentaactc, navso, ec, nc) = gos_runtime::graph_topo_indices79();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(ntripentaactc,   u64::MAX, "k23: NTRIPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{00b3})");
    assert_eq!(nhtripentaactc,  u64::MAX, "k23: NHTRIPENTAACTC=SAT");
    assert_eq!(navso,           u64::MAX, "k23: NAVSO=SAT");
}
