// gos-node-info-harness — V2.23 node info tests
//
// Verifies the combined proc_stat_for_vector + edge_page_for_node API surface
// that backs the new `node info <vec>` / `ninfo <vec>` shell commands.
//
//  1. proc_stat_for_vector on unknown vector returns None.
//  2. proc_stat_for_vector on registered node returns Some with correct key.
//  3. edge_page_for_node on unknown vector returns NodeNotFound.
//  4. Registered node with no edges: edge_page_for_node returns 0 edges.
//  5. After register_edge: edge_page_for_node returns 1 edge from source node.
//  6. From-node edge has Outbound direction; to-node edge has Inbound direction.
//  7. signal_count in proc_stat starts at zero for a freshly registered node.
//  8. edge_out_count matches the number of outbound edges registered.
//  9. After fault_node, proc_stat lifecycle is Faulted; edges still visible.
// 10. After resume_node, proc_stat lifecycle is Ready; edges still visible.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id,
    EdgeSpec, EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    GraphEdgeDirection, NodeId, NodeLifecycle, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};
use gos_runtime::RuntimeError;

static TEST_LOCK: Mutex<()> = Mutex::new(());

const NI_PLUGIN: PluginId   = PluginId::from_ascii("KL_NINFO_H");
const NI_EXEC:   ExecutorId = ExecutorId::from_ascii("ni.exec");

const NI_KEY_A: &str = "ni.alpha";
const NI_KEY_B: &str = "ni.beta";

const NI_ID_A: NodeId = derive_node_id(NI_PLUGIN, NI_KEY_A);
const NI_ID_B: NodeId = derive_node_id(NI_PLUGIN, NI_KEY_B);

const NI_VEC_A: VectorAddress = VectorAddress::new(9, 3, 1, 0);
const NI_VEC_B: VectorAddress = VectorAddress::new(9, 3, 1, 1);
const NI_VEC_UNKNOWN: VectorAddress = VectorAddress::new(9, 9, 7, 7);

const NI_EDGE_KEY: &str = "ni.link";
const NI_EDGE_ID: gos_protocol::EdgeId = derive_edge_id(NI_ID_A, NI_ID_B, NI_EDGE_KEY);

const NI_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    NI_PLUGIN,
    name:         "kl-node-info-harness",
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

fn spec(node_id: NodeId, key: &'static str) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NI_EXEC,
        state_schema_hash: 0xAB23,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn edge_spec_ab() -> EdgeSpec {
    EdgeSpec {
        edge_id:               NI_EDGE_ID,
        from_node:             NI_ID_A,
        to_node:               NI_ID_B,
        edge_type:             RuntimeEdgeType::Call,
        weight:                1.0,
        acl_mask:              u64::MAX,
        route_policy:          RoutePolicy::Direct,
        capability_namespace:  Some("ni"),
        capability_binding:    Some("link"),
        vector_ref:            None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_ab() {
    gos_runtime::discover_plugin(NI_MANIFEST).unwrap();
    gos_runtime::register_node(NI_PLUGIN, NI_VEC_A, spec(NI_ID_A, NI_KEY_A)).unwrap();
    gos_runtime::register_node(NI_PLUGIN, NI_VEC_B, spec(NI_ID_B, NI_KEY_B)).unwrap();
}

// ── 1. proc_stat_for_vector on unknown vector returns None ───────────────────

#[test]
fn node_info_stat_unknown_returns_none() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let result = gos_runtime::proc_stat_for_vector(NI_VEC_UNKNOWN);
    assert!(result.is_none());
}

// ── 2. proc_stat_for_vector on registered node returns Some with correct key ─

#[test]
fn node_info_stat_registered_node_returns_correct_key() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(NI_MANIFEST).unwrap();
    gos_runtime::register_node(NI_PLUGIN, NI_VEC_A, spec(NI_ID_A, NI_KEY_A)).unwrap();
    let summary = gos_runtime::proc_stat_for_vector(NI_VEC_A).unwrap();
    assert_eq!(summary.local_node_key, NI_KEY_A);
}

// ── 3. edge_page_for_node on unknown vector returns NodeNotFound ─────────────

#[test]
fn node_info_edge_page_unknown_returns_not_found() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let mut edges = [gos_protocol::GraphEdgeSummary::EMPTY; 8];
    let result = gos_runtime::edge_page_for_node(NI_VEC_UNKNOWN, 0, &mut edges);
    assert_eq!(result, Err(RuntimeError::NodeNotFound));
}

// ── 4. Registered node with no edges: edge_page_for_node returns 0 edges ─────

#[test]
fn node_info_no_edges_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(NI_MANIFEST).unwrap();
    gos_runtime::register_node(NI_PLUGIN, NI_VEC_A, spec(NI_ID_A, NI_KEY_A)).unwrap();
    let mut edges = [gos_protocol::GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page_for_node(NI_VEC_A, 0, &mut edges).unwrap();
    assert_eq!(total, 0);
    assert_eq!(returned, 0);
}

// ── 5. After register_edge: edge_page_for_node returns 1 edge from source ────

#[test]
fn node_info_one_edge_returned_for_source_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    gos_runtime::register_edge(edge_spec_ab()).unwrap();
    let mut edges = [gos_protocol::GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page_for_node(NI_VEC_A, 0, &mut edges).unwrap();
    assert_eq!(total, 1);
    assert_eq!(returned, 1);
    assert_eq!(edges[0].from_vector, NI_VEC_A);
    assert_eq!(edges[0].to_vector, NI_VEC_B);
}

// ── 6. From-node edge is Outbound; to-node edge is Inbound ───────────────────

#[test]
fn node_info_edge_directions_correct() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    gos_runtime::register_edge(edge_spec_ab()).unwrap();

    let mut edges_a = [gos_protocol::GraphEdgeSummary::EMPTY; 8];
    let (_, returned_a) = gos_runtime::edge_page_for_node(NI_VEC_A, 0, &mut edges_a).unwrap();
    assert_eq!(returned_a, 1);
    assert_eq!(edges_a[0].direction, GraphEdgeDirection::Outbound);

    let mut edges_b = [gos_protocol::GraphEdgeSummary::EMPTY; 8];
    let (_, returned_b) = gos_runtime::edge_page_for_node(NI_VEC_B, 0, &mut edges_b).unwrap();
    assert_eq!(returned_b, 1);
    assert_eq!(edges_b[0].direction, GraphEdgeDirection::Inbound);
}

// ── 7. signal_count starts at zero for freshly registered node ───────────────

#[test]
fn node_info_signal_count_starts_at_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(NI_MANIFEST).unwrap();
    gos_runtime::register_node(NI_PLUGIN, NI_VEC_A, spec(NI_ID_A, NI_KEY_A)).unwrap();
    let summary = gos_runtime::proc_stat_for_vector(NI_VEC_A).unwrap();
    assert_eq!(summary.signal_count, 0);
}

// ── 8. edge_out_count matches outbound edges registered ──────────────────────

#[test]
fn node_info_edge_out_count_matches_registered_edges() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    // Before any edge: edge_out_count == 0
    let before = gos_runtime::proc_stat_for_vector(NI_VEC_A).unwrap();
    assert_eq!(before.edge_out_count, 0);
    // After registering outbound edge: edge_out_count == 1
    gos_runtime::register_edge(edge_spec_ab()).unwrap();
    let after = gos_runtime::proc_stat_for_vector(NI_VEC_A).unwrap();
    assert_eq!(after.edge_out_count, 1);
}

// ── 9. After fault_node: lifecycle is Faulted; edges still visible ───────────

#[test]
fn node_info_edges_visible_after_fault() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    gos_runtime::register_edge(edge_spec_ab()).unwrap();
    gos_runtime::fault_node(NI_VEC_A).unwrap();

    let summary = gos_runtime::proc_stat_for_vector(NI_VEC_A).unwrap();
    assert_eq!(summary.lifecycle, NodeLifecycle::Faulted);

    let mut edges = [gos_protocol::GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page_for_node(NI_VEC_A, 0, &mut edges).unwrap();
    assert_eq!(total, 1);
    assert_eq!(returned, 1);
}

// ── 10. After resume_node: lifecycle is Ready; edges still visible ────────────

#[test]
fn node_info_edges_visible_after_resume() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    gos_runtime::register_edge(edge_spec_ab()).unwrap();
    gos_runtime::fault_node(NI_VEC_A).unwrap();
    gos_runtime::resume_node(NI_VEC_A).unwrap();

    let summary = gos_runtime::proc_stat_for_vector(NI_VEC_A).unwrap();
    assert_eq!(summary.lifecycle, NodeLifecycle::Ready);

    let mut edges = [gos_protocol::GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page_for_node(NI_VEC_A, 0, &mut edges).unwrap();
    assert_eq!(total, 1);
    assert_eq!(returned, 1);
    assert_eq!(edges[0].edge_type, RuntimeEdgeType::Call);
}
