// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: decode the incoming NodeEvent and filter for the timer IRQ.

use gos_protocol::{packet_to_signal, NodeEvent, Signal};

pub struct Input {
    pub irq: u8,
}

pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let signal = unsafe { packet_to_signal((*event).signal) };
    if let Signal::Interrupt { irq } = signal {
        if irq == k_pic::InterruptIndex::Timer.as_u8() {
            return Some(Input { irq });
        }
    }
    None
}
