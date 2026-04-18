// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: validate the incoming NodeEvent, confirm it is a PS/2
// keyboard IRQ, and read the raw scancode from the I/O port.

use gos_protocol::{packet_to_signal, NodeEvent, Signal};
use x86_64::instructions::port::Port;

/// The decoded pre-processing output: a raw scancode byte ready for decoding.
pub struct Input {
    pub scancode: u8,
}

/// Validate the event is a keyboard IRQ and read the scancode.
/// Returns `None` for any non-keyboard signal, causing the pipeline to short-circuit.
pub unsafe fn prepare(event: *const NodeEvent) -> Option<Input> {
    let signal = packet_to_signal(unsafe { (*event).signal });
    let Signal::Interrupt { irq } = signal else {
        return None;
    };
    if irq != k_pic::InterruptIndex::Keyboard.as_u8() {
        return None;
    }
    let mut port = Port::new(0x60u16);
    let scancode: u8 = unsafe { port.read() };
    Some(Input { scancode })
}
