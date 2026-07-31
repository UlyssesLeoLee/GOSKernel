#![no_std]

//! Phase H.1 — receptive Cypher mutation API.
//!
//! `k-cypher` already has a read-only Cypher v1 subset (browse nodes,
//! browse edges, `CALL activate(n)`, `CALL spawn(n)`, `CALL route(e)`).
//! H.1 adds the *write* half — but on a leash:
//!
//!   * Edge mutations, `Mount`/`Use` rebinds, and (V2.5e, ADR-005 option A)
//!     provisional node creation are accepted. Node *delete*, NodeId
//!     reassignment, and plugin manifest mutation are still explicitly
//!     rejected.
//!   * Every accepted mutation produces an `AuditedMutation` record
//!     suitable for control-plane envelope emission AND journal
//!     persistence (F.4 hooks straight in).
//!   * The supervisor enforces the policy in `apply_mutation`; the
//!     parser and the AI suggestion path (H.2) feed the same gate.
//!
//! Why so restrictive: the whole Phase B substrate (instance binding,
//! quota, fault attribution) hinges on stable `NodeId`s. `CreateNode` is
//! safe here because it only ever produces *provisional* nodes
//! (`gos_runtime::create_provisional_node`, ADR-005 §六): `Unbound` /
//! `NodeInstanceId::ZERO`, ineligible for claim/quota until explicitly
//! promoted via a Grant edge (ADR-005 §五 step 3, not yet wired).
//! Destroying or renumbering *existing* nodes would still invalidate
//! every claim and restart_generation count downstream, so those
//! remain rejected.

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
    /// `CREATE (n:Label {props})` for a node pattern not bound by an
    /// earlier `MATCH` (V2.5e, ADR-005 option A). Allocates a fresh
    /// *provisional* node via `gos_runtime::create_provisional_node`:
    /// visible, connectable, and rendered from the next `graph_epoch`
    /// onward, but `Unbound` / `NodeInstanceId::ZERO` until promoted via
    /// a Grant edge. `Label`/`{props}` storage isn't wired yet — the
    /// dispatcher returns the allocated `NodeId` (see [`apply_mutation`])
    /// so a same-statement `CREATE (a)-[:Mount]->(n)` can reference it.
    CreateNode,
}

/// The narrow set of edge types Cypher mutations are allowed to
/// touch.  Spawn / Call / Return / Sync / Stream are runtime-internal
/// and never user-mutable.
///
/// `Depend` (= 3) is restricted to the boot manifest self-repair path.
/// It allows the rewrite engine to create missing dependency edges
/// discovered at boot time via `EdgeAbsent` rules.  General Cypher
/// shell input that tries to create a Depend edge is still rejected by
/// `pre_validate` in the shell gate — the boot path bypasses that gate
/// by calling `gos_runtime::apply_cypher_mutation` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReceptiveEdgeKind {
    Mount = 1,
    Use = 2,
    /// Boot-manifest self-repair only.  Maps to `RuntimeEdgeType::Depend`.
    Depend = 3,
    /// Phase H.1.x.3.link — declared correspondence between a runtime
    /// node and an interface-file node.  See `RuntimeEdgeType::Link`
    /// for semantics; the Cypher surface is the `LINK` verb.
    Link = 4,
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
            // The allocated NodeId doesn't exist until the dispatcher runs
            // (see `apply_mutation`), so there's nothing to pack here — this
            // envelope just audits *that* `source` requested a create.
            // `register_node` emits its own `NodeUpsert` with the real
            // id/vector when the dispatcher actually applies it.
            CypherMutation::CreateNode => (0, 0),
        };
        ControlPlaneEnvelope {
            version: 1,
            kind: ControlPlaneMessageKind::MutationAudit,
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
            ReceptiveEdgeKind::Mount
            | ReceptiveEdgeKind::Use
            | ReceptiveEdgeKind::Depend
            | ReceptiveEdgeKind::Link => Ok(()),
        },
        CypherMutation::RemoveEdge { .. }
        | CypherMutation::RebindUse { .. }
        | CypherMutation::CreateNode => Ok(()),
    }
}

/// Adapter trait the supervisor implements; isolates this crate from
/// runtime-side specifics.  H.1 keeps the verbs minimal; future
/// slices (subgraph mutations, transactional batches) extend this.
///
/// `add_edge` and `rebind_use` return the newly created `EdgeId` so
/// callers (and H.1.x.2 supervisor gate) can stamp it into the audit
/// envelope without round-tripping through the runtime again.
/// `remove_edge` echoes the input id for uniformity.
pub trait MutationDispatcher {
    fn lookup_node(&self, id: NodeId) -> bool;
    fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: ReceptiveEdgeKind,
    ) -> Result<EdgeId, u32>;
    fn remove_edge(&mut self, id: EdgeId) -> Result<EdgeId, u32>;
    fn rebind_use(&mut self, from: NodeId, new_target: NodeId) -> Result<EdgeId, u32>;
    /// Allocate a fresh provisional node (ADR-005 option A, V2.5e) and
    /// return its `NodeId`. Backed by `gos_runtime::create_provisional_node`
    /// in the real dispatcher.
    fn create_node(&mut self) -> Result<NodeId, u32>;
}

/// Apply a mutation through `dispatcher`. Returns `Ok(Some(node_id))` for
/// [`CypherMutation::CreateNode`] (the freshly allocated node's id — there's
/// nothing to return for the other variants, which mutate existing
/// edges/bindings rather than mint new identities; their affected `EdgeId`
/// is available from the dispatcher's own trait methods but not threaded
/// back out here — edge-scoped callers that need it call the dispatcher
/// directly instead of going through this generic entry point).
pub fn apply_mutation<D: MutationDispatcher>(
    dispatcher: &mut D,
    mutation: CypherMutation,
) -> Result<Option<NodeId>, MutationError> {
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
                .map(|_edge_id| None)
                .map_err(MutationError::DispatcherRejected)
        }
        CypherMutation::RemoveEdge { edge_id } => dispatcher
            .remove_edge(edge_id)
            .map(|_edge_id| None)
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
                .map(|_edge_id| None)
                .map_err(MutationError::DispatcherRejected)
        }
        CypherMutation::CreateNode => dispatcher
            .create_node()
            .map(Some)
            .map_err(MutationError::DispatcherRejected),
    }
}
