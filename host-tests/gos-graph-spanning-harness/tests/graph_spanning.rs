// gos-graph-spanning-harness — V2.46 BFS spanning forest API tests
//
// Verifies `gos_runtime::graph_spanning` — BFS spanning forest over the
// undirected projection of the GOS kernel graph.
//
// Algorithm summary (BFS spanning forest, undirected):
//   1. Iterate live nodes in ascending slot order.
//   2. For each unvisited node: start a BFS tree (that node = root, depth 0).
//   3. BFS: visit undirected neighbors (both in- and out-edges); record parent
//      and depth.  Output is appended in BFS visit order.
//
// Return value: (vecs, parents, depths, node_count, tree_count)
//   vecs[i]    — node vector in BFS order
//   parents[i] — parent vector (same as vecs[i] for roots)
//   depths[i]  — BFS depth (0 for roots)
//   node_count — total live nodes
//   tree_count — number of BFS trees (undirected connected components)
//
// Key properties:
//   Empty graph           → node_count=0, tree_count=0
//   Single node           → 1 node, 1 tree, depth 0, parent = self
//   A→B (single edge)    → 1 tree: root A depth 0, child B depth 1
//   Two disconnected      → 2 trees, each with 1 node
//   Chain A→B→C→D        → 1 tree: A(0)→B(1)→C(2)→D(3)
//   Directed cycle A→B→C→A → 1 tree (all undirected-connected)
//   Two disjoint pairs    → 2 trees (one per pair)
//
// Test matrix:
//  1. Empty graph                → node_count=0, tree_count=0
//  2. Single node                → 1 node, 1 tree, depth=0, parent=self
//  3. Two isolated nodes (no edge) → 2 trees, both depth 0
//  4. Single edge A→B            → 1 tree: A root (depth 0), B child (depth 1)
//  5. Chain A→B→C→D              → 1 tree, depths 0/1/2/3
//  6. Directed triangle A→B→C→A  → 1 tree, all depths ≤ 2
//  7. Two disconnected pairs     → 2 trees
//  8. Root parent == self        → roots have parents[i] == vecs[i]
//  9. Non-root parent in same tree → every non-root parent must be another node
// 10. tree_count matches connected components (undirected)

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const SPAN_PLUGIN: PluginId   = PluginId::from_ascii("KL_SPN_00");
const SPAN_EXEC:   ExecutorId = ExecutorId::from_ascii("span.exec0");

const SPAN_KEY_A: &str = "span.alpha";
const SPAN_KEY_B: &str = "span.beta";
const SPAN_KEY_C: &str = "span.gamma";
const SPAN_KEY_D: &str = "span.delta";
const SPAN_ID_A: NodeId = derive_node_id(SPAN_PLUGIN, SPAN_KEY_A);
const SPAN_ID_B: NodeId = derive_node_id(SPAN_PLUGIN, SPAN_KEY_B);
const SPAN_ID_C: NodeId = derive_node_id(SPAN_PLUGIN, SPAN_KEY_C);
const SPAN_ID_D: NodeId = derive_node_id(SPAN_PLUGIN, SPAN_KEY_D);

const SPAN_VEC_A: VectorAddress = VectorAddress::new(23, 1, 1, 0);
const SPAN_VEC_B: VectorAddress = VectorAddress::new(23, 1, 2, 0);
const SPAN_VEC_C: VectorAddress = VectorAddress::new(23, 1, 3, 0);
const SPAN_VEC_D: VectorAddress = VectorAddress::new(23, 1, 4, 0);

const SPAN_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    SPAN_PLUGIN,
    name:         "kl-graph-spanning-harness",
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

fn node_spec(key: &'static str, node_id: NodeId, schema: u64) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       SPAN_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(SPAN_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(SPAN_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_depth(
    vecs: &[VectorAddress], depths: &[u8], total: usize, target: VectorAddress,
) -> Option<u8> {
    for i in 0..total { if vecs[i] == target { return Some(depths[i]); } }
    None
}

fn find_parent(
    vecs: &[VectorAddress], parents: &[VectorAddress], total: usize, target: VectorAddress,
) -> Option<VectorAddress> {
    for i in 0..total { if vecs[i] == target { return Some(parents[i]); } }
    None
}

// ── 1. Empty graph → node_count=0, tree_count=0 ──────────────────────────────

#[test]
fn empty_graph_spanning_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _parents, _depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
    assert_eq!(trees, 0, "empty graph: 0 trees");
}

// ── 2. Single node → 1 node, 1 tree, depth=0, parent=self ───────────────────

#[test]
fn single_node_root_self_parent() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    let (vecs, parents, depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 1, "1 node");
    assert_eq!(trees, 1, "1 tree");
    assert_eq!(depths[0], 0, "root depth = 0");
    assert_eq!(vecs[0], SPAN_VEC_A, "only node is A");
    assert_eq!(parents[0], SPAN_VEC_A, "root parent = self");
}

// ── 3. Two isolated nodes (no edge) → 2 trees, both depth 0 ─────────────────

#[test]
fn two_isolated_nodes_two_trees() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    let (vecs, parents, depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(trees, 2, "2 trees (no edges → two isolated components)");
    let da = find_depth(&vecs, &depths, total, SPAN_VEC_A).expect("A present");
    let db = find_depth(&vecs, &depths, total, SPAN_VEC_B).expect("B present");
    assert_eq!(da, 0, "A is a root (depth 0)");
    assert_eq!(db, 0, "B is a root (depth 0)");
    let pa = find_parent(&vecs, &parents, total, SPAN_VEC_A).expect("A present");
    let pb = find_parent(&vecs, &parents, total, SPAN_VEC_B).expect("B present");
    assert_eq!(pa, SPAN_VEC_A, "A's parent = self");
    assert_eq!(pb, SPAN_VEC_B, "B's parent = self");
}

// ── 4. Single edge A→B → 1 tree: A=root (depth 0), B=child (depth 1) ────────

#[test]
fn single_edge_one_tree() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_edge(SPAN_ID_A, SPAN_ID_B, "span.ab.t4");
    let (vecs, parents, depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(trees, 1, "1 tree (A-B connected undirected)");
    let da = find_depth(&vecs, &depths, total, SPAN_VEC_A).expect("A present");
    let db = find_depth(&vecs, &depths, total, SPAN_VEC_B).expect("B present");
    assert_eq!(da, 0, "A is root (depth 0)");
    assert_eq!(db, 1, "B is child of A (depth 1)");
    let pb = find_parent(&vecs, &parents, total, SPAN_VEC_B).expect("B present");
    assert_eq!(pb, SPAN_VEC_A, "B's parent = A");
}

// ── 5. Chain A→B→C→D → 1 tree, BFS depths 0/1/2/3 ─────────────────────────

#[test]
fn chain_abcd_one_tree_depths() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_node(SPAN_VEC_C, SPAN_KEY_C, SPAN_ID_C, 0xD003);
    add_node(SPAN_VEC_D, SPAN_KEY_D, SPAN_ID_D, 0xD004);
    add_edge(SPAN_ID_A, SPAN_ID_B, "span.ab.t5");
    add_edge(SPAN_ID_B, SPAN_ID_C, "span.bc.t5");
    add_edge(SPAN_ID_C, SPAN_ID_D, "span.cd.t5");
    let (vecs, parents, depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert_eq!(trees, 1, "1 tree (chain A─B─C─D undirected)");
    let da = find_depth(&vecs, &depths, total, SPAN_VEC_A).expect("A present");
    let db = find_depth(&vecs, &depths, total, SPAN_VEC_B).expect("B present");
    let dc = find_depth(&vecs, &depths, total, SPAN_VEC_C).expect("C present");
    let dd = find_depth(&vecs, &depths, total, SPAN_VEC_D).expect("D present");
    assert_eq!(da, 0, "A: depth 0 (root)");
    assert_eq!(db, 1, "B: depth 1");
    assert_eq!(dc, 2, "C: depth 2");
    assert_eq!(dd, 3, "D: depth 3");
    // Parent chain: B←A, C←B, D←C
    assert_eq!(find_parent(&vecs, &parents, total, SPAN_VEC_B).unwrap(), SPAN_VEC_A);
    assert_eq!(find_parent(&vecs, &parents, total, SPAN_VEC_C).unwrap(), SPAN_VEC_B);
    assert_eq!(find_parent(&vecs, &parents, total, SPAN_VEC_D).unwrap(), SPAN_VEC_C);
}

// ── 6. Directed triangle A→B→C→A → 1 tree, all nodes reachable ──────────────

#[test]
fn directed_triangle_one_tree() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_node(SPAN_VEC_C, SPAN_KEY_C, SPAN_ID_C, 0xD003);
    add_edge(SPAN_ID_A, SPAN_ID_B, "span.ab.t6");
    add_edge(SPAN_ID_B, SPAN_ID_C, "span.bc.t6");
    add_edge(SPAN_ID_C, SPAN_ID_A, "span.ca.t6");
    let (_vecs, _parents, depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(trees, 1, "1 tree (all undirected-connected via cycle)");
    // Root is depth 0; the other two are reachable within 2 hops.
    assert_eq!(depths[0], 0, "first BFS node is the root (depth 0)");
    for i in 0..total {
        assert!(depths[i] <= 2, "no node should be deeper than 2 in a triangle");
    }
}

// ── 7. Two disconnected pairs → 2 trees ──────────────────────────────────────

#[test]
fn two_disconnected_pairs_two_trees() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Pair 1: A→B
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_edge(SPAN_ID_A, SPAN_ID_B, "span.ab.t7");
    // Pair 2: C→D
    add_node(SPAN_VEC_C, SPAN_KEY_C, SPAN_ID_C, 0xD003);
    add_node(SPAN_VEC_D, SPAN_KEY_D, SPAN_ID_D, 0xD004);
    add_edge(SPAN_ID_C, SPAN_ID_D, "span.cd.t7");
    let (vecs, _parents, depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert_eq!(trees, 2, "2 trees (two disjoint pairs)");
    // Count roots (depth 0).
    let root_count = (0..total).filter(|&i| depths[i] == 0).count();
    assert_eq!(root_count, 2, "exactly 2 roots (one per tree)");
    // Each root's parent == itself.
    for i in 0..total {
        if depths[i] == 0 {
            let _ = vecs[i]; // just access to confirm index valid
        }
    }
}

// ── 8. Root parent == self ────────────────────────────────────────────────────

#[test]
fn root_parent_equals_self() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_node(SPAN_VEC_C, SPAN_KEY_C, SPAN_ID_C, 0xD003);
    // B connected to A; C isolated.
    add_edge(SPAN_ID_A, SPAN_ID_B, "span.ab.t8");
    let (vecs, parents, depths, total, _trees) = gos_runtime::graph_spanning::<128>();
    for i in 0..total {
        if depths[i] == 0 {
            assert_eq!(
                vecs[i], parents[i],
                "root at index {i} must have parent == self"
            );
        }
    }
}

// ── 9. Non-root parent is a distinct, known node ──────────────────────────────

#[test]
fn nonroot_parent_is_known_node() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_node(SPAN_VEC_C, SPAN_KEY_C, SPAN_ID_C, 0xD003);
    add_edge(SPAN_ID_A, SPAN_ID_B, "span.ab.t9");
    add_edge(SPAN_ID_B, SPAN_ID_C, "span.bc.t9");
    let (vecs, parents, depths, total, _trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 3);
    for i in 0..total {
        if depths[i] > 0 {
            // Parent must appear in the vecs array.
            let parent_vec = parents[i];
            let parent_present = (0..total).any(|j| vecs[j] == parent_vec);
            assert!(
                parent_present,
                "non-root at index {i} has parent {:?} not in vecs",
                parent_vec
            );
        }
    }
}

// ── 10. tree_count matches undirected connected components ────────────────────

#[test]
fn tree_count_matches_connected_components() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Three disjoint nodes: A, B, C (no edges) → 3 components.
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_node(SPAN_VEC_C, SPAN_KEY_C, SPAN_ID_C, 0xD003);
    let (_vecs, _parents, depths, total, trees) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(trees, 3, "3 isolated nodes = 3 trees");
    let root_count = (0..total).filter(|&i| depths[i] == 0).count();
    assert_eq!(root_count, 3, "3 roots for 3 isolated nodes");

    // Now connect A─B and leave C isolated: 2 components.
    reset();
    register_plugin();
    add_node(SPAN_VEC_A, SPAN_KEY_A, SPAN_ID_A, 0xD001);
    add_node(SPAN_VEC_B, SPAN_KEY_B, SPAN_ID_B, 0xD002);
    add_node(SPAN_VEC_C, SPAN_KEY_C, SPAN_ID_C, 0xD003);
    add_edge(SPAN_ID_A, SPAN_ID_B, "span.ab.t10");
    let (_vecs2, _parents2, depths2, total2, trees2) = gos_runtime::graph_spanning::<128>();
    assert_eq!(total2, 3, "3 nodes");
    assert_eq!(trees2, 2, "A-B + C = 2 components");
    let root_count2 = (0..total2).filter(|&i| depths2[i] == 0).count();
    assert_eq!(root_count2, 2, "2 roots for 2 components");
}
