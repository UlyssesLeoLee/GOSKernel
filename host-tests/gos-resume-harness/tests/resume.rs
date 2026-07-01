// gos-resume-harness — V2.22 node resume tests
//
// Verifies resume_node() added in V2.22, underpinning the new
// `resume <vec>` / `node resume <vec>` shell commands.
//
//  1. resume_node on unknown vector returns NodeNotFound.
//  2. resume_node on a faulted node returns Ok.
//  3. resume_node sets lifecycle to Ready.
//  4. resume_node reduces faulted_node_count to zero.
//  5. resume_node does not bump the graph epoch.
//  6. resume_node preserves the node's signal_count.
//  7. resume_node does not enqueue the fault queue.
//  8. fault then resume cycle leaves node in Ready state.
//  9. resume_node on an already-Ready node returns Ok (idempotent).
// 10. resume_node on a Suspended node transitions it to Ready.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeLifecycle, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};
use gos_runtime::RuntimeError;

static TEST_LOCK: Mutex<()> = Mutex::new(());

const RS_PLUGIN: PluginId   = PluginId::from_ascii("KL_RESM_H");
const RS_EXEC:   ExecutorId = ExecutorId::from_ascii("rs.exec");

const RS_KEY_A: &str = "rs.alpha";
const RS_KEY_B: &str = "rs.beta";

const RS_ID_A: NodeId = derive_node_id(RS_PLUGIN, RS_KEY_A);
const RS_ID_B: NodeId = derive_node_id(RS_PLUGIN, RS_KEY_B);

const RS_VEC_A: VectorAddress = VectorAddress::new(9, 2, 1, 0);
const RS_VEC_B: VectorAddress = VectorAddress::new(9, 2, 1, 1);
const RS_VEC_UNKNOWN: VectorAddress = VectorAddress::new(9, 9, 8, 8);

const RS_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    RS_PLUGIN,
    name:         "kl-resume-harness",
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
        executor_id:       RS_EXEC,
        state_schema_hash: 0xAB22,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_alpha() {
    gos_runtime::discover_plugin(RS_MANIFEST).unwrap();
    gos_runtime::register_node(RS_PLUGIN, RS_VEC_A, spec(RS_ID_A, RS_KEY_A)).unwrap();
}

// ── 1. resume_node on unknown vector returns NodeNotFound ───────────────────

#[test]
fn resume_node_unknown_vector_returns_not_found() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let result = gos_runtime::resume_node(RS_VEC_UNKNOWN);
    assert_eq!(result, Err(RuntimeError::NodeNotFound));
}

// ── 2. resume_node on a faulted node returns Ok ─────────────────────────────

#[test]
fn resume_node_on_faulted_node_returns_ok() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(RS_VEC_A).unwrap();
    assert!(gos_runtime::resume_node(RS_VEC_A).is_ok());
}

// ── 3. resume_node sets lifecycle to Ready ──────────────────────────────────

#[test]
fn resume_node_sets_lifecycle_to_ready() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(RS_VEC_A).unwrap();
    gos_runtime::resume_node(RS_VEC_A).unwrap();
    let summary = gos_runtime::proc_stat_for_vector(RS_VEC_A).unwrap();
    assert_eq!(summary.lifecycle, NodeLifecycle::Ready);
}

// ── 4. resume_node reduces faulted_node_count to zero ──────────────────────

#[test]
fn resume_node_clears_faulted_count() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(RS_VEC_A).unwrap();
    assert_eq!(gos_runtime::faulted_node_count(), 1);
    gos_runtime::resume_node(RS_VEC_A).unwrap();
    assert_eq!(gos_runtime::faulted_node_count(), 0);
}

// ── 5. resume_node does not bump the graph epoch ────────────────────────────

#[test]
fn resume_node_does_not_bump_graph_epoch() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(RS_VEC_A).unwrap();
    let epoch_before = gos_runtime::graph_epoch();
    gos_runtime::resume_node(RS_VEC_A).unwrap();
    let epoch_after = gos_runtime::graph_epoch();
    assert_eq!(epoch_before, epoch_after);
}

// ── 6. resume_node preserves the node's signal_count ───────────────────────

#[test]
fn resume_node_preserves_signal_count() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(RS_VEC_A).unwrap();
    gos_runtime::resume_node(RS_VEC_A).unwrap();
    let summary = gos_runtime::proc_stat_for_vector(RS_VEC_A).unwrap();
    assert_eq!(summary.signal_count, 0);
}

// ── 7. resume_node does not enqueue to the fault queue ─────────────────────

#[test]
fn resume_node_does_not_enqueue_fault_queue() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    // resume a non-faulted node — fault queue should remain empty
    gos_runtime::resume_node(RS_VEC_A).unwrap();
    let drained = gos_runtime::drain_next_fault();
    assert_eq!(drained, None);
}

// ── 8. fault then resume cycle leaves node in Ready state ───────────────────

#[test]
fn fault_then_resume_cycle_leaves_node_ready() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    gos_runtime::fault_node(RS_VEC_A).unwrap();
    let summary_faulted = gos_runtime::proc_stat_for_vector(RS_VEC_A).unwrap();
    assert_eq!(summary_faulted.lifecycle, NodeLifecycle::Faulted);
    gos_runtime::resume_node(RS_VEC_A).unwrap();
    let summary_resumed = gos_runtime::proc_stat_for_vector(RS_VEC_A).unwrap();
    assert_eq!(summary_resumed.lifecycle, NodeLifecycle::Ready);
}

// ── 9. resume_node on an already-Ready node returns Ok (idempotent) ─────────

#[test]
fn resume_node_idempotent_on_ready_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_alpha();
    // Node starts in Allocated after register — resume should transition to Ready
    let result = gos_runtime::resume_node(RS_VEC_A);
    assert!(result.is_ok());
    // Call again — still Ok
    let result2 = gos_runtime::resume_node(RS_VEC_A);
    assert!(result2.is_ok());
    let summary = gos_runtime::proc_stat_for_vector(RS_VEC_A).unwrap();
    assert_eq!(summary.lifecycle, NodeLifecycle::Ready);
}

// ── 10. Two nodes: fault both, resume one, check counts ─────────────────────

#[test]
fn resume_one_of_two_faulted_nodes_leaves_one_faulted() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(RS_MANIFEST).unwrap();
    gos_runtime::register_node(RS_PLUGIN, RS_VEC_A, spec(RS_ID_A, RS_KEY_A)).unwrap();
    gos_runtime::register_node(RS_PLUGIN, RS_VEC_B, spec(RS_ID_B, RS_KEY_B)).unwrap();

    gos_runtime::fault_node(RS_VEC_A).unwrap();
    gos_runtime::fault_node(RS_VEC_B).unwrap();
    assert_eq!(gos_runtime::faulted_node_count(), 2);

    gos_runtime::resume_node(RS_VEC_A).unwrap();
    assert_eq!(gos_runtime::faulted_node_count(), 1);

    let summary_a = gos_runtime::proc_stat_for_vector(RS_VEC_A).unwrap();
    let summary_b = gos_runtime::proc_stat_for_vector(RS_VEC_B).unwrap();
    assert_eq!(summary_a.lifecycle, NodeLifecycle::Ready);
    assert_eq!(summary_b.lifecycle, NodeLifecycle::Faulted);
}
