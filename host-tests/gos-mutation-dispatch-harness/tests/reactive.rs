//! V2.3c — `propagate_with`, the closure-based form of the V2.3a reverse-
//! propagation index ([`gos_mutation_dispatch::reactive::propagate`]).
//!
//! `propagate` (exercised via `Engine`/`Rule` in `engine.rs`) enqueues
//! repaint signals into an `Emit` sink. `propagate_with` is the same
//! target/region matching exposed as a direct callback, for callers that
//! want to dispatch immediately without standing up an `Engine` for one
//! reactive step — e.g. `k-shell::apply_theme_choice_raw` (V2.3c), which
//! already knows `theme.current` just mutated and just needs to repaint
//! its subscribers.
//!
//! These tests mirror `engine.rs`'s `reactive_subscribe_propagation_quiesces`
//! and `dirty_rect_propagation_is_region_scoped` cases, pinned down against
//! `propagate_with` directly: same table, same target/region matching, same
//! subscriber set — just delivered via callback instead of `Emit`.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "reactive.rs", type: "file", language: "rust"}),
//!   (pw:Function {name: "propagate_with", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "propagate_with_dispatches_to_all_everything_subscribers", type: "function"}),
//!   (t2:Function {name: "propagate_with_skips_disjoint_regions", type: "function"}),
//!   (t3:Function {name: "propagate_with_no_match_calls_nothing", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (t1)-[:USES]->(pw), (t2)-[:USES]->(pw), (t3)-[:USES]->(pw);
//! ```

use gos_protocol::Region;
use gos_mutation_dispatch::reactive::{propagate_with, Subscription};
use gos_mutation_dispatch::NodeId;

const KIND_REPAINT: u32 = 11;

/// Theme-diffusion case: three subscribers watching `Region::EVERYTHING`
/// all repaint on a mutation, in table order.
#[test]
fn propagate_with_dispatches_to_all_everything_subscribers() {
    let table = [
        Subscription::new(NodeId(100), NodeId(200), Region::EVERYTHING),
        Subscription::new(NodeId(100), NodeId(201), Region::EVERYTHING),
        Subscription::new(NodeId(100), NodeId(202), Region::EVERYTHING),
    ];

    let mut dispatched = Vec::new();
    propagate_with(&table, NodeId(100), Region::EVERYTHING, KIND_REPAINT, |subscriber, kind| {
        dispatched.push((subscriber.0, kind));
    });

    assert_eq!(dispatched, vec![(200, KIND_REPAINT), (201, KIND_REPAINT), (202, KIND_REPAINT)]);
}

/// Dirty-rect case (V2.3 Demo C): a mutation covering `region_a` only
/// repaints the subscriber whose region overlaps it; the disjoint
/// subscriber is skipped entirely — no callback invocation at all.
#[test]
fn propagate_with_skips_disjoint_regions() {
    let region_a = Region::new(0, 0, 100, 100);
    let region_b = Region::new(200, 200, 50, 50); // disjoint from region_a

    let table = [
        Subscription::new(NodeId(100), NodeId(200), Region::new(50, 50, 100, 100)),
        Subscription::new(NodeId(100), NodeId(201), region_b),
    ];

    let mut dispatched = Vec::new();
    propagate_with(&table, NodeId(100), region_a, KIND_REPAINT, |subscriber, kind| {
        dispatched.push((subscriber.0, kind));
    });

    assert_eq!(dispatched, vec![(200, KIND_REPAINT)]);
}

/// A mutation at a node with no subscriptions calls the closure zero times
/// — `apply_theme_choice_raw` relies on this for its `visual_ok` fold to
/// stay `true` (vacuous AND) if a future table ever has no matching row.
#[test]
fn propagate_with_no_match_calls_nothing() {
    let table = [Subscription::new(NodeId(999), NodeId(200), Region::EVERYTHING)];

    let mut calls = 0;
    propagate_with(&table, NodeId(100), Region::EVERYTHING, KIND_REPAINT, |_, _| {
        calls += 1;
    });

    assert_eq!(calls, 0);
}
