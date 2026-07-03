// gos-node-checkpoint-harness — V2.51 node checkpoint observability
//
// Verifies `gos_runtime::node_checkpoint` — snapshot a live node's state into
// the structural diff ring as a `GraphDiffKind::NodeCheckpoint` entry without
// modifying the node or bumping the graph epoch.
//
// Algorithm summary:
//   1. Look up the node by VectorAddress.
//   2. Read its NodeProcSummary (signal_count, lifecycle, edge_out_count, key).
//   3. Push a GraphDiffKind::NodeCheckpoint entry into the diff ring using the
//      node key as the label.  Epoch is captured but NOT advanced.
//   4. Return the summary for immediate shell display.
//
// Return value: Result<NodeProcSummary, RuntimeError>
//   Ok(s) — s.signal_count, s.lifecycle, s.edge_out_count captured at checkpoint time
//   Err   — NodeNotFound (vector not registered)
//
// Key invariants:
//   Unknown vector            → Err (NodeNotFound)
//   Known node                → Ok; diff ring grows by 1
//   Node signal_count         → unchanged after checkpoint
//   Node lifecycle            → unchanged after checkpoint
//   Diff ring entry kind      → GraphDiffKind::NodeCheckpoint
//   Diff ring entry from_vec  → equals the checkpointed node's vector
//   Diff ring entry label     → node's local_node_key (truncated to 16 bytes)
//   Graph epoch               → NOT bumped by checkpoint
//   Multiple checkpoints      → diff ring grows by one each call
//
// Test matrix:
//  1.  Unknown vector (empty graph)          → Err(NodeNotFound)
//  2.  Unknown vector (graph has other nodes)→ Err(NodeNotFound)
//  3.  Known node, basic Ok return           → signal_count=0 returned
//  4.  Diff ring fill increases by 1         → diff_ring_fill goes 0→1
//  5.  Graph epoch unchanged after checkpoint→ epoch same before/after
//  6.  Diff entry kind == NodeCheckpoint     → GraphDiffKind::NodeCheckpoint
//  7.  Diff entry from_vector == node vector → vector matches registered vec
//  8.  Diff entry label == node key          → label truncated to 16 bytes
//  9.  Two checkpoints → ring fill increases by 2
// 10.  Node with edges — edge_out_count correct in returned summary

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, GraphDiffEntry, GraphDiffKind, NodeId, NodeSpec,
    PluginId, PluginManifest, RoutePolicy, RuntimeEdgeType, RuntimeNodeType,
    VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const CP_PLUGIN: PluginId   = PluginId::from_ascii("KL_CKPT_00");
const CP_EXEC:   ExecutorId = ExecutorId::from_ascii("ckpt.exec0");

const CP_KEY_A: &str = "cp.alpha";
const CP_KEY_B: &str = "cp.beta";

const CP_ID_A: NodeId = derive_node_id(CP_PLUGIN, CP_KEY_A);
const CP_ID_B: NodeId = derive_node_id(CP_PLUGIN, CP_KEY_B);

const CP_VEC_A: VectorAddress = VectorAddress::new(28, 1, 1, 0);
const CP_VEC_B: VectorAddress = VectorAddress::new(28, 1, 2, 0);
const CP_VEC_X: VectorAddress = VectorAddress::new(28, 9, 9, 9); // never registered

const CP_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    CP_PLUGIN,
    name:         "kl-node-checkpoint-harness",
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
        executor_id:       CP_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(CP_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(CP_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

fn add_edge(from: NodeId, to: NodeId, key: &'static str) {
    gos_runtime::register_edge(EdgeSpec {
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
    }).unwrap();
}

fn last_diff_entry() -> Option<GraphDiffEntry> {
    let fill = gos_runtime::diff_ring_fill();
    if fill == 0 { return None; }
    let mut entries = [GraphDiffEntry::EMPTY; 128];
    let (_, filled) = gos_runtime::graph_diff_since::<128>(0, &mut entries);
    entries.iter().take(filled).rev().copied().next()
}

// ── 1. Empty graph: unknown vector → Err ─────────────────────────────────────

#[test]
fn empty_graph_unknown_vec_returns_err() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    assert!(gos_runtime::node_checkpoint(CP_VEC_X).is_err(), "empty graph: must return Err");
}

// ── 2. Graph with nodes: unknown vector → Err ─────────────────────────────────

#[test]
fn graph_with_nodes_unknown_vec_returns_err() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);
    assert!(gos_runtime::node_checkpoint(CP_VEC_X).is_err(), "unknown vec: must return Err");
}

// ── 3. Known node → Ok with correct signal_count ─────────────────────────────

#[test]
fn known_node_returns_ok_summary() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);

    let s = gos_runtime::node_checkpoint(CP_VEC_A).expect("checkpoint must succeed");
    assert_eq!(s.signal_count, 0, "fresh node: signal_count=0");
    assert_eq!(s.vector, CP_VEC_A, "returned vector matches");
}

// ── 4. Diff ring fill increases by 1 ─────────────────────────────────────────

#[test]
fn checkpoint_increases_diff_ring_fill_by_one() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);

    // After register_node there is 1 diff entry (NodeAdded).
    let fill_before = gos_runtime::diff_ring_fill();
    gos_runtime::node_checkpoint(CP_VEC_A).unwrap();
    let fill_after = gos_runtime::diff_ring_fill();

    assert_eq!(fill_after, fill_before + 1, "checkpoint adds exactly 1 diff entry");
}

// ── 5. Graph epoch unchanged after checkpoint ─────────────────────────────────

#[test]
fn checkpoint_does_not_bump_epoch() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);

    let epoch_before = gos_runtime::graph_epoch();
    gos_runtime::node_checkpoint(CP_VEC_A).unwrap();
    let epoch_after = gos_runtime::graph_epoch();

    assert_eq!(epoch_after, epoch_before, "checkpoint must not advance graph epoch");
}

// ── 6. Diff entry kind == NodeCheckpoint ─────────────────────────────────────

#[test]
fn diff_entry_kind_is_node_checkpoint() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);

    gos_runtime::node_checkpoint(CP_VEC_A).unwrap();

    let entry = last_diff_entry().expect("diff entry must exist");
    assert_eq!(entry.kind, GraphDiffKind::NodeCheckpoint, "kind must be NodeCheckpoint");
}

// ── 7. Diff entry from_vector == node vector ──────────────────────────────────

#[test]
fn diff_entry_from_vector_matches_node() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);

    gos_runtime::node_checkpoint(CP_VEC_A).unwrap();

    let entry = last_diff_entry().expect("diff entry must exist");
    assert_eq!(entry.from_vector, CP_VEC_A, "from_vector must be checkpointed node's vector");
}

// ── 8. Diff entry label == node key ──────────────────────────────────────────

#[test]
fn diff_entry_label_matches_node_key() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);

    gos_runtime::node_checkpoint(CP_VEC_A).unwrap();

    let entry = last_diff_entry().expect("diff entry must exist");
    // Label is zero-padded 16-byte ASCII; label_str() strips trailing zeros.
    assert_eq!(entry.label_str(), CP_KEY_A, "label must match node key");
}

// ── 9. Two checkpoints → ring fill grows by 2 ────────────────────────────────

#[test]
fn two_checkpoints_grow_fill_by_two() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);
    add_node(CP_VEC_B, CP_KEY_B, CP_ID_B, 0xC002);

    let fill_before = gos_runtime::diff_ring_fill();
    gos_runtime::node_checkpoint(CP_VEC_A).unwrap();
    gos_runtime::node_checkpoint(CP_VEC_B).unwrap();
    let fill_after = gos_runtime::diff_ring_fill();

    assert_eq!(fill_after, fill_before + 2, "two checkpoints: fill grows by 2");
}

// ── 10. Node with edges — edge_out_count in returned summary ─────────────────

#[test]
fn checkpoint_returns_correct_edge_out_count() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(CP_VEC_A, CP_KEY_A, CP_ID_A, 0xC001);
    add_node(CP_VEC_B, CP_KEY_B, CP_ID_B, 0xC002);
    add_edge(CP_ID_A, CP_ID_B, "cp.ab.t10");

    let s = gos_runtime::node_checkpoint(CP_VEC_A).expect("checkpoint must succeed");
    assert_eq!(s.edge_out_count, 1, "A has 1 outbound edge");

    let s_b = gos_runtime::node_checkpoint(CP_VEC_B).expect("checkpoint B must succeed");
    assert_eq!(s_b.edge_out_count, 0, "B has 0 outbound edges");
}
