// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility:
//   1. forward the timer interrupt signal to the Shell node (throttled — see
//      SHELL_HEARTBEAT_DIVISOR)
//   2. Phase E.1 — pulse the supervisor scheduler so it can decrement the
//      currently-dispatching instance's time-slice budget and flip the
//      preempt flag when the slice runs out.

use core::sync::atomic::Ordering;
use gos_protocol::{ExecStatus, Signal};

/// Forward only 1-in-N PIT ticks to the shell. Measured in-VM, each
/// `Heartbeat` redraws ~6 VGA text panels (header/sigil/AI/operator band +
/// periodic command-deck/footer) plus a `gos_runtime::snapshot()` — roughly
/// 20ms of MMIO-bound work. At the raw 120Hz PIT rate that is >2s of demand
/// per second of wall time: an impossible budget that built an ever-growing
/// signal backlog and pinned `service_system_cycle` for hundreds of ms per
/// render-loop pass (diagnostics showed k-shell as the dominant per-pump
/// cost — 200-600ms — while the supervisor reported 0 newly dispatched
/// work, i.e. it was entirely back-pressure from this forward).
/// 1-in-16 -> 7.5Hz: still a smooth pulse cadence for a decorative text
/// animation, and frees up enough budget for the frame loop to run at a
/// healthy rate (measured: ~2.6 FPS -> ~18 FPS in-VM with WHPX).
const SHELL_HEARTBEAT_DIVISOR: usize = 16;

pub fn emit(output: super::proc::Output) -> ExecStatus {
    if super::ticks().load(Ordering::Relaxed) % SHELL_HEARTBEAT_DIVISOR == 0 {
        let _ = gos_hal::ngr::post_signal(
            k_shell::NODE_VEC,
            Signal::Interrupt { irq: output.irq },
        );
    }
    gos_runtime::tick_pulse();
    ExecStatus::Done
}
