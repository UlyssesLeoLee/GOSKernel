#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-pic
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_PIC", name: "k-pic"})
// SET p.executor = "k_pic::EXECUTOR_ID", p.node_type = "Driver", p.state_schema = "0x2006"
//
// -- Hardware Resources
// MERGE (pr_20:PortRange {start: "0x20", end: "0xA1"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_20)
// MERGE (irq_u64::MAX:InterruptLine {irq: "u64::MAX"})
// MERGE (p)-[:BINDS_IRQ]->(irq_u64::MAX)
// ============================================================


use gos_hal::{meta, vaddr};
use gos_protocol::*;
use pic8259::ChainedPics;
use spin::Mutex;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 5, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pic");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(pic_on_init),
    on_event: Some(pic_on_event),
    on_suspend: Some(pic_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
    Mouse = PIC_2_OFFSET + 4,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

#[repr(C)]
struct PicRuntimeState {
    init_count: u64,
    last_signal_kind: u8,
}

pub fn node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

pub fn pics() -> &'static Mutex<ChainedPics> {
    unsafe {
        let p = node_ptr();
        if p.is_null() {
            panic!("K_PIC Matrix not initialized");
        }
        &*(p.add(1024) as *mut Mutex<ChainedPics>)
    }
}

unsafe fn runtime_state_mut(ctx: *mut ExecutorContext) -> &'static mut PicRuntimeState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut PicRuntimeState) }
}

unsafe fn init_hal_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "PIC");
    let state_ptr = p.add(1024) as *mut Mutex<ChainedPics>;
    core::ptr::write(
        state_ptr,
        Mutex::new(ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)),
    );
}

unsafe extern "C" fn pic_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        init_hal_state();
        core::ptr::write(
            (*ctx).state_ptr as *mut PicRuntimeState,
            PicRuntimeState {
                init_count: 0,
                last_signal_kind: 0,
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn pic_on_event(
    ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    // ── Pre-processing: decode signal and check for Spawn ─────────────────────
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    // ── Main processing: initialise PIC if requested ──────────────────────────
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    // ── Post-processing: commit state and return ──────────────────────────────
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn pic_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

pub fn init_pic() {
    unsafe {
        pics().lock().initialize();
        pics().lock().write_masks(0xF8, 0xEF);
    }
}

const PIC_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x20, arg1: 0xA1 },
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: u64::MAX, arg1: 0 },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_PIC"),
        name: "K_PIC",
        version: 1,
        depends_on: &[],
        permissions: PIC_PERMS,
        exports: &[],
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_PIC"), "pic.entry"),
            local_node_key: "pic.entry",
            node_type: RuntimeNodeType::Driver,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x2006,
            permissions: PIC_PERMS,
            exports: &[],
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: PIC_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "pic.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None,
};
