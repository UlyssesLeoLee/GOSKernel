// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: decode the incoming signal into a VGA display operation.
// Data signals map to character writes; Control signals map to display commands.
// All other signal kinds are silently dropped.

use gos_protocol::{packet_to_signal, NodeEvent, Signal};

/// A decoded VGA display operation.
pub enum Input {
    /// Write a single character byte to the current cursor position.
    Data { byte: u8 },
    /// Execute a VGA control command with an optional value byte.
    Control { cmd: u8, val: u8 },
}

/// Decode the event signal into a VGA operation.
/// Returns `None` for Interrupt, Spawn, Call, and Terminate signals.
pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    match unsafe { packet_to_signal((*event).signal) } {
        Signal::Data { byte, .. }    => Some(Input::Data { byte }),
        Signal::Control { cmd, val } => Some(Input::Control { cmd, val }),
        _                            => None,
    }
}
