// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: decode the signal and record its kind for telemetry.
// The PIC only acts on Spawn signals (initialise the 8259 PIC chain).

use gos_protocol::{packet_to_signal, NodeEvent, Signal};

/// Telemetry + action descriptor for a PIC signal.
pub struct Input {
    pub signal_kind: u8,
    /// True when this signal requests PIC initialisation.
    pub do_init: bool,
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

/// Decode the event. Always returns `Some` so telemetry is always recorded.
pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let signal = unsafe { packet_to_signal((*event).signal) };
    let signal_kind = signal_kind_code(signal);
    let do_init = matches!(signal, Signal::Spawn { .. });
    Some(Input { signal_kind, do_init })
}
