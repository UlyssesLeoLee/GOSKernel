// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: execute a kernel halt if requested.
// This function only returns when `do_halt` is false.

pub struct Output {
    pub signal_kind: u8,
    pub did_halt: bool,
}

/// If `input.do_halt` is true, enter the infinite HLT loop (never returns).
/// Otherwise simply propagate telemetry data to the post stage.
pub fn process(input: super::pre::Input) -> Option<Output> {
    if input.do_halt {
        // Increment counter in caller before we spin; post will not run.
        // (Accounted for in post::emit when do_halt is reflected via did_halt.)
        loop {
            x86_64::instructions::hlt();
        }
    }
    Some(Output { signal_kind: input.signal_kind, did_halt: false })
}
