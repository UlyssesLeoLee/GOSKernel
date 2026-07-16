// gos-graph-topo33-harness — V3.44 NSHP + NHSE + NCSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices33()`:
//   Returns (nshp, nhse, ncso_ppm, edge_count, node_count)
//   - nshp     = NSHP(G) = Σ_v S(v)^7                              (exact u64; S-heptic vertex sum)
//   - nhse     = NHSE(G) = Σ_{uv∈E} (S_u+S_v)^6                   (exact u64; S-sextic edge-sum)
//   - ncso_ppm = NCSO(G) × 10^6 = Σ_{uv∈E} (S_u²+S_v²)^{3/2}·10^6 (floor ppm; S-Cubic Sombor)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NSHP(G) = Σ_v S(v)^7
//     S-heptic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31),
//       NSH=Σ S⁶ (topo32), NSHP=Σ S⁷ (topo33).
//     NSHP = n·S^7 for S-regular.
//     Overflow: S^7 ≤ 16129^7 ≈ 2.84×10^28 > u64::MAX → u128 accumulator, clamp to u64::MAX.
//
//   NHSE(G) = Σ_{uv∈E} (S_u+S_v)^6
//     S-sextic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31),
//       NHPS=Σ(S+S)⁵ (topo32), NHSE=Σ(S+S)⁶ (topo33).
//     NHSE = |E|·(2S)^6 = 64|E|S^6 for S-regular.
//     Overflow: (2×16129)^6 ≈ 1.24×10^27 > u64::MAX → u128 accumulator, clamp.
//
//   NCSO(G) × 10^6 = Σ_{uv∈E} (S_u²+S_v²)^{3/2} × 10^6   (floor ppm)
//     S-Cubic Sombor: the S-variant generalised Sombor index with exponent α=3.
//     NSO(topo21) = Σ√(S_u²+S_v²) = generalized SO with α=1 on S-variant.
//     NCSO = Σ (S_u²+S_v²)·√(S_u²+S_v²) = generalized SO with α=3 on S-variant.
//     NCSO = |E|·(2S²)^{3/2}·10^6 = |E|·2√2·S³·10^6 for S-regular.
//     Implementation: per edge = isqrt128((S_u²+S_v²)^3 · 10^12).
//     Overflow guard: (S_u²+S_v²)^3 ≤ (2×16129²)^3 ≈ 1.41×10^26;
//       × 10^12 ≈ 1.41×10^38 < u128::MAX ✓.
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
//  Graph       NSHP(exact)    NHSE(exact)    NCSO(ppm)          edges  nodes
//  Empty                0              0              0              0      0
//  1 node               0              0              0              0      1
//  K₂                   2             64      2_828_427              1      2
//  P₃                 384          8_192     45_254_832              2      3
//  K₃              49_152        786_432    543_058_005              3      3
//  K_{1,4}         81_920      1_048_576    724_077_340              4      5
//  P₄               4_630         77_906    170_111_864              3      4
//  K₄          19_131_876    204_073_344 12_371_540_238              6      4
//  2 isolated           0              0              0              0      2
//  K_{2,3}      1_399_680     17_915_904  3_665_641_548              6      5
//
// DERIVATIONS (exact where possible, isqrt128 for NCSO ppm):
//
//   K₂ (S_A=S_B=1, 1 edge, 2 nodes):
//     NSHP: 1^7 + 1^7 = 2. ✓
//     NHSE: (1+1)^6 = 2^6 = 64. ✓
//     NCSO: isqrt128(2^3·10^12) = floor(√8·10^6) = floor(2√2·10^6) = 2_828_427. ✓
//
//   P₃ = A-B-C (S-uniform S=2, 2 edges, 3 nodes):
//     NSHP: 3 × 2^7 = 3 × 128 = 384. ✓
//     NHSE: 2 × (2+2)^6 = 2 × 4^6 = 2 × 4_096 = 8_192. ✓
//     NCSO: 2 × isqrt128(8^3·10^12) = 2 × isqrt128(512·10^12)
//           = 2 × floor(16√2·10^6) = 2 × 22_627_416 = 45_254_832. ✓
//           (16√2 = 22.627416998...; floor = 22_627_416)
//
//   K₃ (S-uniform S=4, 3 edges, 3 nodes):
//     NSHP: 3 × 4^7 = 3 × 16_384 = 49_152. ✓
//     NHSE: 3 × (4+4)^6 = 3 × 8^6 = 3 × 262_144 = 786_432. ✓
//     NCSO: 3 × isqrt128(32^3·10^12) = 3 × isqrt128(32_768·10^12)
//           = 3 × floor(128√2·10^6) = 3 × 181_019_335 = 543_058_005. ✓
//           (128√2 = 181.019335984...; floor = 181_019_335)
//
//   K_{1,4} (S-uniform S=4, 4 edges, 5 nodes):
//     NSHP: 5 × 4^7 = 5 × 16_384 = 81_920. ✓
//     NHSE: 4 × 8^6 = 4 × 262_144 = 1_048_576. ✓
//     NCSO: 4 × 181_019_335 = 724_077_340. ✓ (same per-edge as K₃; S-uniform S=4)
//
//   P₄ = A-B-C-D (S_A=2, S_B=3, S_C=3, S_D=2; 3 edges, 4 nodes):
//     NSHP: 2^7+3^7+3^7+2^7 = 128+2_187+2_187+128 = 4_630. ✓
//     NHSE: 5^6+6^6+5^6 = 15_625+46_656+15_625 = 77_906. ✓
//     NCSO: {A,B}: isqrt128(13^3·10^12) = floor(13√13·10^6)
//             13√13 = 46.872166...; floor = 46_872_166.
//           {B,C}: isqrt128(18^3·10^12) = floor(54√2·10^6)
//             54√2 = 76.367532...; floor = 76_367_532.
//           {C,D}: same as {A,B} = 46_872_166.
//           Total: 46_872_166 + 76_367_532 + 46_872_166 = 170_111_864. ✓
//
//   K₄ (S-uniform S=9, 6 edges, 4 nodes):
//     NSHP: 4 × 9^7 = 4 × 4_782_969 = 19_131_876. ✓
//     NHSE: 6 × 18^6 = 6 × 34_012_224 = 204_073_344. ✓ (18^6: 18^2=324; 18^3=5832; 18^6=34_012_224)
//     NCSO: 6 × isqrt128(162^3·10^12) = 6 × floor(1458√2·10^6)
//           1458√2 = 2061.923374...; floor = 2_061_923_373.
//           Total: 6 × 2_061_923_373 = 12_371_540_238. ✓
//     (Verify: 162^3=4_251_528=2×1458^2; √4_251_528=1458√2 ✓)
//
//   K_{2,3} (S-uniform S=6, 6 edges, 5 nodes):
//     NSHP: 5 × 6^7 = 5 × 279_936 = 1_399_680. ✓
//     NHSE: 6 × 12^6 = 6 × 2_985_984 = 17_915_904. ✓ (12^6: 12^2=144; 12^3=1728; 12^6=2_985_984)
//     NCSO: 6 × isqrt128(72^3·10^12) = 6 × floor(432√2·10^6)
//           432√2 = 610.940258...; floor = 610_940_258.
//           Total: 6 × 610_940_258 = 3_665_641_548. ✓
//     (72^3=373_248=2×432^2; √373_248=432√2 ✓)
//
// NCSO S-regular formula verification:
//   NCSO_per_edge = floor((2S²)^{3/2}·10^6) = floor(2√2·S³·10^6)
//   S=1: 2√2·10^6 = 2_828_427.1... → 2_828_427 ✓
//   S=2: 2√2·8·10^6 = 16√2·10^6 = 22_627_416.9... → 22_627_416 ✓
//   S=4: 2√2·64·10^6 = 128√2·10^6 = 181_019_335.9... → 181_019_335 ✓
//   S=9: 2√2·729·10^6 = 1458√2·10^6 = 2_061_923_373.9... → 2_061_923_373 ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 64, 2_828_427, 1, 2)
//  4.  Path P₃ = A-B-C                   → (384, 8_192, 45_254_832, 2, 3)
//  5.  Triangle K₃                       → (49_152, 786_432, 543_058_005, 3, 3)
//  6.  Star K_{1,4}                      → (81_920, 1_048_576, 724_077_340, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (4_630, 77_906, 170_111_864, 3, 4)
//  8.  Complete K₄                       → (19_131_876, 204_073_344, 12_371_540_238, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (1_399_680, 17_915_904, 3_665_641_548, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T33_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_33");
const T33_EXEC:   ExecutorId = ExecutorId::from_ascii("t33.exec");

const T33_KEY_A: &str = "t33.alpha";
const T33_KEY_B: &str = "t33.beta";
const T33_KEY_C: &str = "t33.gamma";
const T33_KEY_D: &str = "t33.delta";
const T33_KEY_E: &str = "t33.epsilon";

const T33_ID_A: NodeId = derive_node_id(T33_PLUGIN, T33_KEY_A);
const T33_ID_B: NodeId = derive_node_id(T33_PLUGIN, T33_KEY_B);
const T33_ID_C: NodeId = derive_node_id(T33_PLUGIN, T33_KEY_C);
const T33_ID_D: NodeId = derive_node_id(T33_PLUGIN, T33_KEY_D);
const T33_ID_E: NodeId = derive_node_id(T33_PLUGIN, T33_KEY_E);

// L4=120 namespace for this harness.
const T33_VEC_A: VectorAddress = VectorAddress::new(120, 1, 1, 0);
const T33_VEC_B: VectorAddress = VectorAddress::new(120, 1, 2, 0);
const T33_VEC_C: VectorAddress = VectorAddress::new(120, 1, 3, 0);
const T33_VEC_D: VectorAddress = VectorAddress::new(120, 2, 1, 0);
const T33_VEC_E: VectorAddress = VectorAddress::new(120, 2, 2, 0);

const T33_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T33_PLUGIN,
    name:         "kl-graph-topo33-harness",
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
        executor_id:       T33_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T33_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T33_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nshp, 0, "empty: NSHP=0");
    assert_eq!(nhse, 0, "empty: NHSE=0");
    assert_eq!(ncso, 0, "empty: NCSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NSHP: 0^7=0; NHSE: no edges; NCSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nshp, 0, "single: NSHP=0 (S=0; 0^7=0)");
    assert_eq!(nhse, 0, "single: NHSE=0 (no edges)");
    assert_eq!(ncso, 0, "single: NCSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NSHP: 1^7+1^7 = 2.
// NHSE: (1+1)^6 = 64.
// NCSO: floor(√(2^3)·10^6) = floor(2√2·10^6) = 2_828_427.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);
    add_edge(T33_ID_A, T33_ID_B, "t33.e.ab");

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nshp, 2,         "k2: NSHP=2 (1\u{2077}+1\u{2077}=2; S-uniform S=1)");
    assert_eq!(nhse, 64,        "k2: NHSE=64 ((1+1)\u{2076}=2\u{2076}=64; S-uniform S=1)");
    assert_eq!(ncso, 2_828_427, "k2: NCSO=2_828_427 (\u{230a}2\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NSHP: 3×128=384. NHSE: 2×4096=8192.
// NCSO: 2×floor(16√2·10^6) = 2×22_627_416 = 45_254_832.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);
    add_node(T33_VEC_C, T33_KEY_C, T33_ID_C);
    add_edge(T33_ID_A, T33_ID_B, "t33.e.ab");
    add_edge(T33_ID_B, T33_ID_C, "t33.e.bc");

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   3,          "p3: node_count=3");
    assert_eq!(ec,   2,          "p3: edge_count=2");
    assert_eq!(nshp, 384,        "p3: NSHP=384 (3\u{00d7}128; 2\u{2077}=128; S-uniform S=2)");
    assert_eq!(nhse, 8_192,      "p3: NHSE=8_192 (2\u{00d7}4096; (2+2)\u{2076}=4\u{2076}=4096; S-uniform S=2)");
    assert_eq!(ncso, 45_254_832, "p3: NCSO=45_254_832 (2\u{00d7}22_627_416; \u{230a}16\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NSHP: 3×16_384=49_152. NHSE: 3×262_144=786_432.
// NCSO: 3×floor(128√2·10^6) = 3×181_019_335 = 543_058_005.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);
    add_node(T33_VEC_C, T33_KEY_C, T33_ID_C);
    add_edge(T33_ID_A, T33_ID_B, "t33.e.ab");
    add_edge(T33_ID_B, T33_ID_A, "t33.e.ba");
    add_edge(T33_ID_B, T33_ID_C, "t33.e.bc");
    add_edge(T33_ID_C, T33_ID_B, "t33.e.cb");
    add_edge(T33_ID_A, T33_ID_C, "t33.e.ac");
    add_edge(T33_ID_C, T33_ID_A, "t33.e.ca");

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   3,           "k3: node_count=3");
    assert_eq!(ec,   3,           "k3: edge_count=3");
    assert_eq!(nshp, 49_152,      "k3: NSHP=49_152 (3\u{00d7}16_384; 4\u{2077}=16_384; S-uniform S=4)");
    assert_eq!(nhse, 786_432,     "k3: NHSE=786_432 (3\u{00d7}262_144; (4+4)\u{2076}=8\u{2076}=262_144; S-uniform S=4)");
    assert_eq!(ncso, 543_058_005, "k3: NCSO=543_058_005 (3\u{00d7}181_019_335; \u{230a}128\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHSE (262_144) and NCSO (181_019_335) as K₃; NSHP and totals differ.
// NSHP: 5×16_384=81_920. NHSE: 4×262_144=1_048_576.
// NCSO: 4×181_019_335 = 724_077_340.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);
    add_node(T33_VEC_C, T33_KEY_C, T33_ID_C);
    add_node(T33_VEC_D, T33_KEY_D, T33_ID_D);
    add_node(T33_VEC_E, T33_KEY_E, T33_ID_E);
    add_edge(T33_ID_A, T33_ID_B, "t33.e.ab");
    add_edge(T33_ID_A, T33_ID_C, "t33.e.ac");
    add_edge(T33_ID_A, T33_ID_D, "t33.e.ad");
    add_edge(T33_ID_A, T33_ID_E, "t33.e.ae");

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   5,           "star: node_count=5");
    assert_eq!(ec,   4,           "star: edge_count=4");
    assert_eq!(nshp, 81_920,      "star: NSHP=81_920 (5\u{00d7}16_384; same S as K\u{2083})");
    assert_eq!(nhse, 1_048_576,   "star: NHSE=1_048_576 (4\u{00d7}262_144; same per-edge as K\u{2083})");
    assert_eq!(ncso, 724_077_340, "star: NCSO=724_077_340 (4\u{00d7}181_019_335; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NSHP: 128+2187+2187+128 = 4630.
// NHSE: 5^6+6^6+5^6 = 15625+46656+15625 = 77906.
// NCSO: floor(13√13·10^6)+floor(54√2·10^6)+floor(13√13·10^6)
//       = 46_872_166 + 76_367_532 + 46_872_166 = 170_111_864.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);
    add_node(T33_VEC_C, T33_KEY_C, T33_ID_C);
    add_node(T33_VEC_D, T33_KEY_D, T33_ID_D);
    add_edge(T33_ID_A, T33_ID_B, "t33.e.ab");
    add_edge(T33_ID_B, T33_ID_C, "t33.e.bc");
    add_edge(T33_ID_C, T33_ID_D, "t33.e.cd");

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   4,           "p4: node_count=4");
    assert_eq!(ec,   3,           "p4: edge_count=3");
    assert_eq!(nshp, 4_630,       "p4: NSHP=4_630 (128+2187+2187+128; 2\u{2077}+3\u{2077}+3\u{2077}+2\u{2077})");
    assert_eq!(nhse, 77_906,      "p4: NHSE=77_906 (15625+46656+15625; 5\u{2076}+6\u{2076}+5\u{2076})");
    assert_eq!(ncso, 170_111_864, "p4: NCSO=170_111_864 (46_872_166+76_367_532+46_872_166; \u{230a}13\u{221a}13\u{00b7}10\u{2076}\u{230b}+\u{230a}54\u{221a}2\u{00b7}10\u{2076}\u{230b}+\u{230a}13\u{221a}13\u{00b7}10\u{2076}\u{230b})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NSHP: 4×9^7 = 4×4_782_969 = 19_131_876.
// NHSE: 6×18^6 = 6×34_012_224 = 204_073_344.
// NCSO: 6×floor(1458√2·10^6) = 6×2_061_923_373 = 12_371_540_238.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);
    add_node(T33_VEC_C, T33_KEY_C, T33_ID_C);
    add_node(T33_VEC_D, T33_KEY_D, T33_ID_D);
    add_edge(T33_ID_A, T33_ID_B, "t33.e.ab");
    add_edge(T33_ID_B, T33_ID_A, "t33.e.ba");
    add_edge(T33_ID_A, T33_ID_C, "t33.e.ac");
    add_edge(T33_ID_C, T33_ID_A, "t33.e.ca");
    add_edge(T33_ID_A, T33_ID_D, "t33.e.ad");
    add_edge(T33_ID_D, T33_ID_A, "t33.e.da");
    add_edge(T33_ID_B, T33_ID_C, "t33.e.bc");
    add_edge(T33_ID_C, T33_ID_B, "t33.e.cb");
    add_edge(T33_ID_B, T33_ID_D, "t33.e.bd");
    add_edge(T33_ID_D, T33_ID_B, "t33.e.db");
    add_edge(T33_ID_C, T33_ID_D, "t33.e.cd");
    add_edge(T33_ID_D, T33_ID_C, "t33.e.dc");

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   4,              "k4: node_count=4");
    assert_eq!(ec,   6,              "k4: edge_count=6");
    assert_eq!(nshp, 19_131_876,     "k4: NSHP=19_131_876 (4\u{00d7}4_782_969; 9\u{2077}=4_782_969; S-uniform S=9)");
    assert_eq!(nhse, 204_073_344,    "k4: NHSE=204_073_344 (6\u{00d7}34_012_224; 18\u{2076}=34_012_224; S-uniform S=9)");
    assert_eq!(ncso, 12_371_540_238, "k4: NCSO=12_371_540_238 (6\u{00d7}2_061_923_373; \u{230a}1458\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NSHP=0; NHSE=0; NCSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nshp, 0, "isolated: NSHP=0 (S=0; 0^7=0)");
    assert_eq!(nhse, 0, "isolated: NHSE=0 (no edges)");
    assert_eq!(ncso, 0, "isolated: NCSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NSHP: 5×6^7 = 5×279_936 = 1_399_680.
// NHSE: 6×12^6 = 6×2_985_984 = 17_915_904.
// NCSO: 6×floor(432√2·10^6) = 6×610_940_258 = 3_665_641_548.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T33_VEC_A, T33_KEY_A, T33_ID_A);
    add_node(T33_VEC_B, T33_KEY_B, T33_ID_B);
    add_node(T33_VEC_C, T33_KEY_C, T33_ID_C);
    add_node(T33_VEC_D, T33_KEY_D, T33_ID_D);
    add_node(T33_VEC_E, T33_KEY_E, T33_ID_E);
    add_edge(T33_ID_A, T33_ID_C, "t33.e.ac");
    add_edge(T33_ID_C, T33_ID_A, "t33.e.ca");
    add_edge(T33_ID_A, T33_ID_D, "t33.e.ad");
    add_edge(T33_ID_D, T33_ID_A, "t33.e.da");
    add_edge(T33_ID_A, T33_ID_E, "t33.e.ae");
    add_edge(T33_ID_E, T33_ID_A, "t33.e.ea");
    add_edge(T33_ID_B, T33_ID_C, "t33.e.bc");
    add_edge(T33_ID_C, T33_ID_B, "t33.e.cb");
    add_edge(T33_ID_B, T33_ID_D, "t33.e.bd");
    add_edge(T33_ID_D, T33_ID_B, "t33.e.db");
    add_edge(T33_ID_B, T33_ID_E, "t33.e.be");
    add_edge(T33_ID_E, T33_ID_B, "t33.e.eb");

    let (nshp, nhse, ncso, ec, nc) = gos_runtime::graph_topo_indices33();
    assert_eq!(nc,   5,             "k23: node_count=5");
    assert_eq!(ec,   6,             "k23: edge_count=6");
    assert_eq!(nshp, 1_399_680,     "k23: NSHP=1_399_680 (5\u{00d7}279_936; 6\u{2077}=279_936; S-uniform S=6)");
    assert_eq!(nhse, 17_915_904,    "k23: NHSE=17_915_904 (6\u{00d7}2_985_984; 12\u{2076}=2_985_984; S-uniform S=6)");
    assert_eq!(ncso, 3_665_641_548, "k23: NCSO=3_665_641_548 (6\u{00d7}610_940_258; \u{230a}432\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=6)");
}
