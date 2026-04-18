#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-panic
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_PANIC", name: "k-panic"})
// SET p.executor = "k_panic::EXECUTOR_ID", p.node_type = "Service", p.state_schema = "0x2001"
// ============================================================


use gos_hal::{meta, vaddr};
use gos_protocol::*;

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 0, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.panic");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(panic_on_init),
    on_event: Some(panic_on_event),
    on_suspend: Some(panic_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

#[repr(C)]
struct PanicState {
    halts_requested: u64,
    last_signal_kind: u8,
}

fn hal_node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut PanicState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut PanicState) }
}

unsafe extern "C" fn panic_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        meta::burn_node_metadata(hal_node_ptr(), "BOOT", "PANIC");
        core::ptr::write(
            (*ctx).state_ptr as *mut PanicState,
            PanicState {
                halts_requested: 0,
                last_signal_kind: 0,
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn panic_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    // ── Pre-processing: decode signal, detect halt sentinel ───────────────────
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    // ── Main processing: halt CPU if requested (diverges on halt) ─────────────
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    // ── Post-processing: update telemetry ─────────────────────────────────────
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn panic_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_PANIC"),
        name: "K_PANIC",
        version: 1,
        depends_on: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_PANIC"), "panic.entry"),
            local_node_key: "panic.entry",
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x2001,
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
        local_node_key: "panic.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None,
};
