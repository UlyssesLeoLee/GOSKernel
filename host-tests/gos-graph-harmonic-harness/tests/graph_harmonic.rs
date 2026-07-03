// gos-graph-harmonic-harness — V2.71 harmonic centrality API tests
//
// Verifies `gos_runtime::graph_harmonic` — harmonic centrality on the
// live directed graph.
//
// HC[v] = Σ_{u≠v, d(v,u)<∞} 1_000_000/d(v,u)
//   d(v,u) = BFS shortest-path distance from v to u (directed, unweighted)
//   Isolated nodes (no reachable neighbours) → HC[v] = 0
//   Disconnected pairs (no path) contribute 0 to the sum
//   Integer arithmetic: each term is 1_000_000/d truncated
//
// Key insight: harmonic centrality rewards short-distance reachability via
// the 1/d weighting, so source nodes in long chains beat mid-chain nodes
// (unlike closeness, where the chain tail is best).
//
//  1. Empty graph → total=0, no panics.
//  2. Single isolated node → HC=0, total=1.
//  3. Two-node A→B: HC[A]=1_000_000, HC[B]=0.
//  4. Path A→B→C: HC[A]=1_500_000, HC[B]=1_000_000, HC[C]=0; A is first.
//  5. Star A→{B,C,D}: HC[A]=3_000_000, leaves=0.
//  6. Directed 3-cycle A→B→C→A: all HC=1_500_000 by symmetry.
//  7. Diamond A→{B,C}→D: HC[A]=2_500_000, HC[B]=HC[C]=1_000_000, HC[D]=0.
//  8. Linear 5-node chain A→B→C→D→E: HC descends A>B>C>D>E (source wins).
//  9. Disconnected pair {A→B} ∥ {C→D}: HC[A]=HC[C]=1_000_000, sinks=0.
// 10. Self-loop A→A plus B→C: HC[A]=0 (self-loop no new node), HC[B]=1_000_000.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const HAR_PLUGIN: PluginId   = PluginId::from_ascii("KL_HAR0_00");
const HAR_EXEC:   ExecutorId = ExecutorId::from_ascii("har.exec");

const HAR_KEY_A: &str = "har.alpha";
const HAR_KEY_B: &str = "har.beta";
const HAR_KEY_C: &str = "har.gamma";
const HAR_KEY_D: &str = "har.delta";
const HAR_KEY_E: &str = "har.epsilon";

const HAR_ID_A: NodeId = derive_node_id(HAR_PLUGIN, HAR_KEY_A);
const HAR_ID_B: NodeId = derive_node_id(HAR_PLUGIN, HAR_KEY_B);
const HAR_ID_C: NodeId = derive_node_id(HAR_PLUGIN, HAR_KEY_C);
const HAR_ID_D: NodeId = derive_node_id(HAR_PLUGIN, HAR_KEY_D);
const HAR_ID_E: NodeId = derive_node_id(HAR_PLUGIN, HAR_KEY_E);

const HAR_VEC_A: VectorAddress = VectorAddress::new(47, 1, 1, 0);
const HAR_VEC_B: VectorAddress = VectorAddress::new(47, 1, 2, 0);
const HAR_VEC_C: VectorAddress = VectorAddress::new(47, 1, 3, 0);
const HAR_VEC_D: VectorAddress = VectorAddress::new(47, 1, 4, 0);
const HAR_VEC_E: VectorAddress = VectorAddress::new(47, 1, 5, 0);

const HAR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    HAR_PLUGIN,
    name:         "kl-graph-harmonic-harness",
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
        executor_id:       HAR_EXEC,
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
    gos_runtime::discover_plugin(HAR_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(HAR_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_hc(vecs: &[VectorAddress], hc: &[u32], total: usize, target: VectorAddress) -> Option<u32> {
    for i in 0..total {
        if vecs[i] == target { return Some(hc[i]); }
    }
    None
}

// ── 1. Empty graph → total=0 ─────────────────────────────────────────────────

#[test]
fn empty_graph_harmonic_total_is_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 0, "empty graph: 0 nodes");
}

// ── 2. Single isolated node → HC=0 ───────────────────────────────────────────

#[test]
fn isolated_node_has_zero_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 1, "1 node");
    let score = find_hc(&vecs, &hc, total, HAR_VEC_A).expect("A must be in result");
    assert_eq!(score, 0, "isolated node: HC=0 (no reachable nodes)");
}

// ── 3. Two-node A→B: HC[A]=1_000_000, HC[B]=0 ────────────────────────────────

#[test]
fn two_node_edge_source_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_edge(HAR_ID_A, HAR_ID_B, "har.ab.t3");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 2, "2 nodes");
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    // A reaches B in d=1: HC[A] = 1_000_000/1 = 1_000_000.
    assert_eq!(a, 1_000_000, "A: HC=1_000_000 (reaches B at d=1)");
    assert_eq!(b, 0,         "B: HC=0 (sink, no outgoing edges)");
    assert_eq!(vecs[0], HAR_VEC_A, "A must be first");
}

// ── 4. Path A→B→C: HC[A]=1_500_000, HC[B]=1_000_000, HC[C]=0; A is first ───

#[test]
fn path_abc_source_has_highest_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_node(HAR_VEC_C, HAR_KEY_C, HAR_ID_C, 0xE003);
    add_edge(HAR_ID_A, HAR_ID_B, "har.ab.t4");
    add_edge(HAR_ID_B, HAR_ID_C, "har.bc.t4");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 3, "3 nodes");
    // HC[A] = 1e6/1 + 1e6/2 = 1_000_000 + 500_000 = 1_500_000.
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    assert_eq!(a, 1_500_000, "A: HC=1_500_000 (B at d=1, C at d=2)");
    // HC[B] = 1e6/1 = 1_000_000.
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    assert_eq!(b, 1_000_000, "B: HC=1_000_000 (C at d=1)");
    // HC[C] = 0 (sink).
    let c = find_hc(&vecs, &hc, total, HAR_VEC_C).unwrap();
    assert_eq!(c, 0, "C: HC=0 (sink)");
    // A (highest) must appear first in sorted output.
    assert_eq!(vecs[0], HAR_VEC_A, "A (HC=1_500_000) must be first");
}

// ── 5. Star A→{B,C,D}: HC[A]=3_000_000, leaves=0 ────────────────────────────

#[test]
fn star_center_has_max_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_node(HAR_VEC_C, HAR_KEY_C, HAR_ID_C, 0xE003);
    add_node(HAR_VEC_D, HAR_KEY_D, HAR_ID_D, 0xE004);
    add_edge(HAR_ID_A, HAR_ID_B, "har.ab.t5");
    add_edge(HAR_ID_A, HAR_ID_C, "har.ac.t5");
    add_edge(HAR_ID_A, HAR_ID_D, "har.ad.t5");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 4, "4 nodes");
    // A reaches B, C, D each at d=1: HC[A] = 3 × 1e6/1 = 3_000_000.
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    assert_eq!(a, 3_000_000, "star center A: HC=3_000_000");
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    let c = find_hc(&vecs, &hc, total, HAR_VEC_C).unwrap();
    let d = find_hc(&vecs, &hc, total, HAR_VEC_D).unwrap();
    assert_eq!(b, 0, "leaf B: HC=0 (sink)");
    assert_eq!(c, 0, "leaf C: HC=0 (sink)");
    assert_eq!(d, 0, "leaf D: HC=0 (sink)");
    assert_eq!(vecs[0], HAR_VEC_A, "A (HC=3_000_000) must be first");
}

// ── 6. Directed 3-cycle A→B→C→A: all HC=1_500_000 ───────────────────────────

#[test]
fn directed_cycle_all_nodes_equal_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_node(HAR_VEC_C, HAR_KEY_C, HAR_ID_C, 0xE003);
    add_edge(HAR_ID_A, HAR_ID_B, "har.ab.t6");
    add_edge(HAR_ID_B, HAR_ID_C, "har.bc.t6");
    add_edge(HAR_ID_C, HAR_ID_A, "har.ca.t6");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 3, "3 nodes in cycle");
    // From A: d(A,B)=1, d(A,C)=2 → HC[A] = 1e6/1 + 1e6/2 = 1_500_000.
    // By rotational symmetry all three equal.
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    let c = find_hc(&vecs, &hc, total, HAR_VEC_C).unwrap();
    assert_eq!(a, 1_500_000, "A: HC=1_500_000 (cycle symmetry)");
    assert_eq!(b, 1_500_000, "B: HC=1_500_000 (cycle symmetry)");
    assert_eq!(c, 1_500_000, "C: HC=1_500_000 (cycle symmetry)");
}

// ── 7. Diamond A→{B,C}→D: HC[A]=2_500_000, HC[B]=HC[C]=1_000_000 ────────────

#[test]
fn diamond_source_has_highest_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_node(HAR_VEC_C, HAR_KEY_C, HAR_ID_C, 0xE003);
    add_node(HAR_VEC_D, HAR_KEY_D, HAR_ID_D, 0xE004);
    add_edge(HAR_ID_A, HAR_ID_B, "har.ab.t7");
    add_edge(HAR_ID_A, HAR_ID_C, "har.ac.t7");
    add_edge(HAR_ID_B, HAR_ID_D, "har.bd.t7");
    add_edge(HAR_ID_C, HAR_ID_D, "har.cd.t7");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 4, "4 nodes");
    // HC[A] = 1e6/1 + 1e6/1 + 1e6/2 = 1_000_000 + 1_000_000 + 500_000 = 2_500_000.
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    assert_eq!(a, 2_500_000, "A: HC=2_500_000 (B,C at d=1; D at d=2)");
    // HC[B] = HC[C] = 1e6/1 = 1_000_000 (reach D at d=1).
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    let c = find_hc(&vecs, &hc, total, HAR_VEC_C).unwrap();
    assert_eq!(b, 1_000_000, "B: HC=1_000_000 (D at d=1)");
    assert_eq!(c, 1_000_000, "C: HC=1_000_000 (D at d=1)");
    // HC[D] = 0 (sink).
    let d = find_hc(&vecs, &hc, total, HAR_VEC_D).unwrap();
    assert_eq!(d, 0, "D: HC=0 (sink)");
    assert_eq!(vecs[0], HAR_VEC_A, "A (HC=2_500_000) must be first");
}

// ── 8. 5-chain A→B→C→D→E: HC descends A>B>C>D>E ─────────────────────────────

#[test]
fn linear_five_node_chain_harmonic_ordering() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_node(HAR_VEC_C, HAR_KEY_C, HAR_ID_C, 0xE003);
    add_node(HAR_VEC_D, HAR_KEY_D, HAR_ID_D, 0xE004);
    add_node(HAR_VEC_E, HAR_KEY_E, HAR_ID_E, 0xE005);
    add_edge(HAR_ID_A, HAR_ID_B, "har.ab.t8");
    add_edge(HAR_ID_B, HAR_ID_C, "har.bc.t8");
    add_edge(HAR_ID_C, HAR_ID_D, "har.cd.t8");
    add_edge(HAR_ID_D, HAR_ID_E, "har.de.t8");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 5, "5 nodes");
    // HC[A] = 1e6/1 + 1e6/2 + 1e6/3 + 1e6/4
    //       = 1_000_000 + 500_000 + 333_333 + 250_000 = 2_083_333.
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    assert_eq!(a, 2_083_333, "A: HC=2_083_333");
    // HC[B] = 1e6/1 + 1e6/2 + 1e6/3 = 1_833_333.
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    assert_eq!(b, 1_833_333, "B: HC=1_833_333");
    // HC[C] = 1e6/1 + 1e6/2 = 1_500_000.
    let c = find_hc(&vecs, &hc, total, HAR_VEC_C).unwrap();
    assert_eq!(c, 1_500_000, "C: HC=1_500_000");
    // HC[D] = 1e6/1 = 1_000_000.
    let d = find_hc(&vecs, &hc, total, HAR_VEC_D).unwrap();
    assert_eq!(d, 1_000_000, "D: HC=1_000_000");
    // HC[E] = 0 (sink).
    let e = find_hc(&vecs, &hc, total, HAR_VEC_E).unwrap();
    assert_eq!(e, 0, "E: HC=0 (sink)");
    // Output must be sorted descending.
    for i in 0..total.saturating_sub(1) {
        assert!(
            hc[i] >= hc[i + 1],
            "output[{}]={} must be >= output[{}]={} (descending)",
            i, hc[i], i + 1, hc[i + 1]
        );
    }
    assert_eq!(vecs[0], HAR_VEC_A, "A (HC=2_083_333) must be first");
}

// ── 9. Disconnected {A→B} ∥ {C→D}: HC[A]=HC[C]=1_000_000 ────────────────────

#[test]
fn disconnected_pairs_sources_have_equal_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_node(HAR_VEC_C, HAR_KEY_C, HAR_ID_C, 0xE003);
    add_node(HAR_VEC_D, HAR_KEY_D, HAR_ID_D, 0xE004);
    add_edge(HAR_ID_A, HAR_ID_B, "har.ab.t9");
    add_edge(HAR_ID_C, HAR_ID_D, "har.cd.t9");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 4, "4 nodes in 2 disconnected components");
    // Disconnected nodes contribute 0 (no path); only local component counts.
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    let c = find_hc(&vecs, &hc, total, HAR_VEC_C).unwrap();
    let d = find_hc(&vecs, &hc, total, HAR_VEC_D).unwrap();
    assert_eq!(a, 1_000_000, "A: HC=1_000_000 (reaches B at d=1, C/D unreachable)");
    assert_eq!(b, 0,         "B: HC=0 (sink)");
    assert_eq!(c, 1_000_000, "C: HC=1_000_000 (reaches D at d=1, A/B unreachable)");
    assert_eq!(d, 0,         "D: HC=0 (sink)");
}

// ── 10. Self-loop A→A plus B→C: self-loop contributes 0 to HC ───────────────

#[test]
fn self_loop_does_not_contribute_to_harmonic() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(HAR_VEC_A, HAR_KEY_A, HAR_ID_A, 0xE001);
    add_node(HAR_VEC_B, HAR_KEY_B, HAR_ID_B, 0xE002);
    add_node(HAR_VEC_C, HAR_KEY_C, HAR_ID_C, 0xE003);
    add_edge(HAR_ID_A, HAR_ID_A, "har.aa.t10"); // self-loop
    add_edge(HAR_ID_B, HAR_ID_C, "har.bc.t10");
    let (vecs, hc, total) = gos_runtime::graph_harmonic::<128>();
    assert_eq!(total, 3, "3 nodes");
    // A has only a self-loop: dist[A]=0, loop edge encounters A at dist=0,
    // BFS does not revisit — no u≠A reachable. HC[A]=0.
    let a = find_hc(&vecs, &hc, total, HAR_VEC_A).unwrap();
    assert_eq!(a, 0, "A: HC=0 (self-loop only, no other reachable nodes)");
    // B reaches C at d=1: HC[B] = 1_000_000.
    let b = find_hc(&vecs, &hc, total, HAR_VEC_B).unwrap();
    assert_eq!(b, 1_000_000, "B: HC=1_000_000 (C at d=1)");
    // C is a sink: HC[C] = 0.
    let c = find_hc(&vecs, &hc, total, HAR_VEC_C).unwrap();
    assert_eq!(c, 0, "C: HC=0 (sink)");
    assert_eq!(vecs[0], HAR_VEC_B, "B (HC=1_000_000) must be first");
}
