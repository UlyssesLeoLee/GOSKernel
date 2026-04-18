// ── Post-processing ───────────────────────────────────────────────────────────
// k-pmm: no runtime event handling.

use gos_protocol::ExecStatus;
#[allow(dead_code)]
pub fn emit(_output: super::proc::Output) -> ExecStatus { ExecStatus::Done }
