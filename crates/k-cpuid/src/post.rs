// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: commit telemetry and signal completion.

use gos_protocol::{ExecStatus, ExecutorContext};

pub unsafe fn emit(ctx: *mut ExecutorContext, output: super::proc::Output) -> ExecStatus {
    let state = unsafe { super::state_mut(ctx) };
    state.last_signal_kind = output.signal_kind;
    ExecStatus::Done
}
