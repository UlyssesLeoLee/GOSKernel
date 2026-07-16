// gos-graph-topo35-harness — V3.46 NNC + NHOC + NHSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices35()`:
//   Returns (nnc, nhoc, nhso, edge_count, node_count)
//   - nnc  = NNC(G)  = Σ_v S(v)^9                  (exact u64; S-Nonic vertex sum)
//   - nhoc = NHOC(G) = Σ_{uv∈E} (S_u+S_v)^8        (exact u64; S-Octic edge-sum)
//   - nhso = NHSO(G) = Σ_{uv∈E} (S_u²+S_v²)^3      (exact u64; S-Hextic Sombor, α=6)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NNC(G) = Σ_v S(v)^9
//     S-Nonic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33), NOC=Σ S⁸ (topo34), NNC=Σ S⁹ (topo35).
//     NNC = n·S^9 for S-regular.
//     Overflow: S^9 ≤ 16129^9 ≈ 9×10^36 > u64::MAX → u128 accumulator, clamp to u64::MAX.
//
//   NHOC(G) = Σ_{uv∈E} (S_u+S_v)^8
//     S-Octic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33), NHHS=Σ(S+S)⁷ (topo34),
//       NHOC=Σ(S+S)⁸ (topo35).
//     NHOC = |E|·(2S)^8 = 256|E|S^8 for S-regular.
//     Overflow per edge: (2×16129)^8 ≈ 1.37×10^35 → u128 accumulator.
//
//   NHSO(G) = Σ_{uv∈E} (S_u²+S_v²)^3
//     S-Hextic Sombor: generalised Sombor SO^α with α=6 on S-variant.
//     NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
//     NFSO(topo34)=Σ(S²+S²)^2 (α=4), NHSO(topo35)=Σ(S²+S²)^3 (α=6) — exact integer, no isqrt.
//     NHSO = |E|·(2S²)^3 = 8|E|S^6 for S-regular.
//     Overflow per edge: (2×16129²)^3 ≈ 1.4×10^26 → u128 accumulator.
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
//  Graph       NNC(exact)       NHOC(exact)      NHSO(exact)    edges  nodes
//  Empty                  0               0               0       0      0
//  1 node                 0               0               0       0      1
//  K₂                     2             256               8       1      2
//  P₃                 1_536         131_072           1_024       2      3
//  K₃               786_432      50_331_648          98_304       3      3
//  K_{1,4}        1_310_720      67_108_864         131_072       4      5
//  P₄                40_390       2_460_866          10_226       3      4
//  K₄         1_549_681_956  66_119_763_456      25_509_168       6      4
//  2 isolated             0               0               0       0      2
//  K_{2,3}       50_388_480   2_579_890_176       2_239_488       6      5
//
// DERIVATIONS:
//
//   K₂ (S=1 uniform, 1 edge, 2 nodes):
//     NNC:  1^9 + 1^9 = 2. ✓
//     NHOC: (1+1)^8 = 2^8 = 256. ✓
//     NHSO: (1²+1²)^3 = 2^3 = 8. ✓
//
//   P₃ (S=2 uniform, 2 edges, 3 nodes):
//     NNC:  3×2^9 = 3×512 = 1_536. ✓
//     NHOC: 2×(2+2)^8 = 2×4^8 = 2×65_536 = 131_072. ✓
//     NHSO: 2×(4+4)^3 = 2×8^3 = 2×512 = 1_024. ✓
//
//   K₃ (S=4 uniform, 3 edges, 3 nodes):
//     NNC:  3×4^9 = 3×262_144 = 786_432. ✓
//     NHOC: 3×(4+4)^8 = 3×8^8 = 3×16_777_216 = 50_331_648. ✓
//     NHSO: 3×(16+16)^3 = 3×32^3 = 3×32_768 = 98_304. ✓
//
//   K_{1,4} (S=4 uniform, 4 edges, 5 nodes):
//     NNC:  5×4^9 = 5×262_144 = 1_310_720. ✓
//     NHOC: 4×8^8 = 4×16_777_216 = 67_108_864. ✓
//     NHSO: 4×32^3 = 4×32_768 = 131_072. ✓
//     Note: K₃ and K_{1,4} share S=4; same per-edge NHOC and NHSO; NNC differs by n.
//
//   P₄ (S(A)=2, S(B)=3, S(C)=3, S(D)=2; 3 edges, 4 nodes):
//     NNC:  2^9+3^9+3^9+2^9 = 512+19_683+19_683+512 = 40_390. ✓
//     NHOC: 5^8+6^8+5^8 = 390_625+1_679_616+390_625 = 2_460_866. ✓
//       (5^8=390_625; 6^8=1_679_616)
//     NHSO: (4+9)^3+(9+9)^3+(9+4)^3 = 13^3+18^3+13^3 = 2_197+5_832+2_197 = 10_226. ✓
//       (S_A²+S_B²=4+9=13; S_B²+S_C²=9+9=18; S_C²+S_D²=9+4=13)
//
//   K₄ (S=9 uniform, 6 edges, 4 nodes):
//     NNC:  4×9^9 = 4×387_420_489 = 1_549_681_956. ✓
//     NHOC: 6×18^8 = 6×11_019_960_576 = 66_119_763_456. ✓
//       (18^4=104_976; 18^8=104_976²=11_019_960_576)
//     NHSO: 6×(81+81)^3 = 6×162^3 = 6×4_251_528 = 25_509_168. ✓
//       (162^2=26_244; 162^3=26_244×162=4_251_528)
//
//   K_{2,3} (S=6 uniform, 6 edges, 5 nodes):
//     NNC:  5×6^9 = 5×10_077_696 = 50_388_480. ✓
//     NHOC: 6×12^8 = 6×429_981_696 = 2_579_890_176. ✓
//       (12^4=20_736; 12^8=20_736²=429_981_696)
//     NHSO: 6×(36+36)^3 = 6×72^3 = 6×373_248 = 2_239_488. ✓
//       (72^2=5_184; 72^3=5_184×72=373_248)
//
// S-REGULAR FORMULA VERIFICATION:
//   NNC  = n·S^9                       for S-regular ✓
//   NHOC = |E|·(2S)^8 = 256|E|·S^8    for S-regular ✓
//   NHSO = |E|·(2S²)^3 = 8|E|·S^6     for S-regular ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 256, 8, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_536, 131_072, 1_024, 2, 3)
//  5.  Triangle K₃                       → (786_432, 50_331_648, 98_304, 3, 3)
//  6.  Star K_{1,4}                      → (1_310_720, 67_108_864, 131_072, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (40_390, 2_460_866, 10_226, 3, 4)
//  8.  Complete K₄                       → (1_549_681_956, 66_119_763_456, 25_509_168, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (50_388_480, 2_579_890_176, 2_239_488, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T35_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_35");
const T35_EXEC:   ExecutorId = ExecutorId::from_ascii("t35.exec");

const T35_KEY_A: &str = "t35.alpha";
const T35_KEY_B: &str = "t35.beta";
const T35_KEY_C: &str = "t35.gamma";
const T35_KEY_D: &str = "t35.delta";
const T35_KEY_E: &str = "t35.epsilon";

const T35_ID_A: NodeId = derive_node_id(T35_PLUGIN, T35_KEY_A);
const T35_ID_B: NodeId = derive_node_id(T35_PLUGIN, T35_KEY_B);
const T35_ID_C: NodeId = derive_node_id(T35_PLUGIN, T35_KEY_C);
const T35_ID_D: NodeId = derive_node_id(T35_PLUGIN, T35_KEY_D);
const T35_ID_E: NodeId = derive_node_id(T35_PLUGIN, T35_KEY_E);

// L4=122 namespace for this harness.
const T35_VEC_A: VectorAddress = VectorAddress::new(122, 1, 1, 0);
const T35_VEC_B: VectorAddress = VectorAddress::new(122, 1, 2, 0);
const T35_VEC_C: VectorAddress = VectorAddress::new(122, 1, 3, 0);
const T35_VEC_D: VectorAddress = VectorAddress::new(122, 2, 1, 0);
const T35_VEC_E: VectorAddress = VectorAddress::new(122, 2, 2, 0);

const T35_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T35_PLUGIN,
    name:         "kl-graph-topo35-harness",
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
        executor_id:       T35_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T35_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T35_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nnc,  0, "empty: NNC=0");
    assert_eq!(nhoc, 0, "empty: NHOC=0");
    assert_eq!(nhso, 0, "empty: NHSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NNC: 0^9=0; NHOC: no edges; NHSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nnc,  0, "single: NNC=0 (S=0; 0^9=0)");
    assert_eq!(nhoc, 0, "single: NHOC=0 (no edges)");
    assert_eq!(nhso, 0, "single: NHSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NNC:  1^9+1^9 = 2.
// NHOC: (1+1)^8 = 2^8 = 256.
// NHSO: (1²+1²)^3 = 2^3 = 8.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);
    add_edge(T35_ID_A, T35_ID_B, "t35.e.ab");

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   2,   "k2: node_count=2");
    assert_eq!(ec,   1,   "k2: edge_count=1");
    assert_eq!(nnc,  2,   "k2: NNC=2 (1\u{2079}+1\u{2079}=2; S-uniform S=1)");
    assert_eq!(nhoc, 256, "k2: NHOC=256 ((1+1)\u{2078}=2\u{2078}=256; S-uniform S=1)");
    assert_eq!(nhso, 8,   "k2: NHSO=8 ((1\u{00b2}+1\u{00b2})\u{00b3}=2\u{00b3}=8; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NNC:  3×2^9 = 3×512 = 1_536.
// NHOC: 2×(2+2)^8 = 2×4^8 = 2×65_536 = 131_072.
// NHSO: 2×(4+4)^3 = 2×8^3 = 2×512 = 1_024.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);
    add_node(T35_VEC_C, T35_KEY_C, T35_ID_C);
    add_edge(T35_ID_A, T35_ID_B, "t35.e.ab");
    add_edge(T35_ID_B, T35_ID_C, "t35.e.bc");

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   3,       "p3: node_count=3");
    assert_eq!(ec,   2,       "p3: edge_count=2");
    assert_eq!(nnc,  1_536,   "p3: NNC=1_536 (3\u{00d7}512; 2\u{2079}=512; S-uniform S=2)");
    assert_eq!(nhoc, 131_072, "p3: NHOC=131_072 (2\u{00d7}65_536; (2+2)\u{2078}=4\u{2078}=65_536; S-uniform S=2)");
    assert_eq!(nhso, 1_024,   "p3: NHSO=1_024 (2\u{00d7}512; (4+4)\u{00b3}=8\u{00b3}=512; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NNC:  3×4^9 = 3×262_144 = 786_432.
// NHOC: 3×(4+4)^8 = 3×8^8 = 3×16_777_216 = 50_331_648.
// NHSO: 3×(16+16)^3 = 3×32^3 = 3×32_768 = 98_304.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);
    add_node(T35_VEC_C, T35_KEY_C, T35_ID_C);
    add_edge(T35_ID_A, T35_ID_B, "t35.e.ab");
    add_edge(T35_ID_B, T35_ID_A, "t35.e.ba");
    add_edge(T35_ID_B, T35_ID_C, "t35.e.bc");
    add_edge(T35_ID_C, T35_ID_B, "t35.e.cb");
    add_edge(T35_ID_A, T35_ID_C, "t35.e.ac");
    add_edge(T35_ID_C, T35_ID_A, "t35.e.ca");

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   3,          "k3: node_count=3");
    assert_eq!(ec,   3,          "k3: edge_count=3");
    assert_eq!(nnc,  786_432,    "k3: NNC=786_432 (3\u{00d7}262_144; 4\u{2079}=262_144; S-uniform S=4)");
    assert_eq!(nhoc, 50_331_648, "k3: NHOC=50_331_648 (3\u{00d7}16_777_216; (4+4)\u{2078}=8\u{2078}=16_777_216; S-uniform S=4)");
    assert_eq!(nhso, 98_304,     "k3: NHSO=98_304 (3\u{00d7}32_768; (16+16)\u{00b3}=32\u{00b3}=32_768; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHOC (16_777_216) and NHSO (32_768) as K₃; NNC and totals differ.
// NNC:  5×4^9 = 5×262_144 = 1_310_720.
// NHOC: 4×8^8 = 4×16_777_216 = 67_108_864.
// NHSO: 4×32^3 = 4×32_768 = 131_072.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);
    add_node(T35_VEC_C, T35_KEY_C, T35_ID_C);
    add_node(T35_VEC_D, T35_KEY_D, T35_ID_D);
    add_node(T35_VEC_E, T35_KEY_E, T35_ID_E);
    add_edge(T35_ID_A, T35_ID_B, "t35.e.ab");
    add_edge(T35_ID_A, T35_ID_C, "t35.e.ac");
    add_edge(T35_ID_A, T35_ID_D, "t35.e.ad");
    add_edge(T35_ID_A, T35_ID_E, "t35.e.ae");

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(nnc,  1_310_720,  "star: NNC=1_310_720 (5\u{00d7}262_144; same S as K\u{2083})");
    assert_eq!(nhoc, 67_108_864, "star: NHOC=67_108_864 (4\u{00d7}16_777_216; same per-edge as K\u{2083})");
    assert_eq!(nhso, 131_072,    "star: NHSO=131_072 (4\u{00d7}32_768; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NNC:  2^9+3^9+3^9+2^9 = 512+19_683+19_683+512 = 40_390.
// NHOC: (2+3)^8+(3+3)^8+(3+2)^8 = 5^8+6^8+5^8 = 390_625+1_679_616+390_625 = 2_460_866.
// NHSO: (4+9)^3+(9+9)^3+(9+4)^3 = 13^3+18^3+13^3 = 2_197+5_832+2_197 = 10_226.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);
    add_node(T35_VEC_C, T35_KEY_C, T35_ID_C);
    add_node(T35_VEC_D, T35_KEY_D, T35_ID_D);
    add_edge(T35_ID_A, T35_ID_B, "t35.e.ab");
    add_edge(T35_ID_B, T35_ID_C, "t35.e.bc");
    add_edge(T35_ID_C, T35_ID_D, "t35.e.cd");

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(nnc,  40_390,    "p4: NNC=40_390 (512+19_683+19_683+512; 2\u{2079}+3\u{2079}+3\u{2079}+2\u{2079})");
    assert_eq!(nhoc, 2_460_866, "p4: NHOC=2_460_866 (390_625+1_679_616+390_625; 5\u{2078}+6\u{2078}+5\u{2078})");
    assert_eq!(nhso, 10_226,    "p4: NHSO=10_226 (2_197+5_832+2_197; 13\u{00b3}+18\u{00b3}+13\u{00b3})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NNC:  4×9^9 = 4×387_420_489 = 1_549_681_956.
// NHOC: 6×18^8 = 6×11_019_960_576 = 66_119_763_456.
// NHSO: 6×(81+81)^3 = 6×162^3 = 6×4_251_528 = 25_509_168.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);
    add_node(T35_VEC_C, T35_KEY_C, T35_ID_C);
    add_node(T35_VEC_D, T35_KEY_D, T35_ID_D);
    add_edge(T35_ID_A, T35_ID_B, "t35.e.ab");
    add_edge(T35_ID_B, T35_ID_A, "t35.e.ba");
    add_edge(T35_ID_A, T35_ID_C, "t35.e.ac");
    add_edge(T35_ID_C, T35_ID_A, "t35.e.ca");
    add_edge(T35_ID_A, T35_ID_D, "t35.e.ad");
    add_edge(T35_ID_D, T35_ID_A, "t35.e.da");
    add_edge(T35_ID_B, T35_ID_C, "t35.e.bc");
    add_edge(T35_ID_C, T35_ID_B, "t35.e.cb");
    add_edge(T35_ID_B, T35_ID_D, "t35.e.bd");
    add_edge(T35_ID_D, T35_ID_B, "t35.e.db");
    add_edge(T35_ID_C, T35_ID_D, "t35.e.cd");
    add_edge(T35_ID_D, T35_ID_C, "t35.e.dc");

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   4,                "k4: node_count=4");
    assert_eq!(ec,   6,                "k4: edge_count=6");
    assert_eq!(nnc,  1_549_681_956,    "k4: NNC=1_549_681_956 (4\u{00d7}387_420_489; 9\u{2079}=387_420_489; S-uniform S=9)");
    assert_eq!(nhoc, 66_119_763_456,   "k4: NHOC=66_119_763_456 (6\u{00d7}11_019_960_576; 18\u{2078}=11_019_960_576; S-uniform S=9)");
    assert_eq!(nhso, 25_509_168,       "k4: NHSO=25_509_168 (6\u{00d7}4_251_528; 162\u{00b3}=4_251_528; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NNC=0; NHOC=0; NHSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nnc,  0, "isolated: NNC=0 (S=0; 0^9=0)");
    assert_eq!(nhoc, 0, "isolated: NHOC=0 (no edges)");
    assert_eq!(nhso, 0, "isolated: NHSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NNC:  5×6^9 = 5×10_077_696 = 50_388_480.
// NHOC: 6×12^8 = 6×429_981_696 = 2_579_890_176.
// NHSO: 6×(36+36)^3 = 6×72^3 = 6×373_248 = 2_239_488.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T35_VEC_A, T35_KEY_A, T35_ID_A);
    add_node(T35_VEC_B, T35_KEY_B, T35_ID_B);
    add_node(T35_VEC_C, T35_KEY_C, T35_ID_C);
    add_node(T35_VEC_D, T35_KEY_D, T35_ID_D);
    add_node(T35_VEC_E, T35_KEY_E, T35_ID_E);
    add_edge(T35_ID_A, T35_ID_C, "t35.e.ac");
    add_edge(T35_ID_C, T35_ID_A, "t35.e.ca");
    add_edge(T35_ID_A, T35_ID_D, "t35.e.ad");
    add_edge(T35_ID_D, T35_ID_A, "t35.e.da");
    add_edge(T35_ID_A, T35_ID_E, "t35.e.ae");
    add_edge(T35_ID_E, T35_ID_A, "t35.e.ea");
    add_edge(T35_ID_B, T35_ID_C, "t35.e.bc");
    add_edge(T35_ID_C, T35_ID_B, "t35.e.cb");
    add_edge(T35_ID_B, T35_ID_D, "t35.e.bd");
    add_edge(T35_ID_D, T35_ID_B, "t35.e.db");
    add_edge(T35_ID_B, T35_ID_E, "t35.e.be");
    add_edge(T35_ID_E, T35_ID_B, "t35.e.eb");

    let (nnc, nhoc, nhso, ec, nc) = gos_runtime::graph_topo_indices35();
    assert_eq!(nc,   5,             "k23: node_count=5");
    assert_eq!(ec,   6,             "k23: edge_count=6");
    assert_eq!(nnc,  50_388_480,    "k23: NNC=50_388_480 (5\u{00d7}10_077_696; 6\u{2079}=10_077_696; S-uniform S=6)");
    assert_eq!(nhoc, 2_579_890_176, "k23: NHOC=2_579_890_176 (6\u{00d7}429_981_696; 12\u{2078}=429_981_696; S-uniform S=6)");
    assert_eq!(nhso, 2_239_488,     "k23: NHSO=2_239_488 (6\u{00d7}373_248; 72\u{00b3}=373_248; S-uniform S=6)");
}
