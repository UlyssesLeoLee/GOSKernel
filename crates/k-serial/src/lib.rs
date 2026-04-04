#![no_std]

use uart_16550::SerialPort;
use spin::Mutex;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 2, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub fn serial1() -> &'static Mutex<SerialPort> {
    unsafe { &*(node_ptr().add(1024) as *mut Mutex<SerialPort>) }
}

pub unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "SERIAL");

    let mut serial_port = SerialPort::new(0x3F8);
    serial_port.init();

    let state_ptr = p.add(1024) as *mut Mutex<SerialPort>;
    core::ptr::write(state_ptr, Mutex::new(serial_port));
}

pub struct SerialCell { state: NodeState }

impl SerialCell {
    pub const fn new() -> Self { Self { state: NodeState::Unregistered } }
}

impl NodeCell for SerialCell {
    fn declare(&self) -> CellDeclaration {
        let mut edges = [CellEdge::NONE; MAX_CELL_EDGES];
        edges[0] = CellEdge::new("WRITE", 0x08, 0);
        CellDeclaration {
            vec: NODE_VEC, domain_label: "HAL", name: "SERIAL",
            edges, edge_count: 1, depends_on: &[],
        }
    }

    unsafe fn init(&mut self) { init_node_state(); self.state = NodeState::Ready; }

    fn on_activate(&mut self) -> CellResult { CellResult::Done }

    fn on_signal(&mut self, signal: Signal) -> CellResult {
        match signal {
            Signal::Data { byte, .. } => {
                use core::fmt::Write;
                let _ = serial1().lock().write_char(byte as char);
                CellResult::Done
            }
            _ => CellResult::Done,
        }
    }

    fn on_suspend(&mut self) { self.state = NodeState::Suspended; }
    fn state(&self) -> NodeState { self.state }
    fn vec(&self) -> VectorAddress { NODE_VEC }
}

pub static SERIAL_CELL: spin::Mutex<SerialCell> = spin::Mutex::new(SerialCell::new());

impl PluginEntry for SerialCell {
    const VEC: VectorAddress = NODE_VEC;
    const WAVEFRONT: u32 = 0;

    fn plugin_main(_ctx: &mut BootContext) {
        gos_hal::ngr::try_mount_cell(Self::VEC, &SERIAL_CELL);
    }
}

pub fn _serial_print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        serial1().lock().write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::_serial_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
