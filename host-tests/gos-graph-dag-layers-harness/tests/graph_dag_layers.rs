// gos-graph-dag-layers-harness -- V2.89 DAG topological layering tests
//
// Verifies `gos_runtime::graph_dag_layers` -- multi-source Kahn BFS that assigns
// each node in a DAG its minimum parallel-execution layer.
// Layer 0 = in-degree-0 sources; layer k = nodes whose deepest predecessor is k-1.
// Analogous to systemd dependency levels: services in the same layer start in
// parallel; each layer must wait for the previous layer to complete.
//
//  1. Empty graph             -> is_dag=true, layer_count=0.
//  2. Single isolated node    -> layer=0, layer_count=1.
//  3. Self-loop A->A          -> is_dag=false.
//  4. Single edge A->B        -> layer[A]=0, layer[B]=1, layer_count=2.
//  5. Linear chain A->B->C->D -> layers [0,1,2,3], layer_count=4.
//  6. Diamond A->{B,C}->D    -> B=1, C=1, D=2, A=0; layer_count=3.
//  7. Directed 3-cycle        -> is_dag=false.
//  8. DAG shortcut A->B->C + A->C -> C gets layer 2 (not 1, deepest wins).
//  9. Star fan-out A->{B,C,D} -> B=C=D=1, A=0; layer_count=2.
// 10. Two independent chains  -> correct layering per component; layer_count=max of both.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const DL_PLUGIN: PluginId   = PluginId::from_ascii("KL_DAGLYR_H_");
const DL_EXEC:   ExecutorId = ExecutorId::from_ascii("dl.exec");

const DL_KEY_A: &str = "dl.alpha";
const DL_KEY_B: &str = "dl.beta";
const DL_KEY_C: &str = "dl.gamma";
const DL_KEY_D: &str = "dl.delta";
const DL_KEY_E: &str = "dl.epsilon";
const DL_KEY_F: &str = "dl.zeta";

const DL_ID_A: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_A);
const DL_ID_B: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_B);
const DL_ID_C: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_C);
const DL_ID_D: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_D);
const DL_ID_E: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_E);
const DL_ID_F: NodeId = derive_node_id(DL_PLUGIN, DL_KEY_F);

// L4=65 namespace for this harness.
const DL_VEC_A: VectorAddress = VectorAddress::new(65, 1, 1, 0);
const DL_VEC_B: VectorAddress = VectorAddress::new(65, 1, 2, 0);
const DL_VEC_C: VectorAddress = VectorAddress::new(65, 1, 3, 0);
const DL_VEC_D: VectorAddress = VectorAddress::new(65, 1, 4, 0);
const DL_VEC_E: VectorAddress = VectorAddress::new(65, 1, 5, 0);
const DL_VEC_F: VectorAddress = VectorAddress::new(65, 1, 6, 0);

const DL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    DL_PLUGIN,
    name:         "kl-graph-dag-layers-harness",
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
        executor_id:       DL_EXEC,
        state_schema_hash: 0,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId) {
    gos_runtime::register_node(DL_PLUGIN, vec, node_spec(key, id)).unwrap();
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
    gos_runtime::discover_plugin(DL_MANIFEST).unwrap();
    g
}

fn find_layer(vecs: &[VectorAddress], layers: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(layers[i]); }
    }
    None
}

#[test]
fn test_01_empty_graph() {
    let _g = setup();
    let (_vecs, _layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag, "empty graph is a DAG");
    assert_eq!(nc, 0);
    assert_eq!(layer_count, 0, "no nodes => no layers");
}

#[test]
fn test_02_single_isolated_node() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    let (vecs, layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag, "single isolated node is a DAG");
    assert_eq!(nc, 1);
    assert_eq!(layer_count, 1, "single isolated node occupies exactly layer 0");
    let la = find_layer(&vecs, &layers, nc, DL_VEC_A).unwrap();
    assert_eq!(la, 0, "isolated node has layer 0");
}

#[test]
fn test_03_self_loop_not_dag() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_edge(DL_ID_A, DL_ID_A, "aa");
    let (_vecs, _layers, nc, _layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(!is_dag, "self-loop makes is_dag=false");
    assert_eq!(nc, 1);
}

#[test]
fn test_04_single_edge_two_layers() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    let (vecs, layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag);
    assert_eq!(nc, 2);
    assert_eq!(layer_count, 2, "A->B spans layers 0 and 1");
    let la = find_layer(&vecs, &layers, nc, DL_VEC_A).unwrap();
    let lb = find_layer(&vecs, &layers, nc, DL_VEC_B).unwrap();
    assert_eq!(la, 0, "A is the source (layer 0)");
    assert_eq!(lb, 1, "B depends on A (layer 1)");
}

#[test]
fn test_05_linear_chain_four_layers() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_B, DL_ID_C, "bc");
    add_edge(DL_ID_C, DL_ID_D, "cd");
    let (vecs, layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag);
    assert_eq!(nc, 4);
    assert_eq!(layer_count, 4, "A->B->C->D spans layers 0..3");
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_A).unwrap(), 0);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_B).unwrap(), 1);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_C).unwrap(), 2);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_D).unwrap(), 3);
}

#[test]
fn test_06_diamond_three_layers() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_A, DL_ID_C, "ac");
    add_edge(DL_ID_B, DL_ID_D, "bd");
    add_edge(DL_ID_C, DL_ID_D, "cd");
    let (vecs, layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag);
    assert_eq!(nc, 4);
    assert_eq!(layer_count, 3, "diamond: A=0, B/C=1, D=2");
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_A).unwrap(), 0);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_B).unwrap(), 1);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_C).unwrap(), 1);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_D).unwrap(), 2);
}

#[test]
fn test_07_directed_cycle_not_dag() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_B, DL_ID_C, "bc");
    add_edge(DL_ID_C, DL_ID_A, "ca");
    let (_vecs, _layers, nc, _layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(!is_dag, "3-cycle is not a DAG");
    assert_eq!(nc, 3);
}

#[test]
fn test_08_shortcut_dag_deepest_layer_wins() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_B, DL_ID_C, "bc");
    add_edge(DL_ID_A, DL_ID_C, "ac"); // shortcut: A directly to C
    // Without the shortcut, C would be at layer 2 via A->B->C.
    // With the shortcut, A->C places C at layer 1 but B->C places at layer 2.
    // max(1, 2) = 2 should win.
    let (vecs, layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag);
    assert_eq!(nc, 3);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_A).unwrap(), 0, "A is source");
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_B).unwrap(), 1, "B = A+1");
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_C).unwrap(), 2, "C = max(A+1, B+1) = 2");
    assert_eq!(layer_count, 3);
}

#[test]
fn test_09_star_two_layers() {
    let _g = setup();
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    add_edge(DL_ID_A, DL_ID_C, "ac");
    add_edge(DL_ID_A, DL_ID_D, "ad");
    let (vecs, layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag);
    assert_eq!(nc, 4);
    assert_eq!(layer_count, 2, "star: hub=0, leaves=1");
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_A).unwrap(), 0, "hub A is source");
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_B).unwrap(), 1);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_C).unwrap(), 1);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_D).unwrap(), 1);
}

#[test]
fn test_10_two_independent_chains_max_layers() {
    let _g = setup();
    // Chain 1: A->B (2 layers: 0,1)
    add_node(DL_VEC_A, DL_KEY_A, DL_ID_A);
    add_node(DL_VEC_B, DL_KEY_B, DL_ID_B);
    add_edge(DL_ID_A, DL_ID_B, "ab");
    // Chain 2: C->D->E->F (4 layers: 0,1,2,3)
    add_node(DL_VEC_C, DL_KEY_C, DL_ID_C);
    add_node(DL_VEC_D, DL_KEY_D, DL_ID_D);
    add_node(DL_VEC_E, DL_KEY_E, DL_ID_E);
    add_node(DL_VEC_F, DL_KEY_F, DL_ID_F);
    add_edge(DL_ID_C, DL_ID_D, "cd");
    add_edge(DL_ID_D, DL_ID_E, "de");
    add_edge(DL_ID_E, DL_ID_F, "ef");
    let (vecs, layers, nc, layer_count, is_dag) = gos_runtime::graph_dag_layers::<64>();
    assert!(is_dag);
    assert_eq!(nc, 6);
    // layer_count = max layer + 1 = 4 (from chain 2).
    assert_eq!(layer_count, 4, "deepest chain is C->D->E->F = 4 layers");
    // Chain 1.
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_A).unwrap(), 0);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_B).unwrap(), 1);
    // Chain 2.
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_C).unwrap(), 0);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_D).unwrap(), 1);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_E).unwrap(), 2);
    assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_F).unwrap(), 3);
}
