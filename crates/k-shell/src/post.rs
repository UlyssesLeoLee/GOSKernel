// ============================================================
// k-shell :: post — output emission stage
//
// MERGE (post:Module {id: "k_shell::post", name: "post"})
// MERGE (lib:Module {id: "k_shell::lib", name: "lib"})
// MERGE (post)-[:STAGE_OF]->(lib)
// MERGE (post)-[:CONSUMES]->(:Struct {name: "Output"})
// MERGE (post)-[:PRODUCES]->(:Type {name: "ExecStatus"})
// ============================================================

use gos_protocol::ExecStatus;

/// Forward the `ExecStatus` produced by the shell processing stage to the
/// executor runtime.  All console/VGA output and routing have already been
/// performed inside `proc::process`; this stage is intentionally thin.
pub fn emit(output: super::proc::Output) -> ExecStatus {
    output.status
}
