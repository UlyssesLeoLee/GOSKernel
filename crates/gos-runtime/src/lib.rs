#![no_std]

use core::mem::transmute;
use core::sync::atomic::{AtomicU64, Ordering};

use gos_protocol::{
    packet_to_signal, signal_to_packet, BootContext, CellDeclaration, CellResult,
    ConditionalRoute, ControlPlaneEnvelope, ControlPlaneMessageKind, EdgeId, EdgeSpec,
    EdgeVector, ExecStatus, ExecutorContext, GOS_ABI_VERSION, GraphDiffEntry, GraphDiffKind,
    GraphEdgeDirection, GraphEdgeSummary, GraphNodeSummary, GraphSnapshot, KernelAbi,
    KernelSignalPacket, MAX_CONDITIONAL_ROUTES, NodeCell, NodeEvent, NodeExecutorVTable,
    NodeId, NodeInstanceId, NodeLifecycle, NodeLogEntry, NodeProcSummary, NodeSpec, NodeState,
    NodeTelemetry, NodeTraceEntry,
    PluginId, PluginManifest, PluginState, PluginSummary, RoutePolicy, RuntimeEdgeType, Signal,
    StateDelta, VectorAddress,
    derive_edge_vector, CONTROL_PLANE_PROTOCOL_VERSION, DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED,
};
use spin::Mutex;

pub const MAX_PLUGINS: usize = 32;
pub const MAX_NODES: usize = 128;
pub const MAX_EDGES: usize = 512;
pub const MAX_SUBSCRIBE_PAIRS: usize = 64;
pub const MAX_READY_QUEUE: usize = 256;
pub const MAX_SIGNAL_QUEUE: usize = 512;
pub const MAX_FAULT_QUEUE: usize = 32;
pub const MAX_CALL_FRAMES: usize = 64;
pub const MAX_WAITSETS: usize = 64;
pub const MAX_BARRIERS: usize = 32;
pub const MAX_CONTROL_PLANE_MESSAGES: usize = 256;
pub const NODE_ARENA_PAGES: usize = 64;
/// Structural mutation ring capacity (wraps when full, oldest entries lost).
pub const MAX_DIFF_RING: usize = 128;
/// V2.15: per-node u8 property slots for reactive signal val encoding.
pub const MAX_NODE_PROPS_U8: usize = 16;
/// V2.24: per-node signal trace ring depth — most recent N dispatches.
pub const MAX_NODE_TRACE: usize = 16;
/// V2.25: per-node lifecycle event log depth — most recent N transitions.
pub const MAX_NODE_LOG: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    PluginTableFull,
    NodeTableFull,
    EdgeTableFull,
    ReadyQueueFull,
    SignalQueueFull,
    ControlPlaneQueueFull,
    NodeArenaFull,
    PluginNotFound,
    NodeNotFound,
    EdgeNotFound,
    LegacyCellMissing,
    NativeExecutorMissing,
    SubscribeTableFull,
    Fault(&'static str),
}

#[derive(Clone, Copy)]
struct RingQueue<T: Copy, const N: usize> {
    buffer: [Option<T>; N],
    head: usize,
    tail: usize,
}

impl<T: Copy, const N: usize> RingQueue<T, N> {
    const fn new() -> Self {
        Self {
            buffer: [None; N],
            head: 0,
            tail: 0,
        }
    }

    fn push(&mut self, value: T) -> Result<(), RuntimeError> {
        let next_head = (self.head + 1) % N;
        if next_head == self.tail {
            return Err(RuntimeError::ReadyQueueFull);
        }
        self.buffer[self.head] = Some(value);
        self.head = next_head;
        Ok(())
    }

    fn push_signal(&mut self, value: T) -> Result<(), RuntimeError> {
        let next_head = (self.head + 1) % N;
        if next_head == self.tail {
            return Err(RuntimeError::SignalQueueFull);
        }
        self.buffer[self.head] = Some(value);
        self.head = next_head;
        Ok(())
    }

    fn push_control_plane(&mut self, value: T) -> Result<(), RuntimeError> {
        let next_head = (self.head + 1) % N;
        if next_head == self.tail {
            return Err(RuntimeError::ControlPlaneQueueFull);
        }
        self.buffer[self.head] = Some(value);
        self.head = next_head;
        Ok(())
    }

    fn pop(&mut self) -> Option<T> {
        if self.head == self.tail {
            return None;
        }
        let value = self.buffer[self.tail].take();
        self.tail = (self.tail + 1) % N;
        value
    }

    fn len(&self) -> usize {
        if self.head >= self.tail {
            self.head - self.tail
        } else {
            N - self.tail + self.head
        }
    }

    fn is_empty(&self) -> bool {
        self.head == self.tail
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginLoadState {
    Discovered,
    Loaded,
    Faulted,
}

#[derive(Clone, Copy)]
struct PluginRecord {
    manifest: PluginManifest,
    state: PluginLoadState,
}

type LegacyCellPtr = [usize; 2];

#[derive(Clone, Copy)]
struct NativeExecutorBinding {
    vtable: NodeExecutorVTable,
    initialized: bool,
}

#[derive(Clone, Copy)]
enum NodeBinding {
    Unbound,
    Legacy(LegacyCellPtr),
    Native(NativeExecutorBinding),
}

#[derive(Clone, Copy)]
struct NodeRecord {
    plugin_id: PluginId,
    spec: NodeSpec,
    vector: VectorAddress,
    lifecycle: NodeLifecycle,
    runtime_page: usize,
    binding: NodeBinding,
    /// Active supervisor-issued instance for this node, if any.
    /// `NodeInstanceId::ZERO` means "no instance bound" — boot-time
    /// builtin nodes operate in this mode until the supervisor calls
    /// `bind_instance`.
    instance_id: NodeInstanceId,
    /// Conditional-route table (LangGraph-style edge fan-out).
    /// Populated via `register_node_routes` after the node is registered.
    routes: [ConditionalRoute; MAX_CONDITIONAL_ROUTES],
    route_count: u8,
    /// Cumulative signal dispatches to this node since registration.
    signal_count: u32,
}

#[derive(Clone, Copy)]
struct EdgeRecord {
    spec: EdgeSpec,
    edge_vector: EdgeVector,
}

#[derive(Clone, Copy)]
struct RuntimeSignal {
    target: VectorAddress,
    signal: Signal,
}

#[derive(Clone, Copy)]
enum WorkItem {
    Ready(NodeId),
    Signal(RuntimeSignal),
}

#[derive(Clone, Copy)]
struct CallFrame {
    caller: NodeId,
    callee: NodeId,
    _edge_id: EdgeId,
}

#[derive(Clone, Copy)]
struct WaitSet {
    _node: NodeId,
    _dependency: NodeId,
}

#[derive(Clone, Copy)]
struct Barrier {
    _node: NodeId,
    _dependency: NodeId,
}

#[derive(Clone, Copy)]
struct PreparedDispatch {
    slot: usize,
    node_id: NodeId,
    vector: VectorAddress,
    runtime_page: usize,
    binding: NodeBinding,
    instance_id: NodeInstanceId,
}

struct NodeArena {
    owners: [Option<NodeId>; NODE_ARENA_PAGES],
    pages: [[u8; 4096]; NODE_ARENA_PAGES],
}

impl NodeArena {
    const fn new() -> Self {
        Self {
            owners: [None; NODE_ARENA_PAGES],
            pages: [[0; 4096]; NODE_ARENA_PAGES],
        }
    }

    fn allocate(&mut self, owner: NodeId) -> Result<usize, RuntimeError> {
        for (idx, slot) in self.owners.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(owner);
                self.pages[idx][0] = self.pages[idx][0].wrapping_add(0);
                return Ok(idx);
            }
        }
        Err(RuntimeError::NodeArenaFull)
    }

    fn page_ptr(&mut self, page: usize) -> Result<*mut u8, RuntimeError> {
        let slot = self.pages.get_mut(page).ok_or(RuntimeError::NodeArenaFull)?;
        Ok(slot.as_mut_ptr())
    }
}

#[derive(Clone, Copy)]
struct AdjacencyArena {
    slots: [Option<EdgeId>; MAX_EDGES],
}

impl AdjacencyArena {
    const fn new() -> Self {
        Self {
            slots: [None; MAX_EDGES],
        }
    }

    fn allocate(&mut self, edge_id: EdgeId) -> Result<usize, RuntimeError> {
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(edge_id);
                return Ok(idx);
            }
        }
        Err(RuntimeError::EdgeTableFull)
    }

    fn release(&mut self, edge_id: EdgeId) {
        if let Some(slot) = self
            .slots
            .iter()
            .position(|slot| slot.map(|value| value == edge_id).unwrap_or(false))
        {
            self.slots[slot] = None;
        }
    }
}

/// One entry in the cached edge enumeration order: the edge's storage
/// slot plus its endpoints' node slots, resolved once when the order is
/// (re)built so per-edge summaries skip the O(nodes) id lookup that
/// `edge_summary_from_slot` performs.  `u16::MAX` marks an unresolved
/// endpoint.
#[derive(Clone, Copy)]
struct EdgeOrderEntry {
    slot: u16,
    from_slot: u16,
    to_slot: u16,
}

pub struct GraphRuntime {
    plugins: [Option<PluginRecord>; MAX_PLUGINS],
    nodes: [Option<NodeRecord>; MAX_NODES],
    edges: [Option<EdgeRecord>; MAX_EDGES],
    ready_queue: RingQueue<NodeId, MAX_READY_QUEUE>,
    signal_queue: RingQueue<RuntimeSignal, MAX_SIGNAL_QUEUE>,
    fault_queue: RingQueue<VectorAddress, MAX_FAULT_QUEUE>,
    call_frames: [Option<CallFrame>; MAX_CALL_FRAMES],
    wait_sets: [Option<WaitSet>; MAX_WAITSETS],
    barriers: [Option<Barrier>; MAX_BARRIERS],
    control_plane: RingQueue<ControlPlaneEnvelope, MAX_CONTROL_PLANE_MESSAGES>,
    node_arena: NodeArena,
    adjacency_arena: AdjacencyArena,
    /// Monotonic epoch bumped on every structural mutation (node or edge
    /// add/remove).  Enumeration caches an ordered snapshot keyed on this
    /// epoch, and host bridges can read it to skip re-rendering an
    /// unchanged graph — the change-tracking primitive for live streaming.
    graph_epoch: u64,
    /// V2.3 reactive Subscribe index: pairs of (observed_node, subscriber_node).
    /// When a structural mutation bumps graph_epoch and touches observed_node,
    /// the runtime emits a SubscribeTriggered control-plane envelope for each
    /// matching subscriber — the propagation primitive for Demo #1 and #2.
    subscribe_pairs: [Option<(NodeId, NodeId)>; MAX_SUBSCRIBE_PAIRS],
    /// V2.15 node property store: maps NodeId → u8 val embedded as the `val`
    /// field of `Signal::Control { cmd: DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED }`.
    /// Populated by plugins at boot to declare their reactive signal val so
    /// `fire_subscribers` can encode the active Use-edge target without
    /// hard-coding any theme knowledge inside the runtime.
    node_props_u8: [(NodeId, u8); MAX_NODE_PROPS_U8],
    /// Cached node enumeration order: storage-slot indices sorted by
    /// vector key, rebuilt lazily when `node_order_epoch != graph_epoch`.
    node_order: [u16; MAX_NODES],
    node_order_len: usize,
    node_order_epoch: u64,
    /// Cached edge enumeration order (with resolved endpoint slots),
    /// rebuilt lazily when `edge_order_epoch != graph_epoch`.
    edge_order: [EdgeOrderEntry; MAX_EDGES],
    edge_order_len: usize,
    edge_order_epoch: u64,
    tick: u64,
    /// Structural mutation changelog ring — one entry per node/edge add or remove.
    /// Wraps around when full; head points to the next write slot.
    diff_ring: [GraphDiffEntry; MAX_DIFF_RING],
    diff_ring_head: usize,
    /// Total structural mutations ever recorded (monotonic; used to detect wrap).
    diff_total: u64,
    /// V2.24 per-node signal trace rings, indexed by node slot.
    /// Each ring holds the most recent MAX_NODE_TRACE signal dispatches for that node.
    node_trace: [[NodeTraceEntry; MAX_NODE_TRACE]; MAX_NODES],
    /// Next write position in each node's trace ring.
    node_trace_head: [u8; MAX_NODES],
    /// V2.27: Total trace entries ever written per node slot (monotonic; saturates at u32::MAX).
    /// Separate from signal_count so node trace clear can reset it without affecting proc stats.
    node_trace_count: [u32; MAX_NODES],
    /// V2.25 per-node lifecycle event log rings, indexed by node slot.
    /// Each ring records the most recent MAX_NODE_LOG lifecycle transitions.
    node_log: [[NodeLogEntry; MAX_NODE_LOG]; MAX_NODES],
    /// Next write position in each node's log ring.
    node_log_head: [u8; MAX_NODES],
    /// Total lifecycle events ever logged per node slot (monotonic; saturates at u16::MAX).
    node_log_total: [u16; MAX_NODES],
}

impl Default for GraphRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphRuntime {
    pub const fn new() -> Self {
        Self {
            plugins: [None; MAX_PLUGINS],
            nodes: [None; MAX_NODES],
            edges: [None; MAX_EDGES],
            ready_queue: RingQueue::new(),
            signal_queue: RingQueue::new(),
            fault_queue: RingQueue::new(),
            call_frames: [None; MAX_CALL_FRAMES],
            wait_sets: [None; MAX_WAITSETS],
            barriers: [None; MAX_BARRIERS],
            control_plane: RingQueue::new(),
            node_arena: NodeArena::new(),
            adjacency_arena: AdjacencyArena::new(),
            graph_epoch: 0,
            subscribe_pairs: [None; MAX_SUBSCRIBE_PAIRS],
            node_props_u8: [(NodeId::ZERO, 0u8); MAX_NODE_PROPS_U8],
            node_order: [0u16; MAX_NODES],
            node_order_len: 0,
            node_order_epoch: u64::MAX,
            edge_order: [EdgeOrderEntry {
                slot: 0,
                from_slot: u16::MAX,
                to_slot: u16::MAX,
            }; MAX_EDGES],
            edge_order_len: 0,
            edge_order_epoch: u64::MAX,
            tick: 0,
            diff_ring: [GraphDiffEntry::EMPTY; MAX_DIFF_RING],
            diff_ring_head: 0,
            diff_total: 0,
            node_trace: [[NodeTraceEntry::EMPTY; MAX_NODE_TRACE]; MAX_NODES],
            node_trace_head: [0u8; MAX_NODES],
            node_trace_count: [0u32; MAX_NODES],
            node_log: [[NodeLogEntry::EMPTY; MAX_NODE_LOG]; MAX_NODES],
            node_log_head: [0u8; MAX_NODES],
            node_log_total: [0u16; MAX_NODES],
        }
    }

    pub fn emit_control_plane(
        &mut self,
        kind: ControlPlaneMessageKind,
        subject: [u8; 16],
        arg0: u64,
        arg1: u64,
    ) {
        let _ = self.control_plane.push_control_plane(ControlPlaneEnvelope {
            version: CONTROL_PLANE_PROTOCOL_VERSION,
            kind,
            subject,
            arg0,
            arg1,
        });
    }

    fn plugin_slot(&self, plugin_id: PluginId) -> Option<usize> {
        self.plugins.iter().position(|slot| {
            slot.map(|record| record.manifest.plugin_id == plugin_id)
                .unwrap_or(false)
        })
    }

    fn node_slot_by_id(&self, node_id: NodeId) -> Option<usize> {
        self.nodes.iter().position(|slot| {
            slot.map(|record| record.spec.node_id == node_id)
                .unwrap_or(false)
        })
    }

    fn node_slot_by_vec(&self, vector: VectorAddress) -> Option<usize> {
        self.nodes.iter().position(|slot| {
            slot.map(|record| record.vector == vector).unwrap_or(false)
        })
    }

    fn edge_slot(&self, edge_id: EdgeId) -> Option<usize> {
        self.edges.iter().position(|slot| {
            slot.map(|record| record.spec.edge_id == edge_id)
                .unwrap_or(false)
        })
    }

    fn edge_slot_by_vector(&self, edge_vector: EdgeVector) -> Option<usize> {
        self.edges.iter().position(|slot| {
            slot.map(|record| record.edge_vector == edge_vector)
                .unwrap_or(false)
        })
    }

    fn plugin_name(&self, plugin_id: PluginId) -> &'static str {
        self.plugin_slot(plugin_id)
            .and_then(|slot| self.plugins[slot].map(|record| record.manifest.name))
            .unwrap_or("UNKNOWN")
    }

    fn node_summary_from_slot(&self, slot: usize) -> Option<GraphNodeSummary> {
        let record = self.nodes.get(slot).and_then(|slot| *slot)?;
        Some(GraphNodeSummary {
            vector: record.vector,
            node_id: record.spec.node_id,
            plugin_id: record.plugin_id,
            plugin_name: self.plugin_name(record.plugin_id),
            local_node_key: record.spec.local_node_key,
            node_type: record.spec.node_type,
            lifecycle: record.lifecycle,
            entry_policy: record.spec.entry_policy,
            executor_id: record.spec.executor_id,
            export_count: record.spec.exports.len(),
        })
    }

    fn proc_summary_from_slot(&self, slot: usize) -> Option<NodeProcSummary> {
        let record = self.nodes.get(slot).and_then(|s| *s)?;
        let node_id = record.spec.node_id;
        let edge_out_count = self
            .edges
            .iter()
            .filter(|e| e.map(|r| r.spec.from_node == node_id).unwrap_or(false))
            .count() as u16;
        Some(NodeProcSummary {
            vector: record.vector,
            local_node_key: record.spec.local_node_key,
            plugin_name: self.plugin_name(record.plugin_id),
            lifecycle: record.lifecycle,
            signal_count: record.signal_count,
            edge_out_count,
        })
    }

    /// Return a page of `NodeProcSummary` entries sorted by vector address.
    /// Returns `(total_nodes, filled)`.
    pub fn proc_page<const N: usize>(
        &mut self,
        offset: usize,
        out: &mut [NodeProcSummary; N],
    ) -> (usize, usize) {
        self.refresh_node_order();
        let total = self.node_order_len;
        let mut returned = 0usize;
        let mut cursor = offset.min(total);
        while cursor < total && returned < N {
            let slot = self.node_order[cursor] as usize;
            if let Some(summary) = self.proc_summary_from_slot(slot) {
                out[returned] = summary;
                returned += 1;
            }
            cursor += 1;
        }
        (total, returned)
    }

    /// Total number of registered nodes (same as `node_page` total).
    pub fn proc_count(&self) -> usize {
        self.nodes.iter().filter(|s| s.is_some()).count()
    }

    /// Return a `NodeProcSummary` for the single node whose vector matches `vec`.
    /// Returns `None` if no registered node has that vector address.
    pub fn proc_stat_for_vector(&self, vec: VectorAddress) -> Option<NodeProcSummary> {
        let slot = self.nodes.iter().position(|s| {
            s.map(|r| r.vector == vec).unwrap_or(false)
        })?;
        self.proc_summary_from_slot(slot)
    }

    /// Count registered nodes whose vector address has the given `l4` domain byte.
    pub fn node_count_for_l4(&self, l4: u8) -> usize {
        self.nodes
            .iter()
            .filter(|s| s.map(|r| r.vector.l4 == l4).unwrap_or(false))
            .count()
    }

    /// Return a page of `GraphNodeSummary` for nodes in the given l4 domain,
    /// sorted by vector address.  Returns `(total_in_domain, filled)`.
    pub fn node_page_l4<const N: usize>(
        &mut self,
        l4: u8,
        offset: usize,
        out: &mut [GraphNodeSummary; N],
    ) -> (usize, usize) {
        self.refresh_node_order();
        let mut total = 0usize;
        let mut returned = 0usize;
        let mut cursor = 0usize;
        for i in 0..self.node_order_len {
            let slot = self.node_order[i] as usize;
            let matches = self
                .nodes
                .get(slot)
                .and_then(|s| *s)
                .map(|r| r.vector.l4 == l4)
                .unwrap_or(false);
            if matches {
                if cursor >= offset && returned < N {
                    if let Some(summary) = self.node_summary_from_slot(slot) {
                        out[returned] = summary;
                        returned += 1;
                    }
                }
                total += 1;
                cursor += 1;
            }
        }
        (total, returned)
    }

    /// Count nodes currently in `NodeLifecycle::Faulted` state.
    pub fn faulted_node_count(&self) -> usize {
        self.nodes
            .iter()
            .filter(|s| s.map(|r| r.lifecycle == NodeLifecycle::Faulted).unwrap_or(false))
            .count()
    }

    /// How many valid entries are currently in the structural diff ring.
    /// Equals `min(diff_total, MAX_DIFF_RING)`.
    pub fn diff_ring_fill(&self) -> usize {
        self.diff_total.min(MAX_DIFF_RING as u64) as usize
    }

    // ── Plugin inventory API (V2.20) ──────────────────────────────────────────

    fn plugin_summary_from_slot(&self, slot: usize) -> Option<PluginSummary> {
        let record = self.plugins[slot]?;
        let node_count = self
            .nodes
            .iter()
            .filter(|s| s.map(|r| r.plugin_id == record.manifest.plugin_id).unwrap_or(false))
            .count();
        Some(PluginSummary {
            plugin_id:  record.manifest.plugin_id,
            name:       record.manifest.name,
            version:    record.manifest.version,
            state: match record.state {
                PluginLoadState::Discovered => PluginState::Discovered,
                PluginLoadState::Loaded     => PluginState::Loaded,
                PluginLoadState::Faulted    => PluginState::Faulted,
            },
            node_count,
        })
    }

    /// Return a page of `PluginSummary` in discovery order.
    /// Returns `(total_plugins, filled)`.
    pub fn plugin_page<const N: usize>(
        &self,
        offset: usize,
        out: &mut [PluginSummary; N],
    ) -> (usize, usize) {
        let mut total = 0usize;
        let mut returned = 0usize;
        let mut cursor = 0usize;
        for slot in 0..MAX_PLUGINS {
            if self.plugins[slot].is_none() {
                continue;
            }
            if cursor >= offset && returned < N {
                if let Some(summary) = self.plugin_summary_from_slot(slot) {
                    out[returned] = summary;
                    returned += 1;
                }
            }
            total += 1;
            cursor += 1;
        }
        (total, returned)
    }

    /// Total number of registered (discovered) plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.iter().filter(|s| s.is_some()).count()
    }

    fn edge_summary_from_slot(
        &self,
        slot: usize,
        direction: GraphEdgeDirection,
    ) -> Option<GraphEdgeSummary> {
        let record = self.edges.get(slot).and_then(|slot| *slot)?;
        let from_slot = self.node_slot_by_id(record.spec.from_node)?;
        let to_slot = self.node_slot_by_id(record.spec.to_node)?;
        self.edge_summary_resolved(slot, from_slot, to_slot, direction)
    }

    /// Build an edge summary from a storage slot whose endpoints are
    /// already resolved to node slots.  The cached `edge_page` fast path
    /// uses this so each edge avoids the two O(nodes) id lookups that
    /// `edge_summary_from_slot` performs.  An out-of-range endpoint slot
    /// (e.g. `u16::MAX`, unresolved when the order was built) yields
    /// `None`, matching the skip behaviour of the lookup path.
    fn edge_summary_resolved(
        &self,
        slot: usize,
        from_slot: usize,
        to_slot: usize,
        direction: GraphEdgeDirection,
    ) -> Option<GraphEdgeSummary> {
        let record = self.edges.get(slot).and_then(|slot| *slot)?;
        let from = self.nodes.get(from_slot).and_then(|slot| *slot)?;
        let to = self.nodes.get(to_slot).and_then(|slot| *slot)?;
        Some(GraphEdgeSummary {
            edge_vector: record.edge_vector,
            edge_id: record.spec.edge_id,
            direction,
            from_vector: from.vector,
            from_key: from.spec.local_node_key,
            to_vector: to.vector,
            to_key: to.spec.local_node_key,
            edge_type: record.spec.edge_type,
            route_policy: record.spec.route_policy,
            capability_namespace: record.spec.capability_namespace,
            capability_binding: record.spec.capability_binding,
            weight: record.spec.weight,
            acl_mask: record.spec.acl_mask,
        })
    }

    fn state_delta(&mut self, node_id: NodeId, state: NodeLifecycle) {
        self.emit_control_plane(ControlPlaneMessageKind::StateDelta, node_id.0, state as u64, self.tick);
        // V2.25: append lifecycle transition to the per-node log ring.
        if let Some(slot) = self.node_slot_by_id(node_id) {
            let entry = NodeLogEntry { tick: self.tick, lifecycle: state as u8, _pad: [0u8; 7] };
            let head = self.node_log_head[slot] as usize;
            self.node_log[slot][head] = entry;
            self.node_log_head[slot] = ((head + 1) % MAX_NODE_LOG) as u8;
            self.node_log_total[slot] = self.node_log_total[slot].saturating_add(1);
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Current structural epoch (see the `graph_epoch` field).  Bumped on
    /// every node/edge add or remove; stable across pure reads.
    pub fn graph_epoch(&self) -> u64 {
        self.graph_epoch
    }

    /// Push one entry into the structural mutation ring (overwrites oldest when full).
    fn push_diff(&mut self, kind: GraphDiffKind, from_vec: VectorAddress, to_vec: VectorAddress, label: &[u8]) {
        let mut lbl = [0u8; 16];
        let n = label.len().min(16);
        lbl[..n].copy_from_slice(&label[..n]);
        self.diff_ring[self.diff_ring_head] = GraphDiffEntry {
            epoch: self.graph_epoch,
            kind,
            from_vector: from_vec,
            to_vector: to_vec,
            label: lbl,
        };
        self.diff_ring_head = (self.diff_ring_head + 1) % MAX_DIFF_RING;
        self.diff_total = self.diff_total.wrapping_add(1);
    }

    /// Return all diff entries recorded after `since_epoch`, filling `out`
    /// in chronological order.  Returns `(total_matching, filled)`.
    pub fn graph_diff_since<const N: usize>(
        &self,
        since_epoch: u64,
        out: &mut [GraphDiffEntry; N],
    ) -> (usize, usize) {
        // Walk the ring in insertion order starting from the oldest live slot.
        let count = self.diff_total.min(MAX_DIFF_RING as u64) as usize;
        let oldest = if self.diff_total <= MAX_DIFF_RING as u64 {
            0
        } else {
            self.diff_ring_head // head is the oldest when ring is full
        };
        let mut total = 0usize;
        let mut filled = 0usize;
        for i in 0..count {
            let slot = (oldest + i) % MAX_DIFF_RING;
            let entry = &self.diff_ring[slot];
            if entry.epoch > since_epoch {
                total += 1;
                if filled < N {
                    out[filled] = *entry;
                    filled += 1;
                }
            }
        }
        (total, filled)
    }

    /// Total structural diff entries ever pushed (monotonic; wraps at u64::MAX).
    pub fn diff_total(&self) -> u64 {
        self.diff_total
    }

    /// Register a reactive Subscribe pair: whenever a structural mutation
    /// touches `observed` (node/edge add or remove that bumps graph_epoch),
    /// the runtime emits `SubscribeTriggered` for `subscriber`.  Idempotent:
    /// registering the same pair twice is a no-op.
    pub fn register_subscribe_pair(
        &mut self,
        observed: NodeId,
        subscriber: NodeId,
    ) -> Result<(), RuntimeError> {
        for pair in self.subscribe_pairs.iter().flatten() {
            if pair.0 == observed && pair.1 == subscriber {
                return Ok(());
            }
        }
        let slot = self
            .subscribe_pairs
            .iter_mut()
            .find(|s| s.is_none())
            .ok_or(RuntimeError::SubscribeTableFull)?;
        *slot = Some((observed, subscriber));
        Ok(())
    }

    /// Number of active (non-None) subscribe pairs.  Callers can compare
    /// this against `MAX_SUBSCRIBE_PAIRS` to check table headroom before
    /// registering many pairs in a tight loop.
    pub fn subscribe_pair_count(&self) -> usize {
        self.subscribe_pairs.iter().filter(|s| s.is_some()).count()
    }

    /// Remove an (observed, subscriber) pair from the subscribe table.
    /// No-ops silently when the pair is not present.
    pub fn unregister_subscribe_pair(&mut self, observed: NodeId, subscriber: NodeId) {
        for slot in self.subscribe_pairs.iter_mut() {
            if *slot == Some((observed, subscriber)) {
                *slot = None;
                return;
            }
        }
    }

    /// V2.15: Register a u8 property value for a node used as the reactive
    /// signal val when that node is the active Use-edge target of an observed
    /// node. Idempotent: re-registering the same NodeId overwrites the val.
    /// Returns false when the table is full (MAX_NODE_PROPS_U8 slots).
    pub fn register_node_prop_u8(&mut self, node_id: NodeId, val: u8) -> bool {
        for slot in self.node_props_u8.iter_mut() {
            if slot.0 == node_id {
                slot.1 = val;
                return true;
            }
        }
        for slot in self.node_props_u8.iter_mut() {
            if slot.0 == NodeId::ZERO {
                *slot = (node_id, val);
                return true;
            }
        }
        false
    }

    fn node_prop_u8(&self, node_id: NodeId) -> Option<u8> {
        self.node_props_u8.iter().find_map(|&(id, val)| {
            if id == node_id && id != NodeId::ZERO { Some(val) } else { None }
        })
    }

    /// V2.15: Return the NodeId pointed to by the first Use edge originating
    /// from `source`. Used by `fire_subscribers` to encode which variant of an
    /// observed node is currently active as the reactive signal val.
    pub fn active_use_target(&self, source: NodeId) -> Option<NodeId> {
        self.edges.iter().flatten().find_map(|rec| {
            if rec.spec.from_node == source && rec.spec.edge_type == RuntimeEdgeType::Use {
                Some(rec.spec.to_node)
            } else {
                None
            }
        })
    }

    /// Emit `SubscribeTriggered` for every subscriber of `changed`.
    /// Called internally after any structural mutation that bumps graph_epoch.
    /// V2.15: also posts `Signal::Control { cmd: DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED }`
    /// directly to each subscriber's runtime queue so nodes can react synchronously
    /// without polling the control-plane. The val encodes the active Use-edge
    /// target's registered node property (e.g. the theme index for theme.current).
    fn fire_subscribers(&mut self, changed: NodeId, epoch: u64) {
        let mut subs = [NodeId::ZERO; MAX_SUBSCRIBE_PAIRS];
        let mut count = 0usize;
        for pair in self.subscribe_pairs.iter().flatten() {
            if pair.0 == changed && count < MAX_SUBSCRIBE_PAIRS {
                subs[count] = pair.1;
                count += 1;
            }
        }
        let signal_val: u8 = self
            .active_use_target(changed)
            .and_then(|tid| self.node_prop_u8(tid))
            .unwrap_or(0);
        for sub in subs[..count].iter().copied() {
            let arg0 = u64::from_le_bytes([
                sub.0[0], sub.0[1], sub.0[2], sub.0[3],
                sub.0[4], sub.0[5], sub.0[6], sub.0[7],
            ]);
            self.emit_control_plane(
                ControlPlaneMessageKind::SubscribeTriggered,
                changed.0,
                arg0,
                epoch,
            );
            if let Ok(sub_vec) = self.node_vector(sub) {
                let _ = self.post_signal(sub_vec, Signal::Control {
                    cmd: DISPLAY_CONTROL_SUBSCRIBE_TRIGGERED,
                    val: signal_val,
                });
            }
        }
    }

    /// Check whether a node identified by `NodeId` is present in the graph.
    /// Used by `RuntimeGraphView` in the supervisor so the rewrite engine
    /// can evaluate `NodePresent` and `EdgePresent` patterns without holding
    /// the runtime lock across the full apply-rules sweep.
    pub fn node_exists_by_id(&self, id: NodeId) -> bool {
        self.node_slot_by_id(id).is_some()
    }

    /// Check whether an edge of the given receptive kind exists between
    /// `from` and `to`.  Used by `RuntimeGraphView` for `EdgePresent` /
    /// `EdgeAbsent` pattern evaluation.
    pub fn edge_exists_by_kind(
        &self,
        from: NodeId,
        to: NodeId,
        kind: gos_cypher_mut::ReceptiveEdgeKind,
    ) -> bool {
        let edge_type = match kind {
            gos_cypher_mut::ReceptiveEdgeKind::Mount => RuntimeEdgeType::Mount,
            gos_cypher_mut::ReceptiveEdgeKind::Use => RuntimeEdgeType::Use,
            gos_cypher_mut::ReceptiveEdgeKind::Depend => RuntimeEdgeType::Depend,
        };
        self.edges.iter().flatten().any(|rec| {
            rec.spec.from_node == from
                && rec.spec.to_node == to
                && rec.spec.edge_type == edge_type
        })
    }

    pub fn discover_plugin(&mut self, manifest: PluginManifest) -> Result<(), RuntimeError> {
        if self.plugin_slot(manifest.plugin_id).is_some() {
            return Ok(());
        }

        let slot = self.plugins.iter_mut().find(|slot| slot.is_none());
        match slot {
            Some(slot) => {
                *slot = Some(PluginRecord {
                    manifest,
                    state: PluginLoadState::Discovered,
                });
                self.emit_control_plane(ControlPlaneMessageKind::PluginDiscovered, manifest.plugin_id.0, manifest.version as u64, 0);
                Ok(())
            }
            None => Err(RuntimeError::PluginTableFull),
        }
    }

    pub fn mark_plugin_loaded(&mut self, plugin_id: PluginId) -> Result<(), RuntimeError> {
        let slot = self.plugin_slot(plugin_id).ok_or(RuntimeError::PluginNotFound)?;
        let mut record = self.plugins[slot].ok_or(RuntimeError::PluginNotFound)?;
        record.state = PluginLoadState::Loaded;
        self.plugins[slot] = Some(record);
        Ok(())
    }

    pub fn mark_plugin_fault(&mut self, plugin_id: PluginId) {
        if let Some(slot) = self.plugin_slot(plugin_id) {
            if let Some(mut record) = self.plugins[slot] {
                record.state = PluginLoadState::Faulted;
                self.plugins[slot] = Some(record);
            }
        }
    }

    pub fn register_node(
        &mut self,
        plugin_id: PluginId,
        vector: VectorAddress,
        spec: NodeSpec,
    ) -> Result<NodeId, RuntimeError> {
        if self.node_slot_by_id(spec.node_id).is_some() {
            return Ok(spec.node_id);
        }

        let runtime_page = self.node_arena.allocate(spec.node_id)?;
        let slot = self.nodes.iter_mut().find(|slot| slot.is_none()).ok_or(RuntimeError::NodeTableFull)?;

        *slot = Some(NodeRecord {
            plugin_id,
            spec,
            vector,
            lifecycle: NodeLifecycle::Allocated,
            runtime_page,
            binding: NodeBinding::Unbound,
            instance_id: NodeInstanceId::ZERO,
            routes: [ConditionalRoute { key: 0xFF, target: VectorAddress::new(0, 0, 0, 0) }; MAX_CONDITIONAL_ROUTES],
            route_count: 0,
            signal_count: 0,
        });

        self.emit_control_plane(ControlPlaneMessageKind::NodeUpsert, spec.node_id.0, vector.as_u64(), runtime_page as u64);
        self.state_delta(spec.node_id, NodeLifecycle::Registered);
        self.state_delta(spec.node_id, NodeLifecycle::Allocated);
        self.graph_epoch = self.graph_epoch.wrapping_add(1);
        self.push_diff(GraphDiffKind::NodeAdded, vector, VectorAddress::new(0,0,0,0), spec.local_node_key.as_bytes());
        self.fire_subscribers(spec.node_id, self.graph_epoch);
        Ok(spec.node_id)
    }

    pub fn register_edge(&mut self, spec: EdgeSpec) -> Result<EdgeId, RuntimeError> {
        if self.edge_slot(spec.edge_id).is_some() {
            return Ok(spec.edge_id);
        }

        self.adjacency_arena.allocate(spec.edge_id)?;
        let slot = self.edges.iter_mut().find(|slot| slot.is_none()).ok_or(RuntimeError::EdgeTableFull)?;
        *slot = Some(EdgeRecord {
            edge_vector: derive_edge_vector(spec.edge_id),
            spec,
        });
        self.emit_control_plane(ControlPlaneMessageKind::EdgeUpsert, spec.edge_id.0, spec.from_node.0[0] as u64, spec.to_node.0[0] as u64);
        self.graph_epoch = self.graph_epoch.wrapping_add(1);
        let from_vec = self.node_slot_by_id(spec.from_node)
            .and_then(|s| self.nodes[s].map(|r| r.vector))
            .unwrap_or(VectorAddress::new(0, 0, 0, 0));
        let to_vec = self.node_slot_by_id(spec.to_node)
            .and_then(|s| self.nodes[s].map(|r| r.vector))
            .unwrap_or(VectorAddress::new(0, 0, 0, 0));
        let edge_label = spec.capability_binding.unwrap_or(spec.capability_namespace.unwrap_or("edge")).as_bytes();
        self.push_diff(GraphDiffKind::EdgeAdded, from_vec, to_vec, edge_label);
        self.fire_subscribers(spec.from_node, self.graph_epoch);
        self.fire_subscribers(spec.to_node, self.graph_epoch);
        Ok(spec.edge_id)
    }

    /// Register a conditional-route table for a node (LangGraph-style edge fan-out).
    ///
    /// When the node's `on_event` returns `ExecStatus::Route`, the runtime
    /// reads `ctx.route_key` and posts the current signal to the matching
    /// `ConditionalRoute::target`.  Calling this more than once overwrites
    /// the previous table.  Routes beyond `MAX_CONDITIONAL_ROUTES` are silently
    /// truncated.
    pub fn register_node_routes(
        &mut self,
        vector: VectorAddress,
        routes: &[ConditionalRoute],
    ) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let record = self.nodes[slot].as_mut().ok_or(RuntimeError::NodeNotFound)?;
        let count = routes.len().min(MAX_CONDITIONAL_ROUTES);
        record.route_count = count as u8;
        for (i, r) in routes.iter().take(count).enumerate() {
            record.routes[i] = *r;
        }
        Ok(())
    }

    pub fn unregister_edge(&mut self, edge_id: EdgeId) -> Result<(), RuntimeError> {
        let slot = self.edge_slot(edge_id).ok_or(RuntimeError::EdgeNotFound)?;
        let record = self.edges[slot].ok_or(RuntimeError::EdgeNotFound)?;
        let from_node = record.spec.from_node;
        let to_node = record.spec.to_node;
        let from_vec = self.node_slot_by_id(from_node)
            .and_then(|s| self.nodes[s].map(|r| r.vector))
            .unwrap_or(VectorAddress::new(0, 0, 0, 0));
        let to_vec = self.node_slot_by_id(to_node)
            .and_then(|s| self.nodes[s].map(|r| r.vector))
            .unwrap_or(VectorAddress::new(0, 0, 0, 0));
        let edge_label = record.spec.capability_binding
            .unwrap_or(record.spec.capability_namespace.unwrap_or("edge"));
        self.edges[slot] = None;
        self.adjacency_arena.release(edge_id);
        self.graph_epoch = self.graph_epoch.wrapping_add(1);
        self.push_diff(GraphDiffKind::EdgeRemoved, from_vec, to_vec, edge_label.as_bytes());
        self.fire_subscribers(from_node, self.graph_epoch);
        self.fire_subscribers(to_node, self.graph_epoch);
        Ok(())
    }

    pub fn bind_legacy_cell(
        &mut self,
        vector: VectorAddress,
        cell_ptr: LegacyCellPtr,
    ) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let mut record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        record.binding = NodeBinding::Legacy(cell_ptr);
        self.nodes[slot] = Some(record);
        Ok(())
    }

    pub fn bind_native_executor(
        &mut self,
        vector: VectorAddress,
        vtable: NodeExecutorVTable,
    ) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let mut record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        record.binding = NodeBinding::Native(NativeExecutorBinding {
            vtable,
            initialized: false,
        });
        self.nodes[slot] = Some(record);
        Ok(())
    }

    pub fn describe_legacy_node(&self, vector: VectorAddress) -> Result<CellDeclaration, RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        let NodeBinding::Legacy(ptr) = record.binding else {
            return Err(RuntimeError::LegacyCellMissing);
        };
        let mutex = unsafe { legacy_cell_mutex(ptr) };
        let guard = mutex.lock();
        Ok(guard.declare())
    }

    pub fn node_id_for_vec(&self, vector: VectorAddress) -> Option<NodeId> {
        self.node_slot_by_vec(vector)
            .and_then(|slot| self.nodes[slot].map(|record| record.spec.node_id))
    }

    pub fn edge_vector_for_id(&self, edge_id: EdgeId) -> Option<EdgeVector> {
        self.edge_slot(edge_id)
            .and_then(|slot| self.edges[slot].map(|record| record.edge_vector))
    }

    pub fn edge_id_for_vector(&self, edge_vector: EdgeVector) -> Option<EdgeId> {
        self.edge_slot_by_vector(edge_vector)
            .and_then(|slot| self.edges[slot].map(|record| record.spec.edge_id))
    }

    pub fn node_summary(&self, vector: VectorAddress) -> Option<GraphNodeSummary> {
        let slot = self.node_slot_by_vec(vector)?;
        self.node_summary_from_slot(slot)
    }

    /// Query a node's telemetry via its executor vtable callback.
    pub fn node_telemetry(&self, vector: VectorAddress) -> Option<NodeTelemetry> {
        let slot = self.node_slot_by_vec(vector)?;
        let record = self.nodes[slot]?;
        if let NodeBinding::Native(binding) = record.binding {
            if let Some(telemetry_fn) = binding.vtable.on_telemetry {
                return Some(unsafe { telemetry_fn() });
            }
        }
        None
    }

    pub fn edge_summary(&self, edge_vector: EdgeVector) -> Option<GraphEdgeSummary> {
        let slot = self.edge_slot_by_vector(edge_vector)?;
        self.edge_summary_from_slot(slot, GraphEdgeDirection::Outbound)
    }

    /// Rebuild the cached node enumeration order if the graph mutated
    /// since it was last built.  Sorting once per snapshot — rather than
    /// on every page request — turns a full paginated walk from
    /// O((V/N)·V²) into one O(V²) build plus O(V) of slicing: every page
    /// after the first is a plain array read.
    fn refresh_node_order(&mut self) {
        if self.node_order_epoch == self.graph_epoch {
            return;
        }
        let mut slots = [0u16; MAX_NODES];
        let mut keys = [0u64; MAX_NODES];
        let mut total = 0usize;
        for (idx, slot) in self.nodes.iter().enumerate() {
            if let Some(record) = slot {
                slots[total] = idx as u16;
                keys[total] = record.vector.as_u64();
                total += 1;
            }
        }
        // Insertion sort on cached keys (loop-invariant key reads).
        let mut i = 1usize;
        while i < total {
            let cur_slot = slots[i];
            let cur_key = keys[i];
            let mut j = i;
            while j > 0 && keys[j - 1] > cur_key {
                slots[j] = slots[j - 1];
                keys[j] = keys[j - 1];
                j -= 1;
            }
            slots[j] = cur_slot;
            keys[j] = cur_key;
            i += 1;
        }
        self.node_order[..total].copy_from_slice(&slots[..total]);
        self.node_order_len = total;
        self.node_order_epoch = self.graph_epoch;
    }

    /// Rebuild the cached edge enumeration order if the graph mutated
    /// since it was last built.  Besides sorting once per snapshot, each
    /// entry caches its endpoints' node slots so `edge_page` skips the two
    /// O(nodes) id lookups `edge_summary_from_slot` would do per edge —
    /// collapsing a full paged edge walk from O((E/N)·E·V) to one
    /// O(E·V) build plus O(E) slicing.
    fn refresh_edge_order(&mut self) {
        if self.edge_order_epoch == self.graph_epoch {
            return;
        }
        let mut entries = [EdgeOrderEntry {
            slot: 0,
            from_slot: u16::MAX,
            to_slot: u16::MAX,
        }; MAX_EDGES];
        let mut keys = [0u64; MAX_EDGES];
        let mut total = 0usize;
        for (idx, edge) in self.edges.iter().enumerate() {
            if let Some(record) = edge {
                let from_slot = self
                    .node_slot_by_id(record.spec.from_node)
                    .map(|s| s as u16)
                    .unwrap_or(u16::MAX);
                let to_slot = self
                    .node_slot_by_id(record.spec.to_node)
                    .map(|s| s as u16)
                    .unwrap_or(u16::MAX);
                entries[total] = EdgeOrderEntry {
                    slot: idx as u16,
                    from_slot,
                    to_slot,
                };
                keys[total] = record.edge_vector.as_u64();
                total += 1;
            }
        }
        // Insertion sort on cached keys (loop-invariant key reads).
        let mut i = 1usize;
        while i < total {
            let cur = entries[i];
            let cur_key = keys[i];
            let mut j = i;
            while j > 0 && keys[j - 1] > cur_key {
                entries[j] = entries[j - 1];
                keys[j] = keys[j - 1];
                j -= 1;
            }
            entries[j] = cur;
            keys[j] = cur_key;
            i += 1;
        }
        self.edge_order[..total].copy_from_slice(&entries[..total]);
        self.edge_order_len = total;
        self.edge_order_epoch = self.graph_epoch;
    }

    pub fn node_page<const N: usize>(
        &mut self,
        offset: usize,
        out: &mut [GraphNodeSummary; N],
    ) -> (usize, usize) {
        self.refresh_node_order();
        let total = self.node_order_len;
        let mut returned = 0usize;
        let mut cursor = offset.min(total);
        while cursor < total && returned < N {
            let slot = self.node_order[cursor] as usize;
            if let Some(summary) = self.node_summary_from_slot(slot) {
                out[returned] = summary;
                returned += 1;
            }
            cursor += 1;
        }
        (total, returned)
    }

    pub fn edge_page_for_node<const N: usize>(
        &self,
        node_vec: VectorAddress,
        offset: usize,
        out: &mut [GraphEdgeSummary; N],
    ) -> Result<(usize, usize), RuntimeError> {
        let node_id = self.node_id_for_vec(node_vec).ok_or(RuntimeError::NodeNotFound)?;
        let mut slots = [(usize::MAX, GraphEdgeDirection::Outbound); MAX_EDGES];
        let mut keys = [0u64; MAX_EDGES];
        let mut total = 0usize;
        for (idx, edge) in self.edges.iter().enumerate() {
            let Some(edge) = edge else {
                continue;
            };
            if edge.spec.from_node == node_id {
                slots[total] = (idx, GraphEdgeDirection::Outbound);
                keys[total] = edge.edge_vector.as_u64();
                total += 1;
            } else if edge.spec.to_node == node_id {
                slots[total] = (idx, GraphEdgeDirection::Inbound);
                keys[total] = edge.edge_vector.as_u64();
                total += 1;
            }
        }

        // Insertion sort on cached keys (see node_page): avoids the
        // O(n^2) recomputation of edge_vector.as_u64() inside the
        // comparison.
        let mut i = 1usize;
        while i < total {
            let cur_slot = slots[i];
            let cur_key = keys[i];
            let mut j = i;
            while j > 0 && keys[j - 1] > cur_key {
                slots[j] = slots[j - 1];
                keys[j] = keys[j - 1];
                j -= 1;
            }
            slots[j] = cur_slot;
            keys[j] = cur_key;
            i += 1;
        }

        let mut returned = 0usize;
        let mut cursor = offset.min(total);
        while cursor < total && returned < N {
            let (slot, direction) = slots[cursor];
            if let Some(summary) = self.edge_summary_from_slot(slot, direction) {
                out[returned] = summary;
                returned += 1;
            }
            cursor += 1;
        }
        Ok((total, returned))
    }

    pub fn edge_page<const N: usize>(
        &mut self,
        offset: usize,
        out: &mut [GraphEdgeSummary; N],
    ) -> (usize, usize) {
        self.refresh_edge_order();
        let total = self.edge_order_len;
        let mut returned = 0usize;
        let mut cursor = offset.min(total);
        while cursor < total && returned < N {
            let entry = self.edge_order[cursor];
            if let Some(summary) = self.edge_summary_resolved(
                entry.slot as usize,
                entry.from_slot as usize,
                entry.to_slot as usize,
                GraphEdgeDirection::Outbound,
            ) {
                out[returned] = summary;
                returned += 1;
            }
            cursor += 1;
        }
        (total, returned)
    }

    pub fn resolve_capability(
        &self,
        namespace: &[u8],
        capability: &[u8],
    ) -> Option<VectorAddress> {
        self.nodes.iter().flatten().find_map(|record| {
            let exported = record.spec.exports.iter().any(|export| {
                export.namespace.as_bytes() == namespace && export.name.as_bytes() == capability
            });
            exported.then_some(record.vector)
        })
    }

    pub fn enqueue_ready(&mut self, node_id: NodeId) -> Result<(), RuntimeError> {
        self.ready_queue.push(node_id)
    }

    pub fn post_signal(&mut self, target: VectorAddress, signal: Signal) -> Result<(), RuntimeError> {
        self.signal_queue.push_signal(RuntimeSignal { target, signal })
    }

    fn prepare_activation(&mut self, vector: VectorAddress) -> Result<PreparedDispatch, RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let mut record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        record.lifecycle = NodeLifecycle::Running;
        self.nodes[slot] = Some(record);
        self.state_delta(record.spec.node_id, NodeLifecycle::Running);
        Ok(PreparedDispatch {
            slot,
            node_id: record.spec.node_id,
            vector: record.vector,
            runtime_page: record.runtime_page,
            binding: record.binding,
            instance_id: record.instance_id,
        })
    }

    fn prepare_signal_dispatch(
        &mut self,
        vector: VectorAddress,
        signal: Signal,
    ) -> Result<PreparedDispatch, RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let mut record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        record.lifecycle = NodeLifecycle::Running;
        // Record trace entry before incrementing so serial == index of this dispatch.
        let (kind, from, cmd) = signal_trace_fields(signal);
        let trace = NodeTraceEntry { from, serial: record.signal_count, kind, cmd };
        let head = self.node_trace_head[slot] as usize;
        self.node_trace[slot][head] = trace;
        self.node_trace_head[slot] = ((head + 1) % MAX_NODE_TRACE) as u8;
        self.node_trace_count[slot] = self.node_trace_count[slot].saturating_add(1);
        record.signal_count = record.signal_count.saturating_add(1);
        self.nodes[slot] = Some(record);
        self.state_delta(record.spec.node_id, NodeLifecycle::Running);
        Ok(PreparedDispatch {
            slot,
            node_id: record.spec.node_id,
            vector: record.vector,
            runtime_page: record.runtime_page,
            binding: record.binding,
            instance_id: record.instance_id,
        })
    }

    /// Return the signal trace ring for `vector` — most recent dispatch first.
    /// Returns `(total_traced, entries_written)`.  `total_traced` resets to zero after
    /// `clear_node_trace_inner()` and is independent of the cumulative `signal_count` used by proc.
    pub fn node_trace_page(
        &self,
        vector: VectorAddress,
        out: &mut [NodeTraceEntry; MAX_NODE_TRACE],
    ) -> Result<(u32, usize), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let _record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        let total = self.node_trace_count[slot];
        let ring_fill = (total as usize).min(MAX_NODE_TRACE);
        let head = self.node_trace_head[slot] as usize;
        let mut returned = 0usize;
        let mut idx = 0usize;
        while idx < ring_fill {
            let ring_pos = (head + MAX_NODE_TRACE - 1 - idx) % MAX_NODE_TRACE;
            out[returned] = self.node_trace[slot][ring_pos];
            returned += 1;
            idx += 1;
        }
        Ok((total, returned))
    }

    /// Return the lifecycle event log for `vector` — most recent transition first.
    /// Returns `(total_events, entries_written)`.
    pub fn node_log_page(
        &self,
        vector: VectorAddress,
        out: &mut [NodeLogEntry; MAX_NODE_LOG],
    ) -> Result<(usize, usize), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let total = self.node_log_total[slot] as usize;
        let ring_fill = total.min(MAX_NODE_LOG);
        let head = self.node_log_head[slot] as usize;
        let mut returned = 0usize;
        for idx in 0..ring_fill {
            let ring_pos = (head + MAX_NODE_LOG - 1 - idx) % MAX_NODE_LOG;
            out[returned] = self.node_log[slot][ring_pos];
            returned += 1;
        }
        Ok((total, returned))
    }

    /// V2.26: Clear the per-node lifecycle log ring for `vector`.
    /// Resets the ring, head pointer, and total counter to zero.
    /// Returns `Err(RuntimeError::NodeNotFound)` if the node is not registered.
    pub fn clear_node_log_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        self.node_log[slot] = [NodeLogEntry::EMPTY; MAX_NODE_LOG];
        self.node_log_head[slot] = 0;
        self.node_log_total[slot] = 0;
        Ok(())
    }

    /// V2.27: Clear the per-node signal trace ring for `vector`.
    /// Resets the ring, head pointer, and trace counter to zero.
    /// `signal_count` (used by proc) is preserved — only the buffered trace history is discarded.
    /// Returns `Err(RuntimeError::NodeNotFound)` if the node is not registered.
    pub fn clear_node_trace_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        self.node_trace[slot] = [NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
        self.node_trace_head[slot] = 0;
        self.node_trace_count[slot] = 0;
        Ok(())
    }

    /// V2.28: Reset the cumulative signal_count for `vector` to zero.
    /// Analogous to `perf stat reset` — zeroes the counter shown by `proc` and `stat`
    /// without touching the trace ring or lifecycle log.
    /// Returns `Err(RuntimeError::NodeNotFound)` if the node is not registered.
    pub fn reset_node_stat_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let record = self.nodes[slot].as_mut().ok_or(RuntimeError::NodeNotFound)?;
        record.signal_count = 0;
        Ok(())
    }

    /// V2.31: BFS shortest-path search from `from` to `to` across registered edges.
    ///
    /// Returns `(path, length)` where `path[0..length]` is the ordered sequence of
    /// VectorAddresses from `from` to `to` (inclusive at both ends).  `length == 0`
    /// means no path exists (nodes unreachable or not found).  `length == 1` means
    /// `from == to` (trivial self-path).
    ///
    /// Uses iterative BFS over the flat edge table.  Fixed-size stack arrays only —
    /// no heap allocation, safe in `no_std` interrupt context.
    pub fn find_graph_path_inner<const N: usize>(
        &self,
        from: VectorAddress,
        to: VectorAddress,
    ) -> ([VectorAddress; N], usize) {
        const SENTINEL: usize = MAX_NODES; // "no predecessor"
        let mut out = [VectorAddress::new(0, 0, 0, 0); N];

        let from_slot = match self.node_slot_by_vec(from) {
            Some(s) => s,
            None => return (out, 0),
        };
        let to_slot = match self.node_slot_by_vec(to) {
            Some(s) => s,
            None => return (out, 0),
        };

        // Trivial self-path.
        if from_slot == to_slot {
            out[0] = from;
            return (out, 1);
        }

        let mut visited  = [false;    MAX_NODES];
        let mut prev     = [SENTINEL; MAX_NODES];
        // Ring queue of node slots.
        let mut q        = [0usize;   MAX_NODES];
        let mut q_head   = 0usize;
        let mut q_tail   = 0usize;

        visited[from_slot] = true;
        q[q_tail] = from_slot;
        q_tail = (q_tail + 1) % MAX_NODES;

        let mut found = false;

        'bfs: while q_head != q_tail {
            let cur_slot = q[q_head];
            q_head = (q_head + 1) % MAX_NODES;

            let cur_node_id = match self.nodes[cur_slot] {
                Some(r) => r.spec.node_id,
                None => continue,
            };

            // Enumerate all edges whose from_node == cur_node_id.
            for edge_opt in &self.edges {
                let edge = match edge_opt {
                    Some(e) => e,
                    None => continue,
                };
                if edge.spec.from_node != cur_node_id {
                    continue;
                }
                let next_slot = match self.node_slot_by_id(edge.spec.to_node) {
                    Some(s) => s,
                    None => continue,
                };
                if visited[next_slot] {
                    continue;
                }
                visited[next_slot] = true;
                prev[next_slot] = cur_slot;
                if next_slot == to_slot {
                    found = true;
                    break 'bfs;
                }
                let next_tail = (q_tail + 1) % MAX_NODES;
                // If queue is full, stop exploring this branch (shouldn't happen with MAX_NODES).
                if next_tail != q_head {
                    q[q_tail] = next_slot;
                    q_tail = next_tail;
                }
            }
        }

        if !found {
            return (out, 0);
        }

        // Reconstruct path from to_slot back to from_slot via prev[].
        let mut path_slots = [0usize; MAX_NODES];
        let mut path_len   = 0usize;
        let mut cur = to_slot;
        while cur != SENTINEL && path_len < MAX_NODES {
            path_slots[path_len] = cur;
            path_len += 1;
            if cur == from_slot {
                break;
            }
            cur = prev[cur];
        }

        // path_slots[0..path_len] is currently [to .. from]; reverse it.
        let mut lo = 0usize;
        let mut hi = path_len.saturating_sub(1);
        while lo < hi {
            path_slots.swap(lo, hi);
            lo += 1;
            hi -= 1;
        }

        // Copy into output, capped at N.
        let copy_len = path_len.min(N);
        for i in 0..copy_len {
            let slot = path_slots[i];
            out[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(VectorAddress::new(0,0,0,0));
        }
        (out, copy_len)
    }

    /// V2.36: Transitive reachability — all nodes reachable from `from` via
    /// directed edges, excluding `from` itself.
    ///
    /// Uses iterative DFS with a visited bitmap.  Returns `(out, len)` where
    /// `out[0..len]` is the reachable set sorted ascending by VectorAddress.
    /// Returns `(out, 0)` when `from` is not registered or no outbound edges
    /// lead to unvisited nodes.
    ///
    /// Complexity: O(V + E), no_std safe, stack arrays only.
    /// Analogous to `systemctl list-dependencies --all`, `cargo tree -p`,
    /// or `ldd --recursive`.
    pub fn graph_reachable_inner<const N: usize>(
        &self,
        from: VectorAddress,
    ) -> ([VectorAddress; N], usize) {
        let mut out = [VectorAddress::new(0, 0, 0, 0); N];

        let from_slot = match self.node_slot_by_vec(from) {
            Some(s) => s,
            None => return (out, 0),
        };

        let mut visited = [false; MAX_NODES];
        visited[from_slot] = true;

        // Iterative DFS stack — stores node slots.
        let mut stack = [0usize; MAX_NODES];
        let mut stack_top = 0usize;
        stack[stack_top] = from_slot;
        stack_top += 1;

        // Collect reachable slots (excluding `from_slot`).
        let mut reach_slots = [0usize; MAX_NODES];
        let mut reach_len = 0usize;

        while stack_top > 0 {
            stack_top -= 1;
            let cur_slot = stack[stack_top];

            let cur_id = match self.nodes[cur_slot] {
                Some(r) => r.spec.node_id,
                None => continue,
            };

            // Enumerate outbound edges.
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_id { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) {
                    Some(s) => s,
                    None => continue,
                };
                if nbr_slot == cur_slot { continue; } // skip self-loops
                if visited[nbr_slot] { continue; }
                visited[nbr_slot] = true;
                if reach_len < MAX_NODES {
                    reach_slots[reach_len] = nbr_slot;
                    reach_len += 1;
                }
                if stack_top < MAX_NODES {
                    stack[stack_top] = nbr_slot;
                    stack_top += 1;
                }
            }
        }

        // Sort reach_slots by vector address (ascending).
        // Insertion sort — N ≤ 128, good enough.
        for i in 1..reach_len {
            let key = reach_slots[i];
            let key_vec = self.nodes[key].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            let mut j = i;
            while j > 0 {
                let prev = reach_slots[j - 1];
                let prev_vec = self.nodes[prev].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
                if prev_vec.as_u64() <= key_vec.as_u64() { break; }
                reach_slots[j] = reach_slots[j - 1];
                j -= 1;
            }
            reach_slots[j] = key;
        }

        // Pack into output, capped at N.
        let copy_len = reach_len.min(N);
        for i in 0..copy_len {
            let slot = reach_slots[i];
            out[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
        }
        (out, copy_len)
    }

    /// V2.37: Bipartite-check via BFS 2-coloring on the undirected projection
    /// of the live directed graph.
    ///
    /// A graph is bipartite iff it can be 2-coloured (set A / set B) such that
    /// no two adjacent nodes share a colour.  Equivalently, a graph is bipartite
    /// iff it contains no odd-length cycle.  The check is done on the undirected
    /// projection — every directed edge is treated as undirected.
    ///
    /// Returns `(vecs, colors, total, is_bipartite)`:
    /// - `vecs[0..total]`   — live node vectors (in slot order).
    /// - `colors[0..total]` — 0 = set A, 1 = set B (valid only when is_bipartite).
    /// - `total`            — number of live nodes packed into the output arrays.
    /// - `is_bipartite`     — true iff the graph admits a valid 2-colouring.
    ///
    /// Algorithm: BFS 2-colouring.  O(V+E), no_std safe, fixed-size stack arrays.
    /// OS analogy: `bipartite_check` in graph libraries, or testing whether a
    /// dependency graph can be cleanly split into two non-conflicting sets.
    pub fn graph_bipartite_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u8; N], usize, bool) {
        const UNCOLORED: u8 = u8::MAX;

        let mut out_vecs   = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_colors = [0u8; N];

        // Compact list of live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Per-slot colour: UNCOLORED | 0 (set A) | 1 (set B).
        let mut slot_colors = [UNCOLORED; MAX_NODES];

        // BFS queue holds node slots.
        let mut queue = [0usize; MAX_NODES];

        let mut is_bipartite = true;

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if slot_colors[start_slot] != UNCOLORED {
                continue; // already coloured by a previous BFS component
            }

            // Seed BFS with colour 0.
            slot_colors[start_slot] = 0;
            queue[0] = start_slot;
            let mut q_head = 0usize;
            let mut q_tail = 1usize;

            while q_head < q_tail {
                let cur_slot  = queue[q_head];
                q_head += 1;
                let cur_color = slot_colors[cur_slot];
                let next_color = 1 - cur_color; // flip: 0↔1

                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                // Scan all edges; treat every edge as undirected.
                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => continue };

                    // Resolve the neighbour: either forward or reverse direction.
                    let nbr_id = if edge.spec.from_node == cur_id {
                        edge.spec.to_node
                    } else if edge.spec.to_node == cur_id {
                        edge.spec.from_node
                    } else {
                        continue
                    };

                    let nbr_slot = match self.node_slot_by_id(nbr_id) {
                        Some(s) => s,
                        None    => continue,
                    };
                    if nbr_slot == cur_slot { continue; } // ignore self-loops

                    if slot_colors[nbr_slot] == UNCOLORED {
                        slot_colors[nbr_slot] = next_color;
                        if q_tail < MAX_NODES {
                            queue[q_tail] = nbr_slot;
                            q_tail += 1;
                        }
                    } else if slot_colors[nbr_slot] == cur_color {
                        // Same colour on both endpoints → odd cycle → not bipartite.
                        is_bipartite = false;
                        // Continue colouring so every node gets a slot assignment
                        // (useful for diagnostics even when not bipartite).
                    }
                }
            }
        }

        // Pack output (slot order — same as graph_scc layout).
        let copy_len = node_count.min(N);
        for i in 0..copy_len {
            let slot = node_slots[i];
            out_vecs[i] = self.nodes[slot]
                .map(|r| r.vector)
                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_colors[i] = if slot_colors[slot] == UNCOLORED {
                0
            } else {
                slot_colors[slot]
            };
        }

        (out_vecs, out_colors, copy_len, is_bipartite)
    }

    /// V2.38: In/out degree census — count directed edges incident on each live node.
    ///
    /// For every live node:
    ///   out_degree = number of edges where `from_node == node_id`
    ///   in_degree  = number of edges where `to_node  == node_id`
    ///   total      = out_degree + in_degree
    ///
    /// Output arrays are sorted by descending total degree so hubs appear first.
    /// Self-loops count once toward both in-degree and out-degree.
    ///
    /// Returns `(vecs, out_degrees, in_degrees, total)`:
    ///   vecs[0..total]        — live node vectors, descending total-degree order.
    ///   out_degrees[0..total] — directed out-degree per node.
    ///   in_degrees[0..total]  — directed in-degree per node.
    ///   total                 — number of live nodes packed into the output arrays.
    ///
    /// Algorithm: O(V × E) census — acceptable for V ≤ 128, E ≤ 512.
    /// OS analogy: `ip -s link show` / `ss -s` — per-node TX/RX packet statistics.
    pub fn graph_degree_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u16; N], [u16; N], usize) {
        let mut out_vecs    = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_degrees = [0u16; N];
        let mut in_degrees  = [0u16; N];

        // Compact live-node slot list.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Per-slot degree accumulators.
        let mut slot_out = [0u16; MAX_NODES];
        let mut slot_in  = [0u16; MAX_NODES];

        // Census: scan all edges once, resolving both endpoints.
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if let Some(from_slot) = self.node_slot_by_id(edge.spec.from_node) {
                slot_out[from_slot] = slot_out[from_slot].saturating_add(1);
            }
            if let Some(to_slot) = self.node_slot_by_id(edge.spec.to_node) {
                slot_in[to_slot] = slot_in[to_slot].saturating_add(1);
            }
        }

        // Sort node_slots by descending total degree (insertion sort — N ≤ 128).
        for i in 1..node_count {
            let key_slot  = node_slots[i];
            let key_total = (slot_out[key_slot] as u32) + (slot_in[key_slot] as u32);
            let mut j = i;
            while j > 0 {
                let prev_slot  = node_slots[j - 1];
                let prev_total = (slot_out[prev_slot] as u32) + (slot_in[prev_slot] as u32);
                if prev_total >= key_total { break; }
                node_slots[j] = node_slots[j - 1];
                j -= 1;
            }
            node_slots[j] = key_slot;
        }

        // Pack into output arrays, capped at N.
        let copy_len = node_count.min(N);
        for i in 0..copy_len {
            let slot = node_slots[i];
            out_vecs[i]    = self.nodes[slot].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_degrees[i] = slot_out[slot];
            in_degrees[i]  = slot_in[slot];
        }

        (out_vecs, out_degrees, in_degrees, copy_len)
    }

    /// V2.39: Betweenness centrality via Brandes' algorithm (directed, unweighted).
    ///
    /// For each live node v, computes the raw betweenness centrality:
    ///   BC[v] = Σ_{s≠v≠t} σ(s,t,v) / σ(s,t)
    /// where σ(s,t) = number of shortest paths from s to t, and
    ///       σ(s,t,v) = number of those paths passing through v.
    ///
    /// Implementation uses fixed-point scaling (SCALE = 1_000_000) internally so
    /// that fractional path ratios are preserved during accumulation.  The output
    /// `bc[i]` is the truncated integer part of BC[v] (i.e., raw_scaled / SCALE).
    ///
    /// Returns `(vecs, bc, total)`:
    ///   vecs[0..total] — live node vectors, descending betweenness order.
    ///   bc[0..total]   — truncated betweenness score per node.
    ///   total          — number of live nodes packed into the output arrays.
    ///
    /// Algorithm: Brandes 2001, O(V × E) for unweighted directed graphs.
    /// Fixed-point accumulation avoids floating-point in no_std context.
    /// OS analogy: `traceroute` hop popularity stats — which kernel service node
    /// sits on the most inter-node communication paths?
    pub fn graph_centrality_inner<const N: usize>(&self) -> ([VectorAddress; N], [u32; N], usize) {
        const SCALE: u64 = 1_000_000;

        // Compact list of live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Scaled betweenness accumulator per slot.
        let mut bc_scaled = [0u64; MAX_NODES];

        // ── Brandes' algorithm — one BFS per source ──────────────────────────
        for si in 0..node_count {
            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };
            let _ = s_id; // s_id used implicitly via BFS below

            // BFS data structures (stack arrays, slot-indexed).
            let mut dist    = [u32::MAX; MAX_NODES]; // shortest-path distance from s
            let mut sigma   = [0u32;     MAX_NODES]; // # shortest paths from s to v
            let mut queue   = [0usize;   MAX_NODES]; // BFS queue (slot indices)
            let mut bfs_ord = [0usize;   MAX_NODES]; // BFS traversal order for back-prop

            dist[s]  = 0;
            sigma[s] = 1;
            queue[0] = s;
            let mut q_head  = 0usize;
            let mut q_tail  = 1usize;
            let mut bfs_len = 0usize;

            // ── Forward BFS phase ────────────────────────────────────────────
            while q_head < q_tail {
                let v = queue[q_head];
                q_head += 1;

                if bfs_len < MAX_NODES {
                    bfs_ord[bfs_len] = v;
                    bfs_len += 1;
                }

                let v_id = match self.nodes[v] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => continue,
                    };
                    if edge.spec.from_node != v_id { continue; }

                    let w = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(slot) => slot,
                        None       => continue,
                    };

                    // Discover w for the first time.
                    if dist[w] == u32::MAX {
                        dist[w] = dist[v].saturating_add(1);
                        if q_tail < MAX_NODES {
                            queue[q_tail] = w;
                            q_tail += 1;
                        }
                    }
                    // w is on a shortest path from s through v.
                    if dist[w] == dist[v].saturating_add(1) {
                        sigma[w] = sigma[w].saturating_add(sigma[v]);
                    }
                }
            }

            // ── Back-propagation phase (reverse BFS order) ───────────────────
            // delta[v] accumulates the pair-dependency of v, scaled by SCALE.
            let mut delta = [0u64; MAX_NODES];

            for bi in (0..bfs_len).rev() {
                let w = bfs_ord[bi];
                if w == s || sigma[w] == 0 { continue; }

                let w_id = match self.nodes[w] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                // Sum contributions from all in-neighbors v of w that are
                // predecessors in the shortest-path DAG (dist[w] == dist[v]+1).
                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => continue,
                    };
                    if edge.spec.to_node != w_id { continue; }

                    let v = match self.node_slot_by_id(edge.spec.from_node) {
                        Some(slot) => slot,
                        None       => continue,
                    };
                    if dist[v] == u32::MAX { continue; }
                    if dist[w] != dist[v].saturating_add(1) { continue; }
                    if sigma[w] == 0 { continue; }

                    // δ[v] += (σ[v] / σ[w]) × (SCALE + δ[w])
                    let contribution = (sigma[v] as u64)
                        .saturating_mul(SCALE.saturating_add(delta[w]))
                        / (sigma[w] as u64);
                    delta[v] = delta[v].saturating_add(contribution);
                }

                // Accumulate into betweenness for w (≠ source s).
                bc_scaled[w] = bc_scaled[w].saturating_add(delta[w]);
            }
        }

        // ── Sort node_slots by descending betweenness (insertion sort) ────────
        let mut sorted = node_slots;
        for i in 1..node_count {
            let key_slot = sorted[i];
            let key_bc   = bc_scaled[key_slot];
            let mut j    = i;
            while j > 0 && bc_scaled[sorted[j - 1]] < key_bc {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        // ── Pack output arrays (cap at N) ────────────────────────────────────
        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_bc   = [0u32; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot       = sorted[i];
            out_vecs[i]    = self.nodes[slot].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_bc[i]      = (bc_scaled[slot] / SCALE) as u32;
        }

        (out_vecs, out_bc, copy_len)
    }

    /// V2.32: Directed cycle detection via iterative DFS with 3-color marking.
    ///
    /// Returns `(path, length)` where `path[0..length]` is the detected cycle:
    /// `path[0] == path[length-1]` is the node where the back-edge closes the
    /// cycle.  `length == 0` means no cycle exists (the graph is a DAG).
    ///
    /// Color semantics:
    ///   0 = WHITE (unvisited)
    ///   1 = GRAY  (on the current DFS path — ancestor)
    ///   2 = BLACK (fully explored — no further cycles through here)
    ///
    /// A back edge (edge from a GRAY node to another GRAY node) signals a cycle.
    /// The algorithm is O(V+E), no_std safe, and uses only stack arrays.
    pub fn find_graph_cycle_inner<const N: usize>(&self) -> ([VectorAddress; N], usize) {
        const WHITE: u8 = 0;
        const GRAY:  u8 = 1;
        const BLACK: u8 = 2;

        let mut out   = [VectorAddress::new(0, 0, 0, 0); N];
        let mut color = [WHITE; MAX_NODES];

        // DFS stack: (node_slot, resume_edge_array_index)
        let mut stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        for start_slot in 0..MAX_NODES {
            if self.nodes[start_slot].is_none() { continue; }
            if color[start_slot] != WHITE { continue; }

            color[start_slot] = GRAY;
            stack[0] = (start_slot, 0);
            let mut stack_top = 1usize;

            while stack_top > 0 {
                let frame_idx = stack_top - 1;
                let (cur_slot, scan_start) = stack[frame_idx];

                let cur_node_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None => { color[cur_slot] = BLACK; stack_top -= 1; continue; }
                };

                let mut pushed_child = false;

                let mut ei = scan_start;
                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None => { ei += 1; continue; }
                    };
                    if edge.spec.from_node != cur_node_id {
                        ei += 1;
                        continue;
                    }

                    // Save resume position past this edge before acting on it.
                    stack[frame_idx].1 = ei + 1;

                    let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(s) => s,
                        None => { ei += 1; continue; }
                    };

                    if color[nbr_slot] == GRAY {
                        // Back edge → cycle detected.
                        // Locate where nbr_slot appears on the current path.
                        let mut cycle_start = frame_idx; // fallback to end
                        for j in 0..stack_top {
                            if stack[j].0 == nbr_slot {
                                cycle_start = j;
                                break;
                            }
                        }
                        // Cycle path: stack[cycle_start..=frame_idx] then close back to nbr_slot.
                        let cycle_len = (frame_idx - cycle_start + 1) + 1;
                        let copy_len  = cycle_len.min(N);
                        for k in 0..copy_len.saturating_sub(1) {
                            let s = stack[cycle_start + k].0;
                            out[k] = self.nodes[s].map(|r| r.vector)
                                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
                        }
                        if copy_len > 0 {
                            out[copy_len - 1] = self.nodes[nbr_slot]
                                .map(|r| r.vector)
                                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
                        }
                        return (out, copy_len);

                    } else if color[nbr_slot] == WHITE {
                        // Tree edge → descend into neighbor.
                        color[nbr_slot] = GRAY;
                        stack[stack_top] = (nbr_slot, 0);
                        stack_top += 1;
                        pushed_child = true;
                        break;
                    }
                    // BLACK → already fully explored, no cycle this way.
                    ei += 1;
                }

                if !pushed_child {
                    color[cur_slot] = BLACK;
                    stack_top -= 1;
                }
            }
        }

        (out, 0) // graph is a DAG
    }

    pub fn is_cyclic_inner(&self) -> bool {
        let (_, len) = self.find_graph_cycle_inner::<2>();
        len > 0
    }

    /// V2.33: Topological sort via Kahn's BFS algorithm (in-degree queue).
    ///
    /// Returns `(order, length, is_dag)`:
    ///   - `order[0..length]` is the topological ordering of live nodes.
    ///   - `is_dag` is `true` when the sort covers all nodes (no cycles).
    ///   - When `is_dag` is `false` (`length < node_count`) cycles prevent a
    ///     complete ordering; the partial prefix covers the nodes that CAN be
    ///     sorted before the cyclic component is encountered.
    ///
    /// Self-loops are excluded from the in-degree computation so they do not
    /// stall the queue — their host node is still emitted in dependency order.
    /// O(V+E), no_std safe, fixed stack arrays only.
    pub fn graph_toposort_inner<const N: usize>(&self) -> ([VectorAddress; N], usize, bool) {
        // Collect live node slots into a compact list.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Compute in-degree for each slot (excluding self-loops).
        let mut in_degree = [0u16; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let from_slot = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let to_slot   = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            if from_slot != to_slot {
                in_degree[to_slot] = in_degree[to_slot].saturating_add(1);
            }
        }

        // Seed the BFS queue with all in-degree-0 nodes.
        let mut queue   = [0usize; MAX_NODES];
        let mut q_head  = 0usize;
        let mut q_tail  = 0usize;
        for k in 0..node_count {
            let s = node_slots[k];
            if in_degree[s] == 0 {
                queue[q_tail] = s;
                q_tail += 1;
            }
        }

        // BFS — emit each zero-in-degree node and decrement its successors.
        let mut out     = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_len = 0usize;

        while q_head < q_tail {
            let cur_slot = queue[q_head];
            q_head += 1;

            if out_len < N {
                out[out_len] = self.nodes[cur_slot]
                    .map(|r| r.vector)
                    .unwrap_or(VectorAddress::new(0, 0, 0, 0));
                out_len += 1;
            }

            let cur_node_id = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };

            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_node_id { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
                if nbr_slot == cur_slot { continue; } // skip self-loops
                if in_degree[nbr_slot] > 0 {
                    in_degree[nbr_slot] -= 1;
                    if in_degree[nbr_slot] == 0 {
                        if q_tail < MAX_NODES {
                            queue[q_tail] = nbr_slot;
                            q_tail += 1;
                        }
                    }
                }
            }
        }

        let is_dag = out_len == node_count;
        (out, out_len, is_dag)
    }

    /// V2.34: Strongly Connected Components via Kosaraju's two-pass DFS.
    ///
    /// Returns `(nodes, labels, total, scc_count)`:
    ///   - `nodes[0..total]` — all live nodes packed in SCC order (SCC 0 first,
    ///     then SCC 1, …).
    ///   - `labels[0..total]` — SCC index for the corresponding node (monotone
    ///     non-decreasing, so label boundaries mark component splits).
    ///   - `total` — number of live nodes.
    ///   - `scc_count` — number of distinct strongly-connected components.
    ///
    /// An SCC with > 1 node is a true cycle in the directed graph.  An SCC with
    /// exactly 1 node is either isolated or connected only via DAG edges.
    /// When `scc_count == total` the graph has no directed cycles (it is a DAG).
    ///
    /// Self-loops do not merge an SCC with another — a single node with a
    /// self-loop forms its own SCC of size 1.
    ///
    /// O(V+E), no_std safe, fixed stack arrays only.  N must be ≥ total live
    /// nodes for complete output; results are silently truncated otherwise.
    pub fn graph_scc_inner<const N: usize>(&self) -> ([VectorAddress; N], [u8; N], usize, usize) {
        const UNSET: u16 = u16::MAX;

        // Compact list of live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // ── Phase 1: forward DFS → finish-order stack ─────────────────────────
        let mut visited      = [false; MAX_NODES];
        let mut finish_stack = [0usize; MAX_NODES]; // slots in finish order (earliest first)
        let mut finish_len   = 0usize;

        // Explicit DFS stack: (slot, next_edge_index_to_scan)
        let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if visited[start_slot] { continue; }

            visited[start_slot] = true;
            dfs_stack[0] = (start_slot, 0);
            let mut stack_top = 1usize;

            while stack_top > 0 {
                let fi = stack_top - 1;
                let (cur_slot, scan_start) = dfs_stack[fi];

                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None => { stack_top -= 1; continue; }
                };

                let mut pushed = false;
                let mut ei = scan_start;
                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                    if edge.spec.from_node != cur_id { ei += 1; continue; }

                    dfs_stack[fi].1 = ei + 1; // save resume point

                    let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(s) => s, None => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; } // skip self-loops

                    if !visited[nbr_slot] {
                        visited[nbr_slot] = true;
                        dfs_stack[stack_top] = (nbr_slot, 0);
                        stack_top += 1;
                        pushed = true;
                        break;
                    }
                    ei += 1;
                }

                if !pushed {
                    if finish_len < MAX_NODES { finish_stack[finish_len] = cur_slot; finish_len += 1; }
                    stack_top -= 1;
                }
            }
        }

        // ── Phase 2: transposed DFS in reverse finish order → assign SCC IDs ──
        let mut scc_id:    [u16; MAX_NODES] = [UNSET; MAX_NODES];
        let mut scc_count: usize            = 0;

        for fi in (0..finish_len).rev() {
            let start_slot = finish_stack[fi];
            if scc_id[start_slot] != UNSET { continue; }

            let comp = scc_count as u16;
            scc_id[start_slot] = comp;
            dfs_stack[0] = (start_slot, 0);
            let mut stack_top = 1usize;

            while stack_top > 0 {
                let frame = stack_top - 1;
                let (cur_slot, scan_start) = dfs_stack[frame];

                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None => { stack_top -= 1; continue; }
                };

                let mut pushed = false;
                let mut ei = scan_start;
                while ei < MAX_EDGES {
                    // Transposed graph: follow edges WHERE to_node == cur_id → move to from_node.
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                    if edge.spec.to_node != cur_id { ei += 1; continue; }

                    dfs_stack[frame].1 = ei + 1;

                    let nbr_slot = match self.node_slot_by_id(edge.spec.from_node) {
                        Some(s) => s, None => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; } // skip self-loops

                    if scc_id[nbr_slot] == UNSET {
                        scc_id[nbr_slot] = comp;
                        dfs_stack[stack_top] = (nbr_slot, 0);
                        stack_top += 1;
                        pushed = true;
                        break;
                    }
                    ei += 1;
                }

                if !pushed { stack_top -= 1; }
            }

            scc_count += 1;
        }

        // ── Phase 3: pack output grouped by SCC ID (0, 1, 2, …) ──────────────
        let mut out_nodes  = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_labels = [0u8; N];
        let mut out_len    = 0usize;

        for scc_idx in 0..scc_count {
            for ki in 0..node_count {
                let slot = node_slots[ki];
                if scc_id[slot] as usize != scc_idx { continue; }
                if out_len < N {
                    out_nodes[out_len]  = self.nodes[slot]
                        .map(|r| r.vector)
                        .unwrap_or(VectorAddress::new(0, 0, 0, 0));
                    out_labels[out_len] = scc_idx.min(254) as u8;
                    out_len += 1;
                }
            }
        }

        (out_nodes, out_labels, out_len, scc_count)
    }

    /// V2.35: Condensation DAG of the live node graph.
    ///
    /// Runs the same Kosaraju two-pass DFS as `graph_scc_inner`, then scans
    /// all live edges to record which SCC pairs have at least one crossing edge.
    /// Returns the same `(nodes, labels, total, scc_count)` view as SCC, plus:
    /// - `adj[i]` — u128 bitmask: bit `j` set → condensation edge SCC i→SCC j.
    /// - `cond_edges` — number of distinct condensation edges.
    ///
    /// The condensation is always a DAG (by definition of SCC).
    pub fn graph_condensation_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u8; N], usize, usize, [u128; 128], usize) {
        const UNSET: u16 = u16::MAX;

        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Phase 1: forward DFS → finish-order stack (same as graph_scc_inner).
        let mut visited      = [false; MAX_NODES];
        let mut finish_stack = [0usize; MAX_NODES];
        let mut finish_len   = 0usize;
        let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if visited[start_slot] { continue; }
            visited[start_slot] = true;
            dfs_stack[0] = (start_slot, 0);
            let mut stack_top = 1usize;
            while stack_top > 0 {
                let fi = stack_top - 1;
                let (cur_slot, scan_start) = dfs_stack[fi];
                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None => { stack_top -= 1; continue; }
                };
                let mut pushed = false;
                let mut ei = scan_start;
                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                    if edge.spec.from_node != cur_id { ei += 1; continue; }
                    dfs_stack[fi].1 = ei + 1;
                    let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(s) => s, None => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; }
                    if !visited[nbr_slot] {
                        visited[nbr_slot] = true;
                        dfs_stack[stack_top] = (nbr_slot, 0);
                        stack_top += 1;
                        pushed = true;
                        break;
                    }
                    ei += 1;
                }
                if !pushed {
                    if finish_len < MAX_NODES { finish_stack[finish_len] = cur_slot; finish_len += 1; }
                    stack_top -= 1;
                }
            }
        }

        // Phase 2: transposed DFS in reverse finish order → assign SCC IDs.
        let mut scc_id: [u16; MAX_NODES] = [UNSET; MAX_NODES];
        let mut scc_count: usize = 0;

        for fi in (0..finish_len).rev() {
            let start_slot = finish_stack[fi];
            if scc_id[start_slot] != UNSET { continue; }
            let comp = scc_count as u16;
            scc_id[start_slot] = comp;
            dfs_stack[0] = (start_slot, 0);
            let mut stack_top = 1usize;
            while stack_top > 0 {
                let frame = stack_top - 1;
                let (cur_slot, scan_start) = dfs_stack[frame];
                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None => { stack_top -= 1; continue; }
                };
                let mut pushed = false;
                let mut ei = scan_start;
                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                    if edge.spec.to_node != cur_id { ei += 1; continue; }
                    dfs_stack[frame].1 = ei + 1;
                    let nbr_slot = match self.node_slot_by_id(edge.spec.from_node) {
                        Some(s) => s, None => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; }
                    if scc_id[nbr_slot] == UNSET {
                        scc_id[nbr_slot] = comp;
                        dfs_stack[stack_top] = (nbr_slot, 0);
                        stack_top += 1;
                        pushed = true;
                        break;
                    }
                    ei += 1;
                }
                if !pushed { stack_top -= 1; }
            }
            scc_count += 1;
        }

        // Phase 3: condensation adjacency — one bit per (from_scc, to_scc) pair.
        let mut adj = [0u128; 128];
        let mut cond_edges = 0usize;

        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let from_slot = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let to_slot   = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            if from_slot == to_slot { continue; } // skip self-loops
            let fs = scc_id[from_slot];
            let ts = scc_id[to_slot];
            if fs == UNSET || ts == UNSET || fs == ts { continue; } // intra-SCC or unassigned
            let (fi, ti) = (fs as usize, ts as usize);
            if fi < 128 && ti < 128 {
                let bit = 1u128 << ti;
                if adj[fi] & bit == 0 {
                    adj[fi] |= bit;
                    cond_edges += 1;
                }
            }
        }

        // Phase 4: pack output grouped by SCC ID (same as graph_scc_inner).
        let mut out_nodes  = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_labels = [0u8; N];
        let mut out_len    = 0usize;

        for scc_idx in 0..scc_count {
            for ki in 0..node_count {
                let slot = node_slots[ki];
                if scc_id[slot] as usize != scc_idx { continue; }
                if out_len < N {
                    out_nodes[out_len]  = self.nodes[slot]
                        .map(|r| r.vector)
                        .unwrap_or(VectorAddress::new(0, 0, 0, 0));
                    out_labels[out_len] = scc_idx.min(254) as u8;
                    out_len += 1;
                }
            }
        }

        (out_nodes, out_labels, out_len, scc_count, adj, cond_edges)
    }

    pub fn bind_instance(
        &mut self,
        vector: VectorAddress,
        instance_id: NodeInstanceId,
    ) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let mut record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        record.instance_id = instance_id;
        self.nodes[slot] = Some(record);
        Ok(())
    }

    pub fn instance_id_for_vec(&self, vector: VectorAddress) -> Option<NodeInstanceId> {
        self.node_slot_by_vec(vector)
            .and_then(|slot| self.nodes[slot].map(|r| r.instance_id))
    }

    /// Bind every node of a plugin to a given supervisor instance.
    /// Returns the count of nodes bound.  No-op for unknown plugins.
    pub fn bind_plugin_instance(
        &mut self,
        plugin_id: PluginId,
        instance_id: NodeInstanceId,
    ) -> usize {
        let mut bound = 0usize;
        for slot in self.nodes.iter_mut() {
            if let Some(record) = slot.as_mut() {
                if record.plugin_id == plugin_id {
                    record.instance_id = instance_id;
                    bound += 1;
                }
            }
        }
        bound
    }

    /// Enqueue every node belonging to a plugin onto the ready queue.
    /// Used by the supervisor when draining its lane-class ready queues
    /// into runtime dispatch.  Returns the number of nodes enqueued.
    pub fn enqueue_ready_for_plugin(&mut self, plugin_id: PluginId) -> usize {
        let mut ids: [Option<NodeId>; MAX_NODES] = [None; MAX_NODES];
        let mut count = 0usize;
        for record in self.nodes.iter().flatten() {
            if record.plugin_id == plugin_id && count < MAX_NODES {
                ids[count] = Some(record.spec.node_id);
                count += 1;
            }
        }
        let mut enqueued = 0usize;
        for id in ids.iter().flatten() {
            if self.ready_queue.push(*id).is_ok() {
                enqueued += 1;
            }
        }
        enqueued
    }

    pub fn route_edge(&mut self, edge_id: EdgeId, signal: Signal) -> Result<(), RuntimeError> {
        let slot = self.edge_slot(edge_id).ok_or(RuntimeError::EdgeNotFound)?;
        let edge = self.edges[slot].ok_or(RuntimeError::EdgeNotFound)?.spec;

        match edge.edge_type {
            RuntimeEdgeType::Call => {
                self.alloc_call_frame(edge.from_node, edge.to_node, edge_id)?;
                let target_vec = self.node_vector(edge.to_node)?;
                self.post_signal(target_vec, signal)?;
            }
            RuntimeEdgeType::Spawn
            | RuntimeEdgeType::Signal
            | RuntimeEdgeType::Mount
            | RuntimeEdgeType::Use => {
                let target_vec = self.node_vector(edge.to_node)?;
                self.post_signal(target_vec, signal)?;
            }
            // ── Stream: fan-out to ALL outbound Stream edges from source ──
            // Mimics LangGraph's multi-target edge: one signal, N subscribers.
            RuntimeEdgeType::Stream => {
                let source_node = edge.from_node;
                // Collect targets first to avoid borrow issues.
                let mut targets = [VectorAddress::new(0, 0, 0, 0); MAX_EDGES];
                let mut target_count = 0usize;
                for slot in 0..MAX_EDGES {
                    let Some(e) = self.edges[slot] else { continue };
                    if e.spec.from_node == source_node
                        && e.spec.edge_type == RuntimeEdgeType::Stream
                        && target_count < MAX_EDGES
                    {
                        if let Ok(v) = self.node_vector(e.spec.to_node) {
                            targets[target_count] = v;
                            target_count += 1;
                        }
                    }
                }
                for &v in targets.iter().take(target_count) {
                    let _ = self.post_signal(v, signal);
                }
            }
            RuntimeEdgeType::Depend => {
                self.alloc_wait_set(edge.from_node, edge.to_node)?;
            }
            RuntimeEdgeType::Return => {
                self.complete_call(edge.to_node)?;
            }
            RuntimeEdgeType::Sync => {
                self.alloc_barrier(edge.from_node, edge.to_node)?;
            }
        }

        let _ = edge.route_policy;
        let _ = edge.capability_binding;
        let _ = RoutePolicy::Direct;
        Ok(())
    }

    fn node_vector(&self, node_id: NodeId) -> Result<VectorAddress, RuntimeError> {
        let slot = self.node_slot_by_id(node_id).ok_or(RuntimeError::NodeNotFound)?;
        Ok(self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?.vector)
    }

    fn alloc_call_frame(
        &mut self,
        caller: NodeId,
        callee: NodeId,
        edge_id: EdgeId,
    ) -> Result<(), RuntimeError> {
        let slot = self.call_frames.iter_mut().find(|slot| slot.is_none()).ok_or(RuntimeError::Fault("call frame table full"))?;
        *slot = Some(CallFrame { caller, callee, _edge_id: edge_id });
        Ok(())
    }

    fn complete_call(&mut self, callee: NodeId) -> Result<(), RuntimeError> {
        if let Some(slot) = self.call_frames.iter().position(|slot| {
            slot.map(|frame| frame.callee == callee).unwrap_or(false)
        }) {
            if let Some(frame) = self.call_frames[slot] {
                self.call_frames[slot] = None;
                self.enqueue_ready(frame.caller)?;
            }
        }
        Ok(())
    }

    fn alloc_wait_set(&mut self, node: NodeId, dependency: NodeId) -> Result<(), RuntimeError> {
        let slot = self.wait_sets.iter_mut().find(|slot| slot.is_none()).ok_or(RuntimeError::Fault("wait set table full"))?;
        *slot = Some(WaitSet { _node: node, _dependency: dependency });
        Ok(())
    }

    fn alloc_barrier(&mut self, node: NodeId, dependency: NodeId) -> Result<(), RuntimeError> {
        let slot = self.barriers.iter_mut().find(|slot| slot.is_none()).ok_or(RuntimeError::Fault("barrier table full"))?;
        *slot = Some(Barrier { _node: node, _dependency: dependency });
        Ok(())
    }

    fn finish_legacy_invocation(&mut self, slot: usize, ptr: LegacyCellPtr) {
        if let Some(mut record) = self.nodes[slot] {
            let mutex = unsafe { legacy_cell_mutex(ptr) };
            let guard = mutex.lock();
            record.lifecycle = map_legacy_state(guard.state());
            drop(guard);
            self.nodes[slot] = Some(record);
            self.state_delta(record.spec.node_id, record.lifecycle);
        }
    }

    fn finish_native_invocation(
        &mut self,
        slot: usize,
        status: ExecStatus,
        initialized: bool,
        terminated: bool,
    ) {
        if let Some(mut record) = self.nodes[slot] {
            if let NodeBinding::Native(mut binding) = record.binding {
                binding.initialized = binding.initialized || initialized;
                record.binding = NodeBinding::Native(binding);
            }

            record.lifecycle = if terminated {
                NodeLifecycle::Terminated
            } else {
                map_exec_status(status)
            };

            self.nodes[slot] = Some(record);
            self.state_delta(record.spec.node_id, record.lifecycle);

            if status == ExecStatus::Fault {
                let _ = self.fault_queue.push(record.vector);
            }
        }
    }

    pub fn drain_next_fault(&mut self) -> Option<VectorAddress> {
        self.fault_queue.pop()
    }

    /// Forcibly fault the node at `vector` — the graph-OS equivalent of
    /// `kill -9 <pid>`.  Sets the node's lifecycle to `NodeLifecycle::Faulted`,
    /// emits a `StateDelta` control-plane event, and enqueues the vector on the
    /// fault queue so the supervisor's restart policy fires on the next pump tick.
    /// Already-faulted nodes are re-enqueued without error.
    /// Returns `Err(NodeNotFound)` when no registered node has that vector address.
    pub fn fault_node(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let mut record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        record.lifecycle = NodeLifecycle::Faulted;
        self.nodes[slot] = Some(record);
        self.state_delta(record.spec.node_id, NodeLifecycle::Faulted);
        let _ = self.fault_queue.push(vector);
        Ok(())
    }

    /// Resume a node at `vector` — the graph-OS equivalent of `systemctl restart`
    /// for faulted or suspended nodes.  Sets the node's lifecycle to
    /// `NodeLifecycle::Ready` so it can receive signals again, and emits a
    /// `StateDelta` control-plane event.  Does NOT bump `graph_epoch` (lifecycle
    /// state is not a structural mutation) and does NOT touch the fault queue.
    /// Returns `Err(NodeNotFound)` when no registered node has that vector address.
    pub fn resume_node(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
        let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        let mut record = self.nodes[slot].ok_or(RuntimeError::NodeNotFound)?;
        record.lifecycle = NodeLifecycle::Ready;
        self.nodes[slot] = Some(record);
        self.state_delta(record.spec.node_id, NodeLifecycle::Ready);
        Ok(())
    }

    pub fn plugin_id_for_vec(&self, vector: VectorAddress) -> Option<PluginId> {
        self.node_slot_by_vec(vector)
            .and_then(|slot| self.nodes[slot].map(|record| record.plugin_id))
    }

    fn next_work_item(&mut self) -> Option<WorkItem> {
        if let Some(node_id) = self.ready_queue.pop() {
            return Some(WorkItem::Ready(node_id));
        }

        if let Some(signal) = self.signal_queue.pop() {
            return Some(WorkItem::Signal(signal));
        }

        None
    }

    fn bump_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    pub fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            plugin_count: self.plugins.iter().filter(|slot| slot.is_some()).count(),
            node_count: self.nodes.iter().filter(|slot| slot.is_some()).count(),
            edge_count: self.edges.iter().filter(|slot| slot.is_some()).count(),
            ready_queue_len: self.ready_queue.len(),
            signal_queue_len: self.signal_queue.len(),
            tick: self.tick,
        }
    }

    pub fn is_stable(&self) -> bool {
        self.ready_queue.is_empty()
            && self.signal_queue.is_empty()
            && self.call_frames.iter().all(|slot| slot.is_none())
            && self.wait_sets.iter().all(|slot| slot.is_none())
            && self.barriers.iter().all(|slot| slot.is_none())
            && self.nodes.iter().flatten().all(|record| {
                matches!(
                    record.lifecycle,
                    NodeLifecycle::Ready
                        | NodeLifecycle::Suspended
                        | NodeLifecycle::Terminated
                        | NodeLifecycle::Faulted
                        | NodeLifecycle::Allocated
                )
            })
    }

    pub fn drain_control_plane(&mut self) -> Option<ControlPlaneEnvelope> {
        self.control_plane.pop()
    }

    pub fn emit_hello(&mut self) {
        self.emit_control_plane(ControlPlaneMessageKind::Hello, [0; 16], self.snapshot().node_count as u64, self.tick);
    }

    pub fn last_state_delta(&self, node_id: NodeId) -> Option<StateDelta> {
        let slot = self.node_slot_by_id(node_id)?;
        let record = self.nodes[slot]?;
        Some(StateDelta {
            node_id,
            state: record.lifecycle,
            tick: self.tick,
        })
    }
}

fn map_legacy_state(state: NodeState) -> NodeLifecycle {
    match state {
        NodeState::Unregistered => NodeLifecycle::Loaded,
        NodeState::Ready => NodeLifecycle::Ready,
        NodeState::Running => NodeLifecycle::Running,
        NodeState::Suspended => NodeLifecycle::Suspended,
        NodeState::Terminated => NodeLifecycle::Terminated,
    }
}

fn map_exec_status(status: ExecStatus) -> NodeLifecycle {
    match status {
        ExecStatus::Done | ExecStatus::Route => NodeLifecycle::Ready,
        ExecStatus::Yield => NodeLifecycle::Waiting,
        ExecStatus::Fault => NodeLifecycle::Faulted,
    }
}

unsafe fn legacy_cell_mutex(ptr: LegacyCellPtr) -> &'static Mutex<dyn NodeCell> {
    let fat: *const Mutex<dyn NodeCell> = transmute(ptr);
    &*fat
}

fn control_plane_kind_from_u8(raw: u8) -> ControlPlaneMessageKind {
    match raw {
        x if x == ControlPlaneMessageKind::Hello as u8 => ControlPlaneMessageKind::Hello,
        x if x == ControlPlaneMessageKind::PluginDiscovered as u8 => ControlPlaneMessageKind::PluginDiscovered,
        x if x == ControlPlaneMessageKind::NodeUpsert as u8 => ControlPlaneMessageKind::NodeUpsert,
        x if x == ControlPlaneMessageKind::EdgeUpsert as u8 => ControlPlaneMessageKind::EdgeUpsert,
        x if x == ControlPlaneMessageKind::StateDelta as u8 => ControlPlaneMessageKind::StateDelta,
        x if x == ControlPlaneMessageKind::SnapshotChunk as u8 => ControlPlaneMessageKind::SnapshotChunk,
        x if x == ControlPlaneMessageKind::Fault as u8 => ControlPlaneMessageKind::Fault,
        x if x == ControlPlaneMessageKind::MutationAudit as u8 => ControlPlaneMessageKind::MutationAudit,
        x if x == ControlPlaneMessageKind::CausalOverflow as u8 => ControlPlaneMessageKind::CausalOverflow,
        x if x == ControlPlaneMessageKind::RuleApplied as u8 => ControlPlaneMessageKind::RuleApplied,
        x if x == ControlPlaneMessageKind::SubscribeTriggered as u8 => ControlPlaneMessageKind::SubscribeTriggered,
        _ => ControlPlaneMessageKind::Metric,
    }
}

// ── Pluggable heap backend ──────────────────────────────────────────────────
//
// `kernel_alloc_pages` / `kernel_free_pages` only enforce supervisor quota.
// The actual page-frame allocation is delegated to a backend that the kernel
// installs at boot (typically forwarding to k-pmm / k-heap).  Until a backend
// is installed, alloc returns null and a plugin's failed allocation surfaces
// as `ExecStatus::Fault`, which the B.1 fault bridge then routes to the
// supervisor's restart policy.

#[derive(Clone, Copy)]
pub struct HeapBackend {
    pub alloc: unsafe extern "C" fn(page_count: usize) -> *mut u8,
    pub free: unsafe extern "C" fn(ptr: *mut u8, page_count: usize),
}

static HEAP_BACKEND: Mutex<Option<HeapBackend>> = Mutex::new(None);

pub fn install_heap_backend(backend: HeapBackend) {
    *HEAP_BACKEND.lock() = Some(backend);
}

// Audit counter: every alloc_pages call that takes the NodeInstanceId::ZERO
// fallback (no supervisor instance bound) increments this counter.  After
// realize_boot_modules + the rebind sweep, additional increments mean a
// builtin slipped past B.3.3 — surfaced via shell `where` for verification.
static BOOT_FALLBACK_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn boot_fallback_alloc_count() -> u64 {
    BOOT_FALLBACK_ALLOC_COUNT.load(Ordering::Relaxed)
}

pub fn reset_boot_fallback_alloc_count() {
    BOOT_FALLBACK_ALLOC_COUNT.store(0, Ordering::Relaxed);
}

// Boot manifest verification report — written once at the end of the boot
// sequence by `record_boot_manifest_report()`, which hypervisor calls after
// `verify_boot_manifest_graph()` returns.  Shell's `boot verify` command reads
// these to show the self-heal outcome without re-running the check.
static BOOT_MANIFEST_RULES_CHECKED: AtomicU64 = AtomicU64::new(0);
static BOOT_MANIFEST_EDGES_HEALED:  AtomicU64 = AtomicU64::new(0);

pub fn record_boot_manifest_report(rules_checked: usize, edges_healed: usize) {
    BOOT_MANIFEST_RULES_CHECKED.store(rules_checked as u64, Ordering::Relaxed);
    BOOT_MANIFEST_EDGES_HEALED.store(edges_healed as u64, Ordering::Relaxed);
}

pub fn boot_manifest_rules_checked() -> usize {
    BOOT_MANIFEST_RULES_CHECKED.load(Ordering::Relaxed) as usize
}

pub fn boot_manifest_edges_healed() -> usize {
    BOOT_MANIFEST_EDGES_HEALED.load(Ordering::Relaxed) as usize
}

// Tracks the vector currently dispatching a native plugin so the heap ABI
// can resolve the active instance.  The kernel is single-threaded, so a
// plain Mutex<Option<_>> is sufficient.
static CURRENT_DISPATCH: Mutex<Option<VectorAddress>> = Mutex::new(None);

fn set_current_dispatch(vector: VectorAddress) {
    *CURRENT_DISPATCH.lock() = Some(vector);
}

fn clear_current_dispatch() {
    *CURRENT_DISPATCH.lock() = None;
}

fn current_dispatch_instance() -> Option<NodeInstanceId> {
    let vector = (*CURRENT_DISPATCH.lock())?;
    RUNTIME.lock().instance_id_for_vec(vector)
}

/// Public reader for the currently-dispatching instance.  Used by the
/// fault path in k-idt to attribute CPU exceptions to whichever module
/// was on the stack when the exception fired.  Returns None if the CPU
/// is not inside a native dispatch (boot init, idle, etc).
pub fn dispatching_instance() -> Option<NodeInstanceId> {
    current_dispatch_instance()
}

// ── Fault dispatch hook (Phase B.4.3) ────────────────────────────────────────
//
// When a CPU exception fires inside a native plugin dispatch, the trap
// normalizer needs to notify the supervisor so its ModuleFaultPolicy can
// run.  To avoid a runtime->supervisor dependency cycle, the supervisor
// installs a fault-dispatch hook here at bootstrap, and k-idt calls
// `dispatch_fault(instance_id)` from the trap path.
#[derive(Clone, Copy)]
pub struct FaultDispatch {
    pub fault: unsafe extern "C" fn(instance_id: NodeInstanceId),
}

static FAULT_DISPATCH: Mutex<Option<FaultDispatch>> = Mutex::new(None);

pub fn install_fault_dispatch(hook: FaultDispatch) {
    *FAULT_DISPATCH.lock() = Some(hook);
}

/// Notify the supervisor that the given instance has faulted at the CPU
/// level (page fault, GP fault, etc).  No-op if no supervisor hook is
/// installed (boot-time / unit tests).
pub fn dispatch_fault(instance_id: NodeInstanceId) {
    let hook = *FAULT_DISPATCH.lock();
    if let Some(hook) = hook {
        unsafe { (hook.fault)(instance_id) };
    }
}

// ── Scheduler hooks (Phase E.1) ──────────────────────────────────────────────
//
// PIT tick fires -> Scheduler::on_tick decrements the active instance's
// time-slice budget.  When budget reaches zero, the supervisor sets
// `preempt_requested` on the instance.  The runtime checks the flag
// after every native callback returns; if set, the instance is
// re-enqueued at the ready-queue tail and the flag cleared — soft
// preemption that catches both event-loop hogs and instances that take
// many short callbacks but never voluntarily yield.
//
// True hard preemption (interrupt long-running plugin code mid-callback)
// is Phase E.4 territory and depends on per-domain CR3 + IST stacks
// (B.4.3/.4 are already in).
#[derive(Clone, Copy)]
pub struct Scheduler {
    /// PIT tick — supervisor decrements current instance's budget.
    pub on_tick: unsafe extern "C" fn(),
    /// Did this instance exhaust its time slice since last check?
    pub should_preempt: unsafe extern "C" fn(instance_id: NodeInstanceId) -> bool,
    /// Acknowledge: clear the preempt flag and reset the budget.
    pub clear_preempt: unsafe extern "C" fn(instance_id: NodeInstanceId),
}

static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);
static PREEMPT_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn install_scheduler(hook: Scheduler) {
    *SCHEDULER.lock() = Some(hook);
}

pub fn preempt_count() -> u64 {
    PREEMPT_COUNT.load(Ordering::Relaxed)
}

/// Called from the PIT tick path (k-pit post stage).  No-op when no
/// supervisor hook is installed (boot-time / unit tests that don't
/// need preemption).
pub fn tick_pulse() {
    let hook = *SCHEDULER.lock();
    if let Some(hook) = hook {
        unsafe { (hook.on_tick)() };
    }
}

fn scheduler_should_preempt(instance_id: NodeInstanceId) -> bool {
    if instance_id == NodeInstanceId::ZERO {
        return false;
    }
    let hook = *SCHEDULER.lock();
    match hook {
        Some(hook) => unsafe { (hook.should_preempt)(instance_id) },
        None => false,
    }
}

fn scheduler_clear_preempt(instance_id: NodeInstanceId) {
    let hook = *SCHEDULER.lock();
    if let Some(hook) = hook {
        unsafe { (hook.clear_preempt)(instance_id) };
    }
}

// ── Domain CR3 trampoline (Phase B.4.4) ──────────────────────────────────────
//
// Per the B.4 design doc, every native dispatch is bracketed with a CR3
// switch into the target instance's domain.  The supervisor installs the
// actual switch implementation at bootstrap (`enter` returns a saved
// token, `leave` restores from it).  Without an installed hook the
// trampoline is a no-op — covers host-testing and pre-bootstrap boot.
//
// The hook is permitted (and expected today) to short-circuit when the
// target's root_table_phys equals the live CR3 — that's the case for
// every builtin until ELF-loaded modules ship in B.4.6.  Until then,
// this gives us the API surface, RAII guard, and a measurable
// transition counter without changing on-CPU semantics.
#[derive(Clone, Copy)]
pub struct DomainSwitch {
    /// Switch CR3 to the domain owning `instance_id`; return an opaque
    /// token the supervisor can later use in `leave` to restore.
    pub enter: unsafe extern "C" fn(instance_id: NodeInstanceId) -> u64,
    /// Restore CR3 from a token previously returned by `enter`.
    pub leave: unsafe extern "C" fn(token: u64),
}

static DOMAIN_SWITCH: Mutex<Option<DomainSwitch>> = Mutex::new(None);
static DOMAIN_SWITCH_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn install_domain_switch(hook: DomainSwitch) {
    *DOMAIN_SWITCH.lock() = Some(hook);
}

pub fn domain_switch_count() -> u64 {
    DOMAIN_SWITCH_COUNT.load(Ordering::Relaxed)
}

/// Begin a domain dispatch.  Returns an opaque token the caller must
/// pass to `end_domain_dispatch` (or `DomainGuard::drop` does it).
fn begin_domain_dispatch(instance_id: NodeInstanceId) -> u64 {
    let hook = *DOMAIN_SWITCH.lock();
    let Some(hook) = hook else { return 0 };
    if instance_id == NodeInstanceId::ZERO {
        return 0;
    }
    DOMAIN_SWITCH_COUNT.fetch_add(1, Ordering::Relaxed);
    unsafe { (hook.enter)(instance_id) }
}

fn end_domain_dispatch(token: u64) {
    let hook = *DOMAIN_SWITCH.lock();
    if let Some(hook) = hook {
        unsafe { (hook.leave)(token) };
    }
}

/// RAII guard ensuring `leave` runs on drop.  Used inside route_signal
/// and activate to bracket every native callback.
struct DomainGuard {
    token: u64,
    active: bool,
}

impl DomainGuard {
    fn enter(instance_id: NodeInstanceId) -> Self {
        let active = DOMAIN_SWITCH.lock().is_some() && instance_id != NodeInstanceId::ZERO;
        let token = if active {
            begin_domain_dispatch(instance_id)
        } else {
            0
        };
        Self { token, active }
    }
}

impl Drop for DomainGuard {
    fn drop(&mut self) {
        if self.active {
            end_domain_dispatch(self.token);
        }
    }
}

unsafe extern "C" fn kernel_alloc_pages(page_count: usize) -> *mut u8 {
    if page_count == 0 {
        return core::ptr::null_mut();
    }
    let Some(instance_id) = current_dispatch_instance() else {
        return core::ptr::null_mut();
    };
    if instance_id == NodeInstanceId::ZERO {
        // Boot-time builtin nodes have no instance binding yet — let them
        // through unaccounted for now.  Once every builtin is mapped to an
        // instance this branch can be removed.  Audit count: every hit
        // here after realize_boot_modules indicates an unbound builtin.
        BOOT_FALLBACK_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        let backend = *HEAP_BACKEND.lock();
        return match backend {
            Some(backend) => unsafe { (backend.alloc)(page_count) },
            None => core::ptr::null_mut(),
        };
    }
    // Charge supervisor quota first; refuse on exceed.
    if gos_supervisor_charge_heap(instance_id, page_count as u32).is_err() {
        return core::ptr::null_mut();
    }
    let backend = *HEAP_BACKEND.lock();
    match backend {
        Some(backend) => {
            let ptr = unsafe { (backend.alloc)(page_count) };
            if ptr.is_null() {
                gos_supervisor_credit_heap(instance_id, page_count as u32);
            }
            ptr
        }
        None => {
            // No backend installed — give back the accounting and fail.
            gos_supervisor_credit_heap(instance_id, page_count as u32);
            core::ptr::null_mut()
        }
    }
}

unsafe extern "C" fn kernel_free_pages(ptr: *mut u8, page_count: usize) {
    if ptr.is_null() || page_count == 0 {
        return;
    }
    let backend = *HEAP_BACKEND.lock();
    if let Some(backend) = backend {
        unsafe { (backend.free)(ptr, page_count) };
    }
    if let Some(instance_id) = current_dispatch_instance() {
        if instance_id != NodeInstanceId::ZERO {
            gos_supervisor_credit_heap(instance_id, page_count as u32);
        }
    }
}

// Heap accounting hooks installed by the supervisor at bootstrap.  The
// runtime cannot depend on the supervisor crate directly (that would form
// a dependency cycle), so we use an installable hook table.  When unset,
// allocation is unaccounted (boot-time fallback).
#[derive(Clone, Copy)]
pub struct HeapAccounting {
    pub charge:
        unsafe extern "C" fn(instance_id: NodeInstanceId, page_count: u32) -> i32,
    pub credit: unsafe extern "C" fn(instance_id: NodeInstanceId, page_count: u32),
}

static HEAP_ACCOUNTING: Mutex<Option<HeapAccounting>> = Mutex::new(None);

pub fn install_heap_accounting(hooks: HeapAccounting) {
    *HEAP_ACCOUNTING.lock() = Some(hooks);
}

#[inline]
fn gos_supervisor_charge_heap(
    instance_id: NodeInstanceId,
    page_count: u32,
) -> Result<(), ()> {
    let hooks = *HEAP_ACCOUNTING.lock();
    match hooks {
        Some(hooks) => {
            if unsafe { (hooks.charge)(instance_id, page_count) } == 0 {
                Ok(())
            } else {
                Err(())
            }
        }
        // No supervisor wired yet — accept everything (boot-time fallback).
        None => Ok(()),
    }
}

#[inline]
fn gos_supervisor_credit_heap(instance_id: NodeInstanceId, page_count: u32) {
    let hooks = *HEAP_ACCOUNTING.lock();
    if let Some(hooks) = hooks {
        unsafe { (hooks.credit)(instance_id, page_count) };
    }
}

unsafe extern "C" fn kernel_emit_signal(target: u64, packet: KernelSignalPacket) -> i32 {
    match route_signal(VectorAddress::from_u64(target), packet_to_signal(packet)) {
        Ok(CellResult::Fault(_)) | Err(_) => -1,
        _ => 0,
    }
}

unsafe extern "C" fn kernel_resolve_capability(
    namespace: *const u8,
    namespace_len: usize,
    name: *const u8,
    name_len: usize,
) -> u64 {
    if namespace.is_null() || name.is_null() {
        return 0;
    }

    let namespace = unsafe { core::slice::from_raw_parts(namespace, namespace_len) };
    let name = unsafe { core::slice::from_raw_parts(name, name_len) };
    resolve_capability(namespace, name)
        .map(|vector| vector.as_u64())
        .unwrap_or(0)
}

unsafe extern "C" fn kernel_emit_control_plane(
    kind: u8,
    subject: *const u8,
    subject_len: usize,
    arg0: u64,
    arg1: u64,
) {
    let mut subject_buf = [0u8; 16];
    if !subject.is_null() {
        let copied_len = subject_len.min(subject_buf.len());
        let src = unsafe { core::slice::from_raw_parts(subject, copied_len) };
        subject_buf[..copied_len].copy_from_slice(src);
    }

    with_runtime(|runtime| {
        runtime.emit_control_plane(control_plane_kind_from_u8(kind), subject_buf, arg0, arg1);
    });
}

static KERNEL_ABI: KernelAbi = KernelAbi {
    abi_version: GOS_ABI_VERSION,
    log: None,
    alloc_pages: Some(kernel_alloc_pages),
    free_pages: Some(kernel_free_pages),
    emit_signal: Some(kernel_emit_signal),
    resolve_capability: Some(kernel_resolve_capability),
    emit_control_plane: Some(kernel_emit_control_plane),
};

/// Decompose a `Signal` into the (kind, from, cmd) triple stored in a `NodeTraceEntry`.
fn signal_trace_fields(signal: Signal) -> (u8, u64, u8) {
    match signal {
        Signal::Call { from }               => (0x01, from, 0),
        Signal::Spawn { payload }           => (0x02, payload, 0),
        Signal::Interrupt { irq }           => (0x03, 0, irq),
        Signal::Data { from, byte }         => (0x04, from, byte),
        Signal::Control { cmd, val: _ }     => (0x05, 0, cmd),
        Signal::Terminate                   => (0xFF, 0, 0),
    }
}

static RUNTIME: Mutex<GraphRuntime> = Mutex::new(GraphRuntime::new());

pub fn reset() {
    RUNTIME.lock().reset();
}

pub fn emit_hello() {
    RUNTIME.lock().emit_hello();
}

pub fn discover_plugin(manifest: PluginManifest) -> Result<(), RuntimeError> {
    RUNTIME.lock().discover_plugin(manifest)
}

pub fn mark_plugin_loaded(plugin_id: PluginId) -> Result<(), RuntimeError> {
    RUNTIME.lock().mark_plugin_loaded(plugin_id)
}

pub fn mark_plugin_fault(plugin_id: PluginId) {
    RUNTIME.lock().mark_plugin_fault(plugin_id);
}

pub fn register_node(
    plugin_id: PluginId,
    vector: VectorAddress,
    spec: NodeSpec,
) -> Result<NodeId, RuntimeError> {
    RUNTIME.lock().register_node(plugin_id, vector, spec)
}

pub fn register_edge(spec: EdgeSpec) -> Result<EdgeId, RuntimeError> {
    RUNTIME.lock().register_edge(spec)
}

pub fn unregister_edge(edge_id: EdgeId) -> Result<(), RuntimeError> {
    RUNTIME.lock().unregister_edge(edge_id)
}

pub fn bind_legacy_cell(vector: VectorAddress, cell_ptr: [usize; 2]) -> Result<(), RuntimeError> {
    RUNTIME.lock().bind_legacy_cell(vector, cell_ptr)
}

pub fn bind_native_executor(
    vector: VectorAddress,
    vtable: NodeExecutorVTable,
) -> Result<(), RuntimeError> {
    RUNTIME.lock().bind_native_executor(vector, vtable)
}

pub fn describe_legacy_node(vector: VectorAddress) -> Result<CellDeclaration, RuntimeError> {
    RUNTIME.lock().describe_legacy_node(vector)
}

pub fn node_id_for_vec(vector: VectorAddress) -> Option<NodeId> {
    RUNTIME.lock().node_id_for_vec(vector)
}

pub fn edge_vector_for_id(edge_id: EdgeId) -> Option<EdgeVector> {
    RUNTIME.lock().edge_vector_for_id(edge_id)
}

pub fn edge_id_for_vector(edge_vector: EdgeVector) -> Option<EdgeId> {
    RUNTIME.lock().edge_id_for_vector(edge_vector)
}

pub fn node_summary(vector: VectorAddress) -> Option<GraphNodeSummary> {
    RUNTIME.lock().node_summary(vector)
}

pub fn node_telemetry(vector: VectorAddress) -> Option<NodeTelemetry> {
    RUNTIME.lock().node_telemetry(vector)
}

pub fn edge_summary(edge_vector: EdgeVector) -> Option<GraphEdgeSummary> {
    RUNTIME.lock().edge_summary(edge_vector)
}

/// Current structural epoch of the runtime graph.  Increments on every
/// node/edge add or remove and is stable across pure reads, so a host
/// bridge can poll it to render only when the topology actually changed.
pub fn graph_epoch() -> u64 {
    RUNTIME.lock().graph_epoch()
}

/// All structural diff entries recorded after `since_epoch`, filled into `out`
/// in chronological order.  Returns `(total_matching, filled)`.
///
/// Call with `since_epoch = 0` to get all entries since boot.
/// Call with `since_epoch = graph_epoch()` at time T1, then re-call later
/// to see only the mutations that occurred between T1 and now.
pub fn graph_diff_since<const N: usize>(
    since_epoch: u64,
    out: &mut [GraphDiffEntry; N],
) -> (usize, usize) {
    RUNTIME.lock().graph_diff_since(since_epoch, out)
}

/// Monotonic count of all structural diff entries ever pushed to the ring
/// (wraps at u64::MAX; useful for detecting ring wrap-around).
pub fn diff_total() -> u64 {
    RUNTIME.lock().diff_total()
}

/// Register a reactive Subscribe pair.  Whenever a structural mutation
/// touches `observed` (register_node / register_edge / unregister_edge),
/// the runtime emits `SubscribeTriggered` for `subscriber`.  Idempotent.
pub fn register_subscribe(observed: NodeId, subscriber: NodeId) -> Result<(), RuntimeError> {
    RUNTIME.lock().register_subscribe_pair(observed, subscriber)
}

/// Remove a reactive Subscribe pair.  No-ops when the pair is not present.
/// Must be called during module unload to prevent dead entries accumulating
/// in the subscribe table (which is a fixed-size 64-slot pool).
pub fn unregister_subscribe(observed: NodeId, subscriber: NodeId) {
    RUNTIME.lock().unregister_subscribe_pair(observed, subscriber)
}

/// Current number of active subscribe pairs.  Compare against
/// `MAX_SUBSCRIBE_PAIRS` (64) to check headroom before bulk registration.
pub fn subscribe_pair_count() -> usize {
    RUNTIME.lock().subscribe_pair_count()
}

/// V2.15: Register a u8 property value for `node_id` used as the reactive signal
/// val when that node is the active Use-edge target of an observed node.
/// Returns false when the property table is full (MAX_NODE_PROPS_U8 = 16 slots).
pub fn register_node_prop_u8(node_id: NodeId, val: u8) -> bool {
    RUNTIME.lock().register_node_prop_u8(node_id, val)
}

/// V2.15: Return the NodeId of the first active Use-edge target from `source`,
/// or None if no Use edge exists. Useful for theme-variant graph queries.
pub fn active_use_target(source: NodeId) -> Option<NodeId> {
    RUNTIME.lock().active_use_target(source)
}

/// V2.15: Drain one pending runtime Signal from the signal queue.
/// Returns `(target_vector, signal)` or None when the queue is empty.
/// Primarily for test harnesses that need to verify reactive signal delivery.
pub fn drain_signal() -> Option<(VectorAddress, Signal)> {
    let mut rt = RUNTIME.lock();
    rt.signal_queue.pop().map(|rs| (rs.target, rs.signal))
}

/// Check whether a node with `id` is currently present in the live graph.
/// Used by the supervisor's `RuntimeGraphView` for rewrite-rule pattern
/// evaluation.
pub fn node_exists_by_id(id: NodeId) -> bool {
    RUNTIME.lock().node_exists_by_id(id)
}

/// Check whether a receptive edge of `kind` exists from `from` to `to`.
/// Used by the supervisor's `RuntimeGraphView` for rewrite-rule pattern
/// evaluation.
pub fn edge_exists_by_kind(
    from: NodeId,
    to: NodeId,
    kind: gos_cypher_mut::ReceptiveEdgeKind,
) -> bool {
    RUNTIME.lock().edge_exists_by_kind(from, to, kind)
}

/// Current monotonic tick counter.  Bumped by `pump()` on every dispatch
/// cycle.  Useful as the `tick` field in `AuditedMutation` when the caller
/// does not have a wall-clock source (e.g. bare-metal shell commands).
pub fn runtime_tick() -> u64 {
    RUNTIME.lock().tick
}

pub fn node_page<const N: usize>(
    offset: usize,
    out: &mut [GraphNodeSummary; N],
) -> (usize, usize) {
    RUNTIME.lock().node_page(offset, out)
}

pub fn edge_page_for_node<const N: usize>(
    node_vec: VectorAddress,
    offset: usize,
    out: &mut [GraphEdgeSummary; N],
) -> Result<(usize, usize), RuntimeError> {
    RUNTIME.lock().edge_page_for_node(node_vec, offset, out)
}

pub fn edge_page<const N: usize>(
    offset: usize,
    out: &mut [GraphEdgeSummary; N],
) -> (usize, usize) {
    RUNTIME.lock().edge_page(offset, out)
}

pub fn resolve_capability(namespace: &[u8], capability: &[u8]) -> Option<VectorAddress> {
    RUNTIME.lock().resolve_capability(namespace, capability)
}

/// Register a conditional-route table for a node (LangGraph-style fan-out).
///
/// Call this after the node is registered (e.g. in a `register_hook`).
/// When the node's `on_event` returns `ExecStatus::Route`, the runtime
/// looks up `ctx.route_key` in this table and posts the original signal
/// to the matched `ConditionalRoute::target`.
pub fn register_node_routes(
    vector: VectorAddress,
    routes: &[ConditionalRoute],
) -> Result<(), RuntimeError> {
    RUNTIME.lock().register_node_routes(vector, routes)
}

pub fn enqueue_ready(node_id: NodeId) -> Result<(), RuntimeError> {
    RUNTIME.lock().enqueue_ready(node_id)
}

pub fn post_signal(target: VectorAddress, signal: Signal) -> Result<(), RuntimeError> {
    RUNTIME.lock().post_signal(target, signal)
}

pub fn route_signal(target: VectorAddress, signal: Signal) -> Result<CellResult, RuntimeError> {
    let dispatch = {
        let mut runtime = RUNTIME.lock();
        runtime.prepare_signal_dispatch(target, signal)?
    };

    match dispatch.binding {
        NodeBinding::Legacy(ptr) => {
            let result = {
                let mutex = unsafe { legacy_cell_mutex(ptr) };
                let mut guard = mutex.lock();
                if matches!(signal, Signal::Spawn { .. }) && guard.state() == NodeState::Unregistered {
                    unsafe { guard.init(); }
                }
                guard.on_signal(signal)
            };

            {
                let mut runtime = RUNTIME.lock();
                runtime.finish_legacy_invocation(dispatch.slot, ptr);
            }

            Ok(result)
        }
        NodeBinding::Native(binding) => {
            let state_ptr = {
                let mut runtime = RUNTIME.lock();
                runtime.node_arena.page_ptr(dispatch.runtime_page)?
            };

            // Pre-encode the incoming signal so the node can read or replace it
            // via ctx.route_signal before returning ExecStatus::Route.
            let event_packet = signal_to_packet(signal);

            let mut ctx = ExecutorContext {
                abi: &KERNEL_ABI,
                node_id: dispatch.node_id,
                vector: dispatch.vector,
                state_ptr,
                state_len: 4096,
                instance_id: dispatch.instance_id,
                route_key: 0xFF,       // sentinel — no conditional route
                route_signal: event_packet, // default: forward the original signal
            };

            let mut initialized = binding.initialized;
            let mut status = ExecStatus::Done;
            let terminated = matches!(signal, Signal::Terminate);

            set_current_dispatch(dispatch.vector);
            // Phase B.4.4: bracket the native callback in a CR3
            // trampoline.  Currently a no-op when target root == live
            // CR3 (every builtin until ELF loader ships), but the
            // wiring + transition counter is in place.
            let _domain_guard = DomainGuard::enter(dispatch.instance_id);

            if !binding.initialized {
                if let Some(on_init) = binding.vtable.on_init {
                    status = unsafe { on_init(&mut ctx) };
                }
                if status != ExecStatus::Fault {
                    initialized = true;
                }
            }

            if status != ExecStatus::Fault {
                status = if terminated {
                    if let Some(on_teardown) = binding.vtable.on_teardown {
                        unsafe { on_teardown(&mut ctx) }
                    } else {
                        ExecStatus::Done
                    }
                } else if let Some(on_event) = binding.vtable.on_event {
                    let event = NodeEvent {
                        edge_id: EdgeId::ZERO,
                        source_node: NodeId::ZERO,
                        signal: event_packet,
                    };
                    unsafe { on_event(&mut ctx, &event) }
                } else {
                    ExecStatus::Done
                };
            }

            drop(_domain_guard);
            clear_current_dispatch();

            // ── Conditional routing (LangGraph-style) ────────────────────────
            // When on_event returns Route:
            //   1. Look up ctx.route_key in the node's registered route table.
            //   2. Forward ctx.route_signal (default = original signal, but the
            //      node may have overwritten it for signal-transformation cases).
            if status == ExecStatus::Route {
                let route_key = ctx.route_key;
                let forwarded = packet_to_signal(ctx.route_signal);
                let maybe_target = {
                    let runtime = RUNTIME.lock();
                    if let Some(slot) = runtime.node_slot_by_vec(dispatch.vector) {
                        if let Some(record) = runtime.nodes[slot] {
                            let count = record.route_count as usize;
                            record.routes[..count]
                                .iter()
                                .find(|r| r.key == route_key)
                                .map(|r| r.target)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(target) = maybe_target {
                    let _ = RUNTIME.lock().post_signal(target, forwarded);
                }
            }

            {
                let mut runtime = RUNTIME.lock();
                runtime.finish_native_invocation(dispatch.slot, status, initialized, terminated);
            }

            // Phase E.1: soft preemption.  If the supervisor flagged the
            // instance during this dispatch, re-enqueue it at the ready
            // queue tail so other instances get a turn before it runs
            // again.  Status is reported as Yield even if the callback
            // returned Done, so callers see "still has work to do".
            let preempted = !terminated
                && status != ExecStatus::Fault
                && scheduler_should_preempt(dispatch.instance_id);
            if preempted {
                scheduler_clear_preempt(dispatch.instance_id);
                let _ = RUNTIME.lock().enqueue_ready(dispatch.node_id);
                PREEMPT_COUNT.fetch_add(1, Ordering::Relaxed);
            }

            Ok(match (status, preempted) {
                (_, true) => CellResult::Yield,
                (ExecStatus::Done, _) | (ExecStatus::Route, _) => CellResult::Done,
                (ExecStatus::Yield, _) => CellResult::Yield,
                (ExecStatus::Fault, _) => CellResult::Fault("native executor fault"),
            })
        }
        NodeBinding::Unbound => Err(RuntimeError::NativeExecutorMissing),
    }
}

pub fn activate(target: VectorAddress) -> Result<CellResult, RuntimeError> {
    let dispatch = {
        let mut runtime = RUNTIME.lock();
        runtime.prepare_activation(target)?
    };

    match dispatch.binding {
        NodeBinding::Legacy(ptr) => {
            let result = {
                let mutex = unsafe { legacy_cell_mutex(ptr) };
                let mut guard = mutex.lock();
                guard.on_activate()
            };

            {
                let mut runtime = RUNTIME.lock();
                runtime.finish_legacy_invocation(dispatch.slot, ptr);
            }

            Ok(result)
        }
        NodeBinding::Native(binding) => {
            let state_ptr = {
                let mut runtime = RUNTIME.lock();
                runtime.node_arena.page_ptr(dispatch.runtime_page)?
            };
            let mut ctx = ExecutorContext {
                abi: &KERNEL_ABI,
                node_id: dispatch.node_id,
                vector: dispatch.vector,
                state_ptr,
                state_len: 4096,
                instance_id: dispatch.instance_id,
                route_key: 0xFF,
                route_signal: signal_to_packet(Signal::Spawn { payload: 0 }),
            };

            let mut initialized = binding.initialized;
            let mut status = ExecStatus::Done;

            set_current_dispatch(dispatch.vector);
            let _domain_guard = DomainGuard::enter(dispatch.instance_id);

            if !binding.initialized {
                if let Some(on_init) = binding.vtable.on_init {
                    status = unsafe { on_init(&mut ctx) };
                }
                if status != ExecStatus::Fault {
                    initialized = true;
                }
            }

            if status != ExecStatus::Fault {
                status = if let Some(on_resume) = binding.vtable.on_resume {
                    unsafe { on_resume(&mut ctx) }
                } else {
                    ExecStatus::Done
                };
            }

            drop(_domain_guard);
            clear_current_dispatch();

            {
                let mut runtime = RUNTIME.lock();
                runtime.finish_native_invocation(dispatch.slot, status, initialized, false);
            }

            // Phase E.1: soft preemption mirror of the route_signal path.
            let preempted = status != ExecStatus::Fault
                && scheduler_should_preempt(dispatch.instance_id);
            if preempted {
                scheduler_clear_preempt(dispatch.instance_id);
                let _ = RUNTIME.lock().enqueue_ready(dispatch.node_id);
                PREEMPT_COUNT.fetch_add(1, Ordering::Relaxed);
            }

            Ok(match (status, preempted) {
                (_, true) => CellResult::Yield,
                (ExecStatus::Done, _) | (ExecStatus::Route, _) => CellResult::Done,
                (ExecStatus::Yield, _) => CellResult::Yield,
                (ExecStatus::Fault, _) => CellResult::Fault("native executor fault"),
            })
        }
        NodeBinding::Unbound => Err(RuntimeError::NativeExecutorMissing),
    }
}

pub fn route_edge(edge_id: EdgeId, signal: Signal) -> Result<(), RuntimeError> {
    RUNTIME.lock().route_edge(edge_id, signal)
}

pub fn pump() {
    // Hard cap: a tight signal-loop between two nodes can otherwise
    // pin the kernel inside this pump call.  4096 work items per
    // pump pass is generous (the steady-state queue depth is
    // typically <100) but bounds the worst case.  service_system_
    // cycle calls pump repeatedly, so this cap doesn't drop work —
    // it just gives the supervisor a chance to drain faults / apply
    // restart policy between batches.
    enum Dispatch {
        Activate(Result<VectorAddress, RuntimeError>),
        Signal(RuntimeSignal),
    }

    const MAX_WORK_ITEMS_PER_PUMP: u32 = 4096;
    let mut processed: u32 = 0;
    loop {
        // next_work_item() and node_vector() are both pure/non-recursive
        // reads against the same Runtime, so they share one lock
        // acquisition instead of two.
        let dispatch = {
            let mut runtime = RUNTIME.lock();
            match runtime.next_work_item() {
                Some(WorkItem::Ready(node_id)) => {
                    Some(Dispatch::Activate(runtime.node_vector(node_id)))
                }
                Some(WorkItem::Signal(signal)) => Some(Dispatch::Signal(signal)),
                None => None,
            }
        };

        let Some(dispatch) = dispatch else {
            break;
        };

        match dispatch {
            Dispatch::Activate(Ok(vector)) => {
                let _ = activate(vector);
            }
            Dispatch::Activate(Err(_)) => {}
            Dispatch::Signal(signal) => {
                let _ = route_signal(signal.target, signal.signal);
            }
        }
        processed = processed.wrapping_add(1);
        if processed >= MAX_WORK_ITEMS_PER_PUMP {
            break;
        }

        let mut runtime = RUNTIME.lock();
        runtime.bump_tick();
    }
}

pub fn snapshot() -> GraphSnapshot {
    RUNTIME.lock().snapshot()
}

pub fn is_stable() -> bool {
    RUNTIME.lock().is_stable()
}

pub fn drain_control_plane() -> Option<ControlPlaneEnvelope> {
    RUNTIME.lock().drain_control_plane()
}

pub fn last_state_delta(node_id: NodeId) -> Option<StateDelta> {
    RUNTIME.lock().last_state_delta(node_id)
}

pub fn drain_next_fault() -> Option<VectorAddress> {
    RUNTIME.lock().drain_next_fault()
}

pub fn plugin_id_for_vec(vector: VectorAddress) -> Option<PluginId> {
    RUNTIME.lock().plugin_id_for_vec(vector)
}

pub fn bind_instance(
    vector: VectorAddress,
    instance_id: NodeInstanceId,
) -> Result<(), RuntimeError> {
    RUNTIME.lock().bind_instance(vector, instance_id)
}

pub fn instance_id_for_vec(vector: VectorAddress) -> Option<NodeInstanceId> {
    RUNTIME.lock().instance_id_for_vec(vector)
}

pub fn bind_plugin_instance(plugin_id: PluginId, instance_id: NodeInstanceId) -> usize {
    RUNTIME.lock().bind_plugin_instance(plugin_id, instance_id)
}

pub fn enqueue_ready_for_plugin(plugin_id: PluginId) -> usize {
    RUNTIME.lock().enqueue_ready_for_plugin(plugin_id)
}

pub fn with_runtime<R>(f: impl FnOnce(&mut GraphRuntime) -> R) -> R {
    let mut runtime = RUNTIME.lock();
    f(&mut runtime)
}

/// Process-table page: return up to `N` `NodeProcSummary` entries starting
/// at `offset`, sorted by vector address.  Returns `(total_nodes, filled)`.
pub fn proc_page<const N: usize>(
    offset: usize,
    out: &mut [NodeProcSummary; N],
) -> (usize, usize) {
    RUNTIME.lock().proc_page(offset, out)
}

/// Total number of live nodes (process count).
pub fn proc_count() -> usize {
    RUNTIME.lock().proc_count()
}

/// Return `NodeProcSummary` for the node at `vec`, or `None` if not found.
pub fn proc_stat_for_vector(vec: VectorAddress) -> Option<NodeProcSummary> {
    RUNTIME.lock().proc_stat_for_vector(vec)
}

/// Return the signal trace ring for `vec` — up to `MAX_NODE_TRACE` most recent dispatches,
/// newest first.  Returns `(total_signals, entries_written)`.
pub fn node_trace_page(
    vec: VectorAddress,
    out: &mut [NodeTraceEntry; MAX_NODE_TRACE],
) -> Result<(u32, usize), RuntimeError> {
    RUNTIME.lock().node_trace_page(vec, out)
}

/// V2.25: Return the lifecycle event log for `vec` — up to `MAX_NODE_LOG` most recent
/// lifecycle transitions, newest first.  Returns `(total_events, entries_written)`.
pub fn node_log_page(
    vec: VectorAddress,
    out: &mut [NodeLogEntry; MAX_NODE_LOG],
) -> Result<(usize, usize), RuntimeError> {
    RUNTIME.lock().node_log_page(vec, out)
}

/// V2.26: Clear the lifecycle event log for `vec`.
/// Resets the ring to empty — subsequent node_log_page calls return 0 entries.
pub fn clear_node_log(vec: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().clear_node_log_inner(vec)
}

/// V2.27: Clear the signal trace ring for `vec`.
/// Resets the buffered trace history to empty — subsequent node_trace_page calls return 0 entries.
/// The cumulative signal_count used by `proc` is not affected.
pub fn clear_node_trace(vec: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().clear_node_trace_inner(vec)
}

/// V2.28: Reset the cumulative signal_count for `vec` to zero.
/// Subsequent `proc` and `stat` commands will show 0 for this node until the next dispatch.
/// The trace ring and lifecycle log are not affected.
pub fn reset_node_stat(vec: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().reset_node_stat_inner(vec)
}

/// Count registered nodes whose vector address has the given `l4` domain byte.
pub fn node_count_for_l4(l4: u8) -> usize {
    RUNTIME.lock().node_count_for_l4(l4)
}

/// Return a page of `GraphNodeSummary` for nodes in the given l4 domain,
/// sorted by vector address.  Returns `(total_in_domain, filled)`.
pub fn node_page_l4<const N: usize>(
    l4: u8,
    offset: usize,
    out: &mut [GraphNodeSummary; N],
) -> (usize, usize) {
    RUNTIME.lock().node_page_l4(l4, offset, out)
}

/// Count of live nodes currently in the `Faulted` lifecycle state.
pub fn faulted_node_count() -> usize {
    RUNTIME.lock().faulted_node_count()
}

/// How many valid entries are in the structural diff ring (0–`MAX_DIFF_RING`).
pub fn diff_ring_fill() -> usize {
    RUNTIME.lock().diff_ring_fill()
}

/// Return a page of `PluginSummary` in discovery order.
/// Returns `(total_plugins, filled)`.
pub fn plugin_page<const N: usize>(offset: usize, out: &mut [PluginSummary; N]) -> (usize, usize) {
    RUNTIME.lock().plugin_page(offset, out)
}

/// Total number of registered (discovered) plugins.
pub fn plugin_count() -> usize {
    RUNTIME.lock().plugin_count()
}

/// Forcibly fault the node at `vector` — graph-OS `kill -9` for graph nodes.
/// Sets lifecycle to `NodeLifecycle::Faulted`, emits a StateDelta control-plane
/// event, and enqueues the vector on the fault queue for supervisor restart
/// handling.  Returns `Err(NodeNotFound)` when no node has that vector address.
pub fn fault_node(vector: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().fault_node(vector)
}

/// Resume a node at `vector` — graph-OS `systemctl restart` for faulted or
/// suspended nodes.  Sets lifecycle to `NodeLifecycle::Ready` and emits a
/// StateDelta control-plane event.  Does not bump `graph_epoch` or touch the
/// fault queue.  Returns `Err(NodeNotFound)` when no node has that vector address.
pub fn resume_node(vector: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().resume_node(vector)
}

pub fn bootstrap_context(payload: u64) -> BootContext {
    BootContext::new(payload)
}

/// V2.28: compile-time capacity limits exposed as a typed struct.
/// Analogous to `getrlimit` / `sysctl` on Linux — operators can query
/// the maximum node, edge, and plugin counts without reading source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeCapacity {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_plugins: usize,
    pub max_ready_queue: usize,
    pub max_signal_queue: usize,
    pub max_fault_queue: usize,
    pub max_diff_ring: usize,
    pub max_node_trace: usize,
    pub max_node_log: usize,
    pub max_subscribe_pairs: usize,
    pub abi_major: u8,
    pub abi_minor: u8,
    pub abi_patch: u16,
    pub protocol_version: u16,
}

/// Return the compile-time capacity limits of this GOS runtime instance.
/// This is a pure constant read — no lock, no allocation.
pub fn runtime_capacity() -> RuntimeCapacity {
    RuntimeCapacity {
        max_nodes: MAX_NODES,
        max_edges: MAX_EDGES,
        max_plugins: MAX_PLUGINS,
        max_ready_queue: MAX_READY_QUEUE,
        max_signal_queue: MAX_SIGNAL_QUEUE,
        max_fault_queue: MAX_FAULT_QUEUE,
        max_diff_ring: MAX_DIFF_RING,
        max_node_trace: MAX_NODE_TRACE,
        max_node_log: MAX_NODE_LOG,
        max_subscribe_pairs: MAX_SUBSCRIBE_PAIRS,
        abi_major: gos_protocol::GOS_ABI_MAJOR,
        abi_minor: gos_protocol::GOS_ABI_MINOR,
        abi_patch: gos_protocol::GOS_ABI_PATCH,
        protocol_version: CONTROL_PLANE_PROTOCOL_VERSION,
    }
}

// ── Hardware IRQ → Graph routing table ───────────────────────────────────────
//
// Every IRQ vector (0–255) maps to a target VectorAddress in the graph.
// Plugins register their target with `subscribe_irq(vector, node_vec)` at boot.
//
// Design invariants (enforced here):
//   - At most ONE subscriber per IRQ vector (ownership, no fan-out in interrupt
//     context; fan-out is the Router Node's job if needed).
//   - Subscription table is populated before interrupts are enabled.
//   - `post_irq_signal` is safe to call from `extern "C"` interrupt context:
//     it takes the Spinlock briefly to enqueue a RuntimeSignal, then returns.
//     The actual node dispatch happens on the next `pump()` tick.

/// Maximum number of distinct IRQ vectors that may be subscribed.
pub const MAX_IRQ_VECTORS: usize = 256;

#[derive(Clone, Copy)]
struct IrqSubscription {
    /// Target node vector in the graph.
    target: VectorAddress,
    /// True when this slot is valid.
    active: bool,
}

struct IrqTable {
    entries: [IrqSubscription; MAX_IRQ_VECTORS],
}

impl IrqTable {
    const fn new() -> Self {
        Self {
            entries: [IrqSubscription {
                target: VectorAddress::new(0, 0, 0, 0),
                active: false,
            }; MAX_IRQ_VECTORS],
        }
    }
}

static IRQ_TABLE: Mutex<IrqTable> = Mutex::new(IrqTable::new());

// ── V2.1: MutationDispatcher wired to GraphRuntime ───────────────────────────

impl gos_cypher_mut::MutationDispatcher for GraphRuntime {
    fn lookup_node(&self, id: NodeId) -> bool {
        self.node_slot_by_id(id).is_some()
    }

    fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: gos_cypher_mut::ReceptiveEdgeKind,
    ) -> Result<(), u32> {
        let (edge_type, edge_key) = match kind {
            gos_cypher_mut::ReceptiveEdgeKind::Mount  => (RuntimeEdgeType::Mount,  "cypher.Mount"),
            gos_cypher_mut::ReceptiveEdgeKind::Use    => (RuntimeEdgeType::Use,    "cypher.Use"),
            gos_cypher_mut::ReceptiveEdgeKind::Depend => (RuntimeEdgeType::Depend, "manifest.depend"),
        };
        let edge_id = gos_protocol::derive_edge_id(from, to, edge_key);
        let spec = EdgeSpec {
            edge_id,
            from_node: from,
            to_node: to,
            edge_type,
            weight: 1.0,
            acl_mask: 0,
            route_policy: RoutePolicy::Direct,
            capability_namespace: None,
            capability_binding: None,
            vector_ref: None,
        };
        self.register_edge(spec).map(|_| ()).map_err(|_| 1u32)
    }

    fn remove_edge(&mut self, id: EdgeId) -> Result<(), u32> {
        self.unregister_edge(id).map_err(|_| 2u32)
    }

    fn rebind_use(&mut self, from: NodeId, new_target: NodeId) -> Result<(), u32> {
        // Remove the existing exclusive Use edge originating from `from`, if any.
        let old_id = self
            .edges
            .iter()
            .filter_map(|slot| *slot)
            .find(|rec| {
                rec.spec.from_node == from && rec.spec.edge_type == RuntimeEdgeType::Use
            })
            .map(|rec| rec.spec.edge_id);
        if let Some(old_id) = old_id {
            self.unregister_edge(old_id).map_err(|_| 3u32)?;
        }
        let edge_id = gos_protocol::derive_edge_id(from, new_target, "cypher.Use");
        let spec = EdgeSpec {
            edge_id,
            from_node: from,
            to_node: new_target,
            edge_type: RuntimeEdgeType::Use,
            weight: 1.0,
            acl_mask: 0,
            route_policy: RoutePolicy::Direct,
            capability_namespace: None,
            capability_binding: None,
            vector_ref: None,
        };
        self.register_edge(spec).map(|_| ()).map_err(|_| 4u32)
    }
}

/// Apply a Cypher mutation against the live runtime graph.
///
/// This is the V2.1 write path: every `CREATE EDGE`, `DELETE EDGE`, and
/// `REBIND USE` issued by the Cypher shell or AI bridge flows through here.
/// The mutation is validated by `gos-cypher-mut::apply_mutation` before
/// any runtime state is touched.
///
/// On success the mutation is wrapped in an `AuditedMutation` (stamped with
/// `source` and the current runtime tick) and emitted as a `MutationAudit`
/// control-plane envelope, making it visible to the journal and the shell's
/// live telemetry stream.  The caller receives the `AuditedMutation` back so
/// it can additionally write it to the journal ring if desired.
///
/// `source` is a 16-byte attestation tag identifying the initiator:
/// `b"K_SHELL\0\0\0\0\0\0\0\0\0"` for direct shell entry,
/// `b"K_AI\0\0\0\0\0\0\0\0\0\0\0\0"` for AI bridge mutations.
pub fn apply_cypher_mutation(
    mutation: gos_cypher_mut::CypherMutation,
    source: [u8; 16],
) -> Result<gos_cypher_mut::AuditedMutation, gos_cypher_mut::MutationError> {
    let mut rt = RUNTIME.lock();
    let tick = rt.tick;
    gos_cypher_mut::apply_mutation(&mut *rt, mutation)?;
    let audited = gos_cypher_mut::AuditedMutation { mutation, source, tick };
    let env = audited.to_envelope();
    rt.emit_control_plane(env.kind, env.subject, env.arg0, env.arg1);
    Ok(audited)
}

/// Emit a `CausalOverflow` control-plane event when the cycle iteration cap
/// is hit.  Called by `gos_supervisor::service_system_cycle` instead of
/// silently truncating.  The shell `where` view and serial logs can surface
/// this to help diagnose deep causal chains or livelock candidates.
pub fn notify_causal_overflow(depth: u32) {
    RUNTIME
        .lock()
        .emit_control_plane(ControlPlaneMessageKind::CausalOverflow, [0u8; 16], depth as u64, 0);
}

/// Emit a `RuleApplied` control-plane event after a rewrite rule fires.
/// `label` identifies the rule (e.g. b"K_REWRITE_RULE0\0"); `rule_idx` is
/// the zero-based index in the `RewriteEngine`; `epoch_after` is the graph
/// epoch immediately after the mutation so the shell can correlate telemetry.
pub fn emit_rule_applied(label: [u8; 16], rule_idx: u32, epoch_after: u64) {
    RUNTIME
        .lock()
        .emit_control_plane(ControlPlaneMessageKind::RuleApplied, label, rule_idx as u64, epoch_after);
}

/// V2.31: BFS shortest-path search from `from` to `to` across the live edge graph.
///
/// Returns `(path, length)` where `path[0..length]` is the ordered hop sequence
/// (both endpoints included).  `length == 0` = no path or a node not found.
/// `length == 1` = trivial self-path (`from == to`).
///
/// Analogous to `traceroute` / `pathping` — exposes graph connectivity through
/// the shell surface without needing a separate query language.
pub fn find_graph_path<const N: usize>(
    from: VectorAddress,
    to: VectorAddress,
) -> ([VectorAddress; N], usize) {
    RUNTIME.lock().find_graph_path_inner(from, to)
}

/// V2.32: Directed cycle detection — finds the first cycle in the live graph via
/// iterative DFS with 3-color marking (WHITE/GRAY/BLACK), analogous to
/// `tsort` detecting circular dependencies or `cargo`'s dependency-cycle error.
///
/// Returns `(path, length)` where `path[0..length]` forms a closed walk:
/// `path[0] == path[length-1]` is the node at which the back edge closes the cycle.
/// `length == 0` means the graph is acyclic (a DAG).
///
/// Cap N at 32 for typical shell use.  The cycle will be truncated to N nodes
/// but detection remains accurate — `is_cyclic()` is the canonical "any cycle?"
/// query and is cheaper to call for that purpose alone.
pub fn find_graph_cycle<const N: usize>() -> ([VectorAddress; N], usize) {
    RUNTIME.lock().find_graph_cycle_inner()
}

/// Returns `true` if the live graph contains at least one directed cycle.
///
/// Cheaper than `find_graph_cycle` when you only need the boolean — it uses
/// a cycle path buffer of length 2 so no extra stack cost beyond the DFS
/// working arrays.  Analogous to `tsort … && echo "dag" || echo "cyclic"`.
pub fn is_cyclic() -> bool {
    RUNTIME.lock().is_cyclic_inner()
}

/// V2.33: Topological sort of the live node graph (Kahn's BFS algorithm).
///
/// Returns `(order, length, is_dag)`:
/// - `order[0..length]` — nodes in dependency order: sources (in-degree 0) first,
///   sinks last.  Analogous to the output of `tsort(1)` on POSIX systems, or the
///   build-order output of `cmake --build` / `cargo build` dependency resolution.
/// - `is_dag` — `true` when ALL live nodes appear in `order[0..length]`.
///   When `false`, at least one directed cycle prevents a complete ordering.
///
/// Self-loops are ignored in the in-degree calculation.
/// N controls the output buffer depth; cap at 128 (MAX_NODES) for full coverage.
pub fn graph_toposort<const N: usize>() -> ([VectorAddress; N], usize, bool) {
    RUNTIME.lock().graph_toposort_inner()
}

/// V2.34: Strongly Connected Components of the live node graph (Kosaraju's algorithm).
///
/// Returns `(nodes, labels, total, scc_count)`:
/// - `nodes[0..total]` — all live nodes packed in component order (SCC 0 first,
///   then SCC 1, …).  Analogous to the component listing of `scc(1)` / Graphviz
///   `sccmap`, or the "dependency island" report from `cargo graph`.
/// - `labels[0..total]` — SCC index for the corresponding entry (monotone
///   non-decreasing, so label changes mark SCC boundaries).
/// - `total` — number of live nodes covered.
/// - `scc_count` — number of distinct strongly-connected components.
///
/// An SCC with more than one node contains a directed cycle.
/// When `scc_count == total` the graph is a DAG (no cycles).
/// N controls the output buffer depth; cap at 128 (MAX_NODES) for full coverage.
pub fn graph_scc<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize) {
    RUNTIME.lock().graph_scc_inner()
}

/// V2.35: Condensation DAG of the live node graph.
///
/// Returns `(nodes, labels, total, scc_count, adj, cond_edges)`:
/// - `nodes[0..total]` / `labels[0..total]` — same layout as `graph_scc`:
///   nodes packed by SCC ID (monotone non-decreasing labels).
/// - `total` — number of live nodes.
/// - `scc_count` — number of SCCs (super-nodes in the condensation DAG).
/// - `adj` — condensation adjacency matrix: `(adj[i] >> j) & 1 == 1` means
///   there is a condensation edge from SCC `i` to SCC `j`.
/// - `cond_edges` — count of distinct inter-SCC edges.
///
/// The condensation is always a DAG regardless of cycles in the source graph.
/// Analogous to `sccmap -F` (Graphviz) or the inter-package dependency view
/// from `cargo tree`.  N controls the node buffer depth (cap at 128).
pub fn graph_condensation<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize, [u128; 128], usize) {
    RUNTIME.lock().graph_condensation_inner()
}

/// V2.36: Transitive reachability — all nodes reachable from `from` via
/// directed edges in the live node graph, excluding `from` itself.
///
/// Returns `(out, len)` where `out[0..len]` are the reachable node vectors
/// sorted in ascending order.  Returns `(out, 0)` when `from` is not
/// registered or has no outbound paths to other registered nodes.
///
/// Algorithm: iterative DFS with a `[bool; MAX_NODES]` visited bitmap.
/// O(V+E), no_std safe, fixed-size stack arrays.
///
/// OS analogy: `systemctl list-dependencies --all <svc>`,
/// `cargo tree -p <crate>`, `ldd --recursive <bin>`.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_reachable<const N: usize>(from: VectorAddress) -> ([VectorAddress; N], usize) {
    RUNTIME.lock().graph_reachable_inner(from)
}

/// V2.37: Bipartite check on the live directed graph (undirected projection).
///
/// Returns `(vecs, colors, total, is_bipartite)`:
/// - `vecs[0..total]`   — live node vectors in slot order.
/// - `colors[0..total]` — 0 = set A, 1 = set B (meaningful only when is_bipartite).
/// - `total`            — number of live nodes packed.
/// - `is_bipartite`     — true iff the graph admits a valid 2-colouring (no odd cycle).
///
/// Algorithm: BFS 2-colouring on the undirected projection, O(V+E).
/// OS analogy: checking whether a scheduling dependency graph can be split into
/// two non-conflicting groups — e.g. producer/consumer tier separation.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_bipartite<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, bool) {
    RUNTIME.lock().graph_bipartite_inner()
}

/// V2.38: In/out degree census for all live nodes, sorted by descending total degree.
///
/// Returns `(vecs, out_degrees, in_degrees, total)`:
/// - `vecs[0..total]`        — live node vectors, descending total-degree order.
/// - `out_degrees[0..total]` — directed out-degree (edges leaving each node).
/// - `in_degrees[0..total]`  — directed in-degree (edges entering each node).
/// - `total`                 — number of live nodes packed into the output arrays.
///
/// Algorithm: O(V × E) census, no_std safe, fixed-size stack arrays.
/// OS analogy: `ip -s link show` or `netstat -s` — per-interface TX/RX statistics.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_degree<const N: usize>() -> ([VectorAddress; N], [u16; N], [u16; N], usize) {
    RUNTIME.lock().graph_degree_inner()
}

/// V2.39: Betweenness centrality for all live nodes (Brandes' algorithm, directed).
///
/// Returns `(vecs, bc, total)`:
/// - `vecs[0..total]` — live node vectors, descending betweenness order.
/// - `bc[0..total]`   — truncated betweenness score (raw_scaled / 1_000_000).
/// - `total`          — number of live nodes packed into the output arrays.
///
/// Algorithm: Brandes 2001, O(V × E) for unweighted directed graphs.
/// Fixed-point scaling (1_000_000) preserves fractional path ratios during
/// accumulation; output is the integer truncation of the exact value.
/// OS analogy: `traceroute` hop-popularity — which node sits on the most
/// shortest paths between other nodes in the kernel service graph?
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_centrality<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    RUNTIME.lock().graph_centrality_inner()
}

/// Register a node vector as the handler for a particular IRQ number.
///
/// Must be called before `x86_64::instructions::interrupts::enable()`.
/// Overwrites any existing subscription for that vector (last write wins).
pub fn subscribe_irq(vector: u8, target: VectorAddress) {
    let mut table = IRQ_TABLE.lock();
    table.entries[vector as usize] = IrqSubscription { target, active: true };
}

/// Post a hardware IRQ signal into the graph signal queue.
///
/// Called by `gos_trap_normalizer` (in `k-idt`) on every hardware interrupt.
/// This function must be **extremely fast** — it only enqueues; dispatching
/// happens on the next supervisor `pump()` / `service_system_cycle()` tick.
///
/// If no subscriber is registered for the vector, the signal is silently
/// dropped (the IRQ has been acknowledged at the hardware level already).
pub fn post_irq_signal(vector: u8, signal: Signal) {
    // Look up the subscriber without holding RUNTIME lock simultaneously.
    let maybe_target = {
        let table = IRQ_TABLE.lock();
        let entry = &table.entries[vector as usize];
        if entry.active { Some(entry.target) } else { None }
    };

    if let Some(target) = maybe_target {
        // Enqueue signal — if the queue is full we drop (backpressure ok in
        // interrupt context; the supervisor loop will drain it promptly).
        let _ = RUNTIME.lock().post_signal(target, signal);
    }
}
