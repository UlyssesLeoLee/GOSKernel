// gos-graph-topo32-harness — V3.43 NSH + NHPS + NWSO (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices32()`:
//   Returns (nsh, nhps, nwso_ppm, edge_count, node_count)
//   - nsh      = NSH(G)  = Σ_v S(v)^6                            (exact u64; S-hextic vertex sum)
//   - nhps     = NHPS(G) = Σ_{uv∈E} (S_u+S_v)^5                 (exact u64; S-quintic edge-sum)
//   - nwso_ppm = NWSO(G) × 10^6 = Σ_{uv∈E} S_u·S_v·√(S_u²+S_v²) × 10^6 (floor ppm; S-Weighted Sombor)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NSH(G) = Σ_v S(v)^6
//     S-hextic vertex sum; extends the S-power-vertex series:
//       NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30), NPS=Σ S⁵ (topo31), NSH=Σ S⁶ (topo32).
//     NSH = n·S^6 for S-regular.
//     Overflow: S^6 ≤ 16129^6 ≈ 1.76×10^25 > u64::MAX → u128 accumulator, clamp to u64::MAX.
//
//   NHPS(G) = Σ_{uv∈E} (S_u+S_v)^5
//     S-quintic edge-sum; extends the S-power-edge series:
//       NHM1=Σ(S+S)² (topo23), NHCS=Σ(S+S)³ (topo30), NHQS=Σ(S+S)⁴ (topo31), NHPS=Σ(S+S)⁵ (topo32).
//     NHPS = |E|·(2S)^5 = 32|E|S^5 for S-regular.
//     Overflow: (2×16129)^5 ≈ 3.49×10^22 > u64::MAX → u128 accumulator, clamp.
//
//   NWSO(G) × 10^6 = Σ_{uv∈E} S_u·S_v·√(S_u²+S_v²) × 10^6   (floor ppm)
//     S-Weighted Sombor index: each Sombor edge term √(S_u²+S_v²) is weighted by S_u·S_v.
//     NSO(topo21) = Σ √(S_u²+S_v²); NWSO = Σ S_u·S_v·√(S_u²+S_v²).
//     NWSO = |E|·S³·√2·10^6 for S-regular (= S²·NSO_per_edge).
//     Implementation: per edge = floor(√(S_u²·S_v²·(S_u²+S_v²)·10^12)) via isqrt128.
//     Overflow guard: S_u²·S_v²·(S_u²+S_v²) ≤ 2·16129^6 ≈ 3.52×10^25;
//       × 10^12 ≈ 3.52×10^37 < u128::MAX ✓.
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
//  Graph       NSH(exact)   NHPS(exact)   NWSO(ppm)          edges  nodes
//  Empty                0             0               0           0      0
//  1 node               0             0               0           0      1
//  K₂                   2            32       1_414_213           1      2
//  P₃                 192         2_048      22_627_416           2      3
//  K₃              12_288        98_304     271_529_001           3      3
//  K_{1,4}         20_480       131_072     362_038_668           4      5
//  P₄               1_586        14_026      81_450_380           3      4
//  K₄           2_125_764    11_337_408   6_185_770_116          6      4
//  2 isolated           0             0               0           0      2
//  K_{2,3}        233_280     1_492_992   1_832_820_774           6      5
//
// DERIVATIONS (exact where possible, isqrt128 for NWSO ppm):
//
//   K₂ (S_A=S_B=1, 1 edge, 2 nodes):
//     NSH:  1^6 + 1^6 = 2. ✓
//     NHPS: (1+1)^5 = 2^5 = 32. ✓
//     NWSO: isqrt128(1·1·2·10^12) = floor(√2·10^6) = 1_414_213. ✓
//
//   P₃ = A-B-C (S-uniform S=2, 2 edges, 3 nodes):
//     NSH:  3 × 2^6 = 3 × 64 = 192. ✓
//     NHPS: 2 × (2+2)^5 = 2 × 4^5 = 2 × 1024 = 2048. ✓
//     NWSO: 2 × isqrt128(4·4·8·10^12) = 2 × isqrt128(128·10^12)
//           = 2 × floor(8√2·10^6) = 2 × 11_313_708 = 22_627_416. ✓
//           (8√2 = 11.31370849...; floor = 11_313_708)
//
//   K₃ (S-uniform S=4, 3 edges, 3 nodes):
//     NSH:  3 × 4^6 = 3 × 4096 = 12_288. ✓
//     NHPS: 3 × (4+4)^5 = 3 × 8^5 = 3 × 32768 = 98_304. ✓
//     NWSO: 3 × isqrt128(16·16·32·10^12) = 3 × isqrt128(8192·10^12)
//           = 3 × floor(64√2·10^6) = 3 × 90_509_667 = 271_529_001. ✓
//           (64√2 = 90.50966...; floor = 90_509_667)
//
//   K_{1,4} (S-uniform S=4, 4 edges, 5 nodes):
//     NSH:  5 × 4^6 = 5 × 4096 = 20_480. ✓
//     NHPS: 4 × (4+4)^5 = 4 × 32768 = 131_072. ✓
//     NWSO: 4 × 90_509_667 = 362_038_668. ✓ (same per-edge as K₃; S-uniform S=4)
//
//   P₄ = A-B-C-D (S_A=2, S_B=3, S_C=3, S_D=2; 3 edges, 4 nodes):
//     NSH:  2^6+3^6+3^6+2^6 = 64+729+729+64 = 1586. ✓
//     NHPS: 5^5+6^5+5^5 = 3125+7776+3125 = 14026. ✓
//     NWSO: {A,B}: isqrt128(4·9·13·10^12) = isqrt128(468·10^12) = floor(6√13·10^6)
//             6√13 = 21.63330...; floor = 21_633_307.
//           {B,C}: isqrt128(9·9·18·10^12) = isqrt128(1458·10^12) = floor(27√2·10^6)
//             27√2 = 38.18376...; floor = 38_183_766.
//           {C,D}: same as {A,B} = 21_633_307.
//           Total: 21_633_307 + 38_183_766 + 21_633_307 = 81_450_380. ✓
//
//   K₄ (S-uniform S=9, 6 edges, 4 nodes):
//     NSH:  4 × 9^6 = 4 × 531_441 = 2_125_764. ✓
//     NHPS: 6 × 18^5 = 6 × 1_889_568 = 11_337_408. ✓ (18^5: 18^2=324; 18^4=104976; 18^5=1_889_568)
//     NWSO: 6 × isqrt128(81·81·162·10^12) = 6 × isqrt128(1_062_882·10^12)
//           = 6 × floor(729√2·10^6)
//           729√2 = 1030.9616869700...; floor = 1_030_961_686.
//           Total: 6 × 1_030_961_686 = 6_185_770_116. ✓
//     (Verify: 1_062_882 = 2·729^2 = 2·3^12; √1_062_882 = 729√2 ✓)
//
//   K_{2,3} (S-uniform S=6, 6 edges, 5 nodes):
//     NSH:  5 × 6^6 = 5 × 46_656 = 233_280. ✓
//     NHPS: 6 × 12^5 = 6 × 248_832 = 1_492_992. ✓ (12^5: 12^2=144; 12^4=20736; 12^5=248832)
//     NWSO: 6 × isqrt128(36·36·72·10^12) = 6 × isqrt128(93312·10^12)
//           = 6 × floor(216√2·10^6)
//           216√2 = 305.470129...; floor = 305_470_129.
//           Total: 6 × 305_470_129 = 1_832_820_774. ✓
//     (93312 = 2·216^2 = 2·6^6; √93312 = 216√2 ✓)
//
// NWSO ppm verification helper (S-regular case):
//   NWSO_per_edge = isqrt128(S^4 · 2 · 10^12) = floor(S^2·√2·10^6)
//   For S=1: 1_414_213; S=2: 2^2·√2·10^6 = 4·1_414_213.562=5_656_854.24 → but per-edge
//     isqrt128(4·4·8·10^12) = isqrt128(128·10^12) — wait, S_u=S_v=S →
//     isqrt128(S_u²·S_v²·(S_u²+S_v²)·10^12) = isqrt128(S^4·2S^2·10^12) = isqrt128(2S^6·10^12)
//   For S=1: isqrt128(2·10^12)=1_414_213; for S=2: isqrt128(128·10^12)=11_313_708;
//   for S=4: isqrt128(8192·10^12)=90_509_667; for S=6: isqrt128(93312·10^12)=305_470_129;
//   for S=9: isqrt128(1_062_882·10^12)=1_030_961_686.
//   Formula: floor(S^3·√2·10^6). Verifying S=9: 9^3=729; 729√2=1030.961687; ×10^6=1_030_961_686 ✓.
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2, 32, 1_414_213, 1, 2)
//  4.  Path P₃ = A-B-C                   → (192, 2_048, 22_627_416, 2, 3)
//  5.  Triangle K₃                       → (12_288, 98_304, 271_529_001, 3, 3)
//  6.  Star K_{1,4}                      → (20_480, 131_072, 362_038_668, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (1_586, 14_026, 81_450_380, 3, 4)
//  8.  Complete K₄                       → (2_125_764, 11_337_408, 6_185_770_116, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (233_280, 1_492_992, 1_832_820_774, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T32_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_32");
const T32_EXEC:   ExecutorId = ExecutorId::from_ascii("t32.exec");

const T32_KEY_A: &str = "t32.alpha";
const T32_KEY_B: &str = "t32.beta";
const T32_KEY_C: &str = "t32.gamma";
const T32_KEY_D: &str = "t32.delta";
const T32_KEY_E: &str = "t32.epsilon";

const T32_ID_A: NodeId = derive_node_id(T32_PLUGIN, T32_KEY_A);
const T32_ID_B: NodeId = derive_node_id(T32_PLUGIN, T32_KEY_B);
const T32_ID_C: NodeId = derive_node_id(T32_PLUGIN, T32_KEY_C);
const T32_ID_D: NodeId = derive_node_id(T32_PLUGIN, T32_KEY_D);
const T32_ID_E: NodeId = derive_node_id(T32_PLUGIN, T32_KEY_E);

// L4=119 namespace for this harness.
const T32_VEC_A: VectorAddress = VectorAddress::new(119, 1, 1, 0);
const T32_VEC_B: VectorAddress = VectorAddress::new(119, 1, 2, 0);
const T32_VEC_C: VectorAddress = VectorAddress::new(119, 1, 3, 0);
const T32_VEC_D: VectorAddress = VectorAddress::new(119, 2, 1, 0);
const T32_VEC_E: VectorAddress = VectorAddress::new(119, 2, 2, 0);

const T32_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T32_PLUGIN,
    name:         "kl-graph-topo32-harness",
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
        executor_id:       T32_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T32_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T32_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nsh,  0, "empty: NSH=0");
    assert_eq!(nhps, 0, "empty: NHPS=0");
    assert_eq!(nwso, 0, "empty: NWSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NSH: 0^6=0; NHPS: no edges; NWSO: no edges.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nsh,  0, "single: NSH=0 (S=0; 0^6=0)");
    assert_eq!(nhps, 0, "single: NHPS=0 (no edges)");
    assert_eq!(nwso, 0, "single: NWSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. S-uniform S=1.
// NSH:  1^6+1^6 = 2.
// NHPS: (1+1)^5 = 32.
// NWSO: floor(√(1·1·2)·10^6) = floor(√2·10^6) = 1_414_213.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);
    add_edge(T32_ID_A, T32_ID_B, "t32.e.ab");

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nsh,  2,         "k2: NSH=2 (1\u{2076}+1\u{2076}=2; S-uniform S=1)");
    assert_eq!(nhps, 32,        "k2: NHPS=32 ((1+1)\u{2075}=2\u{2075}=32; S-uniform S=1)");
    assert_eq!(nwso, 1_414_213, "k2: NWSO=1_414_213 (\u{230a}\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NSH:  3×64=192. NHPS: 2×1024=2048.
// NWSO: 2×floor(8√2·10^6) = 2×11_313_708 = 22_627_416.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);
    add_node(T32_VEC_C, T32_KEY_C, T32_ID_C);
    add_edge(T32_ID_A, T32_ID_B, "t32.e.ab");
    add_edge(T32_ID_B, T32_ID_C, "t32.e.bc");

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   3,          "p3: node_count=3");
    assert_eq!(ec,   2,          "p3: edge_count=2");
    assert_eq!(nsh,  192,        "p3: NSH=192 (3\u{00d7}64; 2\u{2076}=64; S-uniform S=2)");
    assert_eq!(nhps, 2_048,      "p3: NHPS=2_048 (2\u{00d7}1024; (2+2)\u{2075}=4\u{2075}=1024; S-uniform S=2)");
    assert_eq!(nwso, 22_627_416, "p3: NWSO=22_627_416 (2\u{00d7}11_313_708; \u{230a}8\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=2)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NSH:  3×4096=12_288. NHPS: 3×32768=98_304.
// NWSO: 3×floor(64√2·10^6) = 3×90_509_667 = 271_529_001.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);
    add_node(T32_VEC_C, T32_KEY_C, T32_ID_C);
    add_edge(T32_ID_A, T32_ID_B, "t32.e.ab");
    add_edge(T32_ID_B, T32_ID_A, "t32.e.ba");
    add_edge(T32_ID_B, T32_ID_C, "t32.e.bc");
    add_edge(T32_ID_C, T32_ID_B, "t32.e.cb");
    add_edge(T32_ID_A, T32_ID_C, "t32.e.ac");
    add_edge(T32_ID_C, T32_ID_A, "t32.e.ca");

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   3,           "k3: node_count=3");
    assert_eq!(ec,   3,           "k3: edge_count=3");
    assert_eq!(nsh,  12_288,      "k3: NSH=12_288 (3\u{00d7}4096; 4\u{2076}=4096; S-uniform S=4)");
    assert_eq!(nhps, 98_304,      "k3: NHPS=98_304 (3\u{00d7}32768; (4+4)\u{2075}=8\u{2075}=32768; S-uniform S=4)");
    assert_eq!(nwso, 271_529_001, "k3: NWSO=271_529_001 (3\u{00d7}90_509_667; \u{230a}64\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=4)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-edge NHPS (32768) and NWSO (90_509_667) as K₃; NSH and totals differ by node/edge count.
// NSH:  5×4096=20_480. NHPS: 4×32768=131_072.
// NWSO: 4×90_509_667 = 362_038_668.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);
    add_node(T32_VEC_C, T32_KEY_C, T32_ID_C);
    add_node(T32_VEC_D, T32_KEY_D, T32_ID_D);
    add_node(T32_VEC_E, T32_KEY_E, T32_ID_E);
    add_edge(T32_ID_A, T32_ID_B, "t32.e.ab");
    add_edge(T32_ID_A, T32_ID_C, "t32.e.ac");
    add_edge(T32_ID_A, T32_ID_D, "t32.e.ad");
    add_edge(T32_ID_A, T32_ID_E, "t32.e.ae");

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   5,           "star: node_count=5");
    assert_eq!(ec,   4,           "star: edge_count=4");
    assert_eq!(nsh,  20_480,      "star: NSH=20_480 (5\u{00d7}4096; same S as K\u{2083})");
    assert_eq!(nhps, 131_072,     "star: NHPS=131_072 (4\u{00d7}32768; same per-edge as K\u{2083})");
    assert_eq!(nwso, 362_038_668, "star: NWSO=362_038_668 (4\u{00d7}90_509_667; same per-edge as K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2. S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NSH:  64+729+729+64 = 1586.
// NHPS: 5^5+6^5+5^5 = 3125+7776+3125 = 14026.
// NWSO: floor(6√13·10^6)+floor(27√2·10^6)+floor(6√13·10^6)
//       = 21_633_307 + 38_183_766 + 21_633_307 = 81_450_380.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);
    add_node(T32_VEC_C, T32_KEY_C, T32_ID_C);
    add_node(T32_VEC_D, T32_KEY_D, T32_ID_D);
    add_edge(T32_ID_A, T32_ID_B, "t32.e.ab");
    add_edge(T32_ID_B, T32_ID_C, "t32.e.bc");
    add_edge(T32_ID_C, T32_ID_D, "t32.e.cd");

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   4,          "p4: node_count=4");
    assert_eq!(ec,   3,          "p4: edge_count=3");
    assert_eq!(nsh,  1_586,      "p4: NSH=1_586 (64+729+729+64; 2\u{2076}+3\u{2076}+3\u{2076}+2\u{2076})");
    assert_eq!(nhps, 14_026,     "p4: NHPS=14_026 (3125+7776+3125; 5\u{2075}+6\u{2075}+5\u{2075})");
    assert_eq!(nwso, 81_450_380, "p4: NWSO=81_450_380 (21_633_307+38_183_766+21_633_307; \u{230a}6\u{221a}13\u{00b7}10\u{2076}\u{230b}+\u{230a}27\u{221a}2\u{00b7}10\u{2076}\u{230b}+\u{230a}6\u{221a}13\u{00b7}10\u{2076}\u{230b})");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=9. S-uniform S=9. 4 nodes, 6 edges.
// NSH:  4×531_441 = 2_125_764.
// NHPS: 6×18^5 = 6×1_889_568 = 11_337_408.
// NWSO: 6×floor(729√2·10^6) = 6×1_030_961_686 = 6_185_770_116. ✓

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);
    add_node(T32_VEC_C, T32_KEY_C, T32_ID_C);
    add_node(T32_VEC_D, T32_KEY_D, T32_ID_D);
    add_edge(T32_ID_A, T32_ID_B, "t32.e.ab");
    add_edge(T32_ID_B, T32_ID_A, "t32.e.ba");
    add_edge(T32_ID_A, T32_ID_C, "t32.e.ac");
    add_edge(T32_ID_C, T32_ID_A, "t32.e.ca");
    add_edge(T32_ID_A, T32_ID_D, "t32.e.ad");
    add_edge(T32_ID_D, T32_ID_A, "t32.e.da");
    add_edge(T32_ID_B, T32_ID_C, "t32.e.bc");
    add_edge(T32_ID_C, T32_ID_B, "t32.e.cb");
    add_edge(T32_ID_B, T32_ID_D, "t32.e.bd");
    add_edge(T32_ID_D, T32_ID_B, "t32.e.db");
    add_edge(T32_ID_C, T32_ID_D, "t32.e.cd");
    add_edge(T32_ID_D, T32_ID_C, "t32.e.dc");

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   4,             "k4: node_count=4");
    assert_eq!(ec,   6,             "k4: edge_count=6");
    assert_eq!(nsh,  2_125_764,     "k4: NSH=2_125_764 (4\u{00d7}531_441; 9\u{2076}=531_441; S-uniform S=9)");
    assert_eq!(nhps, 11_337_408,    "k4: NHPS=11_337_408 (6\u{00d7}1_889_568; 18\u{2075}=1_889_568; S-uniform S=9)");
    assert_eq!(nwso, 6_185_770_116, "k4: NWSO=6_185_770_116 (6\u{00d7}1_030_961_686; \u{230a}729\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=9)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NSH=0; NHPS=0; NWSO=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nsh,  0, "isolated: NSH=0 (S=0; 0^6=0)");
    assert_eq!(nhps, 0, "isolated: NHPS=0 (no edges)");
    assert_eq!(nwso, 0, "isolated: NWSO=0 (no edges)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NSH:  5×6^6 = 5×46_656 = 233_280.
// NHPS: 6×12^5 = 6×248_832 = 1_492_992.
// NWSO: 6×floor(216√2·10^6) = 6×305_470_129 = 1_832_820_774.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T32_VEC_A, T32_KEY_A, T32_ID_A);
    add_node(T32_VEC_B, T32_KEY_B, T32_ID_B);
    add_node(T32_VEC_C, T32_KEY_C, T32_ID_C);
    add_node(T32_VEC_D, T32_KEY_D, T32_ID_D);
    add_node(T32_VEC_E, T32_KEY_E, T32_ID_E);
    add_edge(T32_ID_A, T32_ID_C, "t32.e.ac");
    add_edge(T32_ID_C, T32_ID_A, "t32.e.ca");
    add_edge(T32_ID_A, T32_ID_D, "t32.e.ad");
    add_edge(T32_ID_D, T32_ID_A, "t32.e.da");
    add_edge(T32_ID_A, T32_ID_E, "t32.e.ae");
    add_edge(T32_ID_E, T32_ID_A, "t32.e.ea");
    add_edge(T32_ID_B, T32_ID_C, "t32.e.bc");
    add_edge(T32_ID_C, T32_ID_B, "t32.e.cb");
    add_edge(T32_ID_B, T32_ID_D, "t32.e.bd");
    add_edge(T32_ID_D, T32_ID_B, "t32.e.db");
    add_edge(T32_ID_B, T32_ID_E, "t32.e.be");
    add_edge(T32_ID_E, T32_ID_B, "t32.e.eb");

    let (nsh, nhps, nwso, ec, nc) = gos_runtime::graph_topo_indices32();
    assert_eq!(nc,   5,             "k23: node_count=5");
    assert_eq!(ec,   6,             "k23: edge_count=6");
    assert_eq!(nsh,  233_280,       "k23: NSH=233_280 (5\u{00d7}46_656; 6\u{2076}=46_656; S-uniform S=6)");
    assert_eq!(nhps, 1_492_992,     "k23: NHPS=1_492_992 (6\u{00d7}248_832; 12\u{2075}=248_832; S-uniform S=6)");
    assert_eq!(nwso, 1_832_820_774, "k23: NWSO=1_832_820_774 (6\u{00d7}305_470_129; \u{230a}216\u{221a}2\u{00b7}10\u{2076}\u{230b}; S-uniform S=6)");
}
