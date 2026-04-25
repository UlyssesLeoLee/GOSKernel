#![cfg_attr(not(any(test, feature = "host-testing")), no_std)]

use core::sync::atomic::{AtomicPtr, Ordering};
#[cfg(any(test, feature = "host-testing"))]
use core::sync::atomic::AtomicU64;
#[cfg(test)]
use core::sync::atomic::AtomicUsize;

use gos_protocol::{
    fixed_bytes_16, CapabilitySpec, CapabilityToken, ClaimId, ClaimPolicy, DomainId,
    EndpointId, ExecutionLaneClass, HeapQuota, ImportSpec, LeaseEpoch, ModuleAbiV1,
    ModuleCallStatus, ModuleDescriptor, ModuleEntry, ModuleFaultPolicy, ModuleHandle,
    ModuleId, ModuleLifecycle, ModuleMessage, NodeInstanceId, NodeInstanceLifecycle,
    NodeTemplateId, PermissionSpec, PreemptPolicy, ResourceId, ResourceLease,
    SpawnPolicy, RESOURCE_DISPLAY_CONSOLE, RESOURCE_FRAME_ALLOC, RESOURCE_GPU_ACCEL,
    RESOURCE_HEAP_SOURCE, RESOURCE_PAGE_MAPPER, MODULE_ABI_VERSION,
};
#[cfg(all(feature = "kernel-vmm", not(any(test, feature = "host-testing"))))]
use k_vmm;
use spin::Mutex;

pub const MAX_MODULES: usize = 32;
pub const MAX_TEMPLATES: usize = 64;
pub const MAX_INSTANCES: usize = 128;
pub const MAX_RESOURCES: usize = 32;
pub const MAX_CLAIMS: usize = 128;
pub const MAX_REVOKES: usize = 128;
pub const MAX_HEAP_GRANTS: usize = 256;
pub const MAX_CAPABILITIES: usize = 128;
pub const MAX_ENDPOINTS: usize = 128;
pub const MAX_SUBSCRIPTIONS: usize = 128;
pub const MAX_QUEUED_MESSAGES: usize = 256;

const DOMAIN_BASE: u64 = 0xFFFF_9000_0000_0000;
const DOMAIN_STRIDE: u64 = 0x0000_0000_0200_0000;
const DEFAULT_IMAGE_WINDOW: u64 = 0x0000_0000_0010_0000;
const DEFAULT_STACK_WINDOW: u64 = 0x0000_0000_0002_0000;
const DEFAULT_IPC_WINDOW: u64 = 0x0000_0000_0002_0000;
const DEFAULT_HEAP_WINDOW: u64 = 0x0000_0000_0100_0000;
const PAGE_BYTES: u64 = 4096;
const DEFAULT_HEAP_QUOTA: HeapQuota = HeapQuota {
    class: gos_protocol::HeapClass::Runtime,
    reserved_pages: 0,
    max_pages: 32,
    _reserved: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorError {
    NotBootstrapped,
    ModuleTableFull,
    TemplateTableFull,
    InstanceTableFull,
    ResourceTableFull,
    ClaimTableFull,
    RevokeTableFull,
    HeapGrantTableFull,
    CapabilityTableFull,
    EndpointTableFull,
    SubscriptionTableFull,
    QueueFull,
    ModuleNotFound,
    TemplateNotFound,
    InstanceNotFound,
    ResourceNotFound,
    ClaimNotFound,
    HeapGrantNotFound,
    EndpointNotFound,
    CapabilityNotFound,
    ResourceBusy,
    HeapQuotaExceeded,
    NoActiveInstance,
    InvalidState,
    ModuleRejected,
    DomainCreateFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorSnapshot {
    pub installed_modules: usize,
    pub registered_templates: usize,
    pub live_instances: usize,
    pub ready_instances: usize,
    pub waiting_instances: usize,
    pub suspended_instances: usize,
    pub registered_resources: usize,
    pub active_claims: usize,
    pub pending_revocations: usize,
    pub queued_restarts: usize,
    pub running_modules: usize,
    pub isolated_domains: usize,
    pub heap_grants: usize,
    pub heap_pages_used: usize,
    pub published_capabilities: usize,
    pub endpoints: usize,
    pub queued_messages: usize,
    pub ready_control: usize,
    pub ready_io: usize,
    pub ready_compute: usize,
    pub ready_background: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorBootReport {
    pub discovered_modules: usize,
    pub running_modules: usize,
    pub isolated_domains: usize,
    pub published_capabilities: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleDomain {
    pub id: DomainId,
    pub root_table_phys: u64,
    pub image_base: u64,
    pub image_len: u64,
    pub stack_base: u64,
    pub stack_len: u64,
    pub ipc_base: u64,
    pub ipc_len: u64,
    pub heap_base: u64,
    pub heap_len: u64,
    pub isolated: bool,
}

impl ModuleDomain {
    pub const EMPTY: Self = Self {
        id: DomainId::ZERO,
        root_table_phys: 0,
        image_base: 0,
        image_len: 0,
        stack_base: 0,
        stack_len: 0,
        ipc_base: 0,
        ipc_len: 0,
        heap_base: 0,
        heap_len: 0,
        isolated: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeTemplateSummary {
    pub template_id: NodeTemplateId,
    pub module: ModuleHandle,
    pub spawn_policy: SpawnPolicy,
    pub lane: ExecutionLaneClass,
    pub heap_quota: HeapQuota,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeInstanceSummary {
    pub instance_id: NodeInstanceId,
    pub template_id: NodeTemplateId,
    pub module: ModuleHandle,
    pub lane: ExecutionLaneClass,
    pub lifecycle: NodeInstanceLifecycle,
    pub ready_queued: bool,
    pub heap_quota: HeapQuota,
    pub heap_pages_used: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimSummary {
    pub claim_id: ClaimId,
    pub resource_id: ResourceId,
    pub instance_id: NodeInstanceId,
    pub module: ModuleHandle,
    pub claim_policy: ClaimPolicy,
    pub preempt_policy: PreemptPolicy,
    pub epoch: LeaseEpoch,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapGrantSummary {
    pub module: ModuleHandle,
    pub instance_id: NodeInstanceId,
    pub base: u64,
    pub page_count: usize,
    pub writable: bool,
}

#[derive(Clone, Copy)]
enum ModuleSource {
    Empty,
    Descriptor(ModuleDescriptor),
}

impl ModuleSource {
    fn module_id(&self) -> ModuleId {
        match *self {
            Self::Descriptor(descriptor) => descriptor.module_id,
            Self::Empty => ModuleId::ZERO,
        }
    }

    fn fault_policy(&self) -> ModuleFaultPolicy {
        match *self {
            Self::Descriptor(descriptor) => descriptor.fault_policy,
            Self::Empty => ModuleFaultPolicy::Manual,
        }
    }

    fn permissions(&self) -> &'static [PermissionSpec] {
        match *self {
            Self::Descriptor(descriptor) => descriptor.permissions,
            Self::Empty => &[],
        }
    }

    fn exports(&self) -> &'static [CapabilitySpec] {
        match *self {
            Self::Descriptor(descriptor) => descriptor.exports,
            Self::Empty => &[],
        }
    }

    fn imports(&self) -> &'static [ImportSpec] {
        match *self {
            Self::Descriptor(descriptor) => descriptor.imports,
            Self::Empty => &[],
        }
    }

    fn entry(&self) -> ModuleEntry {
        match *self {
            Self::Descriptor(descriptor) => descriptor.entry,
            Self::Empty => ModuleEntry::NONE,
        }
    }

    fn image_len(&self) -> u64 {
        match *self {
            Self::Descriptor(descriptor) => {
                let mut max_len = 0u64;
                let mut idx = 0usize;
                while idx < descriptor.segments.len() {
                    let segment = descriptor.segments[idx];
                    let end = segment.virt_addr.saturating_add(segment.mem_len);
                    if end > max_len {
                        max_len = end;
                    }
                    idx += 1;
                }
                if max_len == 0 {
                    DEFAULT_IMAGE_WINDOW
                } else {
                    max_len
                }
            }
            Self::Empty => DEFAULT_IMAGE_WINDOW,
        }
    }
}

#[cfg(all(feature = "kernel-vmm", not(any(test, feature = "host-testing"))))]
fn create_domain_root(
    image_base: u64,
    image_len: u64,
    stack_base: u64,
    stack_len: u64,
    ipc_base: u64,
    ipc_len: u64,
) -> Result<u64, SupervisorError> {
    unsafe {
        k_vmm::create_isolated_address_space(
            image_base,
            image_len,
            stack_base,
            stack_len,
            ipc_base,
            ipc_len,
        )
        .map_err(|_| SupervisorError::DomainCreateFailed)
    }
}

#[cfg(any(test, feature = "host-testing"))]
fn create_domain_root(
    _image_base: u64,
    _image_len: u64,
    _stack_base: u64,
    _stack_len: u64,
    _ipc_base: u64,
    _ipc_len: u64,
) -> Result<u64, SupervisorError> {
    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0x1000);
    Ok(NEXT_ROOT.fetch_add(0x1000, Ordering::SeqCst))
}

#[cfg(all(feature = "kernel-vmm", not(any(test, feature = "host-testing"))))]
fn map_domain_heap_pages(
    root_table_phys: u64,
    base: u64,
    page_count: usize,
    writable: bool,
) -> Result<(), SupervisorError> {
    let mut flags = x86_64::structures::paging::PageTableFlags::PRESENT;
    if writable {
        flags |= x86_64::structures::paging::PageTableFlags::WRITABLE;
    }
    flags |= x86_64::structures::paging::PageTableFlags::NO_EXECUTE;
    unsafe {
        k_vmm::map_anonymous_window(root_table_phys, base, page_count as u64 * PAGE_BYTES, flags)
            .map_err(|_| SupervisorError::DomainCreateFailed)
    }
}

#[cfg(any(test, feature = "host-testing"))]
fn map_domain_heap_pages(
    _root_table_phys: u64,
    _base: u64,
    _page_count: usize,
    _writable: bool,
) -> Result<(), SupervisorError> {
    Ok(())
}

#[derive(Clone, Copy)]
struct ModuleRecord {
    occupied: bool,
    handle: ModuleHandle,
    source: ModuleSource,
    state: ModuleLifecycle,
    domain: ModuleDomain,
    queued_restart: bool,
    restart_generation: u32,
}

impl ModuleRecord {
    const EMPTY: Self = Self {
        occupied: false,
        handle: ModuleHandle::ZERO,
        source: ModuleSource::Empty,
        state: ModuleLifecycle::Stopped,
        domain: ModuleDomain::EMPTY,
        queued_restart: false,
        restart_generation: 0,
    };
}

#[derive(Clone, Copy)]
struct TemplateRecord {
    occupied: bool,
    id: NodeTemplateId,
    module: ModuleHandle,
    spawn_policy: SpawnPolicy,
    lane: ExecutionLaneClass,
    heap_quota: HeapQuota,
}

impl TemplateRecord {
    const EMPTY: Self = Self {
        occupied: false,
        id: NodeTemplateId::ZERO,
        module: ModuleHandle::ZERO,
        spawn_policy: SpawnPolicy::OnContention,
        lane: ExecutionLaneClass::Background,
        heap_quota: HeapQuota::EMPTY,
    };
}

#[derive(Clone, Copy)]
struct InstanceRecord {
    occupied: bool,
    id: NodeInstanceId,
    template_id: NodeTemplateId,
    module: ModuleHandle,
    lane: ExecutionLaneClass,
    lifecycle: NodeInstanceLifecycle,
    ready_queued: bool,
    heap_quota: HeapQuota,
    heap_pages_used: u32,
    heap_cursor_pages: u32,
}

impl InstanceRecord {
    const EMPTY: Self = Self {
        occupied: false,
        id: NodeInstanceId::ZERO,
        template_id: NodeTemplateId::ZERO,
        module: ModuleHandle::ZERO,
        lane: ExecutionLaneClass::Background,
        lifecycle: NodeInstanceLifecycle::Stopped,
        ready_queued: false,
        heap_quota: HeapQuota::EMPTY,
        heap_pages_used: 0,
        heap_cursor_pages: 0,
    };
}

#[derive(Clone, Copy)]
struct ResourceRecord {
    occupied: bool,
    id: ResourceId,
    current_epoch: LeaseEpoch,
}

impl ResourceRecord {
    const EMPTY: Self = Self {
        occupied: false,
        id: ResourceId::ZERO,
        current_epoch: LeaseEpoch::ZERO,
    };
}

#[derive(Clone, Copy)]
struct ClaimRecord {
    occupied: bool,
    id: ClaimId,
    resource: ResourceId,
    instance: NodeInstanceId,
    module: ModuleHandle,
    claim_policy: ClaimPolicy,
    preempt_policy: PreemptPolicy,
    epoch: LeaseEpoch,
    active: bool,
}

impl ClaimRecord {
    const EMPTY: Self = Self {
        occupied: false,
        id: ClaimId::ZERO,
        resource: ResourceId::ZERO,
        instance: NodeInstanceId::ZERO,
        module: ModuleHandle::ZERO,
        claim_policy: ClaimPolicy::Exclusive,
        preempt_policy: PreemptPolicy::Never,
        epoch: LeaseEpoch::ZERO,
        active: false,
    };
}

#[derive(Clone, Copy)]
struct RevocationRecord {
    occupied: bool,
    instance: NodeInstanceId,
    lease: ResourceLease,
}

impl RevocationRecord {
    const EMPTY: Self = Self {
        occupied: false,
        instance: NodeInstanceId::ZERO,
        lease: ResourceLease::EMPTY,
    };
}

#[derive(Clone, Copy)]
struct HeapGrantRecord {
    occupied: bool,
    module: ModuleHandle,
    instance: NodeInstanceId,
    base: u64,
    page_count: usize,
    writable: bool,
}

impl HeapGrantRecord {
    const EMPTY: Self = Self {
        occupied: false,
        module: ModuleHandle::ZERO,
        instance: NodeInstanceId::ZERO,
        base: 0,
        page_count: 0,
        writable: false,
    };
}

#[derive(Clone, Copy)]
struct CapabilityRecord {
    occupied: bool,
    token: CapabilityToken,
    provider: ModuleHandle,
    endpoint: EndpointId,
    spec: CapabilitySpec,
}

impl CapabilityRecord {
    const EMPTY: Self = Self {
        occupied: false,
        token: CapabilityToken::ZERO,
        provider: ModuleHandle::ZERO,
        endpoint: EndpointId::ZERO,
        spec: CapabilitySpec {
            namespace: "",
            name: "",
        },
    };
}

#[derive(Clone, Copy)]
struct EndpointRecord {
    occupied: bool,
    id: EndpointId,
    owner: ModuleHandle,
    capability: CapabilityToken,
}

impl EndpointRecord {
    const EMPTY: Self = Self {
        occupied: false,
        id: EndpointId::ZERO,
        owner: ModuleHandle::ZERO,
        capability: CapabilityToken::ZERO,
    };
}

#[derive(Clone, Copy)]
struct SubscriptionRecord {
    occupied: bool,
    consumer_endpoint: EndpointId,
    provider_endpoint: EndpointId,
}

impl SubscriptionRecord {
    const EMPTY: Self = Self {
        occupied: false,
        consumer_endpoint: EndpointId::ZERO,
        provider_endpoint: EndpointId::ZERO,
    };
}

#[derive(Clone, Copy)]
struct QueuedMessage {
    occupied: bool,
    recipient: ModuleHandle,
    endpoint: EndpointId,
    message: ModuleMessage,
}

impl QueuedMessage {
    const EMPTY: Self = Self {
        occupied: false,
        recipient: ModuleHandle::ZERO,
        endpoint: EndpointId::ZERO,
        message: ModuleMessage::EMPTY,
    };
}

#[derive(Clone, Copy)]
struct InstanceQueue<const N: usize> {
    entries: [NodeInstanceId; N],
    head: usize,
    tail: usize,
    len: usize,
}

impl<const N: usize> InstanceQueue<N> {
    const fn new() -> Self {
        Self {
            entries: [NodeInstanceId::ZERO; N],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    fn push(&mut self, instance_id: NodeInstanceId) -> Result<(), SupervisorError> {
        if self.len == N {
            return Err(SupervisorError::QueueFull);
        }
        self.entries[self.head] = instance_id;
        self.head = (self.head + 1) % N;
        self.len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<NodeInstanceId> {
        if self.len == 0 {
            return None;
        }
        let instance_id = self.entries[self.tail];
        self.entries[self.tail] = NodeInstanceId::ZERO;
        self.tail = (self.tail + 1) % N;
        self.len -= 1;
        Some(instance_id)
    }

    const fn len(&self) -> usize {
        self.len
    }
}

#[derive(Clone, Copy)]
struct ModuleQueue<const N: usize> {
    entries: [ModuleHandle; N],
    head: usize,
    tail: usize,
    len: usize,
}

impl<const N: usize> ModuleQueue<N> {
    const fn new() -> Self {
        Self {
            entries: [ModuleHandle::ZERO; N],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    fn push(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        if self.len == N {
            return Err(SupervisorError::QueueFull);
        }
        self.entries[self.head] = handle;
        self.head = (self.head + 1) % N;
        self.len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<ModuleHandle> {
        if self.len == 0 {
            return None;
        }
        let handle = self.entries[self.tail];
        self.entries[self.tail] = ModuleHandle::ZERO;
        self.tail = (self.tail + 1) % N;
        self.len -= 1;
        Some(handle)
    }

    const fn len(&self) -> usize {
        self.len
    }
}

struct Supervisor {
    boot_payload: u64,
    bootstrapped: bool,
    next_handle: u64,
    next_instance: u64,
    next_claim: u64,
    next_domain: u64,
    next_endpoint: u64,
    next_capability: u64,
    modules: [ModuleRecord; MAX_MODULES],
    templates: [TemplateRecord; MAX_TEMPLATES],
    instances: [InstanceRecord; MAX_INSTANCES],
    resources: [ResourceRecord; MAX_RESOURCES],
    claims: [ClaimRecord; MAX_CLAIMS],
    revocations: [RevocationRecord; MAX_REVOKES],
    heap_grants: [HeapGrantRecord; MAX_HEAP_GRANTS],
    capabilities: [CapabilityRecord; MAX_CAPABILITIES],
    endpoints: [EndpointRecord; MAX_ENDPOINTS],
    subscriptions: [SubscriptionRecord; MAX_SUBSCRIPTIONS],
    messages: [QueuedMessage; MAX_QUEUED_MESSAGES],
    ready_control: InstanceQueue<MAX_INSTANCES>,
    ready_io: InstanceQueue<MAX_INSTANCES>,
    ready_compute: InstanceQueue<MAX_INSTANCES>,
    ready_background: InstanceQueue<MAX_INSTANCES>,
    restart_queue: ModuleQueue<MAX_MODULES>,
    active_module: ModuleHandle,
    active_instance: NodeInstanceId,
}

impl Supervisor {
    const fn new() -> Self {
        Self {
            boot_payload: 0,
            bootstrapped: false,
            next_handle: 1,
            next_instance: 1,
            next_claim: 1,
            next_domain: 1,
            next_endpoint: 1,
            next_capability: 1,
            modules: [ModuleRecord::EMPTY; MAX_MODULES],
            templates: [TemplateRecord::EMPTY; MAX_TEMPLATES],
            instances: [InstanceRecord::EMPTY; MAX_INSTANCES],
            resources: [ResourceRecord::EMPTY; MAX_RESOURCES],
            claims: [ClaimRecord::EMPTY; MAX_CLAIMS],
            revocations: [RevocationRecord::EMPTY; MAX_REVOKES],
            heap_grants: [HeapGrantRecord::EMPTY; MAX_HEAP_GRANTS],
            capabilities: [CapabilityRecord::EMPTY; MAX_CAPABILITIES],
            endpoints: [EndpointRecord::EMPTY; MAX_ENDPOINTS],
            subscriptions: [SubscriptionRecord::EMPTY; MAX_SUBSCRIPTIONS],
            messages: [QueuedMessage::EMPTY; MAX_QUEUED_MESSAGES],
            ready_control: InstanceQueue::new(),
            ready_io: InstanceQueue::new(),
            ready_compute: InstanceQueue::new(),
            ready_background: InstanceQueue::new(),
            restart_queue: ModuleQueue::new(),
            active_module: ModuleHandle::ZERO,
            active_instance: NodeInstanceId::ZERO,
        }
    }

    fn reset(&mut self, boot_payload: u64) {
        *self = Self::new();
        self.boot_payload = boot_payload;
        self.bootstrapped = true;
        self.register_default_resources();
    }

    fn ensure_bootstrapped(&self) -> Result<(), SupervisorError> {
        if self.bootstrapped {
            Ok(())
        } else {
            Err(SupervisorError::NotBootstrapped)
        }
    }

    fn allocate_module_slot(&mut self) -> Result<usize, SupervisorError> {
        self.ensure_bootstrapped()?;
        self.modules
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::ModuleTableFull)
    }

    fn find_module_slot(&self, handle: ModuleHandle) -> Result<usize, SupervisorError> {
        self.modules
            .iter()
            .position(|record| record.occupied && record.handle == handle)
            .ok_or(SupervisorError::ModuleNotFound)
    }

    fn contains_module_id(&self, module_id: ModuleId, exclude: ModuleHandle) -> bool {
        self.modules.iter().any(|record| {
            record.occupied
                && record.handle != exclude
                && record.source.module_id() == module_id
        })
    }

    fn find_module_by_module_id(&self, module_id: ModuleId) -> Option<ModuleHandle> {
        self.modules.iter().find_map(|record| {
            (record.occupied && record.source.module_id() == module_id).then_some(record.handle)
        })
    }

    fn register_default_resources(&mut self) {
        let _ = self.register_resource(RESOURCE_FRAME_ALLOC);
        let _ = self.register_resource(RESOURCE_PAGE_MAPPER);
        let _ = self.register_resource(RESOURCE_DISPLAY_CONSOLE);
        let _ = self.register_resource(RESOURCE_HEAP_SOURCE);
        let _ = self.register_resource(RESOURCE_GPU_ACCEL);
    }

    fn register_resource(&mut self, resource_id: ResourceId) -> Result<(), SupervisorError> {
        if self
            .resources
            .iter()
            .any(|record| record.occupied && record.id == resource_id)
        {
            return Ok(());
        }
        let slot = self
            .resources
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::ResourceTableFull)?;
        self.resources[slot] = ResourceRecord {
            occupied: true,
            id: resource_id,
            current_epoch: LeaseEpoch::ZERO,
        };
        Ok(())
    }

    fn find_template_slot(&self, template_id: NodeTemplateId) -> Result<usize, SupervisorError> {
        self.templates
            .iter()
            .position(|record| record.occupied && record.id == template_id)
            .ok_or(SupervisorError::TemplateNotFound)
    }

    fn find_instance_slot(&self, instance_id: NodeInstanceId) -> Result<usize, SupervisorError> {
        self.instances
            .iter()
            .position(|record| record.occupied && record.id == instance_id)
            .ok_or(SupervisorError::InstanceNotFound)
    }

    fn find_resource_slot(&self, resource_id: ResourceId) -> Result<usize, SupervisorError> {
        self.resources
            .iter()
            .position(|record| record.occupied && record.id == resource_id)
            .ok_or(SupervisorError::ResourceNotFound)
    }

    fn find_claim_slot(&self, claim_id: ClaimId) -> Result<usize, SupervisorError> {
        self.claims
            .iter()
            .position(|record| record.occupied && record.id == claim_id)
            .ok_or(SupervisorError::ClaimNotFound)
    }

    fn default_template_id_for_source(source: ModuleSource) -> NodeTemplateId {
        NodeTemplateId::new(source.module_id().0)
    }

    fn default_lane_for_source(source: ModuleSource) -> ExecutionLaneClass {
        let module_id = source.module_id();
        if module_id == ModuleId::from_ascii("K_SHELL")
            || module_id == ModuleId::from_ascii("K_CYPHER")
        {
            ExecutionLaneClass::Control
        } else if module_id == ModuleId::from_ascii("K_AI")
            || module_id == ModuleId::from_ascii("K_CUDA")
        {
            ExecutionLaneClass::Compute
        } else if module_id == ModuleId::from_ascii("K_VGA")
            || module_id == ModuleId::from_ascii("K_SERIAL")
            || module_id == ModuleId::from_ascii("K_IME")
            || module_id == ModuleId::from_ascii("K_NET")
            || module_id == ModuleId::from_ascii("K_MOUSE")
            || module_id == ModuleId::from_ascii("K_PS2")
            || module_id == ModuleId::from_ascii("K_PIC")
            || module_id == ModuleId::from_ascii("K_PIT")
        {
            ExecutionLaneClass::Io
        } else {
            ExecutionLaneClass::Background
        }
    }

    fn ready_queue_mut(&mut self, lane: ExecutionLaneClass) -> &mut InstanceQueue<MAX_INSTANCES> {
        match lane {
            ExecutionLaneClass::Control => &mut self.ready_control,
            ExecutionLaneClass::Io => &mut self.ready_io,
            ExecutionLaneClass::Compute => &mut self.ready_compute,
            ExecutionLaneClass::Background => &mut self.ready_background,
        }
    }

    fn pop_ready_queue(&mut self, lane: ExecutionLaneClass) -> Option<NodeInstanceId> {
        self.ready_queue_mut(lane).pop()
    }

    fn enqueue_ready_instance(&mut self, instance_id: NodeInstanceId) -> Result<(), SupervisorError> {
        let instance_slot = self.find_instance_slot(instance_id)?;
        if self.instances[instance_slot].ready_queued {
            return Ok(());
        }
        if matches!(
            self.instances[instance_slot].lifecycle,
            NodeInstanceLifecycle::Stopped | NodeInstanceLifecycle::Faulted | NodeInstanceLifecycle::Suspended
        ) {
            return Err(SupervisorError::InvalidState);
        }
        self.instances[instance_slot].lifecycle = NodeInstanceLifecycle::Ready;
        self.instances[instance_slot].ready_queued = true;
        let lane = self.instances[instance_slot].lane;
        self.ready_queue_mut(lane).push(instance_id)
    }

    fn dequeue_ready_instance(
        &mut self,
        preferred_lane: Option<ExecutionLaneClass>,
    ) -> Result<Option<NodeInstanceId>, SupervisorError> {
        self.ensure_bootstrapped()?;
        if let Some(lane) = preferred_lane {
            return Ok(self.dequeue_ready_from_lane(lane));
        }

        for lane in [
            ExecutionLaneClass::Control,
            ExecutionLaneClass::Io,
            ExecutionLaneClass::Compute,
            ExecutionLaneClass::Background,
        ] {
            if let Some(instance_id) = self.dequeue_ready_from_lane(lane) {
                return Ok(Some(instance_id));
            }
        }
        Ok(None)
    }

    fn dequeue_ready_from_lane(
        &mut self,
        lane: ExecutionLaneClass,
    ) -> Option<NodeInstanceId> {
        while let Some(instance_id) = self.pop_ready_queue(lane) {
            let Ok(instance_slot) = self.find_instance_slot(instance_id) else {
                continue;
            };
            if !self.instances[instance_slot].occupied
                || !self.instances[instance_slot].ready_queued
                || self.instances[instance_slot].lifecycle != NodeInstanceLifecycle::Ready
            {
                continue;
            }
            self.instances[instance_slot].ready_queued = false;
            self.instances[instance_slot].lifecycle = NodeInstanceLifecycle::Running;
            return Some(instance_id);
        }
        None
    }

    fn enqueue_restart_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        if self.modules[slot].queued_restart {
            return Ok(());
        }
        self.modules[slot].queued_restart = true;
        self.restart_queue.push(handle)
    }

    fn process_next_restart(&mut self) -> Result<Option<ModuleHandle>, SupervisorError> {
        while let Some(handle) = self.restart_queue.pop() {
            let slot = self.find_module_slot(handle)?;
            self.modules[slot].queued_restart = false;
            match self.modules[slot].state {
                ModuleLifecycle::Running
                | ModuleLifecycle::Instantiated
                | ModuleLifecycle::Mapped => {
                    let _ = self.stop_module(handle);
                }
                ModuleLifecycle::Faulted => {
                    self.revoke_capabilities(handle);
                    self.drain_messages(handle);
                    self.teardown_module_instances(handle);
                    self.modules[slot].state = ModuleLifecycle::Stopped;
                }
                ModuleLifecycle::Stopped | ModuleLifecycle::Installed => {}
                _ => continue,
            }
            self.modules[slot].restart_generation =
                self.modules[slot].restart_generation.wrapping_add(1);
            self.validate_module(handle)?;
            self.map_module(handle)?;
            self.instantiate_module(handle)?;
            self.start_module(handle)?;
            return Ok(Some(handle));
        }
        Ok(None)
    }

    fn register_default_template(
        &mut self,
        handle: ModuleHandle,
        source: ModuleSource,
    ) -> Result<NodeTemplateId, SupervisorError> {
        let template_id = Self::default_template_id_for_source(source);
        if self
            .templates
            .iter()
            .any(|record| record.occupied && record.id == template_id)
        {
            return Ok(template_id);
        }
        let slot = self
            .templates
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::TemplateTableFull)?;
        self.templates[slot] = TemplateRecord {
            occupied: true,
            id: template_id,
            module: handle,
            spawn_policy: SpawnPolicy::OnContention,
            lane: Self::default_lane_for_source(source),
            heap_quota: DEFAULT_HEAP_QUOTA,
        };
        Ok(template_id)
    }

    fn template_for_module(&self, handle: ModuleHandle) -> Result<NodeTemplateId, SupervisorError> {
        self.templates
            .iter()
            .find(|record| record.occupied && record.module == handle)
            .map(|record| record.id)
            .ok_or(SupervisorError::TemplateNotFound)
    }

    fn primary_instance_for_module(&self, handle: ModuleHandle) -> Option<NodeInstanceId> {
        self.instances
            .iter()
            .find(|record| record.occupied && record.module == handle)
            .map(|record| record.id)
    }

    fn active_instance_for_module(&self, handle: ModuleHandle) -> Option<NodeInstanceId> {
        if self.active_module == handle && self.active_instance != NodeInstanceId::ZERO {
            return Some(self.active_instance);
        }
        self.primary_instance_for_module(handle)
    }

    fn allocate_endpoint(&mut self, owner: ModuleHandle, capability: CapabilityToken, label: [u8; 16]) -> Result<EndpointId, SupervisorError> {
        let slot = self
            .endpoints
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::EndpointTableFull)?;
        let _ = label;
        let endpoint = EndpointId::new(self.next_endpoint);
        self.next_endpoint += 1;
        self.endpoints[slot] = EndpointRecord {
            occupied: true,
            id: endpoint,
            owner,
            capability,
        };
        Ok(endpoint)
    }

    fn allocate_capability(&mut self, owner: ModuleHandle, spec: CapabilitySpec, endpoint: EndpointId) -> Result<CapabilityToken, SupervisorError> {
        let slot = self
            .capabilities
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::CapabilityTableFull)?;
        let token = CapabilityToken::new(self.next_capability);
        self.next_capability += 1;
        self.capabilities[slot] = CapabilityRecord {
            occupied: true,
            token,
            provider: owner,
            endpoint,
            spec,
        };
        Ok(token)
    }

    fn build_domain(&mut self, source: ModuleSource) -> Result<ModuleDomain, SupervisorError> {
        let domain_id = DomainId::new(self.next_domain);
        self.next_domain += 1;
        let base = DOMAIN_BASE + ((domain_id.0 - 1) * DOMAIN_STRIDE);
        let image_len = source.image_len();
        let image_base = base;
        let stack_base = image_base + image_len + 0x10_000;
        let ipc_base = stack_base + DEFAULT_STACK_WINDOW + 0x10_000;
        let heap_base = ipc_base + DEFAULT_IPC_WINDOW + 0x10_000;
        let root_table_phys = create_domain_root(
            image_base,
            image_len,
            stack_base,
            DEFAULT_STACK_WINDOW,
            ipc_base,
            DEFAULT_IPC_WINDOW,
        )?;
        Ok(ModuleDomain {
            id: domain_id,
            root_table_phys,
            image_base,
            image_len,
            stack_base,
            stack_len: DEFAULT_STACK_WINDOW,
            ipc_base,
            ipc_len: DEFAULT_IPC_WINDOW,
            heap_base,
            heap_len: DEFAULT_HEAP_WINDOW,
            isolated: true,
        })
    }

    fn install_source(&mut self, source: ModuleSource) -> Result<ModuleHandle, SupervisorError> {
        let slot = self.allocate_module_slot()?;
        let handle = ModuleHandle::new(self.next_handle);
        self.next_handle += 1;
        self.modules[slot] = ModuleRecord {
            occupied: true,
            handle,
            source,
            state: ModuleLifecycle::Installed,
            domain: ModuleDomain::EMPTY,
            queued_restart: false,
            restart_generation: 0,
        };
        self.register_default_template(handle, source)?;
        Ok(handle)
    }

    fn install_descriptor(&mut self, descriptor: ModuleDescriptor) -> Result<ModuleHandle, SupervisorError> {
        self.install_source(ModuleSource::Descriptor(descriptor))
    }

    fn validate_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        let state = self.modules[slot].state;
        let source = self.modules[slot].source;
        if !matches!(state, ModuleLifecycle::Installed | ModuleLifecycle::Stopped) {
            return Err(SupervisorError::InvalidState);
        }
        if let ModuleSource::Descriptor(descriptor) = source {
            let mut dep_idx = 0usize;
            while dep_idx < descriptor.dependencies.len() {
                let dependency = descriptor.dependencies[dep_idx];
                if dependency.required && !self.contains_module_id(dependency.module_id, handle) {
                    return Err(SupervisorError::ModuleRejected);
                }
                dep_idx += 1;
            }
        }
        let _ = source.permissions();
        let _ = source.imports();
        self.modules[slot].state = ModuleLifecycle::Validated;
        Ok(())
    }

    fn map_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        let source = self.modules[slot].source;
        if !matches!(self.modules[slot].state, ModuleLifecycle::Validated | ModuleLifecycle::Stopped) {
            return Err(SupervisorError::InvalidState);
        }
        if self.modules[slot].domain.root_table_phys == 0 {
            self.modules[slot].domain = self.build_domain(source)?;
        }
        self.modules[slot].state = ModuleLifecycle::Mapped;
        Ok(())
    }

    fn call_entry(
        &mut self,
        handle: ModuleHandle,
        instance_id: NodeInstanceId,
        entry_fn: Option<unsafe extern "C" fn(abi: *const ModuleAbiV1, handle: ModuleHandle, domain: DomainId) -> ModuleCallStatus>,
    ) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        let domain = self.modules[slot].domain.id;
        let Some(callback) = entry_fn else {
            return Ok(());
        };
        let prev_module = self.active_module;
        let prev_instance = self.active_instance;
        self.active_module = handle;
        self.active_instance = instance_id;
        if instance_id != NodeInstanceId::ZERO {
            let instance_slot = self.find_instance_slot(instance_id)?;
            self.instances[instance_slot].lifecycle = NodeInstanceLifecycle::Running;
            self.instances[instance_slot].ready_queued = false;
        }
        ACTIVE_SUPERVISOR.store(self as *mut Supervisor, Ordering::SeqCst);
        let status = unsafe { callback(&MODULE_ABI_V1, handle, domain) };
        ACTIVE_SUPERVISOR.store(core::ptr::null_mut(), Ordering::SeqCst);
        if instance_id != NodeInstanceId::ZERO {
            let instance_slot = self.find_instance_slot(instance_id)?;
            if self.instances[instance_slot].occupied
                && self.instances[instance_slot].lifecycle == NodeInstanceLifecycle::Running
            {
                self.instances[instance_slot].lifecycle = NodeInstanceLifecycle::Ready;
            }
        }
        self.active_module = prev_module;
        self.active_instance = prev_instance;
        if matches!(status, ModuleCallStatus::Ok | ModuleCallStatus::Retry) {
            Ok(())
        } else {
            Err(SupervisorError::ModuleRejected)
        }
    }

    fn instantiate_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        if self.modules[slot].state != ModuleLifecycle::Mapped {
            return Err(SupervisorError::InvalidState);
        }
        let entry = self.modules[slot].source.entry();
        let instance_id = self.ensure_primary_instance(handle)?;
        self.call_entry(handle, instance_id, entry.module_init)?;
        self.modules[slot].state = ModuleLifecycle::Instantiated;
        Ok(())
    }

    fn publish_exports(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        self.revoke_capabilities(handle);
        let slot = self.find_module_slot(handle)?;
        let exports = self.modules[slot].source.exports();
        for spec in exports {
            let label = fixed_bytes_16(spec.name);
            let endpoint = self.allocate_endpoint(handle, CapabilityToken::ZERO, label)?;
            let token = self.allocate_capability(handle, *spec, endpoint)?;
            if let Some(endpoint_slot) = self
                .endpoints
                .iter()
                .position(|record| record.occupied && record.id == endpoint)
            {
                self.endpoints[endpoint_slot].capability = token;
            }
        }
        Ok(())
    }

    fn start_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        if !matches!(self.modules[slot].state, ModuleLifecycle::Instantiated | ModuleLifecycle::Stopped) {
            return Err(SupervisorError::InvalidState);
        }
        if self.modules[slot].state == ModuleLifecycle::Stopped {
            self.validate_module(handle)?;
            self.map_module(handle)?;
            self.instantiate_module(handle)?;
        }
        let entry = self.modules[slot].source.entry();
        let instance_id = self.ensure_primary_instance(handle)?;
        self.call_entry(handle, instance_id, entry.module_start)?;
        let _ = self.enqueue_ready_instance(instance_id);
        self.publish_exports(handle)?;
        self.modules[slot].state = ModuleLifecycle::Running;
        Ok(())
    }

    fn disconnect_endpoints(&mut self, handle: ModuleHandle) {
        for idx in 0..self.subscriptions.len() {
            if !self.subscriptions[idx].occupied {
                continue;
            }
            let consumer_owned = self
                .endpoint_owner(self.subscriptions[idx].consumer_endpoint)
                .map(|owner| owner == handle)
                .unwrap_or(false);
            let provider_owned = self
                .endpoint_owner(self.subscriptions[idx].provider_endpoint)
                .map(|owner| owner == handle)
                .unwrap_or(false);
            if consumer_owned || provider_owned {
                self.subscriptions[idx] = SubscriptionRecord::EMPTY;
            }
        }

        for idx in 0..self.endpoints.len() {
            if self.endpoints[idx].occupied && self.endpoints[idx].owner == handle {
                self.endpoints[idx] = EndpointRecord::EMPTY;
            }
        }
    }

    fn revoke_capabilities(&mut self, handle: ModuleHandle) {
        for idx in 0..self.capabilities.len() {
            if self.capabilities[idx].occupied && self.capabilities[idx].provider == handle {
                self.capabilities[idx] = CapabilityRecord::EMPTY;
            }
        }
        self.disconnect_endpoints(handle);
    }

    fn drain_messages(&mut self, handle: ModuleHandle) {
        for idx in 0..self.messages.len() {
            if self.messages[idx].occupied && self.messages[idx].recipient == handle {
                self.messages[idx] = QueuedMessage::EMPTY;
            }
        }
    }

    fn stop_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        if !matches!(
            self.modules[slot].state,
            ModuleLifecycle::Running | ModuleLifecycle::Instantiated | ModuleLifecycle::Mapped
        ) {
            return Err(SupervisorError::InvalidState);
        }
        let entry = self.modules[slot].source.entry();
        let instance_id = self.primary_instance_for_module(handle).unwrap_or(NodeInstanceId::ZERO);
        self.call_entry(handle, instance_id, entry.module_stop)?;
        self.modules[slot].state = ModuleLifecycle::Quiescing;
        self.revoke_capabilities(handle);
        self.drain_messages(handle);
        self.teardown_module_instances(handle);
        self.modules[slot].state = ModuleLifecycle::Stopped;
        Ok(())
    }

    fn restart_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        self.enqueue_restart_module(handle)?;
        let _ = self.process_next_restart()?;
        Ok(())
    }

    fn fault_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        self.modules[slot].state = ModuleLifecycle::Faulted;
        match self.modules[slot].source.fault_policy() {
            ModuleFaultPolicy::Restart | ModuleFaultPolicy::RestartAlways => self.restart_module(handle),
            ModuleFaultPolicy::FaultKernelDegraded | ModuleFaultPolicy::Manual => Ok(()),
        }
    }

    fn uninstall_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        let _ = self.stop_module(handle);
        self.modules[slot] = ModuleRecord::EMPTY;
        Ok(())
    }

    fn resolve_capability(&self, namespace: &str, name: &str) -> Option<CapabilityToken> {
        self.capabilities
            .iter()
            .find(|record| {
                record.occupied
                    && record.spec.namespace == namespace
                    && record.spec.name == name
            })
            .map(|record| record.token)
    }

    fn endpoint_owner(&self, endpoint: EndpointId) -> Option<ModuleHandle> {
        self.endpoints
            .iter()
            .find(|record| record.occupied && record.id == endpoint)
            .map(|record| record.owner)
    }

    fn endpoint_for_token(&self, token: CapabilityToken) -> Option<EndpointId> {
        self.capabilities
            .iter()
            .find(|record| record.occupied && record.token == token)
            .map(|record| record.endpoint)
    }

    fn open_endpoint(&mut self, module: ModuleHandle, label: &str) -> Result<EndpointId, SupervisorError> {
        let _ = self.find_module_slot(module)?;
        self.allocate_endpoint(module, CapabilityToken::ZERO, fixed_bytes_16(label))
    }

    fn ensure_primary_instance(&mut self, handle: ModuleHandle) -> Result<NodeInstanceId, SupervisorError> {
        if let Some(instance_id) = self.primary_instance_for_module(handle) {
            return Ok(instance_id);
        }
        let template_id = self.template_for_module(handle)?;
        self.spawn_instance(template_id)
    }

    fn spawn_instance(&mut self, template_id: NodeTemplateId) -> Result<NodeInstanceId, SupervisorError> {
        let template_slot = self.find_template_slot(template_id)?;
        let template = self.templates[template_slot];
        let module_slot = self.find_module_slot(template.module)?;
        if !self.modules[module_slot].occupied {
            return Err(SupervisorError::ModuleNotFound);
        }
        let slot = self
            .instances
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::InstanceTableFull)?;
        let instance_id = NodeInstanceId::new(self.next_instance);
        self.next_instance += 1;
        self.instances[slot] = InstanceRecord {
            occupied: true,
            id: instance_id,
            template_id,
            module: template.module,
            lane: template.lane,
            lifecycle: NodeInstanceLifecycle::Ready,
            ready_queued: false,
            heap_quota: template.heap_quota,
            heap_pages_used: 0,
            heap_cursor_pages: 0,
        };
        Ok(instance_id)
    }

    fn queue_revocation(&mut self, lease: ResourceLease) -> Result<(), SupervisorError> {
        let slot = self
            .revocations
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::RevokeTableFull)?;
        self.revocations[slot] = RevocationRecord {
            occupied: true,
            instance: lease.instance_id,
            lease,
        };
        Ok(())
    }

    fn free_claim_slot(&mut self, slot: usize) -> ClaimRecord {
        let claim = self.claims[slot];
        self.claims[slot] = ClaimRecord::EMPTY;
        claim
    }

    fn release_claim_internal(&mut self, claim_id: ClaimId) -> Result<(), SupervisorError> {
        let slot = self.find_claim_slot(claim_id)?;
        let _ = self.free_claim_slot(slot);
        Ok(())
    }

    fn teardown_instance(&mut self, instance_id: NodeInstanceId) {
        for idx in 0..self.claims.len() {
            if self.claims[idx].occupied && self.claims[idx].instance == instance_id {
                self.claims[idx] = ClaimRecord::EMPTY;
            }
        }
        for idx in 0..self.revocations.len() {
            if self.revocations[idx].occupied && self.revocations[idx].instance == instance_id {
                self.revocations[idx] = RevocationRecord::EMPTY;
            }
        }
        for idx in 0..self.heap_grants.len() {
            if self.heap_grants[idx].occupied && self.heap_grants[idx].instance == instance_id {
                self.heap_grants[idx] = HeapGrantRecord::EMPTY;
            }
        }
        if let Ok(slot) = self.find_instance_slot(instance_id) {
            self.instances[slot] = InstanceRecord::EMPTY;
        }
    }

    fn teardown_module_instances(&mut self, handle: ModuleHandle) {
        let mut victims = [NodeInstanceId::ZERO; MAX_INSTANCES];
        let mut count = 0usize;
        for record in self.instances {
            if record.occupied && record.module == handle {
                victims[count] = record.id;
                count += 1;
            }
        }
        let mut idx = 0usize;
        while idx < count {
            self.teardown_instance(victims[idx]);
            idx += 1;
        }
    }

    fn claim_resource(
        &mut self,
        instance_id: NodeInstanceId,
        resource_id: ResourceId,
        claim_policy: ClaimPolicy,
        preempt_policy: PreemptPolicy,
    ) -> Result<ResourceLease, SupervisorError> {
        let instance_slot = self.find_instance_slot(instance_id)?;
        let module = self.instances[instance_slot].module;
        let resource_slot = self.find_resource_slot(resource_id)?;

        for record in self.claims {
            if record.occupied
                && record.active
                && record.instance == instance_id
                && record.resource == resource_id
            {
                return Ok(ResourceLease {
                    claim_id: record.id,
                    resource_id,
                    instance_id,
                    epoch: record.epoch,
                    claim_policy: record.claim_policy,
                    preempt_policy: record.preempt_policy,
                });
            }
        }

        let mut active_slots = [0usize; MAX_CLAIMS];
        let mut active_count = 0usize;
        let mut has_exclusive = false;
        for (idx, record) in self.claims.iter().enumerate() {
            if record.occupied && record.active && record.resource == resource_id {
                active_slots[active_count] = idx;
                active_count += 1;
                has_exclusive |= record.claim_policy == ClaimPolicy::Exclusive;
            }
        }

        let wants_shared = claim_policy == ClaimPolicy::Shared;
        let can_share = active_count > 0 && !has_exclusive && wants_shared;
        let may_preempt = matches!(preempt_policy, PreemptPolicy::Try | PreemptPolicy::Force);

        if active_count > 0 && !can_share && !may_preempt {
            self.instances[instance_slot].lifecycle = NodeInstanceLifecycle::WaitingClaim;
            self.instances[instance_slot].ready_queued = false;
            return Err(SupervisorError::ResourceBusy);
        }

        if active_count > 0 && !can_share && may_preempt {
            let mut idx = 0usize;
            while idx < active_count {
                let claim_slot = active_slots[idx];
                let claim = self.free_claim_slot(claim_slot);
                let _ = self.queue_revocation(ResourceLease {
                    claim_id: claim.id,
                    resource_id: claim.resource,
                    instance_id: claim.instance,
                    epoch: claim.epoch,
                    claim_policy: claim.claim_policy,
                    preempt_policy: claim.preempt_policy,
                });
                if let Ok(victim_slot) = self.find_instance_slot(claim.instance) {
                    self.instances[victim_slot].lifecycle = NodeInstanceLifecycle::Suspended;
                    self.instances[victim_slot].ready_queued = false;
                }
                idx += 1;
            }
        }

        let claim_slot = self
            .claims
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::ClaimTableFull)?;
        let epoch = if can_share && self.resources[resource_slot].current_epoch != LeaseEpoch::ZERO {
            self.resources[resource_slot].current_epoch
        } else {
            let next_epoch = LeaseEpoch::new(self.resources[resource_slot].current_epoch.0 + 1);
            self.resources[resource_slot].current_epoch = next_epoch;
            next_epoch
        };
        let claim_id = ClaimId::new(self.next_claim);
        self.next_claim += 1;
        self.claims[claim_slot] = ClaimRecord {
            occupied: true,
            id: claim_id,
            resource: resource_id,
            instance: instance_id,
            module,
            claim_policy,
            preempt_policy,
            epoch,
            active: true,
        };
        let _ = self.enqueue_ready_instance(instance_id);
        Ok(ResourceLease {
            claim_id,
            resource_id,
            instance_id,
            epoch,
            claim_policy,
            preempt_policy,
        })
    }

    fn drain_revocation(&mut self, instance_id: NodeInstanceId) -> Option<ResourceLease> {
        let slot = self
            .revocations
            .iter()
            .position(|record| record.occupied && record.instance == instance_id)?;
        let lease = self.revocations[slot].lease;
        self.revocations[slot] = RevocationRecord::EMPTY;
        Some(lease)
    }

    fn request_pages(
        &mut self,
        module: ModuleHandle,
        page_count: usize,
        writable: bool,
    ) -> Result<u64, SupervisorError> {
        let instance_id = self
            .active_instance_for_module(module)
            .ok_or(SupervisorError::NoActiveInstance)?;
        let module_slot = self.find_module_slot(module)?;
        let instance_slot = self.find_instance_slot(instance_id)?;
        let domain = self.modules[module_slot].domain;
        if domain.root_table_phys == 0 || page_count == 0 {
            return Err(SupervisorError::InvalidState);
        }
        let projected = self.instances[instance_slot]
            .heap_pages_used
            .saturating_add(page_count as u32);
        if projected > self.instances[instance_slot].heap_quota.max_pages {
            return Err(SupervisorError::HeapQuotaExceeded);
        }
        let base = domain.heap_base
            + (self.instances[instance_slot].heap_cursor_pages as u64 * PAGE_BYTES);
        let byte_len = page_count as u64 * PAGE_BYTES;
        if base + byte_len > domain.heap_base + domain.heap_len {
            return Err(SupervisorError::HeapQuotaExceeded);
        }
        map_domain_heap_pages(domain.root_table_phys, base, page_count, writable)?;
        let grant_slot = self
            .heap_grants
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::HeapGrantTableFull)?;
        self.heap_grants[grant_slot] = HeapGrantRecord {
            occupied: true,
            module,
            instance: instance_id,
            base,
            page_count,
            writable,
        };
        self.instances[instance_slot].heap_pages_used = projected;
        self.instances[instance_slot].heap_cursor_pages = self.instances[instance_slot]
            .heap_cursor_pages
            .saturating_add(page_count as u32);
        Ok(base)
    }

    fn free_pages(
        &mut self,
        module: ModuleHandle,
        base: u64,
        page_count: usize,
    ) -> Result<(), SupervisorError> {
        let slot = self
            .heap_grants
            .iter()
            .position(|record| {
                record.occupied
                    && record.module == module
                    && record.base == base
                    && record.page_count == page_count
            })
            .ok_or(SupervisorError::HeapGrantNotFound)?;
        let instance_id = self.heap_grants[slot].instance;
        if let Ok(instance_slot) = self.find_instance_slot(instance_id) {
            self.instances[instance_slot].heap_pages_used = self.instances[instance_slot]
                .heap_pages_used
                .saturating_sub(page_count as u32);
        }
        self.heap_grants[slot] = HeapGrantRecord::EMPTY;
        Ok(())
    }

    fn subscribe(&mut self, consumer_endpoint: EndpointId, provider_endpoint: EndpointId) -> Result<(), SupervisorError> {
        let consumer_ok = self.endpoints.iter().any(|record| record.occupied && record.id == consumer_endpoint);
        let provider_ok = self.endpoints.iter().any(|record| record.occupied && record.id == provider_endpoint);
        if !consumer_ok || !provider_ok {
            return Err(SupervisorError::EndpointNotFound);
        }
        let slot = self
            .subscriptions
            .iter()
            .position(|record| !record.occupied)
            .ok_or(SupervisorError::SubscriptionTableFull)?;
        self.subscriptions[slot] = SubscriptionRecord {
            occupied: true,
            consumer_endpoint,
            provider_endpoint,
        };
        Ok(())
    }

    fn post_message(&mut self, endpoint: EndpointId, message: ModuleMessage) -> Result<(), SupervisorError> {
        let recipient = self.endpoint_owner(endpoint).ok_or(SupervisorError::EndpointNotFound)?;
        let slot = self
            .messages
            .iter()
            .position(|queued| !queued.occupied)
            .ok_or(SupervisorError::QueueFull)?;
        let mut routed = message;
        routed.header.to = endpoint;
        self.messages[slot] = QueuedMessage {
            occupied: true,
            recipient,
            endpoint,
            message: routed,
        };
        Ok(())
    }

    fn receive_message(&mut self, module: ModuleHandle, endpoint: EndpointId) -> Result<ModuleMessage, SupervisorError> {
        let _ = self.find_module_slot(module)?;
        let slot = self
            .messages
            .iter()
            .position(|queued| queued.occupied && queued.recipient == module && queued.endpoint == endpoint)
            .ok_or(SupervisorError::EndpointNotFound)?;
        let message = self.messages[slot].message;
        self.messages[slot] = QueuedMessage::EMPTY;
        Ok(message)
    }

    fn realize_boot_modules(&mut self) -> Result<SupervisorBootReport, SupervisorError> {
        self.ensure_bootstrapped()?;
        for idx in 0..self.modules.len() {
            if !self.modules[idx].occupied {
                continue;
            }
            let handle = self.modules[idx].handle;
            if self.modules[idx].state == ModuleLifecycle::Installed {
                self.validate_module(handle)?;
                self.map_module(handle)?;
                self.instantiate_module(handle)?;
                self.start_module(handle)?;
            }
        }
        let snapshot = self.snapshot();
        Ok(SupervisorBootReport {
            discovered_modules: snapshot.installed_modules,
            running_modules: snapshot.running_modules,
            isolated_domains: snapshot.isolated_domains,
            published_capabilities: snapshot.published_capabilities,
        })
    }

    fn snapshot(&self) -> SupervisorSnapshot {
        SupervisorSnapshot {
            installed_modules: self.modules.iter().filter(|record| record.occupied).count(),
            registered_templates: self.templates.iter().filter(|record| record.occupied).count(),
            live_instances: self.instances.iter().filter(|record| record.occupied).count(),
            ready_instances: self
                .instances
                .iter()
                .filter(|record| record.occupied && record.ready_queued)
                .count(),
            waiting_instances: self
                .instances
                .iter()
                .filter(|record| {
                    record.occupied && record.lifecycle == NodeInstanceLifecycle::WaitingClaim
                })
                .count(),
            suspended_instances: self
                .instances
                .iter()
                .filter(|record| {
                    record.occupied && record.lifecycle == NodeInstanceLifecycle::Suspended
                })
                .count(),
            registered_resources: self.resources.iter().filter(|record| record.occupied).count(),
            active_claims: self
                .claims
                .iter()
                .filter(|record| record.occupied && record.active)
                .count(),
            pending_revocations: self.revocations.iter().filter(|record| record.occupied).count(),
            queued_restarts: self.restart_queue.len(),
            running_modules: self
                .modules
                .iter()
                .filter(|record| record.occupied && record.state == ModuleLifecycle::Running)
                .count(),
            isolated_domains: self
                .modules
                .iter()
                .filter(|record| record.occupied && record.domain.id != DomainId::ZERO)
                .count(),
            heap_grants: self.heap_grants.iter().filter(|record| record.occupied).count(),
            heap_pages_used: self
                .instances
                .iter()
                .filter(|record| record.occupied)
                .map(|record| record.heap_pages_used as usize)
                .sum(),
            published_capabilities: self.capabilities.iter().filter(|record| record.occupied).count(),
            endpoints: self.endpoints.iter().filter(|record| record.occupied).count(),
            queued_messages: self.messages.iter().filter(|record| record.occupied).count(),
            ready_control: self.ready_control.len(),
            ready_io: self.ready_io.len(),
            ready_compute: self.ready_compute.len(),
            ready_background: self.ready_background.len(),
        }
    }
}

static SUPERVISOR: Mutex<Supervisor> = Mutex::new(Supervisor::new());
static ACTIVE_SUPERVISOR: AtomicPtr<Supervisor> = AtomicPtr::new(core::ptr::null_mut());

fn with_supervisor<R>(f: impl FnOnce(&Supervisor) -> R) -> R {
    let active = ACTIVE_SUPERVISOR.load(Ordering::SeqCst);
    if !active.is_null() {
        return unsafe { f(&*active) };
    }
    let guard = SUPERVISOR.lock();
    f(&guard)
}

fn with_supervisor_mut<R>(f: impl FnOnce(&mut Supervisor) -> R) -> R {
    let active = ACTIVE_SUPERVISOR.load(Ordering::SeqCst);
    if !active.is_null() {
        return unsafe { f(&mut *active) };
    }
    let mut guard = SUPERVISOR.lock();
    f(&mut guard)
}

unsafe extern "C" fn abi_log(_module: ModuleHandle, _level: u8, _bytes: *const u8, _len: usize) -> ModuleCallStatus {
    ModuleCallStatus::Ok
}

unsafe extern "C" fn abi_send_message(
    _module: ModuleHandle,
    endpoint: EndpointId,
    message: *const ModuleMessage,
) -> ModuleCallStatus {
    if message.is_null() {
        return ModuleCallStatus::Fault;
    }
    match with_supervisor_mut(|supervisor| supervisor.post_message(endpoint, unsafe { *message })) {
        Ok(()) => ModuleCallStatus::Ok,
        Err(SupervisorError::QueueFull) => ModuleCallStatus::Retry,
        Err(SupervisorError::EndpointNotFound) => ModuleCallStatus::Denied,
        Err(_) => ModuleCallStatus::Fault,
    }
}

unsafe extern "C" fn abi_receive_message(
    module: ModuleHandle,
    endpoint: EndpointId,
    out: *mut ModuleMessage,
) -> ModuleCallStatus {
    if out.is_null() {
        return ModuleCallStatus::Fault;
    }
    match with_supervisor_mut(|supervisor| supervisor.receive_message(module, endpoint)) {
        Ok(message) => {
            unsafe { *out = message; }
            ModuleCallStatus::Ok
        }
        Err(SupervisorError::EndpointNotFound) => ModuleCallStatus::Retry,
        Err(_) => ModuleCallStatus::Fault,
    }
}

unsafe extern "C" fn abi_resolve_capability(
    _module: ModuleHandle,
    namespace: *const u8,
    namespace_len: usize,
    name: *const u8,
    name_len: usize,
    out: *mut CapabilityToken,
) -> ModuleCallStatus {
    if namespace.is_null() || name.is_null() || out.is_null() {
        return ModuleCallStatus::Fault;
    }
    let namespace = unsafe { core::slice::from_raw_parts(namespace, namespace_len) };
    let name = unsafe { core::slice::from_raw_parts(name, name_len) };
    let Ok(namespace) = core::str::from_utf8(namespace) else {
        return ModuleCallStatus::Denied;
    };
    let Ok(name) = core::str::from_utf8(name) else {
        return ModuleCallStatus::Denied;
    };
    match with_supervisor(|supervisor| supervisor.resolve_capability(namespace, name)) {
        Some(token) => {
            unsafe { *out = token; }
            ModuleCallStatus::Ok
        }
        None => ModuleCallStatus::Retry,
    }
}

unsafe extern "C" fn abi_open_endpoint(
    module: ModuleHandle,
    label: *const u8,
    label_len: usize,
    out: *mut EndpointId,
) -> ModuleCallStatus {
    if label.is_null() || out.is_null() {
        return ModuleCallStatus::Fault;
    }
    let label = unsafe { core::slice::from_raw_parts(label, label_len) };
    let Ok(label) = core::str::from_utf8(label) else {
        return ModuleCallStatus::Denied;
    };
    match with_supervisor_mut(|supervisor| supervisor.open_endpoint(module, label)) {
        Ok(endpoint) => {
            unsafe { *out = endpoint; }
            ModuleCallStatus::Ok
        }
        Err(SupervisorError::ModuleNotFound) => ModuleCallStatus::Denied,
        Err(SupervisorError::EndpointTableFull) => ModuleCallStatus::Retry,
        Err(_) => ModuleCallStatus::Fault,
    }
}

unsafe extern "C" fn abi_request_pages(
    module: ModuleHandle,
    page_count: usize,
    writable: u8,
    out_base: *mut u64,
) -> ModuleCallStatus {
    if out_base.is_null() {
        return ModuleCallStatus::Fault;
    }
    match with_supervisor_mut(|supervisor| {
        supervisor.request_pages(module, page_count, writable != 0)
    }) {
        Ok(base) => {
            unsafe { *out_base = base; }
            ModuleCallStatus::Ok
        }
        Err(SupervisorError::HeapQuotaExceeded) => ModuleCallStatus::Denied,
        Err(SupervisorError::NoActiveInstance | SupervisorError::InvalidState) => {
            ModuleCallStatus::Denied
        }
        Err(_) => ModuleCallStatus::Fault,
    }
}

unsafe extern "C" fn abi_free_pages(
    module: ModuleHandle,
    base: u64,
    page_count: usize,
) -> ModuleCallStatus {
    match with_supervisor_mut(|supervisor| supervisor.free_pages(module, base, page_count)) {
        Ok(()) => ModuleCallStatus::Ok,
        Err(SupervisorError::HeapGrantNotFound) => ModuleCallStatus::Denied,
        Err(_) => ModuleCallStatus::Fault,
    }
}

unsafe extern "C" fn abi_current_instance(
    module: ModuleHandle,
    out: *mut NodeInstanceId,
) -> ModuleCallStatus {
    if out.is_null() {
        return ModuleCallStatus::Fault;
    }
    match with_supervisor(|supervisor| supervisor.active_instance_for_module(module)) {
        Some(instance_id) => {
            unsafe { *out = instance_id; }
            ModuleCallStatus::Ok
        }
        None => ModuleCallStatus::Denied,
    }
}

unsafe extern "C" fn abi_claim_resource(
    module: ModuleHandle,
    resource: ResourceId,
    claim_policy: ClaimPolicy,
    preempt_policy: PreemptPolicy,
    out_claim: *mut ClaimId,
    out_epoch: *mut LeaseEpoch,
) -> ModuleCallStatus {
    if out_claim.is_null() || out_epoch.is_null() {
        return ModuleCallStatus::Fault;
    }
    let Some(instance_id) = with_supervisor(|supervisor| supervisor.active_instance_for_module(module)) else {
        return ModuleCallStatus::Denied;
    };
    match with_supervisor_mut(|supervisor| {
        supervisor.claim_resource(instance_id, resource, claim_policy, preempt_policy)
    }) {
        Ok(lease) => {
            unsafe {
                *out_claim = lease.claim_id;
                *out_epoch = lease.epoch;
            }
            ModuleCallStatus::Ok
        }
        Err(SupervisorError::ResourceBusy) => ModuleCallStatus::Retry,
        Err(SupervisorError::ResourceNotFound | SupervisorError::InstanceNotFound) => {
            ModuleCallStatus::Denied
        }
        Err(_) => ModuleCallStatus::Fault,
    }
}

unsafe extern "C" fn abi_release_claim(
    _module: ModuleHandle,
    claim: ClaimId,
) -> ModuleCallStatus {
    match with_supervisor_mut(|supervisor| supervisor.release_claim_internal(claim)) {
        Ok(()) => ModuleCallStatus::Ok,
        Err(SupervisorError::ClaimNotFound) => ModuleCallStatus::Denied,
        Err(_) => ModuleCallStatus::Fault,
    }
}

unsafe extern "C" fn abi_subscribe_interrupt(
    _module: ModuleHandle,
    _irq: u8,
    _endpoint: EndpointId,
) -> ModuleCallStatus {
    ModuleCallStatus::Unsupported
}

unsafe extern "C" fn abi_register_lifecycle(
    _module: ModuleHandle,
    _endpoint: EndpointId,
) -> ModuleCallStatus {
    ModuleCallStatus::Ok
}

static MODULE_ABI_V1: ModuleAbiV1 = ModuleAbiV1 {
    abi_version: MODULE_ABI_VERSION,
    log: Some(abi_log),
    send_message: Some(abi_send_message),
    receive_message: Some(abi_receive_message),
    resolve_capability: Some(abi_resolve_capability),
    open_endpoint: Some(abi_open_endpoint),
    request_pages: Some(abi_request_pages),
    free_pages: Some(abi_free_pages),
    current_instance: Some(abi_current_instance),
    claim_resource: Some(abi_claim_resource),
    release_claim: Some(abi_release_claim),
    subscribe_interrupt: Some(abi_subscribe_interrupt),
    register_lifecycle: Some(abi_register_lifecycle),
};

pub fn abi() -> &'static ModuleAbiV1 {
    &MODULE_ABI_V1
}

pub fn bootstrap(boot_payload: u64) {
    SUPERVISOR.lock().reset(boot_payload);
}

pub fn install_module(descriptor: ModuleDescriptor) -> Result<ModuleHandle, SupervisorError> {
    SUPERVISOR.lock().install_descriptor(descriptor)
}

pub fn realize_boot_modules() -> Result<SupervisorBootReport, SupervisorError> {
    SUPERVISOR.lock().realize_boot_modules()
}

pub fn snapshot() -> Result<SupervisorSnapshot, SupervisorError> {
    let guard = SUPERVISOR.lock();
    guard.ensure_bootstrapped()?;
    Ok(guard.snapshot())
}

pub fn fault_module(handle: ModuleHandle) -> Result<(), SupervisorError> {
    SUPERVISOR.lock().fault_module(handle)
}

pub fn restart_module(handle: ModuleHandle) -> Result<(), SupervisorError> {
    SUPERVISOR.lock().restart_module(handle)
}

pub fn uninstall_module(handle: ModuleHandle) -> Result<(), SupervisorError> {
    SUPERVISOR.lock().uninstall_module(handle)
}

pub fn resolve_capability(namespace: &str, name: &str) -> Result<CapabilityToken, SupervisorError> {
    SUPERVISOR
        .lock()
        .resolve_capability(namespace, name)
        .ok_or(SupervisorError::CapabilityNotFound)
}

pub fn endpoint_for_token(token: CapabilityToken) -> Result<EndpointId, SupervisorError> {
    SUPERVISOR
        .lock()
        .endpoint_for_token(token)
        .ok_or(SupervisorError::CapabilityNotFound)
}

pub fn subscribe(consumer_endpoint: EndpointId, provider_endpoint: EndpointId) -> Result<(), SupervisorError> {
    SUPERVISOR.lock().subscribe(consumer_endpoint, provider_endpoint)
}

pub fn template_for_module(handle: ModuleHandle) -> Result<NodeTemplateId, SupervisorError> {
    SUPERVISOR.lock().template_for_module(handle)
}

pub fn spawn_instance(template_id: NodeTemplateId) -> Result<NodeInstanceId, SupervisorError> {
    SUPERVISOR.lock().spawn_instance(template_id)
}

pub fn schedule_instance(instance_id: NodeInstanceId) -> Result<(), SupervisorError> {
    SUPERVISOR.lock().enqueue_ready_instance(instance_id)
}

pub fn dequeue_ready_instance(
    preferred_lane: Option<ExecutionLaneClass>,
) -> Result<Option<NodeInstanceId>, SupervisorError> {
    SUPERVISOR.lock().dequeue_ready_instance(preferred_lane)
}

pub fn current_instance(handle: ModuleHandle) -> Result<NodeInstanceId, SupervisorError> {
    SUPERVISOR
        .lock()
        .active_instance_for_module(handle)
        .ok_or(SupervisorError::InstanceNotFound)
}

pub fn queue_restart(handle: ModuleHandle) -> Result<(), SupervisorError> {
    SUPERVISOR.lock().enqueue_restart_module(handle)
}

pub fn process_restart_queue() -> Result<Option<ModuleHandle>, SupervisorError> {
    SUPERVISOR.lock().process_next_restart()
}

pub fn service_system_cycle() {
    loop {
        let restarted = process_restart_queue().ok().flatten().is_some();
        gos_runtime::pump();
        // Drain node faults produced by this pump tick and apply fault policy.
        while let Some(fault_vec) = gos_runtime::drain_next_fault() {
            if let Some(pid) = gos_runtime::plugin_id_for_vec(fault_vec) {
                let module_id = ModuleId(pid.0);
                if let Some(handle) = SUPERVISOR.lock().find_module_by_module_id(module_id) {
                    let _ = SUPERVISOR.lock().fault_module(handle);
                }
            }
        }
        if !restarted {
            break;
        }
    }
}

pub fn claim_resource(
    instance_id: NodeInstanceId,
    resource_id: ResourceId,
    claim_policy: ClaimPolicy,
    preempt_policy: PreemptPolicy,
) -> Result<ResourceLease, SupervisorError> {
    SUPERVISOR
        .lock()
        .claim_resource(instance_id, resource_id, claim_policy, preempt_policy)
}

pub fn release_claim(claim_id: ClaimId) -> Result<(), SupervisorError> {
    SUPERVISOR.lock().release_claim_internal(claim_id)
}

pub fn drain_revocation(instance_id: NodeInstanceId) -> Result<Option<ResourceLease>, SupervisorError> {
    let mut guard = SUPERVISOR.lock();
    let _ = guard.find_instance_slot(instance_id)?;
    Ok(guard.drain_revocation(instance_id))
}

pub fn template_summary(template_id: NodeTemplateId) -> Result<NodeTemplateSummary, SupervisorError> {
    let guard = SUPERVISOR.lock();
    let slot = guard.find_template_slot(template_id)?;
    let record = guard.templates[slot];
    Ok(NodeTemplateSummary {
        template_id: record.id,
        module: record.module,
        spawn_policy: record.spawn_policy,
        lane: record.lane,
        heap_quota: record.heap_quota,
    })
}

pub fn instance_summary(instance_id: NodeInstanceId) -> Result<NodeInstanceSummary, SupervisorError> {
    let guard = SUPERVISOR.lock();
    let slot = guard.find_instance_slot(instance_id)?;
    let record = guard.instances[slot];
    Ok(NodeInstanceSummary {
        instance_id: record.id,
        template_id: record.template_id,
        module: record.module,
        lane: record.lane,
        lifecycle: record.lifecycle,
        ready_queued: record.ready_queued,
        heap_quota: record.heap_quota,
        heap_pages_used: record.heap_pages_used,
    })
}

pub fn claim_summary(claim_id: ClaimId) -> Result<ClaimSummary, SupervisorError> {
    let guard = SUPERVISOR.lock();
    let slot = guard.find_claim_slot(claim_id)?;
    let record = guard.claims[slot];
    Ok(ClaimSummary {
        claim_id: record.id,
        resource_id: record.resource,
        instance_id: record.instance,
        module: record.module,
        claim_policy: record.claim_policy,
        preempt_policy: record.preempt_policy,
        epoch: record.epoch,
        active: record.active,
    })
}

pub fn heap_grant_summary(module: ModuleHandle, base: u64) -> Result<HeapGrantSummary, SupervisorError> {
    let guard = SUPERVISOR.lock();
    let record = guard
        .heap_grants
        .iter()
        .find(|record| record.occupied && record.module == module && record.base == base)
        .copied()
        .ok_or(SupervisorError::HeapGrantNotFound)?;
    Ok(HeapGrantSummary {
        module: record.module,
        instance_id: record.instance,
        base: record.base,
        page_count: record.page_count,
        writable: record.writable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gos_protocol::{ModuleImageFormat, ModuleMessageHeader, ModuleMessageKind, ModuleSegmentKind};

    static INIT_COUNT: AtomicUsize = AtomicUsize::new(0);
    static START_COUNT: AtomicUsize = AtomicUsize::new(0);
    static STOP_COUNT: AtomicUsize = AtomicUsize::new(0);

    const TEST_EXPORTS: &[CapabilitySpec] = &[CapabilitySpec {
        namespace: "demo",
        name: "echo",
    }];

    const TEST_IMPORTS: &[ImportSpec] = &[ImportSpec {
        namespace: "demo",
        capability: "echo",
        required: true,
    }];

    const TEST_SEGMENTS: &[gos_protocol::ModuleImageSegment] = &[
        gos_protocol::ModuleImageSegment {
            kind: ModuleSegmentKind::Text,
            virt_addr: 0,
            mem_len: 0x4000,
            file_offset: 0,
            file_len: 0x2000,
            flags: 0,
        },
    ];

    unsafe extern "C" fn test_init(
        _abi: *const ModuleAbiV1,
        _handle: ModuleHandle,
        _domain: DomainId,
    ) -> ModuleCallStatus {
        INIT_COUNT.fetch_add(1, Ordering::SeqCst);
        ModuleCallStatus::Ok
    }

    unsafe extern "C" fn test_start(
        _abi: *const ModuleAbiV1,
        _handle: ModuleHandle,
        _domain: DomainId,
    ) -> ModuleCallStatus {
        START_COUNT.fetch_add(1, Ordering::SeqCst);
        ModuleCallStatus::Ok
    }

    unsafe extern "C" fn test_stop(
        _abi: *const ModuleAbiV1,
        _handle: ModuleHandle,
        _domain: DomainId,
    ) -> ModuleCallStatus {
        STOP_COUNT.fetch_add(1, Ordering::SeqCst);
        ModuleCallStatus::Ok
    }

    const TEST_ENTRY: ModuleEntry = ModuleEntry {
        module_init: Some(test_init),
        module_start: Some(test_start),
        module_stop: Some(test_stop),
        module_suspend: None,
        module_resume: None,
    };

    const PROVIDER: ModuleDescriptor = ModuleDescriptor {
        abi_version: MODULE_ABI_VERSION,
        module_id: gos_protocol::ModuleId::from_ascii("MOD.PROVIDER"),
        name: "MOD_PROVIDER",
        version: 1,
        image_format: ModuleImageFormat::Builtin,
        fault_policy: ModuleFaultPolicy::RestartAlways,
        dependencies: &[],
        permissions: &[],
        exports: TEST_EXPORTS,
        imports: &[],
        segments: TEST_SEGMENTS,
        entry: TEST_ENTRY,
        signature: None,
        flags: 0,
    };

    const CONSUMER_DEPS: &[gos_protocol::ModuleDependencySpec] = &[gos_protocol::ModuleDependencySpec {
        module_id: gos_protocol::ModuleId::from_ascii("MOD.PROVIDER"),
        required: true,
    }];

    const CONSUMER: ModuleDescriptor = ModuleDescriptor {
        abi_version: MODULE_ABI_VERSION,
        module_id: gos_protocol::ModuleId::from_ascii("MOD.CONSUMER"),
        name: "MOD_CONSUMER",
        version: 1,
        image_format: ModuleImageFormat::Builtin,
        fault_policy: ModuleFaultPolicy::Manual,
        dependencies: CONSUMER_DEPS,
        permissions: &[],
        exports: &[],
        imports: TEST_IMPORTS,
        segments: TEST_SEGMENTS,
        entry: ModuleEntry::NONE,
        signature: None,
        flags: 0,
    };

    fn reset_counts() {
        INIT_COUNT.store(0, Ordering::SeqCst);
        START_COUNT.store(0, Ordering::SeqCst);
        STOP_COUNT.store(0, Ordering::SeqCst);
    }

    #[test]
    fn realize_descriptor_modules_builds_domains_and_capabilities() {
        reset_counts();
        bootstrap(0);
        let provider = install_module(PROVIDER).expect("provider install");
        let consumer = install_module(CONSUMER).expect("consumer install");
        let report = realize_boot_modules().expect("realize");
        let snapshot = snapshot().expect("snapshot");

        assert_eq!(report.discovered_modules, 2);
        assert_eq!(report.running_modules, 2);
        assert_eq!(snapshot.installed_modules, 2);
        assert_eq!(snapshot.running_modules, 2);
        assert_eq!(snapshot.isolated_domains, 2);
        assert!(snapshot.published_capabilities >= 1);
        assert_eq!(INIT_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(START_COUNT.load(Ordering::SeqCst), 1);

        let token = resolve_capability("demo", "echo").expect("capability token");
        let endpoint = endpoint_for_token(token).expect("endpoint");
        let guard = SUPERVISOR.lock();
        let provider_slot = guard.find_module_slot(provider).expect("provider slot");
        let consumer_slot = guard.find_module_slot(consumer).expect("consumer slot");
        assert_ne!(guard.modules[provider_slot].domain.root_table_phys, 0);
        assert_ne!(guard.modules[consumer_slot].domain.root_table_phys, 0);
        assert_ne!(endpoint, EndpointId::ZERO);
    }

    #[test]
    fn abi_message_round_trip_targets_endpoint_owner() {
        reset_counts();
        bootstrap(0);
        let provider = install_module(PROVIDER).expect("provider install");
        let _consumer = install_module(CONSUMER).expect("consumer install");
        realize_boot_modules().expect("realize");

        let token = resolve_capability("demo", "echo").expect("capability token");
        let endpoint = endpoint_for_token(token).expect("endpoint");

        let message = ModuleMessage {
            header: ModuleMessageHeader {
                kind: ModuleMessageKind::Data,
                from: EndpointId::new(7),
                to: EndpointId::ZERO,
                token,
                length: 3,
                _reserved: 0,
            },
            payload: [1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };

        let send = abi().send_message.expect("send");
        let recv = abi().receive_message.expect("receive");
        assert_eq!(
            unsafe { send(provider, endpoint, &message) },
            ModuleCallStatus::Ok
        );

        let mut out = ModuleMessage::EMPTY;
        assert_eq!(
            unsafe { recv(provider, endpoint, &mut out) },
            ModuleCallStatus::Ok
        );
        assert_eq!(out.header.to, endpoint);
        assert_eq!(out.payload[0], 1);
        assert_eq!(out.payload[1], 2);
        assert_eq!(out.payload[2], 3);
    }

    #[test]
    fn fault_policy_restart_restarts_descriptor_module() {
        reset_counts();
        bootstrap(0);
        let provider = install_module(PROVIDER).expect("provider install");
        realize_boot_modules().expect("realize");

        assert_eq!(START_COUNT.load(Ordering::SeqCst), 1);
        fault_module(provider).expect("fault");
        assert_eq!(START_COUNT.load(Ordering::SeqCst), 2);
        assert_eq!(STOP_COUNT.load(Ordering::SeqCst), 0);
    }
}
