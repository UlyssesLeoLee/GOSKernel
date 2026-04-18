// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: update runtime telemetry and signal pipeline completion.

use gos_protocol::{ExecStatus, ExecutorContext};

pub unsafe fn emit(ctx: *mut ExecutorContext, output: super::proc::Output) -> ExecStatus {
    let state = unsafe { super::runtime_state_mut(ctx) };
    state.last_signal_kind = output.signal_kind;
    if output.did_load {
        state.load_count = state.load_count.saturating_add(1);
    }
    ExecStatus::Done
}
