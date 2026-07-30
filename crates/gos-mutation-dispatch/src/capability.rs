//! V2.4a — Grant-path reachability query (ADR-001 §2.2 `grant`; V2.4 plan
//! line: "capability 检查 = Grant 路径图查询").
//!
//! [`EdgeBits::grant`](../../gos_protocol/edge_algebra/struct.EdgeBits.html#structfield.grant)
//! marks an edge as conferring authority: `A --Grant--> B` means `A` is
//! granted the right to invoke `B`'s exported capability. A chain of `Grant`
//! edges is a delegation path — [`reachable_via_grant`] answers the
//! capability-check question "is there a (possibly transitive) `Grant` path
//! from `from` to `to`?", which V2.4 defines the capability check *to be*
//! (replacing any ambient-authority / pointer-based check).
//!
//! Two of V2.4's killer-demo properties fall out of this being a pure,
//! freshly-recomputed graph query rather than a cached/derived structure:
//! - **hot-plug**: edits to unrelated parts of `edges` (a node re-plugged
//!   elsewhere in the graph) cannot change the result for an existing
//!   `(from, to)` pair whose path is untouched — re-querying always reflects
//!   *current* topology, so an externally-held capability "just works".
//! - **fault containment**: `to` nodes with no `Grant` path from a faulting
//!   `from` are, by construction, outside `from`'s authority — a faulting
//!   subgraph is firewalled to exactly its `Grant`-reachable set.
//!
//! `no_std`, no `alloc`: BFS over a caller-supplied node list with
//! fixed-capacity arrays, mirroring
//! [`boot::resolve_boot_order`](crate::boot::resolve_boot_order)'s
//! "the table is the graph" style.
//!
//! V2.4a's [`reachable_via_grant`] is an abstract primitive over small
//! [`crate::NodeId`] indices, hand-fed a [`GrantEdge`] table. V2.4b bridges
//! it to the runtime's actual edge vocabulary: [`grant_edges_from_specs`]
//! derives that table from real `gos_protocol::EdgeSpec` records by
//! filtering on `edge_type.lower().bits.grant` (ADR-001 §2.3 — currently
//! `Use` and `Call`) and interning each distinct 128-bit
//! `gos_protocol::NodeId` endpoint into a small index; [`capability_check`]
//! wraps both steps into a single `(specs, from, to) -> bool` query. Wiring
//! this into the real syscall/trap dispatch path (V2.4c+) is future work,
//! per [`doc/ADR-001`].
//!
//! [`doc/ADR-001`]: ../../../doc/ADR-001-edge-algebra-constitution.md
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "capability.rs", type: "file", language: "rust"}),
//!   (m:Module {name: "gos_mutation_dispatch::capability", type: "module"}),
//!   (ge:Class {name: "GrantEdge", type: "struct", signature: "{from,to: NodeId}"}),
//!   (ge_new:Function {name: "GrantEdge::new", type: "function", visibility: "pub"}),
//!   (rg:Function {name: "reachable_via_grant", type: "function", visibility: "pub"}),
//!   (gefs:Function {name: "grant_edges_from_specs", type: "function", visibility: "pub"}),
//!   (cc:Function {name: "capability_check", type: "function", visibility: "pub"}),
//!   (eb:Class {name: "gos_protocol::edge_algebra::EdgeBits", type: "struct"}),
//!   (es:Class {name: "gos_protocol::EdgeSpec", type: "struct"}),
//!   (f)-[:CONTAINS]->(m),
//!   (m)-[:CONTAINS]->(ge), (m)-[:CONTAINS]->(rg),
//!   (m)-[:CONTAINS]->(gefs), (m)-[:CONTAINS]->(cc),
//!   (ge)-[:HAS_METHOD]->(ge_new),
//!   (rg)-[:USES]->(ge),
//!   (ge)-[:REPRESENTS]->(eb),
//!   (gefs)-[:READS]->(es), (gefs)-[:PRODUCES]->(ge),
//!   (cc)-[:CALLS]->(gefs), (cc)-[:CALLS]->(rg);
//! ```

use crate::NodeId;

/// Max nodes a single [`reachable_via_grant`] query can cover. Mirrors
/// [`crate::boot::MAX_BOOT_NODES`] — small static graphs, fixed-size arrays,
/// no `alloc`.
pub const MAX_CAPABILITY_NODES: usize = 32;

/// One `Grant` edge (ADR-001 §2.2): `from` is granted authority to invoke
/// `to`'s exported capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantEdge {
    pub from: NodeId,
    pub to: NodeId,
}

impl GrantEdge {
    pub const fn new(from: NodeId, to: NodeId) -> Self {
        Self { from, to }
    }
}

fn index_of(nodes: &[NodeId], id: NodeId) -> Option<usize> {
    nodes.iter().position(|n| n.0 == id.0)
}

/// Is there a path of [`GrantEdge`]s from `from` to `to`?
///
/// `from == to` is trivially `true` — every node holds its own capabilities.
/// `nodes` bounds the query (must list every node `edges` references, with
/// `nodes.len() <= MAX_CAPABILITY_NODES`); a missing endpoint or an
/// over-capacity table returns `false` rather than panicking — a
/// malformed/oversized table grants nothing.
pub fn reachable_via_grant(nodes: &[NodeId], edges: &[GrantEdge], from: NodeId, to: NodeId) -> bool {
    if from.0 == to.0 {
        return true;
    }
    if nodes.len() > MAX_CAPABILITY_NODES {
        return false;
    }
    let Some(start) = index_of(nodes, from) else {
        return false;
    };
    let Some(goal) = index_of(nodes, to) else {
        return false;
    };

    let mut visited = [false; MAX_CAPABILITY_NODES];
    let mut stack = [0usize; MAX_CAPABILITY_NODES];
    let mut sp = 0usize;
    visited[start] = true;
    stack[sp] = start;
    sp += 1;

    while sp > 0 {
        sp -= 1;
        let cur = stack[sp];
        let cur_id = nodes[cur];
        for edge in edges {
            if edge.from.0 == cur_id.0 {
                if let Some(next) = index_of(nodes, edge.to) {
                    if next == goal {
                        return true;
                    }
                    if !visited[next] {
                        visited[next] = true;
                        stack[sp] = next;
                        sp += 1;
                    }
                }
            }
        }
    }
    false
}

/// Bridge from real runtime edges to a [`GrantEdge`] table (V2.4b): filter
/// `specs` to those whose `edge_type.lower().bits.grant` is set (ADR-001
/// §2.3 — currently `Use` and `Call`), interning each distinct
/// `gos_protocol::NodeId` endpoint into `nodes_out` (first-occurrence order)
/// and emitting the corresponding [`GrantEdge`] (over those interned
/// [`crate::NodeId`] indices) into `edges_out`.
///
/// Returns `(node_count, edge_count)`. `None` if `specs` has more distinct
/// `grant`-edge endpoints than `nodes_out` holds, or more `grant` edges than
/// `edges_out` holds — an over-capacity table is reported rather than
/// silently truncated, and [`capability_check`] treats `None` as "grants
/// nothing" (same stance as [`reachable_via_grant`]'s over-capacity case).
pub fn grant_edges_from_specs(
    specs: &[gos_protocol::EdgeSpec],
    nodes_out: &mut [gos_protocol::NodeId],
    edges_out: &mut [GrantEdge],
) -> Option<(usize, usize)> {
    fn intern(nodes: &mut [gos_protocol::NodeId], len: &mut usize, id: gos_protocol::NodeId) -> Option<u32> {
        if let Some(pos) = nodes[..*len].iter().position(|n| *n == id) {
            return Some(pos as u32);
        }
        if *len >= nodes.len() {
            return None;
        }
        nodes[*len] = id;
        *len += 1;
        Some((*len - 1) as u32)
    }

    let mut node_count = 0usize;
    let mut edge_count = 0usize;
    for spec in specs {
        if !spec.edge_type.lower().bits.grant {
            continue;
        }
        let from = intern(nodes_out, &mut node_count, spec.from_node)?;
        let to = intern(nodes_out, &mut node_count, spec.to_node)?;
        if edge_count >= edges_out.len() {
            return None;
        }
        edges_out[edge_count] = GrantEdge::new(NodeId(from), NodeId(to));
        edge_count += 1;
    }
    Some((node_count, edge_count))
}

/// Capability check (V2.4 plan: "capability 检查 = Grant 路径图查询") over real
/// runtime edges: derive a [`GrantEdge`] table from `specs` via
/// [`grant_edges_from_specs`] and ask whether `to` is [`reachable_via_grant`]
/// from `from`.
///
/// `from == to` is `true` regardless of `specs` — checked first, so the
/// reflexive rule holds even for endpoints that appear in no edge at all. An
/// over-capacity `specs` table (more distinct `grant`-edge endpoints than
/// [`MAX_CAPABILITY_NODES`]) "grants nothing": `false` unless `from == to`.
pub fn capability_check(specs: &[gos_protocol::EdgeSpec], from: gos_protocol::NodeId, to: gos_protocol::NodeId) -> bool {
    if from == to {
        return true;
    }

    let mut nodes = [gos_protocol::NodeId::ZERO; MAX_CAPABILITY_NODES];
    let mut edges = [GrantEdge::new(NodeId(0), NodeId(0)); MAX_CAPABILITY_NODES];
    let Some((node_count, edge_count)) = grant_edges_from_specs(specs, &mut nodes, &mut edges) else {
        return false;
    };

    let Some(from_idx) = nodes[..node_count].iter().position(|&n| n == from) else {
        return false;
    };
    let Some(to_idx) = nodes[..node_count].iter().position(|&n| n == to) else {
        return false;
    };

    let index_nodes: [NodeId; MAX_CAPABILITY_NODES] = core::array::from_fn(|i| NodeId(i as u32));
    reachable_via_grant(
        &index_nodes[..node_count],
        &edges[..edge_count],
        NodeId(from_idx as u32),
        NodeId(to_idx as u32),
    )
}
