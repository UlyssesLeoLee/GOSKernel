//! V2.5a — shadow-verification of [`doc/ADR-005`] §三 option A's core premise
//! ("provisional nodes: 可见、可连边、可渲染，但不能持有 claim/quota，直到被
//! 显式 promote") against the two independent primitives V2.3c and V2.4b/c
//! already shipped:
//!
//!   * [`gos_rewrite::reactive::propagate_with`] (V2.3c) — render-Subscribe
//!     propagation over a `&[Subscription]` table, keyed by
//!     `gos_rewrite::NodeId` (table-local render index).
//!   * [`gos_rewrite::capability::capability_check`] (V2.4b/c) — Grant-path
//!     reachability over a `&[gos_protocol::EdgeSpec]` table, keyed by
//!     `gos_protocol::NodeId` (V2 graph identity).
//!
//! Neither primitive's signature references the other's table or node-id
//! space. ADR-005 §三 named two preconditions for option A
//! ("provisional node 的渲染策略，接 ADR-002 §六" / "promote 的触发者与权限，
//! 接 ADR-001 §五") that were open on 2026-06-08 because neither primitive
//! existed yet. With both now shipped, this file restates ADR-005's premise
//! as machine-checked facts about the *existing* APIs — zero new types, zero
//! ABI change, and not a commitment to option A over B/C (the independence
//! holds regardless of how "provisional" is eventually represented).
//!
//! [`doc/ADR-005`]: ../../../doc/ADR-005-node-mutation.md
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "provisional_render.rs", type: "file", language: "rust"}),
//!   (cc:Function {name: "capability_check", type: "function", visibility: "pub"}),
//!   (pw:Function {name: "propagate_with", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "provisional_node_renders_without_capability", type: "function"}),
//!   (t2:Function {name: "promoting_a_node_is_adding_one_grant_edge", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2),
//!   (t1)-[:USES]->(cc), (t1)-[:USES]->(pw), (t2)-[:USES]->(cc);
//! ```

use gos_protocol::{EdgeId, EdgeSpec, NodeId, Region, RoutePolicy, RuntimeEdgeType};
use gos_rewrite::capability::capability_check;
use gos_rewrite::reactive::{propagate_with, Subscription};
use gos_rewrite::NodeId as Idx;

const KIND_REPAINT: u32 = 11;

const AUTHORITY: NodeId = NodeId([1u8; 16]);
const PROMOTED: NodeId = NodeId([2u8; 16]);
const PROVISIONAL: NodeId = NodeId([3u8; 16]);

fn edge(from: NodeId, to: NodeId, edge_type: RuntimeEdgeType) -> EdgeSpec {
    EdgeSpec {
        edge_id: EdgeId::ZERO,
        from_node: from,
        to_node: to,
        edge_type,
        weight: 1.0,
        acl_mask: 0,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

/// ADR-005 §三 sub-question 1 ("provisional node 的渲染策略，接 ADR-002 §六"):
/// `PROVISIONAL` holds *no* Grant edge from `AUTHORITY` — by ADR-005 option
/// A's definition, that *is* "not yet promoted" — yet its render-side
/// `Subscription` (a wholly separate table, keyed by a wholly separate
/// `NodeId` space) propagates exactly as for any other node. Rendering
/// (`propagate_with`) never consults capability (`capability_check`):
/// ADR-002 §六's "renderer is a pure function of graph subscription" already
/// makes rendering promotion-status-blind, by construction.
#[test]
fn provisional_node_renders_without_capability() {
    let specs = [edge(AUTHORITY, PROMOTED, RuntimeEdgeType::Call)];

    // PROVISIONAL holds no capability relationship with AUTHORITY at all —
    // the defining property of "not yet promoted".
    assert!(capability_check(&specs, AUTHORITY, PROMOTED));
    assert!(!capability_check(&specs, AUTHORITY, PROVISIONAL));
    // ...but PROVISIONAL still has a self-identity (V2.4b reflexive rule) —
    // "not promoted" is not "doesn't exist".
    assert!(capability_check(&specs, PROVISIONAL, PROVISIONAL));

    // PROVISIONAL's render node (a wholly separate NodeId space, V2.3c) is
    // a normal Subscribe target — propagation neither knows nor cares that
    // the matching `gos_protocol::NodeId` above holds zero capability.
    let render_target = Idx(300);
    let render_subscriber = Idx(301);
    let table = [Subscription::new(render_target, render_subscriber, Region::EVERYTHING)];

    let mut dispatched = Vec::new();
    propagate_with(&table, render_target, Region::EVERYTHING, KIND_REPAINT, |subscriber, kind| {
        dispatched.push((subscriber.0, kind));
    });
    assert_eq!(dispatched, vec![(301, KIND_REPAINT)]);
}

/// ADR-005 §三 sub-question 2 ("promote 的触发者与权限，接 ADR-001 §五"):
/// mirroring V2.4c's `claim_is_grant_edge_create_revoke_is_delete`, *promote*
/// is itself nothing but one new Grant edge `AUTHORITY -> PROVISIONAL` — the
/// same `capability_check` mechanism V2.4b/c already shipped, no new
/// "promotion" primitive needed. (Render-side independence from this
/// capability-table state is established by
/// `provisional_node_renders_without_capability` above — promotion never
/// touches the `Subscription` table at all.)
#[test]
fn promoting_a_node_is_adding_one_grant_edge() {
    let before = [edge(AUTHORITY, PROMOTED, RuntimeEdgeType::Call)];
    assert!(!capability_check(&before, AUTHORITY, PROVISIONAL));

    // "promote(PROVISIONAL)" == AUTHORITY gains a Call edge to it — same
    // create/delete mechanism as claim/revoke (V2.4c), applied to promotion.
    let after = [
        edge(AUTHORITY, PROMOTED, RuntimeEdgeType::Call),
        edge(AUTHORITY, PROVISIONAL, RuntimeEdgeType::Call),
    ];
    assert!(capability_check(&after, AUTHORITY, PROVISIONAL));
    // PROMOTED's existing capability is unaffected by PROVISIONAL's promotion.
    assert!(capability_check(&after, AUTHORITY, PROMOTED));
}
