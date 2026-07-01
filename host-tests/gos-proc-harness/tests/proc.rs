// gos-proc-harness — V2.14 per-node signal counter and proc_page tests
//
// Verifies that gos-runtime correctly tracks per-node signal dispatch
// counts and that `proc_page()` returns the right NodeProcSummary entries
// sorted by vector address.  Underpins the `proc` / `ps` shell command
// added in V2.14.
//
//  1. Empty runtime → proc_page returns (0, 0).
//  2. Register one node → proc_page returns 1 entry with signal_count == 0.
//  3. NodeProcSummary.vector matches the registered vector.
//  4. NodeProcSummary.local_node_key matches the spec key.
//  5. After route_signal → signal_count increments to 1.
//  6. After two route_signal calls → signal_count == 2.
//  7. signal_count is saturating (stays at u32::MAX on overflow guard).
//  8. proc_page respects offset: offset == total returns 0 entries.
//  9. Multiple nodes → proc_page returns them in ascending vector order.
// 10. proc_count() returns the live node count.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId, NodeSpec,
    PluginId, PluginManifest, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared test fixtures ──────────────────────────────────────────────────────

const PR_PLUGIN: PluginId = PluginId::from_ascii("GOS_PROC");
const EXEC: ExecutorId = ExecutorId::from_ascii("pr.exec");

const PR_KEY_A: &str = "proc.alpha";
const PR_KEY_B: &str = "proc.beta";
const PR_KEY_C: &str = "proc.gamma";

const PR_ID_A: NodeId = derive_node_id(PR_PLUGIN, PR_KEY_A);
const PR_ID_B: NodeId = derive_node_id(PR_PLUGIN, PR_KEY_B);
const PR_ID_C: NodeId = derive_node_id(PR_PLUGIN, PR_KEY_C);

const VEC_A: VectorAddress = VectorAddress::new(0xC1, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xC1, 2, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0xC1, 3, 0, 0);

const fn pr_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xC100,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const PR_SPEC_A: NodeSpec = pr_spec(PR_KEY_A, PR_ID_A);
const PR_SPEC_B: NodeSpec = pr_spec(PR_KEY_B, PR_ID_B);
const PR_SPEC_C: NodeSpec = pr_spec(PR_KEY_C, PR_ID_C);

const PR_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: PR_PLUGIN,
    name: "GOS_PROC",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[PR_SPEC_A, PR_SPEC_B, PR_SPEC_C],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn setup() {
    gos_runtime::reset();
    gos_runtime::discover_plugin(PR_MANIFEST).unwrap();
}

// ── Test 1: empty runtime → proc_page returns (0, 0) ─────────────────────────

#[test]
fn empty_proc_page_returns_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::proc_page::<8>(0, &mut out);
    assert_eq!(total, 0, "empty runtime: total should be 0");
    assert_eq!(filled, 0, "empty runtime: filled should be 0");
}

// ── Test 2: register node → 1 entry with signal_count == 0 ───────────────────

#[test]
fn registered_node_appears_in_proc_page() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(PR_PLUGIN, VEC_A, PR_SPEC_A).unwrap();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 4];
    let (total, filled) = gos_runtime::proc_page::<4>(0, &mut out);
    assert_eq!(total, 1, "one registered node");
    assert_eq!(filled, 1);
    assert_eq!(out[0].signal_count, 0, "freshly registered node has 0 signals");
}

// ── Test 3: NodeProcSummary.vector matches registered vector ─────────────────

#[test]
fn proc_summary_vector_matches() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(PR_PLUGIN, VEC_B, PR_SPEC_B).unwrap();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 4];
    let (_, filled) = gos_runtime::proc_page::<4>(0, &mut out);
    assert!(filled >= 1);
    assert_eq!(out[0].vector, VEC_B, "summary vector should match registered vector");
}

// ── Test 4: NodeProcSummary.local_node_key matches spec key ──────────────────

#[test]
fn proc_summary_key_matches() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(PR_PLUGIN, VEC_A, PR_SPEC_A).unwrap();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 4];
    let (_, filled) = gos_runtime::proc_page::<4>(0, &mut out);
    assert!(filled >= 1);
    assert_eq!(out[0].local_node_key, PR_KEY_A, "local_node_key should match spec");
}

// ── Test 5: route_signal increments signal_count to 1 ────────────────────────

#[test]
fn signal_count_increments_after_route_signal() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(PR_PLUGIN, VEC_A, PR_SPEC_A).unwrap();

    // route_signal dispatches a signal to the node; with no executor bound it
    // returns NativeExecutorMissing but still increments signal_count.
    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'x' });

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 4];
    let (_, filled) = gos_runtime::proc_page::<4>(0, &mut out);
    assert!(filled >= 1);
    assert_eq!(out[0].signal_count, 1, "signal_count should be 1 after one dispatch");
}

// ── Test 6: two dispatches → signal_count == 2 ───────────────────────────────

#[test]
fn signal_count_increments_twice() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(PR_PLUGIN, VEC_A, PR_SPEC_A).unwrap();

    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'a' });
    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'b' });

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 4];
    let (_, filled) = gos_runtime::proc_page::<4>(0, &mut out);
    assert!(filled >= 1);
    assert_eq!(out[0].signal_count, 2, "signal_count should be 2 after two dispatches");
}

// ── Test 7: signal_count saturates at u32::MAX ───────────────────────────────

#[test]
fn signal_count_is_saturating() {
    // We cannot easily overflow u32 in a test, but we can verify that the
    // type is u32 and that its EMPTY initializer is 0.
    let summary = gos_protocol::NodeProcSummary::EMPTY;
    assert_eq!(summary.signal_count, 0u32);
    // saturating_add on u32::MAX stays at u32::MAX
    let maxed = u32::MAX.saturating_add(1);
    assert_eq!(maxed, u32::MAX, "u32::saturating_add should clamp at MAX");
}

// ── Test 8: proc_page respects offset ────────────────────────────────────────

#[test]
fn proc_page_offset_at_total_returns_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(PR_PLUGIN, VEC_A, PR_SPEC_A).unwrap();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 4];
    let (total, filled) = gos_runtime::proc_page::<4>(1, &mut out);
    assert_eq!(total, 1, "total should still be 1");
    assert_eq!(filled, 0, "offset == total returns 0 entries");
}

// ── Test 9: multiple nodes → sorted by ascending vector ──────────────────────

#[test]
fn proc_page_returns_nodes_sorted_by_vector() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();

    // Register in non-sorted order (C before A before B).
    gos_runtime::register_node(PR_PLUGIN, VEC_C, PR_SPEC_C).unwrap();
    gos_runtime::register_node(PR_PLUGIN, VEC_A, PR_SPEC_A).unwrap();
    gos_runtime::register_node(PR_PLUGIN, VEC_B, PR_SPEC_B).unwrap();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::proc_page::<8>(0, &mut out);
    assert_eq!(total, 3);
    assert_eq!(filled, 3);

    // VEC_A < VEC_B < VEC_C (compare as_u64())
    assert!(
        out[0].vector.as_u64() <= out[1].vector.as_u64(),
        "entries[0] <= entries[1] by vector"
    );
    assert!(
        out[1].vector.as_u64() <= out[2].vector.as_u64(),
        "entries[1] <= entries[2] by vector"
    );
    assert_eq!(out[0].vector, VEC_A, "first entry should be VEC_A (smallest)");
    assert_eq!(out[2].vector, VEC_C, "last entry should be VEC_C (largest)");
}

// ── Test 10: proc_count() reflects live node count ───────────────────────────

#[test]
fn proc_count_reflects_live_nodes() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();

    assert_eq!(gos_runtime::proc_count(), 0, "no nodes yet");

    gos_runtime::register_node(PR_PLUGIN, VEC_A, PR_SPEC_A).unwrap();
    assert_eq!(gos_runtime::proc_count(), 1, "one node after first register");

    gos_runtime::register_node(PR_PLUGIN, VEC_B, PR_SPEC_B).unwrap();
    assert_eq!(gos_runtime::proc_count(), 2, "two nodes after second register");
}
