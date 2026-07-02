// gos-node-inspect-harness — V2.8 node ps-list introspection tests
//
// Verifies that gos-runtime's node enumeration API (`node_page`, `node_exists_by_id`,
// `graph_epoch`) correctly supports the `nodes`/`nodes faulted`/`nodes summary`
// shell commands added in V2.8:
//
//  1. Empty runtime → node_page returns (0, 0).
//  2. Register one node → node_page returns (1, 1) with the correct vector.
//  3. Newly registered node has NodeLifecycle::Allocated (not Ready).
//  4. Multiple nodes are all returned across paged calls.
//  5. node_page respects offset: offset ≥ total returns (total, 0).
//  6. node_exists_by_id returns false before registration, true after.
//  7. node_page returns nodes sorted ascending by vector key.
//  8. register_node is idempotent: second call returns the same NodeId.

use std::sync::Mutex;

use gos_protocol::{
    EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId, NodeLifecycle, NodeSpec,
    PluginId, PluginManifest, RuntimeNodeType, VectorAddress, derive_node_id,
    GraphNodeSummary,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared test fixtures ──────────────────────────────────────────────────────

const NI_PLUGIN: PluginId = PluginId::from_ascii("NI_INSPECT");
const EXEC:      ExecutorId = ExecutorId::from_ascii("ni.exec");

const fn ni_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xAB00,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const NI_KEY_A: &str = "ni.a";
const NI_KEY_B: &str = "ni.b";
const NI_KEY_C: &str = "ni.c";

const NI_ID_A: NodeId = derive_node_id(NI_PLUGIN, NI_KEY_A);
const NI_ID_B: NodeId = derive_node_id(NI_PLUGIN, NI_KEY_B);
const NI_ID_C: NodeId = derive_node_id(NI_PLUGIN, NI_KEY_C);

const NI_SPEC_A: NodeSpec = ni_spec(NI_KEY_A, NI_ID_A);
const NI_SPEC_B: NodeSpec = ni_spec(NI_KEY_B, NI_ID_B);
const NI_SPEC_C: NodeSpec = ni_spec(NI_KEY_C, NI_ID_C);

// Vectors chosen so A < B < C by as_u64() for deterministic sort-order tests.
const VEC_A: VectorAddress = VectorAddress::new(0xA0, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xA0, 2, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0xA0, 3, 0, 0);

const NI_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: NI_PLUGIN,
    name: "NI_INSPECT",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[NI_SPEC_A, NI_SPEC_B, NI_SPEC_C],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn setup() {
    gos_runtime::reset();
    gos_supervisor::clear_rewrite_rules();
    gos_runtime::discover_plugin(NI_MANIFEST).ok();
    gos_runtime::mark_plugin_loaded(NI_PLUGIN).ok();
}

fn register_abc() {
    gos_runtime::register_node(NI_PLUGIN, VEC_A, NI_SPEC_A).ok();
    gos_runtime::register_node(NI_PLUGIN, VEC_B, NI_SPEC_B).ok();
    gos_runtime::register_node(NI_PLUGIN, VEC_C, NI_SPEC_C).ok();
}

// ── Test 1: empty runtime → node_page returns (0, 0) ─────────────────────────

#[test]
fn empty_runtime_node_page_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::node_page::<8>(0, &mut out);
    assert_eq!(total, 0, "fresh runtime should have 0 nodes");
    assert_eq!(returned, 0, "no nodes returned for empty runtime");
}

// ── Test 2: register one node → node_page returns (1, 1) with correct vector ─

#[test]
fn single_node_returned_by_node_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(NI_PLUGIN, VEC_A, NI_SPEC_A).unwrap();

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::node_page::<8>(0, &mut out);
    assert_eq!(total, 1, "one node registered → total = 1");
    assert_eq!(returned, 1, "page request at offset 0 should return 1 node");
    assert_eq!(out[0].vector, VEC_A, "returned node must have VEC_A");
}

// ── Test 3: newly registered node is in Allocated lifecycle ──────────────────

#[test]
fn new_node_has_allocated_lifecycle() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(NI_PLUGIN, VEC_A, NI_SPEC_A).unwrap();

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (_, returned) = gos_runtime::node_page::<8>(0, &mut out);
    assert_eq!(returned, 1);
    assert_eq!(
        out[0].lifecycle, NodeLifecycle::Allocated,
        "newly registered node must be in Allocated lifecycle"
    );
}

// ── Test 4: all three nodes are returned across paged enumeration ─────────────

#[test]
fn all_registered_nodes_appear_in_node_page() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    register_abc();

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::node_page::<8>(0, &mut out);
    assert_eq!(total, 3, "three nodes registered → total = 3");
    assert_eq!(returned, 3, "page of 8 should return all 3 nodes");

    // All three vectors must appear (order determined by sorted keys).
    let vectors: [VectorAddress; 3] = [out[0].vector, out[1].vector, out[2].vector];
    assert!(vectors.contains(&VEC_A), "VEC_A must be in the page");
    assert!(vectors.contains(&VEC_B), "VEC_B must be in the page");
    assert!(vectors.contains(&VEC_C), "VEC_C must be in the page");
}

// ── Test 5: offset ≥ total → returned = 0 ────────────────────────────────────

#[test]
fn node_page_offset_beyond_total_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    register_abc();

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (total, returned) = gos_runtime::node_page::<8>(3, &mut out);
    assert_eq!(total, 3, "total is still 3");
    assert_eq!(returned, 0, "offset = total → no items returned");
}

// ── Test 6: node_exists_by_id returns false before, true after registration ───

#[test]
fn node_exists_by_id_tracks_registration() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    assert!(
        !gos_runtime::node_exists_by_id(NI_ID_A),
        "node_exists_by_id must be false before registration"
    );

    gos_runtime::register_node(NI_PLUGIN, VEC_A, NI_SPEC_A).unwrap();

    assert!(
        gos_runtime::node_exists_by_id(NI_ID_A),
        "node_exists_by_id must be true after registration"
    );
    // Sibling node not yet registered.
    assert!(
        !gos_runtime::node_exists_by_id(NI_ID_B),
        "NI_ID_B must still be absent"
    );
}

// ── Test 7: node_page returns nodes sorted ascending by vector key ────────────

#[test]
fn node_page_is_sorted_ascending_by_vector_key() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();
    // Register in reverse order to verify the sort is applied.
    gos_runtime::register_node(NI_PLUGIN, VEC_C, NI_SPEC_C).unwrap();
    gos_runtime::register_node(NI_PLUGIN, VEC_A, NI_SPEC_A).unwrap();
    gos_runtime::register_node(NI_PLUGIN, VEC_B, NI_SPEC_B).unwrap();

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (_, returned) = gos_runtime::node_page::<8>(0, &mut out);
    assert_eq!(returned, 3);

    // Ascending sort by VectorAddress::as_u64()
    let keys: [u64; 3] = [
        out[0].vector.as_u64(),
        out[1].vector.as_u64(),
        out[2].vector.as_u64(),
    ];
    assert!(keys[0] <= keys[1], "node_page[0] key must be ≤ node_page[1] key");
    assert!(keys[1] <= keys[2], "node_page[1] key must be ≤ node_page[2] key");
}

// ── Test 8: register_node is idempotent — second call returns same NodeId ─────

#[test]
fn register_node_is_idempotent() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let id1 = gos_runtime::register_node(NI_PLUGIN, VEC_A, NI_SPEC_A)
        .expect("first register must succeed");
    let id2 = gos_runtime::register_node(NI_PLUGIN, VEC_A, NI_SPEC_A)
        .expect("idempotent re-register must succeed");

    assert_eq!(id1, id2, "idempotent register must return the same NodeId");

    // Exactly one node must be present (no duplicate).
    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (total, _) = gos_runtime::node_page::<8>(0, &mut out);
    assert_eq!(total, 1, "idempotent register must not duplicate the node");
}
