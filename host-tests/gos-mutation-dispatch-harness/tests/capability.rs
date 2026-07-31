//! V2.4a — `reachable_via_grant`, the Grant-path capability-check query
//! (`gos_mutation_dispatch::capability`, ADR-001 §2.2 `grant`).
//!
//! These tests pin down the graph-query semantics V2.4 defines "capability
//! check" to be: a path of [`GrantEdge`]s from `from` to `to`. The hot-plug
//! and fault-containment tests are the host-provable cores of two of V2.4's
//! killer demos — both fall out of `reachable_via_grant` being a pure
//! function of the *current* edge table, not a cached/derived structure.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "capability.rs", type: "file", language: "rust"}),
//!   (rg:Function {name: "reachable_via_grant", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "direct_grant_edge_is_reachable", type: "function"}),
//!   (t2:Function {name: "transitive_grant_chain_is_reachable", type: "function"}),
//!   (t3:Function {name: "no_grant_path_is_unreachable", type: "function"}),
//!   (t4:Function {name: "self_is_always_reachable", type: "function"}),
//!   (t5:Function {name: "unrelated_topology_change_does_not_break_existing_path", type: "function"}),
//!   (t6:Function {name: "fault_subgraph_is_bounded_by_grant_closure", type: "function"}),
//!   (t7:Function {name: "missing_endpoint_is_unreachable", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5), (f)-[:CONTAINS]->(t6),
//!   (f)-[:CONTAINS]->(t7),
//!   (t1)-[:USES]->(rg), (t2)-[:USES]->(rg), (t3)-[:USES]->(rg),
//!   (t4)-[:USES]->(rg), (t5)-[:USES]->(rg), (t6)-[:USES]->(rg), (t7)-[:USES]->(rg);
//! ```

use gos_mutation_dispatch::capability::{reachable_via_grant, GrantEdge};
use gos_mutation_dispatch::NodeId;

const A: NodeId = NodeId(1);
const B: NodeId = NodeId(2);
const C: NodeId = NodeId(3);
const D: NodeId = NodeId(4);
const E: NodeId = NodeId(5);

/// A --Grant--> B is a one-hop capability path.
#[test]
fn direct_grant_edge_is_reachable() {
    let nodes = [A, B];
    let edges = [GrantEdge::new(A, B)];
    assert!(reachable_via_grant(&nodes, &edges, A, B));
}

/// A --Grant--> B --Grant--> C: a delegation chain grants A transitive
/// authority over C, even with no direct A->C edge.
#[test]
fn transitive_grant_chain_is_reachable() {
    let nodes = [A, B, C];
    let edges = [GrantEdge::new(A, B), GrantEdge::new(B, C)];
    assert!(reachable_via_grant(&nodes, &edges, A, C));
    // And the reverse direction is NOT implied — Grant is directional.
    assert!(!reachable_via_grant(&nodes, &edges, C, A));
}

/// Two disjoint Grant edges (A->B, C->D) confer no path between their
/// unrelated endpoints.
#[test]
fn no_grant_path_is_unreachable() {
    let nodes = [A, B, C, D];
    let edges = [GrantEdge::new(A, B), GrantEdge::new(C, D)];
    assert!(!reachable_via_grant(&nodes, &edges, A, D));
    assert!(!reachable_via_grant(&nodes, &edges, A, C));
}

/// Every node holds its own capabilities — `from == to` is trivially
/// reachable, even with an empty edge table.
#[test]
fn self_is_always_reachable() {
    let nodes = [A];
    let edges: [GrantEdge; 0] = [];
    assert!(reachable_via_grant(&nodes, &edges, A, A));
}

/// Hot-plug demo (host-provable core): A's Grant path to C survives edits to
/// an unrelated part of the graph (D->E unplugged, F appears) — the query is
/// recomputed fresh from the current table, so an externally-held capability
/// "just works" regardless of what else changed.
#[test]
fn unrelated_topology_change_does_not_break_existing_path() {
    let nodes_before = [A, B, C, D, E];
    let edges_before = [
        GrantEdge::new(A, B),
        GrantEdge::new(B, C),
        GrantEdge::new(D, E), // unrelated
    ];
    assert!(reachable_via_grant(&nodes_before, &edges_before, A, C));

    // D->E is unplugged; F (a fresh node) appears with its own edge.
    const F: NodeId = NodeId(6);
    let nodes_after = [A, B, C, D, F];
    let edges_after = [
        GrantEdge::new(A, B),
        GrantEdge::new(B, C),
        GrantEdge::new(D, F), // re-plugged elsewhere
    ];
    assert!(reachable_via_grant(&nodes_after, &edges_after, A, C));
}

/// Fault-containment demo (host-provable core): A's Grant-reachable set is
/// {B, C} only. D and E form a disjoint component with no Grant path from A
/// — by construction, a fault at A cannot be A's authority over D/E, because
/// no such authority exists.
#[test]
fn fault_subgraph_is_bounded_by_grant_closure() {
    let nodes = [A, B, C, D, E];
    let edges = [
        GrantEdge::new(A, B),
        GrantEdge::new(B, C),
        GrantEdge::new(D, E), // disjoint component
    ];
    assert!(reachable_via_grant(&nodes, &edges, A, B));
    assert!(reachable_via_grant(&nodes, &edges, A, C));
    assert!(!reachable_via_grant(&nodes, &edges, A, D));
    assert!(!reachable_via_grant(&nodes, &edges, A, E));
}

/// `to` (or `from`) absent from `nodes` returns `false`, not a panic — a
/// malformed table grants nothing.
#[test]
fn missing_endpoint_is_unreachable() {
    let nodes = [A, B];
    let edges = [GrantEdge::new(A, B)];
    let unknown = NodeId(999);
    assert!(!reachable_via_grant(&nodes, &edges, A, unknown));
    assert!(!reachable_via_grant(&nodes, &edges, unknown, B));
}
