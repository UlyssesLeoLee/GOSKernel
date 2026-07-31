//! V2.3a — Subscribe reverse-propagation index ([`doc/ADR-002`] §6, [`doc/ADR-001`] §2.2).
//!
//! A [`Subscription`] is the engine-side reading of a `Subscribe` edge
//! (`gos_protocol::Subscribe`): `target` is the node whose mutation triggers
//! propagation, `subscriber` is the node that receives `repaint`, and
//! `region` is the dirty-rect predicate (ADR-001 §2.2) that scopes it.
//! [`propagate`] is the reverse index itself — given a mutation at `target`
//! covering `mutated_region`, it walks the subscription table and emits
//! `repaint` to every overlapping subscriber.
//!
//! Per the ratified render model (B — graph IS scene, ADR-002 §6), this is
//! the *one* mechanism behind both theme diffusion (`Region::EVERYTHING`
//! subscriptions) and dirty-rect rendering (bounded-region subscriptions) —
//! see the harness for both cases proven side by side.
//!
//! `no_std`, no `alloc`: a pure slice scan, mirroring
//! [`boot::resolve_boot_order`](crate::boot::resolve_boot_order)'s
//! "the table is the graph" style.
//!
//! [`propagate_with`] (V2.3c) is the same scan exposed as a direct callback
//! rather than an [`Emit`] sink — for callers that dispatch immediately
//! instead of going through an [`Engine`](crate::Engine)/[`Rule`](crate::Rule)
//! (e.g. `k-shell::apply_theme_choice_raw`, which already knows
//! `theme.current` just mutated and needs to repaint its subscribers
//! without standing up an engine for one reactive step).
//! [`propagate`] is now a thin [`Emit`]-sink wrapper over it — one matching
//! rule, two call shapes.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "reactive.rs", type: "file", language: "rust"}),
//!   (m:Module {name: "gos_mutation_dispatch::reactive", type: "module"}),
//!   (sub:Class {name: "Subscription", type: "struct", signature: "{target,subscriber: NodeId, region: Region}"}),
//!   (sub_new:Function {name: "Subscription::new", type: "function", visibility: "pub"}),
//!   (prop:Function {name: "propagate", type: "function", visibility: "pub"}),
//!   (propw:Function {name: "propagate_with", type: "function", visibility: "pub"}),
//!   (rg:Class {name: "gos_protocol::Region", type: "struct"}),
//!   (es:Function {name: "Emit::send", type: "function", visibility: "pub"}),
//!   (f)-[:CONTAINS]->(m),
//!   (m)-[:CONTAINS]->(sub), (m)-[:CONTAINS]->(prop), (m)-[:CONTAINS]->(propw),
//!   (sub)-[:HAS_METHOD]->(sub_new),
//!   (sub)-[:USES]->(rg),
//!   (propw)-[:USES]->(sub),
//!   (prop)-[:CALLS]->(propw), (prop)-[:CALLS]->(es);
//! ```

use crate::{Emit, NodeId};
use gos_protocol::Region;

/// One `Subscribe` edge, read for propagation: `target` mutating within
/// `region` causes `subscriber` to receive a `repaint` signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    pub target: NodeId,
    pub subscriber: NodeId,
    pub region: Region,
}

impl Subscription {
    pub const fn new(target: NodeId, subscriber: NodeId, region: Region) -> Self {
        Self { target, subscriber, region }
    }
}

/// Reverse-propagate a mutation: for every `table` entry whose `target`
/// matches and whose `region` overlaps `mutated_region`, call
/// `on_subscriber(subscriber, repaint_kind)` (ADR-001 §2.2, ADR-002 §6).
///
/// This is [`propagate`]'s matching rule without the [`Emit`]/[`Engine`]
/// machinery — for callers that react to a mutation immediately rather than
/// enqueueing a signal for a rule to pick up later.
pub fn propagate_with<F: FnMut(NodeId, u32)>(
    table: &[Subscription],
    target: NodeId,
    mutated_region: Region,
    repaint_kind: u32,
    mut on_subscriber: F,
) {
    for sub in table {
        if sub.target.0 == target.0 && sub.region.overlaps(&mutated_region) {
            on_subscriber(sub.subscriber, repaint_kind);
        }
    }
}

/// Reverse-propagate a mutation: for every `table` entry whose `target`
/// matches and whose `region` overlaps `mutated_region`, emit
/// `Send(repaint_kind)` to `subscriber` (ADR-001 §2.2, ADR-002 §6).
///
/// `out` inherits the firing signal's depth via [`Emit`], so a propagated
/// `repaint` is causally one step deeper than the `mutate` that triggered
/// it — the engine's existing causal-depth meter covers Subscribe fan-out
/// for free.
pub fn propagate(
    table: &[Subscription],
    target: NodeId,
    mutated_region: Region,
    repaint_kind: u32,
    out: &mut Emit,
) {
    propagate_with(table, target, mutated_region, repaint_kind, |subscriber, kind| {
        out.send(subscriber, kind);
    });
}
