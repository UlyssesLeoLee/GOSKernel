#![no_std]

// ============================================================
// GOS KERNEL TOPOLOGY — k-ps2 (native.ps2)
// 以下 Cypher 脚本描述该模块在图数据库中的完整拓扑结构。
// 可直接复制拼接后导入 Neo4j，与其他模块脚本共同构成内核完整图谱。
//
// MERGE (ps2:Plugin {id: "K_PS2", name: "k-ps2", executor: "native.ps2"})
// SET ps2.node_type = "Driver", ps2.entry_policy = "Bootstrap"
// SET ps2.state_schema = "0x2008"
//
// // 硬件资源声明
// MERGE (port_ps2:PortRange {start: "0x60", end: "0x64", label: "PS2 Data+Status"})
// MERGE (irq_kb:InterruptLine {irq: 1, label: "IRQ1 Keyboard"})
// MERGE (ps2)-[:REQUIRES_PORT]->(port_ps2)
// MERGE (ps2)-[:BINDS_IRQ]->(irq_kb)
//
// // 启动依赖 (depends on K_PIC to be initialized first)
// MERGE (pic:Plugin {id: "K_PIC"})
// MERGE (ps2)-[:DEPENDS_ON {required: true}]->(pic)
//
// // 运行时 Capability 消费 (on_init 阶段通过 resolve_capability 动态绑定)
// MERGE (shell_cap:Capability {namespace: "shell", name: "input"})
// MERGE (ps2)-[:IMPORTS {binding: "shell_target", required: false}]->(shell_cap)
//
// // 信号流：PS2 捕获键盘中断 → 编码为 Signal::Data → 发往 shell::input
// MERGE (shell:Plugin {id: "K_SHELL"})
// MERGE (ps2)-[:EMITS_SIGNAL {signal: "Data", trigger: "IRQ1", route: "shell/input"}]->(shell)
// ============================================================

use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1, DecodedKey};
use x86_64::instructions::port::Port;
use spin::Mutex;
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

static mut KEYBOARD: Option<Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>> = None;

pub fn keyboard() -> &'static Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> {
    unsafe {
        (&*core::ptr::addr_of!(KEYBOARD)).as_ref().unwrap()
    }
}

#[repr(C)]
struct Ps2State {
    shell_target: u64,
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut Ps2State {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut Ps2State) }
}

unsafe extern "C" fn ps2_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        KEYBOARD = Some(Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::MapLettersToUnicode,
        )));
        let abi = &*(*ctx).abi;
        let shell_target = if let Some(resolve) = abi.resolve_capability {
            resolve(b"shell".as_ptr(), 5, b"input".as_ptr(), 5)
        } else {
            0
        };
        core::ptr::write(
            (*ctx).state_ptr as *mut Ps2State,
            Ps2State { shell_target },
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

            let shell_target = unsafe { state_mut(ctx) }.shell_target;
            if shell_target == 0 {
                return ExecStatus::Done;
            }

            let mut keyboard = keyboard().lock();
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    let abi = unsafe { &*(*ctx).abi };

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
