// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: signal pipeline completion after a context switch.

use gos_protocol::ExecStatus;

pub fn emit(_output: super::proc::Output) -> ExecStatus {
    ExecStatus::Done
}
