#![no_std]

use core::arch::global_asm;

// Link the assembly context switch routine.
global_asm!(include_str!("switch.S"));

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
            core::ptr::write(rsp as *mut u64, thread_exit as usize as u64);
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
