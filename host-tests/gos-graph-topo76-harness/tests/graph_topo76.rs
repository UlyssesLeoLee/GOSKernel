// gos-graph-topo76-harness — V3.87 NPENTAACTC + NHPENTAACTC + NASSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices76()`:
//   Returns (npentaactc, nhpentaactc, nasso, edge_count, node_count)
//   - npentaactc  = NPENTAACTC(G)  = Σ_v S(v)^50                   (exact u64; S-Pentacontic vertex sum)
//   - nhpentaactc = NHPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^49         (exact u64; S-Nonapentacontic edge-sum)
//   - nasso       = NASSO(G)       = Σ_{uv∈E} (S_u²+S_v²)^44       (exact u64; S-Variant Sombor, α=88)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NPENTAACTC(G) = Σ_v S(v)^50
//     S-Pentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), ...,
//       NOCTOTETRAACTC=Σ S⁴⁸ (topo74), NNONATETRAACTC=Σ S⁴⁹ (topo75),
//       NPENTAACTC=Σ S⁵⁰ (topo76). First of the pentacontic (50-59) series.
//     NPENTAACTC = n·S^50 for S-regular.
//     Overflow: S^50 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^50 = s32 × s16 × s2  (s32=s16^2; 50=32+16+2; 3 mults — efficient!).
//
//   NHPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^49
//     S-Nonapentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHTETRAACTC=Σ(S+S)³⁹ (topo66),
//       NHHENTETRAACTC=Σ(S+S)⁴⁰ (topo67), ..., NHOCTOTETRAACTC=Σ(S+S)⁴⁷ (topo74),
//       NHNONATETRAACTC=Σ(S+S)⁴⁸ (topo75), NHPENTAACTC=Σ(S+S)⁴⁹ (topo76).
//     NHPENTAACTC = |E|·(2S)^49 = 562949953421312|E|·S^49 for S-regular.
//     Overflow per edge: (2×16129)^49 → saturating u128 accumulator.
//     Implementation: ss^49 = ss32 × ss16 × ss  (ss32=ss16^2; 49=32+16+1; 3 mults).
//
//   NASSO(G) = Σ_{uv∈E} (S_u²+S_v²)^44
//     S-Variant Sombor: generalised Sombor SO^α with α=88 on S-variant.
//     3rd-pass double-letter "AS" (after NARSO α=86, topo75).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NARSO(topo75,α=86), NASSO(topo76,α=88).
//     NASSO = |E|·(2S²)^44 = 17592186044416|E|·S^88 for S-regular.
//     Overflow per edge: (2×16129²)^44 → saturating u128 accumulator.
//     Implementation: s2s^44 = s2s32 × s2s8 × s2s4  (44=32+8+4; 3 mults — efficient!).
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
//  Graph     NPENTAACTC(exact)              NHPENTAACTC(exact)             NASSO(exact)              edges  nodes
//  Empty                    0                               0                         0                0      0
//  1 node                   0                               0                         0                0      1
//  K₂                       2               562_949_953_421_312          17_592_186_044_416               1      2
//  P₃      3_377_699_720_527_872               u64::MAX(sat.)               u64::MAX(sat.)              2      3
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
//     NPENTAACTC:  1^50 + 1^50 = 2. ✓
//     NHPENTAACTC: (1+1)^49 = 2^49 = 562_949_953_421_312. ✓
//     NASSO:       (1²+1²)^44 = 2^44 = 17_592_186_044_416. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NPENTAACTC:  3×2^50 = 3×1_125_899_906_842_624 = 3_377_699_720_527_872. ✓
//     NHPENTAACTC: 2×(2+2)^49 = 2×4^49 = 2×2^98 → SATURATES. ✓
//     NASSO:       2×(4+4)^44 = 2×8^44 = 2×2^132 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NPENTAACTC:  3×4^50 = 3×2^100 → SATURATES. ✓
//     NHPENTAACTC: 3×8^49 → SATURATES. ✓
//     NASSO:       3×32^44 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NPENTAACTC:  5×4^50 → SATURATES. ✓
//     NHPENTAACTC: 4×8^49 → SATURATES. ✓
//     NASSO:       4×32^44 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NPENTAACTC:  2×2^50 + 2×3^50.
//       3^41>u64::MAX → 3^50 >> u64::MAX → SATURATES. ✓
//     NHPENTAACTC: 2×5^49 + 6^49 → each term >> u64::MAX → SATURATES. ✓
//     NASSO:       2×13^44 + 18^44 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NPENTAACTC:  4×9^50 → SATURATES. ✓
//     NHPENTAACTC: 6×18^49 → SATURATES. ✓
//     NASSO:       6×162^44 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NPENTAACTC:  5×6^50 → SATURATES. ✓
//     NHPENTAACTC: 6×12^49 → SATURATES. ✓
//     NASSO:       6×72^44 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NPENTAACTC  = n·S^50                                                              for S-regular ✓
//   NHPENTAACTC = |E|·(2S)^49 = 562949953421312|E|·S^49                              for S-regular ✓
//   NASSO       = |E|·(2S²)^44 = 17592186044416|E|·S^88                              for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 562_949_953_421_312, 17_592_186_044_416, 1, 2)
//  4.  Path P₃ = A-B-C                   → (3_377_699_720_527_872, u64::MAX, u64::MAX, 2, 3)
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

const T76_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_76");
const T76_EXEC:   ExecutorId = ExecutorId::from_ascii("t76.exec");

const T76_KEY_A: &str = "t76.alpha";
const T76_KEY_B: &str = "t76.beta";
const T76_KEY_C: &str = "t76.gamma";
const T76_KEY_D: &str = "t76.delta";
const T76_KEY_E: &str = "t76.epsilon";

const T76_ID_A: NodeId = derive_node_id(T76_PLUGIN, T76_KEY_A);
const T76_ID_B: NodeId = derive_node_id(T76_PLUGIN, T76_KEY_B);
const T76_ID_C: NodeId = derive_node_id(T76_PLUGIN, T76_KEY_C);
const T76_ID_D: NodeId = derive_node_id(T76_PLUGIN, T76_KEY_D);
const T76_ID_E: NodeId = derive_node_id(T76_PLUGIN, T76_KEY_E);

// L4=163 namespace for this harness.
const T76_VEC_A: VectorAddress = VectorAddress::new(163, 1, 1, 0);
const T76_VEC_B: VectorAddress = VectorAddress::new(163, 1, 2, 0);
const T76_VEC_C: VectorAddress = VectorAddress::new(163, 1, 3, 0);
const T76_VEC_D: VectorAddress = VectorAddress::new(163, 2, 1, 0);
const T76_VEC_E: VectorAddress = VectorAddress::new(163, 2, 2, 0);

const T76_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T76_PLUGIN,
    name:         "kl-graph-topo76-harness",
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
        executor_id:       T76_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T76_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T76_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           0, "empty: node_count=0");
    assert_eq!(ec,           0, "empty: edge_count=0");
    assert_eq!(npentaactc,   0, "empty: NPENTAACTC=0");
    assert_eq!(nhpentaactc,  0, "empty: NHPENTAACTC=0");
    assert_eq!(nasso,        0, "empty: NASSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           1, "single: node_count=1");
    assert_eq!(ec,           0, "single: edge_count=0");
    assert_eq!(npentaactc,   0, "single: NPENTAACTC=0");
    assert_eq!(nhpentaactc,  0, "single: NHPENTAACTC=0");
    assert_eq!(nasso,        0, "single: NASSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NPENTAACTC:  1^50+1^50 = 2.
// NHPENTAACTC: (1+1)^49 = 2^49 = 562_949_953_421_312.
// NASSO:       (1²+1²)^44 = 2^44 = 17_592_186_044_416.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);
    add_edge(T76_ID_A, T76_ID_B, "t76.e.ab");

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           2,                     "k2: node_count=2");
    assert_eq!(ec,           1,                     "k2: edge_count=1");
    assert_eq!(npentaactc,   2,                     "k2: NPENTAACTC=2 (1\u{2075}\u{2070}+1\u{2075}\u{2070}=2)");
    assert_eq!(nhpentaactc,  562_949_953_421_312,   "k2: NHPENTAACTC=562_949_953_421_312 (2\u{2074}\u{2079}=2^49)");
    assert_eq!(nasso,        17_592_186_044_416,    "k2: NASSO=17_592_186_044_416 (2\u{2074}\u{2074}=2^44)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NPENTAACTC:  3×2^50 = 3×1_125_899_906_842_624 = 3_377_699_720_527_872.
// NHPENTAACTC: 2×(2+2)^49 = 2×4^49 = 2×2^98 → SATURATES.
// NASSO:       2×(4+4)^44 = 2×8^44 = 2×2^132 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);
    add_node(T76_VEC_C, T76_KEY_C, T76_ID_C);
    add_edge(T76_ID_A, T76_ID_B, "t76.e.ab");
    add_edge(T76_ID_B, T76_ID_C, "t76.e.bc");

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           3,                       "p3: node_count=3");
    assert_eq!(ec,           2,                       "p3: edge_count=2");
    assert_eq!(npentaactc,   3_377_699_720_527_872,   "p3: NPENTAACTC=3_377_699_720_527_872 (3\u{00d7}2\u{2075}\u{2070})");
    assert_eq!(nhpentaactc,  u64::MAX,                "p3: NHPENTAACTC=SAT (4\u{2074}\u{2079}>u64)");
    assert_eq!(nasso,        u64::MAX,                "p3: NASSO=SAT (8\u{2074}\u{2074}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);
    add_node(T76_VEC_C, T76_KEY_C, T76_ID_C);
    add_edge(T76_ID_A, T76_ID_B, "t76.e.ab");
    add_edge(T76_ID_B, T76_ID_C, "t76.e.bc");
    add_edge(T76_ID_C, T76_ID_A, "t76.e.ca");

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           3,        "k3: node_count=3");
    assert_eq!(ec,           3,        "k3: edge_count=3");
    assert_eq!(npentaactc,   u64::MAX, "k3: NPENTAACTC=SAT");
    assert_eq!(nhpentaactc,  u64::MAX, "k3: NHPENTAACTC=SAT");
    assert_eq!(nasso,        u64::MAX, "k3: NASSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A); // hub
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);
    add_node(T76_VEC_C, T76_KEY_C, T76_ID_C);
    add_node(T76_VEC_D, T76_KEY_D, T76_ID_D);
    add_node(T76_VEC_E, T76_KEY_E, T76_ID_E);
    add_edge(T76_ID_A, T76_ID_B, "t76.e.ab");
    add_edge(T76_ID_A, T76_ID_C, "t76.e.ac");
    add_edge(T76_ID_A, T76_ID_D, "t76.e.ad");
    add_edge(T76_ID_A, T76_ID_E, "t76.e.ae");

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           5,        "k14: node_count=5");
    assert_eq!(ec,           4,        "k14: edge_count=4");
    assert_eq!(npentaactc,   u64::MAX, "k14: NPENTAACTC=SAT");
    assert_eq!(nhpentaactc,  u64::MAX, "k14: NHPENTAACTC=SAT");
    assert_eq!(nasso,        u64::MAX, "k14: NASSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NPENTAACTC: 2×2^50 + 2×3^50.  3^41>u64::MAX → SATURATES.
// NHPENTAACTC: 5^49+6^49+5^49 → SATURATES.
// NASSO: 13^44+18^44+13^44 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);
    add_node(T76_VEC_C, T76_KEY_C, T76_ID_C);
    add_node(T76_VEC_D, T76_KEY_D, T76_ID_D);
    add_edge(T76_ID_A, T76_ID_B, "t76.e.ab");
    add_edge(T76_ID_B, T76_ID_C, "t76.e.bc");
    add_edge(T76_ID_C, T76_ID_D, "t76.e.cd");

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           4,        "p4: node_count=4");
    assert_eq!(ec,           3,        "p4: edge_count=3");
    assert_eq!(npentaactc,   u64::MAX, "p4: NPENTAACTC=SAT");
    assert_eq!(nhpentaactc,  u64::MAX, "p4: NHPENTAACTC=SAT");
    assert_eq!(nasso,        u64::MAX, "p4: NASSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);
    add_node(T76_VEC_C, T76_KEY_C, T76_ID_C);
    add_node(T76_VEC_D, T76_KEY_D, T76_ID_D);
    add_edge(T76_ID_A, T76_ID_B, "t76.e.ab");
    add_edge(T76_ID_A, T76_ID_C, "t76.e.ac");
    add_edge(T76_ID_A, T76_ID_D, "t76.e.ad");
    add_edge(T76_ID_B, T76_ID_C, "t76.e.bc");
    add_edge(T76_ID_B, T76_ID_D, "t76.e.bd");
    add_edge(T76_ID_C, T76_ID_D, "t76.e.cd");

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           4,        "k4: node_count=4");
    assert_eq!(ec,           6,        "k4: edge_count=6");
    assert_eq!(npentaactc,   u64::MAX, "k4: NPENTAACTC=SAT");
    assert_eq!(nhpentaactc,  u64::MAX, "k4: NHPENTAACTC=SAT");
    assert_eq!(nasso,        u64::MAX, "k4: NASSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           2, "isolated: node_count=2");
    assert_eq!(ec,           0, "isolated: edge_count=0");
    assert_eq!(npentaactc,   0, "isolated: NPENTAACTC=0");
    assert_eq!(nhpentaactc,  0, "isolated: NHPENTAACTC=0");
    assert_eq!(nasso,        0, "isolated: NASSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NPENTAACTC=5×6^50 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T76_VEC_A, T76_KEY_A, T76_ID_A);
    add_node(T76_VEC_B, T76_KEY_B, T76_ID_B);
    add_node(T76_VEC_C, T76_KEY_C, T76_ID_C);
    add_node(T76_VEC_D, T76_KEY_D, T76_ID_D);
    add_node(T76_VEC_E, T76_KEY_E, T76_ID_E);
    add_edge(T76_ID_A, T76_ID_C, "t76.e.ac");
    add_edge(T76_ID_A, T76_ID_D, "t76.e.ad");
    add_edge(T76_ID_A, T76_ID_E, "t76.e.ae");
    add_edge(T76_ID_B, T76_ID_C, "t76.e.bc");
    add_edge(T76_ID_B, T76_ID_D, "t76.e.bd");
    add_edge(T76_ID_B, T76_ID_E, "t76.e.be");

    let (npentaactc, nhpentaactc, nasso, ec, nc) = gos_runtime::graph_topo_indices76();
    assert_eq!(nc,           5,        "k23: node_count=5");
    assert_eq!(ec,           6,        "k23: edge_count=6");
    assert_eq!(npentaactc,   u64::MAX, "k23: NPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{2070})");
    assert_eq!(nhpentaactc,  u64::MAX, "k23: NHPENTAACTC=SAT");
    assert_eq!(nasso,        u64::MAX, "k23: NASSO=SAT");
}
