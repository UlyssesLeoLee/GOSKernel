// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: initialise the 8259 PIC chain when requested.

/// Result of PIC processing: telemetry update data.
pub struct Output {
    pub signal_kind: u8,
    /// Whether `init_pic()` was called this cycle.
    pub did_init: bool,
}

/// Initialise the PIC if `input.do_init` is set.
pub fn process(input: super::pre::Input) -> Option<Output> {
    let did_init = if input.do_init {
        super::init_pic();
        true
    } else {
        false
    };
    Some(Output { signal_kind: input.signal_kind, did_init })
}
