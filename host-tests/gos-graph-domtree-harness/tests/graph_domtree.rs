// gos-graph-domtree-harness -- V2.90 dominator tree tests
//
// Verifies `gos_runtime::graph_domtree` -- Cooper, Harvey & Kennedy 2001
// simple iterative dominator algorithm.  For each node reachable from `start`,
// returns its immediate dominator: the closest node on EVERY path from start.
//
//  1. Empty graph              -> node_count=0, reachable_count=0.
//  2. Single node, start=self  -> reachable=1, idom[start]=start.
//  3. Start not in graph       -> node_count=N, reachable_count=0.
//  4. Single edge A->B, start=A -> idom[A]=A, idom[B]=A.
//  5. Linear chain A->B->C, start=A -> idom[B]=A, idom[C]=B.
//  6. Diamond A->{B,C}->D      -> idom[D]=A (both B and C lead to D, A is LCA).
//  7. Node unreachable from start: A->B, C isolated, start=A -> reachable=2.
//  8. Back edge / cycle: A->B->C->B, start=A -> idom[B]=A, idom[C]=B.
//  9. Isolated start, unreachable rest: A (start) alone, B->C apart -> reachable=1.
// 10. Merge then extend A->{B,C}->D->E -> idom[D]=A, idom[E]=D, reachable=5.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const DT_PLUGIN: PluginId   = PluginId::from_ascii("KL_DOMTREE_H_");
const DT_EXEC:   ExecutorId = ExecutorId::from_ascii("dt.exec");

const DT_KEY_A: &str = "dt.alpha";
const DT_KEY_B: &str = "dt.beta";
const DT_KEY_C: &str = "dt.gamma";
const DT_KEY_D: &str = "dt.delta";
const DT_KEY_E: &str = "dt.epsilon";

const DT_ID_A: NodeId = derive_node_id(DT_PLUGIN, DT_KEY_A);
const DT_ID_B: NodeId = derive_node_id(DT_PLUGIN, DT_KEY_B);
const DT_ID_C: NodeId = derive_node_id(DT_PLUGIN, DT_KEY_C);
const DT_ID_D: NodeId = derive_node_id(DT_PLUGIN, DT_KEY_D);
const DT_ID_E: NodeId = derive_node_id(DT_PLUGIN, DT_KEY_E);

// L4=66 namespace for this harness.
const DT_VEC_A: VectorAddress = VectorAddress::new(66, 1, 1, 0);
const DT_VEC_B: VectorAddress = VectorAddress::new(66, 1, 2, 0);
const DT_VEC_C: VectorAddress = VectorAddress::new(66, 1, 3, 0);
const DT_VEC_D: VectorAddress = VectorAddress::new(66, 1, 4, 0);
const DT_VEC_E: VectorAddress = VectorAddress::new(66, 1, 5, 0);

// Unrelated vec used to test "start not in graph".
const DT_VEC_ABSENT: VectorAddress = VectorAddress::new(66, 9, 9, 9);

const DT_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    DT_PLUGIN,
    name:         "kl-graph-domtree-harness",
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
        executor_id:       DT_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(DT_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(DT_MANIFEST).unwrap();
    g
}

/// Find the immediate dominator of `target` in the output arrays.
fn find_idom(
    vecs:  &[VectorAddress],
    idoms: &[VectorAddress],
    total: usize,
    target: VectorAddress,
) -> Option<VectorAddress> {
    for i in 0..total {
        if vecs[i] == target { return Some(idoms[i]); }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_vecs, _idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 0, "empty graph has 0 nodes");
    assert_eq!(rc, 0, "empty graph: 0 reachable");
}

#[test]
fn test_02_single_node_start_is_self() {
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 1);
    assert_eq!(rc, 1, "start itself is reachable");
    let idom_a = find_idom(&vecs, &idoms, rc, DT_VEC_A).unwrap();
    assert_eq!(idom_a, DT_VEC_A, "start dominates itself");
}

#[test]
fn test_03_start_not_in_graph() {
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    let (_vecs, _idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_ABSENT);
    assert_eq!(nc, 2, "two live nodes");
    assert_eq!(rc, 0, "absent start: nothing reachable");
}

#[test]
fn test_04_single_edge_start_dominates_both() {
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    add_edge(DT_ID_A, DT_ID_B, "ab");
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 2);
    assert_eq!(rc, 2, "A and B both reachable from A");
    let idom_a = find_idom(&vecs, &idoms, rc, DT_VEC_A).unwrap();
    let idom_b = find_idom(&vecs, &idoms, rc, DT_VEC_B).unwrap();
    assert_eq!(idom_a, DT_VEC_A, "A is its own idom (root)");
    assert_eq!(idom_b, DT_VEC_A, "A is the only path to B");
}

#[test]
fn test_05_linear_chain_idoms() {
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    add_node(DT_VEC_C, DT_KEY_C, DT_ID_C);
    add_edge(DT_ID_A, DT_ID_B, "ab");
    add_edge(DT_ID_B, DT_ID_C, "bc");
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 3);
    assert_eq!(rc, 3);
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_A).unwrap(), DT_VEC_A, "A idom = self");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_B).unwrap(), DT_VEC_A, "idom[B] = A");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_C).unwrap(), DT_VEC_B, "idom[C] = B");
}

#[test]
fn test_06_diamond_idom_is_start() {
    // A -> B, A -> C, B -> D, C -> D; start = A.
    // D can be reached via A->B->D or A->C->D.
    // The LCA (lowest common dominator) in the tree is A itself.
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    add_node(DT_VEC_C, DT_KEY_C, DT_ID_C);
    add_node(DT_VEC_D, DT_KEY_D, DT_ID_D);
    add_edge(DT_ID_A, DT_ID_B, "ab");
    add_edge(DT_ID_A, DT_ID_C, "ac");
    add_edge(DT_ID_B, DT_ID_D, "bd");
    add_edge(DT_ID_C, DT_ID_D, "cd");
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 4);
    assert_eq!(rc, 4);
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_A).unwrap(), DT_VEC_A);
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_B).unwrap(), DT_VEC_A);
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_C).unwrap(), DT_VEC_A);
    // D has two predecessors B and C; their LCA in the dominator tree is A.
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_D).unwrap(), DT_VEC_A,
        "idom[D] = A (LCA of B and C paths)");
}

#[test]
fn test_07_unreachable_node_excluded() {
    // A -> B (reachable), C (isolated, unreachable from A); start = A.
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    add_node(DT_VEC_C, DT_KEY_C, DT_ID_C); // no edge from A or B to C
    add_edge(DT_ID_A, DT_ID_B, "ab");
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 3, "three live nodes total");
    assert_eq!(rc, 2, "only A and B are reachable from A");
    assert!(find_idom(&vecs, &idoms, rc, DT_VEC_C).is_none(),
        "C not reachable from A, must not appear in output");
}

#[test]
fn test_08_back_edge_cycle() {
    // A -> B -> C -> B (back edge).  start = A.
    // Every path from A to C goes through A then B, so idom[C]=B and idom[B]=A.
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    add_node(DT_VEC_C, DT_KEY_C, DT_ID_C);
    add_edge(DT_ID_A, DT_ID_B, "ab");
    add_edge(DT_ID_B, DT_ID_C, "bc");
    add_edge(DT_ID_C, DT_ID_B, "cb"); // back edge
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 3);
    assert_eq!(rc, 3, "all three nodes reachable from A");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_A).unwrap(), DT_VEC_A);
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_B).unwrap(), DT_VEC_A,
        "idom[B] = A (A is only entry to B even with cycle)");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_C).unwrap(), DT_VEC_B,
        "idom[C] = B (every path A->...->C passes through B)");
}

#[test]
fn test_09_isolated_start_only_itself_reachable() {
    // A (start, no edges), B -> C (separate subgraph).  start = A.
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    add_node(DT_VEC_C, DT_KEY_C, DT_ID_C);
    add_edge(DT_ID_B, DT_ID_C, "bc");
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 3);
    assert_eq!(rc, 1, "only A itself is reachable from isolated A");
    let idom_a = find_idom(&vecs, &idoms, rc, DT_VEC_A).unwrap();
    assert_eq!(idom_a, DT_VEC_A, "A dominates itself");
}

#[test]
fn test_10_merge_then_extend() {
    // A -> B, A -> C, B -> D, C -> D, D -> E; start = A.
    // idom[B]=A, idom[C]=A, idom[D]=A (two paths merge at D via B and C),
    // idom[E]=D (D is the sole predecessor of E).
    let _g = setup();
    add_node(DT_VEC_A, DT_KEY_A, DT_ID_A);
    add_node(DT_VEC_B, DT_KEY_B, DT_ID_B);
    add_node(DT_VEC_C, DT_KEY_C, DT_ID_C);
    add_node(DT_VEC_D, DT_KEY_D, DT_ID_D);
    add_node(DT_VEC_E, DT_KEY_E, DT_ID_E);
    add_edge(DT_ID_A, DT_ID_B, "ab");
    add_edge(DT_ID_A, DT_ID_C, "ac");
    add_edge(DT_ID_B, DT_ID_D, "bd");
    add_edge(DT_ID_C, DT_ID_D, "cd");
    add_edge(DT_ID_D, DT_ID_E, "de");
    let (vecs, idoms, nc, rc) = gos_runtime::graph_domtree::<64>(DT_VEC_A);
    assert_eq!(nc, 5);
    assert_eq!(rc, 5, "all five nodes reachable from A");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_A).unwrap(), DT_VEC_A, "A: idom=self");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_B).unwrap(), DT_VEC_A, "B: idom=A");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_C).unwrap(), DT_VEC_A, "C: idom=A");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_D).unwrap(), DT_VEC_A,
        "D: idom=A (LCA of B and C, both dominated by A)");
    assert_eq!(find_idom(&vecs, &idoms, rc, DT_VEC_E).unwrap(), DT_VEC_D,
        "E: idom=D (D is sole predecessor of E)");
}
