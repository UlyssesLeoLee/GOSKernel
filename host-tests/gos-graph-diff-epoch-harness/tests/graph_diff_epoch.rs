// gos-graph-diff-epoch-harness — V2.16 epoch-addressed diff query tests
//
// Verifies that graph_diff_since(N) correctly filters topology mutations by epoch N,
// underpinning the new `graph diff <N>` shell command added in V2.16.
//
//  1. diff_since(0) after N mutations returns all N entries.
//  2. diff_since(current_epoch) immediately after mutations returns 0 entries.
//  3. diff_since(mid_epoch) shows only mutations after that epoch.
//  4. Epoch boundary is exclusive: mutation at epoch E not shown by diff_since(E).
//  5. diff_since(u64::MAX) always returns 0 — no future mutations exist.
//  6. Mixed node+edge events: both visible from epoch 0.
//  7. diff_since(epoch_before_edge) shows the edge addition.
//  8. diff_since(epoch_after_node) shows only the edge, not the earlier node add.
//  9. When total > PAGE, filled is capped at PAGE, total reflects true count.
// 10. After unregister_edge, diff_since(pin) shows EdgeRemoved entry.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, GraphDiffKind, NodeId, NodeSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const DE_PLUGIN: PluginId = PluginId::from_ascii("DE_EPOCH");
const EXEC: ExecutorId = ExecutorId::from_ascii("de.exec");

const DE_KEY_A: &str = "de.alpha";
const DE_KEY_B: &str = "de.beta";
const DE_KEY_C: &str = "de.gamma";

const DE_ID_A: NodeId = derive_node_id(DE_PLUGIN, DE_KEY_A);
const DE_ID_B: NodeId = derive_node_id(DE_PLUGIN, DE_KEY_B);
const DE_ID_C: NodeId = derive_node_id(DE_PLUGIN, DE_KEY_C);

const VEC_A: VectorAddress = VectorAddress::new(0xDE, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xDE, 2, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0xDE, 3, 0, 0);

const fn de_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xDE00,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const DE_SPEC_A: NodeSpec = de_spec(DE_KEY_A, DE_ID_A);
const DE_SPEC_B: NodeSpec = de_spec(DE_KEY_B, DE_ID_B);
const DE_SPEC_C: NodeSpec = de_spec(DE_KEY_C, DE_ID_C);

const DE_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: DE_PLUGIN,
    name: "DE_EPOCH",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[DE_SPEC_A, DE_SPEC_B, DE_SPEC_C],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn edge_ab() -> gos_protocol::EdgeId {
    derive_edge_id(DE_ID_A, DE_ID_B, "de.call.ab")
}

fn edge_spec_ab() -> EdgeSpec {
    EdgeSpec {
        edge_id: edge_ab(),
        from_node: DE_ID_A,
        to_node: DE_ID_B,
        edge_type: RuntimeEdgeType::Call,
        weight: 1.0,
        acl_mask: 0,
        route_policy: RoutePolicy::Direct,
        capability_namespace: Some("de"),
        capability_binding: Some("call.ab"),
        vector_ref: None,
    }
}

// ── Test 1: diff_since(0) returns all mutations ───────────────────────────────

#[test]
fn diff_since_zero_returns_all_mutations() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_B, DE_SPEC_B);
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_C, DE_SPEC_C);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 16];
    let (total, filled) = gos_runtime::graph_diff_since::<16>(0, &mut out);

    assert!(total >= 3, "diff_since(0) total should be >= 3; got {}", total);
    assert!(filled >= 3, "diff_since(0) filled should be >= 3; got {}", filled);
}

// ── Test 2: diff_since(current_epoch) returns nothing ────────────────────────

#[test]
fn diff_since_current_epoch_returns_nothing() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);

    let current = gos_runtime::graph_epoch();
    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, _) = gos_runtime::graph_diff_since::<8>(current, &mut out);

    assert_eq!(total, 0, "diff_since(current_epoch) should return 0; got {}", total);
}

// ── Test 3: diff_since(mid_epoch) shows only later mutations ─────────────────

#[test]
fn diff_since_mid_epoch_shows_only_later_mutations() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);
    let mid = gos_runtime::graph_epoch();
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_B, DE_SPEC_B);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, filled) = gos_runtime::graph_diff_since::<8>(mid, &mut out);

    assert!(total >= 1, "diff_since(mid) should see VEC_B mutation; total={}", total);
    let has_a = out[..filled].iter().any(|e| e.from_vector == VEC_A);
    assert!(!has_a, "diff_since(mid) must NOT include VEC_A (registered before mid epoch)");
    let has_b = out[..filled].iter().any(|e| e.from_vector == VEC_B);
    assert!(has_b, "diff_since(mid) must include VEC_B (registered after mid epoch)");
}

// ── Test 4: epoch boundary is exclusive ──────────────────────────────────────

#[test]
fn diff_since_epoch_boundary_is_exclusive() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();

    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);
    let after_a = gos_runtime::graph_epoch();

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, _) = gos_runtime::graph_diff_since::<8>(after_a, &mut out);
    assert_eq!(total, 0, "diff_since(epoch_after_node) should return 0 — boundary is exclusive");
}

// ── Test 5: diff_since(u64::MAX) always returns nothing ──────────────────────

#[test]
fn diff_since_max_epoch_returns_nothing() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, _) = gos_runtime::graph_diff_since::<8>(u64::MAX, &mut out);
    assert_eq!(total, 0, "diff_since(u64::MAX) should never match any recorded epoch");
}

// ── Test 6: mixed node+edge events visible from epoch 0 ──────────────────────

#[test]
fn diff_since_zero_shows_mixed_node_and_edge_events() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_B, DE_SPEC_B);
    let _ = gos_runtime::register_edge(edge_spec_ab());

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 16];
    let (total, filled) = gos_runtime::graph_diff_since::<16>(0, &mut out);

    assert!(total >= 3, "should have at least node_a + node_b + edge_ab entries; total={}", total);
    let has_node = out[..filled].iter().any(|e| e.kind == GraphDiffKind::NodeAdded);
    let has_edge = out[..filled].iter().any(|e| e.kind == GraphDiffKind::EdgeAdded);
    assert!(has_node, "NodeAdded should be visible from epoch 0");
    assert!(has_edge, "EdgeAdded should be visible from epoch 0");
}

// ── Test 7: diff_since(epoch_before_edge) shows the edge addition ─────────────

#[test]
fn diff_since_epoch_before_edge_shows_edge_added() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_B, DE_SPEC_B);
    let epoch_before_edge = gos_runtime::graph_epoch();
    let _ = gos_runtime::register_edge(edge_spec_ab());

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, filled) = gos_runtime::graph_diff_since::<8>(epoch_before_edge, &mut out);

    assert!(total >= 1, "diff_since(epoch_before_edge) should include the edge addition");
    let has_edge = out[..filled].iter().any(|e| e.kind == GraphDiffKind::EdgeAdded);
    assert!(has_edge, "EdgeAdded should appear after pinning before edge registration");
}

// ── Test 8: diff_since(epoch_after_node) shows only edge, not earlier node ───

#[test]
fn diff_since_after_node_shows_edge_not_node() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_B, DE_SPEC_B);
    let epoch_after_nodes = gos_runtime::graph_epoch();
    let _ = gos_runtime::register_edge(edge_spec_ab());

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, filled) = gos_runtime::graph_diff_since::<8>(epoch_after_nodes, &mut out);

    assert!(total >= 1, "diff_since(epoch_after_nodes) should include EdgeAdded");

    let has_node_a = out[..filled].iter().any(|e| {
        e.kind == GraphDiffKind::NodeAdded && e.from_vector == VEC_A
    });
    let has_node_b = out[..filled].iter().any(|e| {
        e.kind == GraphDiffKind::NodeAdded && e.from_vector == VEC_B
    });
    assert!(!has_node_a, "VEC_A (registered before pin) must not appear in diff_since");
    assert!(!has_node_b, "VEC_B (registered before pin) must not appear in diff_since");

    let has_edge = out[..filled].iter().any(|e| e.kind == GraphDiffKind::EdgeAdded);
    assert!(has_edge, "EdgeAdded should be the only entry visible after the node pin");
}

// ── Test 9: when total > PAGE, filled is capped at PAGE ──────────────────────

#[test]
fn diff_since_fills_capped_at_page_size() {
    const PAGE: usize = 4;
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();

    // Register 8 nodes (more than PAGE=4) to ensure total > PAGE.
    const CAP_PLUGIN: PluginId = PluginId::from_ascii("DE_CAP");
    const CAP_EXEC: ExecutorId = ExecutorId::from_ascii("de.cap");
    const CAP_MANIFEST: PluginManifest = PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: CAP_PLUGIN,
        name: "DE_CAP",
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
    gos_runtime::discover_plugin(CAP_MANIFEST).unwrap();
    for i in 0u8..8 {
        let key: &'static str = Box::leak(format!("de.cap.{}", i).into_boxed_str());
        let nid = derive_node_id(CAP_PLUGIN, key);
        let spec = NodeSpec {
            node_id: nid,
            local_node_key: key,
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Manual,
            executor_id: CAP_EXEC,
            state_schema_hash: 0xDE10,
            permissions: &[],
            exports: &[],
            vector_ref: None,
        };
        let vec = VectorAddress::new(0xDE, 0x10 + i as u16, 0, 0);
        let _ = gos_runtime::register_node(CAP_PLUGIN, vec, spec);
    }

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; PAGE];
    let (total, filled) = gos_runtime::graph_diff_since::<PAGE>(0, &mut out);

    assert!(total >= 8, "total should reflect all 8 registrations; got {}", total);
    assert_eq!(filled, PAGE, "filled must be capped at PAGE={}; got {}", PAGE, filled);
}

// ── Test 10: unregister_edge → EdgeRemoved visible from pinned epoch ──────────

#[test]
fn diff_since_pin_shows_edge_removed() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(DE_MANIFEST).unwrap();
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_A, DE_SPEC_A);
    let _ = gos_runtime::register_node(DE_PLUGIN, VEC_B, DE_SPEC_B);
    let eid = gos_runtime::register_edge(edge_spec_ab()).unwrap();

    let pin_epoch = gos_runtime::graph_epoch();
    let _ = gos_runtime::unregister_edge(eid);

    let mut out = [gos_protocol::GraphDiffEntry::EMPTY; 8];
    let (total, filled) = gos_runtime::graph_diff_since::<8>(pin_epoch, &mut out);

    assert!(total >= 1, "diff_since(pin_epoch) should see EdgeRemoved; total={}", total);
    let has_removed = out[..filled].iter().any(|e| e.kind == GraphDiffKind::EdgeRemoved);
    assert!(has_removed, "EdgeRemoved must appear after unregister_edge when using epoch pin");
}
