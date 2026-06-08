//! V2.2a — rewrite-engine core tests ([`doc/ADR-002`] §3-4).
//!
//! Proves the scheduling core in isolation: edge propagation drives the
//! ready-set to quiescence; the causal-depth meter is tracked; a runaway rule
//! is caught by the depth guard (reported, not hung, not silently truncated);
//! and the ratified render-model-B mechanism (reactive Subscribe propagation)
//! reaches quiescence.
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
//!   (f)-[:CONTAINS]->(noop), (f)-[:CONTAINS]->(chain), (f)-[:CONTAINS]->(loop_), (f)-[:CONTAINS]->(react),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3), (f)-[:CONTAINS]->(t4),
//!   (chain)-[:HAS_METHOD]->(t2), (loop_)-[:HAS_METHOD]->(t3), (react)-[:HAS_METHOD]->(t4);
//! ```

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

/// Render-model B: a `mutate` at a node fans out `repaint` to its subscribers;
/// `repaint` is terminal. This is the Subscribe reverse-propagation mechanism
/// (theme 0-line extension / dirty-rect rendering) expressed as a plain rule.
struct Reactive {
    subscribers: Vec<(u32, Vec<u32>)>,
}
impl Rule for Reactive {
    fn fire(&mut self, sig: Signal, out: &mut Emit) {
        if sig.kind == KIND_MUTATE {
            for (node, subs) in &self.subscribers {
                if *node == sig.to.0 {
                    for s in subs {
                        out.send(NodeId(*s), KIND_REPAINT);
                    }
                }
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
    // node 100 (e.g. theme.current) has three subscribers (render nodes).
    let mut e = Engine::new(Reactive {
        subscribers: vec![(100, vec![200, 201, 202])],
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
