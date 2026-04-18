// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: decode the incoming signal and identify halt requests.
// Only Interrupt { irq: 0xFF } triggers a halt; all other signals are telemetry-only.

use gos_protocol::{packet_to_signal, NodeEvent, Signal};

pub struct Input {
    pub signal_kind: u8,
    /// True when the signal is the kernel halt sentinel (irq 0xFF).
    pub do_halt: bool,
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
    let do_halt = matches!(signal, Signal::Interrupt { irq } if irq == 0xFF);
    Some(Input { signal_kind: signal_kind_code(signal), do_halt })
}
