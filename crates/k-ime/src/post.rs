// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: forward the processed bytes to the Shell node.

use gos_protocol::{ExecStatus, ExecutorContext};

/// Emit all bytes in `output.buf[0..output.len]` to Shell.
pub unsafe fn emit(ctx: *mut ExecutorContext, output: super::proc::Output) -> ExecStatus {
    if output.len == 0 {
        return ExecStatus::Done;
    }
    let state = unsafe { super::state_mut(ctx) };
    let target = state.shell_target;
    for &b in &output.buf[..output.len] {
        super::post_shell_byte(target, b);
    }
    ExecStatus::Done
}
