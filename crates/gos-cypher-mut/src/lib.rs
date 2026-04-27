#![no_std]

//! Phase H.1 — receptive Cypher mutation API.
//!
//! `k-cypher` already has a read-only Cypher v1 subset (browse nodes,
//! browse edges, `CALL activate(n)`, `CALL spawn(n)`, `CALL route(e)`).
//! H.1 adds the *write* half — but on a leash:
//!
//!   * Only edge mutations and `Mount`/`Use` rebinds are accepted.
//!     Node create/delete, NodeId reassignment, plugin manifest
//!     mutation are all explicitly rejected.
//!   * Every accepted mutation produces an `AuditedMutation` record
//!     suitable for control-plane envelope emission AND journal
//!     persistence (F.4 hooks straight in).
//!   * The supervisor enforces the policy in `apply_mutation`; the
//!     parser and the AI suggestion path (H.2) feed the same gate.
//!
//! Why so restrictive: the whole Phase B substrate (instance binding,
//! quota, fault attribution) hinges on stable `NodeId`s.  Allowing
//! Cypher to invent or destroy nodes would invalidate every claim and
//! restart_generation count downstream.

use gos_protocol::{ControlPlaneEnvelope, ControlPlaneMessageKind, EdgeId, NodeId, VectorAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationError {
    /// Mutation kind not in the receptive subset (e.g. node create,
    /// property write).  H.1 hard-refuses.
    UnsupportedMutation,
    /// Edge endpoints don't both exist in the runtime.
    UnknownEndpoint(NodeId),
    /// Mount target doesn't exist or isn't a mount-capable node.
    InvalidMountTarget(VectorAddress),
    /// Mutation passed validation but the runtime dispatcher refused
    /// (concurrent mutation, supervisor policy).  Carries the
    /// underlying reason as a tag.
    DispatcherRejected(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CypherMutation {
    /// `CREATE (a)-[:Mount]->(b)` against a clipboard / theme /
    /// dynamic mount node.  Both endpoints must already exist.
    AddEdge {
        from: NodeId,
        to: NodeId,
        edge_kind: ReceptiveEdgeKind,
    },
    /// `MATCH (a)-[r:Mount]->(b) DELETE r`.  Same constraints.
    RemoveEdge { edge_id: EdgeId },
    /// `MATCH (theme.current)-[r:Use]->() DELETE r,
    ///  CREATE (theme.current)-[:Use]->(target)` — atomic rebind of
    ///  the exclusive `Use` edge for theme switching.
    RebindUse {
        from: NodeId,
        new_target: NodeId,
    },
}

/// The narrow set of edge types Cypher mutations are allowed to
/// touch.  Spawn / Call / Return / Sync / Stream are runtime-internal
/// and never user-mutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReceptiveEdgeKind {
    Mount = 1,
    Use = 2,
}

/// Every accepted mutation produces one of these.  Caller writes it
/// into:
///   * the control-plane envelope queue (so shell can show it live)
///   * the journal ring (so reboot replay reconstructs the change)
#[derive(Debug, Clone, Copy)]
pub struct AuditedMutation {
    pub mutation: CypherMutation,
    /// Source attestation: `module_id`-shaped payload describing who
    /// requested the change.  Shell direct entry stamps `b"K_SHELL"`;
    /// AI suggestion (H.2) stamps `b"K_AI"`; future external admin
    /// tools stamp their own id.
    pub source: [u8; 16],
    pub tick: u64,
}

impl AuditedMutation {
    /// Encode the mutation as a control-plane envelope so it flows
    /// through the existing telemetry pipe and lands in the journal.
    pub fn to_envelope(&self) -> ControlPlaneEnvelope {
        let (arg0, arg1) = match self.mutation {
            CypherMutation::AddEdge { from, to, edge_kind } => (
                node_id_low(from) | ((edge_kind as u64) << 56),
                node_id_low(to),
            ),
            CypherMutation::RemoveEdge { edge_id } => (edge_id_low(edge_id), 0),
            CypherMutation::RebindUse { from, new_target } => {
                (node_id_low(from), node_id_low(new_target))
            }
        };
        ControlPlaneEnvelope {
            version: 1,
            kind: ControlPlaneMessageKind::EdgeUpsert,
            subject: self.source,
            arg0,
            arg1,
        }
    }
}

fn node_id_low(id: NodeId) -> u64 {
    let b = id.0;
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

fn edge_id_low(id: EdgeId) -> u64 {
    let b = id.0;
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// Validate a mutation in isolation (no runtime lookup).  Used by the
/// AI suggestion gate (H.2) before the supervisor sees it.
pub fn pre_validate(mutation: &CypherMutation) -> Result<(), MutationError> {
    match mutation {
        CypherMutation::AddEdge {
            edge_kind,
            ..
        } => match edge_kind {
            ReceptiveEdgeKind::Mount | ReceptiveEdgeKind::Use => Ok(()),
        },
        CypherMutation::RemoveEdge { .. } | CypherMutation::RebindUse { .. } => Ok(()),
    }
}

/// Adapter trait the supervisor implements; isolates this crate from
/// runtime-side specifics.  H.1 keeps the verbs minimal; future
/// slices (subgraph mutations, transactional batches) extend this.
pub trait MutationDispatcher {
    fn lookup_node(&self, id: NodeId) -> bool;
    fn add_edge(&mut self, from: NodeId, to: NodeId, kind: ReceptiveEdgeKind) -> Result<(), u32>;
    fn remove_edge(&mut self, id: EdgeId) -> Result<(), u32>;
    fn rebind_use(&mut self, from: NodeId, new_target: NodeId) -> Result<(), u32>;
}

pub fn apply_mutation<D: MutationDispatcher>(
    dispatcher: &mut D,
    mutation: CypherMutation,
) -> Result<(), MutationError> {
    pre_validate(&mutation)?;
    match mutation {
        CypherMutation::AddEdge { from, to, edge_kind } => {
            if !dispatcher.lookup_node(from) {
                return Err(MutationError::UnknownEndpoint(from));
            }
            if !dispatcher.lookup_node(to) {
                return Err(MutationError::UnknownEndpoint(to));
            }
            dispatcher
                .add_edge(from, to, edge_kind)
                .map_err(MutationError::DispatcherRejected)
        }
        CypherMutation::RemoveEdge { edge_id } => dispatcher
            .remove_edge(edge_id)
            .map_err(MutationError::DispatcherRejected),
        CypherMutation::RebindUse { from, new_target } => {
            if !dispatcher.lookup_node(from) {
                return Err(MutationError::UnknownEndpoint(from));
            }
            if !dispatcher.lookup_node(new_target) {
                return Err(MutationError::UnknownEndpoint(new_target));
            }
            dispatcher
                .rebind_use(from, new_target)
                .map_err(MutationError::DispatcherRejected)
        }
    }
}
