#![no_std]

use core::mem::transmute;
use core::sync::atomic::{AtomicU64, Ordering};

use gos_protocol::{
    packet_to_signal, signal_to_packet, BootContext, CellDeclaration, CellResult,
    ConditionalRoute, ControlPlaneEnvelope, ControlPlaneMessageKind, EdgeId, EdgeSpec,
    EdgeVector, ExecStatus, ExecutorContext, GOS_ABI_VERSION, GraphEdgeDirection,
    GraphEdgeSummary, GraphNodeSummary, GraphSnapshot, KernelAbi, KernelSignalPacket,
    MAX_CONDITIONAL_ROUTES, NodeCell, NodeEvent, NodeExecutorVTable, NodeId, NodeInstanceId,
    NodeLifecycle, NodeSpec, NodeState, NodeTelemetry, PluginId, PluginManifest, RoutePolicy,
    RuntimeEdgeType, Signal, StateDelta, VectorAddress, derive_edge_vector,
    CONTROL_PLANE_PROTOCOL_VERSION,
};
use spin::Mutex;

pub const MAX_PLUGINS: usize = 32;
pub const MAX_NODES: usize = 128;
pub const MAX_EDGES: usize = 512;
pub const MAX_READY_QUEUE: usize = 256;
pub const MAX_SIGNAL_QUEUE: usize = 512;
pub const MAX_FAULT_QUEUE: usize = 32;
pub const MAX_CALL_FRAMES: usize = 64;
pub const MAX_WAITSETS: usize = 64;
pub const MAX_BARRIERS: usize = 32;
pub const MAX_CONTROL_PLANE_MESSAGES: usize = 256;
pub const NODE_ARENA_PAGES: usize = 64;

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
    tick: u64,
    /// Incremental counters — updated on insert/remove to avoid O(n)
    /// scans in the hot `snapshot()` path.
    plugin_count: usize,
    node_count: usize,
    edge_count: usize,
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
            tick: 0,
            plugin_count: 0,
            node_count: 0,
            edge_count: 0,
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

    fn edge_summary_from_slot(
        &self,
        slot: usize,
        direction: GraphEdgeDirection,
    ) -> Option<GraphEdgeSummary> {
        let record = self.edges.get(slot).and_then(|slot| *slot)?;
        let from_slot = self.node_slot_by_id(record.spec.from_node)?;
        let to_slot = self.node_slot_by_id(record.spec.to_node)?;
        let from = self.nodes[from_slot]?;
        let to = self.nodes[to_slot]?;
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
    }

    pub fn reset(&mut self) {
        *self = Self::new();
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
                self.plugin_count += 1;
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
        });
        self.node_count += 1;

        self.emit_control_plane(ControlPlaneMessageKind::NodeUpsert, spec.node_id.0, vector.as_u64(), runtime_page as u64);
        self.state_delta(spec.node_id, NodeLifecycle::Registered);
        self.state_delta(spec.node_id, NodeLifecycle::Allocated);
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
        self.edge_count += 1;
        self.emit_control_plane(ControlPlaneMessageKind::EdgeUpsert, spec.edge_id.0, spec.from_node.0[0] as u64, spec.to_node.0[0] as u64);
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
        self.edges[slot] = None;
        self.edge_count = self.edge_count.saturating_sub(1);
        self.adjacency_arena.release(edge_id);
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

    pub fn node_page<const N: usize>(
        &self,
        offset: usize,
        out: &mut [GraphNodeSummary; N],
    ) -> (usize, usize) {
        let mut slots = [usize::MAX; MAX_NODES];
        let mut total = 0usize;
        for (idx, slot) in self.nodes.iter().enumerate() {
            if slot.is_some() {
                slots[total] = idx;
                total += 1;
            }
        }

        let mut i = 1usize;
        while i < total {
            let current = slots[i];
            let mut j = i;
            while j > 0
                && self.nodes[slots[j - 1]].unwrap().vector.as_u64()
                    > self.nodes[current].unwrap().vector.as_u64()
            {
                slots[j] = slots[j - 1];
                j -= 1;
            }
            slots[j] = current;
            i += 1;
        }

        let mut returned = 0usize;
        let mut cursor = offset.min(total);
        while cursor < total && returned < N {
            if let Some(summary) = self.node_summary_from_slot(slots[cursor]) {
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
        let mut total = 0usize;
        for (idx, edge) in self.edges.iter().enumerate() {
            let Some(edge) = edge else {
                continue;
            };
            if edge.spec.from_node == node_id {
                slots[total] = (idx, GraphEdgeDirection::Outbound);
                total += 1;
            } else if edge.spec.to_node == node_id {
                slots[total] = (idx, GraphEdgeDirection::Inbound);
                total += 1;
            }
        }

        let mut i = 1usize;
        while i < total {
            let current = slots[i];
            let mut j = i;
            while j > 0
                && self.edges[slots[j - 1].0].unwrap().edge_vector.as_u64()
                    > self.edges[current.0].unwrap().edge_vector.as_u64()
            {
                slots[j] = slots[j - 1];
                j -= 1;
            }
            slots[j] = current;
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
        &self,
        offset: usize,
        out: &mut [GraphEdgeSummary; N],
    ) -> (usize, usize) {
        let mut slots = [usize::MAX; MAX_EDGES];
        let mut total = 0usize;
        for (idx, edge) in self.edges.iter().enumerate() {
            if edge.is_some() {
                slots[total] = idx;
                total += 1;
            }
        }

        let mut i = 1usize;
        while i < total {
            let current = slots[i];
            let mut j = i;
            while j > 0
                && self.edges[slots[j - 1]].unwrap().edge_vector.as_u64()
                    > self.edges[current].unwrap().edge_vector.as_u64()
            {
                slots[j] = slots[j - 1];
                j -= 1;
            }
            slots[j] = current;
            i += 1;
        }

        let mut returned = 0usize;
        let mut cursor = offset.min(total);
        while cursor < total && returned < N {
            if let Some(summary) =
                self.edge_summary_from_slot(slots[cursor], GraphEdgeDirection::Outbound)
            {
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
    ) -> Result<PreparedDispatch, RuntimeError> {
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
                for i in 0..target_count {
                    let _ = self.post_signal(targets[i], signal);
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
                FAULT_DISPATCH_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn drain_next_fault(&mut self) -> Option<VectorAddress> {
        self.fault_queue.pop()
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
            plugin_count: self.plugin_count,
            node_count: self.node_count,
            edge_count: self.edge_count,
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
/// Incremented once per PIT interrupt (120 Hz) by `tick_pulse()`.
/// Use for wall-clock elapsed time: `pit_tick_count() / 120` = seconds.
static PIT_TICK_COUNT: AtomicU64 = AtomicU64::new(0);
/// Total signals routed since boot (both legacy and native).
static SIGNAL_DISPATCH_COUNT: AtomicU64 = AtomicU64::new(0);
/// Total native-executor activations (Spawn + subsequent events).
static ACTIVATION_COUNT: AtomicU64 = AtomicU64::new(0);
/// Total faults dispatched to the fault queue.
static FAULT_DISPATCH_COUNT: AtomicU64 = AtomicU64::new(0);

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
    PIT_TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    let hook = *SCHEDULER.lock();
    if let Some(hook) = hook {
        unsafe { (hook.on_tick)() };
    }
}

/// Returns the number of PIT interrupts since boot (120 Hz).
/// Divide by 120 for wall-clock seconds.
pub fn pit_tick_count() -> u64 {
    PIT_TICK_COUNT.load(Ordering::Relaxed)
}

/// Total signals routed since boot via `route_signal`.
pub fn signal_dispatch_count() -> u64 {
    SIGNAL_DISPATCH_COUNT.load(Ordering::Relaxed)
}

/// Total node activations since boot via `activate`.
pub fn activation_count() -> u64 {
    ACTIVATION_COUNT.load(Ordering::Relaxed)
}

/// Total fault dispatches pushed to the fault queue since boot.
pub fn fault_dispatch_count() -> u64 {
    FAULT_DISPATCH_COUNT.load(Ordering::Relaxed)
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

static RUNTIME: Mutex<GraphRuntime> = Mutex::new(GraphRuntime::new());

// ── Phase E.3: syscall surface wrappers ─────────────────────────────────────
//
// These thin public wrappers expose the same kernel-ABI functions that
// native plugins call through ExecutorContext::abi, but without requiring
// a context pointer.  The Ring 3 syscall trampoline (hypervisor::ring3)
// calls them after decoding syscall arguments from registers.
//
// Packet encoding for EmitSignal:
//   packet_lo bits [63:56] = KernelSignalKind tag
//   packet_lo bits [55: 0] = arg0 (VectorAddress / payload; 56-bit cap)
//   packet_hi               = arg1

pub unsafe fn syscall_alloc_pages(page_count: usize) -> *mut u8 {
    kernel_alloc_pages(page_count)
}

pub unsafe fn syscall_free_pages(ptr: *mut u8, page_count: usize) {
    kernel_free_pages(ptr, page_count)
}

pub unsafe fn syscall_emit_signal(target: u64, packet_lo: u64, packet_hi: u64) -> i32 {
    let tag = (packet_lo >> 56) as u8;
    let arg0 = packet_lo & 0x00FF_FFFF_FFFF_FFFF;
    let packet = KernelSignalPacket { tag, arg0, arg1: packet_hi, arg2: 0 };
    kernel_emit_signal(target, packet)
}

pub unsafe fn syscall_resolve_capability(
    ns: *const u8,
    ns_len: usize,
    name: *const u8,
    name_len: usize,
) -> u64 {
    kernel_resolve_capability(ns, ns_len, name, name_len)
}

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
    SIGNAL_DISPATCH_COUNT.fetch_add(1, Ordering::Relaxed);
    let dispatch = {
        let mut runtime = RUNTIME.lock();
        runtime.prepare_signal_dispatch(target)?
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
    ACTIVATION_COUNT.fetch_add(1, Ordering::Relaxed);
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
    const MAX_WORK_ITEMS_PER_PUMP: u32 = 4096;
    let mut processed: u32 = 0;
    loop {
        let work = {
            let mut runtime = RUNTIME.lock();
            runtime.next_work_item()
        };

        let Some(work) = work else {
            break;
        };

        match work {
            WorkItem::Ready(node_id) => {
                let vector = {
                    let runtime = RUNTIME.lock();
                    runtime.node_vector(node_id)
                };
                if let Ok(vector) = vector {
                    let _ = activate(vector);
                }
            }
            WorkItem::Signal(signal) => {
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

pub fn bootstrap_context(payload: u64) -> BootContext {
    BootContext::new(payload)
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
