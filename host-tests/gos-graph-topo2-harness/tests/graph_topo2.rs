// gos-graph-topo2-harness — V3.13 H + ABC + F degree-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices2()`:
//   Returns (h_ppm, abc_ppm, f_index, edge_count, node_count)
//   - h_ppm   = H × 10^6 where H   = Σ_{uv∈E} 2/(deg(u)+deg(v))            (Zhong 2012)
//   - abc_ppm = ABC × 10^6 where ABC = Σ_{uv∈E} √((deg(u)+deg(v)−2)/(deg(u)·deg(v)))
//                                                                              (Estrada et al. 2008)
//   - f_index = F(G) = Σ_v deg(v)³  (exact integer)                          (Furtula & Gutman 2015)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// Integer precision:
//   H:   contribution = floor(2_000_000 / s)  where s = da+db  (error ≤ 1 ppm per edge)
//   ABC: contribution = floor(√((s−2)×10^12 / p))  where p = da·db; 0 when s=2 (pendant-pendant)
//   F:   exact; node scan of deg³.
//
// KEY INTEGER PRECISION NOTE for ABC:
//   isqrt64(500_000_000_000) = 707_106  (not 707_107)
//   707_106² = 499_998_895_236 < 5×10^11 ✓
//   707_107² = 500_000_309_449 > 5×10^11 ✗
//   So every edge with (s-2)/p = 1/2 contributes 707_106 ppm, not 707_107.
//   Graphs with all edges at ratio (s-2)/p = 1/2: P₃ (s=3,p=2), K₃ (s=4,p=4), P₄ (all edges), K_{2,3} (s=5,p=6).
//
// Key analytical values:
//
//  Graph        H_ppm     ABC_ppm    F_index   edges   notes
//  Empty              0         0         0      0
//  1 node             0         0         0      0    no edges, deg(v)=0
//  Edge A-B   1_000_000         0         2      1    H=1 exact; ABC: s-2=0 → 0; F=1³+1³=2
//  Path P₃    1_333_332 1_414_212        10      2    H=4/3; ABC=2×707_106; F=1+8+1
//  Triangle K₃ 1_500_000 2_121_318       24      3    H=3/2; ABC=3×707_106; F=3×8
//  Star K_{1,4} 1_600_000 3_464_100      68      4    H=8/5; ABC=4×866_025; F=4³+4×1³
//  Path P₄    1_833_332 2_121_318        18      3    H: 2×2/3+1/2; ABC=3×707_106; F=1+8+8+1
//  Complete K₄ 1_999_998 3_999_996      108      6    H≈2 (floor); ABC=6×666_666; F=4×27
//  2 isolated         0         0         0      0    no edges
//  K_{2,3}    2_400_000 4_242_636        78      6    H: 6×2/5; ABC: 6×707_106; F=2×27+3×8
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (1_000_000, 0, 2, 1, 2)
//  4.  Path P₃ = A-B-C                    → (1_333_332, 1_414_212, 10, 2, 3)
//  5.  Triangle K₃                        → (1_500_000, 2_121_318, 24, 3, 3)
//  6.  Star K_{1,4}                       → (1_600_000, 3_464_100, 68, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (1_833_332, 2_121_318, 18, 3, 4)
//  8.  Complete K₄                        → (1_999_998, 3_999_996, 108, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (2_400_000, 4_242_636, 78, 6, 5)
//      Cross-checks: H = |E| × 2/5 = 12/5 = 2.4 (exact); ABC = 6×707_106 (ratio 3/6=1/2)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T2_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX2");
const T2_EXEC:   ExecutorId = ExecutorId::from_ascii("t2.exec");

const T2_KEY_A: &str = "t2.alpha";
const T2_KEY_B: &str = "t2.beta";
const T2_KEY_C: &str = "t2.gamma";
const T2_KEY_D: &str = "t2.delta";
const T2_KEY_E: &str = "t2.epsilon";

const T2_ID_A: NodeId = derive_node_id(T2_PLUGIN, T2_KEY_A);
const T2_ID_B: NodeId = derive_node_id(T2_PLUGIN, T2_KEY_B);
const T2_ID_C: NodeId = derive_node_id(T2_PLUGIN, T2_KEY_C);
const T2_ID_D: NodeId = derive_node_id(T2_PLUGIN, T2_KEY_D);
const T2_ID_E: NodeId = derive_node_id(T2_PLUGIN, T2_KEY_E);

// L4=89 namespace for this harness.
const T2_VEC_A: VectorAddress = VectorAddress::new(89, 1, 1, 0);
const T2_VEC_B: VectorAddress = VectorAddress::new(89, 1, 2, 0);
const T2_VEC_C: VectorAddress = VectorAddress::new(89, 1, 3, 0);
const T2_VEC_D: VectorAddress = VectorAddress::new(89, 2, 1, 0);
const T2_VEC_E: VectorAddress = VectorAddress::new(89, 2, 2, 0);

const T2_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T2_PLUGIN,
    name:         "kl-graph-topo2-harness",
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
        executor_id:       T2_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T2_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T2_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices and counts are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(h,   0, "empty: H=0");
    assert_eq!(abc, 0, "empty: ABC=0");
    assert_eq!(f,   0, "empty: F=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with degree 0. No edges, so all indices are 0. F=0 (no edge-contributing degree).

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A);

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(h,   0, "single: H=0 (no edges)");
    assert_eq!(abc, 0, "single: ABC=0 (no edges)");
    assert_eq!(f,   0, "single: F=0 (deg=0, 0³=0)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B. Both nodes have degree 1. s=2, p=1.
// H:   floor(2_000_000 / 2) = 1_000_000  (H=1 exact)
// ABC: s-2=0 → contribution=0  (pendant-pendant; same skip rule as AZI)
// F:   A:1³=1, B:1³=1 → F=2

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A);
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B);
    add_edge(T2_ID_A, T2_ID_B, "ab");

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(h,   1_000_000, "edge: H_ppm=1_000_000 (H=1 exact: 2/(1+1)=1)");
    assert_eq!(abc, 0,         "edge: ABC=0 (pendant-pendant: s-2=0)");
    assert_eq!(f,   2,         "edge: F=2 (1³+1³=2)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1. Edges: A-B and B-C.
//
// A-B: da=1, db=2; s=3, p=2, s-2=1.
//   H:   floor(2_000_000/3) = 666_666
//   ABC: floor(√(1×10^12/2)) = floor(√(5×10^11)) = isqrt64(500_000_000_000) = 707_106
//        (707_106² = 499_998_895_236 < 5×10^11; 707_107² = 500_000_309_449 > 5×10^11)
// B-C: same as A-B.
// H total: 2×666_666 = 1_333_332
// ABC total: 2×707_106 = 1_414_212
// F: A:1³=1, B:2³=8, C:1³=1 → F=10

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A);
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B);
    add_node(T2_VEC_C, T2_KEY_C, T2_ID_C);
    add_edge(T2_ID_A, T2_ID_B, "ab");
    add_edge(T2_ID_B, T2_ID_C, "bc");

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  3,         "P₃: node_count=3");
    assert_eq!(ec,  2,         "P₃: edge_count=2");
    assert_eq!(h,   1_333_332, "P₃: H_ppm=1_333_332 (2×666_666; H=4/3)");
    assert_eq!(abc, 1_414_212, "P₃: ABC_ppm=1_414_212 (2×707_106; ABC≈√2; isqrt floor)");
    assert_eq!(f,   10,        "P₃: F=10 (1+8+1)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// Degrees: all 2. 3 undirected edges. s=4, p=4, s-2=2.
//
// H per edge:   floor(2_000_000/4) = 500_000.   Total: 3×500_000 = 1_500_000  (H=3/2 exact)
// ABC per edge: floor(√(2×10^12/4)) = isqrt64(500_000_000_000) = 707_106.
//   Total: 3×707_106 = 2_121_318
//   Exact: ABC(K₃) = 3×√(2/4) = 3/√2 ≈ 2.12132.  Floor gives 2_121_318 (ratio=1/2 → 707_106).
// F: 3×2³ = 24.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A);
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B);
    add_node(T2_VEC_C, T2_KEY_C, T2_ID_C);
    add_edge(T2_ID_A, T2_ID_B, "ab");
    add_edge(T2_ID_B, T2_ID_C, "bc");
    add_edge(T2_ID_C, T2_ID_A, "ca");

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  3,         "K₃: node_count=3");
    assert_eq!(ec,  3,         "K₃: edge_count=3");
    assert_eq!(h,   1_500_000, "K₃: H_ppm=1_500_000 (H=3/2 exact)");
    assert_eq!(abc, 2_121_318, "K₃: ABC_ppm=2_121_318 (3×707_106; ABC≈3/√2; floor)");
    assert_eq!(f,   24,        "K₃: F=24 (3×2³=3×8)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center deg=4, 4 leaves deg=1. 4 edges. For each edge: da=4, db=1; s=5, p=4, s-2=3.
//
// H per edge:   floor(2_000_000/5) = 400_000.   Total: 4×400_000 = 1_600_000  (H=8/5 exact)
// ABC per edge: floor(√(3×10^12/4)) = floor(√(7.5×10^11)) = floor(866_025.4) = 866_025.
//   Total: 4×866_025 = 3_464_100
//   Exact: ABC = 4×√(3/4) = 4×(√3/2) = 2√3 ≈ 3.46410.  ABC_ppm = 3_464_102 exact.
//   Floor: 3_464_100 (2 ppm below exact due to integer truncation per edge).
// F: center:4³=64, 4 leaves:1³=1 each. F = 64+4 = 68.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A); // center
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B);
    add_node(T2_VEC_C, T2_KEY_C, T2_ID_C);
    add_node(T2_VEC_D, T2_KEY_D, T2_ID_D);
    add_node(T2_VEC_E, T2_KEY_E, T2_ID_E);
    add_edge(T2_ID_A, T2_ID_B, "ab");
    add_edge(T2_ID_A, T2_ID_C, "ac");
    add_edge(T2_ID_A, T2_ID_D, "ad");
    add_edge(T2_ID_A, T2_ID_E, "ae");

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  5,         "K_{{1,4}}: node_count=5");
    assert_eq!(ec,  4,         "K_{{1,4}}: edge_count=4");
    assert_eq!(h,   1_600_000, "K_{{1,4}}: H_ppm=1_600_000 (H=8/5 exact: 4×2/5)");
    assert_eq!(abc, 3_464_100, "K_{{1,4}}: ABC_ppm=3_464_100 (4×866_025; ABC=2√3)");
    assert_eq!(f,   68,        "K_{{1,4}}: F=68 (4³+4×1³=64+4)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1. Edges: A-B, B-C, C-D.
//
// A-B: da=1, db=2; s=3, p=2, s-2=1. H=666_666. ABC=707_106.
// B-C: da=2, db=2; s=4, p=4, s-2=2. H=500_000. ABC=707_106.
// C-D: da=2, db=1; s=3, p=2, s-2=1. H=666_666. ABC=707_106.
//
// H total: 2×666_666 + 500_000 = 1_833_332
// ABC total: 3×707_106 = 2_121_318  (same as K₃ — all edges have ratio (s-2)/p = 1/2)
// F: A:1 + B:8 + C:8 + D:1 = 18

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A);
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B);
    add_node(T2_VEC_C, T2_KEY_C, T2_ID_C);
    add_node(T2_VEC_D, T2_KEY_D, T2_ID_D);
    add_edge(T2_ID_A, T2_ID_B, "ab");
    add_edge(T2_ID_B, T2_ID_C, "bc");
    add_edge(T2_ID_C, T2_ID_D, "cd");

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  4,         "P₄: node_count=4");
    assert_eq!(ec,  3,         "P₄: edge_count=3");
    assert_eq!(h,   1_833_332, "P₄: H_ppm=1_833_332 (2×666_666+500_000)");
    assert_eq!(abc, 2_121_318, "P₄: ABC_ppm=2_121_318 (3×707_106; same ratio as K₃: (s-2)/p=1/2)");
    assert_eq!(f,   18,        "P₄: F=18 (1+8+8+1)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All degrees=3. 6 undirected edges. s=6, p=9, s-2=4.
//
// H per edge:   floor(2_000_000/6) = 333_333.    Total: 6×333_333 = 1_999_998
//   Exact H=2; floor error = 6×(1/3 ppm) → total 2 ppm below 2_000_000.
// ABC per edge: floor(√(4×10^12/9)) = floor(√(4.44×10^11)) = floor(666_666.66) = 666_666.
//   Total: 6×666_666 = 3_999_996
//   Exact ABC(K₄) = 6×(2/3) = 4; floor error = 6×(0.66 ppm) ≈ 4 ppm below 4_000_000.
// F: 4×3³ = 4×27 = 108.
//
// KEY INVARIANT: H ≈ |E|/Δ for Δ-regular (H=6/3=2); ABC exact for K₄ = 4.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A);
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B);
    add_node(T2_VEC_C, T2_KEY_C, T2_ID_C);
    add_node(T2_VEC_D, T2_KEY_D, T2_ID_D);
    add_edge(T2_ID_A, T2_ID_B, "ab");
    add_edge(T2_ID_A, T2_ID_C, "ac");
    add_edge(T2_ID_A, T2_ID_D, "ad");
    add_edge(T2_ID_B, T2_ID_C, "bc");
    add_edge(T2_ID_B, T2_ID_D, "bd");
    add_edge(T2_ID_C, T2_ID_D, "cd");

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  4,         "K₄: node_count=4");
    assert_eq!(ec,  6,         "K₄: edge_count=6");
    assert_eq!(h,   1_999_998, "K₄: H_ppm=1_999_998 (H≈2; floor: 6×333_333)");
    assert_eq!(abc, 3_999_996, "K₄: ABC_ppm=3_999_996 (ABC≈4; floor: 6×666_666)");
    assert_eq!(f,   108,       "K₄: F=108 (4×3³=4×27)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Two nodes, no edges. All indices are 0, node_count=2.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A);
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B);

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  2, "2 isolated: node_count=2");
    assert_eq!(ec,  0, "2 isolated: no edges");
    assert_eq!(h,   0, "2 isolated: H=0");
    assert_eq!(abc, 0, "2 isolated: ABC=0");
    assert_eq!(f,   0, "2 isolated: F=0 (deg=0 for both)");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Side-A: A(deg=3), B(deg=3). Side-B: C(deg=2), D(deg=2), E(deg=2). 6 edges.
// All edges: da=3, db=2; s=5, p=6, s-2=3.
//
// H per edge:   floor(2_000_000/5) = 400_000.
//   Total: 6×400_000 = 2_400_000  (H = 12/5 = 2.4 exact — 5 divides 2_000_000 exactly)
// ABC per edge: floor(√(3×10^12/6)) = isqrt64(500_000_000_000) = 707_106.
//   Total: 6×707_106 = 4_242_636
//   Exact: ABC(K_{2,3}) = 6×√(3/6) = 6/√2 = 3√2 ≈ 4.24264.  Floor: 4_242_636.
// F: A:3³=27, B:3³=27, C:2³=8, D:2³=8, E:2³=8. F = 2×27 + 3×8 = 54+24 = 78.
//
// Cross-checks:
//   H exact: 2_400_000 = 6_000_000 × 2/5 (s=5 always exact divisor for H=12/5)
//   ABC ratio invariant: K_{2,3} s=5,p=6 → (s-2)/p=3/6=1/2. Same as P₄ edges (s=4,p=4 → 2/4=1/2
//   and s=3,p=2 → 1/2). All have ratio 1/2 → isqrt64(500_000_000_000)=707_106 per edge. ✓

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    add_node(T2_VEC_A, T2_KEY_A, T2_ID_A); // deg=3
    add_node(T2_VEC_B, T2_KEY_B, T2_ID_B); // deg=3
    add_node(T2_VEC_C, T2_KEY_C, T2_ID_C); // deg=2
    add_node(T2_VEC_D, T2_KEY_D, T2_ID_D); // deg=2
    add_node(T2_VEC_E, T2_KEY_E, T2_ID_E); // deg=2
    add_edge(T2_ID_A, T2_ID_C, "ac");
    add_edge(T2_ID_A, T2_ID_D, "ad");
    add_edge(T2_ID_A, T2_ID_E, "ae");
    add_edge(T2_ID_B, T2_ID_C, "bc");
    add_edge(T2_ID_B, T2_ID_D, "bd");
    add_edge(T2_ID_B, T2_ID_E, "be");

    let (h, abc, f, ec, nc) = gos_runtime::graph_topo_indices2();
    assert_eq!(nc,  5,         "K_{{2,3}}: node_count=5");
    assert_eq!(ec,  6,         "K_{{2,3}}: edge_count=6");
    assert_eq!(h,   2_400_000, "K_{{2,3}}: H_ppm=2_400_000 (H=12/5 exact; 6×400_000)");
    assert_eq!(abc, 4_242_636, "K_{{2,3}}: ABC_ppm=4_242_636 (6×707_106; ABC≈3√2; floor)");
    assert_eq!(f,   78,        "K_{{2,3}}: F=78 (2×27+3×8=54+24)");

    // H is exact for K_{2,3} since s=5 always divides 2_000_000 without remainder.
    assert_eq!(h, 6 * 2_000_000 / 5,
        "K_{{2,3}}: H exact (no floor error when s divides 2_000_000)");

    // ABC ratio invariant: all K_{2,3} edges have (s-2)/p = 3/6 = 1/2.
    // isqrt64(500_000_000_000) = 707_106, so each edge contributes 707_106 ppm.
    assert_eq!(abc, 6 * 707_106,
        "K_{{2,3}}: ABC_ppm = 6 × 707_106 (ratio (s-2)/p=3/6=1/2)");
}
