// ============================================================
// GOS KERNEL TOPOLOGY — k-cuda-host/pre
//
// MERGE (m:Module {id: "k_cuda_host::pre", name: "pre"})
// SET m.role = "decode", m.stage = 0
// MERGE (p:Plugin {id: "K_CUDA"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Structs
// MERGE (s:Struct {id: "k_cuda_host::pre::Input"})
// MERGE (m)-[:DEFINES]->(s)
//
// -- Functions
// MERGE (f:Function {id: "k_cuda_host::pre::prepare"})
// SET f.unsafe = true, f.returns = "Option<Input>"
// MERGE (m)-[:DEFINES]->(f)
// MERGE (f)-[:READS]->(s)
// ============================================================

use gos_protocol::{packet_to_signal, NodeEvent, Signal};
use super::{sink_from_ctx, ConsoleSink, ExecutorContext};

pub struct Input {
    pub sink: ConsoleSink,
    pub signal: Signal,
}

/// Decode the incoming event packet and resolve the execution context.
/// Returns `None` if the pointer is null (safety guard).
pub unsafe fn prepare(ctx: *mut ExecutorContext, event: *const NodeEvent) -> Option<Input> {
    if ctx.is_null() || event.is_null() {
        return None;
    }
    let sink = sink_from_ctx(ctx);
    let signal = packet_to_signal(unsafe { (*event).signal });
    Some(Input { sink, signal })
}
