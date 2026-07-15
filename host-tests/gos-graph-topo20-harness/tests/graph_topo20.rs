// gos-graph-topo20-harness — V3.31 Modified Sombor SO* + Reciprocal Sombor RSO + Reduced Sombor rSO
//
// Verifies `gos_runtime::graph_topo_indices20()`:
//   Returns (so_star_ppm, rso_ppm, rso_red_ppm, edge_count, node_count)
//   - so_star_ppm = SO*(G) × 10^6 = Σ_{uv∈E} d_u·d_v/√(d_u²+d_v²) × 10^6   (floor ppm)
//   - rso_ppm     = RSO(G) × 10^6 = Σ_{uv∈E} 10^6/√(d_u²+d_v²)              (floor ppm)
//   - rso_red_ppm = rSO(G) × 10^6 = Σ_{uv∈E} √((d_u−1)²+(d_v−1)²) × 10^6   (floor ppm)
//   - edge_count  = undirected non-self-loop edges
//   - node_count  = live node count
//
// DEFINITIONS:
//   SO*(G)  = Σ_{uv∈E} d_u·d_v / √(d_u²+d_v²)   (Ghanbari & Rajabi-Parsa 2021)
//   RSO(G)  = Σ_{uv∈E} 1 / √(d_u²+d_v²)          (Gutman 2022 / He et al. 2022)
//   rSO(G)  = Σ_{uv∈E} √((d_u−1)²+(d_v−1)²)      (Doslic et al. 2022)
//
// IMPLEMENTATION FORMULAS (no float, no_std safe):
//   SO* per edge = isqrt128((d_a·d_b)²·10^12 / (d_a²+d_b²))
//   RSO per edge = isqrt64(10^12 / (d_a²+d_b²))
//   rSO per edge = isqrt64(((d_a-1)²+(d_b-1)²)·10^12)
//
// KEY INVARIANTS:
//   SO*(Δ-regular) per edge = Δ/√2·10^6 → SO*(G) = m·Δ/√2·10^6
//   RSO(Δ-regular) per edge = 1/(Δ√2)·10^6
//   rSO=0 iff all edges are pendant-pendant (d_u=d_v=1) e.g., single edge A-B.
//   pendant-hub edge (d_leaf=1, d_hub=k): rSO contribution = (k-1)·10^6 (exact integer, no isqrt rounding).
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph        SO*(ppm)   RSO(ppm)   rSO(ppm)   edges  nodes
//  Empty             0          0          0         0      0
//  1 node            0          0          0         0      1
//  Edge A-B    707_106    707_106          0         1      2
//  Path P₃   1_788_854    894_426  2_000_000         2      3
//  K₃         4_242_639  1_060_659  4_242_639         3      3
//  K_{1,4}    3_880_568    970_140 12_000_000         4      5
//  Path P₄    3_203_067  1_247_979  3_414_213         3      4
//  K₄        12_727_920  1_414_212 16_970_562         6      4
//  2 isolated        0          0          0         0      2
//  K_{2,3}    9_984_600  1_664_100 13_416_402         6      5
//
// Derivations:
//
//   Edge A-B (d_A=d_B=1):
//     SO*: isqrt128(1²·10^12/2) = isqrt128(500_000_000_000) = 707_106. ✓
//     RSO: isqrt64(10^12/2) = isqrt64(500_000_000_000) = 707_106. ✓
//     rSO: isqrt64(0·10^12) = 0. ✓ (rSO=0: all pendant-pendant annotation)
//
//   Path P₃ = A−B−C (d_A=d_C=1, d_B=2):
//     Edge A-B (d=1,2): SO*=isqrt128(4·10^12/5)=isqrt128(800_000_000_000)=894_427;
//                        RSO=isqrt64(10^12/5)=isqrt64(200_000_000_000)=447_213;
//                        rSO=isqrt64(1·10^12)=1_000_000.
//     Edge B-C identical. SO*=1_788_854; RSO=894_426; rSO=2_000_000. ✓
//
//   Triangle K₃ (d=2 for all; 3 edges):
//     Per edge: SO*=isqrt128(16·10^12/8)=isqrt128(2·10^12)=1_414_213;
//               RSO=isqrt64(10^12/8)=isqrt64(125_000_000_000)=353_553;
//               rSO=isqrt64(2·10^12)=1_414_213.
//     SO*=4_242_639; RSO=1_060_659; rSO=4_242_639. ✓ (SO*=rSO for K₃: both = 3√2×10^6)
//
//   Star K_{1,4} (d_hub=4, d_leaf=1; 4 edges hub-leaf):
//     Per edge (d=4,1): SO*=isqrt128(16·10^12/17)=970_142;
//                        RSO=isqrt64(10^12/17)=isqrt64(58_823_529_411)=242_535;
//                        rSO=isqrt64(9·10^12)=3_000_000 (exact: √9=3).
//     SO*=3_880_568; RSO=970_140; rSO=12_000_000. ✓
//
//   Path P₄ = A−B−C−D (d_A=d_D=1, d_B=d_C=2):
//     Edge A-B (d=1,2): SO*=894_427; RSO=447_213; rSO=1_000_000.
//     Edge B-C (d=2,2): SO*=1_414_213; RSO=353_553; rSO=1_414_213.
//     Edge C-D (d=2,1): SO*=894_427; RSO=447_213; rSO=1_000_000.
//     SO*=3_203_067; RSO=1_247_979; rSO=3_414_213. ✓
//
//   Complete K₄ (d=3 for all; 6 edges):
//     Per edge: SO*=isqrt128(81·10^12/18)=isqrt128(4_500_000_000_000)=2_121_320;
//               RSO=isqrt64(10^12/18)=isqrt64(55_555_555_555)=235_702;
//               rSO=isqrt64(8·10^12)=2_828_427.
//     SO*=12_727_920; RSO=1_414_212; rSO=16_970_562. ✓
//
//   K_{2,3} (d_left=3, d_right=2; 6 edges, each d=3,2):
//     Per edge: SO*=isqrt128(36·10^12/13)=isqrt128(2_769_230_769_230)=1_664_100;
//               RSO=isqrt64(10^12/13)=isqrt64(76_923_076_923)=277_350;
//               rSO=isqrt64(5·10^12)=isqrt64(5_000_000_000_000)=2_236_067.
//     SO*=9_984_600; RSO=1_664_100; rSO=13_416_402. ✓
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (707_106, 707_106, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (1_788_854, 894_426, 2_000_000, 2, 3)
//  5.  Triangle K₃                       → (4_242_639, 1_060_659, 4_242_639, 3, 3)
//  6.  Star K_{1,4}                      → (3_880_568, 970_140, 12_000_000, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (3_203_067, 1_247_979, 3_414_213, 3, 4)
//  8.  Complete K₄                       → (12_727_920, 1_414_212, 16_970_562, 6, 4)
//  9.  Two isolated nodes                 → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (9_984_600, 1_664_100, 13_416_402, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T20_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_20");
const T20_EXEC:   ExecutorId = ExecutorId::from_ascii("t20.exec");

const T20_KEY_A: &str = "t20.alpha";
const T20_KEY_B: &str = "t20.beta";
const T20_KEY_C: &str = "t20.gamma";
const T20_KEY_D: &str = "t20.delta";
const T20_KEY_E: &str = "t20.epsilon";

const T20_ID_A: NodeId = derive_node_id(T20_PLUGIN, T20_KEY_A);
const T20_ID_B: NodeId = derive_node_id(T20_PLUGIN, T20_KEY_B);
const T20_ID_C: NodeId = derive_node_id(T20_PLUGIN, T20_KEY_C);
const T20_ID_D: NodeId = derive_node_id(T20_PLUGIN, T20_KEY_D);
const T20_ID_E: NodeId = derive_node_id(T20_PLUGIN, T20_KEY_E);

// L4=107 namespace for this harness.
const T20_VEC_A: VectorAddress = VectorAddress::new(107, 1, 1, 0);
const T20_VEC_B: VectorAddress = VectorAddress::new(107, 1, 2, 0);
const T20_VEC_C: VectorAddress = VectorAddress::new(107, 1, 3, 0);
const T20_VEC_D: VectorAddress = VectorAddress::new(107, 2, 1, 0);
const T20_VEC_E: VectorAddress = VectorAddress::new(107, 2, 2, 0);

const T20_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T20_PLUGIN,
    name:         "kl-graph-topo20-harness",
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
        executor_id:       T20_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T20_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T20_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   0, "empty: node_count=0");
    assert_eq!(ec,   0, "empty: edge_count=0");
    assert_eq!(so,   0, "empty: SO*=0");
    assert_eq!(rso,  0, "empty: RSO=0");
    assert_eq!(rsom, 0, "empty: rSO=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   1, "single: node_count=1");
    assert_eq!(ec,   0, "single: no edges");
    assert_eq!(so,   0, "single: SO*=0 (no edges)");
    assert_eq!(rso,  0, "single: RSO=0 (no edges)");
    assert_eq!(rsom, 0, "single: rSO=0 (no edges)");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// d_A=d_B=1. sum_sq=2. prod=1.
// SO*: isqrt128(1·10^12/2)=isqrt128(500_000_000_000)=707_106.
// RSO: isqrt64(10^12/2)=isqrt64(500_000_000_000)=707_106.
// rSO: isqrt64(0)=0 — both pendant, (1-1)²+(1-1)²=0. rSO=0 annotation.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);
    add_edge(T20_ID_A, T20_ID_B, "t20.e.ab");

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   2,       "edge: node_count=2");
    assert_eq!(ec,   1,       "edge: edge_count=1");
    assert_eq!(so,   707_106, "edge: SO*=707_106 (isqrt128(10^12/2)=707_106)");
    assert_eq!(rso,  707_106, "edge: RSO=707_106 (isqrt64(10^12/2)=707_106)");
    assert_eq!(rsom, 0,       "edge: rSO=0 (both pendant: (0+0)=0)");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d_A=d_C=1, d_B=2. Two edges each (d=1,2).
// Per edge: SO*=isqrt128(4·10^12/5)=894_427; RSO=isqrt64(10^12/5)=447_213; rSO=isqrt64(10^12)=1_000_000.
// Totals: SO*=1_788_854; RSO=894_426; rSO=2_000_000.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);
    add_node(T20_VEC_C, T20_KEY_C, T20_ID_C);
    add_edge(T20_ID_A, T20_ID_B, "t20.e.ab");
    add_edge(T20_ID_B, T20_ID_C, "t20.e.bc");

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   3,         "p3: node_count=3");
    assert_eq!(ec,   2,         "p3: edge_count=2");
    assert_eq!(so,   1_788_854, "p3: SO*=1_788_854 (2\u{00d7}894_427)");
    assert_eq!(rso,  894_426,   "p3: RSO=894_426 (2\u{00d7}447_213)");
    assert_eq!(rsom, 2_000_000, "p3: rSO=2_000_000 (2\u{00d7}1_000_000; pendant-hub)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// d=2 for all. 3 edges. per edge: sum_sq=8, prod=4.
// SO*=isqrt128(16·10^12/8)=isqrt128(2·10^12)=1_414_213. Total=4_242_639.
// RSO=isqrt64(10^12/8)=isqrt64(125_000_000_000)=353_553. Total=1_060_659.
// rSO=isqrt64(2·10^12)=1_414_213. Total=4_242_639. (SO*=rSO for K₃: both = 3√2·10^6)

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);
    add_node(T20_VEC_C, T20_KEY_C, T20_ID_C);
    add_edge(T20_ID_A, T20_ID_B, "t20.e.ab");
    add_edge(T20_ID_B, T20_ID_A, "t20.e.ba");
    add_edge(T20_ID_B, T20_ID_C, "t20.e.bc");
    add_edge(T20_ID_C, T20_ID_B, "t20.e.cb");
    add_edge(T20_ID_A, T20_ID_C, "t20.e.ac");
    add_edge(T20_ID_C, T20_ID_A, "t20.e.ca");

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   3,         "k3: node_count=3");
    assert_eq!(ec,   3,         "k3: edge_count=3");
    assert_eq!(so,   4_242_639, "k3: SO*=4_242_639 (3\u{00d7}1_414_213; SO*=rSO for d=2)");
    assert_eq!(rso,  1_060_659, "k3: RSO=1_060_659 (3\u{00d7}353_553)");
    assert_eq!(rsom, 4_242_639, "k3: rSO=4_242_639 (3\u{00d7}1_414_213; SO*=rSO here)");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// d_center=4, d_leaf=1. 4 hub-leaf edges. sum_sq=17, prod=4.
// SO*: isqrt128(16·10^12/17)=970_142. Total=3_880_568.
// RSO: isqrt64(10^12/17)=isqrt64(58_823_529_411)=242_535. Total=970_140.
// rSO: isqrt64(9·10^12)=3_000_000 (exact; (4-1)²+(1-1)²=9). Total=12_000_000.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);
    add_node(T20_VEC_C, T20_KEY_C, T20_ID_C);
    add_node(T20_VEC_D, T20_KEY_D, T20_ID_D);
    add_node(T20_VEC_E, T20_KEY_E, T20_ID_E);
    add_edge(T20_ID_A, T20_ID_B, "t20.e.ab");
    add_edge(T20_ID_A, T20_ID_C, "t20.e.ac");
    add_edge(T20_ID_A, T20_ID_D, "t20.e.ad");
    add_edge(T20_ID_A, T20_ID_E, "t20.e.ae");

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   5,          "star: node_count=5");
    assert_eq!(ec,   4,          "star: edge_count=4");
    assert_eq!(so,   3_880_568,  "star: SO*=3_880_568 (4\u{00d7}970_142)");
    assert_eq!(rso,  970_140,    "star: RSO=970_140 (4\u{00d7}242_535)");
    assert_eq!(rsom, 12_000_000, "star: rSO=12_000_000 (4\u{00d7}3_000_000 exact; (3)\u{00b2}+0\u{00b2}=9)");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d_A=d_D=1, d_B=d_C=2. Edges: A-B(d=1,2), B-C(d=2,2), C-D(d=2,1).
// A-B: SO*=894_427; RSO=447_213; rSO=1_000_000.
// B-C: SO*=1_414_213; RSO=353_553; rSO=1_414_213.
// C-D: SO*=894_427; RSO=447_213; rSO=1_000_000.
// Totals: SO*=3_203_067; RSO=1_247_979; rSO=3_414_213.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);
    add_node(T20_VEC_C, T20_KEY_C, T20_ID_C);
    add_node(T20_VEC_D, T20_KEY_D, T20_ID_D);
    add_edge(T20_ID_A, T20_ID_B, "t20.e.ab");
    add_edge(T20_ID_B, T20_ID_C, "t20.e.bc");
    add_edge(T20_ID_C, T20_ID_D, "t20.e.cd");

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   4,         "p4: node_count=4");
    assert_eq!(ec,   3,         "p4: edge_count=3");
    assert_eq!(so,   3_203_067, "p4: SO*=3_203_067 (894_427+1_414_213+894_427)");
    assert_eq!(rso,  1_247_979, "p4: RSO=1_247_979 (447_213+353_553+447_213)");
    assert_eq!(rsom, 3_414_213, "p4: rSO=3_414_213 (1_000_000+1_414_213+1_000_000)");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// d=3 for all. 6 edges. per edge sum_sq=18, prod=9.
// SO*=isqrt128(81·10^12/18)=isqrt128(4_500_000_000_000)=2_121_320. Total=12_727_920.
// RSO=isqrt64(10^12/18)=isqrt64(55_555_555_555)=235_702. Total=1_414_212.
// rSO=isqrt64(8·10^12)=2_828_427. Total=16_970_562.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);
    add_node(T20_VEC_C, T20_KEY_C, T20_ID_C);
    add_node(T20_VEC_D, T20_KEY_D, T20_ID_D);
    add_edge(T20_ID_A, T20_ID_B, "t20.e.ab");
    add_edge(T20_ID_B, T20_ID_A, "t20.e.ba");
    add_edge(T20_ID_A, T20_ID_C, "t20.e.ac");
    add_edge(T20_ID_C, T20_ID_A, "t20.e.ca");
    add_edge(T20_ID_A, T20_ID_D, "t20.e.ad");
    add_edge(T20_ID_D, T20_ID_A, "t20.e.da");
    add_edge(T20_ID_B, T20_ID_C, "t20.e.bc");
    add_edge(T20_ID_C, T20_ID_B, "t20.e.cb");
    add_edge(T20_ID_B, T20_ID_D, "t20.e.bd");
    add_edge(T20_ID_D, T20_ID_B, "t20.e.db");
    add_edge(T20_ID_C, T20_ID_D, "t20.e.cd");
    add_edge(T20_ID_D, T20_ID_C, "t20.e.dc");

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   4,          "k4: node_count=4");
    assert_eq!(ec,   6,          "k4: edge_count=6");
    assert_eq!(so,   12_727_920, "k4: SO*=12_727_920 (6\u{00d7}2_121_320)");
    assert_eq!(rso,  1_414_212,  "k4: RSO=1_414_212 (6\u{00d7}235_702)");
    assert_eq!(rsom, 16_970_562, "k4: rSO=16_970_562 (6\u{00d7}2_828_427)");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   2, "isolated: node_count=2");
    assert_eq!(ec,   0, "isolated: no edges");
    assert_eq!(so,   0, "isolated: SO*=0");
    assert_eq!(rso,  0, "isolated: RSO=0");
    assert_eq!(rsom, 0, "isolated: rSO=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}:d=3. Right={C,D,E}:d=2. 6 edges, each (d=3,2). sum_sq=13, prod=6.
// SO*: isqrt128(36·10^12/13)=isqrt128(2_769_230_769_230)=1_664_100. Total=9_984_600.
// RSO: isqrt64(10^12/13)=isqrt64(76_923_076_923)=277_350. Total=1_664_100.
// rSO: isqrt64(5·10^12)=isqrt64(5_000_000_000_000)=2_236_067. Total=13_416_402.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T20_VEC_A, T20_KEY_A, T20_ID_A);
    add_node(T20_VEC_B, T20_KEY_B, T20_ID_B);
    add_node(T20_VEC_C, T20_KEY_C, T20_ID_C);
    add_node(T20_VEC_D, T20_KEY_D, T20_ID_D);
    add_node(T20_VEC_E, T20_KEY_E, T20_ID_E);
    add_edge(T20_ID_A, T20_ID_C, "t20.e.ac");
    add_edge(T20_ID_C, T20_ID_A, "t20.e.ca");
    add_edge(T20_ID_A, T20_ID_D, "t20.e.ad");
    add_edge(T20_ID_D, T20_ID_A, "t20.e.da");
    add_edge(T20_ID_A, T20_ID_E, "t20.e.ae");
    add_edge(T20_ID_E, T20_ID_A, "t20.e.ea");
    add_edge(T20_ID_B, T20_ID_C, "t20.e.bc");
    add_edge(T20_ID_C, T20_ID_B, "t20.e.cb");
    add_edge(T20_ID_B, T20_ID_D, "t20.e.bd");
    add_edge(T20_ID_D, T20_ID_B, "t20.e.db");
    add_edge(T20_ID_B, T20_ID_E, "t20.e.be");
    add_edge(T20_ID_E, T20_ID_B, "t20.e.eb");

    let (so, rso, rsom, ec, nc) = gos_runtime::graph_topo_indices20();
    assert_eq!(nc,   5,          "k23: node_count=5");
    assert_eq!(ec,   6,          "k23: edge_count=6");
    assert_eq!(so,   9_984_600,  "k23: SO*=9_984_600 (6\u{00d7}1_664_100)");
    assert_eq!(rso,  1_664_100,  "k23: RSO=1_664_100 (6\u{00d7}277_350)");
    assert_eq!(rsom, 13_416_402, "k23: rSO=13_416_402 (6\u{00d7}2_236_067)");
}
