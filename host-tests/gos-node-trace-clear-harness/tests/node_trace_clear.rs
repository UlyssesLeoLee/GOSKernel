// gos-node-trace-clear-harness — V2.27 per-node signal trace clear tests
//
// Verifies the clear_node_trace() API added to gos-runtime.
// Analogous to `perf trace reset` / `truncate -s0 strace.log`:
// discards the buffered signal dispatch history for one node while preserving
// the cumulative signal_count used by `proc`.
//
//  1. clear_node_trace on unknown vector returns NodeNotFound.
//  2. After clear on a fresh node, trace returns (0, 0).
//  3. After clear, node_trace_page still succeeds (node still registered).
//  4. After one dispatch then clear, trace is empty.
//  5. After multiple dispatches then clear, trace is empty.
//  6. clear is idempotent: double-clear still returns (0, 0).
//  7. After clear, new dispatches are traced correctly (ring is fresh).
//  8. Clearing node A does not affect node B's trace.
//  9. After clear, total starts fresh from 0 (not from pre-clear total).
// 10. clear_node_trace returns Ok(()) for a live node.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id,
    EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, Signal, VectorAddress,
};
use gos_runtime::{RuntimeError, MAX_NODE_TRACE};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const NTC_PLUGIN: PluginId   = PluginId::from_ascii("KL_NTCLR_H");
const NTC_EXEC:   ExecutorId = ExecutorId::from_ascii("ntc.exec");

const NTC_KEY_A: &str = "ntc.alpha";
const NTC_KEY_B: &str = "ntc.beta";

const NTC_ID_A: NodeId = derive_node_id(NTC_PLUGIN, NTC_KEY_A);
const NTC_ID_B: NodeId = derive_node_id(NTC_PLUGIN, NTC_KEY_B);

const NTC_VEC_A:       VectorAddress = VectorAddress::new(9, 7, 1, 0);
const NTC_VEC_B:       VectorAddress = VectorAddress::new(9, 7, 2, 0);
const NTC_VEC_UNKNOWN: VectorAddress = VectorAddress::new(9, 9, 7, 7);

const NTC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    NTC_PLUGIN,
    name:         "kl-node-trace-clear-harness",
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
        node_id:           NTC_ID_A,
        local_node_key:    NTC_KEY_A,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NTC_EXEC,
        state_schema_hash: 0xC271,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn spec_b() -> NodeSpec {
    NodeSpec {
        node_id:           NTC_ID_B,
        local_node_key:    NTC_KEY_B,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NTC_EXEC,
        state_schema_hash: 0xC272,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_a() {
    gos_runtime::discover_plugin(NTC_MANIFEST).unwrap();
    gos_runtime::register_node(NTC_PLUGIN, NTC_VEC_A, spec_a()).unwrap();
}

fn register_ab() {
    gos_runtime::discover_plugin(NTC_MANIFEST).unwrap();
    gos_runtime::register_node(NTC_PLUGIN, NTC_VEC_A, spec_a()).unwrap();
    gos_runtime::register_node(NTC_PLUGIN, NTC_VEC_B, spec_b()).unwrap();
}

fn dispatch_control_a(cmd: u8) {
    let _ = gos_runtime::route_signal(NTC_VEC_A, Signal::Control { cmd, val: 0 });
}

fn dispatch_control_b(cmd: u8) {
    let _ = gos_runtime::route_signal(NTC_VEC_B, Signal::Control { cmd, val: 0 });
}

fn trace_page_a() -> (u32, usize) {
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    gos_runtime::node_trace_page(NTC_VEC_A, &mut ring).unwrap()
}

fn trace_page_b() -> (u32, usize) {
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    gos_runtime::node_trace_page(NTC_VEC_B, &mut ring).unwrap()
}

// ── 1. Unknown vector returns NodeNotFound ───────────────────────────────────

#[test]
fn clear_unknown_vector_returns_not_found() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let result = gos_runtime::clear_node_trace(NTC_VEC_UNKNOWN);
    assert_eq!(result, Err(RuntimeError::NodeNotFound));
}

// ── 2. After clear on a fresh node, trace is (0, 0) ──────────────────────────

#[test]
fn clear_fresh_node_gives_zero_entries() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    let (total, returned) = trace_page_a();
    assert_eq!(total, 0, "total should be 0 after clear");
    assert_eq!(returned, 0, "returned should be 0 after clear");
}

// ── 3. After clear, node_trace_page still succeeds (node still registered) ───

#[test]
fn clear_does_not_unregister_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let result = gos_runtime::node_trace_page(NTC_VEC_A, &mut ring);
    assert!(result.is_ok(), "node_trace_page should succeed after clear — node is still registered");
}

// ── 4. After one dispatch then clear, trace is empty ─────────────────────────

#[test]
fn clear_discards_single_dispatch_entry() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    dispatch_control_a(0x10);
    // Confirm entry present before clear.
    let (_, returned_before) = trace_page_a();
    assert_eq!(returned_before, 1, "one dispatch should produce one trace entry");
    // Clear.
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    let (total, returned) = trace_page_a();
    assert_eq!(total, 0, "total should be 0 after clear");
    assert_eq!(returned, 0, "entry should be gone after clear");
}

// ── 5. After multiple dispatches then clear, all entries gone ─────────────────

#[test]
fn clear_discards_multiple_dispatch_entries() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    for cmd in 0..5u8 {
        dispatch_control_a(cmd);
    }
    let (_, returned_before) = trace_page_a();
    assert_eq!(returned_before, 5, "5 dispatches should produce 5 trace entries");
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    let (total, returned) = trace_page_a();
    assert_eq!(total, 0, "total must be 0 after clear");
    assert_eq!(returned, 0, "all entries must be gone after clear");
}

// ── 6. clear is idempotent: double-clear still returns (0, 0) ────────────────

#[test]
fn clear_is_idempotent() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    dispatch_control_a(0x20);
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap(); // second clear
    let (total, returned) = trace_page_a();
    assert_eq!(total, 0);
    assert_eq!(returned, 0);
}

// ── 7. After clear, new dispatches are traced correctly (ring is fresh) ───────

#[test]
fn clear_then_new_dispatches_traced_correctly() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    dispatch_control_a(0x30);
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    // Dispatch after clear.
    dispatch_control_a(0x42);
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (total, returned) = gos_runtime::node_trace_page(NTC_VEC_A, &mut ring).unwrap();
    assert_eq!(total, 1, "total should be 1 after one post-clear dispatch");
    assert_eq!(returned, 1, "one entry returned");
    assert_eq!(ring[0].kind, 0x05, "entry kind should be Control (0x05)");
    assert_eq!(ring[0].cmd, 0x42, "entry cmd should match dispatched cmd");
}

// ── 8. Clearing node A does not affect node B's trace ────────────────────────

#[test]
fn clear_does_not_affect_sibling_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    // Dispatch to both nodes.
    dispatch_control_a(0x01);
    dispatch_control_b(0x02);
    let (b_total_before, _) = trace_page_b();
    // Clear only A.
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    let (a_total_after, a_returned) = trace_page_a();
    let (b_total_after, _) = trace_page_b();
    assert_eq!(a_total_after, 0, "A trace cleared");
    assert_eq!(a_returned, 0, "A trace cleared");
    assert_eq!(b_total_after, b_total_before, "B trace unchanged after clearing A");
}

// ── 9. After clear, total starts fresh from 0 ────────────────────────────────

#[test]
fn clear_resets_total_counter_to_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    // Generate some trace entries.
    for cmd in 0..3u8 {
        dispatch_control_a(cmd);
    }
    let (pre_clear_total, _) = trace_page_a();
    assert_eq!(pre_clear_total, 3, "should have 3 entries before clear");
    // Clear.
    gos_runtime::clear_node_trace(NTC_VEC_A).unwrap();
    let (post_clear_total, _) = trace_page_a();
    assert_eq!(post_clear_total, 0, "total must be 0 immediately after clear (not cumulative)");
    // Dispatch one more.
    dispatch_control_a(0x99);
    let (post_event_total, _) = trace_page_a();
    assert_eq!(post_event_total, 1, "total should be 1 after one dispatch following clear");
}

// ── 10. clear_node_trace returns Ok(()) for a live node ───────────────────────

#[test]
fn clear_returns_ok_for_live_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let result = gos_runtime::clear_node_trace(NTC_VEC_A);
    assert_eq!(result, Ok(()), "clear_node_trace must return Ok(()) for a registered node");
}
