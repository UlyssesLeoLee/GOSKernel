// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility:
//   1. forward the timer interrupt signal to the Shell node
//   2. Phase E.1 — pulse the supervisor scheduler so it can decrement the
//      currently-dispatching instance's time-slice budget and flip the
//      preempt flag when the slice runs out.

use gos_protocol::{ExecStatus, Signal};

pub fn emit(output: super::proc::Output) -> ExecStatus {
    let _ = gos_hal::ngr::post_signal(
        k_shell::NODE_VEC,
        Signal::Interrupt { irq: output.irq },
    );
    gos_runtime::tick_pulse();
    ExecStatus::Done
}
