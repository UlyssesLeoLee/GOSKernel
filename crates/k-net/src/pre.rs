// ============================================================
// GOS KERNEL TOPOLOGY — k-net::pre
//
// MERGE (m:Module {id: "K_NET_PRE", name: "k-net::pre"})
// SET m.role = "stage:pre", m.responsibility = "decode and validate incoming NodeEvent"
// MERGE (p:Plugin {id: "K_NET"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (s:Struct {id: "K_NET_PRE_INPUT", name: "Input"})
// MERGE (m)-[:DEFINES]->(s)
// MERGE (fn_prepare:Function {id: "K_NET_PRE_PREPARE", name: "prepare"})
// MERGE (m)-[:DEFINES]->(fn_prepare)
// MERGE (fn_prepare)-[:PRODUCES]->(s)
// ============================================================

use gos_protocol::{ExecutorContext, NodeEvent, Signal, packet_to_signal};

/// Decoded and validated input extracted from the incoming `NodeEvent`.
/// Only produced for signal variants that this plugin handles.
pub struct Input {
    pub signal: Signal,
}

/// Stage 1 — Decode the raw `NodeEvent` signal and short-circuit on irrelevant variants.
///
/// Returns `Some(Input)` when the signal is `Spawn` or `Control` (the two variants
/// that `k-net` acts on).  Returns `None` for everything else so the pipeline stops
/// immediately without touching driver state.
pub unsafe fn prepare(
    _ctx: *mut ExecutorContext,
    event: *const NodeEvent,
) -> Option<Input> {
    let signal = packet_to_signal(unsafe { (*event).signal });
    match signal {
        Signal::Spawn { .. } | Signal::Control { .. } => Some(Input { signal }),
        _ => None,
    }
}
