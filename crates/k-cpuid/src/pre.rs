// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: decode the signal kind. CPUID re-samples on every event.

use gos_protocol::{packet_to_signal, NodeEvent, Signal};

pub struct Input {
    pub signal_kind: u8,
}

pub fn signal_kind_code(signal: Signal) -> u8 {
    match signal {
        Signal::Call { .. }      => 0x01,
        Signal::Spawn { .. }     => 0x02,
        Signal::Interrupt { .. } => 0x03,
        Signal::Data { .. }      => 0x04,
        Signal::Control { .. }   => 0x05,
        Signal::Terminate        => 0xFF,
    }
}

pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let signal = unsafe { packet_to_signal((*event).signal) };
    Some(Input { signal_kind: signal_kind_code(signal) })
}
