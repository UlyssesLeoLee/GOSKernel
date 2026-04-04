#![no_std]

use gos_protocol::{
    derive_edge_id, derive_node_id, BootContext, CapabilitySpec, EdgeSpec, EntryPolicy, ExecutorId,
    ImportSpec, NodeExecutorVTable, NodeSpec, PermissionSpec, PluginId, PluginManifest,
    RoutePolicy, RuntimeEdgeType, RuntimeNodeType, VectorAddress, VectorRef, GOS_ABI_VERSION,
};
use gos_runtime::{self, RuntimeError};

pub const BUNDLE_MAGIC: u32 = 0x474F_5342;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderError {
    InvalidBundle,
    AbiVersionMismatch,
    MissingDependency(PluginId),
    PermissionDenied(PluginId),
    UnresolvedImport(PluginId, &'static str),
    UnsupportedModuleKind,
    DependencyCycle,
    Runtime(RuntimeError),
}

impl From<RuntimeError> for LoaderError {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BundleHeader {
    pub magic: u32,
    pub version: u16,
    pub module_count: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct LegacyNodeTemplate {
    pub vector: VectorAddress,
    pub local_node_key: &'static str,
    pub node_type: RuntimeNodeType,
    pub entry_policy: EntryPolicy,
    pub executor_id: ExecutorId,
    pub state_schema_hash: u64,
    pub permissions: &'static [PermissionSpec],
    pub exports: &'static [CapabilitySpec],
    pub vector_ref: Option<VectorRef>,
}

#[derive(Clone, Copy)]
pub struct LegacyModule {
    pub manifest: PluginManifest,
    pub granted_permissions: &'static [PermissionSpec],
    pub node: LegacyNodeTemplate,
    pub entry: fn(&mut BootContext),
    pub load_hook: Option<fn()>,
}

#[derive(Clone, Copy)]
pub struct ElfRelocModule {
    pub manifest: PluginManifest,
    pub image: &'static [u8],
}

#[derive(Clone, Copy)]
pub struct NativeNodeBinding {
    pub vector: VectorAddress,
    pub local_node_key: &'static str,
    pub executor: NodeExecutorVTable,
}

#[derive(Clone, Copy)]
pub struct NativeModule {
    pub manifest: PluginManifest,
    pub granted_permissions: &'static [PermissionSpec],
    pub nodes: &'static [NativeNodeBinding],
    pub register_hook: Option<fn(&mut BootContext)>,
}

#[derive(Clone, Copy)]
pub enum BundleModule {
    Legacy(LegacyModule),
    Native(NativeModule),
    ElfReloc(ElfRelocModule),
}

impl BundleModule {
    pub const fn manifest(&self) -> PluginManifest {
        match self {
            Self::Legacy(module) => module.manifest,
            Self::Native(module) => module.manifest,
            Self::ElfReloc(module) => module.manifest,
        }
    }

    pub const fn plugin_id(&self) -> PluginId {
        self.manifest().plugin_id
    }
}

#[derive(Clone, Copy)]
pub struct BootBundle {
    pub header: BundleHeader,
    pub modules: &'static [BundleModule],
}

#[derive(Debug, Clone, Copy)]
pub struct BundleLoadReport {
    pub discovered_plugins: usize,
    pub loaded_plugins: usize,
    pub stable_after_load: bool,
}

pub fn load_bundle(bundle: &BootBundle, ctx: &mut BootContext) -> Result<BundleLoadReport, LoaderError> {
    validate_bundle(bundle)?;

    gos_runtime::reset();
    gos_runtime::emit_hello();

    for module in bundle.modules {
        validate_manifest(module.manifest())?;
        gos_runtime::discover_plugin(module.manifest())?;
    }

    validate_imports(bundle)?;

    let mut loaded = [false; gos_runtime::MAX_PLUGINS];
    let mut loaded_count = 0usize;

    while loaded_count < bundle.modules.len() {
        let mut progressed = false;

        for (idx, module) in bundle.modules.iter().enumerate() {
            if loaded[idx] {
                continue;
            }

            if !dependencies_loaded(module.manifest().depends_on, &loaded, bundle.modules) {
                continue;
            }

            load_module(*module, ctx)?;
            loaded[idx] = true;
            loaded_count += 1;
            progressed = true;
        }

        if !progressed {
            return Err(LoaderError::DependencyCycle);
        }
    }

    synchronize_manifest_graph(bundle)?;
    synchronize_legacy_graph(bundle)?;
    gos_runtime::pump();

    Ok(BundleLoadReport {
        discovered_plugins: bundle.modules.len(),
        loaded_plugins: loaded_count,
        stable_after_load: gos_runtime::is_stable(),
    })
}

fn validate_bundle(bundle: &BootBundle) -> Result<(), LoaderError> {
    if bundle.header.magic != BUNDLE_MAGIC {
        return Err(LoaderError::InvalidBundle);
    }

    if bundle.header.module_count as usize != bundle.modules.len() {
        return Err(LoaderError::InvalidBundle);
    }

    Ok(())
}

fn validate_manifest(manifest: PluginManifest) -> Result<(), LoaderError> {
    if manifest.abi_version != GOS_ABI_VERSION {
        return Err(LoaderError::AbiVersionMismatch);
    }
    Ok(())
}

fn validate_imports(bundle: &BootBundle) -> Result<(), LoaderError> {
    for module in bundle.modules {
        let manifest = module.manifest();
        for import in manifest.imports {
            if !capability_is_exported(import, bundle.modules) {
                return Err(LoaderError::UnresolvedImport(manifest.plugin_id, import.capability));
            }
        }
    }
    Ok(())
}

fn capability_is_exported(import: &ImportSpec, modules: &[BundleModule]) -> bool {
    modules.iter().any(|module| {
        module
            .manifest()
            .exports
            .iter()
            .any(|export| export.name == import.capability && export.namespace == import.namespace)
    })
}

fn dependencies_loaded(
    deps: &'static [PluginId],
    loaded: &[bool; gos_runtime::MAX_PLUGINS],
    modules: &[BundleModule],
) -> bool {
    deps.iter().all(|dep| {
        modules
            .iter()
            .enumerate()
            .find(|(_, module)| module.plugin_id() == *dep)
            .map(|(idx, _)| loaded[idx])
            .unwrap_or(false)
    })
}

fn load_module(module: BundleModule, ctx: &mut BootContext) -> Result<(), LoaderError> {
    match module {
        BundleModule::Legacy(module) => load_legacy_module(module, ctx),
        BundleModule::Native(module) => load_native_module(module, ctx),
        BundleModule::ElfReloc(module) => {
            let _ = module.image;
            Err(LoaderError::UnsupportedModuleKind)
        }
    }
}

fn load_legacy_module(module: LegacyModule, ctx: &mut BootContext) -> Result<(), LoaderError> {
    ensure_permissions(module.manifest.plugin_id, module.manifest.permissions, module.granted_permissions)?;

    for spec in module.manifest.nodes {
        gos_runtime::register_node(module.manifest.plugin_id, module.node.vector, *spec)?;
    }

    for edge in module.manifest.edges {
        gos_runtime::register_edge(*edge)?;
    }

    let node_id = derive_node_id(module.manifest.plugin_id, module.node.local_node_key);
    let node_spec = NodeSpec {
        node_id,
        local_node_key: module.node.local_node_key,
        node_type: module.node.node_type,
        entry_policy: module.node.entry_policy,
        executor_id: module.node.executor_id,
        state_schema_hash: module.node.state_schema_hash,
        permissions: module.node.permissions,
        exports: module.node.exports,
        vector_ref: module.node.vector_ref,
    };

    gos_runtime::register_node(module.manifest.plugin_id, module.node.vector, node_spec)?;
    gos_runtime::mark_plugin_loaded(module.manifest.plugin_id)?;

    (module.entry)(ctx);

    if matches!(module.node.entry_policy, EntryPolicy::Bootstrap | EntryPolicy::Background) {
        gos_runtime::post_signal(module.node.vector, gos_protocol::Signal::Spawn { payload: 0 })?;
        gos_runtime::pump();
    }

    if let Some(load_hook) = module.load_hook {
        load_hook();
    }

    Ok(())
}

fn load_native_module(module: NativeModule, ctx: &mut BootContext) -> Result<(), LoaderError> {
    ensure_permissions(module.manifest.plugin_id, module.manifest.permissions, module.granted_permissions)?;

    for spec in module.manifest.nodes {
        let binding = module
            .nodes
            .iter()
            .find(|binding| binding.local_node_key == spec.local_node_key)
            .ok_or(LoaderError::UnsupportedModuleKind)?;
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
                .ok_or(LoaderError::UnsupportedModuleKind)?;
            gos_runtime::post_signal(binding.vector, gos_protocol::Signal::Spawn { payload: 0 })?;
        }
    }
    gos_runtime::pump();

    Ok(())
}

fn ensure_permissions(
    plugin_id: PluginId,
    requested: &'static [PermissionSpec],
    granted: &'static [PermissionSpec],
) -> Result<(), LoaderError> {
    for req in requested {
        let ok = granted.iter().any(|grant| {
            grant.kind == req.kind
                && (grant.arg0 == u64::MAX || grant.arg0 == req.arg0)
                && (grant.arg1 == u64::MAX || grant.arg1 == req.arg1)
        });
        if !ok {
            return Err(LoaderError::PermissionDenied(plugin_id));
        }
    }
    Ok(())
}

fn synchronize_manifest_graph(bundle: &BootBundle) -> Result<(), LoaderError> {
    for module in bundle.modules {
        let Some(source_node) = primary_node_for_module(*module) else {
            continue;
        };

        for dep in module.manifest().depends_on {
            let Some(target_node) = primary_node_for_plugin(*dep, bundle.modules) else {
                return Err(LoaderError::MissingDependency(*dep));
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
            if let Some((provider_node, capability)) = provider_for_import(import, bundle.modules) {
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

fn synchronize_legacy_graph(bundle: &BootBundle) -> Result<(), LoaderError> {
    for module in bundle.modules {
        let BundleModule::Legacy(module) = module else {
            continue;
        };

        let declaration = gos_runtime::describe_legacy_node(module.node.vector)?;
        let source_node = gos_runtime::node_id_for_vec(module.node.vector).ok_or(LoaderError::Runtime(RuntimeError::NodeNotFound))?;

        for (idx, edge) in declaration.edges.iter().take(declaration.edge_count).enumerate() {
            if edge.target_vec == 0 {
                continue;
            }

            let target_vec = VectorAddress::from_u64(edge.target_vec);
            let Some(target_node) = gos_runtime::node_id_for_vec(target_vec) else {
                continue;
            };

            let edge_key = edge_key(edge.tag, idx);
            let edge_spec = EdgeSpec {
                edge_id: derive_edge_id(source_node, target_node, edge_key),
                from_node: source_node,
                to_node: target_node,
                edge_type: map_legacy_edge_type(edge.edge_type),
                weight: 1.0,
                acl_mask: u64::MAX,
                route_policy: RoutePolicy::Direct,
                capability_namespace: None,
                capability_binding: None,
                vector_ref: None,
            };
            gos_runtime::register_edge(edge_spec)?;
        }

        for dependency in declaration.depends_on {
            let target_vec = VectorAddress::from_u64(*dependency);
            let Some(target_node) = gos_runtime::node_id_for_vec(target_vec) else {
                continue;
            };

            let edge_spec = EdgeSpec {
                edge_id: derive_edge_id(source_node, target_node, "depend"),
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
    }

    Ok(())
}

fn primary_node_for_module(module: BundleModule) -> Option<gos_protocol::NodeId> {
    match module {
        BundleModule::Legacy(module) => gos_runtime::node_id_for_vec(module.node.vector),
        BundleModule::Native(module) => module
            .nodes
            .first()
            .and_then(|binding| gos_runtime::node_id_for_vec(binding.vector)),
        BundleModule::ElfReloc(_) => None,
    }
}

fn primary_node_for_plugin(plugin_id: PluginId, modules: &[BundleModule]) -> Option<gos_protocol::NodeId> {
    modules
        .iter()
        .find(|module| module.plugin_id() == plugin_id)
        .and_then(|module| primary_node_for_module(*module))
}

fn provider_for_import(
    import: &ImportSpec,
    modules: &[BundleModule],
) -> Option<(gos_protocol::NodeId, CapabilitySpec)> {
    modules.iter().find_map(|module| {
        let capability = module
            .manifest()
            .exports
            .iter()
            .find(|export| export.namespace == import.namespace && export.name == import.capability)?;
        let node = primary_node_for_module(*module)?;
        Some((node, *capability))
    })
}

fn edge_key(tag: [u8; 12], idx: usize) -> &'static str {
    match (tag, idx) {
        (_, 0) => "edge0",
        (_, 1) => "edge1",
        (_, 2) => "edge2",
        (_, 3) => "edge3",
        (_, 4) => "edge4",
        (_, 5) => "edge5",
        (_, 6) => "edge6",
        (_, 7) => "edge7",
        (_, 8) => "edge8",
        (_, 9) => "edge9",
        (_, 10) => "edge10",
        _ => "edge11",
    }
}

fn map_legacy_edge_type(edge_type: u8) -> RuntimeEdgeType {
    match edge_type {
        0x01 => RuntimeEdgeType::Call,
        0x02 => RuntimeEdgeType::Spawn,
        0x03 => RuntimeEdgeType::Depend,
        0x04 => RuntimeEdgeType::Signal,
        0x05 => RuntimeEdgeType::Return,
        0x06 => RuntimeEdgeType::Mount,
        0x07 => RuntimeEdgeType::Sync,
        _ => RuntimeEdgeType::Stream,
    }
}
