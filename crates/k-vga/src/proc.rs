// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: apply the decoded VGA operation to the display state.
// All mutations to VgaState and the hardware text buffer happen here.

use gos_protocol::ExecutorContext;

/// Confirmation that the VGA operation completed (unit value — VGA is fire-and-forget).
pub struct Output;

/// Apply the decoded operation to VgaState (and ultimately the hardware buffer).
pub unsafe fn process(ctx: *mut ExecutorContext, input: super::pre::Input) -> Option<Output> {
    let state = unsafe { super::state_mut(ctx) };
    match input {
        super::pre::Input::Data { byte }        => super::write_byte(state, byte),
        super::pre::Input::Control { cmd, val } => super::handle_control(state, cmd, val),
    }
    Some(Output)
}
