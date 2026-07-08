// gos-graph-topo15-harness — V3.26 Leap Zagreb topological indices
//
// Verifies `gos_runtime::graph_topo_indices15()`:
//   Returns (lm1, lm2, lm3, edge_count, node_count)
//   - lm1 = LM₁(G) = Σ_v d₂(v)²                          (exact u64; Naji et al. 2017)
//   - lm2 = LM₂(G) = Σ_{uv∈E} d₂(u)·d₂(v)               (exact u64)
//   - lm3 = LM₃(G) = Σ_{uv∈E} (d₂(u)+d₂(v))             (exact u64)
//   - edge_count = undirected non-self-loop edges
//   - node_count = live node count
//
// DEFINITIONS:
//   d₂(v) = |{w : d(v,w) = 2}| = 2-distance degree (2nd-order degree).
//   d(v,w) = BFS shortest-path distance on the undirected projection.
//   Isolated/unreachable nodes: d(v,w)=∞, never counted.
//
// KEY INVARIANTS:
//   LM₁ = LM₂ = LM₃ = 0 for complete graphs (all pairs adjacent → d₂=0 everywhere).
//   LM₂ = 0 whenever every edge {u,v} has at least one endpoint with d₂=0
//         (star graphs: center has d₂=0).
//   LM₁(K_{1,k}): center d₂=0; each leaf d₂=k-1. LM₁ = k·(k-1)².
//   LM₃(K_{1,k}): each of k edges contributes (0+(k-1))=k-1. LM₃ = k·(k-1).
//
// ANALYTICAL CROSS-CHECK TABLE:
//
//  Graph           LM₁   LM₂  LM₃   edges  nodes
//  Empty             0     0    0      0      0
//  1 node            0     0    0      0      1
//  Edge A-B          0     0    0      1      2
//  Path P₃           2     0    2      2      3
//  Triangle K₃       0     0    0      3      3
//  Star K_{1,4}     36     0   12      4      5
//  Path P₄           4     3    6      3      4
//  Complete K₄       0     0    0      6      4
//  Two isolated      0     0    0      0      2
//  K_{2,3}          14    12   18      6      5
//
// Derivations:
//
//   Edge A-B: d(A,B)=1. No vertex at distance 2. d₂(A)=d₂(B)=0.
//     LM₁=0. LM₂=0. LM₃=0.
//
//   Path P₃ (A-B-C):
//     From A: d(A,B)=1, d(A,C)=2. → d₂(A)=1.
//     From B: d(B,A)=1, d(B,C)=1. → d₂(B)=0.
//     From C: d(C,B)=1, d(C,A)=2. → d₂(C)=1.
//     LM₁ = 1²+0²+1² = 2.
//     LM₂: edge{A,B}: 1×0=0; edge{B,C}: 0×1=0. LM₂=0.
//     LM₃: edge{A,B}: 1+0=1; edge{B,C}: 0+1=1. LM₃=2.
//
//   K₃: all pairs adjacent → d₂=0 for all. LM₁=LM₂=LM₃=0.
//
//   K_{1,4} (center A, leaves B,C,D,E):
//     From A: B,C,D,E all at dist 1. d₂(A)=0.
//     From B: d(B,A)=1; d(B,C)=d(B,D)=d(B,E)=2. d₂(B)=3.
//     Similarly d₂(C)=d₂(D)=d₂(E)=3.
//     LM₁ = 0²+4×3² = 36.
//     LM₂: each edge{A,leaf}: d₂(A)×d₂(leaf)=0×3=0. LM₂=0.
//     LM₃: each edge: 0+3=3. LM₃=4×3=12.
//
//   Path P₄ (A-B-C-D):
//     From A: d(A,B)=1, d(A,C)=2, d(A,D)=3. d₂(A)=1.
//     From B: d(B,A)=1, d(B,C)=1, d(B,D)=2. d₂(B)=1.
//     From C: d(C,B)=1, d(C,D)=1, d(C,A)=2. d₂(C)=1.
//     From D: d(D,C)=1, d(D,B)=2, d(D,A)=3. d₂(D)=1.
//     LM₁=1+1+1+1=4.
//     LM₂: edge{A,B}: 1×1=1; {B,C}: 1×1=1; {C,D}: 1×1=1. LM₂=3.
//     LM₃: each edge: 1+1=2. LM₃=3×2=6.
//
//   K₄: all pairs adjacent → d₂=0. LM₁=LM₂=LM₃=0.
//
//   K_{2,3} (left={A,B}, right={C,D,E}):
//     From A: d(A,C)=d(A,D)=d(A,E)=1; d(A,B)=2. d₂(A)=1.
//     From B: d(B,C)=d(B,D)=d(B,E)=1; d(B,A)=2. d₂(B)=1.
//     From C: d(C,A)=d(C,B)=1; d(C,D)=d(C,E)=2. d₂(C)=2.
//     d₂(D)=d₂(E)=2.
//     LM₁ = 2×1²+3×2² = 2+12 = 14.
//     LM₂: 6 edges (left-right); each: d₂(left)×d₂(right)=1×2=2. LM₂=6×2=12.
//     LM₃: each edge: 1+2=3. LM₃=6×3=18.
//
// Tests (10):
//  1.  Empty graph                        → (0, 0, 0, 0, 0)
//  2.  Single isolated node               → (0, 0, 0, 0, 1)
//  3.  Single directed edge A→B           → (0, 0, 0, 1, 2)
//  4.  Path P₃ = A-B-C                   → (2, 0, 2, 2, 3)
//  5.  Triangle K₃                       → (0, 0, 0, 3, 3)
//  6.  Star K_{1,4}                      → (36, 0, 12, 4, 5)
//  7.  Path P₄ = A-B-C-D                 → (4, 3, 6, 3, 4)
//  8.  Complete K₄                       → (0, 0, 0, 6, 4)
//  9.  Two isolated nodes                → (0, 0, 0, 0, 2)
// 10.  K_{2,3} bipartite cross-check     → (14, 12, 18, 6, 5)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const T15_PLUGIN: PluginId   = PluginId::from_ascii("TOPIX_15");
const T15_EXEC:   ExecutorId = ExecutorId::from_ascii("t15.exec");

const T15_KEY_A: &str = "t15.alpha";
const T15_KEY_B: &str = "t15.beta";
const T15_KEY_C: &str = "t15.gamma";
const T15_KEY_D: &str = "t15.delta";
const T15_KEY_E: &str = "t15.epsilon";

const T15_ID_A: NodeId = derive_node_id(T15_PLUGIN, T15_KEY_A);
const T15_ID_B: NodeId = derive_node_id(T15_PLUGIN, T15_KEY_B);
const T15_ID_C: NodeId = derive_node_id(T15_PLUGIN, T15_KEY_C);
const T15_ID_D: NodeId = derive_node_id(T15_PLUGIN, T15_KEY_D);
const T15_ID_E: NodeId = derive_node_id(T15_PLUGIN, T15_KEY_E);

// L4=102 namespace for this harness.
const T15_VEC_A: VectorAddress = VectorAddress::new(102, 1, 1, 0);
const T15_VEC_B: VectorAddress = VectorAddress::new(102, 1, 2, 0);
const T15_VEC_C: VectorAddress = VectorAddress::new(102, 1, 3, 0);
const T15_VEC_D: VectorAddress = VectorAddress::new(102, 2, 1, 0);
const T15_VEC_E: VectorAddress = VectorAddress::new(102, 2, 2, 0);

const T15_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    T15_PLUGIN,
    name:         "kl-graph-topo15-harness",
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
        executor_id:       T15_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(T15_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(T15_MANIFEST).unwrap();
    g
}

// ── Test 1: Empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty() {
    let _g = setup();

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  0, "empty: node_count=0");
    assert_eq!(ec,  0, "empty: edge_count=0");
    assert_eq!(lm1, 0, "empty: LM1=0");
    assert_eq!(lm2, 0, "empty: LM2=0");
    assert_eq!(lm3, 0, "empty: LM3=0");
}

// ── Test 2: Single isolated node ─────────────────────────────────────────────
// d₂=0 (no nodes at distance 2). All indices zero.

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  1, "single: node_count=1");
    assert_eq!(ec,  0, "single: no edges");
    assert_eq!(lm1, 0, "single: LM1=0 (d₂=0 for isolated)");
    assert_eq!(lm2, 0, "single: LM2=0");
    assert_eq!(lm3, 0, "single: LM3=0");
}

// ── Test 3: Single directed edge A→B ─────────────────────────────────────────
// Undirected {A,B}. d(A,B)=1. No vertex at distance 2 from either.
// d₂(A)=d₂(B)=0. LM1=LM2=LM3=0.

#[test]
fn test_03_single_edge() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);
    add_edge(T15_ID_A, T15_ID_B, "t15.e.ab");

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  2, "edge: node_count=2");
    assert_eq!(ec,  1, "edge: edge_count=1");
    assert_eq!(lm1, 0, "edge: LM1=0 (no node at dist 2 from either endpoint)");
    assert_eq!(lm2, 0, "edge: LM2=0");
    assert_eq!(lm3, 0, "edge: LM3=0");
}

// ── Test 4: Path P₃ = A-B-C ──────────────────────────────────────────────────
// d₂(A)=1(C), d₂(B)=0, d₂(C)=1(A).
// LM1=1+0+1=2. LM2=0. LM3=(1+0)+(0+1)=2.

#[test]
fn test_04_path_p3() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);
    add_node(T15_VEC_C, T15_KEY_C, T15_ID_C);
    add_edge(T15_ID_A, T15_ID_B, "t15.e.ab");
    add_edge(T15_ID_B, T15_ID_C, "t15.e.bc");

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  3, "p3: node_count=3");
    assert_eq!(ec,  2, "p3: edge_count=2");
    assert_eq!(lm1, 2, "p3: LM1=2 (d₂:1+0+1; 1²+0²+1²)");
    assert_eq!(lm2, 0, "p3: LM2=0 (center B has d₂=0; products zero)");
    assert_eq!(lm3, 2, "p3: LM3=2 ({{A,B}}:1+0=1; {{B,C}}:0+1=1)");
}

// ── Test 5: Triangle K₃ ──────────────────────────────────────────────────────
// All pairs adjacent (dist 1). No vertex at distance 2. d₂=0 for all.
// LM1=LM2=LM3=0.

#[test]
fn test_05_triangle_k3() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);
    add_node(T15_VEC_C, T15_KEY_C, T15_ID_C);
    add_edge(T15_ID_A, T15_ID_B, "t15.e.ab");
    add_edge(T15_ID_B, T15_ID_A, "t15.e.ba");
    add_edge(T15_ID_B, T15_ID_C, "t15.e.bc");
    add_edge(T15_ID_C, T15_ID_B, "t15.e.cb");
    add_edge(T15_ID_A, T15_ID_C, "t15.e.ac");
    add_edge(T15_ID_C, T15_ID_A, "t15.e.ca");

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  3, "k3: node_count=3");
    assert_eq!(ec,  3, "k3: edge_count=3");
    assert_eq!(lm1, 0, "k3: LM1=0 (all pairs adjacent; d₂=0)");
    assert_eq!(lm2, 0, "k3: LM2=0");
    assert_eq!(lm3, 0, "k3: LM3=0");
}

// ── Test 6: Star K_{1,4} ─────────────────────────────────────────────────────
// Center A: d₂(A)=0 (B,C,D,E all at dist 1).
// Each leaf: d₂=3 (3 other leaves at dist 2; center at dist 1).
// LM1=0²+4×3²=36. LM2=4×(0×3)=0. LM3=4×(0+3)=12.

#[test]
fn test_06_star_k14() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);
    add_node(T15_VEC_C, T15_KEY_C, T15_ID_C);
    add_node(T15_VEC_D, T15_KEY_D, T15_ID_D);
    add_node(T15_VEC_E, T15_KEY_E, T15_ID_E);
    add_edge(T15_ID_A, T15_ID_B, "t15.e.ab");
    add_edge(T15_ID_A, T15_ID_C, "t15.e.ac");
    add_edge(T15_ID_A, T15_ID_D, "t15.e.ad");
    add_edge(T15_ID_A, T15_ID_E, "t15.e.ae");

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  5,  "star: node_count=5");
    assert_eq!(ec,  4,  "star: edge_count=4");
    assert_eq!(lm1, 36, "star: LM1=36 (center d₂=0; 4 leaves d₂=3; 4×9)");
    assert_eq!(lm2, 0,  "star: LM2=0 (center d₂=0 zeros every edge product)");
    assert_eq!(lm3, 12, "star: LM3=12 (4 edges × (0+3))");
}

// ── Test 7: Path P₄ = A-B-C-D ────────────────────────────────────────────────
// d₂(A)=1(C), d₂(B)=1(D), d₂(C)=1(A), d₂(D)=1(B).
// LM1=4. LM2=3×(1×1)=3. LM3=3×(1+1)=6.

#[test]
fn test_07_path_p4() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);
    add_node(T15_VEC_C, T15_KEY_C, T15_ID_C);
    add_node(T15_VEC_D, T15_KEY_D, T15_ID_D);
    add_edge(T15_ID_A, T15_ID_B, "t15.e.ab");
    add_edge(T15_ID_B, T15_ID_C, "t15.e.bc");
    add_edge(T15_ID_C, T15_ID_D, "t15.e.cd");

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  4, "p4: node_count=4");
    assert_eq!(ec,  3, "p4: edge_count=3");
    assert_eq!(lm1, 4, "p4: LM1=4 (d₂=1 for all; 4×1²)");
    assert_eq!(lm2, 3, "p4: LM2=3 (3 edges × 1×1)");
    assert_eq!(lm3, 6, "p4: LM3=6 (3 edges × (1+1))");
}

// ── Test 8: Complete K₄ ──────────────────────────────────────────────────────
// All pairs at dist 1. d₂=0 for all. LM1=LM2=LM3=0.

#[test]
fn test_08_complete_k4() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);
    add_node(T15_VEC_C, T15_KEY_C, T15_ID_C);
    add_node(T15_VEC_D, T15_KEY_D, T15_ID_D);
    add_edge(T15_ID_A, T15_ID_B, "t15.e.ab");
    add_edge(T15_ID_B, T15_ID_A, "t15.e.ba");
    add_edge(T15_ID_A, T15_ID_C, "t15.e.ac");
    add_edge(T15_ID_C, T15_ID_A, "t15.e.ca");
    add_edge(T15_ID_A, T15_ID_D, "t15.e.ad");
    add_edge(T15_ID_D, T15_ID_A, "t15.e.da");
    add_edge(T15_ID_B, T15_ID_C, "t15.e.bc");
    add_edge(T15_ID_C, T15_ID_B, "t15.e.cb");
    add_edge(T15_ID_B, T15_ID_D, "t15.e.bd");
    add_edge(T15_ID_D, T15_ID_B, "t15.e.db");
    add_edge(T15_ID_C, T15_ID_D, "t15.e.cd");
    add_edge(T15_ID_D, T15_ID_C, "t15.e.dc");

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  4, "k4: node_count=4");
    assert_eq!(ec,  6, "k4: edge_count=6");
    assert_eq!(lm1, 0, "k4: LM1=0 (all pairs adjacent; d₂=0 everywhere)");
    assert_eq!(lm2, 0, "k4: LM2=0");
    assert_eq!(lm3, 0, "k4: LM3=0");
}

// ── Test 9: Two isolated nodes ────────────────────────────────────────────────
// n=2, m=0. No BFS distance 2 path. d₂=0. All indices zero.

#[test]
fn test_09_two_isolated() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  2, "isolated: node_count=2");
    assert_eq!(ec,  0, "isolated: no edges");
    assert_eq!(lm1, 0, "isolated: LM1=0 (no BFS reachability)");
    assert_eq!(lm2, 0, "isolated: LM2=0");
    assert_eq!(lm3, 0, "isolated: LM3=0");
}

// ── Test 10: K_{2,3} bipartite cross-check ───────────────────────────────────
// Left={A,B}, right={C,D,E}. All left-right pairs connected.
// From A: C,D,E at dist 1; B at dist 2 (A→C→B). d₂(A)=1.
// From B: C,D,E at dist 1; A at dist 2. d₂(B)=1.
// From C: A,B at dist 1; D,E at dist 2 (C→A→D). d₂(C)=2. Same for D,E.
// LM1 = 2×1²+3×2² = 2+12 = 14.
// LM2: each of 6 edges is (left d₂=1) × (right d₂=2) = 2. LM2=6×2=12.
// LM3: each edge: 1+2=3. LM3=6×3=18.

#[test]
fn test_10_k23_bipartite() {
    let _g = setup();
    add_node(T15_VEC_A, T15_KEY_A, T15_ID_A);
    add_node(T15_VEC_B, T15_KEY_B, T15_ID_B);
    add_node(T15_VEC_C, T15_KEY_C, T15_ID_C);
    add_node(T15_VEC_D, T15_KEY_D, T15_ID_D);
    add_node(T15_VEC_E, T15_KEY_E, T15_ID_E);
    // Left A connects to all right
    add_edge(T15_ID_A, T15_ID_C, "t15.e.ac");
    add_edge(T15_ID_C, T15_ID_A, "t15.e.ca");
    add_edge(T15_ID_A, T15_ID_D, "t15.e.ad");
    add_edge(T15_ID_D, T15_ID_A, "t15.e.da");
    add_edge(T15_ID_A, T15_ID_E, "t15.e.ae");
    add_edge(T15_ID_E, T15_ID_A, "t15.e.ea");
    // Left B connects to all right
    add_edge(T15_ID_B, T15_ID_C, "t15.e.bc");
    add_edge(T15_ID_C, T15_ID_B, "t15.e.cb");
    add_edge(T15_ID_B, T15_ID_D, "t15.e.bd");
    add_edge(T15_ID_D, T15_ID_B, "t15.e.db");
    add_edge(T15_ID_B, T15_ID_E, "t15.e.be");
    add_edge(T15_ID_E, T15_ID_B, "t15.e.eb");

    let (lm1, lm2, lm3, ec, nc) = gos_runtime::graph_topo_indices15();
    assert_eq!(nc,  5,  "k23: node_count=5");
    assert_eq!(ec,  6,  "k23: edge_count=6");
    assert_eq!(lm1, 14, "k23: LM1=14 (left d₂=1: 2×1; right d₂=2: 3×4)");
    assert_eq!(lm2, 12, "k23: LM2=12 (6 edges × 1×2; left×right d₂)");
    assert_eq!(lm3, 18, "k23: LM3=18 (6 edges × (1+2))");
}
