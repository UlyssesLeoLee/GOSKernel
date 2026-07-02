// gos-graph-community-harness — V2.45 LPA community detection API tests
//
// Verifies `gos_runtime::graph_community` — Label Propagation Algorithm (LPA)
// for community detection over the GOS kernel graph.
//
// Algorithm summary (synchronous LPA, 20 iterations):
//   1. Initialize: each node starts in its own community (label = slot index).
//   2. Each round: every node adopts the most-frequent label among all
//      undirected neighbors (in + out).  Tie-break: smallest label.
//   3. After 20 iterations: relabel communities 0, 1, 2... by size descending.
//
// Key properties:
//   Isolated node:                 1 node per community
//   Two disconnected pairs:        2 communities (each pair forms one)
//   Complete bipartite K_{2,2}:    1 community (all 4 reachable undirected)
//   Line graph A─B─C─D:           1 community (fully connected undirected)
//   Two cliques no cross-edge:     2 communities
//   Self-loop edge (A→A):          A remains isolated (self-loops not neighbors)
//
// Test matrix:
//  1. Empty graph                → total=0, communities=0
//  2. Single isolated node       → 1 node, 1 community (isolated)
//  3. Two disconnected nodes     → 2 nodes, 2 communities (no edges)
//  4. Single edge A→B            → 2 nodes, 1 community (undirected: A─B)
//  5. Directed triangle A→B→C→A → 3 nodes, 1 community
//  6. Two disconnected pairs     → 4 nodes, 2 communities; larger has id=0
//  7. Complete bipartite K_{2,2} → 4 nodes, 1 community
//  8. Two triangles, no bridge   → 6 nodes, 2 communities
//  9. Output sorted: members contiguous per community id
// 10. Single community: community_count=1, all ids=0

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const COMM_PLUGIN: PluginId   = PluginId::from_ascii("KL_COM_00");
const COMM_EXEC:   ExecutorId = ExecutorId::from_ascii("comm.exec0");

const COMM_KEY_A: &str = "comm.alpha";
const COMM_KEY_B: &str = "comm.beta";
const COMM_KEY_C: &str = "comm.gamma";
const COMM_KEY_D: &str = "comm.delta";
const COMM_KEY_E: &str = "comm.epsilon";
const COMM_KEY_F: &str = "comm.zeta";

const COMM_ID_A: NodeId = derive_node_id(COMM_PLUGIN, COMM_KEY_A);
const COMM_ID_B: NodeId = derive_node_id(COMM_PLUGIN, COMM_KEY_B);
const COMM_ID_C: NodeId = derive_node_id(COMM_PLUGIN, COMM_KEY_C);
const COMM_ID_D: NodeId = derive_node_id(COMM_PLUGIN, COMM_KEY_D);
const COMM_ID_E: NodeId = derive_node_id(COMM_PLUGIN, COMM_KEY_E);
const COMM_ID_F: NodeId = derive_node_id(COMM_PLUGIN, COMM_KEY_F);

const COMM_VEC_A: VectorAddress = VectorAddress::new(22, 1, 1, 0);
const COMM_VEC_B: VectorAddress = VectorAddress::new(22, 1, 2, 0);
const COMM_VEC_C: VectorAddress = VectorAddress::new(22, 1, 3, 0);
const COMM_VEC_D: VectorAddress = VectorAddress::new(22, 1, 4, 0);
const COMM_VEC_E: VectorAddress = VectorAddress::new(22, 1, 5, 0);
const COMM_VEC_F: VectorAddress = VectorAddress::new(22, 1, 6, 0);

const COMM_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    COMM_PLUGIN,
    name:         "kl-graph-community-harness",
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
        executor_id:       COMM_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(COMM_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(COMM_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_comm(vecs: &[VectorAddress], comm: &[u8], total: usize, target: VectorAddress) -> Option<u8> {
    for i in 0..total { if vecs[i] == target { return Some(comm[i]); } }
    None
}

// ── 1. Empty graph → total=0, communities=0 ──────────────────────────────────

#[test]
fn empty_graph_community_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      0, "empty graph: 0 nodes");
    assert_eq!(comm_count, 0, "empty graph: 0 communities");
}

// ── 2. Single isolated node → 1 node, 1 community ────────────────────────────

#[test]
fn single_isolated_node_one_community() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    let (vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      1, "1 node");
    assert_eq!(comm_count, 1, "1 community");
    let c = find_comm(&vecs, &comm, total, COMM_VEC_A).expect("A must appear");
    assert_eq!(c, 0, "isolated node is in community 0 (the only community)");
}

// ── 3. Two disconnected nodes → 2 communities ────────────────────────────────

#[test]
fn two_disconnected_nodes_two_communities() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    let (vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      2, "2 nodes");
    assert_eq!(comm_count, 2, "2 communities (no edges → cannot merge)");
    let ca = find_comm(&vecs, &comm, total, COMM_VEC_A).expect("A present");
    let cb = find_comm(&vecs, &comm, total, COMM_VEC_B).expect("B present");
    assert_ne!(ca, cb, "A and B must be in different communities");
}

// ── 4. Single edge A→B → 1 community (undirected: A─B) ──────────────────────

#[test]
fn single_edge_one_community() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    add_edge(COMM_ID_A, COMM_ID_B, "comm.ab.t4");
    let (vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      2, "2 nodes");
    assert_eq!(comm_count, 1, "1 community (edge makes them neighbors undirected)");
    let ca = find_comm(&vecs, &comm, total, COMM_VEC_A).expect("A present");
    let cb = find_comm(&vecs, &comm, total, COMM_VEC_B).expect("B present");
    assert_eq!(ca, cb, "A and B must be in the same community");
    assert_eq!(ca, 0,  "only community has id=0");
}

// ── 5. Directed triangle A→B→C→A → 1 community ──────────────────────────────

#[test]
fn directed_triangle_one_community() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    add_node(COMM_VEC_C, COMM_KEY_C, COMM_ID_C, 0xC003);
    add_edge(COMM_ID_A, COMM_ID_B, "comm.ab.t5");
    add_edge(COMM_ID_B, COMM_ID_C, "comm.bc.t5");
    add_edge(COMM_ID_C, COMM_ID_A, "comm.ca.t5");
    let (vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      3, "3 nodes");
    assert_eq!(comm_count, 1, "1 community (cycle: all undirected-connected)");
    let ca = find_comm(&vecs, &comm, total, COMM_VEC_A).unwrap();
    let cb = find_comm(&vecs, &comm, total, COMM_VEC_B).unwrap();
    let cc = find_comm(&vecs, &comm, total, COMM_VEC_C).unwrap();
    assert_eq!(ca, 0, "A in community 0");
    assert_eq!(cb, 0, "B in community 0");
    assert_eq!(cc, 0, "C in community 0");
}

// ── 6. Two disconnected pairs → 2 communities, larger/equal gets id=0 ────────

#[test]
fn two_disconnected_pairs_two_communities() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Pair 1: A─B (via directed edge A→B)
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    add_edge(COMM_ID_A, COMM_ID_B, "comm.ab.t6");
    // Pair 2: C─D (via directed edge C→D)
    add_node(COMM_VEC_C, COMM_KEY_C, COMM_ID_C, 0xC003);
    add_node(COMM_VEC_D, COMM_KEY_D, COMM_ID_D, 0xC004);
    add_edge(COMM_ID_C, COMM_ID_D, "comm.cd.t6");
    let (vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      4, "4 nodes");
    assert_eq!(comm_count, 2, "2 communities (two isolated pairs)");
    let ca = find_comm(&vecs, &comm, total, COMM_VEC_A).unwrap();
    let cb = find_comm(&vecs, &comm, total, COMM_VEC_B).unwrap();
    let cc = find_comm(&vecs, &comm, total, COMM_VEC_C).unwrap();
    let cd = find_comm(&vecs, &comm, total, COMM_VEC_D).unwrap();
    // A and B must be in the same community.
    assert_eq!(ca, cb, "A and B in the same community");
    // C and D must be in the same community.
    assert_eq!(cc, cd, "C and D in the same community");
    // The two pairs must be in different communities.
    assert_ne!(ca, cc, "pair A-B and pair C-D in different communities");
}

// ── 7. Complete bipartite K_{2,2}: A,B → C,D → 1 community ──────────────────

#[test]
fn complete_bipartite_k22_one_community() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    add_node(COMM_VEC_C, COMM_KEY_C, COMM_ID_C, 0xC003);
    add_node(COMM_VEC_D, COMM_KEY_D, COMM_ID_D, 0xC004);
    // A → C, A → D, B → C, B → D
    add_edge(COMM_ID_A, COMM_ID_C, "comm.ac.t7");
    add_edge(COMM_ID_A, COMM_ID_D, "comm.ad.t7");
    add_edge(COMM_ID_B, COMM_ID_C, "comm.bc.t7");
    add_edge(COMM_ID_B, COMM_ID_D, "comm.bd.t7");
    let (vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      4, "4 nodes");
    assert_eq!(comm_count, 1, "1 community (all reachable undirected)");
    let ca = find_comm(&vecs, &comm, total, COMM_VEC_A).unwrap();
    let cb = find_comm(&vecs, &comm, total, COMM_VEC_B).unwrap();
    let cc = find_comm(&vecs, &comm, total, COMM_VEC_C).unwrap();
    let cd = find_comm(&vecs, &comm, total, COMM_VEC_D).unwrap();
    assert_eq!(ca, 0, "A in C0");
    assert_eq!(cb, 0, "B in C0");
    assert_eq!(cc, 0, "C in C0");
    assert_eq!(cd, 0, "D in C0");
}

// ── 8. Two triangles, no bridge → 2 communities ──────────────────────────────

#[test]
fn two_triangles_no_bridge_two_communities() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Triangle 1: A→B→C→A
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    add_node(COMM_VEC_C, COMM_KEY_C, COMM_ID_C, 0xC003);
    add_edge(COMM_ID_A, COMM_ID_B, "comm.ab.t8");
    add_edge(COMM_ID_B, COMM_ID_C, "comm.bc.t8");
    add_edge(COMM_ID_C, COMM_ID_A, "comm.ca.t8");
    // Triangle 2: D→E→F→D  (completely disconnected from triangle 1)
    add_node(COMM_VEC_D, COMM_KEY_D, COMM_ID_D, 0xC004);
    add_node(COMM_VEC_E, COMM_KEY_E, COMM_ID_E, 0xC005);
    add_node(COMM_VEC_F, COMM_KEY_F, COMM_ID_F, 0xC006);
    add_edge(COMM_ID_D, COMM_ID_E, "comm.de.t8");
    add_edge(COMM_ID_E, COMM_ID_F, "comm.ef.t8");
    add_edge(COMM_ID_F, COMM_ID_D, "comm.fd.t8");
    let (vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      6, "6 nodes");
    assert_eq!(comm_count, 2, "2 communities (no cross-edge between triangles)");
    let ca = find_comm(&vecs, &comm, total, COMM_VEC_A).unwrap();
    let cb = find_comm(&vecs, &comm, total, COMM_VEC_B).unwrap();
    let cc = find_comm(&vecs, &comm, total, COMM_VEC_C).unwrap();
    let cd = find_comm(&vecs, &comm, total, COMM_VEC_D).unwrap();
    let ce = find_comm(&vecs, &comm, total, COMM_VEC_E).unwrap();
    let cf = find_comm(&vecs, &comm, total, COMM_VEC_F).unwrap();
    // All in triangle 1 share the same community.
    assert_eq!(ca, cb, "A and B same community");
    assert_eq!(cb, cc, "B and C same community");
    // All in triangle 2 share the same community.
    assert_eq!(cd, ce, "D and E same community");
    assert_eq!(ce, cf, "E and F same community");
    // The two triangles are in different communities.
    assert_ne!(ca, cd, "triangle-1 and triangle-2 are different communities");
}

// ── 9. Output sorted: all members of the same community are contiguous ────────

#[test]
fn output_members_are_contiguous() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Pair A─B (A→B), then isolated C.  Two communities.
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    add_node(COMM_VEC_C, COMM_KEY_C, COMM_ID_C, 0xC003);
    add_edge(COMM_ID_A, COMM_ID_B, "comm.ab.t9");
    let (_vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      3, "3 nodes");
    assert_eq!(comm_count, 2, "2 communities");
    // Verify that the output is contiguous: community ids must be non-decreasing
    // (since sorted by community id asc within the grouped output).
    for i in 1..total {
        assert!(
            comm[i] >= comm[i - 1],
            "community ids must be non-decreasing at position {i}: {} < {}",
            comm[i], comm[i - 1]
        );
    }
}

// ── 10. Single community: all ids=0, community_count=1 ───────────────────────

#[test]
fn fully_connected_chain_single_community() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Line graph A→B→C→D (undirected: A─B─C─D, fully connected).
    add_node(COMM_VEC_A, COMM_KEY_A, COMM_ID_A, 0xC001);
    add_node(COMM_VEC_B, COMM_KEY_B, COMM_ID_B, 0xC002);
    add_node(COMM_VEC_C, COMM_KEY_C, COMM_ID_C, 0xC003);
    add_node(COMM_VEC_D, COMM_KEY_D, COMM_ID_D, 0xC004);
    add_edge(COMM_ID_A, COMM_ID_B, "comm.ab.t10");
    add_edge(COMM_ID_B, COMM_ID_C, "comm.bc.t10");
    add_edge(COMM_ID_C, COMM_ID_D, "comm.cd.t10");
    let (_vecs, comm, total, comm_count) = gos_runtime::graph_community::<128>();
    assert_eq!(total,      4, "4 nodes");
    assert_eq!(comm_count, 1, "1 community: chain is fully connected undirected");
    for i in 0..total {
        assert_eq!(comm[i], 0, "all nodes in community 0");
    }
}
