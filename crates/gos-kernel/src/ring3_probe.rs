//! ADR-019 §五-3 — one-shot proof that the ring3 syscall trampoline
//! (`crate::ring3`) actually catches a real `syscall` instruction issued
//! from CPL3. No ELF-loaded plugin exists yet (Phase G.1.x), so this
//! maps a few bytes of inline shellcode into a user-accessible page
//! instead of loading a real program — exactly what ADR-019 itself asks
//! for: "不必是真正的用户进程，先证明 trampoline 本身接得住".
//!
//! Mechanism: `enter_ring3` stashes the current (kernel) callee-saved
//! registers + RSP into [`KERNEL_RESUME_RSP`] (a manual one-shot
//! setjmp), builds an `iretq` frame targeting a tiny code page
//! containing `syscall; jmp $`, and drops to CPL3. The `syscall`
//! reaches `ring3::rust_syscall_handler` exactly like a real plugin's
//! would; the handler recognizes the sentinel syscall number, records
//! that it fired, and — instead of the normal `sysretq`-back-to-ring3
//! epilogue — longjmps straight back to the saved kernel context via
//! [`return_to_kernel`]. The `jmp $` in the shellcode is a safety net
//! that never executes on the success path (nothing sends control back
//! to ring3 after `syscall`), only reachable if the longjmp itself were
//! somehow skipped.
//!
//! Safety envelope: the constructed ring3 `RFLAGS` has `IF` forced
//! clear, so this probe does not depend on hardware interrupts being
//! safely deliverable from CPL3 — which matters because `k_gdt` only
//! populates the 4 IST-indexed stack slots (`#DF`/`#PF`/`#GP`/`#SS`);
//! `TSS.privilege_stack_table[0]` (RSP0, used by any *other* CPL3→CPL0
//! transition, e.g. a PIT tick) is never set. That's a real gap this
//! probe surfaced and deliberately does not fix — it matters once actual
//! ring3 plugins run with interrupts enabled (Phase E.4/SMP territory
//! per `ring3.rs`'s own doc comment), not for this synchronous,
//! interrupts-off excursion. This module also runs before
//! `x86_64::instructions::interrupts::enable()` in `kernel_main`, so the
//! CPU-global IF is already 0 going in — forcing it off in the
//! constructed RFLAGS is what actually matters, since `iretq` loads
//! RFLAGS from the frame regardless of the ambient kernel IF state.

use core::arch::naked_asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use x86_64::registers::control::Cr3;
use x86_64::structures::paging::PageTableFlags;

const PROBE_CODE_VADDR: u64 = 0x0000_6000_0000_0000;
const PROBE_STACK_VADDR: u64 = 0x0000_6000_0000_1000;
const PROBE_STACK_TOP: u64 = PROBE_STACK_VADDR + 0x1000;

/// Syscall number the probe's shellcode issues. Deliberately outside
/// `ring3::SyscallNo`'s real range (0x01..0x04) so it can never collide
/// with a real syscall a future plugin might issue.
pub const SENTINEL_SYSCALL: u64 = 0xFACE;

static FIRED: AtomicBool = AtomicBool::new(false);

/// One-shot save slot for the kernel-side register/stack state
/// `enter_ring3` is resumed into by `return_to_kernel`. Not reentrant —
/// this whole module is a single synchronous boot-time probe, never
/// called concurrently — the asm accesses this atomic's backing storage
/// directly via `sym` + raw `mov`, not through the atomic API; `Atomic`
/// here is about satisfying this repo's no-`static mut` rule
/// (`tools/verify-graph-architecture.ps1`), not about real concurrent
/// access.
static KERNEL_RESUME_RSP: AtomicU64 = AtomicU64::new(0);

/// Called from `ring3::rust_syscall_handler` when it sees
/// [`SENTINEL_SYSCALL`]. Records that the trampoline genuinely caught a
/// CPL3-issued `syscall`, then longjmps back into `run()` — this never
/// returns to its caller.
pub fn on_sentinel_syscall() -> ! {
    FIRED.store(true, Ordering::SeqCst);
    unsafe { return_to_kernel() }
}

/// Map the probe's code/stack pages into the *current* (kernel's own)
/// address space, drop to CPL3, execute `syscall`, and confirm the
/// trampoline observed it. Returns `true` iff the sentinel syscall
/// fired. Safe to call at most once per boot before pages are reused —
/// intentionally not idempotent-safe (this is a boot-time self-test).
///
/// # Safety
/// Must be called after `k_gdt::boot_init_gdt()` (needs valid
/// `user_code_selector`/`user_data_selector`) and after
/// `ring3::init()` (needs the syscall MSRs armed).
pub unsafe fn run() -> bool {
    let (frame, _) = Cr3::read();
    let root_phys = frame.start_address().as_u64();

    // Code page: PRESENT | WRITABLE | USER_ACCESSIBLE. No NO_EXECUTE --
    // executable by default (that flag is opt-in, not opt-out). Left
    // writable too so the shellcode bytes below can be written directly
    // through this same mapping; this is a throwaway scratch page, not
    // a real plugin image, so W^X doesn't apply.
    k_vmm::map_anonymous_window(
        root_phys,
        PROBE_CODE_VADDR,
        0x1000,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    )
    .expect("map ring3 probe code page");
    k_vmm::map_anonymous_window(
        root_phys,
        PROBE_STACK_VADDR,
        0x1000,
        PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE
            | PageTableFlags::NO_EXECUTE,
    )
    .expect("map ring3 probe stack page");

    // Shellcode: `mov eax, SENTINEL_SYSCALL` (B8 + imm32, zero-extends
    // to RAX) then `syscall` (0F 05) then `jmp $` (EB FE) as a safety
    // net -- see module doc comment for why the jmp should never
    // execute. SENTINEL_SYSCALL must fit in 32 bits for this encoding.
    const _: () = assert!(SENTINEL_SYSCALL <= u32::MAX as u64);
    let code = PROBE_CODE_VADDR as *mut u8;
    core::ptr::write(code, 0xB8);
    core::ptr::write_unaligned(code.add(1) as *mut u32, SENTINEL_SYSCALL as u32);
    core::ptr::write(code.add(5), 0x0F);
    core::ptr::write(code.add(6), 0x05);
    core::ptr::write(code.add(7), 0xEB);
    core::ptr::write(code.add(8), 0xFE);

    let selectors = &k_gdt::gdt_state().selectors;
    enter_ring3(
        PROBE_CODE_VADDR,
        PROBE_STACK_TOP,
        selectors.user_code_selector.0 as u64,
        selectors.user_data_selector.0 as u64,
    );

    FIRED.load(Ordering::SeqCst)
}

/// Save kernel context, drop to CPL3 at `user_rip` with stack
/// `user_rsp`, using `user_cs`/`user_ss` as the ring3 segment
/// selectors (already RPL=3 — `GlobalDescriptorTable::add_entry`
/// encodes the descriptor's DPL into the returned `SegmentSelector`).
/// Returns (via `return_to_kernel`'s `ret`, not by falling off the end)
/// once the sentinel syscall has been observed.
///
/// # Safety
/// `user_rip`/`user_rsp` must point into pages mapped
/// `PRESENT | USER_ACCESSIBLE` (executable / writable respectively) in
/// the currently active address space. `user_cs`/`user_ss` must be
/// valid ring-3 GDT selectors.
#[unsafe(naked)]
unsafe extern "C" fn enter_ring3(user_rip: u64, user_rsp: u64, user_cs: u64, user_ss: u64) {
    naked_asm!(
        // Save the callee-saved registers the C ABI requires us to
        // preserve, then remember RSP -- this is the "setjmp" half of
        // the manual save/restore `return_to_kernel` longjmps back to.
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "lea rax, [rip + {resume}]",
        "mov [rax], rsp",
        // Reload DS/ES/FS/GS to the ring3 data selector (rcx = user_ss,
        // the 4th arg). Loading a selector with RPL=3 while CPL is
        // still 0 (iretq hasn't run yet) is permitted as long as the
        // descriptor's DPL >= 3, which user_data_segment()'s descriptor
        // is by construction.
        "mov ax, cx",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        // rdi/rsi/rdx/rcx still hold the original args (mov ax, cx only
        // read rcx, didn't clobber it). Build the iretq frame: SS, RSP,
        // RFLAGS, CS, RIP.
        "push rcx",              // SS = user_ss
        "push rsi",              // RSP = user_rsp
        "pushfq",
        "pop rax",
        "and rax, 0xfffffffffffffdff", // clear IF (bit 9) -- see module doc
        "or rax, 0x2",                  // bit 1 is always reserved-set
        "push rax",              // RFLAGS
        "push rdx",              // CS = user_cs
        "push rdi",              // RIP = user_rip
        "iretq",
        resume = sym KERNEL_RESUME_RSP,
    )
}

/// Longjmp counterpart to `enter_ring3` — restores the kernel RSP and
/// callee-saved registers `enter_ring3` stashed, then `ret`s to
/// `enter_ring3`'s own caller (i.e. `run()` resumes right after the
/// `enter_ring3(...)` call). Called from `ring3::rust_syscall_handler`
/// while still on the *ring3* stack (the `syscall_entry` trampoline's
/// pushes landed there, per its own doc comment) — this abandons that
/// stack entirely rather than unwinding it, which is fine: it's a
/// throwaway scratch page, not reused afterward.
///
/// # Safety
/// Must only be called after a matching `enter_ring3` has run on this
/// boot (i.e. `KERNEL_RESUME_RSP` holds a live saved context).
#[unsafe(naked)]
unsafe extern "C" fn return_to_kernel() -> ! {
    naked_asm!(
        "lea rax, [rip + {resume}]",
        "mov rsp, [rax]",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "ret",
        resume = sym KERNEL_RESUME_RSP,
    )
}
