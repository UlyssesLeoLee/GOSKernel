// gos-graph-degree-harness — V2.38 in/out degree census API tests
//
// Verifies `gos_runtime::graph_degree` — in/out degree census sorted by
// descending total degree.  Analogous to `ip -s link show` per-interface
// TX/RX packet statistics applied to a directed graph.
//
//  1. Empty graph  → total=0, no panics.
//  2. Single isolated node → out=0, in=0, total-degree=0.
//  3. Single directed edge A→B → A: out=1 in=0, B: out=0 in=1.
//  4. Path A→B→C  → B has highest total-degree (out=1 in=1=2), A and C =1.
//  5. Self-loop A→A → A: out=1, in=1 (self-loop counts to both).
//  6. Fan-out hub: H→{A,B,C} → H has out=3, in=0, highest total-degree.
//  7. Fan-in  hub: {A,B,C}→H → H has out=0, in=3, highest total-degree.
//  8. Bidirectional edge A⇄B (two separate directed edges) → each: out=1 in=1.
//  9. Sort order: nodes appear in descending total-degree order in output.
// 10. Diamond A→B, A→C, B→D, C→D → D and A have total=2; B and C have total=2.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const DEG_PLUGIN: PluginId   = PluginId::from_ascii("KL_DEG0_00");
const DEG_EXEC:   ExecutorId = ExecutorId::from_ascii("deg.exec");

const DEG_KEY_A: &str = "deg.alpha";
const DEG_KEY_B: &str = "deg.beta";
const DEG_KEY_C: &str = "deg.gamma";
const DEG_KEY_D: &str = "deg.delta";
const DEG_KEY_H: &str = "deg.hub";

const DEG_ID_A: NodeId = derive_node_id(DEG_PLUGIN, DEG_KEY_A);
const DEG_ID_B: NodeId = derive_node_id(DEG_PLUGIN, DEG_KEY_B);
const DEG_ID_C: NodeId = derive_node_id(DEG_PLUGIN, DEG_KEY_C);
const DEG_ID_D: NodeId = derive_node_id(DEG_PLUGIN, DEG_KEY_D);
const DEG_ID_H: NodeId = derive_node_id(DEG_PLUGIN, DEG_KEY_H);

const DEG_VEC_A: VectorAddress = VectorAddress::new(15, 2, 1, 0);
const DEG_VEC_B: VectorAddress = VectorAddress::new(15, 2, 2, 0);
const DEG_VEC_C: VectorAddress = VectorAddress::new(15, 2, 3, 0);
const DEG_VEC_D: VectorAddress = VectorAddress::new(15, 2, 4, 0);
const DEG_VEC_H: VectorAddress = VectorAddress::new(15, 2, 5, 0);

const DEG_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    DEG_PLUGIN,
    name:         "kl-graph-degree-harness",
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
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: DEG_EXEC,
        state_schema_hash: schema,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_plugin() {
    gos_runtime::discover_plugin(DEG_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(DEG_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    let spec = EdgeSpec {
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
    };
    gos_runtime::register_edge(spec).unwrap();
}

fn find_degrees(
    vecs: &[VectorAddress],
    out_degs: &[u16],
    in_degs: &[u16],
    total: usize,
    target: VectorAddress,
) -> Option<(u16, u16)> {
    for i in 0..total {
        if vecs[i] == target {
            return Some((out_degs[i], in_degs[i]));
        }
    }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_degree_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _out, _in, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → out=0, in=0 ────────────────────────────────────

#[test]
fn isolated_node_has_zero_degree() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 1, "1 node");
    let (out, inc) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_A)
        .expect("A must be in result");
    assert_eq!(out, 0, "isolated: out-degree 0");
    assert_eq!(inc, 0, "isolated: in-degree 0");
}

// ── 3. Single directed edge A→B ───────────────────────────────────────────────

#[test]
fn single_edge_degrees() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_node(DEG_VEC_B, DEG_KEY_B, DEG_ID_B, 0xD002);
    add_edge(DEG_ID_A, DEG_ID_B, "deg.ab.t3");
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 2, "2 nodes");
    let (a_out, a_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_A).unwrap();
    let (b_out, b_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_B).unwrap();
    assert_eq!(a_out, 1, "A: out=1 (sends to B)");
    assert_eq!(a_in,  0, "A: in=0 (nothing points to A)");
    assert_eq!(b_out, 0, "B: out=0 (sends nowhere)");
    assert_eq!(b_in,  1, "B: in=1 (receives from A)");
}

// ── 4. Path A→B→C  →  B total-degree=2, A and C total-degree=1 ───────────────

#[test]
fn path_middle_node_highest_degree() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_node(DEG_VEC_B, DEG_KEY_B, DEG_ID_B, 0xD002);
    add_node(DEG_VEC_C, DEG_KEY_C, DEG_ID_C, 0xD003);
    add_edge(DEG_ID_A, DEG_ID_B, "deg.ab.t4");
    add_edge(DEG_ID_B, DEG_ID_C, "deg.bc.t4");
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 3, "3 nodes");
    let (b_out, b_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_B).unwrap();
    assert_eq!(b_out, 1, "B: out=1");
    assert_eq!(b_in,  1, "B: in=1");
    assert_eq!((b_out + b_in) as u32, 2, "B total-degree=2 (highest in path)");
    // First slot must be B (highest total-degree).
    assert_eq!(vecs[0], DEG_VEC_B, "B must appear first (highest degree)");
}

// ── 5. Self-loop A→A  →  out=1 in=1 ─────────────────────────────────────────

#[test]
fn self_loop_counts_both_in_and_out() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_edge(DEG_ID_A, DEG_ID_A, "deg.aa.t5"); // self-loop
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 1, "1 node");
    let (out, inc) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_A).unwrap();
    assert_eq!(out, 1, "self-loop: out=1");
    assert_eq!(inc, 1, "self-loop: in=1 (same edge)");
}

// ── 6. Fan-out hub H→{A,B,C}  →  H out=3 in=0, highest total ────────────────

#[test]
fn fan_out_hub_highest_degree() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_H, DEG_KEY_H, DEG_ID_H, 0xD005);
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_node(DEG_VEC_B, DEG_KEY_B, DEG_ID_B, 0xD002);
    add_node(DEG_VEC_C, DEG_KEY_C, DEG_ID_C, 0xD003);
    add_edge(DEG_ID_H, DEG_ID_A, "deg.ha.t6");
    add_edge(DEG_ID_H, DEG_ID_B, "deg.hb.t6");
    add_edge(DEG_ID_H, DEG_ID_C, "deg.hc.t6");
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 4, "4 nodes");
    let (h_out, h_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_H).unwrap();
    assert_eq!(h_out, 3, "H: out=3 (sends to A,B,C)");
    assert_eq!(h_in,  0, "H: in=0 (nothing sends to H)");
    assert_eq!(vecs[0], DEG_VEC_H, "H must appear first (highest total)");
}

// ── 7. Fan-in  hub {A,B,C}→H  →  H in=3, highest total ─────────────────────

#[test]
fn fan_in_hub_highest_degree() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_H, DEG_KEY_H, DEG_ID_H, 0xD005);
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_node(DEG_VEC_B, DEG_KEY_B, DEG_ID_B, 0xD002);
    add_node(DEG_VEC_C, DEG_KEY_C, DEG_ID_C, 0xD003);
    add_edge(DEG_ID_A, DEG_ID_H, "deg.ah.t7");
    add_edge(DEG_ID_B, DEG_ID_H, "deg.bh.t7");
    add_edge(DEG_ID_C, DEG_ID_H, "deg.ch.t7");
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 4, "4 nodes");
    let (h_out, h_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_H).unwrap();
    assert_eq!(h_out, 0, "H: out=0");
    assert_eq!(h_in,  3, "H: in=3 (receives from A,B,C)");
    assert_eq!(vecs[0], DEG_VEC_H, "H must appear first (highest total)");
}

// ── 8. Bidirectional A⇄B  →  each node out=1 in=1 ───────────────────────────

#[test]
fn bidirectional_edge_symmetric_degrees() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_node(DEG_VEC_B, DEG_KEY_B, DEG_ID_B, 0xD002);
    add_edge(DEG_ID_A, DEG_ID_B, "deg.ab.t8");
    add_edge(DEG_ID_B, DEG_ID_A, "deg.ba.t8");
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 2, "2 nodes");
    let (a_out, a_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_A).unwrap();
    let (b_out, b_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_B).unwrap();
    assert_eq!(a_out, 1, "A: out=1");
    assert_eq!(a_in,  1, "A: in=1");
    assert_eq!(b_out, 1, "B: out=1");
    assert_eq!(b_in,  1, "B: in=1");
}

// ── 9. Sort order verified: descending total-degree ──────────────────────────

#[test]
fn output_sorted_descending_total_degree() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // H→A, H→B, H→C  (H total=3) plus B→D (B total=2), C isolated (total=1), D only sink (total=1)
    add_node(DEG_VEC_H, DEG_KEY_H, DEG_ID_H, 0xD005);
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_node(DEG_VEC_B, DEG_KEY_B, DEG_ID_B, 0xD002);
    add_node(DEG_VEC_C, DEG_KEY_C, DEG_ID_C, 0xD003);
    add_node(DEG_VEC_D, DEG_KEY_D, DEG_ID_D, 0xD004);
    add_edge(DEG_ID_H, DEG_ID_A, "deg.ha.t9");
    add_edge(DEG_ID_H, DEG_ID_B, "deg.hb.t9");
    add_edge(DEG_ID_H, DEG_ID_C, "deg.hc.t9");
    add_edge(DEG_ID_B, DEG_ID_D, "deg.bd.t9");
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 5, "5 nodes");
    // Verify descending order: each element's total ≥ the next.
    for i in 0..total.saturating_sub(1) {
        let t_i = (out_degs[i] as u32) + (in_degs[i] as u32);
        let t_j = (out_degs[i + 1] as u32) + (in_degs[i + 1] as u32);
        assert!(
            t_i >= t_j,
            "output[{}] total={} must be ≥ output[{}] total={}",
            i, t_i, i + 1, t_j
        );
    }
    // H must be first.
    assert_eq!(vecs[0], DEG_VEC_H, "H (total=3) must be first");
}

// ── 10. Diamond A→B, A→C, B→D, C→D  →  A and D total=2, B and C total=2 ────

#[test]
fn diamond_topology_degree_census() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(DEG_VEC_A, DEG_KEY_A, DEG_ID_A, 0xD001);
    add_node(DEG_VEC_B, DEG_KEY_B, DEG_ID_B, 0xD002);
    add_node(DEG_VEC_C, DEG_KEY_C, DEG_ID_C, 0xD003);
    add_node(DEG_VEC_D, DEG_KEY_D, DEG_ID_D, 0xD004);
    add_edge(DEG_ID_A, DEG_ID_B, "deg.ab.t10");
    add_edge(DEG_ID_A, DEG_ID_C, "deg.ac.t10");
    add_edge(DEG_ID_B, DEG_ID_D, "deg.bd.t10");
    add_edge(DEG_ID_C, DEG_ID_D, "deg.cd.t10");
    let (vecs, out_degs, in_degs, total) = gos_runtime::graph_degree::<128>();
    assert_eq!(total, 4, "4 nodes");
    let (a_out, a_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_A).unwrap();
    let (b_out, b_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_B).unwrap();
    let (c_out, c_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_C).unwrap();
    let (d_out, d_in) = find_degrees(&vecs, &out_degs, &in_degs, total, DEG_VEC_D).unwrap();
    // A: source — out=2, in=0
    assert_eq!(a_out, 2, "A out=2");
    assert_eq!(a_in,  0, "A in=0");
    // B: pass-through — out=1, in=1
    assert_eq!(b_out, 1, "B out=1");
    assert_eq!(b_in,  1, "B in=1");
    // C: pass-through — out=1, in=1
    assert_eq!(c_out, 1, "C out=1");
    assert_eq!(c_in,  1, "C in=1");
    // D: sink — out=0, in=2
    assert_eq!(d_out, 0, "D out=0");
    assert_eq!(d_in,  2, "D in=2");
    // All four have total-degree=2.
    for i in 0..total {
        let t = (out_degs[i] as u32) + (in_degs[i] as u32);
        assert_eq!(t, 2, "diamond: every node total=2 (node {})", i);
    }
}
