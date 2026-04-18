// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: emit pointer state Control signals to the VGA display node.

use gos_protocol::{ExecStatus, ExecutorContext};

/// Emit the updated pointer position/visibility to the display sink.
pub fn emit(ctx: *mut ExecutorContext, output: super::proc::Output) -> ExecStatus {
    let sink = super::sink_from_ctx(ctx);
    sink.emit_control(gos_protocol::DISPLAY_CONTROL_POINTER_COL, output.col);
    sink.emit_control(gos_protocol::DISPLAY_CONTROL_POINTER_ROW, output.row);
    sink.emit_control(gos_protocol::DISPLAY_CONTROL_POINTER_VISIBLE, output.visible);
    ExecStatus::Done
}
