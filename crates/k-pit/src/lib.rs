#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-pit
//
// MERGE (p:Plugin {id: "K_PIT", name: "k-pit"})
// SET p.executor = "k_pit::EXECUTOR_ID", p.node_type = "Driver", p.state_schema = "0x2007"
//
// MERGE (dep_K_PIC:Plugin {id: "K_PIC"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PIC)
//
// MERGE (pr_40:PortRange {start: "0x40", end: "0x43"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_40)
// MERGE (irq_0:InterruptLine {irq: "0"})
// MERGE (p)-[:BINDS_IRQ]->(irq_0)
// ============================================================

use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::instructions::port::Port;
use gos_protocol::*;
use gos_hal::{vaddr, meta};

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 6, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.pit");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init:     Some(pit_on_init),
    on_event:    Some(pit_on_event),
    on_suspend:  Some(pit_on_suspend),
    on_resume:   None,
    on_teardown: None,
    on_telemetry: None,
};

const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND:  u16 = 0x43;
const PIT_BASE_HZ:  u32 = 1_193_182;

pub fn node_ptr() -> *mut u8 {
    vaddr::resolve_hal_node(NODE_VEC)
}

pub fn ticks() -> &'static AtomicUsize {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_PIT Matrix not initialized"); }
        &*(p.add(1024) as *mut AtomicUsize)
    }
}

unsafe fn init_node_state() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "PIT");
    let state_ptr = p.add(1024) as *mut AtomicUsize;
    core::ptr::write(state_ptr, AtomicUsize::new(0));
}

unsafe extern "C" fn pit_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe { init_node_state(); }
    ExecStatus::Done
}

unsafe extern "C" fn pit_on_event(
    _ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    post::emit(output)
}

unsafe extern "C" fn pit_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

pub fn get_ticks() -> usize {
    ticks().load(Ordering::Relaxed)
}

pub fn init_pit_hz(hz: u32) {
    let requested = hz.clamp(30, 240);
    let divisor = (PIT_BASE_HZ / requested).max(1).min(u16::MAX as u32) as u16;
    let mut command  = Port::<u8>::new(PIT_COMMAND);
    let mut channel0 = Port::<u8>::new(PIT_CHANNEL0);
    unsafe {
        command.write(0x36);
        channel0.write((divisor & 0x00FF) as u8);
        channel0.write((divisor >> 8) as u8);
    }
}
