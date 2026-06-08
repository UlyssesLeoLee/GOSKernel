//! Property tests for the V2.0 edge algebra ([`doc/ADR-001`]).
//!
//! These are the constitution-fidelity tests: they verify that the primitive
//! decomposition in `gos_protocol::edge_algebra` faithfully encodes ADR-001
//! and — critically for V2.0 — that adding it changed *nothing* about the
//! historical [`RuntimeEdgeType`] discriminants.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "edge_algebra.rs", type: "file", language: "rust"}),
//!   (h:Function {name: "all_edges", type: "function", visibility: "private"}),
//!   (t1:Function {name: "discriminants_are_stable", type: "function"}),
//!   (t2:Function {name: "lowering_is_total_and_floor_ok", type: "function"}),
//!   (t3:Function {name: "decomposition_matches_constitution", type: "function"}),
//!   (t4:Function {name: "bijective_edges_round_trip", type: "function"}),
//!   (t5:Function {name: "signal_return_sync_share_one_form", type: "function"}),
//!   (t6:Function {name: "recognize_rejects_non_legacy_forms", type: "function"}),
//!   (t7:Function {name: "compose_is_closed_commutative_idempotent", type: "function"}),
//!   (t8:Function {name: "refer_floor_rejects_visibility_violations", type: "function"}),
//!   (f)-[:CONTAINS]->(h),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5), (f)-[:CONTAINS]->(t6),
//!   (f)-[:CONTAINS]->(t7), (f)-[:CONTAINS]->(t8),
//!   (t2)-[:CALLS]->(h), (t3)-[:CALLS]->(h), (t4)-[:CALLS]->(h), (t6)-[:CALLS]->(h);
//! ```

use gos_protocol::{Cardinality, EdgeAttrs, EdgeBits, EdgeForm, RuntimeEdgeType};

/// All 9 historical edges, in discriminant order.
fn all_edges() -> [RuntimeEdgeType; 9] {
    use RuntimeEdgeType::*;
    [Call, Spawn, Depend, Signal, Return, Mount, Sync, Stream, Use]
}

/// The 6 edges whose `EdgeForm` is unique, so they round-trip exactly.
fn bijective_edges() -> [RuntimeEdgeType; 6] {
    use RuntimeEdgeType::*;
    [Call, Spawn, Depend, Mount, Stream, Use]
}

/// The collision class — these three share one `EdgeForm`.
fn collision_class() -> [RuntimeEdgeType; 3] {
    use RuntimeEdgeType::*;
    [Signal, Return, Sync]
}

/// Zero-behavior-change guard: the wire discriminants must be exactly what they
/// were before V2.0 touched this crate. If a refactor ever renumbers these, the
/// control-plane ABI silently breaks — this test is the tripwire.
#[test]
fn discriminants_are_stable() {
    assert_eq!(RuntimeEdgeType::Call as u8, 0x01);
    assert_eq!(RuntimeEdgeType::Spawn as u8, 0x02);
    assert_eq!(RuntimeEdgeType::Depend as u8, 0x03);
    assert_eq!(RuntimeEdgeType::Signal as u8, 0x04);
    assert_eq!(RuntimeEdgeType::Return as u8, 0x05);
    assert_eq!(RuntimeEdgeType::Mount as u8, 0x06);
    assert_eq!(RuntimeEdgeType::Sync as u8, 0x07);
    assert_eq!(RuntimeEdgeType::Stream as u8, 0x08);
    assert_eq!(RuntimeEdgeType::Use as u8, 0x09);
}

/// Totality + the refer-floor invariant: every edge lowers without panic, and
/// every resulting form is valid and visible (`Refer` set).
#[test]
fn lowering_is_total_and_floor_ok() {
    for e in all_edges() {
        let form = e.lower();
        assert!(form.is_valid(), "{e:?} lowered to an invalid form: {form:?}");
        assert!(form.bits.refer, "{e:?} lowered without the Refer floor: {form:?}");
    }
}

/// The golden decomposition table. This restates ADR-001 §2.3 *independently*
/// of `lower()`'s source, so it is a real oracle, not a tautology.
#[test]
fn decomposition_matches_constitution() {
    let plain = EdgeAttrs::plain();
    let expect = |e: RuntimeEdgeType| -> EdgeForm {
        match e {
            RuntimeEdgeType::Call => EdgeForm::new(EdgeBits::new(true, true, false, true), plain),
            RuntimeEdgeType::Spawn => EdgeForm::new(EdgeBits::new(true, true, true, false), plain),
            RuntimeEdgeType::Depend => EdgeForm::new(EdgeBits::new(true, false, false, false), plain),
            RuntimeEdgeType::Signal => EdgeForm::new(EdgeBits::new(true, true, false, false), plain),
            RuntimeEdgeType::Return => EdgeForm::new(EdgeBits::new(true, true, false, false), plain),
            RuntimeEdgeType::Mount => EdgeForm::new(EdgeBits::new(true, false, true, false), plain),
            RuntimeEdgeType::Sync => EdgeForm::new(EdgeBits::new(true, true, false, false), plain),
            RuntimeEdgeType::Stream => EdgeForm::new(
                EdgeBits::new(true, true, false, false),
                EdgeAttrs::new(false, false, Cardinality::Many, false),
            ),
            RuntimeEdgeType::Use => EdgeForm::new(
                EdgeBits::new(true, false, true, true),
                EdgeAttrs::new(false, true, Cardinality::One, false),
            ),
        }
    };
    for e in all_edges() {
        assert_eq!(e.lower(), expect(e), "{e:?} decomposition drifted from ADR-001 §2.3");
    }
}

/// The 6 unique edges round-trip exactly: lower then recognize is identity.
#[test]
fn bijective_edges_round_trip() {
    for e in bijective_edges() {
        assert_eq!(
            e.lower().recognize(),
            Some(e),
            "{e:?} did not round-trip through its EdgeForm"
        );
    }
}

/// `Signal`/`Return`/`Sync` collapse to one form (predicted by ADR-001 §2.3:
/// their distinction is node-level, not edge-level). `recognize` returns the
/// canonical `Signal`, and form-stability holds for all three.
#[test]
fn signal_return_sync_share_one_form() {
    let canonical = RuntimeEdgeType::Signal.lower();
    for e in collision_class() {
        // Same edge-form.
        assert_eq!(e.lower(), canonical, "{e:?} should share the Signal form");
        // recognize returns the canonical representative for the whole class.
        assert_eq!(e.lower().recognize(), Some(RuntimeEdgeType::Signal));
        // Form-stability: lower ∘ recognize ∘ lower == lower (the honest
        // restatement of "round-trip" for a surjective class).
        let twice = e.lower().recognize().unwrap().lower();
        assert_eq!(twice, e.lower(), "{e:?} form was not stable under re-lowering");
    }
}

/// Forms that no legacy edge produces — persistent, reactive, or an unnamed
/// bit/attr combination — must `recognize` to `None`.
#[test]
fn recognize_rejects_non_legacy_forms() {
    let base = RuntimeEdgeType::Signal.lower();

    // A persistent variant of an otherwise-Signal form has no legacy name.
    let persistent = EdgeForm::new(base.bits, EdgeAttrs::new(true, false, Cardinality::One, false));
    assert_eq!(persistent.recognize(), None);

    // A reactive form is a V2.3 Subscribe, not a legacy edge.
    let reactive = EdgeForm::new(base.bits, EdgeAttrs::new(false, false, Cardinality::One, true));
    assert_eq!(reactive.recognize(), None);

    // Grant without Send/Bind is a bit combination no named point uses.
    let grant_only = EdgeForm::new(EdgeBits::new(true, false, false, true), EdgeAttrs::plain());
    assert_eq!(grant_only.recognize(), None);

    // The empty edge (no Refer) is not a legacy point either.
    let empty = EdgeForm::new(EdgeBits::none(), EdgeAttrs::plain());
    assert_eq!(empty.recognize(), None);
}

/// Closure: composing any two lowered forms yields a valid form; compose is
/// commutative (it is a lattice join) and idempotent.
#[test]
fn compose_is_closed_commutative_idempotent() {
    let edges = all_edges();
    for a in edges {
        let fa = a.lower();
        // Idempotent: joining a form with itself changes nothing.
        assert_eq!(fa.compose(fa), fa, "{a:?} compose with self should be identity");
        for b in edges {
            let fb = b.lower();
            let ab = fa.compose(fb);
            let ba = fb.compose(fa);
            assert!(ab.is_valid(), "compose({a:?},{b:?}) produced an invalid form");
            assert_eq!(ab, ba, "compose({a:?},{b:?}) is not commutative");
        }
    }
}

/// The refer-floor invariant rejects visibility violations directly: any bits
/// carrying causality/lifecycle/authority without `Refer` are ill-formed.
#[test]
fn refer_floor_rejects_visibility_violations() {
    assert!(!EdgeBits::new(false, true, false, false).floor_ok(), "Send without Refer");
    assert!(!EdgeBits::new(false, false, true, false).floor_ok(), "Bind without Refer");
    assert!(!EdgeBits::new(false, false, false, true).floor_ok(), "Grant without Refer");
    // The empty edge and pure Refer are both fine.
    assert!(EdgeBits::none().floor_ok());
    assert!(EdgeBits::refer_only().floor_ok());
}
