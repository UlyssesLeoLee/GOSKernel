#![no_std]

use gos_hal::{meta, vaddr};
use gos_protocol::{
    packet_to_signal, ExecStatus, ExecutorContext, ExecutorId, NodeEvent, NodeExecutorVTable,
    Signal, VectorAddress,
};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 0, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.panic");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(panic_on_init),
    on_event: Some(panic_on_event),
    on_suspend: Some(panic_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

#[repr(C)]
struct PanicState {
    halts_requested: u64,
    last_signal_kind: u8,
}

fn hal_node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut PanicState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut PanicState) }
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

unsafe extern "C" fn panic_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        meta::burn_node_metadata(hal_node_ptr(), "BOOT", "PANIC");
        core::ptr::write(
            (*ctx).state_ptr as *mut PanicState,
            PanicState {
                halts_requested: 0,
                last_signal_kind: 0,
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn panic_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let signal = unsafe { packet_to_signal((*event).signal) };
    let state = unsafe { state_mut(ctx) };
    state.last_signal_kind = signal_kind_code(signal);

    if let Signal::Interrupt { irq } = signal {
        if irq == 0xFF {
            state.halts_requested = state.halts_requested.saturating_add(1);
            loop {
                x86_64::instructions::hlt();
            }
        }
    }

    ExecStatus::Done
}

unsafe extern "C" fn panic_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
