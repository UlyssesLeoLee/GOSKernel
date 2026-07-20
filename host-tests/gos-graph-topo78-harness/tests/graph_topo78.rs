// gos-graph-topo78-harness — V3.89 NDOPENTAACTC + NHDOPENTAACTC + NAUSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices78()`:
//   Returns (ndopentaactc, nhdopentaactc, nauso, edge_count, node_count)
//   - ndopentaactc  = NDOPENTAACTC(G)  = Σ_v S(v)^52                   (exact u64; S-Dopentacontic vertex sum)
//   - nhdopentaactc = NHDOPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^51         (exact u64; S-Henpentacontic edge-sum)
//   - nauso         = NAUSO(G)         = Σ_{uv∈E} (S_u²+S_v²)^46       (exact u64; S-Variant Sombor, α=92)
//   - edge_count    = undirected non-self-loop edges
//   - node_count    = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NDOPENTAACTC(G) = Σ_v S(v)^52
//     S-Dopentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHENPENTAACTC=Σ S⁵¹ (topo77), NDOPENTAACTC=Σ S⁵² (topo78). Third of the pentacontic (50-59) series.
//     NDOPENTAACTC = n·S^52 for S-regular.
//     Overflow: S^52 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^52 = s32 × s16 × s4  (s32=s16^2; 52=32+16+4; 3 mults — efficient!).
//
//   NHDOPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^51
//     S-Henpentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHENPENTAACTC=Σ(S+S)⁵⁰ (topo77),
//       NHDOPENTAACTC=Σ(S+S)⁵¹ (topo78).
//     NHDOPENTAACTC = |E|·(2S)^51 = 2251799813685248|E|·S^51 for S-regular.
//     Overflow per edge: (2×16129)^51 → saturating u128 accumulator.
//     Implementation: ss^51 = ss32 × ss16 × ss2 × ss  (ss32=ss16^2; 51=32+16+2+1; 4 mults).
//
//   NAUSO(G) = Σ_{uv∈E} (S_u²+S_v²)^46
//     S-Variant Sombor: generalised Sombor SO^α with α=92 on S-variant.
//     3rd-pass double-letter "AU" (after NATSO α=90, topo77).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NATSO(topo77,α=90), NAUSO(topo78,α=92).
//     NAUSO = |E|·(2S²)^46 = 70368744177664|E|·S^92 for S-regular.
//     Overflow per edge: (2×16129²)^46 → saturating u128 accumulator.
//     Implementation: s2s^46 = s2s32 × s2s8 × s2s4 × s2s2  (46=32+8+4+2; 4 mults).
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
//  Graph     NDOPENTAACTC(exact)            NHDOPENTAACTC(exact)           NAUSO(exact)              edges  nodes
//  Empty                    0                               0                         0                0      0
//  1 node                   0                               0                         0                0      1
//  K₂                       2             2_251_799_813_685_248          70_368_744_177_664               1      2
//  P₃     13_510_798_882_111_488               u64::MAX(sat.)               u64::MAX(sat.)              2      3
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
//     NDOPENTAACTC:  1^52 + 1^52 = 2. ✓
//     NHDOPENTAACTC: (1+1)^51 = 2^51 = 2_251_799_813_685_248. ✓
//     NAUSO:         (1²+1²)^46 = 2^46 = 70_368_744_177_664. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NDOPENTAACTC:  3×2^52 = 3×4_503_599_627_370_496 = 13_510_798_882_111_488. ✓
//     NHDOPENTAACTC: 2×(2+2)^51 = 2×4^51 = 2×2^102 → SATURATES. ✓
//     NAUSO:         2×(4+4)^46 = 2×8^46 = 2×2^138 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NDOPENTAACTC:  3×4^52 = 3×2^104 → SATURATES. ✓
//     NHDOPENTAACTC: 3×8^51 → SATURATES. ✓
//     NAUSO:         3×32^46 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NDOPENTAACTC:  5×4^52 → SATURATES. ✓
//     NHDOPENTAACTC: 4×8^51 → SATURATES. ✓
//     NAUSO:         4×32^46 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NDOPENTAACTC:  2×2^52 + 2×3^52. 3^42>u64::MAX → SATURATES. ✓
//     NHDOPENTAACTC: 5^51+6^51+5^51 → each term >> u64::MAX → SATURATES. ✓
//     NAUSO:         13^46+18^46+13^46 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NDOPENTAACTC:  4×9^52 → SATURATES. ✓
//     NHDOPENTAACTC: 6×18^51 → SATURATES. ✓
//     NAUSO:         6×162^46 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NDOPENTAACTC:  5×6^52 → SATURATES. ✓
//     NHDOPENTAACTC: 6×12^51 → SATURATES. ✓
//     NAUSO:         6×72^46 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NDOPENTAACTC   = n·S^52                                                              for S-regular ✓
//   NHDOPENTAACTC  = |E|·(2S)^51 = 2251799813685248|E|·S^51                             for S-regular ✓
//   NAUSO          = |E|·(2S²)^46 = 70368744177664|E|·S^92                              for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 2_251_799_813_685_248, 70_368_744_177_664, 1, 2)
//  4.  Path P₃ = A-B-C                   → (13_510_798_882_111_488, u64::MAX, u64::MAX, 2, 3)
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

const T78_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_78");
const T78_EXEC:   ExecutorId = ExecutorId::from_ascii("t78.exec");

const T78_KEY_A: &str = "t78.alpha";
const T78_KEY_B: &str = "t78.beta";
const T78_KEY_C: &str = "t78.gamma";
const T78_KEY_D: &str = "t78.delta";
const T78_KEY_E: &str = "t78.epsilon";

const T78_ID_A: NodeId = derive_node_id(T78_PLUGIN, T78_KEY_A);
const T78_ID_B: NodeId = derive_node_id(T78_PLUGIN, T78_KEY_B);
const T78_ID_C: NodeId = derive_node_id(T78_PLUGIN, T78_KEY_C);
const T78_ID_D: NodeId = derive_node_id(T78_PLUGIN, T78_KEY_D);
const T78_ID_E: NodeId = derive_node_id(T78_PLUGIN, T78_KEY_E);

// L4=165 namespace for this harness.
const T78_VEC_A: VectorAddress = VectorAddress::new(165, 1, 1, 0);
const T78_VEC_B: VectorAddress = VectorAddress::new(165, 1, 2, 0);
const T78_VEC_C: VectorAddress = VectorAddress::new(165, 1, 3, 0);
const T78_VEC_D: VectorAddress = VectorAddress::new(165, 2, 1, 0);
const T78_VEC_E: VectorAddress = VectorAddress::new(165, 2, 2, 0);

const T78_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T78_PLUGIN,
    name:         "kl-graph-topo78-harness",
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
        executor_id:       T78_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T78_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T78_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             0, "empty: node_count=0");
    assert_eq!(ec,             0, "empty: edge_count=0");
    assert_eq!(ndopentaactc,   0, "empty: NDOPENTAACTC=0");
    assert_eq!(nhdopentaactc,  0, "empty: NHDOPENTAACTC=0");
    assert_eq!(nauso,          0, "empty: NAUSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             1, "single: node_count=1");
    assert_eq!(ec,             0, "single: edge_count=0");
    assert_eq!(ndopentaactc,   0, "single: NDOPENTAACTC=0");
    assert_eq!(nhdopentaactc,  0, "single: NHDOPENTAACTC=0");
    assert_eq!(nauso,          0, "single: NAUSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NDOPENTAACTC:  1^52+1^52 = 2.
// NHDOPENTAACTC: (1+1)^51 = 2^51 = 2_251_799_813_685_248.
// NAUSO:         (1²+1²)^46 = 2^46 = 70_368_744_177_664.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);
    add_edge(T78_ID_A, T78_ID_B, "t78.e.ab");

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             2,                       "k2: node_count=2");
    assert_eq!(ec,             1,                       "k2: edge_count=1");
    assert_eq!(ndopentaactc,   2,                       "k2: NDOPENTAACTC=2 (1\u{2075}\u{00b2}+1\u{2075}\u{00b2}=2)");
    assert_eq!(nhdopentaactc,  2_251_799_813_685_248,   "k2: NHDOPENTAACTC=2_251_799_813_685_248 (2\u{2075}\u{00b9}=2^51)");
    assert_eq!(nauso,          70_368_744_177_664,      "k2: NAUSO=70_368_744_177_664 (2\u{2074}\u{2076}=2^46)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NDOPENTAACTC:  3×2^52 = 3×4_503_599_627_370_496 = 13_510_798_882_111_488.
// NHDOPENTAACTC: 2×(2+2)^51 = 2×4^51 = 2×2^102 → SATURATES.
// NAUSO:         2×(4+4)^46 = 2×8^46 = 2×2^138 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);
    add_node(T78_VEC_C, T78_KEY_C, T78_ID_C);
    add_edge(T78_ID_A, T78_ID_B, "t78.e.ab");
    add_edge(T78_ID_B, T78_ID_C, "t78.e.bc");

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             3,                          "p3: node_count=3");
    assert_eq!(ec,             2,                          "p3: edge_count=2");
    assert_eq!(ndopentaactc,   13_510_798_882_111_488,     "p3: NDOPENTAACTC=13_510_798_882_111_488 (3\u{00d7}2\u{2075}\u{00b2})");
    assert_eq!(nhdopentaactc,  u64::MAX,                   "p3: NHDOPENTAACTC=SAT (4\u{2075}\u{00b9}>u64)");
    assert_eq!(nauso,          u64::MAX,                   "p3: NAUSO=SAT (8\u{2074}\u{2076}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);
    add_node(T78_VEC_C, T78_KEY_C, T78_ID_C);
    add_edge(T78_ID_A, T78_ID_B, "t78.e.ab");
    add_edge(T78_ID_B, T78_ID_C, "t78.e.bc");
    add_edge(T78_ID_C, T78_ID_A, "t78.e.ca");

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(ndopentaactc,   u64::MAX, "k3: NDOPENTAACTC=SAT");
    assert_eq!(nhdopentaactc,  u64::MAX, "k3: NHDOPENTAACTC=SAT");
    assert_eq!(nauso,          u64::MAX, "k3: NAUSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A); // hub
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);
    add_node(T78_VEC_C, T78_KEY_C, T78_ID_C);
    add_node(T78_VEC_D, T78_KEY_D, T78_ID_D);
    add_node(T78_VEC_E, T78_KEY_E, T78_ID_E);
    add_edge(T78_ID_A, T78_ID_B, "t78.e.ab");
    add_edge(T78_ID_A, T78_ID_C, "t78.e.ac");
    add_edge(T78_ID_A, T78_ID_D, "t78.e.ad");
    add_edge(T78_ID_A, T78_ID_E, "t78.e.ae");

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(ndopentaactc,   u64::MAX, "k14: NDOPENTAACTC=SAT");
    assert_eq!(nhdopentaactc,  u64::MAX, "k14: NHDOPENTAACTC=SAT");
    assert_eq!(nauso,          u64::MAX, "k14: NAUSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NDOPENTAACTC:  2×2^52 + 2×3^52.  3^42>u64::MAX → SATURATES.
// NHDOPENTAACTC: 5^51+6^51+5^51 → SATURATES.
// NAUSO:         13^46+18^46+13^46 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);
    add_node(T78_VEC_C, T78_KEY_C, T78_ID_C);
    add_node(T78_VEC_D, T78_KEY_D, T78_ID_D);
    add_edge(T78_ID_A, T78_ID_B, "t78.e.ab");
    add_edge(T78_ID_B, T78_ID_C, "t78.e.bc");
    add_edge(T78_ID_C, T78_ID_D, "t78.e.cd");

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(ndopentaactc,   u64::MAX, "p4: NDOPENTAACTC=SAT");
    assert_eq!(nhdopentaactc,  u64::MAX, "p4: NHDOPENTAACTC=SAT");
    assert_eq!(nauso,          u64::MAX, "p4: NAUSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);
    add_node(T78_VEC_C, T78_KEY_C, T78_ID_C);
    add_node(T78_VEC_D, T78_KEY_D, T78_ID_D);
    add_edge(T78_ID_A, T78_ID_B, "t78.e.ab");
    add_edge(T78_ID_A, T78_ID_C, "t78.e.ac");
    add_edge(T78_ID_A, T78_ID_D, "t78.e.ad");
    add_edge(T78_ID_B, T78_ID_C, "t78.e.bc");
    add_edge(T78_ID_B, T78_ID_D, "t78.e.bd");
    add_edge(T78_ID_C, T78_ID_D, "t78.e.cd");

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(ndopentaactc,   u64::MAX, "k4: NDOPENTAACTC=SAT");
    assert_eq!(nhdopentaactc,  u64::MAX, "k4: NHDOPENTAACTC=SAT");
    assert_eq!(nauso,          u64::MAX, "k4: NAUSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             2, "isolated: node_count=2");
    assert_eq!(ec,             0, "isolated: edge_count=0");
    assert_eq!(ndopentaactc,   0, "isolated: NDOPENTAACTC=0");
    assert_eq!(nhdopentaactc,  0, "isolated: NHDOPENTAACTC=0");
    assert_eq!(nauso,          0, "isolated: NAUSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NDOPENTAACTC=5×6^52 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T78_VEC_A, T78_KEY_A, T78_ID_A);
    add_node(T78_VEC_B, T78_KEY_B, T78_ID_B);
    add_node(T78_VEC_C, T78_KEY_C, T78_ID_C);
    add_node(T78_VEC_D, T78_KEY_D, T78_ID_D);
    add_node(T78_VEC_E, T78_KEY_E, T78_ID_E);
    add_edge(T78_ID_A, T78_ID_C, "t78.e.ac");
    add_edge(T78_ID_A, T78_ID_D, "t78.e.ad");
    add_edge(T78_ID_A, T78_ID_E, "t78.e.ae");
    add_edge(T78_ID_B, T78_ID_C, "t78.e.bc");
    add_edge(T78_ID_B, T78_ID_D, "t78.e.bd");
    add_edge(T78_ID_B, T78_ID_E, "t78.e.be");

    let (ndopentaactc, nhdopentaactc, nauso, ec, nc) = gos_runtime::graph_topo_indices78();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(ndopentaactc,   u64::MAX, "k23: NDOPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{00b2})");
    assert_eq!(nhdopentaactc,  u64::MAX, "k23: NHDOPENTAACTC=SAT");
    assert_eq!(nauso,          u64::MAX, "k23: NAUSO=SAT");
}
