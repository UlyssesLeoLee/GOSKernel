#![no_std]

use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1, DecodedKey};
use x86_64::instructions::port::Port;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 7, 0, 0);

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.ps2");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(ps2_on_init),
    on_event: Some(ps2_on_event),
    on_suspend: Some(ps2_on_suspend),
    on_resume: None,
    on_teardown: None,
};

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub fn keyboard() -> &'static Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_PS2 Matrix not initialized"); }
        &*(p.add(1024) as *mut Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>)
    }
}

unsafe extern "C" fn ps2_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "PS2");
    let state_ptr = p.add(1024) as *mut Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>;
    core::ptr::write(state_ptr, Mutex::new(Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::MapLettersToUnicode,
    )));
    ExecStatus::Done
}

unsafe extern "C" fn ps2_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let signal = packet_to_signal(unsafe { (*event).signal });
    if let Signal::Interrupt { irq } = signal {
        if irq == k_pic::InterruptIndex::Keyboard.as_u8() {
            let mut port = Port::new(0x60);
            let scancode: u8 = unsafe { port.read() };

            let mut keyboard = keyboard().lock();
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    let abi = unsafe { &*(*ctx).abi };
                    let shell_vec = VectorAddress::new(6, 1, 0, 0);
                    
                    match key {
                        DecodedKey::Unicode(character) => {
                            let mut b = [0; 4];
                            let s = character.encode_utf8(&mut b);
                            for byte in s.bytes() {
                                if let Some(emit) = abi.emit_signal {
                                    unsafe {
                                        let _ = emit(shell_vec.as_u64(), signal_to_packet(Signal::Data { from: NODE_VEC.as_u64(), byte }));
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
                                        let _ = emit(shell_vec.as_u64(), signal_to_packet(Signal::Data { from: NODE_VEC.as_u64(), byte }));
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
