// gos-rewrite integration harness — V2.2 supervisor wire-up + quiescence tests
//
// Verifies that the RewriteEngine is correctly integrated into the supervisor's
// `service_system_cycle` and that the quiescence contract holds:
//
//  1. `add_rewrite_rule` / `clear_rewrite_rules` round-trip.
//  2. `service_system_cycle` emits `RuleApplied` when a rule fires.
//  3. An `EdgeAbsent` rule quiesces in exactly ONE cycle (fires on cycle 1,
//     stays silent on cycle 2 once the edge exists).
//  4. An `Always` rule fires on every cycle (no quiescence).
//  5. No rules → no `RuleApplied` envelope.

use std::sync::Mutex;

use gos_cypher_mut::{CypherMutation, ReceptiveEdgeKind};
use gos_protocol::{
    ControlPlaneMessageKind, EntryPolicy, ExecutorId, GOS_ABI_VERSION, NodeId, NodeSpec,
    PluginId, PluginManifest, RuntimeNodeType, VectorAddress, derive_node_id, fixed_bytes_16,
};
use gos_rewrite::{RewriteAction, RewritePattern, RewriteRule};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared test constants ─────────────────────────────────────────────────────

const P_ID: PluginId = PluginId::from_ascii("REWRITE_INTEG");
const NODE_A_KEY: &str = "ri.node.a";
const NODE_B_KEY: &str = "ri.node.b";
const VEC_A: VectorAddress = VectorAddress::new(0xA0, 0, 0, 0);
const VEC_B: VectorAddress = VectorAddress::new(0xB0, 0, 0, 0);
const EXEC: ExecutorId = ExecutorId::from_ascii("ri.exec");

const NODE_SPECS: &[NodeSpec] = &[
    NodeSpec {
        node_id: derive_node_id(P_ID, NODE_A_KEY),
        local_node_key: NODE_A_KEY,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xA0A0,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
    NodeSpec {
        node_id: derive_node_id(P_ID, NODE_B_KEY),
        local_node_key: NODE_B_KEY,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0xB0B0,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    },
];

const MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: P_ID,
    name: "REWRITE_INTEG",
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

// ── Test helpers ──────────────────────────────────────────────────────────────

fn setup() {
    gos_runtime::reset();
    gos_supervisor::clear_rewrite_rules();
    gos_runtime::discover_plugin(MANIFEST).ok();
    gos_runtime::mark_plugin_loaded(P_ID).ok();
    gos_runtime::register_node(P_ID, VEC_A, NODE_SPECS[0]).ok();
    gos_runtime::register_node(P_ID, VEC_B, NODE_SPECS[1]).ok();
    // Drain boot telemetry.
    while gos_runtime::drain_control_plane().is_some() {}
    gos_supervisor::bootstrap(0);
}

fn node_a() -> NodeId {
    derive_node_id(P_ID, NODE_A_KEY)
}

fn node_b() -> NodeId {
    derive_node_id(P_ID, NODE_B_KEY)
}

fn make_action(from: NodeId, to: NodeId) -> RewriteAction {
    RewriteAction {
        mutation: CypherMutation::AddEdge { from, to, edge_kind: ReceptiveEdgeKind::Mount },
        source: fixed_bytes_16("REWRITE_INTEG"),
    }
}

fn collect_control_plane() -> Vec<ControlPlaneMessageKind> {
    let mut kinds = Vec::new();
    while let Some(env) = gos_runtime::drain_control_plane() {
        kinds.push(env.kind);
    }
    kinds
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn add_and_clear_rules_roundtrip() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let rule = RewriteRule {
        pattern: RewritePattern::Always,
        guard: None,
        action: make_action(node_a(), node_b()),
        label: fixed_bytes_16("ri.always"),
    };
    let idx = gos_supervisor::add_rewrite_rule(rule).expect("add rule");
    assert_eq!(idx, 0);

    gos_supervisor::clear_rewrite_rules();
    // After clear, the engine slot 0 is available again.
    let idx2 = gos_supervisor::add_rewrite_rule(rule).expect("add after clear");
    assert_eq!(idx2, 0);
    gos_supervisor::clear_rewrite_rules();
}

#[test]
fn service_cycle_always_rule_emits_rule_applied() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let rule = RewriteRule {
        pattern: RewritePattern::Always,
        guard: None,
        action: make_action(node_a(), node_b()),
        label: fixed_bytes_16("ri.always.emit"),
    };
    gos_supervisor::add_rewrite_rule(rule).unwrap();

    gos_supervisor::service_system_cycle();
    let kinds = collect_control_plane();

    assert!(
        kinds.contains(&ControlPlaneMessageKind::RuleApplied),
        "Always rule must emit RuleApplied; got {:?}",
        kinds
    );
    gos_supervisor::clear_rewrite_rules();
}

#[test]
fn edge_absent_rule_quiesces_after_one_cycle() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    let a = node_a();
    let b = node_b();

    // Rule fires while Mount edge a→b is absent; the action creates it.
    // After cycle 1 the edge exists, so the pattern no longer matches → quiescence.
    let rule = RewriteRule {
        pattern: RewritePattern::EdgeAbsent { from: a, to: b, kind: ReceptiveEdgeKind::Mount },
        guard: None,
        action: make_action(a, b),
        label: fixed_bytes_16("ri.edgeabsent"),
    };
    gos_supervisor::add_rewrite_rule(rule).unwrap();

    // Cycle 1: edge absent → rule fires.
    gos_supervisor::service_system_cycle();
    let kinds_1 = collect_control_plane();
    assert!(
        kinds_1.contains(&ControlPlaneMessageKind::RuleApplied),
        "cycle 1: EdgeAbsent rule must fire (edge missing); got {:?}",
        kinds_1
    );

    // Cycle 2: edge now present → rule does NOT fire.
    gos_supervisor::service_system_cycle();
    let kinds_2 = collect_control_plane();
    assert!(
        !kinds_2.contains(&ControlPlaneMessageKind::RuleApplied),
        "cycle 2: EdgeAbsent rule must be quiescent (edge exists); got {:?}",
        kinds_2
    );

    gos_supervisor::clear_rewrite_rules();
}

#[test]
fn always_rule_fires_every_cycle() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // AddEdge is idempotent after the first insertion (duplicate edges are
    // no-ops in gos-runtime), so RuleApplied is emitted every cycle even
    // though the graph stops changing structurally after cycle 1.
    let rule = RewriteRule {
        pattern: RewritePattern::Always,
        guard: None,
        action: make_action(node_a(), node_b()),
        label: fixed_bytes_16("ri.always.3x"),
    };
    gos_supervisor::add_rewrite_rule(rule).unwrap();

    for cycle in 0..3u32 {
        gos_supervisor::service_system_cycle();
        let kinds = collect_control_plane();
        assert!(
            kinds.contains(&ControlPlaneMessageKind::RuleApplied),
            "cycle {cycle}: Always rule must fire every cycle; got {:?}",
            kinds
        );
    }
    gos_supervisor::clear_rewrite_rules();
}

#[test]
fn no_rules_no_rule_applied_envelope() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    gos_supervisor::service_system_cycle();
    let kinds = collect_control_plane();
    assert!(
        !kinds.contains(&ControlPlaneMessageKind::RuleApplied),
        "no rules → no RuleApplied; got {:?}",
        kinds
    );
}

#[test]
fn epoch_gt_rule_fires_when_epoch_exceeds_threshold() {
    let _g = TEST_LOCK.lock().unwrap();
    setup();

    // setup() registers 2 nodes → epoch is at least 2.
    // EpochGt(0) must fire immediately.
    let rule = RewriteRule {
        pattern: RewritePattern::EpochGt(0),
        guard: None,
        action: make_action(node_a(), node_b()),
        label: fixed_bytes_16("ri.epochgt"),
    };
    gos_supervisor::add_rewrite_rule(rule).unwrap();

    gos_supervisor::service_system_cycle();
    let kinds = collect_control_plane();
    assert!(
        kinds.contains(&ControlPlaneMessageKind::RuleApplied),
        "EpochGt(0) must fire when epoch > 0; got {:?}",
        kinds
    );
    gos_supervisor::clear_rewrite_rules();
}
