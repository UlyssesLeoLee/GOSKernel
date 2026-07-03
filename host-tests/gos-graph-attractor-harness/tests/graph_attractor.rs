// gos-graph-attractor-harness — V2.54 attractor-set classification tests
//
// Verifies `gos_runtime::graph_attractor` — Kosaraju SCC + condensation DAG
// classification of every live node into one of three roles:
//   0 = attractor  — member of a bottom SCC (no condensation out-edges)
//   1 = drain      — SCC has a direct condensation edge to an attractor SCC
//   2 = transient  — SCC has out-edges, but none lead directly to an attractor
//
// An attractor is a "trap" — once signal/execution flow enters it, it can
// never escape.  Every finite directed graph has at least one attractor SCC.
//
// VectorAddress namespace: L4=31 (graph-attractor harness).
//
// Test matrix:
//  1.  Empty graph → total=0, attractor_count=0.
//  2.  Single isolated node → total=1, attractor_count=1 (trivial bottom SCC).
//  3.  Two-node path A→B → B=attractor, A=drain; attractor_count=1.
//  4.  Three-node path A→B→C → C=attractor, B=drain, A=transient; attractor_count=1.
//  5.  Bidirectional pair A↔B → single SCC {A,B}=attractor; attractor_count=2.
//  6.  Cycle A→B→A + external C→A → {A,B}=attractor, C=drain; attractor_count=2.
//  7.  Diamond A→{B,C}→D → D=attractor, B/C=drain, A=transient; attractor_count=1.
//  8.  Two disconnected cycles {A,B} + {C,D} → all four=attractor; attractor_count=4.
//  9.  Sort order: role=0 nodes before role=1 before role=2 in output array.
// 10.  Self-loop A→A + isolated B → both trivial attractor SCCs; attractor_count=2.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const ATR_PLUGIN: PluginId   = PluginId::from_ascii("KL_ATR_000");
const ATR_EXEC:   ExecutorId = ExecutorId::from_ascii("atr.exec00");

const ATR_KEY_A: &str = "atr.alpha";
const ATR_KEY_B: &str = "atr.beta";
const ATR_KEY_C: &str = "atr.gamma";
const ATR_KEY_D: &str = "atr.delta";

const ATR_ID_A: NodeId = derive_node_id(ATR_PLUGIN, ATR_KEY_A);
const ATR_ID_B: NodeId = derive_node_id(ATR_PLUGIN, ATR_KEY_B);
const ATR_ID_C: NodeId = derive_node_id(ATR_PLUGIN, ATR_KEY_C);
const ATR_ID_D: NodeId = derive_node_id(ATR_PLUGIN, ATR_KEY_D);

const ATR_VEC_A: VectorAddress = VectorAddress::new(31, 1, 1, 0);
const ATR_VEC_B: VectorAddress = VectorAddress::new(31, 1, 2, 0);
const ATR_VEC_C: VectorAddress = VectorAddress::new(31, 1, 3, 0);
const ATR_VEC_D: VectorAddress = VectorAddress::new(31, 1, 4, 0);

const ATR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    ATR_PLUGIN,
    name:         "kl-graph-attractor-harness",
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
        executor_id:       ATR_EXEC,
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
    gos_runtime::discover_plugin(ATR_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(ATR_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn find_role(
    vecs: &[VectorAddress],
    roles: &[u8],
    total: usize,
    target: VectorAddress,
) -> Option<u8> {
    for i in 0..total {
        if vecs[i] == target { return Some(roles[i]); }
    }
    None
}

// ── 1. Empty graph → total=0, attractor_count=0 ──────────────────────────────

#[test]
fn empty_graph_total_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 0, "empty graph: total=0");
    assert_eq!(attractor_count, 0, "empty graph: attractor_count=0");
}

// ── 2. Single isolated node → attractor ───────────────────────────────────────

#[test]
fn isolated_node_is_attractor() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 1, "1 node");
    assert_eq!(attractor_count, 1, "isolated node is trivial bottom SCC → attractor");
    let role = find_role(&vecs, &roles, total, ATR_VEC_A).expect("A must be in result");
    assert_eq!(role, 0, "A: role=0 (attractor)");
}

// ── 3. Two-node path A→B → B=attractor, A=drain ──────────────────────────────

#[test]
fn two_node_path_sink_attractor_source_drain() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    add_edge(ATR_ID_A, ATR_ID_B, "atr.ab.t3");
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 2, "2 nodes");
    assert_eq!(attractor_count, 1, "only B is an attractor");
    let role_a = find_role(&vecs, &roles, total, ATR_VEC_A).expect("A must be in result");
    let role_b = find_role(&vecs, &roles, total, ATR_VEC_B).expect("B must be in result");
    assert_eq!(role_b, 0, "B: role=0 (attractor — no outgoing edges)");
    assert_eq!(role_a, 1, "A: role=1 (drain — direct edge to attractor B)");
}

// ── 4. Path A→B→C → C=attractor, B=drain, A=transient ───────────────────────

#[test]
fn three_node_path_attractor_drain_transient() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    add_node(ATR_VEC_C, ATR_KEY_C, ATR_ID_C, 0xA003);
    add_edge(ATR_ID_A, ATR_ID_B, "atr.ab.t4");
    add_edge(ATR_ID_B, ATR_ID_C, "atr.bc.t4");
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(attractor_count, 1, "only C is an attractor");
    let role_a = find_role(&vecs, &roles, total, ATR_VEC_A).expect("A must be in result");
    let role_b = find_role(&vecs, &roles, total, ATR_VEC_B).expect("B must be in result");
    let role_c = find_role(&vecs, &roles, total, ATR_VEC_C).expect("C must be in result");
    assert_eq!(role_c, 0, "C: role=0 (attractor — no outgoing edges)");
    assert_eq!(role_b, 1, "B: role=1 (drain — direct condensation edge to attractor C)");
    assert_eq!(role_a, 2, "A: role=2 (transient — edge only to B which is drain, not attractor)");
}

// ── 5. Bidirectional pair A↔B → single SCC, both attractor ───────────────────

#[test]
fn bidirectional_pair_both_attractor() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    add_edge(ATR_ID_A, ATR_ID_B, "atr.ab.t5");
    add_edge(ATR_ID_B, ATR_ID_A, "atr.ba.t5");
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 2, "2 nodes");
    // {A,B} form one SCC; both internal edges → no condensation out-edges → attractor.
    assert_eq!(attractor_count, 2, "{{A,B}} SCC is a bottom SCC → both are attractors");
    let role_a = find_role(&vecs, &roles, total, ATR_VEC_A).expect("A");
    let role_b = find_role(&vecs, &roles, total, ATR_VEC_B).expect("B");
    assert_eq!(role_a, 0, "A: role=0 (attractor)");
    assert_eq!(role_b, 0, "B: role=0 (attractor)");
}

// ── 6. Cycle A→B→A + external C→A → {A,B}=attractor, C=drain ────────────────

#[test]
fn cycle_with_external_node_is_drain() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    add_node(ATR_VEC_C, ATR_KEY_C, ATR_ID_C, 0xA003);
    add_edge(ATR_ID_A, ATR_ID_B, "atr.ab.t6");
    add_edge(ATR_ID_B, ATR_ID_A, "atr.ba.t6");
    add_edge(ATR_ID_C, ATR_ID_A, "atr.ca.t6");
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 3, "3 nodes");
    assert_eq!(attractor_count, 2, "A and B in attractor SCC");
    let role_a = find_role(&vecs, &roles, total, ATR_VEC_A).expect("A");
    let role_b = find_role(&vecs, &roles, total, ATR_VEC_B).expect("B");
    let role_c = find_role(&vecs, &roles, total, ATR_VEC_C).expect("C");
    assert_eq!(role_a, 0, "A: role=0 (attractor)");
    assert_eq!(role_b, 0, "B: role=0 (attractor)");
    assert_eq!(role_c, 1, "C: role=1 (drain — direct condensation edge to attractor {{A,B}})");
}

// ── 7. Diamond A→{B,C}→D → D=attractor, B/C=drain, A=transient ──────────────

#[test]
fn diamond_attractor_drain_transient() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    add_node(ATR_VEC_C, ATR_KEY_C, ATR_ID_C, 0xA003);
    add_node(ATR_VEC_D, ATR_KEY_D, ATR_ID_D, 0xA004);
    // A fans out to B and C; both B and C feed into D.
    add_edge(ATR_ID_A, ATR_ID_B, "atr.ab.t7");
    add_edge(ATR_ID_A, ATR_ID_C, "atr.ac.t7");
    add_edge(ATR_ID_B, ATR_ID_D, "atr.bd.t7");
    add_edge(ATR_ID_C, ATR_ID_D, "atr.cd.t7");
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert_eq!(attractor_count, 1, "only D is an attractor");
    let role_d = find_role(&vecs, &roles, total, ATR_VEC_D).expect("D");
    let role_b = find_role(&vecs, &roles, total, ATR_VEC_B).expect("B");
    let role_c = find_role(&vecs, &roles, total, ATR_VEC_C).expect("C");
    let role_a = find_role(&vecs, &roles, total, ATR_VEC_A).expect("A");
    assert_eq!(role_d, 0, "D: role=0 (attractor — no outgoing edges)");
    assert_eq!(role_b, 1, "B: role=1 (drain — direct edge to attractor D)");
    assert_eq!(role_c, 1, "C: role=1 (drain — direct edge to attractor D)");
    assert_eq!(role_a, 2, "A: role=2 (transient — edges only to drains B/C, not directly to attractor)");
}

// ── 8. Two disconnected cycles → all four nodes are attractors ───────────────

#[test]
fn two_disconnected_cycles_all_attractor() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    add_node(ATR_VEC_C, ATR_KEY_C, ATR_ID_C, 0xA003);
    add_node(ATR_VEC_D, ATR_KEY_D, ATR_ID_D, 0xA004);
    // First cycle: A↔B.
    add_edge(ATR_ID_A, ATR_ID_B, "atr.ab.t8");
    add_edge(ATR_ID_B, ATR_ID_A, "atr.ba.t8");
    // Second cycle: C↔D (disconnected from first).
    add_edge(ATR_ID_C, ATR_ID_D, "atr.cd.t8");
    add_edge(ATR_ID_D, ATR_ID_C, "atr.dc.t8");
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 4, "4 nodes");
    // Both SCCs are bottom SCCs — no edges between them.
    assert_eq!(attractor_count, 4, "all 4 nodes are in attractor SCCs");
    for vec in [ATR_VEC_A, ATR_VEC_B, ATR_VEC_C, ATR_VEC_D] {
        let role = find_role(&vecs, &roles, total, vec)
            .unwrap_or_else(|| panic!("{:?} must be in result", vec));
        assert_eq!(role, 0, "{:?}: role=0 (attractor)", vec);
    }
}

// ── 9. Sort order: role=0 before role=1 before role=2 ────────────────────────

#[test]
fn output_sorted_role_ascending() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Use 4-node path A→B→C→D so we get attractor=D, drain=C, transient=B, transient=A.
    // Actually A→B→C→D: D=attractor, C=drain, B=transient (B→C which is drain), A=transient (A→B transient).
    // Wait: B has edge to C (drain). Is B a drain or transient?
    // B's SCC: outgoing condensation edge to C's SCC. C's SCC: outgoing to D (attractor).
    // scc_has_out[B's SCC] = true (has out-edge to C)
    // scc_adj_attract[B's SCC]: does B have a direct condensation edge to an attractor SCC? B→C (not attractor) → no.
    // So B is transient. Similarly A→B → A's SCC has out-edge to B, scc_adj_attract[A's SCC]? A→B (transient, not attractor) → no. A is transient.
    // C→D: D is attractor. scc_adj_attract[C's SCC] = true. C is drain.
    // So: D=attractor(0), C=drain(1), A=transient(2), B=transient(2).
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    add_node(ATR_VEC_C, ATR_KEY_C, ATR_ID_C, 0xA003);
    add_node(ATR_VEC_D, ATR_KEY_D, ATR_ID_D, 0xA004);
    add_edge(ATR_ID_A, ATR_ID_B, "atr.ab.t9");
    add_edge(ATR_ID_B, ATR_ID_C, "atr.bc.t9");
    add_edge(ATR_ID_C, ATR_ID_D, "atr.cd.t9");
    let (_vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 4, "4 nodes");
    assert_eq!(attractor_count, 1, "D is the only attractor");
    // Verify sort order: roles[0] ≤ roles[1] ≤ ... ≤ roles[total-1].
    for i in 1..total {
        assert!(
            roles[i - 1] <= roles[i],
            "sort order violated: roles[{}]={} > roles[{}]={}",
            i - 1, roles[i - 1], i, roles[i]
        );
    }
    // Verify D (role=0) appears before C (role=1).
    let role_c = find_role(&_vecs, &roles, total, ATR_VEC_C).unwrap();
    let role_d = find_role(&_vecs, &roles, total, ATR_VEC_D).unwrap();
    assert_eq!(role_d, 0, "D: attractor");
    assert_eq!(role_c, 1, "C: drain");
    // Find positions.
    let pos_d = (0..total).find(|&i| _vecs[i] == ATR_VEC_D).unwrap();
    let pos_c = (0..total).find(|&i| _vecs[i] == ATR_VEC_C).unwrap();
    assert!(pos_d < pos_c, "attractor D (pos={}) must appear before drain C (pos={})", pos_d, pos_c);
}

// ── 10. Self-loop A→A + isolated B → both trivial attractor SCCs ─────────────

#[test]
fn self_loop_and_isolated_both_attractor() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(ATR_VEC_A, ATR_KEY_A, ATR_ID_A, 0xA001);
    add_node(ATR_VEC_B, ATR_KEY_B, ATR_ID_B, 0xA002);
    // Self-loop on A: A→A (skipped in DFS — slot==slot — so still trivial SCC).
    add_edge(ATR_ID_A, ATR_ID_A, "atr.aa.t10");
    // B is isolated (no edges).
    let (vecs, roles, total, attractor_count) = gos_runtime::graph_attractor::<128>();
    assert_eq!(total, 2, "2 nodes");
    // Self-loop does NOT create a condensation out-edge (from_slot==to_slot skipped).
    // Both {A} and {B} are trivial SCCs with no condensation out-edges → both attractors.
    assert_eq!(attractor_count, 2, "self-loop node and isolated node are both trivial attractors");
    let role_a = find_role(&vecs, &roles, total, ATR_VEC_A).expect("A");
    let role_b = find_role(&vecs, &roles, total, ATR_VEC_B).expect("B");
    assert_eq!(role_a, 0, "A: role=0 (attractor — self-loop creates no condensation out-edge)");
    assert_eq!(role_b, 0, "B: role=0 (attractor — isolated)");
}
