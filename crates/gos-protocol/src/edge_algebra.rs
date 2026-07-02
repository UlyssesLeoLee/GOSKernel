//! # Edge Algebra — primitive decomposition of the runtime edge set (V2.0)
//!
//! This module encodes the **edge algebra constitution** ([`doc/ADR-001`]) as
//! code, *additively*: it does not change [`RuntimeEdgeType`] or any of its
//! uses. It adds a primitive representation ([`EdgeForm`]) and a total
//! `lower()` projection from the 9 named edges onto that representation, plus
//! the reverse recognition of the canonical named point.
//!
//! An edge is `Refer` (the visibility floor) plus any subset of the three
//! orthogonal causal/lifecycle/authority bits `Send`/`Bind`/`Grant`, modulated
//! by orthogonal attributes (`persistent`, `exclusive`, `cardinality`,
//! `reactive`). The 9 historical edges become *named composition points* — no
//! edge needs a 5th primitive.
//!
//! ## Lowering is a surjection, not a bijection
//!
//! `Signal`, `Return`, and `Sync` all lower to the same `EdgeForm`
//! (`Refer+Send`, cardinality `One`). This is **predicted by ADR-001 §2.3**:
//! `Return`'s reply-pairing lives in the `Call` reply-cap, and `Sync`'s barrier
//! lives in the two endpoints' fire-guards — neither distinction is *edge*
//! level. So `recognize()` returns the canonical representative (`Signal`) for
//! that equivalence class, and round-trip is stated as *form-stability*
//! (`lower(recognize(lower(e))) == lower(e)`) rather than identity for those
//! three. The other six edges round-trip exactly.
//!
//! [`doc/ADR-001`]: ../../../doc/ADR-001-edge-algebra-constitution.md
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "edge_algebra.rs", type: "file", language: "rust"}),
//!   (m:Module {name: "edge_algebra", type: "module"}),
//!   (eb:Class {name: "EdgeBits", type: "struct", signature: "{refer,send,bind,grant: bool}"}),
//!   (ca:Class {name: "Cardinality", type: "enum", signature: "One|Many"}),
//!   (ea:Class {name: "EdgeAttrs", type: "struct", signature: "{persistent,exclusive,cardinality,reactive}"}),
//!   (ef:Class {name: "EdgeForm", type: "struct", signature: "{bits: EdgeBits, attrs: EdgeAttrs}"}),
//!   (ret:Class {name: "RuntimeEdgeType", type: "enum", signature: "9 named edges (defined in lib.rs)"}),
//!   (eb_new:Function {name: "EdgeBits::new", type: "function", visibility: "pub"}),
//!   (eb_none:Function {name: "EdgeBits::none", type: "function", visibility: "pub"}),
//!   (eb_refer:Function {name: "EdgeBits::refer_only", type: "function", visibility: "pub"}),
//!   (eb_union:Function {name: "EdgeBits::union", type: "function", visibility: "pub"}),
//!   (eb_floor:Function {name: "EdgeBits::floor_ok", type: "function", visibility: "pub"}),
//!   (ca_join:Function {name: "Cardinality::join", type: "function", visibility: "pub"}),
//!   (ea_new:Function {name: "EdgeAttrs::new", type: "function", visibility: "pub"}),
//!   (ea_plain:Function {name: "EdgeAttrs::plain", type: "function", visibility: "pub"}),
//!   (ea_merge:Function {name: "EdgeAttrs::merge", type: "function", visibility: "pub"}),
//!   (ef_new:Function {name: "EdgeForm::new", type: "function", visibility: "pub"}),
//!   (ef_valid:Function {name: "EdgeForm::is_valid", type: "function", visibility: "pub"}),
//!   (ef_recog:Function {name: "EdgeForm::recognize", type: "function", visibility: "pub"}),
//!   (ef_comp:Function {name: "EdgeForm::compose", type: "function", visibility: "pub"}),
//!   (low:Function {name: "RuntimeEdgeType::lower", type: "function", visibility: "pub"}),
//!   (f)-[:CONTAINS]->(m),
//!   (m)-[:CONTAINS]->(eb), (m)-[:CONTAINS]->(ca), (m)-[:CONTAINS]->(ea), (m)-[:CONTAINS]->(ef),
//!   (eb)-[:HAS_METHOD]->(eb_new), (eb)-[:HAS_METHOD]->(eb_none), (eb)-[:HAS_METHOD]->(eb_refer),
//!   (eb)-[:HAS_METHOD]->(eb_union), (eb)-[:HAS_METHOD]->(eb_floor),
//!   (ca)-[:HAS_METHOD]->(ca_join),
//!   (ea)-[:HAS_METHOD]->(ea_new), (ea)-[:HAS_METHOD]->(ea_plain), (ea)-[:HAS_METHOD]->(ea_merge),
//!   (ef)-[:HAS_METHOD]->(ef_new), (ef)-[:HAS_METHOD]->(ef_valid),
//!   (ef)-[:HAS_METHOD]->(ef_recog), (ef)-[:HAS_METHOD]->(ef_comp),
//!   (ret)-[:HAS_METHOD]->(low),
//!   (low)-[:CALLS]->(eb_new), (low)-[:CALLS]->(ea_new), (low)-[:CALLS]->(ef_new),
//!   (ef_valid)-[:CALLS]->(eb_floor),
//!   (ef_comp)-[:CALLS]->(eb_union), (ef_comp)-[:CALLS]->(ea_merge), (ef_comp)-[:CALLS]->(ef_new),
//!   (ea_merge)-[:CALLS]->(ca_join);
//! ```

use crate::RuntimeEdgeType;

/// The three orthogonal primitive bits plus the `Refer` visibility floor.
///
/// `Refer` is the substrate: `Send`/`Bind`/`Grant` each *imply* `Refer`, so any
/// well-formed edge that carries causality, lifecycle, or authority must also
/// be visible. [`EdgeBits::floor_ok`] checks that invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeBits {
    /// Visibility: A can name / address B. The floor for everything else.
    pub refer: bool,
    /// Causality: A's fire may deliver a signal that activates B.
    pub send: bool,
    /// Lifecycle: B's lifecycle is coupled to A (cascade).
    pub bind: bool,
    /// Authority: the edge confers the right to invoke B's exported capability.
    pub grant: bool,
}

impl EdgeBits {
    pub const fn new(refer: bool, send: bool, bind: bool, grant: bool) -> Self {
        Self { refer, send, bind, grant }
    }

    /// The empty edge — no relation at all. Useful as a `compose` identity.
    pub const fn none() -> Self {
        Self::new(false, false, false, false)
    }

    /// Pure visibility, nothing more. This is exactly how `Depend` lowers.
    pub const fn refer_only() -> Self {
        Self::new(true, false, false, false)
    }

    /// Lattice join of two bit sets (bitwise OR per dimension). Composing two
    /// edges between the same node pair yields the union of their primitives.
    pub const fn union(self, other: EdgeBits) -> EdgeBits {
        EdgeBits::new(
            self.refer || other.refer,
            self.send || other.send,
            self.bind || other.bind,
            self.grant || other.grant,
        )
    }

    /// The refer-floor invariant: any edge carrying `Send`/`Bind`/`Grant` must
    /// also carry `Refer`. The empty edge ([`none`](EdgeBits::none)) trivially
    /// satisfies it. ADR-001 §三 makes this a constitutional property.
    pub const fn floor_ok(self) -> bool {
        if self.send || self.bind || self.grant {
            self.refer
        } else {
            true
        }
    }
}

/// How many times an edge fires. `Stream` is the only legacy edge that is
/// `Many`; everything else is `One`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    One,
    Many,
}

impl Cardinality {
    /// Join: `Many` dominates `One` (the more permissive wins under compose).
    pub const fn join(self, other: Cardinality) -> Cardinality {
        match (self, other) {
            (Cardinality::Many, _) | (_, Cardinality::Many) => Cardinality::Many,
            _ => Cardinality::One,
        }
    }
}

/// Orthogonal attributes that modulate an edge without adding a new relation
/// dimension. `reactive` is modeled as a bool for V2.0; its region-predicate
/// payload (ADR-001 §2.2) arrives with `Subscribe` in V2.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeAttrs {
    /// Survives power-cycle (else volatile). All 9 legacy edges lower as `false`.
    pub persistent: bool,
    /// At most one such edge may terminate at the target (`theme.current` Use).
    pub exclusive: bool,
    pub cardinality: Cardinality,
    /// Engine auto-propagates a reverse signal on mutation of the target.
    /// `false` for every legacy edge; `true` for V2.3 `Subscribe`.
    pub reactive: bool,
}

impl EdgeAttrs {
    pub const fn new(persistent: bool, exclusive: bool, cardinality: Cardinality, reactive: bool) -> Self {
        Self { persistent, exclusive, cardinality, reactive }
    }

    /// The default attribute vector: volatile, non-exclusive, fires once,
    /// non-reactive. Most legacy edges lower to this.
    pub const fn plain() -> Self {
        Self::new(false, false, Cardinality::One, false)
    }

    /// Lattice join of attributes: OR the bools, join the cardinality.
    pub const fn merge(self, other: EdgeAttrs) -> EdgeAttrs {
        EdgeAttrs::new(
            self.persistent || other.persistent,
            self.exclusive || other.exclusive,
            self.cardinality.join(other.cardinality),
            self.reactive || other.reactive,
        )
    }
}

/// The lowered primitive form of an edge: the primitive bits plus attributes.
/// This is the representation the V2.2 rewrite engine will operate on; in V2.0
/// it is purely a derived view validated against [`RuntimeEdgeType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeForm {
    pub bits: EdgeBits,
    pub attrs: EdgeAttrs,
}

impl EdgeForm {
    pub const fn new(bits: EdgeBits, attrs: EdgeAttrs) -> Self {
        Self { bits, attrs }
    }

    /// A form is valid iff its bits satisfy the refer-floor invariant.
    pub const fn is_valid(self) -> bool {
        self.bits.floor_ok()
    }

    /// Reverse the lowering to the **canonical** named point, or `None` if the
    /// form has no legacy name (e.g. it is persistent or reactive, or it is a
    /// bit/attr combination the 9 legacy edges never produced).
    ///
    /// Surjective: `{Signal, Return, Sync}` share `Refer+Send / One`, so this
    /// returns `Signal` for that whole equivalence class (see module docs).
    pub const fn recognize(self) -> Option<RuntimeEdgeType> {
        // Persistent / reactive forms are V2.3+ constructs with no legacy name.
        if self.attrs.persistent || self.attrs.reactive {
            return None;
        }
        let EdgeBits { refer, send, bind, grant } = self.bits;
        let excl = self.attrs.exclusive;
        let card = self.attrs.cardinality;
        if !refer {
            // Nothing legacy is built without the visibility floor.
            return None;
        }
        match (send, bind, grant, excl, card) {
            // Depend = Refer only.
            (false, false, false, false, Cardinality::One) => Some(RuntimeEdgeType::Depend),
            // Mount = Refer + Bind, non-exclusive.
            (false, true, false, false, Cardinality::One) => Some(RuntimeEdgeType::Mount),
            // Use = Refer + Bind + Grant, exclusive.
            (false, true, true, true, Cardinality::One) => Some(RuntimeEdgeType::Use),
            // Call = Refer + Send + Grant.
            (true, false, true, false, Cardinality::One) => Some(RuntimeEdgeType::Call),
            // Spawn = Refer + Send + Bind.
            (true, true, false, false, Cardinality::One) => Some(RuntimeEdgeType::Spawn),
            // Stream = Refer + Send, Many.
            (true, false, false, false, Cardinality::Many) => Some(RuntimeEdgeType::Stream),
            // Signal/Return/Sync collision class — Signal is the canonical name.
            (true, false, false, false, Cardinality::One) => Some(RuntimeEdgeType::Signal),
            _ => None,
        }
    }

    /// Lattice join of two forms: union the bits, merge the attributes. Closed
    /// — the result is always a valid form (ADR-001 §三 closure property).
    pub const fn compose(self, other: EdgeForm) -> EdgeForm {
        EdgeForm::new(self.bits.union(other.bits), self.attrs.merge(other.attrs))
    }
}

impl RuntimeEdgeType {
    /// Lower a named edge to its primitive form — the ADR-001 §2.3 table,
    /// encoded once. Total: every variant lowers without panic.
    pub const fn lower(self) -> EdgeForm {
        match self {
            // Call = Send + Grant.
            RuntimeEdgeType::Call => {
                EdgeForm::new(EdgeBits::new(true, true, false, true), EdgeAttrs::plain())
            }
            // Spawn = Send + Bind.
            RuntimeEdgeType::Spawn => {
                EdgeForm::new(EdgeBits::new(true, true, true, false), EdgeAttrs::plain())
            }
            // Depend = Refer only (dependency lives in the depender's fire-guard).
            RuntimeEdgeType::Depend => {
                EdgeForm::new(EdgeBits::refer_only(), EdgeAttrs::plain())
            }
            // Signal = Send, fire-and-forget.
            RuntimeEdgeType::Signal => {
                EdgeForm::new(EdgeBits::new(true, true, false, false), EdgeAttrs::plain())
            }
            // Return = Send on the reverse leg of a Call (same edge form as Signal).
            RuntimeEdgeType::Return => {
                EdgeForm::new(EdgeBits::new(true, true, false, false), EdgeAttrs::plain())
            }
            // Mount = Refer + Bind, non-exclusive.
            RuntimeEdgeType::Mount => {
                EdgeForm::new(EdgeBits::new(true, false, true, false), EdgeAttrs::plain())
            }
            // Sync = bidirectional Send; the barrier lives in fire-guards
            // (same edge form as Signal).
            RuntimeEdgeType::Sync => {
                EdgeForm::new(EdgeBits::new(true, true, false, false), EdgeAttrs::plain())
            }
            // Stream = Send, repeated over time.
            RuntimeEdgeType::Stream => EdgeForm::new(
                EdgeBits::new(true, true, false, false),
                EdgeAttrs::new(false, false, Cardinality::Many, false),
            ),
            // Use = Refer + Bind + Grant, exclusive.
            RuntimeEdgeType::Use => EdgeForm::new(
                EdgeBits::new(true, false, true, true),
                EdgeAttrs::new(false, true, Cardinality::One, false),
            ),
        }
    }
}
