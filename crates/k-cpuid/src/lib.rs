#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-cpuid
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_CPUID", name: "k-cpuid"})
// SET p.executor = "k_cpuid::EXECUTOR_ID", p.node_type = "Service", p.state_schema = "0x2005"
// ============================================================


use core::arch::x86_64::__cpuid;

use gos_hal::{meta, vaddr};
use gos_protocol::*;

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 8, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.cpuid");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(cpuid_on_init),
    on_event: Some(cpuid_on_event),
    on_suspend: Some(cpuid_on_suspend),
    on_resume: Some(cpuid_on_resume),
    on_teardown: None,
    on_telemetry: None,
};

#[repr(C)]
struct CpuidState {
    max_basic_leaf: u32,
    max_extended_leaf: u32,
    feature_ecx: u32,
    feature_edx: u32,
    extended_ecx: u32,
    extended_edx: u32,
    sample_count: u32,
    last_signal_kind: u8,
    vendor: [u8; 12],
    brand: [u8; 48],
}

fn hal_node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut CpuidState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut CpuidState) }
}

fn copy_brand_leaf(brand: &mut [u8; 48], index: usize, eax: u32, ebx: u32, ecx: u32, edx: u32) {
    let offset = index * 16;
    let eax = eax.to_le_bytes();
    let ebx = ebx.to_le_bytes();
    let ecx = ecx.to_le_bytes();
    let edx = edx.to_le_bytes();

    brand[offset..offset + 4].copy_from_slice(&eax);
    brand[offset + 4..offset + 8].copy_from_slice(&ebx);
    brand[offset + 8..offset + 12].copy_from_slice(&ecx);
    brand[offset + 12..offset + 16].copy_from_slice(&edx);
}

fn sample_cpuid(state: &mut CpuidState) {
    let basic = __cpuid(0);
    let features = __cpuid(1);
    let extended = __cpuid(0x8000_0000);

    state.max_basic_leaf = basic.eax;
    state.max_extended_leaf = extended.eax;
    state.feature_ecx = features.ecx;
    state.feature_edx = features.edx;
    state.sample_count = state.sample_count.saturating_add(1);
    state.vendor = [0; 12];
    state.brand = [0; 48];

    let ebx = basic.ebx.to_le_bytes();
    let edx = basic.edx.to_le_bytes();
    let ecx = basic.ecx.to_le_bytes();
    state.vendor[0..4].copy_from_slice(&ebx);
    state.vendor[4..8].copy_from_slice(&edx);
    state.vendor[8..12].copy_from_slice(&ecx);

    if extended.eax >= 0x8000_0001 {
        let extended_features = __cpuid(0x8000_0001);
        state.extended_ecx = extended_features.ecx;
        state.extended_edx = extended_features.edx;
    } else {
        state.extended_ecx = 0;
        state.extended_edx = 0;
    }

    if extended.eax >= 0x8000_0004 {
        let brand0 = __cpuid(0x8000_0002);
        let brand1 = __cpuid(0x8000_0003);
        let brand2 = __cpuid(0x8000_0004);
        copy_brand_leaf(&mut state.brand, 0, brand0.eax, brand0.ebx, brand0.ecx, brand0.edx);
        copy_brand_leaf(&mut state.brand, 1, brand1.eax, brand1.ebx, brand1.ecx, brand1.edx);
        copy_brand_leaf(&mut state.brand, 2, brand2.eax, brand2.ebx, brand2.ecx, brand2.edx);
    }
}

unsafe extern "C" fn cpuid_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        meta::burn_node_metadata(hal_node_ptr(), "HAL", "CPUID");
        core::ptr::write(
            (*ctx).state_ptr as *mut CpuidState,
            CpuidState {
                max_basic_leaf: 0,
                max_extended_leaf: 0,
                feature_ecx: 0,
                feature_edx: 0,
                extended_ecx: 0,
                extended_edx: 0,
                sample_count: 0,
                last_signal_kind: 0,
                vendor: [0; 12],
                brand: [0; 48],
            },
        );
        sample_cpuid(state_mut(ctx));
    }
    ExecStatus::Done
}

unsafe extern "C" fn cpuid_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    // ── Pre-processing: decode signal kind ────────────────────────────────────
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    // ── Main processing: re-sample CPUID ──────────────────────────────────────
    let Some(output) = (unsafe { proc::process(ctx, input) }) else { return ExecStatus::Done; };
    // ── Post-processing: commit telemetry ─────────────────────────────────────
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn cpuid_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

unsafe extern "C" fn cpuid_on_resume(ctx: *mut ExecutorContext) -> ExecStatus {
    sample_cpuid(unsafe { state_mut(ctx) });
    ExecStatus::Done
}

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_CPUID"),
        name: "K_CPUID",
        version: 1,
        depends_on: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_CPUID"), "cpuid.entry"),
            local_node_key: "cpuid.entry",
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x2005,
            permissions: &[],
            exports: &[],
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: &[],
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "cpuid.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None,
};
