#![no_std]


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

fn signal_kind_code(signal: Signal) -> u8 {
    match signal {
        Signal::Call { .. } => 0x01,
        Signal::Spawn { .. } => 0x02,
        Signal::Interrupt { .. } => 0x03,
        Signal::Data { .. } => 0x04,
        Signal::Control { .. } => 0x05,
        Signal::Terminate => 0xFF,
    }
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
    state.tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096 * 5;
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
    let signal = unsafe { packet_to_signal((*event).signal) };
    let state = unsafe { runtime_state_mut(ctx) };
    state.last_signal_kind = signal_kind_code(signal);

    if let Signal::Spawn { .. } = signal {
        init_gdt();
        state.load_count = state.load_count.saturating_add(1);
    }

    ExecStatus::Done
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
