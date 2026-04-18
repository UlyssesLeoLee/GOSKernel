// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: forward the timer IRQ signal to the Shell node.

use gos_protocol::{CellResult, Signal};

/// Post the timer interrupt signal to the Shell and signal completion.
pub fn emit(output: super::proc::Output) -> CellResult {
    gos_hal::ngr::post_signal(
        k_shell::NODE_VEC,
        Signal::Interrupt { irq: output.irq },
    );
    CellResult::Done
}
