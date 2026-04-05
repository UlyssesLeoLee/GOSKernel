#![cfg_attr(not(any(test, feature = "host-testing")), no_std)]

#[cfg(any(test, feature = "host-testing"))]
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use core::sync::atomic::AtomicUsize;

use gos_protocol::{
    fixed_bytes_16, CapabilitySpec, CapabilityToken, DomainId, EndpointId, ImportSpec,
    ModuleAbiV1, ModuleCallStatus, ModuleDescriptor, ModuleEntry, ModuleFaultPolicy, ModuleId,
    ModuleHandle, ModuleLifecycle, ModuleMessage, PermissionSpec, PluginManifest,
    MODULE_ABI_VERSION,
};
#[cfg(all(feature = "kernel-vmm", not(any(test, feature = "host-testing"))))]
use k_vmm;
use spin::Mutex;

pub const MAX_MODULES: usize = 32;
pub const MAX_CAPABILITIES: usize = 128;
pub const MAX_ENDPOINTS: usize = 128;
pub const MAX_SUBSCRIPTIONS: usize = 128;
pub const MAX_QUEUED_MESSAGES: usize = 256;

const DOMAIN_BASE: u64 = 0xFFFF_9000_0000_0000;
const DOMAIN_STRIDE: u64 = 0x0000_0000_0200_0000;
const DEFAULT_IMAGE_WINDOW: u64 = 0x0000_0000_0010_0000;
const DEFAULT_STACK_WINDOW: u64 = 0x0000_0000_0002_0000;
const DEFAULT_IPC_WINDOW: u64 = 0x0000_0000_0002_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorError {
    NotBootstrapped,
    ModuleTableFull,
    CapabilityTableFull,
    EndpointTableFull,
    SubscriptionTableFull,
    QueueFull,
    ModuleNotFound,
    EndpointNotFound,
    CapabilityNotFound,
    InvalidState,
    ModuleRejected,
    DomainCreateFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorSnapshot {
    pub installed_modules: usize,
    pub running_modules: usize,
    pub isolated_domains: usize,
    pub published_capabilities: usize,
    pub endpoints: usize,
    pub queued_messages: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorBootReport {
    pub discovered_modules: usize,
    pub running_modules: usize,
    pub isolated_domains: usize,
    pub compat_bridges: usize,
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
        isolated: false,
    };
}

#[derive(Clone, Copy)]
enum ModuleSource {
    Empty,
    Descriptor(ModuleDescriptor),
    Compat(PluginManifest),
}

impl ModuleSource {
    fn module_id(&self) -> ModuleId {
        match *self {
            Self::Descriptor(descriptor) => descriptor.module_id,
            Self::Compat(manifest) => ModuleId::new(manifest.plugin_id.0),
            Self::Empty => ModuleId::ZERO,
        }
    }

    fn fault_policy(&self) -> ModuleFaultPolicy {
        match *self {
            Self::Descriptor(descriptor) => descriptor.fault_policy,
            Self::Compat(manifest) => default_fault_policy(manifest.name),
            Self::Empty => ModuleFaultPolicy::Manual,
        }
    }

    fn permissions(&self) -> &'static [PermissionSpec] {
        match *self {
            Self::Descriptor(descriptor) => descriptor.permissions,
            Self::Compat(manifest) => manifest.permissions,
            Self::Empty => &[],
        }
    }

    fn exports(&self) -> &'static [CapabilitySpec] {
        match *self {
            Self::Descriptor(descriptor) => descriptor.exports,
            Self::Compat(manifest) => manifest.exports,
            Self::Empty => &[],
        }
    }

    fn imports(&self) -> &'static [ImportSpec] {
        match *self {
            Self::Descriptor(descriptor) => descriptor.imports,
            Self::Compat(manifest) => manifest.imports,
            Self::Empty => &[],
        }
    }

    fn entry(&self) -> ModuleEntry {
        match *self {
            Self::Descriptor(descriptor) => descriptor.entry,
            Self::Compat(_) | Self::Empty => ModuleEntry::NONE,
        }
    }

    fn is_compat(&self) -> bool {
        matches!(self, Self::Compat(_))
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
            Self::Compat(_) | Self::Empty => DEFAULT_IMAGE_WINDOW,
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

#[derive(Clone, Copy)]
struct ModuleRecord {
    occupied: bool,
    handle: ModuleHandle,
    source: ModuleSource,
    state: ModuleLifecycle,
    domain: ModuleDomain,
    restart_generation: u32,
}

impl ModuleRecord {
    const EMPTY: Self = Self {
        occupied: false,
        handle: ModuleHandle::ZERO,
        source: ModuleSource::Empty,
        state: ModuleLifecycle::Stopped,
        domain: ModuleDomain::EMPTY,
        restart_generation: 0,
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

struct Supervisor {
    boot_payload: u64,
    bootstrapped: bool,
    next_handle: u64,
    next_domain: u64,
    next_endpoint: u64,
    next_capability: u64,
    modules: [ModuleRecord; MAX_MODULES],
    capabilities: [CapabilityRecord; MAX_CAPABILITIES],
    endpoints: [EndpointRecord; MAX_ENDPOINTS],
    subscriptions: [SubscriptionRecord; MAX_SUBSCRIPTIONS],
    messages: [QueuedMessage; MAX_QUEUED_MESSAGES],
}

impl Supervisor {
    const fn new() -> Self {
        Self {
            boot_payload: 0,
            bootstrapped: false,
            next_handle: 1,
            next_domain: 1,
            next_endpoint: 1,
            next_capability: 1,
            modules: [ModuleRecord::EMPTY; MAX_MODULES],
            capabilities: [CapabilityRecord::EMPTY; MAX_CAPABILITIES],
            endpoints: [EndpointRecord::EMPTY; MAX_ENDPOINTS],
            subscriptions: [SubscriptionRecord::EMPTY; MAX_SUBSCRIPTIONS],
            messages: [QueuedMessage::EMPTY; MAX_QUEUED_MESSAGES],
        }
    }

    fn reset(&mut self, boot_payload: u64) {
        *self = Self::new();
        self.boot_payload = boot_payload;
        self.bootstrapped = true;
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
            restart_generation: 0,
        };
        Ok(handle)
    }

    fn install_descriptor(&mut self, descriptor: ModuleDescriptor) -> Result<ModuleHandle, SupervisorError> {
        self.install_source(ModuleSource::Descriptor(descriptor))
    }

    fn install_compat_manifest(&mut self, manifest: PluginManifest) -> Result<ModuleHandle, SupervisorError> {
        self.install_source(ModuleSource::Compat(manifest))
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

    fn call_entry(&self, handle: ModuleHandle, entry_fn: Option<unsafe extern "C" fn(abi: *const ModuleAbiV1, handle: ModuleHandle, domain: DomainId) -> ModuleCallStatus>) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        let domain = self.modules[slot].domain.id;
        let Some(callback) = entry_fn else {
            return Ok(());
        };
        let status = unsafe { callback(&MODULE_ABI_V1, handle, domain) };
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
        self.call_entry(handle, entry.module_init)?;
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
        self.call_entry(handle, entry.module_start)?;
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
        self.call_entry(handle, entry.module_stop)?;
        self.modules[slot].state = ModuleLifecycle::Quiescing;
        self.revoke_capabilities(handle);
        self.drain_messages(handle);
        self.modules[slot].state = ModuleLifecycle::Stopped;
        Ok(())
    }

    fn restart_module(&mut self, handle: ModuleHandle) -> Result<(), SupervisorError> {
        let slot = self.find_module_slot(handle)?;
        match self.modules[slot].state {
            ModuleLifecycle::Running
            | ModuleLifecycle::Instantiated
            | ModuleLifecycle::Mapped => {
                let _ = self.stop_module(handle);
            }
            ModuleLifecycle::Faulted => {
                self.revoke_capabilities(handle);
                self.drain_messages(handle);
                self.modules[slot].state = ModuleLifecycle::Stopped;
            }
            ModuleLifecycle::Stopped | ModuleLifecycle::Installed => {}
            _ => return Err(SupervisorError::InvalidState),
        }
        self.modules[slot].restart_generation = self.modules[slot].restart_generation.wrapping_add(1);
        self.validate_module(handle)?;
        self.map_module(handle)?;
        self.instantiate_module(handle)?;
        self.start_module(handle)?;
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
        let compat_bridges = self
            .modules
            .iter()
            .filter(|record| record.occupied && record.source.is_compat())
            .count();
        Ok(SupervisorBootReport {
            discovered_modules: snapshot.installed_modules,
            running_modules: snapshot.running_modules,
            isolated_domains: snapshot.isolated_domains,
            compat_bridges,
            published_capabilities: snapshot.published_capabilities,
        })
    }

    fn snapshot(&self) -> SupervisorSnapshot {
        SupervisorSnapshot {
            installed_modules: self.modules.iter().filter(|record| record.occupied).count(),
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
            published_capabilities: self.capabilities.iter().filter(|record| record.occupied).count(),
            endpoints: self.endpoints.iter().filter(|record| record.occupied).count(),
            queued_messages: self.messages.iter().filter(|record| record.occupied).count(),
        }
    }
}

fn default_fault_policy(name: &'static str) -> ModuleFaultPolicy {
    match name {
        "K_VGA" | "K_IME" | "K_NET" | "K_MOUSE" | "K_CYPHER" | "K_SHELL" | "K_AI" => {
            ModuleFaultPolicy::RestartAlways
        }
        _ => ModuleFaultPolicy::FaultKernelDegraded,
    }
}

static SUPERVISOR: Mutex<Supervisor> = Mutex::new(Supervisor::new());

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
    match SUPERVISOR.lock().post_message(endpoint, unsafe { *message }) {
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
    match SUPERVISOR.lock().receive_message(module, endpoint) {
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
    match SUPERVISOR.lock().resolve_capability(namespace, name) {
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
    match SUPERVISOR.lock().open_endpoint(module, label) {
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
    _module: ModuleHandle,
    _page_count: usize,
    _writable: u8,
    _out_base: *mut u64,
) -> ModuleCallStatus {
    ModuleCallStatus::Unsupported
}

unsafe extern "C" fn abi_free_pages(
    _module: ModuleHandle,
    _base: u64,
    _page_count: usize,
) -> ModuleCallStatus {
    ModuleCallStatus::Unsupported
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

pub fn install_compat_manifest(manifest: PluginManifest) -> Result<ModuleHandle, SupervisorError> {
    SUPERVISOR.lock().install_compat_manifest(manifest)
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
        assert_eq!(report.compat_bridges, 0);
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
