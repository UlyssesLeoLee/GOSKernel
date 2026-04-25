// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: increment the tick counter and pass the IRQ forward.

use core::sync::atomic::Ordering;

pub struct Output {
    pub irq: u8,
}

pub fn process(input: super::pre::Input) -> Option<Output> {
    super::ticks().fetch_add(1, Ordering::Relaxed);
    Some(Output { irq: input.irq })
}
