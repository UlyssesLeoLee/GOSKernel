// ── Post-processing ───────────────────────────────────────────────────────────
// k-vmm: no runtime event handling.

use gos_protocol::ExecStatus;
#[allow(dead_code)]
pub fn emit(_output: super::proc::Output) -> ExecStatus { ExecStatus::Done }
