#![no_std]

use core::mem::transmute;

use gos_protocol::{
    packet_to_signal, signal_to_packet, BootContext, CellDeclaration, CellResult,
    ControlPlaneEnvelope, ControlPlaneMessageKind, EdgeId, EdgeSpec, EdgeVector, ExecStatus,
    ExecutorContext, GOS_ABI_VERSION, GraphEdgeDirection, GraphEdgeSummary, GraphNodeSummary,
    GraphSnapshot, KernelAbi, KernelSignalPacket, NodeCell, NodeEvent, NodeExecutorVTable,
    NodeId, NodeLifecycle, NodeSpec, NodeState, PluginId, PluginManifest, RoutePolicy,
    RuntimeEdgeType, Signal, StateDelta, VectorAddress, derive_edge_vector,
    CONTROL_PLANE_PROTOCOL_VERSION,
};
use spin::Mutex;

pub const MAX_PLUGINS: usize = 32;
pub const MAX_NODES: usize = 64;
pub const MAX_EDGES: usize = 256;
pub const MAX_READY_QUEUE: usize = 128;
pub const MAX_SIGNAL_QUEUE: usize = 256;
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
}

pub struct GraphRuntime {
    plugins: [Option<PluginRecord>; MAX_PLUGINS],
    nodes: [Option<NodeRecord>; MAX_NODES],
    edges: [Option<EdgeRecord>; MAX_EDGES],
    ready_queue: RingQueue<NodeId, MAX_READY_QUEUE>,
    signal_queue: RingQueue<RuntimeSignal, MAX_SIGNAL_QUEUE>,
    call_frames: [Option<CallFrame>; MAX_CALL_FRAMES],
    wait_sets: [Option<WaitSet>; MAX_WAITSETS],
    barriers: [Option<Barrier>; MAX_BARRIERS],
    control_plane: RingQueue<ControlPlaneEnvelope, MAX_CONTROL_PLANE_MESSAGES>,
    node_arena: NodeArena,
    adjacency_arena: AdjacencyArena,
    tick: u64,
}

impl GraphRuntime {
    pub const fn new() -> Self {
        Self {
            plugins: [None; MAX_PLUGINS],
            nodes: [None; MAX_NODES],
            edges: [None; MAX_EDGES],
            ready_queue: RingQueue::new(),
            signal_queue: RingQueue::new(),
            call_frames: [None; MAX_CALL_FRAMES],
            wait_sets: [None; MAX_WAITSETS],
            barriers: [None; MAX_BARRIERS],
            control_plane: RingQueue::new(),
            node_arena: NodeArena::new(),
            adjacency_arena: AdjacencyArena::new(),
            tick: 0,
        }
    }

    fn emit_control_plane(
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
        });

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
        self.emit_control_plane(ControlPlaneMessageKind::EdgeUpsert, spec.edge_id.0, spec.from_node.0[0] as u64, spec.to_node.0[0] as u64);
        Ok(spec.edge_id)
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
        })
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
            RuntimeEdgeType::Spawn | RuntimeEdgeType::Signal | RuntimeEdgeType::Stream | RuntimeEdgeType::Mount => {
                let target_vec = self.node_vector(edge.to_node)?;
                self.post_signal(target_vec, signal)?;
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
        }
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
        ExecStatus::Done => NodeLifecycle::Ready,
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
    alloc_pages: None,
    free_pages: None,
    emit_signal: Some(kernel_emit_signal),
    resolve_capability: Some(kernel_resolve_capability),
    emit_control_plane: Some(kernel_emit_control_plane),
};

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

pub fn enqueue_ready(node_id: NodeId) -> Result<(), RuntimeError> {
    RUNTIME.lock().enqueue_ready(node_id)
}

pub fn post_signal(target: VectorAddress, signal: Signal) -> Result<(), RuntimeError> {
    RUNTIME.lock().post_signal(target, signal)
}

pub fn route_signal(target: VectorAddress, signal: Signal) -> Result<CellResult, RuntimeError> {
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
            let mut ctx = ExecutorContext {
                abi: &KERNEL_ABI,
                node_id: dispatch.node_id,
                vector: dispatch.vector,
                state_ptr,
                state_len: 4096,
            };

            let mut initialized = binding.initialized;
            let mut status = ExecStatus::Done;
            let terminated = matches!(signal, Signal::Terminate);

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
                        signal: signal_to_packet(signal),
                    };
                    unsafe { on_event(&mut ctx, &event) }
                } else {
                    ExecStatus::Done
                };
            }

            {
                let mut runtime = RUNTIME.lock();
                runtime.finish_native_invocation(dispatch.slot, status, initialized, terminated);
            }

            Ok(match status {
                ExecStatus::Done => CellResult::Done,
                ExecStatus::Yield => CellResult::Yield,
                ExecStatus::Fault => CellResult::Fault("native executor fault"),
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
            };

            let mut initialized = binding.initialized;
            let mut status = ExecStatus::Done;

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

            {
                let mut runtime = RUNTIME.lock();
                runtime.finish_native_invocation(dispatch.slot, status, initialized, false);
            }

            Ok(match status {
                ExecStatus::Done => CellResult::Done,
                ExecStatus::Yield => CellResult::Yield,
                ExecStatus::Fault => CellResult::Fault("native executor fault"),
            })
        }
        NodeBinding::Unbound => Err(RuntimeError::NativeExecutorMissing),
    }
}

pub fn route_edge(edge_id: EdgeId, signal: Signal) -> Result<(), RuntimeError> {
    RUNTIME.lock().route_edge(edge_id, signal)
}

pub fn pump() {
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

pub fn with_runtime<R>(f: impl FnOnce(&mut GraphRuntime) -> R) -> R {
    let mut runtime = RUNTIME.lock();
    f(&mut runtime)
}

pub fn bootstrap_context(payload: u64) -> BootContext {
    BootContext::new(payload)
}
