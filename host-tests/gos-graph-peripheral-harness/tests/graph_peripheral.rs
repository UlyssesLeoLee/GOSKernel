// gos-graph-peripheral-harness — V2.72 graph peripheral nodes API tests
//
// Verifies `gos_runtime::graph_peripheral` — nodes with eccentricity == diameter.
//
// A peripheral node v satisfies ecc[v] = diameter (max eccentricity).
// They form the boundary of the graph: farthest from at least one other node.
//
//  1. Empty graph → peripheral_count=0, node_count=0, diameter=0.
//  2. Single isolated node → diameter=0, peripheral_count=0, node_count=1.
//  3. Two-node A→B: ecc[A]=1 (diameter); A is peripheral, B (sink, ecc=0) is not.
//  4. Path A→B→C: ecc[A]=2 (diameter=2); only A is peripheral.
//  5. Directed 3-cycle A→B→C→A: all ecc=2, diameter=2; all three are peripheral.
//  6. Star-out A→{B,C,D}: ecc[A]=1 (diameter=1), sinks ecc=0; only A is peripheral.
//  7. Linear 5-chain A→B→C→D→E: ecc[A]=4 (diameter=4); only A is peripheral.
//  8. Disconnected {A→B} ∥ {C→D→E}: diameter=2, only C (ecc=2) is peripheral.
//  9. Bidirectional K3 (A↔B, B↔C, A↔C): all ecc=1, diameter=1; all 3 are peripheral.
// 10. 3-cycle A→B→C→A + isolated D: peripheral={A,B,C}, isolated D excluded.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ─────────────────────────────────────────────────────────────────

const PER_PLUGIN: PluginId   = PluginId::from_ascii("KL_PER0_00");
const PER_EXEC:   ExecutorId = ExecutorId::from_ascii("per.exec");

const PER_KEY_A: &str = "per.alpha";
const PER_KEY_B: &str = "per.beta";
const PER_KEY_C: &str = "per.gamma";
const PER_KEY_D: &str = "per.delta";
const PER_KEY_E: &str = "per.epsilon";

const PER_ID_A: NodeId = derive_node_id(PER_PLUGIN, PER_KEY_A);
const PER_ID_B: NodeId = derive_node_id(PER_PLUGIN, PER_KEY_B);
const PER_ID_C: NodeId = derive_node_id(PER_PLUGIN, PER_KEY_C);
const PER_ID_D: NodeId = derive_node_id(PER_PLUGIN, PER_KEY_D);
const PER_ID_E: NodeId = derive_node_id(PER_PLUGIN, PER_KEY_E);

// L4=48 identifies this harness namespace in the VectorAddress space.
const PER_VEC_A: VectorAddress = VectorAddress::new(48, 1, 1, 0);
const PER_VEC_B: VectorAddress = VectorAddress::new(48, 1, 2, 0);
const PER_VEC_C: VectorAddress = VectorAddress::new(48, 1, 3, 0);
const PER_VEC_D: VectorAddress = VectorAddress::new(48, 1, 4, 0);
const PER_VEC_E: VectorAddress = VectorAddress::new(48, 1, 5, 0);

const PER_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    PER_PLUGIN,
    name:         "kl-graph-peripheral-harness",
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
        executor_id:       PER_EXEC,
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
    gos_runtime::discover_plugin(PER_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(PER_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
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

fn is_peripheral(vecs: &[VectorAddress], count: usize, target: VectorAddress) -> bool {
    for i in 0..count {
        if vecs[i] == target { return true; }
    }
    false
}

// ── 1. Empty graph → peripheral_count=0, node_count=0, diameter=0 ────────────

#[test]
fn empty_graph_no_peripheral_nodes() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let (_vecs, _ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    assert_eq!(periph_count, 0, "empty graph: no peripheral nodes");
    assert_eq!(node_count,   0, "empty graph: no nodes");
    assert_eq!(diameter,     0, "empty graph: diameter=0");
}

// ── 2. Single isolated node → diameter=0, peripheral_count=0, node_count=1 ───

#[test]
fn isolated_node_not_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    let (_vecs, _ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    assert_eq!(node_count,   1, "1 live node");
    assert_eq!(diameter,     0, "isolated: diameter=0 (no reachable pairs)");
    assert_eq!(periph_count, 0, "isolated node: not peripheral (ecc=0)");
}

// ── 3. Two-node A→B: A is peripheral (ecc=1=diameter), B is not ─────────────

#[test]
fn two_node_source_is_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t3");
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    assert_eq!(node_count,   2, "2 nodes");
    assert_eq!(diameter,     1, "diameter=1 (A→B at d=1)");
    assert_eq!(periph_count, 1, "exactly 1 peripheral node");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_A), "A (ecc=1=diameter) is peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_B), "B (ecc=0, sink) is not peripheral");
    assert_eq!(ecc[0], 1, "peripheral node has ecc=diameter=1");
}

// ── 4. Path A→B→C: only A is peripheral (ecc[A]=2=diameter) ─────────────────

#[test]
fn path_abc_source_is_only_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_node(PER_VEC_C, PER_KEY_C, PER_ID_C, 0xF003);
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t4");
    add_edge(PER_ID_B, PER_ID_C, "per.bc.t4");
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    // ecc[A]=2 (reaches B@1, C@2), ecc[B]=1, ecc[C]=0; diameter=2.
    assert_eq!(node_count,   3, "3 nodes");
    assert_eq!(diameter,     2, "diameter=2 (A→B→C)");
    assert_eq!(periph_count, 1, "exactly 1 peripheral node");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_A), "A (ecc=2=diameter) is peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_B), "B (ecc=1) is not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_C), "C (ecc=0) is not peripheral");
    assert_eq!(ecc[0], 2, "peripheral ecc = diameter = 2");
}

// ── 5. Directed 3-cycle A→B→C→A: all nodes peripheral (all ecc=2=diameter) ──

#[test]
fn directed_cycle_all_nodes_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_node(PER_VEC_C, PER_KEY_C, PER_ID_C, 0xF003);
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t5");
    add_edge(PER_ID_B, PER_ID_C, "per.bc.t5");
    add_edge(PER_ID_C, PER_ID_A, "per.ca.t5");
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    // A→B (d=1), A→C (d=2), B→C (d=1), B→A (d=2), C→A (d=1), C→B (d=2).
    // ecc[A]=ecc[B]=ecc[C]=2; diameter=2; radius=2 → all nodes both centre and peripheral.
    assert_eq!(node_count,   3, "3 nodes");
    assert_eq!(diameter,     2, "diameter=2 (longest shortest path in cycle)");
    assert_eq!(periph_count, 3, "all 3 nodes are peripheral (radius==diameter)");
    for i in 0..periph_count {
        assert_eq!(ecc[i], 2, "each peripheral ecc=2");
    }
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_A), "A is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_B), "B is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_C), "C is peripheral");
}

// ── 6. Star A→{B,C,D}: only A is peripheral (diameter=1) ────────────────────

#[test]
fn star_out_center_is_only_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_node(PER_VEC_C, PER_KEY_C, PER_ID_C, 0xF003);
    add_node(PER_VEC_D, PER_KEY_D, PER_ID_D, 0xF004);
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t6");
    add_edge(PER_ID_A, PER_ID_C, "per.ac.t6");
    add_edge(PER_ID_A, PER_ID_D, "per.ad.t6");
    let (vecs, _ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    // ecc[A]=1 (reaches B,C,D each at d=1); B,C,D are sinks with ecc=0.
    assert_eq!(node_count,   4, "4 nodes");
    assert_eq!(diameter,     1, "diameter=1 (star direct edges)");
    assert_eq!(periph_count, 1, "only A is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_A), "A is peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_B), "B (sink) not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_C), "C (sink) not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_D), "D (sink) not peripheral");
}

// ── 7. Linear 5-chain A→B→C→D→E: only A is peripheral (ecc=4=diameter) ─────

#[test]
fn linear_five_chain_source_is_only_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_node(PER_VEC_C, PER_KEY_C, PER_ID_C, 0xF003);
    add_node(PER_VEC_D, PER_KEY_D, PER_ID_D, 0xF004);
    add_node(PER_VEC_E, PER_KEY_E, PER_ID_E, 0xF005);
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t7");
    add_edge(PER_ID_B, PER_ID_C, "per.bc.t7");
    add_edge(PER_ID_C, PER_ID_D, "per.cd.t7");
    add_edge(PER_ID_D, PER_ID_E, "per.de.t7");
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    // ecc[A]=4 (reaches E at d=4); ecc[B]=3, ecc[C]=2, ecc[D]=1, ecc[E]=0.
    assert_eq!(node_count,   5, "5 nodes");
    assert_eq!(diameter,     4, "diameter=4 (A to E)");
    assert_eq!(periph_count, 1, "only A is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_A), "A (ecc=4=diameter) is peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_B), "B (ecc=3) not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_C), "C (ecc=2) not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_E), "E (sink, ecc=0) not peripheral");
    assert_eq!(ecc[0], 4, "peripheral ecc = diameter = 4");
}

// ── 8. Disconnected {A→B} ∥ {C→D→E}: only C is peripheral (ecc=2=diameter) ──

#[test]
fn disconnected_only_long_component_source_is_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_node(PER_VEC_C, PER_KEY_C, PER_ID_C, 0xF003);
    add_node(PER_VEC_D, PER_KEY_D, PER_ID_D, 0xF004);
    add_node(PER_VEC_E, PER_KEY_E, PER_ID_E, 0xF005);
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t8");
    add_edge(PER_ID_C, PER_ID_D, "per.cd.t8");
    add_edge(PER_ID_D, PER_ID_E, "per.de.t8");
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    // Component 1: ecc[A]=1 (reaches B); ecc[B]=0.
    // Component 2: ecc[C]=2 (reaches D@1, E@2); ecc[D]=1; ecc[E]=0.
    // diameter = max(1, 0, 2, 1, 0) = 2; only C qualifies.
    assert_eq!(node_count,   5, "5 nodes (disconnected)");
    assert_eq!(diameter,     2, "diameter=2 (dominated by C→D→E component)");
    assert_eq!(periph_count, 1, "only C is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_C), "C (ecc=2=diameter) is peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_A), "A (ecc=1) not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_D), "D (ecc=1) not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_B), "B (sink) not peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_E), "E (sink) not peripheral");
    assert_eq!(ecc[0], 2, "peripheral ecc = 2");
}

// ── 9. Bidirectional K3 A↔B, B↔C, A↔C: all ecc=1=diameter; all 3 peripheral ─

#[test]
fn complete_bidirectional_k3_all_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_node(PER_VEC_C, PER_KEY_C, PER_ID_C, 0xF003);
    // Bidirectional: A↔B, B↔C, A↔C.
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t9");
    add_edge(PER_ID_B, PER_ID_A, "per.ba.t9");
    add_edge(PER_ID_B, PER_ID_C, "per.bc.t9");
    add_edge(PER_ID_C, PER_ID_B, "per.cb.t9");
    add_edge(PER_ID_A, PER_ID_C, "per.ac.t9");
    add_edge(PER_ID_C, PER_ID_A, "per.ca.t9");
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    // Each node reaches the other two at d=1; ecc[v]=1 for all v; diameter=1.
    assert_eq!(node_count,   3, "3 nodes");
    assert_eq!(diameter,     1, "diameter=1 (all pairs directly connected)");
    assert_eq!(periph_count, 3, "all 3 nodes peripheral (radius==diameter==1)");
    for i in 0..periph_count {
        assert_eq!(ecc[i], 1, "each peripheral ecc=1");
    }
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_A), "A is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_B), "B is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_C), "C is peripheral");
}

// ── 10. 3-cycle A→B→C→A + isolated D: {A,B,C} peripheral, D excluded ────────

#[test]
fn cycle_with_isolated_node_excludes_isolated_from_peripheral() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(PER_VEC_A, PER_KEY_A, PER_ID_A, 0xF001);
    add_node(PER_VEC_B, PER_KEY_B, PER_ID_B, 0xF002);
    add_node(PER_VEC_C, PER_KEY_C, PER_ID_C, 0xF003);
    add_node(PER_VEC_D, PER_KEY_D, PER_ID_D, 0xF004);
    // 3-cycle: A→B→C→A.
    add_edge(PER_ID_A, PER_ID_B, "per.ab.t10");
    add_edge(PER_ID_B, PER_ID_C, "per.bc.t10");
    add_edge(PER_ID_C, PER_ID_A, "per.ca.t10");
    // D is isolated — no edges.
    let (vecs, ecc, periph_count, node_count, diameter) =
        gos_runtime::graph_peripheral::<128>();
    // ecc[A]=ecc[B]=ecc[C]=2; ecc[D]=0 (isolated). diameter=2.
    // Peripheral set = {A,B,C}; D excluded (ecc=0 ≠ diameter=2).
    assert_eq!(node_count,   4, "4 live nodes (A,B,C,D)");
    assert_eq!(diameter,     2, "diameter=2 (cycle longest shortest path)");
    assert_eq!(periph_count, 3, "3 peripheral nodes (A,B,C from cycle)");
    for i in 0..periph_count {
        assert_eq!(ecc[i], 2, "each cycle node has ecc=2=diameter");
    }
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_A), "A is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_B), "B is peripheral");
    assert!(is_peripheral(&vecs, periph_count, PER_VEC_C), "C is peripheral");
    assert!(!is_peripheral(&vecs, periph_count, PER_VEC_D), "D (isolated) is not peripheral");
}
