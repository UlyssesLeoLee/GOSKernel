//! V2.4b — `grant_edges_from_specs` / `capability_check`: bridging V2.4a's
//! abstract `reachable_via_grant` primitive to real `gos_protocol::EdgeSpec`
//! records (`gos_mutation_dispatch::capability`, ADR-001 §2.3).
//!
//! These tests pin down the *filter*: which `RuntimeEdgeType`s carry `grant`
//! (currently `Use` and `Call`, per `EdgeBits::grant`'s `lower()` table) and
//! therefore widen a capability path, and which (`Depend`, `Signal`, ...) do
//! not. The end-to-end `capability_check` tests mirror V2.4a's hot-plug /
//! fault-containment scenarios but driven entirely by `EdgeSpec` tables.
//!
//! V2.4c ([`doc/ADR-006`] §三, option A — "shadow verification", no
//! ABI/production changes): the trailing three tests restate ADR-001 §5's
//! literal claim — "claim/revoke 退化为 Grant 边的 create/delete" — directly:
//! granting a capability *is* adding a `Call`/`Use` edge, revoking it *is*
//! removing that edge, and `capability_check` needs no separate "claim
//! table" to stay in sync with either operation.
//!
//! [`doc/ADR-006`]: ../../../doc/ADR-006-capability-graph-migration.md
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "capability_specs.rs", type: "file", language: "rust"}),
//!   (gefs:Function {name: "grant_edges_from_specs", type: "function", visibility: "pub"}),
//!   (cc:Function {name: "capability_check", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "call_and_use_edges_grant_capability", type: "function"}),
//!   (t2:Function {name: "depend_and_signal_edges_do_not_grant", type: "function"}),
//!   (t3:Function {name: "transitive_call_chain_grants_capability", type: "function"}),
//!   (t4:Function {name: "self_capability_holds_even_for_unknown_node", type: "function"}),
//!   (t5:Function {name: "hot_plug_unrelated_edge_does_not_break_existing_capability", type: "function"}),
//!   (t6:Function {name: "fault_subgraph_is_bounded_by_grant_closure_via_specs", type: "function"}),
//!   (t7:Function {name: "over_capacity_specs_grants_nothing_except_self", type: "function"}),
//!   (t8:Function {name: "claim_is_grant_edge_create_revoke_is_delete", type: "function"}),
//!   (t9:Function {name: "revoking_one_module_does_not_affect_another", type: "function"}),
//!   (t10:Function {name: "revoking_all_of_a_module_capabilities_contains_it", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5), (f)-[:CONTAINS]->(t6),
//!   (f)-[:CONTAINS]->(t7), (f)-[:CONTAINS]->(t8), (f)-[:CONTAINS]->(t9),
//!   (f)-[:CONTAINS]->(t10),
//!   (t1)-[:USES]->(cc), (t2)-[:USES]->(gefs), (t3)-[:USES]->(cc),
//!   (t4)-[:USES]->(cc), (t5)-[:USES]->(cc), (t6)-[:USES]->(cc), (t7)-[:USES]->(cc),
//!   (t8)-[:USES]->(cc), (t9)-[:USES]->(cc), (t10)-[:USES]->(cc);
//! ```

use gos_protocol::{EdgeId, EdgeSpec, NodeId, RoutePolicy, RuntimeEdgeType};
use gos_mutation_dispatch::capability::{capability_check, grant_edges_from_specs, GrantEdge, MAX_CAPABILITY_NODES};
use gos_mutation_dispatch::NodeId as Idx;

const A: NodeId = NodeId([1u8; 16]);
const B: NodeId = NodeId([2u8; 16]);
const C: NodeId = NodeId([3u8; 16]);
const D: NodeId = NodeId([4u8; 16]);
const E: NodeId = NodeId([5u8; 16]);
const F: NodeId = NodeId([6u8; 16]);
const UNKNOWN: NodeId = NodeId([0xffu8; 16]);

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

/// `Call` and `Use` edges (ADR-001 §2.3: `EdgeBits::grant == true`) are
/// enough for `capability_check` to see a path.
#[test]
fn call_and_use_edges_grant_capability() {
    let specs = [edge(A, B, RuntimeEdgeType::Call), edge(B, C, RuntimeEdgeType::Use)];
    assert!(capability_check(&specs, A, C));
}

/// `Depend`/`Signal` edges (`grant == false`) are filtered out by
/// `grant_edges_from_specs` entirely — they don't even appear in the
/// derived node/edge tables.
#[test]
fn depend_and_signal_edges_do_not_grant() {
    let specs = [edge(A, B, RuntimeEdgeType::Depend), edge(B, C, RuntimeEdgeType::Signal)];

    let mut nodes = [NodeId::ZERO; MAX_CAPABILITY_NODES];
    let mut edges = [GrantEdge::new(Idx(0), Idx(0)); MAX_CAPABILITY_NODES];
    let (node_count, edge_count) = grant_edges_from_specs(&specs, &mut nodes, &mut edges).unwrap();
    assert_eq!(node_count, 0);
    assert_eq!(edge_count, 0);

    assert!(!capability_check(&specs, A, C));
    // Self-capability holds regardless — even with no grant edges at all.
    assert!(capability_check(&specs, A, A));
}

/// A --Call--> B --Use--> C: a delegation chain grants A transitive
/// capability over C, derived straight from `EdgeSpec`s. Grant is
/// directional — the reverse path is not implied.
#[test]
fn transitive_call_chain_grants_capability() {
    let specs = [edge(A, B, RuntimeEdgeType::Call), edge(B, C, RuntimeEdgeType::Use)];
    assert!(capability_check(&specs, A, C));
    assert!(!capability_check(&specs, C, A));
}

/// `from == to` is `true` even for a node that appears in no `EdgeSpec` at
/// all — the reflexive rule is checked before the table is built.
#[test]
fn self_capability_holds_even_for_unknown_node() {
    let specs = [edge(A, B, RuntimeEdgeType::Call)];
    assert!(capability_check(&specs, UNKNOWN, UNKNOWN));
    assert!(!capability_check(&specs, UNKNOWN, A));
}

/// Hot-plug demo (V2.4 killer demo, via real `EdgeSpec`s): A's capability
/// over C survives an unrelated edge being unplugged and a fresh node F
/// appearing — `capability_check` recomputes from the current table.
#[test]
fn hot_plug_unrelated_edge_does_not_break_existing_capability() {
    let before = [
        edge(A, B, RuntimeEdgeType::Call),
        edge(B, C, RuntimeEdgeType::Use),
        edge(D, E, RuntimeEdgeType::Call), // unrelated
    ];
    assert!(capability_check(&before, A, C));

    let after = [
        edge(A, B, RuntimeEdgeType::Call),
        edge(B, C, RuntimeEdgeType::Use),
        edge(D, F, RuntimeEdgeType::Call), // re-plugged elsewhere
    ];
    assert!(capability_check(&after, A, C));
}

/// Fault-containment demo (V2.4 killer demo, via real `EdgeSpec`s): A's
/// Grant-reachable set is {B, C} only; D/E form a disjoint component with no
/// `Call`/`Use` path from A.
#[test]
fn fault_subgraph_is_bounded_by_grant_closure_via_specs() {
    let specs = [
        edge(A, B, RuntimeEdgeType::Call),
        edge(B, C, RuntimeEdgeType::Use),
        edge(D, E, RuntimeEdgeType::Call), // disjoint component
    ];
    assert!(capability_check(&specs, A, B));
    assert!(capability_check(&specs, A, C));
    assert!(!capability_check(&specs, A, D));
    assert!(!capability_check(&specs, A, E));
}

/// More distinct grant-edge endpoints than `MAX_CAPABILITY_NODES` makes
/// `grant_edges_from_specs` report `None`, and `capability_check` treats
/// that as "grants nothing" — even a pair that would otherwise be a direct
/// `Call` edge. The `from == to` reflexive case is unaffected, since it's
/// checked before the table is built.
#[test]
fn over_capacity_specs_grants_nothing_except_self() {
    let mut specs = Vec::new();
    for i in 0..=(MAX_CAPABILITY_NODES as u8) {
        specs.push(edge(NodeId([i; 16]), NodeId([i.wrapping_add(1); 16]), RuntimeEdgeType::Call));
    }
    let first = NodeId([0u8; 16]);
    let second = NodeId([1u8; 16]);
    assert!(!capability_check(&specs, first, second));
    assert!(capability_check(&specs, first, first));
}

/// ADR-001 §5 / ADR-006 §三 option A: "claim/revoke 退化为 Grant 边的
/// create/delete". Granting `module` a capability over `resource` *is*
/// adding a `Call` edge `module -> resource`; revoking it *is* removing that
/// edge — `capability_check` reflects exactly the current edge table, no
/// separate claim record to keep in sync.
#[test]
fn claim_is_grant_edge_create_revoke_is_delete() {
    let module = NodeId([10u8; 16]);
    let resource = NodeId([20u8; 16]);

    let no_claim: [EdgeSpec; 0] = [];
    assert!(!capability_check(&no_claim, module, resource));

    let claimed = [edge(module, resource, RuntimeEdgeType::Call)];
    assert!(capability_check(&claimed, module, resource));

    // "release_claim" == delete the edge — back to `no_claim`'s table.
    assert!(!capability_check(&no_claim, module, resource));
}

/// Two modules' claims are independent `Call` edges: M1->R1 and M2->R2.
/// Removing M1's edge (its claim is released) does not affect M2's — each
/// claim/revoke is a local edge create/delete, not a shared table entry.
#[test]
fn revoking_one_module_does_not_affect_another() {
    let m1 = NodeId([11u8; 16]);
    let r1 = NodeId([21u8; 16]);
    let m2 = NodeId([12u8; 16]);
    let r2 = NodeId([22u8; 16]);

    let both = [edge(m1, r1, RuntimeEdgeType::Call), edge(m2, r2, RuntimeEdgeType::Call)];
    assert!(capability_check(&both, m1, r1));
    assert!(capability_check(&both, m2, r2));

    // M1's claim is revoked (its edge is removed); M2's is untouched.
    let m1_revoked = [edge(m2, r2, RuntimeEdgeType::Call)];
    assert!(!capability_check(&m1_revoked, m1, r1));
    assert!(capability_check(&m1_revoked, m2, r2));
}

/// `gos-supervisor::revoke_capabilities(handle)` revokes *all* of a
/// module's capabilities at once. Expressed as Grant edges: M holds two
/// transitive claims (M->Mid->R1, M->Mid->R2); revoking M removes *all* of
/// M's outgoing Grant edges, after which M can reach neither R1 nor R2 — the
/// same closure-boundedness V2.4a/b's fault-containment tests prove, here
/// driven by the revoke operation itself.
#[test]
fn revoking_all_of_a_module_capabilities_contains_it() {
    let m = NodeId([13u8; 16]);
    let mid = NodeId([14u8; 16]);
    let r1 = NodeId([23u8; 16]);
    let r2 = NodeId([24u8; 16]);

    let before = [
        edge(m, mid, RuntimeEdgeType::Call),
        edge(mid, r1, RuntimeEdgeType::Use),
        edge(mid, r2, RuntimeEdgeType::Use),
    ];
    assert!(capability_check(&before, m, r1));
    assert!(capability_check(&before, m, r2));

    // revoke_capabilities(m): remove M's only outgoing Grant edge.
    let m_revoked = [edge(mid, r1, RuntimeEdgeType::Use), edge(mid, r2, RuntimeEdgeType::Use)];
    assert!(!capability_check(&m_revoked, m, r1));
    assert!(!capability_check(&m_revoked, m, r2));
    // M's own identity is unaffected by revocation.
    assert!(capability_check(&m_revoked, m, m));
}
