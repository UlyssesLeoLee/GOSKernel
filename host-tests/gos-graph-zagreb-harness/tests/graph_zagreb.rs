// gos-graph-zagreb-harness — V3.11 Zagreb/Randić/Albertson topology indices
//
// Verifies `gos_runtime::graph_zagreb()`:
//   Returns (m1, m2, randic_ppm, irregularity, edge_count, node_count)
//   - m1            = Σ_v deg(v)²                              (first Zagreb index)
//   - m2            = Σ_{uv∈E} deg(u)×deg(v)                  (second Zagreb index)
//   - randic_ppm    = R × 10^6 where R = Σ 1/√(deg(u)×deg(v)) (Randić 1975; ppm)
//   - irregularity  = Σ_{uv∈E} |deg(u)−deg(v)|                (Albertson 1997; 0=regular)
//   - edge_count    = undirected edges (directed→undirected, no self-loops)
//   - node_count    = live node count
//
// Randić integer precision: contribution = floor(10^12 / floor(sqrt(p × 10^12)))
// where p = deg(u)×deg(v).  Error is at most 1 ppm per edge.
//
// Key analytical values:
//
//  Graph       M1   M2   R_ppm      Irr  edges  notes
//  Empty        0    0        0       0    0
//  1 node       0    0        0       0    0    no edges
//  Edge A-B     2    1  1_000_000     0    1    p=1; R=1 exact
//  Path P₃      6    4  1_414_214     2    2    R=2/√2; floor(10^12/1_414_213)=707_107
//  Triangle K₃ 12   12  1_500_000     0    3    regular; R=3/2=1.5 exact
//  Star K_{1,4}20   16  2_000_000    12    4    p=4; R=4/2=2 exact
//  Path P₄     10    8  1_914_214     2    3    R=2/√2+1/2; 2×707_107+500_000
//  Complete K₄ 36   54  1_999_998     0    6    p=9; R=6/3=2 (floor err -2)
//  2 isolated   0    0        0       0    0    no edges
//  K_{2,3}     30   36  2_449_488     6    6    R=√6 ≈ 2.44949 (floor err -2)
//
// K₄ cross-check (test 10): M2 = 54 = M1 - 2m = 36 + 6×3 = ... actually M2≠M1-2m.
// Verify: K₄ has 4 nodes deg=3 each, 6 edges each (3,3). M2=6×9=54. M1=4×9=36. ✓
//
// Tests (10):
//  1.  Empty graph                       → (0, 0, 0, 0, 0, 0)
//  2.  Single isolated node              → (0, 0, 0, 0, 0, 1)
//  3.  Single directed edge A→B          → (2, 1, 1_000_000, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (6, 4, 1_414_214, 2, 2, 3)
//  5.  Triangle K₃                       → (12, 12, 1_500_000, 0, 3, 3)
//  6.  Star K_{1,4}                      → (20, 16, 2_000_000, 12, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (10, 8, 1_914_214, 2, 3, 4)
//  8.  Complete K₄                       → (36, 54, 1_999_998, 0, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (30, 36, 2_449_488, 6, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const ZG_PLUGIN: PluginId   = PluginId::from_ascii("KL_GRAPH_ZAG_H");
const ZG_EXEC:   ExecutorId = ExecutorId::from_ascii("zagreb.exec");

const ZG_KEY_A: &str = "zagreb.a";
const ZG_KEY_B: &str = "zagreb.b";
const ZG_KEY_C: &str = "zagreb.c";
const ZG_KEY_D: &str = "zagreb.d";
const ZG_KEY_E: &str = "zagreb.e";

const ZG_ID_A: NodeId = derive_node_id(ZG_PLUGIN, ZG_KEY_A);
const ZG_ID_B: NodeId = derive_node_id(ZG_PLUGIN, ZG_KEY_B);
const ZG_ID_C: NodeId = derive_node_id(ZG_PLUGIN, ZG_KEY_C);
const ZG_ID_D: NodeId = derive_node_id(ZG_PLUGIN, ZG_KEY_D);
const ZG_ID_E: NodeId = derive_node_id(ZG_PLUGIN, ZG_KEY_E);

// L4=87 namespace for this harness.
const ZG_VEC_A: VectorAddress = VectorAddress::new(87, 1, 1, 0);
const ZG_VEC_B: VectorAddress = VectorAddress::new(87, 1, 2, 0);
const ZG_VEC_C: VectorAddress = VectorAddress::new(87, 1, 3, 0);
const ZG_VEC_D: VectorAddress = VectorAddress::new(87, 2, 1, 0);
const ZG_VEC_E: VectorAddress = VectorAddress::new(87, 2, 2, 0);

const ZG_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ZG_PLUGIN,
    name:         "kl-graph-zagreb-harness",
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
        executor_id:       ZG_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(ZG_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(ZG_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────
// No nodes, no edges. All indices are 0.

#[test]
fn test_01_empty() {
    let _g = setup();

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(m1,  0, "empty: M1=0");
    assert_eq!(m2,  0, "empty: M2=0");
    assert_eq!(r,   0, "empty: R=0");
    assert_eq!(irr, 0, "empty: irregularity=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// One node with degree 0. M1=0²=0; no edges so M2=R=I=0.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A);

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: edge_count=0");
    assert_eq!(m1,  0, "single: M1=0 (deg=0)");
    assert_eq!(m2,  0, "single: M2=0");
    assert_eq!(r,   0, "single: R=0");
    assert_eq!(irr, 0, "single: I=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected edge A-B. Both nodes have degree 1.
// M1 = 1² + 1² = 2.  M2 = 1×1 = 1.
// R: p=1, s=isqrt_ppm(1)=10^6, contribution=10^12/10^6=10^6. R_ppm=1_000_000.
// I = |1-1| = 0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A);
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B);
    add_edge(ZG_ID_A, ZG_ID_B, "ab");

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  2,         "edge: node_count=2");
    assert_eq!(ec,  1,         "edge: edge_count=1");
    assert_eq!(m1,  2,         "edge: M1=2");
    assert_eq!(m2,  1,         "edge: M2=1");
    assert_eq!(r,   1_000_000, "edge: R_ppm=1_000_000 (R=1 exact)");
    assert_eq!(irr, 0,         "edge: I=0 (both deg-1)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// Degrees: A=1, B=2, C=1.
// M1 = 1² + 2² + 1² = 6.  M2 = (1×2) + (2×1) = 4.
// R: A-B p=2, s=isqrt_ppm(2)=1_414_213, contrib=floor(10^12/1_414_213)=707_107.
//    B-C same. R_ppm = 707_107 + 707_107 = 1_414_214.
// I = |1-2| + |2-1| = 2.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A);
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B);
    add_node(ZG_VEC_C, ZG_KEY_C, ZG_ID_C);
    add_edge(ZG_ID_A, ZG_ID_B, "ab");
    add_edge(ZG_ID_B, ZG_ID_C, "bc");

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  3,         "P3: node_count=3");
    assert_eq!(ec,  2,         "P3: edge_count=2");
    assert_eq!(m1,  6,         "P3: M1=6");
    assert_eq!(m2,  4,         "P3: M2=4");
    assert_eq!(r,   1_414_214, "P3: R_ppm=1_414_214 (floor(10^12/1_414_213)=707_107; 2×707_107)");
    assert_eq!(irr, 2,         "P3: I=2");
}

// ── Test 5: Triangle K₃ ───────────────────────────────────────────────────────
// All three nodes have degree 2 (regular graph).
// M1 = 3×4 = 12.  M2 = 3×(2×2) = 12.
// R: p=4, s=2_000_000, contrib=500_000 per edge. R_ppm = 3×500_000 = 1_500_000.
// I = 0 (regular).

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A);
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B);
    add_node(ZG_VEC_C, ZG_KEY_C, ZG_ID_C);
    add_edge(ZG_ID_A, ZG_ID_B, "ab");
    add_edge(ZG_ID_B, ZG_ID_C, "bc");
    add_edge(ZG_ID_C, ZG_ID_A, "ca");

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  3,         "K3: node_count=3");
    assert_eq!(ec,  3,         "K3: edge_count=3");
    assert_eq!(m1,  12,        "K3: M1=12");
    assert_eq!(m2,  12,        "K3: M2=12");
    assert_eq!(r,   1_500_000, "K3: R_ppm=1_500_000 (R=3/2 exact)");
    assert_eq!(irr, 0,         "K3: I=0 (regular)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Centre A: degree 4. Leaves B,C,D,E: degree 1.
// M1 = 4² + 4×1² = 16+4 = 20.  M2 = 4×(4×1) = 16.
// R: p=4, s=2_000_000, contrib=500_000 each. R_ppm = 4×500_000 = 2_000_000.
// I = 4×|4-1| = 12.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A); // centre
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B);
    add_node(ZG_VEC_C, ZG_KEY_C, ZG_ID_C);
    add_node(ZG_VEC_D, ZG_KEY_D, ZG_ID_D);
    add_node(ZG_VEC_E, ZG_KEY_E, ZG_ID_E);
    add_edge(ZG_ID_A, ZG_ID_B, "ab");
    add_edge(ZG_ID_A, ZG_ID_C, "ac");
    add_edge(ZG_ID_A, ZG_ID_D, "ad");
    add_edge(ZG_ID_A, ZG_ID_E, "ae");

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  5,         "K_{{1,4}}: node_count=5");
    assert_eq!(ec,  4,         "K_{{1,4}}: edge_count=4");
    assert_eq!(m1,  20,        "K_{{1,4}}: M1=20");
    assert_eq!(m2,  16,        "K_{{1,4}}: M2=16");
    assert_eq!(r,   2_000_000, "K_{{1,4}}: R_ppm=2_000_000 (R=2 exact)");
    assert_eq!(irr, 12,        "K_{{1,4}}: I=12");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// Degrees: A=1, B=2, C=2, D=1.
// M1 = 1+4+4+1 = 10.  M2 = (1×2)+(2×2)+(2×1) = 2+4+2 = 8.
// R: A-B: p=2, s=1_414_213, contrib=floor(10^12/1_414_213)=707_107.
//    B-C: p=4, s=2_000_000, contrib=500_000.
//    C-D: p=2, contrib=707_107.
//    R_ppm = 707_107+500_000+707_107 = 1_914_214.  (actual √2+0.5 ≈ 1.914214)
// I = |1-2|+|2-2|+|2-1| = 1+0+1 = 2.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A);
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B);
    add_node(ZG_VEC_C, ZG_KEY_C, ZG_ID_C);
    add_node(ZG_VEC_D, ZG_KEY_D, ZG_ID_D);
    add_edge(ZG_ID_A, ZG_ID_B, "ab");
    add_edge(ZG_ID_B, ZG_ID_C, "bc");
    add_edge(ZG_ID_C, ZG_ID_D, "cd");

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  4,         "P4: node_count=4");
    assert_eq!(ec,  3,         "P4: edge_count=3");
    assert_eq!(m1,  10,        "P4: M1=10");
    assert_eq!(m2,  8,         "P4: M2=8");
    assert_eq!(r,   1_914_214, "P4: R_ppm=1_914_214 (2×707_107+500_000; ≈√2+0.5)");
    assert_eq!(irr, 2,         "P4: I=2");
}

// ── Test 8: Complete K₄ ───────────────────────────────────────────────────────
// All four nodes have degree 3 (regular).
// M1 = 4×9 = 36.  M2 = 6×(3×3) = 54.
// R: p=9, s=3_000_000 (exact), contrib=10^12/3_000_000=333_333 each.
//    R_ppm = 6×333_333 = 1_999_998.  (actual R=6/3=2; floor error = -2)
// I = 0 (regular).

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A);
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B);
    add_node(ZG_VEC_C, ZG_KEY_C, ZG_ID_C);
    add_node(ZG_VEC_D, ZG_KEY_D, ZG_ID_D);
    add_edge(ZG_ID_A, ZG_ID_B, "ab");
    add_edge(ZG_ID_A, ZG_ID_C, "ac");
    add_edge(ZG_ID_A, ZG_ID_D, "ad");
    add_edge(ZG_ID_B, ZG_ID_C, "bc");
    add_edge(ZG_ID_B, ZG_ID_D, "bd");
    add_edge(ZG_ID_C, ZG_ID_D, "cd");

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  4,         "K4: node_count=4");
    assert_eq!(ec,  6,         "K4: edge_count=6");
    assert_eq!(m1,  36,        "K4: M1=36");
    assert_eq!(m2,  54,        "K4: M2=54");
    assert_eq!(r,   1_999_998, "K4: R_ppm=1_999_998 (floor err -2; actual R=2)");
    assert_eq!(irr, 0,         "K4: I=0 (regular)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// No edges, both degree 0. M1=0, M2=0, R=0, I=0.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A);
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B);

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  2, "2-isolated: node_count=2");
    assert_eq!(ec,  0, "2-isolated: edge_count=0");
    assert_eq!(m1,  0, "2-isolated: M1=0");
    assert_eq!(m2,  0, "2-isolated: M2=0");
    assert_eq!(r,   0, "2-isolated: R=0");
    assert_eq!(irr, 0, "2-isolated: I=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left A,B (degree 3); right C,D,E (degree 2). 6 edges.
// M1 = 3²+3²+2²+2²+2² = 9+9+4+4+4 = 30.
// M2 = 6×(3×2) = 36.
// R: p=6 for all 6 edges. s=isqrt_ppm(6)=isqrt64(6×10^12).
//    sqrt(6)≈2.449489742... × 10^6 = 2_449_489.742... → floor = 2_449_489.
//    contrib = 10^12/2_449_489 = 408_248 (floor). R_ppm = 6×408_248 = 2_449_488.
//    (actual R(K_{2,3})=6/√6=√6≈2.449490; floor err = -2)
// I = 6×|3-2| = 6.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(ZG_VEC_A, ZG_KEY_A, ZG_ID_A); // left  A (deg 3)
    add_node(ZG_VEC_B, ZG_KEY_B, ZG_ID_B); // left  B (deg 3)
    add_node(ZG_VEC_C, ZG_KEY_C, ZG_ID_C); // right C (deg 2)
    add_node(ZG_VEC_D, ZG_KEY_D, ZG_ID_D); // right D (deg 2)
    add_node(ZG_VEC_E, ZG_KEY_E, ZG_ID_E); // right E (deg 2)
    add_edge(ZG_ID_A, ZG_ID_C, "ac");
    add_edge(ZG_ID_A, ZG_ID_D, "ad");
    add_edge(ZG_ID_A, ZG_ID_E, "ae");
    add_edge(ZG_ID_B, ZG_ID_C, "bc");
    add_edge(ZG_ID_B, ZG_ID_D, "bd");
    add_edge(ZG_ID_B, ZG_ID_E, "be");

    let (m1, m2, r, irr, ec, nc) = gos_runtime::graph_zagreb();
    assert_eq!(nc,  5,         "K_{{2,3}}: node_count=5");
    assert_eq!(ec,  6,         "K_{{2,3}}: edge_count=6");
    assert_eq!(m1,  30,        "K_{{2,3}}: M1=30");
    assert_eq!(m2,  36,        "K_{{2,3}}: M2=36");
    assert_eq!(r,   2_449_488, "K_{{2,3}}: R_ppm=2_449_488 (≈√6; floor err -2)");
    assert_eq!(irr, 6,         "K_{{2,3}}: I=6");
}
