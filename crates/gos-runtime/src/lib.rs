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
/// V2.55: per-node u32 attribute slots — palette colors, flags, arbitrary scalars.
pub const MAX_NODE_PROPS_U32: usize = 32;
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
    /// V2.55: all slots in a per-node property table (node_props_u8 or node_props_u32) are used.
    PropTableFull,
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

/// Compact read-only snapshot of live graph topology.
/// Built under RUNTIME lock; analytics run on this copy so the lock
/// is released before the O(V×E) iteration begins, preventing deadlock
/// with interrupt-context callers such as `post_irq_signal`.
struct GraphTopologySnapshot {
    node_slots:  [usize; MAX_NODES],
    node_count:  usize,
    slot_live:   [bool; MAX_NODES],
    slot_id:     [NodeId; MAX_NODES],
    slot_vec:    [VectorAddress; MAX_NODES],
    edge_live:   [bool; MAX_EDGES],
    edge_from:   [NodeId; MAX_EDGES],
    edge_to:     [NodeId; MAX_EDGES],
    edge_weight: [f32; MAX_EDGES],
}

impl GraphTopologySnapshot {
    fn node_slot_by_id(&self, id: NodeId) -> Option<usize> {
        for si in 0..self.node_count {
            let s = self.node_slots[si];
            if self.slot_id[s] == id { return Some(s); }
        }
        None
    }
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
    /// V2.55: per-node u32 attribute slots — stores arbitrary scalar attributes
    /// (palette colors, flags, counters) keyed by NodeId. Parallel to node_props_u8
    /// but wider; forms the graph-native replacement for hardcoded PAL_U32 constants.
    node_props_u32: [(NodeId, u32); MAX_NODE_PROPS_U32],
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
            node_props_u32: [(NodeId::ZERO, 0u32); MAX_NODE_PROPS_U32],
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

    fn topology_snapshot(&self) -> GraphTopologySnapshot {
        const ZERO_ID:  NodeId        = NodeId([0u8; 16]);
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        let mut snap = GraphTopologySnapshot {
            node_slots:  [0usize; MAX_NODES],
            node_count:  0,
            slot_live:   [false; MAX_NODES],
            slot_id:     [ZERO_ID;  MAX_NODES],
            slot_vec:    [ZERO_VEC; MAX_NODES],
            edge_live:   [false; MAX_EDGES],
            edge_from:   [ZERO_ID;  MAX_EDGES],
            edge_to:     [ZERO_ID;  MAX_EDGES],
            edge_weight: [1.0f32; MAX_EDGES],
        };
        for i in 0..MAX_NODES {
            if let Some(r) = self.nodes[i] {
                snap.node_slots[snap.node_count] = i;
                snap.node_count += 1;
                snap.slot_live[i] = true;
                snap.slot_id[i]   = r.spec.node_id;
                snap.slot_vec[i]  = r.vector;
            }
        }
        for i in 0..MAX_EDGES {
            if let Some(e) = self.edges[i] {
                snap.edge_live[i]   = true;
                snap.edge_from[i]   = e.spec.from_node;
                snap.edge_to[i]     = e.spec.to_node;
                snap.edge_weight[i] = e.spec.weight;
            }
        }
        snap
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

    /// V2.55: Store a u32 attribute on a node (palette color, flag, scalar).
    /// Idempotent: re-registering the same NodeId overwrites the val.
    /// Returns false when the table is full (MAX_NODE_PROPS_U32 slots).
    pub fn register_node_prop_u32(&mut self, node_id: NodeId, val: u32) -> bool {
        for slot in self.node_props_u32.iter_mut() {
            if slot.0 == node_id {
                slot.1 = val;
                return true;
            }
        }
        for slot in self.node_props_u32.iter_mut() {
            if slot.0 == NodeId::ZERO {
                *slot = (node_id, val);
                return true;
            }
        }
        false
    }

    /// V2.55: Retrieve the u32 attribute stored for `node_id`, or None if absent.
    pub fn node_prop_u32(&self, node_id: NodeId) -> Option<u32> {
        self.node_props_u32.iter().find_map(|&(id, val)| {
            if id == node_id && id != NodeId::ZERO { Some(val) } else { None }
        })
    }

    /// V2.55: Set a u32 attribute on the node at `vector`.
    /// Returns Err(NodeNotFound) if no node is registered at that vector.
    /// Returns Err(PropTableFull) if the attribute table is full.
    pub fn node_attr_set_inner(&mut self, vector: VectorAddress, val: u32) -> Result<(), RuntimeError> {
        let node_id = self.node_id_for_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
        if self.register_node_prop_u32(node_id, val) {
            Ok(())
        } else {
            Err(RuntimeError::PropTableFull)
        }
    }

    /// V2.55: Get the u32 attribute stored on the node at `vector`, or None.
    pub fn node_attr_get_inner(&self, vector: VectorAddress) -> Option<u32> {
        let node_id = self.node_id_for_vec(vector)?;
        self.node_prop_u32(node_id)
    }

    /// V2.58: List all nodes that have a u32 attribute set.
    /// Fills `out_vec` / `out_val` in table order, skipping free (ZERO) slots.
    /// Returns the number of entries written (≤ N).
    pub fn node_attr_list_inner<const N: usize>(
        &self,
        out_vec: &mut [VectorAddress; N],
        out_val: &mut [u32; N],
    ) -> usize {
        let mut count = 0usize;
        for &(node_id, val) in self.node_props_u32.iter() {
            if node_id == NodeId::ZERO { continue; }
            if count >= N { break; }
            out_vec[count] = self.node_vector(node_id).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_val[count] = val;
            count += 1;
        }
        count
    }

    /// V2.61: Global graph clustering coefficient (Watts-Strogatz style).
    /// For each node v with >= 2 undirected neighbors, counts edge-pairs among
    /// those neighbors (treating directed edges as undirected).
    /// Returns (clustering_ppm, total_node_count).
    /// clustering_ppm = total_triangle_pairs * 1_000_000 / total_pair_triplets.
    /// Returns (0, n) when no node has >= 2 neighbors (metric undefined).
    pub fn graph_clustering_inner(&self) -> (u32, usize) {
        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        let mut total_triangles: u64 = 0;
        let mut total_triplets: u64 = 0;

        for slot in 0..MAX_NODES {
            let record = match self.nodes[slot] {
                Some(ref r) => r,
                None => continue,
            };
            let vid = record.spec.node_id;

            // Collect undirected neighbors (deduplicated, self excluded).
            let mut neighbors = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !neighbors[..nb].contains(&other) {
                    neighbors[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }

            if nb < 2 { continue; }

            let k = nb as u64;
            total_triplets += k * (k - 1) / 2;

            // Count edges among neighbors (undirected), one per unordered pair.
            for i in 0..nb {
                for j in (i + 1)..nb {
                    let b = neighbors[i];
                    let c = neighbors[j];
                    let connected = self.edges.iter().flatten().any(|e| {
                        (e.spec.from_node == b && e.spec.to_node == c)
                            || (e.spec.from_node == c && e.spec.to_node == b)
                    });
                    if connected {
                        total_triangles += 1;
                    }
                }
            }
        }

        if total_triplets == 0 {
            return (0, n);
        }
        let ppm = ((total_triangles * 1_000_000) / total_triplets).min(1_000_000) as u32;
        (ppm, n)
    }

    /// V2.63: Global graph transitivity — 3 × triangles / open_triplets.
    ///
    /// Differs from graph_clustering (V2.61, Watts-Strogatz): that averages
    /// per-node local CCs; this computes a single global ratio of closed
    /// to total triplets, giving more weight to high-degree nodes.
    ///
    /// Returns (transitivity_ppm, triangle_count, triplet_count, node_count).
    /// transitivity_ppm = total_triangles * 1_000_000 / total_triplets.
    /// Returns (0, 0, 0, n) when no node has >= 2 neighbors.
    pub fn graph_transitivity_inner(&self) -> (u32, u64, u64, usize) {
        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        let mut total_triangles: u64 = 0;
        let mut total_triplets: u64 = 0;

        for slot in 0..MAX_NODES {
            let record = match self.nodes[slot] {
                Some(ref r) => r,
                None => continue,
            };
            let vid = record.spec.node_id;

            let mut neighbors = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !neighbors[..nb].contains(&other) {
                    neighbors[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }

            if nb < 2 { continue; }

            let k = nb as u64;
            total_triplets += k * (k - 1) / 2;

            for i in 0..nb {
                for j in (i + 1)..nb {
                    let b = neighbors[i];
                    let c = neighbors[j];
                    let connected = self.edges.iter().flatten().any(|e| {
                        (e.spec.from_node == b && e.spec.to_node == c)
                            || (e.spec.from_node == c && e.spec.to_node == b)
                    });
                    if connected {
                        total_triangles += 1;
                    }
                }
            }
        }

        if total_triplets == 0 {
            return (0, 0, 0, n);
        }
        let ppm = ((total_triangles * 1_000_000) / total_triplets).min(1_000_000) as u32;
        (ppm, total_triangles, total_triplets, n)
    }

    /// V2.64: Graph k-core decomposition (Batagelj-Zaversnik peeling).
    ///
    /// Computes the coreness of each live node: the largest k such that the
    /// node belongs to the k-core (the maximal subgraph where every node has
    /// undirected degree ≥ k).  Peels iteratively for k = 1, 2, … removing
    /// nodes whose current effective degree < k and updating neighbours.
    ///
    /// Returns (vecs, coreness, n, max_coreness):
    ///   vecs[0..n]     — nodes sorted by coreness descending
    ///   coreness[0..n] — coreness value (0 = isolated, not in any 1-core)
    ///   n              — total live node count
    ///   max_coreness   — graph degeneracy (highest coreness)
    pub fn graph_kcore_inner<const N: usize>(&self) -> ([VectorAddress; N], [u8; N], usize, u8) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);

        // Compact slot list.
        let mut node_slots = [0usize; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[nc] = i;
                nc += 1;
            }
        }
        if nc == 0 {
            return ([ZERO_VEC; N], [0u8; N], 0, 0);
        }

        // Compute initial undirected effective degree (deduped, no self-loops).
        let mut eff_deg = [0u8; MAX_NODES];
        for ki in 0..nc {
            let slot = node_slots[ki];
            let vid  = self.nodes[slot].as_ref().unwrap().spec.node_id;
            let mut seen = [NodeId::ZERO; MAX_NODES];
            let mut nb   = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !seen[..nb].contains(&other) {
                    seen[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }
            eff_deg[slot] = nb.min(255) as u8;
        }

        // Determine upper bound for k.
        let mut max_deg = 0u8;
        for ki in 0..nc {
            let d = eff_deg[node_slots[ki]];
            if d > max_deg { max_deg = d; }
        }

        // Peeling phase.
        let mut coreness = [0u8; MAX_NODES];
        let mut removed  = [false; MAX_NODES];
        let mut remaining = nc;

        let mut k: u8 = 1;
        while k <= max_deg && remaining > 0 {
            let mut changed = true;
            while changed {
                changed = false;
                for ki in 0..nc {
                    let slot = node_slots[ki];
                    if removed[slot] { continue; }
                    if eff_deg[slot] < k {
                        coreness[slot]  = k.saturating_sub(1);
                        removed[slot]   = true;
                        remaining      -= 1;
                        changed         = true;
                        // Decrement each distinct non-removed neighbour's degree.
                        let vid = self.nodes[slot].as_ref().unwrap().spec.node_id;
                        let mut seen_u = [NodeId::ZERO; MAX_NODES];
                        let mut nb_u   = 0usize;
                        for edge in self.edges.iter().flatten() {
                            let other = if edge.spec.from_node == vid {
                                edge.spec.to_node
                            } else if edge.spec.to_node == vid {
                                edge.spec.from_node
                            } else {
                                continue;
                            };
                            if other == vid { continue; }
                            if seen_u[..nb_u].contains(&other) { continue; }
                            if nb_u < MAX_NODES {
                                seen_u[nb_u] = other;
                                nb_u += 1;
                            }
                            if let Some(ns) = self.node_slot_by_id(other) {
                                if !removed[ns] && eff_deg[ns] > 0 {
                                    eff_deg[ns] -= 1;
                                }
                            }
                        }
                    }
                }
            }
            k = k.saturating_add(1);
        }

        // Nodes that survived all rounds get coreness = k - 1.
        let final_k = k.saturating_sub(1);
        for ki in 0..nc {
            let slot = node_slots[ki];
            if !removed[slot] {
                coreness[slot] = final_k;
            }
        }

        // Sort by coreness descending (insertion sort on slots).
        let mut sorted = node_slots;
        for i in 1..nc {
            let key_slot = sorted[i];
            let key_core = coreness[key_slot];
            let mut j    = i;
            while j > 0 && coreness[sorted[j - 1]] < key_core {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        // Pack output.
        let copy_len = nc.min(N);
        let mut out_vecs = [ZERO_VEC; N];
        let mut out_core = [0u8; N];
        for i in 0..copy_len {
            let slot     = sorted[i];
            out_vecs[i]  = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO_VEC);
            out_core[i]  = coreness[slot];
        }
        let max_coreness = if copy_len > 0 { out_core[0] } else { 0 };
        (out_vecs, out_core, nc, max_coreness)
    }

    /// V2.65: Degree assortativity coefficient (Newman 2002).
    ///
    /// Measures the tendency of nodes to connect to other nodes of similar degree.
    /// Uses each stored directed edge (u→v) once; degree = undirected neighbor count
    /// (same definition as used by graph_clustering and graph_transitivity).
    ///
    /// Formula (integer arithmetic, no float):
    ///   M  = directed edge count
    ///   For each edge (u,v): j = undirected_deg(u), k = undirected_deg(v)
    ///     S1 += j*k
    ///     T  += j+k
    ///     Q  += j²+k²
    ///   Numerator   = 4·M·S1 − T²
    ///   Denominator = 2·M·Q  − T²
    ///   r_ppm = Numerator·1_000_000 / Denominator   (clamped to [−1e6, +1e6])
    ///
    /// Returns (assortativity_ppm, edge_count, node_count).
    ///   +1_000_000 → perfectly assortative (hubs link to hubs)
    ///   −1_000_000 → perfectly disassortative (hubs link to leaves)
    ///        0     → uncorrelated or undefined (no edges / all same degree)
    pub fn graph_assortativity_inner(&self) -> (i32, usize, usize) {
        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        let m = self.edges.iter().filter(|s| s.is_some()).count();
        if m == 0 {
            return (0, 0, n);
        }

        // Pre-compute undirected degree for each node slot.
        let mut deg = [0u32; MAX_NODES];
        for slot in 0..MAX_NODES {
            let record = match self.nodes[slot] {
                Some(ref r) => r,
                None => continue,
            };
            let vid = record.spec.node_id;
            let mut seen = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !seen[..nb].contains(&other) {
                    seen[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }
            deg[slot] = nb as u32;
        }

        // Compute Newman sums over stored directed edges.
        let mut s1: i64 = 0;
        let mut t:  i64 = 0;
        let mut q:  i64 = 0;
        for edge in self.edges.iter().flatten() {
            let u = edge.spec.from_node;
            let v = edge.spec.to_node;
            if u == v { continue; }
            let j = match self.node_slot_by_id(u) { Some(s) => deg[s] as i64, None => continue };
            let k = match self.node_slot_by_id(v) { Some(s) => deg[s] as i64, None => continue };
            s1 += j * k;
            t  += j + k;
            q  += j * j + k * k;
        }

        let m_i = m as i64;
        let numer = 4 * m_i * s1 - t * t;
        let denom = 2 * m_i * q  - t * t;
        if denom == 0 {
            return (0, m, n);
        }
        let r_ppm = ((numer * 1_000_000) / denom)
            .max(-1_000_000)
            .min(1_000_000) as i32;
        (r_ppm, m, n)
    }

    /// V2.66: Graph reciprocity — fraction of directed edges that are mutual.
    ///
    /// For each directed edge (u→v), checks whether the reverse edge (v→u) also
    /// exists. Self-loops are excluded from both counts.
    ///
    /// reciprocity_ppm = mutual_edges / total_edges × 1_000_000
    ///   mutual_edges  = count of directed edges (u,v) where (v,u) also exists
    ///   total_edges   = directed edge count (self-loops excluded)
    ///
    /// Returns (reciprocity_ppm, mutual_edges, total_edges).
    ///   1_000_000 → fully reciprocal (all edges bidirectional)
    ///       0     → no mutual edges, or no edges at all
    pub fn graph_reciprocity_inner(&self) -> (u32, usize, usize) {
        // Collect (from, to) for every non-self-loop edge.
        let mut from_ids = [NodeId::ZERO; MAX_EDGES];
        let mut to_ids   = [NodeId::ZERO; MAX_EDGES];
        let mut m = 0usize;
        for edge in self.edges.iter().flatten() {
            let u = edge.spec.from_node;
            let v = edge.spec.to_node;
            if u == v { continue; }
            if m < MAX_EDGES {
                from_ids[m] = u;
                to_ids[m]   = v;
                m += 1;
            }
        }
        if m == 0 {
            return (0, 0, 0);
        }
        // For each edge (u,v) check if (v,u) also exists.
        let mut mutual = 0usize;
        for i in 0..m {
            let u = from_ids[i];
            let v = to_ids[i];
            for j in 0..m {
                if from_ids[j] == v && to_ids[j] == u {
                    mutual += 1;
                    break;
                }
            }
        }
        let reciprocity_ppm = ((mutual as u64 * 1_000_000) / m as u64) as u32;
        (reciprocity_ppm, mutual, m)
    }

    /// V2.67: Graph modularity — quality of the LPA community partition.
    ///
    /// Runs the same Label Propagation Algorithm as `graph_community` to detect
    /// communities, then evaluates Newman–Girvan modularity Q over that partition.
    ///
    ///   Q = Σ_c [ L_c / m  −  (d_c / (2m))² ]
    ///
    /// where m = undirected edge count (directed pair counted once), L_c = undirected
    /// edges with both endpoints in community c, d_c = Σ degree of nodes in c.
    /// Directed edges are treated as undirected (consistent with LPA). Self-loops excluded.
    ///
    /// Integer arithmetic (avoids float, no_std safe):
    ///   Q_ppm = (4m·ΣL_c − Σd_c²) × 1_000_000 / (4m²)
    ///
    /// Returns (modularity_ppm, community_count, undirected_edge_count, node_count).
    ///   1_000_000 → hypothetically perfect partition; 0 → single community or no edges.
    fn graph_modularity_inner(snap: &GraphTopologySnapshot) -> (i32, usize, usize, usize) {
        const ITERS: usize = 20;

        let node_count = snap.node_count;
        if node_count == 0 {
            return (0, 0, 0, 0);
        }

        // ── 1. LPA (identical to graph_community_inner, without final remapping) ──
        let mut label = [0u8; MAX_NODES];
        for si in 0..node_count {
            label[snap.node_slots[si]] = snap.node_slots[si] as u8;
        }

        for _iter in 0..ITERS {
            for si in 0..node_count {
                let v    = snap.node_slots[si];
                let v_id = snap.slot_id[v];

                let mut freq = [0u8; MAX_NODES];
                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    let nb_id = if snap.edge_from[ei] == v_id {
                        snap.edge_to[ei]
                    } else if snap.edge_to[ei] == v_id {
                        snap.edge_from[ei]
                    } else {
                        continue;
                    };
                    if nb_id == v_id { continue; }
                    if let Some(nb) = snap.node_slot_by_id(nb_id) {
                        let l = label[nb] as usize;
                        if l < MAX_NODES { freq[l] = freq[l].saturating_add(1); }
                    }
                }

                let mut best_l    = MAX_NODES;
                let mut best_freq = 0u8;
                for l in 0..MAX_NODES {
                    if freq[l] == 0 { continue; }
                    if freq[l] > best_freq
                        || (freq[l] == best_freq && (best_l >= MAX_NODES || l < best_l))
                    {
                        best_freq = freq[l];
                        best_l    = l;
                    }
                }
                if best_l < MAX_NODES {
                    label[v] = best_l as u8;
                }
            }
        }

        // ── 2. Count distinct communities ────────────────────────────────────────
        let mut comm_present = [false; MAX_NODES];
        for si in 0..node_count {
            let l = label[snap.node_slots[si]] as usize;
            if l < MAX_NODES { comm_present[l] = true; }
        }
        let comm_count = comm_present.iter().filter(|&&p| p).count();

        // ── 3. Deduplicate directed edges → undirected edge set, compute m ────────
        // Record (from, to); when we encounter (v, u) and (u, v) already recorded
        // it is the same undirected edge — skip it.
        let mut seen_from = [NodeId::ZERO; MAX_EDGES];
        let mut seen_to   = [NodeId::ZERO; MAX_EDGES];
        let mut m = 0usize;

        for ei in 0..MAX_EDGES {
            if !snap.edge_live[ei] { continue; }
            let u = snap.edge_from[ei];
            let v = snap.edge_to[ei];
            if u == v { continue; }
            let already = (0..m).any(|j| seen_from[j] == v && seen_to[j] == u);
            if !already && m < MAX_EDGES {
                seen_from[m] = u;
                seen_to[m]   = v;
                m += 1;
            }
        }

        if m == 0 {
            return (0, comm_count, 0, node_count);
        }

        // ── 4. Undirected degree per slot ─────────────────────────────────────────
        let mut deg = [0u32; MAX_NODES];
        for si in 0..node_count {
            let v    = snap.node_slots[si];
            let v_id = snap.slot_id[v];
            let mut nb_seen = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for ei in 0..MAX_EDGES {
                if !snap.edge_live[ei] { continue; }
                let other = if snap.edge_from[ei] == v_id {
                    snap.edge_to[ei]
                } else if snap.edge_to[ei] == v_id {
                    snap.edge_from[ei]
                } else {
                    continue;
                };
                if other == v_id { continue; }
                if !nb_seen[..nb].contains(&other) && nb < MAX_NODES {
                    nb_seen[nb] = other;
                    nb += 1;
                }
            }
            deg[v] = nb as u32;
        }

        // ── 5. ΣL_c: count intra-community undirected edges ──────────────────────
        let mut sum_l = 0i64;
        for ei in 0..m {
            let u = seen_from[ei];
            let v = seen_to[ei];
            if let (Some(su), Some(sv)) = (snap.node_slot_by_id(u), snap.node_slot_by_id(v)) {
                if label[su] == label[sv] {
                    sum_l += 1;
                }
            }
        }

        // ── 6. Σd_c²: per-community degree sum, then squared ─────────────────────
        let mut dc = [0i64; MAX_NODES];
        for si in 0..node_count {
            let slot = snap.node_slots[si];
            let l    = label[slot] as usize;
            if l < MAX_NODES { dc[l] += deg[slot] as i64; }
        }
        let mut sum_d2 = 0i64;
        for l in 0..MAX_NODES { sum_d2 += dc[l] * dc[l]; }

        // ── 7. Q_ppm = (4m·ΣL − Σd²) × 1_000_000 / (4m²) ──────────────────────
        let m_i   = m as i64;
        let numer = (4 * m_i * sum_l - sum_d2) * 1_000_000;
        let denom = 4 * m_i * m_i;
        let q_ppm = (numer / denom).max(-1_000_000).min(1_000_000) as i32;

        (q_ppm, comm_count, m, node_count)
    }

    /// V2.68: Graph rich-club coefficient for degree threshold `k`.
    ///
    /// The rich-club coefficient ρ(k) measures how densely the "rich" nodes
    /// (those with undirected degree > k) are connected to each other:
    ///
    ///   ρ(k) = E_{>k} / [N_{>k} × (N_{>k} − 1) / 2]
    ///
    /// where N_{>k} = |{v : deg(v) > k}|  and  E_{>k} = undirected edges
    /// with both endpoints in that set.  Directed edges are treated as
    /// undirected (same convention as modularity/assortativity). Self-loops
    /// are excluded.
    ///
    /// Integer arithmetic (no_std safe):
    ///   ρ_ppm(k) = E_{>k} × 2_000_000 / (N_{>k} × (N_{>k} − 1))
    ///
    /// Returns (rich_club_ppm, rich_node_count, edges_among_rich).
    ///   1_000_000 → all rich nodes form a clique.
    ///   0         → no rich nodes or no edges among them.
    fn graph_rich_club_inner(snap: &GraphTopologySnapshot, k: u8) -> (u32, usize, usize) {
        let node_count = snap.node_count;
        if node_count == 0 {
            return (0, 0, 0);
        }

        // ── 1. Undirected degree per slot (neighbour deduplication) ───────────────
        let mut deg = [0u32; MAX_NODES];
        for si in 0..node_count {
            let v    = snap.node_slots[si];
            let v_id = snap.slot_id[v];
            let mut nb_seen = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for ei in 0..MAX_EDGES {
                if !snap.edge_live[ei] { continue; }
                let other = if snap.edge_from[ei] == v_id {
                    snap.edge_to[ei]
                } else if snap.edge_to[ei] == v_id {
                    snap.edge_from[ei]
                } else {
                    continue;
                };
                if other == v_id { continue; }
                if !nb_seen[..nb].contains(&other) && nb < MAX_NODES {
                    nb_seen[nb] = other;
                    nb += 1;
                }
            }
            deg[v] = nb as u32;
        }

        // ── 2. Collect "rich" node slots (degree > k) ─────────────────────────────
        let mut rich_slots = [0usize; MAX_NODES];
        let mut rich_ids   = [NodeId::ZERO; MAX_NODES];
        let mut n_rich = 0usize;
        for si in 0..node_count {
            let v = snap.node_slots[si];
            if deg[v] > k as u32 && n_rich < MAX_NODES {
                rich_slots[n_rich] = v;
                rich_ids[n_rich]   = snap.slot_id[v];
                n_rich += 1;
            }
        }

        if n_rich < 2 {
            return (0, n_rich, 0);
        }

        // ── 3. Deduplicate directed edges → undirected; keep only rich–rich ───────
        let mut seen_from = [NodeId::ZERO; MAX_EDGES];
        let mut seen_to   = [NodeId::ZERO; MAX_EDGES];
        let mut e_rich = 0usize;

        for ei in 0..MAX_EDGES {
            if !snap.edge_live[ei] { continue; }
            let u = snap.edge_from[ei];
            let v = snap.edge_to[ei];
            if u == v { continue; }
            // Both endpoints must be "rich".
            let u_rich = rich_ids[..n_rich].contains(&u);
            let v_rich = rich_ids[..n_rich].contains(&v);
            if !u_rich || !v_rich { continue; }
            // Dedup: skip if reverse direction already seen.
            let already = (0..e_rich).any(|j| seen_from[j] == v && seen_to[j] == u);
            if !already && e_rich < MAX_EDGES {
                seen_from[e_rich] = u;
                seen_to[e_rich]   = v;
                e_rich += 1;
            }
        }

        // ── 4. ρ_ppm = E_{>k} × 2_000_000 / (N_{>k} × (N_{>k} − 1)) ─────────────
        let denom = (n_rich as u64) * ((n_rich as u64) - 1);
        let rho_ppm = ((e_rich as u64) * 2_000_000 / denom) as u32;

        (rho_ppm, n_rich, e_rich)
    }

    /// V2.69: Graph girth — length of the shortest directed cycle.
    ///
    /// Runs BFS from every live node as source; detects the shortest directed
    /// cycle by watching for out-edges that return to the source node.
    ///
    ///   girth = 1       → self-loop (node with an edge to itself)
    ///   girth = 2       → mutual pair A↔B (two opposing directed edges)
    ///   girth = k       → shortest directed cycle traverses k edges
    ///   girth = u32::MAX → no directed cycle (acyclic / DAG)
    ///
    /// Returns `(girth, is_acyclic, node_count)`.
    ///   is_acyclic = girth == u32::MAX.
    /// Directed edges only; self-loops are counted.
    /// O(V × (V + E)) overall — acceptable for V ≤ 128, E ≤ 512.
    pub fn graph_girth_inner(&self) -> (u32, bool, usize) {
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        let mut min_girth: u32 = u32::MAX;

        for si in 0..node_count {
            if min_girth == 1 { break; } // girth cannot be shorter than 1

            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };

            // BFS distance table; u32::MAX = unvisited.
            let mut dist = [u32::MAX; MAX_NODES];
            dist[s] = 0;

            let mut queue = [0usize; MAX_NODES];
            let mut q_head = 0usize;
            let mut q_tail = 0usize;
            queue[q_tail] = s;
            q_tail += 1;

            while q_head < q_tail {
                let cur = queue[q_head];
                q_head += 1;
                let cur_dist = dist[cur];

                // Prune: a cycle found from this frontier can't be shorter.
                if cur_dist + 1 >= min_girth { continue; }

                let cur_id = match self.nodes[cur] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => continue };
                    if edge.spec.from_node != cur_id { continue; }

                    let nbr_id = edge.spec.to_node;

                    // Back-edge to source → directed cycle found.
                    if nbr_id == s_id {
                        let cycle = cur_dist + 1;
                        if cycle < min_girth { min_girth = cycle; }
                        continue;
                    }

                    let nbr_slot = match self.node_slot_by_id(nbr_id) {
                        Some(sl) => sl,
                        None     => continue,
                    };

                    if dist[nbr_slot] == u32::MAX && cur_dist + 1 < min_girth {
                        dist[nbr_slot] = cur_dist + 1;
                        if q_tail < MAX_NODES {
                            queue[q_tail] = nbr_slot;
                            q_tail += 1;
                        }
                    }
                }
            }
        }

        let is_acyclic = min_girth == u32::MAX;
        (min_girth, is_acyclic, node_count)
    }

    /// V2.70: Wiener index — sum of all pairwise directed shortest-path distances.
    ///
    /// W(G) = Σ_{u≠v, path exists} d(u,v)  (directed, unweighted BFS).
    ///
    /// Returns (wiener_index, reachable_pairs, node_count).
    ///   wiener_index    = total sum of finite pairwise distances
    ///   reachable_pairs = count of ordered (u,v) pairs with u≠v and d(u,v)<∞
    ///   node_count      = live nodes
    pub fn graph_wiener_inner(&self) -> (u64, usize, usize) {
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        let mut wiener_index: u64 = 0;
        let mut reachable_pairs: usize = 0;

        for si in 0..node_count {
            let s = node_slots[si];
            if self.nodes[s].is_none() { continue; }

            let mut dist = [u32::MAX; MAX_NODES];
            dist[s] = 0;

            let mut queue = [0usize; MAX_NODES];
            let mut q_head = 0usize;
            let mut q_tail = 0usize;
            queue[q_tail] = s;
            q_tail += 1;

            while q_head < q_tail {
                let cur = queue[q_head];
                q_head += 1;
                let cur_dist = dist[cur];

                let cur_id = match self.nodes[cur] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => continue };
                    if edge.spec.from_node != cur_id { continue; }

                    let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(sl) => sl,
                        None     => continue,
                    };
                    if dist[nbr_slot] == u32::MAX {
                        dist[nbr_slot] = cur_dist + 1;
                        if q_tail < MAX_NODES {
                            queue[q_tail] = nbr_slot;
                            q_tail += 1;
                        }
                    }
                }
            }

            for ti in 0..node_count {
                let t = node_slots[ti];
                if t == s { continue; }
                if dist[t] != u32::MAX {
                    wiener_index += dist[t] as u64;
                    reachable_pairs += 1;
                }
            }
        }

        (wiener_index, reachable_pairs, node_count)
    }

    /// V2.60: List all nodes that have a u8 attribute set.
    /// Fills `out_vec` / `out_val` in table order, skipping free (ZERO) slots.
    /// Returns the number of entries written (≤ N).
    pub fn node_attr_list_u8_inner<const N: usize>(
        &self,
        out_vec: &mut [VectorAddress; N],
        out_val: &mut [u8; N],
    ) -> usize {
        let mut count = 0usize;
        for &(node_id, val) in self.node_props_u8.iter() {
            if node_id == NodeId::ZERO { continue; }
            if count >= N { break; }
            out_vec[count] = self.node_vector(node_id).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_val[count] = val;
            count += 1;
        }
        count
    }

    /// V2.59: Graph density = E / (N*(N-1)) for a directed graph.
    /// Returns (density_ppm, node_count, edge_count) where density_ppm is
    /// the density expressed in parts-per-million (multiply by 1e-6 for 0..1).
    /// Returns (0, n, e) for graphs with fewer than 2 nodes (density undefined).
    pub fn graph_density_inner(&self) -> (u32, usize, usize) {
        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        let e = self.edges.iter().filter(|s| s.is_some()).count();
        if n < 2 {
            return (0, n, e);
        }
        let max_edges = (n as u64) * (n as u64 - 1); // directed: N*(N-1)
        let density_ppm = ((e as u64 * 1_000_000) / max_edges).min(1_000_000) as u32;
        (density_ppm, n, e)
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

    /// V2.51: Snapshot the current node state into the structural diff ring as a
    /// `GraphDiffKind::NodeCheckpoint` entry.  Analogous to `perf record -e cycles`
    /// mark — captures the node's vector, key, signal_count, lifecycle, and
    /// edge_out_count into the diff ring without modifying graph structure.
    /// Returns a `NodeProcSummary` of the captured state, or `NodeNotFound`.
    pub fn node_checkpoint_inner(&mut self, vector: VectorAddress) -> Result<NodeProcSummary, RuntimeError> {
        let summary = self.proc_stat_for_vector(vector).ok_or(RuntimeError::NodeNotFound)?;
        self.push_diff(GraphDiffKind::NodeCheckpoint, vector, VectorAddress::new(0, 0, 0, 0), summary.local_node_key.as_bytes());
        Ok(summary)
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
            let mut sigma   = [0u64;     MAX_NODES]; // # shortest paths from s to v
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

                    // δ[v] += σ[v] × (SCALE + δ[w]) / σ[w]  (multiply before divide)
                    let contribution = sigma[v]
                        .saturating_mul(SCALE.saturating_add(delta[w]))
                        / sigma[w];
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

    /// V2.40: Closeness centrality for each live node (directed, outgoing BFS).
    ///
    /// For a directed graph, the outgoing closeness centrality of node v is:
    ///   CC[v] = r_v / Σ_{u reachable from v, u≠v} d(v,u)
    /// where r_v is the number of nodes reachable from v (excluding v itself)
    /// and d(v,u) is the shortest-path distance from v to u via directed edges.
    ///
    /// Disconnected nodes (r_v = 0) get CC[v] = 0.
    /// Fixed-point scaling (SCALE = 1_000_000) avoids floating-point arithmetic.
    /// Output `cc[i]` = (r_v * SCALE) / Σ d(v,u), integer-truncated.
    ///
    /// Returns `(vecs, cc, total)`:
    ///   vecs[0..total] — live node vectors, descending closeness order.
    ///   cc[0..total]   — closeness score × SCALE per node.
    ///   total          — number of live nodes packed into the output arrays.
    ///
    /// Algorithm: one BFS per source node, O(V × (V+E)).
    /// OS analogy: `ping` round-trip times — which kernel service node can reach
    /// all others in the fewest directed hops on average?
    pub fn graph_closeness_inner<const N: usize>(&self) -> ([VectorAddress; N], [u32; N], usize) {
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

        // Per-slot closeness accumulator (scaled).
        let mut cc_scaled = [0u64; MAX_NODES];

        // One BFS per source node following outgoing edges.
        for si in 0..node_count {
            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };

            let mut dist  = [u32::MAX; MAX_NODES];
            let mut queue = [0usize;   MAX_NODES];

            dist[s]  = 0;
            queue[0] = s;
            let mut q_head = 0usize;
            let mut q_tail = 1usize;

            while q_head < q_tail {
                let v = queue[q_head];
                q_head += 1;

                let v_id = match self.nodes[v] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };
                let _ = s_id; // used via BFS below

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
                    if dist[w] == u32::MAX {
                        dist[w] = dist[v].saturating_add(1);
                        if q_tail < MAX_NODES {
                            queue[q_tail] = w;
                            q_tail += 1;
                        }
                    }
                }
            }

            // Sum distances from s to all reachable nodes (excluding s itself).
            let mut dist_sum = 0u64;
            let mut reachable_count = 0u64;
            for ti in 0..node_count {
                let t = node_slots[ti];
                if t == s { continue; }
                if dist[t] != u32::MAX {
                    dist_sum = dist_sum.saturating_add(dist[t] as u64);
                    reachable_count += 1;
                }
            }

            // CC[s] = reachable_count * SCALE / dist_sum  (0 if isolated).
            cc_scaled[s] = if dist_sum == 0 {
                0
            } else {
                reachable_count.saturating_mul(SCALE) / dist_sum
            };
        }

        // Sort node_slots by descending closeness (insertion sort — N ≤ 128).
        let mut sorted = node_slots;
        for i in 1..node_count {
            let key_slot = sorted[i];
            let key_cc   = cc_scaled[key_slot];
            let mut j    = i;
            while j > 0 && cc_scaled[sorted[j - 1]] < key_cc {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        // Pack output arrays (cap at N).
        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_cc   = [0u32; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_cc[i]   = (cc_scaled[slot]).min(u32::MAX as u64) as u32;
        }

        (out_vecs, out_cc, copy_len)
    }

    /// V2.71: Harmonic centrality for all live nodes.
    ///
    /// HC[v] = Σ_{u≠v, d(v,u)<∞} 1_000_000/d(v,u)
    ///
    /// Unlike closeness centrality, harmonic centrality handles disconnected
    /// graphs naturally — unreachable pairs contribute 0 to the sum.
    /// Returns sorted descending by HC.  Algorithm: one BFS per source, O(V × (V+E)).
    pub fn graph_harmonic_inner<const N: usize>(&self) -> ([VectorAddress; N], [u32; N], usize) {
        const SCALE: u64 = 1_000_000;

        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        let mut hc = [0u64; MAX_NODES];

        for si in 0..node_count {
            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };

            let mut dist  = [u32::MAX; MAX_NODES];
            let mut queue = [0usize;   MAX_NODES];

            dist[s]  = 0;
            queue[0] = s;
            let mut q_head = 0usize;
            let mut q_tail = 1usize;

            while q_head < q_tail {
                let v = queue[q_head];
                q_head += 1;

                let v_id = match self.nodes[v] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };
                let _ = s_id;

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
                    if dist[w] == u32::MAX {
                        dist[w] = dist[v].saturating_add(1);
                        if q_tail < MAX_NODES {
                            queue[q_tail] = w;
                            q_tail += 1;
                        }
                    }
                }
            }

            // HC[s] += 1_000_000/d(s,u) for each reachable u ≠ s (d ≥ 1, no div-by-zero).
            for ti in 0..node_count {
                let t = node_slots[ti];
                if t == s { continue; }
                if dist[t] != u32::MAX {
                    hc[s] = hc[s].saturating_add(SCALE / dist[t] as u64);
                }
            }
        }

        // Sort descending by harmonic centrality (insertion sort, N ≤ 128).
        let mut sorted = node_slots;
        for i in 1..node_count {
            let key_slot = sorted[i];
            let key_hc   = hc[key_slot];
            let mut j    = i;
            while j > 0 && hc[sorted[j - 1]] < key_hc {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_hc   = [0u32; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_hc[i]   = (hc[slot]).min(u32::MAX as u64) as u32;
        }

        (out_vecs, out_hc, copy_len)
    }

    /// V2.72: Graph peripheral nodes — nodes whose eccentricity equals the diameter.
    ///
    /// A peripheral node is one that lies farthest from some other node in the graph;
    /// formally, ecc[v] = diameter.  The set of peripheral nodes is the "boundary" of
    /// the graph — the structural opposite of the centre.
    ///
    /// Returns `(vecs, ecc, peripheral_count, node_count, diameter)`:
    ///   vecs[0..peripheral_count]  — peripheral node vectors (sorted ascending by address)
    ///   ecc[0..peripheral_count]   — eccentricity values (all equal to diameter)
    ///   peripheral_count           — number of peripheral nodes (capped at N)
    ///   node_count                 — total number of live nodes
    ///   diameter                   — max eccentricity (0 if all isolated)
    ///
    /// Edge cases:
    ///   • All-isolated graph → diameter=0, peripheral_count=0.
    ///   • All nodes identical ecc → radius == diameter, every node is both centre and peripheral.
    ///   • Disconnected graph: ecc uses reachable-only BFS; isolated component sinks have ecc=0,
    ///     so they never qualify as peripheral unless diameter happens to be 0 (which requires
    ///     all nodes to be isolated, handled above).
    ///
    /// Algorithm: one BFS per source node, O(V × (V+E)).
    /// OS analogy: `traceroute` max-hop nodes — which kernel services lie at the extreme
    /// boundary of the reachable graph?
    pub fn graph_peripheral_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u32; N], usize, usize, u32) {
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        let mut ecc = [0u32; MAX_NODES];

        for si in 0..node_count {
            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };
            let _ = s_id;

            let mut dist  = [u32::MAX; MAX_NODES];
            let mut queue = [0usize;   MAX_NODES];

            dist[s]  = 0;
            queue[0] = s;
            let mut q_head = 0usize;
            let mut q_tail = 1usize;

            while q_head < q_tail {
                let v = queue[q_head];
                q_head += 1;

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
                    if dist[w] == u32::MAX {
                        dist[w] = dist[v].saturating_add(1);
                        if q_tail < MAX_NODES {
                            queue[q_tail] = w;
                            q_tail += 1;
                        }
                    }
                }
            }

            let mut max_d = 0u32;
            for ti in 0..node_count {
                let t = node_slots[ti];
                if t == s { continue; }
                if dist[t] != u32::MAX && dist[t] > max_d {
                    max_d = dist[t];
                }
            }
            ecc[s] = max_d;
        }

        // Diameter = max eccentricity across all nodes.
        let mut diameter: u32 = 0;
        for si in 0..node_count {
            let s = node_slots[si];
            if ecc[s] > diameter { diameter = ecc[s]; }
        }

        // Collect peripheral nodes: ecc[v] == diameter (diameter must be > 0).
        let mut periph_slots = [0usize; MAX_NODES];
        let mut periph_count = 0usize;
        for si in 0..node_count {
            let s = node_slots[si];
            if diameter > 0 && ecc[s] == diameter {
                periph_slots[periph_count] = s;
                periph_count += 1;
            }
        }

        // Insertion-sort peripheral nodes ascending by VectorAddress (via as_u64()).
        for i in 1..periph_count {
            let key = periph_slots[i];
            let key_addr = self.nodes[key]
                .map(|r| r.vector.as_u64())
                .unwrap_or(0);
            let mut j = i;
            while j > 0 {
                let prev = periph_slots[j - 1];
                let prev_addr = self.nodes[prev]
                    .map(|r| r.vector.as_u64())
                    .unwrap_or(0);
                if prev_addr <= key_addr { break; }
                periph_slots[j] = periph_slots[j - 1];
                j -= 1;
            }
            periph_slots[j] = key;
        }

        // Pack output arrays (cap at N).
        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_ecc  = [0u32; N];
        let copy_len     = periph_count.min(N);
        for i in 0..copy_len {
            let slot    = periph_slots[i];
            out_vecs[i] = self.nodes[slot]
                .map(|r| r.vector)
                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_ecc[i]  = ecc[slot];
        }

        (out_vecs, out_ecc, copy_len, node_count, diameter)
    }

    /// V2.73: Graph center nodes — nodes whose eccentricity equals the graph radius.
    ///
    /// The centre of a graph is the set of nodes with minimum eccentricity.
    /// Formally, v is a centre node iff ecc[v] == radius,
    /// where radius = min_{u: ecc[u] > 0} ecc[u].
    ///
    /// This is the structural complement of peripheral nodes (ecc == diameter).
    /// Centre nodes lie closest (in worst-case terms) to all reachable peers —
    /// the OS analogy is `sched_setaffinity` NUMA-optimal nodes, or the kernel
    /// service with the tightest worst-case latency bound to all reachable nodes.
    ///
    /// Returns `(vecs, ecc, center_count, node_count, radius)`:
    ///   vecs[0..center_count]  — centre node vectors (sorted ascending by address)
    ///   ecc[0..center_count]   — eccentricity values (all equal to radius)
    ///   center_count           — number of centre nodes (capped at N)
    ///   node_count             — total number of live nodes
    ///   radius                 — min nonzero eccentricity (0 if all isolated)
    ///
    /// Edge cases:
    ///   • All-isolated graph → radius=0, center_count=0.
    ///   • radius == diameter → every node is simultaneously centre and peripheral.
    ///   • Sinks (ecc=0) never qualify as centre (ecc=0 < any positive radius).
    ///
    /// Algorithm: one BFS per source node, O(V × (V+E)).
    pub fn graph_center_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u32; N], usize, usize, u32) {
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        let mut ecc = [0u32; MAX_NODES];

        for si in 0..node_count {
            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };
            let _ = s_id;

            let mut dist  = [u32::MAX; MAX_NODES];
            let mut queue = [0usize;   MAX_NODES];

            dist[s]  = 0;
            queue[0] = s;
            let mut q_head = 0usize;
            let mut q_tail = 1usize;

            while q_head < q_tail {
                let v = queue[q_head];
                q_head += 1;

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
                    if dist[w] == u32::MAX {
                        dist[w] = dist[v].saturating_add(1);
                        if q_tail < MAX_NODES {
                            queue[q_tail] = w;
                            q_tail += 1;
                        }
                    }
                }
            }

            let mut max_d = 0u32;
            for ti in 0..node_count {
                let t = node_slots[ti];
                if t == s { continue; }
                if dist[t] != u32::MAX && dist[t] > max_d {
                    max_d = dist[t];
                }
            }
            ecc[s] = max_d;
        }

        // Radius = min nonzero eccentricity (u32::MAX sentinel → 0 if all isolated).
        let mut radius: u32 = u32::MAX;
        for si in 0..node_count {
            let s = node_slots[si];
            if ecc[s] > 0 && ecc[s] < radius { radius = ecc[s]; }
        }
        if radius == u32::MAX { radius = 0; }

        // Collect centre nodes: ecc[v] == radius (radius must be > 0).
        let mut center_slots = [0usize; MAX_NODES];
        let mut center_count = 0usize;
        for si in 0..node_count {
            let s = node_slots[si];
            if radius > 0 && ecc[s] == radius {
                center_slots[center_count] = s;
                center_count += 1;
            }
        }

        // Insertion-sort centre nodes ascending by VectorAddress (via as_u64()).
        for i in 1..center_count {
            let key = center_slots[i];
            let key_addr = self.nodes[key]
                .map(|r| r.vector.as_u64())
                .unwrap_or(0);
            let mut j = i;
            while j > 0 {
                let prev = center_slots[j - 1];
                let prev_addr = self.nodes[prev]
                    .map(|r| r.vector.as_u64())
                    .unwrap_or(0);
                if prev_addr <= key_addr { break; }
                center_slots[j] = center_slots[j - 1];
                j -= 1;
            }
            center_slots[j] = key;
        }

        // Pack output arrays (cap at N).
        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_ecc  = [0u32; N];
        let copy_len     = center_count.min(N);
        for i in 0..copy_len {
            let slot    = center_slots[i];
            out_vecs[i] = self.nodes[slot]
                .map(|r| r.vector)
                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_ecc[i]  = ecc[slot];
        }

        (out_vecs, out_ecc, copy_len, node_count, radius)
    }

    /// V2.74: Global graph efficiency.
    ///
    /// E(G) = 1/(n*(n-1)) * Σ_{i≠j, d(i,j)<∞} 1/d(i,j)
    ///
    /// Quantifies how efficiently a network exchanges information on average,
    /// treating disconnected pairs as contributing 0 (infinite distance).
    /// Returns (efficiency_ppm, pairs_max, node_count):
    ///   efficiency_ppm = E(G) * 1_000_000  (0..=1_000_000)
    ///   pairs_max      = n*(n-1)  — the normalizer (0 when n < 2)
    ///   node_count     = live nodes
    /// Complete directed graph (all d=1) → E=1.0 (ppm=1_000_000).
    /// Disconnected graph (no edges)     → E=0.0 (ppm=0).
    /// Algorithm: one BFS per source node, O(V × (V+E)).
    pub fn graph_global_efficiency_inner(&self) -> (u64, usize, usize) {
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        if node_count < 2 {
            return (0, 0, node_count);
        }

        const SCALE: u64 = 1_000_000;
        let mut sum_recip: u64 = 0;

        for si in 0..node_count {
            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };
            let _ = s_id;

            let mut dist  = [u32::MAX; MAX_NODES];
            let mut queue = [0usize;   MAX_NODES];
            dist[s]  = 0;
            queue[0] = s;
            let mut q_head = 0usize;
            let mut q_tail = 1usize;

            while q_head < q_tail {
                let v = queue[q_head];
                q_head += 1;

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
                    if dist[w] == u32::MAX {
                        dist[w] = dist[v].saturating_add(1);
                        if q_tail < MAX_NODES {
                            queue[q_tail] = w;
                            q_tail += 1;
                        }
                    }
                }
            }

            for ti in 0..node_count {
                let t = node_slots[ti];
                if t == s { continue; }
                if dist[t] != u32::MAX && dist[t] > 0 {
                    sum_recip = sum_recip.saturating_add(SCALE / dist[t] as u64);
                }
            }
        }

        let pairs_max = node_count * (node_count - 1);
        let efficiency_ppm = sum_recip / pairs_max as u64;
        (efficiency_ppm, pairs_max, node_count)
    }

    /// V2.75: Graph average clustering coefficient (true Watts-Strogatz per-node average).
    ///
    /// Distinct from graph_clustering (V2.61) and graph_transitivity (V2.63), both of which
    /// compute the same global ratio total_triangles/total_triplets (weighted by degree).
    /// This function computes the unweighted per-node average:
    ///   CC(v) = triangles_v / C(k_v, 2)   where k_v = undirected degree
    ///   avg_CC = (1/n) × Σ CC(v)           nodes with k_v < 2 contribute 0
    ///
    /// Returns (avg_ppm, nodes_computed, node_count).
    ///   avg_ppm       = avg_CC × 1_000_000  (0..=1_000_000)
    ///   nodes_computed = nodes with k ≥ 2 that contributed a non-zero CC(v)
    ///   node_count    = total alive nodes (denominator of average)
    pub fn graph_avg_clustering_inner(&self) -> (u32, usize, usize) {
        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        if n == 0 {
            return (0, 0, 0);
        }

        let mut sum_cc_ppm: u64 = 0;
        let mut nodes_computed = 0usize;

        for slot in 0..MAX_NODES {
            let record = match self.nodes[slot] {
                Some(ref r) => r,
                None => continue,
            };
            let vid = record.spec.node_id;

            // Collect undirected neighbors (deduplicated, no self-loops).
            let mut neighbors = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !neighbors[..nb].contains(&other) {
                    neighbors[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }

            if nb < 2 { continue; }

            let k = nb as u64;
            let triplets = k * (k - 1) / 2;

            // Count undirected edges among neighbor pairs (one per unordered pair).
            let mut triangles: u64 = 0;
            for i in 0..nb {
                for j in (i + 1)..nb {
                    let b = neighbors[i];
                    let c = neighbors[j];
                    let connected = self.edges.iter().flatten().any(|e| {
                        (e.spec.from_node == b && e.spec.to_node == c)
                            || (e.spec.from_node == c && e.spec.to_node == b)
                    });
                    if connected {
                        triangles += 1;
                    }
                }
            }

            sum_cc_ppm += (triangles * 1_000_000) / triplets;
            nodes_computed += 1;
        }

        // Divide by ALL alive nodes (not just nodes_computed) for true WS average.
        let avg_ppm = (sum_cc_ppm / n as u64).min(1_000_000) as u32;
        (avg_ppm, nodes_computed, n)
    }

    /// V2.76: Graph local efficiency (Latora–Marchiori 2001).
    ///
    /// E_loc(G) = (1/n) × Σ_v E(G_v)
    ///
    /// where G_v is the subgraph induced by the **undirected** neighbours of v
    /// (v itself excluded), and E(G_v) is the directed global efficiency of G_v:
    ///
    ///   E(G_v) = Σ_{i≠j ∈ N(v)} 1/d_{G_v}(i,j)  /  (|N(v)| × (|N(v)|−1))
    ///
    /// d_{G_v}(i,j) is the directed BFS distance from i to j using only edges
    /// whose both endpoints are neighbours of v.  Unreachable pairs contribute 0.
    ///
    /// Nodes with undirected degree < 2 contribute E(G_v) = 0 (no pairs in subgraph).
    ///
    /// Returns (eloc_ppm, nodes_computed, node_count):
    ///   eloc_ppm      = E_loc × 1_000_000
    ///   nodes_computed = nodes with undirected degree ≥ 2
    ///   node_count    = total alive nodes (denominator)
    pub fn graph_local_efficiency_inner(&self) -> (u32, usize, usize) {
        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        if n == 0 {
            return (0, 0, 0);
        }

        const SCALE: u64 = 1_000_000;
        let mut sum_loc_ppm: u64 = 0;
        let mut nodes_computed = 0usize;

        for slot in 0..MAX_NODES {
            let record = match self.nodes[slot] {
                Some(ref r) => r,
                None => continue,
            };
            let vid = record.spec.node_id;

            // Collect undirected neighbors (deduplicated, no self-loops).
            let mut neighbors = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !neighbors[..nb].contains(&other) {
                    neighbors[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }

            if nb < 2 { continue; }
            nodes_computed += 1;

            // Compute E(G_v): directed global efficiency of the subgraph on neighbors[0..nb].
            // dist[ni] is the BFS distance from the current source to neighbors[ni].
            let pairs_max = (nb * (nb - 1)) as u64;
            let mut sum_recip: u64 = 0;

            for si in 0..nb {
                let mut dist = [u32::MAX; MAX_NODES];
                let mut queue = [0usize; MAX_NODES];
                dist[si] = 0;
                queue[0] = si;
                let mut q_head = 0usize;
                let mut q_tail = 1usize;

                while q_head < q_tail {
                    let vi = queue[q_head];
                    q_head += 1;
                    let v_id = neighbors[vi];

                    // Follow directed edges from v_id, restricted to nodes in neighbors[0..nb].
                    for edge in self.edges.iter().flatten() {
                        if edge.spec.from_node != v_id { continue; }
                        let w_id = edge.spec.to_node;
                        // Linear scan for w_id in the neighbor set.
                        let mut wi_opt = None;
                        for ni in 0..nb {
                            if neighbors[ni] == w_id {
                                wi_opt = Some(ni);
                                break;
                            }
                        }
                        let wi = match wi_opt {
                            Some(i) => i,
                            None => continue,
                        };
                        if dist[wi] == u32::MAX {
                            dist[wi] = dist[vi].saturating_add(1);
                            if q_tail < MAX_NODES {
                                queue[q_tail] = wi;
                                q_tail += 1;
                            }
                        }
                    }
                }

                // Accumulate 1/d for all reachable targets.
                for ti in 0..nb {
                    if ti == si { continue; }
                    if dist[ti] != u32::MAX && dist[ti] > 0 {
                        sum_recip = sum_recip.saturating_add(SCALE / dist[ti] as u64);
                    }
                }
            }

            // E(G_v) = sum_recip / (nb * (nb-1))
            let ev_ppm = sum_recip / pairs_max;
            sum_loc_ppm = sum_loc_ppm.saturating_add(ev_ppm);
        }

        // E_loc = (1/n) × Σ E(G_v)
        let eloc_ppm = (sum_loc_ppm / n as u64).min(1_000_000) as u32;
        (eloc_ppm, nodes_computed, n)
    }

    /// V2.77: Graph small-world coefficient σ (Humphries–Gurney 2008).
    ///
    /// σ = (CC / CC_rand) / (L / L_rand)
    ///
    /// where:
    ///   CC       = average clustering coefficient (per-node Watts-Strogatz, V2.75)
    ///   CC_rand  ≈ 2·m / (n·(n−1))  — undirected density (E-R random graph baseline)
    ///   L        = average directed path length = Wiener / reachable_pairs
    ///   L_rand   ≈ ln(n) / ln(⟨k⟩)  — E-R baseline, ⟨k⟩ = 2·m/n (integer)
    ///
    /// All arithmetic is integer / fixed-point (no_std safe). ln values use a
    /// compile-time table for x ∈ 1..=128 (covers all possible n and ⟨k⟩ in
    /// this runtime with MAX_NODES=128).
    ///
    /// Returns `(sigma_ppm, cc_ppm, l_ppm, l_rand_ppm, node_count, m_undir)`:
    ///   sigma_ppm   = σ × 1_000_000  (0 if σ cannot be computed)
    ///   cc_ppm      = CC × 1_000_000
    ///   l_ppm       = L × 1_000_000  (0 if no directed paths exist)
    ///   l_rand_ppm  = L_rand × 1_000_000 (0 if ⟨k⟩ < 2)
    ///   node_count  = total alive nodes
    ///   m_undir     = deduplicated undirected edge count
    ///
    /// σ > 1: small-world structure (high local clustering, short paths)
    /// σ ≈ 1: Erdős–Rényi random-graph-like
    /// σ = 0: insufficient connectivity to compute the coefficient
    pub fn graph_small_world_inner(&self) -> (u32, u32, u64, u64, usize, usize) {
        // ln(x) × 1_000_000, truncated, for x in 1..=128.  Index 0 unused.
        const LN_TABLE: [u32; 129] = [
            0,
            0,         693_147, 1_098_612, 1_386_294, 1_609_437,
            1_791_759, 1_945_910, 2_079_441, 2_197_224, 2_302_585,
            2_397_895, 2_484_906, 2_564_949, 2_639_057, 2_708_050,
            2_772_588, 2_833_213, 2_890_371, 2_944_438, 2_995_732,
            3_044_522, 3_091_042, 3_135_494, 3_178_053, 3_218_875,
            3_258_096, 3_295_836, 3_332_204, 3_367_295, 3_401_197,
            3_433_987, 3_465_735, 3_496_508, 3_526_360, 3_555_348,
            3_583_518, 3_610_917, 3_637_586, 3_663_562, 3_688_879,
            3_713_572, 3_737_669, 3_761_200, 3_784_189, 3_806_662,
            3_828_641, 3_850_147, 3_871_201, 3_891_820, 3_912_023,
            3_931_826, 3_951_244, 3_970_292, 3_988_984, 4_007_333,
            4_025_351, 4_043_051, 4_060_443, 4_077_537, 4_094_344,
            4_110_874, 4_127_134, 4_143_134, 4_158_883, 4_174_387,
            4_189_654, 4_204_692, 4_219_507, 4_234_107, 4_248_494,
            4_262_679, 4_276_666, 4_290_459, 4_304_064, 4_317_488,
            4_330_733, 4_343_805, 4_356_708, 4_369_447, 4_382_026,
            4_394_449, 4_406_719, 4_418_841, 4_430_817, 4_442_651,
            4_454_347, 4_465_908, 4_477_337, 4_488_636, 4_499_810,
            4_510_860, 4_521_789, 4_532_599, 4_543_294, 4_553_877,
            4_564_348, 4_574_711, 4_584_967, 4_595_119, 4_605_170,
            4_615_121, 4_624_972, 4_634_728, 4_644_391, 4_653_960,
            4_663_439, 4_672_829, 4_682_131, 4_691_348, 4_700_480,
            4_709_530, 4_718_498, 4_727_388, 4_736_198, 4_744_932,
            4_753_590, 4_762_174, 4_770_685, 4_779_123, 4_787_491,
            4_795_791, 4_804_021, 4_812_184, 4_820_281, 4_828_313,
            4_836_281, 4_844_187, 4_852_030,
        ];

        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        if n < 2 {
            let (cc_ppm, _, _) = self.graph_avg_clustering_inner();
            return (0, cc_ppm, 0, 0, n, 0);
        }

        // Deduplicate directed edges → undirected edge set.
        let mut seen_from = [NodeId::ZERO; MAX_EDGES];
        let mut seen_to   = [NodeId::ZERO; MAX_EDGES];
        let mut m_undir   = 0usize;
        for edge in self.edges.iter().flatten() {
            let u = edge.spec.from_node;
            let v = edge.spec.to_node;
            if u == v { continue; }
            let already = (0..m_undir).any(|j| seen_from[j] == v && seen_to[j] == u);
            if !already && m_undir < MAX_EDGES {
                seen_from[m_undir] = u;
                seen_to[m_undir]   = v;
                m_undir += 1;
            }
        }

        // Average clustering coefficient (CC).
        let (cc_ppm, _, _) = self.graph_avg_clustering_inner();

        // Average directed path length (L) from Wiener index.
        let (wiener, reachable_pairs, _) = self.graph_wiener_inner();
        let l_ppm: u64 = if reachable_pairs > 0 {
            (wiener * 1_000_000) / reachable_pairs as u64
        } else {
            0
        };

        if l_ppm == 0 {
            return (0, cc_ppm, 0, 0, n, m_undir);
        }

        // CC_rand ≈ 2·m / (n·(n−1))  [undirected density, E-R baseline].
        let cc_rand_ppm: u32 = if n >= 2 {
            ((m_undir as u64 * 2 * 1_000_000) / (n as u64 * (n as u64 - 1))) as u32
        } else {
            0
        };
        if cc_rand_ppm == 0 {
            return (0, cc_ppm, l_ppm, 0, n, m_undir);
        }

        // Average degree ⟨k⟩ = 2·m / n (integer truncation).
        let avg_k = (2 * m_undir) / n;
        if avg_k < 2 || avg_k >= LN_TABLE.len() {
            return (0, cc_ppm, l_ppm, 0, n, m_undir);
        }

        // L_rand ≈ ln(n) / ln(⟨k⟩)  [E-R random-graph baseline].
        let ln_n = LN_TABLE[n.min(128)] as u64;
        let ln_k = LN_TABLE[avg_k] as u64;
        if ln_k == 0 {
            return (0, cc_ppm, l_ppm, 0, n, m_undir);
        }
        let l_rand_ppm: u64 = (ln_n * 1_000_000) / ln_k;

        // σ = (CC/CC_rand) / (L/L_rand) = (cc × l_rand × 1e6) / (cc_rand × l)
        let numer = (cc_ppm as u128) * (l_rand_ppm as u128) * 1_000_000u128;
        let denom = (cc_rand_ppm as u128) * (l_ppm as u128);
        let sigma_ppm = if denom == 0 {
            0u32
        } else {
            (numer / denom).min(u32::MAX as u128) as u32
        };

        (sigma_ppm, cc_ppm, l_ppm, l_rand_ppm, n, m_undir)
    }

    /// V2.78: Degree heterogeneity index κ = ⟨k²⟩/⟨k⟩ for scale-free detection.
    ///
    /// Undirected degree k_v = |distinct undirected neighbours of v| (self-loops excluded).
    ///
    /// Returns `(kappa_ppm, max_degree, avg_degree_ppm, node_count, m_undir)`.
    /// - kappa_ppm     = ⟨k²⟩ × 1_000_000 / ⟨k⟩  (0 when no edges)
    /// - max_degree    = maximum undirected degree (k_max)
    /// - avg_degree_ppm = ⟨k⟩ × 1_000_000         (sum_k × 1_000_000 / n)
    /// - node_count    = number of alive nodes
    /// - m_undir       = deduplicated undirected edge count
    ///
    /// Scale-free heuristic: κ >> ⟨k⟩  ↔  kappa_ppm >> avg_degree_ppm.
    ///   kappa_ppm > 3 × avg_degree_ppm  →  "likely scale-free"
    ///   kappa_ppm > 2 × avg_degree_ppm  →  "heterogeneous"
    ///   otherwise                        →  "homogeneous (regular/random-like)"
    pub fn graph_scale_free_inner(&self) -> (u32, u32, u32, usize, usize) {
        // Count alive nodes.
        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        if n == 0 {
            return (0, 0, 0, 0, 0);
        }

        // Compute undirected degree for each alive node.
        let mut deg = [0u32; MAX_NODES];
        let mut slot_ids = [NodeId::ZERO; MAX_NODES];
        let mut n_counted = 0usize;
        for slot in 0..MAX_NODES {
            if let Some(ref r) = self.nodes[slot] {
                slot_ids[slot] = r.spec.node_id;
                n_counted += 1;
            }
        }
        let _ = n_counted;

        for slot in 0..MAX_NODES {
            if self.nodes[slot].is_none() { continue; }
            let vid = slot_ids[slot];
            let mut neighbors = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !neighbors[..nb].contains(&other) {
                    neighbors[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }
            deg[slot] = nb as u32;
        }

        // Aggregate: sum_k, sum_k2, max_k.
        let mut sum_k:  u64 = 0;
        let mut sum_k2: u64 = 0;
        let mut max_k:  u32 = 0;
        for slot in 0..MAX_NODES {
            if self.nodes[slot].is_none() { continue; }
            let k = deg[slot] as u64;
            sum_k  += k;
            sum_k2 += k * k;
            if deg[slot] > max_k { max_k = deg[slot]; }
        }

        // Deduplicated undirected edge count (mirrors graph_small_world_inner).
        let mut seen_from = [NodeId::ZERO; MAX_EDGES];
        let mut seen_to   = [NodeId::ZERO; MAX_EDGES];
        let mut m_undir = 0usize;
        for edge in self.edges.iter().flatten() {
            let u = edge.spec.from_node;
            let v = edge.spec.to_node;
            if u == v { continue; }
            let already = (0..m_undir).any(|j| seen_from[j] == v && seen_to[j] == u);
            if !already && m_undir < MAX_EDGES {
                seen_from[m_undir] = u;
                seen_to[m_undir]   = v;
                m_undir += 1;
            }
        }

        // kappa_ppm = ⟨k²⟩ × 1_000_000 / ⟨k⟩ = sum_k2 × 1_000_000 / sum_k.
        let kappa_ppm: u32 = if sum_k == 0 {
            0
        } else {
            (sum_k2 * 1_000_000 / sum_k).min(u32::MAX as u64) as u32
        };

        // avg_degree_ppm = sum_k × 1_000_000 / n.
        let avg_degree_ppm: u32 = if n == 0 {
            0
        } else {
            (sum_k * 1_000_000 / n as u64).min(u32::MAX as u64) as u32
        };

        (kappa_ppm, max_k, avg_degree_ppm, n, m_undir)
    }

    /// V2.80: Power-law exponent MLE estimator (Clauset–Newman–Shalizi 2009, eq. 3.1).
    ///
    /// γ̂ = 1 + n_fit × [Σ_{i: k_i ≥ 1} ln(k_i)]^{-1}
    ///
    /// Undirected degree k_i is computed per-node (same deduplication as V2.78).
    /// k_min = 1: isolated nodes (k=0) are excluded from the fit.
    ///
    /// Integer arithmetic via the shared LN_TABLE[1..=128] (ln(k) × 1_000_000).
    ///   sum_ln_ppm = Σ LN_TABLE[k_i]  for k_i ≥ 1
    ///   gamma_ppm  = 1_000_000 + n_fit × 10^12 / sum_ln_ppm
    ///              = 0 if sum_ln_ppm == 0 (all non-isolated nodes have k=1; MLE undefined)
    ///
    /// Returns `(gamma_ppm, n_fit, node_count)`:
    /// - gamma_ppm   — γ̂ × 1_000_000  (0 = undefined)
    /// - n_fit       — nodes included in the fit (k ≥ 1)
    /// - node_count  — total alive nodes
    pub fn graph_power_law_inner(&self) -> (u32, usize, usize) {
        const LN_TABLE: [u32; 129] = [
            0,
            0,         693_147, 1_098_612, 1_386_294, 1_609_437,
            1_791_759, 1_945_910, 2_079_441, 2_197_224, 2_302_585,
            2_397_895, 2_484_906, 2_564_949, 2_639_057, 2_708_050,
            2_772_588, 2_833_213, 2_890_371, 2_944_438, 2_995_732,
            3_044_522, 3_091_042, 3_135_494, 3_178_053, 3_218_875,
            3_258_096, 3_295_836, 3_332_204, 3_367_295, 3_401_197,
            3_433_987, 3_465_735, 3_496_508, 3_526_360, 3_555_348,
            3_583_518, 3_610_917, 3_637_586, 3_663_562, 3_688_879,
            3_713_572, 3_737_669, 3_761_200, 3_784_189, 3_806_662,
            3_828_641, 3_850_147, 3_871_201, 3_891_820, 3_912_023,
            3_931_826, 3_951_244, 3_970_292, 3_988_984, 4_007_333,
            4_025_351, 4_043_051, 4_060_443, 4_077_537, 4_094_344,
            4_110_874, 4_127_134, 4_143_134, 4_158_883, 4_174_387,
            4_189_654, 4_204_692, 4_219_507, 4_234_107, 4_248_494,
            4_262_679, 4_276_666, 4_290_459, 4_304_064, 4_317_488,
            4_330_733, 4_343_805, 4_356_708, 4_369_447, 4_382_026,
            4_394_449, 4_406_719, 4_418_841, 4_430_817, 4_442_651,
            4_454_347, 4_465_908, 4_477_337, 4_488_636, 4_499_810,
            4_510_860, 4_521_789, 4_532_599, 4_543_294, 4_553_877,
            4_564_348, 4_574_711, 4_584_967, 4_595_119, 4_605_170,
            4_615_121, 4_624_972, 4_634_728, 4_644_391, 4_653_960,
            4_663_439, 4_672_829, 4_682_131, 4_691_348, 4_700_480,
            4_709_530, 4_718_498, 4_727_388, 4_736_198, 4_744_932,
            4_753_590, 4_762_174, 4_770_685, 4_779_123, 4_787_491,
            4_795_791, 4_804_021, 4_812_184, 4_820_281, 4_828_313,
            4_836_281, 4_844_187, 4_852_030,
        ];

        let n = self.nodes.iter().filter(|s| s.is_some()).count();
        if n == 0 {
            return (0, 0, 0);
        }

        // Compute undirected degree for each alive node slot (same as graph_scale_free_inner).
        let mut deg = [0u32; MAX_NODES];
        let mut slot_ids = [NodeId::ZERO; MAX_NODES];
        for slot in 0..MAX_NODES {
            if let Some(ref r) = self.nodes[slot] {
                slot_ids[slot] = r.spec.node_id;
            }
        }
        for slot in 0..MAX_NODES {
            if self.nodes[slot].is_none() { continue; }
            let vid = slot_ids[slot];
            let mut neighbors = [NodeId::ZERO; MAX_NODES];
            let mut nb = 0usize;
            for edge in self.edges.iter().flatten() {
                let other = if edge.spec.from_node == vid {
                    edge.spec.to_node
                } else if edge.spec.to_node == vid {
                    edge.spec.from_node
                } else {
                    continue;
                };
                if other == vid { continue; }
                if !neighbors[..nb].contains(&other) {
                    neighbors[nb] = other;
                    nb += 1;
                    if nb >= MAX_NODES { break; }
                }
            }
            deg[slot] = nb as u32;
        }

        // MLE: accumulate Σ ln(k_i) for k_i ≥ 1; count n_fit.
        let mut sum_ln_ppm: u64 = 0;
        let mut n_fit: usize = 0;
        for slot in 0..MAX_NODES {
            if self.nodes[slot].is_none() { continue; }
            let k = deg[slot] as usize;
            if k == 0 { continue; }
            let k_capped = k.min(128);
            sum_ln_ppm += LN_TABLE[k_capped] as u64;
            n_fit += 1;
        }

        if sum_ln_ppm == 0 {
            // All non-isolated nodes have k=1: ln(1)=0, MLE is undefined.
            return (0, n_fit, n);
        }

        // gamma_ppm = 1_000_000 + n_fit × 10^12 / sum_ln_ppm.
        let numer: u64 = (n_fit as u64).saturating_mul(1_000_000_000_000);
        let gamma_ppm = 1_000_000u64 + numer / sum_ln_ppm;

        (gamma_ppm.min(u32::MAX as u64) as u32, n_fit, n)
    }

    /// V2.83: Capture all topology metrics in one RUNTIME lock hold.
    ///
    /// Used by `graph_snapshot_save` and `graph_snapshot_compare`.
    /// Calling all inner methods here avoids repeated lock acquisitions and
    /// ensures the metrics are consistent with one another (same graph epoch).
    fn graph_snapshot_inner(&self) -> MetricSnapshot {
        let (density_ppm, node_count, edge_count) = self.graph_density_inner();
        let (trans_ppm, _, _, _)                   = self.graph_transitivity_inner();
        let (avgcc_ppm, _, _)                       = self.graph_avg_clustering_inner();
        let (geff_ppm, _, _)                        = self.graph_global_efficiency_inner();
        let (leff_ppm, _, _)                        = self.graph_local_efficiency_inner();
        let (sigma_ppm, _, _, _, _, _)              = self.graph_small_world_inner();
        let (kappa_ppm, _, _, _, _)                 = self.graph_scale_free_inner();
        let (gamma_ppm, _, _)                       = self.graph_power_law_inner();
        MetricSnapshot {
            valid: true,
            epoch: self.graph_epoch,
            node_count,
            edge_count,
            density_ppm,
            trans_ppm,
            avgcc_ppm,
            geff_ppm,
            leff_ppm,
            sigma_ppm,
            kappa_ppm,
            gamma_ppm,
        }
    }

    /// V2.41: Graph eccentricity — max shortest-path distance from each node.
    ///
    /// ecc[v] = max d(v, u) over all u reachable from v via directed edges (u ≠ v).
    /// Isolated nodes (no reachable neighbours) → ecc[v] = 0.
    ///
    /// Derived scalars:
    ///   radius   = min ecc[v] for v with ecc[v] > 0  (0 if all isolated)
    ///   diameter = max ecc[v]                          (0 if all isolated)
    ///
    /// Output sorted ascending by eccentricity; centre nodes (ecc == radius) appear
    /// first; isolated nodes (ecc = 0) sort last via a u32::MAX sentinel.
    ///
    /// Algorithm: one BFS per source node, O(V × (V+E)).
    /// OS analogy: `traceroute` worst-case hop count — which kernel node has the
    /// tightest guaranteed latency to all reachable peers?
    pub fn graph_eccentricity_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u32; N], usize, u32, u32) {
        // Compact list of live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Per-slot eccentricity.
        let mut ecc = [0u32; MAX_NODES];

        // One BFS per source node following outgoing directed edges.
        for si in 0..node_count {
            let s = node_slots[si];
            let s_id = match self.nodes[s] {
                Some(r) => r.spec.node_id,
                None    => continue,
            };
            let _ = s_id; // presence check; BFS uses per-step v_id

            let mut dist  = [u32::MAX; MAX_NODES];
            let mut queue = [0usize;   MAX_NODES];

            dist[s]  = 0;
            queue[0] = s;
            let mut q_head = 0usize;
            let mut q_tail = 1usize;

            while q_head < q_tail {
                let v = queue[q_head];
                q_head += 1;

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
                    if dist[w] == u32::MAX {
                        dist[w] = dist[v].saturating_add(1);
                        if q_tail < MAX_NODES {
                            queue[q_tail] = w;
                            q_tail += 1;
                        }
                    }
                }
            }

            // ecc[s] = max dist to any reachable node (excl. s itself).
            let mut max_d = 0u32;
            for ti in 0..node_count {
                let t = node_slots[ti];
                if t == s { continue; }
                if dist[t] != u32::MAX && dist[t] > max_d {
                    max_d = dist[t];
                }
            }
            ecc[s] = max_d; // 0 ⟺ isolated (no reachable non-self nodes)
        }

        // Compute radius (min ecc > 0) and diameter (max ecc > 0).
        let mut radius: u32   = u32::MAX;
        let mut diameter: u32 = 0;
        for si in 0..node_count {
            let s = node_slots[si];
            if ecc[s] > 0 {
                if ecc[s] < radius   { radius   = ecc[s]; }
                if ecc[s] > diameter { diameter = ecc[s]; }
            }
        }
        if radius == u32::MAX { radius = 0; } // all nodes isolated

        // Insertion-sort ascending; isolated (ecc=0) use sentinel u32::MAX so they sort last.
        let sort_key = |slot: usize| -> u32 {
            if ecc[slot] > 0 { ecc[slot] } else { u32::MAX }
        };
        let mut sorted = node_slots;
        for i in 1..node_count {
            let key_slot = sorted[i];
            let key_val  = sort_key(key_slot);
            let mut j    = i;
            while j > 0 && sort_key(sorted[j - 1]) > key_val {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        // Pack output arrays (cap at N).
        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_ecc  = [0u32; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = self.nodes[slot]
                .map(|r| r.vector)
                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_ecc[i] = ecc[slot];
        }

        (out_vecs, out_ecc, copy_len, radius, diameter)
    }

    /// V2.42: Incoming Katz centrality — counts all directed walks ending at each node.
    ///
    /// KC[v] = Σ_{k=1}^{∞} α^k × (number of directed walks of length k ending at v)
    /// where α = 1/ALPHA_DEN = 1/8.
    ///
    /// Iterative fixed-point (20 steps, no_std, integer arithmetic):
    ///   x^(0)[v]   = 0
    ///   x^(t+1)[v] = Σ_{u: u→v edges} (SCALE + x^(t)[u]) / ALPHA_DEN
    ///
    /// Convergence: guaranteed for max_in_degree < ALPHA_DEN (= 8).
    /// For higher in-degree, values saturate at u64::MAX (cast to u32::MAX) but
    /// relative ordering among those nodes remains meaningful.
    ///
    /// Score interpretation (×10⁻⁶):
    ///   0          → leaf   (no walks reach this node)
    ///   0 < s ≤ 1M → relay  (receives limited walk-influence)
    ///   s > 1M     → hub    (receives heavy walk-influence)
    ///
    /// Output sorted descending by Katz score.
    /// OS analogy: `netstat -s` hop weight — which kernel service receives the
    /// most signal traffic summed across all walk lengths?
    fn graph_katz_inner<const N: usize>(snap: &GraphTopologySnapshot) -> ([VectorAddress; N], [u32; N], usize) {
        const SCALE:     u64   = 1_000_000;
        const ALPHA_DEN: u64   = 8;
        const K_ITERS:   usize = 20;

        let node_slots = snap.node_slots;
        let node_count = snap.node_count;

        // Double-buffer: x0 = current iteration, x1 = scratch for next.
        let mut x0 = [0u64; MAX_NODES];
        let mut x1 = [0u64; MAX_NODES];

        for _iter in 0..K_ITERS {
            for si in 0..node_count { x1[node_slots[si]] = 0; }

            for vi in 0..node_count {
                let v    = node_slots[vi];
                let v_id = snap.slot_id[v];

                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    if snap.edge_to[ei] != v_id { continue; }
                    let u = match snap.node_slot_by_id(snap.edge_from[ei]) {
                        Some(slot) => slot, None => continue,
                    };
                    let contrib = SCALE.saturating_add(x0[u]) / ALPHA_DEN;
                    x1[v] = x1[v].saturating_add(contrib);
                }
            }

            for si in 0..node_count { x0[node_slots[si]] = x1[node_slots[si]]; }
        }

        let mut sorted = node_slots;
        for i in 1..node_count {
            let key_slot = sorted[i];
            let key_val  = x0[key_slot];
            let mut j    = i;
            while j > 0 && x0[sorted[j - 1]] < key_val {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_katz = [0u32; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = snap.slot_vec[slot];
            out_katz[i] = x0[slot].min(u32::MAX as u64) as u32;
        }

        (out_vecs, out_katz, copy_len)
    }

    /// V2.43: PageRank centrality (random-walk stationary distribution).
    ///
    /// Classical PageRank with damping factor d = 0.85:
    ///
    ///   PR[v] = (1-d) × SCALE + d × Σ_{u→v, outdeg(u)>0} PR[u] / outdeg(u)
    ///
    /// Dangling nodes (out-degree = 0) absorb their rank (do not redistribute it)
    /// — they are treated as "signal drains": they receive walk-mass but never
    /// forward it.  This is the correct GOS semantic: a node that emits no edges
    /// is a terminal consumer, not a relay.
    ///
    /// SCALE = 1_000_000.  Initial rank = SCALE per node.  20 iterations.
    /// Output is sorted descending (highest PageRank first).
    fn graph_pagerank_inner<const N: usize>(snap: &GraphTopologySnapshot) -> ([VectorAddress; N], [u32; N], usize) {
        const SCALE:    u64   = 1_000_000;
        const DAMP_NUM: u64   = 85;
        const DAMP_DEN: u64   = 100;
        const TELE:     u64   = SCALE * (DAMP_DEN - DAMP_NUM) / DAMP_DEN;
        const PR_ITERS: usize = 20;

        let node_slots = snap.node_slots;
        let node_count = snap.node_count;

        if node_count == 0 {
            return ([VectorAddress::new(0, 0, 0, 0); N], [0u32; N], 0);
        }

        let mut out_deg = [0u32; MAX_NODES];
        for ei in 0..MAX_EDGES {
            if !snap.edge_live[ei] { continue; }
            if let Some(u) = snap.node_slot_by_id(snap.edge_from[ei]) {
                out_deg[u] = out_deg[u].saturating_add(1);
            }
        }

        let mut pr0 = [0u64; MAX_NODES];
        let mut pr1 = [0u64; MAX_NODES];
        for si in 0..node_count { pr0[node_slots[si]] = SCALE; }

        for _iter in 0..PR_ITERS {
            for si in 0..node_count { pr1[node_slots[si]] = TELE; }

            for vi in 0..node_count {
                let v    = node_slots[vi];
                let v_id = snap.slot_id[v];

                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    if snap.edge_to[ei] != v_id { continue; }
                    let u = match snap.node_slot_by_id(snap.edge_from[ei]) {
                        Some(slot) => slot, None => continue,
                    };
                    let od = out_deg[u] as u64;
                    if od == 0 { continue; }
                    let contrib = pr0[u].saturating_mul(DAMP_NUM) / (od * DAMP_DEN);
                    pr1[v] = pr1[v].saturating_add(contrib);
                }
            }

            for si in 0..node_count { pr0[node_slots[si]] = pr1[node_slots[si]]; }
        }

        let mut sorted = node_slots;
        for i in 1..node_count {
            let key_slot = sorted[i];
            let key_val  = pr0[key_slot];
            let mut j    = i;
            while j > 0 && pr0[sorted[j - 1]] < key_val {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_pr   = [0u32; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = snap.slot_vec[slot];
            out_pr[i]   = pr0[slot].min(u32::MAX as u64) as u32;
        }

        (out_vecs, out_pr, copy_len)
    }

    /// V2.44: HITS (Hyperlink-Induced Topic Search) hub and authority scores.
    ///
    /// Kleinberg's HITS algorithm produces two complementary scores per node:
    ///   hub[v]       = how well v points to high-authority nodes
    ///   authority[v] = how well v is pointed to by high-hub nodes
    ///
    /// Update rules (applied simultaneously each iteration):
    ///   new_a[v] = Σ_{u→v}  h[u]    (authority = sum of in-neighbor hub scores)
    ///   new_h[v] = Σ_{v→w}  a[w]    (hub = sum of out-neighbor authority scores)
    ///
    /// Normalization after each step: scores scaled so max = SCALE = 1_000_000.
    /// Dangling nodes (no in-edges): authority → 0; (no out-edges): hub → 0.
    /// 20 fixed-point iterations — convergence verified by harness tests.
    ///
    /// Returns `(vecs, hub, auth, total)` sorted descending by authority score.
    fn graph_hits_inner<const N: usize>(snap: &GraphTopologySnapshot) -> ([VectorAddress; N], [u32; N], [u32; N], usize) {
        const SCALE: u64   = 1_000_000;
        const ITERS: usize = 20;

        let node_slots = snap.node_slots;
        let node_count = snap.node_count;

        if node_count == 0 {
            return ([VectorAddress::new(0, 0, 0, 0); N], [0u32; N], [0u32; N], 0);
        }

        let mut hub  = [0u64; MAX_NODES];
        let mut auth = [0u64; MAX_NODES];
        for si in 0..node_count {
            hub[node_slots[si]]  = SCALE;
            auth[node_slots[si]] = SCALE;
        }

        let mut new_hub  = [0u64; MAX_NODES];
        let mut new_auth = [0u64; MAX_NODES];

        for _iter in 0..ITERS {
            for si in 0..node_count {
                new_hub[node_slots[si]]  = 0;
                new_auth[node_slots[si]] = 0;
            }

            // new_a[v] = Σ_{u→v} h[u]
            for vi in 0..node_count {
                let v    = node_slots[vi];
                let v_id = snap.slot_id[v];
                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    if snap.edge_to[ei] != v_id { continue; }
                    let u = match snap.node_slot_by_id(snap.edge_from[ei]) {
                        Some(s) => s, None => continue,
                    };
                    new_auth[v] = new_auth[v].saturating_add(hub[u]);
                }
            }

            // new_h[v] = Σ_{v→w} a[w]
            for vi in 0..node_count {
                let v    = node_slots[vi];
                let v_id = snap.slot_id[v];
                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    if snap.edge_from[ei] != v_id { continue; }
                    let w = match snap.node_slot_by_id(snap.edge_to[ei]) {
                        Some(s) => s, None => continue,
                    };
                    new_hub[v] = new_hub[v].saturating_add(auth[w]);
                }
            }

            let max_auth = { let mut m = 0u64; for si in 0..node_count { let v = node_slots[si]; if new_auth[v] > m { m = new_auth[v]; } } m };
            let max_hub  = { let mut m = 0u64; for si in 0..node_count { let v = node_slots[si]; if new_hub[v]  > m { m = new_hub[v];  } } m };

            for si in 0..node_count {
                let v = node_slots[si];
                auth[v] = if max_auth > 0 { new_auth[v].saturating_mul(SCALE) / max_auth } else { 0 };
                hub[v]  = if max_hub  > 0 { new_hub[v].saturating_mul(SCALE)  / max_hub  } else { 0 };
            }
        }

        let mut sorted = node_slots;
        for i in 1..node_count {
            let key_slot = sorted[i];
            let key_auth = auth[key_slot];
            let mut j    = i;
            while j > 0 && auth[sorted[j - 1]] < key_auth {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key_slot;
        }

        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_hub  = [0u32; N];
        let mut out_auth = [0u32; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = snap.slot_vec[slot];
            out_auth[i] = auth[slot].min(u32::MAX as u64) as u32;
            out_hub[i]  = hub[slot].min(u32::MAX as u64) as u32;
        }

        (out_vecs, out_hub, out_auth, copy_len)
    }

    /// V2.45: Label Propagation Algorithm (LPA) for community detection.
    ///
    /// Treats directed edges as undirected (combines in-edges and out-edges)
    /// so that the strongly connected kernel sub-systems form natural clusters.
    ///
    /// Algorithm (asynchronous, 20 iterations):
    ///   1. Initialize: each node is in its own community (label = slot index).
    ///   2. Each iteration: every node immediately adopts the most-frequent label
    ///      seen among all its neighbors (both in- and out-neighbors); later nodes
    ///      in the same round already see updated labels (asynchronous update).
    ///      Tie-break: smallest label value (deterministic, avoids oscillation).
    ///   3. After 20 iterations the labels are relabelled 0, 1, 2... sorted by
    ///      community size descending (largest community gets id=0).
    ///
    /// Returns `(vecs, community_ids, node_count, community_count)`.
    /// Output sorted by community_id ascending, then by slot ascending within
    /// each community, so that all members of the same community are contiguous.
    ///
    /// Community roles (by size relative to largest):
    ///   major-community — the largest community
    ///   minor-community — smaller but multi-node community
    ///   isolated        — single-node community (no undirected neighbors)
    fn graph_community_inner<const N: usize>(
        snap: &GraphTopologySnapshot,
    ) -> ([VectorAddress; N], [u8; N], usize, usize) {
        const ITERS: usize = 20;

        let node_slots = snap.node_slots;
        let node_count = snap.node_count;

        if node_count == 0 {
            return ([VectorAddress::new(0, 0, 0, 0); N], [0u8; N], 0, 0);
        }

        // Initialize: label[slot] = slot (each node its own community).
        let mut label = [0u8; MAX_NODES];
        for si in 0..node_count {
            label[node_slots[si]] = node_slots[si] as u8;
        }

        // Asynchronous LPA: update each node's label immediately so that later
        // nodes in the same round see already-updated labels.  This avoids the
        // classic synchronous-LPA oscillation on bipartite and chain topologies
        // (where synchronous LPA cycles between two complementary colorings
        // forever instead of converging to one community).
        for _iter in 0..ITERS {
            for si in 0..node_count {
                let v    = node_slots[si];
                let v_id = snap.slot_id[v];

                // Frequency table: freq[label] = how many neighbors carry that label.
                let mut freq = [0u8; MAX_NODES];
                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    let nb_id = if snap.edge_from[ei] == v_id {
                        snap.edge_to[ei]
                    } else if snap.edge_to[ei] == v_id {
                        snap.edge_from[ei]
                    } else {
                        continue;
                    };
                    if let Some(nb) = snap.node_slot_by_id(nb_id) {
                        let l = label[nb] as usize;
                        if l < MAX_NODES { freq[l] = freq[l].saturating_add(1); }
                    }
                }

                // Adopt the label with the highest frequency; tie-break: smallest.
                let mut best_l    = MAX_NODES; // sentinel
                let mut best_freq = 0u8;
                for l in 0..MAX_NODES {
                    if freq[l] == 0 { continue; }
                    if freq[l] > best_freq
                        || (freq[l] == best_freq && (best_l >= MAX_NODES || l < best_l))
                    {
                        best_freq = freq[l];
                        best_l    = l;
                    }
                }
                // Immediate update (asynchronous): later nodes in this round
                // already see v's new label, enabling chain convergence in one pass.
                if best_l < MAX_NODES {
                    label[v] = best_l as u8;
                }
            }
        }

        // Count per-community sizes.
        let mut comm_size = [0u8; MAX_NODES];
        for si in 0..node_count {
            let l = label[node_slots[si]] as usize;
            if l < MAX_NODES { comm_size[l] = comm_size[l].saturating_add(1); }
        }

        // Collect non-empty communities and sort by size desc, then by label asc.
        let mut comm_order = [0usize; MAX_NODES];
        let mut comm_count = 0usize;
        for l in 0..MAX_NODES {
            if comm_size[l] > 0 {
                comm_order[comm_count] = l;
                comm_count += 1;
            }
        }
        for i in 1..comm_count {
            let key_l = comm_order[i];
            let key_s = comm_size[key_l];
            let mut j = i;
            while j > 0 {
                let p   = comm_order[j - 1];
                let p_s = comm_size[p];
                if p_s > key_s || (p_s == key_s && p < key_l) { break; }
                comm_order[j] = p;
                j -= 1;
            }
            comm_order[j] = key_l;
        }

        // Build label → new community_id mapping.
        let mut lbl_to_comm = [0u8; MAX_NODES];
        for ci in 0..comm_count {
            lbl_to_comm[comm_order[ci]] = ci as u8;
        }

        // Sort nodes by (community_id asc, slot asc) for grouped output.
        let mut sorted = node_slots;
        for i in 1..node_count {
            let ks = sorted[i];
            let kc = lbl_to_comm[label[ks] as usize] as usize;
            let mut j = i;
            while j > 0 {
                let ps = sorted[j - 1];
                let pc = lbl_to_comm[label[ps] as usize] as usize;
                if pc < kc || (pc == kc && ps < ks) { break; }
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = ks;
        }

        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_comm = [0u8; N];
        let copy_len     = node_count.min(N);
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = snap.slot_vec[slot];
            out_comm[i] = lbl_to_comm[label[slot] as usize];
        }

        (out_vecs, out_comm, copy_len, comm_count)
    }

    /// V2.46: BFS spanning forest over the undirected projection of the live graph.
    ///
    /// Treats every directed edge as undirected (combines in-edges and out-edges)
    /// so the forest covers all live nodes regardless of edge direction.
    ///
    /// Algorithm (BFS spanning forest):
    ///   1. Iterate over live nodes in ascending slot order.
    ///   2. For each unvisited node start a new BFS tree (it becomes the root).
    ///   3. At each BFS step visit all undirected neighbors; record parent and depth.
    ///   4. Accumulate output in BFS visit order (root first, then level 1, etc.)
    ///      across all trees.
    ///
    /// Returns `(vecs, parents, depths, node_count, tree_count)`:
    ///   vecs[0..node_count]    — node vectors in BFS order
    ///   parents[0..node_count] — parent vector per node (same as vecs[i] for roots)
    ///   depths[0..node_count]  — BFS depth (0 = root)
    ///   node_count             — total live nodes packed into the arrays
    ///   tree_count             — number of BFS trees (= undirected connected components)
    ///
    /// O(V+E); no_std safe; fixed-size stack arrays only.
    fn graph_spanning_inner<const N: usize>(
        snap: &GraphTopologySnapshot,
    ) -> ([VectorAddress; N], [VectorAddress; N], [u8; N], usize, usize) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);

        let node_count = snap.node_count;

        if node_count == 0 {
            return ([ZERO_VEC; N], [ZERO_VEC; N], [0u8; N], 0, 0);
        }

        // Per-slot BFS state.
        let mut visited     = [false; MAX_NODES];
        let mut parent_slot = [0usize; MAX_NODES]; // self = root
        let mut depth_arr   = [0u8;    MAX_NODES];

        // Output slots in BFS visit order across all trees.
        let mut out_slots  = [0usize; MAX_NODES];
        let mut out_len    = 0usize;
        let mut tree_count = 0usize;

        // BFS queue — slots only.
        let mut queue = [0usize; MAX_NODES];

        // Visit each live node in ascending slot order; unvisited nodes become roots.
        for si in 0..node_count {
            let root = snap.node_slots[si];
            if visited[root] { continue; }

            tree_count          += 1;
            visited[root]        = true;
            parent_slot[root]    = root; // root is its own parent
            depth_arr[root]      = 0;

            // Reset and seed queue for this tree.
            let mut q_head = 0usize;
            let mut q_tail = 0usize;
            queue[q_tail] = root;
            q_tail += 1;

            while q_head < q_tail {
                let cur      = queue[q_head];
                q_head += 1;

                if out_len < MAX_NODES {
                    out_slots[out_len] = cur;
                    out_len += 1;
                }

                let cur_id    = snap.slot_id[cur];
                let cur_depth = depth_arr[cur];

                // Enumerate undirected neighbors (out-edges + in-edges).
                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    let nb_id = if snap.edge_from[ei] == cur_id {
                        snap.edge_to[ei]
                    } else if snap.edge_to[ei] == cur_id {
                        snap.edge_from[ei]
                    } else {
                        continue;
                    };

                    let nb = match snap.node_slot_by_id(nb_id) {
                        Some(s) => s,
                        None    => continue,
                    };
                    if nb == cur    { continue; } // skip self-loops
                    if visited[nb]  { continue; }

                    visited[nb]     = true;
                    parent_slot[nb] = cur;
                    depth_arr[nb]   = cur_depth.saturating_add(1);

                    if q_tail < MAX_NODES {
                        queue[q_tail] = nb;
                        q_tail += 1;
                    }
                }
            }
        }

        let mut out_vecs    = [ZERO_VEC; N];
        let mut out_parents = [ZERO_VEC; N];
        let mut out_depths  = [0u8; N];
        let copy_len = out_len.min(N);

        for i in 0..copy_len {
            let slot      = out_slots[i];
            out_vecs[i]    = snap.slot_vec[slot];
            out_parents[i] = snap.slot_vec[parent_slot[slot]];
            out_depths[i]  = depth_arr[slot];
        }

        (out_vecs, out_parents, out_depths, copy_len, tree_count)
    }

    /// V2.47: Welsh-Powell greedy graph coloring.
    ///
    /// Assigns each node a non-negative integer color such that no two adjacent
    /// nodes (connected by any directed edge, treated as undirected) share a color.
    /// Uses the Welsh-Powell heuristic: process nodes in descending total-degree
    /// order, then greedily assign the smallest available color.
    ///
    /// Returns `(vecs, colors, node_count, chromatic_number)`:
    /// - `vecs[0..node_count]`   — node vectors in descending total-degree order.
    /// - `colors[0..node_count]` — color assignment (0-based integer).
    /// - `node_count`            — total live nodes packed into the arrays.
    /// - `chromatic_number`      — number of distinct colors used (max color + 1).
    ///
    /// An isolated node receives color 0.  The chromatic number is a greedy
    /// upper bound on the true chromatic number (optimal only for some classes).
    ///
    /// OS analogy: colors = CPU affinity / scheduling domains — nodes sharing a
    /// domain would conflict, so coloring finds conflict-free domain partitions.
    fn graph_color_inner<const N: usize>(
        snap: &GraphTopologySnapshot,
    ) -> ([VectorAddress; N], [u8; N], [u8; N], usize, u8) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NO_COLOR: u8 = u8::MAX;

        let n = snap.node_count;
        if n == 0 {
            return ([ZERO_VEC; N], [0u8; N], [0u8; N], 0, 0);
        }

        // Step 1 — compute total (undirected) degree for each slot.
        let mut degree = [0usize; MAX_NODES];
        for ei in 0..MAX_EDGES {
            if !snap.edge_live[ei] { continue; }
            // Find the slot index for from / to node ids.
            if let Some(from_s) = snap.node_slot_by_id(snap.edge_from[ei]) {
                if degree[from_s] < usize::MAX { degree[from_s] += 1; }
            }
            if let Some(to_s) = snap.node_slot_by_id(snap.edge_to[ei]) {
                if snap.edge_from[ei] != snap.edge_to[ei] {
                    // avoid double-counting self-loops
                    if degree[to_s] < usize::MAX { degree[to_s] += 1; }
                }
            }
        }

        // Step 2 — build order array: slots sorted by descending degree (stable).
        let mut order = [0usize; MAX_NODES];
        for (i, si) in order.iter_mut().enumerate().take(n) {
            *si = snap.node_slots[i];
        }
        // Insertion sort (n ≤ 128, so O(n²) is fine).
        for i in 1..n {
            let key = order[i];
            let mut j = i;
            while j > 0 && degree[order[j - 1]] < degree[key] {
                order[j] = order[j - 1];
                j -= 1;
            }
            order[j] = key;
        }

        // Step 3 — greedy coloring in sorted order.
        let mut color_slot = [NO_COLOR; MAX_NODES];
        let mut max_color  = 0u8;

        // Scratch buffer: for each candidate color, is it forbidden at this node?
        let mut forbidden = [false; 256];

        for oi in 0..n {
            let cur = order[oi];
            let cur_id = snap.slot_id[cur];

            // Mark colors used by undirected neighbors that are already colored.
            // Reset forbidden to all-false first (only mark what we need).
            let mut max_forbidden = 0usize;
            for ei in 0..MAX_EDGES {
                if !snap.edge_live[ei] { continue; }
                let nb_id = if snap.edge_from[ei] == cur_id {
                    snap.edge_to[ei]
                } else if snap.edge_to[ei] == cur_id {
                    snap.edge_from[ei]
                } else {
                    continue;
                };
                if nb_id == cur_id { continue; } // self-loop
                if let Some(nb_s) = snap.node_slot_by_id(nb_id) {
                    let c = color_slot[nb_s];
                    if c != NO_COLOR {
                        let ci = c as usize;
                        forbidden[ci] = true;
                        if ci + 1 > max_forbidden { max_forbidden = ci + 1; }
                    }
                }
            }

            // Pick smallest non-forbidden color.
            let assigned = (0..=255u8)
                .find(|&c| !forbidden[c as usize])
                .unwrap_or(0);
            color_slot[cur] = assigned;
            if assigned > max_color { max_color = assigned; }

            // Clear forbidden flags we set (avoid memset of 256 bytes each iter).
            for ci in 0..max_forbidden {
                forbidden[ci] = false;
            }
        }

        let chromatic = if n == 0 { 0u8 } else { max_color + 1 };

        // Step 4 — pack output in sorted order.
        let copy_len = n.min(N);
        let mut out_vecs    = [ZERO_VEC; N];
        let mut out_colors  = [0u8; N];
        let mut out_degrees = [0u8; N];
        for i in 0..copy_len {
            let slot        = order[i];
            out_vecs[i]     = snap.slot_vec[slot];
            out_colors[i]   = color_slot[slot];
            out_degrees[i]  = degree[slot].min(u8::MAX as usize) as u8;
        }

        (out_vecs, out_colors, out_degrees, copy_len, chromatic)
    }

    /// V2.48: Prim's algorithm — Minimum Spanning Forest over the undirected
    /// projection of the live kernel graph.
    ///
    /// Each directed edge is treated as undirected with weight `edge.spec.weight`
    /// (default 1.0 for edges registered without an explicit weight).
    /// Disconnected components each get their own MST root (spanning forest).
    ///
    /// Returns `(vecs, parents, weights, node_count, total_mst_w)`:
    /// - `vecs[0..node_count]`    — nodes in Prim visit order.
    /// - `parents[0..node_count]` — parent vector (self for roots).
    /// - `weights[0..node_count]` — edge weight to parent × 1000 as u32 (0 for roots).
    /// - `node_count`             — total live nodes.
    /// - `total_mst_w`            — sum of all MST edge weights × 1000 as u32.
    ///
    /// OS analogy: MST = minimum-cost signal routing backbone — the set of edges
    /// that keeps all kernel sub-systems connected at the lowest total bandwidth cost.
    fn graph_mst_inner<const N: usize>(
        snap: &GraphTopologySnapshot,
    ) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const INF:      f32           = f32::MAX;

        let n = snap.node_count;
        if n == 0 {
            return ([ZERO_VEC; N], [ZERO_VEC; N], [0u32; N], 0, 0);
        }

        // Prim's: key[s] = minimum edge weight connecting slot s to the MST.
        let mut in_mst      = [false;    MAX_NODES];
        let mut key         = [INF;      MAX_NODES];
        let mut parent_slot = [usize::MAX; MAX_NODES];

        // Output in visit order.
        let mut out_slots  = [0usize; MAX_NODES];
        let mut out_key    = [0.0f32; MAX_NODES];
        let mut emit_count = 0usize;

        let mut remaining = n;

        while remaining > 0 {
            // Find an unvisited node with the minimum key (could be INF = new component root).
            let mut u = usize::MAX;
            let mut u_key = INF;
            for si in 0..n {
                let s = snap.node_slots[si];
                if !in_mst[s] && key[s] <= u_key {
                    // Prefer nodes with smaller slot index to break ties deterministically.
                    if key[s] < u_key || (key[s] == u_key && (u == usize::MAX || s < u)) {
                        u = s;
                        u_key = key[s];
                    }
                }
            }
            if u == usize::MAX { break; }

            // If this node has no parent yet, it is a component root — initialize its key.
            if parent_slot[u] == usize::MAX {
                parent_slot[u] = u; // root: parent = self
                key[u] = 0.0;
                u_key  = 0.0;
            }

            in_mst[u] = true;
            if emit_count < N {
                out_slots[emit_count] = u;
                out_key[emit_count]   = u_key;
                emit_count += 1;
            }
            remaining -= 1;

            // Relax neighbors of u (undirected projection).
            let u_id = snap.slot_id[u];
            for ei in 0..MAX_EDGES {
                if !snap.edge_live[ei] { continue; }
                // Determine if edge connects u to some neighbor v (undirected).
                let nb_id = if snap.edge_from[ei] == u_id {
                    snap.edge_to[ei]
                } else if snap.edge_to[ei] == u_id {
                    snap.edge_from[ei]
                } else {
                    continue;
                };
                if nb_id == u_id { continue; } // skip self-loops
                let v = match snap.node_slot_by_id(nb_id) {
                    Some(s) => s,
                    None    => continue,
                };
                if in_mst[v] { continue; }
                let w = snap.edge_weight[ei];
                if w < key[v] {
                    key[v]         = w;
                    parent_slot[v] = u;
                }
            }
        }

        // Build output arrays.
        let copy_len = emit_count.min(N);
        let mut out_vecs    = [ZERO_VEC; N];
        let mut out_parents = [ZERO_VEC; N];
        let mut out_weights = [0u32; N];
        let mut total_w_f   = 0.0f32;

        for i in 0..copy_len {
            let s = out_slots[i];
            out_vecs[i] = snap.slot_vec[s];
            let ps = parent_slot[s];
            out_parents[i] = if ps == usize::MAX { snap.slot_vec[s] } else { snap.slot_vec[ps] };
            let w = out_key[i];
            let w_u32 = if w >= INF { 0 } else { (w * 1000.0) as u32 };
            out_weights[i] = w_u32;
            if i > 0 {
                // Only add edge weights (roots contribute 0).
                total_w_f += w;
            }
        }

        let total_mst_w = if total_w_f >= INF { 0 } else { (total_w_f * 1000.0) as u32 };
        (out_vecs, out_parents, out_weights, copy_len, total_mst_w)
    }

    /// V2.50: Maximum network flow via Edmonds-Karp (BFS Ford-Fulkerson).
    ///
    /// Computes the maximum flow from `source` to `sink` over the **directed**
    /// kernel graph, treating `edge_weight` as edge capacity.  Uses BFS to
    /// find shortest augmenting paths in the residual graph (Edmonds-Karp,
    /// O(V × E²)).
    ///
    /// Returns `(vecs, out_flow, in_flow, node_count, max_flow)`:
    /// - `vecs[0..node_count]`      — all live nodes; source first, sink second.
    /// - `out_flow[0..node_count]`  — per-node total outgoing flow × 1000.
    /// - `in_flow[0..node_count]`   — per-node total incoming flow × 1000.
    /// - `node_count`               — total live nodes.
    /// - `max_flow`                 — maximum flow × 1000 as u32.
    ///
    /// Returns max_flow=0 when source or sink are not found, or source==sink.
    ///
    /// OS analogy: `tc -s qdisc show` bandwidth accounting — the maximum
    /// throughput between two kernel sub-systems given edge capacity limits.
    fn graph_flow_inner<const N: usize>(
        snap: &GraphTopologySnapshot,
        source: VectorAddress,
        sink:   VectorAddress,
    ) -> ([VectorAddress; N], [u32; N], [u32; N], usize, u32) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);

        let n = snap.node_count;
        if n == 0 {
            return ([ZERO_VEC; N], [0u32; N], [0u32; N], 0, 0);
        }

        // Find source slot.
        let src_slot = {
            let mut found = usize::MAX;
            for si in 0..n {
                let s = snap.node_slots[si];
                if snap.slot_vec[s] == source { found = s; break; }
            }
            found
        };
        // Find sink slot.
        let snk_slot = {
            let mut found = usize::MAX;
            for si in 0..n {
                let s = snap.node_slots[si];
                if snap.slot_vec[s] == sink { found = s; break; }
            }
            found
        };

        let copy_len = n.min(N);

        // Degenerate: missing or identical endpoints → zero flow.
        if src_slot == usize::MAX || snk_slot == usize::MAX || src_slot == snk_slot {
            let mut out_vecs = [ZERO_VEC; N];
            for si in 0..copy_len {
                out_vecs[si] = snap.slot_vec[snap.node_slots[si]];
            }
            return (out_vecs, [0u32; N], [0u32; N], copy_len, 0);
        }

        // Edmonds-Karp: BFS augmenting paths in the residual graph.
        // flow[ei] = current flow on forward edge ei (initially 0).
        let mut edge_flow = [0.0f32; MAX_EDGES];
        let mut total_flow = 0.0f32;

        loop {
            // BFS from src_slot to snk_slot through residual capacity.
            let mut pred      = [usize::MAX; MAX_NODES]; // predecessor node slot
            let mut pred_edge = [usize::MAX; MAX_NODES]; // edge index used
            let mut pred_fwd  = [false;      MAX_NODES]; // true=forward, false=backward
            let mut visited   = [false;      MAX_NODES];
            let mut queue     = [0usize;     MAX_NODES];
            let mut q_head    = 0usize;
            let mut q_tail    = 0usize;

            visited[src_slot] = true;
            queue[q_tail] = src_slot;
            q_tail += 1;

            'bfs: while q_head < q_tail {
                let u    = queue[q_head];
                q_head  += 1;
                let u_id = snap.slot_id[u];

                // Forward edges: residual = capacity - flow.
                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    if snap.edge_from[ei] != u_id { continue; }
                    let residual = snap.edge_weight[ei] - edge_flow[ei];
                    if residual <= 1e-9 { continue; }
                    let v_id = snap.edge_to[ei];
                    let v = match snap.node_slot_by_id(v_id) { Some(s) => s, None => continue };
                    if visited[v] { continue; }
                    visited[v]   = true;
                    pred[v]      = u;
                    pred_edge[v] = ei;
                    pred_fwd[v]  = true;
                    if v == snk_slot { break 'bfs; }
                    if q_tail < MAX_NODES { queue[q_tail] = v; q_tail += 1; }
                }

                // Backward edges: residual = existing flow (cancel it).
                for ei in 0..MAX_EDGES {
                    if !snap.edge_live[ei] { continue; }
                    if snap.edge_to[ei] != u_id { continue; }
                    if edge_flow[ei] <= 1e-9 { continue; }
                    let v_id = snap.edge_from[ei];
                    let v = match snap.node_slot_by_id(v_id) { Some(s) => s, None => continue };
                    if visited[v] { continue; }
                    visited[v]   = true;
                    pred[v]      = u;
                    pred_edge[v] = ei;
                    pred_fwd[v]  = false;
                    if v == snk_slot { break 'bfs; }
                    if q_tail < MAX_NODES { queue[q_tail] = v; q_tail += 1; }
                }
            }

            if !visited[snk_slot] { break; } // No augmenting path — max-flow reached.

            // Find bottleneck: minimum residual capacity along the path.
            let mut bottleneck = f32::MAX;
            let mut cur = snk_slot;
            while cur != src_slot {
                let ei  = pred_edge[cur];
                let fwd = pred_fwd[cur];
                let res = if fwd {
                    snap.edge_weight[ei] - edge_flow[ei]
                } else {
                    edge_flow[ei]
                };
                if res < bottleneck { bottleneck = res; }
                cur = pred[cur];
            }

            if bottleneck <= 1e-9 { break; }

            // Augment flow along the path.
            total_flow += bottleneck;
            cur = snk_slot;
            while cur != src_slot {
                let ei  = pred_edge[cur];
                let fwd = pred_fwd[cur];
                if fwd { edge_flow[ei] += bottleneck; } else { edge_flow[ei] -= bottleneck; }
                cur = pred[cur];
            }
        }

        // Tally per-node inflow and outflow from the final flow assignment.
        let mut node_out = [0.0f32; MAX_NODES];
        let mut node_in  = [0.0f32; MAX_NODES];
        for ei in 0..MAX_EDGES {
            if !snap.edge_live[ei] { continue; }
            let f = edge_flow[ei];
            if f <= 1e-9 { continue; }
            let from_id = snap.edge_from[ei];
            let to_id   = snap.edge_to[ei];
            if let Some(fs) = snap.node_slot_by_id(from_id) { node_out[fs] += f; }
            if let Some(ts) = snap.node_slot_by_id(to_id)   { node_in[ts]  += f; }
        }

        // Pack output: source first, sink second, remaining in slot order.
        let mut out_vecs = [ZERO_VEC; N];
        let mut out_out  = [0u32; N];
        let mut out_in   = [0u32; N];
        let mut idx = 0usize;

        if idx < copy_len {
            out_vecs[idx] = snap.slot_vec[src_slot];
            out_out[idx]  = (node_out[src_slot] * 1000.0) as u32;
            out_in[idx]   = (node_in[src_slot]  * 1000.0) as u32;
            idx += 1;
        }
        if idx < copy_len {
            out_vecs[idx] = snap.slot_vec[snk_slot];
            out_out[idx]  = (node_out[snk_slot] * 1000.0) as u32;
            out_in[idx]   = (node_in[snk_slot]  * 1000.0) as u32;
            idx += 1;
        }
        for si in 0..n {
            if idx >= copy_len { break; }
            let s = snap.node_slots[si];
            if s == src_slot || s == snk_slot { continue; }
            out_vecs[idx] = snap.slot_vec[s];
            out_out[idx]  = (node_out[s] * 1000.0) as u32;
            out_in[idx]   = (node_in[s]  * 1000.0) as u32;
            idx += 1;
        }

        let max_flow_u32 = (total_flow * 1000.0) as u32;
        (out_vecs, out_out, out_in, copy_len, max_flow_u32)
    }

    /// V2.52: Random walk simulation over the live kernel graph.
    ///
    /// Performs `steps` random walk steps starting from a random live node.
    /// At each step the walker samples an outgoing edge proportional to its
    /// weight (uniform if all weights are zero) and moves to the target node.
    /// Dead-end nodes (no live outgoing edges) cause a teleport to a uniformly
    /// random live node (analogous to PageRank's damping restart).
    ///
    /// Returns `(vecs, visits, node_count, actual_steps, stuck_steps)`:
    /// - `vecs[0..node_count]`   — nodes sorted by visit count descending.
    /// - `visits[0..node_count]` — raw visit count for each node.
    /// - `node_count`            — total live nodes.
    /// - `actual_steps`          — steps that traversed an edge.
    /// - `stuck_steps`           — steps that hit a dead end and teleported.
    ///
    /// `sum(visits) == 1 + actual_steps + stuck_steps == 1 + steps`
    /// for any non-empty graph with steps > 0.
    fn graph_sim_inner<const N: usize>(
        snap: &GraphTopologySnapshot,
        steps: u32,
        seed:  u32,
    ) -> ([VectorAddress; N], [u32; N], usize, u32, u32) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);

        let n = snap.node_count;
        if n == 0 || steps == 0 {
            return ([ZERO_VEC; N], [0u32; N], n, 0, 0);
        }

        // xorshift32 PRNG — never zero.
        let mut rng: u32 = if seed == 0 { 0xDEAD_BEEF } else { seed };
        macro_rules! next_rng {
            () => {{
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                rng
            }};
        }

        let mut raw_visits = [0u32; MAX_NODES];

        // Start at a random live node.
        let start_idx  = (next_rng!() as usize) % n;
        let mut cur_slot = snap.node_slots[start_idx];
        raw_visits[cur_slot] = raw_visits[cur_slot].saturating_add(1);

        let mut actual_steps = 0u32;
        let mut stuck_steps  = 0u32;

        for _ in 0..steps {
            let cur_id = snap.slot_id[cur_slot];

            // Collect outgoing edges (to live nodes only) and sum weights.
            let mut out_idxs  = [0usize; MAX_EDGES];
            let mut out_count = 0usize;
            let mut total_w   = 0u32;

            for ei in 0..MAX_EDGES {
                if !snap.edge_live[ei]           { continue; }
                if snap.edge_from[ei] != cur_id  { continue; }
                if snap.node_slot_by_id(snap.edge_to[ei]).is_none() { continue; }
                out_idxs[out_count] = ei;
                let w = (snap.edge_weight[ei] * 1000.0) as u32;
                total_w = total_w.saturating_add(if w == 0 { 1 } else { w });
                out_count += 1;
            }

            if out_count == 0 {
                // Dead end — teleport to a random live node.
                stuck_steps += 1;
                let restart_idx = (next_rng!() as usize) % n;
                cur_slot = snap.node_slots[restart_idx];
                raw_visits[cur_slot] = raw_visits[cur_slot].saturating_add(1);
                continue;
            }

            // Sample an edge proportional to weight.
            let pick   = next_rng!() % total_w;
            let mut cumulative = 0u32;
            let mut chosen_ei  = out_idxs[0];
            for k in 0..out_count {
                let ei = out_idxs[k];
                let w  = (snap.edge_weight[ei] * 1000.0) as u32;
                cumulative += if w == 0 { 1 } else { w };
                if cumulative > pick {
                    chosen_ei = ei;
                    break;
                }
            }

            if let Some(next_slot) = snap.node_slot_by_id(snap.edge_to[chosen_ei]) {
                cur_slot = next_slot;
                raw_visits[cur_slot] = raw_visits[cur_slot].saturating_add(1);
                actual_steps += 1;
            } else {
                stuck_steps += 1;
            }
        }

        // Pack output sorted by visits descending (insertion sort — N ≤ 128).
        let copy_len       = n.min(N);
        let mut out_vecs   = [ZERO_VEC; N];
        let mut out_visits = [0u32; N];

        for i in 0..copy_len {
            let s = snap.node_slots[i];
            out_vecs[i]   = snap.slot_vec[s];
            out_visits[i] = raw_visits[s];
        }
        for i in 1..copy_len {
            let mut j = i;
            while j > 0 && out_visits[j - 1] < out_visits[j] {
                out_vecs.swap(j - 1, j);
                out_visits.swap(j - 1, j);
                j -= 1;
            }
        }

        (out_vecs, out_visits, n, actual_steps, stuck_steps)
    }

    /// V2.53: Weighted betweenness centrality via all-pairs Dijkstra (Brandes).
    ///
    /// Like `graph_centrality` (V2.39, unweighted BFS Brandes) but uses
    /// `edge.spec.weight` to find minimum-weight paths via Dijkstra:
    ///   WBC[v] = Σ_{s≠v≠t} σ_w(s,t,v) / σ_w(s,t)
    /// where σ_w(s,t) counts shortest-weight directed paths from s to t.
    ///
    /// Diverges from unweighted betweenness when indirect low-weight paths are
    /// shorter by weight than direct high-weight edges.  Uniform-weight graphs
    /// give identical results to `graph_centrality`.
    ///
    /// Algorithm: O(V² × (V+E)) — one O(V²) Dijkstra per source, no heap.
    /// Output sorted descending; wbc[i] = raw_scaled / 1_000_000.
    pub fn graph_between_inner<const N: usize>(&self) -> ([VectorAddress; N], [u32; N], usize) {
        const SCALE: u64 = 1_000_000;
        const EPS:   f32 = 1e-6;

        // Compact list of live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Per-slot weighted betweenness accumulator.
        let mut bc_scaled = [0u64; MAX_NODES];

        // ── Brandes + Dijkstra — one pass per source ─────────────────────────
        for si in 0..node_count {
            let s = node_slots[si];

            // Dijkstra state.
            let mut dist    = [f32::MAX; MAX_NODES];
            let mut sigma   = [0u64;     MAX_NODES];
            let mut visited = [false;    MAX_NODES];
            let mut stk     = [0usize;   MAX_NODES]; // extraction order (non-decr dist)
            let mut stk_len = 0usize;

            dist[s]  = 0.0;
            sigma[s] = 1;

            for _ in 0..node_count {
                // Pick unvisited node with minimum distance (O(V²) Dijkstra).
                let mut u     = usize::MAX;
                let mut u_dst = f32::MAX;
                for ni in 0..node_count {
                    let sl = node_slots[ni];
                    if !visited[sl] && dist[sl] < u_dst {
                        u     = sl;
                        u_dst = dist[sl];
                    }
                }
                if u == usize::MAX || u_dst >= f32::MAX { break; }
                visited[u] = true;

                // Record extraction order for back-propagation.
                if stk_len < MAX_NODES {
                    stk[stk_len] = u;
                    stk_len += 1;
                }

                let u_id = match self.nodes[u] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                // Relax directed out-edges from u.
                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => continue,
                    };
                    if edge.spec.from_node != u_id { continue; }
                    let v = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(sl) => sl,
                        None     => continue,
                    };
                    if v == u { continue; } // skip self-loops
                    let w  = edge.spec.weight.max(0.0);
                    let nd = u_dst + w;
                    if nd < dist[v] - EPS {
                        // Strictly shorter: replace path count.
                        dist[v]  = nd;
                        sigma[v] = sigma[u];
                    } else if (nd - dist[v]).abs() <= EPS && dist[v] < f32::MAX {
                        // Equal-weight path: accumulate path count.
                        sigma[v] = sigma[v].saturating_add(sigma[u]);
                    }
                }
            }

            // ── Back-propagation (reverse extraction order) ───────────────────
            let mut delta = [0u64; MAX_NODES];
            for bi in (0..stk_len).rev() {
                let w = stk[bi];
                if w == s || sigma[w] == 0 { continue; }

                let w_id = match self.nodes[w] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                // Find in-edges of w; v is a predecessor iff dist[v]+weight ≈ dist[w].
                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => continue,
                    };
                    if edge.spec.to_node != w_id { continue; }
                    let v = match self.node_slot_by_id(edge.spec.from_node) {
                        Some(sl) => sl,
                        None     => continue,
                    };
                    if sigma[v] == 0 { continue; }
                    if dist[v] >= f32::MAX { continue; }
                    let ew = edge.spec.weight.max(0.0);
                    if (dist[v] + ew - dist[w]).abs() > EPS { continue; }

                    // δ[v] += σ[v] × (SCALE + δ[w]) / σ[w]
                    let contribution = sigma[v]
                        .saturating_mul(SCALE.saturating_add(delta[w]))
                        / sigma[w];
                    delta[v] = delta[v].saturating_add(contribution);
                }

                bc_scaled[w] = bc_scaled[w].saturating_add(delta[w]);
            }
        }

        // ── Sort node_slots by descending WBC (insertion sort) ────────────────
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

        // ── Pack output ───────────────────────────────────────────────────────
        let copy_len = node_count.min(N);
        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_bc   = [0u32; N];
        for i in 0..copy_len {
            let slot    = sorted[i];
            out_vecs[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_bc[i]   = (bc_scaled[slot] / SCALE) as u32;
        }
        (out_vecs, out_bc, copy_len)
    }

    /// V2.54: Attractor set detection over the **directed** kernel graph.
    ///
    /// An **attractor** (bottom SCC / sink SCC) is a strongly-connected component
    /// from which no directed edge leaves to any node outside that component.
    /// Once control/signal flow enters an attractor it can never escape.
    ///
    /// Every finite directed graph has at least one attractor SCC.
    /// Isolated nodes and self-loop-only nodes are trivial attractor SCCs.
    ///
    /// Node roles (stored in `roles[i]`, returned u8):
    ///   0 = **attractor** — member of a bottom SCC; no condensation out-edges.
    ///   1 = **drain**     — not in a bottom SCC but has a direct condensation
    ///                        edge to at least one attractor SCC.
    ///   2 = **transient** — has outgoing condensation edges, but none lead
    ///                        directly to an attractor SCC (must traverse one or
    ///                        more drain SCCs to reach stability).
    ///
    /// Output is packed in role order (0 then 1 then 2); within each tier
    /// nodes appear in stable slot order.
    ///
    /// Returns `(vecs, roles, total, attractor_count)`:
    /// - `vecs[0..total]`  — all live node vectors in role-sorted order.
    /// - `roles[0..total]` — role (0/1/2) for each node.
    /// - `total`           — number of live nodes.
    /// - `attractor_count` — number of nodes with role=0 (in bottom SCCs).
    ///
    /// Algorithm: Kosaraju two-pass DFS for SCC (O(V+E)), then two edge-scan
    /// passes to classify SCCs in the condensation DAG.  O(V+E) total.
    pub fn graph_attractor_inner<const N: usize>(&self) -> ([VectorAddress; N], [u8; N], usize, usize) {
        const UNSET: u16 = u16::MAX;
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);

        // Compact list of live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        if node_count == 0 {
            return ([ZERO_VEC; N], [0u8; N], 0, 0);
        }

        // ── Phase 1: forward DFS → finish-order stack ─────────────────────────
        let mut visited:      [bool;          MAX_NODES] = [false; MAX_NODES];
        let mut finish_stack: [usize;         MAX_NODES] = [0;     MAX_NODES];
        let mut finish_len = 0usize;
        let mut dfs_stack:  [(usize, usize);  MAX_NODES] = [(0, 0); MAX_NODES];

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
                    None    => { stack_top -= 1; continue; }
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
                    if nbr_slot == cur_slot { ei += 1; continue; } // self-loop
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

        // ── Phase 2: transposed DFS in reverse finish order → SCC IDs ─────────
        let mut scc_id: [u16; MAX_NODES] = [UNSET; MAX_NODES];
        let mut scc_count = 0usize;

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
                    None    => { stack_top -= 1; continue; }
                };
                let mut pushed = false;
                let mut ei = scan_start;
                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                    if edge.spec.to_node != cur_id { ei += 1; continue; } // transposed
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

        // ── Phase 3a: scan condensation — find SCCs with outgoing edges ────────
        // scc_has_out[c] = true iff SCC c has a condensation edge to another SCC.
        let mut scc_has_out    = [false; MAX_NODES];
        // scc_adj_attract[c] = true iff SCC c has a direct condensation edge
        // to an attractor SCC (role=1 means drain).
        let mut scc_adj_attract = [false; MAX_NODES];

        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let from_slot = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let to_slot   = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            if from_slot == to_slot { continue; } // self-loop: no condensation edge
            let sf = scc_id[from_slot];
            let st = scc_id[to_slot];
            if sf == UNSET || st == UNSET || sf == st { continue; }
            scc_has_out[sf as usize] = true;
        }

        // ── Phase 3b: scan condensation — find drains ─────────────────────────
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let from_slot = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let to_slot   = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            if from_slot == to_slot { continue; }
            let sf = scc_id[from_slot];
            let st = scc_id[to_slot];
            if sf == UNSET || st == UNSET || sf == st { continue; }
            // If the destination SCC is an attractor (no outgoing condensation edges)
            // then the source SCC is a direct drain.
            if !scc_has_out[st as usize] {
                scc_adj_attract[sf as usize] = true;
            }
        }

        // ── Phase 4: pack output in role order (0 → 1 → 2) ───────────────────
        let mut out_vecs  = [ZERO_VEC; N];
        let mut out_roles = [0u8; N];
        let mut out_len   = 0usize;
        let mut attractor_count = 0usize;

        for role in 0u8..3 {
            for ki in 0..node_count {
                let slot = node_slots[ki];
                let sc = scc_id[slot];
                if sc == UNSET { continue; }
                let sci = sc as usize;
                let node_role: u8 = if !scc_has_out[sci] {
                    0 // attractor
                } else if scc_adj_attract[sci] {
                    1 // drain
                } else {
                    2 // transient
                };
                if node_role != role { continue; }
                if out_len < N {
                    out_vecs[out_len]  = self.nodes[slot]
                        .map(|r| r.vector)
                        .unwrap_or(ZERO_VEC);
                    out_roles[out_len] = node_role;
                    out_len += 1;
                    if node_role == 0 { attractor_count += 1; }
                }
            }
        }

        (out_vecs, out_roles, out_len, attractor_count)
    }

    /// V2.49: Dijkstra single-source shortest-path tree over the **directed**
    /// kernel graph from a given source node.
    ///
    /// Uses edge directions as-is (unlike MST/spanning which treat edges as
    /// undirected).  Edge weights come from `edge.spec.weight` (default 1.0).
    /// Nodes unreachable from the source receive distance `u32::MAX`.
    ///
    /// Returns `(vecs, parents, distances, node_count)`:
    /// - `vecs[0..node_count]`     — all live nodes; source is first.
    /// - `parents[0..node_count]`  — parent in SPT (self for source, ZERO_VEC for unreachable).
    /// - `distances[0..node_count]`— distance × 1000 as u32 (u32::MAX = unreachable).
    /// - `node_count`              — total live nodes.
    ///
    /// If `source` does not match any live node, returns all nodes with
    /// distance u32::MAX and ZERO_VEC parents (no shortest-path tree).
    ///
    /// OS analogy: `ip route show table` with metrics — the minimum-latency
    /// directed path from a source kernel sub-system to all reachable peers.
    fn graph_shortest_inner<const N: usize>(
        snap: &GraphTopologySnapshot,
        source: VectorAddress,
    ) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const INF: f32 = f32::MAX;

        let n = snap.node_count;
        if n == 0 {
            return ([ZERO_VEC; N], [ZERO_VEC; N], [u32::MAX; N], 0);
        }

        // Find source slot.
        let src_slot = {
            let mut found = usize::MAX;
            for si in 0..n {
                let s = snap.node_slots[si];
                if snap.slot_vec[s] == source {
                    found = s;
                    break;
                }
            }
            found
        };

        // Dijkstra state: directed relaxation (only follow out-edges from u).
        let mut visited = [false; MAX_NODES];
        let mut dist    = [INF;   MAX_NODES];
        let mut parent  = [usize::MAX; MAX_NODES];

        if src_slot != usize::MAX {
            dist[src_slot]   = 0.0;
            parent[src_slot] = src_slot;
        }

        for _ in 0..n {
            // Pick unvisited node with minimum distance.
            let mut u = usize::MAX;
            let mut u_dist = INF;
            for si in 0..n {
                let s = snap.node_slots[si];
                if !visited[s] && dist[s] < u_dist {
                    u      = s;
                    u_dist = dist[s];
                }
            }
            if u == usize::MAX || u_dist >= INF { break; } // remaining unreachable
            visited[u] = true;

            // Relax directed out-edges from u only.
            let u_id = snap.slot_id[u];
            for ei in 0..MAX_EDGES {
                if !snap.edge_live[ei] { continue; }
                if snap.edge_from[ei] != u_id { continue; }
                let nb_id = snap.edge_to[ei];
                if nb_id == u_id { continue; } // self-loop
                let v = match snap.node_slot_by_id(nb_id) {
                    Some(s) => s,
                    None    => continue,
                };
                if visited[v] { continue; }
                let w = snap.edge_weight[ei];
                let new_d = u_dist + w;
                if new_d < dist[v] {
                    dist[v]   = new_d;
                    parent[v] = u;
                }
            }
        }

        // Pack output: source first, then all other live nodes in slot order.
        let copy_len = n.min(N);
        let mut out_vecs    = [ZERO_VEC; N];
        let mut out_parents = [ZERO_VEC; N];
        let mut out_dists   = [u32::MAX; N];

        let mut idx = 0usize;
        // Source goes first if found.
        if src_slot != usize::MAX && idx < copy_len {
            out_vecs[idx]    = snap.slot_vec[src_slot];
            out_parents[idx] = snap.slot_vec[src_slot];
            out_dists[idx]   = 0;
            idx += 1;
        }
        for si in 0..n {
            if idx >= copy_len { break; }
            let s = snap.node_slots[si];
            if s == src_slot { continue; }
            out_vecs[idx] = snap.slot_vec[s];
            if parent[s] != usize::MAX {
                out_parents[idx] = snap.slot_vec[parent[s]];
            }
            out_dists[idx] = if dist[s] >= INF {
                u32::MAX
            } else {
                (dist[s] * 1000.0) as u32
            };
            idx += 1;
        }

        (out_vecs, out_parents, out_dists, copy_len)
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

    /// V2.84: Link prediction metrics for node pair (u, v).
    ///
    /// Four classical scores predict whether a missing edge u→v is likely to form:
    ///   CN  = |N(u) ∩ N(v)|                           (common neighbours count)
    ///   Jaccard = CN / |N(u) ∪ N(v)| × 1_000_000     (normalised overlap, ppm)
    ///   AA  = Σ_{w∈CN} 1_000_000/ln(deg(w)) × 1_000_000 (Adamic-Adar, ppm; skips deg≤1)
    ///   RA  = Σ_{w∈CN} 1_000_000/deg(w)               (Resource Allocation, ppm)
    ///
    /// Neighbourhood N(u) is undirected: w∈N(u) if edge u→w or w→u exists.
    /// u and v are excluded from each other's neighbourhoods.
    /// Degenerate (u == v) → all zeros.
    ///
    /// Returns (cn, jaccard_ppm, aa_ppm, ra_ppm, node_count).
    pub fn graph_link_predict_inner(
        &self,
        u: VectorAddress,
        v: VectorAddress,
    ) -> (usize, u32, u32, u32, usize) {
        // LN_TABLE[k] = floor(ln(k) × 1_000_000), k ∈ [0..128].
        // LN_TABLE[0] = 0; LN_TABLE[1] = 0 (ln(1)=0 → skip in AA).
        const LN_TABLE: [u32; 129] = [
            0,
            0,         693_147, 1_098_612, 1_386_294, 1_609_437,
            1_791_759, 1_945_910, 2_079_441, 2_197_224, 2_302_585,
            2_397_895, 2_484_906, 2_564_949, 2_639_057, 2_708_050,
            2_772_588, 2_833_213, 2_890_371, 2_944_438, 2_995_732,
            3_044_522, 3_091_042, 3_135_494, 3_178_053, 3_218_875,
            3_258_096, 3_295_836, 3_332_204, 3_367_295, 3_401_197,
            3_433_987, 3_465_735, 3_496_508, 3_526_360, 3_555_348,
            3_583_518, 3_610_917, 3_637_586, 3_663_562, 3_688_879,
            3_713_572, 3_737_669, 3_761_200, 3_784_189, 3_806_662,
            3_828_641, 3_850_147, 3_871_201, 3_891_820, 3_912_023,
            3_931_826, 3_951_244, 3_970_292, 3_988_984, 4_007_333,
            4_025_351, 4_043_051, 4_060_443, 4_077_537, 4_094_344,
            4_110_874, 4_127_134, 4_143_134, 4_158_883, 4_174_387,
            4_189_654, 4_204_692, 4_219_507, 4_234_107, 4_248_494,
            4_262_679, 4_276_666, 4_290_459, 4_304_064, 4_317_488,
            4_330_733, 4_343_805, 4_356_708, 4_369_447, 4_382_026,
            4_394_449, 4_406_719, 4_418_841, 4_430_817, 4_442_651,
            4_454_347, 4_465_908, 4_477_337, 4_488_636, 4_499_810,
            4_510_860, 4_521_789, 4_532_599, 4_543_294, 4_553_877,
            4_564_348, 4_574_711, 4_584_967, 4_595_119, 4_605_170,
            4_615_121, 4_624_972, 4_634_728, 4_644_391, 4_653_960,
            4_663_439, 4_672_829, 4_682_131, 4_691_348, 4_700_480,
            4_709_530, 4_718_498, 4_727_388, 4_736_198, 4_744_932,
            4_753_590, 4_762_174, 4_770_685, 4_779_123, 4_787_491,
            4_795_791, 4_804_021, 4_812_184, 4_820_281, 4_828_313,
            4_836_281, 4_844_187, 4_852_030,
        ];

        // Count live nodes.
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_count += 1;
            }
        }

        // Resolve slots; return zeros for unknown vectors.
        let u_slot = match self.node_slot_by_vec(u) { Some(s) => s, None => return (0, 0, 0, 0, node_count) };
        let v_slot = match self.node_slot_by_vec(v) { Some(s) => s, None => return (0, 0, 0, 0, node_count) };
        if u_slot == v_slot { return (0, 0, 0, 0, node_count); }

        // Build undirected neighbour bit-vectors (MAX_NODES=128 → 2 × u64).
        let mut nbr_u = [0u64; 2];
        let mut nbr_v = [0u64; 2];
        // Per-slot total undirected degree (self-loops count once).
        let mut deg = [0u32; MAX_NODES];

        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };

            // Undirected neighbourhood membership.
            if fs == u_slot && ts != u_slot { nbr_u[ts / 64] |= 1u64 << (ts % 64); }
            if ts == u_slot && fs != u_slot { nbr_u[fs / 64] |= 1u64 << (fs % 64); }
            if fs == v_slot && ts != v_slot { nbr_v[ts / 64] |= 1u64 << (ts % 64); }
            if ts == v_slot && fs != v_slot { nbr_v[fs / 64] |= 1u64 << (fs % 64); }

            // Degree: self-loops count once; other edges count for both endpoints.
            if fs == ts {
                deg[fs] = deg[fs].saturating_add(1);
            } else {
                deg[fs] = deg[fs].saturating_add(1);
                deg[ts] = deg[ts].saturating_add(1);
            }
        }

        // Exclude u and v themselves from each other's neighbourhood sets.
        nbr_u[u_slot / 64] &= !(1u64 << (u_slot % 64));
        nbr_u[v_slot / 64] &= !(1u64 << (v_slot % 64));
        nbr_v[u_slot / 64] &= !(1u64 << (u_slot % 64));
        nbr_v[v_slot / 64] &= !(1u64 << (v_slot % 64));

        // CN = |intersection|; |union| for Jaccard denominator.
        let inter0 = nbr_u[0] & nbr_v[0];
        let inter1 = nbr_u[1] & nbr_v[1];
        let union0 = nbr_u[0] | nbr_v[0];
        let union1 = nbr_u[1] | nbr_v[1];

        let cn = (inter0.count_ones() + inter1.count_ones()) as usize;
        let un = (union0.count_ones() + union1.count_ones()) as usize;

        let jaccard_ppm: u32 = if un == 0 {
            0
        } else {
            ((cn as u64 * 1_000_000) / un as u64) as u32
        };

        // AA and RA: iterate over common-neighbour slots via bit-scan.
        let mut aa_acc: u64 = 0;
        let mut ra_acc: u64 = 0;

        for word in 0..2usize {
            let mut bits = if word == 0 { inter0 } else { inter1 };
            while bits != 0 {
                let bit  = bits.trailing_zeros() as usize;
                let slot = word * 64 + bit;
                bits    &= bits - 1; // clear lowest set bit

                let d   = deg[slot].min(128) as usize;
                let ln_d = LN_TABLE[d] as u64;
                // AA: 1/ln(d) × 1e6; LN_TABLE[d] is ln(d)×1e6 so term = 1e12/LN_TABLE[d].
                if ln_d > 0 {
                    aa_acc = aa_acc.saturating_add(1_000_000_000_000u64 / ln_d);
                }
                // RA: 1/deg(w) × 1e6.
                if d > 0 {
                    ra_acc = ra_acc.saturating_add(1_000_000u64 / d as u64);
                }
            }
        }

        let aa_ppm = aa_acc.min(u32::MAX as u64) as u32;
        let ra_ppm = ra_acc.min(u32::MAX as u64) as u32;

        (cn, jaccard_ppm, aa_ppm, ra_ppm, node_count)
    }

    /// V2.85: Articulation point detection on the undirected projection.
    ///
    /// Returns `(art_vecs, art_count, node_count)`:
    /// - `art_vecs[0..art_count]` — cut-vertex vectors sorted ascending by as_u64().
    /// - `art_count`              — number of articulation points found.
    /// - `node_count`             — total live node count.
    ///
    /// Algorithm: iterative Tarjan (disc/low-link DFS), O(V+E).
    pub fn graph_articulation_inner<const N: usize>(&self) -> ([VectorAddress; N], usize, usize) {
        let mut out_vecs = [VectorAddress::new(0, 0, 0, 0); N];

        // Collect live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        const UNVISITED: u32 = u32::MAX;
        const NO_PAR: usize  = MAX_NODES;

        // Per-slot DFS state.
        let mut disc        = [UNVISITED; MAX_NODES]; // discovery time
        let mut low         = [0u32;      MAX_NODES]; // low-link value
        let mut par         = [NO_PAR;    MAX_NODES]; // DFS parent slot
        let mut dfs_children= [0u8;       MAX_NODES]; // DFS-tree child count (for root check)
        let mut is_ap       = [false;     MAX_NODES]; // articulation point flag

        let mut timer = 0u32;

        // Iterative DFS stack: (slot, edge_scan_start).
        let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if disc[start_slot] != UNVISITED { continue; }

            disc[start_slot] = timer;
            low[start_slot]  = timer;
            timer           += 1;
            dfs_stack[0]     = (start_slot, 0);
            let mut st_top   = 1usize;

            while st_top > 0 {
                let fi = st_top - 1;
                let (cur_slot, scan_ei) = dfs_stack[fi];
                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None    => { st_top -= 1; continue; }
                };

                let mut found = false;
                let mut ei    = scan_ei;

                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };

                    // Treat edge as undirected: take whichever endpoint is the neighbour.
                    let nbr_id = if edge.spec.from_node == cur_id {
                        edge.spec.to_node
                    } else if edge.spec.to_node == cur_id {
                        edge.spec.from_node
                    } else {
                        ei += 1; continue;
                    };

                    let nbr_slot = match self.node_slot_by_id(nbr_id) {
                        Some(s) => s,
                        None    => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; } // self-loop

                    if disc[nbr_slot] == UNVISITED {
                        // Tree edge: push child.
                        disc[nbr_slot]      = timer;
                        low[nbr_slot]       = timer;
                        timer              += 1;
                        par[nbr_slot]       = cur_slot;
                        dfs_children[cur_slot] = dfs_children[cur_slot].saturating_add(1);
                        dfs_stack[fi].1     = ei + 1; // resume after this edge
                        dfs_stack[st_top]   = (nbr_slot, 0);
                        st_top             += 1;
                        found               = true;
                        break;
                    } else if nbr_slot != par[cur_slot] {
                        // Back edge: update low.
                        if disc[nbr_slot] < low[cur_slot] {
                            low[cur_slot] = disc[nbr_slot];
                        }
                    }
                    ei += 1;
                }

                if !found {
                    // Pop: propagate low to parent and check AP condition.
                    st_top -= 1;
                    let p = par[cur_slot];
                    if p != NO_PAR {
                        if low[cur_slot] < low[p] {
                            low[p] = low[cur_slot];
                        }
                        // Non-root AP: low[child] >= disc[parent].
                        if low[cur_slot] >= disc[p] && par[p] != NO_PAR {
                            is_ap[p] = true;
                        }
                    }
                }
            }

            // Root AP: root of a DFS tree with ≥ 2 tree children.
            if dfs_children[start_slot] >= 2 {
                is_ap[start_slot] = true;
            }
        }

        // Collect and sort articulation points by as_u64() ascending.
        let mut art_count = 0usize;
        for ki in 0..node_count {
            let slot = node_slots[ki];
            if is_ap[slot] {
                if let Some(r) = self.nodes[slot] {
                    if art_count < N {
                        out_vecs[art_count] = r.vector;
                        art_count += 1;
                    }
                }
            }
        }
        // Insertion sort ascending by as_u64().
        for i in 1..art_count {
            let mut j = i;
            while j > 0 && out_vecs[j - 1].as_u64() > out_vecs[j].as_u64() {
                out_vecs.swap(j - 1, j);
                j -= 1;
            }
        }

        (out_vecs, art_count, node_count)
    }

    /// V2.86: Iterative Tarjan disc/low-link DFS — bridge (cut-edge) detection.
    ///
    /// Returns (from_vecs, to_vecs, bridge_count, node_count).
    /// A bridge is an edge whose removal increases the number of connected
    /// components.  Bridge condition: low[child] > disc[parent] (strictly >).
    ///
    /// Parent tracking by edge-index (not parent-slot) so that two anti-parallel
    /// directed edges A→B + B→A are treated as one undirected path, not a bridge.
    pub fn graph_bridges_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
        let mut from_vecs = [VectorAddress::new(0, 0, 0, 0); N];
        let mut to_vecs   = [VectorAddress::new(0, 0, 0, 0); N];

        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        const UNVISITED:  u32   = u32::MAX;
        const NO_PAR_EI:  usize = MAX_EDGES;  // sentinel: no parent edge

        let mut disc     = [UNVISITED; MAX_NODES];
        let mut low      = [0u32;      MAX_NODES];
        let mut par_ei   = [NO_PAR_EI; MAX_NODES]; // edge-index we arrived from
        let mut par_slot = [MAX_NODES;  MAX_NODES]; // parent node slot (for emit)

        let mut timer        = 0u32;
        let mut bridge_count = 0usize;

        let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if disc[start_slot] != UNVISITED { continue; }

            disc[start_slot] = timer;
            low[start_slot]  = timer;
            timer           += 1;
            dfs_stack[0]     = (start_slot, 0);
            let mut st_top   = 1usize;

            while st_top > 0 {
                let fi = st_top - 1;
                let (cur_slot, scan_ei) = dfs_stack[fi];
                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None    => { st_top -= 1; continue; }
                };

                let mut found = false;
                let mut ei    = scan_ei;

                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => { ei += 1; continue; }
                    };

                    let nbr_id = if edge.spec.from_node == cur_id {
                        edge.spec.to_node
                    } else if edge.spec.to_node == cur_id {
                        edge.spec.from_node
                    } else {
                        ei += 1; continue;
                    };

                    let nbr_slot = match self.node_slot_by_id(nbr_id) {
                        Some(s) => s,
                        None    => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; } // self-loop

                    // Skip exactly the edge we arrived on (by index, not slot).
                    if ei == par_ei[cur_slot] { ei += 1; continue; }

                    if disc[nbr_slot] == UNVISITED {
                        disc[nbr_slot]     = timer;
                        low[nbr_slot]      = timer;
                        timer             += 1;
                        par_ei[nbr_slot]   = ei;
                        par_slot[nbr_slot] = cur_slot;
                        dfs_stack[fi].1    = ei + 1;
                        dfs_stack[st_top]  = (nbr_slot, 0);
                        st_top            += 1;
                        found              = true;
                        break;
                    } else {
                        if disc[nbr_slot] < low[cur_slot] {
                            low[cur_slot] = disc[nbr_slot];
                        }
                    }
                    ei += 1;
                }

                if !found {
                    st_top -= 1;
                    let p = par_slot[cur_slot];
                    if p != MAX_NODES {
                        if low[cur_slot] < low[p] {
                            low[p] = low[cur_slot];
                        }
                        // Bridge: low[child] > disc[parent] (strictly >)
                        if low[cur_slot] > disc[p] && bridge_count < N {
                            if let (Some(pr), Some(cr)) =
                                (self.nodes[p], self.nodes[cur_slot])
                            {
                                let a = pr.vector;
                                let b = cr.vector;
                                if a.as_u64() <= b.as_u64() {
                                    from_vecs[bridge_count] = a;
                                    to_vecs[bridge_count]   = b;
                                } else {
                                    from_vecs[bridge_count] = b;
                                    to_vecs[bridge_count]   = a;
                                }
                                bridge_count += 1;
                            }
                        }
                    }
                }
            }
        }

        // Insertion sort by (from.as_u64(), to.as_u64()).
        for i in 1..bridge_count {
            let mut j = i;
            while j > 0 {
                let a = (from_vecs[j - 1].as_u64(), to_vecs[j - 1].as_u64());
                let b = (from_vecs[j].as_u64(),     to_vecs[j].as_u64());
                if a > b {
                    from_vecs.swap(j - 1, j);
                    to_vecs.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        (from_vecs, to_vecs, bridge_count, node_count)
    }

    /// V2.87: Eulerian path/circuit detection (directed graph).
    ///
    /// Returns `(has_circuit, has_path, start_vec, end_vec, node_count)`:
    /// - `has_circuit` — Eulerian circuit exists: all edges traversable in a
    ///   closed walk.  Conditions: weakly connected + every node balanced
    ///   (in_degree == out_degree).
    /// - `has_path`    — Eulerian path (non-circuit) exists: exactly one node
    ///   with out-in=1 (start), exactly one with in-out=1 (end), all others
    ///   balanced, and weakly connected.  Mutually exclusive with has_circuit.
    /// - `start_vec`   — start node vector for path; zero when has_circuit or
    ///   neither.
    /// - `end_vec`     — end node vector for path; zero when has_circuit or
    ///   neither.
    /// - `node_count`  — total live nodes.
    ///
    /// Isolated nodes (no edges) are excluded from connectivity and degree
    /// checks (they contribute nothing to any traversal).
    /// Vacuous case (no edges): has_circuit=true (empty walk).
    ///
    /// Algorithm: one edge scan for degrees, one undirected BFS for weak
    /// connectivity.  O(V+E).
    pub fn graph_eulerian_inner(
        &self,
    ) -> (bool, bool, VectorAddress, VectorAddress, usize) {
        let zero = VectorAddress::new(0, 0, 0, 0);

        // Compact live nodes.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Degree census: directed in/out per slot.
        let mut out_deg = [0u16; MAX_NODES];
        let mut in_deg  = [0u16; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if let Some(fs) = self.node_slot_by_id(edge.spec.from_node) {
                out_deg[fs] = out_deg[fs].saturating_add(1);
            }
            if let Some(ts) = self.node_slot_by_id(edge.spec.to_node) {
                in_deg[ts] = in_deg[ts].saturating_add(1);
            }
        }

        // Collect active (non-isolated) nodes.
        let mut active_slots = [0usize; MAX_NODES];
        let mut active_count = 0usize;
        for ki in 0..node_count {
            let s = node_slots[ki];
            if out_deg[s] > 0 || in_deg[s] > 0 {
                active_slots[active_count] = s;
                active_count += 1;
            }
        }

        // Vacuous case: no edges → trivial Eulerian circuit (empty walk).
        if active_count == 0 {
            return (true, false, zero, zero, node_count);
        }

        // Degree-balance check.
        let mut start_slot    = MAX_NODES;
        let mut end_slot      = MAX_NODES;
        let mut imbalanced    = 0usize;
        let mut path_possible = true;

        for ki in 0..active_count {
            let s    = active_slots[ki];
            let diff = (out_deg[s] as i32) - (in_deg[s] as i32);
            match diff {
                0 => {}          // balanced — ok for circuit or path
                1 => {           // potential start of path
                    if start_slot == MAX_NODES { start_slot = s; } else { path_possible = false; }
                    imbalanced += 1;
                }
                -1 => {          // potential end of path
                    if end_slot == MAX_NODES { end_slot = s; } else { path_possible = false; }
                    imbalanced += 1;
                }
                _ => {           // |diff| >= 2 → neither
                    path_possible = false;
                    imbalanced   += 1;
                }
            }
        }

        let circuit_degree_ok = imbalanced == 0;
        let path_degree_ok    = path_possible
            && imbalanced == 2
            && start_slot != MAX_NODES
            && end_slot   != MAX_NODES;

        if !circuit_degree_ok && !path_degree_ok {
            return (false, false, zero, zero, node_count);
        }

        // Weak-connectivity check: undirected BFS from first active node.
        let mut visited   = [false; MAX_NODES];
        let mut bfs_queue = [0usize; MAX_NODES];
        let mut bfs_head  = 0usize;
        let mut bfs_tail  = 0usize;

        let root = active_slots[0];
        visited[root]        = true;
        bfs_queue[bfs_tail]  = root;
        bfs_tail            += 1;

        while bfs_head < bfs_tail {
            let cur_slot = bfs_queue[bfs_head]; bfs_head += 1;
            let cur_id   = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                let nbr_id = if edge.spec.from_node == cur_id {
                    edge.spec.to_node
                } else if edge.spec.to_node == cur_id {
                    edge.spec.from_node
                } else {
                    continue
                };
                let nbr_slot = match self.node_slot_by_id(nbr_id) { Some(s) => s, None => continue };
                if nbr_slot == cur_slot || visited[nbr_slot] { continue; }
                visited[nbr_slot] = true;
                if bfs_tail < MAX_NODES {
                    bfs_queue[bfs_tail] = nbr_slot;
                    bfs_tail           += 1;
                }
            }
        }

        for ki in 0..active_count {
            if !visited[active_slots[ki]] {
                return (false, false, zero, zero, node_count);
            }
        }

        // Connectivity confirmed.
        if circuit_degree_ok {
            return (true, false, zero, zero, node_count);
        }

        let start_vec = self.nodes[start_slot].map(|r| r.vector).unwrap_or(zero);
        let end_vec   = self.nodes[end_slot].map(|r| r.vector).unwrap_or(zero);
        (false, true, start_vec, end_vec, node_count)
    }

    /// V2.88: Longest path (critical path) in a DAG — Kahn BFS + distance DP.
    ///
    /// Returns `(path_hops, is_dag, start_vec, end_vec, node_count)`:
    ///   - `path_hops`  — hop count of the longest directed path (0 when no edges,
    ///                    or when the graph has a directed cycle).
    ///   - `is_dag`     — true iff the directed graph is acyclic (no directed cycles).
    ///                    Self-loops count as cycles (is_dag = false).
    ///   - `start_vec`  — source end of a longest path (zero when no path or cycle).
    ///   - `end_vec`    — sink end of a longest path (zero when no path or cycle).
    ///   - `node_count` — total live nodes.
    ///
    /// Algorithm: Kahn's BFS topological sort with a simultaneous max-distance DP.
    /// O(V + E), no_std safe, fixed stack arrays only.
    ///
    /// Tie-breaking: when multiple paths share the same maximum length, the node
    /// with the smallest slot index is chosen as the end node (deterministic).
    ///
    /// OS analogy: critical path in a kernel boot-dependency graph — the minimum
    /// serial depth that any parallel initialisation must still traverse.
    /// Equivalent to `systemd-analyze critical-chain` for graph-native subsystems.
    pub fn graph_dag_longest_inner(
        &self,
    ) -> (u32, bool, VectorAddress, VectorAddress, usize) {
        let zero = VectorAddress::new(0, 0, 0, 0);

        // Compact live nodes.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        if node_count == 0 {
            return (0, true, zero, zero, 0);
        }

        // Compute in-degrees.  Self-loops (fs==ts) count here so Kahn's BFS
        // can never drain them — a self-loop node stays stuck at in_deg≥1 and
        // is never emitted, causing processed<node_count → is_dag=false.
        let mut in_deg = [0u16; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let _ = fs; // self-loops: fs==ts is intentional
            in_deg[ts] = in_deg[ts].saturating_add(1);
        }

        // Distance and predecessor arrays.
        let mut dist = [0u32; MAX_NODES];  // max hops from any source
        let mut pred = [MAX_NODES; MAX_NODES]; // predecessor slot for path reconstruction

        // Kahn's BFS queue: seed with all in-degree-0 nodes.
        let mut queue  = [0usize; MAX_NODES];
        let mut q_head = 0usize;
        let mut q_tail = 0usize;
        for ki in 0..node_count {
            let s = node_slots[ki];
            if in_deg[s] == 0 {
                queue[q_tail] = s;
                q_tail += 1;
            }
        }

        let mut processed = 0usize;

        while q_head < q_tail {
            let cur_slot = queue[q_head];
            q_head += 1;
            processed += 1;

            let cur_id = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };

            // Relax all outgoing edges.
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_id { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
                if nbr_slot == cur_slot { continue; } // skip self-loops

                let new_dist = dist[cur_slot].saturating_add(1);
                if new_dist > dist[nbr_slot] {
                    dist[nbr_slot] = new_dist;
                    pred[nbr_slot] = cur_slot;
                }

                if in_deg[nbr_slot] > 0 {
                    in_deg[nbr_slot] -= 1;
                    if in_deg[nbr_slot] == 0 && q_tail < MAX_NODES {
                        queue[q_tail] = nbr_slot;
                        q_tail += 1;
                    }
                }
            }
        }

        // If not all nodes processed → cycle exists.
        let is_dag = processed == node_count;
        if !is_dag {
            return (0, false, zero, zero, node_count);
        }

        // Find node with maximum distance (tie-break: smallest slot index).
        let mut max_dist  = 0u32;
        let mut end_slot  = MAX_NODES;
        for ki in 0..node_count {
            let s = node_slots[ki];
            if dist[s] > max_dist {
                max_dist = dist[s];
                end_slot = s;
            }
        }

        // No edges at all → path_hops = 0.
        if max_dist == 0 || end_slot == MAX_NODES {
            return (0, true, zero, zero, node_count);
        }

        // Trace predecessor chain back to the path source.
        let mut cur = end_slot;
        loop {
            let p = pred[cur];
            if p >= MAX_NODES { break; } // cur is the source (no predecessor)
            cur = p;
        }
        let start_slot = cur;

        let start_vec = self.nodes[start_slot].map(|r| r.vector).unwrap_or(zero);
        let end_vec   = self.nodes[end_slot].map(|r| r.vector).unwrap_or(zero);
        (max_dist, true, start_vec, end_vec, node_count)
    }

    /// V2.90: DAG transitive reach — ancestor and descendant counts for each node.
    ///
    /// For every live node v in a DAG:
    ///   - `anc[v]`  = number of nodes that can reach v   (transitive predecessors)
    ///   - `desc[v]` = number of nodes reachable from v   (transitive successors)
    ///
    /// Returns `(vecs, anc_counts, desc_counts, node_count, is_dag)`:
    ///   - `vecs[0..node_count]`       — live node vectors, sorted descending by desc_count.
    ///   - `anc_counts[0..node_count]` — ancestor count per node.
    ///   - `desc_counts[0..node_count]`— descendant count per node.
    ///   - `node_count`                — total live nodes.
    ///   - `is_dag`                    — false iff the graph contains a directed cycle.
    ///
    /// Algorithm: Kahn BFS topological sort → bitset propagation in two passes.
    ///   Pass 1 (reverse topo): desc bitsets propagated from sinks to sources.
    ///   Pass 2 (forward topo): anc bitsets propagated from sources to sinks.
    ///   Each pass is O(V*(V+E)/64); total O(V*(V+E)/64), no_std safe.
    ///
    /// Bitset encoding: one u128 per node (MAX_NODES=128 fits exactly in 128 bits).
    /// Self-loops count toward in-degree, preventing Kahn drain → is_dag=false.
    ///
    /// OS analogy: `systemctl list-dependencies --reverse <unit>` counts how many
    /// transitive dependents a service has (desc); its transitive dependencies (anc).
    pub fn graph_dag_reach_inner<const N: usize>(&self) -> ([VectorAddress; N], [u32; N], [u32; N], usize, bool) {
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut out_vecs = [zero; N];
        let mut out_anc  = [0u32; N];
        let mut out_desc = [0u32; N];

        // Compact live nodes.
        let mut node_slots  = [0usize; MAX_NODES];
        let mut node_count  = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        if node_count == 0 {
            return (out_vecs, out_anc, out_desc, 0, true);
        }

        // Map slot → rank (index within node_slots) for bitset indexing.
        let mut slot_to_rank = [usize::MAX; MAX_NODES];
        for ki in 0..node_count {
            slot_to_rank[node_slots[ki]] = ki;
        }

        // In-degrees: self-loops included so Kahn BFS can't drain them → cycle detected.
        let mut in_deg = [0u16; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
            in_deg[ts] = in_deg[ts].saturating_add(1);
        }

        // Kahn BFS → topological order in `topo_order`.
        let mut topo_order = [0usize; MAX_NODES];
        let mut queue      = [0usize; MAX_NODES];
        let mut q_head     = 0usize;
        let mut q_tail     = 0usize;
        let mut topo_len   = 0usize;

        for ki in 0..node_count {
            let s = node_slots[ki];
            if in_deg[s] == 0 {
                queue[q_tail] = s;
                q_tail += 1;
            }
        }

        while q_head < q_tail {
            let cur_slot = queue[q_head];
            q_head += 1;
            topo_order[topo_len] = cur_slot;
            topo_len += 1;

            let cur_id = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_id { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
                if nbr_slot == cur_slot { continue; } // self-loops already accounted in in_deg
                if in_deg[nbr_slot] > 0 {
                    in_deg[nbr_slot] -= 1;
                    if in_deg[nbr_slot] == 0 && q_tail < MAX_NODES {
                        queue[q_tail] = nbr_slot;
                        q_tail += 1;
                    }
                }
            }
        }

        let is_dag = topo_len == node_count;
        if !is_dag {
            return (out_vecs, out_anc, out_desc, node_count, false);
        }

        // Bitset propagation.
        // reach_from[slot]: u128 bitset where bit k=1 means rank-k node is reachable FROM slot.
        // reach_to[slot]:   u128 bitset where bit k=1 means rank-k node can reach slot.
        let mut reach_from = [0u128; MAX_NODES];
        let mut reach_to   = [0u128; MAX_NODES];

        // Pass 1 — reverse topological order: propagate descendant bitsets (sinks → sources).
        // When we process cur_slot, all its successors are already processed (they appeared
        // later in topological order, hence earlier in reverse).
        let mut ki = topo_len;
        while ki > 0 {
            ki -= 1;
            let cur_slot = topo_order[ki];
            let cur_id   = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_id { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
                if nbr_slot == cur_slot { continue; }
                let rank_nbr = slot_to_rank[nbr_slot];
                if rank_nbr >= MAX_NODES { continue; }
                // cur_slot can reach nbr_slot and everything nbr_slot can reach.
                reach_from[cur_slot] |= (1u128 << rank_nbr) | reach_from[nbr_slot];
            }
        }

        // Pass 2 — forward topological order: propagate ancestor bitsets (sources → sinks).
        // When we process cur_slot, all its predecessors' reach_to are already finalised.
        for ki in 0..topo_len {
            let cur_slot = topo_order[ki];
            let cur_id   = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };
            let rank_cur = slot_to_rank[cur_slot];
            if rank_cur >= MAX_NODES { continue; }
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_id { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
                if nbr_slot == cur_slot { continue; }
                // cur_slot is a direct ancestor of nbr_slot; all of cur_slot's ancestors too.
                reach_to[nbr_slot] |= (1u128 << rank_cur) | reach_to[cur_slot];
            }
        }

        // Build output: sort by descending desc_count; ties broken by ascending as_u64().
        let mut order = [0usize; MAX_NODES];
        for ki in 0..node_count { order[ki] = node_slots[ki]; }
        for i in 1..node_count {
            let mut j = i;
            while j > 0 {
                let a = order[j - 1];
                let b = order[j];
                let da = reach_from[a].count_ones();
                let db = reach_from[b].count_ones();
                let va = self.nodes[a].map(|r| r.vector.as_u64()).unwrap_or(0);
                let vb = self.nodes[b].map(|r| r.vector.as_u64()).unwrap_or(0);
                if da < db || (da == db && va > vb) {
                    order.swap(j, j - 1);
                    j -= 1;
                } else { break; }
            }
        }

        let n_out = node_count.min(N);
        for i in 0..n_out {
            let s = order[i];
            out_vecs[i] = self.nodes[s].map(|r| r.vector).unwrap_or(zero);
            out_anc[i]  = reach_to[s].count_ones();
            out_desc[i] = reach_from[s].count_ones();
        }

        (out_vecs, out_anc, out_desc, node_count, true)
    }

    pub fn graph_dag_layers_inner<const N: usize>(&self) -> ([VectorAddress; N], [u32; N], usize, u32, bool) {
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut out_vecs   = [zero; N];
        let mut out_layers = [0u32; N];

        // Compact live nodes.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        if node_count == 0 {
            return (out_vecs, out_layers, 0, 0, true);
        }

        // Compute in-degrees.  Self-loops count → those nodes never reach in_deg=0
        // in Kahn's BFS, so if processed < node_count after BFS → is_dag = false.
        let mut in_deg = [0u16; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
            in_deg[ts] = in_deg[ts].saturating_add(1);
        }

        // layer[slot] = BFS layer from all in-degree-0 sources simultaneously.
        let mut layer = [0u32; MAX_NODES];
        // u32::MAX means the node was never reached (cycle member).
        for ki in 0..node_count { layer[node_slots[ki]] = u32::MAX; }

        // Kahn's BFS: seed all in-degree-0 nodes at layer 0.
        let mut queue  = [0usize; MAX_NODES];
        let mut q_head = 0usize;
        let mut q_tail = 0usize;
        for ki in 0..node_count {
            let s = node_slots[ki];
            if in_deg[s] == 0 {
                layer[s] = 0;
                queue[q_tail] = s;
                q_tail += 1;
            }
        }

        let mut processed = 0usize;
        let mut max_layer = 0u32;

        while q_head < q_tail {
            let cur_slot = queue[q_head];
            q_head += 1;
            processed += 1;
            let cur_id    = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };
            let cur_layer = layer[cur_slot];

            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_id { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
                if nbr_slot == cur_slot { continue; } // skip self-loops in relaxation

                if in_deg[nbr_slot] > 0 {
                    in_deg[nbr_slot] -= 1;
                    // Propagate max layer: nbr_layer = max(nbr_layer, cur_layer + 1).
                    let new_layer = cur_layer.saturating_add(1);
                    if layer[nbr_slot] == u32::MAX || new_layer > layer[nbr_slot] {
                        layer[nbr_slot] = new_layer;
                    }
                    if in_deg[nbr_slot] == 0 && q_tail < MAX_NODES {
                        if layer[nbr_slot] > max_layer { max_layer = layer[nbr_slot]; }
                        queue[q_tail] = nbr_slot;
                        q_tail += 1;
                    }
                }
            }
        }

        // Cycle check.
        let is_dag = processed == node_count;
        if !is_dag {
            return (out_vecs, out_layers, node_count, 0, false);
        }

        // Sort by ascending layer (then by as_u64() within a layer) for stable output.
        let mut order = [0usize; MAX_NODES];
        for ki in 0..node_count { order[ki] = node_slots[ki]; }
        for i in 1..node_count {
            let mut j = i;
            while j > 0 {
                let a = order[j - 1];
                let b = order[j];
                let la = layer[a];
                let lb = layer[b];
                let va = self.nodes[a].map(|r| r.vector.as_u64()).unwrap_or(0);
                let vb = self.nodes[b].map(|r| r.vector.as_u64()).unwrap_or(0);
                if la > lb || (la == lb && va > vb) {
                    order.swap(j, j - 1);
                    j -= 1;
                } else { break; }
            }
        }

        let n_out = node_count.min(N);
        for i in 0..n_out {
            let s = order[i];
            out_vecs[i]   = self.nodes[s].map(|r| r.vector).unwrap_or(zero);
            out_layers[i] = layer[s];
        }

        let layer_count = max_layer.saturating_add(1); // 0-indexed → count
        (out_vecs, out_layers, node_count, layer_count, true)
    }

    /// V2.90: Cooper, Harvey & Kennedy 2001 simple iterative dominator algorithm.
    /// Computes the immediate dominator (idom) of every node reachable from `start`.
    /// Node D dominates node N when every directed path from `start` to N passes
    /// through D.  The immediate dominator is the closest such D ≠ N.
    ///
    /// Returns `(vecs, idoms, node_count, reachable_count)`:
    ///   - `vecs[0..reachable_count]`  — reachable nodes in RPO order.
    ///   - `idoms[0..reachable_count]` — immediate dominator vector per node.
    ///   - For `start`: `idoms[i] == vecs[i]` (start dominates itself).
    ///   - `node_count`      — total live nodes.
    ///   - `reachable_count` — nodes reachable from `start` (including start).
    pub fn graph_domtree_inner<const N: usize>(
        &self,
        start: VectorAddress,
    ) -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
        const UNDEF: usize = usize::MAX;
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut out_vecs  = [zero; N];
        let mut out_idoms = [zero; N];

        // Compact live nodes.
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() { node_count += 1; }
        }
        if node_count == 0 {
            return (out_vecs, out_idoms, 0, 0);
        }

        let start_slot = match self.node_slot_by_vec(start) {
            Some(s) => s,
            None => return (out_vecs, out_idoms, node_count, 0),
        };

        // --- Step 1: iterative DFS post-order → reverse = RPO. ---
        let mut rpo_slots = [0usize; MAX_NODES]; // rpo_slots[rpo_i] = slot
        let mut rpo_num   = [UNDEF;  MAX_NODES]; // rpo_num[slot]    = rpo_i

        let mut stk_slot   = [0usize; MAX_NODES];
        let mut stk_cursor = [0usize; MAX_NODES]; // next edge-array index to check
        let mut stk_depth  = 1usize;
        let mut visited    = [false; MAX_NODES];
        let mut post_buf   = [0usize; MAX_NODES]; // post-order sequence
        let mut post_count = 0usize;

        stk_slot[0]         = start_slot;
        stk_cursor[0]       = 0;
        visited[start_slot] = true;

        'dfs: while stk_depth > 0 {
            let cur    = stk_slot[stk_depth - 1];
            let cur_id = match self.nodes[cur] {
                Some(r) => r.spec.node_id,
                None    => { stk_depth -= 1; continue; }
            };
            let mut ei = stk_cursor[stk_depth - 1];
            while ei < MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                if edge.spec.from_node == cur_id {
                    let nbr = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(s) => s,
                        None    => { ei += 1; continue; }
                    };
                    if nbr != cur && !visited[nbr] {
                        stk_cursor[stk_depth - 1] = ei + 1;
                        visited[nbr]              = true;
                        stk_slot[stk_depth]       = nbr;
                        stk_cursor[stk_depth]     = 0;
                        stk_depth                += 1;
                        continue 'dfs;
                    }
                }
                ei += 1;
            }
            // All successors visited — emit in post-order then pop.
            if post_count < MAX_NODES {
                post_buf[post_count] = cur;
                post_count          += 1;
            }
            stk_depth -= 1;
        }

        // Reverse post-order.
        let rpo_count = post_count;
        for i in 0..rpo_count {
            let s       = post_buf[rpo_count - 1 - i];
            rpo_slots[i] = s;
            rpo_num[s]   = i;
        }

        // --- Step 2: idom initialisation. ---
        let mut idom = [UNDEF; MAX_NODES]; // idom[slot] = immediate-dominator slot
        idom[start_slot] = start_slot;

        // --- Step 3: iterate until convergence (Cooper et al. 2001). ---
        let mut changed = true;
        while changed {
            changed = false;
            for rpo_i in 1..rpo_count { // skip start (position 0)
                let b    = rpo_slots[rpo_i];
                let b_id = match self.nodes[b] { Some(r) => r.spec.node_id, None => continue };
                let mut new_idom = UNDEF;

                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => continue };
                    if edge.spec.to_node != b_id { continue; }
                    let p = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
                    if p == b { continue; }          // skip self-loops
                    if idom[p] == UNDEF { continue; } // predecessor not yet processed

                    if new_idom == UNDEF {
                        new_idom = p;
                    } else {
                        // Lattice join: walk up dominator chains to find LCA.
                        let mut a  = p;
                        let mut bb = new_idom;
                        let mut guard = 0usize;
                        while a != bb && guard < MAX_NODES * 2 {
                            while a != UNDEF && bb != UNDEF && rpo_num[a] != UNDEF && rpo_num[bb] != UNDEF && rpo_num[a] > rpo_num[bb] {
                                a = idom[a];
                            }
                            if a == UNDEF { break; }
                            while a != UNDEF && bb != UNDEF && rpo_num[a] != UNDEF && rpo_num[bb] != UNDEF && rpo_num[bb] > rpo_num[a] {
                                bb = idom[bb];
                            }
                            if bb == UNDEF { break; }
                            guard += 1;
                        }
                        if a != UNDEF && bb != UNDEF && a == bb {
                            new_idom = a;
                        }
                    }
                }

                if new_idom != UNDEF && idom[b] != new_idom {
                    idom[b] = new_idom;
                    changed  = true;
                }
            }
        }

        // --- Step 4: pack results in RPO order. ---
        let reachable_count = (0..rpo_count).filter(|&i| idom[rpo_slots[i]] != UNDEF).count();
        let n_out = reachable_count.min(N);
        let mut out_i = 0usize;
        for rpo_i in 0..rpo_count {
            if out_i >= n_out { break; }
            let s = rpo_slots[rpo_i];
            if idom[s] == UNDEF { continue; }
            out_vecs[out_i]  = self.nodes[s].map(|r| r.vector).unwrap_or(zero);
            let id_s = idom[s];
            out_idoms[out_i] = self.nodes[id_s].map(|r| r.vector).unwrap_or(zero);
            out_i += 1;
        }

        (out_vecs, out_idoms, node_count, reachable_count)
    }

    /// V2.91: Directed back-edges (feedback arc set) via iterative DFS 3-coloring.
    ///
    /// Returns (from_vecs, to_vecs, arc_count, node_count).
    /// Each entry is one directed edge (from → to) whose presence creates a cycle.
    /// Removing these arcs leaves the graph acyclic (a DAG).
    ///
    /// Algorithm: iterative DFS with UNVISITED/IN_STACK/DONE coloring, O(V+E).
    /// A back-edge is any directed edge (u→v) where v is currently on the DFS
    /// stack (color = IN_STACK / "gray").  Self-loops (u→u) are included: they
    /// are trivially cyclic and are caught by the same IN_STACK check.
    /// Cross/forward edges (v = DONE) are not feedback arcs and are skipped.
    ///
    /// The result is a valid FAS (not necessarily minimum — the min-FAS problem
    /// is NP-hard in general).  DFS-based FAS is the standard O(V+E) approximation.
    /// Output sorted ascending by (from.as_u64(), to.as_u64()) for determinism.
    pub fn graph_feedback_arc_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut from_vecs = [zero; N];
        let mut to_vecs   = [zero; N];

        // Compact live nodes.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // 3-colour DFS: UNVISITED → IN_STACK (gray) → DONE (black).
        const UNVISITED: u8 = 0;
        const IN_STACK:  u8 = 1;
        const DONE:      u8 = 2;

        let mut color     = [UNVISITED; MAX_NODES];
        let mut arc_count = 0usize;

        // Iterative DFS stack: (slot, next_edge_index_to_scan).
        let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if color[start_slot] != UNVISITED { continue; }

            color[start_slot] = IN_STACK;
            dfs_stack[0] = (start_slot, 0);
            let mut st_top = 1usize;

            while st_top > 0 {
                let fi = st_top - 1;
                let (cur_slot, scan_ei) = dfs_stack[fi];
                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None    => { color[cur_slot] = DONE; st_top -= 1; continue; }
                };

                let mut found_child = false;
                let mut ei          = scan_ei;

                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                    if edge.spec.from_node != cur_id { ei += 1; continue; }

                    let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(s) => s,
                        None    => { ei += 1; continue; }
                    };

                    // Self-loops (nbr_slot == cur_slot): cur_slot is IN_STACK, so they
                    // naturally fall into the IN_STACK arm below and are recorded as arcs.

                    match color[nbr_slot] {
                        UNVISITED => {
                            // Tree edge: push neighbour and resume cur after this edge.
                            color[nbr_slot]   = IN_STACK;
                            dfs_stack[fi].1   = ei + 1;
                            dfs_stack[st_top] = (nbr_slot, 0);
                            st_top           += 1;
                            found_child       = true;
                            break;
                        }
                        IN_STACK => {
                            // Back-edge: directed cycle detected — record as feedback arc.
                            if arc_count < N {
                                if let (Some(rf), Some(rt)) = (
                                    self.nodes[cur_slot],
                                    self.nodes[nbr_slot],
                                ) {
                                    from_vecs[arc_count] = rf.vector;
                                    to_vecs[arc_count]   = rt.vector;
                                    arc_count           += 1;
                                }
                            }
                        }
                        _ => {} // DONE: forward/cross edge — not a back-edge.
                    }
                    ei += 1;
                }

                if !found_child {
                    color[cur_slot] = DONE;
                    st_top -= 1;
                }
            }
        }

        // Sort ascending by (from.as_u64(), to.as_u64()) for determinism.
        for i in 1..arc_count {
            let mut j = i;
            while j > 0
                && (from_vecs[j - 1].as_u64(), to_vecs[j - 1].as_u64())
                   > (from_vecs[j].as_u64(), to_vecs[j].as_u64())
            {
                from_vecs.swap(j - 1, j);
                to_vecs.swap(j - 1, j);
                j -= 1;
            }
        }

        (from_vecs, to_vecs, arc_count, node_count)
    }

    /// V2.92: Maximum bipartite matching — Kuhn's iterative DFS augmenting paths, O(V·E).
    ///
    /// Returns `(left_vecs, right_vecs, match_count, is_bipartite, node_count)`:
    /// - `left_vecs[0..match_count]`  — matched side-A (color 0) nodes.
    /// - `right_vecs[0..match_count]` — matched side-B (color 1) nodes.
    /// - `match_count`                — maximum matching size; 0 if not bipartite.
    /// - `is_bipartite`               — false if an odd-length cycle was detected.
    /// - `node_count`                 — total live nodes.
    ///
    /// Algorithm: BFS 2-colouring to build bipartition, then for each free
    /// side-A node an iterative DFS finds an augmenting alternating path and
    /// updates the matching in-place.  Vertex-disjoint augmenting paths are
    /// guaranteed by the `visited_b` mask reset between free-node attempts.
    pub fn graph_bipartite_match_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [VectorAddress; N], usize, bool, usize) {
        const NIL: usize = usize::MAX;

        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut left_vecs  = [zero; N];
        let mut right_vecs = [zero; N];

        // Compact live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // ── Step 1: BFS 2-colouring (undirected) ────────────────────────────
        const UNCOLORED: u8 = u8::MAX;
        let mut slot_color = [UNCOLORED; MAX_NODES];
        let mut is_bipartite = true;
        {
            let mut q  = [0usize; MAX_NODES];
            let mut qh = 0usize;
            let mut qt = 0usize;

            'outer: for ki in 0..node_count {
                let start = node_slots[ki];
                if slot_color[start] != UNCOLORED { continue; }

                slot_color[start] = 0;
                q[qt] = start;
                qt += 1;

                while qh < qt {
                    let cur = q[qh];
                    qh += 1;
                    let cur_color  = slot_color[cur];
                    let next_color = 1 - cur_color;
                    let cur_id = match self.nodes[cur] { Some(r) => r.spec.node_id, None => continue };

                    for ei in 0..MAX_EDGES {
                        let edge = match self.edges[ei] { Some(e) => e, None => continue };
                        let nbr_id = if edge.spec.from_node == cur_id {
                            edge.spec.to_node
                        } else if edge.spec.to_node == cur_id {
                            edge.spec.from_node
                        } else {
                            continue
                        };
                        let nbr = match self.node_slot_by_id(nbr_id) { Some(s) => s, None => continue };
                        if nbr == cur { continue; } // self-loop
                        if slot_color[nbr] == UNCOLORED {
                            slot_color[nbr] = next_color;
                            if qt < MAX_NODES { q[qt] = nbr; qt += 1; }
                        } else if slot_color[nbr] == cur_color {
                            is_bipartite = false;
                            break 'outer;
                        }
                    }
                }
            }
        }

        if !is_bipartite {
            return (left_vecs, right_vecs, 0, false, node_count);
        }

        // ── Step 2: Kuhn's augmenting-path matching ──────────────────────────
        // match_a[slot] = matched B-slot for A-nodes (color 0); NIL if free.
        // match_b[slot] = matched A-slot for B-nodes (color 1); NIL if free.
        let mut match_a = [NIL; MAX_NODES];
        let mut match_b = [NIL; MAX_NODES];
        let mut match_count = 0usize;

        for ki in 0..node_count {
            let start_a = node_slots[ki];
            if slot_color[start_a] != 0 { continue; }  // only side-A nodes
            if match_a[start_a] != NIL  { continue; }  // already matched

            // visited_b[b]: true once this B-node has been explored in this DFS.
            let mut visited_b = [false; MAX_NODES];

            // Iterative DFS stack: (a_slot, edge_scan_start).
            let mut dfs_stk:  [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];
            // chosen_b[level]: the B-slot chosen at this DFS level (to reconstruct path).
            let mut chosen_b: [usize; MAX_NODES] = [NIL; MAX_NODES];
            let mut st_top = 1usize;
            dfs_stk[0] = (start_a, 0);
            let mut augmented = false;

            'dfs: while st_top > 0 {
                let lvl = st_top - 1;
                let (a_slot, mut ei) = dfs_stk[lvl];
                let a_id = match self.nodes[a_slot] {
                    Some(r) => r.spec.node_id,
                    None    => { st_top -= 1; continue; }
                };

                let mut found_next = false;

                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] { Some(e) => e, None => { ei += 1; continue; } };
                    let nbr_id = if edge.spec.from_node == a_id {
                        edge.spec.to_node
                    } else if edge.spec.to_node == a_id {
                        edge.spec.from_node
                    } else {
                        ei += 1; continue;
                    };
                    let b_slot = match self.node_slot_by_id(nbr_id) { Some(s) => s, None => { ei += 1; continue; } };
                    if b_slot == a_slot       { ei += 1; continue; } // self-loop
                    if slot_color[b_slot] != 1 { ei += 1; continue; } // only B-nodes
                    if visited_b[b_slot]      { ei += 1; continue; } // already tried
                    visited_b[b_slot] = true;
                    ei += 1;

                    if match_b[b_slot] == NIL {
                        // Free B-node: augment path from level 0 up to lvl.
                        chosen_b[lvl] = b_slot;
                        let mut cur_b  = b_slot;
                        let mut cur_lv = lvl;
                        loop {
                            let cur_a = dfs_stk[cur_lv].0;
                            match_a[cur_a] = cur_b;
                            match_b[cur_b] = cur_a;
                            if cur_lv == 0 { break; }
                            cur_lv -= 1;
                            cur_b = chosen_b[cur_lv];
                        }
                        augmented = true;
                        match_count += 1;
                        break 'dfs;
                    } else {
                        // Matched B-node: push its matched A-node onto DFS stack.
                        let next_a = match_b[b_slot];
                        chosen_b[lvl]     = b_slot;
                        dfs_stk[lvl].1    = ei; // save scan position for backtrack
                        if st_top < MAX_NODES {
                            dfs_stk[st_top] = (next_a, 0);
                            st_top += 1;
                        }
                        found_next = true;
                        break;
                    }
                }

                if !found_next && !augmented {
                    // No viable path through this A-node: backtrack.
                    st_top -= 1;
                }
            }
            let _ = augmented; // suppress unused warning
        }

        // ── Step 3: collect matched pairs (sorted by left vec for determinism) ─
        let mut out_count = 0usize;
        for ki in 0..node_count {
            let a_slot = node_slots[ki];
            if slot_color[a_slot] != 0 { continue; }
            let b_slot = match_a[a_slot];
            if b_slot == NIL { continue; }
            if out_count < N {
                if let (Some(a_rec), Some(b_rec)) = (self.nodes[a_slot], self.nodes[b_slot]) {
                    left_vecs[out_count]  = a_rec.vector;
                    right_vecs[out_count] = b_rec.vector;
                    out_count += 1;
                }
            }
        }

        (left_vecs, right_vecs, match_count, true, node_count)
    }

    /// V2.93: 2-edge-connected components (2ECCs) of the live kernel graph.
    ///
    /// Two nodes u, v are in the same 2ECC iff there exist ≥2 edge-disjoint
    /// paths between them (i.e., no single edge removal can disconnect them).
    /// Every vertex belongs to exactly one 2ECC; bridges are the inter-component
    /// boundary edges.
    ///
    /// Returns `(vecs, comp_ids, node_count, comp_count)`:
    ///   - `vecs[0..node_count]`     — live nodes sorted by (comp_id, as_u64()).
    ///   - `comp_ids[0..node_count]` — 0-indexed 2ECC ID for each node.
    ///   - `node_count`              — total live nodes.
    ///   - `comp_count`              — number of distinct 2ECCs.
    ///
    /// Algorithm: Phase 1 — Tarjan low-link bridge detection on the undirected
    /// projection, O(V+E).  Phase 2 — BFS ignoring bridge edges to label each
    /// connected component, O(V+E).  Total O(V+E), no_std safe.
    ///
    /// Self-loops are ignored (they can never be bridges and don't affect
    /// 2ECC membership).  A single isolated node forms its own singleton 2ECC.
    ///
    /// OS analogy: groups of kernel subsystems where any single IPC link can
    /// fail without partitioning the group — analogous to bonded network
    /// interfaces or redundant bus paths in a fault-tolerant system fabric.
    pub fn graph_2ecc_inner<const N: usize>(&self) -> ([VectorAddress; N], [u8; N], usize, usize) {
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut vecs     = [zero; N];
        let mut comp_ids = [0u8;  N];

        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        if node_count == 0 {
            return (vecs, comp_ids, 0, 0);
        }

        // ── Phase 1: Tarjan bridge-finding (undirected projection) ──────────
        const UNVISITED: u32   = u32::MAX;
        const NO_PAR_EI: usize = MAX_EDGES;

        let mut disc      = [UNVISITED; MAX_NODES];
        let mut low       = [0u32;      MAX_NODES];
        let mut par_ei    = [NO_PAR_EI; MAX_NODES]; // edge we arrived on
        let mut par_slot  = [MAX_NODES;  MAX_NODES]; // parent node slot
        let mut is_bridge = [false;     MAX_EDGES];  // edge-index → bridge?

        let mut timer = 0u32;
        let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if disc[start_slot] != UNVISITED { continue; }

            disc[start_slot] = timer;
            low[start_slot]  = timer;
            timer           += 1;
            dfs_stack[0]     = (start_slot, 0);
            let mut st_top   = 1usize;

            while st_top > 0 {
                let fi = st_top - 1;
                let (cur_slot, scan_ei) = dfs_stack[fi];
                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None    => { st_top -= 1; continue; }
                };

                let mut found = false;
                let mut ei    = scan_ei;

                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => { ei += 1; continue; }
                    };

                    let nbr_id = if edge.spec.from_node == cur_id {
                        edge.spec.to_node
                    } else if edge.spec.to_node == cur_id {
                        edge.spec.from_node
                    } else {
                        ei += 1; continue;
                    };

                    let nbr_slot = match self.node_slot_by_id(nbr_id) {
                        Some(s) => s,
                        None    => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; } // self-loop
                    if ei == par_ei[cur_slot] { ei += 1; continue; } // parent edge

                    if disc[nbr_slot] == UNVISITED {
                        disc[nbr_slot]     = timer;
                        low[nbr_slot]      = timer;
                        timer             += 1;
                        par_ei[nbr_slot]   = ei;
                        par_slot[nbr_slot] = cur_slot;
                        dfs_stack[fi].1    = ei + 1;
                        dfs_stack[st_top]  = (nbr_slot, 0);
                        st_top            += 1;
                        found              = true;
                        break;
                    } else {
                        if disc[nbr_slot] < low[cur_slot] {
                            low[cur_slot] = disc[nbr_slot];
                        }
                        ei += 1;
                    }
                }

                if !found {
                    st_top -= 1;
                    let p = par_slot[cur_slot];
                    if p != MAX_NODES {
                        if low[cur_slot] < low[p] {
                            low[p] = low[cur_slot];
                        }
                        // Bridge: low[child] > disc[parent] (strictly >)
                        if low[cur_slot] > disc[p] {
                            let ei_b = par_ei[cur_slot];
                            if ei_b < MAX_EDGES {
                                is_bridge[ei_b] = true;
                            }
                        }
                    }
                }
            }
        }

        // ── Phase 2: BFS on non-bridge undirected edges ─────────────────────
        let mut comp_slot: [u8; MAX_NODES]    = [u8::MAX; MAX_NODES];
        let mut bfs_queue: [usize; MAX_NODES] = [0;       MAX_NODES];
        let mut comp_count = 0usize;

        for ki in 0..node_count {
            let start_slot = node_slots[ki];
            if comp_slot[start_slot] != u8::MAX { continue; }

            let cid = comp_count.min(254) as u8;
            comp_count += 1;
            comp_slot[start_slot] = cid;

            bfs_queue[0] = start_slot;
            let mut bfs_head = 0usize;
            let mut bfs_tail = 1usize;

            while bfs_head < bfs_tail {
                let cur_slot = bfs_queue[bfs_head];
                bfs_head += 1;

                let cur_id = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                for ei in 0..MAX_EDGES {
                    if is_bridge[ei] { continue; }
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => continue,
                    };

                    let nbr_id = if edge.spec.from_node == cur_id {
                        edge.spec.to_node
                    } else if edge.spec.to_node == cur_id {
                        edge.spec.from_node
                    } else {
                        continue;
                    };

                    let nbr_slot = match self.node_slot_by_id(nbr_id) {
                        Some(s) => s,
                        None    => continue,
                    };
                    if nbr_slot == cur_slot { continue; } // self-loop

                    if comp_slot[nbr_slot] == u8::MAX {
                        comp_slot[nbr_slot] = cid;
                        if bfs_tail < MAX_NODES {
                            bfs_queue[bfs_tail] = nbr_slot;
                            bfs_tail += 1;
                        }
                    }
                }
            }
        }

        // ── Phase 3: build output sorted by (comp_id, vecs.as_u64()) ────────
        let out_count = node_count.min(N);
        for ki in 0..out_count {
            let slot = node_slots[ki];
            if let Some(node) = self.nodes[slot] {
                vecs[ki]     = node.vector;
                comp_ids[ki] = comp_slot[slot];
            }
        }

        for i in 1..out_count {
            let mut j = i;
            while j > 0 {
                let a = (comp_ids[j - 1], vecs[j - 1].as_u64());
                let b = (comp_ids[j],     vecs[j].as_u64());
                if a > b {
                    comp_ids.swap(j - 1, j);
                    vecs.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        (vecs, comp_ids, node_count, comp_count)
    }

    /// V2.94: k-truss decomposition (edge-peeling, Wang & Cheng 2012 simplified).
    ///
    /// Assigns each live node a "trussness" = max k such that the node has an
    /// incident edge in the k-truss.  The k-truss is the maximal subgraph where
    /// every edge participates in ≥ k−2 triangles **within** the subgraph.
    ///
    /// Strictly finer than k-core: every k-truss ⊆ (k−1)-core.
    /// Isolated nodes (no edges) → trussness = 0.
    /// Any edge (no triangles) → trussness = 2.
    /// Triangle edge → trussness ≥ 3 (≥ 1 triangle).
    ///
    /// Algorithm:
    ///   1. Build undirected deduped edge list.
    ///   2. For each edge (u,v) compute support = |N(u) ∩ N(v)| (triangle count).
    ///   3. For k = 3, 4, …: iteratively remove active edges with support < k−2,
    ///      decrementing support of surviving edges that shared a triangle.
    ///   4. Edges that survive all rounds get trussness = k_final.
    ///      Edges removed at round k get trussness = k−1.
    ///   5. Node trussness = max trussness of incident edges.
    ///
    /// Returns (vecs, trussness, node_count, max_trussness):
    ///   vecs[0..n]      — nodes sorted trussness-descending, then as_u64 ascending
    ///   trussness[0..n] — per-node trussness (0 = isolated)
    ///   node_count      — total live nodes
    ///   max_trussness   — graph truss number
    pub fn graph_truss_inner<const N: usize>(&self) -> ([VectorAddress; N], [u8; N], usize, u8) {
        const ZERO: VectorAddress = VectorAddress::new(0, 0, 0, 0);

        // Collect live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[nc] = i;
                nc += 1;
            }
        }
        if nc == 0 {
            return ([ZERO; N], [0u8; N], 0, 0);
        }

        // Build undirected edge list: eu[i] = lower slot, ev[i] = higher slot.
        let mut eu = [0u8; MAX_EDGES];
        let mut ev = [0u8; MAX_EDGES];
        let mut ec = 0usize;

        'edge_scan: for i in 0..MAX_EDGES {
            let edge = match self.edges[i] {
                Some(e) => e,
                None => continue,
            };
            let us = match self.node_slot_by_id(edge.spec.from_node) {
                Some(s) => s,
                None => continue,
            };
            let vs = match self.node_slot_by_id(edge.spec.to_node) {
                Some(s) => s,
                None => continue,
            };
            if us == vs { continue; } // self-loop
            let (a, b) = if us < vs {
                (us as u8, vs as u8)
            } else {
                (vs as u8, us as u8)
            };
            // Dedup: skip if already added.
            for k in 0..ec {
                if eu[k] == a && ev[k] == b { continue 'edge_scan; }
            }
            if ec < MAX_EDGES {
                eu[ec] = a;
                ev[ec] = b;
                ec += 1;
            }
        }

        // Compute initial triangle support: support[ei] = |N(a) ∩ N(b)|.
        let mut sup    = [0u8; MAX_EDGES];
        let mut active = [true; MAX_EDGES];

        for ei in 0..ec {
            let a = eu[ei] as usize;
            let b = ev[ei] as usize;
            let mut cnt = 0u8;
            // For each edge ej incident to a (other than ei), get other endpoint w.
            for ej in 0..ec {
                if ej == ei { continue; }
                let x = eu[ej] as usize;
                let y = ev[ej] as usize;
                let w = if x == a { y } else if y == a { x } else { continue };
                if w == b { continue; }
                // Check if (b, w) is also in the edge list.
                for ek in 0..ec {
                    if ek == ei { continue; }
                    let px = eu[ek] as usize;
                    let py = ev[ek] as usize;
                    if (px == b && py == w) || (px == w && py == b) {
                        cnt = cnt.saturating_add(1);
                        break;
                    }
                }
            }
            sup[ei] = cnt;
        }

        // Truss peeling.
        let mut edge_truss = [2u8; MAX_EDGES]; // baseline: every edge is in 2-truss

        let mut k = 3u32;
        loop {
            let thresh = (k - 2) as u8;
            // Cascade: remove all active edges with support < thresh.
            let mut any_removed = true;
            while any_removed {
                any_removed = false;
                for ei in 0..ec {
                    if !active[ei] { continue; }
                    if sup[ei] >= thresh { continue; }

                    active[ei] = false;
                    edge_truss[ei] = (k - 1) as u8;
                    any_removed = true;

                    // Decrement support of edges sharing a triangle with (a, b).
                    let a = eu[ei] as usize;
                    let b = ev[ei] as usize;
                    for ej in 0..ec {
                        if !active[ej] { continue; }
                        let x = eu[ej] as usize;
                        let y = ev[ej] as usize;
                        let w = if x == a { y } else if y == a { x } else { continue };
                        if w == b { continue; }
                        // (a, w) is edge ej; find active edge (b, w).
                        for bwk in 0..ec {
                            if !active[bwk] { continue; }
                            let px = eu[bwk] as usize;
                            let py = ev[bwk] as usize;
                            if (px == b && py == w) || (px == w && py == b) {
                                if sup[ej]  > 0 { sup[ej]  -= 1; }
                                if sup[bwk] > 0 { sup[bwk] -= 1; }
                                break;
                            }
                        }
                    }
                }
            }
            // All remaining active edges have support >= thresh → they are in k-truss.
            let has_active = {
                let mut found = false;
                for i in 0..ec { if active[i] { found = true; break; } }
                found
            };
            if !has_active { break; }
            if k >= 254 { break; }
            // Mark remaining edges with current k (may be updated later).
            for ei in 0..ec {
                if active[ei] { edge_truss[ei] = k as u8; }
            }
            k += 1;
        }

        // Per-node trussness = max over incident edge trussness values.
        let mut node_truss = [0u8; MAX_NODES];
        for ei in 0..ec {
            let t = edge_truss[ei];
            let a = eu[ei] as usize;
            let b = ev[ei] as usize;
            if t > node_truss[a] { node_truss[a] = t; }
            if t > node_truss[b] { node_truss[b] = t; }
        }

        // Sort: trussness descending, then as_u64 ascending (stable tie-break).
        let mut sorted = node_slots;
        for i in 1..nc {
            let s = sorted[i];
            let t = node_truss[s];
            let v = self.nodes[s].map(|r| r.vector.as_u64()).unwrap_or(u64::MAX);
            let mut j = i;
            while j > 0 {
                let ps = sorted[j - 1];
                let pt = node_truss[ps];
                let pv = self.nodes[ps].map(|r| r.vector.as_u64()).unwrap_or(u64::MAX);
                if pt < t || (pt == t && pv > v) {
                    sorted[j] = sorted[j - 1];
                    j -= 1;
                } else {
                    break;
                }
            }
            sorted[j] = s;
        }

        let copy_len = nc.min(N);
        let mut out_vecs  = [ZERO; N];
        let mut out_truss = [0u8; N];
        for i in 0..copy_len {
            let slot = sorted[i];
            out_vecs[i]  = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO);
            out_truss[i] = node_truss[slot];
        }
        let max_truss = if copy_len > 0 { out_truss[0] } else { 0 };
        (out_vecs, out_truss, nc, max_truss)
    }

    /// V2.95: Maximum clique — iterative Bron-Kerbosch with Tomita pivot.
    ///
    /// Returns `(clique_vecs, clique_size, clique_count, node_count)`:
    ///   - `clique_vecs[0..clique_size]` — a representative maximum clique,
    ///     sorted ascending by VectorAddress.
    ///   - `clique_size`  — ω(G), the clique number (0 if no nodes).
    ///   - `clique_count` — number of distinct maximum-size cliques found.
    ///   - `node_count`   — total live nodes.
    ///
    /// Undirected projection: both A→B and B→A are treated as edge A–B.
    /// Self-loops are excluded.  Invariant: clique_size ≥ max_kcore (every
    /// k-clique is a (k-1)-core) and clique_size ≥ max_truss − 1.
    pub fn graph_clique_inner<const N: usize>(&self) -> ([VectorAddress; N], usize, usize, usize) {
        const ZERO: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        // Practical BK depth bound: ω(G) ≤ 128; in practice ≤ 30 for most graphs.
        const BK_MAX: usize = 128;

        // ── 1. Collect live node slots ───────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[nc] = i;
                nc += 1;
            }
        }
        if nc == 0 {
            return ([ZERO; N], 0, 0, 0);
        }

        // ── 2. Build index→slot mapping and undirected adjacency bitsets ─────
        let mut slot_to_idx = [u8::MAX; MAX_NODES];
        for (idx, &s) in node_slots[..nc].iter().enumerate() {
            slot_to_idx[s] = idx as u8;
        }
        // adj[i] = bitmask of undirected neighbours of node-index i
        let mut adj = [0u128; MAX_NODES];
        for i in 0..MAX_EDGES {
            let edge = match self.edges[i] {
                Some(e) => e,
                None    => continue,
            };
            let us = match self.node_slot_by_id(edge.spec.from_node) {
                Some(s) => s,
                None    => continue,
            };
            let vs = match self.node_slot_by_id(edge.spec.to_node) {
                Some(s) => s,
                None    => continue,
            };
            if us == vs { continue; } // self-loop
            let ui = slot_to_idx[us];
            let vi = slot_to_idx[vs];
            if ui == u8::MAX || vi == u8::MAX { continue; }
            let ui = ui as usize;
            let vi = vi as usize;
            adj[ui] |= 1u128 << vi;
            adj[vi] |= 1u128 << ui;
        }

        // ── 3. Iterative Bron-Kerbosch with Tomita pivot ─────────────────────
        // Frame stores state for one recursive BK call level.
        #[derive(Copy, Clone)]
        struct BkFrame {
            r:           u128, // current partial clique (bitmask of node indices)
            p:           u128, // remaining candidates
            x:           u128, // excluded (already processed at this level or above)
            to_try:      u128, // P \ N(pivot) — subset of P still to branch on
            came_from_v: u8,   // node index that created this frame; 0xFF = root
        }

        // Tomita pivot: u from P∪X maximising |P ∩ N(u)|.
        fn choose_pivot(p_x: u128, p: u128, adj: &[u128; MAX_NODES]) -> usize {
            let mut best     = p_x.trailing_zeros() as usize;
            let mut best_cnt = 0u32;
            let mut mask = p_x;
            while mask != 0 {
                let u   = mask.trailing_zeros() as usize;
                mask   &= mask.wrapping_sub(1);
                let cnt = (p & adj[u]).count_ones();
                if cnt > best_cnt {
                    best_cnt = cnt;
                    best     = u;
                }
            }
            best
        }

        let all_p: u128 = if nc >= 128 { u128::MAX } else { (1u128 << nc) - 1 };
        let pivot0   = choose_pivot(all_p, all_p, &adj);
        let to_try0  = all_p & !adj[pivot0];

        let zero_frame = BkFrame { r: 0, p: 0, x: 0, to_try: 0, came_from_v: 0xFF };
        let mut stk    = [zero_frame; BK_MAX];
        stk[0] = BkFrame { r: 0, p: all_p, x: 0, to_try: to_try0, came_from_v: 0xFF };
        let mut depth  = 1usize;

        let mut best_r       = 0u128;
        let mut best_size    = 0usize;
        let mut clique_count = 0usize;

        while depth > 0 {
            let fi = depth - 1;

            // P empty → maximal clique iff X empty; always pop.
            if stk[fi].p == 0 {
                if stk[fi].x == 0 {
                    let size = stk[fi].r.count_ones() as usize;
                    if size > best_size {
                        best_size    = size;
                        best_r       = stk[fi].r;
                        clique_count = 1;
                    } else if size == best_size && size > 0 {
                        clique_count = clique_count.saturating_add(1);
                    }
                }
                depth -= 1;
                if depth > 0 && stk[fi].came_from_v != 0xFF {
                    let v   = stk[fi].came_from_v as usize;
                    let pfi = depth - 1;
                    stk[pfi].p &= !(1u128 << v);
                    stk[pfi].x |=   1u128 << v;
                }
                continue;
            }

            // to_try empty → all necessary branches at this level exhausted; pop.
            if stk[fi].to_try == 0 {
                depth -= 1;
                if depth > 0 && stk[fi].came_from_v != 0xFF {
                    let v   = stk[fi].came_from_v as usize;
                    let pfi = depth - 1;
                    stk[pfi].p &= !(1u128 << v);
                    stk[pfi].x |=   1u128 << v;
                }
                continue;
            }

            // Pick the lowest-index vertex from to_try and branch on it.
            let v = stk[fi].to_try.trailing_zeros() as usize;
            stk[fi].to_try &= !(1u128 << v);

            let new_r  = stk[fi].r | (1u128 << v);
            let new_p  = stk[fi].p & adj[v];
            let new_x  = stk[fi].x & adj[v];
            let new_to_try = if new_p == 0 {
                0
            } else {
                let new_px     = new_p | new_x;
                let cpivot     = choose_pivot(new_px, new_p, &adj);
                new_p & !adj[cpivot]
            };

            if depth < BK_MAX {
                stk[depth] = BkFrame {
                    r:           new_r,
                    p:           new_p,
                    x:           new_x,
                    to_try:      new_to_try,
                    came_from_v: v as u8,
                };
                depth += 1;
            }
            // depth overflow: skip this branch (safety cap; ω ≤ 128 always)
        }

        // ── 4. Reconstruct and sort clique VectorAddresses ───────────────────
        let mut clique_raw = [ZERO; MAX_NODES];
        let mut raw_n      = 0usize;
        let mut mask       = best_r;
        while mask != 0 && raw_n < MAX_NODES {
            let idx    = mask.trailing_zeros() as usize;
            mask      &= mask.wrapping_sub(1);
            let slot   = node_slots[idx];
            clique_raw[raw_n] = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO);
            raw_n += 1;
        }
        // Insertion sort ascending by VectorAddress.as_u64()
        for i in 1..raw_n {
            let key   = clique_raw[i];
            let key_u = key.as_u64();
            let mut j = i;
            while j > 0 && clique_raw[j - 1].as_u64() > key_u {
                clique_raw[j] = clique_raw[j - 1];
                j -= 1;
            }
            clique_raw[j] = key;
        }
        let mut out_vecs = [ZERO; N];
        let copy_len     = raw_n.min(N);
        for i in 0..copy_len {
            out_vecs[i] = clique_raw[i];
        }

        (out_vecs, best_size, clique_count, nc)
    }

    pub fn graph_independent_set_inner<const N: usize>(&self) -> ([VectorAddress; N], usize, usize, usize) {
        const ZERO: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const BK_MAX: usize = 128;

        // ── 1. Collect live node slots ───────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[nc] = i;
                nc += 1;
            }
        }
        if nc == 0 {
            return ([ZERO; N], 0, 0, 0);
        }

        // ── 2. Build index→slot mapping and undirected adjacency bitsets ─────
        let mut slot_to_idx = [u8::MAX; MAX_NODES];
        for (idx, &s) in node_slots[..nc].iter().enumerate() {
            slot_to_idx[s] = idx as u8;
        }
        let mut adj = [0u128; MAX_NODES];
        for i in 0..MAX_EDGES {
            let edge = match self.edges[i] {
                Some(e) => e,
                None    => continue,
            };
            let us = match self.node_slot_by_id(edge.spec.from_node) {
                Some(s) => s,
                None    => continue,
            };
            let vs = match self.node_slot_by_id(edge.spec.to_node) {
                Some(s) => s,
                None    => continue,
            };
            if us == vs { continue; }
            let ui = slot_to_idx[us];
            let vi = slot_to_idx[vs];
            if ui == u8::MAX || vi == u8::MAX { continue; }
            let ui = ui as usize;
            let vi = vi as usize;
            adj[ui] |= 1u128 << vi;
            adj[vi] |= 1u128 << ui;
        }

        // ── 3. Build complement adjacency ────────────────────────────────────
        // comp[i] = all_nodes except i itself and its original-graph neighbours.
        // Max IS in G = max clique in complement graph G̅.
        let all_nodes: u128 = if nc >= 128 { u128::MAX } else { (1u128 << nc) - 1 };
        let mut comp = [0u128; MAX_NODES];
        for i in 0..nc {
            comp[i] = all_nodes & !adj[i] & !(1u128 << i);
        }

        // ── 4. Iterative Bron-Kerbosch with Tomita pivot on G̅ ───────────────
        #[derive(Copy, Clone)]
        struct BkFrame {
            r:           u128,
            p:           u128,
            x:           u128,
            to_try:      u128,
            came_from_v: u8,
        }

        fn choose_pivot_comp(p_x: u128, p: u128, comp: &[u128; MAX_NODES]) -> usize {
            let mut best     = p_x.trailing_zeros() as usize;
            let mut best_cnt = 0u32;
            let mut mask = p_x;
            while mask != 0 {
                let u   = mask.trailing_zeros() as usize;
                mask   &= mask.wrapping_sub(1);
                let cnt = (p & comp[u]).count_ones();
                if cnt > best_cnt {
                    best_cnt = cnt;
                    best     = u;
                }
            }
            best
        }

        let pivot0  = choose_pivot_comp(all_nodes, all_nodes, &comp);
        let to_try0 = all_nodes & !comp[pivot0];

        let zero_frame = BkFrame { r: 0, p: 0, x: 0, to_try: 0, came_from_v: 0xFF };
        let mut stk    = [zero_frame; BK_MAX];
        stk[0] = BkFrame { r: 0, p: all_nodes, x: 0, to_try: to_try0, came_from_v: 0xFF };
        let mut depth  = 1usize;

        let mut best_r   = 0u128;
        let mut best_size = 0usize;
        let mut is_count  = 0usize;

        while depth > 0 {
            let fi = depth - 1;

            if stk[fi].p == 0 {
                if stk[fi].x == 0 {
                    let size = stk[fi].r.count_ones() as usize;
                    if size > best_size {
                        best_size = size;
                        best_r    = stk[fi].r;
                        is_count  = 1;
                    } else if size == best_size && size > 0 {
                        is_count = is_count.saturating_add(1);
                    }
                }
                depth -= 1;
                if depth > 0 && stk[fi].came_from_v != 0xFF {
                    let v   = stk[fi].came_from_v as usize;
                    let pfi = depth - 1;
                    stk[pfi].p &= !(1u128 << v);
                    stk[pfi].x |=   1u128 << v;
                }
                continue;
            }

            if stk[fi].to_try == 0 {
                depth -= 1;
                if depth > 0 && stk[fi].came_from_v != 0xFF {
                    let v   = stk[fi].came_from_v as usize;
                    let pfi = depth - 1;
                    stk[pfi].p &= !(1u128 << v);
                    stk[pfi].x |=   1u128 << v;
                }
                continue;
            }

            let v = stk[fi].to_try.trailing_zeros() as usize;
            stk[fi].to_try &= !(1u128 << v);

            let new_r  = stk[fi].r | (1u128 << v);
            let new_p  = stk[fi].p & comp[v];
            let new_x  = stk[fi].x & comp[v];
            let new_to_try = if new_p == 0 {
                0
            } else {
                let new_px = new_p | new_x;
                let cpivot = choose_pivot_comp(new_px, new_p, &comp);
                new_p & !comp[cpivot]
            };

            if depth < BK_MAX {
                stk[depth] = BkFrame {
                    r:           new_r,
                    p:           new_p,
                    x:           new_x,
                    to_try:      new_to_try,
                    came_from_v: v as u8,
                };
                depth += 1;
            }
        }

        // ── 5. Reconstruct and sort IS VectorAddresses ───────────────────────
        let mut is_raw = [ZERO; MAX_NODES];
        let mut raw_n  = 0usize;
        let mut mask   = best_r;
        while mask != 0 && raw_n < MAX_NODES {
            let idx   = mask.trailing_zeros() as usize;
            mask     &= mask.wrapping_sub(1);
            let slot  = node_slots[idx];
            is_raw[raw_n] = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO);
            raw_n += 1;
        }
        for i in 1..raw_n {
            let key   = is_raw[i];
            let key_u = key.as_u64();
            let mut j = i;
            while j > 0 && is_raw[j - 1].as_u64() > key_u {
                is_raw[j] = is_raw[j - 1];
                j -= 1;
            }
            is_raw[j] = key;
        }
        let mut out_vecs = [ZERO; N];
        let copy_len     = raw_n.min(N);
        for i in 0..copy_len {
            out_vecs[i] = is_raw[i];
        }

        (out_vecs, best_size, is_count, nc)
    }

    /// V2.97: Minimum vertex cover of the live kernel graph.
    ///
    /// Returns `(cover_vecs, cover_size, is_exact, node_count)`:
    /// - `cover_vecs[0..cover_size]` — cover vertices sorted ascending by as_u64().
    /// - `cover_size`                — |min vertex cover|; exact for bipartite, ≤2× opt for general.
    /// - `is_exact`                  — true iff graph is bipartite (König gives exact min cover).
    /// - `node_count`                — total live nodes.
    ///
    /// Bipartite: BFS 2-colouring + Kuhn's max matching + König construction:
    ///   T = (A \ Z_A) ∪ Z_B  where Z is alternating-path reachability from unmatched A-nodes.
    ///   |T| = |max matching| = τ(G) exactly.  Gallai: α(G) + τ(G) = node_count.
    ///
    /// General (non-bipartite): greedy maximal matching — add both endpoints of each
    /// uncovered edge; result is a valid cover with cover_size ≤ 2 × τ(G).
    pub fn graph_vertex_cover_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], usize, bool, usize) {
        const NIL: usize = usize::MAX;
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut cover_vecs = [zero; N];

        // ── 1. Compact live node slots ───────────────────────────────────────
        let mut node_slots  = [0usize; MAX_NODES];
        let mut node_count  = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }
        if node_count == 0 {
            return (cover_vecs, 0, true, 0);
        }

        // ── 2. BFS 2-colouring (undirected) ─────────────────────────────────
        const UNCOLORED: u8 = u8::MAX;
        let mut slot_color  = [UNCOLORED; MAX_NODES];
        let mut is_bipartite = true;
        {
            let mut q  = [0usize; MAX_NODES];
            let mut qh = 0usize;
            let mut qt = 0usize;

            'outer: for ki in 0..node_count {
                let start = node_slots[ki];
                if slot_color[start] != UNCOLORED { continue; }
                slot_color[start] = 0;
                q[qt] = start; qt += 1;
                while qh < qt {
                    let cur        = q[qh]; qh += 1;
                    let cur_color  = slot_color[cur];
                    let next_color = 1 - cur_color;
                    let cur_id = match self.nodes[cur] {
                        Some(r) => r.spec.node_id,
                        None    => continue,
                    };
                    for ei in 0..MAX_EDGES {
                        let edge = match self.edges[ei] { Some(e) => e, None => continue };
                        let nbr_id = if edge.spec.from_node == cur_id {
                            edge.spec.to_node
                        } else if edge.spec.to_node == cur_id {
                            edge.spec.from_node
                        } else {
                            continue
                        };
                        let nbr = match self.node_slot_by_id(nbr_id) {
                            Some(s) => s,
                            None    => continue,
                        };
                        if nbr == cur { continue; } // self-loop
                        if slot_color[nbr] == UNCOLORED {
                            slot_color[nbr] = next_color;
                            if qt < MAX_NODES { q[qt] = nbr; qt += 1; }
                        } else if slot_color[nbr] == cur_color {
                            is_bipartite = false;
                            break 'outer;
                        }
                    }
                }
            }
        }

        if is_bipartite {
            // ── 3. Kuhn's augmenting-path matching (identical to bipartite_match_inner) ──
            let mut match_a = [NIL; MAX_NODES]; // A-slot → matched B-slot
            let mut match_b = [NIL; MAX_NODES]; // B-slot → matched A-slot

            for ki in 0..node_count {
                let start_a = node_slots[ki];
                if slot_color[start_a] != 0 { continue; } // only A-side (color 0)
                if match_a[start_a] != NIL  { continue; } // already matched

                let mut visited_b = [false; MAX_NODES];
                let mut dfs_stk:  [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];
                let mut chosen_b: [usize; MAX_NODES] = [NIL; MAX_NODES];
                let mut st_top    = 1usize;
                dfs_stk[0]        = (start_a, 0);
                let mut augmented = false;

                'dfs: while st_top > 0 {
                    let lvl             = st_top - 1;
                    let (a_slot, mut ei) = dfs_stk[lvl];
                    let a_id = match self.nodes[a_slot] {
                        Some(r) => r.spec.node_id,
                        None    => { st_top -= 1; continue; }
                    };
                    let mut found_next = false;
                    while ei < MAX_EDGES {
                        let edge = match self.edges[ei] {
                            Some(e) => e,
                            None    => { ei += 1; continue; }
                        };
                        let nbr_id = if edge.spec.from_node == a_id {
                            edge.spec.to_node
                        } else if edge.spec.to_node == a_id {
                            edge.spec.from_node
                        } else {
                            ei += 1; continue;
                        };
                        let b_slot = match self.node_slot_by_id(nbr_id) {
                            Some(s) => s,
                            None    => { ei += 1; continue; }
                        };
                        if b_slot == a_slot        { ei += 1; continue; } // self-loop
                        if slot_color[b_slot] != 1 { ei += 1; continue; } // only B-side
                        if visited_b[b_slot]       { ei += 1; continue; }
                        visited_b[b_slot] = true;
                        ei += 1;
                        if match_b[b_slot] == NIL {
                            // Free B-node: augment from level lvl back to 0.
                            chosen_b[lvl] = b_slot;
                            let mut cur_b  = b_slot;
                            let mut cur_lv = lvl;
                            loop {
                                let cur_a = dfs_stk[cur_lv].0;
                                match_a[cur_a] = cur_b;
                                match_b[cur_b] = cur_a;
                                if cur_lv == 0 { break; }
                                cur_lv -= 1;
                                cur_b   = chosen_b[cur_lv];
                            }
                            augmented = true;
                            break 'dfs;
                        } else {
                            let next_a      = match_b[b_slot];
                            chosen_b[lvl]   = b_slot;
                            dfs_stk[lvl].1  = ei;
                            if st_top < MAX_NODES {
                                dfs_stk[st_top] = (next_a, 0);
                                st_top += 1;
                            }
                            found_next = true;
                            break;
                        }
                    }
                    if !found_next && !augmented { st_top -= 1; }
                }
                let _ = augmented;
            }

            // ── 4. König's construction: alternating-path BFS from unmatched A-nodes ──
            // Z = nodes reachable by alternating paths:
            //   A-side → follow unmatched edges to B (b != match_a[a])
            //   B-side → follow matched edge back to A (a = match_b[b])
            // Cover = (A \ Z_A) ∪ Z_B
            let mut in_z = [false; MAX_NODES];
            let mut q    = [0usize; MAX_NODES];
            let mut qh   = 0usize;
            let mut qt   = 0usize;

            for ki in 0..node_count {
                let s = node_slots[ki];
                if slot_color[s] == 0 && match_a[s] == NIL {
                    in_z[s] = true;
                    q[qt] = s; qt += 1;
                }
            }
            while qh < qt {
                let cur    = q[qh]; qh += 1;
                let cur_id = match self.nodes[cur] { Some(r) => r.spec.node_id, None => continue };
                if slot_color[cur] == 0 {
                    // A-node in Z: follow unmatched edges to B-side.
                    for ei in 0..MAX_EDGES {
                        let edge = match self.edges[ei] { Some(e) => e, None => continue };
                        let nbr_id = if edge.spec.from_node == cur_id {
                            edge.spec.to_node
                        } else if edge.spec.to_node == cur_id {
                            edge.spec.from_node
                        } else {
                            continue
                        };
                        let b = match self.node_slot_by_id(nbr_id) { Some(s) => s, None => continue };
                        if b == cur { continue; }              // self-loop
                        if slot_color[b] != 1 { continue; }   // only B-side
                        if match_a[cur] == b { continue; }     // skip the matched edge
                        if !in_z[b] {
                            in_z[b] = true;
                            if qt < MAX_NODES { q[qt] = b; qt += 1; }
                        }
                    }
                } else {
                    // B-node in Z: follow matched edge to A-side.
                    let a = match_b[cur];
                    if a != NIL && !in_z[a] {
                        in_z[a] = true;
                        if qt < MAX_NODES { q[qt] = a; qt += 1; }
                    }
                }
            }

            // Build cover = (A not in Z) + (B in Z), sorted by as_u64().
            let mut cover_slots = [0usize; MAX_NODES];
            let mut n_cover     = 0usize;
            for ki in 0..node_count {
                let s = node_slots[ki];
                let in_cover = if slot_color[s] == 0 { !in_z[s] } else { in_z[s] };
                if in_cover { cover_slots[n_cover] = s; n_cover += 1; }
            }
            for i in 1..n_cover {
                let mut j = i;
                while j > 0 {
                    let sj  = cover_slots[j];
                    let sjm = cover_slots[j - 1];
                    let vj  = self.nodes[sj] .map(|r| r.vector.as_u64()).unwrap_or(0);
                    let vjm = self.nodes[sjm].map(|r| r.vector.as_u64()).unwrap_or(0);
                    if vj < vjm { cover_slots.swap(j, j - 1); j -= 1; } else { break; }
                }
            }
            let cover_size = n_cover.min(N);
            for i in 0..cover_size {
                cover_vecs[i] = self.nodes[cover_slots[i]].map(|r| r.vector).unwrap_or(zero);
            }
            (cover_vecs, cover_size, true, node_count)
        } else {
            // ── 3. 2-approximation: greedy maximal matching ──────────────────
            // For each edge (u,v) with neither endpoint covered, cover both.
            let mut covered     = [false; MAX_NODES];
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node == edge.spec.to_node { continue; } // self-loop
                let us = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
                let vs = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
                if covered[us] || covered[vs] { continue; }
                covered[us] = true;
                covered[vs] = true;
            }

            let mut cover_slots = [0usize; MAX_NODES];
            let mut n_cover     = 0usize;
            for ki in 0..node_count {
                let s = node_slots[ki];
                if covered[s] { cover_slots[n_cover] = s; n_cover += 1; }
            }
            for i in 1..n_cover {
                let mut j = i;
                while j > 0 {
                    let sj  = cover_slots[j];
                    let sjm = cover_slots[j - 1];
                    let vj  = self.nodes[sj] .map(|r| r.vector.as_u64()).unwrap_or(0);
                    let vjm = self.nodes[sjm].map(|r| r.vector.as_u64()).unwrap_or(0);
                    if vj < vjm { cover_slots.swap(j, j - 1); j -= 1; } else { break; }
                }
            }
            let cover_size = n_cover.min(N);
            for i in 0..cover_size {
                cover_vecs[i] = self.nodes[cover_slots[i]].map(|r| r.vector).unwrap_or(zero);
            }
            (cover_vecs, cover_size, false, node_count)
        }
    }

    // V2.98 — minimum dominating set (greedy ln(Δ)+1 approximation)
    //
    // A dominating set D ⊆ V satisfies: every v ∉ D has at least one neighbour in D.
    // Equivalently: V = D ∪ N(D).  γ(G) denotes the minimum size.
    //
    // Algorithm: greedy — at each step select the node that dominates the most
    // currently-undominated vertices (including itself), then remove all newly-
    // dominated nodes from the undominated set.  Achieves ≤ H(Δ)+1 ≈ ln(Δ)+1
    // approximation ratio (where Δ = max degree).
    //
    // Bitmask representation: each compact-index ci has a u128 `dominated[ci]`
    // encoding the set {ci} ∪ undirected-neighbours of ci.  At most 128 nodes.
    //
    // Special cases:
    //   - Isolated nodes have no neighbour; only they can dominate themselves,
    //     so they are always forced into D by the greedy.
    //   - Complete K_n: any single node dominates all → γ=1.
    //   - Star K_{1,k}: centre dominates all → γ=1.
    //   - Path P_n: γ = ⌈n/3⌉ (greedy achieves the optimum for paths).
    //
    // Returns (dom_vecs, dom_size, node_count).
    //   dom_vecs[0..dom_size] = dominating set nodes, sorted ascending by as_u64().
    pub fn graph_dominating_set_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], usize, usize) {
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut dom_vecs = [zero; N];

        // 1. Compact live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }
        if node_count == 0 {
            return (dom_vecs, 0, 0);
        }

        // 2. Map slot → compact index for bitmask operations.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        for ci in 0..node_count {
            slot_to_ci[node_slots[ci]] = ci;
        }

        // 3. Build dominated[ci] = {ci} ∪ undirected-neighbours (bitmask over CIs).
        let mut dominated = [0u128; 128];
        for ci in 0..node_count {
            dominated[ci] |= 1u128 << ci; // every node dominates itself
        }
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if edge.spec.from_node == edge.spec.to_node { continue; } // self-loop
            let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let fci = slot_to_ci[fs];
            let tci = slot_to_ci[ts];
            if fci == usize::MAX || tci == usize::MAX { continue; }
            // Undirected: each endpoint dominates the other.
            dominated[fci] |= 1u128 << tci;
            dominated[tci] |= 1u128 << fci;
        }

        // 4. Greedy selection — always pick the node that covers the most undominated CIs.
        let all_mask: u128 = if node_count >= 128 { u128::MAX } else { (1u128 << node_count) - 1 };
        let mut undominated: u128 = all_mask;
        let mut in_domset = [false; 128];
        let mut dom_size  = 0usize;

        while undominated != 0 && dom_size < node_count {
            let mut best_ci    = 0usize;
            let mut best_count = 0u32;
            for ci in 0..node_count {
                if in_domset[ci] { continue; }
                let coverage = (dominated[ci] & undominated).count_ones();
                if coverage > best_count {
                    best_count = coverage;
                    best_ci    = ci;
                }
            }
            in_domset[best_ci] = true;
            undominated &= !dominated[best_ci];
            dom_size += 1;
        }

        // 5. Collect result, sort ascending by vector.as_u64().
        let mut sort_buf = [(0u64, zero); 128];
        let mut sort_n   = 0usize;
        for ci in 0..node_count {
            if in_domset[ci] {
                let slot = node_slots[ci];
                let vec  = self.nodes[slot].map(|r| r.vector).unwrap_or(zero);
                sort_buf[sort_n] = (vec.as_u64(), vec);
                sort_n += 1;
            }
        }
        for i in 1..sort_n {
            let key = sort_buf[i];
            let mut j = i;
            while j > 0 && sort_buf[j - 1].0 > key.0 {
                sort_buf[j] = sort_buf[j - 1];
                j -= 1;
            }
            sort_buf[j] = key;
        }
        let out_n = sort_n.min(N);
        for i in 0..out_n {
            dom_vecs[i] = sort_buf[i].1;
        }

        (dom_vecs, dom_size, node_count)
    }

    /// V2.99: Minimum path cover (MPC) of a DAG — Kahn BFS + bipartite
    /// expansion (Kuhn matching, u128 bitmasks), O(V·E).
    ///
    /// Returns `(path_vecs, path_ids, path_count, is_dag, node_count)`:
    ///   - `path_vecs[0..node_count]` — all live nodes in path-then-topo order.
    ///   - `path_ids[0..node_count]`  — 0-indexed path ID per node.
    ///   - `path_count`               — n − ν(B(G)) paths (König / Dilworth).
    ///   - `is_dag`                   — false if directed cycle detected.
    ///   - `node_count`               — total live nodes.
    ///
    /// If not a DAG, returns (_, _, 0, false, node_count).
    pub fn graph_min_path_cover_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u8; N], usize, bool, usize) {
        const NIL: usize = usize::MAX;
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut out_vecs    = [zero; N];
        let mut out_path_id = [0u8; N];

        // 1. Compact live nodes.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }
        if node_count == 0 {
            return (out_vecs, out_path_id, 0, true, 0);
        }
        let nc = node_count;

        // slot → compact index
        let mut slot_to_ci = [NIL; MAX_NODES];
        for ci in 0..nc { slot_to_ci[node_slots[ci]] = ci; }

        // 2. Kahn's BFS — verify is_dag, record topological order.
        //    Self-loops keep in_deg > 0, so Kahn never drains them → is_dag=false.
        let mut in_deg = [0u16; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
            in_deg[ts] = in_deg[ts].saturating_add(1);
        }
        let mut topo_order = [0usize; MAX_NODES];
        let mut queue  = [0usize; MAX_NODES];
        let mut q_head = 0usize;
        let mut q_tail = 0usize;
        for ci in 0..nc {
            let s = node_slots[ci];
            if in_deg[s] == 0 { queue[q_tail] = s; q_tail += 1; }
        }
        let mut processed = 0usize;
        while q_head < q_tail {
            let cur_slot = queue[q_head]; q_head += 1;
            topo_order[processed] = cur_slot; processed += 1;
            let cur_id = match self.nodes[cur_slot] { Some(r) => r.spec.node_id, None => continue };
            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                if edge.spec.from_node != cur_id { continue; }
                if edge.spec.from_node == edge.spec.to_node { continue; }
                let nbr_slot = match self.node_slot_by_id(edge.spec.to_node) { Some(s) => s, None => continue };
                if in_deg[nbr_slot] > 0 {
                    in_deg[nbr_slot] -= 1;
                    if in_deg[nbr_slot] == 0 && q_tail < MAX_NODES {
                        queue[q_tail] = nbr_slot; q_tail += 1;
                    }
                }
            }
        }
        if processed != nc {
            return (out_vecs, out_path_id, 0, false, nc);
        }

        // 3. Build bipartite expansion B(G) as bitmask adjacency.
        //    right_adj[u_ci] = bitmask of right-side CIs v where (u,v) ∈ G.
        let mut right_adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if edge.spec.from_node == edge.spec.to_node { continue; }
            let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let fci = slot_to_ci[fs]; let tci = slot_to_ci[ts];
            if fci == NIL || tci == NIL { continue; }
            right_adj[fci] |= 1u128 << tci;
        }

        // 4. Kuhn's augmenting-path matching on B(G).
        //    match_l[ci] = right_ci paired with left_ci; NIL = free.
        //    match_r[ci] = left_ci paired with right_ci; NIL = free.
        let mut match_l = [NIL; MAX_NODES];
        let mut match_r = [NIL; MAX_NODES];
        let mut match_count = 0usize;

        for ti in 0..nc {
            let start_ci = slot_to_ci[topo_order[ti]];
            if start_ci == NIL           { continue; }
            if match_l[start_ci] != NIL { continue; } // already matched

            let mut visited_r = 0u128; // right nodes visited in this DFS
            let mut dfs_lci:  [usize; MAX_NODES] = [NIL; MAX_NODES];
            let mut dfs_rem:  [u128;  128]        = [0;   128];
            let mut chosen_r: [usize; MAX_NODES]  = [NIL; MAX_NODES];
            let mut st_top = 1usize;
            dfs_lci[0] = start_ci;
            dfs_rem[0] = right_adj[start_ci];
            let mut augmented = false;

            'dfs: while st_top > 0 {
                let lvl = st_top - 1;
                let _l_ci = dfs_lci[lvl];
                let avail = dfs_rem[lvl] & !visited_r;
                if avail == 0 { st_top -= 1; continue; } // backtrack

                let r_ci = avail.trailing_zeros() as usize;
                dfs_rem[lvl]  &= !(1u128 << r_ci);
                visited_r     |=   1u128 << r_ci;

                if match_r[r_ci] == NIL {
                    // Free right node — augment path bottom-up.
                    chosen_r[lvl] = r_ci;
                    let (mut cur_r, mut cur_lv) = (r_ci, lvl);
                    loop {
                        let cur_l = dfs_lci[cur_lv];
                        match_l[cur_l] = cur_r; match_r[cur_r] = cur_l;
                        if cur_lv == 0 { break; }
                        cur_lv -= 1; cur_r = chosen_r[cur_lv];
                    }
                    augmented = true; match_count += 1;
                    break 'dfs;
                } else {
                    // Matched right — push its left partner for continuation.
                    let next_l = match_r[r_ci];
                    chosen_r[lvl] = r_ci;
                    if st_top < MAX_NODES {
                        dfs_lci[st_top] = next_l;
                        dfs_rem[st_top] = right_adj[next_l];
                        st_top += 1;
                    }
                }
            }
            let _ = augmented;
        }

        // 5. path_count = n − match_count  (König / Dilworth theorem).
        let path_count = nc - match_count;

        // 6. Reconstruct paths by following match_l[] successor chains.
        //    Path start = a node whose right-side copy is unmatched (match_r[ci] == NIL).
        //    Enumerate starts in topological order → natural top-down path IDs.
        let mut out_count      = 0usize;
        let mut path_id_ctr    = 0u8;
        for ti in 0..nc {
            let ci = slot_to_ci[topo_order[ti]];
            if ci == NIL          { continue; }
            if match_r[ci] != NIL { continue; } // has a predecessor — not a start

            let pid = path_id_ctr;
            if path_id_ctr < u8::MAX { path_id_ctr += 1; }
            let mut cur_ci = ci;
            while cur_ci != NIL && out_count < N {
                let slot = node_slots[cur_ci];
                out_vecs[out_count]    = self.nodes[slot].map(|r| r.vector).unwrap_or(zero);
                out_path_id[out_count] = pid;
                out_count += 1;
                cur_ci = match_l[cur_ci];
            }
        }

        (out_vecs, out_path_id, path_count, true, nc)
    }

    /// V3.00: Minimum spanning arborescence (Chu-Liu / Edmonds 1967), O(V·E).
    ///
    /// Finds the minimum total-weight directed spanning tree rooted at `root`:
    /// every non-root live node is reachable from `root` via a unique directed
    /// path in the tree, and the total arborescence weight is minimised.
    ///
    /// Algorithm: iterative cycle-contraction (Chu-Liu 1965 / Edmonds 1967):
    ///   1. Select minimum-weight incoming edge for every non-root super-node.
    ///   2. If no directed cycle in the selection: it IS the MSA.
    ///   3. Contract one cycle into a new super-node; adjust incoming-edge
    ///      weights (subtract cycle-edge cost each external edge displaces).
    ///   Repeat until no cycle.  Tentative parents are recorded at every round
    ///   and overwritten at the break-point when a cycle is expanded.
    ///
    /// Returns `(vecs, parents, weights, node_count, total_w, is_connected)`:
    ///   `vecs[0..nc]`    — all live nodes, root first.
    ///   `parents[0..nc]` — parent in arborescence (self=root; zero=unreachable).
    ///   `weights[0..nc]` — edge weight×1000 to parent (0 for root).
    ///   `node_count`     — total live nodes.
    ///   `total_w`        — Σ arborescence edge weights × 1000.
    ///   `is_connected`   — true iff a spanning arborescence exists from `root`.
    pub fn graph_arborescence_inner<const N: usize>(
        &self,
        root: VectorAddress,
    ) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32, bool) {
        const NIL: usize  = usize::MAX;
        const INF: f32    = 1.0e30_f32;
        // Super-node space: originals 0..nc + contracted nc..2*nc.
        const MAX_SG: usize = 256; // ≥ 2 × MAX_NODES
        let zero = VectorAddress::new(0, 0, 0, 0);
        let mut out_vecs    = [zero; N];
        let mut out_parents = [zero; N];
        let mut out_weights = [0u32; N];

        // ── 1. Compact live nodes ──────────────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES];
        let mut slot_to_ci = [NIL;    MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                node_slots[nc] = i;
                nc += 1;
            }
        }
        let node_count = nc;
        if nc == 0 {
            return (out_vecs, out_parents, out_weights, 0, 0, true);
        }

        let root_slot = self.node_slot_by_vec(root).unwrap_or(node_slots[0]);
        let root_ci   = slot_to_ci[root_slot];

        let node_vec = |ci: usize| -> VectorAddress {
            self.nodes[node_slots[ci]].map(|r| r.vector).unwrap_or(zero)
        };

        if nc == 1 {
            out_vecs[0]    = node_vec(root_ci);
            out_parents[0] = node_vec(root_ci);
            return (out_vecs, out_parents, out_weights, 1, 0, true);
        }

        // ── 2. Build directed edge list in compact-CI space ────────────────────
        let mut ec     = 0usize;
        let mut e_from = [0u8;    MAX_EDGES];
        let mut e_to   = [0u8;    MAX_EDGES];
        let mut e_wt   = [0.0f32; MAX_EDGES]; // original weight
        let mut e_adj  = [0.0f32; MAX_EDGES]; // accumulated adjustment

        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if edge.spec.from_node == edge.spec.to_node { continue; }
            let fs  = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts  = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let fci = slot_to_ci[fs];
            let tci = slot_to_ci[ts];
            if fci == NIL || tci == NIL { continue; }
            if ec < MAX_EDGES {
                e_from[ec] = fci as u8;
                e_to[ec]   = tci as u8;
                e_wt[ec]   = edge.spec.weight.max(0.0);
                ec += 1;
            }
        }

        // ── 3. Chu-Liu / Edmonds iterative contraction ─────────────────────────
        // group[ci] = current super-node ID.  IDs 0..nc = original; nc.. = contracted.
        let mut group = [0usize; MAX_NODES];
        for ci in 0..nc { group[ci] = ci; }
        let mut num_sg = nc;

        // sel_parent[ci] = parent original CI  (tentative, overwritten at break-point).
        // sel_wt[ci]     = ORIGINAL weight of the chosen incoming edge.
        let mut sel_parent = [NIL;    MAX_NODES];
        let mut sel_wt     = [0.0f32; MAX_NODES];

        let mut in_wt     = [INF;   MAX_SG];
        let mut in_ei     = [NIL;   MAX_SG];
        let mut in_sg_src = [NIL;   MAX_SG];
        let mut color     = [0u8;   MAX_SG]; // 0=white 1=gray 2=black
        let mut is_cyc    = [false; MAX_SG];

        let mut is_connected = true;

        'rounds: for _round in 0..nc {
            let root_sg = group[root_ci];

            for sg in 0..num_sg {
                in_wt[sg]     = INF;
                in_ei[sg]     = NIL;
                in_sg_src[sg] = NIL;
                color[sg]     = 0;
            }

            // Step A: min incoming edge per non-root super-node.
            for ei in 0..ec {
                let f_sg = group[e_from[ei] as usize];
                let t_sg = group[e_to[ei]   as usize];
                if f_sg == t_sg    { continue; }
                if t_sg == root_sg { continue; }
                let eff_w = e_wt[ei] + e_adj[ei];
                if eff_w < in_wt[t_sg]
                    || (eff_w == in_wt[t_sg] && in_ei[t_sg] != NIL && ei < in_ei[t_sg])
                {
                    in_wt[t_sg]     = eff_w;
                    in_ei[t_sg]     = ei;
                    in_sg_src[t_sg] = f_sg;
                }
            }

            // Collect active non-root super-nodes; record tentative parents.
            let mut active_sgs = [0usize; MAX_SG];
            let mut active_n   = 0usize;
            let mut sg_seen    = [false; MAX_SG];

            for ci in 0..nc {
                let sg = group[ci];
                if sg == root_sg { continue; }
                if !sg_seen[sg] {
                    sg_seen[sg] = true;
                    if in_ei[sg] == NIL { is_connected = false; break 'rounds; }
                    active_sgs[active_n] = sg;
                    active_n += 1;
                    let ei_sel = in_ei[sg];
                    sel_parent[e_to[ei_sel] as usize]  = e_from[ei_sel] as usize;
                    sel_wt[e_to[ei_sel] as usize]      = e_wt[ei_sel];
                }
            }

            if active_n == 0 { break 'rounds; }

            // Step B: cycle detection via DFS coloring.
            let mut cycle_sg = NIL;
            'detect: for i in 0..active_n {
                let start = active_sgs[i];
                if color[start] != 0 { continue; }
                let mut cur    = start;
                let mut path   = [0usize; MAX_SG];
                let mut path_n = 0usize;
                loop {
                    if cur == root_sg || in_sg_src[cur] == NIL {
                        for j in 0..path_n { color[path[j]] = 2; }
                        break;
                    }
                    if color[cur] == 2 { for j in 0..path_n { color[path[j]] = 2; } break; }
                    if color[cur] == 1 { cycle_sg = cur; break 'detect; }
                    color[cur] = 1;
                    path[path_n] = cur;
                    path_n += 1;
                    cur = in_sg_src[cur];
                }
            }

            if cycle_sg == NIL { break 'rounds; }

            // Step C: trace full cycle.
            for sg in 0..num_sg { is_cyc[sg] = false; }
            { let mut cur = cycle_sg; loop { is_cyc[cur] = true; cur = in_sg_src[cur]; if cur == cycle_sg { break; } } }

            // Step D: adjust incoming-edge weights before remapping.
            for ei in 0..ec {
                let t_sg = group[e_to[ei]   as usize];
                let f_sg = group[e_from[ei] as usize];
                if is_cyc[t_sg] && !is_cyc[f_sg] { e_adj[ei] -= in_wt[t_sg]; }
            }

            // Step E: remap cycle CIs to new super-node.
            let new_sg = num_sg; num_sg += 1;
            for ci in 0..nc { if is_cyc[group[ci]] { group[ci] = new_sg; } }
        }

        // ── 4. Build output ────────────────────────────────────────────────────
        if !is_connected {
            let out_n = nc.min(N);
            if out_n > 0 { out_vecs[0] = node_vec(root_ci); out_parents[0] = node_vec(root_ci); }
            let mut idx = 1usize;
            for ci in 0..nc {
                if ci == root_ci { continue; }
                if idx >= out_n { break; }
                out_vecs[idx] = node_vec(ci);
                idx += 1;
            }
            return (out_vecs, out_parents, out_weights, node_count, 0, false);
        }

        out_vecs[0]    = node_vec(root_ci);
        out_parents[0] = node_vec(root_ci);
        let mut idx    = 1usize;
        let mut total_f = 0.0f32;
        for ci in 0..nc {
            if ci == root_ci { continue; }
            if idx >= N { break; }
            out_vecs[idx] = node_vec(ci);
            let par_ci = sel_parent[ci];
            if par_ci < nc {
                out_parents[idx] = node_vec(par_ci);
                let w = sel_wt[ci];
                out_weights[idx] = (w * 1000.0) as u32;
                total_f += w;
            }
            idx += 1;
        }

        let total_u32 = (total_f * 1000.0).min(u32::MAX as f32) as u32;
        (out_vecs, out_parents, out_weights, node_count, total_u32, true)
    }

    /// V3.01: Feedback vertex set (FVS) — greedy Kahn-based algorithm.
    ///
    /// Returns `(fvs_vecs, fvs_size, node_count)`:
    /// - `fvs_vecs[0..fvs_size]`: nodes whose removal makes the graph acyclic,
    ///   sorted ascending by `VectorAddress.as_u64()`.
    /// - `fvs_size`: number of nodes in the FVS (greedy upper bound on min-FVS).
    /// - `node_count`: total live nodes in the graph.
    ///
    /// Algorithm: iterative Kahn BFS.  Each round:
    ///   (A) compute in-degree and out-degree for live nodes (self-loops counted);
    ///   (B) build undirected adj bitmask excluding self-loops;
    ///   (C) Kahn BFS: drain in-degree-0 nodes, decrement successors;
    ///   (D) if all drained → acyclic, done;
    ///   (E) else pick undrained node with max `in_deg × out_deg` → FVS, remove.
    ///
    /// Self-loops: always yield in_deg ≥ 1; Kahn never drains them → picked first.
    /// Acyclicity guarantee: removing the returned FVS nodes leaves a DAG.
    pub fn graph_fvs_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], usize, usize) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NIL: usize = usize::MAX;
        let mut out_vecs = [ZERO_VEC; N];

        // ── 1. Compact live nodes ──────────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES];
        let mut slot_to_ci = [NIL;    MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                node_slots[nc] = i;
                nc += 1;
            }
        }
        let node_count = nc;
        if nc == 0 {
            return (out_vecs, 0, 0);
        }

        // ── 2. Greedy FVS loop ─────────────────────────────────────────────
        let mut live     = [true; MAX_NODES]; // live[ci]: not yet in FVS
        let mut fvs_cis  = [0u8;  MAX_NODES]; // FVS compact-indices
        let mut fvs_size = 0usize;

        loop {
            // Count live nodes.
            let mut live_count = 0usize;
            for ci in 0..nc {
                if live[ci] { live_count += 1; }
            }
            if live_count == 0 { break; }

            // Build in-degree, out-degree, and adj bitmask for live nodes.
            // Self-loops count toward in-/out-degree but NOT toward adj.
            let mut in_deg  = [0u32;  MAX_NODES];
            let mut out_deg = [0u32;  MAX_NODES];
            let mut adj     = [0u128; MAX_NODES];

            for ei in 0..MAX_EDGES {
                let edge = match self.edges[ei] { Some(e) => e, None => continue };
                let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
                let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
                let fci = slot_to_ci[fs];
                let tci = slot_to_ci[ts];
                if fci == NIL || tci == NIL { continue; }
                if !live[fci] || !live[tci]  { continue; }
                in_deg[tci]  = in_deg[tci].saturating_add(1);
                out_deg[fci] = out_deg[fci].saturating_add(1);
                if fci != tci && tci < 128 {
                    adj[fci] |= 1u128 << tci;
                }
            }

            // Kahn BFS: drain zero-in-degree nodes.
            let mut queue    = [0u8;   MAX_NODES];
            let mut q_head   = 0usize;
            let mut q_tail   = 0usize;
            let mut in_queue = [false; MAX_NODES];
            let mut processed = 0usize;

            for ci in 0..nc {
                if live[ci] && in_deg[ci] == 0 {
                    in_queue[ci] = true;
                    queue[q_tail] = ci as u8;
                    q_tail += 1;
                }
            }

            while q_head < q_tail {
                let ci = queue[q_head] as usize;
                q_head += 1;
                processed += 1;
                let mut nbrs = adj[ci];
                while nbrs != 0 {
                    let tci = nbrs.trailing_zeros() as usize;
                    nbrs &= nbrs - 1;
                    if in_deg[tci] > 0 { in_deg[tci] -= 1; }
                    if in_deg[tci] == 0 && !in_queue[tci] {
                        in_queue[tci] = true;
                        queue[q_tail] = tci as u8;
                        q_tail += 1;
                    }
                }
            }

            if processed == live_count { break; } // acyclic — done

            // Find undrained node with max in_deg × out_deg score.
            let mut best_ci    = NIL;
            let mut best_score = 0u64;
            for ci in 0..nc {
                if live[ci] && !in_queue[ci] {
                    let score = in_deg[ci] as u64 * out_deg[ci] as u64;
                    if best_ci == NIL || score > best_score {
                        best_score = score;
                        best_ci    = ci;
                    }
                }
            }
            if best_ci == NIL { break; } // safety exit

            if fvs_size < MAX_NODES {
                fvs_cis[fvs_size] = best_ci as u8;
                fvs_size += 1;
            }
            live[best_ci] = false;
        }

        // ── 3. Sort FVS by VectorAddress.as_u64() ascending ───────────────
        let copy_len = fvs_size.min(N);
        let mut tmp = [ZERO_VEC; MAX_NODES];
        for i in 0..fvs_size {
            let ci   = fvs_cis[i] as usize;
            let slot = node_slots[ci];
            tmp[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO_VEC);
        }
        for i in 1..fvs_size {
            let key = tmp[i];
            let mut j = i;
            while j > 0 && tmp[j - 1].as_u64() > key.as_u64() {
                tmp[j] = tmp[j - 1];
                j -= 1;
            }
            tmp[j] = key;
        }
        for i in 0..copy_len {
            out_vecs[i] = tmp[i];
        }

        (out_vecs, fvs_size, node_count)
    }

    /// V3.02: Global minimum cut — Stoer-Wagner 1997.
    ///
    /// Returns `(vecs, sides, node_count, min_cut, side_b_size)`:
    /// - `vecs[0..node_count]` — all live nodes; side-A (sides==0) first, side-B (sides==1) after.
    /// - `sides[0..node_count]` — partition assignment: 0=side A, 1=side B.
    /// - `node_count` — total live nodes.
    /// - `min_cut` — minimum undirected edge cut (= edge connectivity κ'(G)).
    /// - `side_b_size` — count of nodes on side B.
    ///
    /// Uses the undirected projection: A→B and B→A count as one edge.
    /// Disconnected graphs return `min_cut = 0`.
    pub fn graph_min_cut_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [u8; N], usize, u32, usize) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NIL: usize = usize::MAX;
        let mut out_vecs  = [ZERO_VEC; N];
        let mut out_sides = [0u8; N];

        // ── 1. Compact live nodes ──────────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES];
        let mut slot_to_ci = [NIL;    MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                node_slots[nc] = i;
                nc += 1;
            }
        }
        let node_count = nc;
        if nc == 0 { return (out_vecs, out_sides, 0, 0, 0); }
        if nc == 1 {
            out_vecs[0]  = self.nodes[node_slots[0]].map(|r| r.vector).unwrap_or(ZERO_VEC);
            out_sides[0] = 0;
            return (out_vecs, out_sides, 1, 0, 0);
        }

        // ── 2. Build undirected edge list (each unordered pair once) ───────
        // Normalise: uf[i] < ut[i]; use seen_adj bitmask to dedup A↔B pairs.
        let mut uf       = [0u8;   MAX_EDGES];
        let mut ut       = [0u8;   MAX_EDGES];
        let mut uw       = [0u16;  MAX_EDGES]; // weight; starts at 1, accumulates after merges
        let mut ue_live  = [true;  MAX_EDGES];
        let mut ue_count = 0usize;
        // seen_adj[a] bit b = 1 iff undirected pair {a,b} already added (a < b)
        let mut seen_adj = [0u128; MAX_NODES];

        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if edge.spec.from_node == edge.spec.to_node { continue; }
            let fs  = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts  = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let fci = slot_to_ci[fs];
            let tci = slot_to_ci[ts];
            if fci == NIL || tci == NIL { continue; }
            let (a, b) = if fci < tci { (fci, tci) } else { (tci, fci) };
            if b >= 128 { continue; }
            if (seen_adj[a] >> b) & 1 != 0 { continue; }
            seen_adj[a] |= 1u128 << b;
            if ue_count < MAX_EDGES {
                uf[ue_count]      = a as u8;
                ut[ue_count]      = b as u8;
                uw[ue_count]      = 1;
                ue_live[ue_count] = true;
                ue_count         += 1;
            }
        }

        // ── 3. Stoer-Wagner global min-cut phases ──────────────────────────
        // Each phase: maximum-adjacency ordering → cut-of-phase → merge s+t.
        let mut node_active = [false; MAX_NODES];
        for i in 0..nc { node_active[i] = true; }
        let mut active_count = nc;

        // group_mbrs[si]: u128 bitmask of original compact-indices in super-node si
        let mut group_mbrs = [0u128; MAX_NODES];
        for i in 0..nc { group_mbrs[i] = 1u128 << i; }

        let mut min_cut     = u32::MAX;
        let mut best_b_mask = 0u128;

        while active_count > 1 {
            // Maximum adjacency ordering
            let mut key   = [0u16; MAX_NODES];
            let mut in_a  = [false; MAX_NODES];
            let mut last_s = 0usize; // second-to-last added (valid after ≥2 steps)
            let mut last_t = 0usize; // last added

            for _step in 0..active_count {
                // Pick active non-A node with max key; smallest ci breaks ties.
                let mut best   = NIL;
                let mut best_k = 0u16;
                for si in 0..nc {
                    if node_active[si] && !in_a[si] {
                        if best == NIL || key[si] > best_k
                            || (key[si] == best_k && si < best)
                        {
                            best   = si;
                            best_k = key[si];
                        }
                    }
                }
                if best == NIL { break; }
                in_a[best] = true;
                last_s     = last_t;
                last_t     = best;
                // Update keys for active non-A nodes adjacent to best.
                for ei in 0..ue_count {
                    if !ue_live[ei] { continue; }
                    let a = uf[ei] as usize;
                    let b = ut[ei] as usize;
                    let w = uw[ei];
                    if a == best && node_active[b] && !in_a[b] {
                        key[b] = key[b].saturating_add(w);
                    } else if b == best && node_active[a] && !in_a[a] {
                        key[a] = key[a].saturating_add(w);
                    }
                }
            }

            // cut-of-phase = total weight of edges from last_t to the rest of V
            let cop = key[last_t] as u32;
            if cop < min_cut {
                min_cut     = cop;
                best_b_mask = group_mbrs[last_t];
            }

            // Merge last_t into last_s: redirect last_t endpoints → last_s,
            // kill self-loops, then deduplicate parallel edges by adding weights.
            for ei in 0..ue_count {
                if !ue_live[ei] { continue; }
                let a = uf[ei] as usize;
                let b = ut[ei] as usize;
                let na = if a == last_t { last_s } else { a };
                let nb = if b == last_t { last_s } else { b };
                if na == nb { ue_live[ei] = false; continue; } // self-loop
                let (na, nb) = if na < nb { (na, nb) } else { (nb, na) };
                uf[ei] = na as u8;
                ut[ei] = nb as u8;
            }
            for i in 0..ue_count {
                if !ue_live[i] { continue; }
                for j in (i + 1)..ue_count {
                    if !ue_live[j] { continue; }
                    if uf[i] == uf[j] && ut[i] == ut[j] {
                        uw[i] = uw[i].saturating_add(uw[j]);
                        ue_live[j] = false;
                    }
                }
            }
            group_mbrs[last_s] |= group_mbrs[last_t];
            node_active[last_t] = false;
            active_count       -= 1;
        }

        // u32::MAX means no phase ran (nc < 2, already handled above); treat as 0.
        if min_cut == u32::MAX { min_cut = 0; }

        // ── 4. Build output ────────────────────────────────────────────────
        // Collect side-A and side-B nodes separately, then insertion-sort each.
        let mut tmp_a = [ZERO_VEC; MAX_NODES];
        let mut tmp_b = [ZERO_VEC; MAX_NODES];
        let mut cnt_a = 0usize;
        let mut cnt_b = 0usize;

        for ci in 0..nc {
            let slot = node_slots[ci];
            let vec  = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO_VEC);
            if (best_b_mask >> ci) & 1 == 1 {
                if cnt_b < MAX_NODES { tmp_b[cnt_b] = vec; cnt_b += 1; }
            } else {
                if cnt_a < MAX_NODES { tmp_a[cnt_a] = vec; cnt_a += 1; }
            }
        }
        let side_b_size = cnt_b;

        // Sort side-A ascending by as_u64()
        for i in 1..cnt_a {
            let k = tmp_a[i]; let mut j = i;
            while j > 0 && tmp_a[j - 1].as_u64() > k.as_u64() {
                tmp_a[j] = tmp_a[j - 1]; j -= 1;
            }
            tmp_a[j] = k;
        }
        // Sort side-B ascending by as_u64()
        for i in 1..cnt_b {
            let k = tmp_b[i]; let mut j = i;
            while j > 0 && tmp_b[j - 1].as_u64() > k.as_u64() {
                tmp_b[j] = tmp_b[j - 1]; j -= 1;
            }
            tmp_b[j] = k;
        }

        // Pack: side-A then side-B into out_vecs / out_sides
        let copy_len = nc.min(N);
        let mut out_i = 0usize;
        for i in 0..cnt_a {
            if out_i >= copy_len { break; }
            out_vecs[out_i]  = tmp_a[i];
            out_sides[out_i] = 0;
            out_i += 1;
        }
        for i in 0..cnt_b {
            if out_i >= copy_len { break; }
            out_vecs[out_i]  = tmp_b[i];
            out_sides[out_i] = 1;
            out_i += 1;
        }

        (out_vecs, out_sides, node_count, min_cut, side_b_size)
    }

    /// V3.03: Hamiltonian path/circuit detection — iterative backtracking DFS.
    ///
    /// Returns `(path_vecs, path_len, has_circuit, has_path, node_count)`:
    /// - `path_vecs[0..path_len]` — the found Hamiltonian path or circuit nodes in order.
    /// - `path_len` — equals `node_count` if a Ham. path was found; 0 if none found.
    /// - `has_circuit` — true iff a directed Hamiltonian circuit was found (path_len > 0).
    /// - `has_path` — true iff a directed Hamiltonian path was found (has_circuit ⇒ has_path).
    /// - `node_count` — total live nodes.
    ///
    /// Directed graph: edge A→B does NOT imply B→A. Self-loops excluded.
    /// Single node: trivially has_circuit=true, has_path=true.
    ///
    /// Uses iterative DFS with u128 visited bitmask + dead-end pruning.
    /// Step limit: 5 000 000 (avoids hanging; sparse OS graphs terminate well below this).
    pub fn graph_hamiltonian_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], usize, bool, bool, usize) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NIL: usize = usize::MAX;
        let mut out_vecs = [ZERO_VEC; N];

        // ── 1. Compact live nodes ──────────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES];
        let mut slot_to_ci = [NIL;    MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                node_slots[nc] = i;
                nc += 1;
            }
        }
        let node_count = nc;
        if nc == 0 { return (out_vecs, 0, false, false, 0); }
        if nc == 1 {
            let v0 = self.nodes[node_slots[0]].map(|r| r.vector).unwrap_or(ZERO_VEC);
            out_vecs[0] = v0;
            return (out_vecs, 1, true, true, 1);
        }

        // ── 2. Build directed adjacency bitmasks ───────────────────────────
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if edge.spec.from_node == edge.spec.to_node { continue; }
            let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let fci = slot_to_ci[fs];
            let tci = slot_to_ci[ts];
            if fci == NIL || tci == NIL { continue; }
            if tci < 128 { adj[fci] |= 1u128 << tci; }
        }

        // all_mask: bitmask of compact indices 0..nc
        let all_mask: u128 = if nc >= 128 { u128::MAX } else { (1u128 << nc) - 1 };

        // ── 3. Iterative backtracking DFS ──────────────────────────────────
        // Invariant: path[0..depth] are placed; visited = bitmask of placed nodes.
        // cand[d] = remaining successors of path[d] not yet tried for position d+1.

        let mut path      = [0u8;   MAX_NODES];
        let mut cand      = [0u128; MAX_NODES];
        let mut has_circuit = false;
        let mut has_path    = false;
        let mut best_path   = [0u8;  MAX_NODES];

        let mut steps: u64 = 0;
        const STEP_LIMIT: u64 = 5_000_000;

        'start_loop: for start_ci in 0..nc {
            if has_circuit { break; }

            let mut depth    = 1usize;
            let mut visited  = 1u128 << start_ci;
            path[0]  = start_ci as u8;
            cand[0]  = adj[start_ci] & !visited;

            'inner: loop {
                steps += 1;
                if steps > STEP_LIMIT { break 'start_loop; }

                if depth == nc {
                    // All nc nodes placed — Hamiltonian path found.
                    if !has_path {
                        has_path = true;
                        best_path[..nc].copy_from_slice(&path[..nc]);
                    }
                    // Check for circuit: last node has an edge back to start.
                    let last_ci = path[nc - 1] as usize;
                    if (adj[last_ci] >> start_ci) & 1 != 0 {
                        has_circuit = true;
                        best_path[..nc].copy_from_slice(&path[..nc]);
                        break 'start_loop;
                    }
                    // Not a circuit; backtrack to try other path endings.
                    depth  -= 1;
                    visited &= !(1u128 << path[depth]);
                    continue 'inner;
                }

                let d = depth - 1; // cand[d] = candidates extending from path[d]
                if cand[d] == 0 {
                    // No more candidates here; backtrack.
                    if depth == 1 { break 'inner; }
                    depth  -= 1;
                    visited &= !(1u128 << path[depth]);
                } else {
                    // Pick and remove the next candidate.
                    let v     = cand[d].trailing_zeros() as usize;
                    cand[d]  &= cand[d] - 1;

                    // Dead-end pruning: if ≥2 unvisited nodes (after pushing v)
                    // have no successors within the remaining unvisited set, we
                    // cannot route through all of them (at most one can be last).
                    let next_visited    = visited | (1u128 << v);
                    let unvisited_after = all_mask & !next_visited;
                    let remaining       = nc.saturating_sub(depth + 1);
                    let prune = if remaining > 1 {
                        let mut dead_ends = 0usize;
                        let mut um = unvisited_after;
                        let mut found_prune = false;
                        while um != 0 {
                            let w = um.trailing_zeros() as usize;
                            um &= um - 1;
                            if adj[w] & unvisited_after == 0 {
                                dead_ends += 1;
                                if dead_ends > 1 { found_prune = true; break; }
                            }
                        }
                        found_prune
                    } else {
                        false
                    };
                    if prune { continue 'inner; }

                    // Extend path with v.
                    path[depth]  = v as u8;
                    visited      = next_visited;
                    cand[depth]  = adj[v] & !visited;
                    depth       += 1;
                }
            }
        }

        // ── 4. Build output ────────────────────────────────────────────────
        if !has_path {
            return (out_vecs, 0, false, false, node_count);
        }
        let copy_len = nc.min(N);
        for i in 0..copy_len {
            let ci   = best_path[i] as usize;
            let slot = node_slots[ci];
            out_vecs[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO_VEC);
        }
        (out_vecs, nc, has_circuit, true, node_count)
    }

    /// V3.04: Chordal graph recognition — LexBFS + PEO verification.
    ///
    /// A graph is chordal iff every cycle of length ≥ 4 has a chord.
    /// Returns `(peo_vecs, is_chordal, node_count)`.
    pub fn graph_chordal_inner<const N: usize>(&self) -> ([VectorAddress; N], bool, usize) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NIL: usize = usize::MAX;
        let mut out_vecs = [ZERO_VEC; N];

        // ── 1. Compact live nodes ─────────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES];
        let mut slot_to_ci = [NIL;    MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                node_slots[nc] = i;
                nc += 1;
            }
        }
        if nc == 0 { return (out_vecs, true, 0); }
        if nc <= 2 {
            let copy_len = nc.min(N);
            for i in 0..copy_len {
                out_vecs[i] = self.nodes[node_slots[i]].map(|r| r.vector).unwrap_or(ZERO_VEC);
            }
            return (out_vecs, true, nc);
        }

        // ── 2. Build undirected adjacency bitmasks (by compact index) ─────
        // A→B and B→A both contribute the undirected edge {A,B}.
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            if edge.spec.from_node == edge.spec.to_node { continue; }
            let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let fci = slot_to_ci[fs];
            let tci = slot_to_ci[ts];
            if fci == NIL || tci == NIL || fci >= 128 || tci >= 128 { continue; }
            adj[fci] |= 1u128 << tci;
            adj[tci] |= 1u128 << fci;
        }

        // ── 3. LexBFS — compute Perfect Elimination Ordering ──────────────
        // label[ci] = bitmask of PEO positions of already-numbered neighbours.
        // At each step, pick unnumbered node with lex-max label (= max u128).
        let mut label    = [0u128; MAX_NODES];
        let mut numbered = [false; MAX_NODES];
        let mut peo      = [NIL;   MAX_NODES]; // peo[pos] = ci
        let mut pos_of   = [NIL;   MAX_NODES]; // pos_of[ci] = PEO position

        for pos in 0..nc {
            // Pick unnumbered ci with maximum label.
            let mut best_ci    = NIL;
            let mut best_label = 0u128;
            for ci in 0..nc {
                if !numbered[ci] && (best_ci == NIL || label[ci] > best_label) {
                    best_ci    = ci;
                    best_label = label[ci];
                }
            }
            if best_ci == NIL { break; }

            numbered[best_ci] = true;
            peo[pos]          = best_ci;
            pos_of[best_ci]   = pos;

            // Mark this position in each unnumbered neighbour's label.
            let mut nbr_mask = adj[best_ci];
            while nbr_mask != 0 {
                let nci   = nbr_mask.trailing_zeros() as usize;
                nbr_mask &= !(1u128 << nci);
                if nci < nc && !numbered[nci] && pos < 128 {
                    label[nci] |= 1u128 << pos;
                }
            }
        }

        // ── 4. PEO verification (Fulkerson & Gross 1965) ──────────────────
        // For each node v = peo[pos], let N+(v) = neighbours numbered BEFORE v
        // (i.e., pos_of[u] < pos).  N+(v) is a clique iff N+(v)\{w} ⊆ adj[w],
        // where w is the member of N+(v) with the LARGEST pos_of (the most
        // recently numbered among v's earlier neighbours).
        let mut is_chordal = true;
        'peo_check: for pos in 0..nc {
            let vci = peo[pos];
            if vci == NIL { continue; }

            // Collect N+(v) = earlier-numbered neighbours.
            let mut nplus = 0u128;
            let mut nbr   = adj[vci];
            while nbr != 0 {
                let uci  = nbr.trailing_zeros() as usize;
                nbr     &= !(1u128 << uci);
                let u_pos = pos_of[uci];
                if u_pos != NIL && u_pos < pos {
                    nplus |= 1u128 << uci;
                }
            }

            if nplus.count_ones() < 2 { continue; } // trivially a clique

            // Find w: node in N+(v) with the LARGEST pos_of (most recently numbered).
            let mut w_ci  = NIL;
            let mut w_pos = 0usize;
            let mut tmp   = nplus;
            while tmp != 0 {
                let uci  = tmp.trailing_zeros() as usize;
                tmp     &= !(1u128 << uci);
                let u_pos = pos_of[uci];
                if w_ci == NIL || u_pos > w_pos {
                    w_ci  = uci;
                    w_pos = u_pos;
                }
            }

            // N+(v) \ {w} must all be adjacent to w.
            if w_ci < 128 {
                let rest = nplus & !(1u128 << w_ci);
                if rest & adj[w_ci] != rest {
                    is_chordal = false;
                    break 'peo_check;
                }
            }
        }

        // ── 5. Build output in PEO order ──────────────────────────────────
        let copy_len = nc.min(N);
        for i in 0..copy_len {
            let ci = peo[i];
            if ci == NIL { continue; }
            let slot = node_slots[ci];
            out_vecs[i] = self.nodes[slot].map(|r| r.vector).unwrap_or(ZERO_VEC);
        }
        (out_vecs, is_chordal, nc)
    }

    /// V3.05: Biconnected components — iterative Tarjan edge-stack BCC.
    ///
    /// Returns `(vecs, bcc_ids, node_count, bcc_count)`:
    ///   - `vecs[0..node_count]`    — live nodes sorted (bcc_id asc, vec asc).
    ///   - `bcc_ids[0..node_count]` — BCC index (0-based) for each node.
    ///     Nodes in exactly one BCC carry their BCC index.
    ///     Nodes in 2+ BCCs (articulation points) carry 255.
    ///     Isolated nodes each get their own singleton BCC.
    ///   - `node_count`  — total live nodes.
    ///   - `bcc_count`   — total biconnected components (edge-BCCs + isolated-singletons).
    pub fn graph_bcc_inner<const N: usize>(&self) -> ([VectorAddress; N], [u8; N], usize, usize) {
        const ZERO_VEC:   VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NIL:        usize         = usize::MAX;
        const UNVISITED:  u32           = u32::MAX;

        let mut out_vecs = [ZERO_VEC; N];
        let mut out_ids  = [0u8; N];

        // ── 1. Compact live nodes ─────────────────────────────────────────
        let mut node_slots = [0usize; MAX_NODES]; // ci → slot
        let mut slot_to_ci = [NIL;    MAX_NODES]; // slot → ci
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i]  = nc;
                node_slots[nc] = i;
                nc            += 1;
            }
        }
        if nc == 0 { return (out_vecs, out_ids, 0, 0); }

        // ── 2. DFS state (per compact index) ─────────────────────────────
        let mut disc  = [UNVISITED; MAX_NODES]; // discovery time
        let mut low   = [0u32;      MAX_NODES]; // low-link value
        let mut par   = [NIL;       MAX_NODES]; // parent ci (NIL = root)

        // BCC membership:
        //   bcc_primary[ci] = first BCC id assigned to ci (255 = none yet)
        //   bcc_mult[ci]    = ci belongs to 2+ BCCs (is an articulation point)
        let mut bcc_primary = [255u8; MAX_NODES];
        let mut bcc_mult    = [false; MAX_NODES];

        // ── 3. Edge stack: (ci_u, ci_v) as u8 pairs ──────────────────────
        // Each undirected edge pushed at most once (tree edges when child discovered,
        // back edges when disc[nbr] < disc[cur]).
        let mut edge_stk: [(u8, u8); MAX_EDGES] = [(0, 0); MAX_EDGES];
        let mut esp = 0usize;

        // ── 4. DFS stack: (ci, next_edge_scan_index) ─────────────────────
        let mut dfs_stk: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

        let mut timer     = 0u32;
        let mut bcc_count = 0usize;

        for start_ci in 0..nc {
            if disc[start_ci] != UNVISITED { continue; }

            let esp_before   = esp; // safety anchor for root's pop-cleanup
            disc[start_ci]   = timer;
            low[start_ci]    = timer;
            timer           += 1;
            dfs_stk[0]       = (start_ci, 0);
            let mut st_top   = 1;

            'dfs: while st_top > 0 {
                let fi          = st_top - 1;
                let (cur_ci, scan_ei) = dfs_stk[fi];
                let cur_slot    = node_slots[cur_ci];
                let cur_id      = match self.nodes[cur_slot] {
                    Some(r) => r.spec.node_id,
                    None    => { st_top -= 1; continue 'dfs; }
                };

                let mut found = false;
                let mut ei    = scan_ei;

                while ei < MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => { ei += 1; continue; }
                    };

                    // Undirected projection: take the other endpoint.
                    let nbr_id = if edge.spec.from_node == cur_id {
                        edge.spec.to_node
                    } else if edge.spec.to_node == cur_id {
                        edge.spec.from_node
                    } else {
                        ei += 1; continue;
                    };

                    let nbr_slot = match self.node_slot_by_id(nbr_id) {
                        Some(s) => s,
                        None    => { ei += 1; continue; }
                    };
                    if nbr_slot == cur_slot { ei += 1; continue; } // self-loop

                    let nbr_ci = slot_to_ci[nbr_slot];
                    if nbr_ci == NIL { ei += 1; continue; }

                    if disc[nbr_ci] == UNVISITED {
                        // Tree edge: push to edge stack, recurse into child.
                        disc[nbr_ci]    = timer;
                        low[nbr_ci]     = timer;
                        timer          += 1;
                        par[nbr_ci]     = cur_ci;

                        if esp < MAX_EDGES {
                            edge_stk[esp] = (cur_ci as u8, nbr_ci as u8);
                            esp          += 1;
                        }

                        dfs_stk[fi].1   = ei + 1; // resume after this edge on return
                        dfs_stk[st_top] = (nbr_ci, 0);
                        st_top         += 1;
                        found           = true;
                        break;

                    } else if nbr_ci != par[cur_ci] {
                        // Back edge to an already-visited non-parent: update low.
                        if disc[nbr_ci] < low[cur_ci] {
                            low[cur_ci] = disc[nbr_ci];
                        }
                        // Push only if nbr is an ancestor (disc[nbr] < disc[cur]).
                        // This ensures each undirected back-edge is pushed exactly once —
                        // the reverse scan from the ancestor's side has disc[cur] > disc[anc]
                        // so it fails this guard.
                        if disc[nbr_ci] < disc[cur_ci] && esp < MAX_EDGES {
                            edge_stk[esp] = (cur_ci as u8, nbr_ci as u8);
                            esp          += 1;
                        }
                    }
                    ei += 1;
                }

                if !found {
                    st_top -= 1;
                    let p = par[cur_ci];

                    if p < nc {
                        // Non-root: propagate low, then check BCC boundary.
                        if low[cur_ci] < low[p] {
                            low[p] = low[cur_ci];
                        }
                        // BCC condition: low[cur] >= disc[par] means no back-edge
                        // from cur's subtree can reach strictly above par — par is
                        // the BCC boundary (an articulation point, or root).
                        if low[cur_ci] >= disc[p] && bcc_count < 254 {
                            let bid = bcc_count as u8;
                            bcc_count += 1;
                            // Pop edges until the tree edge (p → cur_ci) is popped.
                            while esp > 0 {
                                esp -= 1;
                                let (ea, eb) = edge_stk[esp];
                                let ea = ea as usize;
                                let eb = eb as usize;
                                // Mark ea in this BCC.
                                if bcc_primary[ea] == 255 {
                                    bcc_primary[ea] = bid;
                                } else if bcc_primary[ea] != bid {
                                    bcc_mult[ea] = true;
                                }
                                // Mark eb in this BCC.
                                if bcc_primary[eb] == 255 {
                                    bcc_primary[eb] = bid;
                                } else if bcc_primary[eb] != bid {
                                    bcc_mult[eb] = true;
                                }
                                // Stop after popping the boundary tree edge.
                                if (ea == p && eb == cur_ci) || (eb == p && ea == cur_ci) {
                                    break;
                                }
                            }
                        }
                    } else {
                        // Root popped: safety-net — pop any leftover edges as a BCC.
                        // Normally the root's edges are fully consumed by the child
                        // pops above, but isolated-root components leave nothing here.
                        if esp > esp_before && bcc_count < 254 {
                            let bid = bcc_count as u8;
                            bcc_count += 1;
                            while esp > esp_before {
                                esp -= 1;
                                let (ea, eb) = edge_stk[esp];
                                let ea = ea as usize;
                                let eb = eb as usize;
                                if bcc_primary[ea] == 255 { bcc_primary[ea] = bid; } else if bcc_primary[ea] != bid { bcc_mult[ea] = true; }
                                if bcc_primary[eb] == 255 { bcc_primary[eb] = bid; } else if bcc_primary[eb] != bid { bcc_mult[eb] = true; }
                            }
                        }
                    }
                }
            }
        }

        // ── 5. Isolated nodes → each gets its own singleton BCC ───────────
        for ci in 0..nc {
            if bcc_primary[ci] == 255 && bcc_count < 254 {
                bcc_primary[ci]  = bcc_count as u8;
                bcc_count       += 1;
            }
        }

        // ── 6. Build output arrays ────────────────────────────────────────
        let out_n = nc.min(N);
        for ci in 0..out_n {
            let slot = node_slots[ci];
            if let Some(r) = self.nodes[slot] {
                out_vecs[ci] = r.vector;
                out_ids[ci]  = if bcc_mult[ci] { 255 } else { bcc_primary[ci] };
            }
        }

        // Insertion sort: primary key = bcc_id asc, secondary = vec.as_u64() asc.
        // AP nodes (bcc_id=255) naturally sort last.
        for i in 1..out_n {
            let mut j = i;
            while j > 0 {
                let ka = (out_ids[j - 1] as u64, out_vecs[j - 1].as_u64());
                let kb = (out_ids[j]     as u64, out_vecs[j].as_u64());
                if ka > kb {
                    out_vecs.swap(j - 1, j);
                    out_ids.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        (out_vecs, out_ids, out_n, bcc_count)
    }

    // V3.06: Edge Betweenness Centrality — Brandes (2001).
    // For each directed non-self-loop edge, computes the number of shortest
    // paths (over all ordered source-target pairs) that traverse that edge.
    // Uses the same Dijkstra + back-propagation pass as graph_between_inner
    // but accumulates contribution on edges rather than nodes.
    //
    // Returns (from_vecs, to_vecs, scores, edge_count) sorted descending by
    // score.  Score = raw betweenness (unit: 1 path-pair count).  Edges with
    // equal score are further sorted by (from.as_u64(), to.as_u64()) ascending
    // for deterministic output.
    pub fn graph_betweenness_edge_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize) {
        const SCALE: u64 = 1_000_000;
        const EPS:   f32 = 1e-6;

        // Compact list of live node slots.
        let mut node_slots = [0usize; MAX_NODES];
        let mut node_count = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                node_slots[node_count] = i;
                node_count += 1;
            }
        }

        // Live non-self-loop edge slots.
        let mut edge_slots = [0usize; MAX_EDGES];
        let mut edge_count = 0usize;
        for i in 0..MAX_EDGES {
            if let Some(e) = self.edges[i] {
                if e.spec.from_node != e.spec.to_node {
                    edge_slots[edge_count] = i;
                    edge_count += 1;
                }
            }
        }

        // Per-edge betweenness accumulator (indexed by edge slot).
        let mut edge_bet = [0u64; MAX_EDGES];

        // Brandes + Dijkstra — one pass per source.
        for si in 0..node_count {
            let s = node_slots[si];

            let mut dist    = [f32::MAX; MAX_NODES];
            let mut sigma   = [0u64;     MAX_NODES];
            let mut visited = [false;    MAX_NODES];
            let mut stk     = [0usize;   MAX_NODES];
            let mut stk_len = 0usize;

            dist[s]  = 0.0;
            sigma[s] = 1;

            for _ in 0..node_count {
                let mut u     = usize::MAX;
                let mut u_dst = f32::MAX;
                for ni in 0..node_count {
                    let sl = node_slots[ni];
                    if !visited[sl] && dist[sl] < u_dst {
                        u     = sl;
                        u_dst = dist[sl];
                    }
                }
                if u == usize::MAX || u_dst >= f32::MAX { break; }
                visited[u] = true;

                if stk_len < MAX_NODES {
                    stk[stk_len] = u;
                    stk_len += 1;
                }

                let u_id = match self.nodes[u] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => continue,
                    };
                    if edge.spec.from_node != u_id { continue; }
                    let v = match self.node_slot_by_id(edge.spec.to_node) {
                        Some(sl) => sl,
                        None     => continue,
                    };
                    if v == u { continue; }
                    let w  = edge.spec.weight.max(0.0);
                    let nd = u_dst + w;
                    if nd < dist[v] - EPS {
                        dist[v]  = nd;
                        sigma[v] = sigma[u];
                    } else if (nd - dist[v]).abs() <= EPS && dist[v] < f32::MAX {
                        sigma[v] = sigma[v].saturating_add(sigma[u]);
                    }
                }
            }

            // Back-propagation: accumulate delta and edge betweenness.
            let mut delta = [0u64; MAX_NODES];
            for bi in (0..stk_len).rev() {
                let w = stk[bi];
                if w == s || sigma[w] == 0 { continue; }

                let w_id = match self.nodes[w] {
                    Some(r) => r.spec.node_id,
                    None    => continue,
                };

                for ei in 0..MAX_EDGES {
                    let edge = match self.edges[ei] {
                        Some(e) => e,
                        None    => continue,
                    };
                    if edge.spec.to_node != w_id { continue; }
                    let v = match self.node_slot_by_id(edge.spec.from_node) {
                        Some(sl) => sl,
                        None     => continue,
                    };
                    if sigma[v] == 0 { continue; }
                    if dist[v] >= f32::MAX { continue; }
                    let ew = edge.spec.weight.max(0.0);
                    if (dist[v] + ew - dist[w]).abs() > EPS { continue; }

                    // Contribution to v's delta AND to edge (v→w) betweenness.
                    let contribution = sigma[v]
                        .saturating_mul(SCALE.saturating_add(delta[w]))
                        / sigma[w];
                    delta[v]      = delta[v].saturating_add(contribution);
                    edge_bet[ei]  = edge_bet[ei].saturating_add(contribution);
                }
            }
        }

        // Sort edge_slots by descending edge_bet, then (from_u64, to_u64) asc.
        let mut sorted_e = edge_slots;
        for i in 1..edge_count {
            let key_ei  = sorted_e[i];
            let key_bet = edge_bet[key_ei];
            let key_from = self.edges[key_ei]
                .and_then(|e| self.node_slot_by_id(e.spec.from_node))
                .and_then(|sl| self.nodes[sl])
                .map(|r| r.vector.as_u64())
                .unwrap_or(u64::MAX);
            let key_to = self.edges[key_ei]
                .and_then(|e| self.node_slot_by_id(e.spec.to_node))
                .and_then(|sl| self.nodes[sl])
                .map(|r| r.vector.as_u64())
                .unwrap_or(u64::MAX);
            let mut j = i;
            loop {
                if j == 0 { break; }
                let cmp_ei  = sorted_e[j - 1];
                let cmp_bet = edge_bet[cmp_ei];
                let cmp_from = self.edges[cmp_ei]
                    .and_then(|e| self.node_slot_by_id(e.spec.from_node))
                    .and_then(|sl| self.nodes[sl])
                    .map(|r| r.vector.as_u64())
                    .unwrap_or(u64::MAX);
                let cmp_to = self.edges[cmp_ei]
                    .and_then(|e| self.node_slot_by_id(e.spec.to_node))
                    .and_then(|sl| self.nodes[sl])
                    .map(|r| r.vector.as_u64())
                    .unwrap_or(u64::MAX);
                // Descending bet, then ascending (from, to).
                let should_swap = cmp_bet < key_bet
                    || (cmp_bet == key_bet && (cmp_from, cmp_to) > (key_from, key_to));
                if !should_swap { break; }
                sorted_e[j] = sorted_e[j - 1];
                j -= 1;
            }
            sorted_e[j] = key_ei;
        }

        // Pack output arrays.
        let copy_len = edge_count.min(N);
        let mut out_from  = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_to    = [VectorAddress::new(0, 0, 0, 0); N];
        let mut out_score = [0u32; N];
        for i in 0..copy_len {
            let ei = sorted_e[i];
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            out_from[i] = self.node_slot_by_id(edge.spec.from_node)
                .and_then(|sl| self.nodes[sl])
                .map(|r| r.vector)
                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_to[i] = self.node_slot_by_id(edge.spec.to_node)
                .and_then(|sl| self.nodes[sl])
                .map(|r| r.vector)
                .unwrap_or(VectorAddress::new(0, 0, 0, 0));
            out_score[i] = (edge_bet[ei] / SCALE) as u32;
        }
        (out_from, out_to, out_score, copy_len)
    }

    // ── V3.07: Vertex Connectivity ────────────────────────────────────────────
    pub fn graph_vertex_connectivity_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], usize, u32, u32) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NIL:      usize         = usize::MAX;

        // ── 1. Compact live nodes ─────────────────────────────────────────────
        let mut node_slots = [0usize;  MAX_NODES]; // ci → slot
        let mut slot_to_ci = [NIL;     MAX_NODES]; // slot → ci
        let mut nc         = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i]  = nc;
                node_slots[nc] = i;
                nc            += 1;
            }
        }
        let mut out_vecs = [ZERO_VEC; N];
        if nc == 0 { return (out_vecs, 0, 0, 0); }

        // ── 2. Build undirected adjacency bitmask (compact-index space) ───────
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_slot = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_slot = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci   = slot_to_ci[f_slot];
            let t_ci   = slot_to_ci[t_slot];
            if f_ci == NIL || t_ci == NIL || f_ci == t_ci { continue; }
            adj[f_ci] |= 1u128 << t_ci;
            adj[t_ci] |= 1u128 << f_ci; // undirected projection
        }

        // ── 3. Minimum undirected degree ──────────────────────────────────────
        let all_mask = if nc >= 128 { u128::MAX } else { (1u128 << nc) - 1 };
        let mut min_deg = u32::MAX;
        let mut s_ci    = 0usize;
        for ci in 0..nc {
            let deg = (adj[ci] & all_mask).count_ones();
            if deg < min_deg { min_deg = deg; s_ci = ci; }
        }

        // Build sorted output (shared by all early-return paths)
        let copy_n = nc.min(N);
        for ci in 0..copy_n {
            out_vecs[ci] = self.nodes[node_slots[ci]].map(|r| r.vector).unwrap_or(ZERO_VEC);
        }
        for i in 0..copy_n {
            for j in (i + 1)..copy_n {
                if out_vecs[i].as_u64() > out_vecs[j].as_u64() { out_vecs.swap(i, j); }
            }
        }

        if nc == 1 { return (out_vecs, 1, 0, 0); }

        // ── 4. Connectivity check (BFS on undirected adjacency) ───────────────
        {
            let mut vis = [false; MAX_NODES];
            let mut q   = [0usize; MAX_NODES];
            let mut qh  = 0usize; let mut qt = 0usize;
            vis[0] = true; q[qt] = 0; qt += 1;
            while qh < qt {
                let u = q[qh]; qh += 1;
                let mut nbrs = adj[u] & all_mask;
                while nbrs != 0 {
                    let v = nbrs.trailing_zeros() as usize;
                    nbrs &= nbrs - 1;
                    if !vis[v] { vis[v] = true; q[qt] = v; qt += 1; }
                }
            }
            let mut connected = true;
            for ci in 0..nc { if !vis[ci] { connected = false; break; } }
            if !connected { return (out_vecs, copy_n, 0, min_deg); }
        }

        // ── 5. Complete-graph check: κ = n-1 ─────────────────────────────────
        let mut is_complete = true;
        for ci in 0..nc {
            if (adj[ci] & all_mask).count_ones() as usize != nc - 1 { is_complete = false; break; }
        }
        if is_complete { return (out_vecs, copy_n, (nc - 1) as u32, (nc - 1) as u32); }

        // ── 6. Even's algorithm: κ = min_{t ∉ N(s)} flow(s, t) ──────────────
        // Fix s = min-degree vertex (has at least one non-neighbour since not K_n).
        let s_adj  = adj[s_ci] & all_mask;
        let mut kappa = min_deg; // upper bound: κ ≤ δ

        for t_ci in 0..nc {
            if t_ci == s_ci { continue; }
            if (s_adj >> t_ci) & 1 != 0 { continue; } // adjacent: skip
            let f = vertex_conn_maxflow(&adj, nc, s_ci, t_ci);
            if f < kappa { kappa = f; }
            if kappa == 0 { break; }
        }

        (out_vecs, copy_n, kappa, min_deg)
    }

    // ── V3.08: Edge Colouring (Vizing 1964) ───────────────────────────────────
    pub fn graph_edge_color_inner<const N: usize>(
        &self,
    ) -> ([VectorAddress; N], [VectorAddress; N], [u8; N], usize, u8) {
        const ZERO_VEC: VectorAddress = VectorAddress::new(0, 0, 0, 0);
        const NIL: usize = usize::MAX;

        // 1. Compact node index: ci → slot and slot → ci.
        let mut node_slots = [0usize; MAX_NODES];
        let mut slot_to_ci = [NIL;    MAX_NODES];
        let mut nc         = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i]  = nc;
                node_slots[nc] = i;
                nc            += 1;
            }
        }
        let _ = node_slots;

        let mut out_from   = [ZERO_VEC; N];
        let mut out_to     = [ZERO_VEC; N];
        let mut out_colors = [0u8;      N];

        if nc == 0 { return (out_from, out_to, out_colors, 0, 0); }

        // 2. Build undirected edge list (self-loops excluded; A–B deduplicated
        //    against B–A via seen_adj bitmask; canonical a < b by compact index).
        let mut eu       = [0u8;      MAX_EDGES]; // compact-index a (a < b)
        let mut ev       = [0u8;      MAX_EDGES]; // compact-index b
        let mut ef       = [ZERO_VEC; MAX_EDGES]; // VectorAddress for a
        let mut et       = [ZERO_VEC; MAX_EDGES]; // VectorAddress for b
        let mut seen_adj = [0u128;    MAX_NODES]; // bit b set in seen_adj[a] ↔ {a,b} seen
        let mut ec       = 0usize;

        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == NIL || t_ci == NIL || f_ci == t_ci { continue; }
            let (a, b)       = if f_ci < t_ci { (f_ci, t_ci) } else { (t_ci, f_ci) };
            let (a_sl, b_sl) = if f_ci < t_ci { (f_sl,  t_sl) } else { (t_sl,  f_sl) };
            if (seen_adj[a] >> b) & 1 != 0 { continue; }
            seen_adj[a] |= 1u128 << b;
            if ec >= MAX_EDGES { break; }
            eu[ec] = a as u8;
            ev[ec] = b as u8;
            ef[ec] = self.nodes[a_sl].map(|r| r.vector).unwrap_or(ZERO_VEC);
            et[ec] = self.nodes[b_sl].map(|r| r.vector).unwrap_or(ZERO_VEC);
            ec    += 1;
        }

        if ec == 0 { return (out_from, out_to, out_colors, 0, 0); }

        // 3. Greedy edge colouring.
        //    node_colors[ci]: bit k set ⟹ colour k already on an incident edge.
        //    trailing_ones(forbidden) = index of lowest 0 bit = lowest free colour.
        let mut node_colors = [0u128; MAX_NODES];
        let mut edge_color  = [0u8;   MAX_EDGES];
        let mut max_color   = 0u8;

        for i in 0..ec {
            let a         = eu[i] as usize;
            let b         = ev[i] as usize;
            let forbidden = node_colors[a] | node_colors[b];
            let color     = forbidden.trailing_ones() as u8;
            edge_color[i]  = color;
            node_colors[a] |= 1u128 << color;
            node_colors[b] |= 1u128 << color;
            if color > max_color { max_color = color; }
        }

        let chromatic_index = max_color + 1;

        // 4. Sort by (colour ASC, from.as_u64() ASC, to.as_u64() ASC) and pack.
        let mut order = [0usize; MAX_EDGES];
        for i in 0..ec { order[i] = i; }
        for i in 1..ec {
            let key = order[i];
            let mut j = i;
            while j > 0 {
                let p    = order[j - 1];
                let swap = if edge_color[key] != edge_color[p] {
                    edge_color[key] < edge_color[p]
                } else if ef[key].as_u64() != ef[p].as_u64() {
                    ef[key].as_u64() < ef[p].as_u64()
                } else {
                    et[key].as_u64() < et[p].as_u64()
                };
                if swap { order[j] = p; j -= 1; } else { break; }
            }
            order[j] = key;
        }

        let copy_len = ec.min(N);
        for i in 0..copy_len {
            let oi         = order[i];
            out_from[i]   = ef[oi];
            out_to[i]     = et[oi];
            out_colors[i] = edge_color[oi];
        }

        (out_from, out_to, out_colors, ec, chromatic_index)
    }

    /// V3.09: Spectral radius ρ(A) and algebraic connectivity λ₂(L) via power iteration.
    ///
    /// Returns `(spectral_radius_ppm, algebraic_connectivity_ppm, node_count)`:
    /// - `spectral_radius_ppm`         — ρ(A) × 1_000_000: largest eigenvalue of the
    ///   (undirected) adjacency matrix A.  For d-regular graphs ρ=d; K_n: ρ=n-1;
    ///   star K_{1,k}: ρ=√k.  Epidemic threshold: β < 1/ρ → cascade dies out.
    /// - `algebraic_connectivity_ppm`  — λ₂(L) × 1_000_000: second-smallest eigenvalue of
    ///   the Laplacian L = D−A.  λ₂=0 iff the graph is disconnected; larger λ₂ means
    ///   faster consensus / better fault tolerance.  Cheeger: h(G) ≥ λ₂/2.
    /// - `node_count`                  — live nodes (undirected projection).
    ///
    /// Algorithm: three-phase power iteration, 60/60/80 steps, O(m) per step.
    ///   Normalization uses the infinity-norm (max absolute value) to avoid
    ///   computing sqrt in no_std; the Rayleigh quotient gives the eigenvalue.
    ///   Phase 1: ρ(A) — iterate A·x with inf-norm normalize; Rayleigh quotient → ρ.
    ///   Phase 2: λₙ(L) — iterate L·x with mean-centering (deflates zero eigenspace).
    ///   Phase 3: λ₂(L) — iterate B·x, B=(λₙ+1)I−L, with mean-centering; converges
    ///            to B's next-dominant eigenvalue = (λₙ+1)−λ₂ ⇒ λ₂=(λₙ+1)−result.
    ///
    /// Special cases: empty/single node → (0, 0, n). Disconnected → λ₂=0.
    pub fn graph_spectral_inner(&self) -> (u32, u32, usize) {
        const NIL: usize = usize::MAX;

        // 1. Compact node index.
        let mut slot_to_ci = [NIL; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0); }

        // 2. Undirected adjacency bitmasks + degree (self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        let mut deg = [0u32;  MAX_NODES];

        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == NIL || t_ci == NIL || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                deg[f_ci] += 1;
                deg[t_ci] += 1;
            }
        }

        // Advance bit cursor through a u128 bitmask.
        #[inline(always)]
        fn next_bit(bits: &mut u128) -> usize {
            let j = bits.trailing_zeros() as usize;
            *bits &= *bits - 1;
            j
        }

        // Inf-norm (max absolute value) over the active slice.
        #[inline(always)]
        fn inf_norm(v: &[f32], n: usize) -> f32 {
            let mut m = 0.0f32;
            for i in 0..n { let a = v[i].abs(); if a > m { m = a; } }
            m
        }

        // Rayleigh quotient R(x, w) = (x·w) / (x·x).
        #[inline(always)]
        fn rayleigh(x: &[f32], w: &[f32], n: usize) -> f32 {
            let mut xw = 0.0f32;
            let mut xx = 0.0f32;
            for i in 0..n { xw += x[i] * w[i]; xx += x[i] * x[i]; }
            if xx > 1e-20 { xw / xx } else { 0.0f32 }
        }

        // Phase 1 — ρ(A): iterate A² (two A-multiplications per step) to avoid
        // ±λ oscillation for graphs like P₃ (λ=±√2) or stars (λ=±√k).
        // Rayleigh quotient R(x, A²x) converges to ρ(A²) = ρ(A)².
        // ρ(A) is recovered via integer Newton-Raphson isqrt — no f32::sqrt needed.
        #[inline(always)]
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        let mut x = [0.0f32; MAX_NODES];
        for i in 0..nc { x[i] = 1.0f32; }

        let mut rho_sq = 0.0f32;
        for _ in 0..60 {
            let mut w1 = [0.0f32; MAX_NODES];
            for i in 0..nc {
                let mut s = 0.0f32;
                let mut b = adj[i];
                while b != 0 { s += x[next_bit(&mut b)]; }
                w1[i] = s;
            }
            let mut w2 = [0.0f32; MAX_NODES];
            for i in 0..nc {
                let mut s = 0.0f32;
                let mut b = adj[i];
                while b != 0 { s += w1[next_bit(&mut b)]; }
                w2[i] = s;
            }
            rho_sq = rayleigh(&x, &w2, nc);
            let m = inf_norm(&w2, nc);
            if m < 1e-9 { break; }
            for i in 0..nc { x[i] = w2[i] / m; }
        }

        // Phase 2 — λₙ(L): iterate L·x, deflate zero eigenspace by mean-centering.
        // L·x[i] = deg[i]·x[i] − Σ_{j∈N(i)} x[j]
        let mut xl = [0.0f32; MAX_NODES];
        for i in 0..nc { xl[i] = if i & 1 == 0 { 1.0f32 } else { -1.0f32 }; }

        let mut lambda_max = 0.0f32;
        for _ in 0..60 {
            let mut w = [0.0f32; MAX_NODES];
            for i in 0..nc {
                let mut s = deg[i] as f32 * xl[i];
                let mut b = adj[i];
                while b != 0 { s -= xl[next_bit(&mut b)]; }
                w[i] = s;
            }
            let mean = {
                let mut r = 0.0f32;
                for i in 0..nc { r += w[i]; }
                r / nc as f32
            };
            for i in 0..nc { w[i] -= mean; }
            lambda_max = rayleigh(&xl, &w, nc);
            let m = inf_norm(&w, nc);
            if m < 1e-9 { break; }
            for i in 0..nc { xl[i] = w[i] / m; }
        }

        // λ₂ is only defined for graphs with ≥ 2 nodes.
        // For nc=1: the Laplacian has a single eigenvalue 0 — no λ₂ exists.
        if nc <= 1 {
            let rho_sq_u = (rho_sq.max(0.0f32) * 1_000_000.0f32) as u64;
            let rho_ppm  = isqrt64(rho_sq_u.saturating_mul(1_000_000u64)) as u32;
            return (rho_ppm, 0, nc);
        }

        // Phase 3 — λ₂(L): iterate B·x, B = μI − L, deflating all-ones direction.
        // B's eigenvalues = μ−λᵢ. The largest (= μ, from zero eigenspace of L) is
        // deflated via mean-centering; convergence then finds μ−λ₂.
        let mu = lambda_max + 1.0f32;

        // Init x2 half-positive / half-negative to avoid alignment with all-ones.
        let mut x2 = [0.0f32; MAX_NODES];
        for i in 0..nc { x2[i] = if i < (nc + 1) / 2 { 1.0f32 } else { -1.0f32 }; }
        {
            let mean = {
                let mut r = 0.0f32;
                for i in 0..nc { r += x2[i]; }
                r / nc as f32
            };
            for i in 0..nc { x2[i] -= mean; }
            let m = inf_norm(&x2, nc);
            if m > 1e-9 { for i in 0..nc { x2[i] /= m; } }
        }

        let mut mu_minus_lambda2 = mu;
        for _ in 0..80 {
            let mut w = [0.0f32; MAX_NODES];
            // B·x[i] = (μ − deg[i])·x[i] + Σ_{j∈N(i)} x[j]
            for i in 0..nc {
                let mut s = (mu - deg[i] as f32) * x2[i];
                let mut b = adj[i];
                while b != 0 { s += x2[next_bit(&mut b)]; }
                w[i] = s;
            }
            let mean = {
                let mut r = 0.0f32;
                for i in 0..nc { r += w[i]; }
                r / nc as f32
            };
            for i in 0..nc { w[i] -= mean; }
            mu_minus_lambda2 = rayleigh(&x2, &w, nc);
            let m = inf_norm(&w, nc);
            if m < 1e-9 { break; }
            for i in 0..nc { x2[i] = w[i] / m; }
        }

        let lambda2 = (mu - mu_minus_lambda2).max(0.0f32);

        let rho_sq_u    = (rho_sq.max(0.0f32) * 1_000_000.0f32) as u64;
        let rho_ppm     = isqrt64(rho_sq_u.saturating_mul(1_000_000u64)) as u32;
        let lambda2_ppm = (lambda2 * 1_000_000.0f32).min(u32::MAX as f32) as u32;

        (rho_ppm, lambda2_ppm, nc)
    }

    pub fn graph_entropy_inner(&self) -> (u32, u32, usize) {
        const LN_TABLE: [u32; 129] = [
            0,
            0,         693_147, 1_098_612, 1_386_294, 1_609_437,
            1_791_759, 1_945_910, 2_079_441, 2_197_224, 2_302_585,
            2_397_895, 2_484_906, 2_564_949, 2_639_057, 2_708_050,
            2_772_588, 2_833_213, 2_890_371, 2_944_438, 2_995_732,
            3_044_522, 3_091_042, 3_135_494, 3_178_053, 3_218_875,
            3_258_096, 3_295_836, 3_332_204, 3_367_295, 3_401_197,
            3_433_987, 3_465_735, 3_496_508, 3_526_360, 3_555_348,
            3_583_518, 3_610_917, 3_637_586, 3_663_562, 3_688_879,
            3_713_572, 3_737_669, 3_761_200, 3_784_189, 3_806_662,
            3_828_641, 3_850_147, 3_871_201, 3_891_820, 3_912_023,
            3_931_826, 3_951_244, 3_970_292, 3_988_984, 4_007_333,
            4_025_351, 4_043_051, 4_060_443, 4_077_537, 4_094_344,
            4_110_874, 4_127_134, 4_143_134, 4_158_883, 4_174_387,
            4_189_654, 4_204_692, 4_219_507, 4_234_107, 4_248_494,
            4_262_679, 4_276_666, 4_290_459, 4_304_064, 4_317_488,
            4_330_733, 4_343_805, 4_356_708, 4_369_447, 4_382_026,
            4_394_449, 4_406_719, 4_418_841, 4_430_817, 4_442_651,
            4_454_347, 4_465_908, 4_477_337, 4_488_636, 4_499_810,
            4_510_860, 4_521_789, 4_532_599, 4_543_294, 4_553_877,
            4_564_348, 4_574_711, 4_584_967, 4_595_119, 4_605_170,
            4_615_121, 4_624_972, 4_634_728, 4_644_391, 4_653_960,
            4_663_439, 4_672_829, 4_682_131, 4_691_348, 4_700_480,
            4_709_530, 4_718_498, 4_727_388, 4_736_198, 4_744_932,
            4_753_590, 4_762_174, 4_770_685, 4_779_123, 4_787_491,
            4_795_791, 4_804_021, 4_812_184, 4_820_281, 4_828_313,
            4_836_281, 4_844_187, 4_852_030,
        ];

        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0); }

        // 2. Undirected adjacency bitmasks (self-loops excluded, deduped).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Degree histogram: deg_count[d] = # nodes with undirected degree d.
        let mut deg_count = [0u32; MAX_NODES];
        for ci in 0..nc {
            let d = adj[ci].count_ones() as usize;
            deg_count[d] += 1;
        }

        // 4. Shannon entropy H = -Σ p(d) ln p(d) = (1/n) Σ count[d] (ln n - ln count[d])
        //    entropy_scaled = Σ_{d:count>0} count[d] × (LN_TABLE[nc] - LN_TABLE[count[d]])
        //    entropy_ppm    = entropy_scaled / nc  ≈ H × 10^6  (nats × 10^6)
        let nc_ln = LN_TABLE[nc.min(128)];
        let mut entropy_scaled: u64 = 0;
        for d in 0..nc {
            let cnt = deg_count[d];
            if cnt == 0 { continue; }
            let cnt_u = (cnt as usize).min(128);
            entropy_scaled += cnt as u64 * (nc_ln - LN_TABLE[cnt_u]) as u64;
        }
        let entropy_ppm = (entropy_scaled / nc as u64) as u32;

        // 5. Normalized entropy H' = H / ln(n) ∈ [0, 1]; 1_000_000 = maximum diversity.
        //    H = 0 for regular graphs; H = ln(n) when all degrees distinct.
        let normalized_ppm = if nc > 1 && nc_ln > 0 {
            (entropy_ppm as u64 * 1_000_000 / nc_ln as u64) as u32
        } else {
            0
        };

        (entropy_ppm, normalized_ppm, nc)
    }

    /// V3.11: Zagreb indices M1/M2, Randić connectivity R, and Albertson irregularity I.
    ///
    /// M1(G) = Σ_v deg(v)²                              (Gutman & Trinajstić 1972)
    /// M2(G) = Σ_{uv∈E} deg(u)×deg(v)                  (second Zagreb index)
    /// R(G)  = Σ_{uv∈E} 1/√(deg(u)×deg(v)) × 10^6      (Randić 1975, integer ppm)
    /// I(G)  = Σ_{uv∈E} |deg(u)−deg(v)|                 (Albertson 1997)
    ///
    /// All arithmetic uses undirected edges (directed→undirected dedup, self-loops excluded).
    /// Randić is computed with six-digit precision: sqrt via Newton-Raphson on (p × 10^12).
    pub fn graph_zagreb_inner(&self) -> (u64, u64, u32, u32, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Undirected degree per compact-index node.
        let mut deg = [0u32; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones();
        }

        // 4. M1 = Σ_v deg(v)²
        let mut m1: u64 = 0;
        for ci in 0..nc {
            m1 += (deg[ci] as u64) * (deg[ci] as u64);
        }

        // Newton-Raphson integer sqrt: returns floor(sqrt(p × 10^12)).
        // Gives six decimal digits of sqrt(p), enabling precise ppm Randić computation.
        fn isqrt_ppm(p: u64) -> u64 {
            if p == 0 { return 0; }
            let n = p.saturating_mul(1_000_000_000_000u64);
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        // 5. Scan undirected edges (a < b canonical): accumulate M2, R, I, edge_count.
        let mut m2:           u64 = 0;
        let mut randic_acc:   u64 = 0;
        let mut irregularity: u64 = 0;
        let mut edge_count:   usize = 0;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b <= a { continue; } // each undirected edge counted once (a < b)
                let da = deg[a] as u64;
                let db = deg[b] as u64;
                m2 += da * db;
                irregularity += if da > db { da - db } else { db - da };
                let s = isqrt_ppm(da * db); // floor(sqrt(da*db) × 10^6)
                if s > 0 { randic_acc += 1_000_000_000_000u64 / s; }
                edge_count += 1;
            }
        }

        let randic_ppm   = randic_acc.min(u32::MAX as u64) as u32;
        let irregularity = irregularity.min(u32::MAX as u64) as u32;
        (m1, m2, randic_ppm, irregularity, edge_count, nc)
    }

    /// V3.12: Sum-connectivity (SC), geometric-arithmetic (GA), and augmented Zagreb (AZI) indices.
    ///
    /// Returns `(sc_ppm, ga_ppm, azi_milli, edge_count, node_count)`:
    /// - `sc_ppm`    = SC × 10^6  where SC  = Σ_{uv∈E} 1/√(deg(u)+deg(v))         (Zhou 2009)
    /// - `ga_ppm`    = GA × 10^6  where GA  = Σ_{uv∈E} 2√(deg(u)·deg(v))/(deg(u)+deg(v)) (Vukičević 2009)
    /// - `azi_milli` = AZI × 1000 where AZI = Σ_{uv∈E,q>0} (deg(u)·deg(v)/(deg(u)+deg(v)−2))³
    ///                                                                                (Furtula 2010)
    /// All three use the same O(V+E) undirected edge scan with integer Newton-Raphson isqrt.
    /// Pendant-pendant edges (both deg=1, q=0) contribute 0 to AZI (undefined term skipped).
    pub fn graph_topo_indices_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Undirected degree per compact-index node.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones() as u64;
        }

        // floor(√p × 10^6) via Newton-Raphson on p × 10^12 (same helper as graph_zagreb).
        fn isqrt_ppm(p: u64) -> u64 {
            if p == 0 { return 0; }
            let n = p.saturating_mul(1_000_000_000_000u64);
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        // 4. Scan undirected edges (a < b canonical): accumulate SC, GA, AZI.
        let mut sc_acc:     u64 = 0;
        let mut ga_acc:     u64 = 0;
        let mut azi_acc:    u64 = 0;
        let mut edge_count: usize = 0;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b <= a { continue; } // each undirected edge once (a < b)

                let da = deg[a];
                let db = deg[b];
                let p  = da * db; // product
                let s  = da + db; // sum

                // SC: floor(10^6 / √s) = floor(10^12 / isqrt_ppm(s))
                let sqrt_s = isqrt_ppm(s);
                if sqrt_s > 0 { sc_acc += 1_000_000_000_000u64 / sqrt_s; }

                // GA: floor(2√p / s × 10^6) = 2 × isqrt_ppm(p) / s
                ga_acc += 2 * isqrt_ppm(p) / s;

                // AZI: (p/q)³ × 1000; q = s−2. Skip pendant-pendant edges (q=0).
                let q = s.saturating_sub(2);
                if q > 0 {
                    azi_acc += (p * p * p) * 1_000 / (q * q * q);
                }

                edge_count += 1;
            }
        }

        (sc_acc, ga_acc, azi_acc, edge_count, nc)
    }

    /// V3.13: H + ABC + F degree-based topological indices.
    ///
    /// Returns (h_ppm, abc_ppm, f_index, edge_count, node_count).
    ///   h_ppm   = H × 10^6 where H   = Σ_{uv∈E} 2/(deg(u)+deg(v))          (Zhong 2012)
    ///   abc_ppm = ABC × 10^6 where ABC= Σ_{uv∈E} √((deg(u)+deg(v)−2)/(deg(u)·deg(v)))
    ///                                                                         (Estrada et al. 2008)
    ///   f_index = F(G) = Σ_v deg(v)³  (exact integer)                        (Furtula & Gutman 2015)
    ///
    /// H:   contribution = floor(2_000_000 / s)  where s = da+db  (error ≤ 1 ppm per edge)
    /// ABC: contribution = floor(√((s−2)×10^12 / p))  where p = da·db; 0 when s=2 (pendant-pendant)
    /// F:   exact; node scan of deg³.  Isolated nodes contribute 0.
    pub fn graph_topo_indices2_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Undirected degree per compact-index node.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones() as u64;
        }

        // Integer floor-sqrt via Newton-Raphson.
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        // 4. Scan undirected edges (a < b canonical): accumulate H and ABC.
        let mut h_acc:      u64 = 0;
        let mut abc_acc:    u64 = 0;
        let mut edge_count: usize = 0;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b <= a { continue; } // each undirected edge once (a < b)

                let da = deg[a];
                let db = deg[b];
                let p  = da * db; // product
                let s  = da + db; // sum

                // H: floor(2_000_000 / s)
                h_acc += 2_000_000 / s;

                // ABC: floor(√((s-2) × 10^12 / p)); 0 when s=2 (pendant-pendant)
                if s > 2 && p > 0 {
                    let numer = (s - 2).saturating_mul(1_000_000_000_000u64);
                    abc_acc += isqrt64(numer / p);
                }
                // s == 2 means da=db=1; (s-2)=0; ABC contribution = 0.

                edge_count += 1;
            }
        }

        // 5. F-index: node scan — Σ_v deg(v)³.
        let mut f_index: u64 = 0;
        for ci in 0..nc {
            let d = deg[ci];
            f_index += d * d * d;
        }

        (h_acc, abc_acc, f_index, edge_count, nc)
    }

    /// V3.14: SDD + ISI + Nirmala degree-based topological indices.
    ///
    /// Returns (sdd_ppm, isi_ppm, ni_ppm, edge_count, node_count).
    ///   sdd_ppm = SDD × 10^6 where SDD = Σ_{uv∈E} (da²+db²)/(da·db)  (Vasilyev 2014)
    ///   isi_ppm = ISI × 10^6 where ISI = Σ_{uv∈E} da·db/(da+db)       (Sedlar et al. 2011)
    ///   ni_ppm  = NI  × 10^6 where NI  = Σ_{uv∈E} √(da+db)            (Rather et al. 2021)
    ///
    /// SDD: contribution = floor((da²+db²) × 10^6 / (da·db))
    ///      SDD ≥ 2|E| by AM-GM (equality iff regular graph)
    /// ISI: contribution = floor(da·db × 10^6 / (da+db))
    ///      ISI = |E|·Δ/2 for any Δ-regular graph (exact)
    /// NI:  contribution = isqrt64((da+db) × 10^12)
    ///      NI = |E|·√(2Δ) for Δ-regular (exact when 2Δ is a perfect square)
    pub fn graph_topo_indices3_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Undirected degree per compact-index node.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones() as u64;
        }

        // Integer floor-sqrt via Newton-Raphson.
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        // 4. Scan undirected edges (a < b canonical): accumulate SDD, ISI, NI.
        let mut sdd_acc:    u64 = 0;
        let mut isi_acc:    u64 = 0;
        let mut ni_acc:     u64 = 0;
        let mut edge_count: usize = 0;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b <= a { continue; } // each undirected edge once (a < b)

                let da = deg[a];
                let db = deg[b];
                let p  = da * db; // product (> 0 since both in adj)
                let s  = da + db; // sum

                // SDD: floor((da²+db²) × 10^6 / (da·db))
                sdd_acc += (da * da + db * db) * 1_000_000 / p;

                // ISI: floor(da·db × 10^6 / (da+db))
                isi_acc += p * 1_000_000 / s;

                // NI: floor(√(s × 10^12)) = isqrt64(s × 10^12)
                ni_acc += isqrt64(s * 1_000_000_000_000u64);

                edge_count += 1;
            }
        }

        (sdd_acc, isi_acc, ni_acc, edge_count, nc)
    }

    /// V3.15: Sombor + Reduced Second Zagreb + Sigma degree-based topological indices.
    ///
    /// Returns (so_ppm, rm2, sigma, edge_count, node_count).
    ///   so_ppm = SO × 10^6 where SO = Σ_{uv∈E} √(da²+db²)          (Gutman 2021)
    ///   rm2    = RM₂       where RM₂= Σ_{uv∈E} (da-1)·(db-1)       (Furtula, Gutman & Ediz 2014)
    ///   sigma  = σ(G)      where σ  = Σ_{uv∈E} (da-db)²            (Gutman et al. 2014)
    ///
    /// SO:  contribution = isqrt64((da²+db²) × 10^12)     (floor √(da²+db²) × 10^6)
    ///      SO = |E| · da · √2 for Δ-regular (exact when 2Δ² perfect square, i.e. never for Δ≥1)
    /// RM₂: exact integer; 0 for pendant edges (da=1 or db=1); = |E|·(Δ-1)² for Δ-regular
    /// σ:   exact integer; 0 for regular graphs (da=db always); measures total degree imbalance
    pub fn graph_topo_indices4_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Undirected degree per compact-index node.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones() as u64;
        }

        // Integer floor-sqrt via Newton-Raphson.
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        // 4. Scan undirected edges (a < b canonical): accumulate SO, RM₂, σ.
        let mut so_acc:     u64 = 0;
        let mut rm2_acc:    u64 = 0;
        let mut sigma_acc:  u64 = 0;
        let mut edge_count: usize = 0;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b <= a { continue; } // each undirected edge once (a < b)

                let da = deg[a];
                let db = deg[b];

                // SO: floor(√(da²+db²) × 10^6) = isqrt64((da²+db²) × 10^12)
                so_acc += isqrt64((da * da + db * db) * 1_000_000_000_000u64);

                // RM₂: (da-1)·(db-1); pendant edges (da=1 or db=1) contribute 0
                if da > 0 && db > 0 {
                    rm2_acc += (da - 1) * (db - 1);
                }

                // σ: (da-db)²; 0 for regular graphs
                let diff = if da >= db { da - db } else { db - da };
                sigma_acc += diff * diff;

                edge_count += 1;
            }
        }

        (so_acc, rm2_acc, sigma_acc, edge_count, nc)
    }

    /// V3.16: Hyper-Zagreb HM₁ + HM₂ + Arithmetic-Geometric AG degree-based indices.
    ///
    /// Returns (hm1, hm2, ag_ppm, edge_count, node_count).
    ///   hm1    = HM₁(G) = Σ_{uv∈E} (da+db)²                      (exact; Shirdel et al. 2013)
    ///   hm2    = HM₂(G) = Σ_{uv∈E} (da·db)²                      (exact; Das & Trinajstić 2011)
    ///   ag_ppm = AG(G) × 10^6 where AG = Σ_{uv∈E} (da+db)/(2√(da·db))  (Zheng et al. 2020)
    ///
    /// HM₁: contribution = (s)² where s = da + db
    ///   HM₁ = 4·|E|·Δ² for Δ-regular
    /// HM₂: contribution = (p)² where p = da · db
    ///   HM₂ = |E|·Δ⁴ for Δ-regular
    /// AG:  contribution = floor(s·10^12 / (2·isqrt64(p·10^12)))
    ///   AG = |E| (= m) iff graph is regular (AM = GM when da = db); AG ≥ m always (AM-GM)
    ///
    /// KEY CROSS-CHECKS:
    ///   K₃ (Δ=2): HM₁=3×16=48; HM₂=3×16=48; AG=3_000_000 (regular, AM=GM)
    ///   K₄ (Δ=3): HM₁=6×36=216; HM₂=6×81=486; AG=6_000_000 (regular)
    ///   K_{1,4}: HM₁=4×25=100; HM₂=4×16=64; AG=4×1_250_000=5_000_000
    ///   K_{2,3}: HM₁=6×25=150; HM₂=6×36=216; AG=6×1_020_620=6_123_720
    pub fn graph_topo_indices5_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Undirected degree per compact-index node.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones() as u64;
        }

        // Integer floor-sqrt via Newton-Raphson.
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        // 4. Scan undirected edges (a < b canonical): accumulate HM₁, HM₂, AG.
        let mut hm1_acc:    u64 = 0;
        let mut hm2_acc:    u64 = 0;
        let mut ag_acc:     u64 = 0;
        let mut edge_count: usize = 0;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b <= a { continue; } // each undirected edge once (a < b)

                let da = deg[a];
                let db = deg[b];
                let p  = da * db; // product (> 0 since d ≥ 1 for vertices with edges)
                let s  = da + db; // sum

                // HM₁: (da+db)² = s²
                hm1_acc += s * s;

                // HM₂: (da·db)² = p²
                hm2_acc += p * p;

                // AG: floor(s × 10^12 / (2 × isqrt64(p × 10^12)))
                // = floor((da+db) × 10^6 / (2 × √(da·db)))
                // Denominator 2·x where x = isqrt64(p·10^12) ≥ 1 always (p ≥ 1).
                let x = isqrt64(p * 1_000_000_000_000u64);
                ag_acc += (s * 1_000_000_000_000u64) / (2 * x);

                edge_count += 1;
            }
        }

        (hm1_acc, hm2_acc, ag_acc, edge_count, nc)
    }

    /// V3.17: EM₁ + ABS + RRR degree-based topological indices.
    ///   em1    = EM₁(G)    where EM₁ = Σ_{uv∈E} (da+db-2)²               (exact u64; Milićević et al. 2004)
    ///   abs_ppm = ABS×10^6  where ABS = Σ_{uv∈E} √((da+db-2)/(da+db))    (floor isqrt64; Chen et al. 2022)
    ///   rrr_ppm = RRR×10^6  where RRR = Σ_{uv∈E} √((da-1)·(db-1))        (floor isqrt64; Li & Shi 2008)
    pub fn graph_topo_indices6_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
            }
        }

        // 3. Undirected degree per compact-index node.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones() as u64;
        }

        // Integer floor-sqrt via Newton-Raphson.
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        // 4. Scan undirected edges (a < b canonical): accumulate EM₁, ABS, RRR.
        let mut em1_acc:    u64 = 0;
        let mut abs_acc:    u64 = 0;
        let mut rrr_acc:    u64 = 0;
        let mut edge_count: usize = 0;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b <= a { continue; } // each undirected edge once (a < b)

                let da = deg[a];
                let db = deg[b];
                let s  = da + db;       // degree sum
                let q  = s - 2;         // s − 2  (≥ 0 since da,db ≥ 1 for connected nodes)
                let p1 = da - 1;        // da − 1 (for RRR)
                let p2 = db - 1;        // db − 1 (for RRR)

                // EM₁: (da+db-2)² = q²  (exact integer; 0 when both pendant)
                em1_acc += q * q;

                // ABS: floor(√((s-2)/s) × 10^6) = isqrt64(q × 10^12 / s)
                // When q=0 (da=db=1 pendant pair): isqrt64(0)=0 naturally (no skip needed).
                abs_acc += isqrt64(q * 1_000_000_000_000u64 / s);

                // RRR: floor(√((da-1)(db-1)) × 10^6) = isqrt64(p1 × p2 × 10^12)
                // When da=1 or db=1: p1 or p2=0, isqrt64(0)=0 naturally (no skip needed).
                rrr_acc += isqrt64(p1 * p2 * 1_000_000_000_000u64);

                edge_count += 1;
            }
        }

        (em1_acc, abs_acc, rrr_acc, edge_count, nc)
    }

    pub fn graph_topo_indices8_inner(&self) -> (u64, u64, u32, u32, usize, usize) {
        // Eccentricity-based topological indices:
        //   ECI = ξ(G) = Σ_v deg(v) × ecc(v)          (exact integer; Sharma, Goswami & Madan 1997)
        //   avg_ecc_ppm = (Σ_v ecc(v)) / n × 10^6     (floor ppm; Buckley & Harary 1990)
        //   D = diameter = max_{v} ecc(v)              (0 if nc≤1 or all isolated)
        //   R = radius   = min{ecc(v) | ecc(v)>0}     (0 if no connected pairs)
        //
        // ecc(v) = max BFS distance from v to any reachable node (0 for isolated nodes).
        // Algorithm: BFS from each node on undirected projection, O(n·(n+m)).

        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj  = [0u128; MAX_NODES];
        let mut deg  = [0u32; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                deg[f_ci] += 1;
                deg[t_ci] += 1;
                edge_count += 1;
            }
        }

        // 3. BFS from each source; compute eccentricity ecc[src].
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];
        let mut ecc   = [0u32; MAX_NODES];

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            let mut max_d = 0u32;
            for v in 0..nc {
                if v == src { continue; }
                if dist[v] != INF {
                    let d = dist[v] as u32;
                    if d > max_d { max_d = d; }
                }
            }
            ecc[src] = max_d;
        }

        // 4. Compute ECI, avg_ecc_ppm, diameter D, radius R.
        let mut eci        = 0u64;
        let mut ecc_sum    = 0u64;
        let mut diameter   = 0u32;
        let mut radius     = u32::MAX;
        let mut has_pos    = false;

        for ci in 0..nc {
            let e = ecc[ci];
            eci     += (deg[ci] as u64) * (e as u64);
            ecc_sum += e as u64;
            if e > diameter { diameter = e; }
            if e > 0 {
                if e < radius { radius = e; }
                has_pos = true;
            }
        }
        if !has_pos { radius = 0; }
        let avg_ecc_ppm = if nc == 0 { 0 } else { ecc_sum * 1_000_000 / (nc as u64) };

        (eci, avg_ecc_ppm, diameter, radius, edge_count, nc)
    }

    pub fn graph_topo_indices7_inner(&self) -> (u64, u64, u64, usize, usize) {
        // Distance-based topological indices: Wiener W, Harary H (ppm), Hyper-Wiener WW.
        //   W  = Σ_{u<v} d(u,v)                           (exact; Wiener 1947)
        //   H  = Σ_{u<v} 1/d(u,v) × 10^6                 (floor ppm; Plavšić et al. 1993)
        //   WW = (1/2) Σ_{u<v} [d(u,v) + d(u,v)²]        (exact; Klein & Randić 1993)
        //      = Σ_{u<v} d(u,v)·(d(u,v)+1)/2  (always integer: d·(d+1) is even)
        // Disconnected pairs (d=∞): contribute 0 to all three indices.
        // Algorithm: BFS from each node on undirected projection, O(n·(n+m)).

        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. BFS from each source; accumulate W, H_ppm, WW over pairs (src < v).
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];

        let mut wiener:        u64 = 0;
        let mut harary_ppm:    u64 = 0;
        let mut hyper_wiener:  u64 = 0;

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            for v in (src + 1)..nc {
                let d8 = dist[v];
                if d8 == INF { continue; }
                let d = d8 as u64;
                wiener       += d;
                harary_ppm   += 1_000_000 / d;
                hyper_wiener += d * (d + 1) / 2;
            }
        }

        (wiener, harary_ppm, hyper_wiener, edge_count, nc)
    }

    pub fn graph_topo_indices9_inner(&self) -> (u64, u64, u64, usize, usize) {
        // Degree-distance hybrid topological indices:
        //   W_S  = Schultz MTI  = Σ_{u<v} (deg(u)+deg(v)) × d(u,v)   (exact; Schultz 1989)
        //   W_G  = Gutman index = Σ_{u<v} deg(u)×deg(v)×d(u,v)        (exact; Gutman 1994)
        //   CξE  = Connective eccentric = Σ_v deg(v)/ecc(v) × 10^6    (floor ppm; Gupta et al. 2000)
        // Disconnected pairs (d=∞): contribute 0 to W_S and W_G.
        // Isolated nodes (ecc=0): contribute 0 to CξE (deg=0 as well).
        // Algorithm: BFS from each node on undirected projection, O(n·(n+m)).

        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + degree array.
        let mut adj       = [0u128; MAX_NODES];
        let mut deg       = [0u32;  MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                deg[f_ci] += 1;
                deg[t_ci] += 1;
                edge_count += 1;
            }
        }

        // 3. BFS from each source; accumulate W_S, W_G over pairs (src < v); record ecc[src].
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];
        let mut ecc   = [0u32; MAX_NODES];

        let mut ws: u64 = 0;
        let mut wg: u64 = 0;

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            let mut max_d = 0u32;
            for v in 0..nc {
                if v == src { continue; }
                let d8 = dist[v];
                if d8 == INF { continue; }
                let d = d8 as u64;
                if d as u32 > max_d { max_d = d as u32; }
                if v > src {
                    let ds = (deg[src] as u64) + (deg[v] as u64);
                    let dp = (deg[src] as u64) * (deg[v] as u64);
                    ws += ds * d;
                    wg += dp * d;
                }
            }
            ecc[src] = max_d;
        }

        // 4. Connective eccentric index CξE = Σ_v deg(v)/ecc(v) × 10^6.
        let mut cxe_ppm: u64 = 0;
        for ci in 0..nc {
            let e = ecc[ci];
            if e > 0 {
                cxe_ppm += (deg[ci] as u64) * 1_000_000 / (e as u64);
            }
        }

        (ws, wg, cxe_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices10_inner(&self) -> (u64, u64, u64, usize, usize) {
        // Edge-partition distance topological indices (BFS on undirected projection):
        //   Sz  = Σ_{uv∈E} n_u(uv)·n_v(uv)                        (exact; Gutman & Klavžar 1995)
        //   rSz = Σ_{uv∈E} (n_u+n₀/2)·(n_v+n₀/2) × 10^6          (floor ppm; Pisanski & Randić 2010)
        //   Mo  = Σ_{uv∈E} |n_u − n_v|                             (exact; Doslić et al. 2018)
        // For each undirected edge {a,b} and each vertex w:
        //   n_u = #{w : d(w,a) < d(w,b)},  n_v = #{w : d(w,a) > d(w,b)},
        //   n_0 = #{w : d(w,a) = d(w,b)}  (equidistant; only occurs on cycles)
        // Vertices in other components (both d=∞) excluded; a,b adjacent so same component.
        // rSz: (n_u+n₀/2)(n_v+n₀/2) may be a quarter-integer; stored as 4·rSz·250_000 = rSz×10^6.
        // Tree invariant: n₀=0 for every tree edge → Sz = rSz = Wiener index.
        // Vertex-transitive invariant: n_u = n_v for all edges → Mo = 0.

        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }
        if edge_count == 0 { return (0, 0, 0, 0, nc); }

        // 3. Build undirected edge list (canonical a < b).
        let mut ue_a   = [0u8; MAX_EDGES]; // endpoint a (compact index)
        let mut ue_b   = [0u8; MAX_EDGES]; // endpoint b (compact index)
        let mut ue_nu  = [0u8; MAX_EDGES]; // n_u per edge (≤nc≤128, fits u8)
        let mut ue_nv  = [0u8; MAX_EDGES]; // n_v per edge
        let mut ue_n0  = [0u8; MAX_EDGES]; // n_0 per edge
        let mut ue_cnt = 0usize;

        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    ue_a[ue_cnt] = a as u8;
                    ue_b[ue_cnt] = b as u8;
                    ue_cnt += 1;
                }
            }
        }

        // 4. BFS from each vertex w; classify w for every undirected edge.
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];

        for w in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[w] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = w as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }

            for ei in 0..ue_cnt {
                let da = dist[ue_a[ei] as usize];
                let db = dist[ue_b[ei] as usize];
                // a,b adjacent → same component: both INF (other component) or both finite.
                if da == INF { continue; }
                if da < db       { ue_nu[ei] += 1; }
                else if da > db  { ue_nv[ei] += 1; }
                else             { ue_n0[ei] += 1; }
            }
        }

        // 5. Accumulate Sz, 4·rSz (integer), Mo.
        let mut sz:    u64 = 0;
        let mut rsz_4: u64 = 0;
        let mut mo:    u64 = 0;

        for ei in 0..ue_cnt {
            let nu = ue_nu[ei] as u64;
            let nv = ue_nv[ei] as u64;
            let n0 = ue_n0[ei] as u64;
            sz    += nu * nv;
            rsz_4 += (2 * nu + n0) * (2 * nv + n0);
            mo    += if nu >= nv { nu - nv } else { nv - nu };
        }

        // rSz_ppm = rSz × 10^6 = (4·rSz) × 250_000.
        let rsz_ppm = rsz_4 * 250_000;
        (sz, rsz_ppm, mo, edge_count, nc)
    }

    pub fn graph_topo_indices11_inner(&self) -> (u64, u64, u64, usize, usize) {
        // Transmission-based topological indices (BFS on undirected projection):
        //   j_ppm  = J(G)  × 10^6  (floor ppm; Balaban 1982)
        //          J = (m/μ) × Σ_{uv∈E} 1/√(T_u·T_v)
        //          μ = max(1, m − n + 2)  (cyclomatic number proxy; avoids ÷0)
        //          T_v = Σ_{w reachable} d(v,w)  (vertex transmittance)
        //   ti     = TI(G) = Σ_{uv∈E} |T_u − T_v|  (exact; Abdo & Dimitrov 2014)
        //   piv    = PI_v(G) = Σ_{uv∈E} (T_u + T_v) (exact; Khalifeh et al. 2008)
        //          equivalently PI_v = Σ_v deg(v)·T_v
        // T_v = 0 iff v is isolated (no finite-distance reachable nodes other than self,
        //   but since {u,v} is an edge both T_u ≥ 1 and T_v ≥ 1 for any live edge).
        // J contribution per edge: isqrt64(10^12/(T_u·T_v)) — uses identity
        //   floor(10^6/√x) = floor(√(10^12/x)) valid for positive integer x.
        // Disconnected: T_v counts only within-component distances (BFS only
        //   reaches the same component); J uses μ = max(1, m−n+2) globally.

        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }
        if edge_count == 0 { return (0, 0, 0, 0, nc); }

        // 3. Build undirected edge list (canonical a < b).
        let mut ue_a   = [0u8; MAX_EDGES];
        let mut ue_b   = [0u8; MAX_EDGES];
        let mut ue_cnt = 0usize;
        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    ue_a[ue_cnt] = a as u8;
                    ue_b[ue_cnt] = b as u8;
                    ue_cnt += 1;
                }
            }
        }

        // 4. BFS from each vertex to compute T[v] = Σ_{w reachable} d(v,w).
        const INF: u8 = 255;
        let mut trans = [0u64; MAX_NODES]; // T[ci] = vertex transmittance
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            for v in 0..nc {
                if v != src && dist[v] != INF {
                    trans[src] += dist[v] as u64;
                }
            }
        }

        // 5. Accumulate J (raw_sum), TI, PI_v over undirected edges.
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        let mut j_raw: u64 = 0; // Σ floor(10^6/√(T_a·T_b)) per edge
        let mut ti:    u64 = 0;
        let mut piv:   u64 = 0;

        for ei in 0..ue_cnt {
            let ta = trans[ue_a[ei] as usize];
            let tb = trans[ue_b[ei] as usize];
            // ta,tb ≥ 1 since endpoints are in same connected component.
            let prod = ta * tb;
            // floor(10^6/√(ta·tb)) = floor(√(10^12/(ta·tb))) via isqrt64.
            if prod > 0 {
                let ratio = 1_000_000_000_000u64 / prod;
                j_raw += isqrt64(ratio);
            }
            ti  += if ta >= tb { ta - tb } else { tb - ta };
            piv += ta + tb;
        }

        // J_ppm = (m/μ) × raw_sum; μ = max(1, m − n + 2).
        let mu = if edge_count + 2 > nc { (edge_count + 2 - nc) as u64 } else { 1u64 };
        let j_ppm = j_raw * edge_count as u64 / mu;

        (j_ppm, ti, piv, edge_count, nc)
    }

    pub fn graph_topo_indices12_inner(&self) -> (u64, u64, u64, usize, usize) {
        // Zagreb eccentricity indices (BFS on undirected projection):
        //   m1e  = M1*(G) = Σ_v ecc(v)²                 (exact u64; Vukičević & Graovac 2010)
        //   m2e  = M2*(G) = Σ_{uv∈E} ecc(u)×ecc(v)      (exact u64; Das et al. 2013)
        //   m3e  = M3*(G) = Σ_{uv∈E} |ecc(u)−ecc(v)|    (exact u64; Farooq & Ali 2021)
        //
        // ecc(v) = max BFS distance from v to any reachable node (0 for isolated).
        // M3* = 0 iff self-centered (all ecc equal, e.g. Kn, even cycles, K_{r,s}).
        // M1*(Kn) = n; M2*(Kn) = m; M3*(Kn) = 0.

        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. BFS from each source; compute eccentricity ecc[src].
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];
        let mut ecc   = [0u64; MAX_NODES]; // 0 for isolated nodes

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            let mut max_d = 0u64;
            for v in 0..nc {
                if v != src && dist[v] != INF {
                    let d = dist[v] as u64;
                    if d > max_d { max_d = d; }
                }
            }
            ecc[src] = max_d;
        }

        // 4. M1* (node scan), M2* and M3* (undirected edge scan via a < b).
        let mut m1e = 0u64;
        for ci in 0..nc {
            m1e += ecc[ci] * ecc[ci];
        }

        let mut m2e = 0u64;
        let mut m3e = 0u64;
        for a in 0..nc {
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    m2e += ecc[a] * ecc[b];
                    m3e += if ecc[a] >= ecc[b] { ecc[a] - ecc[b] } else { ecc[b] - ecc[a] };
                }
            }
        }

        (m1e, m2e, m3e, edge_count, nc)
    }

    /// V3.24: Transmission Zagreb TM₁ + TM₂ + Geometric-Arithmetic transmission GA_t.
    ///   TM₁(G)  = Σ_v T_v²                                (exact u64)
    ///   TM₂(G)  = Σ_{uv∈E} T_u·T_v                       (exact u64)
    ///   GA_t(G) = Σ_{uv∈E} 2√(T_u·T_v)/(T_u+T_v) × 10^6 (floor ppm)
    ///
    /// T_v = Σ_{w reachable, w≠v} d(v,w) = vertex transmission.
    /// Isolated nodes: T_v=0, contribute 0 to TM₁, no edge contribution.
    /// GA_t = |E|×10^6 iff transmission-regular (all T_v equal).
    pub fn graph_topo_indices13_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. BFS from each source to compute vertex transmission T_v.
        //    T_v = sum of BFS distances to all reachable nodes (excluding self).
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];
        let mut trans = [0u64; MAX_NODES];

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            let mut tv = 0u64;
            for v in 0..nc {
                if v != src && dist[v] != INF {
                    tv += dist[v] as u64;
                }
            }
            trans[src] = tv;
        }

        // 4. Newton-Raphson integer square root for u128 (no float, no_std safe).
        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // 5. TM₁: node scan (Σ_v T_v²).
        let mut tm1 = 0u64;
        for ci in 0..nc {
            tm1 += trans[ci] * trans[ci];
        }

        // 6. TM₂ and GA_t: undirected edge scan (a < b).
        let mut tm2  = 0u64;
        let mut ga_t = 0u64;
        for a in 0..nc {
            let ta = trans[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let tb = trans[b];
                    tm2 += ta * tb;
                    if ta > 0 && tb > 0 {
                        // GA_t per edge = floor(2√(ta·tb) / (ta+tb) × 10^6)
                        //               = isqrt128(4·ta·tb·10^12) / (ta+tb)
                        let p = 4u128 * ta as u128 * tb as u128 * 1_000_000_000_000u128;
                        let s = (ta + tb) as u128;
                        ga_t += (isqrt128(p) / s) as u64;
                    }
                }
            }
        }

        (tm1, tm2, ga_t, edge_count, nc)
    }

    pub fn graph_topo_indices14_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. BFS from each source: compute ecc(v) and T_v simultaneously.
        //    ecc(v) = max BFS distance to any reachable node (0 for isolated).
        //    T_v    = vertex transmission = Σ_{w reachable, w≠v} d(v,w).
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];
        let mut ecc   = [0u8;  MAX_NODES];
        let mut trans = [0u64; MAX_NODES];

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            let mut max_d = 0u8;
            let mut tv   = 0u64;
            for v in 0..nc {
                if v != src && dist[v] != INF {
                    if dist[v] > max_d { max_d = dist[v]; }
                    tv += dist[v] as u64;
                }
            }
            ecc[src]   = max_d;
            trans[src] = tv;
        }

        // 4. Newton-Raphson isqrt64 (no float, no_std safe).
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let bits = 64u32 - n.leading_zeros();
            let mut x: u64 = 1u64 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // 5. Node scan: TE = Σ_v ecc(v);  EDS = Σ_v ecc(v)·T_v.
        let mut te  = 0u64;
        let mut eds = 0u64;
        for ci in 0..nc {
            let e = ecc[ci] as u64;
            te  += e;
            eds += e * trans[ci];
        }

        // 6. Edge scan (a < b): GEA = Σ_{uv∈E} 2√(ecc(u)·ecc(v))/(ecc(u)+ecc(v)) × 10^6.
        //    Per edge: isqrt64(4·ea·eb·10^12) / (ea+eb).
        //    4·ea·eb·10^12 ≤ 4·127²·10^12 ≈ 6.5×10^16 < u64::MAX — no overflow.
        //    GEA = |E|×10^6 iff graph is self-centered (all ecc equal, AM=GM).
        let mut gea = 0u64;
        for a in 0..nc {
            let ea = ecc[a] as u64;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let eb = ecc[b] as u64;
                    if ea > 0 && eb > 0 {
                        let s = ea + eb;
                        let p = 4 * ea * eb * 1_000_000_000_000u64;
                        gea += isqrt64(p) / s;
                    }
                }
            }
        }

        (te, eds, gea, edge_count, nc)
    }

    pub fn graph_topo_indices15_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. BFS from each source: compute d2[src] = |{w : d(src,w) = 2}|.
        //    d2(v) = 2-distance degree (Naji, Soner & Gutman 2017).
        const INF: u8 = 255;
        let mut dist  = [INF; MAX_NODES];
        let mut queue = [0u8; MAX_NODES];
        let mut d2    = [0u32; MAX_NODES];

        for src in 0..nc {
            for i in 0..nc { dist[i] = INF; }
            dist[src] = 0;
            let mut qhead = 0usize;
            let mut qtail = 0usize;
            queue[qtail] = src as u8; qtail += 1;
            while qhead < qtail {
                let cur   = queue[qhead] as usize; qhead += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    if dist[nb] == INF {
                        dist[nb] = d_cur + 1;
                        queue[qtail] = nb as u8; qtail += 1;
                    }
                }
            }
            let mut cnt2 = 0u32;
            for v in 0..nc {
                if dist[v] == 2 { cnt2 += 1; }
            }
            d2[src] = cnt2;
        }

        // 4. Node scan: LM1 = Σ_v d2(v)².
        let mut lm1 = 0u64;
        for ci in 0..nc {
            let d = d2[ci] as u64;
            lm1 += d * d;
        }

        // 5. Edge scan (a < b): LM2 = Σ_{uv∈E} d2(u)·d2(v); LM3 = Σ_{uv∈E} (d2(u)+d2(v)).
        let mut lm2 = 0u64;
        let mut lm3 = 0u64;
        for a in 0..nc {
            let da = d2[a] as u64;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let db = d2[b] as u64;
                    lm2 += da * db;
                    lm3 += da + db;
                }
            }
        }

        (lm1, lm2, lm3, edge_count, nc)
    }

    pub fn graph_topo_indices16_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Newton-Raphson integer sqrt (no float, no_std safe).
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let bits = 64u32 - n.leading_zeros();
            let mut x: u64 = 1u64 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // 4. Degree array from adj bitmasks.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc {
            deg[ci] = adj[ci].count_ones() as u64;
        }

        // 5. Node scan: Lz(G) = Σ_v d_v²·(n−1−d_v)  (Xia et al. 2019 Lanzhou Index).
        //    d_v ≤ nc−1 by construction → (nf−1−d) ≥ 0 always.
        let mut lz = 0u64;
        let nf = nc as u64;
        for ci in 0..nc {
            let d = deg[ci];
            lz += d * d * (nf - 1 - d);
        }

        // 6. Edge scan (a < b):
        //    ir_ppm = R_{1/2}(G)×10^6 = Σ_{uv∈E} √(d_u·d_v)×10^6    (Product Connectivity; Bollobás & Erdős 1998)
        //    rr_ppm = R_{-1}(G)×10^6  = Σ_{uv∈E} ⌊10^6/(d_u·d_v)⌋   (Reciprocal Randić; floor)
        //    Overflow guard: d_u·d_v ≤ 127²=16_129; 16_129×10^12 < u64::MAX — safe.
        let mut ir_ppm = 0u64;
        let mut rr_ppm = 0u64;
        for a in 0..nc {
            let da = deg[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let db = deg[b];
                    let p  = da * db;
                    ir_ppm += isqrt64(p * 1_000_000_000_000u64);
                    if p > 0 { rr_ppm += 1_000_000 / p; }
                }
            }
        }

        (ir_ppm, rr_ppm, lz, edge_count, nc)
    }

    // ── V3.28: Zagreb coindices M̄₁ + M̄₂ + forgotten coindex F̄ ─────────────
    //
    // Zagreb coindices sum over *non-edges* (pairs {u,v} with u≠v, uv∉E):
    //   M̄₁(G) = Σ_{uv∉E} (d_u + d_v)   (Ashrafi, Došlić & Hamzeh 2010)
    //   M̄₂(G) = Σ_{uv∉E} d_u · d_v     (Ashrafi, Došlić & Hamzeh 2010)
    //   F̄(G)  = Σ_{uv∉E} (d_u² + d_v²) (forgotten coindex; De 2016)
    //
    // Closed-form identities (all derivable from global degree sums):
    //   M̄₁ = 2m(n−1) − M₁      [M₁ = Σ_v d_v² = Σ_{uv∈E}(d_u+d_v)]
    //   M̄₂ = 2m² − M₁/2 − M₂  [M₂ = Σ_{uv∈E} d_u·d_v; M₁ always even]
    //   F̄  = (n−1)·M₁ − F      [F = Σ_v d_v³ (forgotten index)]
    //
    // Proof M₁ always even: M₁=Σ d_v² ≡ #{odd-degree vertices} (mod 2) = 0
    // (handshaking lemma: # odd-degree vertices is always even).
    //
    // Algorithm: single O(V+E) degree scan — no BFS, no complement scan.
    // Stack: adj[128](u128=2KB) + deg[128](u64=1KB) ≈ 3KB total.
    pub fn graph_topo_indices17_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array + accumulate M₁, M₂, F in a single scan.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        let mut m1 = 0u64; // Σ_v d²
        let mut m2 = 0u64; // Σ_{uv∈E} d_u·d_v
        let mut f  = 0u64; // Σ_v d³

        for ci in 0..nc {
            let d = deg[ci];
            m1 += d * d;
            f  += d * d * d;
        }
        for a in 0..nc {
            let da = deg[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a { m2 += da * deg[b]; }
            }
        }

        // 4. Apply closed-form identities.
        let m  = edge_count as u64;
        let n  = nc as u64;
        // M̄₁ = 2m(n−1) − M₁  (n≥1 guaranteed by early return above; 2m(n-1)≥M₁ always)
        let mbar1 = 2 * m * (n - 1) - m1;
        // M̄₂ = 2m² − M₁/2 − M₂  (M₁ always even; cf. proof above)
        let mbar2 = 2 * m * m - m1 / 2 - m2;
        // F̄  = (n−1)·M₁ − F
        let fbar  = (n - 1) * m1 - f;

        (mbar1, mbar2, fbar, edge_count, nc)
    }

    // ── V3.29: Neighborhood Zagreb NM₁ + NM₂ + GA₂ ──────────────────────────
    //
    // Let S(v) = Σ_{u∈N(v)} deg(u)  (sum of neighbor degrees; "2nd-order degree").
    //
    // NM₁(G) = Σ_v S(v)²                                          (Mondal et al. 2019)
    // NM₂(G) = Σ_{uv∈E} S(u)·S(v)                                (Mondal et al. 2019)
    // GA₂(G) = Σ_{uv∈E} 2√(S(u)·S(v))/(S(u)+S(v))              (neighborhood GA)
    //
    // KEY INVARIANTS:
    //   NM₁=NM₂=GA₂=0 for empty graph or all-isolated nodes (S(v)=0 everywhere).
    //   GA₂ = |E| × 10^6  when all S(v) are equal (S-uniform graph).
    //   NM₁=NM₂=GA₂=0 for single isolated node.
    //
    // S-UNIFORM EXAMPLES (all S equal):
    //   P₃: S(A)=S(B)=S(C)=2; K₃: S(v)=4; K₄: S(v)=9; K_{1,4}: S(v)=4; K_{2,3}: S(v)=6.
    //
    // Algorithm: O(V+E) — compute S(v) from adj+deg, then edge scan.
    // Stack: adj[128](u128=2KB) + deg[128](u64=1KB) + sv[128](u64=1KB) ≈ 4KB total.
    pub fn graph_topo_indices18_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. S(v) = Σ_{u∈N(v)} deg(u): neighbor-degree sum.
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Newton-Raphson integer square root for u128 (no float, no_std safe).
        //    S(v) ≤ Δ·(n-1) ≤ 127·127 = 16129; 4·S_u·S_v·10^12 ≤ 4·16129²·10^12 ≈ 10^21 — fits u128.
        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // 6. NM₁ (node scan) + NM₂ + GA₂ (undirected edge scan a < b).
        let mut nm1 = 0u64;
        let mut nm2 = 0u64;
        let mut ga2 = 0u64;

        for ci in 0..nc {
            nm1 += sv[ci] * sv[ci];
        }
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb = sv[b];
                    nm2 += sa * sb;
                    let s_sum = sa + sb;
                    if s_sum > 0 {
                        // GA₂ per edge = floor(2√(sa·sb)/(sa+sb) × 10^6)
                        //              = isqrt128(4·sa·sb·10^12) / (sa+sb)
                        let p = 4u128 * sa as u128 * sb as u128 * 1_000_000_000_000u128;
                        ga2 += (isqrt128(p) / s_sum as u128) as u64;
                    }
                }
            }
        }

        (nm1, nm2, ga2, edge_count, nc)
    }

    // ── V3.30: Reverse Wiener Λ + Reciprocal Complementary Wiener RCW + Terminal Wiener TW ─
    //
    // Λ(G)   = Σ_c [C(n_c,2) × D_c − W_c]                          (exact u64; Randić et al. 2000)
    //        = Σ_c [n_c(n_c-1)/2 × diam_c − Σ_{u<v in c} d(u,v)]
    //        = 0 iff all components have diam=1 (K_n blocks) or are singletons.
    //
    // RCW(G) = Σ_{u<v, connected} floor(10^6/(D_c+1-d(u,v)))         (floor ppm)
    //        complementary denominator D_c+1-d is maximised at d=1 (→D/1 fractional)
    //        and equals 1 at d=D_c (→1_000_000 floor).
    //        For complete K_n (D=1): all d=1 → denom=1 → each pair contributes 10^6.
    //
    // TW(G)  = Σ_{u<v, both pendant (deg=1)} d(u,v)                  (exact u64; Gutman et al. 2004)
    //        = 0 iff fewer than 2 pendant nodes in the graph.
    //
    // DISCONNECTED GRAPHS: each component c contributes independently.
    //   Λ: per-component C(n_c,2)×D_c − W_c (isolates: C(1,2)=0, contribution=0).
    //   RCW: pairs within the same component only; per-component D_c.
    //   TW: pendant pairs across the whole graph (regardless of component).
    //
    // ALGORITHM: two BFS phases O(n(n+m)) total.
    //   Phase 0: component detection O(V+E).
    //   Phase 1: BFS from each node → ecc[], comp_wiener[], TW.
    //   Phase 2: BFS from each node → RCW (needs per-component D_c from phase 1).
    //   Stack: adj[128](u128=2KB) + aux arrays (~2KB) + dist/queue(256B) ≈ 4.5KB total.
    pub fn graph_topo_indices19_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Phase 0: connected-component detection O(V+E).
        //    comp_id[ci]: component index (0..n_comp-1); u8::MAX = unvisited.
        let mut comp_id   = [u8::MAX; MAX_NODES];
        let mut comp_size = [0u64;    MAX_NODES]; // n_c per component
        let mut n_comp    = 0usize;
        let mut dist      = [255u8;   MAX_NODES];
        let mut queue     = [0u8;     MAX_NODES];

        for start in 0..nc {
            if comp_id[start] != u8::MAX { continue; }
            let c = n_comp; n_comp += 1;
            comp_id[start] = c as u8;
            comp_size[c]  += 1;
            let mut qh = 0usize; let mut qt = 0usize;
            queue[qt] = start as u8; qt += 1;
            while qh < qt {
                let cur = queue[qh] as usize; qh += 1;
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize; bits &= bits - 1;
                    if comp_id[nb] == u8::MAX {
                        comp_id[nb] = c as u8;
                        comp_size[c] += 1;
                        queue[qt] = nb as u8; qt += 1;
                    }
                }
            }
        }

        // 4. Phase 1: BFS from each node → ecc[], comp_wiener[], TW.
        //    Pendant detection: adj[ci].count_ones() == 1.
        let mut ecc         = [0u8;  MAX_NODES]; // eccentricity (max BFS distance from ci)
        let mut comp_wiener = [0u64; MAX_NODES]; // Wiener index per component
        let mut tw          = 0u64;

        for src in 0..nc {
            if adj[src] == 0 { continue; } // isolated: no reachable pairs
            for i in 0..nc { dist[i] = 255; }
            dist[src] = 0;
            let mut qh = 0usize; let mut qt = 0usize;
            queue[qt] = src as u8; qt += 1;
            while qh < qt {
                let cur = queue[qh] as usize; qh += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize; bits &= bits - 1;
                    if dist[nb] == 255 {
                        dist[nb] = d_cur + 1;
                        queue[qt] = nb as u8; qt += 1;
                    }
                }
            }
            let c            = comp_id[src] as usize;
            let src_pendant  = adj[src].count_ones() == 1;
            let mut max_d    = 0u8;
            for v in (src + 1)..nc {
                let d = dist[v];
                if d == 255 { continue; }
                if d > max_d { max_d = d; }
                comp_wiener[c] += d as u64;
                if src_pendant && adj[v].count_ones() == 1 {
                    tw += d as u64;
                }
            }
            if max_d > ecc[src] { ecc[src] = max_d; }
        }

        // 5. Per-component diameter: D_c = max(ecc[v]) for v in c.
        let mut comp_diam = [0u8; MAX_NODES];
        for ci in 0..nc {
            let c = comp_id[ci] as usize;
            if ecc[ci] > comp_diam[c] { comp_diam[c] = ecc[ci]; }
        }

        // 6. Phase 2: BFS from each node → RCW (uses comp_diam).
        let mut rcw_ppm = 0u64;
        for src in 0..nc {
            if adj[src] == 0 { continue; }
            for i in 0..nc { dist[i] = 255; }
            dist[src] = 0;
            let mut qh = 0usize; let mut qt = 0usize;
            queue[qt] = src as u8; qt += 1;
            while qh < qt {
                let cur = queue[qh] as usize; qh += 1;
                let d_cur = dist[cur];
                let mut bits = adj[cur];
                while bits != 0 {
                    let nb = bits.trailing_zeros() as usize; bits &= bits - 1;
                    if dist[nb] == 255 {
                        dist[nb] = d_cur + 1;
                        queue[qt] = nb as u8; qt += 1;
                    }
                }
            }
            let d_c = comp_diam[comp_id[src] as usize] as u64;
            for v in (src + 1)..nc {
                let d = dist[v];
                if d == 255 { continue; }
                // RCW per pair = floor(10^6 / (D_c + 1 − d(u,v)))
                // denom ≥ 1 always since d ≤ D_c.
                rcw_ppm += 1_000_000 / (d_c + 1 - d as u64);
            }
        }

        // 7. Reverse Wiener: Λ = Σ_c [C(n_c,2) × D_c − W_c].
        //    W_c ≤ C(n_c,2) × D_c always (d ≤ D_c for all pairs), so no underflow.
        let mut rw = 0u64;
        for c in 0..n_comp {
            let n_c  = comp_size[c];              // nodes in component c
            let d_c  = comp_diam[c] as u64;       // diameter of c
            let w_c  = comp_wiener[c];            // Wiener index of c
            let pairs = n_c * (n_c - 1) / 2;     // C(n_c, 2)
            rw += pairs * d_c - w_c;
        }

        (rw, rcw_ppm, tw, edge_count, nc)
    }

    // ── V3.31: Modified Sombor SO* + Reciprocal Sombor RSO + Reduced Sombor rSO ──
    //
    // Sombor-family variants (all O(V+E) degree-scan, no BFS):
    //   SO*(G) × 10^6 = Σ_{uv∈E} d_u·d_v / √(d_u²+d_v²) × 10^6   (floor ppm; Ghanbari & Rajabi-Parsa 2021)
    //   RSO(G) × 10^6 = Σ_{uv∈E} 10^6 / √(d_u²+d_v²)              (floor ppm; Gutman 2022)
    //   rSO(G) × 10^6 = Σ_{uv∈E} √((d_u−1)²+(d_v−1)²) × 10^6     (floor ppm; Doslic et al. 2022)
    //
    // Overflow safety (all max-degree=127):
    //   SO*: (d_a·d_b)²·10^12 ≤ (127²)²·10^12 = 260,144,641·10^12 < u128::MAX ✓
    //   RSO: 10^12/(d_a²+d_b²) ≤ 10^12/2 = 500,000,000,000 < u64::MAX ✓
    //   rSO: ((d_a-1)²+(d_b-1)²)·10^12 ≤ 2·126²·10^12 ≈ 3.2×10^16 < u64::MAX ✓
    //
    // Key invariants:
    //   SO*(Δ-regular) = m·Δ/√2·10^6 per edge (≥ m·10^6 for Δ≥√2; AM-GM: d_a·d_b ≤ (d_a²+d_b²)/2)
    //   RSO(Δ-regular) = m·10^6/(Δ√2) per edge (decreasing in Δ)
    //   rSO=0 iff all edges are pendant-pendant (d_u=d_v=1); pendant-hub d_u=1,d_v=k: rSO contribution=(k-1)·10^6 (exact)
    //   SO* per edge = RSO per edge × (d_u·d_v) (the RSO weight is 1, the SO* weight is d_u·d_v)
    //
    // Algorithm: O(V+E) — degree array from adj bitmasks; single undirected edge scan.
    // Stack: adj[128](u128=2KB) + deg[128](u64=1KB) ≈ 3KB total.
    pub fn graph_topo_indices20_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Newton-Raphson integer sqrt helpers (no float, no_std safe).
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let bits = 64u32 - n.leading_zeros();
            let mut x: u64 = 1u64 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }
        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // 4. Degree array from adj bitmasks.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 5. Edge scan (a < b): accumulate SO*, RSO, rSO.
        //    SO* formula:  isqrt128((d_a·d_b)²·10^12 / (d_a²+d_b²))  [identity: floor(A/√B)=floor(√(A²/B))]
        //    RSO formula:  isqrt64(10^12 / (d_a²+d_b²))               [same identity, A=10^6]
        //    rSO formula:  isqrt64(((d_a-1)²+(d_b-1)²)·10^12)         [floor(√C·10^6)=isqrt64(C·10^12)]
        let mut so_star_ppm = 0u64;
        let mut rso_ppm     = 0u64;
        let mut rso_red_ppm = 0u64;
        for a in 0..nc {
            let da = deg[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let db       = deg[b];
                    let sum_sq   = da * da + db * db;       // d_a² + d_b² ≥ 2
                    let prod     = da * db;                  // d_a · d_b
                    let so_num   = (prod as u128) * (prod as u128) * 1_000_000_000_000u128;
                    so_star_ppm += isqrt128(so_num / (sum_sq as u128)) as u64;
                    rso_ppm     += isqrt64(1_000_000_000_000u64 / sum_sq);
                    let ra       = da - 1;  // ≥ 0; da ≥ 1 guaranteed (adj[a] ≠ 0)
                    let rb       = db - 1;  // ≥ 0; db ≥ 1 guaranteed
                    let red_sq   = ra * ra + rb * rb;
                    rso_red_ppm += isqrt64(red_sq * 1_000_000_000_000u64);
                }
            }
        }

        (so_star_ppm, rso_ppm, rso_red_ppm, edge_count, nc)
    }

    /// V3.32 — ABC₄ + Neighborhood Harmonic NH + Neighborhood Sombor NSO
    ///
    /// All three use S(v) = Σ_{w∈N(v)} d(w) (neighbor degree sum), computed in
    /// one O(V+E) degree scan — no BFS needed.
    ///
    /// abc4_ppm  = ABC₄(G) × 10^6 = Σ_{uv∈E} √((S_u+S_v−2)/(S_u·S_v)) × 10^6
    /// nh_ppm    = NH(G)  × 10^6 = Σ_{uv∈E} 2/(S_u+S_v)              × 10^6
    /// nso_ppm   = NSO(G) × 10^6 = Σ_{uv∈E} √(S_u²+S_v²)             × 10^6
    ///
    /// Formulas (no float, no_std safe):
    ///   ABC₄ per edge: isqrt64((S_u+S_v−2)×10^12/(S_u×S_v)); 0 if S_u+S_v=2
    ///   NH   per edge: 2_000_000 / (S_u+S_v)  (integer floor)
    ///   NSO  per edge: isqrt128((S_u²+S_v²) as u128 × 10^12) cast to u64
    pub fn graph_topo_indices21_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Integer sqrt helpers (Newton-Raphson, no float, no_std safe).
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let bits = 64u32 - n.leading_zeros();
            let mut x: u64 = 1u64 << ((bits + 1) / 2);
            loop { let y = (x + n / x) / 2; if y >= x { return x; } x = y; }
        }
        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop { let y = (x + n / x) / 2; if y >= x { return x; } x = y; }
        }

        // 4. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 5. Neighbor degree sum S(v) = Σ_{w∈N(v)} d(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 6. Edge scan (a < b): accumulate ABC₄, NH, NSO.
        //    ABC₄: isqrt64((S_u+S_v−2)×10^12/(S_u×S_v))  [0 when S_u+S_v=2]
        //    NH:   2_000_000 / (S_u+S_v)                   [floor integer]
        //    NSO:  isqrt128((S_u²+S_v²)×10^12) as u64      [u128 to avoid overflow for large S]
        let mut abc4_ppm = 0u64;
        let mut nh_ppm   = 0u64;
        let mut nso_ppm  = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b];
                    let ssum = sa + sb;
                    // ABC₄
                    if ssum > 2 {
                        abc4_ppm += isqrt64((ssum - 2) * 1_000_000_000_000u64 / (sa * sb));
                    }
                    // NH (neighborhood harmonic)
                    nh_ppm += 2_000_000u64 / ssum;
                    // NSO (neighborhood Sombor) — use u128 to prevent s²×10^12 overflow
                    let nso_n = ((sa * sa + sb * sb) as u128) * 1_000_000_000_000u128;
                    nso_ppm += isqrt128(nso_n) as u64;
                }
            }
        }

        (abc4_ppm, nh_ppm, nso_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices22_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Integer sqrt helper (Newton-Raphson, no float, no_std safe).
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let bits = 64u32 - n.leading_zeros();
            let mut x: u64 = 1u64 << ((bits + 1) / 2);
            loop { let y = (x + n / x) / 2; if y >= x { return x; } x = y; }
        }

        // 4. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 5. Neighbor degree sum S(v) = Σ_{w∈N(v)} d(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 6. Neighborhood Forgotten NF = Σ_v S(v)³  (exact u64; node scan).
        //    S(v) ≤ 127² = 16129; S³ ≤ 16129³ ≈ 4.2×10^12; 128×4.2×10^12 < u64::MAX.
        let mut nf = 0u64;
        for ci in 0..nc { nf += sv[ci] * sv[ci] * sv[ci]; }

        // 7. Edge scan (a < b): accumulate NR and NSC.
        //    NR  = Σ_{uv∈E} 1/√(S_u·S_v)·10^6  = Σ isqrt64(10^12/(S_u·S_v))
        //    NSC = Σ_{uv∈E} 1/√(S_u+S_v)·10^6  = Σ isqrt64(10^12/(S_u+S_v))
        //    Both have S_u,S_v ≥ 1 at edge endpoints (isolated nodes have no edges).
        let mut nr_ppm  = 0u64;
        let mut nsc_ppm = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b];
                    let sp   = sa * sb;
                    let ssum = sa + sb;
                    nr_ppm  += isqrt64(1_000_000_000_000u64 / sp);
                    nsc_ppm += isqrt64(1_000_000_000_000u64 / ssum);
                }
            }
        }

        (nr_ppm, nf, nsc_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices23_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Edge scan (a < b): accumulate NHM1, NSDD, NM3.
        //    NHM1 = Σ_{uv∈E} (S_u+S_v)²              (exact u64; S-analogue of HM₁)
        //    NSDD = Σ_{uv∈E} (S_u²+S_v²)/(S_u·S_v)  (floor ppm; S-analogue of SDD; ≥2|E|×10^6)
        //    NM3  = Σ_{uv∈E} |S_u−S_v|               (exact u64; S-analogue of M₃ irregularity; =0 iff S-regular)
        //
        //  Overflow safety:
        //    NHM1: (ssum)² ≤ 32258² ≈ 1.04×10^9 per edge; 512 edges × 10^9 < u64::MAX ✓
        //    NSDD: (sa²+sb²)×10^6 ≤ 5.2×10^14 per edge; sum over 512 edges < u64::MAX ✓
        //    NM3:  |sa−sb| ≤ 16129 per edge; 512×16129 < u64::MAX ✓
        let mut nhm1     = 0u64;
        let mut nsdd_ppm = 0u64;
        let mut nm3      = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b];
                    let ssum = sa + sb;
                    let sp   = sa * sb;
                    // NHM1 = (S_u+S_v)²
                    nhm1 += ssum * ssum;
                    // NSDD = floor((S_u²+S_v²)×10^6 / (S_u·S_v)); sp=0 only if sa or sb=0 (isolated nodes have no edges)
                    if sp > 0 {
                        nsdd_ppm += (sa * sa + sb * sb) * 1_000_000u64 / sp;
                    }
                    // NM3 = |S_u−S_v|
                    nm3 += if sa >= sb { sa - sb } else { sb - sa };
                }
            }
        }

        (nhm1, nsdd_ppm, nm3, edge_count, nc)
    }

    pub fn graph_topo_indices24_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Edge scan (a < b): accumulate NISI, NAZI, NEM1.
        //    NISI_ppm = Σ_{uv∈E} floor(S_u·S_v·10^6/(S_u+S_v))     (floor ppm; S-analogue of ISI)
        //    NAZI_milli = Σ_{uv∈E} floor((S_u·S_v)^3·10^3/(S_u+S_v-2)^3)  (floor milli; S-analogue of AZI;
        //                 skip edges with S_u+S_v=2, i.e. K₂-type pendant pairs)
        //    NEM1     = Σ_{uv∈E} (S_u+S_v-2)^2                      (exact u64; S-analogue of EM₁)
        //
        //  Overflow safety:
        //    NISI:  S_u·S_v·10^6 ≤ 16129²×10^6 ≈ 2.6×10^17 < u64::MAX ✓
        //    NAZI:  (S_u·S_v)^3 ≤ (16129²)^3 ≈ 1.76×10^25 → needs u128 intermediate;
        //           after /q^3: per-edge result ≤ ~5.24×10^14 × 1000 ≈ 5.24×10^14, fits u64 ✓
        //    NEM1:  (ssum-2)^2 ≤ 32256^2 ≈ 10^9 per edge; ×8065 edges ≈ 8×10^12 < u64::MAX ✓
        let mut nisi_ppm  = 0u64;
        let mut nazi_milli = 0u64;
        let mut nem1       = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b];
                    let ssum = sa + sb;
                    let sp   = sa * sb;
                    // NISI = floor(S_u·S_v·10^6/(S_u+S_v))
                    nisi_ppm += sp * 1_000_000u64 / ssum;
                    // NAZI = floor((S_u·S_v)^3·10^3/(S_u+S_v-2)^3); skip when q=0 (S_u=S_v=1)
                    if ssum > 2 {
                        let q = ssum - 2;
                        let sp3 = (sp as u128) * (sp as u128) * (sp as u128);
                        let q3  = (q  as u128) * (q  as u128) * (q  as u128);
                        nazi_milli += (sp3 * 1000 / q3) as u64;
                    }
                    // NEM1 = (S_u+S_v-2)^2
                    let q64 = if ssum >= 2 { ssum - 2 } else { 0 };
                    nem1 += q64 * q64;
                }
            }
        }

        (nisi_ppm, nazi_milli, nem1, edge_count, nc)
    }

    pub fn graph_topo_indices25_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Edge scan (a < b): accumulate NHM₂, NAG, NABS.
        //
        //    NHM₂     = Σ_{uv∈E} (S_u·S_v)²                          (exact; u128 acc → u64)
        //    NAG_ppm  = Σ_{uv∈E} floor((S_u+S_v)·10^6 / (2√(S_u·S_v)))
        //             = Σ_{uv∈E} floor(ssum·10^12 / (2·isqrt128(sp·10^12)))
        //    NABS_ppm = Σ_{uv∈E} floor(√((S_u+S_v-2)/(S_u+S_v))·10^6)
        //             = Σ_{uv∈E} isqrt64((ssum-2)·10^12 / ssum)
        //
        //  Overflow safety:
        //    NHM₂:  sp = S_u·S_v ≤ 16129² = 260_144_641; sp² ≤ 6.77×10^16 < u64::MAX ✓
        //           accumulator: u128 (sum could exceed u64 for dense large graphs)
        //    NAG:   sp·10^12 ≤ 260_144_641·10^12 ≈ 2.6×10^20 → use u128 for isqrt128 ✓
        //           ssum·10^12 ≤ 32258·10^12 ≈ 3.2×10^16 < u64::MAX ✓
        //           result ≤ ssum·10^6/(2·1) = 16129·10^6 ≈ 1.6×10^13 < u64::MAX per edge ✓
        //    NABS:  (ssum-2)·10^12/ssum ≤ 32256·10^12 ≈ 3.2×10^16 < u64::MAX ✓
        //           result ≤ 10^6 per edge ✓

        // Integer square root u128 — bit-shift init, Babylonian, no float, no_std safe.
        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // Integer square root u64 — Babylonian method, no float, no_std safe.
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        let mut nhm2_acc: u128 = 0;
        let mut nag_ppm       = 0u64;
        let mut nabs_ppm      = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b];
                    let ssum = sa + sb;
                    let sp   = sa * sb;
                    // NHM₂ = (S_u·S_v)²
                    nhm2_acc += (sp as u128) * (sp as u128);
                    // NAG = floor(ssum·10^6 / (2·√sp))
                    let isqrt_sp = isqrt128((sp as u128) * 1_000_000_000_000u128);
                    if isqrt_sp > 0 {
                        nag_ppm += (ssum * 1_000_000_000_000u64) / (2 * isqrt_sp as u64);
                    }
                    // NABS = floor(√((ssum-2)/ssum)·10^6) = isqrt64((ssum-2)·10^12/ssum)
                    if ssum >= 2 {
                        nabs_ppm += isqrt64((ssum - 2) * 1_000_000_000_000u64 / ssum);
                    }
                }
            }
        }

        (nhm2_acc as u64, nag_ppm, nabs_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices26_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Edge scan (a < b): accumulate NPC, NRM₂, NRSO.
        //
        //    NPC_ppm  = Σ_{uv∈E} floor(√(S_u·S_v)·10^6)
        //             = Σ_{uv∈E} isqrt128(S_u·S_v·10^12)         (S-analogue of R_{1/2})
        //    NRM2     = Σ_{uv∈E} (S_u-1)·(S_v-1)                 (exact u64; S-analogue of RM₂)
        //    NRSO_ppm = Σ_{uv∈E} floor(10^6/√(S_u²+S_v²))
        //             = Σ_{uv∈E} isqrt64(10^12/(S_u²+S_v²))      (S-analogue of RSO)
        //
        //  Overflow safety:
        //    NPC:  S_u·S_v ≤ 16129² ≈ 2.6×10^8; ×10^12 ≈ 2.6×10^20 → u128 for isqrt128 ✓
        //          result ≤ 16129·10^6 ≈ 1.6×10^10 per edge; total ≤ 512·1.6×10^10 < u64::MAX ✓
        //    NRM₂: (S_u-1)·(S_v-1) ≤ 16128² ≈ 2.6×10^8 < u64::MAX; total < u64::MAX ✓
        //    NRSO: S_u²+S_v² ≥ 2 for all edges; denominator ≤ 2·16129² ≈ 5.2×10^8;
        //          10^12/denom ≤ 5×10^11 < u64::MAX; result ≤ 707_107 per edge ✓

        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let mut x = n;
            let mut y = (x + 1) / 2;
            while y < x { x = y; y = (x + n / x) / 2; }
            x
        }

        let mut npc_ppm  = 0u64;
        let mut nrm2     = 0u64;
        let mut nrso_ppm = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb = sv[b];
                    // NPC = floor(√(S_u·S_v)·10^6) = isqrt128(S_u·S_v·10^12)
                    let sp = sa * sb;
                    npc_ppm += isqrt128((sp as u128) * 1_000_000_000_000u128) as u64;
                    // NRM₂ = (S_u-1)·(S_v-1)  [0 if either S=0, u64 saturating-sub guards underflow]
                    let pa = sa.saturating_sub(1);
                    let pb = sb.saturating_sub(1);
                    nrm2 += pa * pb;
                    // NRSO = isqrt64(10^12 / (S_u²+S_v²))
                    let denom = sa * sa + sb * sb;
                    if denom > 0 {
                        nrso_ppm += isqrt64(1_000_000_000_000u64 / denom);
                    }
                }
            }
        }

        (npc_ppm, nrm2, nrso_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices27_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Edge scan (a < b): accumulate NRR, NSO*, NrSO.
        //
        //    NRR_ppm  = Σ_{uv∈E} floor(10^6/(S_u·S_v))                         (S-analogue of R_{-1})
        //    NSOs_ppm = Σ_{uv∈E} isqrt128(S_u²·S_v²·10^12/(S_u²+S_v²))        (S-analogue of SO*)
        //    NrSO_ppm = Σ_{uv∈E} isqrt128(((S_u-1)²+(S_v-1)²)·10^12) as u64   (S-analogue of rSO)
        //
        //  Overflow:
        //    NRR:  sp≥1; floor(10^6/sp)≤10^6; total≤512×10^6 < u64::MAX ✓
        //    NSOs: S²≤16129²≈2.6×10^8; S²·S²·10^12≤~6.8×10^28 < u128::MAX ✓
        //          result per edge ≤ 16129²/√2·10^6 ≈ 1.8×10^14; total < u64::MAX ✓
        //    NrSO: (S-1)²≤16128²≈2.6×10^8; sum×10^12≤5.2×10^20 > u64::MAX → u128 needed;
        //          result per edge ≤ √(5.2×10^20) ≈ 7.2×10^10; total < u64::MAX ✓

        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        let mut nrr_ppm  = 0u64;
        let mut nsos_ppm = 0u64;
        let mut nrso_ppm = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb = sv[b];
                    // NRR = floor(10^6/(S_u·S_v))
                    let sp = sa * sb;
                    if sp > 0 { nrr_ppm += 1_000_000u64 / sp; }
                    // NSO* = isqrt128(S_u²·S_v²·10^12/(S_u²+S_v²))
                    let ss2 = sa * sa + sb * sb;
                    if ss2 > 0 {
                        let sp2 = (sa as u128) * (sa as u128) * (sb as u128) * (sb as u128);
                        nsos_ppm += isqrt128(sp2 * 1_000_000_000_000u128 / (ss2 as u128)) as u64;
                    }
                    // NrSO = isqrt128(((S_u-1)²+(S_v-1)²)·10^12)
                    let pa = sa.saturating_sub(1) as u128;
                    let pb = sb.saturating_sub(1) as u128;
                    nrso_ppm += isqrt128((pa * pa + pb * pb) * 1_000_000_000_000u128) as u64;
                }
            }
        }

        (nrr_ppm, nsos_ppm, nrso_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices28_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Edge scan (a < b): accumulate NNI, NNMI, NSM1.
        //
        //    NNI_ppm  = Σ_{uv∈E} isqrt64((S_u+S_v)×10^12)              (floor ppm; S-Nirmala)
        //    NNMI_ppm = Σ_{uv∈E} (S_u+S_v)×isqrt64((S_u+S_v)×10^12)   (floor ppm; S-Modified Nirmala)
        //    NSM1     = Σ_{uv∈E} (S_u+S_v)                              (exact u64; S-edge M₁)
        //
        //  NNMI identity: floor((S+S)^{3/2}×10^6) = (S_u+S_v)×floor(√(S_u+S_v)×10^6)
        //  because (S_u+S_v) is integer ⟹ it factors out of the floor.
        //
        //  Overflow:
        //    ssum×10^12: max ssum=32258; 32258×10^12=3.23×10^16 < u64::MAX ✓
        //    NNMI acc:   per edge ≤32258×179_606_381≈5.79×10^12; sum≤8128×5.79×10^12≈4.71×10^16 < u64::MAX ✓
        //    NSM1:       sum ≤ 8128×32258 ≈ 2.62×10^8 << u64::MAX ✓

        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let bits = 64u32 - n.leading_zeros();
            let mut x: u64 = 1u64 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        let mut nni_ppm  = 0u64;
        let mut nnmi_ppm = 0u64;
        let mut nsm1     = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb  = sv[b];
                    let ssum = sa + sb;
                    // NNI per edge = floor(√(S_u+S_v) × 10^6) = isqrt64(ssum × 10^12)
                    let nni_e = isqrt64(ssum * 1_000_000_000_000u64);
                    nni_ppm  += nni_e;
                    // NNMI per edge = floor((S_u+S_v)^{3/2} × 10^6) = ssum × nni_e
                    nnmi_ppm += ssum * nni_e;
                    // NSM1 = exact sum of (S_u+S_v) over all edges
                    nsm1 += ssum;
                }
            }
        }

        (nni_ppm, nnmi_ppm, nsm1, edge_count, nc)
    }

    pub fn graph_topo_indices29_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. isqrt64 helper (Newton-Raphson, no_std safe).
        fn isqrt64(n: u64) -> u64 {
            if n == 0 { return 0; }
            let bits = 64u32 - n.leading_zeros();
            let mut x: u64 = 1u64 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // 6. Node scan: NZ₀ (inverse sqrt vertex sum) and NSe (sqrt vertex sum).
        //
        //    NZ₀_ppm = Σ_{v: S(v)>0} isqrt64(10^12 / S(v))  (floor ppm; S-zero-order Randić)
        //    NSe_ppm = Σ_v             isqrt64(S(v) × 10^12) (floor ppm; S-sqrt vertex sum)
        //
        //    Overflow: 10^12/S(v) ≤ 10^12 < u64::MAX ✓
        //              S(v)×10^12 ≤ 16129×10^12 = 1.61×10^16 < u64::MAX ✓
        //    Node-sum ≤ 127 × 10^6 ≈ 1.27×10^8 << u64::MAX ✓

        let mut nz0_ppm = 0u64;
        let mut nse_ppm = 0u64;
        for ci in 0..nc {
            let s = sv[ci];
            if s > 0 {
                nz0_ppm += isqrt64(1_000_000_000_000u64 / s);
            }
            nse_ppm += isqrt64(s * 1_000_000_000_000u64);
        }

        // 7. Edge scan (a < b): accumulate NEM₂.
        //
        //    NEM₂ = Σ_{uv∈E} S_u · S_v · (S_u+S_v−2)  (exact u64; S-Reformulated 2nd Zagreb)
        //
        //    EM₂(G) = Σ_{uv∈E} d_u·d_v·(d_u+d_v-2)  [Miličević et al. 2004]
        //    NEM₂=0 iff all edges have S_u+S_v=2 (both S=1; only K₂-type pairs).
        //    NEM₂ = |E|·S²·(2S-2) for S-regular; = 0 for K₂ (ssum=2, factor=0).
        //
        //    Overflow: max per-edge = 16129×16129×32256 ≈ 8.39×10^12 < u64::MAX ✓
        //              sum ≤ 8128 × 8.39×10^12 ≈ 6.82×10^16 < u64::MAX ✓

        let mut nem2 = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b];
                    let ssum = sa + sb;
                    nem2 += sa * sb * ssum.saturating_sub(2);
                }
            }
        }

        (nz0_ppm, nem2, nse_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices30_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. isqrt128 helper (Newton-Raphson, no_std safe).
        fn isqrt128(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        // 6. Vertex scan: NVQ (S-Quartic vertex sum).
        //
        //    NVQ(G) = Σ_v S(v)^4  (exact u64; S-analogue of 4th-power vertex sum)
        //
        //    Extension of NM₁ = Σ S² (topo18) and NF = Σ S³ (topo22) to 4th power.
        //    NVQ = n·S^4 for S-regular.
        //
        //    Overflow: S(v) ≤ 127² = 16129; S^4 ≤ 16129^4 ≈ 6.77×10^16 < u64::MAX ✓
        //              sum ≤ 128 × 6.77×10^16 ≈ 8.67×10^18 < u64::MAX ✓

        let mut nvq = 0u64;
        for ci in 0..nc {
            let s2 = sv[ci] * sv[ci];
            nvq += s2 * s2;
        }

        // 7. Edge scan (a < b): NRGS (S-Generalized Randić 3/2) and NHCS (S-Cubic-Sum).
        //
        //    NRGS(G) × 10^6 = Σ_{uv∈E} isqrt128((S_u·S_v)^3 × 10^12)
        //                   = Σ_{uv∈E} (S_u·S_v)^{3/2} × 10^6          (floor ppm)
        //    S-analogue of Generalized Randić χ_{3/2}(G) = Σ_{uv∈E} (d_u·d_v)^{3/2}.
        //    NRGS = |E| × S^3 × 10^6 for S-regular (exact: (S²)^{3/2} = S^3).
        //    K₃ and K_{1,4} share same per-edge NRGS (S-uniform S=4; (16)^{3/2}=64).
        //
        //    Overflow: (S_u·S_v)^3 ≤ (16129)^6 — too large for u64 but fits in u128.
        //    Intermediate: sp = (S_u·S_v) as u128; sp^3 × 10^12 ≤ ~1.76×10^37 < u128::MAX ✓
        //
        //    NHCS(G) = Σ_{uv∈E} (S_u+S_v)^3  (exact u64)
        //    S-analogue of cubic edge-sum. Extends NHM₁ = Σ (S+S)² (topo23) to 3rd power.
        //    NHCS = |E| × (2S)^3 = 8|E|S^3 for S-regular.
        //    K₃ and K_{1,4}: both S-uniform S=4; (4+4)^3=512; totals differ by |E|.
        //
        //    Overflow: (S_u+S_v)^3 ≤ (32258)^3 ≈ 3.36×10^13 per edge; sum ≤ 2.73×10^17 < u64::MAX ✓

        let mut nrgs_ppm = 0u64;
        let mut nhcs     = 0u64;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb  = sv[b];
                    // NRGS: (S_a·S_b)^{3/2} × 10^6 via isqrt128((S_a·S_b)^3 × 10^12)
                    let sp  = (sa as u128) * (sb as u128);
                    let sp3 = sp * sp * sp;
                    nrgs_ppm += isqrt128(sp3 * 1_000_000_000_000u128) as u64;
                    // NHCS: (S_a+S_b)^3
                    let ss  = sa + sb;
                    nhcs += ss * ss * ss;
                }
            }
        }

        (nvq, nrgs_ppm, nhcs, edge_count, nc)
    }

    pub fn graph_topo_indices31_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NPS (S-Penta vertex sum = Σ_v S(v)^5).
        //
        //    NPS(G) = Σ_v S(v)^5  (exact for test graphs; S-analogue of 5th-power vertex sum)
        //
        //    Extension of NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30) to 5th power.
        //    NPS = n·S^5 for S-regular.
        //
        //    Overflow: S(v) ≤ 127² = 16129; S^5 ≤ 16129^5 ≈ 1.09×10^21 > u64::MAX.
        //    Use u128 accumulator; clamp to u64::MAX for pathological inputs.
        //    Test graphs (S ≤ 9): 4 × 9^5 = 236_196 — no overflow in practice.

        let mut nps_acc: u128 = 0;
        for ci in 0..nc {
            let s  = sv[ci] as u128;
            let s2 = s * s;
            nps_acc = nps_acc.saturating_add(s2 * s2 * s);
        }
        let nps = nps_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NSig (S-Sigma) and NHQS (S-Quartic edge-sum).
        //
        //    NSig(G) = Σ_{uv∈E} (S_u−S_v)²  (exact u64; S-variant of Sigma irregularity index)
        //    S-analogue of σ(G) = Σ_{uv∈E} (d_u−d_v)² (Gutman et al.).
        //    NSig = 0 iff S-regular (all S values equal across edge endpoints).
        //    Overflow: (S_u−S_v)² ≤ 16129² ≈ 2.60×10^8; sum ≤ 4064 × 2.60×10^8 ≈ 1.06×10^12 < u64::MAX ✓
        //
        //    NHQS(G) = Σ_{uv∈E} (S_u+S_v)^4  (S-quartic edge-sum; extends NHM1=Σ(S+S)², NHCS=Σ(S+S)³)
        //    S-analogue of 4th-power edge-sum. NHQS = |E|·(2S)^4 = 16|E|S^4 for S-regular.
        //    K₃ and K_{1,4}: both S-uniform S=4 → same per-edge NHQS (8^4=4096); totals differ by |E|.
        //    Overflow per edge: (2×16129)^4 = 32258^4 ≈ 1.08×10^18 < u64::MAX ✓
        //    Sum: 4064 × 1.08×10^18 ≈ 4.4×10^21 > u64::MAX — use u128 accumulator.

        let mut nsig:     u64  = 0;
        let mut nhqs_acc: u128 = 0;
        for a in 0..nc {
            let sa = sv[a];
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb = sv[b];
                    // NSig: (S_a - S_b)^2 (u64 safe per overflow analysis above)
                    let diff = if sa >= sb { sa - sb } else { sb - sa };
                    nsig += diff * diff;
                    // NHQS: (S_a + S_b)^4 accumulated in u128
                    let ss = (sa + sb) as u128;
                    let ss2 = ss * ss;
                    nhqs_acc = nhqs_acc.saturating_add(ss2 * ss2);
                }
            }
        }
        let nhqs = nhqs_acc.min(u64::MAX as u128) as u64;

        (nsig, nhqs, nps, edge_count, nc)
    }

    pub fn graph_topo_indices32_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NSH (S-Hextic vertex sum = Σ_v S(v)^6).
        //
        //    NSH(G) = Σ_v S(v)^6  (S-analogue of 6th-power vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32)
        //    NSH = n·S^6 for S-regular.
        //
        //    Overflow: S(v) ≤ 127² = 16129; S^6 ≤ 16129^6 ≈ 1.76×10^25 > u64::MAX.
        //    Use u128 accumulator; clamp to u64::MAX for pathological inputs.
        //    Test graphs (S ≤ 9): 4 × 9^6 = 2_125_764 — no overflow in practice.

        let mut nsh_acc: u128 = 0;
        for ci in 0..nc {
            let s  = sv[ci] as u128;
            let s2 = s * s;
            let s3 = s2 * s;
            nsh_acc = nsh_acc.saturating_add(s3 * s3);
        }
        let nsh = nsh_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHPS (S-Quintic edge-sum) and NWSO (S-Weighted Sombor).
        //
        //    NHPS(G) = Σ_{uv∈E} (S_u+S_v)^5  (S-quintic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → NHCS=Σ(S+S)³ (topo30)
        //      → NHQS=Σ(S+S)⁴ (topo31) → NHPS=Σ(S+S)⁵ (topo32)
        //    NHPS = |E|·(2S)^5 = 32|E|S^5 for S-regular.
        //    Overflow per edge: (2×16129)^5 = 32258^5 ≈ 3.49×10^22 > u64::MAX → u128 accumulator.
        //    u128::MAX ≈ 3.4×10^38; sum ≤ 4064 × 3.49×10^22 ≈ 1.42×10^26 < u128::MAX ✓.
        //
        //    NWSO(G) × 10^6 = Σ_{uv∈E} S_u·S_v·√(S_u²+S_v²) × 10^6  (floor ppm)
        //    S-Weighted Sombor: weight each Sombor edge term by S_u·S_v.
        //    NSO = Σ√(S²+S²) (topo21); NWSO = Σ S_u·S_v·√(S_u²+S_v²) (topo32).
        //    NWSO = |E|·S²·S√2·10^6 = |E|·S³√2·10^6 for S-regular.
        //    Implementation: floor(√(S_u²·S_v²·(S_u²+S_v²)·10^12)) via isqrt128.
        //    Overflow: S_u²·S_v²·(S_u²+S_v²) ≤ 16129^4·2·16129^2 = 2·16129^6 ≈ 3.52×10^25.
        //    × 10^12 ≈ 3.52×10^37 < u128::MAX (3.4×10^38) ✓ per-edge.
        //    Sum of ppm values: ≤ 4064 × (16129^3·√2·10^6) ≈ very large → use u128 accumulator.

        // Integer square root u128 — bit-shift init, Babylonian, no float, no_std safe.
        fn isqrt128_t32(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        let mut nhps_acc:  u128 = 0;
        let mut nwso_acc:  u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb = sv[b] as u128;
                    // NHPS: (S_a + S_b)^5 accumulated in u128
                    let ss  = sa + sb;
                    let ss2 = ss * ss;
                    let ss4 = ss2 * ss2;
                    nhps_acc = nhps_acc.saturating_add(ss4 * ss);
                    // NWSO ppm: floor(√(S_a²·S_b²·(S_a²+S_b²)·10^12)) via isqrt128
                    let sa2 = sa * sa;
                    let sb2 = sb * sb;
                    let inner = sa2.saturating_mul(sb2).saturating_mul(sa2.saturating_add(sb2));
                    let inner_scaled = inner.saturating_mul(1_000_000_000_000u128);
                    nwso_acc = nwso_acc.saturating_add(isqrt128_t32(inner_scaled));
                }
            }
        }
        let nhps     = nhps_acc.min(u64::MAX as u128) as u64;
        let nwso_ppm = nwso_acc.min(u64::MAX as u128) as u64;

        (nsh, nhps, nwso_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices33_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NSHP (S-Heptic vertex sum = Σ_v S(v)^7).
        //
        //    NSHP(G) = Σ_v S(v)^7  (S-analogue of 7th-power vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //    NSHP = n·S^7 for S-regular.
        //
        //    Overflow: S(v) ≤ 127² = 16129; S^7 ≤ 16129^7 ≈ 2.84×10^28 > u64::MAX.
        //    Use u128 accumulator; clamp to u64::MAX.
        //    Test graphs (S ≤ 9): 4 × 9^7 = 19_131_876 — no overflow in practice.

        let mut nshp_acc: u128 = 0;
        for ci in 0..nc {
            let s  = sv[ci] as u128;
            let s3 = s * s * s;
            let s7 = s3 * s3 * s;
            nshp_acc = nshp_acc.saturating_add(s7);
        }
        let nshp = nshp_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHSE (S-Sextic edge-sum) and NCSO (S-Cubic Sombor).
        //
        //    NHSE(G) = Σ_{uv∈E} (S_u+S_v)^6  (S-sextic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → NHCS=Σ(S+S)³ (topo30) → NHQS=Σ(S+S)⁴ (topo31)
        //      → NHPS=Σ(S+S)⁵ (topo32) → NHSE=Σ(S+S)⁶ (topo33)
        //    NHSE = |E|·(2S)^6 = 64|E|S^6 for S-regular.
        //    Overflow per edge: (2×16129)^6 ≈ 1.24×10^27 > u64::MAX → u128 accumulator.
        //
        //    NCSO(G) × 10^6 = Σ_{uv∈E} (S_u²+S_v²)^{3/2} × 10^6  (floor ppm)
        //    S-Cubic Sombor: generalised Sombor SO^α with α=3, applied to S-variant.
        //    NSO(topo21) = Σ√(S_u²+S_v²) = SO^1 applied to S.
        //    NCSO = Σ (S_u²+S_v²)·√(S_u²+S_v²) = SO^3 applied to S.
        //    NCSO = |E|·(2S²)^{3/2}·10^6 = |E|·2√2·S³·10^6 for S-regular
        //         = √2 · NWSO/S per edge (for S-regular; = 2×NSO_per_edge² via ppm scaling).
        //    Implementation: per edge = isqrt128((S_u²+S_v²)^3 · 10^12) via isqrt128.
        //    Overflow: (S_u²+S_v²)^3 ≤ (2×16129²)^3 ≈ 1.41×10^26.
        //      × 10^12 ≈ 1.41×10^38 < u128::MAX (3.4×10^38) ✓ per-edge.
        //    Sum of ppm values: ≤ 4064 × (2√2·S³·10^6) for S=16129 ≈ large → u128 accumulator.

        fn isqrt128_t33(n: u128) -> u128 {
            if n == 0 { return 0; }
            let bits = 128u32 - n.leading_zeros();
            let mut x: u128 = 1u128 << ((bits + 1) / 2);
            loop {
                let y = (x + n / x) / 2;
                if y >= x { return x; }
                x = y;
            }
        }

        let mut nhse_acc:  u128 = 0;
        let mut ncso_acc:  u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb = sv[b] as u128;
                    // NHSE: (S_a + S_b)^6 accumulated in u128
                    let ss  = sa + sb;
                    let ss2 = ss * ss;
                    let ss3 = ss2 * ss;
                    nhse_acc = nhse_acc.saturating_add(ss3 * ss3);
                    // NCSO ppm: floor(√((S_a²+S_b²)^3 · 10^12)) via isqrt128
                    let sa2  = sa * sa;
                    let sb2  = sb * sb;
                    let s2s  = sa2.saturating_add(sb2);
                    let s2s3 = s2s.saturating_mul(s2s).saturating_mul(s2s);
                    let inner_scaled = s2s3.saturating_mul(1_000_000_000_000u128);
                    ncso_acc = ncso_acc.saturating_add(isqrt128_t33(inner_scaled));
                }
            }
        }
        let nhse     = nhse_acc.min(u64::MAX as u128) as u64;
        let ncso_ppm = ncso_acc.min(u64::MAX as u128) as u64;

        (nshp, nhse, ncso_ppm, edge_count, nc)
    }

    pub fn graph_topo_indices34_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NOC (S-Octic vertex sum = Σ_v S(v)^8).
        //
        //    NOC(G) = Σ_v S(v)^8  (exact u128→u64; S-octic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33) → NOC=Σ S⁸ (topo34)
        //    NOC = n·S^8 for S-regular.
        //    Overflow: S(v) ≤ 127² = 16129; S^8 ≤ 16129^8 ≈ 5.6×10^32 > u64::MAX.
        //    Use u128 accumulator (n × S^8 ≤ 127 × 5.6×10^32 ≈ 7.2×10^34 < u128::MAX); clamp.

        let mut noc_acc: u128 = 0;
        for ci in 0..nc {
            let s  = sv[ci] as u128;
            let s4 = s * s * s * s;
            let s8 = s4 * s4;
            noc_acc = noc_acc.saturating_add(s8);
        }
        let noc = noc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHHS (S-Septic edge-sum) and NFSO (S-Fourth Sombor).
        //
        //    NHHS(G) = Σ_{uv∈E} (S_u+S_v)^7  (exact u128→u64; S-septic/hepta edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → NHCS=Σ(S+S)³ (topo30) → NHQS=Σ(S+S)⁴ (topo31)
        //      → NHPS=Σ(S+S)⁵ (topo32) → NHSE=Σ(S+S)⁶ (topo33) → NHHS=Σ(S+S)⁷ (topo34)
        //    NHHS = |E|·(2S)^7 = 128|E|S^7 for S-regular.
        //    Overflow per edge: (2×16129)^7 ≈ 1.51×10^30 > u64::MAX → u128 accumulator.
        //    Total: m × max ≤ 8001 × 1.51×10^30 ≈ 1.2×10^34 < u128::MAX ✓.
        //
        //    NFSO(G) = Σ_{uv∈E} (S_u²+S_v²)²  (exact u128→u64; S-Fourth Sombor)
        //    S-variant generalised Sombor SO^α with α=4: (S²+S²)^{α/2} = (S²+S²)^2.
        //    NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
        //    NFSO(topo34)=Σ(S²+S²)^2 (α=4) — exact integer, no isqrt.
        //    NFSO = |E|·(2S²)² = 4|E|S⁴ for S-regular.
        //    Overflow per edge: (2×16129²)^2 ≈ 2.7×10^17 > u64::MAX → u128 accumulator.
        //    Total: m × 2.7×10^17 ≤ 8001 × 2.7×10^17 ≈ 2.2×10^21 < u128::MAX ✓.

        let mut nhhs_acc: u128 = 0;
        let mut nfso_acc: u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb  = sv[b] as u128;
                    // NHHS: (S_a + S_b)^7 in u128; ss^7 = ss^4 · ss^2 · ss
                    let ss  = sa + sb;
                    let ss2 = ss * ss;
                    let ss4 = ss2 * ss2;
                    nhhs_acc = nhhs_acc.saturating_add(ss4.saturating_mul(ss2.saturating_mul(ss)));
                    // NFSO: (S_a² + S_b²)² in u128
                    let s2s = sa * sa + sb * sb;
                    nfso_acc = nfso_acc.saturating_add(s2s * s2s);
                }
            }
        }
        let nhhs = nhhs_acc.min(u64::MAX as u128) as u64;
        let nfso = nfso_acc.min(u64::MAX as u128) as u64;

        (noc, nhhs, nfso, edge_count, nc)
    }

    pub fn graph_topo_indices35_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NNC (S-Nonic vertex sum = Σ_v S(v)^9).
        //
        //    NNC(G) = Σ_v S(v)^9  (exact u128→u64; S-nonic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //      → NOC=Σ S⁸ (topo34) → NNC=Σ S⁹ (topo35)
        //    NNC = n·S^9 for S-regular.
        //    Overflow: S(v) ≤ 127² = 16129; S^9 ≤ 16129^9 ≈ 9×10^36 > u64::MAX.
        //    Use u128 accumulator; clamp to u64::MAX.

        let mut nnc_acc: u128 = 0;
        for ci in 0..nc {
            let s  = sv[ci] as u128;
            let s4 = s * s * s * s;
            let s8 = s4 * s4;
            let s9 = s8.saturating_mul(s);
            nnc_acc = nnc_acc.saturating_add(s9);
        }
        let nnc = nnc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHOC (S-Octic edge-sum) and NHSO (S-Hextic Sombor).
        //
        //    NHOC(G) = Σ_{uv∈E} (S_u+S_v)^8  (exact u128→u64; S-octic/eighth edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → NHCS=Σ(S+S)³ (topo30) → NHQS=Σ(S+S)⁴ (topo31)
        //      → NHPS=Σ(S+S)⁵ (topo32) → NHSE=Σ(S+S)⁶ (topo33) → NHHS=Σ(S+S)⁷ (topo34)
        //      → NHOC=Σ(S+S)⁸ (topo35)
        //    NHOC = |E|·(2S)^8 = 256|E|S^8 for S-regular.
        //    Overflow per edge: (2×16129)^8 ≈ 1.37×10^35 → u128 accumulator.
        //
        //    NHSO(G) = Σ_{uv∈E} (S_u²+S_v²)^3  (exact u128→u64; S-Hextic Sombor)
        //    S-variant generalised Sombor SO^α with α=6: SO^α=(S²+S²)^{α/2}.
        //    NSO(topo21)=Σ(S²+S²)^{1/2} (α=1), NCSO(topo33)=Σ(S²+S²)^{3/2} (α=3),
        //    NFSO(topo34)=Σ(S²+S²)^2 (α=4), NHSO(topo35)=Σ(S²+S²)^3 (α=6) — exact, no isqrt.
        //    NHSO = |E|·(2S²)^3 = 8|E|S^6 for S-regular.
        //    Overflow per edge: (2×16129²)^3 ≈ 1.4×10^26 → u128 accumulator.

        let mut nhoc_acc: u128 = 0;
        let mut nhso_acc: u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb  = sv[b] as u128;
                    // NHOC: (S_a + S_b)^8 = ((S_a+S_b)^4)^2
                    let ss  = sa + sb;
                    let ss2 = ss * ss;
                    let ss4 = ss2 * ss2;
                    let ss8 = ss4.saturating_mul(ss4);
                    nhoc_acc = nhoc_acc.saturating_add(ss8);
                    // NHSO: (S_a² + S_b²)^3
                    let s2s = sa * sa + sb * sb;
                    let s2s3 = s2s.saturating_mul(s2s.saturating_mul(s2s));
                    nhso_acc = nhso_acc.saturating_add(s2s3);
                }
            }
        }
        let nhoc = nhoc_acc.min(u64::MAX as u128) as u64;
        let nhso = nhso_acc.min(u64::MAX as u128) as u64;

        (nnc, nhoc, nhso, edge_count, nc)
    }

    pub fn graph_topo_indices36_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NDC (S-Decic vertex sum = Σ_v S(v)^10).
        //
        //    NDC(G) = Σ_v S(v)^10  (exact u128→u64; S-decic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //      → NOC=Σ S⁸ (topo34) → NNC=Σ S⁹ (topo35) → NDC=Σ S¹⁰ (topo36)
        //    NDC = n·S^10 for S-regular.
        //    Overflow: S(v) ≤ 16129; S^10 ≤ 16129^10 ≈ 2.6×10^41 > u128::MAX.
        //    Use saturating u128 accumulator; clamp to u64::MAX.

        let mut ndc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s10 = s8.saturating_mul(s2);
            ndc_acc = ndc_acc.saturating_add(s10);
        }
        let ndc = ndc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHNC (S-Nonic edge-sum) and NOSO (S-Octic Sombor).
        //
        //    NHNC(G) = Σ_{uv∈E} (S_u+S_v)^9  (exact u128→u64; S-nonic/ninth edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → NHCS=Σ(S+S)³ (topo30) → NHQS=Σ(S+S)⁴ (topo31)
        //      → NHPS=Σ(S+S)⁵ (topo32) → NHSE=Σ(S+S)⁶ (topo33) → NHHS=Σ(S+S)⁷ (topo34)
        //      → NHOC=Σ(S+S)⁸ (topo35) → NHNC=Σ(S+S)⁹ (topo36)
        //    NHNC = |E|·(2S)^9 = 512|E|S^9 for S-regular.
        //    Overflow per edge: (2×16129)^9 ≈ 3.5×10^40 > u128::MAX → saturating.
        //
        //    NOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^4  (exact u128→u64; S-Octic Sombor)
        //    S-variant generalised Sombor SO^α with α=8: SO^α=(S²+S²)^{α/2}.
        //    NSO(topo21)=α=1, NCSO(topo33)=α=3, NFSO(topo34)=α=4,
        //    NHSO(topo35)=α=6, NOSO(topo36)=Σ(S²+S²)^4 (α=8) — exact, no isqrt.
        //    NOSO = |E|·(2S²)^4 = 16|E|S^8 for S-regular.
        //    Overflow per edge: (2×16129²)^4 ≈ 7.3×10^34 < u128::MAX. Sum ≤ 128 terms → safe.

        let mut nhnc_acc: u128 = 0;
        let mut noso_acc: u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHNC: (S_a + S_b)^9 = ((S_a+S_b)^8) × (S_a+S_b)
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss9  = ss8.saturating_mul(ss);
                    nhnc_acc = nhnc_acc.saturating_add(ss9);
                    // NOSO: (S_a² + S_b²)^4 = ((S_a²+S_b²)^2)^2
                    let s2s  = sa * sa + sb * sb;
                    let s2s2 = s2s * s2s;
                    let s2s4 = s2s2.saturating_mul(s2s2);
                    noso_acc = noso_acc.saturating_add(s2s4);
                }
            }
        }
        let nhnc = nhnc_acc.min(u64::MAX as u128) as u64;
        let noso = noso_acc.min(u64::MAX as u128) as u64;

        (ndc, nhnc, noso, edge_count, nc)
    }

    pub fn graph_topo_indices37_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NUC (S-Undecic vertex sum = Σ_v S(v)^11).
        //
        //    NUC(G) = Σ_v S(v)^11  (exact u128→u64; S-undecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //      → NOC=Σ S⁸ (topo34) → NNC=Σ S⁹ (topo35) → NDC=Σ S¹⁰ (topo36)
        //      → NUC=Σ S¹¹ (topo37)
        //    NUC = n·S^11 for S-regular.
        //    Overflow: S^11 ≤ 16129^11 ≈ 4.2×10^45 > u128::MAX → saturating.

        let mut nuc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s11 = s8.saturating_mul(s2).saturating_mul(s);
            nuc_acc = nuc_acc.saturating_add(s11);
        }
        let nuc = nuc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHDC (S-Decic edge-sum) and NTSO (S-Tenth Sombor).
        //
        //    NHDC(G) = Σ_{uv∈E} (S_u+S_v)^10  (exact u128→u64; S-decic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHOC=Σ(S+S)⁸ (topo35)
        //      → NHNC=Σ(S+S)⁹ (topo36) → NHDC=Σ(S+S)¹⁰ (topo37)
        //    NHDC = |E|·(2S)^10 = 1024|E|S^10 for S-regular.
        //    Overflow per edge: (2×16129)^10 ≈ 5.6×10^44 > u128::MAX → saturating.
        //
        //    NTSO(G) = Σ_{uv∈E} (S_u²+S_v²)^5  (exact u128→u64; S-Tenth Sombor α=10)
        //    S-variant generalised Sombor SO^α with α=10: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37).
        //    NTSO = |E|·(2S²)^5 = 32|E|S^10 for S-regular.
        //    Overflow per edge: (2×16129²)^5 ≈ 3.8×10^43 > u128::MAX → saturating.

        let mut nhdc_acc: u128 = 0;
        let mut ntso_acc: u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHDC: (S_a + S_b)^10 = ((S_a+S_b)^8) × (S_a+S_b)^2
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss10 = ss8.saturating_mul(ss2);
                    nhdc_acc = nhdc_acc.saturating_add(ss10);
                    // NTSO: (S_a² + S_b²)^5 = ((S_a²+S_b²)^4) × (S_a²+S_b²)
                    let s2s  = sa * sa + sb * sb;
                    let s2s2 = s2s * s2s;
                    let s2s4 = s2s2.saturating_mul(s2s2);
                    let s2s5 = s2s4.saturating_mul(s2s);
                    ntso_acc = ntso_acc.saturating_add(s2s5);
                }
            }
        }
        let nhdc = nhdc_acc.min(u64::MAX as u128) as u64;
        let ntso = ntso_acc.min(u64::MAX as u128) as u64;

        (nuc, nhdc, ntso, edge_count, nc)
    }

    pub fn graph_topo_indices38_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NDoC (S-Dodecic vertex sum = Σ_v S(v)^12).
        //
        //    NDoC(G) = Σ_v S(v)^12  (exact u128→u64; S-dodecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //      → NOC=Σ S⁸ (topo34) → NNC=Σ S⁹ (topo35) → NDC=Σ S¹⁰ (topo36)
        //      → NUC=Σ S¹¹ (topo37) → NDoC=Σ S¹² (topo38)
        //    NDoC = n·S^12 for S-regular.
        //    Overflow: S^12 ≤ 16129^12 ≈ 3.8×10^49 > u128::MAX → saturating.

        let mut ndoc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s12 = s8.saturating_mul(s4);
            ndoc_acc = ndoc_acc.saturating_add(s12);
        }
        let ndoc = ndoc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHUC (S-Undecic edge-sum) and NDSO (S-Duodecic Sombor).
        //
        //    NHUC(G) = Σ_{uv∈E} (S_u+S_v)^11  (exact u128→u64; S-undecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHDC=Σ(S+S)¹⁰ (topo37)
        //      → NHUC=Σ(S+S)¹¹ (topo38)
        //    NHUC = |E|·(2S)^11 = 2048|E|S^11 for S-regular.
        //    Overflow per edge: (2×16129)^11 ≈ 9.0×10^48 > u128::MAX → saturating.
        //
        //    NDSO(G) = Σ_{uv∈E} (S_u²+S_v²)^6  (exact u128→u64; S-Duodecic Sombor α=12)
        //    S-variant generalised Sombor SO^α with α=12: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37), NDSO(α=12,topo38).
        //    NDSO = |E|·(2S²)^6 = 64|E|S^12 for S-regular.
        //    Overflow per edge: (2×16129²)^6 ≈ 6.1×10^52 > u128::MAX → saturating.

        let mut nhuc_acc: u128 = 0;
        let mut ndso_acc: u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHUC: (S_a + S_b)^11 = ((S_a+S_b)^8) × (S_a+S_b)^2 × (S_a+S_b)
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss11 = ss8.saturating_mul(ss2).saturating_mul(ss);
                    nhuc_acc = nhuc_acc.saturating_add(ss11);
                    // NDSO: (S_a² + S_b²)^6 = ((S_a²+S_b²)^4) × (S_a²+S_b²)^2
                    let s2s  = sa * sa + sb * sb;
                    let s2s2 = s2s * s2s;
                    let s2s4 = s2s2.saturating_mul(s2s2);
                    let s2s6 = s2s4.saturating_mul(s2s2);
                    ndso_acc = ndso_acc.saturating_add(s2s6);
                }
            }
        }
        let nhuc = nhuc_acc.min(u64::MAX as u128) as u64;
        let ndso = ndso_acc.min(u64::MAX as u128) as u64;

        (ndoc, nhuc, ndso, edge_count, nc)
    }

    pub fn graph_topo_indices39_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NTC (S-Tridecic vertex sum = Σ_v S(v)^13).
        //
        //    NTC(G) = Σ_v S(v)^13  (exact u128→u64; S-tridecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //      → NOC=Σ S⁸ (topo34) → NNC=Σ S⁹ (topo35) → NDC=Σ S¹⁰ (topo36)
        //      → NUC=Σ S¹¹ (topo37) → NDoC=Σ S¹² (topo38) → NTC=Σ S¹³ (topo39)
        //    NTC = n·S^13 for S-regular.
        //    Overflow: S^13 ≤ 16129^13 ≈ 6.1×10^53 > u128::MAX → saturating.

        let mut ntc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s13 = s8.saturating_mul(s4).saturating_mul(s);
            ntc_acc = ntc_acc.saturating_add(s13);
        }
        let ntc = ntc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHDOC (S-Dodecic edge-sum) and NESO (S-Tetradecic Sombor).
        //
        //    NHDOC(G) = Σ_{uv∈E} (S_u+S_v)^12  (exact u128→u64; S-dodecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHUC=Σ(S+S)¹¹ (topo38)
        //      → NHDOC=Σ(S+S)¹² (topo39)
        //    NHDOC = |E|·(2S)^12 = 4096|E|S^12 for S-regular.
        //    Overflow per edge: (2×16129)^12 ≈ 2.9×10^52 > u128::MAX → saturating.
        //
        //    NESO(G) = Σ_{uv∈E} (S_u²+S_v²)^7  (exact u128→u64; S-Tetradecic Sombor α=14)
        //    S-variant generalised Sombor SO^α with α=14: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39).
        //    NESO = |E|·(2S²)^7 = 128|E|S^14 for S-regular.
        //    Overflow per edge: (2×16129²)^7 ≈ 2.7×10^61 > u128::MAX → saturating.

        let mut nhdoc_acc: u128 = 0;
        let mut neso_acc:  u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHDOC: (S_a + S_b)^12 = ss^8 × ss^4
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss12 = ss8.saturating_mul(ss4);
                    nhdoc_acc = nhdoc_acc.saturating_add(ss12);
                    // NESO: (S_a² + S_b²)^7 = s2s^4 × s2s^2 × s2s
                    let s2s  = sa * sa + sb * sb;
                    let s2s2 = s2s * s2s;
                    let s2s4 = s2s2.saturating_mul(s2s2);
                    let s2s7 = s2s4.saturating_mul(s2s2).saturating_mul(s2s);
                    neso_acc = neso_acc.saturating_add(s2s7);
                }
            }
        }
        let nhdoc = nhdoc_acc.min(u64::MAX as u128) as u64;
        let neso  = neso_acc.min(u64::MAX as u128) as u64;

        (ntc, nhdoc, neso, edge_count, nc)
    }

    pub fn graph_topo_indices40_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NQTC (S-Tetradecic vertex sum = Σ_v S(v)^14).
        //
        //    NQTC(G) = Σ_v S(v)^14  (exact u128→u64; S-Tetradecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //      → NOC=Σ S⁸ (topo34) → NNC=Σ S⁹ (topo35) → NDC=Σ S¹⁰ (topo36)
        //      → NUC=Σ S¹¹ (topo37) → NDoC=Σ S¹² (topo38) → NTC=Σ S¹³ (topo39)
        //      → NQTC=Σ S¹⁴ (topo40)
        //    NQTC = n·S^14 for S-regular.
        //    Overflow: S^14 ≤ 16129^14 → saturating u128 accumulator, clamp to u64::MAX.

        let mut nqtc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s14 = s8.saturating_mul(s4).saturating_mul(s2);
            nqtc_acc = nqtc_acc.saturating_add(s14);
        }
        let nqtc = nqtc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHTC (S-Tridecic edge-sum) and NGSO (S-Hexadecic Sombor).
        //
        //    NHTC(G) = Σ_{uv∈E} (S_u+S_v)^13  (exact u128→u64; S-Tridecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHDOC=Σ(S+S)¹² (topo39)
        //      → NHTC=Σ(S+S)¹³ (topo40)
        //    NHTC = |E|·(2S)^13 = 8192|E|S^13 for S-regular.
        //    Overflow per edge: (2×16129)^13 → saturating u128 accumulator.
        //
        //    NGSO(G) = Σ_{uv∈E} (S_u²+S_v²)^8  (exact u128→u64; S-Hexadecic Sombor α=16)
        //    S-variant generalised Sombor SO^α with α=16: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40).
        //    NGSO = |E|·(2S²)^8 = 256|E|S^16 for S-regular.
        //    Overflow per edge: (2×16129²)^8 → saturating u128 accumulator.

        let mut nhtc_acc: u128 = 0;
        let mut ngso_acc: u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHTC: (S_a + S_b)^13 = ss^8 × ss^4 × ss
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss13 = ss8.saturating_mul(ss4).saturating_mul(ss);
                    nhtc_acc = nhtc_acc.saturating_add(ss13);
                    // NGSO: (S_a² + S_b²)^8 = s2s^4 × s2s^4
                    let s2s  = sa * sa + sb * sb;
                    let s2s2 = s2s * s2s;
                    let s2s4 = s2s2.saturating_mul(s2s2);
                    let s2s8 = s2s4.saturating_mul(s2s4);
                    ngso_acc = ngso_acc.saturating_add(s2s8);
                }
            }
        }
        let nhtc = nhtc_acc.min(u64::MAX as u128) as u64;
        let ngso = ngso_acc.min(u64::MAX as u128) as u64;

        (nqtc, nhtc, ngso, edge_count, nc)
    }

    pub fn graph_topo_indices41_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NPTC (S-Pentadecic vertex sum = Σ_v S(v)^15).
        //
        //    NPTC(G) = Σ_v S(v)^15  (exact u128→u64; S-Pentadecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → NF=Σ S³ (topo22) → NVQ=Σ S⁴ (topo30)
        //      → NPS=Σ S⁵ (topo31) → NSH=Σ S⁶ (topo32) → NSHP=Σ S⁷ (topo33)
        //      → NOC=Σ S⁸ (topo34) → NNC=Σ S⁹ (topo35) → NDC=Σ S¹⁰ (topo36)
        //      → NUC=Σ S¹¹ (topo37) → NDoC=Σ S¹² (topo38) → NTC=Σ S¹³ (topo39)
        //      → NQTC=Σ S¹⁴ (topo40) → NPTC=Σ S¹⁵ (topo41)
        //    NPTC = n·S^15 for S-regular.
        //    Overflow: S^15 ≤ 16129^15 → saturating u128 accumulator, clamp to u64::MAX.

        let mut nptc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s15 = s8.saturating_mul(s4).saturating_mul(s2).saturating_mul(s);
            nptc_acc = nptc_acc.saturating_add(s15);
        }
        let nptc = nptc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHQTC (S-Tetradecic edge-sum) and NIOSO (S-Octadecic Sombor).
        //
        //    NHQTC(G) = Σ_{uv∈E} (S_u+S_v)^14  (exact u128→u64; S-Tetradecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHTC=Σ(S+S)¹³ (topo40)
        //      → NHQTC=Σ(S+S)¹⁴ (topo41)
        //    NHQTC = |E|·(2S)^14 = 16384|E|S^14 for S-regular.
        //    Overflow per edge: (2×16129)^14 → saturating u128 accumulator.
        //
        //    NIOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^9  (exact u128→u64; S-Octadecic Sombor α=18)
        //    S-variant generalised Sombor SO^α with α=18: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40), NIOSO(α=18,topo41).
        //    NIOSO = |E|·(2S²)^9 = 512|E|S^18 for S-regular.
        //    Overflow per edge: (2×16129²)^9 → saturating u128 accumulator.

        let mut nhqtc_acc: u128 = 0;
        let mut nioso_acc: u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHQTC: (S_a + S_b)^14 = ss^8 × ss^4 × ss^2
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss14 = ss8.saturating_mul(ss4).saturating_mul(ss2);
                    nhqtc_acc = nhqtc_acc.saturating_add(ss14);
                    // NIOSO: (S_a² + S_b²)^9 = s2s^8 × s2s
                    let s2s  = sa * sa + sb * sb;
                    let s2s2 = s2s * s2s;
                    let s2s4 = s2s2.saturating_mul(s2s2);
                    let s2s8 = s2s4.saturating_mul(s2s4);
                    let s2s9 = s2s8.saturating_mul(s2s);
                    nioso_acc = nioso_acc.saturating_add(s2s9);
                }
            }
        }
        let nhqtc = nhqtc_acc.min(u64::MAX as u128) as u64;
        let nioso = nioso_acc.min(u64::MAX as u128) as u64;

        (nptc, nhqtc, nioso, edge_count, nc)
    }

    pub fn graph_topo_indices42_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NSTC (S-Hexadecic vertex sum = Σ_v S(v)^16).
        //
        //    NSTC(G) = Σ_v S(v)^16  (exact u128→u64; S-Hexadecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NPTC=Σ S¹⁵ (topo41) → NSTC=Σ S¹⁶ (topo42)
        //    NSTC = n·S^16 for S-regular.
        //    Overflow: S^16 ≤ 16129^16 → saturating u128 accumulator, clamp to u64::MAX.

        let mut nstc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            nstc_acc = nstc_acc.saturating_add(s16);
        }
        let nstc = nstc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHPTC (S-Pentadecic edge-sum) and NJSO (S-Eicosic Sombor).
        //
        //    NHPTC(G) = Σ_{uv∈E} (S_u+S_v)^15  (exact u128→u64; S-Pentadecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHQTC=Σ(S+S)¹⁴ (topo41)
        //      → NHPTC=Σ(S+S)¹⁵ (topo42)
        //    NHPTC = |E|·(2S)^15 = 32768|E|·S^15 for S-regular.
        //    Overflow per edge: (2×16129)^15 → saturating u128 accumulator.
        //
        //    NJSO(G) = Σ_{uv∈E} (S_u²+S_v²)^10  (exact u128→u64; S-Eicosic Sombor α=20)
        //    S-variant generalised Sombor SO^α with α=20: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42).
        //    NJSO = |E|·(2S²)^10 = 1024|E|·S^20 for S-regular.
        //    Overflow per edge: (2×16129²)^10 → saturating u128 accumulator.

        let mut nhptc_acc: u128 = 0;
        let mut njso_acc:  u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHPTC: (S_a + S_b)^15 = ss^8 × ss^4 × ss^2 × ss
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss15 = ss8.saturating_mul(ss4).saturating_mul(ss2).saturating_mul(ss);
                    nhptc_acc = nhptc_acc.saturating_add(ss15);
                    // NJSO: (S_a² + S_b²)^10 = s2s^8 × s2s^2
                    let s2s   = sa * sa + sb * sb;
                    let s2s2  = s2s * s2s;
                    let s2s4  = s2s2.saturating_mul(s2s2);
                    let s2s8  = s2s4.saturating_mul(s2s4);
                    let s2s10 = s2s8.saturating_mul(s2s2);
                    njso_acc = njso_acc.saturating_add(s2s10);
                }
            }
        }
        let nhptc = nhptc_acc.min(u64::MAX as u128) as u64;
        let njso  = njso_acc.min(u64::MAX as u128) as u64;

        (nstc, nhptc, njso, edge_count, nc)
    }

    pub fn graph_topo_indices43_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NHEPTC (S-Heptadecic vertex sum = Σ_v S(v)^17).
        //
        //    NHEPTC(G) = Σ_v S(v)^17  (exact u128→u64; S-Heptadecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NSTC=Σ S¹⁶ (topo42) → NHEPTC=Σ S¹⁷ (topo43)
        //    NHEPTC = n·S^17 for S-regular.
        //    Overflow: S^17 ≤ 16129^17 → saturating u128 accumulator, clamp to u64::MAX.

        let mut nheptc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s17 = s16.saturating_mul(s);
            nheptc_acc = nheptc_acc.saturating_add(s17);
        }
        let nheptc = nheptc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHSTC (S-Hexadecic edge-sum) and NKSO (S-Docosic Sombor).
        //
        //    NHSTC(G) = Σ_{uv∈E} (S_u+S_v)^16  (exact u128→u64; S-Hexadecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHPTC=Σ(S+S)¹⁵ (topo42)
        //      → NHSTC=Σ(S+S)¹⁶ (topo43)
        //    NHSTC = |E|·(2S)^16 = 65536|E|·S^16 for S-regular.
        //    Overflow per edge: (2×16129)^16 → saturating u128 accumulator.
        //
        //    NKSO(G) = Σ_{uv∈E} (S_u²+S_v²)^11  (exact u128→u64; S-Docosic Sombor α=22)
        //    S-variant generalised Sombor SO^α with α=22: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43) — exact, no isqrt.
        //    NKSO = |E|·(2S²)^11 = 2048|E|·S^22 for S-regular.
        //    Overflow per edge: (2×16129²)^11 → saturating u128 accumulator.

        let mut nhstc_acc: u128 = 0;
        let mut nkso_acc:  u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHSTC: (S_a + S_b)^16 = ss^8 × ss^8
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss16 = ss8.saturating_mul(ss8);
                    nhstc_acc = nhstc_acc.saturating_add(ss16);
                    // NKSO: (S_a² + S_b²)^11 = s2s^8 × s2s^2 × s2s
                    let s2s   = sa * sa + sb * sb;
                    let s2s2  = s2s * s2s;
                    let s2s4  = s2s2.saturating_mul(s2s2);
                    let s2s8  = s2s4.saturating_mul(s2s4);
                    let s2s11 = s2s8.saturating_mul(s2s2).saturating_mul(s2s);
                    nkso_acc = nkso_acc.saturating_add(s2s11);
                }
            }
        }
        let nhstc = nhstc_acc.min(u64::MAX as u128) as u64;
        let nkso  = nkso_acc.min(u64::MAX as u128) as u64;

        (nheptc, nhstc, nkso, edge_count, nc)
    }

    pub fn graph_topo_indices44_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NOCTC (S-Octadecic vertex sum = Σ_v S(v)^18).
        //
        //    NOCTC(G) = Σ_v S(v)^18  (exact u128→u64; S-Octadecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NHEPTC=Σ S¹⁷ (topo43) → NOCTC=Σ S¹⁸ (topo44)
        //    NOCTC = n·S^18 for S-regular.
        //    Overflow: S^18 ≤ 16129^18 → saturating u128 accumulator, clamp to u64::MAX.

        let mut noctc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s18 = s16.saturating_mul(s2);
            noctc_acc = noctc_acc.saturating_add(s18);
        }
        let noctc = noctc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHOCTC (S-Heptadecic edge-sum) and NLSO (S-Tetracosic Sombor).
        //
        //    NHOCTC(G) = Σ_{uv∈E} (S_u+S_v)^17  (exact u128→u64; S-Heptadecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHSTC=Σ(S+S)¹⁶ (topo43)
        //      → NHOCTC=Σ(S+S)¹⁷ (topo44)
        //    NHOCTC = |E|·(2S)^17 = 131072|E|·S^17 for S-regular.
        //    Overflow per edge: (2×16129)^17 → saturating u128 accumulator.
        //
        //    NLSO(G) = Σ_{uv∈E} (S_u²+S_v²)^12  (exact u128→u64; S-Tetracosic Sombor α=24)
        //    S-variant generalised Sombor SO^α with α=24: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44) — exact, no isqrt.
        //    NLSO = |E|·(2S²)^12 = 4096|E|·S^24 for S-regular.
        //    Overflow per edge: (2×16129²)^12 → saturating u128 accumulator.

        let mut nhoctc_acc: u128 = 0;
        let mut nlso_acc:   u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb   = sv[b] as u128;
                    // NHOCTC: (S_a + S_b)^17 = ss^16 × ss
                    let ss   = sa + sb;
                    let ss2  = ss * ss;
                    let ss4  = ss2 * ss2;
                    let ss8  = ss4.saturating_mul(ss4);
                    let ss16 = ss8.saturating_mul(ss8);
                    let ss17 = ss16.saturating_mul(ss);
                    nhoctc_acc = nhoctc_acc.saturating_add(ss17);
                    // NLSO: (S_a² + S_b²)^12 = s2s^8 × s2s^4
                    let s2s   = sa * sa + sb * sb;
                    let s2s2  = s2s * s2s;
                    let s2s4  = s2s2.saturating_mul(s2s2);
                    let s2s8  = s2s4.saturating_mul(s2s4);
                    let s2s12 = s2s8.saturating_mul(s2s4);
                    nlso_acc = nlso_acc.saturating_add(s2s12);
                }
            }
        }
        let nhoctc = nhoctc_acc.min(u64::MAX as u128) as u64;
        let nlso   = nlso_acc.min(u64::MAX as u128) as u64;

        (noctc, nhoctc, nlso, edge_count, nc)
    }

    pub fn graph_topo_indices45_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NNONTC (S-Nonadecic vertex sum = Σ_v S(v)^19).
        //
        //    NNONTC(G) = Σ_v S(v)^19  (exact u128→u64; S-Nonadecic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NOCTC=Σ S¹⁸ (topo44) → NNONTC=Σ S¹⁹ (topo45)
        //    NNONTC = n·S^19 for S-regular.
        //    Overflow: S^19 ≤ 16129^19 → saturating u128 accumulator, clamp to u64::MAX.

        let mut nnontc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s3  = s2 * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s19 = s16.saturating_mul(s3);
            nnontc_acc = nnontc_acc.saturating_add(s19);
        }
        let nnontc = nnontc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHNONTC (S-Octadecic edge-sum) and NMSO (S-Hexacosic Sombor).
        //
        //    NHNONTC(G) = Σ_{uv∈E} (S_u+S_v)^18  (exact u128→u64; S-Octadecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHOCTC=Σ(S+S)¹⁷ (topo44)
        //      → NHNONTC=Σ(S+S)¹⁸ (topo45)
        //    NHNONTC = |E|·(2S)^18 = 262144|E|·S^18 for S-regular.
        //    Overflow per edge: (2×16129)^18 → saturating u128 accumulator.
        //
        //    NMSO(G) = Σ_{uv∈E} (S_u²+S_v²)^13  (exact u128→u64; S-Hexacosic Sombor α=26)
        //    S-variant generalised Sombor SO^α with α=26: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45) — exact, no isqrt.
        //    NMSO = |E|·(2S²)^13 = 8192|E|·S^26 for S-regular.
        //    Overflow per edge: (2×16129²)^13 → saturating u128 accumulator.

        let mut nhnontc_acc: u128 = 0;
        let mut nmso_acc:    u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb    = sv[b] as u128;
                    // NHNONTC: (S_a + S_b)^18 = ss^16 × ss^2
                    let ss    = sa + sb;
                    let ss2   = ss * ss;
                    let ss4   = ss2 * ss2;
                    let ss8   = ss4.saturating_mul(ss4);
                    let ss16  = ss8.saturating_mul(ss8);
                    let ss18  = ss16.saturating_mul(ss2);
                    nhnontc_acc = nhnontc_acc.saturating_add(ss18);
                    // NMSO: (S_a² + S_b²)^13 = s2s^8 × s2s^4 × s2s
                    let s2s   = sa * sa + sb * sb;
                    let s2s2  = s2s * s2s;
                    let s2s4  = s2s2.saturating_mul(s2s2);
                    let s2s8  = s2s4.saturating_mul(s2s4);
                    let s2s13 = s2s8.saturating_mul(s2s4).saturating_mul(s2s);
                    nmso_acc = nmso_acc.saturating_add(s2s13);
                }
            }
        }
        let nhnontc = nhnontc_acc.min(u64::MAX as u128) as u64;
        let nmso    = nmso_acc.min(u64::MAX as u128) as u64;

        (nnontc, nhnontc, nmso, edge_count, nc)
    }

    pub fn graph_topo_indices46_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NEICTC (S-Eicosic vertex sum = Σ_v S(v)^20).
        //
        //    NEICTC(G) = Σ_v S(v)^20  (exact u128→u64; S-Eicosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NNONTC=Σ S¹⁹ (topo45) → NEICTC=Σ S²⁰ (topo46)
        //    NEICTC = n·S^20 for S-regular.
        //    Overflow: S^20 ≤ 16129^20 → saturating u128 accumulator, clamp to u64::MAX.

        let mut neictc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s20 = s16.saturating_mul(s4);
            neictc_acc = neictc_acc.saturating_add(s20);
        }
        let neictc = neictc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHEICTC (S-Nonadecic edge-sum) and NNSO (S-Octacosic Sombor).
        //
        //    NHEICTC(G) = Σ_{uv∈E} (S_u+S_v)^19  (exact u128→u64; S-Nonadecic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHNONTC=Σ(S+S)¹⁸ (topo45)
        //      → NHEICTC=Σ(S+S)¹⁹ (topo46)
        //    NHEICTC = |E|·(2S)^19 = 524288|E|·S^19 for S-regular.
        //    Overflow per edge: (2×16129)^19 → saturating u128 accumulator.
        //
        //    NNSO(G) = Σ_{uv∈E} (S_u²+S_v²)^14  (exact u128→u64; S-Octacosic Sombor α=28)
        //    S-variant generalised Sombor SO^α with α=28: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46) — exact, no isqrt.
        //    NNSO = |E|·(2S²)^14 = 16384|E|·S^28 for S-regular.
        //    Overflow per edge: (2×16129²)^14 → saturating u128 accumulator.

        let mut nheictc_acc: u128 = 0;
        let mut nnso_acc:    u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb    = sv[b] as u128;
                    // NHEICTC: (S_a + S_b)^19 = ss^16 × ss^2 × ss
                    let ss    = sa + sb;
                    let ss2   = ss * ss;
                    let ss4   = ss2 * ss2;
                    let ss8   = ss4.saturating_mul(ss4);
                    let ss16  = ss8.saturating_mul(ss8);
                    let ss19  = ss16.saturating_mul(ss2).saturating_mul(ss);
                    nheictc_acc = nheictc_acc.saturating_add(ss19);
                    // NNSO: (S_a² + S_b²)^14 = s2s^8 × s2s^4 × s2s^2
                    let s2s   = sa * sa + sb * sb;
                    let s2s2  = s2s * s2s;
                    let s2s4  = s2s2.saturating_mul(s2s2);
                    let s2s8  = s2s4.saturating_mul(s2s4);
                    let s2s14 = s2s8.saturating_mul(s2s4).saturating_mul(s2s2);
                    nnso_acc = nnso_acc.saturating_add(s2s14);
                }
            }
        }
        let nheictc = nheictc_acc.min(u64::MAX as u128) as u64;
        let nnso    = nnso_acc.min(u64::MAX as u128) as u64;

        (neictc, nheictc, nnso, edge_count, nc)
    }

    pub fn graph_topo_indices47_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NHENTC (S-Heneicosic vertex sum = Σ_v S(v)^21).
        //
        //    NHENTC(G) = Σ_v S(v)^21  (exact u128→u64; S-Heneicosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NEICTC=Σ S²⁰ (topo46) → NHENTC=Σ S²¹ (topo47)
        //    NHENTC = n·S^21 for S-regular.
        //    Overflow: S^21 ≤ 16129^21 → saturating u128 accumulator, clamp to u64::MAX.

        let mut nhentc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s21 = s16.saturating_mul(s4).saturating_mul(s);
            nhentc_acc = nhentc_acc.saturating_add(s21);
        }
        let nhentc = nhentc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHHENTC (S-Eicosic edge-sum) and NPSO (S-Triacontyl Sombor).
        //
        //    NHHENTC(G) = Σ_{uv∈E} (S_u+S_v)^20  (exact u128→u64; S-Eicosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHEICTC=Σ(S+S)¹⁹ (topo46)
        //      → NHHENTC=Σ(S+S)²⁰ (topo47)
        //    NHHENTC = |E|·(2S)^20 = 1048576|E|·S^20 for S-regular.
        //    Overflow per edge: (2×16129)^20 → saturating u128 accumulator.
        //
        //    NPSO(G) = Σ_{uv∈E} (S_u²+S_v²)^15  (exact u128→u64; S-Triacontyl Sombor α=30)
        //    S-variant generalised Sombor SO^α with α=30: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47) — exact, no isqrt. (O skipped: NOSO=α=8 already taken)
        //    NPSO = |E|·(2S²)^15 = 32768|E|·S^30 for S-regular.
        //    Overflow per edge: (2×16129²)^15 → saturating u128 accumulator.

        let mut nhhentc_acc: u128 = 0;
        let mut npso_acc:    u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHHENTC: (S_a + S_b)^20 = ss^16 × ss^4
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss20   = ss16.saturating_mul(ss4);
                    nhhentc_acc = nhhentc_acc.saturating_add(ss20);
                    // NPSO: (S_a² + S_b²)^15 = s2s^8 × s2s^4 × s2s^2 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s15  = s2s8.saturating_mul(s2s4).saturating_mul(s2s2).saturating_mul(s2s);
                    npso_acc = npso_acc.saturating_add(s2s15);
                }
            }
        }
        let nhhentc = nhhentc_acc.min(u64::MAX as u128) as u64;
        let npso    = npso_acc.min(u64::MAX as u128) as u64;

        (nhentc, nhhentc, npso, edge_count, nc)
    }

    pub fn graph_topo_indices48_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NDOCTC (S-Docosic vertex sum = Σ_v S(v)^22).
        //
        //    NDOCTC(G) = Σ_v S(v)^22  (exact u128→u64; S-Docosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NHENTC=Σ S²¹ (topo47) → NDOCTC=Σ S²² (topo48)
        //    NDOCTC = n·S^22 for S-regular.
        //    Overflow: S^22 ≤ 16129^22 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^22 = s^16 × s^4 × s^2.

        let mut ndoctc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s22 = s16.saturating_mul(s4).saturating_mul(s2);
            ndoctc_acc = ndoctc_acc.saturating_add(s22);
        }
        let ndoctc = ndoctc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHDOCTC (S-Heneicosic edge-sum) and NQSO (S-Dotriacontyl Sombor).
        //
        //    NHDOCTC(G) = Σ_{uv∈E} (S_u+S_v)^21  (exact u128→u64; S-Heneicosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHHENTC=Σ(S+S)²⁰ (topo47)
        //      → NHDOCTC=Σ(S+S)²¹ (topo48)
        //    NHDOCTC = |E|·(2S)^21 = 2097152|E|·S^21 for S-regular.
        //    Overflow per edge: (2×16129)^21 → saturating u128 accumulator.
        //    Implementation: ss^21 = ss^16 × ss^4 × ss.
        //
        //    NQSO(G) = Σ_{uv∈E} (S_u²+S_v²)^16  (exact u128→u64; S-Dotriacontyl Sombor α=32)
        //    S-variant generalised Sombor SO^α with α=32: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48) — exact, no isqrt.
        //    (Q used: O=α=8 taken; P=α=30 taken; Q follows in sequence)
        //    NQSO = |E|·(2S²)^16 = 65536|E|·S^32 for S-regular.
        //    Overflow per edge: (2×16129²)^16 → saturating u128 accumulator.
        //    Implementation: s2s^16 = s2s^8 × s2s^8.

        let mut nhdoctc_acc: u128 = 0;
        let mut nqso_acc:    u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHDOCTC: (S_a + S_b)^21 = ss^16 × ss^4 × ss
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss21   = ss16.saturating_mul(ss4).saturating_mul(ss);
                    nhdoctc_acc = nhdoctc_acc.saturating_add(ss21);
                    // NQSO: (S_a² + S_b²)^16 = s2s^8 × s2s^8
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    nqso_acc = nqso_acc.saturating_add(s2s16);
                }
            }
        }
        let nhdoctc = nhdoctc_acc.min(u64::MAX as u128) as u64;
        let nqso    = nqso_acc.min(u64::MAX as u128) as u64;

        (ndoctc, nhdoctc, nqso, edge_count, nc)
    }

    pub fn graph_topo_indices49_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NTRICTC (S-Tricosic vertex sum = Σ_v S(v)^23).
        //
        //    NTRICTC(G) = Σ_v S(v)^23  (exact u128→u64; S-Tricosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NDOCTC=Σ S²² (topo48) → NTRICTC=Σ S²³ (topo49)
        //    NTRICTC = n·S^23 for S-regular.
        //    Overflow: S^23 ≤ 16129^23 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^23 = s^16 × s^4 × s^2 × s.

        let mut ntrictc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s23 = s16.saturating_mul(s4).saturating_mul(s2).saturating_mul(s);
            ntrictc_acc = ntrictc_acc.saturating_add(s23);
        }
        let ntrictc = ntrictc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHTRICTC (S-Docosic edge-sum) and NRSO (S-Tetratriacontyl Sombor).
        //
        //    NHTRICTC(G) = Σ_{uv∈E} (S_u+S_v)^22  (exact u128→u64; S-Docosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHDOCTC=Σ(S+S)²¹ (topo48)
        //      → NHTRICTC=Σ(S+S)²² (topo49)
        //    NHTRICTC = |E|·(2S)^22 = 4194304|E|·S^22 for S-regular.
        //    Overflow per edge: (2×16129)^22 → saturating u128 accumulator.
        //    Implementation: ss^22 = ss^16 × ss^4 × ss^2.
        //
        //    NRSO(G) = Σ_{uv∈E} (S_u²+S_v²)^17  (exact u128→u64; S-Tetratriacontyl Sombor α=34)
        //    S-variant generalised Sombor SO^α with α=34: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49) — exact, no isqrt.
        //    (R used: O=α=8 taken; P=α=30 taken; Q=α=32 taken; R follows in sequence)
        //    NRSO = |E|·(2S²)^17 = 131072|E|·S^34 for S-regular.
        //    Overflow per edge: (2×16129²)^17 → saturating u128 accumulator.
        //    Implementation: s2s^17 = s2s^8 × s2s^8 × s2s.

        let mut nhtrictc_acc: u128 = 0;
        let mut nrso_acc:     u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHTRICTC: (S_a + S_b)^22 = ss^16 × ss^4 × ss^2
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss22   = ss16.saturating_mul(ss4).saturating_mul(ss2);
                    nhtrictc_acc = nhtrictc_acc.saturating_add(ss22);
                    // NRSO: (S_a² + S_b²)^17 = s2s^8 × s2s^8 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s17  = s2s8.saturating_mul(s2s8).saturating_mul(s2s);
                    nrso_acc = nrso_acc.saturating_add(s2s17);
                }
            }
        }
        let nhtrictc = nhtrictc_acc.min(u64::MAX as u128) as u64;
        let nrso     = nrso_acc.min(u64::MAX as u128) as u64;

        (ntrictc, nhtrictc, nrso, edge_count, nc)
    }

    pub fn graph_topo_indices54_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NOCTATC (S-Octacosic vertex sum = Σ_v S(v)^28).
        //
        //    NOCTATC(G) = Σ_v S(v)^28  (exact u128→u64; S-Octacosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NHEPTATC=Σ S²⁷ (topo53) → NOCTATC=Σ S²⁸ (topo54)
        //    NOCTATC = n·S^28 for S-regular.
        //    Overflow: S^28 ≤ 16129^28 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^28 = s^16 × s^8 × s^4.

        let mut noctatc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s28 = s16.saturating_mul(s8).saturating_mul(s4);
            noctatc_acc = noctatc_acc.saturating_add(s28);
        }
        let noctatc = noctatc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHOCTATC (S-Heptacosic edge-sum) and NYSO (S-Tetratetracontyl Sombor).
        //
        //    NHOCTATC(G) = Σ_{uv∈E} (S_u+S_v)^27  (exact u128→u64; S-Heptacosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHHEPTATC=Σ(S+S)²⁶ (topo53)
        //      → NHOCTATC=Σ(S+S)²⁷ (topo54)
        //    NHOCTATC = |E|·(2S)^27 = 134217728|E|·S^27 for S-regular.
        //    Overflow per edge: (2×16129)^27 → saturating u128 accumulator.
        //    Implementation: ss^27 = ss^16 × ss^8 × ss^2 × ss.
        //
        //    NYSO(G) = Σ_{uv∈E} (S_u²+S_v²)^22  (exact u128→u64; S-Tetratetracontyl Sombor α=44)
        //    S-variant generalised Sombor SO^α with α=44: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54).
        //    NYSO = |E|·(2S²)^22 = 4194304|E|·S^44 for S-regular.
        //    Overflow per edge: (2×16129²)^22 → saturating u128 accumulator.
        //    Implementation: s2s^22 = s2s^16 × s2s^4 × s2s^2.

        let mut nhoctatc_acc: u128 = 0;
        let mut nyso_acc:     u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHOCTATC: (S_a + S_b)^27 = ss^16 × ss^8 × ss^2 × ss
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss27   = ss16.saturating_mul(ss8).saturating_mul(ss2).saturating_mul(ss);
                    nhoctatc_acc = nhoctatc_acc.saturating_add(ss27);
                    // NYSO: (S_a² + S_b²)^22 = s2s^16 × s2s^4 × s2s^2
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s22  = s2s16.saturating_mul(s2s4).saturating_mul(s2s2);
                    nyso_acc = nyso_acc.saturating_add(s2s22);
                }
            }
        }
        let nhoctatc = nhoctatc_acc.min(u64::MAX as u128) as u64;
        let nyso     = nyso_acc.min(u64::MAX as u128) as u64;

        (noctatc, nhoctatc, nyso, edge_count, nc)
    }

    pub fn graph_topo_indices62_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NHEXATRIACTC (S-Hexatriacontic vertex sum = Σ_v S(v)^36).
        //
        //    NHEXATRIACTC(G) = Σ_v S(v)^36  (exact u128→u64; S-Hexatriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NPENTTRIACTC=Σ S³⁵ (topo61) → NHEXATRIACTC=Σ S³⁶ (topo62)
        //    NHEXATRIACTC = n·S^36 for S-regular.
        //    Overflow: S^36 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^36 = s32 × s4  (s32 = s16^2 perfect square; s4 = s2^2).

        let mut nhexatriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s36 = s32.saturating_mul(s4);
            nhexatriactc_acc = nhexatriactc_acc.saturating_add(s36);
        }
        let nhexatriactc = nhexatriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHHEXATRIACTC (S-Pentatriacontic edge-sum) and NAESO (S-Hexacontyl Sombor).
        //
        //    NHHEXATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^35  (exact u128→u64; S-Pentatriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHPENTTRIACTC=Σ(S+S)³⁴ (topo61)
        //      → NHHEXATRIACTC=Σ(S+S)³⁵ (topo62)
        //    NHHEXATRIACTC = |E|·(2S)^35 = 34359738368|E|·S^35 for S-regular.
        //    Overflow per edge: (2×16129)^35 → saturating u128 accumulator.
        //    Implementation: ss^35 = ss32 × ss2 × ss  (ss32 = ss16^2 perfect square).
        //
        //    NAESO(G) = Σ_{uv∈E} (S_u²+S_v²)^30  (exact u128→u64; S-Hexacontyl Sombor α=60)
        //    S-variant generalised Sombor SO^α with α=60: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56), NBSO(α=50,topo57),
        //    NAASO(α=52,topo58), NABSO(α=54,topo59), NACSO(α=56,topo60), NADSO(α=58,topo61),
        //    NAESO(α=60,topo62). (3rd-pass AE.)
        //    NAESO = |E|·(2S²)^30 = 1073741824|E|·S^60 for S-regular.
        //    Overflow per edge: (2×16129²)^30 → saturating u128 accumulator.
        //    Implementation: s2s^30 = s2s16 × s2s8 × s2s4 × s2s2.

        let mut nhhexatriactc_acc: u128 = 0;
        let mut naeso_acc:          u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHHEXATRIACTC: (S_a + S_b)^35 = ss32 × ss2 × ss  (ss32 = ss16^2)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    let ss35   = ss32.saturating_mul(ss2).saturating_mul(ss);
                    nhhexatriactc_acc = nhhexatriactc_acc.saturating_add(ss35);
                    // NAESO: (S_a² + S_b²)^30 = s2s16 × s2s8 × s2s4 × s2s2
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s30  = s2s16.saturating_mul(s2s8).saturating_mul(s2s4).saturating_mul(s2s2);
                    naeso_acc = naeso_acc.saturating_add(s2s30);
                }
            }
        }
        let nhhexatriactc = nhhexatriactc_acc.min(u64::MAX as u128) as u64;
        let naeso         = naeso_acc.min(u64::MAX as u128) as u64;

        (nhexatriactc, nhhexatriactc, naeso, edge_count, nc)
    }

    pub fn graph_topo_indices63_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NHEPTATRIACTC (S-Heptatriacontic vertex sum = Σ_v S(v)^37).
        //
        //    NHEPTATRIACTC(G) = Σ_v S(v)^37  (exact u128→u64; S-Heptatriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NHEXATRIACTC=Σ S³⁶ (topo62) → NHEPTATRIACTC=Σ S³⁷ (topo63)
        //    NHEPTATRIACTC = n·S^37 for S-regular.
        //    Overflow: S^37 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^37 = s32 × s4 × s  (s32 = s16^2 perfect square; s4 = s2^2).

        let mut nheptatriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s37 = s32.saturating_mul(s4).saturating_mul(s);
            nheptatriactc_acc = nheptatriactc_acc.saturating_add(s37);
        }
        let nheptatriactc = nheptatriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHHEPTATRIACTC (S-Hexatriacontic edge-sum) and NAFSO (S-Hexahexacontyl Sombor).
        //
        //    NHHEPTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^36  (exact u128→u64; S-Hexatriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHHEXATRIACTC=Σ(S+S)³⁵ (topo62)
        //      → NHHEPTATRIACTC=Σ(S+S)³⁶ (topo63)
        //    NHHEPTATRIACTC = |E|·(2S)^36 = 68719476736|E|·S^36 for S-regular.
        //    Overflow per edge: (2×16129)^36 → saturating u128 accumulator.
        //    Implementation: ss^36 = ss32 × ss4  (ss32 = ss16^2 perfect square; ss4 = ss2^2).
        //
        //    NAFSO(G) = Σ_{uv∈E} (S_u²+S_v²)^31  (exact u128→u64; S-Hexahexacontyl Sombor α=62)
        //    S-variant generalised Sombor SO^α with α=62: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56), NBSO(α=50,topo57),
        //    NAASO(α=52,topo58), NABSO(α=54,topo59), NACSO(α=56,topo60), NADSO(α=58,topo61),
        //    NAESO(α=60,topo62), NAFSO(α=62,topo63). (3rd-pass AF.)
        //    NAFSO = |E|·(2S²)^31 = 2147483648|E|·S^62 for S-regular.
        //    Overflow per edge: (2×16129²)^31 → saturating u128 accumulator.
        //    Implementation: s2s^31 = s2s16 × s2s8 × s2s4 × s2s2 × s2s.

        let mut nhheptatriactc_acc: u128 = 0;
        let mut nafso_acc:           u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHHEPTATRIACTC: (S_a + S_b)^36 = ss32 × ss4  (ss32 = ss16^2; ss4 = ss2^2)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    let ss36   = ss32.saturating_mul(ss4);
                    nhheptatriactc_acc = nhheptatriactc_acc.saturating_add(ss36);
                    // NAFSO: (S_a² + S_b²)^31 = s2s16 × s2s8 × s2s4 × s2s2 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s31  = s2s16.saturating_mul(s2s8).saturating_mul(s2s4).saturating_mul(s2s2).saturating_mul(s2s);
                    nafso_acc = nafso_acc.saturating_add(s2s31);
                }
            }
        }
        let nhheptatriactc = nhheptatriactc_acc.min(u64::MAX as u128) as u64;
        let nafso          = nafso_acc.min(u64::MAX as u128) as u64;

        (nheptatriactc, nhheptatriactc, nafso, edge_count, nc)
    }

    pub fn graph_topo_indices64_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NOCTATRIACTC (S-Octatriacontic vertex sum = Σ_v S(v)^38).
        //
        //    NOCTATRIACTC(G) = Σ_v S(v)^38  (exact u128→u64; S-Octatriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NHEPTATRIACTC=Σ S³⁷ (topo63) → NOCTATRIACTC=Σ S³⁸ (topo64)
        //    NOCTATRIACTC = n·S^38 for S-regular.
        //    Overflow: S^38 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^38 = s32 × s4 × s2  (s32=s16^2 perfect square; s4=s2^2; 38=32+4+2).

        let mut noctatriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s38 = s32.saturating_mul(s4).saturating_mul(s2);
            noctatriactc_acc = noctatriactc_acc.saturating_add(s38);
        }
        let noctatriactc = noctatriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHOCTATRIACTC (S-Heptatriacontic edge-sum) and NAGSO (S-Tetrahexacontyl Sombor).
        //
        //    NHOCTATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^37  (exact u128→u64; S-Heptatriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHHEPTATRIACTC=Σ(S+S)³⁶ (topo63)
        //      → NHOCTATRIACTC=Σ(S+S)³⁷ (topo64)
        //    NHOCTATRIACTC = |E|·(2S)^37 = 137438953472|E|·S^37 for S-regular.
        //    Overflow per edge: (2×16129)^37 → saturating u128 accumulator.
        //    Implementation: ss^37 = ss32 × ss4 × ss  (ss32=ss16^2; ss4=ss2^2; 37=32+4+1).
        //
        //    NAGSO(G) = Σ_{uv∈E} (S_u²+S_v²)^32  (exact u128→u64; S-Tetrahexacontyl Sombor α=64)
        //    S-variant generalised Sombor SO^α with α=64: exact integer (no isqrt).
        //    NSO(α=1), NCSO(α=3), NFSO(α=4), NHSO(α=6), NOSO(α=8), NTSO(α=10),
        //    NDSO(α=12), NESO(α=14), NGSO(α=16), NIOSO(α=18), NJSO(α=20), NKSO(α=22),
        //    NLSO(α=24), NMSO(α=26), NNSO(α=28), NPSO(α=30), NQSO(α=32), NRSO(α=34),
        //    NSSO(α=36), NUSO(α=38), NVSO(α=40), NXSO(α=42), NYSO(α=44), NZSO(α=46),
        //    NASO(α=48), NBSO(α=50), NAASO(α=52), NABSO(α=54), NACSO(α=56), NADSO(α=58),
        //    NAESO(α=60), NAFSO(α=62), NAGSO(α=64). (3rd-pass AG.)
        //    NAGSO = |E|·(2S²)^32 = 4294967296|E|·S^64 for S-regular.
        //    Overflow per edge: (2×16129²)^32 → saturating u128 accumulator.
        //    Implementation: s2s^32 = s2s16 × s2s16  (perfect square; 32=16+16).

        let mut nhoctatriactc_acc: u128 = 0;
        let mut nagso_acc:          u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHOCTATRIACTC: (S_a + S_b)^37 = ss32 × ss4 × ss  (ss32=ss16^2; ss4=ss2^2)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    let ss37   = ss32.saturating_mul(ss4).saturating_mul(ss);
                    nhoctatriactc_acc = nhoctatriactc_acc.saturating_add(ss37);
                    // NAGSO: (S_a² + S_b²)^32 = s2s16 × s2s16  (perfect square; 32=16+16)
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s32  = s2s16.saturating_mul(s2s16);
                    nagso_acc = nagso_acc.saturating_add(s2s32);
                }
            }
        }
        let nhoctatriactc = nhoctatriactc_acc.min(u64::MAX as u128) as u64;
        let nagso         = nagso_acc.min(u64::MAX as u128) as u64;

        (noctatriactc, nhoctatriactc, nagso, edge_count, nc)
    }

    pub fn graph_topo_indices65_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NNONATRIACTC (S-Nonatriacontic vertex sum = Σ_v S(v)^39).
        //
        //    NNONATRIACTC(G) = Σ_v S(v)^39  (exact u128→u64; S-Nonatriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NOCTATRIACTC=Σ S³⁸ (topo64) → NNONATRIACTC=Σ S³⁹ (topo65)
        //    NNONATRIACTC = n·S^39 for S-regular.
        //    Overflow: S^39 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^39 = s32 × s4 × s2 × s  (s32=s16^2; s4=s2^2; 39=32+4+2+1).

        let mut nnonatriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s39 = s32.saturating_mul(s4).saturating_mul(s2).saturating_mul(s);
            nnonatriactc_acc = nnonatriactc_acc.saturating_add(s39);
        }
        let nnonatriactc = nnonatriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHNONATRIACTC (S-Octatriacontic edge-sum) and NAHSO (S-Hexahexacontyl Sombor).
        //
        //    NHNONATRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^38  (exact u128→u64; S-Octatriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHOCTATRIACTC=Σ(S+S)³⁷ (topo64)
        //      → NHNONATRIACTC=Σ(S+S)³⁸ (topo65)
        //    NHNONATRIACTC = |E|·(2S)^38 = 274877906944|E|·S^38 for S-regular.
        //    Overflow per edge: (2×16129)^38 → saturating u128 accumulator.
        //    Implementation: ss^38 = ss32 × ss4 × ss2  (ss32=ss16^2; ss4=ss2^2; 38=32+4+2).
        //
        //    NAHSO(G) = Σ_{uv∈E} (S_u²+S_v²)^33  (exact u128→u64; S-Hexahexacontyl Sombor α=66)
        //    S-variant generalised Sombor SO^α with α=66: exact integer (no isqrt).
        //    NSO(α=1), NCSO(α=3), NFSO(α=4), NHSO(α=6), NOSO(α=8), NTSO(α=10),
        //    NDSO(α=12), NESO(α=14), NGSO(α=16), NIOSO(α=18), NJSO(α=20), NKSO(α=22),
        //    NLSO(α=24), NMSO(α=26), NNSO(α=28), NPSO(α=30), NQSO(α=32), NRSO(α=34),
        //    NSSO(α=36), NUSO(α=38), NVSO(α=40), NXSO(α=42), NYSO(α=44), NZSO(α=46),
        //    NASO(α=48), NBSO(α=50), NAASO(α=52), NABSO(α=54), NACSO(α=56), NADSO(α=58),
        //    NAESO(α=60), NAFSO(α=62), NAGSO(α=64), NAHSO(α=66). (3rd-pass AH.)
        //    NAHSO = |E|·(2S²)^33 = 8589934592|E|·S^66 for S-regular.
        //    Overflow per edge: (2×16129²)^33 → saturating u128 accumulator.
        //    Implementation: s2s^33 = s2s32 × s2s  (s2s32=s2s16^2; 33=32+1).

        let mut nhnonatriactc_acc: u128 = 0;
        let mut nahso_acc:          u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHNONATRIACTC: (S_a + S_b)^38 = ss32 × ss4 × ss2  (ss32=ss16^2; ss4=ss2^2)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    let ss38   = ss32.saturating_mul(ss4).saturating_mul(ss2);
                    nhnonatriactc_acc = nhnonatriactc_acc.saturating_add(ss38);
                    // NAHSO: (S_a² + S_b²)^33 = s2s32 × s2s  (s2s32=s2s16^2; 33=32+1)
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s32  = s2s16.saturating_mul(s2s16);
                    let s2s33  = s2s32.saturating_mul(s2s);
                    nahso_acc = nahso_acc.saturating_add(s2s33);
                }
            }
        }
        let nhnonatriactc = nhnonatriactc_acc.min(u64::MAX as u128) as u64;
        let nahso         = nahso_acc.min(u64::MAX as u128) as u64;

        (nnonatriactc, nhnonatriactc, nahso, edge_count, nc)
    }

    pub fn graph_topo_indices66_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NTETRAACTC (S-Tetracontic vertex sum = Σ_v S(v)^40).
        //
        //    NTETRAACTC(G) = Σ_v S(v)^40  (exact u128→u64; S-Tetracontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NNONATRIACTC=Σ S³⁹ (topo65) → NTETRAACTC=Σ S⁴⁰ (topo66)
        //    NTETRAACTC = n·S^40 for S-regular.
        //    Overflow: S^40 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^40 = s32 × s8  (s32=s16^2; s8=s4^2; 40=32+8).

        let mut ntetraactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s40 = s32.saturating_mul(s8);
            ntetraactc_acc = ntetraactc_acc.saturating_add(s40);
        }
        let ntetraactc = ntetraactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHTETRAACTC (S-Nonatriacontic edge-sum) and NAISO (S-Octahexacontyl Sombor).
        //
        //    NHTETRAACTC(G) = Σ_{uv∈E} (S_u+S_v)^39  (exact u128→u64; S-Nonatriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHNONATRIACTC=Σ(S+S)³⁸ (topo65)
        //      → NHTETRAACTC=Σ(S+S)³⁹ (topo66)
        //    NHTETRAACTC = |E|·(2S)^39 = 549755813888|E|·S^39 for S-regular.
        //    Overflow per edge: (2×16129)^39 → saturating u128 accumulator.
        //    Implementation: ss^39 = ss32 × ss4 × ss2 × ss  (ss32=ss16^2; ss4=ss2^2; 39=32+4+2+1).
        //
        //    NAISO(G) = Σ_{uv∈E} (S_u²+S_v²)^34  (exact u128→u64; S-Octahexacontyl Sombor α=68)
        //    S-variant generalised Sombor SO^α with α=68: exact integer (no isqrt).
        //    NSO(α=1), NCSO(α=3), NFSO(α=4), NHSO(α=6), NOSO(α=8), NTSO(α=10),
        //    NDSO(α=12), NESO(α=14), NGSO(α=16), NIOSO(α=18), NJSO(α=20), NKSO(α=22),
        //    NLSO(α=24), NMSO(α=26), NNSO(α=28), NPSO(α=30), NQSO(α=32), NRSO(α=34),
        //    NSSO(α=36), NUSO(α=38), NVSO(α=40), NXSO(α=42), NYSO(α=44), NZSO(α=46),
        //    NASO(α=48), NBSO(α=50), NAASO(α=52), NABSO(α=54), NACSO(α=56), NADSO(α=58),
        //    NAESO(α=60), NAFSO(α=62), NAGSO(α=64), NAHSO(α=66), NAISO(α=68). (3rd-pass AI.)
        //    NAISO = |E|·(2S²)^34 = 17179869184|E|·S^68 for S-regular.
        //    Overflow per edge: (2×16129²)^34 → saturating u128 accumulator.
        //    Implementation: s2s^34 = s2s32 × s2s2  (s2s32=s2s16^2; 34=32+2).

        let mut nhtetraactc_acc: u128 = 0;
        let mut naiso_acc:        u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHTETRAACTC: (S_a + S_b)^39 = ss32 × ss4 × ss2 × ss  (39=32+4+2+1)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    let ss39   = ss32.saturating_mul(ss4).saturating_mul(ss2).saturating_mul(ss);
                    nhtetraactc_acc = nhtetraactc_acc.saturating_add(ss39);
                    // NAISO: (S_a² + S_b²)^34 = s2s32 × s2s2  (34=32+2)
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s32  = s2s16.saturating_mul(s2s16);
                    let s2s34  = s2s32.saturating_mul(s2s2);
                    naiso_acc = naiso_acc.saturating_add(s2s34);
                }
            }
        }
        let nhtetraactc = nhtetraactc_acc.min(u64::MAX as u128) as u64;
        let naiso       = naiso_acc.min(u64::MAX as u128) as u64;

        (ntetraactc, nhtetraactc, naiso, edge_count, nc)
    }

    pub fn graph_topo_indices61_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NPENTTRIACTC (S-Pentatriacontic vertex sum = Σ_v S(v)^35).
        //
        //    NPENTTRIACTC(G) = Σ_v S(v)^35  (exact u128→u64; S-Pentatriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NTETRTRIACTC=Σ S³⁴ (topo60) → NPENTTRIACTC=Σ S³⁵ (topo61)
        //    NPENTTRIACTC = n·S^35 for S-regular.
        //    Overflow: S^35 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^35 = s16 × s16 × s2 × s  (s^32 as perfect square, then × s^2 × s).

        let mut npenttriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s35 = s32.saturating_mul(s2).saturating_mul(s);
            npenttriactc_acc = npenttriactc_acc.saturating_add(s35);
        }
        let npenttriactc = npenttriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHPENTTRIACTC (S-Tetratriacontic edge-sum) and NADSO (S-Octopentacontyl Sombor).
        //
        //    NHPENTTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^34  (exact u128→u64; S-Tetratriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHTETRTRIACTC=Σ(S+S)³³ (topo60)
        //      → NHPENTTRIACTC=Σ(S+S)³⁴ (topo61)
        //    NHPENTTRIACTC = |E|·(2S)^34 = 17179869184|E|·S^34 for S-regular.
        //    Overflow per edge: (2×16129)^34 → saturating u128 accumulator.
        //    Implementation: ss^34 = ss16 × ss16 × ss2  (ss^32 as perfect square, then × ss^2).
        //
        //    NADSO(G) = Σ_{uv∈E} (S_u²+S_v²)^29  (exact u128→u64; S-Octopentacontyl Sombor α=58)
        //    S-variant generalised Sombor SO^α with α=58: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56), NBSO(α=50,topo57),
        //    NAASO(α=52,topo58), NABSO(α=54,topo59), NACSO(α=56,topo60), NADSO(α=58,topo61). (3rd-pass AD.)
        //    NADSO = |E|·(2S²)^29 = 536870912|E|·S^58 for S-regular.
        //    Overflow per edge: (2×16129²)^29 → saturating u128 accumulator.
        //    Implementation: s2s^29 = s2s16 × s2s8 × s2s4 × s2s.

        let mut nhpenttriactc_acc: u128 = 0;
        let mut nadso_acc:          u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHPENTTRIACTC: (S_a + S_b)^34 = ss^32 × ss^2  (ss^32 as perfect square)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    let ss34   = ss32.saturating_mul(ss2);
                    nhpenttriactc_acc = nhpenttriactc_acc.saturating_add(ss34);
                    // NADSO: (S_a² + S_b²)^29 = s2s16 × s2s8 × s2s4 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s29  = s2s16.saturating_mul(s2s8).saturating_mul(s2s4).saturating_mul(s2s);
                    nadso_acc = nadso_acc.saturating_add(s2s29);
                }
            }
        }
        let nhpenttriactc = nhpenttriactc_acc.min(u64::MAX as u128) as u64;
        let nadso         = nadso_acc.min(u64::MAX as u128) as u64;

        (npenttriactc, nhpenttriactc, nadso, edge_count, nc)
    }

    pub fn graph_topo_indices60_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NTETRTRIACTC (S-Tetratriacontic vertex sum = Σ_v S(v)^34).
        //
        //    NTETRTRIACTC(G) = Σ_v S(v)^34  (exact u128→u64; S-Tetratriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NTRITRIACTC=Σ S³³ (topo59) → NTETRTRIACTC=Σ S³⁴ (topo60)
        //    NTETRTRIACTC = n·S^34 for S-regular.
        //    Overflow: S^34 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^34 = s16 × s16 × s2  (s^32 as perfect square, then × s^2).

        let mut ntetrtriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s34 = s32.saturating_mul(s2);
            ntetrtriactc_acc = ntetrtriactc_acc.saturating_add(s34);
        }
        let ntetrtriactc = ntetrtriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHTETRTRIACTC (S-Tritriacontic edge-sum) and NACSO (S-Hexapentacontyl Sombor).
        //
        //    NHTETRTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^33  (exact u128→u64; S-Tritriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHTRITRIACTC=Σ(S+S)³² (topo59)
        //      → NHTETRTRIACTC=Σ(S+S)³³ (topo60)
        //    NHTETRTRIACTC = |E|·(2S)^33 = 8589934592|E|·S^33 for S-regular.
        //    Overflow per edge: (2×16129)^33 → saturating u128 accumulator.
        //    Implementation: ss^33 = ss16 × ss16 × ss  (ss^32 as perfect square, then × ss).
        //
        //    NACSO(G) = Σ_{uv∈E} (S_u²+S_v²)^28  (exact u128→u64; S-Hexapentacontyl Sombor α=56)
        //    S-variant generalised Sombor SO^α with α=56: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56), NBSO(α=50,topo57),
        //    NAASO(α=52,topo58), NABSO(α=54,topo59), NACSO(α=56,topo60). (3rd-pass AC.)
        //    NACSO = |E|·(2S²)^28 = 268435456|E|·S^56 for S-regular.
        //    Overflow per edge: (2×16129²)^28 → saturating u128 accumulator.
        //    Implementation: s2s^28 = s2s16 × s2s8 × s2s4.

        let mut nhtetrtriactc_acc: u128 = 0;
        let mut nacso_acc:          u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHTETRTRIACTC: (S_a + S_b)^33 = ss^32 × ss  (ss^32 as perfect square)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    let ss33   = ss32.saturating_mul(ss);
                    nhtetrtriactc_acc = nhtetrtriactc_acc.saturating_add(ss33);
                    // NACSO: (S_a² + S_b²)^28 = s2s16 × s2s8 × s2s4
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s28  = s2s16.saturating_mul(s2s8).saturating_mul(s2s4);
                    nacso_acc = nacso_acc.saturating_add(s2s28);
                }
            }
        }
        let nhtetrtriactc = nhtetrtriactc_acc.min(u64::MAX as u128) as u64;
        let nacso         = nacso_acc.min(u64::MAX as u128) as u64;

        (ntetrtriactc, nhtetrtriactc, nacso, edge_count, nc)
    }

    pub fn graph_topo_indices59_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NTRITRIACTC (S-Tritriacontic vertex sum = Σ_v S(v)^33).
        //
        //    NTRITRIACTC(G) = Σ_v S(v)^33  (exact u128→u64; S-Tritriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NDOTRIACTC=Σ S³² (topo58) → NTRITRIACTC=Σ S³³ (topo59)
        //    NTRITRIACTC = n·S^33 for S-regular.
        //    Overflow: S^33 > u64::MAX for S≥2 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^33 = s16 × s16 × s  (s^32 as perfect square, then × s).

        let mut ntritriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            let s33 = s32.saturating_mul(s);
            ntritriactc_acc = ntritriactc_acc.saturating_add(s33);
        }
        let ntritriactc = ntritriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHTRITRIACTC (S-Dotriacontic edge-sum) and NABSO (S-Dopentatecontyl Sombor).
        //
        //    NHTRITRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^32  (exact u128→u64; S-Dotriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHDOTRIACTC=Σ(S+S)³¹ (topo58)
        //      → NHTRITRIACTC=Σ(S+S)³² (topo59)
        //    NHTRITRIACTC = |E|·(2S)^32 = 4294967296|E|·S^32 for S-regular.
        //    Overflow per edge: (2×16129)^32 → saturating u128 accumulator.
        //    Implementation: ss^32 = ss16 × ss16  (perfect square, simplest form).
        //
        //    NABSO(G) = Σ_{uv∈E} (S_u²+S_v²)^27  (exact u128→u64; S-Dopentatecontyl Sombor α=54)
        //    S-variant generalised Sombor SO^α with α=54: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56), NBSO(α=50,topo57),
        //    NAASO(α=52,topo58), NABSO(α=54,topo59). (3rd-pass double-letter AB.)
        //    NABSO = |E|·(2S²)^27 = 134217728|E|·S^54 for S-regular.
        //    Overflow per edge: (2×16129²)^27 → saturating u128 accumulator.
        //    Implementation: s2s^27 = s2s16 × s2s8 × s2s2 × s2s.

        let mut nhtritriactc_acc: u128 = 0;
        let mut nabso_acc:         u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHTRITRIACTC: (S_a + S_b)^32 = ss^16 × ss^16  (perfect square)
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss32   = ss16.saturating_mul(ss16);
                    nhtritriactc_acc = nhtritriactc_acc.saturating_add(ss32);
                    // NABSO: (S_a² + S_b²)^27 = s2s^16 × s2s^8 × s2s^2 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s27  = s2s16.saturating_mul(s2s8).saturating_mul(s2s2).saturating_mul(s2s);
                    nabso_acc = nabso_acc.saturating_add(s2s27);
                }
            }
        }
        let nhtritriactc = nhtritriactc_acc.min(u64::MAX as u128) as u64;
        let nabso        = nabso_acc.min(u64::MAX as u128) as u64;

        (ntritriactc, nhtritriactc, nabso, edge_count, nc)
    }

    pub fn graph_topo_indices58_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NDOTRIACTC (S-Dotriacontic vertex sum = Σ_v S(v)^32).
        //
        //    NDOTRIACTC(G) = Σ_v S(v)^32  (exact u128→u64; S-Dotriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NHENTRIACTC=Σ S³¹ (topo57) → NDOTRIACTC=Σ S³² (topo58)
        //    NDOTRIACTC = n·S^32 for S-regular.
        //    Overflow: S^32 ≤ 16129^32 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^32 = s16 × s16  (perfect square, simplest form).

        let mut ndotriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s32 = s16.saturating_mul(s16);
            ndotriactc_acc = ndotriactc_acc.saturating_add(s32);
        }
        let ndotriactc = ndotriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHDOTRIACTC (S-Hentriacontic edge-sum) and NAASO (S-Dopentecontyl Sombor).
        //
        //    NHDOTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^31  (exact u128→u64; S-Hentriacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHHENTRIACTC=Σ(S+S)³⁰ (topo57)
        //      → NHDOTRIACTC=Σ(S+S)³¹ (topo58)
        //    NHDOTRIACTC = |E|·(2S)^31 = 2147483648|E|·S^31 for S-regular.
        //    Overflow per edge: (2×16129)^31 → saturating u128 accumulator.
        //    Implementation: ss^31 = ss16 × ss8 × ss4 × ss2 × ss.
        //
        //    NAASO(G) = Σ_{uv∈E} (S_u²+S_v²)^26  (exact u128→u64; S-Dopentecontyl Sombor α=52)
        //    S-variant generalised Sombor SO^α with α=52: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56), NBSO(α=50,topo57),
        //    NAASO(α=52,topo58). (All 26 single letters exhausted; 3rd-pass double-letter AA.)
        //    NAASO = |E|·(2S²)^26 = 67108864|E|·S^52 for S-regular.
        //    Overflow per edge: (2×16129²)^26 → saturating u128 accumulator.
        //    Implementation: s2s^26 = s2s16 × s2s8 × s2s2.

        let mut nhdotriactc_acc: u128 = 0;
        let mut naaso_acc:        u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHDOTRIACTC: (S_a + S_b)^31 = ss^16 × ss^8 × ss^4 × ss^2 × ss
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss31   = ss16.saturating_mul(ss8).saturating_mul(ss4).saturating_mul(ss2).saturating_mul(ss);
                    nhdotriactc_acc = nhdotriactc_acc.saturating_add(ss31);
                    // NAASO: (S_a² + S_b²)^26 = s2s^16 × s2s^8 × s2s^2
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s26  = s2s16.saturating_mul(s2s8).saturating_mul(s2s2);
                    naaso_acc = naaso_acc.saturating_add(s2s26);
                }
            }
        }
        let nhdotriactc = nhdotriactc_acc.min(u64::MAX as u128) as u64;
        let naaso       = naaso_acc.min(u64::MAX as u128) as u64;

        (ndotriactc, nhdotriactc, naaso, edge_count, nc)
    }

    pub fn graph_topo_indices57_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NHENTRIACTC (S-Hentriacontic vertex sum = Σ_v S(v)^31).
        //
        //    NHENTRIACTC(G) = Σ_v S(v)^31  (exact u128→u64; S-Hentriacontic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NTRIACTC=Σ S³⁰ (topo56) → NHENTRIACTC=Σ S³¹ (topo57)
        //    NHENTRIACTC = n·S^31 for S-regular.
        //    Overflow: S^31 ≤ 16129^31 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^31 = s^16 × s^8 × s^4 × s^2 × s.

        let mut nhentriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s31 = s16.saturating_mul(s8).saturating_mul(s4).saturating_mul(s2).saturating_mul(s);
            nhentriactc_acc = nhentriactc_acc.saturating_add(s31);
        }
        let nhentriactc = nhentriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHHENTRIACTC (S-Triacontic edge-sum) and NBSO (S-Pentacontyl Sombor).
        //
        //    NHHENTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^30  (exact u128→u64; S-Triacontic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHTRIACTC=Σ(S+S)²⁹ (topo56)
        //      → NHHENTRIACTC=Σ(S+S)³⁰ (topo57)
        //    NHHENTRIACTC = |E|·(2S)^30 = 1073741824|E|·S^30 for S-regular.
        //    Overflow per edge: (2×16129)^30 → saturating u128 accumulator.
        //    Implementation: ss^30 = ss^16 × ss^8 × ss^4 × ss^2.
        //
        //    NBSO(G) = Σ_{uv∈E} (S_u²+S_v²)^25  (exact u128→u64; S-Pentacontyl Sombor α=50)
        //    S-variant generalised Sombor SO^α with α=50: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56), NBSO(α=50,topo57).
        //    NBSO = |E|·(2S²)^25 = 33554432|E|·S^50 for S-regular.
        //    Overflow per edge: (2×16129²)^25 → saturating u128 accumulator.
        //    Implementation: s2s^25 = s2s^16 × s2s^8 × s2s.

        let mut nhhentriactc_acc: u128 = 0;
        let mut nbso_acc:         u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHHENTRIACTC: (S_a + S_b)^30 = ss^16 × ss^8 × ss^4 × ss^2
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss30   = ss16.saturating_mul(ss8).saturating_mul(ss4).saturating_mul(ss2);
                    nhhentriactc_acc = nhhentriactc_acc.saturating_add(ss30);
                    // NBSO: (S_a² + S_b²)^25 = s2s^16 × s2s^8 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s25  = s2s16.saturating_mul(s2s8).saturating_mul(s2s);
                    nbso_acc = nbso_acc.saturating_add(s2s25);
                }
            }
        }
        let nhhentriactc = nhhentriactc_acc.min(u64::MAX as u128) as u64;
        let nbso         = nbso_acc.min(u64::MAX as u128) as u64;

        (nhentriactc, nhhentriactc, nbso, edge_count, nc)
    }

    pub fn graph_topo_indices56_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NTRIACTC (S-Triacontyl vertex sum = Σ_v S(v)^30).
        //
        //    NTRIACTC(G) = Σ_v S(v)^30  (exact u128→u64; S-Triacontyl vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NNONATC=Σ S²⁹ (topo55) → NTRIACTC=Σ S³⁰ (topo56)
        //    NTRIACTC = n·S^30 for S-regular.
        //    Overflow: S^30 ≤ 16129^30 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^30 = s^16 × s^8 × s^4 × s^2.

        let mut ntriactc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s30 = s16.saturating_mul(s8).saturating_mul(s4).saturating_mul(s2);
            ntriactc_acc = ntriactc_acc.saturating_add(s30);
        }
        let ntriactc = ntriactc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHTRIACTC (S-Nonacosic edge-sum) and NASO (S-Octatetracontyl Sombor).
        //
        //    NHTRIACTC(G) = Σ_{uv∈E} (S_u+S_v)^29  (exact u128→u64; S-Nonacosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHNONATC=Σ(S+S)²⁸ (topo55)
        //      → NHTRIACTC=Σ(S+S)²⁹ (topo56)
        //    NHTRIACTC = |E|·(2S)^29 = 536870912|E|·S^29 for S-regular.
        //    Overflow per edge: (2×16129)^29 → saturating u128 accumulator.
        //    Implementation: ss^29 = ss^16 × ss^8 × ss^4 × ss.
        //
        //    NASO(G) = Σ_{uv∈E} (S_u²+S_v²)^24  (exact u128→u64; S-Octatetracontyl Sombor α=48)
        //    S-variant generalised Sombor SO^α with α=48: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55), NASO(α=48,topo56).
        //    NASO = |E|·(2S²)^24 = 16777216|E|·S^48 for S-regular.
        //    Overflow per edge: (2×16129²)^24 → saturating u128 accumulator.
        //    Implementation: s2s^24 = s2s^16 × s2s^8.

        let mut nhtriactc_acc: u128 = 0;
        let mut naso_acc:      u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHTRIACTC: (S_a + S_b)^29 = ss^16 × ss^8 × ss^4 × ss
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss29   = ss16.saturating_mul(ss8).saturating_mul(ss4).saturating_mul(ss);
                    nhtriactc_acc = nhtriactc_acc.saturating_add(ss29);
                    // NASO: (S_a² + S_b²)^24 = s2s^16 × s2s^8
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s24  = s2s16.saturating_mul(s2s8);
                    naso_acc = naso_acc.saturating_add(s2s24);
                }
            }
        }
        let nhtriactc = nhtriactc_acc.min(u64::MAX as u128) as u64;
        let naso      = naso_acc.min(u64::MAX as u128) as u64;

        (ntriactc, nhtriactc, naso, edge_count, nc)
    }

    pub fn graph_topo_indices55_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NNONATC (S-Nonacosic vertex sum = Σ_v S(v)^29).
        //
        //    NNONATC(G) = Σ_v S(v)^29  (exact u128→u64; S-Nonacosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NOCTATC=Σ S²⁸ (topo54) → NNONATC=Σ S²⁹ (topo55)
        //    NNONATC = n·S^29 for S-regular.
        //    Overflow: S^29 ≤ 16129^29 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^29 = s^16 × s^8 × s^4 × s.

        let mut nnonatc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s29 = s16.saturating_mul(s8).saturating_mul(s4).saturating_mul(s);
            nnonatc_acc = nnonatc_acc.saturating_add(s29);
        }
        let nnonatc = nnonatc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHNONATC (S-Octacosic edge-sum) and NZSO (S-Hexatetracontyl Sombor).
        //
        //    NHNONATC(G) = Σ_{uv∈E} (S_u+S_v)^28  (exact u128→u64; S-Octacosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHOCTATC=Σ(S+S)²⁷ (topo54)
        //      → NHNONATC=Σ(S+S)²⁸ (topo55)
        //    NHNONATC = |E|·(2S)^28 = 268435456|E|·S^28 for S-regular.
        //    Overflow per edge: (2×16129)^28 → saturating u128 accumulator.
        //    Implementation: ss^28 = ss^16 × ss^8 × ss^4.
        //
        //    NZSO(G) = Σ_{uv∈E} (S_u²+S_v²)^23  (exact u128→u64; S-Hexatetracontyl Sombor α=46)
        //    S-variant generalised Sombor SO^α with α=46: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53),
        //    NYSO(α=44,topo54), NZSO(α=46,topo55).
        //    NZSO = |E|·(2S²)^23 = 8388608|E|·S^46 for S-regular.
        //    Overflow per edge: (2×16129²)^23 → saturating u128 accumulator.
        //    Implementation: s2s^23 = s2s^16 × s2s^4 × s2s^2 × s2s.

        let mut nhnonatc_acc: u128 = 0;
        let mut nzso_acc:     u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHNONATC: (S_a + S_b)^28 = ss^16 × ss^8 × ss^4
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss28   = ss16.saturating_mul(ss8).saturating_mul(ss4);
                    nhnonatc_acc = nhnonatc_acc.saturating_add(ss28);
                    // NZSO: (S_a² + S_b²)^23 = s2s^16 × s2s^4 × s2s^2 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s23  = s2s16.saturating_mul(s2s4).saturating_mul(s2s2).saturating_mul(s2s);
                    nzso_acc = nzso_acc.saturating_add(s2s23);
                }
            }
        }
        let nhnonatc = nhnonatc_acc.min(u64::MAX as u128) as u64;
        let nzso     = nzso_acc.min(u64::MAX as u128) as u64;

        (nnonatc, nhnonatc, nzso, edge_count, nc)
    }

    pub fn graph_topo_indices53_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NHEPTATC (S-Heptacosic vertex sum = Σ_v S(v)^27).
        //
        //    NHEPTATC(G) = Σ_v S(v)^27  (exact u128→u64; S-Heptacosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NHEXATC=Σ S²⁶ (topo52) → NHEPTATC=Σ S²⁷ (topo53)
        //    NHEPTATC = n·S^27 for S-regular.
        //    Overflow: S^27 ≤ 16129^27 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^27 = s^16 × s^8 × s^2 × s.

        let mut nheptatc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s27 = s16.saturating_mul(s8).saturating_mul(s2).saturating_mul(s);
            nheptatc_acc = nheptatc_acc.saturating_add(s27);
        }
        let nheptatc = nheptatc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHHEPTATC (S-Hexacosic edge-sum) and NXSO (S-Dotetracontyl Sombor).
        //
        //    NHHEPTATC(G) = Σ_{uv∈E} (S_u+S_v)^26  (exact u128→u64; S-Hexacosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHHEXATC=Σ(S+S)²⁵ (topo52)
        //      → NHHEPTATC=Σ(S+S)²⁶ (topo53)
        //    NHHEPTATC = |E|·(2S)^26 = 67108864|E|·S^26 for S-regular.
        //    Overflow per edge: (2×16129)^26 → saturating u128 accumulator.
        //    Implementation: ss^26 = ss^16 × ss^8 × ss^2.
        //
        //    NXSO(G) = Σ_{uv∈E} (S_u²+S_v²)^21  (exact u128→u64; S-Dotetracontyl Sombor α=42)
        //    S-variant generalised Sombor SO^α with α=42: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52), NXSO(α=42,topo53).
        //    NXSO = |E|·(2S²)^21 = 2097152|E|·S^42 for S-regular.
        //    Overflow per edge: (2×16129²)^21 → saturating u128 accumulator.
        //    Implementation: s2s^21 = s2s^16 × s2s^4 × s2s.

        let mut nhheptatc_acc: u128 = 0;
        let mut nxso_acc:      u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHHEPTATC: (S_a + S_b)^26 = ss^16 × ss^8 × ss^2
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss26   = ss16.saturating_mul(ss8).saturating_mul(ss2);
                    nhheptatc_acc = nhheptatc_acc.saturating_add(ss26);
                    // NXSO: (S_a² + S_b²)^21 = s2s^16 × s2s^4 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s21  = s2s16.saturating_mul(s2s4).saturating_mul(s2s);
                    nxso_acc = nxso_acc.saturating_add(s2s21);
                }
            }
        }
        let nhheptatc = nhheptatc_acc.min(u64::MAX as u128) as u64;
        let nxso      = nxso_acc.min(u64::MAX as u128) as u64;

        (nheptatc, nhheptatc, nxso, edge_count, nc)
    }

    pub fn graph_topo_indices52_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NHEXATC (S-Hexacosic vertex sum = Σ_v S(v)^26).
        //
        //    NHEXATC(G) = Σ_v S(v)^26  (exact u128→u64; S-Hexacosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NPENTTC=Σ S²⁵ (topo51) → NHEXATC=Σ S²⁶ (topo52)
        //    NHEXATC = n·S^26 for S-regular.
        //    Overflow: S^26 ≤ 16129^26 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^26 = s^16 × s^8 × s^2.

        let mut nhexatc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s26 = s16.saturating_mul(s8).saturating_mul(s2);
            nhexatc_acc = nhexatc_acc.saturating_add(s26);
        }
        let nhexatc = nhexatc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHHEXATC (S-Pentacosic edge-sum) and NVSO (S-Tetracontyl Sombor).
        //
        //    NHHEXATC(G) = Σ_{uv∈E} (S_u+S_v)^25  (exact u128→u64; S-Pentacosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHPENTTC=Σ(S+S)²⁴ (topo51)
        //      → NHHEXATC=Σ(S+S)²⁵ (topo52)
        //    NHHEXATC = |E|·(2S)^25 = 33554432|E|·S^25 for S-regular.
        //    Overflow per edge: (2×16129)^25 → saturating u128 accumulator.
        //    Implementation: ss^25 = ss^16 × ss^8 × ss.
        //
        //    NVSO(G) = Σ_{uv∈E} (S_u²+S_v²)^20  (exact u128→u64; S-Tetracontyl Sombor α=40)
        //    S-variant generalised Sombor SO^α with α=40: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51), NVSO(α=40,topo52).
        //    NVSO = |E|·(2S²)^20 = 1048576|E|·S^40 for S-regular.
        //    Overflow per edge: (2×16129²)^20 → saturating u128 accumulator.
        //    Implementation: s2s^20 = s2s^16 × s2s^4.

        let mut nhhexatc_acc: u128 = 0;
        let mut nvso_acc:     u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHHEXATC: (S_a + S_b)^25 = ss^16 × ss^8 × ss
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss25   = ss16.saturating_mul(ss8).saturating_mul(ss);
                    nhhexatc_acc = nhhexatc_acc.saturating_add(ss25);
                    // NVSO: (S_a² + S_b²)^20 = s2s^16 × s2s^4
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s20  = s2s16.saturating_mul(s2s4);
                    nvso_acc = nvso_acc.saturating_add(s2s20);
                }
            }
        }
        let nhhexatc = nhhexatc_acc.min(u64::MAX as u128) as u64;
        let nvso     = nvso_acc.min(u64::MAX as u128) as u64;

        (nhexatc, nhhexatc, nvso, edge_count, nc)
    }

    pub fn graph_topo_indices51_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NPENTTC (S-Pentacosic vertex sum = Σ_v S(v)^25).
        //
        //    NPENTTC(G) = Σ_v S(v)^25  (exact u128→u64; S-Pentacosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NTETRTC=Σ S²⁴ (topo50) → NPENTTC=Σ S²⁵ (topo51)
        //    NPENTTC = n·S^25 for S-regular.
        //    Overflow: S^25 ≤ 16129^25 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^25 = s^16 × s^8 × s.

        let mut npenttc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s25 = s16.saturating_mul(s8).saturating_mul(s);
            npenttc_acc = npenttc_acc.saturating_add(s25);
        }
        let npenttc = npenttc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHPENTTC (S-Tetracosic edge-sum) and NUSO (S-Octatriacontyl Sombor).
        //
        //    NHPENTTC(G) = Σ_{uv∈E} (S_u+S_v)^24  (exact u128→u64; S-Tetracosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHTETRTC=Σ(S+S)²³ (topo50)
        //      → NHPENTTC=Σ(S+S)²⁴ (topo51)
        //    NHPENTTC = |E|·(2S)^24 = 16777216|E|·S^24 for S-regular.
        //    Overflow per edge: (2×16129)^24 → saturating u128 accumulator.
        //    Implementation: ss^24 = ss^16 × ss^8.
        //
        //    NUSO(G) = Σ_{uv∈E} (S_u²+S_v²)^19  (exact u128→u64; S-Octatriacontyl Sombor α=38)
        //    S-variant generalised Sombor SO^α with α=38: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49),
        //    NSSO(α=36,topo50), NUSO(α=38,topo51).
        //    (T skipped: NTSO already used for α=10)
        //    NUSO = |E|·(2S²)^19 = 524288|E|·S^38 for S-regular.
        //    Overflow per edge: (2×16129²)^19 → saturating u128 accumulator.
        //    Implementation: s2s^19 = s2s^16 × s2s^2 × s2s.

        let mut nhpenttc_acc: u128 = 0;
        let mut nuso_acc:     u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHPENTTC: (S_a + S_b)^24 = ss^16 × ss^8
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss24   = ss16.saturating_mul(ss8);
                    nhpenttc_acc = nhpenttc_acc.saturating_add(ss24);
                    // NUSO: (S_a² + S_b²)^19 = s2s^16 × s2s^2 × s2s
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s19  = s2s16.saturating_mul(s2s2).saturating_mul(s2s);
                    nuso_acc = nuso_acc.saturating_add(s2s19);
                }
            }
        }
        let nhpenttc = nhpenttc_acc.min(u64::MAX as u128) as u64;
        let nuso     = nuso_acc.min(u64::MAX as u128) as u64;

        (npenttc, nhpenttc, nuso, edge_count, nc)
    }

    pub fn graph_topo_indices50_inner(&self) -> (u64, u64, u64, usize, usize) {
        // 1. Compact node index.
        let mut slot_to_ci = [usize::MAX; MAX_NODES];
        let mut nc = 0usize;
        for i in 0..MAX_NODES {
            if self.nodes[i].is_some() {
                slot_to_ci[i] = nc;
                nc += 1;
            }
        }
        if nc == 0 { return (0, 0, 0, 0, 0); }

        // 2. Undirected adjacency bitmasks + edge count.
        let mut adj        = [0u128; MAX_NODES];
        let mut edge_count = 0usize;
        for ei in 0..MAX_EDGES {
            let edge = match self.edges[ei] { Some(e) => e, None => continue };
            let f_sl = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
            let t_sl = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
            let f_ci = slot_to_ci[f_sl];
            let t_ci = slot_to_ci[t_sl];
            if f_ci == usize::MAX || t_ci == usize::MAX || f_ci == t_ci { continue; }
            if (adj[f_ci] >> t_ci) & 1 == 0 {
                adj[f_ci] |= 1u128 << t_ci;
                adj[t_ci] |= 1u128 << f_ci;
                edge_count += 1;
            }
        }

        // 3. Degree array.
        let mut deg = [0u64; MAX_NODES];
        for ci in 0..nc { deg[ci] = adj[ci].count_ones() as u64; }

        // 4. Neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w).
        let mut sv = [0u64; MAX_NODES];
        for ci in 0..nc {
            let mut bits = adj[ci];
            while bits != 0 {
                let nb = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                sv[ci] += deg[nb];
            }
        }

        // 5. Vertex scan: NTETRTC (S-Tetracosic vertex sum = Σ_v S(v)^24).
        //
        //    NTETRTC(G) = Σ_v S(v)^24  (exact u128→u64; S-Tetracosic vertex sum)
        //
        //    Extends the S-power-vertex series:
        //      NM₁=Σ S² (topo18) → ... → NTRICTC=Σ S²³ (topo49) → NTETRTC=Σ S²⁴ (topo50)
        //    NTETRTC = n·S^24 for S-regular.
        //    Overflow: S^24 ≤ 16129^24 → saturating u128 accumulator, clamp to u64::MAX.
        //    Implementation: s^24 = s^16 × s^8.

        let mut ntetrtc_acc: u128 = 0;
        for ci in 0..nc {
            let s   = sv[ci] as u128;
            let s2  = s * s;
            let s4  = s2 * s2;
            let s8  = s4.saturating_mul(s4);
            let s16 = s8.saturating_mul(s8);
            let s24 = s16.saturating_mul(s8);
            ntetrtc_acc = ntetrtc_acc.saturating_add(s24);
        }
        let ntetrtc = ntetrtc_acc.min(u64::MAX as u128) as u64;

        // 6. Edge scan (a < b): NHTETRTC (S-Tricosic edge-sum) and NSSO (S-Hexatriacontyl Sombor).
        //
        //    NHTETRTC(G) = Σ_{uv∈E} (S_u+S_v)^23  (exact u128→u64; S-Tricosic edge-sum)
        //    Extends the S-power-edge series:
        //      NHM1=Σ(S+S)² (topo23) → ... → NHTRICTC=Σ(S+S)²² (topo49)
        //      → NHTETRTC=Σ(S+S)²³ (topo50)
        //    NHTETRTC = |E|·(2S)^23 = 8388608|E|·S^23 for S-regular.
        //    Overflow per edge: (2×16129)^23 → saturating u128 accumulator.
        //    Implementation: ss^23 = ss^16 × ss^4 × ss^2 × ss.
        //
        //    NSSO(G) = Σ_{uv∈E} (S_u²+S_v²)^18  (exact u128→u64; S-Hexatriacontyl Sombor α=36)
        //    S-variant generalised Sombor SO^α with α=36: exact integer (no isqrt).
        //    NSO(α=1,topo21), NCSO(α=3,topo33), NFSO(α=4,topo34),
        //    NHSO(α=6,topo35), NOSO(α=8,topo36), NTSO(α=10,topo37),
        //    NDSO(α=12,topo38), NESO(α=14,topo39), NGSO(α=16,topo40),
        //    NIOSO(α=18,topo41), NJSO(α=20,topo42), NKSO(α=22,topo43),
        //    NLSO(α=24,topo44), NMSO(α=26,topo45), NNSO(α=28,topo46),
        //    NPSO(α=30,topo47), NQSO(α=32,topo48), NRSO(α=34,topo49), NSSO(α=36,topo50).
        //    (S follows R; S not taken as middle letter in prior sequence)
        //    NSSO = |E|·(2S²)^18 = 262144|E|·S^36 for S-regular.
        //    Overflow per edge: (2×16129²)^18 → saturating u128 accumulator.
        //    Implementation: s2s^18 = s2s^16 × s2s^2.

        let mut nhtetrtc_acc: u128 = 0;
        let mut nsso_acc:     u128 = 0;
        for a in 0..nc {
            let sa  = sv[a] as u128;
            let mut bits = adj[a];
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if b > a {
                    let sb     = sv[b] as u128;
                    // NHTETRTC: (S_a + S_b)^23 = ss^16 × ss^4 × ss^2 × ss
                    let ss     = sa + sb;
                    let ss2    = ss * ss;
                    let ss4    = ss2 * ss2;
                    let ss8    = ss4.saturating_mul(ss4);
                    let ss16   = ss8.saturating_mul(ss8);
                    let ss23   = ss16.saturating_mul(ss4).saturating_mul(ss2).saturating_mul(ss);
                    nhtetrtc_acc = nhtetrtc_acc.saturating_add(ss23);
                    // NSSO: (S_a² + S_b²)^18 = s2s^16 × s2s^2
                    let s2s    = sa * sa + sb * sb;
                    let s2s2   = s2s * s2s;
                    let s2s4   = s2s2.saturating_mul(s2s2);
                    let s2s8   = s2s4.saturating_mul(s2s4);
                    let s2s16  = s2s8.saturating_mul(s2s8);
                    let s2s18  = s2s16.saturating_mul(s2s2);
                    nsso_acc = nsso_acc.saturating_add(s2s18);
                }
            }
        }
        let nhtetrtc = nhtetrtc_acc.min(u64::MAX as u128) as u64;
        let nsso     = nsso_acc.min(u64::MAX as u128) as u64;

        (ntetrtc, nhtetrtc, nsso, edge_count, nc)
    }
}

// ── Vertex-connectivity helper: max vertex-disjoint paths via node-split flow ──
//
// Builds the standard node-split network for the undirected graph given by
// `adj_und[ci]` bitmasks (compact-index space, n nodes).  Virtual node
// numbering: 2*ci = ci_in, 2*ci+1 = ci_out.  Internal edges (ci_in→ci_out,
// cap=1) are added for all non-terminal nodes.  Cross-edges (u_out→v_in and
// v_out→u_in, cap=INF) are added for every undirected edge {u,v}.  Runs
// Edmonds-Karp BFS augmentation; each augmenting path carries exactly 1 unit
// of flow (unit-capacity internal edges).  The final max-flow equals the
// maximum number of internally vertex-disjoint s-t paths (Menger 1927).
//
// Edge pairs are always added in order (forward at index 2k, backward at 2k+1)
// so the backward edge of any edge `ei` is always `ei ^ 1`.
fn vertex_conn_maxflow(
    adj_und: &[u128; MAX_NODES],
    n:       usize,
    s_ci:    usize,
    t_ci:    usize,
) -> u32 {
    const ME:  usize = 2560; // edge-list capacity (128 internal + 4×512 cross ≤ 2300)
    const MV:  usize = 256;  // virtual-node capacity (2 × MAX_NODES)
    const INF: u8    = 127;  // "infinite" cap; max flow ≤ n-2 ≤ 126 < 127

    let mut ef = [0u8; ME]; // edge from (virtual node)
    let mut et = [0u8; ME]; // edge to   (virtual node)
    let mut ec = [0u8; ME]; // residual capacity
    let mut ne = 0usize;    // edge count (always even: pairs)

    // Internal edges (cap=1) for non-terminal nodes
    for ci in 0..n {
        if ci == s_ci || ci == t_ci { continue; }
        if ne + 2 > ME { break; }
        ef[ne] = (2 * ci) as u8;     et[ne] = (2 * ci + 1) as u8; ec[ne] = 1;   ne += 1;
        ef[ne] = (2 * ci + 1) as u8; et[ne] = (2 * ci) as u8;     ec[ne] = 0;   ne += 1;
    }
    // Cross-edges from undirected adjacency (upper-triangle to avoid duplicates)
    for u_ci in 0..n {
        let u_adj = adj_und[u_ci];
        for v_ci in (u_ci + 1)..n {
            if (u_adj >> v_ci) & 1 == 0 { continue; }
            if ne + 4 > ME { break; }
            ef[ne] = (2 * u_ci + 1) as u8; et[ne] = (2 * v_ci) as u8;     ec[ne] = INF; ne += 1;
            ef[ne] = (2 * v_ci) as u8;     et[ne] = (2 * u_ci + 1) as u8; ec[ne] = 0;   ne += 1;
            ef[ne] = (2 * v_ci + 1) as u8; et[ne] = (2 * u_ci) as u8;     ec[ne] = INF; ne += 1;
            ef[ne] = (2 * u_ci) as u8;     et[ne] = (2 * v_ci + 1) as u8; ec[ne] = 0;   ne += 1;
        }
    }

    let src_vn = (2 * s_ci + 1) as u8; // s_out
    let snk_vn = (2 * t_ci) as u8;     // t_in
    let mut flow = 0u32;

    loop {
        // BFS to find an augmenting path
        let mut prev_e = [u16::MAX; MV];
        let mut vis    = [false;    MV];
        let mut q      = [0u8;      MV];
        let mut qh     = 0usize;
        let mut qt     = 0usize;

        vis[src_vn as usize] = true;
        q[qt] = src_vn;
        qt += 1;

        'bfs: while qh < qt {
            let u = q[qh]; qh += 1;
            for ei in 0..ne {
                if ef[ei] != u || ec[ei] == 0 { continue; }
                let v = et[ei];
                if vis[v as usize] { continue; }
                vis[v as usize]    = true;
                prev_e[v as usize] = ei as u16;
                if v == snk_vn { break 'bfs; }
                if qt < MV { q[qt] = v; qt += 1; }
            }
        }

        if !vis[snk_vn as usize] { break; }

        // Augment by 1 unit — backward edge always at ei ^ 1 (pairs)
        let mut cur = snk_vn;
        while cur != src_vn {
            let ei = prev_e[cur as usize] as usize;
            ec[ei]     -= 1;
            ec[ei ^ 1] += 1;
            cur         = ef[ei];
        }
        flow += 1;
    }

    flow
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

/// V2.51: Snapshot the current node state into the structural diff ring.
/// Pushes a `GraphDiffKind::NodeCheckpoint` entry capturing the node's key,
/// signal_count, lifecycle, and edge_out_count.  Graph epoch is NOT bumped.
/// Returns a `NodeProcSummary` of the captured state, or `NodeNotFound`.
pub fn node_checkpoint(vec: VectorAddress) -> Result<NodeProcSummary, RuntimeError> {
    RUNTIME.lock().node_checkpoint_inner(vec)
}

/// V2.55: Set a u32 attribute on the node at `vector`.
/// The attribute slot is keyed by NodeId and stores arbitrary scalars
/// (palette colors, flags, counters).  Idempotent: re-setting overwrites.
/// Returns `Err(NodeNotFound)` if no node is at `vector`,
/// `Err(PropTableFull)` if all MAX_NODE_PROPS_U32 slots are exhausted.
pub fn node_attr_set(vec: VectorAddress, val: u32) -> Result<(), RuntimeError> {
    RUNTIME.lock().node_attr_set_inner(vec, val)
}

/// V2.55: Get the u32 attribute stored on the node at `vector`.
/// Returns `None` when the node does not exist or has no attribute set.
pub fn node_attr_get(vec: VectorAddress) -> Option<u32> {
    RUNTIME.lock().node_attr_get_inner(vec)
}

/// V2.55: Register a u32 attribute directly by NodeId (boot-time use).
/// Returns false when the table is full (MAX_NODE_PROPS_U32 slots).
pub fn register_node_prop_u32(node_id: NodeId, val: u32) -> bool {
    RUNTIME.lock().register_node_prop_u32(node_id, val)
}

/// V2.58: List all nodes that have a u32 attribute set.
/// Returns (vectors, values, count) — at most N entries, in table order.
pub fn node_attr_list<const N: usize>(
    out_vec: &mut [VectorAddress; N],
    out_val: &mut [u32; N],
) -> usize {
    RUNTIME.lock().node_attr_list_inner(out_vec, out_val)
}

/// V2.59: Graph density = E / (N*(N-1)) for a directed graph.
/// Returns (density_ppm, node_count, edge_count) where density_ppm is in
/// parts-per-million (0 = empty/undefined, 1_000_000 = complete graph).
pub fn graph_density() -> (u32, usize, usize) {
    RUNTIME.lock().graph_density_inner()
}

/// V2.61: Global graph clustering coefficient (Watts-Strogatz style).
/// Returns (clustering_ppm, node_count) where clustering_ppm is in
/// parts-per-million (0 = no triplets/undefined, 1_000_000 = fully clustered).
pub fn graph_clustering() -> (u32, usize) {
    RUNTIME.lock().graph_clustering_inner()
}

/// V2.63: Global graph transitivity (3 × triangles / open_triplets).
pub fn graph_transitivity() -> (u32, u64, u64, usize) {
    RUNTIME.lock().graph_transitivity_inner()
}

/// V2.64: Graph k-core decomposition — coreness of each node.
/// Returns (vecs, coreness, n, max_coreness) sorted by coreness descending.
pub fn graph_kcore<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, u8) {
    RUNTIME.lock().graph_kcore_inner::<N>()
}

/// V2.65: Degree assortativity coefficient (Newman 2002).
/// Returns (assortativity_ppm, edge_count, node_count).
/// +1_000_000 = assortative; −1_000_000 = disassortative; 0 = uncorrelated/undefined.
pub fn graph_assortativity() -> (i32, usize, usize) {
    RUNTIME.lock().graph_assortativity_inner()
}

/// V2.66: Graph reciprocity — fraction of directed edges that are mutual.
/// Returns (reciprocity_ppm, mutual_edges, total_edges).
/// 1_000_000 = fully reciprocal; 0 = no mutual edges or no edges.
pub fn graph_reciprocity() -> (u32, usize, usize) {
    RUNTIME.lock().graph_reciprocity_inner()
}

/// V2.67: Graph modularity — quality of the LPA community partition (Newman–Girvan Q).
///
/// Runs LPA community detection (same algorithm as `graph_community`) then
/// evaluates Q = Σ_c [ L_c/m − (d_c/(2m))² ] over the resulting partition.
/// Directed edges treated as undirected; self-loops excluded.
///
/// Returns (modularity_ppm, community_count, undirected_edge_count, node_count).
///   modularity_ppm ∈ [0, 1_000_000] for LPA partitions of connected components.
///   0 → single community or no edges.
pub fn graph_modularity() -> (i32, usize, usize, usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_modularity_inner(&snap)
}

/// V2.68: Graph rich-club coefficient for degree threshold `k`.
///
/// Measures how densely the "rich" nodes (undirected degree > k) are interconnected.
///   ρ(k) = E_{>k} / [N_{>k} × (N_{>k} − 1) / 2]
/// Directed edges treated as undirected; self-loops excluded.
///
/// Returns (rich_club_ppm, rich_node_count, edges_among_rich).
///   1_000_000 → rich nodes form a clique; 0 → no rich nodes or no edges among them.
pub fn graph_rich_club(k: u8) -> (u32, usize, usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_rich_club_inner(&snap, k)
}

/// V2.69: Girth of the directed graph — length of the shortest directed cycle.
///
/// Returns (girth, is_acyclic, node_count).
///   girth = u32::MAX → acyclic directed graph (DAG); is_acyclic = true.
///   girth = 1        → self-loop present.
///   girth = k        → shortest directed k-cycle.
pub fn graph_girth() -> (u32, bool, usize) {
    RUNTIME.lock().graph_girth_inner()
}

/// V2.70: Wiener index of the directed graph.
///
/// W(G) = Σ_{u≠v, d(u,v)<∞} d(u,v)  (BFS, unweighted).
///
/// Returns (wiener_index, reachable_pairs, node_count).
///   wiener_index    = sum of all finite directed pairwise distances
///   reachable_pairs = ordered pairs (u,v) with u≠v and a directed path
///   node_count      = live nodes
pub fn graph_wiener() -> (u64, usize, usize) {
    RUNTIME.lock().graph_wiener_inner()
}

/// V2.60: List all nodes that have a u8 attribute set.
/// Returns the number of entries written into `out_vec` / `out_val` (≤ N).
pub fn node_attr_list_u8<const N: usize>(
    out_vec: &mut [VectorAddress; N],
    out_val: &mut [u8; N],
) -> usize {
    RUNTIME.lock().node_attr_list_u8_inner(out_vec, out_val)
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

/// V2.40: Outgoing closeness centrality for all live nodes (directed BFS).
///
/// Returns `(vecs, cc, total)`:
/// - `vecs[0..total]` — live node vectors, descending closeness order.
/// - `cc[0..total]`   — closeness score × 1_000_000 per node.
/// - `total`          — number of live nodes packed into the output arrays.
///
/// For each node v: CC[v] = r_v × 1_000_000 / Σ d(v,u) over reachable u≠v.
/// Isolated nodes (r_v = 0) → CC = 0.
/// Algorithm: one BFS per source, O(V × (V+E)).
/// OS analogy: `ping` RTT average — which kernel service can reach all others
/// in the fewest directed hops?
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_closeness<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    RUNTIME.lock().graph_closeness_inner()
}

/// V2.71: Harmonic centrality for all live nodes.
///
/// HC[v] = Σ_{u≠v, d(v,u)<∞} 1_000_000/d(v,u)  (sum of reciprocal BFS distances).
///
/// Handles disconnected graphs naturally: unreachable pairs contribute 0.
/// Isolated nodes → HC = 0.  Algorithm: one BFS per source, O(V × (V+E)).
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_harmonic<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    RUNTIME.lock().graph_harmonic_inner()
}

/// V2.72: Graph peripheral nodes — nodes with eccentricity equal to the diameter.
///
/// Returns `(vecs, ecc, peripheral_count, node_count, diameter)`:
/// - `vecs[0..peripheral_count]`  — peripheral node vectors, sorted ascending by address.
/// - `ecc[0..peripheral_count]`   — eccentricity per peripheral node (all == diameter).
/// - `peripheral_count`           — number of peripheral nodes (capped at N).
/// - `node_count`                 — total number of live nodes.
/// - `diameter`                   — graph diameter (0 if all nodes isolated).
///
/// Peripheral nodes have the maximum eccentricity — they are the boundary of the graph.
/// All-isolated graph → diameter=0, peripheral_count=0.
/// If radius == diameter every node is simultaneously centre and peripheral.
/// Algorithm: one BFS per source node, O(V × (V+E)).
pub fn graph_peripheral<const N: usize>() -> ([VectorAddress; N], [u32; N], usize, usize, u32) {
    RUNTIME.lock().graph_peripheral_inner()
}

/// V2.73: Graph center nodes — nodes whose eccentricity equals the graph radius.
///
/// Returns `(vecs, ecc, center_count, node_count, radius)`.
///   vecs[0..center_count]  — centre node vectors, sorted ascending by VectorAddress.
///   ecc[0..center_count]   — eccentricity (all equal to radius).
///   center_count           — number of centre nodes (capped at N).
///   node_count             — total live nodes.
///   radius                 — min nonzero eccentricity; 0 if all nodes are isolated.
/// Algorithm: one BFS per source node, O(V × (V+E)).
pub fn graph_center<const N: usize>() -> ([VectorAddress; N], [u32; N], usize, usize, u32) {
    RUNTIME.lock().graph_center_inner()
}

/// V2.74: Global efficiency of the directed graph.
///
/// E(G) = 1/(n*(n-1)) * Σ_{i≠j, d(i,j)<∞} 1/d(i,j)  (unweighted BFS).
///
/// Returns (efficiency_ppm, pairs_max, node_count).
///   efficiency_ppm = E(G) scaled by 1_000_000 (0 = fully disconnected, 1_000_000 = complete).
///   pairs_max      = n*(n-1), the normalisation denominator (0 when n < 2).
///   node_count     = live nodes.
pub fn graph_global_efficiency() -> (u64, usize, usize) {
    RUNTIME.lock().graph_global_efficiency_inner()
}

/// V2.75: Graph average clustering coefficient (Watts-Strogatz per-node average).
///
/// Returns `(avg_ppm, nodes_computed, node_count)`:
/// - `avg_ppm`       — avg_CC × 1_000_000
/// - `nodes_computed` — nodes with undirected degree ≥ 2 that contributed to the sum
/// - `node_count`    — total alive nodes (denominator of the average)
///
/// Differs from `graph_clustering` (V2.61) which computes the global transitivity ratio.
pub fn graph_avg_clustering() -> (u32, usize, usize) {
    RUNTIME.lock().graph_avg_clustering_inner()
}

/// V2.76: Graph local efficiency (Latora–Marchiori 2001).
///
/// E_loc(G) = (1/n) × Σ_v E(G_v)
///
/// where G_v is the directed subgraph induced by the undirected neighbours of v.
/// Nodes with undirected degree < 2 contribute 0.
///
/// Returns `(eloc_ppm, nodes_computed, node_count)`:
/// - `eloc_ppm`       — E_loc × 1_000_000
/// - `nodes_computed` — nodes with undirected degree ≥ 2
/// - `node_count`     — total alive nodes (denominator)
pub fn graph_local_efficiency() -> (u32, usize, usize) {
    RUNTIME.lock().graph_local_efficiency_inner()
}

/// V2.77: Graph small-world coefficient σ (Humphries–Gurney 2008).
///
/// σ = (CC / CC_rand) / (L / L_rand)
///
/// CC_rand ≈ 2·m / (n·(n−1)) — Erdős–Rényi density baseline.
/// L_rand  ≈ ln(n) / ln(⟨k⟩) — E-R average path baseline.
///
/// Returns `(sigma_ppm, cc_ppm, l_ppm, l_rand_ppm, node_count, m_undir)`:
/// - `sigma_ppm`   — σ × 1_000_000 (0 when σ cannot be computed)
/// - `cc_ppm`      — average clustering coefficient × 1_000_000
/// - `l_ppm`       — average directed path length × 1_000_000
/// - `l_rand_ppm`  — L_rand estimate × 1_000_000
/// - `node_count`  — total alive nodes
/// - `m_undir`     — deduplicated undirected edge count
pub fn graph_small_world() -> (u32, u32, u64, u64, usize, usize) {
    RUNTIME.lock().graph_small_world_inner()
}

/// V2.78: Degree heterogeneity index κ = ⟨k²⟩/⟨k⟩ for scale-free detection.
///
/// κ is the ratio of the second moment to the first moment of the undirected degree
/// distribution.  For a scale-free (power-law) network κ >> ⟨k⟩; for an Erdős–Rényi
/// random graph κ ≈ ⟨k⟩ + 1; for a k-regular graph κ = ⟨k⟩.
///
/// Returns `(kappa_ppm, max_degree, avg_degree_ppm, node_count, m_undir)`:
/// - `kappa_ppm`       — κ × 1_000_000  (0 when no edges exist)
/// - `max_degree`      — maximum undirected degree k_max
/// - `avg_degree_ppm`  — ⟨k⟩ × 1_000_000  (average undirected degree)
/// - `node_count`      — total alive nodes
/// - `m_undir`         — deduplicated undirected edge count
///
/// Heuristic classification (shell output):
///   kappa_ppm > 3 × avg_degree_ppm  →  "likely scale-free"
///   kappa_ppm > 2 × avg_degree_ppm  →  "heterogeneous"
///   otherwise                        →  "homogeneous (regular/random-like)"
pub fn graph_scale_free() -> (u32, u32, u32, usize, usize) {
    RUNTIME.lock().graph_scale_free_inner()
}

/// V2.80: Power-law exponent MLE estimator (Clauset–Newman–Shalizi 2009).
///
/// Estimates the power-law exponent γ̂ from the undirected degree sequence
/// using the maximum-likelihood estimator with k_min = 1 (non-isolated nodes).
///
///   γ̂ = 1 + n_fit × [Σ_{k_i ≥ 1} ln(k_i)]^{-1}
///
/// Returns `(gamma_ppm, n_fit, node_count)`:
/// - `gamma_ppm`  — γ̂ × 1_000_000  (0 = undefined: all k=1 or no non-isolated nodes)
/// - `n_fit`      — nodes in the fit (non-isolated, k ≥ 1)
/// - `node_count` — total alive nodes
///
/// Typical power-law networks: γ ∈ [2, 3] (gamma_ppm ∈ [2_000_000, 3_000_000]).
/// Regular graphs: γ > 1 + n/ln(k)^{-1}; pure stars: γ >> 3.
pub fn graph_power_law() -> (u32, usize, usize) {
    RUNTIME.lock().graph_power_law_inner()
}

/// V2.83: Snapshot of all topology metrics captured at a single point in time.
///
/// Produced by `graph_snapshot_save` and returned by `graph_snapshot_compare`.
/// All ppm fields are ×1_000_000 (parts-per-million integer representation).
#[derive(Copy, Clone)]
pub struct MetricSnapshot {
    /// false only for the initial unset static; true after first `graph_snapshot_save`.
    pub valid: bool,
    /// `graph_epoch` at time of save (bumped by every structural mutation).
    pub epoch: u64,
    /// Total live nodes.
    pub node_count: usize,
    /// Total directed edges.
    pub edge_count: usize,
    /// Graph density × 1_000_000.
    pub density_ppm: u32,
    /// Global transitivity (3×triangles/triplets) × 1_000_000.
    pub trans_ppm: u32,
    /// Average clustering coefficient (Watts-Strogatz) × 1_000_000.
    pub avgcc_ppm: u32,
    /// Global efficiency E(G) × 1_000_000  (u64: same width as `graph_global_efficiency`).
    pub geff_ppm: u64,
    /// Local efficiency E_loc(G) × 1_000_000.
    pub leff_ppm: u32,
    /// Small-world coefficient σ × 1_000_000  (0 = undefined/insufficient connectivity).
    pub sigma_ppm: u32,
    /// Degree heterogeneity index κ × 1_000_000  (0 = no edges).
    pub kappa_ppm: u32,
    /// Power-law exponent γ̂ × 1_000_000  (0 = undefined: all k=1 or no edges).
    pub gamma_ppm: u32,
}

static METRIC_SNAPSHOT: Mutex<MetricSnapshot> = Mutex::new(MetricSnapshot {
    valid:       false,
    epoch:       0,
    node_count:  0,
    edge_count:  0,
    density_ppm: 0,
    trans_ppm:   0,
    avgcc_ppm:   0,
    geff_ppm:    0,
    leff_ppm:    0,
    sigma_ppm:   0,
    kappa_ppm:   0,
    gamma_ppm:   0,
});

/// V2.83: Save all current topology metrics into the persistent snapshot slot.
///
/// Call this before a topology change (e.g. after boot or a stable operation
/// point) to establish a baseline; then use `graph_snapshot_compare` to diff
/// the baseline against the live metrics at any later time.
///
/// All metrics are computed inside a single RUNTIME lock hold (consistent epoch).
/// Returns the `graph_epoch` at which the snapshot was captured.
pub fn graph_snapshot_save() -> u64 {
    let snap = RUNTIME.lock().graph_snapshot_inner();
    let epoch = snap.epoch;
    *METRIC_SNAPSHOT.lock() = snap;
    epoch
}

/// V2.83: Compare the saved metric snapshot against the current live metrics.
///
/// Returns `(saved, current)`:
/// - `saved`   — the snapshot from the last `graph_snapshot_save` call;
///              `saved.valid == false` if no snapshot has been saved yet.
/// - `current` — metrics computed right now (always `valid == true`).
///
/// Both are computed / retrieved under separate lock acquisitions, so they
/// may differ by a race if the graph changes concurrently — but for the
/// interactive shell use-case this is fine.
pub fn graph_snapshot_compare() -> (MetricSnapshot, MetricSnapshot) {
    let saved   = *METRIC_SNAPSHOT.lock();
    let current = RUNTIME.lock().graph_snapshot_inner();
    (saved, current)
}

/// V2.84: Link prediction metrics for node pair (u, v) — Common Neighbors, Jaccard,
/// Adamic-Adar, and Resource Allocation.
///
/// Returns `(cn, jaccard_ppm, aa_ppm, ra_ppm, node_count)`:
/// - `cn`           — common-neighbour count (integer)
/// - `jaccard_ppm`  — Jaccard coefficient × 1_000_000; 0 when union is empty
/// - `aa_ppm`       — Adamic-Adar index × 1_000_000; skips common neighbours with deg ≤ 1
/// - `ra_ppm`       — Resource Allocation index × 1_000_000
/// - `node_count`   — total live nodes
///
/// Score interpretation: higher → stronger prediction of a missing edge u→v.
/// Neighbourhood is undirected; u and v are excluded from each other's sets.
/// Returns all zeros when u == v or either vector is not registered.
///
/// OS analogy: `predict` in iproute2 or LLDP neighbor-table projection —
/// which kernel subsystems are likely to form a new dependency edge?
pub fn graph_link_predict(u: VectorAddress, v: VectorAddress) -> (usize, u32, u32, u32, usize) {
    RUNTIME.lock().graph_link_predict_inner(u, v)
}

/// V2.41: Graph eccentricity and graph radius / diameter.
///
/// Returns `(vecs, ecc, total, radius, diameter)`.
///   ecc[v]   = max shortest-path distance from v to any reachable node (0 if isolated).
///   radius   = min(ecc[v]) for non-isolated nodes (0 if all isolated).
///   diameter = max(ecc[v]) (0 if all isolated).
/// Output sorted ascending so centre nodes (ecc == radius) appear first.
/// Algorithm: one BFS per source node, O(V × (V+E)).
pub fn graph_eccentricity<const N: usize>() -> ([VectorAddress; N], [u32; N], usize, u32, u32) {
    RUNTIME.lock().graph_eccentricity_inner()
}

/// V2.42: Incoming Katz centrality for all live nodes (iterative power series).
///
/// Returns `(vecs, katz, total)`:
/// - `vecs[0..total]`  — live node vectors, descending Katz score order.
/// - `katz[0..total]`  — Katz score × 1_000_000 per node (capped at u32::MAX).
/// - `total`           — number of live nodes packed into the output arrays.
///
/// KC[v] = Σ_{k=1}^{∞} (1/8)^k × (directed walks of length k ending at v).
/// Isolated nodes → KC = 0.  Self-loops contribute α/(1-α) = 1/7 ≈ 142_857 × 10⁻⁶.
/// Algorithm: 20 fixed-point iterations, O(K × V × E).
/// OS analogy: `netstat -s` — which kernel service receives the most indirect
/// signal traffic summed across all path lengths?
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_katz<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_katz_inner::<N>(&snap)
}

/// V2.43: PageRank centrality for all live nodes (random-walk stationary distribution).
///
/// Returns `(vecs, pr, total)`:
/// - `vecs[0..total]`  — live node vectors, descending PageRank order.
/// - `pr[0..total]`    — PageRank score × 1_000_000 per node (capped at u32::MAX).
/// - `total`           — number of live nodes packed into the output arrays.
///
/// Classical PageRank: PR[v] = (1-d)×SCALE + d × Σ_{u→v} PR[u]/outdeg(u),  d=0.85.
/// Dangling nodes (out-degree = 0) absorb rank (no redistribution — they are
/// signal drains, not relays).  20 fixed-point iterations; O(K × V × E).
///
/// Score interpretation (×10⁻⁶):
///   PR ≥ 1_000_000   → authority (disproportionate random-walk traffic)
///   300_000 < PR < 1M → relay    (above-floor link contribution)
///   PR ≤ 300_000      → sink     (≈ teleportation floor only, few inbound links)
///
/// OS analogy: `top` sorted by incoming-signal weight — which kernel nodes
/// dominate the random walk over the live graph topology?
pub fn graph_pagerank<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_pagerank_inner::<N>(&snap)
}

/// V2.44: HITS hub and authority scores for all live nodes.
///
/// Returns `(vecs, hub, auth, total)`:
/// - `vecs[0..total]`  — node vectors, sorted descending by authority score.
/// - `hub[0..total]`   — hub score × 1_000_000 (how well node points to authorities).
/// - `auth[0..total]`  — authority score × 1_000_000 (how well node is cited by hubs).
/// - `total`           — number of live nodes packed into the arrays.
///
/// Algorithm: Kleinberg's HITS — 20 iterations of simultaneous update + L∞ normalisation.
///   new_a[v] = Σ_{u→v} h[u]  ;  new_h[v] = Σ_{v→w} a[w]  ;  then max-normalise both.
/// Isolated nodes converge to hub=0, auth=0.  Cyclic/mutual nodes converge to hub=auth=1M.
///
/// Role interpretation:
///   hub  ≥ 800_000  → top-hub       (excellent pointer to authorities)
///   auth ≥ 800_000  → top-authority  (cited by the best hubs)
///   both ≥ 800_000  → hub+authority  (symmetric role, e.g. nodes in cycles)
///   both < 200_000  → isolated       (no structural role in the bipartite HITS view)
///
/// OS analogy: `vmstat` + `top` bipartite — which kernel nodes are the best
/// signal-forwarders (hub) vs the most-cited destinations (authority)?
pub fn graph_hits<const N: usize>() -> ([VectorAddress; N], [u32; N], [u32; N], usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_hits_inner::<N>(&snap)
}

/// V2.45: Label Propagation community detection over the live kernel graph.
///
/// Treats directed edges as undirected; returns community assignments
/// for all live nodes.  See `RuntimeState::graph_community_inner` for the
/// full algorithm description.
///
/// Returns `(vecs, community_ids, node_count, community_count)`.
/// The arrays `vecs[0..node_count]` and `community_ids[0..node_count]`
/// are sorted so that all members of the same community are contiguous
/// and communities are ordered by size descending (largest = id 0).
pub fn graph_community<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_community_inner::<N>(&snap)
}

/// V2.46: BFS spanning forest over the undirected projection of the live kernel graph.
///
/// Treats every directed edge as undirected so the forest covers all live nodes.
/// Roots are chosen in ascending slot order; each unvisited node starts a new tree.
///
/// Returns `(vecs, parents, depths, node_count, tree_count)`:
/// - `vecs[0..node_count]`    — node vectors in BFS order (tree 0 first).
/// - `parents[0..node_count]` — parent vector per node (same as vecs[i] for roots).
/// - `depths[0..node_count]`  — BFS depth (0 = root).
/// - `node_count`             — total live nodes packed into the arrays.
/// - `tree_count`             — number of BFS trees (= undirected connected components).
///
/// OS analogy: `ip route show` / spanning-tree protocol — the minimal backbone
/// that connects all kernel subsystems without redundant cross-links.
pub fn graph_spanning<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], [u8; N], usize, usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_spanning_inner::<N>(&snap)
}

/// V2.47: Welsh-Powell greedy graph coloring.
///
/// Assigns each live node a color (u8) such that no two directly connected nodes
/// (undirected projection) share the same color.  Nodes are processed in
/// descending total-degree order (Welsh-Powell heuristic), then greedy
/// smallest-available-color is assigned.
///
/// Returns `(vecs, colors, node_count, chromatic_number)`:
/// - `vecs[0..node_count]`   — node vectors in descending total-degree order.
/// - `colors[0..node_count]` — color index (0-based).
/// - `node_count`            — total live nodes.
/// - `chromatic_number`      — number of distinct colors used.
///
/// OS analogy: colors = conflict-free scheduling domains / CPU-affinity groups.
pub fn graph_color<const N: usize>() -> ([VectorAddress; N], [u8; N], [u8; N], usize, u8) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_color_inner::<N>(&snap)
}

/// V2.48: Prim's MST spanning forest over the undirected projection of the live kernel graph.
///
/// Treats every directed edge as undirected with weight `edge.spec.weight`
/// (defaults to 1.0 when edges are registered without an explicit weight).
/// Disconnected components each receive their own MST root (forest).
///
/// Returns `(vecs, parents, weights, node_count, total_mst_w)`:
/// - `vecs[0..node_count]`    — node vectors in Prim visit order.
/// - `parents[0..node_count]` — parent vector (self for component roots).
/// - `weights[0..node_count]` — edge weight to parent × 1000 as u32 (0 for roots).
/// - `node_count`             — total live nodes.
/// - `total_mst_w`            — sum of all MST edge weights × 1000 as u32.
///
/// OS analogy: `ip route show metric` — the minimum-cost set of signal routes
/// that keeps all kernel sub-systems reachable, analogous to a routing table
/// built for minimum total latency/bandwidth cost.
pub fn graph_mst<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_mst_inner::<N>(&snap)
}

/// V2.49: Dijkstra single-source shortest-path tree from `source` over the
/// **directed** live kernel graph.
///
/// Uses directed edges only (unlike MST which treats edges as undirected).
/// Edge weights come from `edge.spec.weight` (default 1.0).
///
/// Returns `(vecs, parents, distances, node_count)`:
/// - `vecs[0..node_count]`      — all live nodes; source appears first.
/// - `parents[0..node_count]`   — parent in shortest-path tree (self for source,
///                                ZERO_VEC for unreachable nodes).
/// - `distances[0..node_count]` — distance from source × 1000 as u32
///                                (`u32::MAX` = unreachable).
/// - `node_count`               — total live nodes.
///
/// OS analogy: `ip route get <dst>` — the minimum-latency directed path from
/// one kernel sub-system to all reachable peers.
pub fn graph_shortest<const N: usize>(
    source: VectorAddress,
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_shortest_inner::<N>(&snap, source)
}

/// V2.50: Maximum network flow between two nodes (Edmonds-Karp BFS Ford-Fulkerson).
///
/// Returns `(vecs, out_flow, in_flow, node_count, max_flow)`:
/// - `vecs[0..node_count]`     — all live nodes; source first, sink second.
/// - `out_flow[0..node_count]` — per-node total outgoing flow × 1000.
/// - `in_flow[0..node_count]`  — per-node total incoming flow × 1000.
/// - `node_count`              — total live nodes.
/// - `max_flow`                — maximum flow × 1000 as u32.
///
/// OS analogy: `tc -s qdisc show` — maximum achievable throughput from one
/// kernel sub-system to another given the edge capacity constraints.
pub fn graph_flow<const N: usize>(
    source: VectorAddress,
    sink:   VectorAddress,
) -> ([VectorAddress; N], [u32; N], [u32; N], usize, u32) {
    let snap = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_flow_inner::<N>(&snap, source, sink)
}

/// V2.52: Random walk simulation over the live kernel graph.
///
/// Simulates at most `steps` (clamped to 256) random walk steps starting
/// from a random live node, sampling outgoing edges proportional to their
/// weight.  Dead-end nodes trigger a teleport to a uniformly random live
/// node.  `seed` is mixed with the graph epoch for varied-but-deterministic
/// output on consecutive calls.
///
/// Returns `(vecs, visits, node_count, actual_steps, stuck_steps)`:
/// - `vecs[0..node_count]`   — nodes sorted by visit count descending.
/// - `visits[0..node_count]` — raw visit count per node.
/// - `node_count`            — total live nodes.
/// - `actual_steps`          — steps that traversed an edge.
/// - `stuck_steps`           — steps that teleported (dead ends).
///
/// Invariant (non-empty graph, steps > 0):
///   `sum(visits) == 1 + actual_steps + stuck_steps == 1 + min(steps, 256)`
///
/// OS analogy: `strace -e trace=signal` — identifies which kernel
/// subsystems dominate signal traffic under simulated random load.
pub fn graph_sim<const N: usize>(
    steps: u32,
    seed:  u32,
) -> ([VectorAddress; N], [u32; N], usize, u32, u32) {
    let steps = steps.min(256);
    let snap  = RUNTIME.lock().topology_snapshot();
    GraphRuntime::graph_sim_inner::<N>(&snap, steps, seed)
}

/// V2.53: Weighted betweenness centrality for all live nodes (Brandes + Dijkstra).
///
/// Returns `(vecs, wbc, total)`:
/// - `vecs[0..total]` — live node vectors, descending WBC order.
/// - `wbc[0..total]`  — WBC score × 1_000_000 per node (truncated to u32).
/// - `total`          — number of live nodes packed into the output arrays.
///
/// Unlike `graph_centrality` (V2.39, BFS hop-count), uses `edge.spec.weight`
/// so minimum-weight paths are found via Dijkstra.  Uniform-weight graphs
/// produce identical results to `graph_centrality`.
/// Algorithm: O(V² × (V+E)), no heap (O(V²) Dijkstra per source).
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_between<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    RUNTIME.lock().graph_between_inner()
}

/// V2.54: Attractor-set classification of the live kernel graph.
///
/// Returns `(vecs, roles, total, attractor_count)`:
/// - `vecs[0..total]`  — live node vectors, sorted role-ascending (0 then 1 then 2).
/// - `roles[0..total]` — 0=attractor, 1=drain, 2=transient.
/// - `total`           — number of live nodes.
/// - `attractor_count` — count of nodes in bottom SCCs (role=0).
///
/// Role definitions (condensation DAG perspective):
///   0 attractor — bottom SCC: no condensation out-edges; signal/flow cannot escape.
///   1 drain     — SCC has a direct condensation edge to an attractor SCC.
///   2 transient — SCC has out-edges but none lead directly to an attractor SCC.
///
/// O(V+E) — Kosaraju SCC + two edge-scan passes.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_attractor<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize) {
    RUNTIME.lock().graph_attractor_inner()
}

/// V2.85: Articulation points (cut vertices) of the live kernel graph.
///
/// Returns `(art_vecs, art_count, node_count)`:
/// - `art_vecs[0..art_count]` — cut-vertex vectors sorted ascending by as_u64().
/// - `art_count`              — number of articulation points found.
/// - `node_count`             — total live node count.
///
/// An articulation point is a node whose removal increases the number of
/// connected components in the undirected projection of the graph.
/// Equivalently: a node v is a cut vertex iff there exist s, t ≠ v such that
/// every undirected path from s to t passes through v.
///
/// Algorithm: iterative Tarjan disc/low-link DFS, O(V+E).
/// OS analogy: identifying single-point-of-failure kernel subsystems whose
/// removal would partition the dependency graph into disconnected islands.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_articulation<const N: usize>() -> ([VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_articulation_inner()
}

/// V2.86: Find all bridge edges (cut edges) in the undirected projection of
/// the live kernel graph.
///
/// Returns (from_vecs, to_vecs, bridge_count, node_count).
/// Each bridge is reported as a canonicalized pair (smaller.as_u64() first).
/// Sorted ascending by (from.as_u64(), to.as_u64()).
///
/// A bridge is an edge whose removal increases connected components.
/// Algorithm: iterative Tarjan disc/low-link DFS, O(V+E).
/// Parent tracked by edge-index (not slot) to correctly handle anti-parallel
/// directed pairs (A→B + B→A) which are NOT bridges.
///
/// OS analogy: a network link whose failure partitions the routing fabric —
/// analogous to a single uplink between a leaf switch and the core.
/// N controls the output buffer depth (cap at MAX_EDGES = 512).
pub fn graph_bridges<const N: usize>() -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_bridges_inner()
}

/// V2.87: Eulerian path/circuit detection for the live kernel graph.
///
/// Returns `(has_circuit, has_path, start_vec, end_vec, node_count)`.
///
/// `has_circuit`: closed walk traversing every edge exactly once exists.
///   Conditions: weakly connected (ignoring isolated nodes) AND every node
///   satisfies in_degree == out_degree.
///
/// `has_path`: open walk traversing every edge exactly once exists.
///   Conditions: weakly connected AND exactly one node has out−in=1 (start)
///   AND exactly one has in−out=1 (end).  Mutually exclusive with has_circuit.
///
/// `start_vec` / `end_vec`: vectors of path endpoints; zero when has_circuit
///   or neither.
///
/// Isolated nodes are excluded from the analysis.
/// Vacuous (no edges): has_circuit=true (empty closed walk).
///
/// Algorithm: O(V+E) — one edge-degree scan + one undirected BFS.
/// OS analogy: can a maintenance daemon visit every IPC channel exactly once
/// and return to base (circuit) or traverse the whole message bus end-to-end
/// (path)?  Equivalent to asking whether a network audit walk is possible.
pub fn graph_eulerian() -> (bool, bool, VectorAddress, VectorAddress, usize) {
    RUNTIME.lock().graph_eulerian_inner()
}

/// V2.88: Longest directed path (critical path) in the graph's DAG projection.
///
/// Returns `(path_hops, is_dag, start_vec, end_vec, node_count)`:
///   - `path_hops`  — hop count of the longest directed path; 0 if no edges,
///                    no unique longest path, or if a directed cycle is present.
///   - `is_dag`     — true iff the graph has no directed cycles (self-loops
///                    included).  False means the graph is not a DAG.
///   - `start_vec`  — source end of a critical path (zero when no path found).
///   - `end_vec`    — sink end of a critical path (zero when no path found).
///   - `node_count` — total live nodes.
///
/// Algorithm: Kahn's BFS topological sort with simultaneous max-distance DP.
/// O(V+E), no_std safe.
///
/// OS analogy: `systemd-analyze critical-chain` — the minimum serial depth
/// that any parallel boot sequence must still traverse in a DAG of service
/// dependencies.
pub fn graph_dag_longest() -> (u32, bool, VectorAddress, VectorAddress, usize) {
    RUNTIME.lock().graph_dag_longest_inner()
}

/// V2.89: DAG topological layer assignment — assigns each node its earliest
/// parallel-execution level in the DAG (Kahn BFS multi-source layering).
///
/// Returns `(vecs, layers, node_count, layer_count, is_dag)`:
///   - `vecs[0..node_count]`   — live node vectors, sorted ascending by layer then VectorAddress.
///   - `layers[0..node_count]` — layer number for each node (0 = source, 1 = one hop from source, …).
///   - `node_count`            — total live nodes.
///   - `layer_count`           — number of distinct layers (= max_layer + 1); 0 if cyclic.
///   - `is_dag`                — false iff the graph contains a directed cycle (layers undefined).
///
/// Algorithm: multi-source Kahn BFS with layer propagation, O(V+E), no_std safe.
/// All in-degree-0 nodes are seeded at layer 0; each subsequent layer is the max
/// predecessor layer + 1.  Unlike DAG longest path (V2.88) which finds a single
/// critical path, this assigns EVERY node its minimum possible depth.
///
/// OS analogy: `systemd --analyze` dependency levels — which services can boot in
/// parallel (same layer) and which must wait for the previous layer to complete.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_dag_layers<const N: usize>() -> ([VectorAddress; N], [u32; N], usize, u32, bool) {
    RUNTIME.lock().graph_dag_layers_inner()
}

/// V2.90: Graph dominator tree — Cooper, Harvey & Kennedy 2001 simple iterative
/// algorithm.  For every node reachable from `start`, computes its immediate
/// dominator: the closest node that lies on every directed path from `start`.
///
/// Returns `(vecs, idoms, node_count, reachable_count)`:
///   - `vecs[0..reachable_count]`  — reachable nodes in RPO order.
///   - `idoms[0..reachable_count]` — immediate dominator vector per node.
///   - For `start`: `idoms[i] == vecs[i]` (start dominates itself).
///   - `node_count`      — total live nodes in the graph.
///   - `reachable_count` — nodes reachable from `start` (including start).
///
/// If `start` is not in the graph, returns `(_, _, node_count, 0)`.
/// Unreachable nodes (no directed path from `start`) are excluded from output.
///
/// OS analogy: compiler CFG dominator analysis — which kernel subsystem must
/// be initialised (with no alternative path) before this component can run.
/// Like `systemd-analyze critical-chain --all` but reports the single mandatory
/// predecessor for each service, not just the root chain.
/// N caps output depth (max MAX_NODES = 128).
pub fn graph_domtree<const N: usize>(
    start: VectorAddress,
) -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_domtree_inner(start)
}

/// V2.91: Directed back-edges that form the feedback arc set (FAS) of the
/// live kernel graph.
///
/// Returns `(from_vecs, to_vecs, arc_count, node_count)`:
///   - `from_vecs[0..arc_count]` / `to_vecs[0..arc_count]` — feedback arcs,
///     sorted ascending by (from.as_u64(), to.as_u64()).
///   - `arc_count`  — number of feedback arcs found.
///   - `node_count` — total live nodes.
///
/// A feedback arc is a directed edge (u→v) whose removal (along with all such
/// arcs) leaves the graph acyclic.  Self-loops are included (trivially cyclic).
/// Removing all returned arcs yields a DAG (a valid topological-sort ordering
/// then exists).
///
/// Algorithm: iterative 3-colour DFS (UNVISITED → IN_STACK → DONE), O(V+E).
/// Each edge to an IN_STACK ("gray") node is a back-edge = feedback arc.
/// Cross/forward edges (neighbour is DONE) are not arcs and are skipped.
/// The result is a valid FAS; minimum-FAS is NP-hard and not claimed here.
///
/// OS analogy: `tsort --debug` or `systemd --show-environment` cycle report —
/// the exact dependency edges that introduce circular boot-order requirements.
/// Removing all returned arcs permits a clean topological initialisation of
/// every kernel subsystem, with no deadlocked wait cycles.
/// N controls the output buffer depth (cap at MAX_EDGES = 512).
pub fn graph_feedback_arc<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_feedback_arc_inner()
}

/// V2.92: Maximum bipartite matching — Kuhn's iterative DFS, O(V·E).
///
/// Returns `(left_vecs, right_vecs, match_count, is_bipartite, node_count)`:
/// - `left_vecs[0..match_count]`  — matched side-A (color 0) nodes.
/// - `right_vecs[0..match_count]` — matched side-B (color 1) nodes.
/// - `match_count`                — maximum matching size; 0 if not bipartite.
/// - `is_bipartite`               — false if an odd-length cycle was detected.
/// - `node_count`                 — total live nodes.
///
/// Edges are treated as undirected; the bipartition is determined by BFS
/// 2-colouring.  For each free side-A node, an iterative DFS explores
/// alternating paths and augments the matching when a free side-B node is
/// reached.  Removing both the left_vecs and right_vecs arrays from the result
/// and cross-referencing by index gives the full matched pair list.
///
/// OS analogy: `taskset` / `numactl --cpunodebind` optimal CPU↔task assignment —
/// maximum number of tasks that can each be exclusively bound to a distinct CPU,
/// given a bipartite affinity graph of which task can run on which CPU.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_bipartite_match<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], usize, bool, usize) {
    RUNTIME.lock().graph_bipartite_match_inner()
}

/// V2.93: 2-edge-connected components (2ECCs) of the live kernel graph.
///
/// Two nodes are in the same 2ECC iff no single edge removal can disconnect
/// them (∃ ≥2 edge-disjoint paths between them).  Every vertex belongs to
/// exactly one 2ECC.  Bridges are the boundary edges between components.
///
/// Returns `(vecs, comp_ids, node_count, comp_count)`:
///   - `vecs[0..node_count]`     — live nodes sorted by (comp_id, VectorAddress).
///   - `comp_ids[0..node_count]` — 0-indexed 2ECC ID for each node.
///   - `node_count`              — total live nodes.
///   - `comp_count`              — number of distinct 2ECCs.
///
/// Algorithm: Tarjan bridge-finding + BFS on non-bridge undirected edges,
/// O(V+E), no_std safe.
///
/// OS analogy: kernel subsystem clusters resilient to any single IPC link
/// failure — analogous to bonded NICs or RAID-1 paths in storage fabric.
pub fn graph_2ecc<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize) {
    RUNTIME.lock().graph_2ecc_inner()
}

/// V2.94: k-truss decomposition — edge-triangle cohesion.
///
/// Returns (vecs, trussness, node_count, max_trussness).
///   vecs[0..n]     — nodes sorted trussness-descending
///   trussness[i]   — max k such that node has an incident edge in the k-truss
///                    (0 = isolated; 2 = has edges but no triangles; ≥ 3 = in triangles)
///   max_trussness  — graph truss number (highest node trussness)
///
/// Strictly finer than k-core: a k-truss ⊆ (k−1)-core.
pub fn graph_truss<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, u8) {
    RUNTIME.lock().graph_truss_inner()
}

/// V2.95: Maximum clique — Bron-Kerbosch with Tomita pivot, O(3^{n/3}).
///
/// Returns `(clique_vecs, clique_size, clique_count, node_count)`:
///   - `clique_vecs[0..clique_size]` — a representative maximum clique,
///     sorted ascending by VectorAddress.
///   - `clique_size`  — ω(G), the clique number (0 if graph is empty).
///   - `clique_count` — number of distinct maximum-size cliques found.
///   - `node_count`   — total live nodes.
///
/// Works on the undirected projection (both A→B and B→A → edge A–B).
/// Self-loops excluded.
///
/// Density hierarchy: clique ⊇ truss ⊇ core:
///   ω(G) ≥ max_truss − 1  and  ω(G) ≥ max_kcore (not always tight).
///
/// OS analogy: the tightest cluster of kernel subsystems where every module
/// directly depends on every other — the hardest coupling to decouple for
/// hot-patching or fault isolation.
/// N controls the output buffer depth (cap at MAX_NODES = 128).
pub fn graph_clique<const N: usize>() -> ([VectorAddress; N], usize, usize, usize) {
    RUNTIME.lock().graph_clique_inner()
}

pub fn graph_independent_set<const N: usize>() -> ([VectorAddress; N], usize, usize, usize) {
    RUNTIME.lock().graph_independent_set_inner()
}

/// V2.97: Minimum vertex cover of the live kernel graph.
///
/// Returns `(cover_vecs, cover_size, is_exact, node_count)`:
/// - `cover_vecs[0..cover_size]` — cover vertices sorted ascending by as_u64().
/// - `cover_size`                — |vertex cover|; exact for bipartite, ≤2× opt for general.
/// - `is_exact`                  — true iff bipartite (König exact); false = 2-approximation.
/// - `node_count`                — total live nodes.
///
/// Bipartite: Kuhn matching + König construction; |T| = ν(G) = τ(G).
/// General: greedy maximal matching (cover both endpoints of each selected edge).
/// Key invariants: Gallai α+τ=n (bipartite); König τ=ν (bipartite).
///
/// OS analogy: minimum set of kernel modules each supervising at least one side of
/// every IPC channel — the smallest possible audit-checkpoint set for all cross-module
/// communication in the system dependency graph.
pub fn graph_vertex_cover<const N: usize>() -> ([VectorAddress; N], usize, bool, usize) {
    RUNTIME.lock().graph_vertex_cover_inner()
}

/// V2.98 — minimum dominating set (greedy ln(Δ)+1 approximation).
///
/// Returns (dom_vecs, dom_size, node_count).
/// dom_vecs[0..dom_size] = dominating set, sorted ascending by VectorAddress.as_u64().
pub fn graph_dominating_set<const N: usize>() -> ([VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_dominating_set_inner()
}

/// V2.99: Minimum path cover (MPC) of the live kernel graph (DAG only).
///
/// Returns `(path_vecs, path_ids, path_count, is_dag, node_count)`:
/// - `path_vecs[0..node_count]` — all live nodes concatenated in path order.
/// - `path_ids[0..node_count]`  — 0-indexed path ID per node.
/// - `path_count`               — minimum number of vertex-disjoint directed paths
///                                covering every node; `path_count = n − ν(B(G))`
///                                (König / Dilworth theorem).
/// - `is_dag`                   — false if directed cycle detected (MPC undefined).
/// - `node_count`               — total live nodes.
///
/// Algorithm: Kahn BFS (DAG check + topo order) → bipartite expansion B(G)
/// (left_u → right_v for each directed edge u→v) → Kuhn augmenting-path
/// matching with u128 bitmasks, O(V·E).
///
/// OS analogy: the minimum number of sequential install/upgrade chains
/// needed to deploy a kernel patch across all modules in a dependency DAG —
/// like `make -j<path_count>` where each job is one ordered dependency chain.
pub fn graph_min_path_cover<const N: usize>(
) -> ([VectorAddress; N], [u8; N], usize, bool, usize) {
    RUNTIME.lock().graph_min_path_cover_inner()
}

/// V3.00: Minimum spanning arborescence (directed MST) from `root`.
///
/// Uses the Chu-Liu / Edmonds 1967 algorithm: iteratively selects minimum
/// incoming edges per super-node and contracts directed cycles until no cycles
/// remain, giving the minimum total-weight spanning arborescence.
///
/// Returns `(vecs, parents, weights, node_count, total_w, is_connected)`:
///   `vecs[0..nc]`    — all live nodes, root first.
///   `parents[0..nc]` — parent in arborescence (self=root; zero=unreachable).
///   `weights[0..nc]` — edge weight×1000 to parent (0 for root).
///   `node_count`     — total live nodes.
///   `total_w`        — Σ arborescence edge weights × 1000.
///   `is_connected`   — true iff a spanning arborescence exists from `root`.
///
/// OS analogy: minimum total-latency directed dependency tree from a boot
/// controller — the cheapest way to propagate a startup signal from one kernel
/// module to every other module exactly once, following directed IPC edges.
/// Equivalent to `systemd-analyze critical-chain` but minimises total weight
/// rather than just depth.
///
/// Literature: Chu & Liu 1965; Edmonds 1967 ("Optimum branchings");
///             Tarjan 1977 (O(E log V) improvement).
pub fn graph_arborescence<const N: usize>(
    root: VectorAddress,
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32, bool) {
    RUNTIME.lock().graph_arborescence_inner(root)
}

/// V3.01: Feedback vertex set — greedy Kahn-based cycle-breaking.
///
/// Returns `(fvs_vecs, fvs_size, node_count)`.
/// `fvs_vecs[0..fvs_size]` = nodes to remove to make the graph acyclic,
/// sorted ascending by `VectorAddress.as_u64()`.
pub fn graph_fvs<const N: usize>() -> ([VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_fvs_inner()
}

/// V3.02: Global minimum cut — Stoer-Wagner 1997.
///
/// Returns `(vecs, sides, node_count, min_cut, side_b_size)`:
/// - `vecs[0..node_count]` — all live nodes; side-A first (sides==0), then side-B (sides==1).
/// - `sides[0..node_count]` — 0=side A, 1=side B.
/// - `node_count` — total live nodes.
/// - `min_cut` — minimum undirected edge cut (edge connectivity κ'(G)).
/// - `side_b_size` — count of side-B nodes.
///
/// Undirected projection: A→B and B→A count as one edge.
/// Disconnected graphs return `min_cut = 0`.
pub fn graph_min_cut<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, u32, usize) {
    RUNTIME.lock().graph_min_cut_inner()
}

/// V3.03: Hamiltonian path/circuit detection.
///
/// Returns `(path_vecs, path_len, has_circuit, has_path, node_count)`:
/// - `path_vecs[0..path_len]` — nodes in the found Hamiltonian path/circuit order.
/// - `path_len` — equals `node_count` if found, 0 if not found.
/// - `has_circuit` — true iff a directed Hamiltonian circuit exists.
/// - `has_path` — true iff a directed Hamiltonian path exists (has_circuit ⇒ has_path).
/// - `node_count` — total live nodes.
///
/// Directed graph. Self-loops excluded. Single node: trivially Ham. circuit.
pub fn graph_hamiltonian<const N: usize>() -> ([VectorAddress; N], usize, bool, bool, usize) {
    RUNTIME.lock().graph_hamiltonian_inner()
}

/// V3.04: Chordal graph recognition — LexBFS + PEO verification.
///
/// A graph is *chordal* iff every cycle of length ≥ 4 has a chord (an edge
/// connecting two non-adjacent cycle vertices).  Equivalently, it admits a
/// Perfect Elimination Ordering (PEO): an ordering v₁, …, vₙ where each vᵢ's
/// later neighbours form a clique.
///
/// Returns `(peo_vecs, is_chordal, node_count)`:
/// - `peo_vecs[0..node_count]` — LexBFS perfect elimination ordering.
/// - `is_chordal` — true iff the graph is chordal (the LexBFS PEO is valid).
/// - `node_count` — total live nodes.
///
/// Undirected projection: A→B and B→A together count as one undirected edge.
/// Self-loops are excluded.  Empty and ≤ 2-node graphs are trivially chordal.
///
/// Algorithm: LexBFS (Rose, Tarjan & Lueker 1976), O(V+E).
/// PEO check: N⁺(v) = earlier-numbered neighbours; N⁺(v)\{w} ⊆ N(w) where w
/// is the most-recently-numbered member of N⁺(v) (Fulkerson & Gross 1965).
///
/// OS analogy: a chordal dependency graph admits a perfect elimination order
/// for bringing subsystems online/offline — each removed subsystem sees its
/// remaining peers all mutually inter-operating, enabling zero-surprise isolation.
pub fn graph_chordal<const N: usize>() -> ([VectorAddress; N], bool, usize) {
    RUNTIME.lock().graph_chordal_inner()
}

/// V3.05: Biconnected components (BCCs) of the live kernel graph's undirected
/// projection — iterative Tarjan edge-stack algorithm, O(V+E).
///
/// Returns `(vecs, bcc_ids, node_count, bcc_count)`:
///   - `vecs[0..node_count]`    — all live nodes, sorted (bcc_id asc, vec asc).
///   - `bcc_ids[0..node_count]` — biconnected component index per node.
///     Regular nodes carry their BCC index (0-based).
///     Articulation points — nodes that appear in 2+ BCCs — carry `255`.
///     Each isolated node gets its own singleton BCC.
///   - `node_count`  — total live nodes.
///   - `bcc_count`   — total BCCs (edge-BCCs + isolated-singletons).
///
/// A biconnected component is a maximal 2-vertex-connected subgraph: removing
/// any single vertex within it does not disconnect it.  Articulation points
/// (cut vertices from `graph_articulation`) are exactly the nodes with
/// `bcc_id == 255` in this output.
///
/// OS analogy: BCCs are the fault-isolation "blocks" of the kernel dependency
/// graph — within a block, any subsystem can crash without partitioning the
/// block's connectivity.  Articulation points (bcc_id=255) are the single
/// points-of-failure bridging two or more blocks.
///
/// Literature: Tarjan 1972 (low-link DFS), Hopcroft & Tarjan 1973.
pub fn graph_bcc<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize) {
    RUNTIME.lock().graph_bcc_inner()
}

/// V3.06: Edge Betweenness Centrality — Brandes (2001).
///
/// Returns `(from_vecs, to_vecs, scores, edge_count)`:
/// - `from_vecs[0..edge_count]` / `to_vecs[0..edge_count]` — directed edge endpoints.
/// - `scores[0..edge_count]` — betweenness count for each edge (# ordered source-target
///   pairs whose unique shortest path traverses this edge).
/// - `edge_count` — number of live non-self-loop directed edges.
///
/// Output is sorted descending by score; ties broken by (from.as_u64(), to.as_u64()) asc.
pub fn graph_betweenness_edge<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize) {
    RUNTIME.lock().graph_betweenness_edge_inner()
}

/// V3.07: Vertex connectivity κ(G) — minimum vertex cut (Even 1975 / Menger 1927).
///
/// Returns `(vecs, node_count, kappa, min_degree)`:
/// - `vecs[0..node_count]`  — all live nodes sorted ascending by VectorAddress.
/// - `node_count`           — number of live nodes.
/// - `kappa`                — κ(G): minimum number of vertices whose removal
///                            disconnects (or trivialises) the graph.
/// - `min_degree`           — δ(G): minimum undirected degree.
///
/// Whitney invariant: κ(G) ≤ κ'(G) ≤ δ(G)
/// (vertex connectivity ≤ edge connectivity ≤ min degree).
///
/// Algorithm: undirected projection of the directed graph; BFS connectivity
/// check; complete-graph shortcut; then Even's reduction — fix the minimum-
/// degree vertex s, and for each non-neighbour t compute max vertex-disjoint
/// s-t paths via Edmonds-Karp on the node-split network.  O(δ · n · (n+m)).
///
/// Special cases:
/// - Disconnected or empty → κ=0.
/// - K_n → κ=n-1 (complete graph shortcut).
/// - Single node          → κ=0.
///
/// OS analogy: minimum kernel subsystems to disable/mask to split the
/// dependency graph into two isolated fault domains (compare min-cut V3.02,
/// which counts minimum edges).
pub fn graph_vertex_connectivity<const N: usize>() -> ([VectorAddress; N], usize, u32, u32) {
    RUNTIME.lock().graph_vertex_connectivity_inner::<N>()
}

/// V3.08: Graph edge colouring — greedy χ'(G) (Vizing 1964).
///
/// Assigns a colour to every undirected edge so that no two edges sharing
/// a common endpoint receive the same colour.  By Vizing's theorem (1964),
/// the chromatic index χ'(G) is either Δ(G) or Δ(G)+1.  König (1916) shows
/// that bipartite graphs always achieve χ'(G) = Δ(G) (class 1).
///
/// Returns `(from_vecs, to_vecs, edge_colors, edge_count, chromatic_index)`:
/// - `from_vecs[0..edge_count]`   — canonical "from" vector for each edge.
/// - `to_vecs[0..edge_count]`     — canonical "to" vector.
/// - `edge_colors[0..edge_count]` — 0-indexed colour assigned to each edge.
/// - `edge_count`                 — total undirected edges (self-loops excluded).
/// - `chromatic_index`            — χ'(G) = max colour used + 1; 0 if no edges.
///
/// Sort: ascending (colour, from.as_u64(), to.as_u64()).
///
/// Algorithm: greedy — for each edge (a,b) assign the lowest colour not
/// already used by any edge incident to a or b.  Uses a u128 bitmask per
/// node; `trailing_ones(node_colors[a] | node_colors[b])` gives the lowest
/// free colour in O(1).  O(E) time, O(V+E) space.
///
/// OS analogy: minimum number of non-conflicting time-slots to schedule all
/// IPC channels so no two channels sharing a kernel-subsystem endpoint are
/// active in the same slot — the epoch width for contention-free round-robin
/// syscall dispatch (analogous to O_DIRECT I/O slot multiplexing or NIC
/// transmit-queue striping without head-of-line blocking).
pub fn graph_edge_color<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], [u8; N], usize, u8) {
    RUNTIME.lock().graph_edge_color_inner()
}

/// V3.09: Spectral radius ρ(A) and algebraic connectivity λ₂(L).
///
/// Returns `(spectral_radius_ppm, algebraic_connectivity_ppm, node_count)`:
/// - `spectral_radius_ppm`        — ρ(A) × 1_000_000: largest eigenvalue of the
///   undirected adjacency matrix, computed via 60-step power iteration.
/// - `algebraic_connectivity_ppm` — λ₂(L) × 1_000_000: second-smallest eigenvalue of
///   the Laplacian L = D − A.  Zero for disconnected graphs.
/// - `node_count`                 — number of live nodes.
///
/// OS analogy: ρ(A) is the signal-amplification factor in the kernel event graph;
/// 1/ρ is the epidemic threshold — if a fault cascades with rate β > 1/ρ it will
/// saturate the kernel.  λ₂(L) is the mixing speed of supervisor broadcasts;
/// low λ₂ signals a bottleneck (single-bridge topology) in the dependency graph,
/// analogous to a slow convergence in BGP or a poorly-connected NUMA topology.
///
/// Literature: Fiedler 1973 (algebraic connectivity), Anderson & Morley 1985,
/// Wang et al. 2003 (epidemic threshold), Mohar 1991 (Cheeger: h ≥ λ₂/2).
pub fn graph_spectral() -> (u32, u32, usize) {
    RUNTIME.lock().graph_spectral_inner()
}

/// V3.10: Shannon entropy of the undirected degree distribution.
///
/// Returns `(entropy_ppm, normalized_ppm, node_count)`:
/// - `entropy_ppm`    = H × 10^6  where H = −Σ p(d) ln p(d)  (nats)
/// - `normalized_ppm` = H/ln(n) × 10^6  ∈ [0, 1_000_000]  (1_000_000 = max diversity)
/// - `node_count`     = number of alive nodes
///
/// H = 0 for regular graphs (all same degree); H = ln(n) when all degrees distinct.
/// Algorithm: undirected degree histogram → integer LN_TABLE accumulation, O(V+E).
pub fn graph_entropy() -> (u32, u32, usize) {
    RUNTIME.lock().graph_entropy_inner()
}

/// V3.11: Zagreb indices M1/M2, Randić connectivity R(G), and Albertson irregularity I(G).
///
/// Returns `(m1, m2, randic_ppm, irregularity, edge_count, node_count)`:
/// - m1            = Σ_v deg(v)²  (first Zagreb index; Gutman & Trinajstić 1972)
/// - m2            = Σ_{uv∈E} deg(u)×deg(v)  (second Zagreb index)
/// - randic_ppm    = R(G) × 10^6 where R = Σ_{uv∈E} 1/√(deg(u)×deg(v))  (Randić 1975)
/// - irregularity  = I(G) = Σ_{uv∈E} |deg(u)−deg(v)|  (Albertson 1997; 0 iff regular)
/// - edge_count    = undirected edge count (directed→undirected dedup, self-loops excluded)
/// - node_count    = live node count
pub fn graph_zagreb() -> (u64, u64, u32, u32, usize, usize) {
    RUNTIME.lock().graph_zagreb_inner()
}

/// V3.12: Sum-connectivity (SC), geometric-arithmetic (GA), and augmented Zagreb (AZI) indices.
///
/// Returns `(sc_ppm, ga_ppm, azi_milli, edge_count, node_count)`:
/// - `sc_ppm`    = SC × 10^6  (Zhou & Trinajstić 2009; SC = Σ 1/√(deg(u)+deg(v)))
/// - `ga_ppm`    = GA × 10^6  (Vukičević & Furtula 2009; GA = Σ 2√(deg(u)·deg(v))/(deg(u)+deg(v)))
/// - `azi_milli` = AZI × 1000 (Furtula, Graovac & Vukičević 2010; AZI = Σ (deg·deg/(deg+deg−2))³)
/// - `edge_count` = undirected edge count
/// - `node_count` = alive node count
///
/// GA = |E| iff the graph is regular (all nodes have the same degree).
/// AZI skips pendant-pendant edges (both endpoints have degree 1, denominator = 0).
/// All indices use integer Newton-Raphson isqrt for ppm/milli precision, O(V+E).
pub fn graph_topo_indices() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices_inner()
}

/// V3.13: H + ABC + F degree-based topological indices.
///
/// Returns (h_ppm, abc_ppm, f_index, edge_count, node_count).
///   h_ppm   = H(G) × 10^6   where H   = Σ_{uv∈E} 2/(deg(u)+deg(v))         (Zhong 2012)
///   abc_ppm = ABC(G) × 10^6 where ABC = Σ_{uv∈E} √((s−2)/p), s=deg-sum, p=deg-product
///                                                                              (Estrada et al. 2008)
///   f_index = F(G) = Σ_v deg(v)³  (exact integer)                            (Furtula & Gutman 2015)
///   edge_count = undirected non-self-loop edges
///   node_count = alive nodes
pub fn graph_topo_indices2() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices2_inner()
}

/// V3.14: SDD + ISI + Nirmala degree-based topological indices.
///
/// Returns (sdd_ppm, isi_ppm, ni_ppm, edge_count, node_count).
///   sdd_ppm = SDD × 10^6 where SDD = Σ_{uv∈E} (da²+db²)/(da·db)  (Vasilyev 2014)
///   isi_ppm = ISI × 10^6 where ISI = Σ_{uv∈E} da·db/(da+db)       (Sedlar et al. 2011)
///   ni_ppm  = NI  × 10^6 where NI  = Σ_{uv∈E} √(da+db)            (Rather et al. 2021)
pub fn graph_topo_indices3() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices3_inner()
}

/// V3.15: Sombor + Reduced Second Zagreb + Sigma degree-based topological indices.
///
/// Returns (so_ppm, rm2, sigma, edge_count, node_count).
///   so_ppm = SO × 10^6 where SO  = Σ_{uv∈E} √(da²+db²)      (Gutman 2021)
///   rm2    = RM₂        where RM₂ = Σ_{uv∈E} (da-1)·(db-1)   (Furtula, Gutman & Ediz 2014)
///   sigma  = σ(G)       where σ   = Σ_{uv∈E} (da-db)²        (Gutman et al. 2014)
pub fn graph_topo_indices4() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices4_inner()
}

/// V3.16: Hyper-Zagreb HM₁ + HM₂ + Arithmetic-Geometric AG degree-based indices.
///
/// Returns (hm1, hm2, ag_ppm, edge_count, node_count).
///   hm1    = HM₁(G)    where HM₁ = Σ_{uv∈E} (da+db)²                    (Shirdel et al. 2013)
///   hm2    = HM₂(G)    where HM₂ = Σ_{uv∈E} (da·db)²                    (Das & Trinajstić 2011)
///   ag_ppm = AG × 10^6 where AG  = Σ_{uv∈E} (da+db)/(2√(da·db))         (Zheng et al. 2020)
pub fn graph_topo_indices5() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices5_inner()
}

/// V3.17: `graph topo6` — EM₁ + ABS + RRR degree-based topological indices (undirected projection).
///   em1    = EM₁(G)    where EM₁ = Σ_{uv∈E} (da+db-2)²               (Milićević et al. 2004)
///   abs_ppm = ABS×10^6  where ABS = Σ_{uv∈E} √((da+db-2)/(da+db))    (Chen et al. 2022)
///   rrr_ppm = RRR×10^6  where RRR = Σ_{uv∈E} √((da-1)·(db-1))        (Li & Shi 2008)
pub fn graph_topo_indices6() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices6_inner()
}

/// V3.18: `graph topo7` — Wiener W + Harary H + Hyper-Wiener WW distance-based topological indices.
///   wiener       = W(G)       = Σ_{u<v} d(u,v)                             (Wiener 1947)
///   harary_ppm   = H(G)×10^6 = Σ_{u<v} 1/d(u,v) × 10^6                   (Plavšić et al. 1993)
///   hyper_wiener = WW(G)      = (1/2) Σ_{u<v} [d(u,v) + d(u,v)²]          (Klein & Randić 1993)
/// Disconnected pairs are excluded (d=∞ contributes 0). BFS on undirected projection.
pub fn graph_topo_indices7() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices7_inner()
}

/// V3.19: `graph topo8` — eccentricity-based topological indices.
///
/// Returns (eci, avg_ecc_ppm, diameter, radius, edge_count, node_count).
///   eci         = ξ(G) = Σ_v deg(v) × ecc(v)          (exact u64; Sharma, Goswami & Madan 1997)
///   avg_ecc_ppm = (Σ_v ecc(v)) / n × 10^6             (floor ppm; Buckley & Harary 1990)
///   diameter    = D(G) = max_{v} ecc(v)                (exact u32; 0 if nc≤1 or all isolated)
///   radius      = R(G) = min{ecc(v) | ecc(v)>0}       (exact u32; 0 if no connected pairs)
///
/// ecc(v) = max BFS distance from v to any reachable node (0 for isolated or single node).
/// BFS on undirected projection, O(n·(n+m)).
pub fn graph_topo_indices8() -> (u64, u64, u32, u32, usize, usize) {
    RUNTIME.lock().graph_topo_indices8_inner()
}

/// V3.20: `graph topo9` — Schultz MTI + Gutman Index + Connective Eccentric Index (degree-distance hybrid).
///
/// Returns (ws, wg, cxe_ppm, edge_count, node_count).
///   ws      = W_S(G) = Σ_{u<v} (deg(u)+deg(v))×d(u,v)    (exact u64; Schultz 1989)
///   wg      = W_G(G) = Σ_{u<v} deg(u)×deg(v)×d(u,v)      (exact u64; Gutman 1994)
///   cxe_ppm = CξE(G)×10^6 = Σ_v deg(v)/ecc(v) × 10^6     (floor ppm; Gupta et al. 2000)
/// Disconnected pairs (d=∞) contribute 0. Isolated nodes (ecc=0) contribute 0 to CξE.
/// BFS on undirected projection, O(n·(n+m)).
pub fn graph_topo_indices9() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices9_inner()
}

/// V3.21: `graph topo10` — Szeged Sz + Revised Szeged rSz + Mostar Mo (edge-partition distance).
///
/// Returns (sz, rsz_ppm, mo, edge_count, node_count).
///   sz      = Sz(G)  = Σ_{uv∈E} n_u(uv)·n_v(uv)                    (exact u64; Gutman & Klavžar 1995)
///   rsz_ppm = rSz(G)×10^6 = Σ_{uv∈E} (n_u+n₀/2)·(n_v+n₀/2)×10^6  (floor ppm; Pisanski & Randić 2010)
///   mo      = Mo(G)  = Σ_{uv∈E} |n_u − n_v|                         (exact u64; Doslić et al. 2018)
/// For each edge {u,v}: n_u=#{w:d(w,u)<d(w,v)}, n_v=#{w:d(w,u)>d(w,v)}, n₀=equidistant count.
/// Tree invariant: n₀=0 for all tree edges → Sz = rSz = Wiener index.
/// BFS on undirected projection, O(n·(n+m)).
pub fn graph_topo_indices10() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices10_inner()
}

/// V3.22 transmission-based topological indices on the current graph.
///
/// Returns `(j_ppm, ti, piv, edge_count, node_count)`:
///   j_ppm = J(G) × 10^6  (floor ppm; Balaban 1982)
///           J = (m/μ) × Σ_{uv∈E} 1/√(T_u·T_v)
///           μ = max(1, m−n+2); T_v = Σ_{w reachable} d(v,w)
///   ti    = TI(G) = Σ_{uv∈E} |T_u − T_v|  (exact u64; Abdo & Dimitrov 2014)
///   piv   = PI_v(G) = Σ_{uv∈E} (T_u + T_v)  (exact u64; Khalifeh et al. 2008)
/// BFS on undirected projection, O(n·(n+m)).
pub fn graph_topo_indices11() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices11_inner()
}

/// V3.23 Zagreb eccentricity indices on the current graph.
///
/// Returns `(m1e, m2e, m3e, edge_count, node_count)`:
///   m1e = M1*(G) = Σ_v ecc(v)²                 (exact u64; Vukičević & Graovac 2010)
///   m2e = M2*(G) = Σ_{uv∈E} ecc(u)×ecc(v)      (exact u64; Das et al. 2013)
///   m3e = M3*(G) = Σ_{uv∈E} |ecc(u)−ecc(v)|    (exact u64; Farooq & Ali 2021)
/// ecc(v) = max BFS distance from v to any reachable node (0 for isolated nodes).
/// M3* = 0 iff self-centered (all ecc equal, e.g. complete graphs, even cycles).
/// BFS on undirected projection, O(n·(n+m)).
pub fn graph_topo_indices12() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices12_inner()
}

/// V3.24: Transmission Zagreb indices.
///   Returns (tm1, tm2, ga_t_ppm, edge_count, node_count)
///   tm1     = TM₁(G) = Σ_v T_v²                                (exact u64)
///   tm2     = TM₂(G) = Σ_{uv∈E} T_u·T_v                       (exact u64)
///   ga_t    = GA_t(G) × 10^6 = Σ_{uv∈E} 2√(T_u·T_v)/(T_u+T_v) (floor ppm)
/// T_v = vertex transmission = Σ_{w reachable,w≠v} d(v,w).
/// GA_t = |E|×10^6 iff all nodes have equal transmission.
pub fn graph_topo_indices13() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices13_inner()
}

/// V3.25: Eccentricity-based topological indices — TE + EDS + GEA.
///   Returns (te, eds, gea_ppm, edge_count, node_count)
///   te      = TE(G)  = Σ_v ecc(v)                                    (exact u64; Dankelmann et al. 2004)
///   eds     = EDS(G) = Σ_v ecc(v)·T_v                                (exact u64; Gupta et al. 2008)
///   gea_ppm = GEA(G) × 10^6 = Σ_{uv∈E} 2√(ecc(u)·ecc(v))/(ecc(u)+ecc(v)) (floor ppm)
/// ecc(v)=max BFS distance from v (0 for isolated). T_v=vertex transmission.
/// GEA = |E|×10^6 iff graph is self-centered (all ecc equal).
pub fn graph_topo_indices14() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices14_inner()
}

/// V3.26: Leap Zagreb indices — LM₁ + LM₂ + LM₃.
///   Returns (lm1, lm2, lm3, edge_count, node_count)
///   lm1 = LM₁(G) = Σ_v d₂(v)²                          (exact u64; Naji et al. 2017)
///   lm2 = LM₂(G) = Σ_{uv∈E} d₂(u)·d₂(v)               (exact u64)
///   lm3 = LM₃(G) = Σ_{uv∈E} (d₂(u)+d₂(v))             (exact u64)
/// d₂(v) = |{w : d(v,w) = 2}| = 2-distance degree.
/// LM₁=LM₂=LM₃=0 for complete graphs (d₂=0 everywhere; all pairs adjacent).
/// BFS on undirected projection, O(n·(n+m)).
pub fn graph_topo_indices15() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices15_inner()
}

/// V3.27: Generalized Randić family — R_{1/2} + R_{-1} + Lanzhou Lz.
///   Returns (ir_ppm, rr_ppm, lz, edge_count, node_count)
///   ir_ppm = R_{1/2}(G)×10^6 = Σ_{uv∈E} √(d_u·d_v)×10^6  (floor ppm; Bollobás & Erdős 1998)
///   rr_ppm = R_{-1}(G)×10^6  = Σ_{uv∈E} ⌊10^6/(d_u·d_v)⌋ (floor ppm; Bollobás & Erdős 1998)
///   lz     = Lz(G) = Σ_v d_v²·(n−1−d_v)                   (exact u64; Xia et al. 2019)
/// R_{1/2} (Product Connectivity) ≥ m always; = m·Δ for Δ-regular (ppm = m·Δ·10^6).
/// R_{-1} (Reciprocal Randić) ≤ m always; = m/Δ² for Δ-regular.
/// Lz = 0 for complete graphs (n−1−d=0); = 0 for empty graphs.
/// Algorithm: O(V+E) degree scan; no BFS needed.
pub fn graph_topo_indices16() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices16_inner()
}

/// V3.28: `graph topo17` — Zagreb coindices M̄₁ + M̄₂ + forgotten coindex F̄.
///
///   Returns (mbar1, mbar2, fbar, edge_count, node_count).
///   M̄₁(G) = Σ_{uv∉E} (d_u + d_v)   = 2m(n−1) − M₁  (Ashrafi, Došlić & Hamzeh 2010)
///   M̄₂(G) = Σ_{uv∉E} d_u · d_v    = 2m² − M₁/2 − M₂ (Ashrafi, Došlić & Hamzeh 2010)
///   F̄(G)  = Σ_{uv∉E} (d_u²+d_v²)  = (n−1)·M₁ − F    (forgotten coindex; De 2016)
/// All three computed analytically from Zagreb indices — no complement scan.
/// M̄₁=M̄₂=F̄=0 for complete graphs (no non-edges).
/// Algorithm: O(V+E) degree scan; no BFS needed.
pub fn graph_topo_indices17() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices17_inner()
}

/// V3.29: `graph topo18` — NM₁ + NM₂ + GA₂ Neighborhood Zagreb indices.
///
///   Returns (nm1, nm2, ga2_ppm, edge_count, node_count).
///   Let S(v) = Σ_{u∈N(v)} deg(u)  (neighbor-degree sum; "2nd-order degree").
///   NM₁(G) = Σ_v S(v)²                                       (Mondal et al. 2019)
///   NM₂(G) = Σ_{uv∈E} S(u)·S(v)                             (Mondal et al. 2019)
///   GA₂(G) × 10^6 = Σ_{uv∈E} floor(2√(S(u)·S(v))/(S(u)+S(v)) × 10^6)
/// GA₂ = |E| × 10^6 when all S(v) are equal (S-uniform: K_n, K_{r,s}, K_{1,k}).
/// Algorithm: O(V+E) — neighbor-degree-sum pass then edge scan; no BFS needed.
pub fn graph_topo_indices18() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices18_inner()
}

/// V3.30: `graph topo19` — Reverse Wiener Λ + Reciprocal Complementary Wiener RCW + Terminal Wiener TW.
///
///   Returns (rw, rcw_ppm, tw, edge_count, node_count).
///   Λ(G)   = Σ_c [C(n_c,2) × D_c − W_c]                     (exact u64; Randić et al. 2000)
///   RCW(G) × 10^6 = Σ_{u<v,conn} floor(10^6/(D_c+1−d(u,v))) (floor ppm; Vukičević 2010)
///   TW(G)  = Σ_{u<v, both pendant} d(u,v)                    (exact u64; Gutman et al. 2004)
/// Λ=0 iff all components have D=1 (complete blocks) or are singletons.
/// TW=0 iff fewer than 2 pendant nodes (deg=1) in the entire graph.
/// Algorithm: O(n(n+m)) — 2 BFS phases; component detection O(V+E).
pub fn graph_topo_indices19() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices19_inner()
}

/// V3.31: `graph topo20` — Sombor-family variants: SO* + RSO + rSO.
///
///   Returns (so_star_ppm, rso_ppm, rso_red_ppm, edge_count, node_count).
///   SO*(G)  × 10^6 = Σ_{uv∈E} d_u·d_v/√(d_u²+d_v²) × 10^6   (floor ppm; Ghanbari & Rajabi-Parsa 2021)
///   RSO(G)  × 10^6 = Σ_{uv∈E} 10^6/√(d_u²+d_v²)              (floor ppm; Gutman 2022)
///   rSO(G)  × 10^6 = Σ_{uv∈E} √((d_u−1)²+(d_v−1)²)×10^6     (floor ppm; Doslic et al. 2022)
/// rSO=0 iff all edges are pendant-pendant (d_u=d_v=1).
/// Algorithm: O(V+E) degree scan only — no BFS needed.
pub fn graph_topo_indices20() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices20_inner()
}

/// V3.32: `graph topo21` — ABC₄ + Neighborhood Harmonic NH + Neighborhood Sombor NSO.
///
///   Returns (abc4_ppm, nh_ppm, nso_ppm, edge_count, node_count).
///   All indices use S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant").
///   ABC₄(G) × 10^6 = Σ_{uv∈E} √((S_u+S_v−2)/(S_u·S_v)) × 10^6   (floor ppm; Ghorbani & Hosseinzadeh 2010)
///   NH(G)   × 10^6 = Σ_{uv∈E} 2/(S_u+S_v) × 10^6                  (floor ppm; S-analogue of Harmonic)
///   NSO(G)  × 10^6 = Σ_{uv∈E} √(S_u²+S_v²) × 10^6                 (floor ppm; S-analogue of Sombor)
/// ABC₄=0 when S_u+S_v=2 for all edges (only K₂: S=1).
/// S-uniform invariant: K₃ and K_{1,4} share S=4 everywhere → identical per-edge values.
/// Algorithm: O(V+E) S-scan — degree pass then edge pass; no BFS needed.
pub fn graph_topo_indices21() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices21_inner()
}

/// V3.33: `graph topo22` — NR + NF + NSC (Neighborhood Randić, Forgotten, Sum Connectivity).
///
///   Returns (nr_ppm, nf, nsc_ppm, edge_count, node_count).
///   All indices use S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant").
///   NR(G)  × 10^6 = Σ_{uv∈E} 1/√(S_u·S_v) × 10^6   (floor ppm; S-analogue of Randić R)
///   NF(G)          = Σ_v S(v)³                        (exact u64; S-analogue of Forgotten F)
///   NSC(G) × 10^6 = Σ_{uv∈E} 1/√(S_u+S_v) × 10^6   (floor ppm; S-analogue of Sum Connectivity SC)
/// NR=NSC for S=2 uniform graphs (S_u×S_v=S_u+S_v=4 when S=2).
/// For S-uniform graphs with S=c: NR=m×floor(10^6/c), NSC=m×isqrt64(10^12/(2c)).
/// Algorithm: O(V+E) S-scan — degree pass then S-pass then node+edge pass; no BFS needed.
pub fn graph_topo_indices22() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices22_inner()
}

/// V3.34: `graph topo23` — NHM1 + NSDD + NM3 (Neighborhood S-variant HM₁, SDD, M₃).
///
///   Returns (nhm1, nsdd_ppm, nm3, edge_count, node_count).
///   All indices use S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant").
///   NHM1(G) = Σ_{uv∈E} (S_u+S_v)²               (exact u64; S-analogue of HM₁, Shirdel et al. 2013)
///   NSDD(G) × 10^6 = Σ_{uv∈E} (S_u²+S_v²)/(S_u·S_v) × 10^6 (floor ppm; S-analogue of SDD; ≥2|E|×10^6)
///   NM3(G)  = Σ_{uv∈E} |S_u−S_v|                (exact u64; S-analogue of M₃ irregularity; =0 iff S-regular)
/// NSDD invariant: NSDD=2|E|×10^6 iff S-regular (AM-GM: (S²_u+S²_v)/(S_u·S_v)≥2; equality when S_u=S_v).
/// NM3 invariant: NM3=0 iff S-regular (all S values equal).
/// Algorithm: O(V+E) S-scan — degree pass then S-pass then edge scan; no BFS needed.
pub fn graph_topo_indices23() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices23_inner()
}

pub fn graph_topo_indices24() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices24_inner()
}

pub fn graph_topo_indices25() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices25_inner()
}

pub fn graph_topo_indices26() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices26_inner()
}

pub fn graph_topo_indices27() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices27_inner()
}

pub fn graph_topo_indices28() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices28_inner()
}

pub fn graph_topo_indices29() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices29_inner()
}

pub fn graph_topo_indices30() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices30_inner()
}

/// V3.42: `graph topo31` — NSig + NHQS + NPS (S-variant irregularity and power-sum family).
///
///   Returns (nsig, nhqs, nps, edge_count, node_count).
///   All indices use S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant").
///   NSig(G) = Σ_{uv∈E} (S_u−S_v)²                (exact u64; S-Sigma irregularity; =0 iff S-regular)
///   NHQS(G) = Σ_{uv∈E} (S_u+S_v)^4               (u128→u64; S-quartic edge-sum; extends NHCS Σ(S+S)³)
///   NPS(G)  = Σ_v S(v)^5                          (u128→u64; S-penta vertex sum; extends NVQ Σ S⁴)
/// NSig=0 iff S-regular (all neighbor-degree sums equal across each edge).
/// NHQS = n·(2S)^4 = 16n·S^4 for S-regular; NPS = n·S^5 for S-regular.
/// Algorithm: O(V+E) S-scan — degree pass then S-pass then vertex+edge scan; no BFS needed.
pub fn graph_topo_indices31() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices31_inner()
}

/// V3.43: `graph topo32` — NSH + NHPS + NWSO (S-variant power-series and weighted Sombor).
///
///   Returns (nsh, nhps, nwso_ppm, edge_count, node_count).
///   All indices use S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant").
///   NSH(G)  = Σ_v S(v)^6                            (u128→u64; S-hextic vertex sum; extends NPS Σ S⁵)
///   NHPS(G) = Σ_{uv∈E} (S_u+S_v)^5                 (u128→u64; S-quintic edge-sum; extends NHQS Σ(S+S)⁴)
///   NWSO(G) × 10^6 = Σ_{uv∈E} S_u·S_v·√(S_u²+S_v²)·10^6 (floor ppm; S-Weighted Sombor)
/// NSH = n·S^6 for S-regular; NHPS = |E|·(2S)^5 = 32|E|S^5 for S-regular.
/// NWSO = |E|·S³·√2·10^6 for S-regular (= S²·NSO_per_edge).
/// NSO(topo21) = Σ√(S²+S²); NWSO adds S_u·S_v weight: = Σ S_u·S_v·√(S_u²+S_v²).
/// Algorithm: O(V+E) S-scan — degree pass then S-pass then vertex+edge scan; no BFS needed.
pub fn graph_topo_indices32() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices32_inner()
}

/// Compute S-variant topo33 indices: NSHP + NHSE + NCSO.
///
/// Returns `(nshp, nhse, ncso_ppm, edge_count, node_count)`.
///   NSHP(G) = Σ_v S(v)^7                              (exact u64; S-heptic vertex sum)
///   NHSE(G) = Σ_{uv∈E} (S_u+S_v)^6                   (exact u64; S-sextic edge-sum)
///   NCSO(G) × 10^6 = Σ_{uv∈E} (S_u²+S_v²)^{3/2}·10^6 (floor ppm; S-Cubic Sombor)
/// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum.
/// NSHP extends NSH=Σ S⁶ (topo32) to 7th power; NHSE extends NHPS=Σ(S+S)⁵ to 6th power.
/// NCSO = S-variant of the generalized Sombor index SO^3 (α=3).
/// Algorithm: O(V+E) S-scan — degree pass then S-pass then vertex+edge scan; no BFS needed.
pub fn graph_topo_indices33() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices33_inner()
}

/// Compute S-variant topo34 indices: NOC + NHHS + NFSO.
///
/// Returns `(noc, nhhs, nfso, edge_count, node_count)`.
///   NOC(G)  = Σ_v S(v)^8                          (exact u64; S-octic vertex sum)
///   NHHS(G) = Σ_{uv∈E} (S_u+S_v)^7               (exact u64; S-septic/hepta edge-sum)
///   NFSO(G) = Σ_{uv∈E} (S_u²+S_v²)²              (exact u64; S-Fourth Sombor, α=4)
/// where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum.
/// NOC extends NSHP=Σ S^7 (topo33) to 8th power; NHHS extends NHSE=Σ(S+S)^6 to 7th power.
/// NFSO = S-variant of generalised Sombor SO^α with α=4: (S²+S²)^2 — exact integer, no isqrt.
/// All three use u128 accumulators for overflow safety; no isqrt required.
/// Algorithm: O(V+E) S-scan — degree pass then S-pass then vertex+edge scan; no BFS needed.
pub fn graph_topo_indices34() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices34_inner()
}

pub fn graph_topo_indices35() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices35_inner()
}

pub fn graph_topo_indices36() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices36_inner()
}

pub fn graph_topo_indices37() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices37_inner()
}

pub fn graph_topo_indices38() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices38_inner()
}

pub fn graph_topo_indices39() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices39_inner()
}

pub fn graph_topo_indices40() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices40_inner()
}

pub fn graph_topo_indices41() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices41_inner()
}

pub fn graph_topo_indices42() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices42_inner()
}

pub fn graph_topo_indices43() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices43_inner()
}

pub fn graph_topo_indices44() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices44_inner()
}

pub fn graph_topo_indices45() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices45_inner()
}

pub fn graph_topo_indices46() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices46_inner()
}

pub fn graph_topo_indices47() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices47_inner()
}

pub fn graph_topo_indices48() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices48_inner()
}

pub fn graph_topo_indices49() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices49_inner()
}

pub fn graph_topo_indices65() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices65_inner()
}

pub fn graph_topo_indices66() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices66_inner()
}

pub fn graph_topo_indices64() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices64_inner()
}

pub fn graph_topo_indices63() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices63_inner()
}

pub fn graph_topo_indices62() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices62_inner()
}

pub fn graph_topo_indices61() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices61_inner()
}

pub fn graph_topo_indices60() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices60_inner()
}

pub fn graph_topo_indices59() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices59_inner()
}

pub fn graph_topo_indices58() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices58_inner()
}

pub fn graph_topo_indices57() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices57_inner()
}

pub fn graph_topo_indices56() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices56_inner()
}

pub fn graph_topo_indices55() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices55_inner()
}

pub fn graph_topo_indices54() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices54_inner()
}

pub fn graph_topo_indices53() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices53_inner()
}

pub fn graph_topo_indices52() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices52_inner()
}

pub fn graph_topo_indices51() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices51_inner()
}

pub fn graph_topo_indices50() -> (u64, u64, u64, usize, usize) {
    RUNTIME.lock().graph_topo_indices50_inner()
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
