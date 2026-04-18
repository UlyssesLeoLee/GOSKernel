// ── Pre-processing ────────────────────────────────────────────────────────────
// k-vmm does not process runtime events — setup is performed in on_init.

use gos_protocol::NodeEvent;
pub struct Input;
#[allow(unused_variables)]
pub fn prepare(_event: *const NodeEvent) -> Option<Input> { None }
