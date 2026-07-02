#![no_std]

// CYPHER GRAPH STRUCTURE:
// CREATE (RewriteEngine)-[:Contains]->(RewriteRule)
// CREATE (RewriteRule)-[:Matches]->(RewritePattern)
// CREATE (RewriteRule)-[:Guards]->(GuardFn)
// CREATE (RewriteRule)-[:Emits]->(RewriteAction)
// CREATE (RewriteEngine)-[:QueriesVia]->(GraphView)
// CREATE (RewriteAction)-[:Wraps]->(CypherMutation)

//! V2.2 Rewrite Engine — `match pattern → guard → emit` skeleton.
//!
//! A `RewriteRule` combines three parts:
//!   * `RewritePattern`  — structural predicate on the current graph state
//!   * `GuardFn`         — additional snapshot-level predicate (epoch, depth…)
//!   * `RewriteAction`   — the `CypherMutation` + source attestation to emit
//!
//! `RewriteEngine::apply_rules` evaluates every registered rule against a
//! `GraphView`, collects the actions of all firing rules into a caller-owned
//! output slice, and returns how many rules fired.  The supervisor feeds each
//! action to `gos_runtime::apply_cypher_mutation` and then calls
//! `gos_runtime::emit_rule_applied` for telemetry.
//!
//! All types are `no_std`, `Copy`, and const-constructible so the engine can
//! live in a static `Mutex<RewriteEngine>` on the kernel heap.

use gos_cypher_mut::{CypherMutation, ReceptiveEdgeKind};
use gos_protocol::{GraphSnapshot, NodeId};

pub const MAX_REWRITE_RULES: usize = 64;

// ── Pattern ──────────────────────────────────────────────────────────────────

/// Structural predicate over the current graph state.
/// Evaluated by `RewriteEngine::apply_rules` via a `GraphView`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewritePattern {
    /// Matches when a specific node is present (by stable `NodeId`).
    NodePresent(NodeId),
    /// Matches when no edge of `kind` exists between `from` and `to`.
    EdgeAbsent {
        from: NodeId,
        to: NodeId,
        kind: ReceptiveEdgeKind,
    },
    /// Matches when an edge of `kind` exists between `from` and `to`.
    EdgePresent {
        from: NodeId,
        to: NodeId,
        kind: ReceptiveEdgeKind,
    },
    /// Matches when the graph epoch is strictly greater than `threshold`.
    /// Useful for "fire once after N structural changes" rules.
    EpochGt(u64),
    /// Matches when the graph epoch equals `value` exactly.
    EpochEq(u64),
    /// Always matches — unconditional trigger rule.
    Always,
}

// ── Guard ─────────────────────────────────────────────────────────────────────

/// Called after the pattern matches; receives the current graph snapshot for
/// additional runtime checks (queue depth, tick count, etc.).
/// Must be a plain function pointer so it is `Copy` and `const`-constructible.
pub type GuardFn = fn(&GraphSnapshot) -> bool;

// ── Action ────────────────────────────────────────────────────────────────────

/// Emitted when a rule fires — the mutation to apply plus the source label used
/// for audit trail and `MutationAudit` telemetry.
#[derive(Debug, Clone, Copy)]
pub struct RewriteAction {
    /// The Cypher mutation to apply via `apply_cypher_mutation`.
    pub mutation: CypherMutation,
    /// Source attestation: 16-byte label stamped on the `MutationAudit`
    /// envelope.  Convention: `b"REWRITE_<rule>\0\0"`.
    pub source: [u8; 16],
}

// ── Rule ──────────────────────────────────────────────────────────────────────

/// A single rewrite rule: pattern + optional guard + action.
#[derive(Clone, Copy)]
pub struct RewriteRule {
    pub pattern: RewritePattern,
    /// When `None`, the pattern match alone is sufficient to fire.
    pub guard: Option<GuardFn>,
    pub action: RewriteAction,
    /// Short ASCII label for telemetry — stamped into the `RuleApplied`
    /// control-plane envelope as the `subject` field.
    pub label: [u8; 16],
}

// ── GraphView trait ───────────────────────────────────────────────────────────

/// Query interface the engine uses to evaluate patterns.
/// Implemented by the supervisor/runtime bridge in production; by a mock in tests.
pub trait GraphView {
    fn node_exists(&self, id: NodeId) -> bool;
    fn edge_exists(&self, from: NodeId, to: NodeId, kind: ReceptiveEdgeKind) -> bool;
    fn epoch(&self) -> u64;
    fn snapshot(&self) -> GraphSnapshot;
}

// ── Engine ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteError {
    /// No room for another rule in the engine's fixed-size table.
    RuleTableFull,
}

/// The rewrite engine: a fixed-capacity table of rules evaluated each cycle.
pub struct RewriteEngine {
    rules: [Option<RewriteRule>; MAX_REWRITE_RULES],
    rule_count: usize,
    /// Monotonically increasing counter of total rule firings since creation.
    pub fire_count: u64,
}

impl Default for RewriteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RewriteEngine {
    pub const fn new() -> Self {
        Self {
            rules: [None; MAX_REWRITE_RULES],
            rule_count: 0,
            fire_count: 0,
        }
    }

    /// Register a new rule.  Rules are evaluated in insertion order.
    pub fn add_rule(&mut self, rule: RewriteRule) -> Result<usize, RewriteError> {
        if self.rule_count >= MAX_REWRITE_RULES {
            return Err(RewriteError::RuleTableFull);
        }
        let idx = self.rule_count;
        self.rules[idx] = Some(rule);
        self.rule_count += 1;
        Ok(idx)
    }

    /// Remove all rules, reset fire counter.
    pub fn clear(&mut self) {
        self.rules = [None; MAX_REWRITE_RULES];
        self.rule_count = 0;
        self.fire_count = 0;
    }

    pub fn rule_count(&self) -> usize {
        self.rule_count
    }

    /// Evaluate every registered rule against `view`.
    ///
    /// For each rule whose pattern matches and whose guard (if any) passes,
    /// appends the rule's `(index, action)` pair into `out`.  Returns the
    /// number of rules that fired.  `out` is truncated at `out.len()` — the
    /// caller should size it to `MAX_REWRITE_RULES` to capture all firings.
    ///
    /// The caller is responsible for applying the actions via
    /// `gos_runtime::apply_cypher_mutation` and emitting telemetry via
    /// `gos_runtime::emit_rule_applied`.
    pub fn apply_rules<V: GraphView>(
        &mut self,
        view: &V,
        out: &mut [(usize, RewriteAction)],
    ) -> usize {
        let epoch = view.epoch();
        let snap = view.snapshot();
        let mut fired = 0;

        for i in 0..self.rule_count {
            if fired >= out.len() {
                break;
            }
            let rule = match self.rules[i] {
                Some(r) => r,
                None => continue,
            };

            let pattern_ok = match rule.pattern {
                RewritePattern::Always => true,
                RewritePattern::NodePresent(id) => view.node_exists(id),
                RewritePattern::EdgeAbsent { from, to, kind } => {
                    !view.edge_exists(from, to, kind)
                }
                RewritePattern::EdgePresent { from, to, kind } => {
                    view.edge_exists(from, to, kind)
                }
                RewritePattern::EpochGt(threshold) => epoch > threshold,
                RewritePattern::EpochEq(value) => epoch == value,
            };

            if !pattern_ok {
                continue;
            }

            let guard_ok = match rule.guard {
                None => true,
                Some(g) => g(&snap),
            };

            if guard_ok {
                out[fired] = (i, rule.action);
                fired += 1;
                self.fire_count = self.fire_count.wrapping_add(1);
            }
        }

        fired
    }
}
