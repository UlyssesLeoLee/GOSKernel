// ============================================================
// GOS KERNEL TOPOLOGY — k-ai::pre
//
// MERGE (m:Module {id: "K_AI_PRE", name: "pre", file: "crates/k-ai/src/pre.rs"})
// MERGE (p:Plugin {id: "K_AI"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Structs defined here
// MERGE (s:Struct {id: "K_AI_PRE_INPUT", name: "Input"})
// MERGE (m)-[:DEFINES]->(s)
//
// -- Functions defined here
// MERGE (f:Function {id: "K_AI_PRE_PREPARE", name: "prepare"})
// MERGE (m)-[:DEFINES]->(f)
// ============================================================

use gos_protocol::{packet_to_signal, ExecutorContext, NodeEvent, Signal};
use super::{sink_from_ctx, state_mut, ConsoleSink, AiState};

pub struct Input {
    pub sink: ConsoleSink,
    pub state: &'static mut AiState,
    pub signal: Signal,
}

/// Decode the incoming event and extract domain data.
/// Returns `None` if the event cannot be processed.
pub unsafe fn prepare(ctx: *mut ExecutorContext, event: *const NodeEvent) -> Option<Input> {
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };
    let signal = packet_to_signal(unsafe { (*event).signal });
    Some(Input { sink, state, signal })
}
