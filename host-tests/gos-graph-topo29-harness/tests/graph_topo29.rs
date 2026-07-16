// gos-graph-topo29-harness — V3.40 NZ0 + NEM2 + NSe (S-variant family)
//
// Verifies `gos_runtime::graph_topo_indices29()`:
//   Returns (nz0_ppm, nem2, nse_ppm, edge_count, node_count)
//   - nz0_ppm  = NZ₀(G) × 10^6 = Σ_{v: S(v)>0} isqrt64(10^12/S(v))  (floor ppm; S-zero-order Randić)
//   - nem2     = NEM₂(G)        = Σ_{uv∈E} S_u·S_v·(S_u+S_v−2)       (exact u64; S-Reformulated 2nd Zagreb)
//   - nse_ppm  = NSe(G) × 10^6  = Σ_v isqrt64(S(v)×10^12)             (floor ppm; S-sqrt vertex sum)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum ("S-variant").
//
// DEFINITIONS:
//   NZ₀(G)  = Σ_{v: S(v)>0} 1/√S(v)  (S-analogue of zeroth-order Randić χ₀; Randić 1975)
//   NEM₂(G) = Σ_{uv∈E} d_u·d_v·(d_u+d_v−2) with d replaced by S  (Miličević et al. 2004)
//   NSe(G)  = Σ_v √S(v)               (S-sqrt vertex sum; complement to NF=Σ_v S³)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   NZ₀  per vertex = isqrt64(10^12 / S(v))       [u64; 10^12/S≤10^12 < u64::MAX; skip S=0]
//   NEM₂ per edge   = S_u · S_v · (S_u+S_v−2)    [u64; max ≈ 8.39×10^12 per edge; accumulator ≤ 6.82×10^16]
//   NSe  per vertex = isqrt64(S(v) × 10^12)       [u64; S×10^12 ≤ 1.61×10^16 < u64::MAX]
//
// KEY INVARIANTS:
//   NZ₀  = n × 10^6 / √S for S-regular (all S equal; = n × isqrt64(10^12/S)).
//   NEM₂ = 0 iff all edges have S_u+S_v=2 (only K₂-type: both S=1).
//   NEM₂ = |E|·S²·(2S−2) for S-regular.
//   NSe  = n × isqrt64(S × 10^12) for S-regular.
//   Isolated nodes (S=0): contribute 0 to NZ₀ (skip); contribute 0 to NSe (isqrt64(0)=0).
//   K₃ and K_{1,4}: both S-uniform S=4 → same per-vertex NZ₀ and NSe; NEM₂ differs (|E| factor).
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
//  Graph         NZ₀(ppm)   NEM₂(exact)  NSe(ppm)    edges  nodes
//  Empty                0            0           0       0      0
//  1 node               0            0           0       0      1
//  Edge K₂      2_000_000            0   2_000_000       1      2
//  Path P₃      2_121_318           16   4_242_639       2      3
//  Triangle K₃  1_500_000          288   6_000_000       3      3
//  Star K_{1,4} 2_500_000          384  10_000_000       4      5
//  Path P₄      2_568_912           72   6_292_526       3      4
//  Complete K₄  1_333_332        7_776  12_000_000       6      4
//  2 isolated           0            0           0       0      2
//  K_{2,3}      2_041_240        2_160  12_247_445       6      5
//
// DERIVATIONS:
//
//   K₂ (S_A=S_B=1):
//     NZ₀: 2 × isqrt64(10^12/1) = 2 × 1_000_000 = 2_000_000. ✓
//     NEM₂: 1×1×(1+1−2) = 1×0 = 0. ✓
//     NSe: 2 × isqrt64(1×10^12) = 2 × 1_000_000 = 2_000_000. ✓
//
//   P₃ (S-uniform S=2, 3 nodes, 2 edges):
//     NZ₀: 3 × isqrt64(10^12/2) = 3 × isqrt64(500_000_000_000) = 3 × 707_106 = 2_121_318.
//       (√500_000_000_000 = 707_106.781...; 707_106² = 499_998_907_236 ≤ 500_000_000_000) ✓
//     NEM₂: 2 edges × 2×2×(2+2−2) = 2 × 4×2 = 16. ✓
//     NSe: 3 × isqrt64(2×10^12) = 3 × 1_414_213 = 4_242_639.
//       (√2×10^6 = 1_414_213.562...; 1_414_213² = 1_999_998_477_369 ≤ 2×10^12) ✓
//
//   K₃ (S-uniform S=4, 3 nodes, 3 edges):
//     NZ₀: 3 × isqrt64(10^12/4) = 3 × isqrt64(250_000_000_000) = 3 × 500_000 = 1_500_000.
//       (250_000_000_000 = 500_000²; exact square root.) ✓
//     NEM₂: 3 × 4×4×(4+4−2) = 3 × 16×6 = 3 × 96 = 288. ✓
//     NSe: 3 × isqrt64(4×10^12) = 3 × 2_000_000 = 6_000_000. (exact) ✓
//
//   K_{1,4} (S-uniform S=4, 5 nodes, 4 edges):
//     NZ₀: 5 × 500_000 = 2_500_000. ✓
//     NEM₂: 4 × 4×4×6 = 4 × 96 = 384. ✓
//     NSe: 5 × 2_000_000 = 10_000_000. ✓
//
//   P₄ (S_A=2, S_B=3, S_C=3, S_D=2; 3 edges):
//     NZ₀: isqrt64(10^12/2)+isqrt64(10^12/3)+isqrt64(10^12/3)+isqrt64(10^12/2)
//         = 707_106 + isqrt64(333_333_333_333) + isqrt64(333_333_333_333) + 707_106
//       isqrt64(333_333_333_333): √(10^12/3)=10^6/√3=577_350.269...; 577_350²=333_293_032_500≤333_333_333_333) ✓
//       = 707_106 + 577_350 + 577_350 + 707_106 = 2_568_912. ✓
//     NEM₂:
//       {A,B}(S=2,3): 2×3×(5−2) = 6×3 = 18.
//       {B,C}(S=3,3): 3×3×(6−2) = 9×4 = 36.
//       {C,D}(S=3,2): 2×3×3 = 18.
//       Total = 18+36+18 = 72. ✓
//     NSe: isqrt64(2×10^12)+isqrt64(3×10^12)+isqrt64(3×10^12)+isqrt64(2×10^12)
//         = 1_414_213 + isqrt64(3_000_000_000_000) + isqrt64(3_000_000_000_000) + 1_414_213
//       isqrt64(3×10^12): √3×10^6=1_732_050.808...; 1_732_050²=2_999_997_162_500≤3×10^12 ✓
//       = 1_414_213 + 1_732_050 + 1_732_050 + 1_414_213 = 6_292_526. ✓
//
//   K₄ (S-uniform S=9, 4 nodes, 6 edges):
//     NZ₀: 4 × isqrt64(10^12/9) = 4 × isqrt64(111_111_111_111) = 4 × 333_333 = 1_333_332.
//       (√(10^12/9)=10^6/3=333_333.333...; 333_333²=111_110_888_889≤111_111_111_111) ✓
//     NEM₂: 6 × 9×9×(18−2) = 6 × 81×16 = 6 × 1_296 = 7_776. ✓
//     NSe: 4 × isqrt64(9×10^12) = 4 × 3_000_000 = 12_000_000. (exact) ✓
//
//   K_{2,3} (S-uniform S=6, 5 nodes, 6 edges):
//     NZ₀: 5 × isqrt64(10^12/6) = 5 × isqrt64(166_666_666_666) = 5 × 408_248 = 2_041_240.
//       (√(10^12/6)=10^6/√6=408_248.290...; 408_248²=166_666_408_504≤166_666_666_666) ✓
//     NEM₂: 6 × 6×6×(12−2) = 6 × 36×10 = 6 × 360 = 2_160. ✓
//     NSe: 5 × isqrt64(6×10^12) = 5 × 2_449_489 = 12_247_445.
//       (√6×10^6=2_449_489.742...; 2_449_489²=5_999_996_361_121≤6×10^12) ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B (K₂)     → (2_000_000, 0, 2_000_000, 1, 2)
//  4.  Path P₃ = A-B-C                   → (2_121_318, 16, 4_242_639, 2, 3)
//  5.  Triangle K₃                       → (1_500_000, 288, 6_000_000, 3, 3)
//  6.  Star K_{1,4}                      → (2_500_000, 384, 10_000_000, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (2_568_912, 72, 6_292_526, 3, 4)
//  8.  Complete K₄                       → (1_333_332, 7_776, 12_000_000, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (2_041_240, 2_160, 12_247_445, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T29_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_29");
const T29_EXEC:   ExecutorId = ExecutorId::from_ascii("t29.exec");

const T29_KEY_A: &str = "t29.alpha";
const T29_KEY_B: &str = "t29.beta";
const T29_KEY_C: &str = "t29.gamma";
const T29_KEY_D: &str = "t29.delta";
const T29_KEY_E: &str = "t29.epsilon";

const T29_ID_A: NodeId = derive_node_id(T29_PLUGIN, T29_KEY_A);
const T29_ID_B: NodeId = derive_node_id(T29_PLUGIN, T29_KEY_B);
const T29_ID_C: NodeId = derive_node_id(T29_PLUGIN, T29_KEY_C);
const T29_ID_D: NodeId = derive_node_id(T29_PLUGIN, T29_KEY_D);
const T29_ID_E: NodeId = derive_node_id(T29_PLUGIN, T29_KEY_E);

// L4=116 namespace for this harness.
const T29_VEC_A: VectorAddress = VectorAddress::new(116, 1, 1, 0);
const T29_VEC_B: VectorAddress = VectorAddress::new(116, 1, 2, 0);
const T29_VEC_C: VectorAddress = VectorAddress::new(116, 1, 3, 0);
const T29_VEC_D: VectorAddress = VectorAddress::new(116, 2, 1, 0);
const T29_VEC_E: VectorAddress = VectorAddress::new(116, 2, 2, 0);

const T29_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T29_PLUGIN,
    name:         "kl-graph-topo29-harness",
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
        executor_id:       T29_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T29_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T29_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(nz0,  0, "empty: NZ0=0");
    assert_eq!(nem2, 0, "empty: NEM2=0");
    assert_eq!(nse,  0, "empty: NSe=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// S=0 (no neighbors) → NZ0 skip; NSe: isqrt64(0)=0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(nz0,  0, "single: NZ0=0 (S=0, isolated skipped)");
    assert_eq!(nem2, 0, "single: NEM2=0 (no edges)");
    assert_eq!(nse,  0, "single: NSe=0 (S=0, isqrt64(0)=0)");
}

// ── Test 3: Single directed edge A→B (K₂) ────────────────────────────────────
// S(A)=deg(B)=1, S(B)=deg(A)=1.
// NZ0: 2×isqrt64(10^12/1)=2×1_000_000=2_000_000.
// NEM2: 1×1×(1+1−2)=0.
// NSe: 2×isqrt64(1×10^12)=2×1_000_000=2_000_000.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);
    add_edge(T29_ID_A, T29_ID_B, "t29.e.ab");

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   2,         "k2: node_count=2");
    assert_eq!(ec,   1,         "k2: edge_count=1");
    assert_eq!(nz0,  2_000_000, "k2: NZ0=2_000_000 (2\u{00d7}isqrt64(10\u{00b9}\u{00b2}/1)=2\u{00d7}1_000_000; S=1)");
    assert_eq!(nem2, 0,         "k2: NEM2=0 (1\u{00d7}1\u{00d7}(1+1\u{2212}2)=0; K\u{2082} factor=0)");
    assert_eq!(nse,  2_000_000, "k2: NSe=2_000_000 (2\u{00d7}isqrt64(1\u{00d7}10\u{00b9}\u{00b2})=2\u{00d7}1_000_000)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d(A)=d(C)=1, d(B)=2. S-uniform S=2. 3 nodes, 2 edges.
// NZ0: 3×isqrt64(10^12/2)=3×707_106=2_121_318.
// NEM2: 2×2×2×(2+2−2)=2×4×2=16.
// NSe: 3×isqrt64(2×10^12)=3×1_414_213=4_242_639.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);
    add_node(T29_VEC_C, T29_KEY_C, T29_ID_C);
    add_edge(T29_ID_A, T29_ID_B, "t29.e.ab");
    add_edge(T29_ID_B, T29_ID_C, "t29.e.bc");

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(nz0,  2_121_318, "p3: NZ0=2_121_318 (3\u{00d7}707_106; isqrt64(500_000_000_000)=707_106; S-uniform S=2)");
    assert_eq!(nem2, 16,        "p3: NEM2=16 (2\u{00d7}4\u{00d7}2; S-uniform S=2; per-edge=8)");
    assert_eq!(nse,  4_242_639, "p3: NSe=4_242_639 (3\u{00d7}1_414_213; isqrt64(2\u{00d7}10\u{00b9}\u{00b2})=1_414_213)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. S(each)=4. S-uniform S=4. 3 nodes, 3 edges.
// NZ0: 3×isqrt64(10^12/4)=3×500_000=1_500_000.  (exact: 500_000²=250_000_000_000_000_000≠250_000_000_000)
//   Wait: 10^12/4=250_000_000_000. isqrt64(250_000_000_000)=500_000 (500_000²=2.5×10^11 ✓).
// NEM2: 3×4×4×(4+4−2)=3×16×6=3×96=288.
// NSe: 3×isqrt64(4×10^12)=3×2_000_000=6_000_000.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);
    add_node(T29_VEC_C, T29_KEY_C, T29_ID_C);
    add_edge(T29_ID_A, T29_ID_B, "t29.e.ab");
    add_edge(T29_ID_B, T29_ID_A, "t29.e.ba");
    add_edge(T29_ID_B, T29_ID_C, "t29.e.bc");
    add_edge(T29_ID_C, T29_ID_B, "t29.e.cb");
    add_edge(T29_ID_A, T29_ID_C, "t29.e.ac");
    add_edge(T29_ID_C, T29_ID_A, "t29.e.ca");

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   3,         "k3: node_count=3");
    assert_eq!(ec,   3,         "k3: edge_count=3");
    assert_eq!(nz0,  1_500_000, "k3: NZ0=1_500_000 (3\u{00d7}500_000; isqrt64(250_000_000_000)=500_000; S-uniform S=4)");
    assert_eq!(nem2, 288,       "k3: NEM2=288 (3\u{00d7}96; S-uniform S=4; per-edge=4\u{00d7}4\u{00d7}6=96)");
    assert_eq!(nse,  6_000_000, "k3: NSe=6_000_000 (3\u{00d7}2_000_000; isqrt64(4\u{00d7}10\u{00b9}\u{00b2})=2_000_000; exact)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d(hub)=4, d(leaf)=1. S(hub)=4, S(leaf)=4. S-uniform S=4. 5 nodes, 4 edges.
// Same per-vertex NZ0 and NSe as K₃ (S-uniform S=4 coincidence); NEM2 differs.
// NZ0: 5×500_000=2_500_000. NEM2: 4×96=384. NSe: 5×2_000_000=10_000_000.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);
    add_node(T29_VEC_C, T29_KEY_C, T29_ID_C);
    add_node(T29_VEC_D, T29_KEY_D, T29_ID_D);
    add_node(T29_VEC_E, T29_KEY_E, T29_ID_E);
    add_edge(T29_ID_A, T29_ID_B, "t29.e.ab");
    add_edge(T29_ID_A, T29_ID_C, "t29.e.ac");
    add_edge(T29_ID_A, T29_ID_D, "t29.e.ad");
    add_edge(T29_ID_A, T29_ID_E, "t29.e.ae");

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(nz0,  2_500_000,  "star: NZ0=2_500_000 (5\u{00d7}500_000; S-uniform S=4 coincides with K\u{2083})");
    assert_eq!(nem2, 384,        "star: NEM2=384 (4\u{00d7}96; S-uniform S=4; same per-edge as K\u{2083})");
    assert_eq!(nse,  10_000_000, "star: NSe=10_000_000 (5\u{00d7}2_000_000; S-uniform S=4; more nodes than K\u{2083})");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d(A)=d(D)=1, d(B)=d(C)=2.
// S(A)=2, S(B)=3, S(C)=3, S(D)=2. Mixed S.
// NZ0: 707_106+577_350+577_350+707_106=2_568_912.
// NEM2: {A,B}=18 + {B,C}=36 + {C,D}=18 = 72.
// NSe: 1_414_213+1_732_050+1_732_050+1_414_213=6_292_526.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);
    add_node(T29_VEC_C, T29_KEY_C, T29_ID_C);
    add_node(T29_VEC_D, T29_KEY_D, T29_ID_D);
    add_edge(T29_ID_A, T29_ID_B, "t29.e.ab");
    add_edge(T29_ID_B, T29_ID_C, "t29.e.bc");
    add_edge(T29_ID_C, T29_ID_D, "t29.e.cd");

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(nz0,  2_568_912, "p4: NZ0=2_568_912 (707_106+577_350+577_350+707_106; S values 2,3,3,2)");
    assert_eq!(nem2, 72,        "p4: NEM2=72 (18+36+18; {{A,B}}=2\u{00d7}3\u{00d7}3=18; {{B,C}}=3\u{00d7}3\u{00d7}4=36)");
    assert_eq!(nse,  6_292_526, "p4: NSe=6_292_526 (1_414_213+1_732_050+1_732_050+1_414_213; S values 2,3,3,2)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. S(each)=3×3=9. S-uniform S=9. 4 nodes, 6 edges.
// NZ0: 4×isqrt64(10^12/9)=4×333_333=1_333_332.
// NEM2: 6×9×9×(18−2)=6×81×16=6×1_296=7_776.
// NSe: 4×isqrt64(9×10^12)=4×3_000_000=12_000_000.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);
    add_node(T29_VEC_C, T29_KEY_C, T29_ID_C);
    add_node(T29_VEC_D, T29_KEY_D, T29_ID_D);
    add_edge(T29_ID_A, T29_ID_B, "t29.e.ab");
    add_edge(T29_ID_B, T29_ID_A, "t29.e.ba");
    add_edge(T29_ID_A, T29_ID_C, "t29.e.ac");
    add_edge(T29_ID_C, T29_ID_A, "t29.e.ca");
    add_edge(T29_ID_A, T29_ID_D, "t29.e.ad");
    add_edge(T29_ID_D, T29_ID_A, "t29.e.da");
    add_edge(T29_ID_B, T29_ID_C, "t29.e.bc");
    add_edge(T29_ID_C, T29_ID_B, "t29.e.cb");
    add_edge(T29_ID_B, T29_ID_D, "t29.e.bd");
    add_edge(T29_ID_D, T29_ID_B, "t29.e.db");
    add_edge(T29_ID_C, T29_ID_D, "t29.e.cd");
    add_edge(T29_ID_D, T29_ID_C, "t29.e.dc");

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   4,          "k4: node_count=4");
    assert_eq!(ec,   6,          "k4: edge_count=6");
    assert_eq!(nz0,  1_333_332,  "k4: NZ0=1_333_332 (4\u{00d7}333_333; isqrt64(111_111_111_111)=333_333; S-uniform S=9)");
    assert_eq!(nem2, 7_776,      "k4: NEM2=7_776 (6\u{00d7}1_296; S-uniform S=9; per-edge=81\u{00d7}16=1_296)");
    assert_eq!(nse,  12_000_000, "k4: NSe=12_000_000 (4\u{00d7}3_000_000; isqrt64(9\u{00d7}10\u{00b9}\u{00b2})=3_000_000; exact)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// S=0 for both → NZ0: skip S=0; NSe: isqrt64(0)=0. NEM2: no edges.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(nz0,  0, "isolated: NZ0=0 (S=0 for both; skip)");
    assert_eq!(nem2, 0, "isolated: NEM2=0 (no edges)");
    assert_eq!(nse,  0, "isolated: NSe=0 (S=0 for both; isqrt64(0)=0)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}: d=3. Right={C,D,E}: d=2.
// S(A)=S(B)=3×2=6. S(C)=S(D)=S(E)=2×3=6. S-uniform S=6. 5 nodes, 6 edges.
// NZ0: 5×isqrt64(10^12/6)=5×408_248=2_041_240.
//   (√(10^12/6)=10^6/√6=408_248.290...; 408_248²=166_666_408_504≤166_666_666_666) ✓
// NEM2: 6×6×6×(12−2)=6×36×10=6×360=2_160.
// NSe: 5×isqrt64(6×10^12)=5×2_449_489=12_247_445.
//   (√6×10^6=2_449_489.742...; 2_449_489²=5_999_996_361_121≤6×10^12) ✓

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T29_VEC_A, T29_KEY_A, T29_ID_A);
    add_node(T29_VEC_B, T29_KEY_B, T29_ID_B);
    add_node(T29_VEC_C, T29_KEY_C, T29_ID_C);
    add_node(T29_VEC_D, T29_KEY_D, T29_ID_D);
    add_node(T29_VEC_E, T29_KEY_E, T29_ID_E);
    add_edge(T29_ID_A, T29_ID_C, "t29.e.ac");
    add_edge(T29_ID_C, T29_ID_A, "t29.e.ca");
    add_edge(T29_ID_A, T29_ID_D, "t29.e.ad");
    add_edge(T29_ID_D, T29_ID_A, "t29.e.da");
    add_edge(T29_ID_A, T29_ID_E, "t29.e.ae");
    add_edge(T29_ID_E, T29_ID_A, "t29.e.ea");
    add_edge(T29_ID_B, T29_ID_C, "t29.e.bc");
    add_edge(T29_ID_C, T29_ID_B, "t29.e.cb");
    add_edge(T29_ID_B, T29_ID_D, "t29.e.bd");
    add_edge(T29_ID_D, T29_ID_B, "t29.e.db");
    add_edge(T29_ID_B, T29_ID_E, "t29.e.be");
    add_edge(T29_ID_E, T29_ID_B, "t29.e.eb");

    let (nz0, nem2, nse, ec, nc) = gos_runtime::graph_topo_indices29();
    assert_eq!(nc,   5,          "k23: node_count=5");
    assert_eq!(ec,   6,          "k23: edge_count=6");
    assert_eq!(nz0,  2_041_240,  "k23: NZ0=2_041_240 (5\u{00d7}408_248; isqrt64(166_666_666_666)=408_248; S-uniform S=6)");
    assert_eq!(nem2, 2_160,      "k23: NEM2=2_160 (6\u{00d7}360; S-uniform S=6; per-edge=36\u{00d7}10=360)");
    assert_eq!(nse,  12_247_445, "k23: NSe=12_247_445 (5\u{00d7}2_449_489; isqrt64(6\u{00d7}10\u{00b9}\u{00b2})=2_449_489; S-uniform S=6)");
}
