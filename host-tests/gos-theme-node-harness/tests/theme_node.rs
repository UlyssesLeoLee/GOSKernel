// gos-theme-node-harness — V2.15 Theme Palette Nodes + Subscribe Auto-Repaint tests
//
// Verifies the V2.15 reactive theme mechanism:
//
//  1. register_node_prop_u8 roundtrip: stored val is retrievable via active_use_target chain.
//  2. active_use_target returns None when no Use edge exists from source.
//  3. active_use_target returns the correct target after a Use edge is registered.
//  4. active_use_target ignores non-Use edges (Mount, Signal) from the same source.
//  5. fire_subscribers posts Signal::Control{cmd=SUBSCRIBE_TRIGGERED} to subscriber queue.
//  6. Signal val equals the registered node prop of the active Use-edge target.
//  7. Signal val is 0 when no node prop is registered for the Use-edge target.
//  8. Signal val updates when the Use-edge target switches (wabi → shoji).
//  9. register_node_prop_u8 overwrites when same NodeId re-registered.
// 10. register_node_prop_u8 returns false when the 16-slot table is full.

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeSpec, PluginId, PluginManifest, RoutePolicy, RuntimeEdgeType, RuntimeNodeType,
    Signal, VectorAddress, DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED, DISPLAY_THEME_SHOJI,
    DISPLAY_THEME_WABI,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const P_ID: PluginId = PluginId::from_ascii("THEME_HARN");
const EXEC: ExecutorId = ExecutorId::from_ascii("theme.exec");

const KEY_OBSERVED: &str = "theme.current";
const KEY_WABI: &str = "theme.wabi";
const KEY_SHOJI: &str = "theme.shoji";
const KEY_SUBSCRIBER: &str = "vga.entry";
const KEY_UNRELATED: &str = "theme.unrelated";

const VEC_OBSERVED: VectorAddress = VectorAddress::new(0x20, 0, 0, 0);
const VEC_WABI: VectorAddress = VectorAddress::new(0x21, 0, 0, 0);
const VEC_SHOJI: VectorAddress = VectorAddress::new(0x22, 0, 0, 0);
const VEC_SUBSCRIBER: VectorAddress = VectorAddress::new(0x23, 0, 0, 0);
const VEC_UNRELATED: VectorAddress = VectorAddress::new(0x24, 0, 0, 0);

const NODE_SPECS: &[NodeSpec] = &[
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_OBSERVED),
        local_node_key: KEY_OBSERVED,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x2001,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_WABI),
        local_node_key: KEY_WABI,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x2002,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_SHOJI),
        local_node_key: KEY_SHOJI,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x2003,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, KEY_SUBSCRIBER),
        local_node_key: KEY_SUBSCRIBER,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0x2004,
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
        state_schema_hash: 0x2005,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
];

const MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: P_ID,
    name: "THEME_HARN",
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
    let vecs = [VEC_OBSERVED, VEC_WABI, VEC_SHOJI, VEC_SUBSCRIBER, VEC_UNRELATED];
    for (spec, vec) in NODE_SPECS.iter().zip(vecs.iter()) {
        gos_runtime::register_node(P_ID, *vec, *spec).ok();
    }
    while gos_runtime::drain_control_plane().is_some() {}
    while gos_runtime::drain_signal().is_some() {}
    gos_supervisor::bootstrap(0);
}

fn node(key: &str) -> NodeId {
    derive_node_id(P_ID, key)
}

fn use_edge(from: NodeId, to: NodeId, key: &str) -> EdgeSpec {
    EdgeSpec {
        edge_id: derive_edge_id(from, to, key),
        from_node: from,
        to_node: to,
        edge_type: RuntimeEdgeType::Use,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

fn non_use_edge(from: NodeId, to: NodeId, etype: RuntimeEdgeType, key: &str) -> EdgeSpec {
    EdgeSpec {
        edge_id: derive_edge_id(from, to, key),
        from_node: from,
        to_node: to,
        edge_type: etype,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

fn drain_all_signals() -> Vec<(VectorAddress, Signal)> {
    let mut out = Vec::new();
    while let Some(item) = gos_runtime::drain_control_signal() {
        out.push(item);
    }
    while let Some(item) = gos_runtime::drain_signal() {
        out.push(item);
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn active_use_target_none_when_no_use_edge() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let result = gos_runtime::active_use_target(observed);
    assert!(result.is_none(), "must return None when no Use edge from observed; got {result:?}");
}

#[test]
fn active_use_target_returns_correct_node_after_use_edge() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);

    gos_runtime::register_edge(use_edge(observed, wabi, "t2.wabi")).unwrap();
    while gos_runtime::drain_control_plane().is_some() {}
    while gos_runtime::drain_signal().is_some() {}

    let target = gos_runtime::active_use_target(observed);
    assert_eq!(target, Some(wabi), "active_use_target must return wabi node; got {target:?}");
}

#[test]
fn active_use_target_ignores_non_use_edges() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);

    // Register a Signal-type edge (NOT Use)
    gos_runtime::register_edge(non_use_edge(observed, wabi, RuntimeEdgeType::Signal, "t3.sig")).unwrap();
    while gos_runtime::drain_control_plane().is_some() {}
    while gos_runtime::drain_signal().is_some() {}

    let target = gos_runtime::active_use_target(observed);
    assert!(
        target.is_none(),
        "active_use_target must return None when only non-Use edges exist; got {target:?}"
    );
}

#[test]
fn node_prop_u8_roundtrip_via_subscribe_signal_val() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);
    let subscriber = node(KEY_SUBSCRIBER);

    // Register wabi's property and a subscribe pair
    assert!(gos_runtime::register_node_prop_u8(wabi, DISPLAY_THEME_WABI));
    gos_runtime::register_subscribe(observed, subscriber).unwrap();
    while gos_runtime::drain_signal().is_some() {}

    // Trigger fire_subscribers by adding a Use edge observed → wabi
    gos_runtime::register_edge(use_edge(observed, wabi, "t4.wabi")).unwrap();

    let signals = drain_all_signals();
    let triggered = signals.iter().find(|(vec, sig)| {
        *vec == VEC_SUBSCRIBER
            && matches!(sig, Signal::Control { cmd, val }
                if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED && *val == DISPLAY_THEME_WABI)
    });
    assert!(
        triggered.is_some(),
        "must deliver SUBSCRIBE_TRIGGERED with val=WABI to subscriber; signals: {signals:?}"
    );
}

#[test]
fn subscribe_signal_val_zero_when_no_prop_registered() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);
    let subscriber = node(KEY_SUBSCRIBER);

    // Do NOT register a node prop for wabi
    gos_runtime::register_subscribe(observed, subscriber).unwrap();
    while gos_runtime::drain_signal().is_some() {}

    gos_runtime::register_edge(use_edge(observed, wabi, "t5.wabi")).unwrap();

    let signals = drain_all_signals();
    let triggered = signals.iter().find(|(vec, sig)| {
        *vec == VEC_SUBSCRIBER
            && matches!(sig, Signal::Control { cmd, val }
                if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED && *val == 0)
    });
    assert!(
        triggered.is_some(),
        "val must be 0 when no prop registered for Use-edge target; signals: {signals:?}"
    );
}

#[test]
fn subscribe_signal_val_updates_on_use_edge_switch() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);
    let shoji = node(KEY_SHOJI);
    let subscriber = node(KEY_SUBSCRIBER);

    assert!(gos_runtime::register_node_prop_u8(wabi, DISPLAY_THEME_WABI));
    assert!(gos_runtime::register_node_prop_u8(shoji, DISPLAY_THEME_SHOJI));
    gos_runtime::register_subscribe(observed, subscriber).unwrap();
    while gos_runtime::drain_signal().is_some() {}

    // First: Use edge to wabi — expect val=WABI
    gos_runtime::register_edge(use_edge(observed, wabi, "t6.wabi")).unwrap();
    let sigs_wabi = drain_all_signals();
    let wabi_triggered = sigs_wabi.iter().any(|(vec, sig)| {
        *vec == VEC_SUBSCRIBER
            && matches!(sig, Signal::Control { cmd, val }
                if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED && *val == DISPLAY_THEME_WABI)
    });
    assert!(wabi_triggered, "first switch must deliver val=WABI; signals: {sigs_wabi:?}");

    // Remove wabi edge, add shoji edge — expect val=SHOJI
    let wabi_edge_id = derive_edge_id(observed, wabi, "t6.wabi");
    gos_runtime::unregister_edge(wabi_edge_id).unwrap();
    while gos_runtime::drain_signal().is_some() {}

    gos_runtime::register_edge(use_edge(observed, shoji, "t6.shoji")).unwrap();
    let sigs_shoji = drain_all_signals();
    let shoji_triggered = sigs_shoji.iter().any(|(vec, sig)| {
        *vec == VEC_SUBSCRIBER
            && matches!(sig, Signal::Control { cmd, val }
                if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED && *val == DISPLAY_THEME_SHOJI)
    });
    assert!(shoji_triggered, "second switch must deliver val=SHOJI; signals: {sigs_shoji:?}");
}

#[test]
fn node_prop_u8_overwrite_updates_existing_entry() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);
    let subscriber = node(KEY_SUBSCRIBER);

    // Register with val=0 (WABI), then overwrite with val=99
    assert!(gos_runtime::register_node_prop_u8(wabi, DISPLAY_THEME_WABI));
    assert!(gos_runtime::register_node_prop_u8(wabi, 99), "overwrite must succeed");

    gos_runtime::register_subscribe(observed, subscriber).unwrap();
    while gos_runtime::drain_signal().is_some() {}

    gos_runtime::register_edge(use_edge(observed, wabi, "t7.wabi")).unwrap();

    let signals = drain_all_signals();
    let triggered = signals.iter().find(|(vec, sig)| {
        *vec == VEC_SUBSCRIBER
            && matches!(sig, Signal::Control { cmd, val }
                if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED && *val == 99)
    });
    assert!(
        triggered.is_some(),
        "overwritten prop val (99) must be delivered; signals: {signals:?}"
    );
}

#[test]
fn node_prop_u8_table_full_returns_false() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // Fill all 16 slots with distinct NodeIds.
    // gos_runtime::MAX_NODE_PROPS_U8 is 16.
    for i in 0u8..16 {
        let fake_id = NodeId([i, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(
            gos_runtime::register_node_prop_u8(fake_id, i),
            "slot {i} must succeed"
        );
    }
    // The 17th unique NodeId must return false.
    let overflow_id = NodeId([0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let result = gos_runtime::register_node_prop_u8(overflow_id, 0xFF);
    assert!(!result, "must return false when node-prop table is full (16 slots)");
}

#[test]
fn subscribe_triggered_not_delivered_without_subscribe_pair() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);

    assert!(gos_runtime::register_node_prop_u8(wabi, DISPLAY_THEME_WABI));
    // No register_subscribe call
    while gos_runtime::drain_signal().is_some() {}

    gos_runtime::register_edge(use_edge(observed, wabi, "t9.wabi")).unwrap();

    let signals = drain_all_signals();
    let spurious = signals.iter().any(|(_, sig)| {
        matches!(sig, Signal::Control { cmd, .. } if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED)
    });
    assert!(
        !spurious,
        "must NOT deliver SUBSCRIBE_TRIGGERED without a registered subscribe pair; signals: {signals:?}"
    );
}

#[test]
fn subscribe_signal_delivered_to_correct_subscriber_vector() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let observed = node(KEY_OBSERVED);
    let wabi = node(KEY_WABI);
    let subscriber = node(KEY_SUBSCRIBER);
    let _unrelated = node(KEY_UNRELATED);

    assert!(gos_runtime::register_node_prop_u8(wabi, DISPLAY_THEME_WABI));
    gos_runtime::register_subscribe(observed, subscriber).unwrap();
    while gos_runtime::drain_signal().is_some() {}

    gos_runtime::register_edge(use_edge(observed, wabi, "t10.wabi")).unwrap();

    let signals = drain_all_signals();
    // Must go to VEC_SUBSCRIBER, NOT VEC_UNRELATED
    let to_subscriber = signals.iter().any(|(vec, sig)| {
        *vec == VEC_SUBSCRIBER
            && matches!(sig, Signal::Control { cmd, .. } if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED)
    });
    let to_unrelated = signals.iter().any(|(vec, sig)| {
        *vec == VEC_UNRELATED
            && matches!(sig, Signal::Control { cmd, .. } if *cmd == DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED)
    });
    assert!(to_subscriber, "SUBSCRIBE_TRIGGERED must reach VEC_SUBSCRIBER; signals: {signals:?}");
    assert!(!to_unrelated, "SUBSCRIBE_TRIGGERED must NOT reach VEC_UNRELATED; signals: {signals:?}");
}
