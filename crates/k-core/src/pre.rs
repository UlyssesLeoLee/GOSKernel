// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: decode the incoming signal and extract context-switch pointers.
// Only Control { cmd: CORE_CONTROL_SWITCH_CONTEXT } signals carry valid pointers.

use gos_protocol::{packet_to_signal, NodeEvent, Signal, CORE_CONTROL_SWITCH_CONTEXT};
use super::TaskContext;

/// Context-switch request with prev/next task pointers.
pub struct Input {
    pub prev: *mut TaskContext,
    pub next: *const TaskContext,
}

/// Decode the event. Returns `None` for non-context-switch signals.
pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let signal = unsafe { packet_to_signal((*event).signal) };
    if let Signal::Control { cmd, .. } = signal {
        if cmd == CORE_CONTROL_SWITCH_CONTEXT {
            let packet = unsafe { (*event).signal };
            let prev = packet.arg1 as *mut TaskContext;
            let next = packet.arg2 as *const TaskContext;
            if !prev.is_null() && !next.is_null() {
                return Some(Input { prev, next });
            }
        }
    }
    None
}
