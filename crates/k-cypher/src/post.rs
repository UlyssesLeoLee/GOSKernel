// ============================================================
// GOS KERNEL TOPOLOGY — k-cypher/post
//
// MERGE (m:Module {id: "K_CYPHER_POST", name: "post", plugin: "K_CYPHER"})
// SET m.role = "Emit"
// MERGE (p:Plugin {id: "K_CYPHER"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Exports
// MERGE (fn_emit:Function {id: "K_CYPHER_POST_emit", name: "emit"})
// MERGE (m)-[:EXPORTS]->(fn_emit)
//
// -- Dependencies
// MERGE (m_proc:Module {id: "K_CYPHER_PROC"})
// MERGE (m)-[:USES]->(m_proc)
// ============================================================

use gos_protocol::{ExecutorContext, ExecStatus};

use super::proc::Output;

/// Forward the processed result to downstream nodes and return the final
/// `ExecStatus`.
///
/// For k-cypher all console output is already emitted inside `proc::process`
/// via the `ConsoleSink`; this stage simply surfaces the status.
pub unsafe fn emit(_ctx: *mut ExecutorContext, output: Output) -> ExecStatus {
    output.status
}
