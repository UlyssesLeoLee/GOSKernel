//! V2.2a/V2.3a — rewrite-engine core tests ([`doc/ADR-002`] §3-4, §6).
//!
//! Proves the scheduling core in isolation: edge propagation drives the
//! ready-set to quiescence; the causal-depth meter is tracked; a runaway rule
//! is caught by the depth guard (reported, not hung, not silently truncated);
//! and the ratified render-model-B mechanism (reactive `Subscribe`
//! propagation, via [`gos_rewrite::reactive::propagate`]) reaches quiescence
//! for both the theme-diffusion case (`Region::EVERYTHING`) and the
//! dirty-rect case (region-scoped subscriptions, V2.3 Demo C).
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "engine.rs", type: "file", language: "rust"}),
//!   (noop:Class {name: "Noop", type: "struct"}),
//!   (chain:Class {name: "Chain", type: "struct"}),
//!   (loop_:Class {name: "SelfLoop", type: "struct"}),
//!   (react:Class {name: "Reactive", type: "struct"}),
//!   (t1:Function {name: "empty_engine_is_immediately_quiescent", type: "function"}),
//!   (t2:Function {name: "terminating_chain_quiesces_with_expected_depth", type: "function"}),
//!   (t3:Function {name: "runaway_rule_is_caught_by_depth_guard_not_hung", type: "function"}),
//!   (t4:Function {name: "reactive_subscribe_propagation_quiesces", type: "function"}),
//!   (t5:Function {name: "dirty_rect_propagation_is_region_scoped", type: "function"}),
//!   (sub:Module {name: "gos_rewrite::reactive", type: "module"}),
//!   (f)-[:CONTAINS]->(noop), (f)-[:CONTAINS]->(chain), (f)-[:CONTAINS]->(loop_), (f)-[:CONTAINS]->(react),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3), (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5),
//!   (chain)-[:HAS_METHOD]->(t2), (loop_)-[:HAS_METHOD]->(t3), (react)-[:HAS_METHOD]->(t4), (react)-[:HAS_METHOD]->(t5),
//!   (react)-[:USES]->(sub), (t4)-[:USES]->(sub), (t5)-[:USES]->(sub);
//! ```

use gos_protocol::Region;
use gos_rewrite::reactive::{propagate, Subscription};
use gos_rewrite::{Emit, Engine, NodeId, Rule, Signal};

const KIND_STEP: u32 = 1;
const KIND_MUTATE: u32 = 10;
const KIND_REPAINT: u32 = 11;

struct Noop;
impl Rule for Noop {
    fn fire(&mut self, _sig: Signal, _out: &mut Emit) {}
}

/// Each node `n>0` propagates one step to `n-1`; node 0 is terminal.
struct Chain;
impl Rule for Chain {
    fn fire(&mut self, sig: Signal, out: &mut Emit) {
        if sig.to.0 > 0 {
            out.send(NodeId(sig.to.0 - 1), KIND_STEP);
        }
    }
}

/// Always re-emits to itself — never quiesces. The depth guard must catch it.
struct SelfLoop;
impl Rule for SelfLoop {
    fn fire(&mut self, sig: Signal, out: &mut Emit) {
        out.send(sig.to, sig.kind);
    }
}

/// Render-model B: a `mutate` at a node fans out `repaint` to its
/// subscribers via the V2.3a reverse-propagation index
/// ([`gos_rewrite::reactive::propagate`]), scoped by each subscription's
/// [`Region`] (ADR-001 §2.2). `repaint` is terminal. The *same* `table` +
/// `propagate` mechanism backs both theme diffusion
/// (`reactive_subscribe_propagation_quiesces`, `Region::EVERYTHING`) and
/// dirty-rect rendering (`dirty_rect_propagation_is_region_scoped`,
/// bounded regions) — one mechanism, two features (V2.3 plan, "关键涌现证据").
struct Reactive {
    table: Vec<Subscription>,
    /// The region each `mutate`d node is considered to have dirtied.
    mutated_regions: Vec<(u32, Region)>,
}
impl Rule for Reactive {
    fn fire(&mut self, sig: Signal, out: &mut Emit) {
        if sig.kind == KIND_MUTATE {
            if let Some(&(_, region)) = self.mutated_regions.iter().find(|(n, _)| *n == sig.to.0) {
                propagate(&self.table, sig.to, region, KIND_REPAINT, out);
            }
        }
        // KIND_REPAINT is terminal (the "render" happens; no further signals).
    }
}

#[test]
fn empty_engine_is_immediately_quiescent() {
    let mut e = Engine::new(Noop);
    let r = e.run_to_quiescence(1024);
    assert!(r.quiesced);
    assert_eq!(r.steps, 0);
    assert_eq!(r.max_causal_depth, 0);
    assert!(!r.overflowed);
}

#[test]
fn terminating_chain_quiesces_with_expected_depth() {
    let mut e = Engine::new(Chain);
    assert!(e.post(Signal::external(NodeId(5), KIND_STEP)));
    let r = e.run_to_quiescence(1024);

    assert!(r.quiesced, "a terminating rule must reach quiescence");
    // Nodes 5,4,3,2,1,0 each fire once.
    assert_eq!(r.steps, 6);
    // Deepest chain: node 0 is reached at depth 5 (5->4->3->2->1->0).
    assert_eq!(r.max_causal_depth, 5);
    assert!(!r.overflowed);
}

#[test]
fn runaway_rule_is_caught_by_depth_guard_not_hung() {
    let mut e = Engine::new(SelfLoop);
    assert!(e.post(Signal::external(NodeId(1), KIND_STEP)));

    // If the guard were a silent truncation (old 2048 cap), this would just
    // stop with no signal. Instead it reports non-quiescence at the depth it
    // reached — attributable livelock, and crucially it returns (no hang).
    let r = e.run_to_quiescence(64);
    assert!(!r.quiesced, "a self-looping rule must NOT report quiescence");
    assert!(
        r.max_causal_depth >= 64,
        "depth meter should reach the guard ({} < 64)",
        r.max_causal_depth
    );
}

#[test]
fn reactive_subscribe_propagation_quiesces() {
    // node 100 (e.g. theme.current) has three subscribers (render nodes),
    // each subscribed to EVERYTHING — the theme-diffusion case (ADR-001
    // §2.2: a theme switch is region-independent).
    let mut e = Engine::new(Reactive {
        table: vec![
            Subscription::new(NodeId(100), NodeId(200), Region::EVERYTHING),
            Subscription::new(NodeId(100), NodeId(201), Region::EVERYTHING),
            Subscription::new(NodeId(100), NodeId(202), Region::EVERYTHING),
        ],
        mutated_regions: vec![(100, Region::EVERYTHING)],
    });
    assert!(e.post(Signal::external(NodeId(100), KIND_MUTATE)));
    let r = e.run_to_quiescence(1024);

    assert!(r.quiesced, "reactive propagation must terminate");
    // 1 mutate fire + 3 repaint fires.
    assert_eq!(r.steps, 4);
    // mutate at depth 0, repaints at depth 1.
    assert_eq!(r.max_causal_depth, 1);
    assert!(!r.overflowed);
}

/// V2.3 Demo C ("脏矩形免费"): a mutation covering `region_a` only repaints
/// subscribers whose region overlaps `region_a` — a render node watching a
/// disjoint region does not redraw. Same `table`/`propagate` mechanism as
/// `reactive_subscribe_propagation_quiesces`, just with bounded regions
/// instead of `EVERYTHING`.
#[test]
fn dirty_rect_propagation_is_region_scoped() {
    let region_a = Region::new(0, 0, 100, 100);
    let region_b = Region::new(200, 200, 50, 50); // disjoint from region_a

    let mut e = Engine::new(Reactive {
        table: vec![
            // subscriber 200 watches a region overlapping region_a.
            Subscription::new(NodeId(100), NodeId(200), Region::new(50, 50, 100, 100)),
            // subscriber 201 watches a disjoint region — must not repaint.
            Subscription::new(NodeId(100), NodeId(201), region_b),
        ],
        mutated_regions: vec![(100, region_a)],
    });
    assert!(e.post(Signal::external(NodeId(100), KIND_MUTATE)));
    let r = e.run_to_quiescence(1024);

    assert!(r.quiesced, "region-scoped reactive propagation must terminate");
    // 1 mutate fire + 1 repaint fire (only subscriber 200; 201 is out of region).
    assert_eq!(r.steps, 2);
    assert_eq!(r.max_causal_depth, 1);
    assert!(!r.overflowed);
}
