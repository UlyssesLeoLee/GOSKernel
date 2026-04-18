// ── Pre-processing ────────────────────────────────────────────────────────────
// k-pmm does not process runtime events — frame allocation is via direct API calls.

use gos_protocol::NodeEvent;
pub struct Input;
#[allow(unused_variables)]
pub fn prepare(_event: *const NodeEvent) -> Option<Input> { None }
