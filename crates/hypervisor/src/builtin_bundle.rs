use gos_protocol::{
    derive_edge_id, derive_node_id, BootContext, CapabilitySpec, EdgeSpec, EntryPolicy,
    ImportSpec, ModuleDependencySpec, ModuleDescriptor, ModuleEntry,
    ModuleFaultPolicy, ModuleId, ModuleImageFormat, NodeExecutorVTable, NodeSpec,
    PermissionKind, PermissionSpec, PluginId, PluginManifest, RoutePolicy,
    RuntimeEdgeType, RuntimeNodeType, Signal, VectorAddress, GOS_ABI_VERSION,
    MODULE_ABI_VERSION,
};
use gos_runtime::{self, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinBootError {
    AbiVersionMismatch(PluginId),
    MissingDependency(PluginId),
    PermissionDenied(PluginId),
    UnresolvedImport(PluginId, &'static str),
    Runtime(RuntimeError),
}

impl From<RuntimeError> for BuiltinBootError {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinBootReport {
    pub discovered_plugins: usize,
    pub loaded_plugins: usize,
    pub stable_after_load: bool,
}

#[derive(Clone, Copy)]
struct NativeNodeBinding {
    vector: VectorAddress,
    local_node_key: &'static str,
    executor: NodeExecutorVTable,
}

#[derive(Clone, Copy)]
struct NativeModule {
    manifest: PluginManifest,
    granted_permissions: &'static [PermissionSpec],
    nodes: &'static [NativeNodeBinding],
    register_hook: Option<fn(&mut BootContext)>,
}

#[derive(Clone, Copy)]
enum BuiltinModule {
    Native(NativeModule),
}

impl BuiltinModule {
    const fn manifest(&self) -> PluginManifest {
        match *self {
            Self::Native(module) => module.manifest,
        }
    }

    const fn plugin_id(&self) -> PluginId {
        self.manifest().plugin_id
    }
}

const K_PANIC_ID: PluginId = PluginId::from_ascii("K_PANIC");
const K_SERIAL_ID: PluginId = PluginId::from_ascii("K_SERIAL");
const K_VGA_ID: PluginId = PluginId::from_ascii("K_VGA");
const K_GDT_ID: PluginId = PluginId::from_ascii("K_GDT");
const K_CPUID_ID: PluginId = PluginId::from_ascii("K_CPUID");
const K_PIC_ID: PluginId = PluginId::from_ascii("K_PIC");
const K_PIT_ID: PluginId = PluginId::from_ascii("K_PIT");
const K_PS2_ID: PluginId = PluginId::from_ascii("K_PS2");
const K_IDT_ID: PluginId = PluginId::from_ascii("K_IDT");
const K_PMM_ID: PluginId = PluginId::from_ascii("K_PMM");
const K_VMM_ID: PluginId = PluginId::from_ascii("K_VMM");
const K_HEAP_ID: PluginId = PluginId::from_ascii("K_HEAP");
const K_IME_ID: PluginId = PluginId::from_ascii("K_IME");
const K_NET_ID: PluginId = PluginId::from_ascii("K_NET");
const K_MOUSE_ID: PluginId = PluginId::from_ascii("K_MOUSE");
const K_CYPHER_ID: PluginId = PluginId::from_ascii("K_CYPHER");
const K_CUDA_ID: PluginId = PluginId::from_ascii("K_CUDA");
const K_SHELL_ID: PluginId = PluginId::from_ascii("K_SHELL");
const K_AI_ID: PluginId = PluginId::from_ascii("K_AI");
const K_CHAT_ID: PluginId = PluginId::from_ascii("K_CHAT");
const K_NIM_ID:  PluginId = PluginId::from_ascii("K_NIM");

const NONE_PERMS: &[PermissionSpec] = &[];

const SERIAL_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x3F8, arg1: 8 },
];
const VGA_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PhysMap, arg0: 0xA0000, arg1: 65536 },
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x3C8, arg1: 0x3C9 },
];
const IRQ_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: u64::MAX, arg1: 0 },
];
const PIC_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x20, arg1: 0xA1 },
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: u64::MAX, arg1: 0 },
];
const PS2_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x60, arg1: 0x64 },
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: 1, arg1: 0 },
];
const PIT_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x40, arg1: 0x43 },
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: 0, arg1: 0 },
];
const MEM_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PhysMap, arg0: u64::MAX, arg1: u64::MAX },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
];
const SHELL_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::GraphRead, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::ExternalSync, arg0: 0, arg1: 0 },
];
const CYPHER_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::GraphRead, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
];
const CHAT_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x2F8, arg1: 8 }, // COM2
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityExport, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::ExternalSync, arg0: 0, arg1: 0 },
];
const NIM_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityExport, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::ExternalSync, arg0: 0, arg1: 0 },
];
const AI_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::GraphRead, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityExport, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::ExternalSync, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::ScheduleHint, arg0: 0, arg1: 0 },
];
const CUDA_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityExport, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::ExternalSync, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::ScheduleHint, arg0: 0, arg1: 0 },
];
const IME_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::GraphRead, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityExport, arg0: 0, arg1: 0 },
];
const NET_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0xCF8, arg1: 8 },
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityExport, arg0: 0, arg1: 0 },
];
const MOUSE_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x60, arg1: 0x64 },
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: 12, arg1: 0 },
    PermissionSpec { kind: PermissionKind::CapabilityConsume, arg0: 0, arg1: 0 },
];

const VGA_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "console", name: "write" },
    CapabilitySpec { namespace: "display", name: "pointer" },
];
const SERIAL_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "serial", name: "write" },
];
const PMM_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "memory", name: "frame_alloc" },
];
const VMM_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "memory", name: "map_page" },
];
const HEAP_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "memory", name: "alloc" },
];
const SHELL_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "shell", name: "input" },
];
const CLIPBOARD_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "clipboard", name: "buffer" },
];
const CHAT_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "chat", name: "bridge" },
];
const NIM_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "nim", name: "inference" },
];
const AI_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "ai", name: "supervisor" },
    CapabilitySpec { namespace: "graph", name: "orchestrate" },
];
const CUDA_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "cuda", name: "bridge" },
];
const CYPHER_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "cypher", name: "query" },
];
const IME_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "ime", name: "control" },
];
const NET_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "net", name: "uplink" },
];

const SHELL_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
    ImportSpec { namespace: "ime", capability: "control", required: true },
    ImportSpec { namespace: "ai", capability: "supervisor", required: true },
    ImportSpec { namespace: "cypher", capability: "query", required: true },
    ImportSpec { namespace: "net", capability: "uplink", required: true },
    ImportSpec { namespace: "cuda", capability: "bridge", required: true },
];
const CYPHER_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
];
const CUDA_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
    ImportSpec { namespace: "serial", capability: "write", required: true },
];
const CHAT_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
    ImportSpec { namespace: "net",     capability: "uplink", required: false },
];
const NIM_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
    ImportSpec { namespace: "net",     capability: "uplink", required: false },
];
const AI_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
    ImportSpec { namespace: "shell", capability: "input", required: true },
];
const IME_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "shell", capability: "input", required: true },
];
const NET_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
];
const MOUSE_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "display", capability: "pointer", required: true },
];
const PS2_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "shell", capability: "input", required: true },
];

const DEP_PIT: &[PluginId] = &[K_PIC_ID];
const DEP_PS2: &[PluginId] = &[K_PIC_ID];
const DEP_IDT: &[PluginId] = &[K_GDT_ID, K_PIT_ID, K_PS2_ID];
const DEP_VMM: &[PluginId] = &[K_PMM_ID];
const DEP_HEAP: &[PluginId] = &[K_PMM_ID, K_VMM_ID];
const DEP_NET: &[PluginId] = &[K_VGA_ID];
const DEP_MOUSE: &[PluginId] = &[K_VGA_ID, K_PS2_ID, K_IDT_ID];
const DEP_CYPHER: &[PluginId] = &[K_VGA_ID];
const DEP_CUDA: &[PluginId] = &[K_VGA_ID, K_SERIAL_ID];
const DEP_SHELL: &[PluginId] = &[K_VGA_ID, K_PS2_ID, K_HEAP_ID, K_IME_ID, K_NET_ID, K_CYPHER_ID, K_CUDA_ID];
const DEP_CHAT: &[PluginId] = &[K_VGA_ID, K_NET_ID];
const DEP_NIM:  &[PluginId] = &[K_VGA_ID, K_NET_ID];
const DEP_AI: &[PluginId] = &[K_SHELL_ID];

const MOD_DEP_PIT: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: module_id(K_PIC_ID),
    required: true,
}];
const MOD_DEP_PS2: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: module_id(K_PIC_ID),
    required: true,
}];
const MOD_DEP_IDT: &[ModuleDependencySpec] = &[
    ModuleDependencySpec {
        module_id: module_id(K_GDT_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_PIT_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_PS2_ID),
        required: true,
    },
];
const MOD_DEP_VMM: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: module_id(K_PMM_ID),
    required: true,
}];
const MOD_DEP_HEAP: &[ModuleDependencySpec] = &[
    ModuleDependencySpec {
        module_id: module_id(K_PMM_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_VMM_ID),
        required: true,
    },
];
const MOD_DEP_NET: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: module_id(K_VGA_ID),
    required: true,
}];
const MOD_DEP_MOUSE: &[ModuleDependencySpec] = &[
    ModuleDependencySpec {
        module_id: module_id(K_VGA_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_PS2_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_IDT_ID),
        required: true,
    },
];
const MOD_DEP_CYPHER: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: module_id(K_VGA_ID),
    required: true,
}];
const MOD_DEP_CUDA: &[ModuleDependencySpec] = &[
    ModuleDependencySpec {
        module_id: module_id(K_VGA_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_SERIAL_ID),
        required: true,
    },
];
const MOD_DEP_SHELL: &[ModuleDependencySpec] = &[
    ModuleDependencySpec {
        module_id: module_id(K_VGA_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_PS2_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_HEAP_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_IME_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_NET_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_CYPHER_ID),
        required: true,
    },
    ModuleDependencySpec {
        module_id: module_id(K_CUDA_ID),
        required: true,
    },
];
const MOD_DEP_CHAT: &[ModuleDependencySpec] = &[
    ModuleDependencySpec { module_id: module_id(K_VGA_ID), required: true },
    ModuleDependencySpec { module_id: module_id(K_NET_ID), required: false },
];
const MOD_DEP_NIM: &[ModuleDependencySpec] = &[
    ModuleDependencySpec { module_id: module_id(K_VGA_ID), required: true },
    ModuleDependencySpec { module_id: module_id(K_NET_ID), required: false },
];
const MOD_DEP_AI: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: module_id(K_SHELL_ID),
    required: true,
}];

const PANIC_MANIFEST: PluginManifest = manifest_with_nodes(
    K_PANIC_ID,
    "K_PANIC",
    &[],
    NONE_PERMS,
    &[],
    &[],
    PANIC_NODE_SPECS,
);
const SERIAL_MANIFEST: PluginManifest = manifest_with_nodes(
    K_SERIAL_ID,
    "K_SERIAL",
    &[],
    SERIAL_PERMS,
    SERIAL_EXPORTS,
    &[],
    SERIAL_NODE_SPECS,
);
const K_PANIC_NODE_ID: gos_protocol::NodeId = derive_node_id(K_PANIC_ID, "panic.entry");
const K_SERIAL_NODE_ID: gos_protocol::NodeId = derive_node_id(K_SERIAL_ID, "serial.entry");
const K_GDT_NODE_ID: gos_protocol::NodeId = derive_node_id(K_GDT_ID, "gdt.entry");
const K_CPUID_NODE_ID: gos_protocol::NodeId = derive_node_id(K_CPUID_ID, "cpuid.entry");
const K_PIC_NODE_ID: gos_protocol::NodeId = derive_node_id(K_PIC_ID, "pic.entry");
const K_PIT_NODE_ID: gos_protocol::NodeId = derive_node_id(K_PIT_ID, "pit.entry");
const K_IDT_NODE_ID: gos_protocol::NodeId = derive_node_id(K_IDT_ID, "idt.entry");
const K_PS2_NODE_ID: gos_protocol::NodeId = derive_node_id(K_PS2_ID, "ps2.entry");
const K_PMM_NODE_ID: gos_protocol::NodeId = derive_node_id(K_PMM_ID, "pmm.entry");
const K_VMM_NODE_ID: gos_protocol::NodeId = derive_node_id(K_VMM_ID, "vmm.entry");
const K_HEAP_NODE_ID: gos_protocol::NodeId = derive_node_id(K_HEAP_ID, "heap.entry");
const K_VGA_NODE_ID: gos_protocol::NodeId = derive_node_id(K_VGA_ID, "vga.entry");
const K_IME_NODE_ID: gos_protocol::NodeId = derive_node_id(K_IME_ID, "ime.router");
const K_NET_NODE_ID: gos_protocol::NodeId = derive_node_id(K_NET_ID, "net.uplink");
const K_MOUSE_NODE_ID: gos_protocol::NodeId = derive_node_id(K_MOUSE_ID, "mouse.pointer");
const K_CYPHER_NODE_ID: gos_protocol::NodeId = derive_node_id(K_CYPHER_ID, "cypher.query");
const K_CUDA_NODE_ID: gos_protocol::NodeId = derive_node_id(K_CUDA_ID, "cuda.bridge");
const K_SHELL_NODE_ID: gos_protocol::NodeId = derive_node_id(K_SHELL_ID, "shell.entry");
const K_THEME_WABI_NODE_ID: gos_protocol::NodeId = derive_node_id(K_SHELL_ID, "theme.wabi");
const K_THEME_SHOJI_NODE_ID: gos_protocol::NodeId = derive_node_id(K_SHELL_ID, "theme.shoji");
const K_THEME_CURRENT_NODE_ID: gos_protocol::NodeId = derive_node_id(K_SHELL_ID, "theme.current");
const K_CLIPBOARD_NODE_ID: gos_protocol::NodeId = derive_node_id(K_SHELL_ID, "clipboard.mount");
const K_AI_NODE_ID: gos_protocol::NodeId = derive_node_id(K_AI_ID, "ai.supervisor");
const K_CHAT_NODE_ID: gos_protocol::NodeId = derive_node_id(K_CHAT_ID, "chat.bridge");
const K_NIM_NODE_ID:  gos_protocol::NodeId = derive_node_id(K_NIM_ID,  "nim.inference");

const PANIC_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_PANIC_NODE_ID,
    local_node_key: "panic.entry",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_panic::EXECUTOR_ID,
    state_schema_hash: 0x2001,
    permissions: NONE_PERMS,
    exports: &[],
    vector_ref: None,
}];

const SERIAL_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_SERIAL_NODE_ID,
    local_node_key: "serial.entry",
    node_type: RuntimeNodeType::Driver,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_serial::EXECUTOR_ID,
    state_schema_hash: 0x2002,
    permissions: SERIAL_PERMS,
    exports: SERIAL_EXPORTS,
    vector_ref: None,
}];

const GDT_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_GDT_NODE_ID,
    local_node_key: "gdt.entry",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_gdt::EXECUTOR_ID,
    state_schema_hash: 0x2004,
    permissions: MEM_PERMS,
    exports: &[],
    vector_ref: None,
}];

const CPUID_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_CPUID_NODE_ID,
    local_node_key: "cpuid.entry",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_cpuid::EXECUTOR_ID,
    state_schema_hash: 0x2005,
    permissions: NONE_PERMS,
    exports: &[],
    vector_ref: None,
}];

const PIC_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_PIC_NODE_ID,
    local_node_key: "pic.entry",
    node_type: RuntimeNodeType::Driver,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_pic::EXECUTOR_ID,
    state_schema_hash: 0x2006,
    permissions: PIC_PERMS,
    exports: &[],
    vector_ref: None,
}];

const PIT_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_PIT_NODE_ID,
    local_node_key: "pit.entry",
    node_type: RuntimeNodeType::Driver,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_pit::EXECUTOR_ID,
    state_schema_hash: 0x2007,
    permissions: PIT_PERMS,
    exports: &[],
    vector_ref: None,
}];

const IDT_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_IDT_NODE_ID,
    local_node_key: "idt.entry",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_idt::EXECUTOR_ID,
    state_schema_hash: 0x2009,
    permissions: IRQ_PERMS,
    exports: &[],
    vector_ref: None,
}];

const PS2_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_PS2_NODE_ID,
    local_node_key: "ps2.entry",
    node_type: RuntimeNodeType::Driver,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_ps2::EXECUTOR_ID,
    state_schema_hash: 0x2008,
    permissions: PS2_PERMS,
    exports: &[],
    vector_ref: None,
}];

const PMM_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_PMM_NODE_ID,
    local_node_key: "pmm.entry",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_pmm::EXECUTOR_ID,
    state_schema_hash: 0x200A,
    permissions: MEM_PERMS,
    exports: PMM_EXPORTS,
    vector_ref: None,
}];

const VMM_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_VMM_NODE_ID,
    local_node_key: "vmm.entry",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_vmm::EXECUTOR_ID,
    state_schema_hash: 0x200B,
    permissions: MEM_PERMS,
    exports: VMM_EXPORTS,
    vector_ref: None,
}];

const HEAP_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_HEAP_NODE_ID,
    local_node_key: "heap.entry",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_heap::EXECUTOR_ID,
    state_schema_hash: 0x200C,
    permissions: MEM_PERMS,
    exports: HEAP_EXPORTS,
    vector_ref: None,
}];

const VGA_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_VGA_NODE_ID,
    local_node_key: "vga.entry",
    node_type: RuntimeNodeType::Driver,
    entry_policy: EntryPolicy::Bootstrap,
    executor_id: k_vga::EXECUTOR_ID,
    state_schema_hash: 0x2003,
    permissions: VGA_PERMS,
    exports: VGA_EXPORTS,
    vector_ref: None,
}];

const IME_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_IME_NODE_ID,
    local_node_key: "ime.router",
    node_type: RuntimeNodeType::Router,
    entry_policy: EntryPolicy::OnDemand,
    executor_id: k_ime::EXECUTOR_ID,
    state_schema_hash: 0x2011,
    permissions: IME_PERMS,
    exports: IME_EXPORTS,
    vector_ref: None,
}];

const NET_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_NET_NODE_ID,
    local_node_key: "net.uplink",
    node_type: RuntimeNodeType::Driver,
    entry_policy: EntryPolicy::Background,
    executor_id: k_net::EXECUTOR_ID,
    state_schema_hash: 0x2015,
    permissions: NET_PERMS,
    exports: NET_EXPORTS,
    vector_ref: None,
}];

const MOUSE_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_MOUSE_NODE_ID,
    local_node_key: "mouse.pointer",
    node_type: RuntimeNodeType::Driver,
    entry_policy: EntryPolicy::Background,
    executor_id: k_mouse::EXECUTOR_ID,
    state_schema_hash: 0x2013,
    permissions: MOUSE_PERMS,
    exports: &[],
    vector_ref: None,
}];

const CYPHER_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_CYPHER_NODE_ID,
    local_node_key: "cypher.query",
    node_type: RuntimeNodeType::Router,
    entry_policy: EntryPolicy::OnDemand,
    executor_id: k_cypher::EXECUTOR_ID,
    state_schema_hash: 0x2014,
    permissions: CYPHER_PERMS,
    exports: CYPHER_EXPORTS,
    vector_ref: None,
}];

const CUDA_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_CUDA_NODE_ID,
    local_node_key: "cuda.bridge",
    node_type: RuntimeNodeType::Compute,
    entry_policy: EntryPolicy::Background,
    executor_id: k_cuda_host::EXECUTOR_ID,
    state_schema_hash: 0x2016,
    permissions: CUDA_PERMS,
    exports: CUDA_EXPORTS,
    vector_ref: None,
}];

const SHELL_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_SHELL_NODE_ID,
    local_node_key: "shell.entry",
    node_type: RuntimeNodeType::PluginEntry,
    entry_policy: EntryPolicy::Manual,
    executor_id: k_shell::EXECUTOR_ID,
    state_schema_hash: 0x200E,
    permissions: SHELL_PERMS,
    exports: SHELL_EXPORTS,
    vector_ref: None,
}, NodeSpec {
    node_id: K_THEME_CURRENT_NODE_ID,
    local_node_key: "theme.current",
    node_type: RuntimeNodeType::Vector,
    entry_policy: EntryPolicy::Manual,
    executor_id: k_shell::THEME_EXECUTOR_ID,
    state_schema_hash: 0x2019,
    permissions: SHELL_PERMS,
    exports: &[],
    vector_ref: None,
}, NodeSpec {
    node_id: K_CLIPBOARD_NODE_ID,
    local_node_key: "clipboard.mount",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::OnDemand,
    executor_id: k_shell::CLIPBOARD_EXECUTOR_ID,
    state_schema_hash: 0x2020,
    permissions: SHELL_PERMS,
    exports: CLIPBOARD_EXPORTS,
    vector_ref: None,
}, NodeSpec {
    node_id: K_THEME_WABI_NODE_ID,
    local_node_key: "theme.wabi",
    node_type: RuntimeNodeType::Vector,
    entry_policy: EntryPolicy::Manual,
    executor_id: k_shell::THEME_EXECUTOR_ID,
    state_schema_hash: 0x2017,
    permissions: SHELL_PERMS,
    exports: &[],
    vector_ref: None,
}, NodeSpec {
    node_id: K_THEME_SHOJI_NODE_ID,
    local_node_key: "theme.shoji",
    node_type: RuntimeNodeType::Vector,
    entry_policy: EntryPolicy::Manual,
    executor_id: k_shell::THEME_EXECUTOR_ID,
    state_schema_hash: 0x2018,
    permissions: SHELL_PERMS,
    exports: &[],
    vector_ref: None,
}];

const AI_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_AI_NODE_ID,
    local_node_key: "ai.supervisor",
    node_type: RuntimeNodeType::Aggregator,
    entry_policy: EntryPolicy::Background,
    executor_id: k_ai::EXECUTOR_ID,
    state_schema_hash: 0x2011,
    permissions: AI_PERMS,
    exports: AI_EXPORTS,
    vector_ref: None,
}];

const CHAT_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_CHAT_NODE_ID,
    local_node_key: "chat.bridge",
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::OnDemand,
    executor_id: k_chat::EXECUTOR_ID,
    state_schema_hash: 0x2010,
    permissions: CHAT_PERMS,
    exports: CHAT_EXPORTS,
    vector_ref: None,
}];
const NIM_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: K_NIM_NODE_ID,
    local_node_key: "nim.inference",
    node_type: RuntimeNodeType::Compute,
    entry_policy: EntryPolicy::OnDemand,
    executor_id: k_nim::EXECUTOR_ID,
    state_schema_hash: 0x2021,
    permissions: NIM_PERMS,
    exports: NIM_EXPORTS,
    vector_ref: None,
}];

const PANIC_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_panic::NODE_VEC,
    local_node_key: "panic.entry",
    executor: k_panic::EXECUTOR_VTABLE,
}];

const SERIAL_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_serial::NODE_VEC,
    local_node_key: "serial.entry",
    executor: k_serial::EXECUTOR_VTABLE,
}];

const GDT_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_gdt::NODE_VEC,
    local_node_key: "gdt.entry",
    executor: k_gdt::EXECUTOR_VTABLE,
}];

const CPUID_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_cpuid::NODE_VEC,
    local_node_key: "cpuid.entry",
    executor: k_cpuid::EXECUTOR_VTABLE,
}];

const PIC_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_pic::NODE_VEC,
    local_node_key: "pic.entry",
    executor: k_pic::EXECUTOR_VTABLE,
}];

const PIT_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_pit::NODE_VEC,
    local_node_key: "pit.entry",
    executor: k_pit::EXECUTOR_VTABLE,
}];

const IDT_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_idt::NODE_VEC,
    local_node_key: "idt.entry",
    executor: k_idt::EXECUTOR_VTABLE,
}];

const PS2_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_ps2::NODE_VEC,
    local_node_key: "ps2.entry",
    executor: k_ps2::EXECUTOR_VTABLE,
}];

const PMM_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_pmm::NODE_VEC,
    local_node_key: "pmm.entry",
    executor: k_pmm::EXECUTOR_VTABLE,
}];

const VMM_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_vmm::NODE_VEC,
    local_node_key: "vmm.entry",
    executor: k_vmm::EXECUTOR_VTABLE,
}];

const HEAP_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_heap::NODE_VEC,
    local_node_key: "heap.entry",
    executor: k_heap::EXECUTOR_VTABLE,
}];

const VGA_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_vga::NODE_VEC,
    local_node_key: "vga.entry",
    executor: k_vga::EXECUTOR_VTABLE,
}];

const IME_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_ime::NODE_VEC,
    local_node_key: "ime.router",
    executor: k_ime::EXECUTOR_VTABLE,
}];

const NET_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_net::NODE_VEC,
    local_node_key: "net.uplink",
    executor: k_net::EXECUTOR_VTABLE,
}];

const MOUSE_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_mouse::NODE_VEC,
    local_node_key: "mouse.pointer",
    executor: k_mouse::EXECUTOR_VTABLE,
}];

const CYPHER_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_cypher::NODE_VEC,
    local_node_key: "cypher.query",
    executor: k_cypher::EXECUTOR_VTABLE,
}];

const CUDA_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_cuda_host::NODE_VEC,
    local_node_key: "cuda.bridge",
    executor: k_cuda_host::EXECUTOR_VTABLE,
}];

const SHELL_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_shell::NODE_VEC,
    local_node_key: "shell.entry",
    executor: k_shell::EXECUTOR_VTABLE,
}, NativeNodeBinding {
    vector: k_shell::THEME_CURRENT_NODE_VEC,
    local_node_key: "theme.current",
    executor: k_shell::THEME_EXECUTOR_VTABLE,
}, NativeNodeBinding {
    vector: k_shell::CLIPBOARD_NODE_VEC,
    local_node_key: "clipboard.mount",
    executor: k_shell::CLIPBOARD_EXECUTOR_VTABLE,
}, NativeNodeBinding {
    vector: k_shell::THEME_WABI_NODE_VEC,
    local_node_key: "theme.wabi",
    executor: k_shell::THEME_EXECUTOR_VTABLE,
}, NativeNodeBinding {
    vector: k_shell::THEME_SHOJI_NODE_VEC,
    local_node_key: "theme.shoji",
    executor: k_shell::THEME_EXECUTOR_VTABLE,
}];

const AI_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_ai::NODE_VEC,
    local_node_key: "ai.supervisor",
    executor: k_ai::EXECUTOR_VTABLE,
}];

const CHAT_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_chat::NODE_VEC,
    local_node_key: "chat.bridge",
    executor: k_chat::EXECUTOR_VTABLE,
}];
const NIM_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_nim::NODE_VEC,
    local_node_key: "nim.inference",
    executor: k_nim::EXECUTOR_VTABLE,
}];

const VGA_MANIFEST: PluginManifest = manifest_with_nodes(
    K_VGA_ID,
    "K_VGA",
    &[],
    VGA_PERMS,
    VGA_EXPORTS,
    &[],
    VGA_NODE_SPECS,
);
const GDT_MANIFEST: PluginManifest = manifest_with_nodes(
    K_GDT_ID,
    "K_GDT",
    &[],
    MEM_PERMS,
    &[],
    &[],
    GDT_NODE_SPECS,
);
const CPUID_MANIFEST: PluginManifest = manifest_with_nodes(
    K_CPUID_ID,
    "K_CPUID",
    &[],
    NONE_PERMS,
    &[],
    &[],
    CPUID_NODE_SPECS,
);
const PIC_MANIFEST: PluginManifest = manifest_with_nodes(
    K_PIC_ID,
    "K_PIC",
    &[],
    PIC_PERMS,
    &[],
    &[],
    PIC_NODE_SPECS,
);
const PIT_MANIFEST: PluginManifest = manifest_with_nodes(K_PIT_ID, "K_PIT", DEP_PIT, PIT_PERMS, &[], &[], PIT_NODE_SPECS);
const PS2_MANIFEST: PluginManifest = manifest_with_nodes(
    K_PS2_ID,
    "K_PS2",
    DEP_PS2,
    PS2_PERMS,
    &[],
    PS2_IMPORTS,
    PS2_NODE_SPECS,
);
const IDT_MANIFEST: PluginManifest = manifest_with_nodes(
    K_IDT_ID,
    "K_IDT",
    DEP_IDT,
    IRQ_PERMS,
    &[],
    &[],
    IDT_NODE_SPECS,
);
const PMM_MANIFEST: PluginManifest = manifest_with_nodes(
    K_PMM_ID,
    "K_PMM",
    &[],
    MEM_PERMS,
    PMM_EXPORTS,
    &[],
    PMM_NODE_SPECS,
);
const VMM_MANIFEST: PluginManifest = manifest_with_nodes(
    K_VMM_ID,
    "K_VMM",
    DEP_VMM,
    MEM_PERMS,
    VMM_EXPORTS,
    &[],
    VMM_NODE_SPECS,
);
const HEAP_MANIFEST: PluginManifest = manifest_with_nodes(
    K_HEAP_ID,
    "K_HEAP",
    DEP_HEAP,
    MEM_PERMS,
    HEAP_EXPORTS,
    &[],
    HEAP_NODE_SPECS,
);
const IME_MANIFEST: PluginManifest = manifest_with_nodes(
    K_IME_ID,
    "K_IME",
    &[],
    IME_PERMS,
    IME_EXPORTS,
    IME_IMPORTS,
    IME_NODE_SPECS,
);
const NET_MANIFEST: PluginManifest = manifest_with_nodes(
    K_NET_ID,
    "K_NET",
    DEP_NET,
    NET_PERMS,
    NET_EXPORTS,
    NET_IMPORTS,
    NET_NODE_SPECS,
);
const MOUSE_MANIFEST: PluginManifest = manifest_with_nodes(
    K_MOUSE_ID,
    "K_MOUSE",
    DEP_MOUSE,
    MOUSE_PERMS,
    &[],
    MOUSE_IMPORTS,
    MOUSE_NODE_SPECS,
);
const CYPHER_MANIFEST: PluginManifest = manifest_with_nodes(
    K_CYPHER_ID,
    "K_CYPHER",
    DEP_CYPHER,
    CYPHER_PERMS,
    CYPHER_EXPORTS,
    CYPHER_IMPORTS,
    CYPHER_NODE_SPECS,
);
const CUDA_MANIFEST: PluginManifest = manifest_with_nodes(
    K_CUDA_ID,
    "K_CUDA",
    DEP_CUDA,
    CUDA_PERMS,
    CUDA_EXPORTS,
    CUDA_IMPORTS,
    CUDA_NODE_SPECS,
);
const SHELL_MANIFEST: PluginManifest = manifest_with_nodes(
    K_SHELL_ID,
    "K_SHELL",
    DEP_SHELL,
    SHELL_PERMS,
    SHELL_EXPORTS,
    SHELL_IMPORTS,
    SHELL_NODE_SPECS,
);
const AI_MANIFEST: PluginManifest = manifest_with_nodes(
    K_AI_ID,
    "K_AI",
    DEP_AI,
    AI_PERMS,
    AI_EXPORTS,
    AI_IMPORTS,
    AI_NODE_SPECS,
);
const CHAT_MANIFEST: PluginManifest = manifest_with_nodes(
    K_CHAT_ID,
    "K_CHAT",
    DEP_CHAT,
    CHAT_PERMS,
    CHAT_EXPORTS,
    CHAT_IMPORTS,
    CHAT_NODE_SPECS,
);
const NIM_MANIFEST: PluginManifest = manifest_with_nodes(
    K_NIM_ID,
    "K_NIM",
    DEP_NIM,
    NIM_PERMS,
    NIM_EXPORTS,
    NIM_IMPORTS,
    NIM_NODE_SPECS,
);

const fn module_id(plugin_id: PluginId) -> ModuleId {
    ModuleId::new(plugin_id.0)
}

const fn module_descriptor(
    plugin_id: PluginId,
    name: &'static str,
    dependencies: &'static [ModuleDependencySpec],
    permissions: &'static [PermissionSpec],
    exports: &'static [CapabilitySpec],
    imports: &'static [ImportSpec],
    fault_policy: ModuleFaultPolicy,
) -> ModuleDescriptor {
    ModuleDescriptor {
        abi_version: MODULE_ABI_VERSION,
        module_id: module_id(plugin_id),
        name,
        version: 1,
        image_format: ModuleImageFormat::Builtin,
        fault_policy,
        dependencies,
        permissions,
        exports,
        imports,
        segments: &[],
        entry: ModuleEntry::NONE,
        signature: None,
        flags: 0,
    }
}

const fn manifest_with_nodes(
    plugin_id: PluginId,
    name: &'static str,
    depends_on: &'static [PluginId],
    permissions: &'static [PermissionSpec],
    exports: &'static [CapabilitySpec],
    imports: &'static [ImportSpec],
    nodes: &'static [NodeSpec],
) -> PluginManifest {
    PluginManifest {
        abi_version: gos_protocol::GOS_ABI_VERSION,
        plugin_id,
        name,
        version: 1,
        depends_on,
        permissions,
        exports,
        imports,
        nodes,
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    }
}

const BUILTIN_MODULES: [BuiltinModule; 21] = [
    BuiltinModule::Native(NativeModule {
        manifest: PANIC_MANIFEST,
        granted_permissions: NONE_PERMS,
        nodes: PANIC_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: SERIAL_MANIFEST,
        granted_permissions: SERIAL_PERMS,
        nodes: SERIAL_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: VGA_MANIFEST,
        granted_permissions: VGA_PERMS,
        nodes: VGA_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: GDT_MANIFEST,
        granted_permissions: MEM_PERMS,
        nodes: GDT_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: CPUID_MANIFEST,
        granted_permissions: NONE_PERMS,
        nodes: CPUID_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: PIC_MANIFEST,
        granted_permissions: PIC_PERMS,
        nodes: PIC_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: PIT_MANIFEST,
        granted_permissions: PIT_PERMS,
        nodes: PIT_NATIVE_NODES,
        register_hook: Some(pit_register_hook),
    }),
    BuiltinModule::Native(NativeModule {
        manifest: PS2_MANIFEST,
        granted_permissions: PS2_PERMS,
        nodes: PS2_NATIVE_NODES,
        register_hook: Some(k_ps2::register_hook),
    }),
    BuiltinModule::Native(NativeModule {
        manifest: IDT_MANIFEST,
        granted_permissions: IRQ_PERMS,
        nodes: IDT_NATIVE_NODES,
        register_hook: Some(idt_load_hook),
    }),
    BuiltinModule::Native(NativeModule {
        manifest: PMM_MANIFEST,
        granted_permissions: MEM_PERMS,
        nodes: PMM_NATIVE_NODES,
        register_hook: Some(k_pmm::register_hook),
    }),
    BuiltinModule::Native(NativeModule {
        manifest: VMM_MANIFEST,
        granted_permissions: MEM_PERMS,
        nodes: VMM_NATIVE_NODES,
        register_hook: Some(k_vmm::register_hook),
    }),
    BuiltinModule::Native(NativeModule {
        manifest: HEAP_MANIFEST,
        granted_permissions: MEM_PERMS,
        nodes: HEAP_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: IME_MANIFEST,
        granted_permissions: IME_PERMS,
        nodes: IME_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: NET_MANIFEST,
        granted_permissions: NET_PERMS,
        nodes: NET_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: MOUSE_MANIFEST,
        granted_permissions: MOUSE_PERMS,
        nodes: MOUSE_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: CYPHER_MANIFEST,
        granted_permissions: CYPHER_PERMS,
        nodes: CYPHER_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: CUDA_MANIFEST,
        granted_permissions: CUDA_PERMS,
        nodes: CUDA_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: SHELL_MANIFEST,
        granted_permissions: SHELL_PERMS,
        nodes: SHELL_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: AI_MANIFEST,
        granted_permissions: AI_PERMS,
        nodes: AI_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: CHAT_MANIFEST,
        granted_permissions: CHAT_PERMS,
        nodes: CHAT_NATIVE_NODES,
        register_hook: None,
    }),
    BuiltinModule::Native(NativeModule {
        manifest: NIM_MANIFEST,
        granted_permissions: NIM_PERMS,
        nodes: NIM_NATIVE_NODES,
        register_hook: None,
    }),
];

const BUILTIN_SUPERVISOR_MODULES: [ModuleDescriptor; 21] = [
    module_descriptor(
        K_PANIC_ID,
        "K_PANIC",
        &[],
        NONE_PERMS,
        &[],
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_SERIAL_ID,
        "K_SERIAL",
        &[],
        SERIAL_PERMS,
        SERIAL_EXPORTS,
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_VGA_ID,
        "K_VGA",
        &[],
        VGA_PERMS,
        VGA_EXPORTS,
        &[],
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_GDT_ID,
        "K_GDT",
        &[],
        MEM_PERMS,
        &[],
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_CPUID_ID,
        "K_CPUID",
        &[],
        NONE_PERMS,
        &[],
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_PIC_ID,
        "K_PIC",
        &[],
        PIC_PERMS,
        &[],
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_PIT_ID,
        "K_PIT",
        MOD_DEP_PIT,
        PIT_PERMS,
        &[],
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_PS2_ID,
        "K_PS2",
        MOD_DEP_PS2,
        PS2_PERMS,
        &[],
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_IDT_ID,
        "K_IDT",
        MOD_DEP_IDT,
        IRQ_PERMS,
        &[],
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_PMM_ID,
        "K_PMM",
        &[],
        MEM_PERMS,
        PMM_EXPORTS,
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_VMM_ID,
        "K_VMM",
        MOD_DEP_VMM,
        MEM_PERMS,
        VMM_EXPORTS,
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_HEAP_ID,
        "K_HEAP",
        MOD_DEP_HEAP,
        MEM_PERMS,
        HEAP_EXPORTS,
        &[],
        ModuleFaultPolicy::FaultKernelDegraded,
    ),
    module_descriptor(
        K_IME_ID,
        "K_IME",
        &[],
        IME_PERMS,
        IME_EXPORTS,
        IME_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_NET_ID,
        "K_NET",
        MOD_DEP_NET,
        NET_PERMS,
        NET_EXPORTS,
        NET_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_MOUSE_ID,
        "K_MOUSE",
        MOD_DEP_MOUSE,
        MOUSE_PERMS,
        &[],
        MOUSE_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_CYPHER_ID,
        "K_CYPHER",
        MOD_DEP_CYPHER,
        CYPHER_PERMS,
        CYPHER_EXPORTS,
        CYPHER_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_CUDA_ID,
        "K_CUDA",
        MOD_DEP_CUDA,
        CUDA_PERMS,
        CUDA_EXPORTS,
        CUDA_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_SHELL_ID,
        "K_SHELL",
        MOD_DEP_SHELL,
        SHELL_PERMS,
        SHELL_EXPORTS,
        SHELL_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_AI_ID,
        "K_AI",
        MOD_DEP_AI,
        AI_PERMS,
        AI_EXPORTS,
        AI_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_CHAT_ID,
        "K_CHAT",
        MOD_DEP_CHAT,
        CHAT_PERMS,
        CHAT_EXPORTS,
        CHAT_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
    module_descriptor(
        K_NIM_ID,
        "K_NIM",
        MOD_DEP_NIM,
        NIM_PERMS,
        NIM_EXPORTS,
        NIM_IMPORTS,
        ModuleFaultPolicy::RestartAlways,
    ),
];

pub fn boot_builtin_graph(boot_payload: u64) -> Result<BuiltinBootReport, BuiltinBootError> {
    gos_runtime::reset();
    gos_runtime::emit_hello();

    for module in BUILTIN_MODULES {
        validate_manifest(module.manifest())?;
        gos_runtime::discover_plugin(module.manifest())?;
    }
    validate_imports(&BUILTIN_MODULES)?;

    let mut ctx = BootContext::new(boot_payload);
    let mut loaded = [false; BUILTIN_MODULES.len()];
    let mut idx = 0usize;
    while idx < BUILTIN_MODULES.len() {
        let module = BUILTIN_MODULES[idx];
        if let Some(dep) = first_missing_dependency(module.manifest().depends_on, &loaded, &BUILTIN_MODULES) {
            return Err(BuiltinBootError::MissingDependency(dep));
        }
        load_builtin_module(module, &mut ctx)?;
        loaded[idx] = true;
        idx += 1;
    }

    synchronize_manifest_graph(&BUILTIN_MODULES)?;
    synchronize_clipboard_mount_graph()?;
    gos_supervisor::service_system_cycle();

    Ok(BuiltinBootReport {
        discovered_plugins: BUILTIN_MODULES.len(),
        loaded_plugins: loaded.iter().filter(|loaded| **loaded).count(),
        stable_after_load: gos_runtime::is_stable(),
    })
}

pub fn builtin_supervisor_modules() -> &'static [ModuleDescriptor] {
    &BUILTIN_SUPERVISOR_MODULES
}

fn validate_manifest(manifest: PluginManifest) -> Result<(), BuiltinBootError> {
    // Phase D.5: semver compatibility — major must match, plugin's minor
    // must not exceed host's minor.  Patch is observational only.
    if !gos_protocol::abi_compatible(manifest.abi_version, GOS_ABI_VERSION) {
        return Err(BuiltinBootError::AbiVersionMismatch(manifest.plugin_id));
    }
    Ok(())
}

fn validate_imports(modules: &[BuiltinModule]) -> Result<(), BuiltinBootError> {
    for module in modules {
        let manifest = module.manifest();
        for import in manifest.imports {
            if !capability_is_exported(import, modules) {
                return Err(BuiltinBootError::UnresolvedImport(
                    manifest.plugin_id,
                    import.capability,
                ));
            }
        }
    }
    Ok(())
}

fn capability_is_exported(import: &ImportSpec, modules: &[BuiltinModule]) -> bool {
    modules.iter().any(|module| {
        module
            .manifest()
            .exports
            .iter()
            .any(|export| export.namespace == import.namespace && export.name == import.capability)
    })
}

fn first_missing_dependency(
    deps: &'static [PluginId],
    loaded: &[bool; BUILTIN_MODULES.len()],
    modules: &[BuiltinModule],
) -> Option<PluginId> {
    deps.iter().copied().find(|dep| {
        modules
            .iter()
            .enumerate()
            .find(|(_, module)| module.plugin_id() == *dep)
            .map(|(idx, _)| !loaded[idx])
            .unwrap_or(true)
    })
}

fn load_builtin_module(module: BuiltinModule, ctx: &mut BootContext) -> Result<(), BuiltinBootError> {
    let BuiltinModule::Native(module) = module;
    load_native_module(module, ctx)
}

fn load_native_module(module: NativeModule, ctx: &mut BootContext) -> Result<(), BuiltinBootError> {
    ensure_permissions(
        module.manifest.plugin_id,
        module.manifest.permissions,
        module.granted_permissions,
    )?;

    for spec in module.manifest.nodes {
        let binding = module
            .nodes
            .iter()
            .find(|binding| binding.local_node_key == spec.local_node_key)
            .ok_or(BuiltinBootError::Runtime(RuntimeError::NativeExecutorMissing))?;
        gos_runtime::register_node(module.manifest.plugin_id, binding.vector, *spec)?;
        gos_runtime::bind_native_executor(binding.vector, binding.executor)?;
    }

    for edge in module.manifest.edges {
        gos_runtime::register_edge(*edge)?;
    }

    gos_runtime::mark_plugin_loaded(module.manifest.plugin_id)?;

    if let Some(register_hook) = module.register_hook {
        register_hook(ctx);
    }

    for spec in module.manifest.nodes {
        if matches!(spec.entry_policy, EntryPolicy::Bootstrap | EntryPolicy::Background) {
            let binding = module
                .nodes
                .iter()
                .find(|binding| binding.local_node_key == spec.local_node_key)
                .ok_or(BuiltinBootError::Runtime(RuntimeError::NativeExecutorMissing))?;
            gos_runtime::post_signal(binding.vector, Signal::Spawn { payload: 0 })?;
        }
    }
    gos_supervisor::service_system_cycle();

    Ok(())
}

fn ensure_permissions(
    plugin_id: PluginId,
    requested: &'static [PermissionSpec],
    granted: &'static [PermissionSpec],
) -> Result<(), BuiltinBootError> {
    for req in requested {
        let ok = granted.iter().any(|grant| {
            grant.kind == req.kind
                && (grant.arg0 == u64::MAX || grant.arg0 == req.arg0)
                && (grant.arg1 == u64::MAX || grant.arg1 == req.arg1)
        });
        if !ok {
            return Err(BuiltinBootError::PermissionDenied(plugin_id));
        }
    }
    Ok(())
}

fn synchronize_manifest_graph(modules: &[BuiltinModule]) -> Result<(), BuiltinBootError> {
    for module in modules {
        let Some(source_node) = primary_node_for_module(*module) else {
            continue;
        };

        for dep in module.manifest().depends_on {
            let Some(target_node) = primary_node_for_plugin(*dep, modules) else {
                return Err(BuiltinBootError::MissingDependency(*dep));
            };

            let edge_spec = EdgeSpec {
                edge_id: derive_edge_id(source_node, target_node, "manifest.depend"),
                from_node: source_node,
                to_node: target_node,
                edge_type: RuntimeEdgeType::Depend,
                weight: 1.0,
                acl_mask: u64::MAX,
                route_policy: RoutePolicy::FailFast,
                capability_namespace: None,
                capability_binding: None,
                vector_ref: None,
            };
            gos_runtime::register_edge(edge_spec)?;
        }

        for import in module.manifest().imports {
            if let Some((provider_node, capability)) = provider_for_import(import, modules) {
                let edge_spec = EdgeSpec {
                    edge_id: derive_edge_id(source_node, provider_node, capability.name),
                    from_node: source_node,
                    to_node: provider_node,
                    edge_type: RuntimeEdgeType::Mount,
                    weight: 1.0,
                    acl_mask: u64::MAX,
                    route_policy: RoutePolicy::Direct,
                    capability_namespace: Some(capability.namespace),
                    capability_binding: Some(capability.name),
                    vector_ref: None,
                };
                gos_runtime::register_edge(edge_spec)?;
            }
        }
    }

    Ok(())
}

fn synchronize_clipboard_mount_graph() -> Result<(), BuiltinBootError> {
    for source_node in [K_SHELL_NODE_ID, K_CYPHER_NODE_ID, K_AI_NODE_ID] {
        let edge_spec = EdgeSpec {
            edge_id: derive_edge_id(source_node, K_CLIPBOARD_NODE_ID, "clipboard.mount"),
            from_node: source_node,
            to_node: K_CLIPBOARD_NODE_ID,
            edge_type: RuntimeEdgeType::Mount,
            weight: 1.0,
            acl_mask: u64::MAX,
            route_policy: RoutePolicy::Direct,
            capability_namespace: Some("clipboard"),
            capability_binding: Some("buffer"),
            vector_ref: None,
        };
        gos_runtime::register_edge(edge_spec)?;
    }

    Ok(())
}

fn primary_node_for_module(module: BuiltinModule) -> Option<gos_protocol::NodeId> {
    let BuiltinModule::Native(module) = module;
    module.nodes.first().and_then(|binding| gos_runtime::node_id_for_vec(binding.vector))
}

fn primary_node_for_plugin(
    plugin_id: PluginId,
    modules: &[BuiltinModule],
) -> Option<gos_protocol::NodeId> {
    modules
        .iter()
        .find(|module| module.plugin_id() == plugin_id)
        .and_then(|module| primary_node_for_module(*module))
}

fn provider_for_import(
    import: &ImportSpec,
    modules: &[BuiltinModule],
) -> Option<(gos_protocol::NodeId, CapabilitySpec)> {
    modules.iter().find_map(|module| {
        let capability = module
            .manifest()
            .exports
            .iter()
            .find(|export| {
                export.namespace == import.namespace && export.name == import.capability
            })?;
        let node = primary_node_for_module(*module)?;
        Some((node, *capability))
    })
}







fn pit_register_hook(_ctx: &mut BootContext) {
    k_pit::init_pit_hz(120);
}

fn idt_load_hook(_ctx: &mut BootContext) {
    // ── Graph-native IRQ routing ──────────────────────────────────────
    // Instead of injecting per-driver extern "x86-interrupt" handler
    // function pointers into the IDT, we register each driver's graph
    // node as the subscriber for its IRQ vector. The unified handlers
    // in k-idt will normalise every interrupt into a HardwareEvent
    // token and post it through `gos_runtime::post_irq_signal()`.
    // The supervisor's pump() loop then delivers Signal::Interrupt to
    // the subscribed node's on_signal() callback.
    //
    // This removes the hard coupling between k-idt ↔ k-pit/k-ps2/k-mouse
    // and makes IRQ ownership a first-class graph relationship.

    gos_runtime::subscribe_irq(
        k_pic::InterruptIndex::Timer.as_u8(),
        k_pit::NODE_VEC,
    );
    gos_runtime::subscribe_irq(
        k_pic::InterruptIndex::Keyboard.as_u8(),
        k_ps2::NODE_VEC,
    );
    gos_runtime::subscribe_irq(
        k_pic::InterruptIndex::Mouse.as_u8(),
        k_mouse::NODE_VEC,
    );
}
