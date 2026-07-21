// gos-graph-topo83-harness — V3.94 NHEPTPENTAACTC + NHHEPTPENTAACTC + NAZSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices83()`:
//   Returns (nheptpentaactc, nhheptpentaactc, nazso, edge_count, node_count)
//   - nheptpentaactc  = NHEPTPENTAACTC(G)  = Σ_v S(v)^57                   (exact u64; S-Heptapentacontic vertex sum)
//   - nhheptpentaactc = NHHEPTPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^56         (exact u64; S-Hexapentacontic edge-sum)
//   - nazso           = NAZSO(G)           = Σ_{uv∈E} (S_u²+S_v²)^51       (exact u64; S-Variant Sombor, α=102)
//   - edge_count      = undirected non-self-loop edges
//   - node_count      = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NHEPTPENTAACTC(G) = Σ_v S(v)^57
//     S-Heptapentacontic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), ...,
//       NHEXPENTAACTC=Σ S⁵⁶ (topo82), NHEPTPENTAACTC=Σ S⁵⁷ (topo83). Eighth of the pentacontic (50-59) series.
//     NHEPTPENTAACTC = n·S^57 for S-regular.
//     Overflow: S^57 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
//     Implementation: s^57 = s32 × s16 × s8 × s  (57=32+16+8+1; 4 mults).
//
//   NHHEPTPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^56
//     S-Hexapentacontic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), ..., NHHEXPENTAACTC=Σ(S+S)⁵⁵ (topo82),
//       NHHEPTPENTAACTC=Σ(S+S)⁵⁶ (topo83).
//     NHHEPTPENTAACTC = |E|·(2S)^56 = 72057594037927936|E|·S^56 for S-regular.
//     Overflow per edge: (2×16129)^56 → saturating u128 accumulator.
//     Implementation: ss^56 = ss32 × ss16 × ss8  (56=32+16+8; 3 mults — efficient!).
//
//   NAZSO(G) = Σ_{uv∈E} (S_u²+S_v²)^51
//     S-Variant Sombor: generalised Sombor SO^α with α=102 on S-variant.
//     3rd-pass double-letter "AZ" (after NAYSO α=100, topo82).
//     NSO(topo21,α=1),..., NAASO(topo58,α=52),..., NAYSO(topo82,α=100), NAZSO(topo83,α=102).
//     NAZSO = |E|·(2S²)^51 = 2251799813685248|E|·S^102 for S-regular.
//     Overflow per edge: (2×16129²)^51 → saturating u128 accumulator.
//     Implementation: s2s^51 = s2s32 × s2s16 × s2s2 × s2s  (51=32+16+2+1; 4 mults).
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
//  Graph     NHEPTPENTAACTC(exact)            NHHEPTPENTAACTC(exact)      NAZSO(exact)              edges  nodes
//  Empty                       0                             0                      0                0      0
//  1 node                      0                             0                      0                0      1
//  K₂                          2            72_057_594_037_927_936    2_251_799_813_685_248              1      2
//  P₃       432_345_564_227_567_616               u64::MAX(sat.)             u64::MAX(sat.)            2      3
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
//     NHEPTPENTAACTC:  1^57 + 1^57 = 2. ✓
//     NHHEPTPENTAACTC: (1+1)^56 = 2^56 = 72_057_594_037_927_936. ✓
//     NAZSO:           (1²+1²)^51 = 2^51 = 2_251_799_813_685_248. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NHEPTPENTAACTC:  3×2^57 = 3×144_115_188_075_855_872 = 432_345_564_227_567_616. ✓
//     NHHEPTPENTAACTC: 2×(2+2)^56 = 2×4^56 = 2×2^112 → SATURATES. ✓
//     NAZSO:           2×(4+4)^51 = 2×8^51 = 2×2^153 → SATURATES. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NHEPTPENTAACTC:  3×4^57 = 3×2^114 → SATURATES. ✓
//     NHHEPTPENTAACTC: 3×8^56 → SATURATES. ✓
//     NAZSO:           3×32^51 → SATURATES. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NHEPTPENTAACTC:  5×4^57 → SATURATES. ✓
//     NHHEPTPENTAACTC: 4×8^56 → SATURATES. ✓
//     NAZSO:           4×32^51 → SATURATES. ✓
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NHEPTPENTAACTC:  2×2^57 + 2×3^57.  3^41>u64::MAX → 3^57 >> u64::MAX → SATURATES. ✓
//     NHHEPTPENTAACTC: 5^56+6^56+5^56 → each term >> u64::MAX → SATURATES. ✓
//     NAZSO:           13^51+18^51+13^51 → SATURATES. ✓
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NHEPTPENTAACTC:  4×9^57 → SATURATES. ✓
//     NHHEPTPENTAACTC: 6×18^56 → SATURATES. ✓
//     NAZSO:           6×162^51 → SATURATES. ✓
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NHEPTPENTAACTC:  5×6^57 → SATURATES. ✓
//     NHHEPTPENTAACTC: 6×12^56 → SATURATES. ✓
//     NAZSO:           6×72^51 → SATURATES. ✓
//
// S-REGULAR FORMULA VERIFICATION:
//   NHEPTPENTAACTC  = n·S^57                                                                    for S-regular ✓
//   NHHEPTPENTAACTC = |E|·(2S)^56 = 72057594037927936|E|·S^56                                  for S-regular ✓
//   NAZSO           = |E|·(2S²)^51 = 2251799813685248|E|·S^102                                 for S-regular ✓
//   Note: ss^56 = ss32×ss16×ss8 is efficient (56=32+16+8, three powers of 2, only 3 mults)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 72_057_594_037_927_936, 2_251_799_813_685_248, 1, 2)
//  4.  Path P₃ = A-B-C                   → (432_345_564_227_567_616, u64::MAX, u64::MAX, 2, 3)
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

const T83_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_83");
const T83_EXEC:   ExecutorId = ExecutorId::from_ascii("t83.exec");

const T83_KEY_A: &str = "t83.alpha";
const T83_KEY_B: &str = "t83.beta";
const T83_KEY_C: &str = "t83.gamma";
const T83_KEY_D: &str = "t83.delta";
const T83_KEY_E: &str = "t83.epsilon";

const T83_ID_A: NodeId = derive_node_id(T83_PLUGIN, T83_KEY_A);
const T83_ID_B: NodeId = derive_node_id(T83_PLUGIN, T83_KEY_B);
const T83_ID_C: NodeId = derive_node_id(T83_PLUGIN, T83_KEY_C);
const T83_ID_D: NodeId = derive_node_id(T83_PLUGIN, T83_KEY_D);
const T83_ID_E: NodeId = derive_node_id(T83_PLUGIN, T83_KEY_E);

// L4=170 namespace for this harness.
const T83_VEC_A: VectorAddress = VectorAddress::new(170, 1, 1, 0);
const T83_VEC_B: VectorAddress = VectorAddress::new(170, 1, 2, 0);
const T83_VEC_C: VectorAddress = VectorAddress::new(170, 1, 3, 0);
const T83_VEC_D: VectorAddress = VectorAddress::new(170, 2, 1, 0);
const T83_VEC_E: VectorAddress = VectorAddress::new(170, 2, 2, 0);

const T83_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T83_PLUGIN,
    name:         "kl-graph-topo83-harness",
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
        executor_id:       T83_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T83_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T83_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
#[test]
fn test_01_empty() {
    let _g = setup();
    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,                0, "empty: node_count=0");
    assert_eq!(ec,                0, "empty: edge_count=0");
    assert_eq!(nheptpentaactc,    0, "empty: NHEPTPENTAACTC=0");
    assert_eq!(nhheptpentaactc,   0, "empty: NHHEPTPENTAACTC=0");
    assert_eq!(nazso,             0, "empty: NAZSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S(A)=0 → no edges → all indices 0.
#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,                1, "single: node_count=1");
    assert_eq!(ec,                0, "single: edge_count=0");
    assert_eq!(nheptpentaactc,    0, "single: NHEPTPENTAACTC=0");
    assert_eq!(nhheptpentaactc,   0, "single: NHHEPTPENTAACTC=0");
    assert_eq!(nazso,             0, "single: NAZSO=0");
}

// ── Test 3: Single directed edge A→B (K₂) ───────────────────────────────────
// deg(A)=deg(B)=1.  S(A)=S(B)=1.  1 edge, 2 nodes.
// NHEPTPENTAACTC:  1^57+1^57 = 2.
// NHHEPTPENTAACTC: (1+1)^56 = 2^56 = 72_057_594_037_927_936.
// NAZSO:           (1²+1²)^51 = 2^51 = 2_251_799_813_685_248.
#[test]
fn test_03_k2_edge() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);
    add_edge(T83_ID_A, T83_ID_B, "t83.e.ab");

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,                2,                          "k2: node_count=2");
    assert_eq!(ec,                1,                          "k2: edge_count=1");
    assert_eq!(nheptpentaactc,    2,                          "k2: NHEPTPENTAACTC=2 (1\u{2075}\u{2077}+1\u{2075}\u{2077}=2)");
    assert_eq!(nhheptpentaactc,   72_057_594_037_927_936,     "k2: NHHEPTPENTAACTC=72_057_594_037_927_936 (2\u{2075}\u{2076}=2^56)");
    assert_eq!(nazso,             2_251_799_813_685_248,      "k2: NAZSO=2_251_799_813_685_248 (2\u{2075}\u{00b9}=2^51)");
}

// ── Test 4: Path P₃ = A-B-C ─────────────────────────────────────────────────
// deg: A=1,B=2,C=1.  S: S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S=2 uniform, 2 edges, 3 nodes.
// NHEPTPENTAACTC:  3×2^57 = 3×144_115_188_075_855_872 = 432_345_564_227_567_616.
// NHHEPTPENTAACTC: 2×(2+2)^56 = 2×4^56 = 2×2^112 → SATURATES.
// NAZSO:           2×(4+4)^51 = 2×8^51 = 2×2^153 → SATURATES.
#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);
    add_node(T83_VEC_C, T83_KEY_C, T83_ID_C);
    add_edge(T83_ID_A, T83_ID_B, "t83.e.ab");
    add_edge(T83_ID_B, T83_ID_C, "t83.e.bc");

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,                3,                          "p3: node_count=3");
    assert_eq!(ec,                2,                          "p3: edge_count=2");
    assert_eq!(nheptpentaactc,    432_345_564_227_567_616,    "p3: NHEPTPENTAACTC=432_345_564_227_567_616 (3\u{00d7}2\u{2075}\u{2077})");
    assert_eq!(nhheptpentaactc,   u64::MAX,                   "p3: NHHEPTPENTAACTC=SAT (4\u{2075}\u{2076}>u64)");
    assert_eq!(nazso,             u64::MAX,                   "p3: NAZSO=SAT (8\u{2075}\u{00b9}>u64)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// deg=2 for all.  S(each)=4.  3 edges, 3 nodes. All saturate.
#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);
    add_node(T83_VEC_C, T83_KEY_C, T83_ID_C);
    add_edge(T83_ID_A, T83_ID_B, "t83.e.ab");
    add_edge(T83_ID_B, T83_ID_C, "t83.e.bc");
    add_edge(T83_ID_C, T83_ID_A, "t83.e.ca");

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,               3,        "k3: node_count=3");
    assert_eq!(ec,               3,        "k3: edge_count=3");
    assert_eq!(nheptpentaactc,   u64::MAX, "k3: NHEPTPENTAACTC=SAT");
    assert_eq!(nhheptpentaactc,  u64::MAX, "k3: NHHEPTPENTAACTC=SAT");
    assert_eq!(nazso,            u64::MAX, "k3: NAZSO=SAT");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Hub has deg=4, leaves deg=1.  S(hub)=4×1=4, S(leaf)=deg(hub)=4.
// S=4 uniform → all saturate.
#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A); // hub
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);
    add_node(T83_VEC_C, T83_KEY_C, T83_ID_C);
    add_node(T83_VEC_D, T83_KEY_D, T83_ID_D);
    add_node(T83_VEC_E, T83_KEY_E, T83_ID_E);
    add_edge(T83_ID_A, T83_ID_B, "t83.e.ab");
    add_edge(T83_ID_A, T83_ID_C, "t83.e.ac");
    add_edge(T83_ID_A, T83_ID_D, "t83.e.ad");
    add_edge(T83_ID_A, T83_ID_E, "t83.e.ae");

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,               5,        "k14: node_count=5");
    assert_eq!(ec,               4,        "k14: edge_count=4");
    assert_eq!(nheptpentaactc,   u64::MAX, "k14: NHEPTPENTAACTC=SAT");
    assert_eq!(nhheptpentaactc,  u64::MAX, "k14: NHHEPTPENTAACTC=SAT");
    assert_eq!(nazso,            u64::MAX, "k14: NAZSO=SAT");
}

// ── Test 7: Path P₄ = A-B-C-D ───────────────────────────────────────────────
// deg: A=1,B=2,C=2,D=1.  S(A)=2,S(B)=1+2=3,S(C)=2+1=3,S(D)=2.
// NHEPTPENTAACTC:  2×2^57 + 2×3^57.  3^41>u64::MAX → SATURATES.
// NHHEPTPENTAACTC: 5^56+6^56+5^56 → SATURATES.
// NAZSO:           13^51+18^51+13^51 → SATURATES.
#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);
    add_node(T83_VEC_C, T83_KEY_C, T83_ID_C);
    add_node(T83_VEC_D, T83_KEY_D, T83_ID_D);
    add_edge(T83_ID_A, T83_ID_B, "t83.e.ab");
    add_edge(T83_ID_B, T83_ID_C, "t83.e.bc");
    add_edge(T83_ID_C, T83_ID_D, "t83.e.cd");

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,               4,        "p4: node_count=4");
    assert_eq!(ec,               3,        "p4: edge_count=3");
    assert_eq!(nheptpentaactc,   u64::MAX, "p4: NHEPTPENTAACTC=SAT");
    assert_eq!(nhheptpentaactc,  u64::MAX, "p4: NHHEPTPENTAACTC=SAT");
    assert_eq!(nazso,            u64::MAX, "p4: NAZSO=SAT");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// deg=3 for all.  S(each)=3+3+3=9.  6 edges, 4 nodes. All saturate.
#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);
    add_node(T83_VEC_C, T83_KEY_C, T83_ID_C);
    add_node(T83_VEC_D, T83_KEY_D, T83_ID_D);
    add_edge(T83_ID_A, T83_ID_B, "t83.e.ab");
    add_edge(T83_ID_A, T83_ID_C, "t83.e.ac");
    add_edge(T83_ID_A, T83_ID_D, "t83.e.ad");
    add_edge(T83_ID_B, T83_ID_C, "t83.e.bc");
    add_edge(T83_ID_B, T83_ID_D, "t83.e.bd");
    add_edge(T83_ID_C, T83_ID_D, "t83.e.cd");

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,               4,        "k4: node_count=4");
    assert_eq!(ec,               6,        "k4: edge_count=6");
    assert_eq!(nheptpentaactc,   u64::MAX, "k4: NHEPTPENTAACTC=SAT");
    assert_eq!(nhheptpentaactc,  u64::MAX, "k4: NHHEPTPENTAACTC=SAT");
    assert_eq!(nazso,            u64::MAX, "k4: NAZSO=SAT");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges → S=0 everywhere → all indices 0.
#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,                2, "isolated: node_count=2");
    assert_eq!(ec,                0, "isolated: edge_count=0");
    assert_eq!(nheptpentaactc,    0, "isolated: NHEPTPENTAACTC=0");
    assert_eq!(nhheptpentaactc,   0, "isolated: NHHEPTPENTAACTC=0");
    assert_eq!(nazso,             0, "isolated: NAZSO=0");
}

// ── Test 10: K_{2,3} bipartite ───────────────────────────────────────────────
// Parts {A,B} (deg=3) and {C,D,E} (deg=2).
// S(A)=S(B)=deg(C)+deg(D)+deg(E)=6; S(C)=S(D)=S(E)=deg(A)+deg(B)=6.
// S=6 uniform. NHEPTPENTAACTC=5×6^57 → SAT; all three saturate.
#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T83_VEC_A, T83_KEY_A, T83_ID_A);
    add_node(T83_VEC_B, T83_KEY_B, T83_ID_B);
    add_node(T83_VEC_C, T83_KEY_C, T83_ID_C);
    add_node(T83_VEC_D, T83_KEY_D, T83_ID_D);
    add_node(T83_VEC_E, T83_KEY_E, T83_ID_E);
    add_edge(T83_ID_A, T83_ID_C, "t83.e.ac");
    add_edge(T83_ID_A, T83_ID_D, "t83.e.ad");
    add_edge(T83_ID_A, T83_ID_E, "t83.e.ae");
    add_edge(T83_ID_B, T83_ID_C, "t83.e.bc");
    add_edge(T83_ID_B, T83_ID_D, "t83.e.bd");
    add_edge(T83_ID_B, T83_ID_E, "t83.e.be");

    let (nheptpentaactc, nhheptpentaactc, nazso, ec, nc) = gos_runtime::graph_topo_indices83();
    assert_eq!(nc,               5,        "k23: node_count=5");
    assert_eq!(ec,               6,        "k23: edge_count=6");
    assert_eq!(nheptpentaactc,   u64::MAX, "k23: NHEPTPENTAACTC=SAT (5\u{00d7}6\u{2075}\u{2077})");
    assert_eq!(nhheptpentaactc,  u64::MAX, "k23: NHHEPTPENTAACTC=SAT");
    assert_eq!(nazso,            u64::MAX, "k23: NAZSO=SAT");
}
