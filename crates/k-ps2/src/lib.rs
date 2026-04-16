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
// ============================================================


use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1, DecodedKey};
use x86_64::instructions::port::Port;
use gos_protocol::*;

pub const NODE_VEC: VectorAddress = gos_protocol::vectors::CORE_PS2;

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

#[repr(C)]
struct Ps2State {
    shell_target: u64,
    keyboard: Keyboard<layouts::Us104Key, ScancodeSet1>,
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut Ps2State {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut Ps2State) }
}

unsafe extern "C" fn ps2_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
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
                )
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn ps2_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let signal = packet_to_signal(unsafe { (*event).signal });
    if let Signal::Interrupt { irq } = signal {
        if irq == k_pic::InterruptIndex::Keyboard.as_u8() {
            let mut port = Port::new(0x60);
            let scancode: u8 = unsafe { port.read() };

            let state = unsafe { state_mut(ctx) };

            // Lazy resolution: PS2 boots before Shell, so shell_target may
            // still be 0 from on_init.  Re-resolve on every event until the
            // capability appears in the graph.
            if state.shell_target == 0 {
                let abi = unsafe { &*(*ctx).abi };
                if let Some(resolve) = abi.resolve_capability {
                    let resolved = unsafe { resolve(b"shell".as_ptr(), 5, b"input".as_ptr(), 5) };
                    if resolved != 0 {
                        state.shell_target = resolved;
                    }
                }
                if state.shell_target == 0 {
                    return ExecStatus::Done;
                }
            }

            let keyboard = &mut state.keyboard;
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    let abi = unsafe { &*(*ctx).abi };
                    let shell_target = state.shell_target;

                    match key {
                        DecodedKey::Unicode(character) => {
                            let mut b = [0; 4];
                            let s = character.encode_utf8(&mut b);
                            for byte in s.bytes() {
                                if let Some(emit) = abi.emit_signal {
                                    unsafe {
                                        let _ = emit(shell_target, signal_to_packet(Signal::Data { from: NODE_VEC.as_u64(), byte }));
                                    }
                                }
                            }
                        }
                        DecodedKey::RawKey(key) => {
                            let special = if key == pc_keyboard::KeyCode::Backspace {
                                Some(0x08)
                            } else if key == pc_keyboard::KeyCode::ArrowUp {
                                Some(INPUT_KEY_UP)
                            } else if key == pc_keyboard::KeyCode::ArrowDown {
                                Some(INPUT_KEY_DOWN)
                            } else if key == pc_keyboard::KeyCode::PageUp {
                                Some(INPUT_KEY_PAGE_UP)
                            } else if key == pc_keyboard::KeyCode::PageDown {
                                Some(INPUT_KEY_PAGE_DOWN)
                            } else if key == pc_keyboard::KeyCode::Escape {
                                Some(0x1B)
                            } else {
                                None
                            };

                            if let Some(byte) = special {
                                if let Some(emit) = abi.emit_signal {
                                    unsafe {
                                        let _ = emit(shell_target, signal_to_packet(Signal::Data { from: NODE_VEC.as_u64(), byte }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    ExecStatus::Done
}

unsafe extern "C" fn ps2_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
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
    register_hook: None,
};
