//! Property tests for the V2.3a `Subscribe` edge + `Region` predicate
//! (`gos_protocol::edge_algebra`, [`doc/ADR-001`] §2.2/§2.3).
//!
//! `Subscribe` was predicted but not built in V2.0: `EdgeAttrs::reactive` was
//! modeled as a bare `bool`, with its region-predicate payload explicitly
//! deferred ("...arrives with `Subscribe` in V2.3"). These tests pin down
//! that payload (`Region`) and the `Subscribe` constructor that pairs it
//! with the `Refer + reactive` form ADR-001 §2.3 specifies — *zero* new
//! primitives, exactly as §三 ("完备性") argues.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "subscribe.rs", type: "file", language: "rust"}),
//!   (sub:Class {name: "Subscribe", type: "struct"}),
//!   (rg:Class {name: "Region", type: "struct"}),
//!   (t1:Function {name: "subscribe_form_is_refer_only_and_reactive", type: "function"}),
//!   (t2:Function {name: "subscribe_has_no_legacy_recognition", type: "function"}),
//!   (t3:Function {name: "subscribe_compose_with_signal_preserves_both", type: "function"}),
//!   (t4:Function {name: "region_overlap_is_symmetric", type: "function"}),
//!   (t5:Function {name: "everything_overlaps_any_nonempty_region", type: "function"}),
//!   (t6:Function {name: "disjoint_regions_dont_overlap", type: "function"}),
//!   (t7:Function {name: "empty_region_overlaps_nothing", type: "function"}),
//!   (t8:Function {name: "identical_nonempty_regions_overlap", type: "function"}),
//!   (t9:Function {name: "adjacent_regions_dont_overlap", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5), (f)-[:CONTAINS]->(t6),
//!   (f)-[:CONTAINS]->(t7), (f)-[:CONTAINS]->(t8), (f)-[:CONTAINS]->(t9),
//!   (t1)-[:USES]->(sub), (t2)-[:USES]->(sub), (t3)-[:USES]->(sub),
//!   (t4)-[:USES]->(rg), (t5)-[:USES]->(rg), (t6)-[:USES]->(rg),
//!   (t7)-[:USES]->(rg), (t8)-[:USES]->(rg), (t9)-[:USES]->(rg);
//! ```

use gos_protocol::{Cardinality, EdgeAttrs, EdgeBits, Region, RuntimeEdgeType, Subscribe};

/// `Subscribe` lowers to exactly `Refer` + `reactive=true`, nothing else
/// (ADR-001 §2.3 row: "不需要新 primitive——只是带 reactive 属性的 Refer").
#[test]
fn subscribe_form_is_refer_only_and_reactive() {
    let s = Subscribe::new(Region::new(0, 0, 100, 100));
    assert_eq!(s.form.bits, EdgeBits::refer_only(), "Subscribe carries only the Refer floor");
    assert_eq!(
        s.form.attrs,
        EdgeAttrs::new(false, false, Cardinality::One, true),
        "Subscribe is `plain` except reactive=true"
    );
    assert!(s.form.is_valid());
}

/// A reactive form has no legacy name — `recognize` must reject it, exactly
/// as `recognize_rejects_non_legacy_forms` (V2.0) already pins down for a
/// bare reactive `EdgeForm`. `Subscribe` doesn't change that.
#[test]
fn subscribe_has_no_legacy_recognition() {
    assert_eq!(Subscribe::everything().form.recognize(), None);
    assert_eq!(Subscribe::new(Region::new(1, 2, 3, 4)).form.recognize(), None);
}

/// Composing a reactive `Refer` with `Signal` (`Refer+Send`) must keep
/// *both* the causal `send` bit and the `reactive` attribute. This is the
/// algebraic foundation of "theme diffusion and dirty-rect rendering share
/// one mechanism" (V2.3 plan, "关键涌现证据"): an edge can be `Signal` *and*
/// `Subscribe` at once, and compose proves neither property is lost.
#[test]
fn subscribe_compose_with_signal_preserves_both() {
    let composed = Subscribe::everything().form.compose(RuntimeEdgeType::Signal.lower());
    assert!(composed.bits.refer);
    assert!(composed.bits.send, "Send from Signal must survive compose");
    assert!(composed.attrs.reactive, "reactive from Subscribe must survive compose");
    assert!(composed.is_valid());
}

/// `overlaps` is symmetric by construction — pin it down across the
/// overlapping / disjoint / adjacent / EVERYTHING cases the other tests rely on.
#[test]
fn region_overlap_is_symmetric() {
    let pairs = [
        (Region::new(0, 0, 10, 10), Region::new(5, 5, 10, 10)),  // overlapping
        (Region::new(0, 0, 10, 10), Region::new(20, 20, 5, 5)),  // disjoint
        (Region::new(0, 0, 10, 10), Region::new(10, 0, 10, 10)), // adjacent (touching edge)
        (Region::EVERYTHING, Region::new(3, 3, 1, 1)),
    ];
    for (a, b) in pairs {
        assert_eq!(a.overlaps(&b), b.overlaps(&a), "overlap must be symmetric for {a:?}/{b:?}");
    }
}

/// `Region::EVERYTHING` is the theme-diffusion subscription: it overlaps any
/// non-empty region, including itself.
#[test]
fn everything_overlaps_any_nonempty_region() {
    assert!(Region::EVERYTHING.overlaps(&Region::new(10, 10, 5, 5)));
    assert!(Region::new(10, 10, 5, 5).overlaps(&Region::EVERYTHING));
    assert!(Region::EVERYTHING.overlaps(&Region::EVERYTHING));
}

/// Two rects with no shared pixels don't overlap — the dirty-rect filter
/// that keeps an unrelated render node from repainting (Demo C).
#[test]
fn disjoint_regions_dont_overlap() {
    let a = Region::new(0, 0, 10, 10);
    let b = Region::new(20, 20, 10, 10);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}

/// A zero-area region overlaps nothing — not even itself, and not even
/// `EVERYTHING`. A `Subscribe` with an empty region is inert.
#[test]
fn empty_region_overlaps_nothing() {
    let empty = Region::new(5, 5, 0, 10);
    assert!(!empty.overlaps(&Region::new(5, 5, 10, 10)));
    assert!(!empty.overlaps(&empty));
    assert!(!empty.overlaps(&Region::EVERYTHING));
    assert!(!Region::EVERYTHING.overlaps(&empty));
}

/// A region always overlaps an identical copy of itself (non-empty case).
#[test]
fn identical_nonempty_regions_overlap() {
    let r = Region::new(7, 7, 3, 3);
    assert!(r.overlaps(&r));
}

/// Rects that share only a boundary line have zero-area intersection and do
/// not overlap — strict inequality, not `<=`.
#[test]
fn adjacent_regions_dont_overlap() {
    let a = Region::new(0, 0, 10, 10);
    let b = Region::new(10, 0, 10, 10);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}
