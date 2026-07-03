// gos-graph-center-harness — V2.73 graph center nodes API tests
//
// Verifies `gos_runtime::graph_center` — nodes with eccentricity == radius.
//
// A centre node v satisfies ecc[v] = radius (min nonzero eccentricity).
// They are the structural complement of peripheral nodes (ecc == diameter):
// centre nodes have the tightest worst-case reach to all reachable peers.
//
//  1. Empty graph → center_count=0, node_count=0, radius=0.
//  2. Single isolated node → radius=0, center_count=0, node_count=1.
//  3. Two-node A→B: ecc[A]=1=radius; A is centre, B (sink, ecc=0) is not.
//  4. Path A→B→C: ecc[A]=2, ecc[B]=1=radius, ecc[C]=0; only B is centre.
//  5. Directed 3-cycle A→B→C→A: all ecc=2=radius; all three are centre.
//  6. Star-out A→{B,C,D}: ecc[A]=1=radius, sinks ecc=0; only A is centre.
//  7. Linear 5-chain A→B→C→D→E: radius=1 (ecc[D]=1); only D is centre.
//  8. Disconnected {A→B} ∥ {C→D→E}: radius=1; A (ecc=1) and D (ecc=1) are centre.
//  9. Bidirectional K3 A↔B, B↔C, A↔C: all ecc=1=radius; all 3 are centre.
// 10. 3-cycle A→B→C→A + isolated D: radius=2; {A,B,C} are centre, D excluded.

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

const CTR_ID_A: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_A);
const CTR_ID_B: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_B);
const CTR_ID_C: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_C);
const CTR_ID_D: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_D);
const CTR_ID_E: NodeId = derive_node_id(CTR_PLUGIN, CTR_KEY_E);

// L4=49 identifies this harness namespace in the VectorAddress space.
const CTR_VEC_A: VectorAddress = VectorAddress::new(49, 1, 1, 0);
const CTR_VEC_B: VectorAddress = VectorAddress::new(49, 1, 2, 0);
const CTR_VEC_C: VectorAddress = VectorAddress::new(49, 1, 3, 0);
const CTR_VEC_D: VectorAddress = VectorAddress::new(49, 1, 4, 0);
const CTR_VEC_E: VectorAddress = VectorAddress::new(49, 1, 5, 0);

const CTR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    CTR_PLUGIN,
    name:         "kl-graph-center-harness",
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

fn is_center(vecs: &[VectorAddress], count: usize, target: VectorAddress) -> bool {
    for i in 0..count {
        if vecs[i] == target { return true; }
    }
    false
}

// ── 1. Empty graph → center_count=0, node_count=0, radius=0 ──────────────────

#[test]
fn empty_graph_no_center_nodes() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    assert_eq!(center_count, 0, "empty graph: no center nodes");
    assert_eq!(node_count,   0, "empty graph: no nodes");
    assert_eq!(radius,       0, "empty graph: radius=0");
}

// ── 2. Single isolated node → radius=0, center_count=0, node_count=1 ─────────

#[test]
fn isolated_node_not_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    let (_vecs, _ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    assert_eq!(node_count,   1, "1 live node");
    assert_eq!(radius,       0, "isolated: radius=0 (no reachable pairs)");
    assert_eq!(center_count, 0, "isolated node: not a center (ecc=0)");
}

// ── 3. Two-node A→B: A is centre (ecc=1=radius), B (sink) is not ─────────────

#[test]
fn two_node_source_is_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t3");
    let (vecs, ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    assert_eq!(node_count,   2, "2 nodes");
    assert_eq!(radius,       1, "radius=1 (A→B at d=1, min nonzero ecc)");
    assert_eq!(center_count, 1, "exactly 1 center node");
    assert!(is_center(&vecs, center_count, CTR_VEC_A), "A (ecc=1=radius) is center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_B), "B (ecc=0, sink) is not center");
    assert_eq!(ecc[0], 1, "center node has ecc=radius=1");
}

// ── 4. Path A→B→C: only B is centre (ecc[B]=1=radius) ───────────────────────

#[test]
fn path_abc_middle_is_only_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xF003);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t4");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t4");
    let (vecs, ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    // ecc[A]=2, ecc[B]=1, ecc[C]=0; radius=1 (min nonzero).
    assert_eq!(node_count,   3, "3 nodes");
    assert_eq!(radius,       1, "radius=1 (B→C at d=1)");
    assert_eq!(center_count, 1, "exactly 1 center node");
    assert!(is_center(&vecs, center_count, CTR_VEC_B), "B (ecc=1=radius) is center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_A), "A (ecc=2) is not center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_C), "C (ecc=0, sink) is not center");
    assert_eq!(ecc[0], 1, "center ecc = radius = 1");
}

// ── 5. Directed 3-cycle A→B→C→A: all nodes centre (all ecc=2=radius) ─────────

#[test]
fn directed_cycle_all_nodes_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xF003);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t5");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t5");
    add_edge(CTR_ID_C, CTR_ID_A, "ctr.ca.t5");
    let (vecs, ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    // A→B (d=1), A→C (d=2), B→C (d=1), B→A (d=2), C→A (d=1), C→B (d=2).
    // ecc[A]=ecc[B]=ecc[C]=2; radius=2; all are centre (and peripheral).
    assert_eq!(node_count,   3, "3 nodes");
    assert_eq!(radius,       2, "radius=2 (uniform cycle eccentricity)");
    assert_eq!(center_count, 3, "all 3 nodes are center (radius==diameter)");
    for i in 0..center_count {
        assert_eq!(ecc[i], 2, "each center ecc=2");
    }
    assert!(is_center(&vecs, center_count, CTR_VEC_A), "A is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_B), "B is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_C), "C is center");
}

// ── 6. Star A→{B,C,D}: only A is centre (ecc[A]=1=radius) ───────────────────

#[test]
fn star_out_hub_is_only_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xF003);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xF004);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t6");
    add_edge(CTR_ID_A, CTR_ID_C, "ctr.ac.t6");
    add_edge(CTR_ID_A, CTR_ID_D, "ctr.ad.t6");
    let (vecs, _ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    // ecc[A]=1 (reaches B,C,D each at d=1); B,C,D are sinks with ecc=0.
    assert_eq!(node_count,   4, "4 nodes");
    assert_eq!(radius,       1, "radius=1 (A reaches all at d=1)");
    assert_eq!(center_count, 1, "only A is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_A), "A is center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_B), "B (sink) not center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_C), "C (sink) not center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_D), "D (sink) not center");
}

// ── 7. Linear 5-chain A→B→C→D→E: only D is centre (ecc[D]=1=radius) ─────────

#[test]
fn linear_five_chain_penultimate_is_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xF003);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xF004);
    add_node(CTR_VEC_E, CTR_KEY_E, CTR_ID_E, 0xF005);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t7");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t7");
    add_edge(CTR_ID_C, CTR_ID_D, "ctr.cd.t7");
    add_edge(CTR_ID_D, CTR_ID_E, "ctr.de.t7");
    let (vecs, ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    // ecc[A]=4, ecc[B]=3, ecc[C]=2, ecc[D]=1, ecc[E]=0; radius=1.
    assert_eq!(node_count,   5, "5 nodes");
    assert_eq!(radius,       1, "radius=1 (D→E at d=1, min nonzero ecc)");
    assert_eq!(center_count, 1, "only D is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_D), "D (ecc=1=radius) is center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_A), "A (ecc=4) not center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_C), "C (ecc=2) not center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_E), "E (sink, ecc=0) not center");
    assert_eq!(ecc[0], 1, "center ecc = radius = 1");
}

// ── 8. Disconnected {A→B} ∥ {C→D→E}: A (ecc=1) and D (ecc=1) are centre ─────

#[test]
fn disconnected_two_centers_from_both_components() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xF003);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xF004);
    add_node(CTR_VEC_E, CTR_KEY_E, CTR_ID_E, 0xF005);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t8");
    add_edge(CTR_ID_C, CTR_ID_D, "ctr.cd.t8");
    add_edge(CTR_ID_D, CTR_ID_E, "ctr.de.t8");
    let (vecs, _ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    // Component 1: ecc[A]=1, ecc[B]=0.
    // Component 2: ecc[C]=2, ecc[D]=1, ecc[E]=0.
    // radius = min(1, 2, 1) = 1; centre nodes: A and D.
    assert_eq!(node_count,   5, "5 nodes (disconnected)");
    assert_eq!(radius,       1, "radius=1 (min nonzero ecc across components)");
    assert_eq!(center_count, 2, "2 center nodes: A and D");
    assert!(is_center(&vecs, center_count, CTR_VEC_A), "A (ecc=1=radius) is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_D), "D (ecc=1=radius) is center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_C), "C (ecc=2) not center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_B), "B (sink) not center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_E), "E (sink) not center");
}

// ── 9. Bidirectional K3 A↔B, B↔C, A↔C: all ecc=1=radius; all 3 centre ───────

#[test]
fn complete_bidirectional_k3_all_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xF003);
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t9");
    add_edge(CTR_ID_B, CTR_ID_A, "ctr.ba.t9");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t9");
    add_edge(CTR_ID_C, CTR_ID_B, "ctr.cb.t9");
    add_edge(CTR_ID_A, CTR_ID_C, "ctr.ac.t9");
    add_edge(CTR_ID_C, CTR_ID_A, "ctr.ca.t9");
    let (vecs, ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    // Each node reaches the other two at d=1; ecc[v]=1 for all v; radius=1.
    assert_eq!(node_count,   3, "3 nodes");
    assert_eq!(radius,       1, "radius=1 (all pairs directly connected)");
    assert_eq!(center_count, 3, "all 3 nodes are center (radius==diameter==1)");
    for i in 0..center_count {
        assert_eq!(ecc[i], 1, "each center ecc=1");
    }
    assert!(is_center(&vecs, center_count, CTR_VEC_A), "A is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_B), "B is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_C), "C is center");
}

// ── 10. 3-cycle A→B→C→A + isolated D: {A,B,C} centre, D excluded ────────────

#[test]
fn cycle_with_isolated_node_excludes_isolated_from_center() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CTR_VEC_A, CTR_KEY_A, CTR_ID_A, 0xF001);
    add_node(CTR_VEC_B, CTR_KEY_B, CTR_ID_B, 0xF002);
    add_node(CTR_VEC_C, CTR_KEY_C, CTR_ID_C, 0xF003);
    add_node(CTR_VEC_D, CTR_KEY_D, CTR_ID_D, 0xF004);
    // 3-cycle: A→B→C→A.
    add_edge(CTR_ID_A, CTR_ID_B, "ctr.ab.t10");
    add_edge(CTR_ID_B, CTR_ID_C, "ctr.bc.t10");
    add_edge(CTR_ID_C, CTR_ID_A, "ctr.ca.t10");
    // D is isolated — no edges.
    let (vecs, ecc, center_count, node_count, radius) =
        gos_runtime::graph_center::<128>();
    // ecc[A]=ecc[B]=ecc[C]=2; ecc[D]=0 (isolated). radius=2 (min nonzero=2).
    // Centre set = {A,B,C}; D excluded (ecc=0 < radius=2).
    assert_eq!(node_count,   4, "4 live nodes (A,B,C,D)");
    assert_eq!(radius,       2, "radius=2 (cycle min eccentricity)");
    assert_eq!(center_count, 3, "3 center nodes (A,B,C from cycle)");
    for i in 0..center_count {
        assert_eq!(ecc[i], 2, "each cycle node has ecc=2=radius");
    }
    assert!(is_center(&vecs, center_count, CTR_VEC_A), "A is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_B), "B is center");
    assert!(is_center(&vecs, center_count, CTR_VEC_C), "C is center");
    assert!(!is_center(&vecs, center_count, CTR_VEC_D), "D (isolated) is not center");
}
