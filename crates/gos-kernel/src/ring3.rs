//! Phase E.2 — syscall entry surface for Ring 3 plugins.
//!
//! This module wires the kernel side of the syscall path:
//!
//!   1. `IA32_EFER.SCE = 1` — enable the `syscall` instruction.
//!   2. `IA32_STAR` — encodes the kernel CS/SS and user CS/SS selectors
//!      so `syscall`/`sysret` flip the segment registers atomically.
//!   3. `IA32_LSTAR` — kernel handler RIP for the `syscall` instruction.
//!   4. `IA32_FMASK` — RFLAGS bits to clear on `syscall` entry (masks IF
//!      so interrupts are disabled until the handler is on its kernel stack).
//!
//! The handler is a small naked trampoline that calls
//! [`rust_syscall_handler`], which decodes RAX as the syscall number
//! and dispatches to the existing kernel ABI (alloc_pages, emit_signal,
//! resolve_capability, ...).
//!
//! ── Status ────────────────────────────────────────────────────────────
//! Until an ELF-loaded plugin actually runs in Ring 3 (B.4.6.x +
//! Phase E.3), no code path issues `syscall`.  The MSRs are programmed
//! anyway so that the moment a user-mode .gosmod is mapped, its first
//! `syscall` lands on a working trampoline instead of `#GP`.

use core::arch::naked_asm;

use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};
use x86_64::registers::rflags::RFlags;
use x86_64::VirtAddr;

/// Syscall numbers — kept stable so external plugins can hard-code
/// them.  Numbers are arbitrary but never reused.  Future syscalls
/// extend the table without renumbering.
#[repr(u64)]
pub enum SyscallNo {
    /// alloc_pages(page_count) -> *mut u8 (or null on quota fail)
    AllocPages = 0x01,
    /// free_pages(ptr, page_count)
    FreePages = 0x02,
    /// emit_signal(target_vec, packet_lo, packet_hi) -> i32
    EmitSignal = 0x03,
    /// resolve_capability(ns_ptr, ns_len, name_ptr, name_len) -> u64
    ResolveCapability = 0x04,
}

/// Initialize the Ring 3 entry surface.  Call after the GDT has been
/// loaded and `k_gdt::init_gdt()` has run, so the user/kernel selectors
/// in `k_gdt::gdt_state().selectors` are populated.
///
/// # Safety
///
/// Must be called from kernel boot context, exactly once, after the
/// GDT is loaded.  Touches MSRs that affect the entire CPU.
pub unsafe fn init() {
    let selectors = &k_gdt::gdt_state().selectors;

    // IA32_STAR layout:
    //   bits 32..47  -> kernel CS, kernel SS = kernel CS + 8
    //   bits 48..63  -> user-32 CS, user data SS = user-32 CS + 8,
    //                   user-64 CS = user-32 CS + 16
    // The x86_64 crate's `Star::write` validates this contiguity:
    //   * user_data    == user_code - 8        (sysret layout)
    //   * kernel_data  == kernel_code + 8      (syscall layout)
    //
    // Phase E.2.1 added `kernel_data_selector` to k-gdt's GDT
    // (immediately after `code_selector`), so both invariants now hold.
    // The kernel still runs with SS=0 day-to-day — this descriptor only
    // matters as the Ring 0 SS that `syscall` switches to.
    if let Err(reason) = Star::write(
        selectors.user_code_selector,
        selectors.user_data_selector,
        selectors.code_selector,
        selectors.kernel_data_selector,
    ) {
        // Should be unreachable after E.2.1 — keep the diagnostic so a
        // future GDT-layout change that breaks the +8/-8 invariants
        // fails loud on serial instead of silently #GP-ing on the first
        // `syscall`.
        crate::raw_serial_println(format_args!(
            "ring3: skipping IA32_STAR program — {reason} (GDT layout violates Star::write invariants)"
        ));
        return;
    }

    // Handler RIP.
    LStar::write(VirtAddr::new(syscall_entry as *const () as u64));

    // Mask IF on entry — interrupts stay disabled while we save user
    // state and switch to the kernel stack.  Direction flag is also
    // masked so memcpy / rep movs behave deterministically.
    SFMask::write(RFlags::INTERRUPT_FLAG | RFlags::DIRECTION_FLAG);

    // Enable the `syscall` instruction.
    let mut efer = Efer::read();
    efer.insert(EferFlags::SYSTEM_CALL_EXTENSIONS);
    Efer::write(efer);

    crate::raw_serial_println(format_args!(
        "ring3: syscall surface armed (IA32_STAR/LSTAR/FMASK programmed, EFER.SCE=1)"
    ));
}

// ── Syscall trampoline ──────────────────────────────────────────────────
//
// Calling convention (Linux-compatible x86_64 syscall ABI):
//   - RAX = syscall number
//   - RDI, RSI, RDX, R10, R8, R9 = args 0..5  (note: R10, not RCX!)
//   - return value in RAX
//   - On entry CPU has put user RIP in RCX and user RFLAGS in R11 —
//     these MUST survive to `sysretq` or we cannot return to user.
//
// This is a stub implementation:
//   * We do NOT yet swap to a per-CPU kernel stack — single-CPU kernel
//     and no Ring 3 plugin runs today, so the handler executes on
//     whatever stack the user-mode caller had.  When E.4 (SMP) lands,
//     this swap becomes mandatory via swapgs + per-CPU GS base.
//   * We DO save/restore the user RIP/RFLAGS via R11/RCX so sysret
//     reaches the right place.

#[unsafe(naked)]
extern "C" fn syscall_entry() {
    naked_asm!(
        // Save caller-saved registers we may clobber.
        "push rcx",                    // user RIP
        "push r11",                    // user RFLAGS
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // syscall ABI leaves (RAX=nr, RDI=arg0, RSI=arg1, RDX=arg2,
        // R10=arg3, R8=arg4, R9=arg5) -- none of that lines up with the
        // C ABI's (RDI, RSI, RDX, RCX, R8, R9) argument registers a
        // `call` needs, so it has to be shuffled explicitly rather than
        // just moving R10 -> RCX (ADR-019 §五-3: this was a real,
        // never-exercised bug -- rust_syscall_handler's parameters used
        // to be *named* rax/rdi/rsi/rdx/rcx/r8 but the C ABI actually
        // delivered them shifted by one register, so "rax" silently
        // received RDI's value, "rdi" received RSI's, etc., and the
        // 7th declared parameter read uninitialized stack memory since
        // only 6 args fit in registers. RBP/RBX/R12-R15 are already
        // saved above, so they're free scratch here; their *original*
        // values still get restored correctly by the pops below since
        // those read from the stack, not these registers.
        "mov r15, r8",   // stash arg4
        "mov r14, r10",  // stash arg3
        "mov rcx, rdx",  // rcx (C arg3) <- arg2
        "mov rdx, rsi",  // rdx (C arg2) <- arg1
        "mov rsi, rdi",  // rsi (C arg1) <- arg0
        "mov rdi, rax",  // rdi (C arg0) <- syscall number
        "mov r8, r14",   // r8  (C arg4) <- arg3
        "mov r9, r15",   // r9  (C arg5) <- arg4
        "call rust_syscall_handler",
        // RAX = return value.
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "pop r11",
        "pop rcx",
        "sysretq",
    )
}

/// Decode the syscall number and dispatch.  Until the kernel has
/// plugins issuing syscalls, this never runs — but the table is wired
/// so the first real Ring 3 plugin lands on the right ABI fn.
///
/// Parameter names match what `syscall_entry`'s register shuffle
/// actually delivers (see its comment) — `syscall_no` is the raw
/// `syscall`-instruction RAX value, `arg0..arg4` are args 0-4 of the
/// Linux-compatible syscall ABI this module's doc comment describes.
/// Arg 5 is dropped: nothing implemented today uses a 6th argument, and
/// the C ABI has no 7th register to carry it in anyway.
///
/// SAFETY: `syscall_no` is user-supplied; out-of-range values return
/// `u64::MAX` (sentinel for "ENOSYS"). Pointer args are validated
/// lazily inside the corresponding ABI fn.
#[no_mangle]
pub extern "C" fn rust_syscall_handler(
    syscall_no: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
) -> u64 {
    match syscall_no {
        // ADR-019 §五-3: the boot-time ring3 probe's sentinel. Diverges
        // (longjmps straight back to the probe's saved kernel context)
        // instead of falling through to the normal sysretq epilogue --
        // see crate::ring3_probe's module doc comment.
        x if x == crate::ring3_probe::SENTINEL_SYSCALL => crate::ring3_probe::on_sentinel_syscall(),
        x if x == SyscallNo::AllocPages as u64 => {
            // alloc_pages goes through the same gos-runtime ABI shim
            // the kernel-side path uses today; quota accounting and
            // Phase E.3: delegate to gos-runtime's quota-aware allocator.
            // dispatching_instance() tracks the instance on the current
            // executor dispatch; when a Ring 3 plugin issues `syscall` the
            // instance context is preserved via the kernel stack.
            let _ = (arg1, arg2, arg3);
            unsafe { gos_runtime::syscall_alloc_pages(arg0 as usize) as u64 }
        }
        x if x == SyscallNo::FreePages as u64 => {
            // free_pages(ptr, page_count): arg0 = ptr, arg1 = page_count.
            let _ = (arg2, arg3);
            unsafe { gos_runtime::syscall_free_pages(arg0 as *mut u8, arg1 as usize) };
            0
        }
        x if x == SyscallNo::EmitSignal as u64 => {
            // emit_signal(target_vec, packet_lo, packet_hi): arg0, arg1, arg2.
            // packet_lo[63:56] = signal kind tag; packet_lo[55:0] = arg0.
            let _ = arg3;
            unsafe { gos_runtime::syscall_emit_signal(arg0, arg1, arg2) as u64 }
        }
        x if x == SyscallNo::ResolveCapability as u64 => {
            // resolve_capability(ns_ptr, ns_len, name_ptr, name_len).
            unsafe {
                gos_runtime::syscall_resolve_capability(
                    arg0 as *const u8,
                    arg1 as usize,
                    arg2 as *const u8,
                    arg3 as usize,
                )
            }
        }
        _ => u64::MAX, // ENOSYS
    }
}
