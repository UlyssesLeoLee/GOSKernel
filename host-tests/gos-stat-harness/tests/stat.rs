// gos-stat-harness — V2.15 proc_stat_for_vector tests
//
// Verifies that gos-runtime correctly exposes per-node stat lookup by
// VectorAddress — the backing API for the `stat <vec>` shell command added
// in V2.15.  The command is analogous to `cat /proc/<pid>/status` on Linux.
//
//  1. Unknown vector → proc_stat_for_vector returns None.
//  2. Registered node → proc_stat_for_vector returns Some.
//  3. Returned summary.vector matches the queried vector.
//  4. Returned summary.local_node_key matches the spec key.
//  5. Returned summary.plugin_name matches the registered plugin.
//  6. Fresh node has signal_count == 0.
//  7. After one route_signal → signal_count == 1.
//  8. After two route_signal calls → signal_count == 2.
//  9. edge_out_count == 0 when no outbound edges are registered.
// 10. Looking up by wrong vector returns None (not a different node).

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId, NodeSpec,
    PluginId, PluginManifest, RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const ST_PLUGIN: PluginId = PluginId::from_ascii("GOS_STAT");
const EXEC: ExecutorId = ExecutorId::from_ascii("st.exec");

const ST_KEY_A: &str = "stat.alpha";
const ST_KEY_B: &str = "stat.beta";

const ST_ID_A: NodeId = derive_node_id(ST_PLUGIN, ST_KEY_A);
const ST_ID_B: NodeId = derive_node_id(ST_PLUGIN, ST_KEY_B);

const VEC_A: VectorAddress = VectorAddress::new(0xD1, 1, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xD1, 2, 0, 0);
const VEC_UNKNOWN: VectorAddress = VectorAddress::new(0xFF, 0, 0, 0);

const fn st_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
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

const ST_SPEC_A: NodeSpec = st_spec(ST_KEY_A, ST_ID_A);
const ST_SPEC_B: NodeSpec = st_spec(ST_KEY_B, ST_ID_B);

const ST_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: ST_PLUGIN,
    name: "GOS_STAT",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[ST_SPEC_A, ST_SPEC_B],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn setup() {
    gos_runtime::reset();
    gos_runtime::discover_plugin(ST_MANIFEST).unwrap();
}

// ── Test 1: unknown vector → None ────────────────────────────────────────────

#[test]
fn unknown_vector_returns_none() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();

    let result = gos_runtime::proc_stat_for_vector(VEC_UNKNOWN);
    assert!(result.is_none(), "unknown vector should return None");
}

// ── Test 2: registered node → Some ───────────────────────────────────────────

#[test]
fn registered_node_returns_some() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();

    let result = gos_runtime::proc_stat_for_vector(VEC_A);
    assert!(result.is_some(), "registered node should return Some");
}

// ── Test 3: returned summary.vector matches queried vector ───────────────────

#[test]
fn stat_vector_matches() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();

    let s = gos_runtime::proc_stat_for_vector(VEC_A).unwrap();
    assert_eq!(s.vector, VEC_A, "summary.vector should match queried vector");
}

// ── Test 4: returned summary.local_node_key matches spec key ─────────────────

#[test]
fn stat_key_matches() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();

    let s = gos_runtime::proc_stat_for_vector(VEC_A).unwrap();
    assert_eq!(s.local_node_key, ST_KEY_A, "local_node_key should match spec");
}

// ── Test 5: returned summary.plugin_name matches registered plugin ───────────

#[test]
fn stat_plugin_name_matches() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();

    let s = gos_runtime::proc_stat_for_vector(VEC_A).unwrap();
    assert_eq!(s.plugin_name, "GOS_STAT", "plugin_name should match manifest name");
}

// ── Test 6: fresh node has signal_count == 0 ─────────────────────────────────

#[test]
fn fresh_node_signal_count_is_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_B, ST_SPEC_B).unwrap();

    let s = gos_runtime::proc_stat_for_vector(VEC_B).unwrap();
    assert_eq!(s.signal_count, 0, "freshly registered node has signal_count 0");
}

// ── Test 7: after one route_signal → signal_count == 1 ───────────────────────

#[test]
fn stat_signal_count_after_one_dispatch() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();

    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'x' });

    let s = gos_runtime::proc_stat_for_vector(VEC_A).unwrap();
    assert_eq!(s.signal_count, 1, "signal_count should be 1 after one dispatch");
}

// ── Test 8: after two route_signal calls → signal_count == 2 ─────────────────

#[test]
fn stat_signal_count_after_two_dispatches() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();

    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'a' });
    let _ = gos_runtime::route_signal(VEC_A, gos_protocol::Signal::Data { from: 0, byte: b'b' });

    let s = gos_runtime::proc_stat_for_vector(VEC_A).unwrap();
    assert_eq!(s.signal_count, 2, "signal_count should be 2 after two dispatches");
}

// ── Test 9: edge_out_count == 0 when no edges registered ─────────────────────

#[test]
fn stat_edge_out_count_zero_when_no_edges() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();

    let s = gos_runtime::proc_stat_for_vector(VEC_A).unwrap();
    assert_eq!(s.edge_out_count, 0, "no outbound edges: edge_out_count should be 0");
}

// ── Test 10: wrong vector returns None, not a different node ─────────────────

#[test]
fn wrong_vector_returns_none_not_other_node() {
    let _lock = TEST_LOCK.lock().unwrap();
    setup();
    gos_runtime::register_node(ST_PLUGIN, VEC_A, ST_SPEC_A).unwrap();
    gos_runtime::register_node(ST_PLUGIN, VEC_B, ST_SPEC_B).unwrap();

    // Query VEC_UNKNOWN (not registered) — must return None even when other
    // nodes exist in the runtime.
    let result = gos_runtime::proc_stat_for_vector(VEC_UNKNOWN);
    assert!(
        result.is_none(),
        "querying an unregistered vector must return None, not a neighbouring node"
    );
}
