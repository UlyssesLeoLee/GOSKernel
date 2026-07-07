// gos-graph-topo6-harness — V3.17 EM₁ + ABS + RRR degree-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices6()`:
//   Returns (em1, abs_ppm, rrr_ppm, edge_count, node_count)
//   - em1    = EM₁(G)    where EM₁ = Σ_{uv∈E} (da+db-2)²               (exact u64)
//   - abs_ppm = ABS×10^6  where ABS = Σ_{uv∈E} √((da+db-2)/(da+db))    (floor isqrt64)
//   - rrr_ppm = RRR×10^6  where RRR = Σ_{uv∈E} √((da-1)·(db-1))        (floor isqrt64)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// Integer precision:
//   EM₁:  contribution = q² where q = da+db-2; exact integer always
//   ABS:  contribution = isqrt64(q × 10^12 / s) where s = da+db; floor error ≤ 1 ppm per edge
//   RRR:  contribution = isqrt64((da-1)×(db-1)×10^12);            floor error ≤ 1 ppm per edge
//
// KEY INVARIANTS:
//   EM₁ = 4·|E|·(Δ-1)²   for Δ-regular graphs   (= 0 for pendant-only graphs)
//   RRR = |E|·(Δ-1)·10^6 for Δ-regular graphs   (exact: isqrt((Δ-1)² × 10^12) = (Δ-1)×10^6)
//   RRR = 0               iff all edges are pendant (at least one endpoint has degree 1)
//   EM₁ = 0 / ABS = 0     when da=db=1 for all edges (both pendant, q=0)
//
// KEY isqrt64 VALUES:
//   isqrt64(500_000_000_000) = 707_106  (√(1/2) × 10^6; q=2,s=4 i.e. da=db=2)
//   isqrt64(333_333_333_333) = 577_350  (√(1/3) × 10^6; q=1,s=3 i.e. da=1,db=2)
//   isqrt64(600_000_000_000) = 774_596  (√(3/5) × 10^6; q=3,s=5 i.e. da+db=5,q=3)
//   isqrt64(666_666_666_666) = 816_496  (√(2/3) × 10^6; q=4,s=6 i.e. da=db=3)
//   isqrt64(1_000_000_000_000) = 1_000_000  (√1 × 10^6; (da-1)(db-1)=1: da=db=2)
//   isqrt64(2_000_000_000_000) = 1_414_213  (√2 × 10^6; (da-1)(db-1)=2: da=2,db=3)
//   isqrt64(4_000_000_000_000) = 2_000_000  (√4 × 10^6; (da-1)(db-1)=4: da=db=3, exact)
//
// Analytical cross-check table:
//
//  Graph           EM₁   ABS_ppm    RRR_ppm
//  Empty             0         0          0
//  1 node            0         0          0
//  Edge A-B          0         0          0  (da=db=1; q=0; pendant: RRR=0)
//  Path P₃           2 1_154_700          0  (da=1,db=2 each; q=1,s=3; pendant: RRR=0)
//  Triangle K₃      12 2_121_318  3_000_000  (da=db=2; q=2,s=4; (da-1)(db-1)=1; exact)
//  Star K_{1,4}     36 3_098_384          0  (da=4,db=1; q=3,s=5; all pendant: RRR=0)
//  Path P₄           6 1_861_806  1_000_000  (mix of s=3 and s=4 edges)
//  Complete K₄      96 4_898_976 12_000_000  (da=db=3; 4·6·4=96; 6·2·10^6=12M)
//  2 isolated        0         0          0
//  K_{2,3}          54 4_647_576  8_485_278  (da=3,db=2; q=3,s=5; (da-1)(db-1)=2)
//
// Per-edge derivations:
//   da=db=1 (A-B edge): q=0; EM₁=0; ABS=isqrt64(0)=0; RRR=isqrt64(0)=0
//   da=1,db=2 (P₃ leaf edge): q=1,s=3; EM₁=1; ABS=isqrt64(333_333_333_333)=577_350; RRR=isqrt64(0)=0
//   da=db=2 (K₃ edge): q=2,s=4; EM₁=4; ABS=isqrt64(500_000_000_000)=707_106; RRR=isqrt64(10^12)=10^6
//   da=4,db=1 (K_{1,4} edge): q=3,s=5; EM₁=9; ABS=isqrt64(600_000_000_000)=774_596; RRR=isqrt64(0)=0
//   da=db=3 (K₄ edge): q=4,s=6; EM₁=16; ABS=isqrt64(666_666_666_666)=816_496; RRR=isqrt64(4·10^12)=2·10^6
//   da=2,db=3 (K_{2,3} edge): q=3,s=5; EM₁=9; ABS=774_596; RRR=isqrt64(2·10^12)=1_414_213
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (0, 0, 0, 1, 2)
//  4.  Path P₃ = A-B-C                    → (2, 1_154_700, 0, 2, 3)
//  5.  Triangle K₃ (regular invariants)   → (12, 2_121_318, 3_000_000, 3, 3)
//  6.  Star K_{1,4}                       → (36, 3_098_384, 0, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (6, 1_861_806, 1_000_000, 3, 4)
//  8.  Complete K₄ (regular invariants)   → (96, 4_898_976, 12_000_000, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (54, 4_647_576, 8_485_278, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T6_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX6");
const T6_EXEC:   ExecutorId = ExecutorId::from_ascii("t6.exec");

const T6_KEY_A: &str = "t6.alpha";
const T6_KEY_B: &str = "t6.beta";
const T6_KEY_C: &str = "t6.gamma";
const T6_KEY_D: &str = "t6.delta";
const T6_KEY_E: &str = "t6.epsilon";

const T6_ID_A: NodeId = derive_node_id(T6_PLUGIN, T6_KEY_A);
const T6_ID_B: NodeId = derive_node_id(T6_PLUGIN, T6_KEY_B);
const T6_ID_C: NodeId = derive_node_id(T6_PLUGIN, T6_KEY_C);
const T6_ID_D: NodeId = derive_node_id(T6_PLUGIN, T6_KEY_D);
const T6_ID_E: NodeId = derive_node_id(T6_PLUGIN, T6_KEY_E);

// L4=93 namespace for this harness.
const T6_VEC_A: VectorAddress = VectorAddress::new(93, 1, 1, 0);
const T6_VEC_B: VectorAddress = VectorAddress::new(93, 1, 2, 0);
const T6_VEC_C: VectorAddress = VectorAddress::new(93, 1, 3, 0);
const T6_VEC_D: VectorAddress = VectorAddress::new(93, 2, 1, 0);
const T6_VEC_E: VectorAddress = VectorAddress::new(93, 2, 2, 0);

const T6_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T6_PLUGIN,
    name:         "kl-graph-topo6-harness",
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
        executor_id:       T6_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T6_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T6_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices and counts are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(em1, 0, "empty: EM1=0");
    assert_eq!(abs, 0, "empty: ABS=0");
    assert_eq!(rrr, 0, "empty: RRR=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with degree 0. No edges, so all indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A);

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(em1, 0, "single: EM1=0 (no edges)");
    assert_eq!(abs, 0, "single: ABS=0 (no edges)");
    assert_eq!(rrr, 0, "single: RRR=0 (no edges)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B. Both nodes have degree 1. da=db=1.
//
// q = da+db-2 = 0; s = da+db = 2
// EM₁ = q² = 0
// ABS = isqrt64(q × 10^12 / s) = isqrt64(0) = 0
// RRR = isqrt64((da-1)×(db-1)×10^12) = isqrt64(0×0×10^12) = 0
//
// All three are 0 for a pendant-pair edge: this is the minimum-degree case.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A);
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B);
    add_edge(T6_ID_A, T6_ID_B, "ab");

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  2, "edge: node_count=2");
    assert_eq!(ec,  1, "edge: edge_count=1");
    assert_eq!(em1, 0, "edge: EM1=0 (da=db=1; q=(1+1-2)=0)");
    assert_eq!(abs, 0, "edge: ABS=0 (q=0 → isqrt64(0)=0)");
    assert_eq!(rrr, 0, "edge: RRR=0 (da=1→(da-1)=0; pendant)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1. 2 undirected edges A-B and B-C.
// Each edge: da=1, db=2. s=3, q=1.
//
// EM₁ per edge: q² = 1; total = 2×1 = 2
// ABS per edge: isqrt64(1 × 10^12 / 3) = isqrt64(333_333_333_333) = 577_350
//   total = 2×577_350 = 1_154_700
// RRR per edge: isqrt64((1-1)×(2-1)×10^12) = isqrt64(0×1×10^12) = 0 (pendant: da=1)
//   total = 0
//
// Verification: √(1/3) × 10^6 = 577_350.27... → floor 577_350 ✓
// P₃ is non-regular (d_A=1 ≠ d_B=2); RRR=0 as all edges are pendant.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A);
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B);
    add_node(T6_VEC_C, T6_KEY_C, T6_ID_C);
    add_edge(T6_ID_A, T6_ID_B, "ab");
    add_edge(T6_ID_B, T6_ID_C, "bc");

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  3,         "P₃: node_count=3");
    assert_eq!(ec,  2,         "P₃: edge_count=2");
    assert_eq!(em1, 2,         "P₃: EM1=2 (2×(1+2-2)²=2×1)");
    assert_eq!(abs, 1_154_700, "P₃: ABS=1_154_700 (2×577_350; isqrt64(333_333_333_333))");
    assert_eq!(rrr, 0,         "P₃: RRR=0 (all pendant: da=1 at each edge endpoint)");

    // P₃: all edges pendant → RRR=0
    assert_eq!(rrr, 0, "P₃: RRR=0 (pendant edges have (da-1)(db-1)=0)");
}

// ── Test 5: Triangle K₃ (regular graph invariants) ───────────────────────────
// All degrees = 2. 3 undirected edges. Each edge: da=db=2. s=4, q=2.
//
// EM₁ per edge: q² = 4; total = 3×4 = 12  = 4·|E|·(Δ-1)² = 4×3×1² = 12 ✓
// ABS per edge: isqrt64(2 × 10^12 / 4) = isqrt64(500_000_000_000) = 707_106
//   total = 3×707_106 = 2_121_318
// RRR per edge: isqrt64((2-1)×(2-1)×10^12) = isqrt64(10^12) = 1_000_000 (exact)
//   total = 3×1_000_000 = 3_000_000  = |E|·(Δ-1)·10^6 = 3×1×10^6 ✓
//
// Verification: √(1/2) × 10^6 = 707_106.78... → floor 707_106 ✓
// Regularity: EM₁=4m(Δ-1)²=12; RRR=m(Δ-1)×10^6=3_000_000 (exact integers both)

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A);
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B);
    add_node(T6_VEC_C, T6_KEY_C, T6_ID_C);
    add_edge(T6_ID_A, T6_ID_B, "ab");
    add_edge(T6_ID_B, T6_ID_C, "bc");
    add_edge(T6_ID_C, T6_ID_A, "ca");

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  3,         "K₃: node_count=3");
    assert_eq!(ec,  3,         "K₃: edge_count=3");
    assert_eq!(em1, 12,        "K₃: EM1=12 (3×(2+2-2)²=3×4; 4·m·(Δ-1)²=4×3×1=12)");
    assert_eq!(abs, 2_121_318, "K₃: ABS=2_121_318 (3×707_106; isqrt64(500_000_000_000))");
    assert_eq!(rrr, 3_000_000, "K₃: RRR=3_000_000 (3×10^6; exact: m·(Δ-1)·10^6=3×1×10^6)");

    // Regularity invariants (Δ=2)
    assert_eq!(em1, 4 * ec as u64 * 1, "K₃: EM1=4·m·(Δ-1)²=4×3×1=12");
    assert_eq!(rrr, ec as u64 * 1 * 1_000_000, "K₃: RRR=m·(Δ-1)·10^6=3×1×10^6=3_000_000 (exact)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center deg=4, 4 leaves deg=1. 4 edges. Each edge: da=4, db=1. s=5, q=3.
//
// EM₁ per edge: q² = 9; total = 4×9 = 36
// ABS per edge: isqrt64(3 × 10^12 / 5) = isqrt64(600_000_000_000) = 774_596
//   total = 4×774_596 = 3_098_384
// RRR per edge: isqrt64((4-1)×(1-1)×10^12) = isqrt64(3×0×10^12) = 0 (db=1 pendant)
//   total = 0
//
// Verification: √(3/5) × 10^6 = 774_596.67... → floor 774_596 ✓
// Star: all leaf edges are pendant (db=1 → (db-1)=0) → RRR=0 always.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A); // center deg=4
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B); // leaf  deg=1
    add_node(T6_VEC_C, T6_KEY_C, T6_ID_C); // leaf  deg=1
    add_node(T6_VEC_D, T6_KEY_D, T6_ID_D); // leaf  deg=1
    add_node(T6_VEC_E, T6_KEY_E, T6_ID_E); // leaf  deg=1
    add_edge(T6_ID_A, T6_ID_B, "ab");
    add_edge(T6_ID_A, T6_ID_C, "ac");
    add_edge(T6_ID_A, T6_ID_D, "ad");
    add_edge(T6_ID_A, T6_ID_E, "ae");

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  5,         "K_{{1,4}}: node_count=5");
    assert_eq!(ec,  4,         "K_{{1,4}}: edge_count=4");
    assert_eq!(em1, 36,        "K_{{1,4}}: EM1=36 (4×(4+1-2)²=4×9)");
    assert_eq!(abs, 3_098_384, "K_{{1,4}}: ABS=3_098_384 (4×774_596; isqrt64(600_000_000_000))");
    assert_eq!(rrr, 0,         "K_{{1,4}}: RRR=0 (all pendant: db=1 at each leaf → (db-1)=0)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1. 3 edges.
//
// Edge A-B (da=1, db=2): q=1, s=3; EM₁+=1; ABS+=isqrt64(333_333_333_333)=577_350; RRR+=0
// Edge B-C (da=2, db=2): q=2, s=4; EM₁+=4; ABS+=isqrt64(500_000_000_000)=707_106; RRR+=isqrt64(10^12)=10^6
// Edge C-D (da=2, db=1): q=1, s=3; EM₁+=1; ABS+=577_350; RRR+=0
//
// P₄ totals (3 edges):
//   EM₁ = 1 + 4 + 1 = 6
//   ABS_ppm = 577_350 + 707_106 + 577_350 = 1_861_806
//   RRR_ppm = 0 + 10^6 + 0 = 1_000_000

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A);
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B);
    add_node(T6_VEC_C, T6_KEY_C, T6_ID_C);
    add_node(T6_VEC_D, T6_KEY_D, T6_ID_D);
    add_edge(T6_ID_A, T6_ID_B, "ab");
    add_edge(T6_ID_B, T6_ID_C, "bc");
    add_edge(T6_ID_C, T6_ID_D, "cd");

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  4,         "P₄: node_count=4");
    assert_eq!(ec,  3,         "P₄: edge_count=3");
    assert_eq!(em1, 6,         "P₄: EM1=6 (1+4+1; A-B and C-D q=1; B-C q=2)");
    assert_eq!(abs, 1_861_806, "P₄: ABS=1_861_806 (577_350+707_106+577_350)");
    assert_eq!(rrr, 1_000_000, "P₄: RRR=1_000_000 (only B-C interior: isqrt64(10^12)=10^6)");

    // P₄ middle edge B-C is the only interior edge (da=db=2; contributes RRR=10^6)
    assert_eq!(rrr, 1_000_000, "P₄: interior edge B-C gives exact RRR=10^6");
}

// ── Test 8: Complete K₄ (regular graph invariants) ───────────────────────────
// All degrees = 3. 6 undirected edges. Each edge: da=db=3. s=6, q=4.
//
// EM₁ per edge: q² = 16; total = 6×16 = 96  = 4·|E|·(Δ-1)² = 4×6×4 = 96 ✓
// ABS per edge: isqrt64(4 × 10^12 / 6) = isqrt64(666_666_666_666) = 816_496
//   total = 6×816_496 = 4_898_976
// RRR per edge: isqrt64((3-1)×(3-1)×10^12) = isqrt64(4×10^12) = 2_000_000 (exact: √4=2)
//   total = 6×2_000_000 = 12_000_000 = |E|·(Δ-1)·10^6 = 6×2×10^6 ✓
//
// Verification: √(2/3) × 10^6 = 816_496.58... → floor 816_496 ✓
// Regularity: EM₁=4m(Δ-1)²=96; RRR=m(Δ-1)×10^6=12_000_000 (both exact)

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A);
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B);
    add_node(T6_VEC_C, T6_KEY_C, T6_ID_C);
    add_node(T6_VEC_D, T6_KEY_D, T6_ID_D);
    add_edge(T6_ID_A, T6_ID_B, "ab");
    add_edge(T6_ID_A, T6_ID_C, "ac");
    add_edge(T6_ID_A, T6_ID_D, "ad");
    add_edge(T6_ID_B, T6_ID_C, "bc");
    add_edge(T6_ID_B, T6_ID_D, "bd");
    add_edge(T6_ID_C, T6_ID_D, "cd");

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  4,          "K₄: node_count=4");
    assert_eq!(ec,  6,          "K₄: edge_count=6");
    assert_eq!(em1, 96,         "K₄: EM1=96 (6×(3+3-2)²=6×16; 4·m·(Δ-1)²=4×6×4=96)");
    assert_eq!(abs, 4_898_976,  "K₄: ABS=4_898_976 (6×816_496; isqrt64(666_666_666_666))");
    assert_eq!(rrr, 12_000_000, "K₄: RRR=12_000_000 (6×2·10^6; exact: (Δ-1)²=4; √4=2 exact)");

    // Regularity invariants (Δ=3)
    assert_eq!(em1, 4 * ec as u64 * 4, "K₄: EM1=4·m·(Δ-1)²=4×6×4=96");
    assert_eq!(rrr, ec as u64 * 2 * 1_000_000, "K₄: RRR=m·(Δ-1)·10^6=6×2×10^6=12_000_000");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Two nodes, no edges. All indices are 0, node_count=2.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A);
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B);

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  2, "2 isolated: node_count=2");
    assert_eq!(ec,  0, "2 isolated: no edges");
    assert_eq!(em1, 0, "2 isolated: EM1=0");
    assert_eq!(abs, 0, "2 isolated: ABS=0");
    assert_eq!(rrr, 0, "2 isolated: RRR=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Side-A: A(deg=3), B(deg=3). Side-B: C(deg=2), D(deg=2), E(deg=2). 6 edges.
// All edges: da=3, db=2. s=5, q=3.
//
// EM₁ per edge: q² = 9; total = 6×9 = 54
// ABS per edge: isqrt64(3 × 10^12 / 5) = isqrt64(600_000_000_000) = 774_596
//   total = 6×774_596 = 4_647_576
// RRR per edge: isqrt64((3-1)×(2-1)×10^12) = isqrt64(2×10^12) = 1_414_213
//   total = 6×1_414_213 = 8_485_278
//
// Cross-checks:
//   EM₁ = 6×9 = 54 (exact; q=3 for all edges)
//   ABS = 6×774_596 = 4_647_576 (non-regular: ABS < ABS_of_regular_5_edge_pair)
//   RRR = 6×1_414_213 = 8_485_278 (non-regular: da≠db)
//   Verification: √(3/5)×10^6 = 774_596.67... ✓; √2×10^6 = 1_414_213.56... ✓

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    add_node(T6_VEC_A, T6_KEY_A, T6_ID_A); // deg=3
    add_node(T6_VEC_B, T6_KEY_B, T6_ID_B); // deg=3
    add_node(T6_VEC_C, T6_KEY_C, T6_ID_C); // deg=2
    add_node(T6_VEC_D, T6_KEY_D, T6_ID_D); // deg=2
    add_node(T6_VEC_E, T6_KEY_E, T6_ID_E); // deg=2
    add_edge(T6_ID_A, T6_ID_C, "ac");
    add_edge(T6_ID_A, T6_ID_D, "ad");
    add_edge(T6_ID_A, T6_ID_E, "ae");
    add_edge(T6_ID_B, T6_ID_C, "bc");
    add_edge(T6_ID_B, T6_ID_D, "bd");
    add_edge(T6_ID_B, T6_ID_E, "be");

    let (em1, abs, rrr, ec, nc) = gos_runtime::graph_topo_indices6();
    assert_eq!(nc,  5,         "K_{{2,3}}: node_count=5");
    assert_eq!(ec,  6,         "K_{{2,3}}: edge_count=6");
    assert_eq!(em1, 54,        "K_{{2,3}}: EM1=54 (6×(3+2-2)²=6×9; exact)");
    assert_eq!(abs, 4_647_576, "K_{{2,3}}: ABS=4_647_576 (6×774_596; isqrt64(600_000_000_000))");
    assert_eq!(rrr, 8_485_278, "K_{{2,3}}: RRR=8_485_278 (6×1_414_213; isqrt64(2·10^12))");

    // Exact integer cross-checks
    assert_eq!(em1, 6 * 9,  "K_{{2,3}}: EM1=6×9=54 (q=3 per edge)");
    assert_eq!(rrr, 6 * 1_414_213, "K_{{2,3}}: RRR=6×1_414_213 (√2×10^6 per edge; non-exact)");
}
