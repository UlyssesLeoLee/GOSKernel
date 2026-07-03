// gos-graph-flow-harness — V2.50 Edmonds-Karp maximum network flow
//
// Verifies `gos_runtime::graph_flow` — maximum flow from a source to a sink
// over the directed GOS kernel graph, treating edge weights as capacities.
//
// Algorithm summary (Edmonds-Karp):
//   1. Maintain residual graph: forward residual = capacity - flow,
//      backward residual = flow (allows cancellation).
//   2. BFS from source to sink in residual graph to find shortest
//      augmenting path.
//   3. Augment flow by the path's bottleneck (minimum residual).
//   4. Repeat until no augmenting path exists.
//   O(V × E²) complexity, polynomial and correct for integer/real capacities.
//
// Return value: (vecs, out_flow, in_flow, node_count, max_flow)
//   vecs[i]      — all live nodes (source first, sink second)
//   out_flow[i]  — per-node total outgoing flow × 1000 (u32)
//   in_flow[i]   — per-node total incoming flow × 1000 (u32)
//   node_count   — total live nodes
//   max_flow     — total maximum flow × 1000 as u32
//
// Key invariants:
//   Empty graph            → node_count=0, max_flow=0
//   Source not found       → max_flow=0
//   Sink not found         → max_flow=0
//   Source == Sink         → max_flow=0
//   K₂ A→B(w)             → max_flow = w×1000
//   Bottleneck edge        → max_flow limited by min capacity on any path
//   Parallel paths         → max_flow = sum of independent path flows
//   No path S→T           → max_flow=0
//
// Test matrix:
//  1.  Empty graph                                    → node_count=0, max_flow=0
//  2.  Single node, source==sink (degenerate)         → max_flow=0
//  3.  Source vector not registered                   → max_flow=0
//  4.  Sink vector not registered                     → max_flow=0
//  5.  K₂ A→B(cap=3.0), flow(A,B)                   → max_flow=3000
//  6.  K₂ A→B(cap=2.5), flow(A,B)                   → max_flow=2500
//  7.  Bottleneck: A→B(5.0), B→C(2.0), source=A,C   → max_flow=2000
//  8.  Diamond: A→B(3), A→C(2), B→D(3), C→D(2)     → max_flow=5000
//  9.  Extra direct: A→B(1),A→C(1),B→D(1),C→D(1),A→D(0.5) → max_flow=2500
// 10.  No path (disconnected): A→B exists, C→D exists, flow(A,D) → max_flow=0

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const FL_PLUGIN: PluginId   = PluginId::from_ascii("KL_FLW_00");
const FL_EXEC:   ExecutorId = ExecutorId::from_ascii("flow.exec0");

const FL_KEY_A: &str = "fl.alpha";
const FL_KEY_B: &str = "fl.beta";
const FL_KEY_C: &str = "fl.gamma";
const FL_KEY_D: &str = "fl.delta";

const FL_ID_A: NodeId = derive_node_id(FL_PLUGIN, FL_KEY_A);
const FL_ID_B: NodeId = derive_node_id(FL_PLUGIN, FL_KEY_B);
const FL_ID_C: NodeId = derive_node_id(FL_PLUGIN, FL_KEY_C);
const FL_ID_D: NodeId = derive_node_id(FL_PLUGIN, FL_KEY_D);

const FL_VEC_A: VectorAddress = VectorAddress::new(27, 1, 1, 0);
const FL_VEC_B: VectorAddress = VectorAddress::new(27, 1, 2, 0);
const FL_VEC_C: VectorAddress = VectorAddress::new(27, 1, 3, 0);
const FL_VEC_D: VectorAddress = VectorAddress::new(27, 1, 4, 0);
const FL_VEC_X: VectorAddress = VectorAddress::new(27, 9, 9, 9); // never registered

const FL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    FL_PLUGIN,
    name:         "kl-graph-flow-harness",
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
        executor_id:       FL_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(FL_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(FL_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_max_flow(source: VectorAddress, sink: VectorAddress) -> u32 {
    let (_vecs, _out, _inf, _total, max_flow) =
        gos_runtime::graph_flow::<128>(source, sink);
    max_flow
}

fn find_out_flow(vecs: &[VectorAddress], flows: &[u32], total: usize, target: VectorAddress) -> u32 {
    for i in 0..total { if vecs[i] == target { return flows[i]; } }
    0
}

fn find_in_flow(vecs: &[VectorAddress], in_flows: &[u32], total: usize, target: VectorAddress) -> u32 {
    for i in 0..total { if vecs[i] == target { return in_flows[i]; } }
    0
}

// ── 1. Empty graph ────────────────────────────────────────────────────────────

#[test]
fn empty_graph() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _out, _inf, total, max_flow) = gos_runtime::graph_flow::<128>(FL_VEC_A, FL_VEC_B);
    assert_eq!(total, 0, "empty: no nodes");
    assert_eq!(max_flow, 0, "empty: no flow");
}

// ── 2. Single node, source == sink → degenerate, max_flow=0 ──────────────────

#[test]
fn single_node_same_source_sink() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    // source == sink: trivially 0 flow
    let max_flow = find_max_flow(FL_VEC_A, FL_VEC_A);
    assert_eq!(max_flow, 0, "source==sink: no flow");
}

// ── 3. Source vector not registered → max_flow=0 ─────────────────────────────

#[test]
fn source_not_registered() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t3", 5.0);
    // FL_VEC_X is not registered — use as source
    let max_flow = find_max_flow(FL_VEC_X, FL_VEC_B);
    assert_eq!(max_flow, 0, "unknown source: no flow");
}

// ── 4. Sink vector not registered → max_flow=0 ───────────────────────────────

#[test]
fn sink_not_registered() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t4", 5.0);
    // FL_VEC_X is not registered — use as sink
    let max_flow = find_max_flow(FL_VEC_A, FL_VEC_X);
    assert_eq!(max_flow, 0, "unknown sink: no flow");
}

// ── 5. K₂ A→B(cap=3.0) → max_flow=3000 ──────────────────────────────────────

#[test]
fn k2_integer_capacity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t5", 3.0);
    let (vecs, out_flows, in_flows, total, max_flow) = gos_runtime::graph_flow::<128>(FL_VEC_A, FL_VEC_B);
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(max_flow, 3000, "max flow = 3.000");
    // Source A should show outflow=3000, inflow=0
    assert_eq!(find_out_flow(&vecs, &out_flows, total, FL_VEC_A), 3000, "A outflow=3.000");
    // Sink B should show inflow=3000
    assert_eq!(find_in_flow(&vecs, &in_flows, total, FL_VEC_B), 3000, "B inflow=3.000");
}

// ── 6. K₂ A→B(cap=2.5) → max_flow=2500 ──────────────────────────────────────

#[test]
fn k2_fractional_capacity() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t6", 2.5);
    let max_flow = find_max_flow(FL_VEC_A, FL_VEC_B);
    assert_eq!(max_flow, 2500, "max flow = 2.500");
}

// ── 7. Bottleneck: A→B(5.0), B→C(2.0); source=A, sink=C → max_flow=2000 ─────

#[test]
fn bottleneck_limits_flow() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_node(FL_VEC_C, FL_KEY_C, FL_ID_C, 0xF003);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t7", 5.0);
    add_edge(FL_ID_B, FL_ID_C, "fl.bc.t7", 2.0);
    let (vecs, out_flows, in_flows, total, max_flow) = gos_runtime::graph_flow::<128>(FL_VEC_A, FL_VEC_C);
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(max_flow, 2000, "bottleneck B→C(2.0) limits flow to 2.000");
    // Relay B: outflow == inflow == 2.000
    let b_out = find_out_flow(&vecs, &out_flows, total, FL_VEC_B);
    let b_in  = find_in_flow(&vecs, &in_flows, total, FL_VEC_B);
    assert_eq!(b_out, 2000, "relay B outflow=2.000");
    assert_eq!(b_in,  2000, "relay B inflow=2.000 (flow conservation)");
}

// ── 8. Diamond: A→B(3),A→C(2),B→D(3),C→D(2); source=A, sink=D → 5000 ────────

#[test]
fn diamond_parallel_paths_sum() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_node(FL_VEC_C, FL_KEY_C, FL_ID_C, 0xF003);
    add_node(FL_VEC_D, FL_KEY_D, FL_ID_D, 0xF004);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t8", 3.0);
    add_edge(FL_ID_A, FL_ID_C, "fl.ac.t8", 2.0);
    add_edge(FL_ID_B, FL_ID_D, "fl.bd.t8", 3.0);
    add_edge(FL_ID_C, FL_ID_D, "fl.cd.t8", 2.0);
    let (vecs, out_flows, in_flows, total, max_flow) = gos_runtime::graph_flow::<128>(FL_VEC_A, FL_VEC_D);
    assert_eq!(total, 4, "4 nodes");
    // A→B(3)→D(3) and A→C(2)→D(2) are independent; total = 5.
    assert_eq!(max_flow, 5000, "parallel paths: 3+2 = 5.000");
    // Sink D receives total 5.000
    assert_eq!(find_in_flow(&vecs, &in_flows, total, FL_VEC_D), 5000, "D inflow=5.000");
    // Source A sends total 5.000
    assert_eq!(find_out_flow(&vecs, &out_flows, total, FL_VEC_A), 5000, "A outflow=5.000");
}

// ── 9. Extra direct: A→B(1),A→C(1),B→D(1),C→D(1),A→D(0.5) → max_flow=2500 ──

#[test]
fn three_path_sum_with_direct_edge() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_node(FL_VEC_C, FL_KEY_C, FL_ID_C, 0xF003);
    add_node(FL_VEC_D, FL_KEY_D, FL_ID_D, 0xF004);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t9", 1.0);
    add_edge(FL_ID_A, FL_ID_C, "fl.ac.t9", 1.0);
    add_edge(FL_ID_B, FL_ID_D, "fl.bd.t9", 1.0);
    add_edge(FL_ID_C, FL_ID_D, "fl.cd.t9", 1.0);
    add_edge(FL_ID_A, FL_ID_D, "fl.ad.t9", 0.5);
    let (_vecs, _out, _inf, total, max_flow) = gos_runtime::graph_flow::<128>(FL_VEC_A, FL_VEC_D);
    assert_eq!(total, 4, "4 nodes");
    // A→B→D(1) + A→C→D(1) + A→D(0.5) = 2.5
    assert_eq!(max_flow, 2500, "three paths: 1+1+0.5 = 2.500");
}

// ── 10. Disconnected: A→B exists, C→D exists; flow(A,D) → max_flow=0 ─────────

#[test]
fn no_path_between_components() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(FL_VEC_A, FL_KEY_A, FL_ID_A, 0xF001);
    add_node(FL_VEC_B, FL_KEY_B, FL_ID_B, 0xF002);
    add_node(FL_VEC_C, FL_KEY_C, FL_ID_C, 0xF003);
    add_node(FL_VEC_D, FL_KEY_D, FL_ID_D, 0xF004);
    add_edge(FL_ID_A, FL_ID_B, "fl.ab.t10", 5.0); // component 1
    add_edge(FL_ID_C, FL_ID_D, "fl.cd.t10", 3.0); // component 2, disconnected
    // No path from A to D across disconnected components.
    let max_flow = find_max_flow(FL_VEC_A, FL_VEC_D);
    assert_eq!(max_flow, 0, "disconnected: A cannot reach D, max_flow=0");
}
