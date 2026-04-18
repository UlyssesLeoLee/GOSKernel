// ── Pre-processing ────────────────────────────────────────────────────────────
// k-idt: on_event is unused — interrupt dispatch is performed directly by the
// assembly trampolines via gos_trap_normalizer → gos_runtime::post_irq_signal.
// This stage exists to maintain the three-stage module layout.

use gos_protocol::NodeEvent;
pub struct Input;
#[allow(unused_variables)]
pub fn prepare(_event: *const NodeEvent) -> Option<Input> { None }
