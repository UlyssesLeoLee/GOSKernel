// ── Main processing ───────────────────────────────────────────────────────────
// k-pmm: no runtime event handling (PMM_CONTROL_QUERY_STATS reserved for future).

pub struct Output;
#[allow(dead_code)]
pub fn process(_input: super::pre::Input) -> Option<Output> { None }
