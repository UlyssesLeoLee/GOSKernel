// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: signal pipeline completion.
// VGA is a fire-and-forget output driver — no downstream routing required.

use gos_protocol::ExecStatus;

pub fn emit(_output: super::proc::Output) -> ExecStatus {
    ExecStatus::Done
}
