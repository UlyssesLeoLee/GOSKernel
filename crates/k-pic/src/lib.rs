#![no_std]

use gos_hal::{meta, vaddr};
use gos_protocol::{
    packet_to_signal, ExecStatus, ExecutorContext, ExecutorId, NodeEvent, NodeExecutorVTable,
    Signal, VectorAddress,
};
use pic8259::ChainedPics;
use spin::Mutex;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 5, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pic");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(pic_on_init),
    on_event: Some(pic_on_event),
    on_suspend: Some(pic_on_suspend),
    on_resume: None,
    on_teardown: None,
};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
    Mouse = PIC_2_OFFSET + 4,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

#[repr(C)]
struct PicRuntimeState {
    init_count: u64,
    last_signal_kind: u8,
}

pub fn node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

pub fn pics() -> &'static Mutex<ChainedPics> {
    unsafe {
        let p = node_ptr();
        if p.is_null() {
            panic!("K_PIC Matrix not initialized");
        }
        &*(p.add(1024) as *mut Mutex<ChainedPics>)
    }
}

unsafe fn runtime_state_mut(ctx: *mut ExecutorContext) -> &'static mut PicRuntimeState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut PicRuntimeState) }
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

unsafe fn init_hal_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "PIC");
    let state_ptr = p.add(1024) as *mut Mutex<ChainedPics>;
    core::ptr::write(
        state_ptr,
        Mutex::new(ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)),
    );
}

unsafe extern "C" fn pic_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe {
        init_hal_state();
        core::ptr::write(
            (*ctx).state_ptr as *mut PicRuntimeState,
            PicRuntimeState {
                init_count: 0,
                last_signal_kind: 0,
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn pic_on_event(
    ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    let signal = unsafe { packet_to_signal((*event).signal) };
    let state = unsafe { runtime_state_mut(ctx) };
    state.last_signal_kind = signal_kind_code(signal);

    if let Signal::Spawn { .. } = signal {
        init_pic();
        state.init_count = state.init_count.saturating_add(1);
    }

    ExecStatus::Done
}

unsafe extern "C" fn pic_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

pub fn init_pic() {
    unsafe {
        pics().lock().initialize();
        pics().lock().write_masks(0xF8, 0xEF);
    }
}
