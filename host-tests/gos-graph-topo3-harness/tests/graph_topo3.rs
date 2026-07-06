// gos-graph-topo3-harness — V3.14 SDD + ISI + Nirmala degree-based topological indices
//
// Verifies `gos_runtime::graph_topo_indices3()`:
//   Returns (sdd_ppm, isi_ppm, ni_ppm, edge_count, node_count)
//   - sdd_ppm = SDD × 10^6 where SDD = Σ_{uv∈E} (da²+db²)/(da·db)  (Vasilyev 2014)
//   - isi_ppm = ISI × 10^6 where ISI = Σ_{uv∈E} da·db/(da+db)       (Sedlar et al. 2011)
//   - ni_ppm  = NI  × 10^6 where NI  = Σ_{uv∈E} √(da+db)            (Rather et al. 2021)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// Integer precision:
//   SDD: contribution = floor((da²+db²) × 10^6 / (da·db))   exact when da=db (regular)
//   ISI: contribution = floor(da·db × 10^6 / (da+db))        exact for Δ-regular
//   NI:  contribution = isqrt64((da+db) × 10^12)             exact when da+db is perfect square
//
// KEY INVARIANTS:
//   SDD regular: For Δ-regular graph, da=db → SDD = 2|E| exactly (AM-GM equality)
//   ISI regular: For Δ-regular graph, ISI = |E|·Δ/2 exactly
//   NI regular:  For Δ-regular graph, NI = |E|·√(2Δ) (exact when 2Δ is perfect square)
//   NI K₃ (Δ=2): NI = 3·√4 = 6 exactly (da+db=4, isqrt64(4×10^12)=2_000_000)
//   SDD ≥ 2|E| always (AM-GM: (da²+db²)/(da·db) ≥ 2, equality iff da=db)
//
// KEY isqrt64 VALUES:
//   isqrt64(2_000_000_000_000) = 1_414_213  (√2 × 10^6; 1_414_213²=1_999_998_722_969 < 2×10^12)
//   isqrt64(3_000_000_000_000) = 1_732_050  (√3 × 10^6; 1_732_050²=2_999_997_202_500 < 3×10^12)
//   isqrt64(4_000_000_000_000) = 2_000_000  (√4 × 10^6; exact)
//   isqrt64(5_000_000_000_000) = 2_236_067  (√5 × 10^6; 2_236_067²=4_999_995_628_489 < 5×10^12)
//   isqrt64(6_000_000_000_000) = 2_449_489  (√6 × 10^6; 2_449_489²=5_999_996_361_121 < 6×10^12)
//
// Analytical cross-check table:
//
//  Graph        SDD_ppm     ISI_ppm    NI_ppm     edges  nodes
//  Empty              0           0         0      0      0
//  1 node             0           0         0      0      1
//  Edge A-B   2_000_000     500_000 1_414_213      1      2
//  Path P₃    5_000_000   1_333_332 3_464_100      2      3
//  Triangle K₃ 6_000_000  3_000_000 6_000_000      3      3  (all regular invariants exact)
//  Star K_{1,4} 17_000_000 3_200_000 8_944_268      4      5
//  Path P₄    7_000_000   2_333_332 5_464_100      3      4
//  Complete K₄ 12_000_000  9_000_000 14_696_934      6      4
//  2 isolated         0           0         0      0      2
//  K_{2,3}   12_999_996   7_200_000 13_416_402      6      5
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (2_000_000, 500_000, 1_414_213, 1, 2)
//  4.  Path P₃ = A-B-C                    → (5_000_000, 1_333_332, 3_464_100, 2, 3)
//  5.  Triangle K₃ (regular invariants)   → (6_000_000, 3_000_000, 6_000_000, 3, 3)
//  6.  Star K_{1,4}                       → (17_000_000, 3_200_000, 8_944_268, 4, 5)
//  7.  Path P₄ = A-B-C-D                  → (7_000_000, 2_333_332, 5_464_100, 3, 4)
//  8.  Complete K₄ (regular invariants)   → (12_000_000, 9_000_000, 14_696_934, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check      → (12_999_996, 7_200_000, 13_416_402, 6, 5)
//      Cross-checks: SDD ≥ 2|E|=12 (non-regular); ISI = 6×6/5 (exact); NI = 6×√5

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T3_PLUGIN: PluginId   = PluginId::from_ascii("TOPO_IX3");
const T3_EXEC:   ExecutorId = ExecutorId::from_ascii("t3.exec");

const T3_KEY_A: &str = "t3.alpha";
const T3_KEY_B: &str = "t3.beta";
const T3_KEY_C: &str = "t3.gamma";
const T3_KEY_D: &str = "t3.delta";
const T3_KEY_E: &str = "t3.epsilon";

const T3_ID_A: NodeId = derive_node_id(T3_PLUGIN, T3_KEY_A);
const T3_ID_B: NodeId = derive_node_id(T3_PLUGIN, T3_KEY_B);
const T3_ID_C: NodeId = derive_node_id(T3_PLUGIN, T3_KEY_C);
const T3_ID_D: NodeId = derive_node_id(T3_PLUGIN, T3_KEY_D);
const T3_ID_E: NodeId = derive_node_id(T3_PLUGIN, T3_KEY_E);

// L4=90 namespace for this harness.
const T3_VEC_A: VectorAddress = VectorAddress::new(90, 1, 1, 0);
const T3_VEC_B: VectorAddress = VectorAddress::new(90, 1, 2, 0);
const T3_VEC_C: VectorAddress = VectorAddress::new(90, 1, 3, 0);
const T3_VEC_D: VectorAddress = VectorAddress::new(90, 2, 1, 0);
const T3_VEC_E: VectorAddress = VectorAddress::new(90, 2, 2, 0);

const T3_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T3_PLUGIN,
    name:         "kl-graph-topo3-harness",
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
        executor_id:       T3_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T3_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T3_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices and counts are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(sdd, 0, "empty: SDD=0");
    assert_eq!(isi, 0, "empty: ISI=0");
    assert_eq!(ni,  0, "empty: NI=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with degree 0. No edges, so all indices are 0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A);

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(sdd, 0, "single: SDD=0 (no edges)");
    assert_eq!(isi, 0, "single: ISI=0 (no edges)");
    assert_eq!(ni,  0, "single: NI=0 (no edges)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected A-B. Both nodes have degree 1. da=db=1; s=2, p=1.
//
// SDD: (1²+1²)/(1·1) = 2 → SDD_ppm = 2_000_000  (da=db, AM-GM equality, regular)
// ISI: 1·1/(1+1) = 1/2 → ISI_ppm = floor(1_000_000/2) = 500_000
// NI:  isqrt64((1+1)×10^12) = isqrt64(2_000_000_000_000) = 1_414_213
//      (1_414_213² = 1_999_998_722_969 < 2×10^12; 1_414_214²=2_000_001_551_396 > 2×10^12)

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A);
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B);
    add_edge(T3_ID_A, T3_ID_B, "ab");

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(sdd, 2_000_000, "edge: SDD_ppm=2_000_000 ((1+1)/1=2 exact; da=db=1)");
    assert_eq!(isi, 500_000,   "edge: ISI_ppm=500_000 (1·1/(1+1)=1/2)");
    assert_eq!(ni,  1_414_213, "edge: NI_ppm=1_414_213 (isqrt64(2×10^12); √2×10^6 floor)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1. 2 undirected edges A-B and B-C.
// Each edge: da=1, db=2; s=3, p=2.
//
// SDD per edge: (1²+2²)/(1·2) = (1+4)/2 = 5/2 → floor(5_000_000/2) = 2_500_000
// ISI per edge: 1·2/(1+2) = 2/3 → floor(2_000_000/3) = 666_666
// NI per edge:  isqrt64(3_000_000_000_000) = 1_732_050
//   (1_732_050² = 2_999_997_202_500 < 3×10^12; 1_732_051²=3_000_000_666_601 > 3×10^12)
//
// P₃ totals (2 edges):
//   SDD_ppm = 2 × 2_500_000 = 5_000_000
//   ISI_ppm = 2 × 666_666   = 1_333_332
//   NI_ppm  = 2 × 1_732_050 = 3_464_100

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A);
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B);
    add_node(T3_VEC_C, T3_KEY_C, T3_ID_C);
    add_edge(T3_ID_A, T3_ID_B, "ab");
    add_edge(T3_ID_B, T3_ID_C, "bc");

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  3,         "P₃: node_count=3");
    assert_eq!(ec,  2,         "P₃: edge_count=2");
    assert_eq!(sdd, 5_000_000, "P₃: SDD_ppm=5_000_000 (2×2_500_000; each edge (1+4)/2)");
    assert_eq!(isi, 1_333_332, "P₃: ISI_ppm=1_333_332 (2×666_666; 2/3 per edge, floor)");
    assert_eq!(ni,  3_464_100, "P₃: NI_ppm=3_464_100 (2×1_732_050; isqrt64(3×10^12))");
}

// ── Test 5: Triangle K₃ (regular graph invariants) ───────────────────────────
// All degrees = 2. 3 undirected edges. Each edge: da=db=2; s=4, p=4.
//
// SDD per edge: (4+4)/4 = 2 → 2_000_000 (exact; da=db regular AM-GM equality)
// ISI per edge: 4/4 = 1 → 1_000_000 (exact; Δ-regular ISI = |E|·Δ/2 = 3·2/2 = 3)
// NI per edge:  isqrt64(4_000_000_000_000) = 2_000_000 (exact; √4=2; perfect square!)
//
// K₃ totals (3 edges, all regular invariants exact):
//   SDD_ppm = 3 × 2_000_000 = 6_000_000  = 2×3×10^6  ✓ (SDD=2|E|=6)
//   ISI_ppm = 3 × 1_000_000 = 3_000_000  = |E|·Δ/2·10^6 = 3·1·10^6 ✓
//   NI_ppm  = 3 × 2_000_000 = 6_000_000  = |E|·√(2Δ)·10^6 = 3·√4·10^6 ✓ (exact)

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A);
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B);
    add_node(T3_VEC_C, T3_KEY_C, T3_ID_C);
    add_edge(T3_ID_A, T3_ID_B, "ab");
    add_edge(T3_ID_B, T3_ID_C, "bc");
    add_edge(T3_ID_C, T3_ID_A, "ca");

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  3,         "K₃: node_count=3");
    assert_eq!(ec,  3,         "K₃: edge_count=3");
    assert_eq!(sdd, 6_000_000, "K₃: SDD_ppm=6_000_000 (SDD=2|E|=6 exact; da=db regular)");
    assert_eq!(isi, 3_000_000, "K₃: ISI_ppm=3_000_000 (ISI=|E|·Δ/2=3 exact)");
    assert_eq!(ni,  6_000_000, "K₃: NI_ppm=6_000_000 (NI=|E|·√(2·2)=6 exact; da+db=4)");

    // Confirm all three regular-graph invariants simultaneously.
    assert_eq!(sdd, 2 * ec as u64 * 1_000_000,
        "K₃: SDD = 2|E| exactly (AM-GM equality on regular graph)");
    // ISI = |E|·Δ/2 = 3·2/2 = 3; ISI_ppm = 3_000_000
    assert_eq!(isi, ec as u64 * 2 / 2 * 1_000_000,
        "K₃: ISI = |E|·Δ/2 exactly (Δ=2, 3-regular)");
    // NI = |E|·√(2Δ) = 3·√4 = 6 (exact, 2Δ=4 perfect square)
    assert_eq!(ni, ec as u64 * 2_000_000,
        "K₃: NI = |E|·√4 = 3·2 exactly (2Δ=4 perfect square)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center deg=4, 4 leaves deg=1. 4 edges. Each edge: da=4, db=1; s=5, p=4.
//
// SDD per edge: (16+1)/(4·1) = 17/4 → floor(17_000_000/4) = 4_250_000
// ISI per edge: 4·1/(4+1) = 4/5 → floor(4_000_000/5) = 800_000 (exact; 5 | 4_000_000)
// NI per edge:  isqrt64(5_000_000_000_000) = 2_236_067
//   (2_236_067²=4_999_995_628_489 < 5×10^12; 2_236_068²=5_000_000_100_624 > 5×10^12)
//
// K_{1,4} totals (4 edges):
//   SDD_ppm = 4 × 4_250_000 = 17_000_000  (SDD=17 exact; 17/4 × 4 edges)
//   ISI_ppm = 4 × 800_000   = 3_200_000   (ISI=16/5=3.2 exact)
//   NI_ppm  = 4 × 2_236_067 = 8_944_268   (NI=4√5; floor error ≈ 4 ppm below 4×√5×10^6)

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A); // center deg=4
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B); // leaf  deg=1
    add_node(T3_VEC_C, T3_KEY_C, T3_ID_C); // leaf  deg=1
    add_node(T3_VEC_D, T3_KEY_D, T3_ID_D); // leaf  deg=1
    add_node(T3_VEC_E, T3_KEY_E, T3_ID_E); // leaf  deg=1
    add_edge(T3_ID_A, T3_ID_B, "ab");
    add_edge(T3_ID_A, T3_ID_C, "ac");
    add_edge(T3_ID_A, T3_ID_D, "ad");
    add_edge(T3_ID_A, T3_ID_E, "ae");

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  5,          "K_{{1,4}}: node_count=5");
    assert_eq!(ec,  4,          "K_{{1,4}}: edge_count=4");
    assert_eq!(sdd, 17_000_000, "K_{{1,4}}: SDD_ppm=17_000_000 (4×4_250_000; each (16+1)/4)");
    assert_eq!(isi, 3_200_000,  "K_{{1,4}}: ISI_ppm=3_200_000 (4×800_000; 4/5 exact per edge)");
    assert_eq!(ni,  8_944_268,  "K_{{1,4}}: NI_ppm=8_944_268 (4×2_236_067; NI=4√5; floor)");

    // SDD > 2|E| for non-regular graph (strict AM-GM inequality)
    assert!(sdd > 2 * ec as u64 * 1_000_000,
        "K_{{1,4}}: SDD > 2|E| (non-regular: AM-GM strict inequality)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1. 3 edges.
//
// Edges A-B and C-D: da=1, db=2 (same as P₃ edges)
//   SDD = 2_500_000; ISI = 666_666; NI = 1_732_050
// Edge B-C: da=2, db=2 (regular)
//   SDD = 2_000_000; ISI = 1_000_000; NI = 2_000_000
//
// P₄ totals (3 edges):
//   SDD_ppm = 2×2_500_000 + 2_000_000 = 7_000_000
//   ISI_ppm = 2×666_666   + 1_000_000 = 2_333_332
//   NI_ppm  = 2×1_732_050 + 2_000_000 = 5_464_100

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A);
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B);
    add_node(T3_VEC_C, T3_KEY_C, T3_ID_C);
    add_node(T3_VEC_D, T3_KEY_D, T3_ID_D);
    add_edge(T3_ID_A, T3_ID_B, "ab");
    add_edge(T3_ID_B, T3_ID_C, "bc");
    add_edge(T3_ID_C, T3_ID_D, "cd");

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  4,         "P₄: node_count=4");
    assert_eq!(ec,  3,         "P₄: edge_count=3");
    assert_eq!(sdd, 7_000_000, "P₄: SDD_ppm=7_000_000 (2×2_500_000+2_000_000)");
    assert_eq!(isi, 2_333_332, "P₄: ISI_ppm=2_333_332 (2×666_666+1_000_000)");
    assert_eq!(ni,  5_464_100, "P₄: NI_ppm=5_464_100 (2×1_732_050+2_000_000)");
}

// ── Test 8: Complete K₄ (regular graph invariants) ───────────────────────────
// All degrees = 3. 6 undirected edges. Each edge: da=db=3; s=6, p=9.
//
// SDD per edge: (9+9)/9 = 2 → 2_000_000 (exact; da=db regular)
// ISI per edge: 9/6 = 3/2 → 1_500_000 (exact; Δ-regular ISI = |E|·Δ/2 = 6·3/2 = 9)
// NI per edge:  isqrt64(6_000_000_000_000) = 2_449_489
//   (2_449_489²=5_999_996_361_121 < 6×10^12; 2_449_490²=6_000_001_260_100 > 6×10^12)
//
// K₄ totals (6 edges):
//   SDD_ppm = 6 × 2_000_000 = 12_000_000  = 2×6×10^6  ✓ (SDD=2|E|=12)
//   ISI_ppm = 6 × 1_500_000 = 9_000_000   = |E|·Δ/2·10^6 = 6·3/2·10^6 ✓
//   NI_ppm  = 6 × 2_449_489 = 14_696_934  ≈ 6√6×10^6 (2Δ=6 not perfect square; floor)

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A);
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B);
    add_node(T3_VEC_C, T3_KEY_C, T3_ID_C);
    add_node(T3_VEC_D, T3_KEY_D, T3_ID_D);
    add_edge(T3_ID_A, T3_ID_B, "ab");
    add_edge(T3_ID_A, T3_ID_C, "ac");
    add_edge(T3_ID_A, T3_ID_D, "ad");
    add_edge(T3_ID_B, T3_ID_C, "bc");
    add_edge(T3_ID_B, T3_ID_D, "bd");
    add_edge(T3_ID_C, T3_ID_D, "cd");

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  4,          "K₄: node_count=4");
    assert_eq!(ec,  6,          "K₄: edge_count=6");
    assert_eq!(sdd, 12_000_000, "K₄: SDD_ppm=12_000_000 (SDD=2|E|=12 exact; da=db=3)");
    assert_eq!(isi, 9_000_000,  "K₄: ISI_ppm=9_000_000 (ISI=|E|·Δ/2=9 exact; Δ=3)");
    assert_eq!(ni,  14_696_934, "K₄: NI_ppm=14_696_934 (6×2_449_489; NI=6√6; floor)");

    // Confirm SDD=2|E| and ISI=|E|·Δ/2 exact for regular graph
    assert_eq!(sdd, 2 * ec as u64 * 1_000_000,
        "K₄: SDD = 2|E| exactly (regular AM-GM equality)");
    assert_eq!(isi, ec as u64 * 3 / 2 * 1_000_000,
        "K₄: ISI = |E|·Δ/2 = 9 exactly (Δ=3, 6 edges)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// Two nodes, no edges. All indices are 0, node_count=2.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A);
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B);

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  2, "2 isolated: node_count=2");
    assert_eq!(ec,  0, "2 isolated: no edges");
    assert_eq!(sdd, 0, "2 isolated: SDD=0");
    assert_eq!(isi, 0, "2 isolated: ISI=0");
    assert_eq!(ni,  0, "2 isolated: NI=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Side-A: A(deg=3), B(deg=3). Side-B: C(deg=2), D(deg=2), E(deg=2). 6 edges.
// All edges: da=3, db=2; s=5, p=6.
//
// SDD per edge: (9+4)/(3·2) = 13/6 → floor(13_000_000/6) = 2_166_666
// ISI per edge: 3·2/(3+2) = 6/5 → floor(6_000_000/5) = 1_200_000 (exact; 5|6_000_000)
// NI per edge:  isqrt64(5_000_000_000_000) = 2_236_067 (same as K_{1,4} edges: s=5)
//
// K_{2,3} totals (6 edges):
//   SDD_ppm = 6 × 2_166_666 = 12_999_996  (SDD=13 exact minus 6×floor_err of 2/3)
//   ISI_ppm = 6 × 1_200_000 = 7_200_000   (ISI=36/5=7.2 exact; 5 divides evenly)
//   NI_ppm  = 6 × 2_236_067 = 13_416_402  (NI=6√5; floor error ≈ 6×0.9 ppm total)
//
// Cross-checks:
//   SDD > 2|E| = 12 (non-regular; strict AM-GM)
//   ISI_ppm = 6 × 1_200_000 exactly (5 | 6_000_000; no floor rounding)
//   NI: same isqrt64 as K_{1,4} edges (both have s=5)

#[test]
fn test_10_k23_cross_check() {
    let _g = setup();
    add_node(T3_VEC_A, T3_KEY_A, T3_ID_A); // deg=3
    add_node(T3_VEC_B, T3_KEY_B, T3_ID_B); // deg=3
    add_node(T3_VEC_C, T3_KEY_C, T3_ID_C); // deg=2
    add_node(T3_VEC_D, T3_KEY_D, T3_ID_D); // deg=2
    add_node(T3_VEC_E, T3_KEY_E, T3_ID_E); // deg=2
    add_edge(T3_ID_A, T3_ID_C, "ac");
    add_edge(T3_ID_A, T3_ID_D, "ad");
    add_edge(T3_ID_A, T3_ID_E, "ae");
    add_edge(T3_ID_B, T3_ID_C, "bc");
    add_edge(T3_ID_B, T3_ID_D, "bd");
    add_edge(T3_ID_B, T3_ID_E, "be");

    let (sdd, isi, ni, ec, nc) = gos_runtime::graph_topo_indices3();
    assert_eq!(nc,  5,          "K_{{2,3}}: node_count=5");
    assert_eq!(ec,  6,          "K_{{2,3}}: edge_count=6");
    assert_eq!(sdd, 12_999_996, "K_{{2,3}}: SDD_ppm=12_999_996 (6×2_166_666; 13/6 per edge)");
    assert_eq!(isi, 7_200_000,  "K_{{2,3}}: ISI_ppm=7_200_000 (6×1_200_000; 6/5 exact; 5|6M)");
    assert_eq!(ni,  13_416_402, "K_{{2,3}}: NI_ppm=13_416_402 (6×2_236_067; NI=6√5; floor)");

    // SDD > 2|E| for non-regular graph
    assert!(sdd > 2 * ec as u64 * 1_000_000,
        "K_{{2,3}}: SDD > 2|E| (non-regular; AM-GM strict inequality)");

    // ISI exact: 6/5 per edge, 5 divides 6_000_000 evenly
    assert_eq!(isi, 6 * 6_000_000 / 5,
        "K_{{2,3}}: ISI_ppm = 6 × 6_000_000/5 (exact; no floor rounding)");

    // NI: same isqrt64 value as K_{1,4} per edge (s=5 in both)
    assert_eq!(ni, 6 * 2_236_067,
        "K_{{2,3}}: NI_ppm = 6 × isqrt64(5×10^12) = 6 × 2_236_067");
}
