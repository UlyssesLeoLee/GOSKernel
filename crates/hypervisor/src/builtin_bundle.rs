use gos_loader::{
    BootBundle, BundleHeader, BundleModule, LegacyModule, LegacyNodeTemplate, NativeModule,
    NativeNodeBinding, BUNDLE_MAGIC,
};
use gos_protocol::{
    derive_node_id, BootContext, CapabilitySpec, EntryPolicy, ExecutorId, ImportSpec,
    ModuleDependencySpec, ModuleDescriptor, ModuleEntry, ModuleFaultPolicy, ModuleId,
    ModuleImageFormat, NodeSpec, PermissionKind, PermissionSpec, PluginId, PluginManifest,
    PluginEntry, RuntimeNodeType, MODULE_ABI_VERSION,
};

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
const K_SHELL_ID: PluginId = PluginId::from_ascii("K_SHELL");
const K_AI_ID: PluginId = PluginId::from_ascii("K_AI");

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
const AI_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::GraphRead, arg0: 0, arg1: 0 },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
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
const AI_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "ai", name: "supervisor" },
    CapabilitySpec { namespace: "graph", name: "orchestrate" },
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
];
const CYPHER_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "console", capability: "write", required: true },
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

const DEP_PIT: &[PluginId] = &[K_PIC_ID];
const DEP_PS2: &[PluginId] = &[K_PIC_ID];
const DEP_IDT: &[PluginId] = &[K_GDT_ID, K_PIT_ID, K_PS2_ID];
const DEP_VMM: &[PluginId] = &[K_PMM_ID];
const DEP_HEAP: &[PluginId] = &[K_PMM_ID, K_VMM_ID];
const DEP_NET: &[PluginId] = &[K_VGA_ID];
const DEP_MOUSE: &[PluginId] = &[K_VGA_ID, K_PS2_ID, K_IDT_ID];
const DEP_CYPHER: &[PluginId] = &[K_VGA_ID];
const DEP_SHELL: &[PluginId] = &[K_VGA_ID, K_PS2_ID, K_HEAP_ID, K_IME_ID, K_NET_ID, K_CYPHER_ID];
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
];
const MOD_DEP_AI: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: module_id(K_SHELL_ID),
    required: true,
}];

const PANIC_MANIFEST: PluginManifest = manifest(K_PANIC_ID, "K_PANIC", &[], NONE_PERMS, &[], &[]);
const SERIAL_MANIFEST: PluginManifest = manifest(K_SERIAL_ID, "K_SERIAL", &[], SERIAL_PERMS, SERIAL_EXPORTS, &[]);
const K_VGA_NODE_ID: gos_protocol::NodeId = derive_node_id(K_VGA_ID, "vga.entry");
const K_IME_NODE_ID: gos_protocol::NodeId = derive_node_id(K_IME_ID, "ime.router");
const K_NET_NODE_ID: gos_protocol::NodeId = derive_node_id(K_NET_ID, "net.uplink");
const K_MOUSE_NODE_ID: gos_protocol::NodeId = derive_node_id(K_MOUSE_ID, "mouse.pointer");
const K_CYPHER_NODE_ID: gos_protocol::NodeId = derive_node_id(K_CYPHER_ID, "cypher.query");
const K_SHELL_NODE_ID: gos_protocol::NodeId = derive_node_id(K_SHELL_ID, "shell.entry");
const K_AI_NODE_ID: gos_protocol::NodeId = derive_node_id(K_AI_ID, "ai.supervisor");

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

const SHELL_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_shell::NODE_VEC,
    local_node_key: "shell.entry",
    executor: k_shell::EXECUTOR_VTABLE,
}];

const AI_NATIVE_NODES: &[NativeNodeBinding] = &[NativeNodeBinding {
    vector: k_ai::NODE_VEC,
    local_node_key: "ai.supervisor",
    executor: k_ai::EXECUTOR_VTABLE,
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
const GDT_MANIFEST: PluginManifest = manifest(K_GDT_ID, "K_GDT", &[], MEM_PERMS, &[], &[]);
const CPUID_MANIFEST: PluginManifest = manifest(K_CPUID_ID, "K_CPUID", &[], NONE_PERMS, &[], &[]);
const PIC_MANIFEST: PluginManifest = manifest(K_PIC_ID, "K_PIC", &[], PIC_PERMS, &[], &[]);
const PIT_MANIFEST: PluginManifest = manifest(K_PIT_ID, "K_PIT", DEP_PIT, PIT_PERMS, &[], &[]);
const PS2_MANIFEST: PluginManifest = manifest(K_PS2_ID, "K_PS2", DEP_PS2, PS2_PERMS, &[], &[]);
const IDT_MANIFEST: PluginManifest = manifest(K_IDT_ID, "K_IDT", DEP_IDT, IRQ_PERMS, &[], &[]);
const PMM_MANIFEST: PluginManifest = manifest(K_PMM_ID, "K_PMM", &[], MEM_PERMS, PMM_EXPORTS, &[]);
const VMM_MANIFEST: PluginManifest = manifest(K_VMM_ID, "K_VMM", DEP_VMM, MEM_PERMS, VMM_EXPORTS, &[]);
const HEAP_MANIFEST: PluginManifest = manifest(K_HEAP_ID, "K_HEAP", DEP_HEAP, MEM_PERMS, HEAP_EXPORTS, &[]);
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

const fn manifest(
    plugin_id: PluginId,
    name: &'static str,
    depends_on: &'static [PluginId],
    permissions: &'static [PermissionSpec],
    exports: &'static [CapabilitySpec],
    imports: &'static [ImportSpec],
) -> PluginManifest {
    manifest_with_nodes(plugin_id, name, depends_on, permissions, exports, imports, &[])
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

const BUILTIN_MODULES: [BundleModule; 18] = [
    BundleModule::Legacy(LegacyModule {
        manifest: PANIC_MANIFEST,
        granted_permissions: NONE_PERMS,
        node: legacy_node(k_panic::NODE_VEC, "panic.entry", RuntimeNodeType::Service, "legacy.panic", 0x1001, NONE_PERMS, &[]),
        entry: panic_entry,
        load_hook: None,
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: SERIAL_MANIFEST,
        granted_permissions: SERIAL_PERMS,
        node: legacy_node(k_serial::NODE_VEC, "serial.entry", RuntimeNodeType::Driver, "legacy.serial", 0x1002, SERIAL_PERMS, SERIAL_EXPORTS),
        entry: serial_entry,
        load_hook: None,
    }),
    BundleModule::Native(NativeModule {
        manifest: VGA_MANIFEST,
        granted_permissions: VGA_PERMS,
        nodes: VGA_NATIVE_NODES,
        register_hook: None,
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: GDT_MANIFEST,
        granted_permissions: MEM_PERMS,
        node: legacy_node(k_gdt::NODE_VEC, "gdt.entry", RuntimeNodeType::Service, "legacy.gdt", 0x1004, MEM_PERMS, &[]),
        entry: gdt_entry,
        load_hook: Some(gdt_load_hook),
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: CPUID_MANIFEST,
        granted_permissions: NONE_PERMS,
        node: legacy_node(k_cpuid::NODE_VEC, "cpuid.entry", RuntimeNodeType::Service, "legacy.cpuid", 0x1005, NONE_PERMS, &[]),
        entry: cpuid_entry,
        load_hook: None,
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: PIC_MANIFEST,
        granted_permissions: PIC_PERMS,
        node: legacy_node(k_pic::NODE_VEC, "pic.entry", RuntimeNodeType::Driver, "legacy.pic", 0x1006, PIC_PERMS, &[]),
        entry: pic_entry,
        load_hook: Some(pic_load_hook),
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: PIT_MANIFEST,
        granted_permissions: PIT_PERMS,
        node: legacy_node(k_pit::NODE_VEC, "pit.entry", RuntimeNodeType::Driver, "legacy.pit", 0x1007, PIT_PERMS, &[]),
        entry: pit_entry,
        load_hook: Some(pit_load_hook),
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: PS2_MANIFEST,
        granted_permissions: PS2_PERMS,
        node: legacy_node(k_ps2::NODE_VEC, "ps2.entry", RuntimeNodeType::Driver, "legacy.ps2", 0x1008, PS2_PERMS, &[]),
        entry: ps2_entry,
        load_hook: None,
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: IDT_MANIFEST,
        granted_permissions: IRQ_PERMS,
        node: legacy_node(k_idt::NODE_VEC, "idt.entry", RuntimeNodeType::Service, "legacy.idt", 0x1009, IRQ_PERMS, &[]),
        entry: idt_entry,
        load_hook: Some(idt_load_hook),
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: PMM_MANIFEST,
        granted_permissions: MEM_PERMS,
        node: legacy_node(k_pmm::NODE_VEC, "pmm.entry", RuntimeNodeType::Service, "legacy.pmm", 0x100A, MEM_PERMS, PMM_EXPORTS),
        entry: pmm_entry,
        load_hook: None,
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: VMM_MANIFEST,
        granted_permissions: MEM_PERMS,
        node: legacy_node(k_vmm::NODE_VEC, "vmm.entry", RuntimeNodeType::Service, "legacy.vmm", 0x100B, MEM_PERMS, VMM_EXPORTS),
        entry: vmm_entry,
        load_hook: None,
    }),
    BundleModule::Legacy(LegacyModule {
        manifest: HEAP_MANIFEST,
        granted_permissions: MEM_PERMS,
        node: legacy_node(k_heap::NODE_VEC, "heap.entry", RuntimeNodeType::Service, "legacy.heap", 0x100C, MEM_PERMS, HEAP_EXPORTS),
        entry: heap_entry,
        load_hook: None,
    }),
    BundleModule::Native(NativeModule {
        manifest: IME_MANIFEST,
        granted_permissions: IME_PERMS,
        nodes: IME_NATIVE_NODES,
        register_hook: None,
    }),
    BundleModule::Native(NativeModule {
        manifest: NET_MANIFEST,
        granted_permissions: NET_PERMS,
        nodes: NET_NATIVE_NODES,
        register_hook: None,
    }),
    BundleModule::Native(NativeModule {
        manifest: MOUSE_MANIFEST,
        granted_permissions: MOUSE_PERMS,
        nodes: MOUSE_NATIVE_NODES,
        register_hook: None,
    }),
    BundleModule::Native(NativeModule {
        manifest: CYPHER_MANIFEST,
        granted_permissions: CYPHER_PERMS,
        nodes: CYPHER_NATIVE_NODES,
        register_hook: None,
    }),
    BundleModule::Native(NativeModule {
        manifest: SHELL_MANIFEST,
        granted_permissions: SHELL_PERMS,
        nodes: SHELL_NATIVE_NODES,
        register_hook: None,
    }),
    BundleModule::Native(NativeModule {
        manifest: AI_MANIFEST,
        granted_permissions: AI_PERMS,
        nodes: AI_NATIVE_NODES,
        register_hook: None,
    }),
];

const BUNDLE: BootBundle = BootBundle {
    header: BundleHeader {
        magic: BUNDLE_MAGIC,
        version: 1,
        module_count: BUILTIN_MODULES.len() as u16,
    },
    modules: &BUILTIN_MODULES,
};

const BUILTIN_SUPERVISOR_MODULES: [ModuleDescriptor; 18] = [
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
];

const fn legacy_node(
    vector: gos_protocol::VectorAddress,
    local_node_key: &'static str,
    node_type: RuntimeNodeType,
    executor: &'static str,
    state_schema_hash: u64,
    permissions: &'static [PermissionSpec],
    exports: &'static [CapabilitySpec],
) -> LegacyNodeTemplate {
    LegacyNodeTemplate {
        vector,
        local_node_key,
        node_type,
        entry_policy: EntryPolicy::Bootstrap,
        executor_id: ExecutorId::from_ascii(executor),
        state_schema_hash,
        permissions,
        exports,
        vector_ref: None,
    }
}

pub fn builtin_bundle() -> &'static BootBundle {
    &BUNDLE
}

pub fn builtin_supervisor_modules() -> &'static [ModuleDescriptor] {
    &BUILTIN_SUPERVISOR_MODULES
}

fn panic_entry(ctx: &mut BootContext) {
    <k_panic::PanicCell as PluginEntry>::plugin_main(ctx);
}

fn serial_entry(ctx: &mut BootContext) {
    <k_serial::SerialCell as PluginEntry>::plugin_main(ctx);
}

fn gdt_entry(ctx: &mut BootContext) {
    <k_gdt::GdtCell as PluginEntry>::plugin_main(ctx);
}

fn cpuid_entry(ctx: &mut BootContext) {
    <k_cpuid::CpuidCell as PluginEntry>::plugin_main(ctx);
}

fn pic_entry(ctx: &mut BootContext) {
    <k_pic::PicCell as PluginEntry>::plugin_main(ctx);
}

fn pit_entry(ctx: &mut BootContext) {
    <k_pit::PitCell as PluginEntry>::plugin_main(ctx);
}

fn ps2_entry(ctx: &mut BootContext) {
    <k_ps2::Ps2Cell as PluginEntry>::plugin_main(ctx);
}

fn idt_entry(ctx: &mut BootContext) {
    <k_idt::IdtCell as PluginEntry>::plugin_main(ctx);
}

fn pmm_entry(ctx: &mut BootContext) {
    <k_pmm::PmmCell as PluginEntry>::plugin_main(ctx);
}

fn vmm_entry(ctx: &mut BootContext) {
    <k_vmm::VmmCell as PluginEntry>::plugin_main(ctx);
}

fn heap_entry(ctx: &mut BootContext) {
    <k_heap::HeapCell as PluginEntry>::plugin_main(ctx);
}

fn gdt_load_hook() {
    k_gdt::init_gdt();
}

fn pic_load_hook() {
    k_pic::init_pic();
}

fn pit_load_hook() {
    k_pit::init_pit_hz(120);
}

fn idt_load_hook() {
    k_idt::inject_irq_handler(
        k_pic::InterruptIndex::Timer.as_usize(),
        k_pit::timer_interrupt_handler,
    );
    k_idt::inject_irq_handler(
        k_pic::InterruptIndex::Keyboard.as_usize(),
        k_ps2::keyboard_interrupt_handler,
    );
    k_idt::inject_irq_handler(
        k_pic::InterruptIndex::Mouse.as_usize(),
        k_mouse::mouse_interrupt_handler,
    );
    k_idt::init_idt();
}
