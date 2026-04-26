// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: execute a kernel halt if requested.
// This function only returns when `do_halt` is false.

pub struct Output {
    pub signal_kind: u8,
    /// Reserved for future telemetry envelopes (would be `true` if we
    /// somehow returned from the halt loop).  Kept to preserve the
    /// shape consumers expect once telemetry is wired through.
    #[allow(dead_code)]
    pub did_halt: bool,
}

/// If `input.do_halt` is true, enter the infinite HLT loop (never returns).
/// Otherwise simply propagate telemetry data to the post stage.
/// Write a fixed message to the serial port (0x3F8) directly via I/O port.
/// Called before entering the halt loop so the operator sees the reason.
fn serial_write(msg: &[u8]) {
    let mut port = x86_64::instructions::port::Port::<u8>::new(0x3F8);
    for &byte in msg {
        unsafe { port.write(byte); }
    }
}

pub fn process(input: super::pre::Input) -> Option<Output> {
    if input.do_halt {
        serial_write(b"\n[K-PANIC] halt sentinel received -- kernel halted\n");
        loop {
            x86_64::instructions::hlt();
        }
    }
    Some(Output { signal_kind: input.signal_kind, did_halt: false })
}
