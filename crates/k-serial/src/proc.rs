// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: write the data byte to the UART.

use core::fmt::Write;

/// Confirmation that the byte was successfully written (carries the byte for
/// telemetry use in the post stage).
pub struct Output {
    /// Reserved for future telemetry envelopes; currently the post stage
    /// only forwards `signal_kind`.  Kept on the struct to avoid an ABI
    /// break the moment we wire telemetry through.
    #[allow(dead_code)]
    pub byte: u8,
    pub signal_kind: u8,
}

/// Write the byte from `input` to the serial port.
/// Runs inside `without_interrupts` to avoid re-entrant UART writes.
pub fn process(input: super::pre::Input) -> Option<Output> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let _ = crate::serial1().lock().write_char(input.byte as char);
    });
    Some(Output { byte: input.byte, signal_kind: input.signal_kind })
}
