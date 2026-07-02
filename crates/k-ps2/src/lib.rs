#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-ps2
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_PS2", name: "k-ps2"})
// SET p.executor = "k_ps2::EXECUTOR_ID", p.node_type = "Driver", p.state_schema = "0x2008"
//
// -- Dependencies
// MERGE (dep_K_PIC:Plugin {id: "K_PIC"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PIC)
//
// -- Hardware Resources
// MERGE (pr_60:PortRange {start: "0x60", end: "0x64"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_60)
// MERGE (irq_1:InterruptLine {irq: "1"})
// MERGE (p)-[:BINDS_IRQ]->(irq_1)
//
// -- Conditional Routes (registered at boot via register_hook)
// MERGE (r0:Route {key: "0x00", label: "SHELL"})
// MERGE (p)-[:ROUTES {key: 0}]->(r0)-[:TO]->(shell:Plugin {id: "K_SHELL"})
// MERGE (r1:Route {key: "0x01", label: "IME"})
// MERGE (p)-[:ROUTES {key: 1}]->(r1)-[:TO]->(ime:Plugin {id: "K_IME"})
// ============================================================

use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1};
use gos_protocol::*;

pub const NODE_VEC: VectorAddress = gos_protocol::vectors::CORE_PS2;

/// Route key → k_shell::NODE_VEC (ASCII / special keys).
pub const PS2_ROUTE_SHELL: u8 = 0x00;
/// Route key → k_ime::NODE_VEC (reserved for IME pre-processing).
pub const PS2_ROUTE_IME: u8 = 0x01;

// ── Global decoded-key ring for in-kernel consumers (the 3D desktop taskbar's
//    command input). Filled in post::emit (alongside shell routing), drained by
//    take_key. ─────────────────────────────────────────────────────────────────
const KEY_RING: usize = 64;
static KEY_BUF: [core::sync::atomic::AtomicU8; KEY_RING] =
    [const { core::sync::atomic::AtomicU8::new(0) }; KEY_RING];
static KEY_HEAD: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static KEY_TAIL: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Push a decoded ASCII byte for desktop consumers (in addition to shell routing).
///
/// Wraps in a critical section so the IRQ1 post::emit path and the main-loop
/// inject_byte path cannot both reserve the same KEY_BUF slot.
pub fn push_key(b: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        use core::sync::atomic::Ordering::Relaxed;
        let h = KEY_HEAD.load(Relaxed);
        let nh = (h + 1) % KEY_RING;
        if nh != KEY_TAIL.load(Relaxed) {
            KEY_BUF[h].store(b, Relaxed);
            KEY_HEAD.store(nh, Relaxed);
        }
    });
}

/// Drain one decoded key for the desktop, or None if empty.
pub fn take_key() -> Option<u8> {
    use core::sync::atomic::Ordering::Relaxed;
    let t = KEY_TAIL.load(Relaxed);
    if t == KEY_HEAD.load(Relaxed) {
        return None;
    }
    let b = KEY_BUF[t].load(Relaxed);
    KEY_TAIL.store((t + 1) % KEY_RING, Relaxed);
    Some(b)
}

/// Inject a byte from a synthetic input source (e.g. the B3b host-bridged
/// viewer over COM3) as if it had arrived via the real PS/2 IRQ path:
/// push it into the desktop key ring and route it to `k_shell::NODE_VEC`,
/// mirroring `post::emit`'s `Output::Ascii` branch. Keeps callers (e.g.
/// `kernel_main`) from hardcoding plugin node vectors or posting signals
/// directly.
pub fn inject_byte(b: u8) {
    push_key(b);
    let _ = gos_runtime::post_signal(
        k_shell::NODE_VEC,
        Signal::Data { from: NODE_VEC.as_u64(), byte: b },
    );
}

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.ps2");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(ps2_on_init),
    on_event: Some(ps2_on_event),
    on_suspend: Some(ps2_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

// ── State ─────────────────────────────────────────────────────────────────────
// shell_target is kept as a fallback for the multi-byte UTF-8 path (non-ASCII
// output from exotic keyboard layouts).  For the common US-ASCII case the
// conditional-route table is used instead (no capability lookup overhead).

#[repr(C)]
struct Ps2State {
    shell_target: u64,
    keyboard: Keyboard<layouts::Us104Key, ScancodeSet1>,
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut Ps2State {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut Ps2State) }
}

// ── Executor callbacks ────────────────────────────────────────────────────────

unsafe extern "C" fn ps2_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        // Attempt capability resolution now; if Shell isn't loaded yet the
        // lazy-resolution path in on_event will succeed on the first IRQ.
        let abi = &*(*ctx).abi;
        let shell_target = if let Some(resolve) = abi.resolve_capability {
            resolve(b"shell".as_ptr(), 5, b"input".as_ptr(), 5)
        } else {
            0
        };
        core::ptr::write(
            (*ctx).state_ptr as *mut Ps2State,
            Ps2State {
                shell_target,
                keyboard: Keyboard::new(
                    ScancodeSet1::new(),
                    layouts::Us104Key,
                    HandleControl::MapLettersToUnicode,
                ),
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn ps2_on_event(
    ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    // ── Diagnostic: confirm the IRQ-driven dispatch path actually ─────────────
    // reaches us.  Writes a single byte to COM1 so the serial log shows
    // each invocation as a 'K'.  Cheap and non-blocking.
    {
        use x86_64::instructions::port::Port;
        let mut com1: Port<u8> = Port::new(0x3F8);
        unsafe { com1.write(b'K'); }
    }
    // ── Pre-processing: validate IRQ and read scancode ────────────────────────
    let Some(input) = (unsafe { pre::prepare(event) }) else {
        return ExecStatus::Done;
    };
    // ── Main processing: decode scancode through keyboard state machine ────────
    let state = unsafe { state_mut(ctx) };
    let Some(output) = proc::process(&mut state.keyboard, &input) else {
        return ExecStatus::Done;
    };
    // ── Post-processing: route decoded key to shell or IME ────────────────────
    unsafe { post::emit(ctx, state, output) }
}

/// Lazy-resolve shell_target via capability lookup (fallback for multi-byte path).
fn lazy_resolve_shell(ctx: *mut ExecutorContext, state: &mut Ps2State) {
    if state.shell_target != 0 {
        return;
    }
    let abi = unsafe { &*(*ctx).abi };
    if let Some(resolve) = abi.resolve_capability {
        let resolved =
            unsafe { resolve(b"shell".as_ptr(), 5, b"input".as_ptr(), 5) };
        if resolved != 0 {
            state.shell_target = resolved;
        }
    }
}

unsafe extern "C" fn ps2_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

// ── Boot-time route registration ──────────────────────────────────────────────

/// Called by `builtin_bundle` after the node is registered.
/// Populates the conditional-route table so `ps2_on_event` can return
/// `ExecStatus::Route` without any capability-lookup overhead on the hot path.
pub fn register_hook(_ctx: &mut BootContext) {
    let routes = [
        ConditionalRoute { key: PS2_ROUTE_SHELL, target: k_shell::NODE_VEC },
        ConditionalRoute { key: PS2_ROUTE_IME,   target: k_ime::NODE_VEC   },
    ];
    let _ = gos_runtime::register_node_routes(NODE_VEC, &routes);
}

// ── Plugin Descriptor ────────────────────────────────────────────────────────

const PS2_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x60, arg1: 0x64 },
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: 1, arg1: 0 },
];
const PS2_IMPORTS: &[ImportSpec] = &[
    ImportSpec { namespace: "shell", capability: "input", required: true },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_PS2"),
        name: "K_PS2",
        version: 1,
        depends_on: &[PluginId::from_ascii("K_PIC")],
        permissions: PS2_PERMS,
        exports: &[],
        imports: PS2_IMPORTS,
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_PS2"), "ps2.entry"),
            local_node_key: "ps2.entry",
            node_type: RuntimeNodeType::Driver,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x2008,
            permissions: PS2_PERMS,
            exports: &[],
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: PS2_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "ps2.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None, // register_hook wired in builtin_bundle::load_native_module
};
