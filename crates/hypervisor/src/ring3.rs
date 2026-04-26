//! Phase E.2 — syscall entry surface for Ring 3 plugins.
//!
//! This module wires the kernel side of the syscall path:
//!
//!   1. `IA32_EFER.SCE = 1`  — enable the `syscall` instruction.
//!   2. `IA32_STAR`          — encodes the kernel CS/SS and user CS/SS
//!                             selectors so `syscall`/`sysret` flip the
//!                             segment registers atomically.
//!   3. `IA32_LSTAR`         — kernel handler RIP for the `syscall`
//!                             instruction.
//!   4. `IA32_FMASK`         — RFLAGS bits to clear on `syscall` entry
//!                             (we mask IF so interrupts are disabled
//!                             until the handler is on its kernel
//!                             stack and ready).
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
    // The x86_64 crate's `Star::write` validates this contiguity.
    Star::write(
        selectors.user_code_selector,
        selectors.user_data_selector,
        selectors.code_selector,
        // KERNEL data selector — k_gdt today does not separately track
        // it; in the SYSV layout kernel SS is computed by the CPU as
        // kernel_cs + 8.  We pass the kernel CS as the kernel SS hint
        // and the x86_64 crate enforces the +8 invariant.
        selectors.code_selector,
    )
    .expect("E.2: invalid GDT selector layout for IA32_STAR");

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
        // syscall ABI passes the 4th arg in R10; C ABI expects RCX.
        "mov rcx, r10",
        // Rust handler reads (RAX, RDI, RSI, RDX, RCX, R8, R9).
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

/// Decode RAX and dispatch.  Until the kernel has plugins issuing
/// syscalls, this never runs — but the table is wired so the first
/// real Ring 3 plugin lands on the right ABI fn.
///
/// SAFETY: `rax` is the user-supplied syscall number; out-of-range
/// values return `u64::MAX` (sentinel for "ENOSYS").  Pointer args
/// are validated lazily inside the corresponding ABI fn.
#[no_mangle]
pub extern "C" fn rust_syscall_handler(
    rax: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    r8: u64,
    _r9: u64,
) -> u64 {
    match rax {
        x if x == SyscallNo::AllocPages as u64 => {
            // alloc_pages goes through the same gos-runtime ABI shim
            // the kernel-side path uses today; quota accounting and
            // backend forwarding both happen there.
            // The signature is `unsafe extern "C" fn(usize) -> *mut u8`.
            // We rely on KernelAbi being installed before any user
            // plugin runs — gos_runtime exposes the live ABI via
            // KERNEL_ABI; we call through the public alloc helper if
            // present, else return null.
            // For this stub, we route through gos-runtime's installed
            // backend by re-creating an ExecutorContext-less call.
            // Implementation lands when E.3 (per-instance privilege
            // gating) lands; until then, return null.
            let _ = (rdi, rsi, rdx, rcx, r8);
            0
        }
        x if x == SyscallNo::FreePages as u64 => 0,
        x if x == SyscallNo::EmitSignal as u64 => 0,
        x if x == SyscallNo::ResolveCapability as u64 => 0,
        _ => u64::MAX, // ENOSYS
    }
}
