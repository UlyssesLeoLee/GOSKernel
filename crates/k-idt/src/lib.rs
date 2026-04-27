#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-idt
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_IDT", name: "k-idt"})
// SET p.executor = "k_idt::EXECUTOR_ID", p.node_type = "Service", p.state_schema = "0x2009"
//
// -- Dependencies
// MERGE (dep_K_GDT:Plugin {id: "K_GDT"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_GDT)
// MERGE (dep_K_PIT:Plugin {id: "K_PIT"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PIT)
// MERGE (dep_K_PS2:Plugin {id: "K_PS2"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_PS2)
// ============================================================


use core::arch::asm;
use core::arch::global_asm;
use core::arch::naked_asm;

use x86_64::structures::idt::InterruptDescriptorTable;
use gos_protocol::{
    TrapFrame, TrapVector, HardwareEvent,
    VectorAddress, Signal,
    ExecutorId, NodeExecutorVTable, ExecutorContext, ExecStatus, NodeEvent,
    BuiltinPluginDescriptor, NativeNodeBinding, PluginManifest, GOS_ABI_VERSION,
    PluginId, NodeSpec, RuntimeNodeType, EntryPolicy, PermissionSpec, PermissionKind,
    derive_node_id,
};
use gos_hal::{vaddr, meta};

// ── Node allocation ───────────────────────────────────────────────────────────
pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 4, 0, 0);

pub fn node_ptr() -> *mut u8 { vaddr::resolve_hal_node(NODE_VEC) }

pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.idt");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(idt_on_init),
    on_event: Some(idt_on_event),
    on_suspend: Some(idt_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

// ── Per-CPU trap frame buffer ─────────────────────────────────────────────────
// 4 slots allow nesting: e.g. timer IRQ → page fault → double fault → NMI.
const TRAP_BUF_SLOTS: usize = 4;

#[repr(C, align(64))]
struct TrapBuf {
    frames:  [TrapFrame; TRAP_BUF_SLOTS],
    in_use:  [u8;        TRAP_BUF_SLOTS],
}

static mut TRAP_BUF: TrapBuf = TrapBuf {
    frames:  [TrapFrame::zeroed(0, 0); TRAP_BUF_SLOTS],
    in_use:  [0; TRAP_BUF_SLOTS],
};

// ── TSC snapshot ─────────────────────────────────────────────────────────────
#[inline(always)]
fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe { asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack)); }
    (hi as u64) << 32 | lo as u64
}

// ── Trap Normalizer ───────────────────────────────────────────────────────────
//
// Three regimes:
//   1. Hardware IRQs (vector >= 32): enqueue a Signal::Interrupt and EOI.
//   2. CPU faults that may fire inside a native dispatch (#PF, #GP, #SS,
//      #DF) — Phase B.4.3 routes these to gos_runtime::dispatch_fault,
//      which calls into the supervisor's ModuleFaultPolicy if a current
//      dispatching instance can be resolved.  Runs on a dedicated IST
//      stack so the handler doesn't trip over the interrupted code's
//      stack state.
//   3. Other exceptions (#BP etc): fall through to enqueue.
const FAULT_PAGE_FAULT: u64 = 14;
const FAULT_GP: u64 = 13;
const FAULT_DOUBLE: u64 = 8;
const FAULT_STACK_SEGMENT: u64 = 12;

#[inline]
fn is_cpu_fault(vector: u64) -> bool {
    matches!(vector, FAULT_PAGE_FAULT | FAULT_GP | FAULT_DOUBLE | FAULT_STACK_SEGMENT)
}

#[no_mangle]
pub unsafe extern "C" fn gos_trap_normalizer(frame: *mut TrapFrame) {
    let frame = &mut *frame;

    if frame.vector == TrapVector::PageFault as u64 {
        let cr2: u64;
        asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack));
        frame.set_page_fault_addr(cr2);
    }

    // Find the first free slot; fall back to slot 0 if all are in use.
    let mut slot = 0usize;
    for i in 0..TRAP_BUF_SLOTS {
        if TRAP_BUF.in_use[i] == 0 {
            slot = i;
            break;
        }
    }
    TRAP_BUF.frames[slot] = *frame;
    TRAP_BUF.in_use[slot] = 1;

    let buf_token: u64 = slot as u64;
    let event = HardwareEvent::from_trap(frame, buf_token, rdtsc());

    if is_cpu_fault(frame.vector) {
        // Phase B.4.3: attribute the fault to the currently-dispatching
        // instance and let the supervisor's fault policy decide what to
        // do (restart / degrade / panic).  If no dispatch is active the
        // fault was kernel-internal and the dispatch hook is a no-op.
        if let Some(instance_id) = gos_runtime::dispatching_instance() {
            gos_runtime::dispatch_fault(instance_id);
        }
        // We still post a signal so observers (shell, control-plane)
        // see the trap; the supervisor's degrade path will have torn
        // the offending instance down by the time pump() picks it up.
        let signal = Signal::Interrupt { irq: event.vector };
        gos_runtime::post_irq_signal(event.vector, signal);
    } else {
        let signal = Signal::Interrupt { irq: event.vector };
        gos_runtime::post_irq_signal(event.vector, signal);
    }

    if frame.trap_vector().is_irq() {
        let mut master = x86_64::instructions::port::Port::<u8>::new(0x20);
        master.write(0x20);
        if frame.vector >= 40 {
            let mut slave = x86_64::instructions::port::Port::<u8>::new(0xA0);
            slave.write(0x20);
        }
    }

    TRAP_BUF.in_use[slot] = 0;
}

// ── Assembly Trampolines ─────────────────────────────────────────────────────

#[repr(C, packed)]
pub struct RawStack {
    r15: u64, r14: u64, r13: u64, r12: u64,
    r11: u64, r10: u64, r9:  u64, r8:  u64,
    rdi: u64, rsi: u64, rbp: u64, rbx: u64,
    rdx: u64, rcx: u64, rax: u64,
    vector: u64, error_code: u64,
    rip: u64, cs: u64, rflags: u64, rsp: u64, ss: u64,
}

#[no_mangle]
extern "C" fn rust_trap_handler(raw: *const RawStack) {
    let raw = unsafe { &*raw };
    let mut frame = TrapFrame::zeroed(raw.vector, raw.error_code);
    frame.r15 = raw.r15; frame.r14 = raw.r14; frame.r13 = raw.r13; frame.r12 = raw.r12;
    frame.r11 = raw.r11; frame.r10 = raw.r10; frame.r9  = raw.r9;  frame.r8  = raw.r8;
    frame.rdi = raw.rdi; frame.rsi = raw.rsi; frame.rbp = raw.rbp; frame.rbx = raw.rbx;
    frame.rdx = raw.rdx; frame.rcx = raw.rcx; frame.rax = raw.rax;
    
    frame.rip = raw.rip;
    frame.cs = raw.cs;
    frame.rflags = raw.rflags;
    frame.rsp = raw.rsp;
    frame.ss = raw.ss;

    unsafe { gos_trap_normalizer(&mut frame); }
}

global_asm!(
    ".global irq_common_save",
    "irq_common_save:",
    "push rax",
    "push rcx",
    "push rdx",
    "push rbx",
    "push rbp",
    "push rsi",
    "push rdi",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rdi, rsp",
    "call rust_trap_handler",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rdi",
    "pop rsi",
    "pop rbp",
    "pop rbx",
    "pop rdx",
    "pop rcx",
    "pop rax",
    "add rsp, 16",
    "iretq"
);

macro_rules! exc_handler_noerr {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        extern "C" fn $name() {
            naked_asm!(
                "push 0",
                concat!("push ", $vec),
                "jmp irq_common_save",
            )
        }
    };
}

macro_rules! exc_handler_err {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        extern "C" fn $name() {
            naked_asm!(
                concat!("push ", $vec),
                "jmp irq_common_save",
            )
        }
    };
}

macro_rules! irq_handler {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        extern "C" fn $name() {
            naked_asm!(
                "push 0",
                concat!("push ", $vec),
                "jmp irq_common_save",
            )
        }
    };
}

exc_handler_noerr!(handle_breakpoint, 3);
exc_handler_err!(handle_page_fault, 14);
exc_handler_err!(handle_general_protection, 13);
exc_handler_err!(handle_double_fault, 8); // Double fault pushes error code 0
exc_handler_err!(handle_stack_segment, 12); // Stack-segment fault

irq_handler!(handle_irq_timer,    32);
irq_handler!(handle_irq_keyboard, 33);
irq_handler!(handle_irq_cascade,  34);
irq_handler!(handle_irq_mouse,    44);

// ── Native Executor ───────────────────────────────────────────────────────────

pub fn idt() -> &'static InterruptDescriptorTable {
    unsafe {
        let p = node_ptr();
        if p.is_null() { panic!("K_IDT not initialised"); }
        &*(p.add(1024) as *mut InterruptDescriptorTable)
    }
}

/// Phase G.1 — synchronous IDT setup, callable directly from the
/// kernel-tier boot init pass (before interrupts).  `idt_on_init`
/// below is a thin wrapper for the runtime-dispatch path; both end
/// up calling this.  Idempotent: re-running with the same IDT layout
/// is harmless because `lidt` just reloads.
pub unsafe fn init_idt() {
    let p = node_ptr();
    meta::burn_node_metadata(p, "HAL", "IDT");

    let mut idt = InterruptDescriptorTable::new();

    idt.breakpoint.set_handler_fn(core::mem::transmute(handle_breakpoint as *const () as usize));
    // Phase B.4.3: route the fault classes that may fire inside a native
    // dispatch onto dedicated IST stacks (allocated in k-gdt) so the
    // handler executes on a known-good stack regardless of the
    // interrupted code's stack state.
    idt.page_fault
        .set_handler_fn(core::mem::transmute(handle_page_fault as *const () as usize))
        .set_stack_index(k_gdt::PAGE_FAULT_IST_INDEX);
    idt.general_protection_fault
        .set_handler_fn(core::mem::transmute(handle_general_protection as *const () as usize))
        .set_stack_index(k_gdt::GENERAL_PROTECTION_IST_INDEX);
    idt.stack_segment_fault
        .set_handler_fn(core::mem::transmute(handle_stack_segment as *const () as usize))
        .set_stack_index(k_gdt::STACK_SEGMENT_IST_INDEX);
    idt.double_fault
        .set_handler_fn(core::mem::transmute(handle_double_fault as *const () as usize))
        .set_stack_index(k_gdt::DOUBLE_FAULT_IST_INDEX);

    idt[32].set_handler_fn(core::mem::transmute(handle_irq_timer as *const () as usize));
    idt[33].set_handler_fn(core::mem::transmute(handle_irq_keyboard as *const () as usize));
    idt[34].set_handler_fn(core::mem::transmute(handle_irq_cascade as *const () as usize));
    idt[44].set_handler_fn(core::mem::transmute(handle_irq_mouse as *const () as usize));

    let state_ptr = p.add(1024) as *mut InterruptDescriptorTable;
    core::ptr::write(state_ptr, idt);

    crate::idt().load();
}

unsafe extern "C" fn idt_on_init(_ctx: *mut ExecutorContext) -> ExecStatus {
    unsafe { init_idt(); }
    ExecStatus::Done
}

unsafe extern "C" fn idt_on_event(_ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    // Interrupt dispatch is handled by assembly trampolines, not by on_event.
    let Some(input) = pre::prepare(event) else { return ExecStatus::Done; };
    let Some(output) = proc::process(input) else { return ExecStatus::Done; };
    post::emit(output)
}

unsafe extern "C" fn idt_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}

// ── Plugin Descriptor ────────────────────────────────────────────────────────

const IDT_PERMS: &[PermissionSpec] = &[
    PermissionSpec { kind: PermissionKind::IrqBind, arg0: u64::MAX, arg1: 0 },
];

pub const PLUGIN_DESCRIPTOR: BuiltinPluginDescriptor = BuiltinPluginDescriptor {
    manifest: PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PluginId::from_ascii("K_IDT"),
        name: "K_IDT",
        version: 1,
        depends_on: &[PluginId::from_ascii("K_GDT"), PluginId::from_ascii("K_PIT"), PluginId::from_ascii("K_PS2")],
        permissions: IDT_PERMS,
        exports: &[],
        imports: &[],
        nodes: &[NodeSpec {
            node_id: derive_node_id(PluginId::from_ascii("K_IDT"), "idt.entry"),
            local_node_key: "idt.entry",
            node_type: RuntimeNodeType::Service,
            entry_policy: EntryPolicy::Bootstrap,
            executor_id: EXECUTOR_ID,
            state_schema_hash: 0x2009,
            permissions: IDT_PERMS,
            exports: &[],
            vector_ref: None,
        }],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    },
    granted_permissions: IDT_PERMS,
    nodes: &[NativeNodeBinding {
        vector: NODE_VEC,
        local_node_key: "idt.entry",
        executor: EXECUTOR_VTABLE,
    }],
    register_hook: None,
};
