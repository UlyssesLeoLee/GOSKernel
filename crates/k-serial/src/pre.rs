// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: decode the incoming signal and extract the byte to write.
// Only Data signals carry output bytes; all other signal kinds are silently ignored.

use gos_protocol::{packet_to_signal, NodeEvent, Signal};

/// The decoded pre-processing output: a single byte ready for serial output,
/// together with the signal-kind code for telemetry.
pub struct Input {
    /// The data byte to write.
    pub byte: u8,
    /// Signal-kind code for telemetry (recorded in state.last_signal_kind).
    pub signal_kind: u8,
}

/// Map a signal to its telemetry kind code.
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

/// Decode the event signal.
/// Returns `None` for non-Data signals (pipeline short-circuits to `Done`).
pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let signal = unsafe { packet_to_signal((*event).signal) };
    let signal_kind = signal_kind_code(signal);
    if let Signal::Data { byte, .. } = signal {
        Some(Input { byte, signal_kind })
    } else {
        None
    }
}
