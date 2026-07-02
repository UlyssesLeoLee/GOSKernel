// gos-node-trace-harness — V2.24 per-node signal trace ring tests
//
// Verifies the node_trace_page() API and the signal trace ring in gos-runtime.
// Analogous to strace -p <pid>: shows recent signal dispatches for one node.
//
//  1. node_trace_page on unknown vector returns NodeNotFound.
//  2. Fresh node has zero trace entries (no dispatches yet).
//  3. After one Control dispatch, trace returns 1 entry.
//  4. Trace entry kind matches Control signal (kind == 0x05).
//  5. Trace entry cmd matches the cmd byte of a Control signal.
//  6. After a Data dispatch, entry kind == 0x04 and cmd == data byte.
//  7. Trace entry serial starts at 0 for the first dispatch.
//  8. Second dispatch produces serial == 1.
//  9. Ring wraps after MAX_NODE_TRACE dispatches: returned == MAX_NODE_TRACE.
// 10. Most recent dispatch is at index 0 (newest-first ordering).

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id,
    EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, Signal, VectorAddress,
};
use gos_runtime::{RuntimeError, MAX_NODE_TRACE};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const NT_PLUGIN: PluginId   = PluginId::from_ascii("KL_NTRACE_H");
const NT_EXEC:   ExecutorId = ExecutorId::from_ascii("nt.exec");

const NT_KEY_A: &str = "nt.alpha";

const NT_ID_A: NodeId = derive_node_id(NT_PLUGIN, NT_KEY_A);

const NT_VEC_A:       VectorAddress = VectorAddress::new(9, 4, 1, 0);
const NT_VEC_UNKNOWN: VectorAddress = VectorAddress::new(9, 9, 8, 8);

const NT_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    NT_PLUGIN,
    name:         "kl-node-trace-harness",
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
        node_id:           NT_ID_A,
        local_node_key:    NT_KEY_A,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NT_EXEC,
        state_schema_hash: 0xAB24,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_a() {
    gos_runtime::discover_plugin(NT_MANIFEST).unwrap();
    gos_runtime::register_node(NT_PLUGIN, NT_VEC_A, spec_a()).unwrap();
}

// ── 1. Unknown vector returns NodeNotFound ───────────────────────────────────

#[test]
fn trace_unknown_vector_returns_not_found() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let result = gos_runtime::node_trace_page(NT_VEC_UNKNOWN, &mut ring);
    assert_eq!(result, Err(RuntimeError::NodeNotFound));
}

// ── 2. Fresh node has zero trace entries ─────────────────────────────────────

#[test]
fn trace_fresh_node_returns_zero_entries() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (total, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(total, 0);
    assert_eq!(returned, 0);
}

// ── 3. One dispatch → one trace entry ────────────────────────────────────────

#[test]
fn trace_one_dispatch_returns_one_entry() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0x42, val: 0 });
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (total, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(total, 1);
    assert_eq!(returned, 1);
}

// ── 4. Entry kind == 0x05 for Control signal ─────────────────────────────────

#[test]
fn trace_entry_kind_matches_control() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0x42, val: 0 });
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (_, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(returned, 1);
    assert_eq!(ring[0].kind, 0x05, "Control signal should have kind 0x05");
}

// ── 5. Entry cmd matches the Control cmd byte ─────────────────────────────────

#[test]
fn trace_entry_cmd_matches_control_cmd() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0x42, val: 99 });
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (_, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(returned, 1);
    assert_eq!(ring[0].cmd, 0x42, "cmd should match the Control.cmd field");
}

// ── 6. Data dispatch: kind==0x04, cmd==data byte ─────────────────────────────

#[test]
fn trace_data_signal_kind_and_cmd() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Data { from: 0, byte: b'X' });
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (_, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(returned, 1);
    assert_eq!(ring[0].kind, 0x04, "Data signal should have kind 0x04");
    assert_eq!(ring[0].cmd, b'X', "cmd should hold the data byte for Data signals");
}

// ── 7. First entry has serial == 0 ───────────────────────────────────────────

#[test]
fn trace_first_entry_serial_is_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0x01, val: 0 });
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (_, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(returned, 1);
    assert_eq!(ring[0].serial, 0, "first dispatch serial should be 0 (signal_count before first increment)");
}

// ── 8. Second dispatch serial == 1 ───────────────────────────────────────────

#[test]
fn trace_second_entry_serial_is_one() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0x01, val: 0 });
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0x02, val: 0 });
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (_, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(returned, 2);
    // Newest first: ring[0] is dispatch #2 (serial=1), ring[1] is dispatch #1 (serial=0)
    assert_eq!(ring[0].serial, 1, "most recent entry should have serial == 1");
    assert_eq!(ring[1].serial, 0, "older entry should have serial == 0");
}

// ── 9. Ring wraps: returned == MAX_NODE_TRACE after overflow ──────────────────

#[test]
fn trace_ring_wraps_after_max_entries() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    // Dispatch MAX_NODE_TRACE + 4 signals to overflow the ring.
    for i in 0..(MAX_NODE_TRACE + 4) {
        let _ = gos_runtime::route_signal(
            NT_VEC_A,
            Signal::Control { cmd: i as u8, val: 0 },
        );
    }
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (total, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(total as usize, MAX_NODE_TRACE + 4, "total should reflect all dispatches");
    assert_eq!(returned, MAX_NODE_TRACE, "ring should be full (capped at MAX_NODE_TRACE)");
}

// ── 10. Most recent dispatch is at index 0 ────────────────────────────────────

#[test]
fn trace_newest_first_ordering() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    // Send 3 signals with distinct cmd bytes so we can identify them.
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0xAA, val: 0 });
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0xBB, val: 0 });
    let _ = gos_runtime::route_signal(NT_VEC_A, Signal::Control { cmd: 0xCC, val: 0 });
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (_, returned) = gos_runtime::node_trace_page(NT_VEC_A, &mut ring).unwrap();
    assert_eq!(returned, 3);
    // Index 0 = most recent (0xCC), index 1 = second (0xBB), index 2 = oldest (0xAA)
    assert_eq!(ring[0].cmd, 0xCC, "most recent dispatch should be at index 0");
    assert_eq!(ring[1].cmd, 0xBB, "second most recent at index 1");
    assert_eq!(ring[2].cmd, 0xAA, "oldest at index 2");
}
