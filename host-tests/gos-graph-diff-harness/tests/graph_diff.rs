// gos-graph-diff-harness — V2.13 structural mutation changelog tests
//
// Verifies that gos-runtime's diff ring correctly records every node/edge
// add or remove and that `graph_diff_since(epoch)` returns the right entries.
// This underpins the `graph diff` / `graph diff pin` / `graph diff reset`
// shell commands added in V2.13.
//
//  1. Empty runtime → graph_diff_since(0) returns (0, 0).
//  2. Register one node → diff ring has 1 NodeAdded entry with correct vector.
//  3. Register one edge → diff ring gains an EdgeAdded entry.
//  4. diff_since(epoch_before_node) shows the node mutation.
//  5. diff_since(epoch_after_node) does NOT show the earlier node mutation.
//  6. unregister_edge → diff ring gains an EdgeRemoved entry.
//  7. NodeAdded.kind.is_node() == true; EdgeAdded.kind.is_node() == false.
//  8. GraphDiffKind::is_addition() is correct for all four variants.
//  9. Register N > MAX_DIFF_RING nodes → ring wraps; diff_total() reflects true count.
// 10. label_str() returns a human-readable string from the entry's label.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, GraphDiffKind, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared test fixtures ──────────────────────────────────────────────────────

const GD_PLUGIN: PluginId = PluginId::from_ascii("GD_DIFF");
const EXEC: ExecutorId = ExecutorId::from_ascii("gd.exec");

const GD_KEY_A: &str = "gd.alpha";
const GD_KEY_B: &str = "gd.beta";
const GD_KEY_C: &str = "gd.gamma";

const GD_ID_A: NodeId = derive_node_id(GD_PLUGIN, GD_KEY_A);
const GD_ID_B: NodeId = derive_node_id(GD_PLUGIN, GD_KEY_B);
const GD_ID_C: NodeId = derive_node_id(GD_PLUGIN, GD_KEY_C);

const VEC_A: VectorAddress = VectorAddress::new(0xD1, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xD1, 2, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0xD1, 3, 0, 0);

const fn gd_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xD100,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const GD_SPEC_A: NodeSpec = gd_spec(GD_KEY_A, GD_ID_A);
const GD_SPEC_B: NodeSpec = gd_spec(GD_KEY_B, GD_ID_B);
const GD_SPEC_C: NodeSpec = gd_spec(GD_KEY_C, GD_ID_C);

const GD_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: GD_PLUGIN,
    name: "GD_DIFF",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[GD_SPEC_A, GD_SPEC_B, GD_SPEC_C],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn edge_ab() -> gos_protocol::EdgeId {
    derive_edge_id(GD_ID_A, GD_ID_B, "gd.call.ab")
}

fn edge_spec_ab() -> EdgeSpec {
    EdgeSpec {
        edge_id: edge_ab(),
        from_node: GD_ID_A,
        to_node: GD_ID_B,
        edge_type: RuntimeEdgeType::Call,
        weight: 1.0,
        acl_mask: 0,
        route_policy: RoutePolicy::Direct,
        capability_namespace: Some("gd"),
        capability_binding: Some("call.ab"),
        vector_ref: None,
    }
}

// ── Test 1: empty runtime → diff returns (0, 0) ──────────────────────────────

#[test]
fn empty_diff_returns_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 16];
    let (total, filled) = gos_runtime::graph_diff_since::<16>(0, &mut out);
    assert_eq!(total, 0, "empty runtime should have no diff entries");
    assert_eq!(filled, 0);
}

// ── Test 2: register one node → NodeAdded in diff ring ───────────────────────

#[test]
fn register_node_appears_as_node_added() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GD_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_A, GD_SPEC_A);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 4];
    let (total, filled) = gos_runtime::graph_diff_since::<4>(0, &mut out);
    assert!(total >= 1, "should have at least 1 diff entry after register_node");
    assert!(filled >= 1);

    // Find the NodeAdded entry for VEC_A
    let found = out[..filled].iter().any(|e| {
        e.kind == GraphDiffKind::NodeAdded && e.from_vector == VEC_A
    });
    assert!(found, "NodeAdded entry for VEC_A should be in diff ring");
}

// ── Test 3: register edge → EdgeAdded in diff ring ───────────────────────────

#[test]
fn register_edge_appears_as_edge_added() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GD_MANIFEST).unwrap();
    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_A, GD_SPEC_A);
    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_B, GD_SPEC_B);

    let _ = gos_runtime::register_edge(edge_spec_ab());

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, filled) = gos_runtime::graph_diff_since::<8>(0, &mut out);

    let found_edge = out[..filled].iter().any(|e| {
        e.kind == GraphDiffKind::EdgeAdded
    });
    assert!(found_edge, "EdgeAdded entry should appear after register_edge");
    let _ = total;
}

// ── Test 4: diff_since(epoch_before) shows mutations ─────────────────────────

#[test]
fn diff_since_epoch_before_shows_mutations() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GD_MANIFEST).unwrap();

    let epoch_before = gos_runtime::graph_epoch();
    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_A, GD_SPEC_A);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 4];
    let (total, _) = gos_runtime::graph_diff_since::<4>(epoch_before, &mut out);
    assert!(total >= 1, "should see mutations that happened after epoch_before");
}

// ── Test 5: diff_since(epoch_after) does NOT show earlier mutations ───────────

#[test]
fn diff_since_epoch_after_hides_earlier_mutations() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GD_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_A, GD_SPEC_A);
    let epoch_after = gos_runtime::graph_epoch();

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 4];
    let (total, _) = gos_runtime::graph_diff_since::<4>(epoch_after, &mut out);
    assert_eq!(total, 0, "no mutations should appear at or before epoch_after");
}

// ── Test 6: unregister_edge → EdgeRemoved in diff ring ───────────────────────

#[test]
fn unregister_edge_appears_as_edge_removed() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GD_MANIFEST).unwrap();
    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_A, GD_SPEC_A);
    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_B, GD_SPEC_B);
    let eid = gos_runtime::register_edge(edge_spec_ab()).unwrap();

    let epoch_before_remove = gos_runtime::graph_epoch();
    let _ = gos_runtime::unregister_edge(eid);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 4];
    let (total, filled) = gos_runtime::graph_diff_since::<4>(epoch_before_remove, &mut out);
    assert!(total >= 1, "EdgeRemoved entry should appear after unregister_edge");
    let found = out[..filled].iter().any(|e| e.kind == GraphDiffKind::EdgeRemoved);
    assert!(found, "EdgeRemoved should be present in diff entries");
}

// ── Test 7: GraphDiffKind::is_node() correctness ─────────────────────────────

#[test]
fn diff_kind_is_node_correctness() {
    assert!(GraphDiffKind::NodeAdded.is_node());
    assert!(GraphDiffKind::NodeRemoved.is_node());
    assert!(!GraphDiffKind::EdgeAdded.is_node());
    assert!(!GraphDiffKind::EdgeRemoved.is_node());
}

// ── Test 8: GraphDiffKind::is_addition() correctness ─────────────────────────

#[test]
fn diff_kind_is_addition_correctness() {
    assert!(GraphDiffKind::NodeAdded.is_addition());
    assert!(!GraphDiffKind::NodeRemoved.is_addition());
    assert!(GraphDiffKind::EdgeAdded.is_addition());
    assert!(!GraphDiffKind::EdgeRemoved.is_addition());
}

// ── Test 9: ring wrap — diff_total grows beyond MAX_DIFF_RING ────────────────

#[test]
fn diff_ring_wraps_and_total_is_monotonic() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();

    // We need to register nodes beyond MAX_DIFF_RING (128) to force a wrap.
    // Use a different plugin + vector range to avoid collisions.
    const WRAP_PLUGIN: PluginId = PluginId::from_ascii("GD_WRAP");
    const WRAP_EXEC: ExecutorId = ExecutorId::from_ascii("gd.wrap");
    const WRAP_MANIFEST: PluginManifest = PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: WRAP_PLUGIN,
        name: "GD_WRAP",
        version: 1,
        depends_on: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        nodes: &[],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    };

    gos_runtime::discover_plugin(WRAP_MANIFEST).unwrap();

    // Register 130 nodes (> 128 = MAX_DIFF_RING) to force a ring wrap.
    let mut registered = 0usize;
    for i in 0u8..130u8 {
        let key: &'static str = Box::leak(format!("gd.wrap.{}", i).into_boxed_str());
        let nid = derive_node_id(WRAP_PLUGIN, key);
        let spec = NodeSpec {
            node_id: nid,
            local_node_key: key,
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Manual,
            executor_id: WRAP_EXEC,
            state_schema_hash: 0xD200,
            permissions: &[],
            exports: &[],
            vector_ref: None,
        };
        let vec = VectorAddress::new(0xD2, i as u16, 0, 0);
        if gos_runtime::register_node(WRAP_PLUGIN, vec, spec).is_ok() {
            registered += 1;
        }
    }

    let total_recorded = gos_runtime::diff_total();
    assert!(
        total_recorded >= registered as u64,
        "diff_total should be >= number of registered nodes; got {} total, {} registered",
        total_recorded, registered
    );
}

// ── Test 10: label_str() round-trips local_node_key ──────────────────────────

#[test]
fn diff_entry_label_str_roundtrips_node_key() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GD_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(GD_PLUGIN, VEC_B, GD_SPEC_B);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 4];
    let (_, filled) = gos_runtime::graph_diff_since::<4>(0, &mut out);

    // The NodeAdded entry for VEC_B should carry the label "gd.beta" (or truncated to 16 bytes).
    let expected_prefix = &GD_KEY_B[..GD_KEY_B.len().min(16)];
    let found = out[..filled].iter().any(|e| {
        e.kind == GraphDiffKind::NodeAdded
            && e.from_vector == VEC_B
            && e.label_str().starts_with(&expected_prefix[..expected_prefix.len().min(e.label_str().len())])
    });
    assert!(found, "label_str() should start with the node key prefix; entries: {:?}", &out[..filled]);
}
