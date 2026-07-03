// gos-graph-between-harness — V2.53 weighted betweenness centrality API tests
//
// Verifies `gos_runtime::graph_between` — Brandes betweenness centrality
// using Dijkstra (edge weights) instead of BFS (hop count).
//
// WBC[v] = Σ_{s≠v≠t} σ_w(s,t,v)/σ_w(s,t) where σ_w counts minimum-weight
// directed paths.  Diverges from `graph_centrality` (V2.39) when a cheaper
// indirect path exists alongside a more expensive direct edge.
//
// VectorAddress namespace: L4=30 (graph-between harness).
//
// Test matrix:
//  1.  Empty graph → total=0, no panic.
//  2.  Single isolated node → WBC=0, total=1.
//  3.  Two-node A→B (w=1.0) → WBC[A]=WBC[B]=0 (no intermediaries).
//  4.  Path A→B→C (w=1.0 each) → WBC[B]=1 (standard bottleneck).
//  5.  Bottleneck {A,B}→X→{C,D} (w=1.0) → WBC[X]=4.
//  6.  Weight-sensitive: A→C w=0.5, C→B w=0.5, A→B w=2.0
//        → WBC[C]=1 (Dijkstra picks indirect cheaper path A→C→B;
//          `graph_centrality` BFS would give WBC[C]=0 via direct 1-hop).
//  7.  Bottleneck {A,B,C}→X→{D,E,F} (w=1.0) → WBC[X]=9.
//  8.  Linear 5-node path (w=1.0) → WBC[C]=4, WBC[B]=WBC[D]=3.
//  9.  Output is sorted descending by WBC score.
// 10.  Self-loop A→A plus isolated B → no crash, WBC[A]=0, WBC[B]=0.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const BTW_PLUGIN: PluginId   = PluginId::from_ascii("KL_BTW_000");
const BTW_EXEC:   ExecutorId = ExecutorId::from_ascii("btw.exec00");

const BTW_KEY_A: &str = "btw.alpha";
const BTW_KEY_B: &str = "btw.beta";
const BTW_KEY_C: &str = "btw.gamma";
const BTW_KEY_D: &str = "btw.delta";
const BTW_KEY_E: &str = "btw.epsilon";
const BTW_KEY_F: &str = "btw.zeta";
const BTW_KEY_X: &str = "btw.hub";

const BTW_ID_A: NodeId = derive_node_id(BTW_PLUGIN, BTW_KEY_A);
const BTW_ID_B: NodeId = derive_node_id(BTW_PLUGIN, BTW_KEY_B);
const BTW_ID_C: NodeId = derive_node_id(BTW_PLUGIN, BTW_KEY_C);
const BTW_ID_D: NodeId = derive_node_id(BTW_PLUGIN, BTW_KEY_D);
const BTW_ID_E: NodeId = derive_node_id(BTW_PLUGIN, BTW_KEY_E);
const BTW_ID_F: NodeId = derive_node_id(BTW_PLUGIN, BTW_KEY_F);
const BTW_ID_X: NodeId = derive_node_id(BTW_PLUGIN, BTW_KEY_X);

const BTW_VEC_A: VectorAddress = VectorAddress::new(30, 1, 1, 0);
const BTW_VEC_B: VectorAddress = VectorAddress::new(30, 1, 2, 0);
const BTW_VEC_C: VectorAddress = VectorAddress::new(30, 1, 3, 0);
const BTW_VEC_D: VectorAddress = VectorAddress::new(30, 1, 4, 0);
const BTW_VEC_E: VectorAddress = VectorAddress::new(30, 1, 5, 0);
const BTW_VEC_F: VectorAddress = VectorAddress::new(30, 1, 6, 0);
const BTW_VEC_X: VectorAddress = VectorAddress::new(30, 1, 7, 0);

const BTW_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    BTW_PLUGIN,
    name:         "kl-graph-between-harness",
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
        executor_id:       BTW_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_plugin() {
    gos_runtime::discover_plugin(BTW_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(BTW_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str, weight: f32) {
    let spec = EdgeSpec {
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
    };
    gos_runtime::register_edge(spec).unwrap();
}

fn find_wbc(vecs: &[VectorAddress], wbc: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(wbc[i]); }
    }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_between_total_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → WBC=0 ─────────────────────────────────────────

#[test]
fn isolated_node_has_zero_wbc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 1, "1 node");
    let score = find_wbc(&vecs, &wbc, total, BTW_VEC_A).expect("A must be in result");
    assert_eq!(score, 0, "isolated node: WBC=0 (no paths through it)");
}

// ── 3. Two nodes A→B → WBC[A]=WBC[B]=0 ──────────────────────────────────────

#[test]
fn two_nodes_one_edge_both_zero_wbc() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_edge(BTW_ID_A, BTW_ID_B, "btw.ab.t3", 1.0);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_wbc(&vecs, &wbc, total, BTW_VEC_A).unwrap();
    let b = find_wbc(&vecs, &wbc, total, BTW_VEC_B).unwrap();
    assert_eq!(a, 0, "A: WBC=0 (source, no s≠A≠t pair uses A as intermediary)");
    assert_eq!(b, 0, "B: WBC=0 (sink, no s≠B≠t pair uses B as intermediary)");
}

// ── 4. Path A→B→C (w=1.0 each) → WBC[B]=1 ───────────────────────────────────

#[test]
fn path_abc_uniform_weight_middle_node_wbc_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_node(BTW_VEC_C, BTW_KEY_C, BTW_ID_C, 0xB003);
    add_edge(BTW_ID_A, BTW_ID_B, "btw.ab.t4", 1.0);
    add_edge(BTW_ID_B, BTW_ID_C, "btw.bc.t4", 1.0);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 3);
    let a = find_wbc(&vecs, &wbc, total, BTW_VEC_A).unwrap();
    let b = find_wbc(&vecs, &wbc, total, BTW_VEC_B).unwrap();
    let c = find_wbc(&vecs, &wbc, total, BTW_VEC_C).unwrap();
    assert_eq!(b, 1, "B: WBC=1 (sole intermediary on min-weight path A→C)");
    assert_eq!(a, 0, "A: WBC=0 (source, never an intermediary)");
    assert_eq!(c, 0, "C: WBC=0 (sink, never an intermediary)");
    assert_eq!(vecs[0], BTW_VEC_B, "B (WBC=1) must be first in output");
}

// ── 5. Bottleneck {A,B}→X→{C,D} → WBC[X]=4 ──────────────────────────────────

#[test]
fn bottleneck_two_into_two_wbc_four() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_node(BTW_VEC_X, BTW_KEY_X, BTW_ID_X, 0xB007);
    add_node(BTW_VEC_C, BTW_KEY_C, BTW_ID_C, 0xB003);
    add_node(BTW_VEC_D, BTW_KEY_D, BTW_ID_D, 0xB004);
    add_edge(BTW_ID_A, BTW_ID_X, "btw.ax.t5", 1.0);
    add_edge(BTW_ID_B, BTW_ID_X, "btw.bx.t5", 1.0);
    add_edge(BTW_ID_X, BTW_ID_C, "btw.xc.t5", 1.0);
    add_edge(BTW_ID_X, BTW_ID_D, "btw.xd.t5", 1.0);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 5, "5 nodes");
    let x = find_wbc(&vecs, &wbc, total, BTW_VEC_X).unwrap();
    assert_eq!(x, 4, "X: WBC=4 (pairs A→C, A→D, B→C, B→D all go through X)");
    assert_eq!(vecs[0], BTW_VEC_X, "X (WBC=4) must be first");
}

// ── 6. Weight-sensitive: indirect cheaper path → relay is now a keystone ──────
//
// Topology: A→C (w=0.5), C→B (w=0.5), A→B (w=2.0)
// Dijkstra: A→C→B costs 1.0 < A→B direct costs 2.0 → C is on min-weight path.
// WBC[C]=1 (unlike BFS/graph_centrality which sees A→B as 1 hop and ignores C).

#[test]
fn weight_sensitive_cheaper_path_via_relay_node() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_node(BTW_VEC_C, BTW_KEY_C, BTW_ID_C, 0xB003);
    add_edge(BTW_ID_A, BTW_ID_C, "btw.ac.t6", 0.5);
    add_edge(BTW_ID_C, BTW_ID_B, "btw.cb.t6", 0.5);
    add_edge(BTW_ID_A, BTW_ID_B, "btw.ab.t6", 2.0);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 3, "3 nodes");
    let c = find_wbc(&vecs, &wbc, total, BTW_VEC_C).unwrap();
    let a = find_wbc(&vecs, &wbc, total, BTW_VEC_A).unwrap();
    let b = find_wbc(&vecs, &wbc, total, BTW_VEC_B).unwrap();
    // Dijkstra selects A→C→B (cost 1.0) over A→B (cost 2.0):
    // σ_w(A,B)=1, path goes through C → WBC[C]+=1
    assert_eq!(c, 1, "C: WBC=1 (on the only min-weight path A→B via cheaper indirect route)");
    assert_eq!(a, 0, "A: WBC=0 (source)");
    assert_eq!(b, 0, "B: WBC=0 (sink)");
    assert_eq!(vecs[0], BTW_VEC_C, "C (WBC=1) must be first in output");
}

// ── 7. Bottleneck {A,B,C}→X→{D,E,F} → WBC[X]=9 ─────────────────────────────

#[test]
fn bottleneck_three_into_three_wbc_nine() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_node(BTW_VEC_C, BTW_KEY_C, BTW_ID_C, 0xB003);
    add_node(BTW_VEC_X, BTW_KEY_X, BTW_ID_X, 0xB007);
    add_node(BTW_VEC_D, BTW_KEY_D, BTW_ID_D, 0xB004);
    add_node(BTW_VEC_E, BTW_KEY_E, BTW_ID_E, 0xB005);
    add_node(BTW_VEC_F, BTW_KEY_F, BTW_ID_F, 0xB006);
    add_edge(BTW_ID_A, BTW_ID_X, "btw.ax.t7", 1.0);
    add_edge(BTW_ID_B, BTW_ID_X, "btw.bx.t7", 1.0);
    add_edge(BTW_ID_C, BTW_ID_X, "btw.cx.t7", 1.0);
    add_edge(BTW_ID_X, BTW_ID_D, "btw.xd.t7", 1.0);
    add_edge(BTW_ID_X, BTW_ID_E, "btw.xe.t7", 1.0);
    add_edge(BTW_ID_X, BTW_ID_F, "btw.xf.t7", 1.0);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 7, "7 nodes");
    let x = find_wbc(&vecs, &wbc, total, BTW_VEC_X).unwrap();
    assert_eq!(x, 9, "X: WBC=9 (3×3 pairs A/B/C → D/E/F all through X)");
    assert_eq!(vecs[0], BTW_VEC_X, "X (WBC=9) must be first");
}

// ── 8. Linear 5-node A→B→C→D→E (w=1.0) ─────────────────────────────────────

#[test]
fn linear_five_node_path_wbc_values() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_node(BTW_VEC_C, BTW_KEY_C, BTW_ID_C, 0xB003);
    add_node(BTW_VEC_D, BTW_KEY_D, BTW_ID_D, 0xB004);
    add_node(BTW_VEC_E, BTW_KEY_E, BTW_ID_E, 0xB005);
    add_edge(BTW_ID_A, BTW_ID_B, "btw.ab.t8", 1.0);
    add_edge(BTW_ID_B, BTW_ID_C, "btw.bc.t8", 1.0);
    add_edge(BTW_ID_C, BTW_ID_D, "btw.cd.t8", 1.0);
    add_edge(BTW_ID_D, BTW_ID_E, "btw.de.t8", 1.0);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 5, "5 nodes");
    let a = find_wbc(&vecs, &wbc, total, BTW_VEC_A).unwrap();
    let b = find_wbc(&vecs, &wbc, total, BTW_VEC_B).unwrap();
    let c = find_wbc(&vecs, &wbc, total, BTW_VEC_C).unwrap();
    let d = find_wbc(&vecs, &wbc, total, BTW_VEC_D).unwrap();
    let e = find_wbc(&vecs, &wbc, total, BTW_VEC_E).unwrap();
    // Uniform weights → identical to graph_centrality:
    // WBC[C]=4, WBC[B]=3, WBC[D]=3, WBC[A]=0, WBC[E]=0.
    assert_eq!(c, 4, "C: WBC=4 (centre of 5-node path)");
    assert_eq!(b, 3, "B: WBC=3 (3 pairs from A route through B)");
    assert_eq!(d, 3, "D: WBC=3 (3 pairs to E route through D)");
    assert_eq!(a, 0, "A: WBC=0 (source, never an intermediary)");
    assert_eq!(e, 0, "E: WBC=0 (sink, never an intermediary)");
    assert_eq!(vecs[0], BTW_VEC_C, "C (WBC=4) must be first");
}

// ── 9. Sort order: output is descending by WBC score ─────────────────────────

#[test]
fn output_sorted_descending_by_wbc_score() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Reuse linear-5 topology (WBC: C=4, B=D=3, A=E=0).
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_node(BTW_VEC_C, BTW_KEY_C, BTW_ID_C, 0xB003);
    add_node(BTW_VEC_D, BTW_KEY_D, BTW_ID_D, 0xB004);
    add_node(BTW_VEC_E, BTW_KEY_E, BTW_ID_E, 0xB005);
    add_edge(BTW_ID_A, BTW_ID_B, "btw.ab.t9", 1.0);
    add_edge(BTW_ID_B, BTW_ID_C, "btw.bc.t9", 1.0);
    add_edge(BTW_ID_C, BTW_ID_D, "btw.cd.t9", 1.0);
    add_edge(BTW_ID_D, BTW_ID_E, "btw.de.t9", 1.0);
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 5);
    for i in 1..total {
        assert!(
            wbc[i - 1] >= wbc[i],
            "output must be descending: wbc[{}]={} < wbc[{}]={}",
            i - 1, wbc[i - 1], i, wbc[i]
        );
    }
    let _ = vecs;
}

// ── 10. Self-loop A→A + isolated B → no crash, WBC=0 ─────────────────────────

#[test]
fn self_loop_no_crash_wbc_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(BTW_VEC_A, BTW_KEY_A, BTW_ID_A, 0xB001);
    add_node(BTW_VEC_B, BTW_KEY_B, BTW_ID_B, 0xB002);
    add_edge(BTW_ID_A, BTW_ID_A, "btw.aa.t10", 1.0); // self-loop
    let (vecs, wbc, total) = gos_runtime::graph_between::<128>();
    assert_eq!(total, 2, "2 nodes (A with self-loop + isolated B)");
    let a = find_wbc(&vecs, &wbc, total, BTW_VEC_A).unwrap();
    let b = find_wbc(&vecs, &wbc, total, BTW_VEC_B).unwrap();
    assert_eq!(a, 0, "A: WBC=0 (self-loop is not an intermediary path)");
    assert_eq!(b, 0, "B: WBC=0 (isolated)");
}
