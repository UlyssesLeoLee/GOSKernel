// gos-graph-closeness-harness — V2.40 closeness centrality API tests
//
// Verifies `gos_runtime::graph_closeness` — outgoing closeness centrality on
// the live directed graph.
//
// CC[v] = r_v × 1_000_000 / Σ_{u reachable from v, u≠v} d(v,u)
//   r_v  = number of nodes reachable from v via directed edges (excl. v itself)
//   d(v,u) = BFS shortest-path distance from v to u
//   Isolated nodes (r_v = 0) → CC[v] = 0
//
// OS analogy: `ping` average RTT census — which kernel service reaches all others
// in the fewest directed hops on average?
//
//  1. Empty graph → total=0, no panics.
//  2. Single isolated node → CC=0, total=1.
//  3. Two-node edge A→B: CC[A]=1_000_000, CC[B]=0 (sink, unreachable).
//  4. Path A→B→C: CC[B]=1_000_000 (reaches C in 1 hop), CC[A]=666_666,
//     CC[C]=0 (pure sink).
//  5. Star center A→{B,C,D}: CC[A]=1_000_000, leaves = 0.
//  6. Directed cycle A→B→C→A: all nodes equal CC=666_666 by symmetry.
//  7. Diamond A→{B,C}→D: CC[B]=CC[C]=1_000_000 (reach D in 1 hop), CC[A]=750_000.
//  8. Linear 5-node chain A→B→C→D→E: CC descends D>C>B>A>E.
//  9. Disconnected pair {A→B} ∥ {C→D}: CC[A]=CC[C]=1_000_000, sinks=0.
// 10. Self-loop A→A plus isolated B: CC[A]=0 (self-loop, no other reachable).

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const CLO_PLUGIN: PluginId   = PluginId::from_ascii("KL_CLO0_00");
const CLO_EXEC:   ExecutorId = ExecutorId::from_ascii("clo.exec");

const CLO_KEY_A: &str = "clo.alpha";
const CLO_KEY_B: &str = "clo.beta";
const CLO_KEY_C: &str = "clo.gamma";
const CLO_KEY_D: &str = "clo.delta";
const CLO_KEY_E: &str = "clo.epsilon";

const CLO_ID_A: NodeId = derive_node_id(CLO_PLUGIN, CLO_KEY_A);
const CLO_ID_B: NodeId = derive_node_id(CLO_PLUGIN, CLO_KEY_B);
const CLO_ID_C: NodeId = derive_node_id(CLO_PLUGIN, CLO_KEY_C);
const CLO_ID_D: NodeId = derive_node_id(CLO_PLUGIN, CLO_KEY_D);
const CLO_ID_E: NodeId = derive_node_id(CLO_PLUGIN, CLO_KEY_E);

const CLO_VEC_A: VectorAddress = VectorAddress::new(17, 1, 1, 0);
const CLO_VEC_B: VectorAddress = VectorAddress::new(17, 1, 2, 0);
const CLO_VEC_C: VectorAddress = VectorAddress::new(17, 1, 3, 0);
const CLO_VEC_D: VectorAddress = VectorAddress::new(17, 1, 4, 0);
const CLO_VEC_E: VectorAddress = VectorAddress::new(17, 1, 5, 0);

const CLO_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    CLO_PLUGIN,
    name:         "kl-graph-closeness-harness",
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
        executor_id:       CLO_EXEC,
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
    gos_runtime::discover_plugin(CLO_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(CLO_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_cc(vecs: &[VectorAddress], cc: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(cc[i]); }
    }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_closeness_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → CC=0 ───────────────────────────────────────────

#[test]
fn isolated_node_has_zero_closeness() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 1, "1 node");
    let score = find_cc(&vecs, &cc, total, CLO_VEC_A).expect("A must be in result");
    assert_eq!(score, 0, "isolated node: CC=0 (no reachable nodes)");
}

// ── 3. Two-node A→B: CC[A]=1_000_000, CC[B]=0 ────────────────────────────────

#[test]
fn two_node_edge_source_is_max_closeness() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_edge(CLO_ID_A, CLO_ID_B, "clo.ab.t3");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    // A reaches B in 1 hop: CC[A] = 1×1_000_000/1 = 1_000_000.
    assert_eq!(a, 1_000_000, "A→B: CC[A]=1_000_000 (reaches B in 1 hop)");
    // B is a sink: CC[B]=0.
    assert_eq!(b, 0, "B: CC=0 (sink, no outgoing edges)");
    // A must appear first (highest CC).
    assert_eq!(vecs[0], CLO_VEC_A, "A (CC=1_000_000) must be first in output");
}

// ── 4. Path A→B→C: B is most central ─────────────────────────────────────────

#[test]
fn path_abc_middle_node_most_central() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_node(CLO_VEC_C, CLO_KEY_C, CLO_ID_C, 0xD003);
    add_edge(CLO_ID_A, CLO_ID_B, "clo.ab.t4");
    add_edge(CLO_ID_B, CLO_ID_C, "clo.bc.t4");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 3, "3 nodes");
    // B reaches C in 1 hop: CC[B] = 1×1_000_000/1 = 1_000_000.
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    assert_eq!(b, 1_000_000, "B: CC=1_000_000 (reaches C in 1 hop)");
    // A reaches {B,C}: sum = 1+2 = 3, CC[A] = 2×1_000_000/3 = 666_666.
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    assert_eq!(a, 666_666, "A: CC=666_666 (reaches B+C, avg dist = 1.5)");
    // C is a sink: CC[C] = 0.
    let c = find_cc(&vecs, &cc, total, CLO_VEC_C).unwrap();
    assert_eq!(c, 0, "C: CC=0 (sink, no outgoing edges)");
    assert_eq!(vecs[0], CLO_VEC_B, "B (CC=1_000_000) must be first");
}

// ── 5. Star A→{B,C,D}: A has max CC=1_000_000, leaves=0 ─────────────────────

#[test]
fn star_center_has_max_closeness() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_node(CLO_VEC_C, CLO_KEY_C, CLO_ID_C, 0xD003);
    add_node(CLO_VEC_D, CLO_KEY_D, CLO_ID_D, 0xD004);
    add_edge(CLO_ID_A, CLO_ID_B, "clo.ab.t5");
    add_edge(CLO_ID_A, CLO_ID_C, "clo.ac.t5");
    add_edge(CLO_ID_A, CLO_ID_D, "clo.ad.t5");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 4, "4 nodes");
    // A reaches {B,C,D} all in 1 hop: CC[A] = 3×1_000_000/3 = 1_000_000.
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    assert_eq!(a, 1_000_000, "star center A: CC=1_000_000");
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    let c = find_cc(&vecs, &cc, total, CLO_VEC_C).unwrap();
    let d = find_cc(&vecs, &cc, total, CLO_VEC_D).unwrap();
    assert_eq!(b, 0, "leaf B: CC=0 (sink)");
    assert_eq!(c, 0, "leaf C: CC=0 (sink)");
    assert_eq!(d, 0, "leaf D: CC=0 (sink)");
    assert_eq!(vecs[0], CLO_VEC_A, "A (CC=1_000_000) must be first");
}

// ── 6. Directed 3-cycle A→B→C→A: all equal CC=666_666 ───────────────────────

#[test]
fn directed_cycle_all_nodes_equal_closeness() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_node(CLO_VEC_C, CLO_KEY_C, CLO_ID_C, 0xD003);
    add_edge(CLO_ID_A, CLO_ID_B, "clo.ab.t6");
    add_edge(CLO_ID_B, CLO_ID_C, "clo.bc.t6");
    add_edge(CLO_ID_C, CLO_ID_A, "clo.ca.t6");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 3, "3 nodes in cycle");
    // By rotational symmetry, all have the same BFS distances.
    // BFS from A: d(A,B)=1, d(A,C)=2. sum=3, r=2. CC = 2×1e6/3 = 666_666.
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    let c = find_cc(&vecs, &cc, total, CLO_VEC_C).unwrap();
    assert_eq!(a, 666_666, "A: CC=666_666 (cycle symmetry)");
    assert_eq!(b, 666_666, "B: CC=666_666 (cycle symmetry)");
    assert_eq!(c, 666_666, "C: CC=666_666 (cycle symmetry)");
}

// ── 7. Diamond A→{B,C}→D: B and C have max CC ───────────────────────────────

#[test]
fn diamond_branches_have_higher_closeness_than_root() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_node(CLO_VEC_C, CLO_KEY_C, CLO_ID_C, 0xD003);
    add_node(CLO_VEC_D, CLO_KEY_D, CLO_ID_D, 0xD004);
    add_edge(CLO_ID_A, CLO_ID_B, "clo.ab.t7");
    add_edge(CLO_ID_A, CLO_ID_C, "clo.ac.t7");
    add_edge(CLO_ID_B, CLO_ID_D, "clo.bd.t7");
    add_edge(CLO_ID_C, CLO_ID_D, "clo.cd.t7");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 4, "4 nodes");
    // B reaches {D}: sum=1, r=1. CC[B] = 1×1e6/1 = 1_000_000.
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    let c = find_cc(&vecs, &cc, total, CLO_VEC_C).unwrap();
    assert_eq!(b, 1_000_000, "B: CC=1_000_000 (reaches D in 1 hop)");
    assert_eq!(c, 1_000_000, "C: CC=1_000_000 (reaches D in 1 hop)");
    // A reaches {B,C,D}: d(A,B)=1, d(A,C)=1, d(A,D)=2. sum=4, r=3. CC = 3×1e6/4 = 750_000.
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    assert_eq!(a, 750_000, "A: CC=750_000 (3 reachable, sum_dist=4)");
    let d = find_cc(&vecs, &cc, total, CLO_VEC_D).unwrap();
    assert_eq!(d, 0, "D: CC=0 (sink)");
    // B or C must be first (both equal max CC).
    assert!(
        vecs[0] == CLO_VEC_B || vecs[0] == CLO_VEC_C,
        "first entry must be B or C (both have max CC=1_000_000)"
    );
}

// ── 8. Linear 5-node chain A→B→C→D→E: CC descends tail→head ─────────────────

#[test]
fn linear_five_node_chain_closeness_ordering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_node(CLO_VEC_C, CLO_KEY_C, CLO_ID_C, 0xD003);
    add_node(CLO_VEC_D, CLO_KEY_D, CLO_ID_D, 0xD004);
    add_node(CLO_VEC_E, CLO_KEY_E, CLO_ID_E, 0xD005);
    add_edge(CLO_ID_A, CLO_ID_B, "clo.ab.t8");
    add_edge(CLO_ID_B, CLO_ID_C, "clo.bc.t8");
    add_edge(CLO_ID_C, CLO_ID_D, "clo.cd.t8");
    add_edge(CLO_ID_D, CLO_ID_E, "clo.de.t8");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 5, "5 nodes");
    // CC[D] = 1×1e6/1 = 1_000_000  (reaches E in 1 hop)
    // CC[C] = 2×1e6/3 = 666_666    (reaches D+E, sum=1+2=3)
    // CC[B] = 3×1e6/6 = 500_000    (reaches C+D+E, sum=1+2+3=6)
    // CC[A] = 4×1e6/10 = 400_000   (reaches B+C+D+E, sum=1+2+3+4=10)
    // CC[E] = 0                     (sink)
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    let c = find_cc(&vecs, &cc, total, CLO_VEC_C).unwrap();
    let d = find_cc(&vecs, &cc, total, CLO_VEC_D).unwrap();
    let e = find_cc(&vecs, &cc, total, CLO_VEC_E).unwrap();
    assert_eq!(d, 1_000_000, "D: CC=1_000_000");
    assert_eq!(c, 666_666,   "C: CC=666_666");
    assert_eq!(b, 500_000,   "B: CC=500_000");
    assert_eq!(a, 400_000,   "A: CC=400_000");
    assert_eq!(e, 0,         "E: CC=0 (sink)");
    // Output must be sorted descending.
    for i in 0..total.saturating_sub(1) {
        assert!(
            cc[i] >= cc[i + 1],
            "output[{}]={} must be >= output[{}]={} (descending)",
            i, cc[i], i + 1, cc[i + 1]
        );
    }
    assert_eq!(vecs[0], CLO_VEC_D, "D (CC=1_000_000) must be first");
}

// ── 9. Disconnected pairs {A→B} ∥ {C→D}: sources equal 1_000_000 ─────────────

#[test]
fn disconnected_pairs_sources_have_max_closeness() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_node(CLO_VEC_C, CLO_KEY_C, CLO_ID_C, 0xD003);
    add_node(CLO_VEC_D, CLO_KEY_D, CLO_ID_D, 0xD004);
    add_edge(CLO_ID_A, CLO_ID_B, "clo.ab.t9");
    add_edge(CLO_ID_C, CLO_ID_D, "clo.cd.t9");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 4, "4 nodes in 2 disconnected components");
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    let c = find_cc(&vecs, &cc, total, CLO_VEC_C).unwrap();
    let d = find_cc(&vecs, &cc, total, CLO_VEC_D).unwrap();
    // A reaches B in 1 hop: CC[A] = 1_000_000.
    assert_eq!(a, 1_000_000, "A: CC=1_000_000 (reaches B in 1 hop)");
    assert_eq!(b, 0,         "B: CC=0 (sink)");
    assert_eq!(c, 1_000_000, "C: CC=1_000_000 (reaches D in 1 hop)");
    assert_eq!(d, 0,         "D: CC=0 (sink)");
}

// ── 10. Self-loop A→A plus B→C: self-loop contributes no reachability ─────────

#[test]
fn self_loop_does_not_contribute_to_closeness() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CLO_VEC_A, CLO_KEY_A, CLO_ID_A, 0xD001);
    add_node(CLO_VEC_B, CLO_KEY_B, CLO_ID_B, 0xD002);
    add_node(CLO_VEC_C, CLO_KEY_C, CLO_ID_C, 0xD003);
    add_edge(CLO_ID_A, CLO_ID_A, "clo.aa.t10"); // self-loop
    add_edge(CLO_ID_B, CLO_ID_C, "clo.bc.t10");
    let (vecs, cc, total) = gos_runtime::graph_closeness::<128>();
    assert_eq!(total, 3, "3 nodes");
    // A has only a self-loop: BFS from A sees no new nodes (dist[A]=0 already).
    // r_A = 0, CC[A] = 0.
    let a = find_cc(&vecs, &cc, total, CLO_VEC_A).unwrap();
    assert_eq!(a, 0, "A: CC=0 (self-loop, no other nodes reachable)");
    // B reaches C in 1 hop: CC[B] = 1_000_000.
    let b = find_cc(&vecs, &cc, total, CLO_VEC_B).unwrap();
    assert_eq!(b, 1_000_000, "B: CC=1_000_000 (reaches C in 1 hop)");
    // C is a sink: CC[C] = 0.
    let c = find_cc(&vecs, &cc, total, CLO_VEC_C).unwrap();
    assert_eq!(c, 0, "C: CC=0 (sink)");
    assert_eq!(vecs[0], CLO_VEC_B, "B (CC=1_000_000) must be first");
}
