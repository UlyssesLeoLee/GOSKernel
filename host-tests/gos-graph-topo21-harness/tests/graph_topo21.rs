// gos-graph-topo21-harness — V3.32 ABC₄ + Neighborhood Harmonic NH + Neighborhood Sombor NSO
//
// Verifies `gos_runtime::graph_topo_indices21()`:
//   Returns (abc4_ppm, nh_ppm, nso_ppm, edge_count, node_count)
//   - abc4_ppm = ABC₄(G) × 10^6 = Σ_{uv∈E} √((S_u+S_v−2)/(S_u·S_v)) × 10^6   (floor ppm; 0 when S_u+S_v=2)
//   - nh_ppm   = NH(G)  × 10^6 = Σ_{uv∈E} 2/(S_u+S_v) × 10^6                  (floor ppm)
//   - nso_ppm  = NSO(G) × 10^6 = Σ_{uv∈E} √(S_u²+S_v²) × 10^6                 (floor ppm; u128 intermediate)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant" of degree).
//
// DEFINITIONS:
//   ABC₄(G) = Σ_{uv∈E} √((S_u+S_v−2)/(S_u·S_v))   (Ghorbani & Hosseinzadeh 2010; 4th-gen ABC)
//   NH(G)   = Σ_{uv∈E} 2/(S_u+S_v)                  (Neighborhood Harmonic; S-analogue of H)
//   NSO(G)  = Σ_{uv∈E} √(S_u²+S_v²)                 (Neighborhood Sombor; S-analogue of SO)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   ABC₄ per edge = isqrt64((ssum−2)×10^12 / (S_u·S_v))     [0 when ssum≤2]
//   NH   per edge = floor(2_000_000 / ssum)                   [exact integer division]
//   NSO  per edge = isqrt128((S_u²+S_v²)×10^12) as u64       [u128 intermediate; S²≤127⁴≈2.6×10^8]
//
// KEY INVARIANTS:
//   ABC₄=0 when S_u+S_v=2 for all edges → only for K₂ (S_A=S_B=1).
//   NH=NH(K_{1,k}) = k × floor(2×10^6 / (2k)) = 10^6 for all stars (S-uniform S=k).
//   NSO per edge = √(2)·S·10^6 when S-uniform (S_u=S_v=S).
//   ABC₄ per edge = √((2S−2)/(S²))·10^6 = √(2(S−1)/S²)·10^6 when S-uniform.
//   K₃ and K_{1,4} share S=4 on every endpoint → same per-edge values for all three indices.
//
// S VALUES PER GRAPH:
//   K₂        : S(each)=1               ssum=2  → ABC₄=0 (ssum not > 2)
//   P₃=A-B-C  : S(A)=S(B)=S(C)=2        ssum=4  (S-uniform)
//   K₃        : S(each)=4               ssum=8  (S-uniform)
//   K_{1,4}   : S(hub)=4, S(leaf)=4     ssum=8  (S-uniform; same as K₃ per edge!)
//   P₄=A-B-C-D: S(A)=S(D)=2, S(B)=S(C)=3  ssum∈{5,6}
//   K₄        : S(each)=9               ssum=18 (S-uniform)
//   K_{2,3}   : S(left,d=3)=6, S(right,d=2)=6  ssum=12 (S-uniform!)
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        ABC₄(ppm)  NH(ppm)    NSO(ppm)     edges  nodes
//  Empty              0          0           0         0      0
//  1 node             0          0           0         0      1
//  Edge K₂            0  1_000_000   1_414_213         1      2
//  Path P₃    1_414_212  1_000_000   5_656_854         2      3
//  Triangle K₃ 1_837_116    750_000  16_970_562         3      3
//  Star K_{1,4} 2_449_488 1_000_000  22_627_416         4      5
//  Path P₄    2_080_878  1_133_333  11_453_742         3      4
//  Complete K₄ 2_666_664    666_666  76_367_532         6      4
//  2 isolated         0          0           0         0      2
//  K_{2,3}    3_162_276    999_996  50_911_686         6      5
//
// Derivations (isqrt64 = floor Newton-Raphson; isqrt128 same for u128):
//
//   K₂ (S_A=S_B=1, ssum=2):
//     ABC₄: ssum=2 → NOT > 2, contribution=0. Total=0. ✓
//     NH:   floor(2_000_000/2)=1_000_000. Total=1_000_000. ✓
//     NSO:  isqrt128((1+1)×10^12)=isqrt128(2×10^12)=floor(√2·10^6)=1_414_213. Total=1_414_213. ✓
//
//   P₃=A-B-C (S_A=S_B=S_C=2, ssum=4; 2 edges; S-uniform):
//     Per edge ABC₄: isqrt64((4-2)×10^12/(2×2))=isqrt64(5×10^11)=floor(707_106.78)=707_106.
//     Per edge NH:   floor(2_000_000/4)=500_000.
//     Per edge NSO:  isqrt128(8×10^12)=floor(2√2·10^6)=2_828_427.
//     Total: (2×707_106, 2×500_000, 2×2_828_427)=(1_414_212, 1_000_000, 5_656_854). ✓
//
//   K₃ (S=4 for all, ssum=8; 3 edges; S-uniform):
//     Per edge ABC₄: isqrt64(6×10^12/16)=isqrt64(375×10^9)=floor(612_372.43)=612_372.
//     Per edge NH:   floor(2_000_000/8)=250_000.
//     Per edge NSO:  isqrt128(32×10^12)=floor(4√2·10^6)=5_656_854.
//     Total: (3×612_372, 3×250_000, 3×5_656_854)=(1_837_116, 750_000, 16_970_562). ✓
//
//   K_{1,4} (S_hub=4, S_leaf=4, ssum=8; 4 edges; S-uniform — identical per-edge to K₃!):
//     Per edge ABC₄: 612_372. Per edge NH: 250_000. Per edge NSO: 5_656_854.
//     Total: (4×612_372, 4×250_000, 4×5_656_854)=(2_449_488, 1_000_000, 22_627_416). ✓
//
//   P₄=A-B-C-D (S_A=S_D=2, S_B=S_C=3):
//     Edge A-B (ssum=5, S_A=2, S_B=3):
//       ABC₄: isqrt64(3×10^12/6)=isqrt64(5×10^11)=707_106.
//       NH:   floor(2_000_000/5)=400_000.
//       NSO:  isqrt128(13×10^12)=floor(√13·10^6)=3_605_551.
//     Edge B-C (ssum=6, S_B=3, S_C=3):
//       ABC₄: isqrt64(4×10^12/9)=isqrt64(444_444_444_444)=floor(666_666.67)=666_666.
//       NH:   floor(2_000_000/6)=333_333.
//       NSO:  isqrt128(18×10^12)=floor(3√2·10^6)=4_242_640.
//     Edge C-D (ssum=5, S_C=3, S_D=2): same as A-B by symmetry.
//     Total: (707_106+666_666+707_106, 400_000+333_333+400_000, 3_605_551+4_242_640+3_605_551)
//          = (2_080_878, 1_133_333, 11_453_742). ✓
//
//   K₄ (S=9 for all, ssum=18; 6 edges; S-uniform):
//     Per edge ABC₄: isqrt64(16×10^12/81)=isqrt64(197_530_864_197)=floor(444_444.44)=444_444.
//     Per edge NH:   floor(2_000_000/18)=111_111.
//     Per edge NSO:  isqrt128(162×10^12)=floor(9√2·10^6)=12_727_922.
//     Total: (6×444_444, 6×111_111, 6×12_727_922)=(2_666_664, 666_666, 76_367_532). ✓
//
//   K_{2,3} (S_left=6, S_right=6, ssum=12; 6 edges; S-uniform):
//     Per edge ABC₄: isqrt64(10×10^12/36)=isqrt64(277_777_777_777)=floor(527_046.27)=527_046.
//     Per edge NH:   floor(2_000_000/12)=166_666. (NOTE: 999_996 ≠ 10^6 due to floor)
//     Per edge NSO:  isqrt128(72×10^12)=floor(6√2·10^6)=8_485_281.
//     Total: (6×527_046, 6×166_666, 6×8_485_281)=(3_162_276, 999_996, 50_911_686). ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (0, 1_000_000, 1_414_213, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_414_212, 1_000_000, 5_656_854, 2, 3)
//  5.  Triangle K₃                       → (1_837_116, 750_000, 16_970_562, 3, 3)
//  6.  Star K_{1,4}                      → (2_449_488, 1_000_000, 22_627_416, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (2_080_878, 1_133_333, 11_453_742, 3, 4)
//  8.  Complete K₄                       → (2_666_664, 666_666, 76_367_532, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (3_162_276, 999_996, 50_911_686, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T21_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_21");
const T21_EXEC:   ExecutorId = ExecutorId::from_ascii("t21.exec");

const T21_KEY_A: &str = "t21.alpha";
const T21_KEY_B: &str = "t21.beta";
const T21_KEY_C: &str = "t21.gamma";
const T21_KEY_D: &str = "t21.delta";
const T21_KEY_E: &str = "t21.epsilon";

const T21_ID_A: NodeId = derive_node_id(T21_PLUGIN, T21_KEY_A);
const T21_ID_B: NodeId = derive_node_id(T21_PLUGIN, T21_KEY_B);
const T21_ID_C: NodeId = derive_node_id(T21_PLUGIN, T21_KEY_C);
const T21_ID_D: NodeId = derive_node_id(T21_PLUGIN, T21_KEY_D);
const T21_ID_E: NodeId = derive_node_id(T21_PLUGIN, T21_KEY_E);

// L4=108 namespace for this harness.
const T21_VEC_A: VectorAddress = VectorAddress::new(108, 1, 1, 0);
const T21_VEC_B: VectorAddress = VectorAddress::new(108, 1, 2, 0);
const T21_VEC_C: VectorAddress = VectorAddress::new(108, 1, 3, 0);
const T21_VEC_D: VectorAddress = VectorAddress::new(108, 2, 1, 0);
const T21_VEC_E: VectorAddress = VectorAddress::new(108, 2, 2, 0);

const T21_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T21_PLUGIN,
    name:         "kl-graph-topo21-harness",
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
        executor_id:       T21_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T21_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T21_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(abc4, 0, "empty: ABC\u{2084}=0");
    assert_eq!(nh,   0, "empty: NH=0");
    assert_eq!(nso,  0, "empty: NSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(abc4, 0, "single: ABC\u{2084}=0 (no edges)");
    assert_eq!(nh,   0, "single: NH=0 (no edges)");
    assert_eq!(nso,  0, "single: NSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1. ssum=2.
// ABC₄: ssum=2, NOT > 2 → 0. (S_u+S_v=2 is the degenerate case)
// NH:   floor(2_000_000/2) = 1_000_000.
// NSO:  isqrt128((1+1)×10^12) = isqrt128(2×10^12) = floor(√2·10^6) = 1_414_213.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);
    add_edge(T21_ID_A, T21_ID_B, "t21.e.ab");

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(abc4, 0,         "k2: ABC\u{2084}=0 (S_u+S_v=2, degenerate case)");
    assert_eq!(nh,   1_000_000, "k2: NH=1_000_000 (floor(2e6/2)=1_000_000)");
    assert_eq!(nso,  1_414_213, "k2: NSO=1_414_213 (isqrt128(2\u{00d7}10^12)=\u{221a}2\u{00d7}10^6)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=2, S(C)=deg(B)=2.
// S-uniform S=2, ssum=4. (All three nodes have the same S!)
// Per edge:
//   ABC₄: isqrt64(2×10^12/4) = isqrt64(5×10^11) = floor(707_106.78) = 707_106.
//   NH:   floor(2_000_000/4) = 500_000.
//   NSO:  isqrt128(8×10^12) = floor(2√2·10^6) = 2_828_427.
// 2 edges: ABC₄=1_414_212, NH=1_000_000, NSO=5_656_854.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);
    add_node(T21_VEC_C, T21_KEY_C, T21_ID_C);
    add_edge(T21_ID_A, T21_ID_B, "t21.e.ab");
    add_edge(T21_ID_B, T21_ID_C, "t21.e.bc");

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(abc4, 1_414_212, "p3: ABC\u{2084}=1_414_212 (2\u{00d7}707_106; S-uniform S=2)");
    assert_eq!(nh,   1_000_000, "p3: NH=1_000_000 (2\u{00d7}500_000)");
    assert_eq!(nso,  5_656_854, "p3: NSO=5_656_854 (2\u{00d7}2_828_427)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=deg(A)+deg(B)=2+2=4. S-uniform S=4, ssum=8.
// Per edge:
//   ABC₄: isqrt64(6×10^12/16) = isqrt64(375×10^9) = floor(612_372.43) = 612_372.
//   NH:   floor(2_000_000/8) = 250_000.
//   NSO:  isqrt128(32×10^12) = floor(4√2·10^6) = 5_656_854.
// 3 edges: ABC₄=1_837_116, NH=750_000, NSO=16_970_562.
// NOTE: K₃ and K_{1,4} share the same per-edge S values (both have S=4 everywhere).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);
    add_node(T21_VEC_C, T21_KEY_C, T21_ID_C);
    add_edge(T21_ID_A, T21_ID_B, "t21.e.ab");
    add_edge(T21_ID_B, T21_ID_A, "t21.e.ba");
    add_edge(T21_ID_B, T21_ID_C, "t21.e.bc");
    add_edge(T21_ID_C, T21_ID_B, "t21.e.cb");
    add_edge(T21_ID_A, T21_ID_C, "t21.e.ac");
    add_edge(T21_ID_C, T21_ID_A, "t21.e.ca");

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   3,          "k3: node_count=3");
    assert_eq!(ec,   3,          "k3: edge_count=3");
    assert_eq!(abc4, 1_837_116,  "k3: ABC\u{2084}=1_837_116 (3\u{00d7}612_372; S-uniform S=4)");
    assert_eq!(nh,   750_000,    "k3: NH=750_000 (3\u{00d7}250_000)");
    assert_eq!(nso,  16_970_562, "k3: NSO=16_970_562 (3\u{00d7}5_656_854)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4×1=4, S(leaf)=deg(hub)=4. S-uniform S=4, ssum=8.
// Per-edge values identical to K₃ (same S=4 S-uniform).
// Per edge: ABC₄=612_372, NH=250_000, NSO=5_656_854.
// 4 edges: ABC₄=2_449_488, NH=1_000_000, NSO=22_627_416.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);
    add_node(T21_VEC_C, T21_KEY_C, T21_ID_C);
    add_node(T21_VEC_D, T21_KEY_D, T21_ID_D);
    add_node(T21_VEC_E, T21_KEY_E, T21_ID_E);
    add_edge(T21_ID_A, T21_ID_B, "t21.e.ab");
    add_edge(T21_ID_A, T21_ID_C, "t21.e.ac");
    add_edge(T21_ID_A, T21_ID_D, "t21.e.ad");
    add_edge(T21_ID_A, T21_ID_E, "t21.e.ae");

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(abc4, 2_449_488,  "star: ABC\u{2084}=2_449_488 (4\u{00d7}612_372; same S=4 as K\u{2083})");
    assert_eq!(nh,   1_000_000,  "star: NH=1_000_000 (4\u{00d7}250_000)");
    assert_eq!(nso,  22_627_416, "star: NSO=22_627_416 (4\u{00d7}5_656_854)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=deg(B)=2, S(B)=deg(A)+deg(C)=3, S(C)=deg(B)+deg(D)=3, S(D)=deg(C)=2.
// Edge A-B (ssum=5, S=2,3):
//   ABC₄: isqrt64(3×10^12/6)=isqrt64(5×10^11)=707_106. NH=floor(2e6/5)=400_000.
//   NSO:  isqrt128(13×10^12)=floor(√13·10^6)=3_605_551.
// Edge B-C (ssum=6, S=3,3):
//   ABC₄: isqrt64(4×10^12/9)=isqrt64(444_444_444_444)=floor(666_666.67)=666_666.
//   NH:   floor(2_000_000/6)=333_333. NSO: isqrt128(18×10^12)=floor(3√2·10^6)=4_242_640.
// Edge C-D (ssum=5, S=3,2): same as A-B.
// Totals: ABC₄=2_080_878, NH=1_133_333, NSO=11_453_742.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);
    add_node(T21_VEC_C, T21_KEY_C, T21_ID_C);
    add_node(T21_VEC_D, T21_KEY_D, T21_ID_D);
    add_edge(T21_ID_A, T21_ID_B, "t21.e.ab");
    add_edge(T21_ID_B, T21_ID_C, "t21.e.bc");
    add_edge(T21_ID_C, T21_ID_D, "t21.e.cd");

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   4,          "p4: node_count=4");
    assert_eq!(ec,   3,          "p4: edge_count=3");
    assert_eq!(abc4, 2_080_878,  "p4: ABC\u{2084}=2_080_878 (707_106+666_666+707_106)");
    assert_eq!(nh,   1_133_333,  "p4: NH=1_133_333 (400_000+333_333+400_000)");
    assert_eq!(nso,  11_453_742, "p4: NSO=11_453_742 (3_605_551+4_242_640+3_605_551)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9, ssum=18.
// Per edge:
//   ABC₄: isqrt64(16×10^12/81)=isqrt64(197_530_864_197)=floor(444_444.44)=444_444.
//   NH:   floor(2_000_000/18)=111_111.
//   NSO:  isqrt128(162×10^12)=floor(9√2·10^6)=12_727_922.
// 6 edges: ABC₄=2_666_664, NH=666_666, NSO=76_367_532.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);
    add_node(T21_VEC_C, T21_KEY_C, T21_ID_C);
    add_node(T21_VEC_D, T21_KEY_D, T21_ID_D);
    add_edge(T21_ID_A, T21_ID_B, "t21.e.ab");
    add_edge(T21_ID_B, T21_ID_A, "t21.e.ba");
    add_edge(T21_ID_A, T21_ID_C, "t21.e.ac");
    add_edge(T21_ID_C, T21_ID_A, "t21.e.ca");
    add_edge(T21_ID_A, T21_ID_D, "t21.e.ad");
    add_edge(T21_ID_D, T21_ID_A, "t21.e.da");
    add_edge(T21_ID_B, T21_ID_C, "t21.e.bc");
    add_edge(T21_ID_C, T21_ID_B, "t21.e.cb");
    add_edge(T21_ID_B, T21_ID_D, "t21.e.bd");
    add_edge(T21_ID_D, T21_ID_B, "t21.e.db");
    add_edge(T21_ID_C, T21_ID_D, "t21.e.cd");
    add_edge(T21_ID_D, T21_ID_C, "t21.e.dc");

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   4,          "k4: node_count=4");
    assert_eq!(ec,   6,          "k4: edge_count=6");
    assert_eq!(abc4, 2_666_664,  "k4: ABC\u{2084}=2_666_664 (6\u{00d7}444_444; S-uniform S=9)");
    assert_eq!(nh,   666_666,    "k4: NH=666_666 (6\u{00d7}111_111)");
    assert_eq!(nso,  76_367_532, "k4: NSO=76_367_532 (6\u{00d7}12_727_922)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(abc4, 0, "isolated: ABC\u{2084}=0");
    assert_eq!(nh,   0, "isolated: NH=0");
    assert_eq!(nso,  0, "isolated: NSO=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2.
// S(A)=deg(C)+deg(D)+deg(E)=3×2=6. S(B)=6.
// S(C)=deg(A)+deg(B)=3+3=6. S(D)=S(E)=6.
// S-uniform S=6 (despite A,B having d=3 and C,D,E having d=2!), ssum=12.
// Per edge:
//   ABC₄: isqrt64(10×10^12/36)=isqrt64(277_777_777_777)=floor(527_046.27)=527_046.
//   NH:   floor(2_000_000/12)=166_666. (Total 999_996 ≠ 10^6 due to floor loss)
//   NSO:  isqrt128(72×10^12)=floor(6√2·10^6)=8_485_281.
// 6 edges: ABC₄=3_162_276, NH=999_996, NSO=50_911_686.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T21_VEC_A, T21_KEY_A, T21_ID_A);
    add_node(T21_VEC_B, T21_KEY_B, T21_ID_B);
    add_node(T21_VEC_C, T21_KEY_C, T21_ID_C);
    add_node(T21_VEC_D, T21_KEY_D, T21_ID_D);
    add_node(T21_VEC_E, T21_KEY_E, T21_ID_E);
    add_edge(T21_ID_A, T21_ID_C, "t21.e.ac");
    add_edge(T21_ID_C, T21_ID_A, "t21.e.ca");
    add_edge(T21_ID_A, T21_ID_D, "t21.e.ad");
    add_edge(T21_ID_D, T21_ID_A, "t21.e.da");
    add_edge(T21_ID_A, T21_ID_E, "t21.e.ae");
    add_edge(T21_ID_E, T21_ID_A, "t21.e.ea");
    add_edge(T21_ID_B, T21_ID_C, "t21.e.bc");
    add_edge(T21_ID_C, T21_ID_B, "t21.e.cb");
    add_edge(T21_ID_B, T21_ID_D, "t21.e.bd");
    add_edge(T21_ID_D, T21_ID_B, "t21.e.db");
    add_edge(T21_ID_B, T21_ID_E, "t21.e.be");
    add_edge(T21_ID_E, T21_ID_B, "t21.e.eb");

    let (abc4, nh, nso, ec, nc) = gos_runtime::graph_topo_indices21();
    assert_eq!(nc,   5,          "k23: node_count=5");
    assert_eq!(ec,   6,          "k23: edge_count=6");
    assert_eq!(abc4, 3_162_276,  "k23: ABC\u{2084}=3_162_276 (6\u{00d7}527_046; S-uniform S=6)");
    assert_eq!(nh,   999_996,    "k23: NH=999_996 (6\u{00d7}166_666; floor loss vs 6\u{00d7}166_666.\u{0305}=999_999.\u{0305})");
    assert_eq!(nso,  50_911_686, "k23: NSO=50_911_686 (6\u{00d7}8_485_281)");
}
