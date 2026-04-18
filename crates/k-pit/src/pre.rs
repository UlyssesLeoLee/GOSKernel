// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: filter the incoming signal and identify timer IRQ events.
// PIT only reacts to Interrupt { irq: TIMER } signals.

use gos_protocol::Signal;

/// Decoded PIT input: the timer IRQ number to forward.
pub struct Input {
    pub irq: u8,
}

/// Returns `Some(Input)` only for timer IRQ signals.
pub fn prepare(signal: Signal) -> Option<Input> {
    if let Signal::Interrupt { irq } = signal {
        if irq == k_pic::InterruptIndex::Timer.as_u8() {
            return Some(Input { irq });
        }
    }
    None
}
