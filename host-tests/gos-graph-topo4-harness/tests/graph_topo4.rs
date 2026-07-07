// gos-graph-topo4-harness — V3.15 Sombor + RM₂ + Sigma degree-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices4()`:
//   Returns (so_ppm, rm2, sigma, edge_count, node_count)
//   - so_ppm = SO × 10^6 where SO  = Σ_{uv∈E} √(da²+db²)     (Gutman 2021)
//   - rm2    = RM₂        where RM₂ = Σ_{uv∈E} (da-1)·(db-1)  (Furtula, Gutman & Ediz 2014)
//   - sigma  = σ(G)       where σ   = Σ_{uv∈E} (da-db)²       (Gutman et al. 2014)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// Integer precision:
//   SO:  contribution = isqrt64((da²+db²) × 10^12)   floor √(da²+db²) × 10^6
//   RM₂: exact integer; (da-1)·(db-1); 0 for pendant edges (da=1 or db=1)
//   σ:   exact integer; (da-db)²;       0 for regular graphs (all da=db)
//
// KEY INVARIANTS:
//   σ = 0 iff graph is regular (all undirected degrees equal) — rigorous test
//   RM₂ = 0 for pendant-endpoint edges; = |E|·(Δ-1)² for Δ-regular graphs
//   SO for Δ-regular: SO = |E| · Δ·√2 (floor); exact when 2Δ² is perfect square (never for Δ≥1)
//
// KEY isqrt64 VALUES for SO:
//   isqrt64(2_000_000_000_000)  = 1_414_213  (√2 × 10^6; √(1²+1²) for edge A-B with da=db=1)
//   isqrt64(5_000_000_000_000)  = 2_236_067  (√5 × 10^6; √(1²+2²))
//   isqrt64(8_000_000_000_000)  = 2_828_427  (2√2 × 10^6; √(2²+2²) for K₃ regular Δ=2)
//   isqrt64(13_000_000_000_000) = 3_605_551  (√13 × 10^6; √(2²+3²) for K_{2,3})
//   isqrt64(17_000_000_000_000) = 4_123_105  (√17 × 10^6; √(1²+4²) for K_{1,4})
//   isqrt64(18_000_000_000_000) = 4_242_640  (3√2 × 10^6; √(3²+3²) for K₄ regular Δ=3)
//
// Analytical cross-check table:
//
//  Graph         SO_ppm      RM₂    σ    edges  nodes
//  Empty              0        0    0      0      0
//  1 node             0        0    0      0      1
//  Edge A-B   1_414_213        0    0      1      2  (da=db=1; rm2=0 pendant; σ=0 equal)
//  Path P₃    4_472_134        0    2      2      3  (da=1,db=2; rm2=0 pendant; σ=(1-2)²×2)
//  Triangle K₃ 8_485_281       3    0      3      3  (all da=db=2; rm2=(2-1)²×3=3; σ=0 regular)
//  Star K_{1,4} 16_492_420     0   36      4      5  (da=4,db=1; rm2=0 pendant; σ=(4-1)²×4=36)
//  Path P₄    7_300_561        1    2      3      4  (mixed; rm2=1 for inner edge B-C)
//  Complete K₄ 25_455_840     24    0      6      4  (all da=db=3; rm2=(3-1)²×6=24; σ=0)
//  2 isolated         0        0    0      0      2
//  K_{2,3}   21_633_306       12    6      6      5  (da=3,db=2; rm2=2×6=12; σ=1×6=6)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (1_414_213, 0, 0, 1, 2)
//  4.  Path P₃ = A-B-C                    → (4_472_134, 0, 2, 2, 3)
//  5.  Triangle K₃ (regular invariants)   → (8_485_281, 3, 0, 3, 3)
//  6.  Star K_{1,4}                       → (16_492_420, 0, 36, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (7_300_561, 1, 2, 3, 4)
//  8.  Complete K₄ (regular invariants)   → (25_455_840, 24, 0, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (21_633_306, 12, 6, 6, 5)
//      Cross-checks: σ>0 (non-regular); RM₂ = 6×(3-1)×(2-1) = 12; SO = 6×√13×10^6

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T4_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX4");
const T4_EXEC:   ExecutorId = ExecutorId::from_ascii("t4.exec");

const T4_KEY_A: &str = "t4.alpha";
const T4_KEY_B: &str = "t4.beta";
const T4_KEY_C: &str = "t4.gamma";
const T4_KEY_D: &str = "t4.delta";
const T4_KEY_E: &str = "t4.epsilon";

const T4_ID_A: NodeId = derive_node_id(T4_PLUGIN, T4_KEY_A);
const T4_ID_B: NodeId = derive_node_id(T4_PLUGIN, T4_KEY_B);
const T4_ID_C: NodeId = derive_node_id(T4_PLUGIN, T4_KEY_C);
const T4_ID_D: NodeId = derive_node_id(T4_PLUGIN, T4_KEY_D);
const T4_ID_E: NodeId = derive_node_id(T4_PLUGIN, T4_KEY_E);

// L4=91 namespace for this harness.
const T4_VEC_A: VectorAddress = VectorAddress::new(91, 1, 1, 0);
const T4_VEC_B: VectorAddress = VectorAddress::new(91, 1, 2, 0);
const T4_VEC_C: VectorAddress = VectorAddress::new(91, 1, 3, 0);
const T4_VEC_D: VectorAddress = VectorAddress::new(91, 2, 1, 0);
const T4_VEC_E: VectorAddress = VectorAddress::new(91, 2, 2, 0);

const T4_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T4_PLUGIN,
    name:         "kl-graph-topo4-harness",
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
        executor_id:       T4_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T4_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T4_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices and counts are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    0, "empty: node_count=0");
    assert_eq!(ec,    0, "empty: edge_count=0");
    assert_eq!(so,    0, "empty: SO=0");
    assert_eq!(rm2,   0, "empty: RM2=0");
    assert_eq!(sigma, 0, "empty: sigma=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with degree 0. No edges, so all indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A);

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    1, "single: node_count=1");
    assert_eq!(ec,    0, "single: no edges");
    assert_eq!(so,    0, "single: SO=0 (no edges)");
    assert_eq!(rm2,   0, "single: RM2=0 (no edges)");
    assert_eq!(sigma, 0, "single: sigma=0 (no edges)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B. Both nodes have degree 1. da=db=1.
//
// SO: isqrt64((1²+1²) × 10^12) = isqrt64(2×10^12) = 1_414_213
//   (1_414_213² = 1,999,999,293,929 < 2×10^12; 1_414_214² > 2×10^12 ✓)
// RM₂: (1-1)·(1-1) = 0 (both endpoints pendant)
// σ:   (1-1)² = 0 (da=db; regular)

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A);
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B);
    add_edge(T4_ID_A, T4_ID_B, "ab");

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    2,         "edge: node_count=2");
    assert_eq!(ec,    1,         "edge: edge_count=1");
    assert_eq!(so,    1_414_213, "edge: SO_ppm=1_414_213 (isqrt64(2×10^12); √(1+1)×10^6)");
    assert_eq!(rm2,   0,         "edge: RM2=0 (both pendant: (1-1)·(1-1)=0)");
    assert_eq!(sigma, 0,         "edge: sigma=0 (da=db=1; (1-1)²=0; regular)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1. 2 undirected edges A-B and B-C.
// Each edge: da=1, db=2.
//
// SO per edge: isqrt64((1²+2²)×10^12) = isqrt64(5×10^12) = 2_236_067
//   (2_236_067² = 4,999,995,628,489 < 5×10^12; 2_236_068² > 5×10^12 ✓)
// RM₂ per edge: (1-1)·(2-1) = 0×1 = 0 (pendant: da=1)
// σ per edge:   (1-2)² = 1
//
// P₃ totals (2 edges):
//   SO_ppm = 2 × 2_236_067 = 4_472_134
//   RM₂    = 2 × 0         = 0
//   σ      = 2 × 1         = 2

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A);
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B);
    add_node(T4_VEC_C, T4_KEY_C, T4_ID_C);
    add_edge(T4_ID_A, T4_ID_B, "ab");
    add_edge(T4_ID_B, T4_ID_C, "bc");

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    3,         "P₃: node_count=3");
    assert_eq!(ec,    2,         "P₃: edge_count=2");
    assert_eq!(so,    4_472_134, "P₃: SO_ppm=4_472_134 (2×2_236_067; √(1+4)×10^6 each)");
    assert_eq!(rm2,   0,         "P₃: RM2=0 (pendant edges: da=1; (1-1)·(2-1)=0 each)");
    assert_eq!(sigma, 2,         "P₃: sigma=2 (2×(1-2)²=2; non-regular)");
}

// ── Test 5: Triangle K₃ (regular graph invariants) ───────────────────────────
// All degrees = 2. 3 undirected edges. Each edge: da=db=2.
//
// SO per edge: isqrt64((2²+2²)×10^12) = isqrt64(8×10^12) = 2_828_427
//   (2_828_427² = 7,999,999,293,929 < 8×10^12; 2_828_428² > 8×10^12 ✓)
// RM₂ per edge: (2-1)·(2-1) = 1×1 = 1
// σ per edge:   (2-2)² = 0 (regular!)
//
// K₃ totals (3 edges, regular graph):
//   SO_ppm = 3 × 2_828_427 = 8_485_281  ≈ 3·Δ·√2·10^6 = 3·2·√2·10^6 (Δ=2)
//   RM₂    = 3 × 1         = 3           = |E|·(Δ-1)² = 3·1²  ✓
//   σ      = 0                            (regular: all da=db=2)

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A);
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B);
    add_node(T4_VEC_C, T4_KEY_C, T4_ID_C);
    add_edge(T4_ID_A, T4_ID_B, "ab");
    add_edge(T4_ID_B, T4_ID_C, "bc");
    add_edge(T4_ID_C, T4_ID_A, "ca");

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    3,         "K₃: node_count=3");
    assert_eq!(ec,    3,         "K₃: edge_count=3");
    assert_eq!(so,    8_485_281, "K₃: SO_ppm=8_485_281 (3×2_828_427; √(4+4)×10^6 each)");
    assert_eq!(rm2,   3,         "K₃: RM2=3 (3×(2-1)·(2-1)=3×1=3; |E|·(Δ-1)²=3·1²)");
    assert_eq!(sigma, 0,         "K₃: sigma=0 (regular: all da=db=2; no imbalance)");

    // σ = 0 is the rigorous regularity test: all degrees equal
    assert_eq!(sigma, 0, "K₃: σ=0 certifies regularity (Δ=2 regular graph)");
    // RM₂ = |E|·(Δ-1)² for Δ-regular: 3·(2-1)² = 3
    assert_eq!(rm2, ec as u64 * 1, "K₃: RM2 = |E|·(Δ-1)² = 3·1 (Δ=2)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center deg=4, 4 leaves deg=1. 4 edges. Each edge: da=4, db=1.
//
// SO per edge: isqrt64((4²+1²)×10^12) = isqrt64(17×10^12) = 4_123_105
//   (4_123_105² = 16,999,995,841,025 < 17×10^12; 4_123_106² > 17×10^12 ✓)
// RM₂ per edge: (4-1)·(1-1) = 3×0 = 0 (leaf endpoint: db=1)
// σ per edge:   (4-1)² = 9
//
// K_{1,4} totals (4 edges):
//   SO_ppm = 4 × 4_123_105 = 16_492_420  ≈ 4·√17·10^6
//   RM₂    = 4 × 0         = 0            (all pendant on leaf side)
//   σ      = 4 × 9         = 36

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A); // center deg=4
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B); // leaf  deg=1
    add_node(T4_VEC_C, T4_KEY_C, T4_ID_C); // leaf  deg=1
    add_node(T4_VEC_D, T4_KEY_D, T4_ID_D); // leaf  deg=1
    add_node(T4_VEC_E, T4_KEY_E, T4_ID_E); // leaf  deg=1
    add_edge(T4_ID_A, T4_ID_B, "ab");
    add_edge(T4_ID_A, T4_ID_C, "ac");
    add_edge(T4_ID_A, T4_ID_D, "ad");
    add_edge(T4_ID_A, T4_ID_E, "ae");

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    5,          "K_{{1,4}}: node_count=5");
    assert_eq!(ec,    4,          "K_{{1,4}}: edge_count=4");
    assert_eq!(so,    16_492_420, "K_{{1,4}}: SO_ppm=16_492_420 (4×4_123_105; √(16+1)×10^6)");
    assert_eq!(rm2,   0,          "K_{{1,4}}: RM2=0 (all pendant; (4-1)·(1-1)=0 each)");
    assert_eq!(sigma, 36,         "K_{{1,4}}: sigma=36 (4×(4-1)²=4×9; non-regular)");

    // σ > 0 confirms non-regularity
    assert!(sigma > 0, "K_{{1,4}}: σ>0 (non-regular graph)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1. 3 edges.
//
// Edge A-B (da=1, db=2):
//   SO=2_236_067; RM₂=(1-1)·(2-1)=0; σ=(1-2)²=1
// Edge B-C (da=2, db=2, inner regular):
//   SO=isqrt64(8×10^12)=2_828_427; RM₂=(2-1)·(2-1)=1; σ=0
// Edge C-D (da=2, db=1):
//   SO=2_236_067; RM₂=0; σ=1
//
// P₄ totals (3 edges):
//   SO_ppm = 2×2_236_067 + 2_828_427 = 7_300_561
//   RM₂    = 0 + 1 + 0              = 1
//   σ      = 1 + 0 + 1              = 2

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A);
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B);
    add_node(T4_VEC_C, T4_KEY_C, T4_ID_C);
    add_node(T4_VEC_D, T4_KEY_D, T4_ID_D);
    add_edge(T4_ID_A, T4_ID_B, "ab");
    add_edge(T4_ID_B, T4_ID_C, "bc");
    add_edge(T4_ID_C, T4_ID_D, "cd");

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    4,         "P₄: node_count=4");
    assert_eq!(ec,    3,         "P₄: edge_count=3");
    assert_eq!(so,    7_300_561, "P₄: SO_ppm=7_300_561 (2×2_236_067+2_828_427)");
    assert_eq!(rm2,   1,         "P₄: RM2=1 (only inner edge B-C: (2-1)·(2-1)=1)");
    assert_eq!(sigma, 2,         "P₄: sigma=2 (2 pendant edges each (1-2)²=1; inner=0)");
}

// ── Test 8: Complete K₄ (regular graph invariants) ───────────────────────────
// All degrees = 3. 6 undirected edges. Each edge: da=db=3.
//
// SO per edge: isqrt64((3²+3²)×10^12) = isqrt64(18×10^12) = 4_242_640
//   (4_242_640² = 17,999,994,169,600 < 18×10^12; 4_242_641² > 18×10^12 ✓)
// RM₂ per edge: (3-1)·(3-1) = 2×2 = 4
// σ per edge:   (3-3)² = 0 (regular!)
//
// K₄ totals (6 edges, regular graph):
//   SO_ppm = 6 × 4_242_640 = 25_455_840  ≈ 6·3·√2·10^6 (Δ=3)
//   RM₂    = 6 × 4         = 24           = |E|·(Δ-1)² = 6·(3-1)² = 6·4 ✓
//   σ      = 0                             (regular: all da=db=3)

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A);
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B);
    add_node(T4_VEC_C, T4_KEY_C, T4_ID_C);
    add_node(T4_VEC_D, T4_KEY_D, T4_ID_D);
    add_edge(T4_ID_A, T4_ID_B, "ab");
    add_edge(T4_ID_A, T4_ID_C, "ac");
    add_edge(T4_ID_A, T4_ID_D, "ad");
    add_edge(T4_ID_B, T4_ID_C, "bc");
    add_edge(T4_ID_B, T4_ID_D, "bd");
    add_edge(T4_ID_C, T4_ID_D, "cd");

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    4,          "K₄: node_count=4");
    assert_eq!(ec,    6,          "K₄: edge_count=6");
    assert_eq!(so,    25_455_840, "K₄: SO_ppm=25_455_840 (6×4_242_640; √(9+9)×10^6 each)");
    assert_eq!(rm2,   24,         "K₄: RM2=24 (6×(3-1)·(3-1)=6×4=24; |E|·(Δ-1)²=6·4)");
    assert_eq!(sigma, 0,          "K₄: sigma=0 (regular: all da=db=3; no imbalance)");

    // Rigorous invariants for K₄ regular graph
    assert_eq!(sigma, 0, "K₄: σ=0 certifies regularity (Δ=3 regular graph)");
    // RM₂ = |E|·(Δ-1)² for Δ-regular: 6·(3-1)² = 6·4 = 24
    assert_eq!(rm2, ec as u64 * (3 - 1) * (3 - 1),
        "K₄: RM2 = |E|·(Δ-1)² = 6·4 = 24 (Δ=3)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Two nodes, no edges. All indices are 0, node_count=2.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A);
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B);

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    2, "2 isolated: node_count=2");
    assert_eq!(ec,    0, "2 isolated: no edges");
    assert_eq!(so,    0, "2 isolated: SO=0");
    assert_eq!(rm2,   0, "2 isolated: RM2=0");
    assert_eq!(sigma, 0, "2 isolated: sigma=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Side-A: A(deg=3), B(deg=3). Side-B: C(deg=2), D(deg=2), E(deg=2). 6 edges.
// All edges: da=3, db=2.
//
// SO per edge: isqrt64((3²+2²)×10^12) = isqrt64(13×10^12) = 3_605_551
//   (3_605_551² = 12,999,998,013,601 < 13×10^12; 3_605_552² > 13×10^12 ✓)
// RM₂ per edge: (3-1)·(2-1) = 2×1 = 2
// σ per edge:   (3-2)² = 1
//
// K_{2,3} totals (6 edges):
//   SO_ppm = 6 × 3_605_551 = 21_633_306  = 6·√13·10^6 (floor)
//   RM₂    = 6 × 2         = 12
//   σ      = 6 × 1         = 6            (non-regular; all edges have |da-db|=1)
//
// Cross-checks:
//   σ > 0 (non-regular: side-A deg=3 ≠ side-B deg=2)
//   RM₂ = 6·(3-1)·(2-1) = 12 (exact; no floor rounding)
//   SO = 6·isqrt64(13×10^12) = 6·3_605_551 (each edge has same da,db)

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    add_node(T4_VEC_A, T4_KEY_A, T4_ID_A); // deg=3
    add_node(T4_VEC_B, T4_KEY_B, T4_ID_B); // deg=3
    add_node(T4_VEC_C, T4_KEY_C, T4_ID_C); // deg=2
    add_node(T4_VEC_D, T4_KEY_D, T4_ID_D); // deg=2
    add_node(T4_VEC_E, T4_KEY_E, T4_ID_E); // deg=2
    add_edge(T4_ID_A, T4_ID_C, "ac");
    add_edge(T4_ID_A, T4_ID_D, "ad");
    add_edge(T4_ID_A, T4_ID_E, "ae");
    add_edge(T4_ID_B, T4_ID_C, "bc");
    add_edge(T4_ID_B, T4_ID_D, "bd");
    add_edge(T4_ID_B, T4_ID_E, "be");

    let (so, rm2, sigma, ec, nc) = gos_runtime::graph_topo_indices4();
    assert_eq!(nc,    5,          "K_{{2,3}}: node_count=5");
    assert_eq!(ec,    6,          "K_{{2,3}}: edge_count=6");
    assert_eq!(so,    21_633_306, "K_{{2,3}}: SO_ppm=21_633_306 (6×3_605_551; √(9+4)×10^6)");
    assert_eq!(rm2,   12,         "K_{{2,3}}: RM2=12 (6×(3-1)·(2-1)=6×2; exact)");
    assert_eq!(sigma, 6,          "K_{{2,3}}: sigma=6 (6×(3-2)²=6×1; non-regular)");

    // σ > 0 confirms non-regularity
    assert!(sigma > 0, "K_{{2,3}}: σ>0 (non-regular: deg3 ≠ deg2)");

    // RM₂ exact: (3-1)·(2-1)=2 per edge, no rounding
    assert_eq!(rm2, 6 * 2, "K_{{2,3}}: RM2 = 6×2 = 12 (exact; integer arithmetic)");

    // SO: uniform isqrt64(13×10^12) per edge
    assert_eq!(so, 6 * 3_605_551, "K_{{2,3}}: SO = 6 × isqrt64(13×10^12) = 6 × 3_605_551");
}
