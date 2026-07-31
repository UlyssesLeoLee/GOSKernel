// gos-rewrite host harness — V2.2 RewriteEngine contract tests
//
// Verifies the `match pattern → guard → emit` engine in isolation using a
// `MockGraphView`.  No kernel or runtime required.

use gos_cypher_mut::{CypherMutation, ReceptiveEdgeKind};
use gos_protocol::{fixed_bytes_16, GraphSnapshot, NodeId};
use gos_rewrite::{
    GraphView, RewriteAction, RewriteEngine, RewriteError, RewritePattern, RewriteRule,
    MAX_REWRITE_RULES,
};

// ── MockGraphView ─────────────────────────────────────────────────────────────

struct MockGraphView {
    nodes: Vec<NodeId>,
    edges: Vec<(NodeId, NodeId, ReceptiveEdgeKind)>,
    epoch: u64,
}

impl MockGraphView {
    fn empty() -> Self {
        Self { nodes: Vec::new(), edges: Vec::new(), epoch: 0 }
    }

    fn with_node(mut self, id: NodeId) -> Self {
        self.nodes.push(id);
        self
    }

    fn with_edge(mut self, from: NodeId, to: NodeId, kind: ReceptiveEdgeKind) -> Self {
        self.edges.push((from, to, kind));
        self
    }

    fn with_epoch(mut self, epoch: u64) -> Self {
        self.epoch = epoch;
        self
    }
}

impl GraphView for MockGraphView {
    fn node_exists(&self, id: NodeId) -> bool {
        self.nodes.iter().any(|n| n.0 == id.0)
    }

    fn edge_exists(&self, from: NodeId, to: NodeId, kind: ReceptiveEdgeKind) -> bool {
        self.edges.iter().any(|(f, t, k)| f.0 == from.0 && t.0 == to.0 && *k == kind)
    }

    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            plugin_count: 0,
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            ready_queue_len: 0,
            signal_queue_len: 0,
            control_queue_len: 0,
            tick: 0,
            graph_epoch: self.epoch,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn node(tag: &str) -> NodeId {
    NodeId(fixed_bytes_16(tag))
}

fn action_add_edge(from: NodeId, to: NodeId) -> RewriteAction {
    RewriteAction {
        mutation: CypherMutation::AddEdge { from, to, edge_kind: ReceptiveEdgeKind::Mount },
        source: *b"REWRITE_TEST\0\0\0\0",
    }
}

fn rule_always(from: NodeId, to: NodeId) -> RewriteRule {
    RewriteRule {
        pattern: RewritePattern::Always,
        guard: None,
        action: action_add_edge(from, to),
        label: *b"rule.always\0\0\0\0\0",
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn new_engine_starts_empty() {
    let engine = RewriteEngine::new();
    assert_eq!(engine.rule_count(), 0);
    assert_eq!(engine.fire_count, 0);
}

#[test]
fn add_rule_increments_rule_count() {
    let mut engine = RewriteEngine::new();
    let a = node("node_a");
    let b = node("node_b");
    let idx = engine.add_rule(rule_always(a, b)).expect("first rule");
    assert_eq!(idx, 0);
    assert_eq!(engine.rule_count(), 1);

    let idx2 = engine.add_rule(rule_always(b, a)).expect("second rule");
    assert_eq!(idx2, 1);
    assert_eq!(engine.rule_count(), 2);
}

#[test]
fn rule_table_full_returns_error() {
    let mut engine = RewriteEngine::new();
    let a = node("node_a");
    let b = node("node_b");
    for _ in 0..MAX_REWRITE_RULES {
        engine.add_rule(rule_always(a, b)).expect("fill table");
    }
    let err = engine.add_rule(rule_always(a, b)).expect_err("should be full");
    assert_eq!(err, RewriteError::RuleTableFull);
}

#[test]
fn always_pattern_fires_unconditionally() {
    let mut engine = RewriteEngine::new();
    let a = node("always_a");
    let b = node("always_b");
    engine.add_rule(rule_always(a, b)).unwrap();

    let view = MockGraphView::empty();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];
    let fired = engine.apply_rules(&view, &mut out);

    assert_eq!(fired, 1);
    assert_eq!(engine.fire_count, 1);
    let (idx, action) = out[0];
    assert_eq!(idx, 0);
    assert!(matches!(action.mutation, CypherMutation::AddEdge { from, to, .. } if from.0 == a.0 && to.0 == b.0));
}

#[test]
fn node_present_fires_only_when_node_exists() {
    let a = node("nod_a");
    let b = node("nod_b");

    let rule = RewriteRule {
        pattern: RewritePattern::NodePresent(a),
        guard: None,
        action: action_add_edge(a, b),
        label: *b"rule.nodepresent",
    };

    // Without node: should NOT fire.
    let mut engine = RewriteEngine::new();
    engine.add_rule(rule).unwrap();
    let view_absent = MockGraphView::empty();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];
    assert_eq!(engine.apply_rules(&view_absent, &mut out), 0);
    assert_eq!(engine.fire_count, 0);

    // With node: should fire.
    let view_present = MockGraphView::empty().with_node(a);
    let fired = engine.apply_rules(&view_present, &mut out);
    assert_eq!(fired, 1);
    assert_eq!(engine.fire_count, 1);
}

#[test]
fn edge_absent_fires_when_edge_missing() {
    let a = node("ea_a");
    let b = node("ea_b");

    let rule = RewriteRule {
        pattern: RewritePattern::EdgeAbsent { from: a, to: b, kind: ReceptiveEdgeKind::Mount },
        guard: None,
        action: action_add_edge(a, b),
        label: *b"rule.edgeabsent\0",
    };

    let mut engine = RewriteEngine::new();
    engine.add_rule(rule).unwrap();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];

    // Edge absent → should fire.
    let view_no_edge = MockGraphView::empty().with_node(a).with_node(b);
    assert_eq!(engine.apply_rules(&view_no_edge, &mut out), 1);

    // Edge present → should NOT fire.
    let view_with_edge = view_no_edge.with_edge(a, b, ReceptiveEdgeKind::Mount);
    assert_eq!(engine.apply_rules(&view_with_edge, &mut out), 0);
}

#[test]
fn edge_present_fires_when_edge_exists() {
    let a = node("ep_a");
    let b = node("ep_b");

    let rule = RewriteRule {
        pattern: RewritePattern::EdgePresent { from: a, to: b, kind: ReceptiveEdgeKind::Use },
        guard: None,
        action: action_add_edge(a, b),
        label: *b"rule.edgepresent",
    };

    let mut engine = RewriteEngine::new();
    engine.add_rule(rule).unwrap();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];

    // No edge → should NOT fire.
    let view_no_edge = MockGraphView::empty();
    assert_eq!(engine.apply_rules(&view_no_edge, &mut out), 0);

    // Edge present → should fire.
    let view_with_edge = MockGraphView::empty().with_edge(a, b, ReceptiveEdgeKind::Use);
    assert_eq!(engine.apply_rules(&view_with_edge, &mut out), 1);
}

#[test]
fn epoch_gt_fires_only_above_threshold() {
    let a = node("eg_a");
    let b = node("eg_b");

    let rule = RewriteRule {
        pattern: RewritePattern::EpochGt(5),
        guard: None,
        action: action_add_edge(a, b),
        label: *b"rule.epochgt\0\0\0\0",
    };

    let mut engine = RewriteEngine::new();
    engine.add_rule(rule).unwrap();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];

    // epoch == 5: NOT > 5, should NOT fire.
    assert_eq!(engine.apply_rules(&MockGraphView::empty().with_epoch(5), &mut out), 0);
    // epoch == 6: > 5, should fire.
    assert_eq!(engine.apply_rules(&MockGraphView::empty().with_epoch(6), &mut out), 1);
}

#[test]
fn epoch_eq_fires_at_exact_epoch() {
    let a = node("ee_a");
    let b = node("ee_b");

    let rule = RewriteRule {
        pattern: RewritePattern::EpochEq(42),
        guard: None,
        action: action_add_edge(a, b),
        label: *b"rule.epocheq\0\0\0\0",
    };

    let mut engine = RewriteEngine::new();
    engine.add_rule(rule).unwrap();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];

    assert_eq!(engine.apply_rules(&MockGraphView::empty().with_epoch(41), &mut out), 0);
    assert_eq!(engine.apply_rules(&MockGraphView::empty().with_epoch(42), &mut out), 1);
    assert_eq!(engine.apply_rules(&MockGraphView::empty().with_epoch(43), &mut out), 0);
}

#[test]
fn guard_fn_vetoes_matching_pattern() {
    let a = node("guard_a");
    let b = node("guard_b");

    fn always_false(_snap: &GraphSnapshot) -> bool {
        false
    }
    fn always_true(_snap: &GraphSnapshot) -> bool {
        true
    }

    let rule_vetoed = RewriteRule {
        pattern: RewritePattern::Always,
        guard: Some(always_false),
        action: action_add_edge(a, b),
        label: *b"rule.vetoed\0\0\0\0\0",
    };
    let rule_allowed = RewriteRule {
        pattern: RewritePattern::Always,
        guard: Some(always_true),
        action: action_add_edge(b, a),
        label: *b"rule.allowed\0\0\0\0",
    };

    let mut engine = RewriteEngine::new();
    engine.add_rule(rule_vetoed).unwrap();
    engine.add_rule(rule_allowed).unwrap();

    let view = MockGraphView::empty();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];
    let fired = engine.apply_rules(&view, &mut out);

    assert_eq!(fired, 1, "only the guard-passing rule fires");
    let (idx, _) = out[0];
    assert_eq!(idx, 1, "rule at index 1 (allowed) fires, not index 0 (vetoed)");
    assert_eq!(engine.fire_count, 1);
}

#[test]
fn multiple_rules_fire_in_same_cycle() {
    let a = node("multi_a");
    let b = node("multi_b");
    let c = node("multi_c");

    let mut engine = RewriteEngine::new();
    engine.add_rule(RewriteRule {
        pattern: RewritePattern::NodePresent(a),
        guard: None,
        action: action_add_edge(a, b),
        label: *b"rule.multi.0\0\0\0\0",
    }).unwrap();
    engine.add_rule(RewriteRule {
        pattern: RewritePattern::NodePresent(b),
        guard: None,
        action: action_add_edge(b, c),
        label: *b"rule.multi.1\0\0\0\0",
    }).unwrap();
    engine.add_rule(RewriteRule {
        pattern: RewritePattern::NodePresent(c),
        guard: None,
        action: action_add_edge(c, a),
        label: *b"rule.multi.2\0\0\0\0",
    }).unwrap();

    // All three nodes present → all three rules fire.
    let view = MockGraphView::empty().with_node(a).with_node(b).with_node(c);
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];
    let fired = engine.apply_rules(&view, &mut out);

    assert_eq!(fired, 3);
    assert_eq!(engine.fire_count, 3);
    assert_eq!(out[0].0, 0);
    assert_eq!(out[1].0, 1);
    assert_eq!(out[2].0, 2);
}

#[test]
fn clear_resets_engine_to_empty() {
    let mut engine = RewriteEngine::new();
    let a = node("cl_a");
    let b = node("cl_b");
    engine.add_rule(rule_always(a, b)).unwrap();
    engine.add_rule(rule_always(b, a)).unwrap();

    let view = MockGraphView::empty();
    let mut out = [(0, action_add_edge(a, b)); MAX_REWRITE_RULES];
    engine.apply_rules(&view, &mut out);

    engine.clear();
    assert_eq!(engine.rule_count(), 0);
    assert_eq!(engine.fire_count, 0);
    assert_eq!(engine.apply_rules(&view, &mut out), 0);
}
