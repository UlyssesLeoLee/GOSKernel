// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: forward the timer interrupt signal to the Shell node.

use gos_protocol::{ExecStatus, Signal};

pub fn emit(output: super::proc::Output) -> ExecStatus {
    let _ = gos_hal::ngr::post_signal(
        k_shell::NODE_VEC,
        Signal::Interrupt { irq: output.irq },
    );
    ExecStatus::Done
}
