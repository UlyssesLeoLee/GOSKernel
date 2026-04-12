#![no_std]

//! GOS Protocol — Universal Node Graph Charter
//!
//! This crate defines the cross-plugin ABI, graph descriptors, and the
//! legacy compatibility layer used during the v0.2 runtime migration.

pub mod stem;
pub use stem::*;

pub mod trap;
pub use trap::{TrapFrame, TrapVector, TrapClass, HardwareEvent};

pub const KERNEL_BASE: u64 = 0xFFFF_8000_0000_0000;
pub const GOS_ABI_VERSION: u32 = 2;
pub const CONTROL_PLANE_PROTOCOL_VERSION: u16 = 1;

/// A 48-bit canonical vector address decomposed into graph coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VectorAddress {
    pub l4: u8,
    pub l3: u16,
    pub l2: u16,
    pub offset: u16,
}

impl VectorAddress {
    pub const fn new(l4: u8, l3: u16, l2: u16, offset: u16) -> Self {
        Self { l4, l3, l2, offset }
    }

    pub const fn from_u64(addr: u64) -> Self {
        Self {
            l4: ((addr >> 36) & 0xFF) as u8,
            l3: ((addr >> 24) & 0x0FFF) as u16,
            l2: ((addr >> 12) & 0x0FFF) as u16,
            offset: (addr & 0x0FFF) as u16,
        }
    }

    pub const fn as_u64(&self) -> u64 {
        KERNEL_BASE
            | ((self.l4 as u64) << 36)
            | ((self.l3 as u64) << 24)
            | ((self.l2 as u64) << 12)
            | (self.offset as u64)
    }

    pub fn as_ptr<T>(&self) -> *mut T {
        self.as_u64() as *mut T
    }

    pub fn parse(text: &str) -> Option<Self> {
        parse_vector_components(text).map(|(l4, l3, l2, offset)| Self::new(l4, l3, l2, offset))
    }
}

/// A stable synthetic vector for edges. It uses the same 48-bit canonical shape
/// as node vectors, but remains a distinct protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeVector {
    pub l4: u8,
    pub l3: u16,
    pub l2: u16,
    pub offset: u16,
}

impl EdgeVector {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);

    pub const fn new(l4: u8, l3: u16, l2: u16, offset: u16) -> Self {
        Self { l4, l3, l2, offset }
    }

    pub const fn from_u64(addr: u64) -> Self {
        Self {
            l4: ((addr >> 36) & 0xFF) as u8,
            l3: ((addr >> 24) & 0x0FFF) as u16,
            l2: ((addr >> 12) & 0x0FFF) as u16,
            offset: (addr & 0x0FFF) as u16,
        }
    }

    pub const fn as_u64(&self) -> u64 {
        KERNEL_BASE
            | ((self.l4 as u64) << 36)
            | ((self.l3 as u64) << 24)
            | ((self.l2 as u64) << 12)
            | (self.offset as u64)
    }

    pub fn parse(text: &str) -> Option<Self> {
        parse_vector_components(text).map(|(l4, l3, l2, offset)| Self::new(l4, l3, l2, offset))
    }
}

fn parse_vector_components(text: &str) -> Option<(u8, u16, u16, u16)> {
    if let Some(raw) = parse_canonical_hex(text) {
        return Some((
            ((raw >> 36) & 0xFF) as u8,
            ((raw >> 24) & 0x0FFF) as u16,
            ((raw >> 12) & 0x0FFF) as u16,
            (raw & 0x0FFF) as u16,
        ));
    }

    let mut parts = text.split('.');
    let l4 = parse_dec_component(parts.next()?)?;
    let l3 = parse_dec_component(parts.next()?)?;
    let l2 = parse_dec_component(parts.next()?)?;
    let offset = parse_dec_component(parts.next()?)?;
    if parts.next().is_some() || l4 > 0xFF || l3 > 0x0FFF || l2 > 0x0FFF || offset > 0x0FFF {
        return None;
    }
    Some((l4 as u8, l3 as u16, l2 as u16, offset as u16))
}

fn parse_canonical_hex(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    let (hex, explicit_prefix) = if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        (hex, true)
    } else {
        (trimmed, false)
    };
    if hex.len() < 2 || !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    if !explicit_prefix && hex.len() < 16 {
        return None;
    }
    u64::from_str_radix(hex, 16).ok()
}

fn parse_dec_component(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() || !trimmed.as_bytes().iter().all(u8::is_ascii_digit) {
        return None;
    }
    trimmed.parse::<u64>().ok()
}

pub const GOS_NODE_MAGIC: u32 = 0x474F5321;
pub const GOS_EDGE_MAGIC: u32 = 0x45444745;

#[repr(C)]
pub struct NodeHeader {
    pub magic: u32,
    pub uuid: [u8; 16],
    pub label: [u8; 16],
    pub name: [u8; 16],
    pub version: u32,
    pub acl: u64,
    pub cell_ptr: [u64; 2],
    pub _res: [u8; 176],
}

impl NodeHeader {
    pub const fn new(label: &str, name: &str) -> Self {
        let mut n = NodeHeader {
            magic: GOS_NODE_MAGIC,
            uuid: [0; 16],
            label: [0; 16],
            name: [0; 16],
            version: 1,
            acl: 0xFFFF,
            cell_ptr: [0; 2],
            _res: [0; 176],
        };

        let label_bytes = label.as_bytes();
        let name_bytes = name.as_bytes();

        let mut i = 0;
        while i < label_bytes.len() && i < 16 {
            n.label[i] = label_bytes[i];
            i += 1;
        }

        let mut j = 0;
        while j < name_bytes.len() && j < 16 {
            n.name[j] = name_bytes[j];
            j += 1;
        }

        n
    }
}

#[repr(C)]
pub struct EdgeHeader {
    pub magic: u32,
    pub type_name: [u8; 12],
    pub target_vec: u64,
    pub weight: f32,
    pub acl_mask: u64,
    pub _res: [u8; 28],
}

impl EdgeHeader {
    pub const fn open(type_name: &str, target_vec: u64) -> Self {
        let mut e = EdgeHeader {
            magic: GOS_EDGE_MAGIC,
            type_name: [0; 12],
            target_vec,
            weight: 1.0,
            acl_mask: u64::MAX,
            _res: [0; 28],
        };

        let tb = type_name.as_bytes();
        let mut i = 0;
        while i < tb.len() && i < 12 {
            e.type_name[i] = tb[i];
            i += 1;
        }

        e
    }

    #[inline]
    pub fn permits(&self, caller_vec: u64) -> bool {
        self.acl_mask == u64::MAX || (caller_vec & self.acl_mask) == self.target_vec
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeState {
    Unregistered = 0x00,
    Ready = 0x01,
    Running = 0x02,
    Suspended = 0x03,
    Terminated = 0xFF,
}

#[derive(Debug, Clone, Copy)]
pub enum Signal {
    Call { from: u64 },
    Spawn { payload: u64 },
    Interrupt { irq: u8 },
    Data { from: u64, byte: u8 },
    Control { cmd: u8, val: u8 },
    Terminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KernelSignalKind {
    Call = 0x01,
    Spawn = 0x02,
    Interrupt = 0x03,
    Data = 0x04,
    Control = 0x05,
    Terminate = 0xFF,
}

pub const AI_CONTROL_API_BEGIN: u8 = 0xA0;
pub const AI_CONTROL_API_COMMIT: u8 = 0xA1;
pub const AI_CONTROL_CHAT_BEGIN: u8 = 0xA2;
pub const AI_CONTROL_CHAT_COMMIT: u8 = 0xA3;
pub const CYPHER_CONTROL_QUERY_BEGIN: u8 = 0xA8;
pub const CYPHER_CONTROL_QUERY_COMMIT: u8 = 0xA9;
pub const IME_CONTROL_SET_MODE: u8 = 0xB0;
pub const IME_MODE_ASCII: u8 = 0x00;
pub const IME_MODE_ZH_PINYIN: u8 = 0x01;
pub const INPUT_KEY_PAGE_UP: u8 = 0xF1;
pub const INPUT_KEY_PAGE_DOWN: u8 = 0xF2;
pub const INPUT_KEY_UP: u8 = 0xF3;
pub const INPUT_KEY_DOWN: u8 = 0xF4;
pub const NET_CONTROL_REPORT: u8 = 0xD0;
pub const NET_CONTROL_PROBE: u8 = 0xD1;
pub const NET_CONTROL_RESET: u8 = 0xD2;
pub const CUDA_CONTROL_JOB_BEGIN: u8 = 0xE0;
pub const CUDA_CONTROL_JOB_COMMIT: u8 = 0xE1;
pub const CUDA_CONTROL_REPORT: u8 = 0xE2;
pub const CUDA_CONTROL_RESET: u8 = 0xE3;
pub const CLIPBOARD_DATA_BEGIN: u8 = 0xF8;
pub const CLIPBOARD_DATA_COMMIT: u8 = 0xF9;
pub const CLIPBOARD_DATA_CLEAR: u8 = 0xFA;
pub const DISPLAY_CONTROL_POINTER_COL: u8 = 0xC0;
pub const DISPLAY_CONTROL_POINTER_ROW: u8 = 0xC1;
pub const DISPLAY_CONTROL_POINTER_VISIBLE: u8 = 0xC2;
pub const DISPLAY_CONTROL_THEME: u8 = 0xC3;
pub const DISPLAY_THEME_WABI: u8 = 0x00;
pub const DISPLAY_THEME_SHOJI: u8 = 0x01;

#[derive(Debug, Clone, Copy)]
pub enum CellResult {
    Done,
    Yield,
    Fault(&'static str),
}

pub const MAX_CELL_EDGES: usize = 12;

#[derive(Clone, Copy)]
pub struct CellEdge {
    pub tag: [u8; 12],
    pub edge_type: u8,
    pub target_vec: u64,
}

impl CellEdge {
    pub const fn new(tag: &str, edge_type: u8, target_vec: u64) -> Self {
        let mut t = [0u8; 12];
        let tb = tag.as_bytes();
        let mut i = 0;
        while i < tb.len() && i < 12 {
            t[i] = tb[i];
            i += 1;
        }
        Self { tag: t, edge_type, target_vec }
    }

    pub const NONE: Self = Self { tag: [0; 12], edge_type: 0, target_vec: 0 };
}

#[derive(Clone, Copy)]
pub struct CellDeclaration {
    pub vec: VectorAddress,
    pub domain_label: &'static str,
    pub name: &'static str,
    pub edges: [CellEdge; MAX_CELL_EDGES],
    pub edge_count: usize,
    pub depends_on: &'static [u64],
}

/// The universal protocol every legacy GOS node must implement.
pub trait NodeCell: core::marker::Send {
    fn declare(&self) -> CellDeclaration;
    unsafe fn init(&mut self);
    fn on_activate(&mut self) -> CellResult;
    fn on_signal(&mut self, signal: Signal) -> CellResult;
    fn on_suspend(&mut self);
    fn state(&self) -> NodeState;
    fn vec(&self) -> VectorAddress;
}

pub struct BootContext {
    pub payload: u64,
}

impl BootContext {
    pub const fn new(payload: u64) -> Self {
        Self { payload }
    }
}

/// Legacy chain-boot compatibility trait.
pub trait PluginEntry {
    const VEC: VectorAddress;
    const WAVEFRONT: u32;
    fn plugin_main(ctx: &mut BootContext);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PluginId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EdgeId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ExecutorId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct VectorStorageKey(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ModuleId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeTemplateId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ResourceId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ModuleHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeInstanceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ClaimId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LeaseEpoch(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct DomainId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EndpointId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct CapabilityToken(pub u64);

impl PluginId {
    pub const ZERO: Self = Self([0; 16]);

    pub const fn new(raw: [u8; 16]) -> Self {
        Self(raw)
    }

    pub const fn from_ascii(name: &str) -> Self {
        Self(fixed_bytes_16(name))
    }
}

impl NodeId {
    pub const ZERO: Self = Self([0; 16]);
}

impl EdgeId {
    pub const ZERO: Self = Self([0; 16]);
}

impl ExecutorId {
    pub const ZERO: Self = Self([0; 16]);

    pub const fn from_ascii(name: &str) -> Self {
        Self(fixed_bytes_16(name))
    }
}

impl VectorStorageKey {
    pub const ZERO: Self = Self([0; 16]);
}

impl ModuleId {
    pub const ZERO: Self = Self([0; 16]);

    pub const fn new(raw: [u8; 16]) -> Self {
        Self(raw)
    }

    pub const fn from_ascii(name: &str) -> Self {
        Self(fixed_bytes_16(name))
    }
}

impl NodeTemplateId {
    pub const ZERO: Self = Self([0; 16]);

    pub const fn new(raw: [u8; 16]) -> Self {
        Self(raw)
    }

    pub const fn from_ascii(name: &str) -> Self {
        Self(fixed_bytes_16(name))
    }
}

impl ResourceId {
    pub const ZERO: Self = Self([0; 16]);

    pub const fn new(raw: [u8; 16]) -> Self {
        Self(raw)
    }

    pub const fn from_ascii(name: &str) -> Self {
        Self(fixed_bytes_16(name))
    }
}

impl ModuleHandle {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl NodeInstanceId {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl ClaimId {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl LeaseEpoch {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl DomainId {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl EndpointId {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl CapabilityToken {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

pub const fn fixed_bytes_16(value: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < 16 {
        out[i] = bytes[i];
        i += 1;
    }
    out
}

pub const fn derive_stable_id(namespace: &[u8], local_key: &[u8]) -> [u8; 16] {
    let mut hi: u64 = 0x6C62_272E_07BB_0142;
    let mut lo: u64 = 0x62B8_2175_6295_C58D;

    let mut idx = 0usize;
    while idx < namespace.len() {
        let byte = namespace[idx];
        hi ^= byte as u64;
        hi = hi.wrapping_mul(0x0000_0100_0000_01B3);
        lo ^= (byte as u64) << 1 | 1;
        lo = lo.wrapping_mul(0x0000_0100_0000_01B3);
        idx += 1;
    }

    idx = 0;
    while idx < local_key.len() {
        let byte = local_key[idx];
        hi ^= byte as u64;
        hi = hi.wrapping_mul(0x0000_0100_0000_01B3);
        lo ^= (byte as u64) << 1 | 1;
        lo = lo.wrapping_mul(0x0000_0100_0000_01B3);
        idx += 1;
    }

    let mut out = [0u8; 16];
    let hi_bytes = hi.to_be_bytes();
    let lo_bytes = lo.to_be_bytes();
    idx = 0;
    while idx < 8 {
        out[idx] = hi_bytes[idx];
        out[idx + 8] = lo_bytes[idx];
        idx += 1;
    }
    out
}

pub const fn derive_node_id(plugin_id: PluginId, local_node_key: &str) -> NodeId {
    NodeId(derive_stable_id(&plugin_id.0, local_node_key.as_bytes()))
}

pub const fn derive_edge_id(from: NodeId, to: NodeId, edge_key: &str) -> EdgeId {
    let mut seed = [0u8; 32];
    let mut idx = 0usize;
    while idx < 16 {
        seed[idx] = from.0[idx];
        seed[idx + 16] = to.0[idx];
        idx += 1;
    }
    EdgeId(derive_stable_id(&seed, edge_key.as_bytes()))
}

pub const fn derive_edge_vector(edge_id: EdgeId) -> EdgeVector {
    EdgeVector {
        l4: edge_id.0[0],
        l3: ((edge_id.0[1] as u16) << 4) | ((edge_id.0[2] as u16) >> 4),
        l2: (((edge_id.0[2] as u16) & 0x0F) << 8) | (edge_id.0[3] as u16),
        offset: ((edge_id.0[4] as u16) << 4) | ((edge_id.0[5] as u16) >> 4),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RuntimeNodeType {
    Hardware = 0x01,
    Driver = 0x02,
    Service = 0x03,
    PluginEntry = 0x10,
    Compute = 0x20,
    Router = 0x30,
    Aggregator = 0x40,
    Vector = 0xFF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RuntimeEdgeType {
    Call = 0x01,
    Spawn = 0x02,
    Depend = 0x03,
    Signal = 0x04,
    Return = 0x05,
    Mount = 0x06,
    Sync = 0x07,
    Stream = 0x08,
    Use = 0x09,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EntryPolicy {
    Manual = 0x00,
    Bootstrap = 0x01,
    OnDemand = 0x02,
    Background = 0x03,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RoutePolicy {
    Direct = 0x00,
    Weighted = 0x01,
    Broadcast = 0x02,
    FailFast = 0x03,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphEdgeDirection {
    Outbound = 0x01,
    Inbound = 0x02,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PermissionKind {
    PortIo = 0x01,
    IrqBind = 0x02,
    PhysMap = 0x03,
    GraphRead = 0x04,
    GraphWrite = 0x05,
    CapabilityExport = 0x06,
    CapabilityConsume = 0x07,
    ExternalSync = 0x08,
    ScheduleHint = 0x09,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeLifecycle {
    Discovered = 0x00,
    Loaded = 0x01,
    Registered = 0x02,
    Allocated = 0x03,
    Ready = 0x04,
    Running = 0x05,
    Waiting = 0x06,
    Suspended = 0x07,
    Terminated = 0x08,
    Faulted = 0xFF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlPlaneMessageKind {
    Hello = 0x01,
    PluginDiscovered = 0x02,
    NodeUpsert = 0x03,
    EdgeUpsert = 0x04,
    StateDelta = 0x05,
    SnapshotChunk = 0x06,
    Fault = 0x07,
    Metric = 0x08,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlPlaneHintKind {
    ScheduleHint = 0x01,
    EdgeReweightHint = 0x02,
    ActivateNode = 0x03,
    QuiesceNode = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionSpec {
    pub kind: PermissionKind,
    pub arg0: u64,
    pub arg1: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySpec {
    pub name: &'static str,
    pub namespace: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSpec {
    pub capability: &'static str,
    pub namespace: &'static str,
    pub required: bool,
}

pub const MODULE_ABI_VERSION: u32 = 1;

pub const RESOURCE_FRAME_ALLOC: ResourceId = ResourceId::from_ascii("RS.FRAME");
pub const RESOURCE_PAGE_MAPPER: ResourceId = ResourceId::from_ascii("RS.VMM");
pub const RESOURCE_DISPLAY_CONSOLE: ResourceId = ResourceId::from_ascii("RS.CONSOLE");
pub const RESOURCE_HEAP_SOURCE: ResourceId = ResourceId::from_ascii("RS.HEAP");
pub const RESOURCE_GPU_ACCEL: ResourceId = ResourceId::from_ascii("RS.GPU");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleImageFormat {
    Builtin = 0x01,
    ElfReloc = 0x02,
    ElfShared = 0x03,
    FlatBinary = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleLifecycle {
    Installed = 0x01,
    Validated = 0x02,
    Mapped = 0x03,
    Instantiated = 0x04,
    Running = 0x05,
    Quiescing = 0x06,
    Stopped = 0x07,
    Faulted = 0xFF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleFaultPolicy {
    FaultKernelDegraded = 0x01,
    Restart = 0x02,
    RestartAlways = 0x03,
    Manual = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpawnPolicy {
    Singleton = 0x01,
    OnDemand = 0x02,
    OnContention = 0x03,
    Replicated = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClaimPolicy {
    Shared = 0x01,
    Exclusive = 0x02,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PreemptPolicy {
    Never = 0x01,
    Try = 0x02,
    Force = 0x03,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HeapClass {
    Bootstrap = 0x01,
    Runtime = 0x02,
    Burst = 0x03,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionLaneClass {
    Control = 0x01,
    Io = 0x02,
    Compute = 0x03,
    Background = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeInstanceLifecycle {
    Allocated = 0x01,
    Ready = 0x02,
    Running = 0x03,
    WaitingClaim = 0x04,
    Suspended = 0x05,
    Stopped = 0x06,
    Faulted = 0xFF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleSegmentKind {
    Text = 0x01,
    Rodata = 0x02,
    Data = 0x03,
    Bss = 0x04,
    Stack = 0x05,
    Ipc = 0x06,
    Shared = 0x07,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleMessageKind {
    Data = 0x01,
    Event = 0x02,
    Control = 0x03,
    Interrupt = 0x04,
    CapabilityRevoked = 0x05,
    Fault = 0x06,
    Shutdown = 0x07,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ModuleCallStatus {
    Ok = 0,
    Retry = 1,
    Denied = -1,
    Unsupported = -2,
    Fault = -3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleDependencySpec {
    pub module_id: ModuleId,
    pub required: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapQuota {
    pub class: HeapClass,
    pub reserved_pages: u32,
    pub max_pages: u32,
    pub _reserved: u32,
}

impl HeapQuota {
    pub const EMPTY: Self = Self {
        class: HeapClass::Runtime,
        reserved_pages: 0,
        max_pages: 0,
        _reserved: 0,
    };

    pub const fn runtime(max_pages: u32) -> Self {
        Self {
            class: HeapClass::Runtime,
            reserved_pages: 0,
            max_pages,
            _reserved: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLease {
    pub claim_id: ClaimId,
    pub resource_id: ResourceId,
    pub instance_id: NodeInstanceId,
    pub epoch: LeaseEpoch,
    pub claim_policy: ClaimPolicy,
    pub preempt_policy: PreemptPolicy,
}

impl ResourceLease {
    pub const EMPTY: Self = Self {
        claim_id: ClaimId::ZERO,
        resource_id: ResourceId::ZERO,
        instance_id: NodeInstanceId::ZERO,
        epoch: LeaseEpoch::ZERO,
        claim_policy: ClaimPolicy::Exclusive,
        preempt_policy: PreemptPolicy::Never,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleImageSegment {
    pub kind: ModuleSegmentKind,
    pub virt_addr: u64,
    pub mem_len: u64,
    pub file_offset: u64,
    pub file_len: u64,
    pub flags: u64,
}

impl ModuleImageSegment {
    pub const EMPTY: Self = Self {
        kind: ModuleSegmentKind::Text,
        virt_addr: 0,
        mem_len: 0,
        file_offset: 0,
        file_len: 0,
        flags: 0,
    };
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleMessageHeader {
    pub kind: ModuleMessageKind,
    pub from: EndpointId,
    pub to: EndpointId,
    pub token: CapabilityToken,
    pub length: u16,
    pub _reserved: u16,
}

impl ModuleMessageHeader {
    pub const EMPTY: Self = Self {
        kind: ModuleMessageKind::Data,
        from: EndpointId::ZERO,
        to: EndpointId::ZERO,
        token: CapabilityToken::ZERO,
        length: 0,
        _reserved: 0,
    };
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleMessage {
    pub header: ModuleMessageHeader,
    pub payload: [u8; 48],
}

impl ModuleMessage {
    pub const EMPTY: Self = Self {
        header: ModuleMessageHeader::EMPTY,
        payload: [0; 48],
    };
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleAbiV1 {
    pub abi_version: u32,
    pub log: Option<unsafe extern "C" fn(module: ModuleHandle, level: u8, bytes: *const u8, len: usize) -> ModuleCallStatus>,
    pub send_message:
        Option<unsafe extern "C" fn(module: ModuleHandle, endpoint: EndpointId, message: *const ModuleMessage) -> ModuleCallStatus>,
    pub receive_message:
        Option<unsafe extern "C" fn(module: ModuleHandle, endpoint: EndpointId, out: *mut ModuleMessage) -> ModuleCallStatus>,
    pub resolve_capability:
        Option<unsafe extern "C" fn(module: ModuleHandle, namespace: *const u8, namespace_len: usize, name: *const u8, name_len: usize, out: *mut CapabilityToken) -> ModuleCallStatus>,
    pub open_endpoint:
        Option<unsafe extern "C" fn(module: ModuleHandle, label: *const u8, label_len: usize, out: *mut EndpointId) -> ModuleCallStatus>,
    pub request_pages:
        Option<unsafe extern "C" fn(module: ModuleHandle, page_count: usize, writable: u8, out_base: *mut u64) -> ModuleCallStatus>,
    pub free_pages:
        Option<unsafe extern "C" fn(module: ModuleHandle, base: u64, page_count: usize) -> ModuleCallStatus>,
    pub current_instance:
        Option<unsafe extern "C" fn(module: ModuleHandle, out: *mut NodeInstanceId) -> ModuleCallStatus>,
    pub claim_resource:
        Option<unsafe extern "C" fn(module: ModuleHandle, resource: ResourceId, claim_policy: ClaimPolicy, preempt_policy: PreemptPolicy, out_claim: *mut ClaimId, out_epoch: *mut LeaseEpoch) -> ModuleCallStatus>,
    pub release_claim:
        Option<unsafe extern "C" fn(module: ModuleHandle, claim: ClaimId) -> ModuleCallStatus>,
    pub subscribe_interrupt:
        Option<unsafe extern "C" fn(module: ModuleHandle, irq: u8, endpoint: EndpointId) -> ModuleCallStatus>,
    pub register_lifecycle:
        Option<unsafe extern "C" fn(module: ModuleHandle, endpoint: EndpointId) -> ModuleCallStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleEntry {
    pub module_init:
        Option<unsafe extern "C" fn(abi: *const ModuleAbiV1, handle: ModuleHandle, domain: DomainId) -> ModuleCallStatus>,
    pub module_start:
        Option<unsafe extern "C" fn(abi: *const ModuleAbiV1, handle: ModuleHandle, domain: DomainId) -> ModuleCallStatus>,
    pub module_stop:
        Option<unsafe extern "C" fn(abi: *const ModuleAbiV1, handle: ModuleHandle, domain: DomainId) -> ModuleCallStatus>,
    pub module_suspend:
        Option<unsafe extern "C" fn(abi: *const ModuleAbiV1, handle: ModuleHandle, domain: DomainId) -> ModuleCallStatus>,
    pub module_resume:
        Option<unsafe extern "C" fn(abi: *const ModuleAbiV1, handle: ModuleHandle, domain: DomainId) -> ModuleCallStatus>,
}

impl ModuleEntry {
    pub const NONE: Self = Self {
        module_init: None,
        module_start: None,
        module_stop: None,
        module_suspend: None,
        module_resume: None,
    };
}

#[derive(Clone, Copy)]
pub struct ModuleDescriptor {
    pub abi_version: u32,
    pub module_id: ModuleId,
    pub name: &'static str,
    pub version: u32,
    pub image_format: ModuleImageFormat,
    pub fault_policy: ModuleFaultPolicy,
    pub dependencies: &'static [ModuleDependencySpec],
    pub permissions: &'static [PermissionSpec],
    pub exports: &'static [CapabilitySpec],
    pub imports: &'static [ImportSpec],
    pub segments: &'static [ModuleImageSegment],
    pub entry: ModuleEntry,
    pub signature: Option<&'static [u8]>,
    pub flags: u64,
}

impl ModuleDescriptor {
    pub const fn empty(module_id: ModuleId, name: &'static str) -> Self {
        Self {
            abi_version: MODULE_ABI_VERSION,
            module_id,
            name,
            version: 1,
            image_format: ModuleImageFormat::Builtin,
            fault_policy: ModuleFaultPolicy::Manual,
            dependencies: &[],
            permissions: &[],
            exports: &[],
            imports: &[],
            segments: &[],
            entry: ModuleEntry::NONE,
            signature: None,
            flags: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VectorRef {
    pub storage_key: VectorStorageKey,
    pub hint_len: u8,
    pub hint: [f32; 4],
}

impl VectorRef {
    pub const NONE: Self = Self {
        storage_key: VectorStorageKey::ZERO,
        hint_len: 0,
        hint: [0.0; 4],
    };
}

#[derive(Debug, Clone, Copy)]
pub struct NodeSpec {
    pub node_id: NodeId,
    pub local_node_key: &'static str,
    pub node_type: RuntimeNodeType,
    pub entry_policy: EntryPolicy,
    pub executor_id: ExecutorId,
    pub state_schema_hash: u64,
    pub permissions: &'static [PermissionSpec],
    pub exports: &'static [CapabilitySpec],
    pub vector_ref: Option<VectorRef>,
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeSpec {
    pub edge_id: EdgeId,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub edge_type: RuntimeEdgeType,
    pub weight: f32,
    pub acl_mask: u64,
    pub route_policy: RoutePolicy,
    pub capability_namespace: Option<&'static str>,
    pub capability_binding: Option<&'static str>,
    pub vector_ref: Option<VectorRef>,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginManifest {
    pub abi_version: u32,
    pub plugin_id: PluginId,
    pub name: &'static str,
    pub version: u32,
    pub depends_on: &'static [PluginId],
    pub permissions: &'static [PermissionSpec],
    pub exports: &'static [CapabilitySpec],
    pub imports: &'static [ImportSpec],
    pub nodes: &'static [NodeSpec],
    pub edges: &'static [EdgeSpec],
    pub signature: Option<&'static [u8]>,
    pub policy_hash: [u8; 16],
}

impl PluginManifest {
    pub const fn empty(plugin_id: PluginId, name: &'static str) -> Self {
        Self {
            abi_version: GOS_ABI_VERSION,
            plugin_id,
            name,
            version: 1,
            depends_on: &[],
            permissions: &[],
            exports: &[],
            imports: &[],
            nodes: &[],
            edges: &[],
            signature: None,
            policy_hash: [0; 16],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KernelSignalPacket {
    pub tag: u8,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
}

impl KernelSignalPacket {
    pub const fn new(kind: KernelSignalKind, arg0: u64, arg1: u64, arg2: u64) -> Self {
        Self {
            tag: kind as u8,
            arg0,
            arg1,
            arg2,
        }
    }

    pub const fn terminate() -> Self {
        Self::new(KernelSignalKind::Terminate, 0, 0, 0)
    }
}

pub const fn signal_to_packet(signal: Signal) -> KernelSignalPacket {
    match signal {
        Signal::Call { from } => KernelSignalPacket::new(KernelSignalKind::Call, from, 0, 0),
        Signal::Spawn { payload } => KernelSignalPacket::new(KernelSignalKind::Spawn, payload, 0, 0),
        Signal::Interrupt { irq } => KernelSignalPacket::new(KernelSignalKind::Interrupt, irq as u64, 0, 0),
        Signal::Data { from, byte } => KernelSignalPacket::new(KernelSignalKind::Data, from, byte as u64, 0),
        Signal::Control { cmd, val } => KernelSignalPacket::new(KernelSignalKind::Control, cmd as u64, val as u64, 0),
        Signal::Terminate => KernelSignalPacket::terminate(),
    }
}

pub const fn packet_to_signal(packet: KernelSignalPacket) -> Signal {
    match packet.tag {
        x if x == KernelSignalKind::Call as u8 => Signal::Call { from: packet.arg0 },
        x if x == KernelSignalKind::Spawn as u8 => Signal::Spawn { payload: packet.arg0 },
        x if x == KernelSignalKind::Interrupt as u8 => Signal::Interrupt { irq: packet.arg0 as u8 },
        x if x == KernelSignalKind::Data as u8 => Signal::Data {
            from: packet.arg0,
            byte: packet.arg1 as u8,
        },
        x if x == KernelSignalKind::Control as u8 => Signal::Control {
            cmd: packet.arg0 as u8,
            val: packet.arg1 as u8,
        },
        _ => Signal::Terminate,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KernelAbi {
    pub abi_version: u32,
    pub log: Option<unsafe extern "C" fn(level: u8, bytes: *const u8, len: usize)>,
    pub alloc_pages: Option<unsafe extern "C" fn(page_count: usize) -> *mut u8>,
    pub free_pages: Option<unsafe extern "C" fn(ptr: *mut u8, page_count: usize)>,
    pub emit_signal:
        Option<unsafe extern "C" fn(target: u64, packet: KernelSignalPacket) -> i32>,
    pub resolve_capability:
        Option<unsafe extern "C" fn(namespace: *const u8, namespace_len: usize, name: *const u8, name_len: usize) -> u64>,
    pub emit_control_plane:
        Option<unsafe extern "C" fn(kind: u8, subject: *const u8, subject_len: usize, arg0: u64, arg1: u64)>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ExecutorContext {
    pub abi: *const KernelAbi,
    pub node_id: NodeId,
    pub vector: VectorAddress,
    pub state_ptr: *mut u8,
    pub state_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NodeEvent {
    pub edge_id: EdgeId,
    pub source_node: NodeId,
    pub signal: KernelSignalPacket,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecStatus {
    Done = 0,
    Yield = 1,
    Fault = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NodeExecutorVTable {
    pub executor_id: ExecutorId,
    pub on_init: Option<unsafe extern "C" fn(ctx: *mut ExecutorContext) -> ExecStatus>,
    pub on_event: Option<unsafe extern "C" fn(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus>,
    pub on_suspend: Option<unsafe extern "C" fn(ctx: *mut ExecutorContext) -> ExecStatus>,
    pub on_resume: Option<unsafe extern "C" fn(ctx: *mut ExecutorContext) -> ExecStatus>,
    pub on_teardown: Option<unsafe extern "C" fn(ctx: *mut ExecutorContext) -> ExecStatus>,
}

#[derive(Debug, Clone, Copy)]
pub struct ControlPlaneEnvelope {
    pub version: u16,
    pub kind: ControlPlaneMessageKind,
    pub subject: [u8; 16],
    pub arg0: u64,
    pub arg1: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct StateDelta {
    pub node_id: NodeId,
    pub state: NodeLifecycle,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphSnapshot {
    pub plugin_count: usize,
    pub node_count: usize,
    pub edge_count: usize,
    pub ready_queue_len: usize,
    pub signal_queue_len: usize,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphNodeSummary {
    pub vector: VectorAddress,
    pub node_id: NodeId,
    pub plugin_id: PluginId,
    pub plugin_name: &'static str,
    pub local_node_key: &'static str,
    pub node_type: RuntimeNodeType,
    pub lifecycle: NodeLifecycle,
    pub entry_policy: EntryPolicy,
    pub executor_id: ExecutorId,
    pub export_count: usize,
}

impl GraphNodeSummary {
    pub const EMPTY: Self = Self {
        vector: VectorAddress::new(0, 0, 0, 0),
        node_id: NodeId::ZERO,
        plugin_id: PluginId::ZERO,
        plugin_name: "",
        local_node_key: "",
        node_type: RuntimeNodeType::Hardware,
        lifecycle: NodeLifecycle::Discovered,
        entry_policy: EntryPolicy::Manual,
        executor_id: ExecutorId::ZERO,
        export_count: 0,
    };
}

#[derive(Debug, Clone, Copy)]
pub struct GraphEdgeSummary {
    pub edge_vector: EdgeVector,
    pub edge_id: EdgeId,
    pub direction: GraphEdgeDirection,
    pub from_vector: VectorAddress,
    pub from_key: &'static str,
    pub to_vector: VectorAddress,
    pub to_key: &'static str,
    pub edge_type: RuntimeEdgeType,
    pub route_policy: RoutePolicy,
    pub capability_namespace: Option<&'static str>,
    pub capability_binding: Option<&'static str>,
    pub weight: f32,
    pub acl_mask: u64,
}

impl GraphEdgeSummary {
    pub const EMPTY: Self = Self {
        edge_vector: EdgeVector::ZERO,
        edge_id: EdgeId::ZERO,
        direction: GraphEdgeDirection::Outbound,
        from_vector: VectorAddress::new(0, 0, 0, 0),
        from_key: "",
        to_vector: VectorAddress::new(0, 0, 0, 0),
        to_key: "",
        edge_type: RuntimeEdgeType::Signal,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        weight: 0.0,
        acl_mask: 0,
    };
}


#[derive(Debug, Clone, Copy)]
pub struct ControlPlaneHint {
    pub kind: ControlPlaneHintKind,
    pub subject: [u8; 16],
    pub arg0: u64,
    pub arg1: u64,
}
