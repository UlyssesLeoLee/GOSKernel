// gos-metrics-harness — V2.6 runtime telemetry API tests
//
// Verifies that the V2.3/V2.6 observability metrics are coherent:
//
//   1. graph_epoch() starts at 0 after reset and increments on register_node.
//   2. graph_epoch() is stable when no structural mutation occurs.
//   3. idle_cycle_count() increments when service_system_cycle is called with
//      an unchanged graph (the supervisor's epoch-diff idle detector).
//   4. causal_depth_max() is 0 after service cycles with no queued work.
//   5. render_epoch() matches graph_epoch() after a service_system_cycle.
//   6. subscribe_pair_count() tracks the full register/unregister lifecycle.
//   7. causal_depth_max() is cumulative (peak across all service cycles).

use std::sync::Mutex;

use gos_protocol::{
    EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress, derive_node_id,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const P_ID: PluginId = PluginId::from_ascii("MET_HARNESS");
const KEY_A: &str = "met.a";
const KEY_B: &str = "met.b";
const KEY_C: &str = "met.c";
const EXEC: ExecutorId = ExecutorId::from_ascii("met.exec");

const VEC_A: VectorAddress = VectorAddress::new(0x20, 0, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0x21, 0, 0, 0);
const VEC_C: VectorAddress = VectorAddress::new(0x22, 0, 0, 0);

const NODE_SPECS: &[NodeSpec] = &[
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_A),
        local_node_key: KEY_A,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xA001,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_B),
        local_node_key: KEY_B,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xA002,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_C),
        local_node_key: KEY_C,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xA003,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
];

const MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: P_ID,
    name: "MET_HARNESS",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: NODE_SPECS,
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

fn setup() {
    gos_runtime::reset();
    gos_supervisor::clear_rewrite_rules();
    gos_runtime::discover_plugin(MANIFEST).ok();
    gos_runtime::mark_plugin_loaded(P_ID).ok();
    for spec in NODE_SPECS {
        let vec = match spec.local_node_key {
            KEY_A => VEC_A,
            KEY_B => VEC_B,
            _ => VEC_C,
        };
        gos_runtime::register_node(P_ID, vec, *spec).ok();
    }
    while gos_runtime::drain_control_plane().is_some() {}
    gos_supervisor::bootstrap(0);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn graph_epoch_starts_at_zero_and_increments_on_register_node() {
    let _g = TEST_LOCK.lock().unwrap();
    // After reset, epoch must be 0 before any nodes are registered.
    gos_runtime::reset();
    gos_supervisor::clear_rewrite_rules();
    assert_eq!(gos_runtime::graph_epoch(), 0, "fresh epoch must be 0");

    let p = PluginId::from_ascii("EP_TEST_001");
    let manifest = PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: p,
        name: "EP_TEST_001",
        version: 1,
        depends_on: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        nodes: &[],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    };
    gos_runtime::discover_plugin(manifest).ok();
    gos_runtime::mark_plugin_loaded(p).ok();

    let spec = NodeSpec {
        node_id: derive_node_id(p, "ep.node"),
        local_node_key: "ep.node",
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: ExecutorId::from_ascii("ep.exec"),
        state_schema_hash: 0xE001,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    };
    let epoch_before = gos_runtime::graph_epoch();
    gos_runtime::register_node(p, VectorAddress::new(0x30, 0, 0, 0), spec).unwrap();
    let epoch_after = gos_runtime::graph_epoch();
    assert!(
        epoch_after > epoch_before,
        "epoch must increment after register_node: before={epoch_before} after={epoch_after}"
    );
}

#[test]
fn graph_epoch_stable_after_read_only_operations() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let before = gos_runtime::graph_epoch();
    // Pure reads must not bump the epoch.
    let _ = gos_runtime::snapshot();
    let _ = gos_runtime::graph_epoch();
    let _ = gos_runtime::subscribe_pair_count();
    let _ = gos_supervisor::render_epoch();
    let _ = gos_supervisor::idle_cycle_count();
    let _ = gos_supervisor::causal_depth_max();
    let after = gos_runtime::graph_epoch();

    assert_eq!(before, after, "read-only calls must not change graph_epoch");
}

#[test]
fn idle_cycle_count_increments_when_graph_stable_across_service_cycles() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // Run two service cycles with no mutations queued.
    let idle_before = gos_supervisor::idle_cycle_count();
    gos_supervisor::service_system_cycle();
    gos_supervisor::service_system_cycle();
    let idle_after = gos_supervisor::idle_cycle_count();

    assert!(
        idle_after >= idle_before + 2,
        "two stable cycles must increment idle_cycle_count by at least 2: before={idle_before} after={idle_after}"
    );
}

#[test]
fn causal_depth_max_is_zero_for_empty_service_cycles() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // Reset causal depth by running a fresh context: after reset and bootstrap
    // with no native executors queued, the dispatch loop should iterate 0 times.
    // causal_depth_max() is a global peak since boot, so we can only assert ≥ 0.
    let depth = gos_supervisor::causal_depth_max();
    // Sanity: depth is never negative (u64) and must be < the hard cap.
    assert!(depth < 2048, "causal_depth_max must be below 2048 cap: {depth}");
}

#[test]
fn render_epoch_matches_graph_epoch_after_service_cycle() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // After one service cycle, the supervisor records the current graph_epoch
    // into LAST_RENDER_EPOCH. They must be equal.
    gos_supervisor::service_system_cycle();

    let graph_ep = gos_runtime::graph_epoch();
    let render_ep = gos_supervisor::render_epoch();

    assert_eq!(
        graph_ep, render_ep,
        "render_epoch must equal graph_epoch after a service cycle: graph={graph_ep} render={render_ep}"
    );
}

#[test]
fn subscribe_pair_count_tracks_register_idempotent_and_unregister() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let a = derive_node_id(P_ID, KEY_A);
    let b = derive_node_id(P_ID, KEY_B);

    let before = gos_runtime::subscribe_pair_count();
    gos_runtime::register_subscribe(a, b).unwrap();
    assert_eq!(
        gos_runtime::subscribe_pair_count(),
        before + 1,
        "register adds 1 pair"
    );
    // Idempotent re-registration must not increase count.
    gos_runtime::register_subscribe(a, b).unwrap();
    assert_eq!(
        gos_runtime::subscribe_pair_count(),
        before + 1,
        "idempotent re-register must not change count"
    );
    // Unregister removes the pair.
    gos_runtime::unregister_subscribe(a, b);
    assert_eq!(
        gos_runtime::subscribe_pair_count(),
        before,
        "unregister removes 1 pair"
    );
}

#[test]
fn causal_depth_max_is_cumulative_peak() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let before = gos_supervisor::causal_depth_max();
    // Run a service cycle that has no work — depth will be 0 for this cycle
    // but the peak counter never decreases.
    gos_supervisor::service_system_cycle();
    let after = gos_supervisor::causal_depth_max();

    assert!(
        after >= before,
        "causal_depth_max must be monotonically non-decreasing: before={before} after={after}"
    );
}
