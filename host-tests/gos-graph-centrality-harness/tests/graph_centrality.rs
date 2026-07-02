// gos-graph-centrality-harness — V2.39 betweenness centrality API tests
//
// Verifies `gos_runtime::graph_centrality` — Brandes' betweenness centrality
// on the live directed graph.  BC[v] = Σ_{s≠v≠t} σ(s,t,v)/σ(s,t), where
// σ(s,t) = number of shortest paths from s to t and σ(s,t,v) = those through v.
// OS analogy: traceroute hop-frequency — which node handles the most routing.
//
//  1. Empty graph → total=0, no panics.
//  2. Single isolated node → BC=0, total=1.
//  3. Two-node edge A→B → BC[A]=BC[B]=0 (no intermediaries possible).
//  4. Path A→B→C → BC[B]=1 (only node on unique shortest path A→C).
//  5. Bottleneck {A,B}→X→{C,D}: all cross-paths go through X → BC[X]=4.
//  6. Bottleneck {A,B,C}→X→{D,E,F}: 9 cross-paths through X → BC[X]=9.
//  7. Linear 5-node A→B→C→D→E: BC[C]=4 (max), BC[B]=BC[D]=3, BC[A]=BC[E]=0.
//  8. Fork-join A→{B,C}→E→F: BC[E]=3 (bottleneck after fork), others ≤ 0.
//  9. Output is sorted descending by BC score.
// 10. Self-loop A→A plus isolated B: no crash, BC[A]=0 (self-loop not a path).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const CTR_PLUGIN: PluginId   = PluginId::from_ascii("KL_CTR0_00");
const CTR_EXEC:   ExecutorId = ExecutorId::from_ascii("ctr.exec");

const CTR_KEY_A: &str = "ctr.alpha";
const CTR_KEY_B: &str = "ctr.beta";
const CTR_KEY_C: &str = "ctr.gamma";
const CTR_KEY_D: &str = "ctr.delta";
const CTR_KEY_E: &str = "ctr.epsilon";
const CTR_KEY_F: &str = "ctr.zeta";
const CTR_KEY_X: &str = "ctr.hub";

const CTR_ID_A: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_A);
const CTR_ID_B: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_B);
const CTR_ID_C: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_C);
const CTR_ID_D: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_D);
const CTR_ID_E: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_E);
const CTR_ID_F: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_F);
const CTR_ID_X: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_X);

const CTR_VEC_A: VectorAddress = VectorAddress::new(16, 1, 1, 0);
const CTR_VEC_B: VectorAddress = VectorAddress::new(16, 1, 2, 0);
const CTR_VEC_C: VectorAddress = VectorAddress::new(16, 1, 3, 0);
const CTR_VEC_D: VectorAddress = VectorAddress::new(16, 1, 4, 0);
const CTR_VEC_E: VectorAddress = VectorAddress::new(16, 1, 5, 0);
const CTR_VEC_F: VectorAddress = VectorAddress::new(16, 1, 6, 0);
const CTR_VEC_X: VectorAddress = VectorAddress::new(16, 1, 7, 0);

const CTR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    CTR_PLUGIN,
    name:         "kl-graph-centrality-harness",
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
        executor_id:       CTR_EXEC,
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
    gos_runtime::discover_plugin(CTR_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(CTR_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_bc(vecs: &[VectorAddress], bc: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(bc[i]); }
    }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_centrality_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → BC=0 ───────────────────────────────────────────

#[test]
fn isolated_node_has_zero_centrality() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 1, "1 node");
    let score = find_bc(&vecs, &bc, total, CTR_VEC_A).expect("A must be in result");
    assert_eq!(score, 0, "isolated node: BC=0 (no paths go through it)");
}

// ── 3. Two nodes A→B → BC[A]=0, BC[B]=0 ──────────────────────────────────────

#[test]
fn two_nodes_one_edge_both_zero_centrality() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t3");
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_bc(&vecs, &bc, total, CTR_VEC_A).unwrap();
    let b = find_bc(&vecs, &bc, total, CTR_VEC_B).unwrap();
    assert_eq!(a, 0, "A: BC=0 (source, no s≠A≠t path uses A)");
    assert_eq!(b, 0, "B: BC=0 (sink, no s≠B≠t path uses B)");
}

// ── 4. Path A→B→C → BC[B]=1 ─────────────────────────────────────────────────

#[test]
fn path_abc_middle_node_centrality_is_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xC003);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t4");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t4");
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 3);
    let a = find_bc(&vecs, &bc, total, CTR_VEC_A).unwrap();
    let b = find_bc(&vecs, &bc, total, CTR_VEC_B).unwrap();
    let c = find_bc(&vecs, &bc, total, CTR_VEC_C).unwrap();
    assert_eq!(b, 1, "B: BC=1 (sole intermediary for A→C)");
    assert_eq!(a, 0, "A: BC=0 (source, no path has A as intermediary)");
    assert_eq!(c, 0, "C: BC=0 (sink, no path has C as intermediary)");
    // B must appear first (highest BC).
    assert_eq!(vecs[0], CTR_VEC_B, "B (BC=1) must be first in output");
}

// ── 5. Bottleneck {A,B}→X→{C,D}: BC[X]=4 ────────────────────────────────────

#[test]
fn bottleneck_two_into_two_centrality_four() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_node(CTR_VEC_X, CTR_KEY_X, CTR_ID_X, 0xC007);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xC003);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xC004);
    add_edge(CTR_ID_A, CTR_ID_X, "ctr.ax.t5");
    add_edge(CTR_ID_B, CTR_ID_X, "ctr.bx.t5");
    add_edge(CTR_ID_X, CTR_ID_C, "ctr.xc.t5");
    add_edge(CTR_ID_X, CTR_ID_D, "ctr.xd.t5");
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 5, "5 nodes");
    let x = find_bc(&vecs, &bc, total, CTR_VEC_X).unwrap();
    assert_eq!(x, 4, "X: BC=4 (pairs: A→C, A→D, B→C, B→D all through X)");
    assert_eq!(vecs[0], CTR_VEC_X, "X (BC=4) must be first in output");
}

// ── 6. Bottleneck {A,B,C}→X→{D,E,F}: BC[X]=9 ────────────────────────────────

#[test]
fn bottleneck_three_into_three_centrality_nine() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xC003);
    add_node(CTR_VEC_X, CTR_KEY_X, CTR_ID_X, 0xC007);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xC004);
    add_node(CTR_VEC_E, CTR_KEY_E, CTR_ID_E, 0xC005);
    add_node(CTR_VEC_F, CTR_KEY_F, CTR_ID_F, 0xC006);
    add_edge(CTR_ID_A, CTR_ID_X, "ctr.ax.t6");
    add_edge(CTR_ID_B, CTR_ID_X, "ctr.bx.t6");
    add_edge(CTR_ID_C, CTR_ID_X, "ctr.cx.t6");
    add_edge(CTR_ID_X, CTR_ID_D, "ctr.xd.t6");
    add_edge(CTR_ID_X, CTR_ID_E, "ctr.xe.t6");
    add_edge(CTR_ID_X, CTR_ID_F, "ctr.xf.t6");
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 7, "7 nodes");
    let x = find_bc(&vecs, &bc, total, CTR_VEC_X).unwrap();
    assert_eq!(x, 9, "X: BC=9 (3×3 pairs: A/B/C → D/E/F all through X)");
    assert_eq!(vecs[0], CTR_VEC_X, "X (BC=9) must be first in output");
}

// ── 7. Linear 5-node path A→B→C→D→E ─────────────────────────────────────────

#[test]
fn linear_five_node_path_centrality_values() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xC003);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xC004);
    add_node(CTR_VEC_E, CTR_KEY_E, CTR_ID_E, 0xC005);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t7");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t7");
    add_edge(CTR_ID_C, CTR_ID_D, "ctr.cd.t7");
    add_edge(CTR_ID_D, CTR_ID_E, "ctr.de.t7");
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 5, "5 nodes");
    // BC[C]=4 (A→D, A→E, B→D, B→E all pass through C)
    // BC[B]=3 (A→C, A→D, A→E pass through B)
    // BC[D]=3 (B→E, C→E, A→E pass through D)
    // BC[A]=0, BC[E]=0
    let a = find_bc(&vecs, &bc, total, CTR_VEC_A).unwrap();
    let b = find_bc(&vecs, &bc, total, CTR_VEC_B).unwrap();
    let c = find_bc(&vecs, &bc, total, CTR_VEC_C).unwrap();
    let d = find_bc(&vecs, &bc, total, CTR_VEC_D).unwrap();
    let e = find_bc(&vecs, &bc, total, CTR_VEC_E).unwrap();
    assert_eq!(a, 0, "A: BC=0 (source, no one reaches it as intermediary)");
    assert_eq!(e, 0, "E: BC=0 (sink, no path uses E as intermediary)");
    assert_eq!(c, 4, "C: BC=4 (middle node, 4 pairs route through it)");
    assert_eq!(b, 3, "B: BC=3 (3 pairs from A route through B)");
    assert_eq!(d, 3, "D: BC=3 (3 pairs to E route through D)");
    // C must be first (highest BC).
    assert_eq!(vecs[0], CTR_VEC_C, "C (BC=4) must be first");
}

// ── 8. Fork-join A→{B,C}→E→F: BC[E]=3 ───────────────────────────────────────

#[test]
fn fork_join_bottleneck_centrality() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // A → B, A → C, B → E, C → E, E → F
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xC003);
    add_node(CTR_VEC_E, CTR_KEY_E, CTR_ID_E, 0xC005);
    add_node(CTR_VEC_F, CTR_KEY_F, CTR_ID_F, 0xC006);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t8");
    add_edge(CTR_ID_A, CTR_ID_C, "ctr.ac.t8");
    add_edge(CTR_ID_B, CTR_ID_E, "ctr.be.t8");
    add_edge(CTR_ID_C, CTR_ID_E, "ctr.ce.t8");
    add_edge(CTR_ID_E, CTR_ID_F, "ctr.ef.t8");
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 5, "5 nodes");
    // E is the unique post-fork bottleneck:
    //   from B: B→E→F, BC[E] += 1
    //   from C: C→E→F, BC[E] += 1
    //   from A: A→{B,C}→E→F — E is after the fork, it's on ALL paths to F.
    //           σ(A,F) via E = 2, σ(A,F,E) = 2 → contribution 2/2 = 1.
    //   Total BC[E] = 1 + 1 + 1 = 3.
    let e = find_bc(&vecs, &bc, total, CTR_VEC_E).unwrap();
    assert_eq!(e, 3, "E: BC=3 (post-fork bottleneck for all paths to F)");
    assert_eq!(vecs[0], CTR_VEC_E, "E (BC=3) must be first in output");
}

// ── 9. Sort order: output is descending by BC score ──────────────────────────

#[test]
fn output_sorted_descending_by_bc_score() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Reuse linear-5 topology (BC: C=4, B=D=3, A=E=0).
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xC003);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xC004);
    add_node(CTR_VEC_E, CTR_KEY_E, CTR_ID_E, 0xC005);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t9");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t9");
    add_edge(CTR_ID_C, CTR_ID_D, "ctr.cd.t9");
    add_edge(CTR_ID_D, CTR_ID_E, "ctr.de.t9");
    let (_, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 5);
    for i in 0..total.saturating_sub(1) {
        assert!(
            bc[i] >= bc[i + 1],
            "output[{}]={} must be ≥ output[{}]={} (descending order)",
            i, bc[i], i + 1, bc[i + 1]
        );
    }
}

// ── 10. Self-loop A→A: no crash, BC[A]=0 ─────────────────────────────────────

#[test]
fn self_loop_does_not_panic_and_bc_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xC001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xC002);
    add_edge(CTR_ID_A, CTR_ID_A, "ctr.aa.t10"); // self-loop
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t10"); // A → B (B is a sink)
    let (vecs, bc, total) = gos_runtime::graph_centrality::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_bc(&vecs, &bc, total, CTR_VEC_A).unwrap();
    let b = find_bc(&vecs, &bc, total, CTR_VEC_B).unwrap();
    // Self-loop A→A cannot be on any s≠A≠t shortest path (A would be both s and t).
    // A→B: A is source, B is sink — no intermediate node involved.
    assert_eq!(a, 0, "A: BC=0 (self-loop contributes no intermediary paths)");
    assert_eq!(b, 0, "B: BC=0 (sink node, never intermediary)");
}
