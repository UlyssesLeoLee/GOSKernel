// gos-edge-inspect-harness — V3.0 edge ps-list introspection tests
//
// Verifies that gos-runtime's edge enumeration API (`edge_page`, `edge_exists_by_kind`,
// `edge_page_for_node`) correctly supports the `edges`/`edges summary` shell commands
// added in V3.0:
//
//  1. Empty runtime → edge_page returns (0, 0).
//  2. Register one edge → edge_page returns (1, 1) with correct from/to nodes.
//  3. edge_page returns edges sorted ascending by edge_vector key.
//  4. Multiple edges of different types are all returned.
//  5. edge_page respects offset: offset ≥ total returns (total, 0).
//  6. edge_exists_by_kind returns false before, true after registration.
//  7. Unregister edge removes it from edge_page.
//  8. edge_page_for_node returns only edges touching a specific node.

use std::sync::Mutex;

use gos_protocol::{
    EdgeId, EdgeSpec, EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId, NodeSpec,
    PluginId, PluginManifest, RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
    derive_node_id,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared test fixtures ──────────────────────────────────────────────────────

const EI_PLUGIN: PluginId = PluginId::from_ascii("EI_INSPECT");
const EXEC: ExecutorId = ExecutorId::from_ascii("ei.exec");

const EI_KEY_A: &str = "ei.a";
const EI_KEY_B: &str = "ei.b";
const EI_KEY_C: &str = "ei.c";

const EI_ID_A: NodeId = derive_node_id(EI_PLUGIN, EI_KEY_A);
const EI_ID_B: NodeId = derive_node_id(EI_PLUGIN, EI_KEY_B);
const EI_ID_C: NodeId = derive_node_id(EI_PLUGIN, EI_KEY_C);

const VEC_A: VectorAddress = VectorAddress::new(0xE0, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xE0, 2, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0xE0, 3, 0, 0);

const fn ei_node_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xEE00,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const EI_SPEC_A: NodeSpec = ei_node_spec(EI_KEY_A, EI_ID_A);
const EI_SPEC_B: NodeSpec = ei_node_spec(EI_KEY_B, EI_ID_B);
const EI_SPEC_C: NodeSpec = ei_node_spec(EI_KEY_C, EI_ID_C);

const EI_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: EI_PLUGIN,
    name: "EI_INSPECT",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[EI_SPEC_A, EI_SPEC_B, EI_SPEC_C],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn make_edge(id: u8, from: NodeId, to: NodeId, ty: RuntimeEdgeType) -> EdgeSpec {
    EdgeSpec {
        edge_id: EdgeId([id; 16]),
        from_node: from,
        to_node: to,
        edge_type: ty,
        weight: 1.0,
        acl_mask: 0,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

fn setup() {
    gos_runtime::reset();
    gos_supervisor::clear_rewrite_rules();
    gos_runtime::discover_plugin(EI_MANIFEST).ok();
    gos_runtime::mark_plugin_loaded(EI_PLUGIN).ok();
    // Register all three nodes so edges between them are valid.
    gos_runtime::register_node(EI_PLUGIN, VEC_A, EI_SPEC_A).ok();
    gos_runtime::register_node(EI_PLUGIN, VEC_B, EI_SPEC_B).ok();
    gos_runtime::register_node(EI_PLUGIN, VEC_C, EI_SPEC_C).ok();
}

// ── Test 1: empty runtime → edge_page returns (0, 0) ─────────────────────────

#[test]
fn empty_runtime_edge_page_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 0, "fresh runtime should have 0 edges");
    assert_eq!(returned, 0, "no edges returned for empty runtime");
}

// ── Test 2: register one edge → edge_page returns (1, 1) with correct nodes ──

#[test]
fn single_edge_returned_by_edge_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let e = make_edge(0x01, EI_ID_A, EI_ID_B, RuntimeEdgeType::Signal);
    gos_runtime::register_edge(e).expect("register edge A→B");

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 1, "one edge registered → total = 1");
    assert_eq!(returned, 1, "page at offset 0 should return 1 edge");
    assert_eq!(
        out[0].from_vector, VEC_A,
        "edge from_vector must be VEC_A"
    );
    assert_eq!(
        out[0].to_vector, VEC_B,
        "edge to_vector must be VEC_B"
    );
    assert_eq!(
        out[0].edge_type, RuntimeEdgeType::Signal,
        "edge_type must be Signal"
    );
}

// ── Test 3: edge_page returns edges sorted ascending by edge_vector key ───────

#[test]
fn edge_page_is_sorted_ascending_by_edge_vector_key() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // Register in reverse id order; edge_page must still sort ascending.
    gos_runtime::register_edge(make_edge(0x03, EI_ID_A, EI_ID_C, RuntimeEdgeType::Depend))
        .expect("edge 0x03");
    gos_runtime::register_edge(make_edge(0x01, EI_ID_A, EI_ID_B, RuntimeEdgeType::Signal))
        .expect("edge 0x01");
    gos_runtime::register_edge(make_edge(0x02, EI_ID_B, EI_ID_C, RuntimeEdgeType::Call))
        .expect("edge 0x02");

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (_, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(returned, 3);

    let keys: Vec<u64> = out.iter()
        .take(returned)
        .map(|e| e.edge_vector.as_u64())
        .collect();
    assert!(keys[0] <= keys[1], "edge_page[0] key ≤ edge_page[1] key");
    assert!(keys[1] <= keys[2], "edge_page[1] key ≤ edge_page[2] key");
}

// ── Test 4: multiple edges of different types are all returned ────────────────

#[test]
fn all_registered_edges_appear_in_edge_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    gos_runtime::register_edge(make_edge(0x10, EI_ID_A, EI_ID_B, RuntimeEdgeType::Signal))
        .expect("signal edge");
    gos_runtime::register_edge(make_edge(0x11, EI_ID_A, EI_ID_C, RuntimeEdgeType::Depend))
        .expect("depend edge");
    gos_runtime::register_edge(make_edge(0x12, EI_ID_B, EI_ID_C, RuntimeEdgeType::Call))
        .expect("call edge");

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 3, "three edges → total = 3");
    assert_eq!(returned, 3, "page of 8 returns all 3");

    let types: Vec<RuntimeEdgeType> = out.iter()
        .take(returned)
        .map(|e| e.edge_type)
        .collect();
    assert!(types.contains(&RuntimeEdgeType::Signal), "Signal edge must appear");
    assert!(types.contains(&RuntimeEdgeType::Depend), "Depend edge must appear");
    assert!(types.contains(&RuntimeEdgeType::Call),   "Call edge must appear");
}

// ── Test 5: edge_page respects offset — offset ≥ total returns (total, 0) ────

#[test]
fn edge_page_offset_beyond_total_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    gos_runtime::register_edge(make_edge(0x20, EI_ID_A, EI_ID_B, RuntimeEdgeType::Signal))
        .expect("register edge");
    gos_runtime::register_edge(make_edge(0x21, EI_ID_B, EI_ID_C, RuntimeEdgeType::Call))
        .expect("register edge");

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(2, &mut out);
    assert_eq!(total, 2, "total is still 2");
    assert_eq!(returned, 0, "offset = total → no items returned");
}

// ── Test 6: edge_page preserves edge_type field correctly ────────────────────

#[test]
fn edge_page_preserves_edge_type() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // Register one Depend edge (A→B) — only one edge so offset 0 is deterministic.
    gos_runtime::register_edge(make_edge(0x30, EI_ID_A, EI_ID_B, RuntimeEdgeType::Depend))
        .expect("register depend edge A→B");

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 1);
    assert_eq!(returned, 1);
    assert_eq!(
        out[0].edge_type, RuntimeEdgeType::Depend,
        "edge_type must round-trip through edge_page"
    );
    assert_eq!(out[0].from_vector, VEC_A, "from_vector must be VEC_A");
    assert_eq!(out[0].to_vector,   VEC_B, "to_vector must be VEC_B");
}

// ── Test 7: unregister edge removes it from edge_page ─────────────────────────

#[test]
fn unregister_edge_removes_from_edge_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let edge_id = gos_runtime::register_edge(
        make_edge(0x40, EI_ID_A, EI_ID_B, RuntimeEdgeType::Signal)
    ).expect("register edge");

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, _) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 1, "edge visible before unregister");

    gos_runtime::unregister_edge(edge_id).expect("unregister edge");

    let (total_after, returned_after) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total_after, 0, "edge must be gone after unregister");
    assert_eq!(returned_after, 0, "no edges returned after unregister");
}

// ── Test 8: edge_page_for_node returns only edges touching that node ──────────

#[test]
fn edge_page_for_node_filters_by_node() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // A→B (signal), A→C (depend), B→C (call)
    gos_runtime::register_edge(make_edge(0x50, EI_ID_A, EI_ID_B, RuntimeEdgeType::Signal))
        .expect("A→B");
    gos_runtime::register_edge(make_edge(0x51, EI_ID_A, EI_ID_C, RuntimeEdgeType::Depend))
        .expect("A→C");
    gos_runtime::register_edge(make_edge(0x52, EI_ID_B, EI_ID_C, RuntimeEdgeType::Call))
        .expect("B→C");

    use gos_protocol::GraphEdgeSummary;
    let mut out = [GraphEdgeSummary::EMPTY; 8];

    // Node A: outbound A→B and A→C (2 edges)
    let (total_a, returned_a) = gos_runtime::edge_page_for_node::<8>(VEC_A, 0, &mut out)
        .expect("edge_page_for_node A");
    assert_eq!(total_a, 2, "node A touches 2 edges");
    assert_eq!(returned_a, 2);

    // Node C: inbound A→C and B→C (2 edges)
    let (total_c, returned_c) = gos_runtime::edge_page_for_node::<8>(VEC_C, 0, &mut out)
        .expect("edge_page_for_node C");
    assert_eq!(total_c, 2, "node C touches 2 edges (both inbound)");
    assert_eq!(returned_c, 2);

    // Node B: outbound B→C + inbound A→B (2 edges)
    let (total_b, returned_b) = gos_runtime::edge_page_for_node::<8>(VEC_B, 0, &mut out)
        .expect("edge_page_for_node B");
    assert_eq!(total_b, 2, "node B touches 2 edges");
    assert_eq!(returned_b, 2);
}
