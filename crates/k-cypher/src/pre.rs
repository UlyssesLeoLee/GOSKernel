// ============================================================
// GOS KERNEL TOPOLOGY — k-cypher/pre
//
// MERGE (m:Module {id: "K_CYPHER_PRE", name: "pre", plugin: "K_CYPHER"})
// SET m.role = "Decode"
// MERGE (p:Plugin {id: "K_CYPHER"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Exports
// MERGE (fn_prepare:Function {id: "K_CYPHER_PRE_prepare", name: "prepare"})
// MERGE (m)-[:EXPORTS]->(fn_prepare)
// MERGE (struct_input:Struct {id: "K_CYPHER_PRE_Input", name: "Input"})
// MERGE (m)-[:EXPORTS]->(struct_input)
// ============================================================

use gos_protocol::{NodeEvent, ExecutorContext, Signal, packet_to_signal};

use super::ConsoleSink;
use super::sink_from_ctx;

/// All data extracted from the raw event that the pipeline needs to act on.
pub struct Input {
    pub sink: ConsoleSink,
    pub signal: Signal,
}

/// Decode and validate the incoming event.
///
/// Returns `None` if the signal cannot be decoded or is not actionable,
/// causing `on_event` to return `ExecStatus::Done` immediately.
pub unsafe fn prepare(ctx: *mut ExecutorContext, event: *const NodeEvent) -> Option<Input> {
    let sink = sink_from_ctx(ctx);
    let signal = packet_to_signal(unsafe { (*event).signal });
    Some(Input { sink, signal })
}
