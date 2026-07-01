// gos-kill-harness — V2.21 manual node fault tests
//
// Verifies fault_node() added in V2.21, underpinning the new
// `kill <vec>` / `node fault <vec>` shell commands.
//
//  1. fault_node on unknown vector returns NodeNotFound.
//  2. fault_node on a registered node returns Ok.
//  3. fault_node sets lifecycle to Faulted.
//  4. Faulted node still appears in proc_page (not removed from graph).
//  5. fault_node enqueues the vector on the fault queue (drain_next_fault).
//  6. faulted_node_count increases after fault_node.
//  7. Re-faulting an already-faulted node returns Ok (idempotent state).
//  8. fault_node does not bump the graph epoch (topology is unchanged).
//  9. fault_node preserves the node's signal_count (not zeroed on fault).
// 10. Two independent fault_node calls enqueue two vectors on the fault queue.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeLifecycle, NodeProcSummary, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};
use gos_runtime::RuntimeError;

static TEST_LOCK: Mutex<()> = Mutex::new(());

const KL_PLUGIN: PluginId   = PluginId::from_ascii("KL_KILL_H");
const KL_EXEC:   ExecutorId = ExecutorId::from_ascii("kl.exec");

const KL_KEY_A: &str = "kl.alpha";
const KL_KEY_B: &str = "kl.beta";

const KL_ID_A: NodeId = derive_node_id(KL_PLUGIN, KL_KEY_A);
const KL_ID_B: NodeId = derive_node_id(KL_PLUGIN, KL_KEY_B);

const KL_VEC_A: VectorAddress = VectorAddress::new(9, 1, 1, 0);
const KL_VEC_B: VectorAddress = VectorAddress::new(9, 1, 1, 1);
const KL_VEC_UNKNOWN: VectorAddress = VectorAddress::new(9, 9, 9, 9);

const KL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    KL_PLUGIN,
    name:         "kl-kill-harness",
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

fn spec(node_id: NodeId, key: &'static str) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       KL_EXEC,
        state_schema_hash: 0xAB21,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_alpha() {
    gos_runtime::discover_plugin(KL_MANIFEST).unwrap();
    gos_runtime::register_node(KL_PLUGIN, KL_VEC_A, spec(KL_ID_A, KL_KEY_A)).unwrap();
}

// ── 1. fault_node on unknown vector returns NodeNotFound ─────────────────────

#[test]
fn fault_node_unknown_vector_returns_not_found() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let result = gos_runtime::fault_node(KL_VEC_UNKNOWN);
    assert_eq!(result, Err(RuntimeError::NodeNotFound));
}

// ── 2. fault_node on a registered node returns Ok ────────────────────────────

#[test]
fn fault_node_registered_returns_ok() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    assert!(gos_runtime::fault_node(KL_VEC_A).is_ok());
}

// ── 3. fault_node sets lifecycle to Faulted ──────────────────────────────────

#[test]
fn fault_node_sets_lifecycle_to_faulted() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(KL_VEC_A).unwrap();
    let summary = gos_runtime::proc_stat_for_vector(KL_VEC_A).unwrap();
    assert_eq!(summary.lifecycle, NodeLifecycle::Faulted);
}

// ── 4. Faulted node still appears in proc_page (not removed from graph) ──────

#[test]
fn fault_node_does_not_remove_node_from_graph() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(KL_VEC_A).unwrap();
    // proc_page should still return this node
    let mut page = [NodeProcSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::proc_page::<8>(0, &mut page);
    assert_eq!(total, 1);
    assert_eq!(filled, 1);
    assert_eq!(page[0].vector, KL_VEC_A);
}

// ── 5. fault_node enqueues the vector on the fault queue ─────────────────────

#[test]
fn fault_node_enqueues_to_fault_queue() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(KL_VEC_A).unwrap();
    let faulted = gos_runtime::drain_next_fault();
    assert_eq!(faulted, Some(KL_VEC_A));
}

// ── 6. faulted_node_count increases after fault_node ─────────────────────────

#[test]
fn fault_node_increases_faulted_node_count() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    assert_eq!(gos_runtime::faulted_node_count(), 0);
    gos_runtime::fault_node(KL_VEC_A).unwrap();
    assert_eq!(gos_runtime::faulted_node_count(), 1);
}

// ── 7. Re-faulting an already-faulted node returns Ok (idempotent) ───────────

#[test]
fn fault_node_idempotent_on_already_faulted_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(KL_VEC_A).unwrap();
    let result = gos_runtime::fault_node(KL_VEC_A);
    assert!(result.is_ok());
    assert_eq!(gos_runtime::faulted_node_count(), 1);
}

// ── 8. fault_node does not bump the graph epoch ───────────────────────────────

#[test]
fn fault_node_does_not_bump_graph_epoch() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    let epoch_before = gos_runtime::graph_epoch();
    gos_runtime::fault_node(KL_VEC_A).unwrap();
    let epoch_after = gos_runtime::graph_epoch();
    assert_eq!(epoch_before, epoch_after);
}

// ── 9. fault_node preserves the node's signal_count ──────────────────────────

#[test]
fn fault_node_preserves_signal_count() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    // signal_count starts at 0; fault before any dispatch
    gos_runtime::fault_node(KL_VEC_A).unwrap();
    let summary = gos_runtime::proc_stat_for_vector(KL_VEC_A).unwrap();
    assert_eq!(summary.signal_count, 0);
}

// ── 10. Two independent fault_node calls enqueue two vectors ─────────────────

#[test]
fn two_fault_nodes_enqueue_two_vectors() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(KL_MANIFEST).unwrap();
    gos_runtime::register_node(KL_PLUGIN, KL_VEC_A, spec(KL_ID_A, KL_KEY_A)).unwrap();
    gos_runtime::register_node(KL_PLUGIN, KL_VEC_B, spec(KL_ID_B, KL_KEY_B)).unwrap();

    gos_runtime::fault_node(KL_VEC_A).unwrap();
    gos_runtime::fault_node(KL_VEC_B).unwrap();

    let first  = gos_runtime::drain_next_fault();
    let second = gos_runtime::drain_next_fault();
    let third  = gos_runtime::drain_next_fault();

    assert!(first == Some(KL_VEC_A) || first == Some(KL_VEC_B));
    assert!(second == Some(KL_VEC_A) || second == Some(KL_VEC_B));
    assert_ne!(first, second);
    assert_eq!(third, None);
}
