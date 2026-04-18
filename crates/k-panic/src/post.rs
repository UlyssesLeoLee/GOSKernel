// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: commit telemetry state and signal completion.

use gos_protocol::{ExecStatus, ExecutorContext};

pub unsafe fn emit(ctx: *mut ExecutorContext, output: super::proc::Output) -> ExecStatus {
    let state = unsafe { super::state_mut(ctx) };
    state.last_signal_kind = output.signal_kind;
    // Note: if do_halt was true, proc::process never returns so this path
    // is only reached for non-halt signals.
    ExecStatus::Done
}
