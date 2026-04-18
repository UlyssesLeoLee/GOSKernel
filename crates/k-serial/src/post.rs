// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: update telemetry state and signal pipeline completion.

use gos_protocol::{ExecStatus, ExecutorContext};

/// Update state counters and return Done.
pub unsafe fn emit(
    ctx: *mut ExecutorContext,
    output: super::proc::Output,
) -> ExecStatus {
    let state = unsafe { super::state_mut(ctx) };
    state.last_signal_kind = output.signal_kind;
    state.bytes_written = state.bytes_written.saturating_add(1);
    ExecStatus::Done
}
