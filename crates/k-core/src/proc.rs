// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: perform the assembly-level context switch.

pub struct Output;

/// Call into the assembly `switch_context` routine.
pub unsafe fn process(input: super::pre::Input) -> Option<Output> {
    unsafe {
        super::switch_context(input.prev, input.next);
    }
    Some(Output)
}
