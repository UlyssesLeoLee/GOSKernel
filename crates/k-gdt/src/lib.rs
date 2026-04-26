#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-gdt
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_GDT", name: "k-gdt"})
// SET p.executor = "k_gdt::EXECUTOR_ID", p.node_type = "Service", p.state_schema = "0x2004"
// ============================================================


use gos_hal::{meta, vaddr};
use gos_protocol::*;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
// Phase B.4.3: dedicated IST stacks for the fault classes that may fire
// inside a native plugin dispatch.  Using separate stacks (instead of
// reusing the kernel stack) ensures the handler can run even if the
// interrupted code corrupted its own stack — and lets the fault path
// safely call into the supervisor without a risk of recursion.
pub const PAGE_FAULT_IST_INDEX: u16 = 1;
pub const GENERAL_PROTECTION_IST_INDEX: u16 = 2;
pub const STACK_SEGMENT_IST_INDEX: u16 = 3;
pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 3, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.gdt");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(gdt_on_init),
    on_event: Some(gdt_on_event),
    on_suspend: Some(gdt_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

#[repr(C)]
pub struct GdtState {
    pub tss: TaskStateSegment,
    pub gdt: GlobalDescriptorTable,
    pub selectors: Selectors,
}

#[repr(C)]
struct GdtRuntimeState {
    load_count: u64,
    last_signal_kind: u8,
}

pub fn node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

pub fn gdt_state() -> &'static GdtState {
    unsafe {
        let p = node_ptr();
        if p.is_null() {
            panic!("K_GDT Matrix not initialized");
        }
        &*(p.add(1024) as *mut GdtState)
    }
}

unsafe fn runtime_state_mut(ctx: *mut ExecutorContext) -> &'static mut GdtRuntimeState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut GdtRuntimeState) }
}

unsafe fn init_hal_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "GDT");

    let state_ptr = p.add(1024) as *mut GdtState;
    core::ptr::write(
        state_ptr,
        GdtState {
            tss: TaskStateSegment::new(),
            gdt: GlobalDescriptorTable::new(),
            selectors: Selectors {
                code_selector: SegmentSelector(0),
                tss_selector: SegmentSelector(0),
            },
        },
    );

    let state = unsafe { &mut *state_ptr };
    // ── IST stack 0: double fault (#DF) ───────────────────────────────────
    state.tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096 * 5;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
        let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(STACK) as *const ());
        stack_start + STACK_SIZE
    };
    // ── IST stack 1: page fault (#PF) — Phase B.4.3 ───────────────────────
    state.tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096 * 4;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
        let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(STACK) as *const ());
        stack_start + STACK_SIZE
    };
    // ── IST stack 2: general protection (#GP) — Phase B.4.3 ───────────────
    state.tss.interrupt_stack_table[GENERAL_PROTECTION_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096 * 4;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
        let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(STACK) as *const ());
        stack_start + STACK_SIZE
    };
    // ── IST stack 3: stack-segment fault (#SS) — Phase B.4.3 ──────────────
    state.tss.interrupt_stack_table[STACK_SEGMENT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096 * 4;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
        let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(STACK) as *const ());
        stack_start + STACK_SIZE
    };

    state.selectors.code_selector = state.gdt.add_entry(Descriptor::kernel_code_segment());
    state.selectors.tss_selector = state.gdt.add_entry(Descriptor::tss_segment(&state.tss));
}

unsafe extern "C" fn gdt_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        init_hal_state();
        core::ptr::write(
            (*ctx).state_ptr as *mut GdtRuntimeState,
            GdtRuntimeState {
                load_count: 0,
                last_signal_kind: 0,
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn gdt_on_event(
    ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    // ── Pre-processing: decode signal, check for Spawn ────────────────────────
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    // ── Main processing: load GDT if requested ────────────────────────────────
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    // ── Post-processing: commit telemetry and return ──────────────────────────
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn gdt_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

pub fn init_gdt() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;

    unsafe {
        let state = gdt_state();
        state.gdt.load();
        CS::set_reg(state.selectors.code_selector);
        load_tss(state.selectors.tss_selector);
    }
}

const GDT_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PhysMap, arg0: u64::MAX, arg1: u64::MAX },
    PermissionSpec { kind: PermissionKind::GraphWrite, arg0: 0, arg1: 0 },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_GDT"),
        name: "K_GDT",
        version: 1,
        depends_on: &[],
        permissions: GDT_PERMS,
        exports: &[],
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_GDT"), "gdt.entry"),
            local_node_key: "gdt.entry",
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x2004,
            permissions: GDT_PERMS,
            exports: &[],
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: GDT_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "gdt.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None,
};
