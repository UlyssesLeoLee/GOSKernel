// ── Pre-processing ────────────────────────────────────────────────────────────
// k-heap does not process runtime events — all heap setup is done in on_init.
// This stage is a deliberate no-op that keeps the module layout consistent.

use gos_protocol::NodeEvent;

pub struct Input;

/// Always returns `None` — no runtime event handling required for the heap.
#[allow(unused_variables)]
pub fn prepare(_event: *const NodeEvent) -> Option<Input> {
    None
}
