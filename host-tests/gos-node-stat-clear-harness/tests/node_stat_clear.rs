// gos-node-stat-clear-harness — V2.28 per-node signal_count reset tests
//
// Verifies the reset_node_stat() API added to gos-runtime.
// Analogous to `perf stat reset` / `echo 0 > /proc/<pid>/clear_refs`:
// zeroes the cumulative signal dispatch counter shown by `proc` and `stat`
// without affecting the trace ring or lifecycle log.
//
//  1. reset_node_stat on unknown vector returns NodeNotFound.
//  2. reset on a fresh node (signal_count == 0) returns Ok(()).
//  3. After one dispatch then reset, signal_count is 0.
//  4. After multiple dispatches then reset, signal_count is 0.
//  5. reset is idempotent: double-reset stays at 0.
//  6. New signals after reset increment from 0 (not cumulative).
//  7. Resetting node A does not affect node B's signal_count.
//  8. reset does not affect the trace ring (entries still present).
//  9. After reset, proc_page reflects signal_count == 0.
// 10. reset returns Ok(()) for a live node.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id,
    EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, Signal, VectorAddress,
};
use gos_runtime::{RuntimeError, MAX_NODE_TRACE};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const NSC_PLUGIN: PluginId   = PluginId::from_ascii("KL_NSTCLR_H");
const NSC_EXEC:   ExecutorId = ExecutorId::from_ascii("nsc.exec");

const NSC_KEY_A: &str = "nsc.alpha";
const NSC_KEY_B: &str = "nsc.beta";

const NSC_ID_A: NodeId = derive_node_id(NSC_PLUGIN, NSC_KEY_A);
const NSC_ID_B: NodeId = derive_node_id(NSC_PLUGIN, NSC_KEY_B);

const NSC_VEC_A:       VectorAddress = VectorAddress::new(9, 8, 1, 0);
const NSC_VEC_B:       VectorAddress = VectorAddress::new(9, 8, 2, 0);
const NSC_VEC_UNKNOWN: VectorAddress = VectorAddress::new(9, 9, 8, 8);

const NSC_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    NSC_PLUGIN,
    name:         "kl-node-stat-clear-harness",
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
        node_id:           NSC_ID_A,
        local_node_key:    NSC_KEY_A,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NSC_EXEC,
        state_schema_hash: 0xD281,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn spec_b() -> NodeSpec {
    NodeSpec {
        node_id:           NSC_ID_B,
        local_node_key:    NSC_KEY_B,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NSC_EXEC,
        state_schema_hash: 0xD282,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() {
    gos_runtime::reset();
}

fn register_a() {
    gos_runtime::discover_plugin(NSC_MANIFEST).unwrap();
    gos_runtime::register_node(NSC_PLUGIN, NSC_VEC_A, spec_a()).unwrap();
}

fn register_ab() {
    gos_runtime::discover_plugin(NSC_MANIFEST).unwrap();
    gos_runtime::register_node(NSC_PLUGIN, NSC_VEC_A, spec_a()).unwrap();
    gos_runtime::register_node(NSC_PLUGIN, NSC_VEC_B, spec_b()).unwrap();
}

fn dispatch_to_a(cmd: u8) {
    let _ = gos_runtime::route_signal(NSC_VEC_A, Signal::Control { cmd, val: 0 });
}

fn dispatch_to_b(cmd: u8) {
    let _ = gos_runtime::route_signal(NSC_VEC_B, Signal::Control { cmd, val: 0 });
}

fn signal_count_a() -> u32 {
    gos_runtime::proc_stat_for_vector(NSC_VEC_A)
        .expect("proc_stat_for_vector should succeed for registered node A")
        .signal_count
}

fn signal_count_b() -> u32 {
    gos_runtime::proc_stat_for_vector(NSC_VEC_B)
        .expect("proc_stat_for_vector should succeed for registered node B")
        .signal_count
}

fn trace_fill_a() -> usize {
    let mut ring = [gos_protocol::NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    let (_, returned) = gos_runtime::node_trace_page(NSC_VEC_A, &mut ring).unwrap();
    returned
}

// ── 1. Unknown vector returns NodeNotFound ────────────────────────────────────

#[test]
fn reset_stat_unknown_vector_returns_not_found() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let result = gos_runtime::reset_node_stat(NSC_VEC_UNKNOWN);
    assert_eq!(result, Err(RuntimeError::NodeNotFound));
}

// ── 2. reset on a fresh node (signal_count == 0) returns Ok(()) ───────────────

#[test]
fn reset_stat_fresh_node_returns_ok() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let result = gos_runtime::reset_node_stat(NSC_VEC_A);
    assert_eq!(result, Ok(()));
    assert_eq!(signal_count_a(), 0, "signal_count should remain 0 after reset on fresh node");
}

// ── 3. After one dispatch then reset, signal_count is 0 ──────────────────────

#[test]
fn reset_stat_zeroes_signal_count_after_one_dispatch() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    dispatch_to_a(0x10);
    assert_eq!(signal_count_a(), 1, "one dispatch should give signal_count == 1");
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    assert_eq!(signal_count_a(), 0, "signal_count must be 0 after reset");
}

// ── 4. After multiple dispatches then reset, signal_count is 0 ───────────────

#[test]
fn reset_stat_zeroes_signal_count_after_many_dispatches() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    for cmd in 0..7u8 {
        dispatch_to_a(cmd);
    }
    assert_eq!(signal_count_a(), 7, "seven dispatches should give signal_count == 7");
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    assert_eq!(signal_count_a(), 0, "signal_count must be 0 after reset");
}

// ── 5. reset is idempotent: double-reset stays at 0 ──────────────────────────

#[test]
fn reset_stat_is_idempotent() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    dispatch_to_a(0x20);
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    assert_eq!(signal_count_a(), 0, "double-reset should still leave signal_count == 0");
}

// ── 6. New signals after reset increment from 0 (not cumulative) ─────────────

#[test]
fn reset_stat_new_dispatches_increment_from_zero() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    for cmd in 0..5u8 {
        dispatch_to_a(cmd);
    }
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    // Dispatch 3 more after reset.
    for cmd in 0..3u8 {
        dispatch_to_a(cmd + 0x80);
    }
    assert_eq!(signal_count_a(), 3, "signal_count should be 3 (post-reset only), not 8");
}

// ── 7. Resetting node A does not affect node B's signal_count ─────────────────

#[test]
fn reset_stat_does_not_affect_sibling_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_ab();
    dispatch_to_a(0x01);
    dispatch_to_b(0x02);
    dispatch_to_b(0x03);
    let b_before = signal_count_b();
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    assert_eq!(signal_count_a(), 0, "A signal_count must be 0 after reset");
    assert_eq!(signal_count_b(), b_before, "B signal_count must be unchanged after resetting A");
}

// ── 8. reset does not affect the trace ring (entries still present) ───────────

#[test]
fn reset_stat_preserves_trace_ring() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    dispatch_to_a(0x30);
    dispatch_to_a(0x31);
    let fill_before = trace_fill_a();
    assert_eq!(fill_before, 2, "two dispatches should produce 2 trace entries");
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    let fill_after = trace_fill_a();
    assert_eq!(fill_after, 2, "trace ring must be unaffected by stat reset");
}

// ── 9. After reset, proc_page reflects signal_count == 0 ─────────────────────

#[test]
fn reset_stat_reflects_in_proc_page() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    dispatch_to_a(0x40);
    dispatch_to_a(0x41);
    // Confirm proc_page shows 2 before reset.
    let mut page = [gos_protocol::NodeProcSummary::EMPTY; 4];
    let (_, filled) = gos_runtime::proc_page::<4>(0, &mut page);
    assert_eq!(filled, 1);
    assert_eq!(page[0].signal_count, 2, "proc_page should show signal_count == 2 before reset");
    // Reset.
    gos_runtime::reset_node_stat(NSC_VEC_A).unwrap();
    // proc_page should now show 0.
    let (_, filled2) = gos_runtime::proc_page::<4>(0, &mut page);
    assert_eq!(filled2, 1);
    assert_eq!(page[0].signal_count, 0, "proc_page should show signal_count == 0 after reset");
}

// ── 10. reset returns Ok(()) for a live node ──────────────────────────────────

#[test]
fn reset_stat_returns_ok_for_live_node() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    register_a();
    let result = gos_runtime::reset_node_stat(NSC_VEC_A);
    assert_eq!(result, Ok(()), "reset_node_stat must return Ok(()) for a registered node");
}
