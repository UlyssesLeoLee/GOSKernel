// ============================================================
// GOS Protocol — Hardware Trap & Graph Event Bridge
//
// This module defines the ABI between the raw x86-64 hardware
// interrupt/exception layer and the graph routing layer.
//
// Design invariants (x86-64 specific):
//   - TrapFrame is `#[repr(C)]` with the EXACT register layout
//     that the naked-function trampolines push onto the stack.
//     Never reorder fields — the CPU / assembler depends on this.
//   - HardwareEvent is always passed to the Interrupt Router Node
//     as a *capability-tagged reference*, not a copy. The 8-byte
//     `buf_token` field encodes {DomainId[32] | SlotIdx[32]}
//     and is validated by the supervisor before dispatch.
//   - All pointer fields in TrapFrame are `u64`, not raw pointers,
//     so the struct remains safely `Copy` and `no_std`-compatible.
//   - x86-64 canonical address tagging:
//     High 16 bits of a kernel pointer carry TrapClass(4b) |
//     DomainTag(4b) | Reserved(8b). This is consistent with the
//     GOS Tagged Pointer strategy described in the arch spec.
// ============================================================

/// x86-64 CPU exception / interrupt number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TrapVector {
    // CPU Exceptions (0–31, Intel defined)
    DivideError         = 0,
    Debug               = 1,
    Nmi                 = 2,
    Breakpoint          = 3,
    Overflow            = 4,
    BoundRange          = 5,
    InvalidOpcode       = 6,
    DeviceNotAvailable  = 7,
    DoubleFault         = 8,
    // 9 = CoprocessorSegOverrun (reserved, not used)
    InvalidTss          = 10,
    SegmentNotPresent   = 11,
    StackSegmentFault   = 12,
    GeneralProtection   = 13,
    PageFault           = 14,
    // 15 = reserved
    X87FloatingPoint    = 16,
    AlignmentCheck      = 17,
    MachineCheck        = 18,
    SimdException       = 19,
    VirtualizationExn   = 20,
    ControlProtection   = 21,
    // 22–31 = reserved

    // Hardware IRQ remapped by PIC/APIC (32–255)
    IrqTimer            = 32,  // PIT/HPET — IRQ0
    IrqKeyboard         = 33,  // PS/2 KBD — IRQ1
    IrqCascade          = 34,  // PIC cascade — IRQ2
    IrqCom2             = 35,  // COM2  — IRQ3
    IrqCom1             = 36,  // COM1  — IRQ4
    IrqSound            = 37,  // LPT2  — IRQ5
    IrqFloppy           = 38,  // Floppy— IRQ6
    IrqLpt1             = 39,  // LPT1  — IRQ7
    IrqCmos             = 40,  // RTC   — IRQ8
    IrqMouse            = 44,  // PS/2 mouse IRQ12
    IrqAta1             = 46,  // Primary ATA — IRQ14
    IrqAta2             = 47,  // Secondary ATA — IRQ15

    // GOS Syscall gate
    Syscall             = 0x80,

    // Catch-all for unmapped or future vectors
    Unknown             = 0xFF,
}

impl TrapVector {
    /// Classify a raw vector number from the IDT stub.
    #[inline]
    pub const fn from_raw(v: u8) -> Self {
        match v {
            0  => Self::DivideError,
            1  => Self::Debug,
            2  => Self::Nmi,
            3  => Self::Breakpoint,
            4  => Self::Overflow,
            5  => Self::BoundRange,
            6  => Self::InvalidOpcode,
            7  => Self::DeviceNotAvailable,
            8  => Self::DoubleFault,
            10 => Self::InvalidTss,
            11 => Self::SegmentNotPresent,
            12 => Self::StackSegmentFault,
            13 => Self::GeneralProtection,
            14 => Self::PageFault,
            16 => Self::X87FloatingPoint,
            17 => Self::AlignmentCheck,
            18 => Self::MachineCheck,
            19 => Self::SimdException,
            20 => Self::VirtualizationExn,
            21 => Self::ControlProtection,
            32 => Self::IrqTimer,
            33 => Self::IrqKeyboard,
            34 => Self::IrqCascade,
            35 => Self::IrqCom2,
            36 => Self::IrqCom1,
            37 => Self::IrqSound,
            38 => Self::IrqFloppy,
            39 => Self::IrqLpt1,
            40 => Self::IrqCmos,
            44 => Self::IrqMouse,
            46 => Self::IrqAta1,
            47 => Self::IrqAta2,
            0x80 => Self::Syscall,
            _ => Self::Unknown,
        }
    }

    /// True if this is a hardware IRQ that must send EOI to PIC/APIC.
    #[inline]
    pub const fn is_irq(self) -> bool {
        (self as u8) >= 32 && (self as u8) != 0x80 && (self as u8) != 0xFF
    }

    /// True for CPU exceptions that push an error code onto the stack.
    #[inline]
    pub const fn has_error_code(self) -> bool {
        matches!(
            self,
            Self::DoubleFault
                | Self::InvalidTss
                | Self::SegmentNotPresent
                | Self::StackSegmentFault
                | Self::GeneralProtection
                | Self::PageFault
                | Self::AlignmentCheck
                | Self::ControlProtection
        )
    }
}

// ============================================================
// x86-64 TrapFrame
//
// Layout mirrors what the kernel trampoline pushes.
// The assembler stub does (in order):
//   push rax; push rcx; push rdx; push rbx;
//   push rbp; push rsi; push rdi;
//   push r8 – r15;
//   [optional: push error_code]
//   push vector_number
//   call trap_normalizer
//
// TrapFrame is 256 bytes (32 × u64) intentionally aligned to
// a cache-line boundary so the whole frame hits a single L1
// set. Fields that the CPU pushes automatically (rip, cs,
// rflags, rsp, ss) come last as they appear above the frame
// pointer in the x86-64 hardware-defined iretq layout.
// ============================================================
#[repr(C, align(64))]   // 64-byte cache-line aligned
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    // ── Callee-saved (pushed by trampoline, not CPU) ──────
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9:  u64,
    pub r8:  u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,

    // ── Trap metadata ─────────────────────────────────────
    /// Raw IDT vector number.
    pub vector: u64,
    /// CPU error code, or 0 if the exception does not push one.
    pub error_code: u64,

    // ── CPU-pushed interrupt frame (iretq layout) ─────────
    /// RIP of the interrupted instruction.
    pub rip: u64,
    /// Code segment selector.
    pub cs:  u64,
    /// RFLAGS at time of interrupt.
    pub rflags: u64,
    /// RSP at time of interrupt (only valid on CPL change).
    pub rsp: u64,
    /// Stack segment selector.
    pub ss:  u64,

    // ── Reserved / future use (pad to 256 bytes) ──────────
    /// Auxiliary scratch space. `_pad[0]` holds CR2 for page faults.
    /// Public so external crates can construct a TrapFrame literal.
    pub _pad: [u64; 9],
}

// Static assertion: TrapFrame must be exactly 256 bytes.
// If this fails at compile time the trampoline offsets are wrong.
const _ASSERT_TRAPFRAME_SIZE: () = {
    assert!(
        core::mem::size_of::<TrapFrame>() == 256,
        "TrapFrame must be 256 bytes to match trampoline offsets"
    );
};

impl TrapFrame {
    /// Construct a zeroed TrapFrame with the given vector and error_code.
    /// GPR fields default to 0; the caller fills what it knows from
    /// the `InterruptStackFrame` (rip, cs, rflags, rsp, ss).
    #[inline]
    pub const fn zeroed(vector: u64, error_code: u64) -> Self {
        Self {
            r15: 0, r14: 0, r13: 0, r12: 0,
            r11: 0, r10: 0, r9:  0, r8:  0,
            rdi: 0, rsi: 0, rbp: 0, rbx: 0,
            rdx: 0, rcx: 0, rax: 0,
            vector, error_code,
            rip: 0, cs: 0, rflags: 0, rsp: 0, ss: 0,
            _pad: [0; 9],
        }
    }

    /// Classify the raw vector into a `TrapVector`.
    #[inline]
    pub fn trap_vector(&self) -> TrapVector {
        TrapVector::from_raw(self.vector as u8)
    }

    /// True when this exception is unrecoverable (DF, MCE, …).
    #[inline]
    pub fn is_fatal(&self) -> bool {
        matches!(
            self.trap_vector(),
            TrapVector::DoubleFault | TrapVector::MachineCheck | TrapVector::Nmi
        )
    }

    /// For page faults: the faulting virtual address (CR2 snapshot).
    /// Callers are responsible for reading CR2 and storing it here.
    /// We reuse `_pad[0]` for this purpose under convention.
    #[inline]
    pub fn page_fault_addr(&self) -> u64 {
        self._pad[0]
    }

    /// Store the CR2 snapshot for a page fault.
    #[inline]
    pub fn set_page_fault_addr(&mut self, cr2: u64) {
        self._pad[0] = cr2;
    }
}

// ============================================================
// HardwareEvent — the token that flows along the graph edge
//
// When the TrapFrame Normalizer wants to hand off control to
// the Interrupt Router Node, it stores the TrapFrame in a
// per-CPU emergency buffer (or in a supervisor-allocated slot)
// and sends this compact 32-byte token along the control edge.
//
// The token carries:
//   - The graph-level event class (TrapClass)
//   - The raw vector and error code (for fast dispatch)
//   - A capability token to the backing TrapFrame buffer
//     (domain-validated; read-only from the router node)
//   - A timestamp for latency observation
// ============================================================

/// High-level classification of a hardware trap event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TrapClass {
    /// Recoverable CPU exception (e.g., page fault, GP).
    CpuFault  = 0x01,
    /// Hardware IRQ (timer, keyboard, NIC, …).
    HwIrq     = 0x02,
    /// Syscall gate (INT 0x80 / SYSCALL instruction).
    Syscall   = 0x03,
    /// Non-maskable interrupt — requires immediate escalation.
    Nmi       = 0x04,
    /// Unrecoverable fault (double fault, machine check).
    Fatal     = 0x05,
}

/// Compact 32-byte graph event token produced by TrapFrame Normalizer.
///
/// This is the *only* thing sent along the `Interrupt -> Router` edge.
/// The actual register state lives in the backing buffer referenced by
/// `buf_token`. This design avoids copying 256-byte frames through the
/// signal queue.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct HardwareEvent {
    /// High-level trap class for fast dispatch in the Router Node.
    pub trap_class: TrapClass,
    /// Raw IDT vector (matches TrapFrame.vector).
    pub vector: u8,
    /// CPU error code (0 if none).
    pub error_code: u16,
    /// x86-64 CR2 snapshot for page faults (0 otherwise).
    pub fault_addr: u64,
    /// Supervisor-issued capability token for the backing TrapFrame buffer.
    /// Format: DomainId[32] | SlotIdx[32]
    /// The router node calls `supervisor::read_trap_frame(buf_token)` to
    /// access full register state under capability check.
    pub buf_token: u64,
    /// TSC snapshot at interrupt entry (nanosecond resolution with CPUID).
    pub tsc: u64,
    /// Reserved for future use (APIC ID, NUMA node, …).
    pub _reserved: u32,
}

// Static assertion: HardwareEvent layout check.
// TrapClass(u8) + vector(u8) + error_code(u16) = 4b
// + 4b padding (align to u64)
// + fault_addr(u64) = 8b
// + buf_token(u64) = 8b
// + tsc(u64) = 8b
// + _reserved(u32) = 4b
// + 4b padding (align to u64 boundary of struct)
// Total = 40 bytes
const _ASSERT_HW_EVENT_SIZE: () = {
    assert!(
        core::mem::size_of::<HardwareEvent>() == 40,
        "HardwareEvent size unexpected — check field layout"
    );
};

impl HardwareEvent {
    /// Construct from a normalised TrapFrame + supervisor-issued token.
    #[inline]
    pub fn from_trap(frame: &TrapFrame, buf_token: u64, tsc: u64) -> Self {
        let trap_v = frame.trap_vector();
        let trap_class = if frame.is_fatal() {
            if trap_v == TrapVector::Nmi { TrapClass::Nmi } else { TrapClass::Fatal }
        } else if trap_v == TrapVector::Syscall {
            TrapClass::Syscall
        } else if trap_v.is_irq() {
            TrapClass::HwIrq
        } else {
            TrapClass::CpuFault
        };
        Self {
            trap_class,
            vector: frame.vector as u8,
            error_code: frame.error_code as u16,
            fault_addr: frame.page_fault_addr(),
            buf_token,
            tsc,
            _reserved: 0,
        }
    }

    /// Convenience: is this event a timer IRQ (vector 32)?
    #[inline]
    pub fn is_timer(&self) -> bool {
        self.vector == 32
    }

    /// Convenience: is this event a keyboard IRQ (vector 33)?
    #[inline]
    pub fn is_keyboard(&self) -> bool {
        self.vector == 33
    }
}
