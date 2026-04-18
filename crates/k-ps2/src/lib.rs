#![no_std]

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

use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1, DecodedKey};
use x86_64::instructions::port::Port;
use gos_protocol::*;

pub const NODE_VEC: VectorAddress = gos_protocol::vectors::CORE_PS2;

/// Route key → k_shell::NODE_VEC (ASCII / special keys).
pub const PS2_ROUTE_SHELL: u8 = 0x00;
/// Route key → k_ime::NODE_VEC (reserved for IME pre-processing).
pub const PS2_ROUTE_IME: u8 = 0x01;

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
    let signal = packet_to_signal(unsafe { (*event).signal });
    let Signal::Interrupt { irq } = signal else {
        return ExecStatus::Done;
    };
    if irq != k_pic::InterruptIndex::Keyboard.as_u8() {
        return ExecStatus::Done;
    }

    let mut port = Port::new(0x60u16);
    let scancode: u8 = unsafe { port.read() };

    let state = unsafe { state_mut(ctx) };

    // ── Decode scancode ───────────────────────────────────────────────────────
    let keyboard = &mut state.keyboard;
    let Ok(Some(key_event)) = keyboard.add_byte(scancode) else {
        return ExecStatus::Done;
    };
    let Some(key) = keyboard.process_keyevent(key_event) else {
        return ExecStatus::Done;
    };

    // ── Resolve the output byte(s) ────────────────────────────────────────────
    let byte = match key {
        DecodedKey::Unicode(ch) => {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            let bytes = s.as_bytes();

            if bytes.len() == 1 {
                // Fast path: single-byte ASCII — use conditional routing.
                // ctx.route_signal is set below; ctx.route_key selects the
                // target from the route table registered in register_hook.
                Some(bytes[0])
            } else {
                // Multi-byte UTF-8 (e.g. exotic composed characters on
                // non-US layouts): fall back to direct emit so every byte
                // reaches Shell in order.
                lazy_resolve_shell(ctx, state);
                if state.shell_target != 0 {
                    let abi = unsafe { &*(*ctx).abi };
                    for &b in bytes {
                        if let Some(emit) = abi.emit_signal {
                            unsafe {
                                let _ = emit(
                                    state.shell_target,
                                    signal_to_packet(Signal::Data {
                                        from: NODE_VEC.as_u64(),
                                        byte: b,
                                    }),
                                );
                            }
                        }
                    }
                }
                return ExecStatus::Done;
            }
        }
        DecodedKey::RawKey(k) => match k {
            pc_keyboard::KeyCode::Backspace => Some(0x08),
            pc_keyboard::KeyCode::ArrowUp   => Some(INPUT_KEY_UP),
            pc_keyboard::KeyCode::ArrowDown => Some(INPUT_KEY_DOWN),
            pc_keyboard::KeyCode::PageUp    => Some(INPUT_KEY_PAGE_UP),
            pc_keyboard::KeyCode::PageDown  => Some(INPUT_KEY_PAGE_DOWN),
            pc_keyboard::KeyCode::Escape    => Some(0x1B),
            _ => None,
        },
    };

    let Some(b) = byte else {
        return ExecStatus::Done;
    };

    // ── Conditional routing ───────────────────────────────────────────────────
    // Write the Data signal into ctx.route_signal so the runtime forwards
    // the *decoded* byte (not the raw Interrupt) to the chosen target.
    // The route table (key 0 → Shell, key 1 → IME) is registered in
    // register_hook and stored in the runtime's NodeRecord.
    unsafe {
        (*ctx).route_signal =
            signal_to_packet(Signal::Data { from: NODE_VEC.as_u64(), byte: b });
        (*ctx).route_key = PS2_ROUTE_SHELL;
    }
    ExecStatus::Route
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
