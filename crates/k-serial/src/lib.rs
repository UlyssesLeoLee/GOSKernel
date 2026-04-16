#![no_std]


// ==============================================================
// GOS KERNEL TOPOLOGY — k-serial (native.serial)
// 以下 Cypher 脚本可直接导入 Neo4j，与其他模块共同还原内核完整图谱。
//
// MERGE (p:Plugin {id: "K_SERIAL", name: "k-serial"})
// SET p.executor = "native.serial", p.node_type = "Driver", p.state_schema = "0x2002"
//
// // ── 硬件资源边界 ──────────────────────────────────────────
// MERGE (hw_3f8:PortRange {start: "0x3F8", end: "8", label: "COM1 Serial Port"})
// MERGE (p)-[:REQUIRES_PORT]->(hw_3f8)
//
// // ── 能力导出 (EXPORTS Capability) ────────────────────────
// MERGE (cap_serial_write:Capability {namespace: "serial", name: "write"})
// MERGE (p)-[:EXPORTS]->(cap_serial_write)
// ==============================================================

use gos_hal::{meta, vaddr};
use gos_protocol::{
    packet_to_signal, ExecStatus, ExecutorContext, ExecutorId, NodeEvent, NodeExecutorVTable,
    Signal, VectorAddress,
};
use spin::Mutex;
use uart_16550::SerialPort;

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 2, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.serial");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(serial_on_init),
    on_event: Some(serial_on_event),
    on_suspend: Some(serial_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

#[repr(C)]
struct SerialState {
    bytes_written: u64,
    last_signal_kind: u8,
}

pub fn node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

pub fn serial1() -> &'static Mutex<SerialPort> {
    unsafe { &*(node_ptr().add(1024) as *mut Mutex<SerialPort>) }
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut SerialState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut SerialState) }
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

unsafe extern "C" fn serial_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let hal_ptr = node_ptr();
    unsafe {
        meta::burn_node_metadata(hal_ptr, "HAL", "SERIAL");
        let mut serial_port = SerialPort::new(0x3F8);
        serial_port.init();
        core::ptr::write(hal_ptr.add(1024) as *mut Mutex<SerialPort>, Mutex::new(serial_port));
        core::ptr::write(
            (*ctx).state_ptr as *mut SerialState,
            SerialState {
                bytes_written: 0,
                last_signal_kind: 0,
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn serial_on_event(
    ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    let signal = unsafe { packet_to_signal((*event).signal) };
    let state = unsafe { state_mut(ctx) };
    state.last_signal_kind = signal_kind_code(signal);

    if let Signal::Data { byte, .. } = signal {
        use core::fmt::Write;
        x86_64::instructions::interrupts::without_interrupts(|| {
            let _ = serial1().lock().write_char(byte as char);
        });
        state.bytes_written = state.bytes_written.saturating_add(1);
    }

    ExecStatus::Done
}

unsafe extern "C" fn serial_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
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
