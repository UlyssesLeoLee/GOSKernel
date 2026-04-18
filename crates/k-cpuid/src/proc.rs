// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: execute CPUID instruction and update the state snapshot.

use gos_protocol::ExecutorContext;

pub struct Output {
    pub signal_kind: u8,
}

/// Re-sample CPUID and return the signal kind for post-stage telemetry commit.
pub unsafe fn process(ctx: *mut ExecutorContext, input: super::pre::Input) -> Option<Output> {
    let state = unsafe { super::state_mut(ctx) };
    super::sample_cpuid(state);
    Some(Output { signal_kind: input.signal_kind })
}
