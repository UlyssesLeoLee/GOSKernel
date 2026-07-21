// gos-graph-topo81-harness — V3.92 NPENTAPENTAACTC + NHPENTAPENTAACTC + NAXSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices81()`:
//   Returns (npentapentaactc, nhpentapentaactc, naxso, edge_count, node_count)
//   - npentapentaactc  = NPENTAPENTAACTC(G)  = Σ_v S(v)^55                   (exact u64; S-Pentapentacontic vertex sum)
//   - nhpentapentaactc = NHPENTAPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^54         (exact u64; S-Tetrapentacontic edge-sum)
//   - naxso            = NAXSO(G)            = Σ_{uv∈E} (S_u²+S_v²)^49       (exact u64; S-Variant Sombor, α=98)
//   - edge_count       = undirected non-self-loop edges
//   - node_count       = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NPENTAPENTAACTC(G) = Σ_v S(v)^55
//     S-Pentapentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NTETRAPENTAACTC=Σ S⁵⁴ (topo80), NPENTAPENTAACTC=Σ S⁵⁵ (topo81). Sixth of the pentacontic (50-59) series.
//     NPENTAPENTAACTC = n·S^55 for S-regular.
//     Overflow: S^55 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^55 = s32 × s16 × s4 × s2 × s  (55=32+16+4+2+1; 5 mults).
//
//   NHPENTAPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^54
//     S-Tetrapentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHTETRAPENTAACTC=Σ(S+S)⁵³ (topo80),
//       NHPENTAPENTAACTC=Σ(S+S)⁵⁴ (topo81).
//     NHPENTAPENTAACTC = |E|·(2S)^54 = 18014398509481984|E|·S^54 for S-regular.
//     Overflow per edge: (2×16129)^54 → saturating u128 accumulator.
//     Implementation: ss^54 = ss32 × ss16 × ss4 × ss2  (54=32+16+4+2; 4 mults).
//
//   NAXSO(G) = Σ_{uv∈E} (S_u²+S_v²)^49
//     S-Variant Sombor: generalised Sombor SO^α with α=98 on S-variant.
//     3rd-pass double-letter "AX" (after NAWSO α=96, topo80).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NAWSO(topo80,α=96), NAXSO(topo81,α=98).
//     NAXSO = |E|·(2S²)^49 = 562949953421312|E|·S^98 for S-regular.
//     Overflow per edge: (2×16129²)^49 → saturating u128 accumulator.
//     Implementation: s2s^49 = s2s32 × s2s16 × s2s  (49=32+16+1; 3 mults).
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
//  Graph     NPENTAPENTAACTC(exact)           NHPENTAPENTAACTC(exact)       NAXSO(exact)              edges  nodes
//  Empty                     0                               0                        0                0      0
//  1 node                    0                               0                        0                0      1
//  K₂                        2            18_014_398_509_481_984        562_949_953_421_312               1      2
//  P₃     108_086_391_056_891_904               u64::MAX(sat.)               u64::MAX(sat.)             2      3
//  K₃            u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)             3      3
//  K_{1,4}       u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)             4      5
//  P₄            u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)             3      4
//  K₄            u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)             6      4
//  2 isolated                0                               0                        0                0      2
//  K_{2,3}       u64::MAX(sat.)                u64::MAX(sat.)               u64::MAX(sat.)             6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NPENTAPENTAACTC:  1^55 + 1^55 = 2. ✓
//     NHPENTAPENTAACTC: (1+1)^54 = 2^54 = 18_014_398_509_481_984. ✓
//     NAXSO:            (1²+1²)^49 = 2^49 = 562_949_953_421_312. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NPENTAPENTAACTC:  3×2^55 = 3×36_028_797_018_963_968 = 108_086_391_056_891_904. ✓
//     NHPENTAPENTAACTC: 2×(2+2)^54 = 2×4^54 = 2×2^108 → SATURATES. ✓
//     NAXSO:            2×(4+4)^49 = 2×8^49 = 2×2^147 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NPENTAPENTAACTC:  3×4^55 = 3×2^110 → SATURATES. ✓
//     NHPENTAPENTAACTC: 3×8^54 → SATURATES. ✓
//     NAXSO:            3×32^49 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NPENTAPENTAACTC:  5×4^55 → SATURATES. ✓
//     NHPENTAPENTAACTC: 4×8^54 → SATURATES. ✓
//     NAXSO:            4×32^49 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NPENTAPENTAACTC:  2×2^55 + 2×3^55.  3^41>u64::MAX → 3^55 >> u64::MAX → SATURATES. ✓
//     NHPENTAPENTAACTC: 5^54+6^54+5^54 → each term >> u64::MAX → SATURATES. ✓
//     NAXSO:            13^49+18^49+13^49 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NPENTAPENTAACTC:  4×9^55 → SATURATES. ✓
//     NHPENTAPENTAACTC: 6×18^54 → SATURATES. ✓
//     NAXSO:            6×162^49 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NPENTAPENTAACTC:  5×6^55 → SATURATES. ✓
//     NHPENTAPENTAACTC: 6×12^54 → SATURATES. ✓
//     NAXSO:            6×72^49 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NPENTAPENTAACTC   = n·S^55                                                                for S-regular ✓
//   NHPENTAPENTAACTC  = |E|·(2S)^54 = 18014398509481984|E|·S^54                              for S-regular ✓
//   NAXSO             = |E|·(2S²)^49 = 562949953421312|E|·S^98                               for S-regular ✓
//   Note: s2s^49 = s2s32 × s2s16 × s2s  (49=32+16+1; 3 mults — efficient!)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 18_014_398_509_481_984, 562_949_953_421_312, 1, 2)
//  4.  Path P₃ = A-B-C                   → (108_086_391_056_891_904, u64::MAX, u64::MAX, 2, 3)
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

const T81_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_81");
const T81_EXEC:   ExecutorId = ExecutorId::from_ascii("t81.exec");

const T81_KEY_A: &str = "t81.alpha";
const T81_KEY_B: &str = "t81.beta";
const T81_KEY_C: &str = "t81.gamma";
const T81_KEY_D: &str = "t81.delta";
const T81_KEY_E: &str = "t81.epsilon";

const T81_ID_A: NodeId = derive_node_id(T81_PLUGIN, T81_KEY_A);
const T81_ID_B: NodeId = derive_node_id(T81_PLUGIN, T81_KEY_B);
const T81_ID_C: NodeId = derive_node_id(T81_PLUGIN, T81_KEY_C);
const T81_ID_D: NodeId = derive_node_id(T81_PLUGIN, T81_KEY_D);
const T81_ID_E: NodeId = derive_node_id(T81_PLUGIN, T81_KEY_E);

// L4=168 namespace for this harness.
const T81_VEC_A: VectorAddress = VectorAddress::new(168, 1, 1, 0);
const T81_VEC_B: VectorAddress = VectorAddress::new(168, 1, 2, 0);
const T81_VEC_C: VectorAddress = VectorAddress::new(168, 1, 3, 0);
const T81_VEC_D: VectorAddress = VectorAddress::new(168, 2, 1, 0);
const T81_VEC_E: VectorAddress = VectorAddress::new(168, 2, 2, 0);

const T81_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T81_PLUGIN,
    name:         "kl-graph-topo81-harness",
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
        executor_id:       T81_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T81_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T81_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                 0, "empty: node_count=0");
    assert_eq!(ec,                 0, "empty: edge_count=0");
    assert_eq!(npentapentaactc,    0, "empty: NPENTAPENTAACTC=0");
    assert_eq!(nhpentapentaactc,   0, "empty: NHPENTAPENTAACTC=0");
    assert_eq!(naxso,              0, "empty: NAXSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                 1, "single: node_count=1");
    assert_eq!(ec,                 0, "single: edge_count=0");
    assert_eq!(npentapentaactc,    0, "single: NPENTAPENTAACTC=0");
    assert_eq!(nhpentapentaactc,   0, "single: NHPENTAPENTAACTC=0");
    assert_eq!(naxso,              0, "single: NAXSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NPENTAPENTAACTC:  1^55+1^55 = 2.
// NHPENTAPENTAACTC: (1+1)^54 = 2^54 = 18_014_398_509_481_984.
// NAXSO:            (1²+1²)^49 = 2^49 = 562_949_953_421_312.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);
    add_edge(T81_ID_A, T81_ID_B, "t81.e.ab");

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                 2,                         "k2: node_count=2");
    assert_eq!(ec,                 1,                         "k2: edge_count=1");
    assert_eq!(npentapentaactc,    2,                         "k2: NPENTAPENTAACTC=2 (1\u{2075}\u{2075}+1\u{2075}\u{2075}=2)");
    assert_eq!(nhpentapentaactc,   18_014_398_509_481_984,    "k2: NHPENTAPENTAACTC=18_014_398_509_481_984 (2\u{2075}\u{2074}=2^54)");
    assert_eq!(naxso,              562_949_953_421_312,       "k2: NAXSO=562_949_953_421_312 (2\u{2074}\u{2079}=2^49)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NPENTAPENTAACTC:  3×2^55 = 3×36_028_797_018_963_968 = 108_086_391_056_891_904.
// NHPENTAPENTAACTC: 2×(2+2)^54 = 2×4^54 = 2×2^108 → SATURATES.
// NAXSO:            2×(4+4)^49 = 2×8^49 = 2×2^147 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);
    add_node(T81_VEC_C, T81_KEY_C, T81_ID_C);
    add_edge(T81_ID_A, T81_ID_B, "t81.e.ab");
    add_edge(T81_ID_B, T81_ID_C, "t81.e.bc");

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                 3,                            "p3: node_count=3");
    assert_eq!(ec,                 2,                            "p3: edge_count=2");
    assert_eq!(npentapentaactc,    108_086_391_056_891_904,      "p3: NPENTAPENTAACTC=108_086_391_056_891_904 (3\u{00d7}2\u{2075}\u{2075})");
    assert_eq!(nhpentapentaactc,   u64::MAX,                     "p3: NHPENTAPENTAACTC=SAT (4\u{2075}\u{2074}>u64)");
    assert_eq!(naxso,              u64::MAX,                     "p3: NAXSO=SAT (8\u{2074}\u{2079}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);
    add_node(T81_VEC_C, T81_KEY_C, T81_ID_C);
    add_edge(T81_ID_A, T81_ID_B, "t81.e.ab");
    add_edge(T81_ID_B, T81_ID_C, "t81.e.bc");
    add_edge(T81_ID_C, T81_ID_A, "t81.e.ca");

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                3,        "k3: node_count=3");
    assert_eq!(ec,                3,        "k3: edge_count=3");
    assert_eq!(npentapentaactc,   u64::MAX, "k3: NPENTAPENTAACTC=SAT");
    assert_eq!(nhpentapentaactc,  u64::MAX, "k3: NHPENTAPENTAACTC=SAT");
    assert_eq!(naxso,             u64::MAX, "k3: NAXSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A); // hub
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);
    add_node(T81_VEC_C, T81_KEY_C, T81_ID_C);
    add_node(T81_VEC_D, T81_KEY_D, T81_ID_D);
    add_node(T81_VEC_E, T81_KEY_E, T81_ID_E);
    add_edge(T81_ID_A, T81_ID_B, "t81.e.ab");
    add_edge(T81_ID_A, T81_ID_C, "t81.e.ac");
    add_edge(T81_ID_A, T81_ID_D, "t81.e.ad");
    add_edge(T81_ID_A, T81_ID_E, "t81.e.ae");

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                5,        "k14: node_count=5");
    assert_eq!(ec,                4,        "k14: edge_count=4");
    assert_eq!(npentapentaactc,   u64::MAX, "k14: NPENTAPENTAACTC=SAT");
    assert_eq!(nhpentapentaactc,  u64::MAX, "k14: NHPENTAPENTAACTC=SAT");
    assert_eq!(naxso,             u64::MAX, "k14: NAXSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NPENTAPENTAACTC:  2×2^55 + 2×3^55.  3^41>u64::MAX → SATURATES.
// NHPENTAPENTAACTC: 5^54+6^54+5^54 → SATURATES.
// NAXSO:            13^49+18^49+13^49 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);
    add_node(T81_VEC_C, T81_KEY_C, T81_ID_C);
    add_node(T81_VEC_D, T81_KEY_D, T81_ID_D);
    add_edge(T81_ID_A, T81_ID_B, "t81.e.ab");
    add_edge(T81_ID_B, T81_ID_C, "t81.e.bc");
    add_edge(T81_ID_C, T81_ID_D, "t81.e.cd");

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                4,        "p4: node_count=4");
    assert_eq!(ec,                3,        "p4: edge_count=3");
    assert_eq!(npentapentaactc,   u64::MAX, "p4: NPENTAPENTAACTC=SAT");
    assert_eq!(nhpentapentaactc,  u64::MAX, "p4: NHPENTAPENTAACTC=SAT");
    assert_eq!(naxso,             u64::MAX, "p4: NAXSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);
    add_node(T81_VEC_C, T81_KEY_C, T81_ID_C);
    add_node(T81_VEC_D, T81_KEY_D, T81_ID_D);
    add_edge(T81_ID_A, T81_ID_B, "t81.e.ab");
    add_edge(T81_ID_A, T81_ID_C, "t81.e.ac");
    add_edge(T81_ID_A, T81_ID_D, "t81.e.ad");
    add_edge(T81_ID_B, T81_ID_C, "t81.e.bc");
    add_edge(T81_ID_B, T81_ID_D, "t81.e.bd");
    add_edge(T81_ID_C, T81_ID_D, "t81.e.cd");

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                4,        "k4: node_count=4");
    assert_eq!(ec,                6,        "k4: edge_count=6");
    assert_eq!(npentapentaactc,   u64::MAX, "k4: NPENTAPENTAACTC=SAT");
    assert_eq!(nhpentapentaactc,  u64::MAX, "k4: NHPENTAPENTAACTC=SAT");
    assert_eq!(naxso,             u64::MAX, "k4: NAXSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                 2, "isolated: node_count=2");
    assert_eq!(ec,                 0, "isolated: edge_count=0");
    assert_eq!(npentapentaactc,    0, "isolated: NPENTAPENTAACTC=0");
    assert_eq!(nhpentapentaactc,   0, "isolated: NHPENTAPENTAACTC=0");
    assert_eq!(naxso,              0, "isolated: NAXSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NPENTAPENTAACTC=5×6^55 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T81_VEC_A, T81_KEY_A, T81_ID_A);
    add_node(T81_VEC_B, T81_KEY_B, T81_ID_B);
    add_node(T81_VEC_C, T81_KEY_C, T81_ID_C);
    add_node(T81_VEC_D, T81_KEY_D, T81_ID_D);
    add_node(T81_VEC_E, T81_KEY_E, T81_ID_E);
    add_edge(T81_ID_A, T81_ID_C, "t81.e.ac");
    add_edge(T81_ID_A, T81_ID_D, "t81.e.ad");
    add_edge(T81_ID_A, T81_ID_E, "t81.e.ae");
    add_edge(T81_ID_B, T81_ID_C, "t81.e.bc");
    add_edge(T81_ID_B, T81_ID_D, "t81.e.bd");
    add_edge(T81_ID_B, T81_ID_E, "t81.e.be");

    let (npentapentaactc, nhpentapentaactc, naxso, ec, nc) = gos_runtime::graph_topo_indices81();
    assert_eq!(nc,                5,        "k23: node_count=5");
    assert_eq!(ec,                6,        "k23: edge_count=6");
    assert_eq!(npentapentaactc,   u64::MAX, "k23: NPENTAPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{2075})");
    assert_eq!(nhpentapentaactc,  u64::MAX, "k23: NHPENTAPENTAACTC=SAT");
    assert_eq!(naxso,             u64::MAX, "k23: NAXSO=SAT");
}
