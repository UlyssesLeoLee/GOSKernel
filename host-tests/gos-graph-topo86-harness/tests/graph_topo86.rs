// gos-graph-topo86-harness — V3.97 NHEXAACTC + NHHEXAACTC + NBCSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices86()`:
//   Returns (nhexaactc, nhhexaactc, nbcso, edge_count, node_count)
//   - nhexaactc  = NHEXAACTC(G)  = Σ_v S(v)^60                   (exact u64; S-Hexacontic vertex sum)
//   - nhhexaactc = NHHEXAACTC(G) = Σ_{uv∈E} (S_u+S_v)^59         (exact u64; S-Nonapentacontic edge-sum)
//   - nbcso      = NBCSO(G)      = Σ_{uv∈E} (S_u²+S_v²)^54       (exact u64; S-Variant Sombor, α=108)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEXAACTC(G) = Σ_v S(v)^60
//     S-Hexacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NNONAPENTAACTC=Σ S⁵⁹ (topo85), NHEXAACTC=Σ S⁶⁰ (topo86).
//     First of the hexacontic (60-69) series.
//     NHEXAACTC = n·S^60 for S-regular.
//     Overflow: S^60 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^60 = s32 × s16 × s8 × s4  (60=32+16+8+4; 4 mults — efficient!).
//
//   NHHEXAACTC(G) = Σ_{uv∈E} (S_u+S_v)^59
//     S-Nonapentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHNONAPENTAACTC=Σ(S+S)⁵⁸ (topo85),
//       NHHEXAACTC=Σ(S+S)⁵⁹ (topo86).
//     NHHEXAACTC = |E|·(2S)^59 = 576460752303423488|E|·S^59 for S-regular.
//     Overflow per edge: (2×16129)^59 → saturating u128 accumulator.
//     Implementation: ss^59 = ss32 × ss16 × ss8 × ss2 × ss  (59=32+16+8+2+1; 5 mults).
//
//   NBCSO(G) = Σ_{uv∈E} (S_u²+S_v²)^54
//     S-Variant Sombor: generalised Sombor SO^α with α=108 on S-variant.
//     3rd of NB series, letter C (after NBASO α=104 topo84, NBBSO α=106 topo85).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NBBSO(topo85,α=106), NBCSO(topo86,α=108).
//     NBCSO = |E|·(2S²)^54 = 18014398509481984|E|·S^108 for S-regular.
//     Overflow per edge: (2×16129²)^54 → saturating u128 accumulator.
//     Implementation: s2s^54 = s2s32 × s2s16 × s2s4 × s2s2  (54=32+16+4+2; 4 mults).
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
//  Graph     NHEXAACTC(exact)                 NHHEXAACTC(exact)           NBCSO(exact)              edges  nodes
//  Empty                       0                             0                      0                0      0
//  1 node                      0                             0                      0                0      1
//  K₂                          2               576_460_752_303_423_488   18_014_398_509_481_984          1      2
//  P₃     3_458_764_513_820_540_928              u64::MAX(sat.)             u64::MAX(sat.)            2      3
//  K₃              u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            3      3
//  K_{1,4}         u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            4      5
//  P₄              u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            3      4
//  K₄              u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            6      4
//  2 isolated                  0                             0                      0                0      2
//  K_{2,3}         u64::MAX(sat.)                u64::MAX(sat.)             u64::MAX(sat.)            6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NHEXAACTC:  1^60 + 1^60 = 2. ✓
//     NHHEXAACTC: (1+1)^59 = 2^59 = 576_460_752_303_423_488. ✓
//     NBCSO:      (1²+1²)^54 = 2^54 = 18_014_398_509_481_984. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEXAACTC:  3×2^60 = 3×1_152_921_504_606_846_976 = 3_458_764_513_820_540_928. ✓
//     NHHEXAACTC: 2×(2+2)^59 = 2×4^59 = 2×2^118 → SATURATES. ✓
//     NBCSO:      2×(4+4)^54 = 2×8^54 = 2×2^162 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEXAACTC:  3×4^60 = 3×2^120 → SATURATES. ✓
//     NHHEXAACTC: 3×8^59 → SATURATES. ✓
//     NBCSO:      3×32^54 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEXAACTC:  5×4^60 → SATURATES. ✓
//     NHHEXAACTC: 4×8^59 → SATURATES. ✓
//     NBCSO:      4×32^54 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEXAACTC:  2×2^60 + 2×3^60.  3^38>u64::MAX → 3^60 >> u64::MAX → SATURATES. ✓
//     NHHEXAACTC: 5^59+6^59+5^59 → each term >> u64::MAX → SATURATES. ✓
//     NBCSO:      13^54+18^54+13^54 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEXAACTC:  4×9^60 → SATURATES. ✓
//     NHHEXAACTC: 6×18^59 → SATURATES. ✓
//     NBCSO:      6×162^54 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEXAACTC:  5×6^60 → SATURATES. ✓
//     NHHEXAACTC: 6×12^59 → SATURATES. ✓
//     NBCSO:      6×72^54 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEXAACTC  = n·S^60                                                                              for S-regular ✓
//   NHHEXAACTC = |E|·(2S)^59 = 576460752303423488|E|·S^59                                           for S-regular ✓
//   NBCSO      = |E|·(2S²)^54 = 18014398509481984|E|·S^108                                          for S-regular ✓
//   Note: s^60=s32×s16×s8×s4 is efficient (60=32+16+8+4, four powers of 2, only 4 mults)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 576_460_752_303_423_488, 18_014_398_509_481_984, 1, 2)
//  4.  Path P₃ = A-B-C                   → (3_458_764_513_820_540_928, u64::MAX, u64::MAX, 2, 3)
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

const T86_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_86");
const T86_EXEC:   ExecutorId = ExecutorId::from_ascii("t86.exec");

const T86_KEY_A: &str = "t86.alpha";
const T86_KEY_B: &str = "t86.beta";
const T86_KEY_C: &str = "t86.gamma";
const T86_KEY_D: &str = "t86.delta";
const T86_KEY_E: &str = "t86.epsilon";

const T86_ID_A: NodeId = derive_node_id(T86_PLUGIN, T86_KEY_A);
const T86_ID_B: NodeId = derive_node_id(T86_PLUGIN, T86_KEY_B);
const T86_ID_C: NodeId = derive_node_id(T86_PLUGIN, T86_KEY_C);
const T86_ID_D: NodeId = derive_node_id(T86_PLUGIN, T86_KEY_D);
const T86_ID_E: NodeId = derive_node_id(T86_PLUGIN, T86_KEY_E);

// L4=173 namespace for this harness.
const T86_VEC_A: VectorAddress = VectorAddress::new(173, 1, 1, 0);
const T86_VEC_B: VectorAddress = VectorAddress::new(173, 1, 2, 0);
const T86_VEC_C: VectorAddress = VectorAddress::new(173, 1, 3, 0);
const T86_VEC_D: VectorAddress = VectorAddress::new(173, 2, 1, 0);
const T86_VEC_E: VectorAddress = VectorAddress::new(173, 2, 2, 0);

const T86_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T86_PLUGIN,
    name:         "kl-graph-topo86-harness",
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
        executor_id:       T86_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T86_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T86_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,          0, "empty: node_count=0");
    assert_eq!(ec,          0, "empty: edge_count=0");
    assert_eq!(nhexaactc,   0, "empty: NHEXAACTC=0");
    assert_eq!(nhhexaactc,  0, "empty: NHHEXAACTC=0");
    assert_eq!(nbcso,       0, "empty: NBCSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,          1, "single: node_count=1");
    assert_eq!(ec,          0, "single: edge_count=0");
    assert_eq!(nhexaactc,   0, "single: NHEXAACTC=0");
    assert_eq!(nhhexaactc,  0, "single: NHHEXAACTC=0");
    assert_eq!(nbcso,       0, "single: NBCSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEXAACTC:  1^60+1^60 = 2.
// NHHEXAACTC: (1+1)^59 = 2^59 = 576_460_752_303_423_488.
// NBCSO:      (1²+1²)^54 = 2^54 = 18_014_398_509_481_984.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);
    add_edge(T86_ID_A, T86_ID_B, "t86.e.ab");

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,         2,                          "k2: node_count=2");
    assert_eq!(ec,         1,                          "k2: edge_count=1");
    assert_eq!(nhexaactc,  2,                          "k2: NHEXAACTC=2 (1\u{2076}\u{2070}+1\u{2076}\u{2070}=2)");
    assert_eq!(nhhexaactc, 576_460_752_303_423_488,    "k2: NHHEXAACTC=576_460_752_303_423_488 (2\u{2075}\u{2079}=2^59)");
    assert_eq!(nbcso,      18_014_398_509_481_984,     "k2: NBCSO=18_014_398_509_481_984 (2\u{2075}\u{2074}=2^54)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHEXAACTC:  3×2^60 = 3×1_152_921_504_606_846_976 = 3_458_764_513_820_540_928.
// NHHEXAACTC: 2×(2+2)^59 = 2×4^59 = 2×2^118 → SATURATES.
// NBCSO:      2×(4+4)^54 = 2×8^54 = 2×2^162 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);
    add_node(T86_VEC_C, T86_KEY_C, T86_ID_C);
    add_edge(T86_ID_A, T86_ID_B, "t86.e.ab");
    add_edge(T86_ID_B, T86_ID_C, "t86.e.bc");

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,         3,                          "p3: node_count=3");
    assert_eq!(ec,         2,                          "p3: edge_count=2");
    assert_eq!(nhexaactc,  3_458_764_513_820_540_928,  "p3: NHEXAACTC=3_458_764_513_820_540_928 (3\u{00d7}2\u{2076}\u{2070})");
    assert_eq!(nhhexaactc, u64::MAX,                   "p3: NHHEXAACTC=SAT (4\u{2075}\u{2079}>u64)");
    assert_eq!(nbcso,      u64::MAX,                   "p3: NBCSO=SAT (8\u{2075}\u{2074}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);
    add_node(T86_VEC_C, T86_KEY_C, T86_ID_C);
    add_edge(T86_ID_A, T86_ID_B, "t86.e.ab");
    add_edge(T86_ID_B, T86_ID_C, "t86.e.bc");
    add_edge(T86_ID_C, T86_ID_A, "t86.e.ca");

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,        3,        "k3: node_count=3");
    assert_eq!(ec,        3,        "k3: edge_count=3");
    assert_eq!(nhexaactc, u64::MAX, "k3: NHEXAACTC=SAT");
    assert_eq!(nhhexaactc,u64::MAX, "k3: NHHEXAACTC=SAT");
    assert_eq!(nbcso,     u64::MAX, "k3: NBCSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A); // hub
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);
    add_node(T86_VEC_C, T86_KEY_C, T86_ID_C);
    add_node(T86_VEC_D, T86_KEY_D, T86_ID_D);
    add_node(T86_VEC_E, T86_KEY_E, T86_ID_E);
    add_edge(T86_ID_A, T86_ID_B, "t86.e.ab");
    add_edge(T86_ID_A, T86_ID_C, "t86.e.ac");
    add_edge(T86_ID_A, T86_ID_D, "t86.e.ad");
    add_edge(T86_ID_A, T86_ID_E, "t86.e.ae");

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,        5,        "k14: node_count=5");
    assert_eq!(ec,        4,        "k14: edge_count=4");
    assert_eq!(nhexaactc, u64::MAX, "k14: NHEXAACTC=SAT");
    assert_eq!(nhhexaactc,u64::MAX, "k14: NHHEXAACTC=SAT");
    assert_eq!(nbcso,     u64::MAX, "k14: NBCSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NHEXAACTC:  2×2^60 + 2×3^60.  3^38>u64::MAX → SATURATES.
// NHHEXAACTC: 5^59+6^59+5^59 → SATURATES.
// NBCSO:      13^54+18^54+13^54 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);
    add_node(T86_VEC_C, T86_KEY_C, T86_ID_C);
    add_node(T86_VEC_D, T86_KEY_D, T86_ID_D);
    add_edge(T86_ID_A, T86_ID_B, "t86.e.ab");
    add_edge(T86_ID_B, T86_ID_C, "t86.e.bc");
    add_edge(T86_ID_C, T86_ID_D, "t86.e.cd");

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,        4,        "p4: node_count=4");
    assert_eq!(ec,        3,        "p4: edge_count=3");
    assert_eq!(nhexaactc, u64::MAX, "p4: NHEXAACTC=SAT");
    assert_eq!(nhhexaactc,u64::MAX, "p4: NHHEXAACTC=SAT");
    assert_eq!(nbcso,     u64::MAX, "p4: NBCSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);
    add_node(T86_VEC_C, T86_KEY_C, T86_ID_C);
    add_node(T86_VEC_D, T86_KEY_D, T86_ID_D);
    add_edge(T86_ID_A, T86_ID_B, "t86.e.ab");
    add_edge(T86_ID_A, T86_ID_C, "t86.e.ac");
    add_edge(T86_ID_A, T86_ID_D, "t86.e.ad");
    add_edge(T86_ID_B, T86_ID_C, "t86.e.bc");
    add_edge(T86_ID_B, T86_ID_D, "t86.e.bd");
    add_edge(T86_ID_C, T86_ID_D, "t86.e.cd");

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,        4,        "k4: node_count=4");
    assert_eq!(ec,        6,        "k4: edge_count=6");
    assert_eq!(nhexaactc, u64::MAX, "k4: NHEXAACTC=SAT");
    assert_eq!(nhhexaactc,u64::MAX, "k4: NHHEXAACTC=SAT");
    assert_eq!(nbcso,     u64::MAX, "k4: NBCSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,         2, "isolated: node_count=2");
    assert_eq!(ec,         0, "isolated: edge_count=0");
    assert_eq!(nhexaactc,  0, "isolated: NHEXAACTC=0");
    assert_eq!(nhhexaactc, 0, "isolated: NHHEXAACTC=0");
    assert_eq!(nbcso,      0, "isolated: NBCSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NHEXAACTC=5×6^60 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T86_VEC_A, T86_KEY_A, T86_ID_A);
    add_node(T86_VEC_B, T86_KEY_B, T86_ID_B);
    add_node(T86_VEC_C, T86_KEY_C, T86_ID_C);
    add_node(T86_VEC_D, T86_KEY_D, T86_ID_D);
    add_node(T86_VEC_E, T86_KEY_E, T86_ID_E);
    add_edge(T86_ID_A, T86_ID_C, "t86.e.ac");
    add_edge(T86_ID_A, T86_ID_D, "t86.e.ad");
    add_edge(T86_ID_A, T86_ID_E, "t86.e.ae");
    add_edge(T86_ID_B, T86_ID_C, "t86.e.bc");
    add_edge(T86_ID_B, T86_ID_D, "t86.e.bd");
    add_edge(T86_ID_B, T86_ID_E, "t86.e.be");

    let (nhexaactc, nhhexaactc, nbcso, ec, nc) = gos_runtime::graph_topo_indices86();
    assert_eq!(nc,        5,        "k23: node_count=5");
    assert_eq!(ec,        6,        "k23: edge_count=6");
    assert_eq!(nhexaactc, u64::MAX, "k23: NHEXAACTC=SAT (5\u{00d7}6\u{2076}\u{2070})");
    assert_eq!(nhhexaactc,u64::MAX, "k23: NHHEXAACTC=SAT");
    assert_eq!(nbcso,     u64::MAX, "k23: NBCSO=SAT");
}
