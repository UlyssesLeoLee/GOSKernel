#![no_std]
#![feature(abi_x86_interrupt)]

use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1, DecodedKey};
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 7, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub fn keyboard() -> &'static Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_PS2 Matrix not initialized"); }
        &*(p.add(1024) as *mut Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>)
    }
}

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "PS2");
    let state_ptr = p.add(1024) as *mut Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>;
    core::ptr::write(state_ptr, Mutex::new(Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::MapLettersToUnicode,
    )));
}

pub struct Ps2Cell { state: NodeState }

impl Ps2Cell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for Ps2Cell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("DATA", 0x04, VectorAddress::new(6, 1, 0, 0).as_u64());
        CellDeclaration {
            vec: NODE_VEC, domain_label: "HAL", name: "PS2",
            edges, edge_count: 1, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { init_node_state(); self.state = NodeState::Ready; }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }
    fn on_signal(&mut self, _: Signal) -> CellResult { CellResult::Done }
    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static PS2_CELL: spin::Mutex<Ps2Cell> = spin::Mutex::new(Ps2Cell::new());

impl PluginEntry for Ps2Cell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 2; // Depends on PIC

    fn plugin_main(_ctx: &mut BootContext) {
        gos_hal::ngr::try_mount_cell(Self::VEC, &PS2_CELL);
    }
}

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    let mut keyboard = keyboard().lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    let shell_vec = VectorAddress::new(6, 1, 0, 0);
                    let mut b = [0; 4];
                    let s = character.encode_utf8(&mut b);
                    for byte in s.bytes() {
                        gos_hal::ngr::post_signal(
                            shell_vec,
                            Signal::Data { from: NODE_VEC.as_u64(), byte }
                        );
                    }
                }
                DecodedKey::RawKey(key) => {
                    let shell_vec = VectorAddress::new(6, 1, 0, 0);
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
                        gos_hal::ngr::post_signal(
                            shell_vec,
                            Signal::Data { from: NODE_VEC.as_u64(), byte }
                        );
                    }
                }
            }
        }
    }

    unsafe {
        k_pic::pics().lock()
            .notify_end_of_interrupt(k_pic::InterruptIndex::Keyboard.as_u8());
    }
}
