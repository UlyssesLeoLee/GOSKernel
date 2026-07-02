// gos-edge-inspect-harness — V2.12 edge enumeration introspection tests
//
// Verifies that gos-runtime's edge enumeration APIs (`edge_page`, `edge_page_for_node`,
// `register_edge`, `unregister_edge`) correctly support the `edges`/`edges count`/
// `edges <type>` shell commands added in V2.12:
//
//  1. Empty runtime → edge_page returns (0, 0).
//  2. Register one edge → edge_page returns (1, 1) with correct from/to vectors.
//  3. Edge type is preserved through register/read roundtrip.
//  4. Multiple edges are all returned across paged calls.
//  5. edge_page offset beyond total → returned = 0.
//  6. unregister_edge removes the edge; subsequent edge_page total decrements.
//  7. edge_page_for_node filters to a specific node's incident edges.
//  8. edge_page_for_node returns both outbound edges from a node.
//  9. register_edge is idempotent: second call with same EdgeId returns same id.
// 10. edge_page returns edges of mixed types; all types round-trip correctly.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, GraphEdgeSummary, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
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

const VEC_A: VectorAddress = VectorAddress::new(0xB0, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xB0, 2, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0xB0, 3, 0, 0);

const fn ei_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xBC00,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const EI_SPEC_A: NodeSpec = ei_spec(EI_KEY_A, EI_ID_A);
const EI_SPEC_B: NodeSpec = ei_spec(EI_KEY_B, EI_ID_B);
const EI_SPEC_C: NodeSpec = ei_spec(EI_KEY_C, EI_ID_C);

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

// Stable edge ids derived from node ids + key strings.
fn edge_ab_call() -> gos_protocol::EdgeId {
    derive_edge_id(EI_ID_A, EI_ID_B, "ei.call.ab")
}

fn edge_bc_mount() -> gos_protocol::EdgeId {
    derive_edge_id(EI_ID_B, EI_ID_C, "ei.mount.bc")
}

fn edge_ac_use() -> gos_protocol::EdgeId {
    derive_edge_id(EI_ID_A, EI_ID_C, "ei.use.ac")
}

fn spec_ab_call() -> EdgeSpec {
    EdgeSpec {
        edge_id: edge_ab_call(),
        from_node: EI_ID_A,
        to_node: EI_ID_B,
        edge_type: RuntimeEdgeType::Call,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

fn spec_bc_mount() -> EdgeSpec {
    EdgeSpec {
        edge_id: edge_bc_mount(),
        from_node: EI_ID_B,
        to_node: EI_ID_C,
        edge_type: RuntimeEdgeType::Mount,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: Some("ei"),
        capability_binding: Some("resource"),
        vector_ref: None,
    }
}

fn spec_ac_use() -> EdgeSpec {
    EdgeSpec {
        edge_id: edge_ac_use(),
        from_node: EI_ID_A,
        to_node: EI_ID_C,
        edge_type: RuntimeEdgeType::Use,
        weight: 0.8,
        acl_mask: u64::MAX,
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
    gos_runtime::register_node(EI_PLUGIN, VEC_A, EI_SPEC_A).ok();
    gos_runtime::register_node(EI_PLUGIN, VEC_B, EI_SPEC_B).ok();
    gos_runtime::register_node(EI_PLUGIN, VEC_C, EI_SPEC_C).ok();
}

// ── Test 1: empty runtime → edge_page returns (0, 0) ─────────────────────────

#[test]
fn empty_runtime_edge_page_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 0, "fresh runtime with no edges should have total = 0");
    assert_eq!(returned, 0, "no edges returned from empty runtime");
}

// ── Test 2: register one edge → edge_page returns (1, 1) with correct vectors ─

#[test]
fn single_edge_returned_by_edge_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_edge(spec_ab_call()).unwrap();

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 1, "one edge registered → total = 1");
    assert_eq!(returned, 1, "page at offset 0 should return 1 edge");
    assert_eq!(out[0].from_vector, VEC_A, "from_vector must be VEC_A");
    assert_eq!(out[0].to_vector, VEC_B, "to_vector must be VEC_B");
}

// ── Test 3: edge type round-trips correctly through register/read ─────────────

#[test]
fn edge_type_roundtrips_through_register_and_read() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_edge(spec_ab_call()).unwrap();

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (_, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(returned, 1);
    assert_eq!(
        out[0].edge_type,
        RuntimeEdgeType::Call,
        "edge_type must survive the register→edge_page roundtrip"
    );
}

// ── Test 4: all three edges returned across paged calls ───────────────────────

#[test]
fn all_registered_edges_appear_in_edge_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_edge(spec_ab_call()).unwrap();
    gos_runtime::register_edge(spec_bc_mount()).unwrap();
    gos_runtime::register_edge(spec_ac_use()).unwrap();

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 3, "three edges registered → total = 3");
    assert_eq!(returned, 3, "page of 8 should return all 3 edges");
}

// ── Test 5: edge_page offset beyond total → returned = 0 ─────────────────────

#[test]
fn edge_page_offset_beyond_total_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_edge(spec_ab_call()).unwrap();
    gos_runtime::register_edge(spec_bc_mount()).unwrap();

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(2, &mut out);
    assert_eq!(total, 2, "total is still 2");
    assert_eq!(returned, 0, "offset = total → no items returned");
}

// ── Test 6: unregister_edge removes it from edge_page ────────────────────────

#[test]
fn unregister_edge_removes_from_edge_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_edge(spec_ab_call()).unwrap();
    gos_runtime::register_edge(spec_bc_mount()).unwrap();

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total_before, _) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total_before, 2, "two edges before unregister");

    gos_runtime::unregister_edge(edge_ab_call())
        .expect("unregister_edge must succeed for a registered edge");

    let (total_after, returned_after) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total_after, 1, "total must drop to 1 after unregister");
    assert_eq!(returned_after, 1, "one edge remains after unregister");
    assert_eq!(
        out[0].from_vector, VEC_B,
        "remaining edge must be the bc-mount edge"
    );
}

// ── Test 7: edge_page_for_node filters to a specific node's edges ─────────────

#[test]
fn edge_page_for_node_filters_to_node() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_edge(spec_ab_call()).unwrap();
    gos_runtime::register_edge(spec_bc_mount()).unwrap();
    gos_runtime::register_edge(spec_ac_use()).unwrap();

    // VEC_C is the `to` node for both bc-mount and ac-use, and has no outbound edges.
    // edge_page_for_node returns inbound edges for VEC_C.
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page_for_node(VEC_C, 0, &mut out)
        .expect("edge_page_for_node must succeed for a registered node");
    assert!(total >= 1, "VEC_C must have at least 1 incident edge");
    assert!(returned >= 1, "at least 1 edge returned for VEC_C");

    // All returned edges must involve VEC_C (as from or to).
    for summary in out.iter().take(returned) {
        let involves_c = summary.from_vector == VEC_C || summary.to_vector == VEC_C;
        assert!(involves_c, "edge_page_for_node must only return edges incident to VEC_C");
    }
}

// ── Test 8: edge_page_for_node outbound edges from source node ────────────────

#[test]
fn edge_page_for_node_returns_outbound_from_source() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_edge(spec_ab_call()).unwrap();
    gos_runtime::register_edge(spec_ac_use()).unwrap();

    // VEC_A has two outbound edges: ab-call and ac-use.
    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page_for_node(VEC_A, 0, &mut out)
        .expect("edge_page_for_node must succeed for VEC_A");
    assert_eq!(total, 2, "VEC_A should have 2 outbound edges");
    assert_eq!(returned, 2, "both outbound edges from VEC_A must be returned");

    for summary in out.iter().take(returned) {
        assert_eq!(
            summary.from_vector, VEC_A,
            "outbound edges from VEC_A must have from_vector = VEC_A"
        );
    }
}

// ── Test 9: register_edge is idempotent ───────────────────────────────────────

#[test]
fn register_edge_is_idempotent() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let id1 = gos_runtime::register_edge(spec_ab_call())
        .expect("first register_edge must succeed");
    let id2 = gos_runtime::register_edge(spec_ab_call())
        .expect("idempotent re-register must succeed");
    assert_eq!(id1, id2, "idempotent register_edge must return the same EdgeId");

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, _) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 1, "idempotent register must not duplicate the edge");
}

// ── Test 10: mixed edge types all round-trip correctly ────────────────────────

#[test]
fn mixed_edge_types_all_round_trip() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    gos_runtime::register_edge(spec_ab_call()).unwrap();
    gos_runtime::register_edge(spec_bc_mount()).unwrap();
    gos_runtime::register_edge(spec_ac_use()).unwrap();

    let mut out = [GraphEdgeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::edge_page::<8>(0, &mut out);
    assert_eq!(total, 3);
    assert_eq!(returned, 3);

    let types: Vec<RuntimeEdgeType> = out.iter().take(returned).map(|s| s.edge_type).collect();
    assert!(
        types.contains(&RuntimeEdgeType::Call),
        "Call edge must appear in edge_page"
    );
    assert!(
        types.contains(&RuntimeEdgeType::Mount),
        "Mount edge must appear in edge_page"
    );
    assert!(
        types.contains(&RuntimeEdgeType::Use),
        "Use edge must appear in edge_page"
    );
}
