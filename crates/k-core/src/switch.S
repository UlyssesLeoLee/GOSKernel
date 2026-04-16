.global switch_context

// extern "C" fn switch_context(prev: *mut TaskContext, next: *const TaskContext);
//
// Arguments passed via SysV AMD64 ABI:
// rdi = prev (*mut TaskContext)
// rsi = next (*const TaskContext)

switch_context:
    // ----------------------------------------------------
    // 1. Save current context (prev)
    // ----------------------------------------------------

    // Save callee-saved registers
    mov [rdi + 0x00], r15
    mov [rdi + 0x08], r14
    mov [rdi + 0x10], r13
    mov [rdi + 0x18], r12
    mov [rdi + 0x20], rbp
    mov [rdi + 0x28], rbx

    // Save RSP
    // Note: The 'call' instruction pushed RIP onto the stack.
    // RSP currently points to RIP. So saving RSP now effectively saves
    // the stack state including the return address.
    mov [rdi + 0x30], rsp

    // Save current CR3
    mov rax, cr3
    mov [rdi + 0x38], rax

    // ----------------------------------------------------
    // 2. Restore new context (next)
    // ----------------------------------------------------

    // Read new CR3
    mov r8, [rsi + 0x38]
    
    // Read current CR3
    mov rax, cr3

    // Compare and only load if different (avoid unnecessary TLB flush)
    cmp rax, r8
    je .skip_cr3_load
    mov cr3, r8
.skip_cr3_load:

    // Restore RSP
    mov rsp, [rsi + 0x30]

    // Restore callee-saved registers
    mov r15, [rsi + 0x00]
    mov r14, [rsi + 0x08]
    mov r13, [rsi + 0x10]
    mov r12, [rsi + 0x18]
    mov rbp, [rsi + 0x20]
    mov rbx, [rsi + 0x28]

    // ----------------------------------------------------
    // 3. Return (execute next task)
    // ----------------------------------------------------

    // 'ret' pops the return address off the newly restored stack
    // (which is the next task's saved RIP) and jumps to it.
    ret
