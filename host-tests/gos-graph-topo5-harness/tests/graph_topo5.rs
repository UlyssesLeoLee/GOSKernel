// gos-graph-topo5-harness — V3.16 HM₁ + HM₂ + AG degree-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices5()`:
//   Returns (hm1, hm2, ag_ppm, edge_count, node_count)
//   - hm1    = HM₁(G)    where HM₁ = Σ_{uv∈E} (da+db)²                  (exact u64)
//   - hm2    = HM₂(G)    where HM₂ = Σ_{uv∈E} (da·db)²                  (exact u64)
//   - ag_ppm = AG × 10^6 where AG  = Σ_{uv∈E} (da+db)/(2√(da·db))       (floor isqrt64)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// Integer precision:
//   HM₁: contribution = s² where s = da + db; exact integer always
//   HM₂: contribution = p² where p = da · db; exact integer always
//   AG:  contribution = floor(s × 10^12 / (2 × isqrt64(p × 10^12)))
//        floor error ≤ 1 ppm per edge
//
// KEY INVARIANTS:
//   AG = |E| (ag_ppm = edge_count × 10^6) iff graph is regular (AM-GM equality: da=db)
//   HM₁ = 4·|E|·Δ² for Δ-regular graphs
//   HM₂ = |E|·Δ⁴   for Δ-regular graphs
//   AG ≥ |E| always (AM ≥ GM; equality iff regular)
//
// KEY isqrt64 VALUES for AG:
//   isqrt64(1_000_000_000_000)  = 1_000_000  (√1 × 10^6; p=1: pendant edge da=db=1)
//   isqrt64(2_000_000_000_000)  = 1_414_213  (√2 × 10^6; p=2: da=1,db=2)
//   isqrt64(4_000_000_000_000)  = 2_000_000  (√4 × 10^6; p=4: da=db=2 or da=1,db=4)
//   isqrt64(6_000_000_000_000)  = 2_449_489  (√6 × 10^6; p=6: da=2,db=3)
//   isqrt64(9_000_000_000_000)  = 3_000_000  (√9 × 10^6; p=9: da=db=3)
//
// AG per-edge derivation:
//   contribution = floor(s × 10^12 / (2 × isqrt64(p × 10^12)))
//   da=db → s=2Δ, p=Δ², isqrt64(Δ²×10^12)=Δ×10^6 → floor(2Δ×10^12/(2×Δ×10^6))=10^6  ✓ (regular=1)
//   da=1,db=4 → s=5,p=4, x=2_000_000 → floor(5×10^12/4_000_000)=1_250_000 (AG=1.25 per edge)
//   da=2,db=3 → s=5,p=6, x=2_449_489 → floor(5×10^12/4_898_978)=1_020_620 (AG≈1.0206)
//
// Analytical cross-check table:
//
//  Graph           HM₁    HM₂      AG_ppm
//  Empty              0      0           0
//  1 node             0      0           0
//  Edge A-B           4      1   1_000_000  (da=db=1; regular: AG=1.0)
//  Path P₃           18      8   2_121_320  (2×1_060_660; s=3,p=2 per edge)
//  Triangle K₃       48     48   3_000_000  (regular Δ=2; AG=3=|E|)
//  Star K_{1,4}     100     64   5_000_000  (4×1_250_000; s=5,p=4 per edge)
//  Path P₄           34     24   3_121_320  (1_060_660+1_000_000+1_060_660)
//  Complete K₄      216    486   6_000_000  (regular Δ=3; AG=6=|E|)
//  2 isolated         0      0           0
//  K_{2,3}          150    216   6_123_726  (6×1_020_621; s=5,p=6 per edge)
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (4, 1, 1_000_000, 1, 2)
//  4.  Path P₃ = A-B-C                    → (18, 8, 2_121_320, 2, 3)
//  5.  Triangle K₃ (regular invariants)   → (48, 48, 3_000_000, 3, 3)
//  6.  Star K_{1,4}                       → (100, 64, 5_000_000, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (34, 24, 3_121_320, 3, 4)
//  8.  Complete K₄ (regular invariants)   → (216, 486, 6_000_000, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (150, 216, 6_123_720, 6, 5)
//      Cross-checks: AG>6_000_000 (non-regular; AM>GM); HM₁=6×25; HM₂=6×36

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T5_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX5");
const T5_EXEC:   ExecutorId = ExecutorId::from_ascii("t5.exec");

const T5_KEY_A: &str = "t5.alpha";
const T5_KEY_B: &str = "t5.beta";
const T5_KEY_C: &str = "t5.gamma";
const T5_KEY_D: &str = "t5.delta";
const T5_KEY_E: &str = "t5.epsilon";

const T5_ID_A: NodeId = derive_node_id(T5_PLUGIN, T5_KEY_A);
const T5_ID_B: NodeId = derive_node_id(T5_PLUGIN, T5_KEY_B);
const T5_ID_C: NodeId = derive_node_id(T5_PLUGIN, T5_KEY_C);
const T5_ID_D: NodeId = derive_node_id(T5_PLUGIN, T5_KEY_D);
const T5_ID_E: NodeId = derive_node_id(T5_PLUGIN, T5_KEY_E);

// L4=92 namespace for this harness.
const T5_VEC_A: VectorAddress = VectorAddress::new(92, 1, 1, 0);
const T5_VEC_B: VectorAddress = VectorAddress::new(92, 1, 2, 0);
const T5_VEC_C: VectorAddress = VectorAddress::new(92, 1, 3, 0);
const T5_VEC_D: VectorAddress = VectorAddress::new(92, 2, 1, 0);
const T5_VEC_E: VectorAddress = VectorAddress::new(92, 2, 2, 0);

const T5_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T5_PLUGIN,
    name:         "kl-graph-topo5-harness",
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
        executor_id:       T5_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T5_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T5_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices and counts are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(hm1, 0, "empty: HM1=0");
    assert_eq!(hm2, 0, "empty: HM2=0");
    assert_eq!(ag,  0, "empty: AG=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with degree 0. No edges, so all indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A);

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(hm1, 0, "single: HM1=0 (no edges)");
    assert_eq!(hm2, 0, "single: HM2=0 (no edges)");
    assert_eq!(ag,  0, "single: AG=0 (no edges)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B. Both nodes have degree 1. da=db=1.
//
// s = 1+1 = 2; p = 1×1 = 1
// HM₁ = s² = 4
// HM₂ = p² = 1
// AG: x = isqrt64(1×10^12) = 10^6; contribution = floor(2×10^12/(2×10^6)) = 1_000_000
// (regular: da=db; AM=GM → AG=1.0 = |E|×1.0)

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A);
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B);
    add_edge(T5_ID_A, T5_ID_B, "ab");

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(hm1, 4,         "edge: HM1=4 (s²=(1+1)²=4)");
    assert_eq!(hm2, 1,         "edge: HM2=1 (p²=(1×1)²=1)");
    assert_eq!(ag,  1_000_000, "edge: AG=1_000_000 (da=db=1; regular: AM=GM)");

    // Regular graph: AG = |E| × 10^6
    assert_eq!(ag, ec as u64 * 1_000_000, "edge: AG=m×10^6 (regular)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1. 2 undirected edges A-B and B-C.
// Each edge: da=1, db=2. s=3, p=2.
//
// HM₁ per edge: s² = 9; total = 2×9 = 18
// HM₂ per edge: p² = 4; total = 2×4 = 8
// AG: x = isqrt64(2×10^12) = 1_414_213
//   per edge = floor(3×10^12 / (2×1_414_213)) = floor(3×10^12 / 2_828_426) = 1_060_660
//   total = 2×1_060_660 = 2_121_320
//
// Verification: (1+2)/(2√2) = 3/(2√2) = 3√2/4 ≈ 1.06066... per edge ✓

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A);
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B);
    add_node(T5_VEC_C, T5_KEY_C, T5_ID_C);
    add_edge(T5_ID_A, T5_ID_B, "ab");
    add_edge(T5_ID_B, T5_ID_C, "bc");

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  3,         "P₃: node_count=3");
    assert_eq!(ec,  2,         "P₃: edge_count=2");
    assert_eq!(hm1, 18,        "P₃: HM1=18 (2×(1+2)²=2×9)");
    assert_eq!(hm2, 8,         "P₃: HM2=8 (2×(1×2)²=2×4)");
    assert_eq!(ag,  2_121_320, "P₃: AG=2_121_320 (2×1_060_660; s=3,p=2; 3√2/4×10^6 each)");

    // P₃ is not regular (d_A=1≠d_B=2), so AG > edge_count × 10^6
    assert!(ag > ec as u64 * 1_000_000, "P₃: AG>m (non-regular: AM>GM)");
}

// ── Test 5: Triangle K₃ (regular graph invariants) ───────────────────────────
// All degrees = 2. 3 undirected edges. Each edge: da=db=2. s=4, p=4.
//
// HM₁ per edge: s² = 16; total = 3×16 = 48  = 4·|E|·Δ² = 4×3×4 = 48 ✓
// HM₂ per edge: p² = 16; total = 3×16 = 48  = |E|·Δ⁴   = 3×16 = 48 ✓
// AG: x = isqrt64(4×10^12) = 2_000_000
//   per edge = floor(4×10^12 / (2×2_000_000)) = floor(4×10^12/4_000_000) = 1_000_000
//   total = 3×1_000_000 = 3_000_000 = |E|×10^6 (regular: AM=GM) ✓

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A);
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B);
    add_node(T5_VEC_C, T5_KEY_C, T5_ID_C);
    add_edge(T5_ID_A, T5_ID_B, "ab");
    add_edge(T5_ID_B, T5_ID_C, "bc");
    add_edge(T5_ID_C, T5_ID_A, "ca");

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  3,         "K₃: node_count=3");
    assert_eq!(ec,  3,         "K₃: edge_count=3");
    assert_eq!(hm1, 48,        "K₃: HM1=48 (3×(2+2)²=3×16; 4·|E|·Δ²=4×3×4=48)");
    assert_eq!(hm2, 48,        "K₃: HM2=48 (3×(2×2)²=3×16; |E|·Δ⁴=3×16=48)");
    assert_eq!(ag,  3_000_000, "K₃: AG=3_000_000 (regular Δ=2; AG=|E|; AM=GM)");

    // Regularity invariants
    assert_eq!(ag, ec as u64 * 1_000_000, "K₃: AG=m×10^6 (regular)");
    // HM₁ = 4·|E|·Δ² for Δ-regular (Δ=2)
    assert_eq!(hm1, 4 * ec as u64 * 4, "K₃: HM1=4·|E|·Δ²=4×3×4=48");
    // HM₂ = |E|·Δ⁴ for Δ-regular (Δ=2)
    assert_eq!(hm2, ec as u64 * 16, "K₃: HM2=|E|·Δ⁴=3×16=48");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center deg=4, 4 leaves deg=1. 4 edges. Each edge: da=4, db=1. s=5, p=4.
//
// HM₁ per edge: s² = 25; total = 4×25 = 100
// HM₂ per edge: p² = 16; total = 4×16 = 64
// AG: x = isqrt64(4×10^12) = 2_000_000
//   per edge = floor(5×10^12 / (2×2_000_000)) = floor(5×10^12/4_000_000) = 1_250_000
//   total = 4×1_250_000 = 5_000_000
//
// Verification: (4+1)/(2√4) = 5/4 = 1.25 per edge × 4 = 5.0 (exact) ✓

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A); // center deg=4
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B); // leaf  deg=1
    add_node(T5_VEC_C, T5_KEY_C, T5_ID_C); // leaf  deg=1
    add_node(T5_VEC_D, T5_KEY_D, T5_ID_D); // leaf  deg=1
    add_node(T5_VEC_E, T5_KEY_E, T5_ID_E); // leaf  deg=1
    add_edge(T5_ID_A, T5_ID_B, "ab");
    add_edge(T5_ID_A, T5_ID_C, "ac");
    add_edge(T5_ID_A, T5_ID_D, "ad");
    add_edge(T5_ID_A, T5_ID_E, "ae");

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  5,         "K_{{1,4}}: node_count=5");
    assert_eq!(ec,  4,         "K_{{1,4}}: edge_count=4");
    assert_eq!(hm1, 100,       "K_{{1,4}}: HM1=100 (4×(4+1)²=4×25)");
    assert_eq!(hm2, 64,        "K_{{1,4}}: HM2=64 (4×(4×1)²=4×16)");
    assert_eq!(ag,  5_000_000, "K_{{1,4}}: AG=5_000_000 (4×1_250_000; (4+1)/(2√4)=5/4 exact)");

    // AG > m (non-regular star): AM > GM
    assert!(ag > ec as u64 * 1_000_000, "K_{{1,4}}: AG>m (non-regular: center deg 4 ≠ leaf deg 1)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1. 3 edges.
//
// Edge A-B (da=1, db=2): s=3, p=2; HM₁+=9, HM₂+=4, AG+=1_060_660
// Edge B-C (da=2, db=2): s=4, p=4; HM₁+=16, HM₂+=16, AG+=1_000_000
// Edge C-D (da=2, db=1): s=3, p=2; HM₁+=9, HM₂+=4, AG+=1_060_660
//
// P₄ totals (3 edges):
//   HM₁ = 9 + 16 + 9 = 34
//   HM₂ = 4 + 16 + 4 = 24
//   AG_ppm = 1_060_660 + 1_000_000 + 1_060_660 = 3_121_320

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A);
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B);
    add_node(T5_VEC_C, T5_KEY_C, T5_ID_C);
    add_node(T5_VEC_D, T5_KEY_D, T5_ID_D);
    add_edge(T5_ID_A, T5_ID_B, "ab");
    add_edge(T5_ID_B, T5_ID_C, "bc");
    add_edge(T5_ID_C, T5_ID_D, "cd");

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  4,         "P₄: node_count=4");
    assert_eq!(ec,  3,         "P₄: edge_count=3");
    assert_eq!(hm1, 34,        "P₄: HM1=34 (9+16+9; A-B and C-D s=3; B-C s=4)");
    assert_eq!(hm2, 24,        "P₄: HM2=24 (4+16+4; A-B and C-D p=2; B-C p=4)");
    assert_eq!(ag,  3_121_320, "P₄: AG=3_121_320 (1_060_660+1_000_000+1_060_660)");
}

// ── Test 8: Complete K₄ (regular graph invariants) ───────────────────────────
// All degrees = 3. 6 undirected edges. Each edge: da=db=3. s=6, p=9.
//
// HM₁ per edge: s² = 36; total = 6×36 = 216  = 4·|E|·Δ² = 4×6×9 = 216 ✓
// HM₂ per edge: p² = 81; total = 6×81 = 486  = |E|·Δ⁴   = 6×81 = 486 ✓
// AG: x = isqrt64(9×10^12) = 3_000_000 (exact: √9=3)
//   per edge = floor(6×10^12 / (2×3_000_000)) = floor(6×10^12/6_000_000) = 1_000_000
//   total = 6×1_000_000 = 6_000_000 = |E|×10^6 (regular: AM=GM) ✓

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A);
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B);
    add_node(T5_VEC_C, T5_KEY_C, T5_ID_C);
    add_node(T5_VEC_D, T5_KEY_D, T5_ID_D);
    add_edge(T5_ID_A, T5_ID_B, "ab");
    add_edge(T5_ID_A, T5_ID_C, "ac");
    add_edge(T5_ID_A, T5_ID_D, "ad");
    add_edge(T5_ID_B, T5_ID_C, "bc");
    add_edge(T5_ID_B, T5_ID_D, "bd");
    add_edge(T5_ID_C, T5_ID_D, "cd");

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  4,         "K₄: node_count=4");
    assert_eq!(ec,  6,         "K₄: edge_count=6");
    assert_eq!(hm1, 216,       "K₄: HM1=216 (6×(3+3)²=6×36; 4·|E|·Δ²=4×6×9=216)");
    assert_eq!(hm2, 486,       "K₄: HM2=486 (6×(3×3)²=6×81; |E|·Δ⁴=6×81=486)");
    assert_eq!(ag,  6_000_000, "K₄: AG=6_000_000 (regular Δ=3; AG=|E|; x=3_000_000 exact)");

    // Rigorous invariants for K₄ regular graph
    assert_eq!(ag, ec as u64 * 1_000_000, "K₄: AG=m×10^6 (regular)");
    // HM₁ = 4·|E|·Δ² for Δ-regular (Δ=3): 4×6×9=216
    assert_eq!(hm1, 4 * ec as u64 * 9, "K₄: HM1=4·|E|·Δ²=4×6×9=216");
    // HM₂ = |E|·Δ⁴ for Δ-regular (Δ=3): 6×81=486
    assert_eq!(hm2, ec as u64 * 81, "K₄: HM2=|E|·Δ⁴=6×81=486");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Two nodes, no edges. All indices are 0, node_count=2.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A);
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B);

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  2, "2 isolated: node_count=2");
    assert_eq!(ec,  0, "2 isolated: no edges");
    assert_eq!(hm1, 0, "2 isolated: HM1=0");
    assert_eq!(hm2, 0, "2 isolated: HM2=0");
    assert_eq!(ag,  0, "2 isolated: AG=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Side-A: A(deg=3), B(deg=3). Side-B: C(deg=2), D(deg=2), E(deg=2). 6 edges.
// All edges: da=3, db=2. s=5, p=6.
//
// HM₁ per edge: s² = 25; total = 6×25 = 150
// HM₂ per edge: p² = 36; total = 6×36 = 216
// AG: x = isqrt64(6×10^12) = 2_449_489 (floor(√6 × 10^6) = floor(2_449_489.742...))
//   2x = 4_898_978
//   per edge = floor(5×10^12 / 4_898_978) = 1_020_621
//     verify: 4_898_978 × 1_020_621 = 4_999_999_825_338 < 5×10^12
//             4_898_978 × 1_020_622 = 5_000_004_724_316 > 5×10^12 ✓
//   total = 6×1_020_621 = 6_123_726
//
// Cross-checks:
//   AG > 6_000_000 = |E|×10^6 (non-regular; AM>GM: da=3 ≠ db=2)
//   HM₁ = 6×(2+3)² = 6×25 = 150 (exact; (da+db)² per edge)
//   HM₂ = 6×(2×3)² = 6×36 = 216 (exact; (da·db)² per edge)

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    add_node(T5_VEC_A, T5_KEY_A, T5_ID_A); // deg=3
    add_node(T5_VEC_B, T5_KEY_B, T5_ID_B); // deg=3
    add_node(T5_VEC_C, T5_KEY_C, T5_ID_C); // deg=2
    add_node(T5_VEC_D, T5_KEY_D, T5_ID_D); // deg=2
    add_node(T5_VEC_E, T5_KEY_E, T5_ID_E); // deg=2
    add_edge(T5_ID_A, T5_ID_C, "ac");
    add_edge(T5_ID_A, T5_ID_D, "ad");
    add_edge(T5_ID_A, T5_ID_E, "ae");
    add_edge(T5_ID_B, T5_ID_C, "bc");
    add_edge(T5_ID_B, T5_ID_D, "bd");
    add_edge(T5_ID_B, T5_ID_E, "be");

    let (hm1, hm2, ag, ec, nc) = gos_runtime::graph_topo_indices5();
    assert_eq!(nc,  5,         "K_{{2,3}}: node_count=5");
    assert_eq!(ec,  6,         "K_{{2,3}}: edge_count=6");
    assert_eq!(hm1, 150,       "K_{{2,3}}: HM1=150 (6×(3+2)²=6×25; exact)");
    assert_eq!(hm2, 216,       "K_{{2,3}}: HM2=216 (6×(3×2)²=6×36; exact)");
    assert_eq!(ag,  6_123_726, "K_{{2,3}}: AG=6_123_726 (6×1_020_621; s=5,p=6; floor(5e12/4_898_978)=1_020_621)");

    // AG > m confirms non-regularity (AM > GM when da ≠ db)
    assert!(ag > ec as u64 * 1_000_000, "K_{{2,3}}: AG>m (non-regular: deg3 ≠ deg2)");

    // HM₁ and HM₂ are exact integers (no floor rounding)
    assert_eq!(hm1, 6 * 25, "K_{{2,3}}: HM1 = 6×25 = 150 (exact)");
    assert_eq!(hm2, 6 * 36, "K_{{2,3}}: HM2 = 6×36 = 216 (exact)");

    // AG uniform: each edge has same (da,db)=(3,2) so 6 × isqrt64 contribution
    assert_eq!(ag, 6 * 1_020_621, "K_{{2,3}}: AG = 6×1_020_621 (uniform s=5,p=6 per edge; floor(5e12/4_898_978))");
}
