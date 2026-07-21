// gos-graph-topo85-harness — V3.96 NNONAPENTAACTC + NHNONAPENTAACTC + NBBSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices85()`:
//   Returns (nnonapentaactc, nhnonapentaactc, nbbso, edge_count, node_count)
//   - nnonapentaactc  = NNONAPENTAACTC(G)  = Σ_v S(v)^59                   (exact u64; S-Nonapentacontic vertex sum)
//   - nhnonapentaactc = NHNONAPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^58         (exact u64; S-Octapentacontic edge-sum)
//   - nbbso           = NBBSO(G)           = Σ_{uv∈E} (S_u²+S_v²)^53       (exact u64; S-Variant Sombor, α=106)
//   - edge_count      = undirected non-self-loop edges
//   - node_count      = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NNONAPENTAACTC(G) = Σ_v S(v)^59
//     S-Nonapentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NOCTOPENTAACTC=Σ S⁵⁸ (topo84), NNONAPENTAACTC=Σ S⁵⁹ (topo85).
//     Tenth and last of the pentacontic (50-59) series.
//     NNONAPENTAACTC = n·S^59 for S-regular.
//     Overflow: S^59 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^59 = s32 × s16 × s8 × s2 × s  (59=32+16+8+2+1; 5 mults).
//
//   NHNONAPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^58
//     S-Octapentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHOCTOPENTAACTC=Σ(S+S)⁵⁷ (topo84),
//       NHNONAPENTAACTC=Σ(S+S)⁵⁸ (topo85).
//     NHNONAPENTAACTC = |E|·(2S)^58 = 288230376151711744|E|·S^58 for S-regular.
//     Overflow per edge: (2×16129)^58 → saturating u128 accumulator.
//     Implementation: ss^58 = ss32 × ss16 × ss8 × ss2  (58=32+16+8+2; 4 mults — efficient!).
//
//   NBBSO(G) = Σ_{uv∈E} (S_u²+S_v²)^53
//     S-Variant Sombor: generalised Sombor SO^α with α=106 on S-variant.
//     4th-pass double-letter "BB" (after NBASO α=104, topo84; second of NB series).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NBASO(topo84,α=104), NBBSO(topo85,α=106).
//     NBBSO = |E|·(2S²)^53 = 9007199254740992|E|·S^106 for S-regular.
//     Overflow per edge: (2×16129²)^53 → saturating u128 accumulator.
//     Implementation: s2s^53 = s2s32 × s2s16 × s2s4 × s2s  (53=32+16+4+1; 4 mults).
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
//  Graph     NNONAPENTAACTC(exact)            NHNONAPENTAACTC(exact)      NBBSO(exact)              edges  nodes
//  Empty                       0                             0                      0                0      0
//  1 node                      0                             0                      0                0      1
//  K₂                          2           288_230_376_151_711_744    9_007_199_254_740_992              1      2
//  P₃     1_729_382_256_910_270_464              u64::MAX(sat.)             u64::MAX(sat.)            2      3
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
//     NNONAPENTAACTC:  1^59 + 1^59 = 2. ✓
//     NHNONAPENTAACTC: (1+1)^58 = 2^58 = 288_230_376_151_711_744. ✓
//     NBBSO:           (1²+1²)^53 = 2^53 = 9_007_199_254_740_992. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NNONAPENTAACTC:  3×2^59 = 3×576_460_752_303_423_488 = 1_729_382_256_910_270_464. ✓
//     NHNONAPENTAACTC: 2×(2+2)^58 = 2×4^58 = 2×2^116 → SATURATES. ✓
//     NBBSO:           2×(4+4)^53 = 2×8^53 = 2×2^159 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NNONAPENTAACTC:  3×4^59 = 3×2^118 → SATURATES. ✓
//     NHNONAPENTAACTC: 3×8^58 → SATURATES. ✓
//     NBBSO:           3×32^53 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NNONAPENTAACTC:  5×4^59 → SATURATES. ✓
//     NHNONAPENTAACTC: 4×8^58 → SATURATES. ✓
//     NBBSO:           4×32^53 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NNONAPENTAACTC:  2×2^59 + 2×3^59.  3^41>u64::MAX → 3^59 >> u64::MAX → SATURATES. ✓
//     NHNONAPENTAACTC: 5^58+6^58+5^58 → each term >> u64::MAX → SATURATES. ✓
//     NBBSO:           13^53+18^53+13^53 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NNONAPENTAACTC:  4×9^59 → SATURATES. ✓
//     NHNONAPENTAACTC: 6×18^58 → SATURATES. ✓
//     NBBSO:           6×162^53 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NNONAPENTAACTC:  5×6^59 → SATURATES. ✓
//     NHNONAPENTAACTC: 6×12^58 → SATURATES. ✓
//     NBBSO:           6×72^53 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NNONAPENTAACTC  = n·S^59                                                                             for S-regular ✓
//   NHNONAPENTAACTC = |E|·(2S)^58 = 288230376151711744|E|·S^58                                          for S-regular ✓
//   NBBSO           = |E|·(2S²)^53 = 9007199254740992|E|·S^106                                          for S-regular ✓
//   Note: ss^58 = ss32×ss16×ss8×ss2 is efficient (58=32+16+8+2, four powers of 2, only 4 mults)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 288_230_376_151_711_744, 9_007_199_254_740_992, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_729_382_256_910_270_464, u64::MAX, u64::MAX, 2, 3)
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

const T85_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_85");
const T85_EXEC:   ExecutorId = ExecutorId::from_ascii("t85.exec");

const T85_KEY_A: &str = "t85.alpha";
const T85_KEY_B: &str = "t85.beta";
const T85_KEY_C: &str = "t85.gamma";
const T85_KEY_D: &str = "t85.delta";
const T85_KEY_E: &str = "t85.epsilon";

const T85_ID_A: NodeId = derive_node_id(T85_PLUGIN, T85_KEY_A);
const T85_ID_B: NodeId = derive_node_id(T85_PLUGIN, T85_KEY_B);
const T85_ID_C: NodeId = derive_node_id(T85_PLUGIN, T85_KEY_C);
const T85_ID_D: NodeId = derive_node_id(T85_PLUGIN, T85_KEY_D);
const T85_ID_E: NodeId = derive_node_id(T85_PLUGIN, T85_KEY_E);

// L4=172 namespace for this harness.
const T85_VEC_A: VectorAddress = VectorAddress::new(172, 1, 1, 0);
const T85_VEC_B: VectorAddress = VectorAddress::new(172, 1, 2, 0);
const T85_VEC_C: VectorAddress = VectorAddress::new(172, 1, 3, 0);
const T85_VEC_D: VectorAddress = VectorAddress::new(172, 2, 1, 0);
const T85_VEC_E: VectorAddress = VectorAddress::new(172, 2, 2, 0);

const T85_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T85_PLUGIN,
    name:         "kl-graph-topo85-harness",
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
        executor_id:       T85_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T85_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T85_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                 0, "empty: node_count=0");
    assert_eq!(ec,                 0, "empty: edge_count=0");
    assert_eq!(nnonapentaactc,     0, "empty: NNONAPENTAACTC=0");
    assert_eq!(nhnonapentaactc,    0, "empty: NHNONAPENTAACTC=0");
    assert_eq!(nbbso,              0, "empty: NBBSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                 1, "single: node_count=1");
    assert_eq!(ec,                 0, "single: edge_count=0");
    assert_eq!(nnonapentaactc,     0, "single: NNONAPENTAACTC=0");
    assert_eq!(nhnonapentaactc,    0, "single: NHNONAPENTAACTC=0");
    assert_eq!(nbbso,              0, "single: NBBSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NNONAPENTAACTC:  1^59+1^59 = 2.
// NHNONAPENTAACTC: (1+1)^58 = 2^58 = 288_230_376_151_711_744.
// NBBSO:           (1²+1²)^53 = 2^53 = 9_007_199_254_740_992.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);
    add_edge(T85_ID_A, T85_ID_B, "t85.e.ab");

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                 2,                           "k2: node_count=2");
    assert_eq!(ec,                 1,                           "k2: edge_count=1");
    assert_eq!(nnonapentaactc,     2,                           "k2: NNONAPENTAACTC=2 (1\u{2075}\u{2079}+1\u{2075}\u{2079}=2)");
    assert_eq!(nhnonapentaactc,    288_230_376_151_711_744,     "k2: NHNONAPENTAACTC=288_230_376_151_711_744 (2\u{2075}\u{2078}=2^58)");
    assert_eq!(nbbso,              9_007_199_254_740_992,       "k2: NBBSO=9_007_199_254_740_992 (2\u{2075}\u{00b3}=2^53)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NNONAPENTAACTC:  3×2^59 = 3×576_460_752_303_423_488 = 1_729_382_256_910_270_464.
// NHNONAPENTAACTC: 2×(2+2)^58 = 2×4^58 = 2×2^116 → SATURATES.
// NBBSO:           2×(4+4)^53 = 2×8^53 = 2×2^159 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);
    add_node(T85_VEC_C, T85_KEY_C, T85_ID_C);
    add_edge(T85_ID_A, T85_ID_B, "t85.e.ab");
    add_edge(T85_ID_B, T85_ID_C, "t85.e.bc");

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                 3,                           "p3: node_count=3");
    assert_eq!(ec,                 2,                           "p3: edge_count=2");
    assert_eq!(nnonapentaactc,     1_729_382_256_910_270_464,   "p3: NNONAPENTAACTC=1_729_382_256_910_270_464 (3\u{00d7}2\u{2075}\u{2079})");
    assert_eq!(nhnonapentaactc,    u64::MAX,                    "p3: NHNONAPENTAACTC=SAT (4\u{2075}\u{2078}>u64)");
    assert_eq!(nbbso,              u64::MAX,                    "p3: NBBSO=SAT (8\u{2075}\u{00b3}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);
    add_node(T85_VEC_C, T85_KEY_C, T85_ID_C);
    add_edge(T85_ID_A, T85_ID_B, "t85.e.ab");
    add_edge(T85_ID_B, T85_ID_C, "t85.e.bc");
    add_edge(T85_ID_C, T85_ID_A, "t85.e.ca");

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                3,        "k3: node_count=3");
    assert_eq!(ec,                3,        "k3: edge_count=3");
    assert_eq!(nnonapentaactc,    u64::MAX, "k3: NNONAPENTAACTC=SAT");
    assert_eq!(nhnonapentaactc,   u64::MAX, "k3: NHNONAPENTAACTC=SAT");
    assert_eq!(nbbso,             u64::MAX, "k3: NBBSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A); // hub
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);
    add_node(T85_VEC_C, T85_KEY_C, T85_ID_C);
    add_node(T85_VEC_D, T85_KEY_D, T85_ID_D);
    add_node(T85_VEC_E, T85_KEY_E, T85_ID_E);
    add_edge(T85_ID_A, T85_ID_B, "t85.e.ab");
    add_edge(T85_ID_A, T85_ID_C, "t85.e.ac");
    add_edge(T85_ID_A, T85_ID_D, "t85.e.ad");
    add_edge(T85_ID_A, T85_ID_E, "t85.e.ae");

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                5,        "k14: node_count=5");
    assert_eq!(ec,                4,        "k14: edge_count=4");
    assert_eq!(nnonapentaactc,    u64::MAX, "k14: NNONAPENTAACTC=SAT");
    assert_eq!(nhnonapentaactc,   u64::MAX, "k14: NHNONAPENTAACTC=SAT");
    assert_eq!(nbbso,             u64::MAX, "k14: NBBSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NNONAPENTAACTC:  2×2^59 + 2×3^59.  3^41>u64::MAX → SATURATES.
// NHNONAPENTAACTC: 5^58+6^58+5^58 → SATURATES.
// NBBSO:           13^53+18^53+13^53 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);
    add_node(T85_VEC_C, T85_KEY_C, T85_ID_C);
    add_node(T85_VEC_D, T85_KEY_D, T85_ID_D);
    add_edge(T85_ID_A, T85_ID_B, "t85.e.ab");
    add_edge(T85_ID_B, T85_ID_C, "t85.e.bc");
    add_edge(T85_ID_C, T85_ID_D, "t85.e.cd");

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                4,        "p4: node_count=4");
    assert_eq!(ec,                3,        "p4: edge_count=3");
    assert_eq!(nnonapentaactc,    u64::MAX, "p4: NNONAPENTAACTC=SAT");
    assert_eq!(nhnonapentaactc,   u64::MAX, "p4: NHNONAPENTAACTC=SAT");
    assert_eq!(nbbso,             u64::MAX, "p4: NBBSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);
    add_node(T85_VEC_C, T85_KEY_C, T85_ID_C);
    add_node(T85_VEC_D, T85_KEY_D, T85_ID_D);
    add_edge(T85_ID_A, T85_ID_B, "t85.e.ab");
    add_edge(T85_ID_A, T85_ID_C, "t85.e.ac");
    add_edge(T85_ID_A, T85_ID_D, "t85.e.ad");
    add_edge(T85_ID_B, T85_ID_C, "t85.e.bc");
    add_edge(T85_ID_B, T85_ID_D, "t85.e.bd");
    add_edge(T85_ID_C, T85_ID_D, "t85.e.cd");

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                4,        "k4: node_count=4");
    assert_eq!(ec,                6,        "k4: edge_count=6");
    assert_eq!(nnonapentaactc,    u64::MAX, "k4: NNONAPENTAACTC=SAT");
    assert_eq!(nhnonapentaactc,   u64::MAX, "k4: NHNONAPENTAACTC=SAT");
    assert_eq!(nbbso,             u64::MAX, "k4: NBBSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                 2, "isolated: node_count=2");
    assert_eq!(ec,                 0, "isolated: edge_count=0");
    assert_eq!(nnonapentaactc,     0, "isolated: NNONAPENTAACTC=0");
    assert_eq!(nhnonapentaactc,    0, "isolated: NHNONAPENTAACTC=0");
    assert_eq!(nbbso,              0, "isolated: NBBSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NNONAPENTAACTC=5×6^59 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T85_VEC_A, T85_KEY_A, T85_ID_A);
    add_node(T85_VEC_B, T85_KEY_B, T85_ID_B);
    add_node(T85_VEC_C, T85_KEY_C, T85_ID_C);
    add_node(T85_VEC_D, T85_KEY_D, T85_ID_D);
    add_node(T85_VEC_E, T85_KEY_E, T85_ID_E);
    add_edge(T85_ID_A, T85_ID_C, "t85.e.ac");
    add_edge(T85_ID_A, T85_ID_D, "t85.e.ad");
    add_edge(T85_ID_A, T85_ID_E, "t85.e.ae");
    add_edge(T85_ID_B, T85_ID_C, "t85.e.bc");
    add_edge(T85_ID_B, T85_ID_D, "t85.e.bd");
    add_edge(T85_ID_B, T85_ID_E, "t85.e.be");

    let (nnonapentaactc, nhnonapentaactc, nbbso, ec, nc) = gos_runtime::graph_topo_indices85();
    assert_eq!(nc,                5,        "k23: node_count=5");
    assert_eq!(ec,                6,        "k23: edge_count=6");
    assert_eq!(nnonapentaactc,    u64::MAX, "k23: NNONAPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{2079})");
    assert_eq!(nhnonapentaactc,   u64::MAX, "k23: NHNONAPENTAACTC=SAT");
    assert_eq!(nbbso,             u64::MAX, "k23: NBBSO=SAT");
}
