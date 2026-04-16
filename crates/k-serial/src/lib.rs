#![no_std]


// ============================================================
// GOS KERNEL TOPOLOGY — k-serial
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_SERIAL", name: "k-serial"})
// SET p.executor = "k_serial::EXECUTOR_ID", p.node_type = "Driver", p.state_schema = "0x2002"
//
// -- Hardware Resources
// MERGE (pr_3F8:PortRange {start: "0x3F8", end: "8"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_3F8)
//
// -- Exported Capabilities (APIs)
// MERGE (cap_serial_write:Capability {namespace: "serial", name: "write"})
// MERGE (p)-[:EXPORTS]->(cap_serial_write)
// ============================================================


use gos_hal::{meta, vaddr};
use gos_protocol::*;
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

const SERIAL_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::PortIo, arg0: 0x3F8, arg1: 8 },
];
const SERIAL_EXPORTS: &[CapabilitySpec] = &[
    CapabilitySpec { namespace: "serial", name: "write" },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_SERIAL"),
        name: "K_SERIAL",
        version: 1,
        depends_on: &[],
        permissions: SERIAL_PERMS,
        exports: SERIAL_EXPORTS,
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_SERIAL"), "serial.entry"),
            local_node_key: "serial.entry",
            node_type: RuntimeNodeType::Driver,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x2002,
            permissions: SERIAL_PERMS,
            exports: SERIAL_EXPORTS,
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: SERIAL_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "serial.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None,
};
