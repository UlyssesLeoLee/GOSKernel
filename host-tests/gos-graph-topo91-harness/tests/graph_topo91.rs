// gos-graph-topo91-harness — V3.102 NHEXAPENTACTC + NHHEXAPENTACTC + NBHSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices91()`:
//   Returns (nhexapentactc, nhhexapentactc, nbhso, edge_count, node_count)
//   - nhexapentactc  = NHEXAPENTACTC(G) = Σ_v S(v)^65                    (exact u64; S-Hexapentacontic vertex sum)
//   - nhhexapentactc = NHHEXAPENTACTC(G) = Σ_{uv∈E} (S_u+S_v)^64         (exact u64; S-Hexapentacontic edge-sum)
//   - nbhso          = NBHSO(G)          = Σ_{uv∈E} (S_u²+S_v²)^59       (exact u64; S-Variant Sombor, α=118)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXAPENTACTC(G) = Σ_v S(v)^65
//     S-Hexapentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXATETRAACTC=Σ S⁶⁴ (topo90), NHEXAPENTACTC=Σ S⁶⁵ (topo91).
//     Sixth of the hexacontic (60-69) series.
//     NHEXAPENTACTC = n·S^65 for S-regular.
//     Overflow: S^65 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^65 = s64 × s  (65=64+1; s64=s32×s32; 7 mults total).
//
//   NHHEXAPENTACTC(G) = Σ_{uv∈E} (S_u+S_v)^64
//     S-Hexapentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXATETRAACTC=Σ(S+S)⁶³ (topo90),
//       NHHEXAPENTACTC=Σ(S+S)⁶⁴ (topo91).
//     NHHEXAPENTACTC = |E|·(2S)^64 = 18446744073709551616|E|·S^64 for S-regular
//       (saturates for |E|≥1, S≥1 since 2^64 > u64::MAX).
//     Overflow per edge: (2×16129)^64 → saturating u128 accumulator.
//     Implementation: ss^64 = ss32 × ss32  (64=32+32; 6 squarings; EFFICIENT — all powers of 2).
//
//   NBHSO(G) = Σ_{uv∈E} (S_u²+S_v²)^59
//     S-Variant Sombor: generalised Sombor SO^α with α=118 on S-variant.
//     8th of NB series, letter H (after NBGSO α=116 topo90).
//     NSO(topo21,α=1),..., NBGSO(topo90,α=116), NBHSO(topo91,α=118).
//     NBHSO = |E|·(2S²)^59 = 576460752303423488|E|·S^118 for S-regular.
//     Overflow per edge: (2×16129²)^59 → saturating u128 accumulator.
//     Implementation: s2s^59 = s2s32 × s2s16 × s2s8 × s2s2 × s2s  (59=32+16+8+2+1; 5 mults).
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
//  Graph     NHEXAPENTACTC(exact)         NHHEXAPENTACTC(exact)        NBHSO(exact)            edges  nodes
//  Empty                      0                             0                   0                0      0
//  1 node                     0                             0                   0                0      1
//  K₂                         2              u64::MAX(sat.)   576_460_752_303_423_488            1      2
//  P₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)            2      3
//  K₃              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)            3      3
//  K_{1,4}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)            4      5
//  P₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)            3      4
//  K₄              u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)            6      4
//  2 isolated                 0                             0                   0                0      2
//  K_{2,3}         u64::MAX(sat.)            u64::MAX(sat.)            u64::MAX(sat.)            6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXAPENTACTC:  1^65 + 1^65 = 2. ✓
//     NHHEXAPENTACTC: (1+1)^64 = 2^64 = 18_446_744_073_709_551_616 > u64::MAX → SATURATES. ✓
//     NBHSO:          (1²+1²)^59 = 2^59 = 576_460_752_303_423_488. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXAPENTACTC:  3×2^65 = 3×2^65 > u64::MAX → SATURATES. ✓
//     NHHEXAPENTACTC: 2×(2+2)^64 = 2×4^64 → SATURATES. ✓
//     NBHSO:          2×(4+4)^59 = 2×8^59 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXAPENTACTC:  3×4^65 → SATURATES. ✓
//     NHHEXAPENTACTC: 3×8^64 → SATURATES. ✓
//     NBHSO:          3×32^59 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXAPENTACTC:  5×4^65 → SATURATES. ✓
//     NHHEXAPENTACTC: 4×8^64 → SATURATES. ✓
//     NBHSO:          4×32^59 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXAPENTACTC:  2×2^65 + 2×3^65. 3^65 >> u64::MAX → SATURATES. ✓
//     NHHEXAPENTACTC: 5^64+6^64+5^64 → SATURATES. ✓
//     NBHSO:          13^59+18^59+13^59 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXAPENTACTC:  4×9^65 → SATURATES. ✓
//     NHHEXAPENTACTC: 6×18^64 → SATURATES. ✓
//     NBHSO:          6×162^59 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXAPENTACTC:  5×6^65 → SATURATES. ✓
//     NHHEXAPENTACTC: 6×12^64 → SATURATES. ✓
//     NBHSO:          6×72^59 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXAPENTACTC  = n·S^65                                                                              for S-regular ✓
//   NHHEXAPENTACTC = |E|·(2S)^64 = 18446744073709551616|E|·S^64 (saturates for |E|≥1,S≥1)              for S-regular ✓
//   NBHSO          = |E|·(2S²)^59 = 576460752303423488|E|·S^118                                         for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, u64::MAX, 576_460_752_303_423_488, 1, 2)
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

const T91_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_91");
const T91_EXEC:   ExecutorId = ExecutorId::from_ascii("t91.exec");

const T91_KEY_A: &str = "t91.alpha";
const T91_KEY_B: &str = "t91.beta";
const T91_KEY_C: &str = "t91.gamma";
const T91_KEY_D: &str = "t91.delta";
const T91_KEY_E: &str = "t91.epsilon";

const T91_ID_A: NodeId = derive_node_id(T91_PLUGIN, T91_KEY_A);
const T91_ID_B: NodeId = derive_node_id(T91_PLUGIN, T91_KEY_B);
const T91_ID_C: NodeId = derive_node_id(T91_PLUGIN, T91_KEY_C);
const T91_ID_D: NodeId = derive_node_id(T91_PLUGIN, T91_KEY_D);
const T91_ID_E: NodeId = derive_node_id(T91_PLUGIN, T91_KEY_E);

// L4=178 namespace for this harness.
const T91_VEC_A: VectorAddress = VectorAddress::new(178, 1, 1, 0);
const T91_VEC_B: VectorAddress = VectorAddress::new(178, 1, 2, 0);
const T91_VEC_C: VectorAddress = VectorAddress::new(178, 1, 3, 0);
const T91_VEC_D: VectorAddress = VectorAddress::new(178, 2, 1, 0);
const T91_VEC_E: VectorAddress = VectorAddress::new(178, 2, 2, 0);

const T91_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T91_PLUGIN,
    name:         "kl-graph-topo91-harness",
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
        executor_id:       T91_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T91_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T91_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,              0, "empty: node_count=0");
    assert_eq!(ec,              0, "empty: edge_count=0");
    assert_eq!(nhexapentactc,   0, "empty: NHEXAPENTACTC=0");
    assert_eq!(nhhexapentactc,  0, "empty: NHHEXAPENTACTC=0");
    assert_eq!(nbhso,           0, "empty: NBHSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,              1, "single: node_count=1");
    assert_eq!(ec,              0, "single: edge_count=0");
    assert_eq!(nhexapentactc,   0, "single: NHEXAPENTACTC=0");
    assert_eq!(nhhexapentactc,  0, "single: NHHEXAPENTACTC=0");
    assert_eq!(nbhso,           0, "single: NBHSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXAPENTACTC:  1^65+1^65 = 2.
// NHHEXAPENTACTC: (1+1)^64 = 2^64 = 18_446_744_073_709_551_616 > u64::MAX → SATURATES.
// NBHSO:          (1²+1²)^59 = 2^59 = 576_460_752_303_423_488.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);
    add_edge(T91_ID_A, T91_ID_B, "t91.e.ab");

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,              2,                         "k2: node_count=2");
    assert_eq!(ec,              1,                         "k2: edge_count=1");
    assert_eq!(nhexapentactc,   2,                         "k2: NHEXAPENTACTC=2 (1^65+1^65=2)");
    assert_eq!(nhhexapentactc,  u64::MAX,                  "k2: NHHEXAPENTACTC=SAT (2^64>u64::MAX)");
    assert_eq!(nbhso,           576_460_752_303_423_488,   "k2: NBHSO=576_460_752_303_423_488 (2^59)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// S=2 uniform, 2 edges, 3 nodes. All saturate.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);
    add_node(T91_VEC_C, T91_KEY_C, T91_ID_C);
    add_edge(T91_ID_A, T91_ID_B, "t91.e.ab");
    add_edge(T91_ID_B, T91_ID_C, "t91.e.bc");

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,             3,         "p3: node_count=3");
    assert_eq!(ec,             2,         "p3: edge_count=2");
    assert_eq!(nhexapentactc,  u64::MAX,  "p3: NHEXAPENTACTC=SAT (3\u{00d7}2^65>u64)");
    assert_eq!(nhhexapentactc, u64::MAX,  "p3: NHHEXAPENTACTC=SAT (4^64>u64)");
    assert_eq!(nbhso,          u64::MAX,  "p3: NBHSO=SAT (8^59>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// S=4 uniform, 3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);
    add_node(T91_VEC_C, T91_KEY_C, T91_ID_C);
    add_edge(T91_ID_A, T91_ID_B, "t91.e.ab");
    add_edge(T91_ID_B, T91_ID_C, "t91.e.bc");
    add_edge(T91_ID_C, T91_ID_A, "t91.e.ca");

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,             3,        "k3: node_count=3");
    assert_eq!(ec,             3,        "k3: edge_count=3");
    assert_eq!(nhexapentactc,  u64::MAX, "k3: NHEXAPENTACTC=SAT");
    assert_eq!(nhhexapentactc, u64::MAX, "k3: NHHEXAPENTACTC=SAT");
    assert_eq!(nbhso,          u64::MAX, "k3: NBHSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// S=4 uniform, 4 edges, 5 nodes. All saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A); // hub
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);
    add_node(T91_VEC_C, T91_KEY_C, T91_ID_C);
    add_node(T91_VEC_D, T91_KEY_D, T91_ID_D);
    add_node(T91_VEC_E, T91_KEY_E, T91_ID_E);
    add_edge(T91_ID_A, T91_ID_B, "t91.e.ab");
    add_edge(T91_ID_A, T91_ID_C, "t91.e.ac");
    add_edge(T91_ID_A, T91_ID_D, "t91.e.ad");
    add_edge(T91_ID_A, T91_ID_E, "t91.e.ae");

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,             5,        "k14: node_count=5");
    assert_eq!(ec,             4,        "k14: edge_count=4");
    assert_eq!(nhexapentactc,  u64::MAX, "k14: NHEXAPENTACTC=SAT");
    assert_eq!(nhhexapentactc, u64::MAX, "k14: NHHEXAPENTACTC=SAT");
    assert_eq!(nbhso,          u64::MAX, "k14: NBHSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// S(A)=2, S(B)=3, S(C)=3, S(D)=2. All saturate.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);
    add_node(T91_VEC_C, T91_KEY_C, T91_ID_C);
    add_node(T91_VEC_D, T91_KEY_D, T91_ID_D);
    add_edge(T91_ID_A, T91_ID_B, "t91.e.ab");
    add_edge(T91_ID_B, T91_ID_C, "t91.e.bc");
    add_edge(T91_ID_C, T91_ID_D, "t91.e.cd");

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,             4,        "p4: node_count=4");
    assert_eq!(ec,             3,        "p4: edge_count=3");
    assert_eq!(nhexapentactc,  u64::MAX, "p4: NHEXAPENTACTC=SAT");
    assert_eq!(nhhexapentactc, u64::MAX, "p4: NHHEXAPENTACTC=SAT");
    assert_eq!(nbhso,          u64::MAX, "p4: NBHSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// S=9 uniform, 6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);
    add_node(T91_VEC_C, T91_KEY_C, T91_ID_C);
    add_node(T91_VEC_D, T91_KEY_D, T91_ID_D);
    add_edge(T91_ID_A, T91_ID_B, "t91.e.ab");
    add_edge(T91_ID_A, T91_ID_C, "t91.e.ac");
    add_edge(T91_ID_A, T91_ID_D, "t91.e.ad");
    add_edge(T91_ID_B, T91_ID_C, "t91.e.bc");
    add_edge(T91_ID_B, T91_ID_D, "t91.e.bd");
    add_edge(T91_ID_C, T91_ID_D, "t91.e.cd");

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,             4,        "k4: node_count=4");
    assert_eq!(ec,             6,        "k4: edge_count=6");
    assert_eq!(nhexapentactc,  u64::MAX, "k4: NHEXAPENTACTC=SAT");
    assert_eq!(nhhexapentactc, u64::MAX, "k4: NHHEXAPENTACTC=SAT");
    assert_eq!(nbhso,          u64::MAX, "k4: NBHSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,              2, "isolated: node_count=2");
    assert_eq!(ec,              0, "isolated: edge_count=0");
    assert_eq!(nhexapentactc,   0, "isolated: NHEXAPENTACTC=0");
    assert_eq!(nhhexapentactc,  0, "isolated: NHHEXAPENTACTC=0");
    assert_eq!(nbhso,           0, "isolated: NBHSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// S=6 uniform, 6 edges, 5 nodes. All saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T91_VEC_A, T91_KEY_A, T91_ID_A);
    add_node(T91_VEC_B, T91_KEY_B, T91_ID_B);
    add_node(T91_VEC_C, T91_KEY_C, T91_ID_C);
    add_node(T91_VEC_D, T91_KEY_D, T91_ID_D);
    add_node(T91_VEC_E, T91_KEY_E, T91_ID_E);
    add_edge(T91_ID_A, T91_ID_C, "t91.e.ac");
    add_edge(T91_ID_A, T91_ID_D, "t91.e.ad");
    add_edge(T91_ID_A, T91_ID_E, "t91.e.ae");
    add_edge(T91_ID_B, T91_ID_C, "t91.e.bc");
    add_edge(T91_ID_B, T91_ID_D, "t91.e.bd");
    add_edge(T91_ID_B, T91_ID_E, "t91.e.be");

    let (nhexapentactc, nhhexapentactc, nbhso, ec, nc) = gos_runtime::graph_topo_indices91();
    assert_eq!(nc,             5,        "k23: node_count=5");
    assert_eq!(ec,             6,        "k23: edge_count=6");
    assert_eq!(nhexapentactc,  u64::MAX, "k23: NHEXAPENTACTC=SAT (5\u{00d7}6^65)");
    assert_eq!(nhhexapentactc, u64::MAX, "k23: NHHEXAPENTACTC=SAT");
    assert_eq!(nbhso,          u64::MAX, "k23: NBHSO=SAT");
}
