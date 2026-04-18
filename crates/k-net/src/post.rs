// ============================================================
// GOS KERNEL TOPOLOGY — k-net::post
//
// MERGE (m:Module {id: "K_NET_POST", name: "k-net::post"})
// SET m.role = "stage:post", m.responsibility = "emit probe report downstream and finalise execution"
// MERGE (p:Plugin {id: "K_NET"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (fn_emit:Function {id: "K_NET_POST_EMIT", name: "emit"})
// MERGE (m)-[:DEFINES]->(fn_emit)
// MERGE (fn_emit)-[:CONSUMES]->(output_s:Struct {id: "K_NET_PROC_OUTPUT"})
// MERGE (fn_emit)-[:PRODUCES]->(status:Type {id: "EXEC_STATUS"})
// ============================================================

use gos_protocol::{ExecStatus, ExecutorContext};

use super::{print_probe_report, proc, sink_from_ctx, state_mut};

/// Stage 3 — Emit the network probe report to the console sink and signal
/// completion to the executor runtime.
pub unsafe fn emit(ctx: *mut ExecutorContext, output: proc::Output) -> ExecStatus {
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };
    print_probe_report(&sink, state, output.title);
    ExecStatus::Done
}
