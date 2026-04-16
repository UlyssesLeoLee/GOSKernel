#![no_std]


// ============================================================
// GOS KERNEL TOPOLOGY — k-mouse
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_MOUSE", name: "k-mouse"})
// SET p.executor = "k_mouse::EXECUTOR_ID", p.node_type = "Driver", p.state_schema = "0x2013"
//
// -- Dependencies
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
// MERGE (dep_K_PS2:Plugin {id: "K_PS2"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PS2)
// MERGE (dep_K_IDT:Plugin {id: "K_IDT"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_IDT)
//
// -- Hardware Resources
// MERGE (pr_60:PortRange {start: "0x60", end: "0x64"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_60)
// MERGE (irq_12:InterruptLine {irq: "12"})
// MERGE (p)-[:BINDS_IRQ]->(irq_12)
//
// -- Imported Capabilities (Dependencies)
// MERGE (cap_display_pointer:Capability {namespace: "display", name: "pointer"})
// MERGE (p)-[:IMPORTS]->(cap_display_pointer)
// ============================================================


use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};

use gos_protocol::{
    packet_to_signal, signal_to_packet, DISPLAY_CONTROL_POINTER_COL, DISPLAY_CONTROL_POINTER_ROW,
    DISPLAY_CONTROL_POINTER_VISIBLE, ExecStatus, ExecutorContext, ExecutorId, KernelAbi,
    NodeEvent, NodeExecutorVTable, Signal, VectorAddress,
};
use x86_64::instructions::port::Port;

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 5, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.mouse");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(mouse_on_init),
    on_event: Some(mouse_on_event),
    on_suspend: Some(mouse_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

const DISPLAY_FALLBACK_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
const PS2_DATA_PORT: u16 = 0x60;
const PS2_STATUS_PORT: u16 = 0x64;
const PS2_COMMAND_PORT: u16 = 0x64;
const MOUSE_IRQ: u8 = k_pic::InterruptIndex::Mouse as u8;

static PACKET_INDEX: AtomicU8 = AtomicU8::new(0);
static PACKET0: AtomicU8 = AtomicU8::new(0);
static PACKET1: AtomicU8 = AtomicU8::new(0);
static PACKET2: AtomicU8 = AtomicU8::new(0);
static PENDING_DX: AtomicI32 = AtomicI32::new(0);
static PENDING_DY: AtomicI32 = AtomicI32::new(0);
static PENDING_BUTTONS: AtomicU8 = AtomicU8::new(0);

#[repr(C)]
struct MouseState {
    display_target: u64,
    x_px: i32,
    y_px: i32,
    col: u8,
    row: u8,
    buttons: u8,
    visible: u8,
    online: u8,
}

#[derive(Clone, Copy)]
struct DisplaySink {
    target: u64,
    abi: &'static KernelAbi,
}

impl DisplaySink {
    fn emit_control(&self, cmd: u8, val: u8) {
        if let Some(emit_signal) = self.abi.emit_signal {
            unsafe {
                let _ = emit_signal(self.target, signal_to_packet(Signal::Control { cmd, val }));
            }
        }
    }
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut MouseState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut MouseState) }
}

fn sink_from_ctx(ctx: *mut ExecutorContext) -> DisplaySink {
    let ctx_ref = unsafe { &*ctx };
    let abi = unsafe { &*ctx_ref.abi };
    let state = unsafe { state_mut(ctx) };
    DisplaySink {
        target: if state.display_target == 0 {
            DISPLAY_FALLBACK_VEC.as_u64()
        } else {
            state.display_target
        },
        abi,
    }
}

fn wait_can_write() {
    let mut status = Port::<u8>::new(PS2_STATUS_PORT);
    let mut spins = 0usize;
    while spins < 100_000 {
        let flags = unsafe { status.read() };
        if (flags & 0x02) == 0 {
            break;
        }
        spins += 1;
        core::hint::spin_loop();
    }
}

fn wait_can_read() {
    let mut status = Port::<u8>::new(PS2_STATUS_PORT);
    let mut spins = 0usize;
    while spins < 100_000 {
        let flags = unsafe { status.read() };
        if (flags & 0x01) != 0 {
            break;
        }
        spins += 1;
        core::hint::spin_loop();
    }
}

fn controller_write(cmd: u8) {
    wait_can_write();
    let mut port = Port::<u8>::new(PS2_COMMAND_PORT);
    unsafe {
        port.write(cmd);
    }
}

fn data_write(val: u8) {
    wait_can_write();
    let mut port = Port::<u8>::new(PS2_DATA_PORT);
    unsafe {
        port.write(val);
    }
}

fn data_read() -> u8 {
    wait_can_read();
    let mut port = Port::<u8>::new(PS2_DATA_PORT);
    unsafe { port.read() }
}

fn mouse_write(val: u8) {
    controller_write(0xD4);
    data_write(val);
}

fn mouse_expect_ack() {
    let _ = data_read();
}

fn init_ps2_mouse() {
    controller_write(0xA8);
    controller_write(0x20);
    let status = data_read() | 0x02;
    controller_write(0x60);
    data_write(status);

    mouse_write(0xF6);
    mouse_expect_ack();
    mouse_write(0xF4);
    mouse_expect_ack();
}

fn clamp(value: i32, min: i32, max: i32) -> i32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn push_pointer_state(sink: &DisplaySink, state: &MouseState) {
    sink.emit_control(DISPLAY_CONTROL_POINTER_COL, state.col);
    sink.emit_control(DISPLAY_CONTROL_POINTER_ROW, state.row);
    sink.emit_control(DISPLAY_CONTROL_POINTER_VISIBLE, state.visible);
}

fn apply_pending_motion(ctx: *mut ExecutorContext) {
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };
    let dx = PENDING_DX.swap(0, Ordering::Relaxed);
    let dy = PENDING_DY.swap(0, Ordering::Relaxed);
    let buttons = PENDING_BUTTONS.load(Ordering::Relaxed);

    if dx == 0 && dy == 0 && buttons == state.buttons {
        return;
    }

    state.x_px = clamp(state.x_px + dx, 0, 639);
    state.y_px = clamp(state.y_px - dy, 0, 399);
    state.col = (state.x_px / 8) as u8;
    state.row = (state.y_px / 16) as u8;
    state.buttons = buttons;
    state.visible = 1;
    push_pointer_state(&sink, state);
}

unsafe extern "C" fn mouse_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let display_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"display".as_ptr(),
                    b"display".len(),
                    b"pointer".as_ptr(),
                    b"pointer".len(),
                )
            }
        } else {
            0
        }
    };

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut MouseState,
            MouseState {
                display_target: if display_target == 0 {
                    DISPLAY_FALLBACK_VEC.as_u64()
                } else {
                    display_target
                },
                x_px: 320,
                y_px: 192,
                col: 40,
                row: 12,
                buttons: 0,
                visible: 1,
                online: 1,
            },
        );
    }

    init_ps2_mouse();
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };
    push_pointer_state(&sink, state);
    ExecStatus::Done
}

unsafe extern "C" fn mouse_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let signal = packet_to_signal(unsafe { (*event).signal });
    match signal {
        Signal::Spawn { .. } => {
            apply_pending_motion(ctx);
            ExecStatus::Done
        }
        Signal::Interrupt { irq } if irq == MOUSE_IRQ => {
            let mut port = Port::<u8>::new(PS2_DATA_PORT);
            let byte = unsafe { port.read() };
            let slot = PACKET_INDEX.load(Ordering::Relaxed);

            match slot {
                0 => {
                    PACKET0.store(byte, Ordering::Relaxed);
                    PACKET_INDEX.store(1, Ordering::Relaxed);
                }
                1 => {
                    PACKET1.store(byte, Ordering::Relaxed);
                    PACKET_INDEX.store(2, Ordering::Relaxed);
                }
                _ => {
                    PACKET2.store(byte, Ordering::Relaxed);
                    PACKET_INDEX.store(0, Ordering::Relaxed);

                    let p0 = PACKET0.load(Ordering::Relaxed);
                    let p1 = PACKET1.load(Ordering::Relaxed);
                    let p2 = PACKET2.load(Ordering::Relaxed);
                    if (p0 & 0x08) != 0 && (p0 & 0xC0) == 0 {
                        PENDING_DX.fetch_add((p1 as i8) as i32, Ordering::Relaxed);
                        PENDING_DY.fetch_add((p2 as i8) as i32, Ordering::Relaxed);
                        PENDING_BUTTONS.store(p0 & 0x07, Ordering::Relaxed);
                        
                        apply_pending_motion(ctx);
                    }
                }
            }
            ExecStatus::Done
        }
        _ => ExecStatus::Done,
    }
}

unsafe extern "C" fn mouse_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
