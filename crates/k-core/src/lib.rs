#![no_std]


// ==============================================================
// GOS KERNEL TOPOLOGY — k-core (native.core)
// 以下 Cypher 脚本可直接导入 Neo4j，与其他模块共同还原内核完整图谱。
//
// MERGE (p:Plugin {id: "K_CORE", name: "k-core"})
// SET p.executor = "native.core", p.node_type = "Service", p.state_schema = "0x2021"
// ==============================================================

// MERGE (p:Plugin {id: "K_CORE"})
// SET p.name = "k-core"

use core::arch::global_asm;
use gos_protocol::{
    packet_to_signal, ExecStatus, ExecutorContext, ExecutorId, NodeEvent, NodeExecutorVTable,
    Signal, VectorAddress, CORE_CONTROL_SWITCH_CONTEXT,
};

// Link the assembly context switch routine.
global_asm!(include_str!("switch.S"));

pub const NODE_VEC: VectorAddress = gos_protocol::vectors::CORE_CTX;
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.core");

pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(core_on_init),
    on_event: Some(core_on_event),
    on_suspend: Some(core_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

unsafe extern "C" fn core_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

unsafe extern "C" fn core_on_event(
    _ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> ExecStatus {
    // Decode the incoming signal and act on context-switch control messages.
    let signal = unsafe { packet_to_signal((*event).signal) };
    if let Signal::Control { cmd, val: _ } = signal {
        if cmd == CORE_CONTROL_SWITCH_CONTEXT {
            let packet = unsafe { (*event).signal };
            let prev = packet.arg1 as *mut TaskContext;
            let next = packet.arg2 as *const TaskContext;

            if !prev.is_null() && !next.is_null() {
                unsafe {
                    switch_context(prev, next);
                }
            }
        }
    }
    ExecStatus::Done
}

unsafe extern "C" fn core_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskContext {
    pub r15: u64, // 0x00
    pub r14: u64, // 0x08
    pub r13: u64, // 0x10
    pub r12: u64, // 0x18
    pub rbp: u64, // 0x20
    pub rbx: u64, // 0x28
    pub rsp: u64, // 0x30
    pub cr3: u64, // 0x38
}

impl TaskContext {
    /// Initialize a new context for a kernel thread.
    /// `stack_top` must be valid allocated memory mapped into `cr3`.
    /// `entry` is the function to execute.
    pub fn new_kernel_thread(entry: u64, stack_top: u64, cr3: u64) -> Self {
        // We simulate a call to `entry` by pushing it to the stack,
        // so when context_switch runs `ret`, it pops `entry` into RIP.
        let mut rsp = stack_top;
        
        unsafe {
            // Push the entry point to the stack.
            rsp -= 8;
            core::ptr::write(rsp as *mut u64, entry);
            
            // Push a fake return address for the entry function.
            // If the thread returns, it will panic or exit gracefully.
            rsp -= 8;
            core::ptr::write(rsp as *mut u64, thread_exit as *const () as usize as u64);
        }

        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbp: rsp,
            rbx: 0,
            rsp,
            cr3,
        }
    }
}

extern "C" {
    /// Assembly routine to switch between two tasks.
    /// 
    /// - `prev`: Pointer to the `TaskContext` of the task being suspended.
    /// - `next`: Pointer to the `TaskContext` of the task being resumed.
    pub fn switch_context(prev: *mut TaskContext, next: *const TaskContext);
}

/// Fallback exit routine for threads that accidentally return from their entry point.
extern "C" fn thread_exit() -> ! {
    panic!("Kernel thread returned! Unhandled graceful exit.");
}
