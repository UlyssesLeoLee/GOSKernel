// gos-node-log-clear-harness — V2.26 per-node lifecycle log clear tests
//
// Verifies the clear_node_log() API added to gos-runtime.
// Analogous to `journalctl --vacuum-time` / `truncate -s0 /var/log/syslog`:
// discards stored lifecycle history for one node.
//
//  1. clear_node_log on unknown vector returns NodeNotFound.
//  2. After clear on a fresh node, log returns 0 total and 0 returned.
//  3. After clear, node_log_page succeeds (node still registered).
//  4. After fault then clear, log is empty (Faulted entry discarded).
//  5. After resume then clear, log is empty (Ready entry discarded).
//  6. clear is idempotent: double-clear still returns (0, 0).
//  7. After clear, new lifecycle events are logged correctly (ring is fresh).
//  8. Clearing node A does not affect node B's log.
//  9. After clear, total starts fresh from 0 (not from pre-clear total).
// 10. clear_node_log returns Ok(()) for a live node.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id,
    EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeLifecycle, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};
use gos_runtime::{RuntimeError, MAX_NODE_LOG};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const NLC_PLUGIN: PluginId   = PluginId::from_ascii("KL_NLCLR_H");
const NLC_EXEC:   ExecutorId = ExecutorId::from_ascii("nlc.exec");

const NLC_KEY_A: &str = "nlc.alpha";
const NLC_KEY_B: &str = "nlc.beta";

const NLC_ID_A: NodeId = derive_node_id(NLC_PLUGIN, NLC_KEY_A);
const NLC_ID_B: NodeId = derive_node_id(NLC_PLUGIN, NLC_KEY_B);

const NLC_VEC_A:       VectorAddress = VectorAddress::new(9, 6, 1, 0);
const NLC_VEC_B:       VectorAddress = VectorAddress::new(9, 6, 2, 0);
const NLC_VEC_UNKNOWN: VectorAddress = VectorAddress::new(9, 9, 8, 8);

const NLC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    NLC_PLUGIN,
    name:         "kl-node-log-clear-harness",
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

fn spec_a() -> NodeSpec {
    NodeSpec {
        node_id:           NLC_ID_A,
        local_node_key:    NLC_KEY_A,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NLC_EXEC,
        state_schema_hash: 0xC201,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn spec_b() -> NodeSpec {
    NodeSpec {
        node_id:           NLC_ID_B,
        local_node_key:    NLC_KEY_B,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NLC_EXEC,
        state_schema_hash: 0xC202,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_a() {
    gos_runtime::discover_plugin(NLC_MANIFEST).unwrap();
    gos_runtime::register_node(NLC_PLUGIN, NLC_VEC_A, spec_a()).unwrap();
}

fn register_ab() {
    gos_runtime::discover_plugin(NLC_MANIFEST).unwrap();
    gos_runtime::register_node(NLC_PLUGIN, NLC_VEC_A, spec_a()).unwrap();
    gos_runtime::register_node(NLC_PLUGIN, NLC_VEC_B, spec_b()).unwrap();
}

fn log_page_a() -> (usize, usize) {
    let mut buf = [gos_protocol::NodeLogEntry::EMPTY; MAX_NODE_LOG];
    gos_runtime::node_log_page(NLC_VEC_A, &mut buf).unwrap()
}

fn log_page_b() -> (usize, usize) {
    let mut buf = [gos_protocol::NodeLogEntry::EMPTY; MAX_NODE_LOG];
    gos_runtime::node_log_page(NLC_VEC_B, &mut buf).unwrap()
}

// ── 1. Unknown vector returns NodeNotFound ───────────────────────────────────

#[test]
fn clear_unknown_vector_returns_not_found() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let result = gos_runtime::clear_node_log(NLC_VEC_UNKNOWN);
    assert_eq!(result, Err(RuntimeError::NodeNotFound));
}

// ── 2. After clear on a fresh node, log is (0, 0) ────────────────────────────

#[test]
fn clear_fresh_node_gives_zero_entries() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    let (total, returned) = log_page_a();
    assert_eq!(total, 0, "total should be 0 after clear");
    assert_eq!(returned, 0, "returned should be 0 after clear");
}

// ── 3. After clear, node_log_page still succeeds (node still registered) ─────

#[test]
fn clear_does_not_unregister_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    let mut buf = [gos_protocol::NodeLogEntry::EMPTY; MAX_NODE_LOG];
    let result = gos_runtime::node_log_page(NLC_VEC_A, &mut buf);
    assert!(result.is_ok(), "node_log_page should succeed after clear — node is still registered");
}

// ── 4. After fault then clear, Faulted entry is discarded ────────────────────

#[test]
fn clear_discards_faulted_entry() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::fault_node(NLC_VEC_A).unwrap();
    // Confirm Faulted entry is present before clear.
    let mut buf = [gos_protocol::NodeLogEntry::EMPTY; MAX_NODE_LOG];
    let (_, returned) = gos_runtime::node_log_page(NLC_VEC_A, &mut buf).unwrap();
    let had_faulted = buf[..returned].iter().any(|e| e.lifecycle == NodeLifecycle::Faulted as u8);
    assert!(had_faulted, "Faulted entry should exist before clear");
    // Now clear.
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    let (total, returned) = log_page_a();
    assert_eq!(total, 0, "total should be 0 after clear");
    assert_eq!(returned, 0, "Faulted entry should be gone after clear");
}

// ── 5. After resume then clear, Ready entry is discarded ──────────────────────

#[test]
fn clear_discards_ready_entry() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::fault_node(NLC_VEC_A).unwrap();
    gos_runtime::resume_node(NLC_VEC_A).unwrap();
    // Confirm Ready entry present.
    let mut buf = [gos_protocol::NodeLogEntry::EMPTY; MAX_NODE_LOG];
    let (_, _returned) = gos_runtime::node_log_page(NLC_VEC_A, &mut buf).unwrap();
    assert_eq!(buf[0].lifecycle, NodeLifecycle::Ready as u8, "Ready should be most recent before clear");
    // Clear.
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    let (total, returned_after) = log_page_a();
    assert_eq!(total, 0, "total 0 after clear");
    assert_eq!(returned_after, 0, "Ready entry gone after clear");
}

// ── 6. clear is idempotent: double-clear still returns (0, 0) ────────────────

#[test]
fn clear_is_idempotent() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::fault_node(NLC_VEC_A).unwrap();
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap(); // second clear
    let (total, returned) = log_page_a();
    assert_eq!(total, 0);
    assert_eq!(returned, 0);
}

// ── 7. After clear, new events are logged fresh ───────────────────────────────

#[test]
fn clear_then_new_events_logged_correctly() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::fault_node(NLC_VEC_A).unwrap();
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    // Generate a new event after clear.
    gos_runtime::resume_node(NLC_VEC_A).unwrap();
    let mut buf = [gos_protocol::NodeLogEntry::EMPTY; MAX_NODE_LOG];
    let (total, returned) = gos_runtime::node_log_page(NLC_VEC_A, &mut buf).unwrap();
    assert!(total >= 1, "at least one event after post-clear resume");
    assert!(returned >= 1, "at least one entry returned");
    assert_eq!(
        buf[0].lifecycle,
        NodeLifecycle::Ready as u8,
        "first entry after clear should be Ready (from resume_node)"
    );
}

// ── 8. Clearing node A does not affect node B ────────────────────────────────

#[test]
fn clear_does_not_affect_sibling_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    // Fault both, then clear only A.
    gos_runtime::fault_node(NLC_VEC_A).unwrap();
    gos_runtime::fault_node(NLC_VEC_B).unwrap();
    let (b_total_before, _) = log_page_b();
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    let (a_total_after, a_returned) = log_page_a();
    let (b_total_after, _) = log_page_b();
    assert_eq!(a_total_after, 0, "A log cleared");
    assert_eq!(a_returned, 0, "A log cleared");
    assert_eq!(b_total_after, b_total_before, "B log unchanged after clearing A");
}

// ── 9. After clear, total starts from 0 (not cumulative from pre-clear) ───────

#[test]
fn clear_resets_total_counter_to_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    // Generate some events.
    gos_runtime::fault_node(NLC_VEC_A).unwrap();
    gos_runtime::resume_node(NLC_VEC_A).unwrap();
    let (pre_clear_total, _) = log_page_a();
    assert!(pre_clear_total >= 2, "should have multiple events before clear");
    // Clear.
    gos_runtime::clear_node_log(NLC_VEC_A).unwrap();
    let (post_clear_total, _) = log_page_a();
    assert_eq!(post_clear_total, 0, "total must be 0 immediately after clear (not cumulative)");
    // Generate one new event.
    gos_runtime::fault_node(NLC_VEC_A).unwrap();
    let (post_event_total, _) = log_page_a();
    assert_eq!(post_event_total, 1, "total should be 1 after one event following clear");
}

// ── 10. clear_node_log returns Ok(()) for a live node ─────────────────────────

#[test]
fn clear_returns_ok_for_live_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let result = gos_runtime::clear_node_log(NLC_VEC_A);
    assert_eq!(result, Ok(()), "clear_node_log must return Ok(()) for a registered node");
}
