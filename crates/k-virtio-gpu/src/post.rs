// ============================================================
// GOS KERNEL TOPOLOGY — k-virtio-gpu::post
//
// MERGE (m:Module {id: "K_VIRTIO_GPU_POST", name: "k-virtio-gpu::post"})
// SET m.role = "stage:post", m.responsibility = "emit probe report to the console"
// MERGE (p:Plugin {id: "K_VIRTIO_GPU"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================

use gos_protocol::{ExecStatus, ExecutorContext};

use super::{print_probe_report, proc, sink_from_ctx, state_mut};

/// Stage 3 — emit the result to the console sink.
pub unsafe fn emit(ctx: *mut ExecutorContext, output: proc::Output) -> ExecStatus {
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };

    match output {
        proc::Output::Report { title } => {
            print_probe_report(&sink, state, title);
        }
    }

    ExecStatus::Done
}
