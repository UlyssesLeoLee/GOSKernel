// gos-runtime host harness
//
// Drives gos_runtime in isolation on the host: registers a synthetic
// plugin + node, binds a native executor that returns ExecStatus::Fault,
// pushes a signal through route_signal, and asserts the runtime fault
// queue captures the offending vector address.  Locks in the Phase B.1
// fault-attribution contract end-to-end without needing a kernel.
//
// The runtime exposes a global RUNTIME singleton, so tests must serialize
// against the shared TEST_LOCK.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecStatus, ExecutorContext, ExecutorId, NodeEvent,
    NodeExecutorVTable, NodeSpec, PluginId, PluginManifest, RuntimeNodeType, Signal,
    VectorAddress, GOS_ABI_VERSION,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const TEST_PLUGIN_ID: PluginId = PluginId::from_ascii("HARNESS_RT");
const TEST_NODE_KEY: &str = "harness.entry";
const TEST_VECTOR: VectorAddress = VectorAddress::new(7, 7, 7, 7);
const TEST_EXECUTOR: ExecutorId = ExecutorId::from_ascii("native.harness");

const TEST_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: derive_node_id(TEST_PLUGIN_ID, TEST_NODE_KEY),
    local_node_key: TEST_NODE_KEY,
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Manual,
    executor_id: TEST_EXECUTOR,
    state_schema_hash: 0xDEAD_BEEF,
    permissions: &[],
    exports: &[],
    vector_ref: None,
}];

const TEST_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: TEST_PLUGIN_ID,
    name: "HARNESS_RT",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: TEST_NODE_SPECS,
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

unsafe extern "C" fn faulting_on_event(
    _ctx: *mut ExecutorContext,
    _event: *const NodeEvent,
) -> ExecStatus {
    ExecStatus::Fault
}

const FAULTING_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: TEST_EXECUTOR,
    on_init: None,
    on_event: Some(faulting_on_event),
    on_suspend: None,
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

fn install_test_node() {
    gos_runtime::reset();
    gos_runtime::discover_plugin(TEST_MANIFEST).expect("discover plugin");
    gos_runtime::mark_plugin_loaded(TEST_PLUGIN_ID).expect("mark loaded");
    gos_runtime::register_node(TEST_PLUGIN_ID, TEST_VECTOR, TEST_NODE_SPECS[0])
        .expect("register node");
    gos_runtime::bind_native_executor(TEST_VECTOR, FAULTING_VTABLE)
        .expect("bind executor");
}

#[test]
fn route_signal_to_faulting_executor_pushes_vector_into_fault_queue() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    install_test_node();

    // Sanity: vector resolves to the registered node's plugin id, and no
    // instance is bound yet (boot-fallback regime).
    assert_eq!(
        gos_runtime::plugin_id_for_vec(TEST_VECTOR),
        Some(TEST_PLUGIN_ID)
    );
    assert!(gos_runtime::drain_next_fault().is_none());

    // Drive a signal through the dispatch path; the vtable returns
    // ExecStatus::Fault, so the runtime must enqueue TEST_VECTOR.
    let _ = gos_runtime::route_signal(TEST_VECTOR, Signal::Spawn { payload: 0 });

    let drained = gos_runtime::drain_next_fault();
    assert_eq!(drained, Some(TEST_VECTOR));
    assert!(
        gos_runtime::drain_next_fault().is_none(),
        "fault queue must be empty after a single drain"
    );
}

#[test]
fn instance_binding_propagates_through_dispatch_and_clears_on_unbind() {
    use gos_protocol::NodeInstanceId;

    let _guard = TEST_LOCK.lock().expect("test lock");
    install_test_node();

    // Initial: no instance bound.
    assert_eq!(
        gos_runtime::instance_id_for_vec(TEST_VECTOR),
        Some(NodeInstanceId::ZERO)
    );

    // Bind via plugin-wide helper; every node of the plugin should pick
    // up the new instance id.
    let inst = NodeInstanceId::new(42);
    let bound = gos_runtime::bind_plugin_instance(TEST_PLUGIN_ID, inst);
    assert_eq!(bound, 1);
    assert_eq!(gos_runtime::instance_id_for_vec(TEST_VECTOR), Some(inst));

    // Re-bind to ZERO simulating module teardown — the runtime must
    // forget the prior id.
    let cleared = gos_runtime::bind_plugin_instance(TEST_PLUGIN_ID, NodeInstanceId::ZERO);
    assert_eq!(cleared, 1);
    assert_eq!(
        gos_runtime::instance_id_for_vec(TEST_VECTOR),
        Some(NodeInstanceId::ZERO)
    );
}
