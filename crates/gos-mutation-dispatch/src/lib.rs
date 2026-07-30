#![no_std]
//! # gos-mutation-dispatch — rewrite-engine core (V2.2a walking skeleton, [`doc/ADR-002`])
//!
//! This is the **scheduling core** of the V2.2 rewrite engine, in isolation:
//! it models ADR-002 §3 (scheduling = edge propagation) and §4 (quiescence is
//! the prime invariant; a causal-depth meter replaces the old 2048 hard cap).
//! It is deliberately abstract — `NodeId` is an opaque `u32`, a [`Rule`] maps a
//! delivered [`Signal`] to emitted signals — so the core loop and its
//! termination property can be proven by harness *before* the engine is wired
//! to the real runtime graph (that wiring is V2.2b). It coexists with
//! `gos_supervisor::service_system_cycle`; nothing here touches the boot path.
//!
//! Per the ratified render model (**B — graph IS scene**, ADR-002 §6), reactive
//! propagation (`Subscribe` = `Refer` + `reactive`) is not engine magic — it is
//! just a [`Rule`] that, on a `mutate` signal at a node, emits `repaint`
//! signals to that node's subscribers. The engine core stays minimal; the
//! reactive specialization lives in rule logic. See the harness for a worked
//! reactive example.
//!
//! `no_std`, no `alloc`: bounded ring queue + fixed fan-out, mirroring the
//! capacity-bounded style of `gos-runtime`.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "lib.rs", type: "file", language: "rust"}),
//!   (m:Module {name: "gos_mutation_dispatch", type: "module"}),
//!   (nid:Class {name: "NodeId", type: "struct"}),
//!   (sig:Class {name: "Signal", type: "struct"}),
//!   (emit:Class {name: "Emit", type: "struct"}),
//!   (rq:Class {name: "RingQueue", type: "struct"}),
//!   (rule:Class {name: "Rule", type: "trait"}),
//!   (eng:Class {name: "Engine", type: "struct"}),
//!   (rep:Class {name: "QuiescenceReport", type: "struct"}),
//!   (sig_new:Function {name: "Signal::external", type: "function", visibility: "pub"}),
//!   (emit_send:Function {name: "Emit::send", type: "function", visibility: "pub"}),
//!   (rq_push:Function {name: "RingQueue::push", type: "function", visibility: "private"}),
//!   (rq_pop:Function {name: "RingQueue::pop", type: "function", visibility: "private"}),
//!   (rule_fire:Function {name: "Rule::fire", type: "function", visibility: "pub"}),
//!   (eng_new:Function {name: "Engine::new", type: "function", visibility: "pub"}),
//!   (eng_post:Function {name: "Engine::post", type: "function", visibility: "pub"}),
//!   (eng_run:Function {name: "Engine::run_to_quiescence", type: "function", visibility: "pub"}),
//!   (f)-[:CONTAINS]->(m),
//!   (m)-[:CONTAINS]->(nid), (m)-[:CONTAINS]->(sig), (m)-[:CONTAINS]->(emit),
//!   (m)-[:CONTAINS]->(rq), (m)-[:CONTAINS]->(rule), (m)-[:CONTAINS]->(eng), (m)-[:CONTAINS]->(rep),
//!   (sig)-[:HAS_METHOD]->(sig_new),
//!   (emit)-[:HAS_METHOD]->(emit_send),
//!   (rq)-[:HAS_METHOD]->(rq_push), (rq)-[:HAS_METHOD]->(rq_pop),
//!   (rule)-[:HAS_METHOD]->(rule_fire),
//!   (eng)-[:HAS_METHOD]->(eng_new), (eng)-[:HAS_METHOD]->(eng_post), (eng)-[:HAS_METHOD]->(eng_run),
//!   (eng_run)-[:CALLS]->(rq_pop), (eng_run)-[:CALLS]->(rule_fire), (eng_run)-[:CALLS]->(rq_push),
//!   (emit_send)-[:USES]->(sig);
//! ```

pub mod boot;
pub mod capability;
pub mod reactive;

/// Capacity of the engine's pending-signal queue (back-pressure bound).
pub const MAX_PENDING: usize = 256;
/// Max signals one rule firing may emit (fan-out bound).
pub const MAX_FANOUT: usize = 16;

/// Opaque node identity for the skeleton. V2.2b replaces this with the runtime
/// `NodeId`; the engine semantics are identity-agnostic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeId(pub u32);

/// A pending propagation along a `Send` edge: deliver `kind` to node `to`.
/// `depth` is the causal distance from an external (depth-0) stimulus — the
/// telemetry that ADR-002 §4 uses instead of a silent iteration cap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Signal {
    pub to: NodeId,
    pub kind: u32,
    pub depth: u32,
}

impl Signal {
    const ZERO: Signal = Signal { to: NodeId(0), kind: 0, depth: 0 };

    /// An externally injected signal — the root of a causal chain (depth 0).
    pub const fn external(to: NodeId, kind: u32) -> Signal {
        Signal { to, kind, depth: 0 }
    }
}

/// Sink a [`Rule`] emits into while firing. Emitted signals inherit
/// `depth = firing_signal.depth + 1`, so causal depth is tracked automatically.
pub struct Emit {
    buf: [Signal; MAX_FANOUT],
    len: usize,
    next_depth: u32,
}

impl Emit {
    const fn new(next_depth: u32) -> Self {
        Self { buf: [Signal::ZERO; MAX_FANOUT], len: 0, next_depth }
    }

    /// Emit a signal to `to` of `kind`. Returns `false` if this firing already
    /// hit `MAX_FANOUT` (the signal is dropped — a rule should not fan out
    /// unboundedly; that would itself be a livelock smell).
    pub fn send(&mut self, to: NodeId, kind: u32) -> bool {
        if self.len >= MAX_FANOUT {
            return false;
        }
        self.buf[self.len] = Signal { to, kind, depth: self.next_depth };
        self.len += 1;
        true
    }
}

/// A node's behaviour: react to a delivered signal by emitting zero or more
/// new signals. This is the skeleton's stand-in for ADR-002 §2's
/// LHS→guard→RHS rule (the match/guard are folded into the `match sig.kind`
/// the implementor writes; the RHS is the `out.send(..)` calls).
pub trait Rule {
    fn fire(&mut self, sig: Signal, out: &mut Emit);
}

/// Result of driving the engine to quiescence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuiescenceReport {
    /// `true` iff the pending queue drained within `depth_cap`. `false` means
    /// the causal-depth guard tripped — reported, never silently truncated.
    pub quiesced: bool,
    /// Number of signals fired (rule invocations).
    pub steps: u64,
    /// Deepest causal chain observed — the ADR-002 §4 telemetry.
    pub max_causal_depth: u32,
    /// `true` if the pending queue overflowed `MAX_PENDING` (back-pressure).
    pub overflowed: bool,
}

struct RingQueue {
    buf: [Signal; MAX_PENDING],
    head: usize,
    tail: usize,
    len: usize,
}

impl RingQueue {
    const fn new() -> Self {
        Self { buf: [Signal::ZERO; MAX_PENDING], head: 0, tail: 0, len: 0 }
    }

    fn push(&mut self, sig: Signal) -> bool {
        if self.len == MAX_PENDING {
            return false;
        }
        self.buf[self.tail] = sig;
        self.tail = (self.tail + 1) % MAX_PENDING;
        self.len += 1;
        true
    }

    fn pop(&mut self) -> Option<Signal> {
        if self.len == 0 {
            return None;
        }
        let sig = self.buf[self.head];
        self.head = (self.head + 1) % MAX_PENDING;
        self.len -= 1;
        Some(sig)
    }
}

/// The rewrite-engine core: a ready-set (pending signals) driven to quiescence
/// by propagating each fired signal's emissions back into the set.
pub struct Engine<R: Rule> {
    pending: RingQueue,
    rule: R,
    steps: u64,
    max_causal_depth: u32,
}

impl<R: Rule> Engine<R> {
    pub fn new(rule: R) -> Self {
        Self { pending: RingQueue::new(), rule, steps: 0, max_causal_depth: 0 }
    }

    /// Inject an external (depth-0) stimulus. Returns `false` if the pending
    /// queue is full.
    pub fn post(&mut self, sig: Signal) -> bool {
        self.pending.push(sig)
    }

    /// Fire signals until the pending set is empty (**quiescence**) or a signal
    /// exceeds `depth_cap` causal depth (the **livelock guard**). The guard
    /// replaces the old 2048 hard cap: instead of silently truncating, it
    /// stops and reports `quiesced: false` with the depth reached, so a runaway
    /// rule is *attributable* (ADR-002 §4).
    pub fn run_to_quiescence(&mut self, depth_cap: u32) -> QuiescenceReport {
        let mut overflowed = false;
        while let Some(sig) = self.pending.pop() {
            if sig.depth > depth_cap {
                if sig.depth > self.max_causal_depth {
                    self.max_causal_depth = sig.depth;
                }
                return QuiescenceReport {
                    quiesced: false,
                    steps: self.steps,
                    max_causal_depth: self.max_causal_depth,
                    overflowed,
                };
            }
            self.steps += 1;
            if sig.depth > self.max_causal_depth {
                self.max_causal_depth = sig.depth;
            }
            // Fire into a temp sink, then drain into pending — keeps the
            // `rule` and `pending` borrows disjoint.
            let mut emit = Emit::new(sig.depth + 1);
            self.rule.fire(sig, &mut emit);
            let mut i = 0;
            while i < emit.len {
                if !self.pending.push(emit.buf[i]) {
                    overflowed = true;
                }
                i += 1;
            }
        }
        QuiescenceReport {
            quiesced: true,
            steps: self.steps,
            max_causal_depth: self.max_causal_depth,
            overflowed,
        }
    }
}
