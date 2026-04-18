// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: commit telemetry state updates and signal pipeline completion.

use gos_protocol::{ExecStatus, ExecutorContext};

/// Update PIC runtime state counters and return Done.
pub unsafe fn emit(ctx: *mut ExecutorContext, output: super::proc::Output) -> ExecStatus {
    let state = unsafe { super::runtime_state_mut(ctx) };
    state.last_signal_kind = output.signal_kind;
    if output.did_init {
        state.init_count = state.init_count.saturating_add(1);
    }
    ExecStatus::Done
}
