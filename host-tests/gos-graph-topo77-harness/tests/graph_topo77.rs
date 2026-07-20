// gos-graph-topo77-harness — V3.88 NHENPENTAACTC + NHHENPENTAACTC + NATSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices77()`:
//   Returns (nhenpentaactc, nhhenpentaactc, natso, edge_count, node_count)
//   - nhenpentaactc  = NHENPENTAACTC(G)  = Σ_v S(v)^51                   (exact u64; S-Henpentacontic vertex sum)
//   - nhhenpentaactc = NHHENPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^50         (exact u64; S-Pentacontic edge-sum)
//   - natso          = NATSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^45       (exact u64; S-Variant Sombor, α=90)
//   - edge_count     = undirected non-self-loop edges
//   - node_count     = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHENPENTAACTC(G) = Σ_v S(v)^51
//     S-Henpentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NNONATETRAACTC=Σ S⁴⁹ (topo75), NPENTAACTC=Σ S⁵⁰ (topo76),
//       NHENPENTAACTC=Σ S⁵¹ (topo77). Second of the pentacontic (50-59) series.
//     NHENPENTAACTC = n·S^51 for S-regular.
//     Overflow: S^51 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^51 = s32 × s16 × s2 × s  (s32=s16^2; 51=32+16+2+1; 4 mults).
//
//   NHHENPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^50
//     S-Pentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHPENTAACTC=Σ(S+S)⁴⁹ (topo76),
//       NHHENPENTAACTC=Σ(S+S)⁵⁰ (topo77).
//     NHHENPENTAACTC = |E|·(2S)^50 = 1125899906842624|E|·S^50 for S-regular.
//     Overflow per edge: (2×16129)^50 → saturating u128 accumulator.
//     Implementation: ss^50 = ss32 × ss16 × ss2  (ss32=ss16^2; 50=32+16+2; 3 mults — efficient!).
//
//   NATSO(G) = Σ_{uv∈E} (S_u²+S_v²)^45
//     S-Variant Sombor: generalised Sombor SO^α with α=90 on S-variant.
//     3rd-pass double-letter "AT" (after NASSO α=88, topo76).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NASSO(topo76,α=88), NATSO(topo77,α=90).
//     NATSO = |E|·(2S²)^45 = 35184372088832|E|·S^90 for S-regular.
//     Overflow per edge: (2×16129²)^45 → saturating u128 accumulator.
//     Implementation: s2s^45 = s2s32 × s2s8 × s2s4 × s2s  (45=32+8+4+1; 4 mults).
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
//  Graph     NHENPENTAACTC(exact)           NHHENPENTAACTC(exact)          NATSO(exact)              edges  nodes
//  Empty                    0                               0                         0                0      0
//  1 node                   0                               0                         0                0      1
//  K₂                       2             1_125_899_906_842_624          35_184_372_088_832               1      2
//  P₃      6_755_399_441_055_744               u64::MAX(sat.)               u64::MAX(sat.)              2      3
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
//     NHENPENTAACTC:  1^51 + 1^51 = 2. ✓
//     NHHENPENTAACTC: (1+1)^50 = 2^50 = 1_125_899_906_842_624. ✓
//     NATSO:          (1²+1²)^45 = 2^45 = 35_184_372_088_832. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHENPENTAACTC:  3×2^51 = 3×2_251_799_813_685_248 = 6_755_399_441_055_744. ✓
//     NHHENPENTAACTC: 2×(2+2)^50 = 2×4^50 = 2×2^100 → SATURATES. ✓
//     NATSO:          2×(4+4)^45 = 2×8^45 = 2×2^135 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHENPENTAACTC:  3×4^51 = 3×2^102 → SATURATES. ✓
//     NHHENPENTAACTC: 3×8^50 → SATURATES. ✓
//     NATSO:          3×32^45 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHENPENTAACTC:  5×4^51 → SATURATES. ✓
//     NHHENPENTAACTC: 4×8^50 → SATURATES. ✓
//     NATSO:          4×32^45 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHENPENTAACTC:  2×2^51 + 2×3^51. 3^41>u64::MAX → SATURATES. ✓
//     NHHENPENTAACTC: 5^50+6^50+5^50 → each term >> u64::MAX → SATURATES. ✓
//     NATSO:          13^45+18^45+13^45 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHENPENTAACTC:  4×9^51 → SATURATES. ✓
//     NHHENPENTAACTC: 6×18^50 → SATURATES. ✓
//     NATSO:          6×162^45 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHENPENTAACTC:  5×6^51 → SATURATES. ✓
//     NHHENPENTAACTC: 6×12^50 → SATURATES. ✓
//     NATSO:          6×72^45 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHENPENTAACTC  = n·S^51                                                              for S-regular ✓
//   NHHENPENTAACTC = |E|·(2S)^50 = 1125899906842624|E|·S^50                             for S-regular ✓
//   NATSO          = |E|·(2S²)^45 = 35184372088832|E|·S^90                              for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 1_125_899_906_842_624, 35_184_372_088_832, 1, 2)
//  4.  Path P₃ = A-B-C                   → (6_755_399_441_055_744, u64::MAX, u64::MAX, 2, 3)
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

const T77_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_77");
const T77_EXEC:   ExecutorId = ExecutorId::from_ascii("t77.exec");

const T77_KEY_A: &str = "t77.alpha";
const T77_KEY_B: &str = "t77.beta";
const T77_KEY_C: &str = "t77.gamma";
const T77_KEY_D: &str = "t77.delta";
const T77_KEY_E: &str = "t77.epsilon";

const T77_ID_A: NodeId = derive_node_id(T77_PLUGIN, T77_KEY_A);
const T77_ID_B: NodeId = derive_node_id(T77_PLUGIN, T77_KEY_B);
const T77_ID_C: NodeId = derive_node_id(T77_PLUGIN, T77_KEY_C);
const T77_ID_D: NodeId = derive_node_id(T77_PLUGIN, T77_KEY_D);
const T77_ID_E: NodeId = derive_node_id(T77_PLUGIN, T77_KEY_E);

// L4=164 namespace for this harness.
const T77_VEC_A: VectorAddress = VectorAddress::new(164, 1, 1, 0);
const T77_VEC_B: VectorAddress = VectorAddress::new(164, 1, 2, 0);
const T77_VEC_C: VectorAddress = VectorAddress::new(164, 1, 3, 0);
const T77_VEC_D: VectorAddress = VectorAddress::new(164, 2, 1, 0);
const T77_VEC_E: VectorAddress = VectorAddress::new(164, 2, 2, 0);

const T77_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T77_PLUGIN,
    name:         "kl-graph-topo77-harness",
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
        executor_id:       T77_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T77_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T77_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(nhenpentaactc,   0, "empty: NHENPENTAACTC=0");
    assert_eq!(nhhenpentaactc,  0, "empty: NHHENPENTAACTC=0");
    assert_eq!(natso,           0, "empty: NATSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(nhenpentaactc,   0, "single: NHENPENTAACTC=0");
    assert_eq!(nhhenpentaactc,  0, "single: NHHENPENTAACTC=0");
    assert_eq!(natso,           0, "single: NATSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHENPENTAACTC:  1^51+1^51 = 2.
// NHHENPENTAACTC: (1+1)^50 = 2^50 = 1_125_899_906_842_624.
// NATSO:          (1²+1²)^45 = 2^45 = 35_184_372_088_832.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);
    add_edge(T77_ID_A, T77_ID_B, "t77.e.ab");

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              2,                       "k2: node_count=2");
    assert_eq!(ec,              1,                       "k2: edge_count=1");
    assert_eq!(nhenpentaactc,   2,                       "k2: NHENPENTAACTC=2 (1\u{2075}\u{00b9}+1\u{2075}\u{00b9}=2)");
    assert_eq!(nhhenpentaactc,  1_125_899_906_842_624,   "k2: NHHENPENTAACTC=1_125_899_906_842_624 (2\u{2075}\u{2070}=2^50)");
    assert_eq!(natso,           35_184_372_088_832,      "k2: NATSO=35_184_372_088_832 (2\u{2074}\u{2075}=2^45)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHENPENTAACTC:  3×2^51 = 3×2_251_799_813_685_248 = 6_755_399_441_055_744.
// NHHENPENTAACTC: 2×(2+2)^50 = 2×4^50 = 2×2^100 → SATURATES.
// NATSO:          2×(4+4)^45 = 2×8^45 = 2×2^135 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);
    add_node(T77_VEC_C, T77_KEY_C, T77_ID_C);
    add_edge(T77_ID_A, T77_ID_B, "t77.e.ab");
    add_edge(T77_ID_B, T77_ID_C, "t77.e.bc");

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              3,                         "p3: node_count=3");
    assert_eq!(ec,              2,                         "p3: edge_count=2");
    assert_eq!(nhenpentaactc,   6_755_399_441_055_744,     "p3: NHENPENTAACTC=6_755_399_441_055_744 (3\u{00d7}2\u{2075}\u{00b9})");
    assert_eq!(nhhenpentaactc,  u64::MAX,                  "p3: NHHENPENTAACTC=SAT (4\u{2075}\u{2070}>u64)");
    assert_eq!(natso,           u64::MAX,                  "p3: NATSO=SAT (8\u{2074}\u{2075}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);
    add_node(T77_VEC_C, T77_KEY_C, T77_ID_C);
    add_edge(T77_ID_A, T77_ID_B, "t77.e.ab");
    add_edge(T77_ID_B, T77_ID_C, "t77.e.bc");
    add_edge(T77_ID_C, T77_ID_A, "t77.e.ca");

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              3,        "k3: node_count=3");
    assert_eq!(ec,              3,        "k3: edge_count=3");
    assert_eq!(nhenpentaactc,   u64::MAX, "k3: NHENPENTAACTC=SAT");
    assert_eq!(nhhenpentaactc,  u64::MAX, "k3: NHHENPENTAACTC=SAT");
    assert_eq!(natso,           u64::MAX, "k3: NATSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A); // hub
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);
    add_node(T77_VEC_C, T77_KEY_C, T77_ID_C);
    add_node(T77_VEC_D, T77_KEY_D, T77_ID_D);
    add_node(T77_VEC_E, T77_KEY_E, T77_ID_E);
    add_edge(T77_ID_A, T77_ID_B, "t77.e.ab");
    add_edge(T77_ID_A, T77_ID_C, "t77.e.ac");
    add_edge(T77_ID_A, T77_ID_D, "t77.e.ad");
    add_edge(T77_ID_A, T77_ID_E, "t77.e.ae");

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              5,        "k14: node_count=5");
    assert_eq!(ec,              4,        "k14: edge_count=4");
    assert_eq!(nhenpentaactc,   u64::MAX, "k14: NHENPENTAACTC=SAT");
    assert_eq!(nhhenpentaactc,  u64::MAX, "k14: NHHENPENTAACTC=SAT");
    assert_eq!(natso,           u64::MAX, "k14: NATSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NHENPENTAACTC: 2×2^51 + 2×3^51.  3^41>u64::MAX → SATURATES.
// NHHENPENTAACTC: 5^50+6^50+5^50 → SATURATES.
// NATSO: 13^45+18^45+13^45 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);
    add_node(T77_VEC_C, T77_KEY_C, T77_ID_C);
    add_node(T77_VEC_D, T77_KEY_D, T77_ID_D);
    add_edge(T77_ID_A, T77_ID_B, "t77.e.ab");
    add_edge(T77_ID_B, T77_ID_C, "t77.e.bc");
    add_edge(T77_ID_C, T77_ID_D, "t77.e.cd");

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              4,        "p4: node_count=4");
    assert_eq!(ec,              3,        "p4: edge_count=3");
    assert_eq!(nhenpentaactc,   u64::MAX, "p4: NHENPENTAACTC=SAT");
    assert_eq!(nhhenpentaactc,  u64::MAX, "p4: NHHENPENTAACTC=SAT");
    assert_eq!(natso,           u64::MAX, "p4: NATSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);
    add_node(T77_VEC_C, T77_KEY_C, T77_ID_C);
    add_node(T77_VEC_D, T77_KEY_D, T77_ID_D);
    add_edge(T77_ID_A, T77_ID_B, "t77.e.ab");
    add_edge(T77_ID_A, T77_ID_C, "t77.e.ac");
    add_edge(T77_ID_A, T77_ID_D, "t77.e.ad");
    add_edge(T77_ID_B, T77_ID_C, "t77.e.bc");
    add_edge(T77_ID_B, T77_ID_D, "t77.e.bd");
    add_edge(T77_ID_C, T77_ID_D, "t77.e.cd");

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              4,        "k4: node_count=4");
    assert_eq!(ec,              6,        "k4: edge_count=6");
    assert_eq!(nhenpentaactc,   u64::MAX, "k4: NHENPENTAACTC=SAT");
    assert_eq!(nhhenpentaactc,  u64::MAX, "k4: NHHENPENTAACTC=SAT");
    assert_eq!(natso,           u64::MAX, "k4: NATSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              2, "isolated: node_count=2");
    assert_eq!(ec,              0, "isolated: edge_count=0");
    assert_eq!(nhenpentaactc,   0, "isolated: NHENPENTAACTC=0");
    assert_eq!(nhhenpentaactc,  0, "isolated: NHHENPENTAACTC=0");
    assert_eq!(natso,           0, "isolated: NATSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NHENPENTAACTC=5×6^51 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T77_VEC_A, T77_KEY_A, T77_ID_A);
    add_node(T77_VEC_B, T77_KEY_B, T77_ID_B);
    add_node(T77_VEC_C, T77_KEY_C, T77_ID_C);
    add_node(T77_VEC_D, T77_KEY_D, T77_ID_D);
    add_node(T77_VEC_E, T77_KEY_E, T77_ID_E);
    add_edge(T77_ID_A, T77_ID_C, "t77.e.ac");
    add_edge(T77_ID_A, T77_ID_D, "t77.e.ad");
    add_edge(T77_ID_A, T77_ID_E, "t77.e.ae");
    add_edge(T77_ID_B, T77_ID_C, "t77.e.bc");
    add_edge(T77_ID_B, T77_ID_D, "t77.e.bd");
    add_edge(T77_ID_B, T77_ID_E, "t77.e.be");

    let (nhenpentaactc, nhhenpentaactc, natso, ec, nc) = gos_runtime::graph_topo_indices77();
    assert_eq!(nc,              5,        "k23: node_count=5");
    assert_eq!(ec,              6,        "k23: edge_count=6");
    assert_eq!(nhenpentaactc,   u64::MAX, "k23: NHENPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{00b9})");
    assert_eq!(nhhenpentaactc,  u64::MAX, "k23: NHHENPENTAACTC=SAT");
    assert_eq!(natso,           u64::MAX, "k23: NATSO=SAT");
}
