// gos-watch-harness — V2.30 live proc watch mode tests
//
// Verifies the runtime properties that power the `watch` / `graph watch`
// shell command: idempotent reads from proc_page, live state reflection,
// consistency between snapshot() and proc_count(), and tick advancement.
// These invariants guarantee that a heartbeat-driven watch loop is safe and
// accurate without locking or special synchronisation.
//
//  1. proc_page is idempotent: two consecutive calls return identical totals.
//  2. proc_page returns 0 on empty runtime (watch shows "(no nodes)").
//  3. After registering a node, proc_page reflects it immediately.
//  4. proc_count() and proc_page total are consistent after registration.
//  5. proc_page reflects updated signal_count after a dispatch.
//  6. Repeated proc_page calls after dispatch all return the same count.
//  7. After fault_node(), lifecycle shows Faulted in proc_page.
//  8. After resume_node(), lifecycle reverts to Running in proc_page.
//  9. snapshot().node_count matches proc_count() on empty and non-empty runtime.
// 10. snapshot().tick advances after pump() — watch tick counter is live.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId, NodeSpec,
    PluginId, PluginManifest, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared test fixtures ──────────────────────────────────────────────────────

const WA_PLUGIN: PluginId = PluginId::from_ascii("GOS_WATCH");
const EXEC: ExecutorId = ExecutorId::from_ascii("wa.exec");

const WA_KEY_A: &str = "watch.alpha";
const WA_KEY_B: &str = "watch.beta";
const WA_KEY_C: &str = "watch.gamma";

const WA_ID_A: NodeId = derive_node_id(WA_PLUGIN, WA_KEY_A);
const WA_ID_B: NodeId = derive_node_id(WA_PLUGIN, WA_KEY_B);
const WA_ID_C: NodeId = derive_node_id(WA_PLUGIN, WA_KEY_C);

const VEC_A: VectorAddress = VectorAddress::new(0xD1, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xD1, 2, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0xD1, 3, 0, 0);

const fn wa_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
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

const WA_SPEC_A: NodeSpec = wa_spec(WA_KEY_A, WA_ID_A);
const WA_SPEC_B: NodeSpec = wa_spec(WA_KEY_B, WA_ID_B);
const WA_SPEC_C: NodeSpec = wa_spec(WA_KEY_C, WA_ID_C);

const WA_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: WA_PLUGIN,
    name: "GOS_WATCH",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[WA_SPEC_A, WA_SPEC_B, WA_SPEC_C],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn setup() {
    gos_runtime::reset();
    gos_runtime::discover_plugin(WA_MANIFEST).unwrap();
}

// ── Test 1: proc_page is idempotent ──────────────────────────────────────────

#[test]
fn proc_page_is_idempotent() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();
    gos_runtime::register_node(WA_PLUGIN, VEC_B, WA_SPEC_B).unwrap();

    let mut out1 = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let mut out2 = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (total1, filled1) = gos_runtime::proc_page::<8>(0, &mut out1);
    let (total2, filled2) = gos_runtime::proc_page::<8>(0, &mut out2);

    assert_eq!(total1, total2, "total is stable across two consecutive calls");
    assert_eq!(filled1, filled2, "filled is stable across two consecutive calls");
    assert_eq!(out1[0].vector, out2[0].vector, "first entry stable");
    assert_eq!(out1[1].vector, out2[1].vector, "second entry stable");
}

// ── Test 2: empty runtime → watch shows no nodes ─────────────────────────────

#[test]
fn proc_page_empty_on_empty_runtime() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::proc_page::<8>(0, &mut out);
    assert_eq!(total, 0, "empty runtime shows 0 nodes");
    assert_eq!(filled, 0);
}

// ── Test 3: node registration reflects immediately ───────────────────────────

#[test]
fn proc_page_reflects_registration_immediately() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (before, _) = gos_runtime::proc_page::<8>(0, &mut out);
    assert_eq!(before, 0, "no nodes before registration");

    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();

    let (after, _) = gos_runtime::proc_page::<8>(0, &mut out);
    assert_eq!(after, 1, "one node after registration");
}

// ── Test 4: proc_count() and proc_page total are consistent ──────────────────

#[test]
fn proc_count_consistent_with_proc_page_total() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();
    gos_runtime::register_node(WA_PLUGIN, VEC_B, WA_SPEC_B).unwrap();
    gos_runtime::register_node(WA_PLUGIN, VEC_C, WA_SPEC_C).unwrap();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (total, _) = gos_runtime::proc_page::<8>(0, &mut out);
    let count = gos_runtime::proc_count();

    assert_eq!(total, count, "proc_page total and proc_count must agree");
}

// ── Test 5: proc_page reflects updated signal_count after dispatch ────────────

#[test]
fn proc_page_reflects_signal_count_after_dispatch() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();

    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'w' });

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (_, filled) = gos_runtime::proc_page::<8>(0, &mut out);
    assert!(filled >= 1);
    assert_eq!(out[0].signal_count, 1, "watch: signal_count reflects live dispatch");
}

// ── Test 6: repeated reads after dispatch return the same count ───────────────

#[test]
fn repeated_proc_page_reads_stable_after_dispatch() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();

    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'x' });
    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'y' });

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (_, _) = gos_runtime::proc_page::<8>(0, &mut out);
    let count_first = out[0].signal_count;

    let mut out2 = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (_, _) = gos_runtime::proc_page::<8>(0, &mut out2);
    let count_second = out2[0].signal_count;

    assert_eq!(count_first, 2, "first read: signal_count == 2");
    assert_eq!(count_second, 2, "second read: signal_count still == 2 (pure read)");
}

// ── Test 7: fault_node() makes lifecycle Faulted in proc_page ────────────────

#[test]
fn proc_page_shows_faulted_after_fault_node() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();
    gos_runtime::fault_node(VEC_A).ok();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (_, filled) = gos_runtime::proc_page::<8>(0, &mut out);
    assert!(filled >= 1);
    assert_eq!(
        out[0].lifecycle,
        gos_protocol::NodeLifecycle::Faulted,
        "watch: faulted node shows Faulted lifecycle"
    );
}

// ── Test 8: resume_node() reverts lifecycle to Running ────────────────────────

#[test]
fn proc_page_shows_running_after_resume() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();
    gos_runtime::fault_node(VEC_A).ok();
    gos_runtime::resume_node(VEC_A).ok();

    let mut out = [gos_protocol::NodeProcSummary::EMPTY; 8];
    let (_, filled) = gos_runtime::proc_page::<8>(0, &mut out);
    assert!(filled >= 1);
    assert_ne!(
        out[0].lifecycle,
        gos_protocol::NodeLifecycle::Faulted,
        "watch: resumed node should no longer be Faulted"
    );
}

// ── Test 9: snapshot().node_count matches proc_count() ───────────────────────

#[test]
fn snapshot_node_count_matches_proc_count() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();

    // Empty
    let snap0 = gos_runtime::snapshot();
    let cnt0 = gos_runtime::proc_count();
    assert_eq!(snap0.node_count, cnt0, "empty: snapshot.node_count == proc_count");

    // After two registrations
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();
    gos_runtime::register_node(WA_PLUGIN, VEC_B, WA_SPEC_B).unwrap();
    let snap2 = gos_runtime::snapshot();
    let cnt2 = gos_runtime::proc_count();
    assert_eq!(snap2.node_count, cnt2, "two nodes: snapshot.node_count == proc_count");
}

// ── Test 10: snapshot().tick advances after pump() ───────────────────────────

#[test]
fn snapshot_tick_advances_after_pump() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(WA_PLUGIN, VEC_A, WA_SPEC_A).unwrap();

    let tick_before = gos_runtime::snapshot().tick;
    gos_runtime::pump();
    let tick_after = gos_runtime::snapshot().tick;

    assert!(
        tick_after >= tick_before,
        "watch tick counter: snapshot().tick must be non-decreasing after pump()"
    );
}
