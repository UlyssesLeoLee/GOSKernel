// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: load (or re-load) the Global Descriptor Table when requested.

pub struct Output {
    pub signal_kind: u8,
    pub did_load: bool,
}

pub fn process(input: super::pre::Input) -> Option<Output> {
    let did_load = if input.do_load {
        super::init_gdt();
        true
    } else {
        false
    };
    Some(Output { signal_kind: input.signal_kind, did_load })
}
