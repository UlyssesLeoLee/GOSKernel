// ============================================================
// GOS KERNEL TOPOLOGY — k-cuda-host/post
//
// MERGE (m:Module {id: "k_cuda_host::post", name: "post"})
// SET m.role = "emit", m.stage = 2
// MERGE (p:Plugin {id: "K_CUDA"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Functions
// MERGE (f:Function {id: "k_cuda_host::post::emit"})
// SET f.unsafe = true, f.returns = "ExecStatus"
// MERGE (m)-[:DEFINES]->(f)
// ============================================================

use gos_protocol::ExecStatus;
use super::ExecutorContext;
use crate::proc::Output;

/// Forward the pipeline output as the final `ExecStatus` to the kernel scheduler.
/// All downstream node emissions have already been performed inside `proc::process`;
/// this stage finalises the status and returns control to the executor.
pub unsafe fn emit(_ctx: *mut ExecutorContext, output: Output) -> ExecStatus {
    output.status
}
