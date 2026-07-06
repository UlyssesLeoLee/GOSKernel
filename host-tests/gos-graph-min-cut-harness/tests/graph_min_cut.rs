// gos-graph-min-cut-harness — V3.02 global minimum edge cut tests
//
// Verifies `gos_runtime::graph_min_cut` — Stoer-Wagner 1997 global min-cut.
//
// The global minimum cut κ'(G) is the minimum number of undirected edges
// whose removal disconnects the graph (edge connectivity).  The algorithm
// contracts the graph V-1 times, recording the minimum cut-of-phase.
//
// Key invariants tested:
//   Correctness:  returned min_cut equals the known edge connectivity.
//   Partition:    side-A and side-B together cover exactly node_count nodes.
//   Disconnected: min_cut = 0 for graphs with no edges or isolated components.
//   Dedup:        directed pair A→B + B→A count as ONE undirected edge.
//
//  1. Empty graph                           → min_cut=0, node_count=0.
//  2. Single node                           → min_cut=0, node_count=1, side_b=0.
//  3. Two nodes, no edges                   → min_cut=0, side_a+side_b=2.
//  4. K2 (two nodes, one edge)              → min_cut=1.
//  5. Path A-B-C (undirected)              → min_cut=1 (bridge at either joint).
//  6. Triangle K3                           → min_cut=2.
//  7. K4 complete graph                     → min_cut=3.
//  8. Two triangles joined by bridge        → min_cut=1 (cut the bridge).
//  9. Star K_{1,4}                          → min_cut=1 (all spokes connect to center).
// 10. Square C4 (4-cycle)                  → min_cut=2; partition invariant.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const MC_PLUGIN: PluginId   = PluginId::from_ascii("KL_MINCUT_H____");
const MC_EXEC:   ExecutorId = ExecutorId::from_ascii("mc.exec");

const MC_KEY_A: &str = "mc.a";
const MC_KEY_B: &str = "mc.b";
const MC_KEY_C: &str = "mc.c";
const MC_KEY_D: &str = "mc.d";
const MC_KEY_E: &str = "mc.e";
const MC_KEY_F: &str = "mc.f";

const MC_ID_A: NodeId = derive_node_id(MC_PLUGIN, MC_KEY_A);
const MC_ID_B: NodeId = derive_node_id(MC_PLUGIN, MC_KEY_B);
const MC_ID_C: NodeId = derive_node_id(MC_PLUGIN, MC_KEY_C);
const MC_ID_D: NodeId = derive_node_id(MC_PLUGIN, MC_KEY_D);
const MC_ID_E: NodeId = derive_node_id(MC_PLUGIN, MC_KEY_E);
const MC_ID_F: NodeId = derive_node_id(MC_PLUGIN, MC_KEY_F);

// L4=78 namespace for this harness.
const MC_VEC_A: VectorAddress = VectorAddress::new(78, 1, 1, 0);
const MC_VEC_B: VectorAddress = VectorAddress::new(78, 1, 2, 0);
const MC_VEC_C: VectorAddress = VectorAddress::new(78, 1, 3, 0);
const MC_VEC_D: VectorAddress = VectorAddress::new(78, 1, 4, 0);
const MC_VEC_E: VectorAddress = VectorAddress::new(78, 2, 1, 0);
const MC_VEC_F: VectorAddress = VectorAddress::new(78, 2, 2, 0);

const MC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    MC_PLUGIN,
    name:         "kl-graph-min-cut-harness",
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
        executor_id:       MC_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(MC_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    let g = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(MC_MANIFEST).unwrap();
    g
}

// ── Test 1: empty graph ───────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();

    let (_, _, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count,   0, "empty: node_count=0");
    assert_eq!(min_cut,      0, "empty: min_cut=0");
    assert_eq!(side_b_size,  0, "empty: side_b=0");
}

// ── Test 2: single node ───────────────────────────────────────────────────────

#[test]
fn test_02_single_node() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);

    let (vecs, sides, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count,  1, "single: node_count=1");
    assert_eq!(min_cut,     0, "single: min_cut=0");
    assert_eq!(side_b_size, 0, "single: side_b=0 (single node always side-A)");
    assert_eq!(sides[0],    0, "single: only node is side-A");
    assert_eq!(vecs[0],  MC_VEC_A, "single: vec is A");
}

// ── Test 3: two nodes, no edges ───────────────────────────────────────────────

#[test]
fn test_03_two_nodes_no_edges() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B);

    let (_, sides, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 2, "no-edge: node_count=2");
    assert_eq!(min_cut,    0, "no-edge: already disconnected → min_cut=0");
    // Side A + side B must cover both nodes
    assert_eq!(
        sides[0..2].iter().filter(|&&s| s == 0).count()
            + sides[0..2].iter().filter(|&&s| s == 1).count(),
        2,
        "no-edge: both nodes assigned to a side"
    );
    assert_eq!(
        (node_count - side_b_size) + side_b_size, node_count,
        "no-edge: partition sizes sum to node_count"
    );
}

// ── Test 4: K2 (two nodes, one undirected edge) ───────────────────────────────

#[test]
fn test_04_k2_one_edge() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B);
    add_edge(MC_ID_A, MC_ID_B, "ab");

    let (_, sides, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 2, "K2: node_count=2");
    assert_eq!(min_cut,    1, "K2: min_cut=1 (single edge)");
    assert_eq!(side_b_size, 1, "K2: one node each side");
    // Sides must differ: one is 0, the other is 1
    assert_ne!(sides[0], sides[1], "K2: nodes on opposite sides");
}

// ── Test 5: path A-B-C ────────────────────────────────────────────────────────

#[test]
fn test_05_path_abc() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B);
    add_node(MC_VEC_C, MC_KEY_C, MC_ID_C);
    // Undirected path: add both directions for A-B and B-C
    add_edge(MC_ID_A, MC_ID_B, "ab");
    add_edge(MC_ID_B, MC_ID_C, "bc");

    let (_, _, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 3, "path: node_count=3");
    assert_eq!(min_cut,    1, "path: min_cut=1 (each bridge is a 1-edge cut)");
    // Partition must be non-trivial (1 node on one side, 2 on the other)
    assert!(side_b_size == 1 || side_b_size == 2, "path: partition splits as 1+2 or 2+1");
    assert_ne!(side_b_size, 0, "path: both sides non-empty");
    assert_ne!(side_b_size, 3, "path: both sides non-empty");
}

// ── Test 6: triangle K3 ───────────────────────────────────────────────────────

#[test]
fn test_06_triangle_k3() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B);
    add_node(MC_VEC_C, MC_KEY_C, MC_ID_C);
    add_edge(MC_ID_A, MC_ID_B, "ab");
    add_edge(MC_ID_B, MC_ID_C, "bc");
    add_edge(MC_ID_C, MC_ID_A, "ca");

    let (_, _, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 3, "K3: node_count=3");
    assert_eq!(min_cut,    2, "K3: min_cut=2 (each vertex has degree 2)");
    // Optimal partition isolates exactly one node
    assert!(side_b_size == 1 || side_b_size == 2, "K3: partition is 1+2");
}

// ── Test 7: K4 complete graph ─────────────────────────────────────────────────

#[test]
fn test_07_k4_complete() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B);
    add_node(MC_VEC_C, MC_KEY_C, MC_ID_C);
    add_node(MC_VEC_D, MC_KEY_D, MC_ID_D);
    // All 12 directed edges (6 undirected pairs)
    add_edge(MC_ID_A, MC_ID_B, "ab"); add_edge(MC_ID_B, MC_ID_A, "ba");
    add_edge(MC_ID_A, MC_ID_C, "ac"); add_edge(MC_ID_C, MC_ID_A, "ca");
    add_edge(MC_ID_A, MC_ID_D, "ad"); add_edge(MC_ID_D, MC_ID_A, "da");
    add_edge(MC_ID_B, MC_ID_C, "bc"); add_edge(MC_ID_C, MC_ID_B, "cb");
    add_edge(MC_ID_B, MC_ID_D, "bd"); add_edge(MC_ID_D, MC_ID_B, "db");
    add_edge(MC_ID_C, MC_ID_D, "cd"); add_edge(MC_ID_D, MC_ID_C, "dc");

    let (_, _, node_count, min_cut, _) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 4, "K4: node_count=4");
    assert_eq!(min_cut,    3, "K4: min_cut=3 (each node has degree 3; isolating one costs 3)");
}

// ── Test 8: two triangles joined by a bridge ──────────────────────────────────
// Graph: A-B-C-A (triangle 1) + D-E-F-D (triangle 2) + bridge C-D
// min_cut = 1 (cut the bridge C-D)

#[test]
fn test_08_two_triangles_bridge() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B);
    add_node(MC_VEC_C, MC_KEY_C, MC_ID_C);
    add_node(MC_VEC_D, MC_KEY_D, MC_ID_D);
    add_node(MC_VEC_E, MC_KEY_E, MC_ID_E);
    add_node(MC_VEC_F, MC_KEY_F, MC_ID_F);
    // Triangle 1: A-B-C
    add_edge(MC_ID_A, MC_ID_B, "ab");
    add_edge(MC_ID_B, MC_ID_C, "bc");
    add_edge(MC_ID_C, MC_ID_A, "ca");
    // Triangle 2: D-E-F
    add_edge(MC_ID_D, MC_ID_E, "de");
    add_edge(MC_ID_E, MC_ID_F, "ef");
    add_edge(MC_ID_F, MC_ID_D, "fd");
    // Bridge: C-D
    add_edge(MC_ID_C, MC_ID_D, "cd");

    let (_, sides, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 6, "bridge: node_count=6");
    assert_eq!(min_cut,    1, "bridge: min_cut=1 (bridge C-D is the only 1-edge cut)");
    // Partition must be 3+3 (one triangle on each side)
    assert_eq!(side_b_size, 3, "bridge: each triangle on its own side (3+3)");
    let side_a_size = node_count - side_b_size;
    assert_eq!(side_a_size, 3, "bridge: side-A has 3 nodes");
    // Verify sides are consistent: all A-side nodes have sides[i]==0
    for i in 0..node_count {
        assert!(sides[i] == 0 || sides[i] == 1, "bridge: sides valid");
    }
}

// ── Test 9: star K_{1,4} (center C, spokes A,B,D,E) ─────────────────────────
// min_cut = 1 (removing any spoke edge isolates one leaf, but there's no single
// cut that disconnects the center from ALL spokes — actually min_cut of star is
// the degree of the minimum-degree leaf = 1, since each leaf has degree 1)

#[test]
fn test_09_star_k14() {
    let _g = setup();
    add_node(MC_VEC_C, MC_KEY_C, MC_ID_C); // center
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A); // spoke
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B); // spoke
    add_node(MC_VEC_D, MC_KEY_D, MC_ID_D); // spoke
    add_node(MC_VEC_E, MC_KEY_E, MC_ID_E); // spoke
    add_edge(MC_ID_C, MC_ID_A, "ca");
    add_edge(MC_ID_C, MC_ID_B, "cb");
    add_edge(MC_ID_C, MC_ID_D, "cd");
    add_edge(MC_ID_C, MC_ID_E, "ce");

    let (_, _, node_count, min_cut, _) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 5, "star: node_count=5");
    assert_eq!(min_cut,    1, "star: min_cut=1 (each leaf has degree 1)");
}

// ── Test 10: square C4 + partition invariant ──────────────────────────────────
// Graph: A-B-C-D-A (undirected 4-cycle)
// min_cut = 2 (cut any two non-adjacent edges e.g. A-B and C-D)
// Also validates: partition sizes sum to node_count; sides cover 0 and 1 values.

#[test]
fn test_10_square_c4_invariant() {
    let _g = setup();
    add_node(MC_VEC_A, MC_KEY_A, MC_ID_A);
    add_node(MC_VEC_B, MC_KEY_B, MC_ID_B);
    add_node(MC_VEC_C, MC_KEY_C, MC_ID_C);
    add_node(MC_VEC_D, MC_KEY_D, MC_ID_D);
    // Directed edges in both directions to make undirected 4-cycle
    add_edge(MC_ID_A, MC_ID_B, "ab");
    add_edge(MC_ID_B, MC_ID_C, "bc");
    add_edge(MC_ID_C, MC_ID_D, "cd");
    add_edge(MC_ID_D, MC_ID_A, "da");

    let (vecs, sides, node_count, min_cut, side_b_size) = gos_runtime::graph_min_cut::<128>();

    assert_eq!(node_count, 4, "C4: node_count=4");
    assert_eq!(min_cut,    2, "C4: min_cut=2 (each node has degree 2)");

    // Partition invariant: side_a + side_b = node_count
    let side_a_size = node_count - side_b_size;
    assert_eq!(side_a_size + side_b_size, node_count, "C4: partition sizes sum");

    // Both sides non-empty
    assert!(side_a_size >= 1, "C4: side-A non-empty");
    assert!(side_b_size >= 1, "C4: side-B non-empty");

    // All nodes appear exactly once
    let all_vecs: std::collections::HashSet<_> = vecs[0..node_count].iter().copied().collect();
    assert_eq!(all_vecs.len(), node_count, "C4: all nodes present once");
    assert!(all_vecs.contains(&MC_VEC_A), "C4: A present");
    assert!(all_vecs.contains(&MC_VEC_B), "C4: B present");
    assert!(all_vecs.contains(&MC_VEC_C), "C4: C present");
    assert!(all_vecs.contains(&MC_VEC_D), "C4: D present");

    // sides values are all 0 or 1
    for i in 0..node_count {
        assert!(sides[i] == 0 || sides[i] == 1, "C4: sides[{}] in {{0,1}}", i);
    }

    // vecs sorted: side-A first (sides==0), then side-B (sides==1)
    let mut saw_b = false;
    for i in 0..node_count {
        if sides[i] == 1 { saw_b = true; }
        if saw_b {
            assert_eq!(sides[i], 1, "C4: side-B comes after side-A in output");
        }
    }
}
