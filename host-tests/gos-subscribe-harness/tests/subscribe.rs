// gos-subscribe harness — V2.3 reactive Subscribe index tests
//
// Verifies the Subscribe propagation contract introduced in V2.3:
//
//   1. `register_subscribe(observed, subscriber)` is idempotent.
//   2. When a structural mutation touches `observed` (register_edge),
//      the runtime emits `SubscribeTriggered` for each registered subscriber.
//   3. A mutation to a node that has NO subscriber emits no SubscribeTriggered.
//   4. Two subscribers to the same observed node both receive notification.
//   5. The `arg1` field of SubscribeTriggered equals the new graph_epoch.
//   6. Supervisor idle_cycle_count increments when epoch is stable across cycles.

use std::sync::Mutex;

use gos_protocol::{
    ControlPlaneMessageKind, EdgeSpec, EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId,
    NodeSpec, PluginId, PluginManifest, RoutePolicy, RuntimeEdgeType, RuntimeNodeType,
    VectorAddress, derive_edge_id, derive_node_id,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared test constants ─────────────────────────────────────────────────────

const P_ID: PluginId = PluginId::from_ascii("SUB_HARNESS");
const KEY_OBSERVED: &str = "sub.observed";
const KEY_SUBSCRIBER_A: &str = "sub.subscriber.a";
const KEY_SUBSCRIBER_B: &str = "sub.subscriber.b";
const KEY_UNRELATED: &str = "sub.unrelated";
const KEY_TARGET: &str = "sub.target";

const VEC_OBSERVED: VectorAddress = VectorAddress::new(0x10, 0, 0, 0);
const VEC_SUB_A: VectorAddress = VectorAddress::new(0x11, 0, 0, 0);
const VEC_SUB_B: VectorAddress = VectorAddress::new(0x12, 0, 0, 0);
const VEC_UNRELATED: VectorAddress = VectorAddress::new(0x13, 0, 0, 0);
const VEC_TARGET: VectorAddress = VectorAddress::new(0x14, 0, 0, 0);
const EXEC: ExecutorId = ExecutorId::from_ascii("sub.exec");

const NODE_SPECS: &[NodeSpec] = &[
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_OBSERVED),
        local_node_key: KEY_OBSERVED,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x0001,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_SUBSCRIBER_A),
        local_node_key: KEY_SUBSCRIBER_A,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x0002,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_SUBSCRIBER_B),
        local_node_key: KEY_SUBSCRIBER_B,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x0003,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_UNRELATED),
        local_node_key: KEY_UNRELATED,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x0004,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_TARGET),
        local_node_key: KEY_TARGET,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x0005,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
];

const MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: P_ID,
    name: "SUB_HARNESS",
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() {
    gos_runtime::reset();
    gos_supervisor::clear_rewrite_rules();
    gos_runtime::discover_plugin(MANIFEST).ok();
    gos_runtime::mark_plugin_loaded(P_ID).ok();
    for spec in NODE_SPECS {
        let vec = match spec.local_node_key {
            KEY_OBSERVED => VEC_OBSERVED,
            KEY_SUBSCRIBER_A => VEC_SUB_A,
            KEY_SUBSCRIBER_B => VEC_SUB_B,
            KEY_UNRELATED => VEC_UNRELATED,
            KEY_TARGET => VEC_TARGET,
            _ => VEC_TARGET,
        };
        gos_runtime::register_node(P_ID, vec, *spec).ok();
    }
    // Drain all boot telemetry (NodeUpsert, StateDelta, SubscribeTriggered
    // from boot registrations, etc.).
    while gos_runtime::drain_control_plane().is_some() {}
    gos_supervisor::bootstrap(0);
}

fn node(key: &str) -> NodeId {
    derive_node_id(P_ID, key)
}

fn make_edge_spec(from: NodeId, to: NodeId, key: &str) -> EdgeSpec {
    EdgeSpec {
        edge_id: derive_edge_id(from, to, key),
        from_node: from,
        to_node: to,
        edge_type: RuntimeEdgeType::Signal,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

fn collect_control_plane() -> Vec<(ControlPlaneMessageKind, [u8; 16], u64, u64)> {
    let mut items = Vec::new();
    while let Some(env) = gos_runtime::drain_control_plane() {
        items.push((env.kind, env.subject, env.arg0, env.arg1));
    }
    items
}

fn count_subscribe_triggered(items: &[(ControlPlaneMessageKind, [u8; 16], u64, u64)]) -> usize {
    items.iter().filter(|(k, _, _, _)| *k == ControlPlaneMessageKind::SubscribeTriggered).count()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn subscribe_pair_registration_succeeds_and_is_idempotent() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let sub_a = node(KEY_SUBSCRIBER_A);

    // First registration: Ok(())
    gos_runtime::register_subscribe(observed, sub_a).expect("first registration");
    // Second registration of the same pair: also Ok(()) — idempotent.
    gos_runtime::register_subscribe(observed, sub_a).expect("idempotent re-registration");
}

#[test]
fn subscribe_triggered_fires_when_observed_node_gets_new_edge() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let sub_a = node(KEY_SUBSCRIBER_A);
    let target = node(KEY_TARGET);

    gos_runtime::register_subscribe(observed, sub_a).unwrap();
    // Drain any subscription-registration telemetry (none expected, but be safe).
    while gos_runtime::drain_control_plane().is_some() {}

    // Structural mutation: add edge from observed → target.
    let spec = make_edge_spec(observed, target, "sub.e1");
    gos_runtime::register_edge(spec).expect("register_edge");

    let items = collect_control_plane();
    let triggered = count_subscribe_triggered(&items);

    // `observed` gained an outbound edge → at least one SubscribeTriggered expected.
    // (register_edge fires for from_node AND to_node; observed is from_node here.)
    assert!(
        triggered >= 1,
        "expected ≥1 SubscribeTriggered after register_edge touching observed; got {triggered}: {items:?}"
    );

    // Subject of the SubscribeTriggered must identify the observed node.
    let found = items.iter().any(|(k, subj, _, _)| {
        *k == ControlPlaneMessageKind::SubscribeTriggered && *subj == observed.0
    });
    assert!(found, "SubscribeTriggered subject must be observed node's NodeId");
}

#[test]
fn no_subscribe_triggered_for_mutation_to_non_observed_node() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let sub_a = node(KEY_SUBSCRIBER_A);
    let unrelated = node(KEY_UNRELATED);
    let target = node(KEY_TARGET);

    gos_runtime::register_subscribe(observed, sub_a).unwrap();
    while gos_runtime::drain_control_plane().is_some() {}

    // Mutation that touches UNRELATED (not observed).
    let spec = make_edge_spec(unrelated, target, "sub.e2");
    gos_runtime::register_edge(spec).expect("register_edge");

    let items = collect_control_plane();

    // No SubscribeTriggered whose subject = observed node should appear.
    let spurious = items.iter().any(|(k, subj, _, _)| {
        *k == ControlPlaneMessageKind::SubscribeTriggered && *subj == observed.0
    });
    assert!(
        !spurious,
        "must NOT emit SubscribeTriggered for observed when only unrelated node changed; got {items:?}"
    );
}

#[test]
fn multiple_subscribers_both_notified() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let sub_a = node(KEY_SUBSCRIBER_A);
    let sub_b = node(KEY_SUBSCRIBER_B);
    let target = node(KEY_TARGET);

    gos_runtime::register_subscribe(observed, sub_a).unwrap();
    gos_runtime::register_subscribe(observed, sub_b).unwrap();
    while gos_runtime::drain_control_plane().is_some() {}

    let spec = make_edge_spec(observed, target, "sub.e3");
    gos_runtime::register_edge(spec).expect("register_edge");

    let items = collect_control_plane();

    // Both sub_a and sub_b must appear as subscribers (arg0 encodes the subscriber).
    let arg0_sub_a = u64::from_le_bytes([
        sub_a.0[0], sub_a.0[1], sub_a.0[2], sub_a.0[3],
        sub_a.0[4], sub_a.0[5], sub_a.0[6], sub_a.0[7],
    ]);
    let arg0_sub_b = u64::from_le_bytes([
        sub_b.0[0], sub_b.0[1], sub_b.0[2], sub_b.0[3],
        sub_b.0[4], sub_b.0[5], sub_b.0[6], sub_b.0[7],
    ]);

    let has_sub_a = items.iter().any(|(k, subj, arg0, _)| {
        *k == ControlPlaneMessageKind::SubscribeTriggered
            && *subj == observed.0
            && *arg0 == arg0_sub_a
    });
    let has_sub_b = items.iter().any(|(k, subj, arg0, _)| {
        *k == ControlPlaneMessageKind::SubscribeTriggered
            && *subj == observed.0
            && *arg0 == arg0_sub_b
    });

    assert!(has_sub_a, "SubscribeTriggered missing for sub_a; items: {items:?}");
    assert!(has_sub_b, "SubscribeTriggered missing for sub_b; items: {items:?}");
}

#[test]
fn subscribe_triggered_arg1_is_new_graph_epoch() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let sub_a = node(KEY_SUBSCRIBER_A);
    let target = node(KEY_TARGET);

    gos_runtime::register_subscribe(observed, sub_a).unwrap();
    while gos_runtime::drain_control_plane().is_some() {}

    let epoch_before = gos_runtime::graph_epoch();
    let spec = make_edge_spec(observed, target, "sub.e4");
    gos_runtime::register_edge(spec).expect("register_edge");
    let epoch_after = gos_runtime::graph_epoch();

    // Epoch must have advanced.
    assert!(epoch_after > epoch_before, "epoch must advance after register_edge");

    let items = collect_control_plane();
    let triggered = items.iter().find(|(k, subj, _, _)| {
        *k == ControlPlaneMessageKind::SubscribeTriggered && *subj == observed.0
    });

    let (_, _, _, arg1) = triggered.expect("SubscribeTriggered must be emitted");
    assert_eq!(
        *arg1, epoch_after,
        "SubscribeTriggered arg1 must equal new graph_epoch ({epoch_after}); got {arg1}"
    );
}

#[test]
fn idle_cycle_count_increments_when_graph_stable() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // After setup, graph_epoch has a fixed value.  No more structural mutations.
    // Run several cycles — each should see a stable epoch and increment idle_count.
    let idle_before = gos_supervisor::idle_cycle_count();

    gos_supervisor::service_system_cycle();
    gos_supervisor::service_system_cycle();
    gos_supervisor::service_system_cycle();

    let idle_after = gos_supervisor::idle_cycle_count();

    // At least 2 cycles must have been idle (first cycle after setup updates
    // LAST_RENDER_EPOCH, subsequent cycles are truly idle).
    assert!(
        idle_after >= idle_before + 2,
        "expected idle_cycle_count to grow by ≥2 over 3 stable cycles; before={idle_before} after={idle_after}"
    );
}
