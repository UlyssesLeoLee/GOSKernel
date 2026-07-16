// gos-graph-topo36-harness — V3.47 NDC + NHNC + NOSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices36()`:
//   Returns (ndc, nhnc, noso, edge_count, node_count)
//   - ndc  = NDC(G)  = Σ_v S(v)^10                   (exact u64; S-Decic vertex sum)
//   - nhnc = NHNC(G) = Σ_{uv∈E} (S_u+S_v)^9          (exact u64; S-Nonic edge-sum)
//   - noso = NOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^4        (exact u64; S-Octic Sombor, α=8)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NDC(G) = Σ_v S(v)^10
//     S-Decic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35),
//       NDC=Σ S¹⁰ (topo36).
//     NDC = n·S^10 for S-regular.
//     Overflow: S^10 ≤ 16129^10 ≈ 2.6×10^41 > u128::MAX → saturating u128 accumulator, clamp to u64::MAX.
//
//   NHNC(G) = Σ_{uv∈E} (S_u+S_v)^9
//     S-Nonic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35), NHNC=Σ(S+S)⁹ (topo36).
//     NHNC = |E|·(2S)^9 = 512|E|S^9 for S-regular.
//     Overflow per edge: (2×16129)^9 ≈ 3.5×10^40 → saturating u128 accumulator.
//
//   NOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^4
//     S-Octic Sombor: generalised Sombor SO^α with α=8 on S-variant.
//     NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
//     NFSO(topo34)=Σ(S²+S²)^2 (α=4), NHSO(topo35)=Σ(S²+S²)^3 (α=6),
//     NOSO(topo36)=Σ(S²+S²)^4 (α=8) — exact integer, no isqrt.
//     NOSO = |E|·(2S²)^4 = 16|E|S^8 for S-regular.
//     Overflow per edge: (2×16129²)^4 ≈ 7.3×10^34 < u128::MAX; sum of 128 terms ≈ 9.4×10^36 < u128::MAX.
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
//  Graph       NDC(exact)       NHNC(exact)        NOSO(exact)    edges  nodes
//  Empty                  0                  0                0       0      0
//  1 node                 0                  0                0       0      1
//  K₂                     2                512               16       1      2
//  P₃                 3_072            524_288            8_192       2      3
//  K₃             3_145_728        402_653_184        3_145_728       3      3
//  K_{1,4}        5_242_880        536_870_912        4_194_304       4      5
//  P₄               120_146         13_983_946          162_098       3      4
//  K₄        13_947_137_604  1_190_155_742_208    4_132_485_216       6      4
//  2 isolated             0                  0                0       0      2
//  K_{2,3}      302_330_880     30_958_682_112      161_243_136       6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NDC:  1^10 + 1^10 = 2. ✓
//     NHNC: (1+1)^9 = 2^9 = 512. ✓
//     NOSO: (1²+1²)^4 = 2^4 = 16. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NDC:  3×2^10 = 3×1_024 = 3_072. ✓
//     NHNC: 2×(2+2)^9 = 2×4^9 = 2×262_144 = 524_288. ✓
//     NOSO: 2×(4+4)^4 = 2×8^4 = 2×4_096 = 8_192. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NDC:  3×4^10 = 3×1_048_576 = 3_145_728. ✓
//     NHNC: 3×(4+4)^9 = 3×8^9 = 3×134_217_728 = 402_653_184. ✓
//     NOSO: 3×(16+16)^4 = 3×32^4 = 3×1_048_576 = 3_145_728. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NDC:  5×4^10 = 5×1_048_576 = 5_242_880. ✓
//     NHNC: 4×8^9 = 4×134_217_728 = 536_870_912. ✓
//     NOSO: 4×32^4 = 4×1_048_576 = 4_194_304. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHNC and NOSO; NDC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NDC:  2^10+3^10+3^10+2^10 = 1_024+59_049+59_049+1_024 = 120_146. ✓
//     NHNC: 5^9+6^9+5^9 = 1_953_125+10_077_696+1_953_125 = 13_983_946. ✓
//       (5^9=1_953_125; 6^9=10_077_696)
//     NOSO: (4+9)^4+(9+9)^4+(9+4)^4 = 13^4+18^4+13^4 = 28_561+104_976+28_561 = 162_098. ✓
//       (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NDC:  4×9^10 = 4×3_486_784_401 = 13_947_137_604. ✓
//     NHNC: 6×18^9 = 6×198_359_290_368 = 1_190_155_742_208. ✓
//       (18^5=1_889_568; 18^9=18^5×18^4=1_889_568×104_976=198_359_290_368)
//     NOSO: 6×(81+81)^4 = 6×162^4 = 6×688_747_536 = 4_132_485_216. ✓
//       (162^2=26_244; 162^4=26_244^2=688_747_536)
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NDC:  5×6^10 = 5×60_466_176 = 302_330_880. ✓
//     NHNC: 6×12^9 = 6×5_159_780_352 = 30_958_682_112. ✓
//       (12^5=248_832; 12^9=12^5×12^4=248_832×20_736=5_159_780_352)
//     NOSO: 6×(36+36)^4 = 6×72^4 = 6×26_873_856 = 161_243_136. ✓
//       (72^2=5_184; 72^4=5_184^2=26_873_856)
//
// S-REGULAR FORMULA VERIFICATION:
//   NDC  = n·S^10                        for S-regular ✓
//   NHNC = |E|·(2S)^9 = 512|E|·S^9      for S-regular ✓
//   NOSO = |E|·(2S²)^4 = 16|E|·S^8      for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 512, 16, 1, 2)
//  4.  Path P₃ = A-B-C                   → (3_072, 524_288, 8_192, 2, 3)
//  5.  Triangle K₃                       → (3_145_728, 402_653_184, 3_145_728, 3, 3)
//  6.  Star K_{1,4}                      → (5_242_880, 536_870_912, 4_194_304, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (120_146, 13_983_946, 162_098, 3, 4)
//  8.  Complete K₄                       → (13_947_137_604, 1_190_155_742_208, 4_132_485_216, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (302_330_880, 30_958_682_112, 161_243_136, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T36_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_36");
const T36_EXEC:   ExecutorId = ExecutorId::from_ascii("t36.exec");

const T36_KEY_A: &str = "t36.alpha";
const T36_KEY_B: &str = "t36.beta";
const T36_KEY_C: &str = "t36.gamma";
const T36_KEY_D: &str = "t36.delta";
const T36_KEY_E: &str = "t36.epsilon";

const T36_ID_A: NodeId = derive_node_id(T36_PLUGIN, T36_KEY_A);
const T36_ID_B: NodeId = derive_node_id(T36_PLUGIN, T36_KEY_B);
const T36_ID_C: NodeId = derive_node_id(T36_PLUGIN, T36_KEY_C);
const T36_ID_D: NodeId = derive_node_id(T36_PLUGIN, T36_KEY_D);
const T36_ID_E: NodeId = derive_node_id(T36_PLUGIN, T36_KEY_E);

// L4=123 namespace for this harness.
const T36_VEC_A: VectorAddress = VectorAddress::new(123, 1, 1, 0);
const T36_VEC_B: VectorAddress = VectorAddress::new(123, 1, 2, 0);
const T36_VEC_C: VectorAddress = VectorAddress::new(123, 1, 3, 0);
const T36_VEC_D: VectorAddress = VectorAddress::new(123, 2, 1, 0);
const T36_VEC_E: VectorAddress = VectorAddress::new(123, 2, 2, 0);

const T36_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T36_PLUGIN,
    name:         "kl-graph-topo36-harness",
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
        executor_id:       T36_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T36_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T36_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(ndc,  0, "empty: NDC=0");
    assert_eq!(nhnc, 0, "empty: NHNC=0");
    assert_eq!(noso, 0, "empty: NOSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NDC: 0^10=0; NHNC: no edges; NOSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(ndc,  0, "single: NDC=0 (S=0; 0^10=0)");
    assert_eq!(nhnc, 0, "single: NHNC=0 (no edges)");
    assert_eq!(noso, 0, "single: NOSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NDC:  1^10+1^10 = 2.
// NHNC: (1+1)^9 = 2^9 = 512.
// NOSO: (1²+1²)^4 = 2^4 = 16.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);
    add_edge(T36_ID_A, T36_ID_B, "t36.e.ab");

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   2,   "k2: node_count=2");
    assert_eq!(ec,   1,   "k2: edge_count=1");
    assert_eq!(ndc,  2,   "k2: NDC=2 (1\u{00b9}\u{2070}+1\u{00b9}\u{2070}=2; S-uniform S=1)");
    assert_eq!(nhnc, 512, "k2: NHNC=512 ((1+1)\u{2079}=2\u{2079}=512; S-uniform S=1)");
    assert_eq!(noso, 16,  "k2: NOSO=16 ((1\u{00b2}+1\u{00b2})\u{2074}=2\u{2074}=16; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NDC:  3×2^10 = 3×1_024 = 3_072.
// NHNC: 2×(2+2)^9 = 2×4^9 = 2×262_144 = 524_288.
// NOSO: 2×(4+4)^4 = 2×8^4 = 2×4_096 = 8_192.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);
    add_node(T36_VEC_C, T36_KEY_C, T36_ID_C);
    add_edge(T36_ID_A, T36_ID_B, "t36.e.ab");
    add_edge(T36_ID_B, T36_ID_C, "t36.e.bc");

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   3,       "p3: node_count=3");
    assert_eq!(ec,   2,       "p3: edge_count=2");
    assert_eq!(ndc,  3_072,   "p3: NDC=3_072 (3\u{00d7}1_024; 2\u{00b9}\u{2070}=1_024; S-uniform S=2)");
    assert_eq!(nhnc, 524_288, "p3: NHNC=524_288 (2\u{00d7}262_144; (2+2)\u{2079}=4\u{2079}=262_144; S-uniform S=2)");
    assert_eq!(noso, 8_192,   "p3: NOSO=8_192 (2\u{00d7}4_096; (4+4)\u{2074}=8\u{2074}=4_096; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NDC:  3×4^10 = 3×1_048_576 = 3_145_728.
// NHNC: 3×(4+4)^9 = 3×8^9 = 3×134_217_728 = 402_653_184.
// NOSO: 3×(16+16)^4 = 3×32^4 = 3×1_048_576 = 3_145_728.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);
    add_node(T36_VEC_C, T36_KEY_C, T36_ID_C);
    add_edge(T36_ID_A, T36_ID_B, "t36.e.ab");
    add_edge(T36_ID_B, T36_ID_A, "t36.e.ba");
    add_edge(T36_ID_B, T36_ID_C, "t36.e.bc");
    add_edge(T36_ID_C, T36_ID_B, "t36.e.cb");
    add_edge(T36_ID_A, T36_ID_C, "t36.e.ac");
    add_edge(T36_ID_C, T36_ID_A, "t36.e.ca");

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   3,           "k3: node_count=3");
    assert_eq!(ec,   3,           "k3: edge_count=3");
    assert_eq!(ndc,  3_145_728,   "k3: NDC=3_145_728 (3\u{00d7}1_048_576; 4\u{00b9}\u{2070}=1_048_576; S-uniform S=4)");
    assert_eq!(nhnc, 402_653_184, "k3: NHNC=402_653_184 (3\u{00d7}134_217_728; (4+4)\u{2079}=8\u{2079}=134_217_728; S-uniform S=4)");
    assert_eq!(noso, 3_145_728,   "k3: NOSO=3_145_728 (3\u{00d7}1_048_576; (16+16)\u{2074}=32\u{2074}=1_048_576; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHNC (134_217_728) and NOSO (1_048_576) as K₃; NDC and totals differ.
// NDC:  5×4^10 = 5×1_048_576 = 5_242_880.
// NHNC: 4×8^9 = 4×134_217_728 = 536_870_912.
// NOSO: 4×32^4 = 4×1_048_576 = 4_194_304.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);
    add_node(T36_VEC_C, T36_KEY_C, T36_ID_C);
    add_node(T36_VEC_D, T36_KEY_D, T36_ID_D);
    add_node(T36_VEC_E, T36_KEY_E, T36_ID_E);
    add_edge(T36_ID_A, T36_ID_B, "t36.e.ab");
    add_edge(T36_ID_A, T36_ID_C, "t36.e.ac");
    add_edge(T36_ID_A, T36_ID_D, "t36.e.ad");
    add_edge(T36_ID_A, T36_ID_E, "t36.e.ae");

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   5,           "star: node_count=5");
    assert_eq!(ec,   4,           "star: edge_count=4");
    assert_eq!(ndc,  5_242_880,   "star: NDC=5_242_880 (5\u{00d7}1_048_576; same S as K\u{2083})");
    assert_eq!(nhnc, 536_870_912, "star: NHNC=536_870_912 (4\u{00d7}134_217_728; same per-edge as K\u{2083})");
    assert_eq!(noso, 4_194_304,   "star: NOSO=4_194_304 (4\u{00d7}1_048_576; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NDC:  2^10+3^10+3^10+2^10 = 1_024+59_049+59_049+1_024 = 120_146.
// NHNC: (2+3)^9+(3+3)^9+(3+2)^9 = 5^9+6^9+5^9 = 1_953_125+10_077_696+1_953_125 = 13_983_946.
// NOSO: (4+9)^4+(9+9)^4+(9+4)^4 = 13^4+18^4+13^4 = 28_561+104_976+28_561 = 162_098.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);
    add_node(T36_VEC_C, T36_KEY_C, T36_ID_C);
    add_node(T36_VEC_D, T36_KEY_D, T36_ID_D);
    add_edge(T36_ID_A, T36_ID_B, "t36.e.ab");
    add_edge(T36_ID_B, T36_ID_C, "t36.e.bc");
    add_edge(T36_ID_C, T36_ID_D, "t36.e.cd");

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   4,          "p4: node_count=4");
    assert_eq!(ec,   3,          "p4: edge_count=3");
    assert_eq!(ndc,  120_146,    "p4: NDC=120_146 (1_024+59_049+59_049+1_024; 2\u{00b9}\u{2070}+3\u{00b9}\u{2070}+3\u{00b9}\u{2070}+2\u{00b9}\u{2070})");
    assert_eq!(nhnc, 13_983_946, "p4: NHNC=13_983_946 (1_953_125+10_077_696+1_953_125; 5\u{2079}+6\u{2079}+5\u{2079})");
    assert_eq!(noso, 162_098,    "p4: NOSO=162_098 (28_561+104_976+28_561; 13\u{2074}+18\u{2074}+13\u{2074})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NDC:  4×9^10 = 4×3_486_784_401 = 13_947_137_604.
// NHNC: 6×18^9 = 6×198_359_290_368 = 1_190_155_742_208.
// NOSO: 6×(81+81)^4 = 6×162^4 = 6×688_747_536 = 4_132_485_216.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);
    add_node(T36_VEC_C, T36_KEY_C, T36_ID_C);
    add_node(T36_VEC_D, T36_KEY_D, T36_ID_D);
    add_edge(T36_ID_A, T36_ID_B, "t36.e.ab");
    add_edge(T36_ID_B, T36_ID_A, "t36.e.ba");
    add_edge(T36_ID_A, T36_ID_C, "t36.e.ac");
    add_edge(T36_ID_C, T36_ID_A, "t36.e.ca");
    add_edge(T36_ID_A, T36_ID_D, "t36.e.ad");
    add_edge(T36_ID_D, T36_ID_A, "t36.e.da");
    add_edge(T36_ID_B, T36_ID_C, "t36.e.bc");
    add_edge(T36_ID_C, T36_ID_B, "t36.e.cb");
    add_edge(T36_ID_B, T36_ID_D, "t36.e.bd");
    add_edge(T36_ID_D, T36_ID_B, "t36.e.db");
    add_edge(T36_ID_C, T36_ID_D, "t36.e.cd");
    add_edge(T36_ID_D, T36_ID_C, "t36.e.dc");

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   4,                    "k4: node_count=4");
    assert_eq!(ec,   6,                    "k4: edge_count=6");
    assert_eq!(ndc,  13_947_137_604,       "k4: NDC=13_947_137_604 (4\u{00d7}3_486_784_401; 9\u{00b9}\u{2070}=3_486_784_401; S-uniform S=9)");
    assert_eq!(nhnc, 1_190_155_742_208,    "k4: NHNC=1_190_155_742_208 (6\u{00d7}198_359_290_368; 18\u{2079}=198_359_290_368; S-uniform S=9)");
    assert_eq!(noso, 4_132_485_216,        "k4: NOSO=4_132_485_216 (6\u{00d7}688_747_536; 162\u{2074}=688_747_536; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NDC=0; NHNC=0; NOSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(ndc,  0, "isolated: NDC=0 (S=0; 0^10=0)");
    assert_eq!(nhnc, 0, "isolated: NHNC=0 (no edges)");
    assert_eq!(noso, 0, "isolated: NOSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NDC:  5×6^10 = 5×60_466_176 = 302_330_880.
// NHNC: 6×12^9 = 6×5_159_780_352 = 30_958_682_112.
// NOSO: 6×(36+36)^4 = 6×72^4 = 6×26_873_856 = 161_243_136.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T36_VEC_A, T36_KEY_A, T36_ID_A);
    add_node(T36_VEC_B, T36_KEY_B, T36_ID_B);
    add_node(T36_VEC_C, T36_KEY_C, T36_ID_C);
    add_node(T36_VEC_D, T36_KEY_D, T36_ID_D);
    add_node(T36_VEC_E, T36_KEY_E, T36_ID_E);
    add_edge(T36_ID_A, T36_ID_C, "t36.e.ac");
    add_edge(T36_ID_C, T36_ID_A, "t36.e.ca");
    add_edge(T36_ID_A, T36_ID_D, "t36.e.ad");
    add_edge(T36_ID_D, T36_ID_A, "t36.e.da");
    add_edge(T36_ID_A, T36_ID_E, "t36.e.ae");
    add_edge(T36_ID_E, T36_ID_A, "t36.e.ea");
    add_edge(T36_ID_B, T36_ID_C, "t36.e.bc");
    add_edge(T36_ID_C, T36_ID_B, "t36.e.cb");
    add_edge(T36_ID_B, T36_ID_D, "t36.e.bd");
    add_edge(T36_ID_D, T36_ID_B, "t36.e.db");
    add_edge(T36_ID_B, T36_ID_E, "t36.e.be");
    add_edge(T36_ID_E, T36_ID_B, "t36.e.eb");

    let (ndc, nhnc, noso, ec, nc) = gos_runtime::graph_topo_indices36();
    assert_eq!(nc,   5,               "k23: node_count=5");
    assert_eq!(ec,   6,               "k23: edge_count=6");
    assert_eq!(ndc,  302_330_880,     "k23: NDC=302_330_880 (5\u{00d7}60_466_176; 6\u{00b9}\u{2070}=60_466_176; S-uniform S=6)");
    assert_eq!(nhnc, 30_958_682_112,  "k23: NHNC=30_958_682_112 (6\u{00d7}5_159_780_352; 12\u{2079}=5_159_780_352; S-uniform S=6)");
    assert_eq!(noso, 161_243_136,     "k23: NOSO=161_243_136 (6\u{00d7}26_873_856; 72\u{2074}=26_873_856; S-uniform S=6)");
}
