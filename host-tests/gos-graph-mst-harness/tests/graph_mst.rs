// gos-graph-mst-harness — V2.48 Minimum Spanning Tree / Forest tests
//
// Verifies `gos_runtime::graph_mst` — Prim's minimum spanning forest over the
// undirected projection of the GOS kernel graph.
//
// Algorithm summary (Prim's greedy MST):
//   1. Treat every directed edge as undirected with weight edge.spec.weight
//      (default 1.0 when not explicitly set).
//   2. Start from the lowest-slot unvisited node; grow the MST greedily by
//      always picking the minimum-weight edge to an unvisited neighbor.
//   3. Disconnected components restart from the next unvisited node (spanning
//      forest, not just spanning tree).
//
// Return value: (vecs, parents, weights, node_count, total_mst_w)
//   vecs[i]       — node vector in Prim visit order
//   parents[i]    — parent vector (same as vecs[i] for component roots)
//   weights[i]    — edge weight to parent × 1000 as u32 (0 for roots)
//   node_count    — total live nodes
//   total_mst_w   — sum of all MST edge weights × 1000 as u32
//
// Key invariants:
//   Empty graph              → node_count=0, total_mst_w=0
//   Single node              → node_count=1, weight=0, parent=self
//   K₂ edge(A,B,w=2.5)      → MST contains that edge; total_mst_w=2500
//   Two disconnected nodes   → total_mst_w=0 (no edges to include)
//   Path A-2-B-3-C          → MST = both edges; total=5000
//   Triangle A-1-B-2-C-3-A  → MST skips heaviest edge; total=1000+2000=3000
//   Forest A-B, C (isolated) → two roots; total=1000
//
// Test matrix:
//  1.  Empty graph                          → node_count=0, total=0
//  2.  Single node                          → node_count=1, root parent=self, weight=0
//  3.  Two isolated nodes (no edge)         → total_mst_w=0, two roots
//  4.  K₂: one edge weight=1.0             → total=1000, one root, one child
//  5.  K₂: one edge weight=2.5             → total=2500
//  6.  Path A-B-C all weight 1.0           → total=2000
//  7.  Triangle (K₃) with weights 1,2,3    → MST selects 1+2 edges (total=3000)
//  8.  Two components (A-B, C isolated)    → total=1000, C is second root
//  9.  Root parent == self for every root  → parents[i]==vecs[i] when weights[i]==0
// 10.  Validity: every non-root is reachable via parent chain

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const MST_PLUGIN: PluginId   = PluginId::from_ascii("KL_MST_00");
const MST_EXEC:   ExecutorId = ExecutorId::from_ascii("mst.exec0");

const MST_KEY_A: &str = "mst.alpha";
const MST_KEY_B: &str = "mst.beta";
const MST_KEY_C: &str = "mst.gamma";
const MST_KEY_D: &str = "mst.delta";

const MST_ID_A: NodeId = derive_node_id(MST_PLUGIN, MST_KEY_A);
const MST_ID_B: NodeId = derive_node_id(MST_PLUGIN, MST_KEY_B);
const MST_ID_C: NodeId = derive_node_id(MST_PLUGIN, MST_KEY_C);
const MST_ID_D: NodeId = derive_node_id(MST_PLUGIN, MST_KEY_D);

const MST_VEC_A: VectorAddress = VectorAddress::new(25, 1, 1, 0);
const MST_VEC_B: VectorAddress = VectorAddress::new(25, 1, 2, 0);
const MST_VEC_C: VectorAddress = VectorAddress::new(25, 1, 3, 0);
const MST_VEC_D: VectorAddress = VectorAddress::new(25, 1, 4, 0);

const MST_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    MST_PLUGIN,
    name:         "kl-graph-mst-harness",
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
        executor_id:       MST_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(MST_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(MST_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str, weight: f32) {
    gos_runtime::register_edge(EdgeSpec {
        edge_id:              derive_edge_id(from, to, key),
        from_node:            from,
        to_node:              to,
        edge_type:            RuntimeEdgeType::Signal,
        weight,
        acl_mask:             u64::MAX,
        route_policy:         RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding:   None,
        vector_ref:           None,
    }).unwrap();
}

fn vec_of(vecs: &[VectorAddress], total: usize, target: VectorAddress) -> Option<usize> {
    for i in 0..total { if vecs[i] == target { return Some(i); } }
    None
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph_no_mst() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _parents, _weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 0, "empty: 0 nodes");
    assert_eq!(mst_w, 0, "empty: total MST weight = 0");
}

// ── 2. Single node → root, weight=0, parent=self ─────────────────────────────

#[test]
fn single_node_root() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    let (vecs, parents, weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 1, "1 node");
    assert_eq!(mst_w, 0, "single node: MST weight = 0");
    assert_eq!(vecs[0], MST_VEC_A, "A is the node");
    assert_eq!(parents[0], MST_VEC_A, "A is its own parent (root)");
    assert_eq!(weights[0], 0, "root has weight 0");
}

// ── 3. Two isolated nodes (no edge) → two roots, total=0 ─────────────────────

#[test]
fn two_isolated_nodes_two_roots() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    let (vecs, parents, weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(mst_w, 0, "no edges → total MST weight = 0");
    // Both nodes are roots (parent == self, weight == 0).
    let ia = vec_of(&vecs, total, MST_VEC_A).expect("A present");
    let ib = vec_of(&vecs, total, MST_VEC_B).expect("B present");
    assert_eq!(parents[ia], MST_VEC_A, "A is its own parent");
    assert_eq!(parents[ib], MST_VEC_B, "B is its own parent");
    assert_eq!(weights[ia], 0, "A weight=0 (root)");
    assert_eq!(weights[ib], 0, "B weight=0 (root)");
}

// ── 4. K₂: one edge weight=1.0 → total_mst_w=1000 ──────────────────────────

#[test]
fn k2_weight_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    add_edge(MST_ID_A, MST_ID_B, "mst.ab.t4", 1.0);
    let (vecs, parents, weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(mst_w, 1000, "K₂ weight=1.0 → total MST = 1000");
    // One of A, B must be the root (weight=0) and the other the child (weight=1000).
    let ia = vec_of(&vecs, total, MST_VEC_A).expect("A present");
    let ib = vec_of(&vecs, total, MST_VEC_B).expect("B present");
    let root_idx = if weights[ia] == 0 { ia } else { ib };
    let child_idx = if root_idx == ia { ib } else { ia };
    assert_eq!(weights[root_idx], 0, "root has weight 0");
    assert_eq!(weights[child_idx], 1000, "child has weight 1000");
    assert_eq!(parents[root_idx], vecs[root_idx], "root parent = self");
    assert_eq!(parents[child_idx], vecs[root_idx], "child parent = root");
}

// ── 5. K₂: one edge weight=2.5 → total_mst_w=2500 ──────────────────────────

#[test]
fn k2_weight_two_point_five() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    add_edge(MST_ID_A, MST_ID_B, "mst.ab.t5", 2.5);
    let (_vecs, _parents, _weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(mst_w, 2500, "K₂ weight=2.5 → total MST = 2500");
}

// ── 6. Path A─B─C all weight=1.0 → total=2000 ───────────────────────────────

#[test]
fn path_abc_total_two() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    add_node(MST_VEC_C, MST_KEY_C, MST_ID_C, 0xD003);
    add_edge(MST_ID_A, MST_ID_B, "mst.ab.t6", 1.0);
    add_edge(MST_ID_B, MST_ID_C, "mst.bc.t6", 1.0);
    let (_vecs, _parents, _weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(mst_w, 2000, "path A-B-C: total MST weight = 2000");
}

// ── 7. K₃ triangle (weights 1, 2, 3) → MST skips edge-3, total=3000 ─────────

#[test]
fn triangle_mst_skips_heaviest() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    add_node(MST_VEC_C, MST_KEY_C, MST_ID_C, 0xD003);
    // A-B weight 1, B-C weight 2, A-C weight 3 (heaviest — should be excluded)
    add_edge(MST_ID_A, MST_ID_B, "mst.ab.t7", 1.0);
    add_edge(MST_ID_B, MST_ID_C, "mst.bc.t7", 2.0);
    add_edge(MST_ID_A, MST_ID_C, "mst.ac.t7", 3.0);
    let (_vecs, _parents, _weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(mst_w, 3000, "triangle MST: 1+2=3 (heaviest edge 3 excluded) → 3000");
}

// ── 8. Two components (A─B isolated, C isolated) → total=1000, C is root ─────

#[test]
fn two_components_forest() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    add_node(MST_VEC_C, MST_KEY_C, MST_ID_C, 0xD003);
    add_edge(MST_ID_A, MST_ID_B, "mst.ab.t8", 1.0);
    // C is isolated — forms its own MST component.
    let (vecs, parents, weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(mst_w, 1000, "A-B connected component: 1000; C isolated: +0 → total=1000");
    // C must be a root.
    let ic = vec_of(&vecs, total, MST_VEC_C).expect("C present");
    assert_eq!(parents[ic], MST_VEC_C, "isolated C is its own root");
    assert_eq!(weights[ic], 0, "isolated C has weight 0");
}

// ── 9. Root invariant: parent==self for every node with weight==0 ─────────────

#[test]
fn root_parent_equals_self() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Two disconnected pairs: (A-B) and (C-D), so two roots.
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    add_node(MST_VEC_C, MST_KEY_C, MST_ID_C, 0xD003);
    add_node(MST_VEC_D, MST_KEY_D, MST_ID_D, 0xD004);
    add_edge(MST_ID_A, MST_ID_B, "mst.ab.t9", 1.0);
    add_edge(MST_ID_C, MST_ID_D, "mst.cd.t9", 1.0);
    let (vecs, parents, weights, total, _mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 4, "4 nodes");
    // For every node with weight==0, parent must equal its own vector.
    for i in 0..total {
        if weights[i] == 0 {
            assert_eq!(
                parents[i], vecs[i],
                "node {:?} is a root → parent must equal self",
                vecs[i]
            );
        }
    }
}

// ── 10. Connectivity: every non-root has a parent in the output ───────────────

#[test]
fn every_non_root_parent_in_output() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Star: B is center (connects to A, C, D)
    add_node(MST_VEC_A, MST_KEY_A, MST_ID_A, 0xD001);
    add_node(MST_VEC_B, MST_KEY_B, MST_ID_B, 0xD002);
    add_node(MST_VEC_C, MST_KEY_C, MST_ID_C, 0xD003);
    add_node(MST_VEC_D, MST_KEY_D, MST_ID_D, 0xD004);
    add_edge(MST_ID_B, MST_ID_A, "mst.ba.t10", 1.0);
    add_edge(MST_ID_B, MST_ID_C, "mst.bc.t10", 2.0);
    add_edge(MST_ID_B, MST_ID_D, "mst.bd.t10", 3.0);
    let (vecs, parents, weights, total, mst_w) = gos_runtime::graph_mst::<128>();
    assert_eq!(total, 4, "4 nodes");
    // MST of a star = all edges (it IS a tree), total = 1+2+3 = 6000
    assert_eq!(mst_w, 6000, "star MST: 1+2+3=6 → 6000");
    // Every non-root parent must appear as a vector in the output array.
    for i in 0..total {
        if weights[i] > 0 {
            let parent = parents[i];
            let found = (0..total).any(|j| vecs[j] == parent);
            assert!(found, "parent {:?} of {:?} must be in output vecs", parent, vecs[i]);
        }
    }
}
