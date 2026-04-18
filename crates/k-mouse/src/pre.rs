// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: classify the incoming signal and read the PS/2 mouse byte.
// Three signal kinds are handled:
//   Spawn    → flush accumulated delta to the display (pointer sync).
//   Interrupt { MOUSE_IRQ } → accumulate a PS/2 packet byte.
//   All others are silently ignored.

use gos_protocol::{packet_to_signal, NodeEvent, Signal};
use x86_64::instructions::port::Port;

const PS2_DATA_PORT: u16 = 0x60;
const MOUSE_IRQ: u8 = k_pic::InterruptIndex::Mouse as u8;

/// The decoded pre-processing output.
pub enum Input {
    /// Flush accumulated pointer deltas to the display.
    FlushMotion,
    /// A raw PS/2 mouse byte received from the I/O port.
    PacketByte(u8),
}

pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let signal = unsafe { packet_to_signal((*event).signal) };
    match signal {
        Signal::Spawn { .. } => Some(Input::FlushMotion),
        Signal::Interrupt { irq } if irq == MOUSE_IRQ => {
            let mut port = Port::<u8>::new(PS2_DATA_PORT);
            let byte = unsafe { port.read() };
            Some(Input::PacketByte(byte))
        }
        _ => None,
    }
}
