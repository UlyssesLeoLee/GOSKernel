// ============================================================
// GOS KERNEL TOPOLOGY — k-virtio-gpu::proc
//
// MERGE (m:Module {id: "K_VIRTIO_GPU_PROC", name: "k-virtio-gpu::proc"})
// SET m.role = "stage:proc", m.responsibility = "PCI discovery + BAR mapping state machine"
// MERGE (p:Plugin {id: "K_VIRTIO_GPU"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================

use gos_protocol::ExecutorContext;

use super::{pre, refresh_gpu_state, state_mut};

/// The output produced by the proc stage.
pub enum Output {
    /// A probe/status report — post will print a summary.
    Report { title: &'static str },
}

/// Stage 2 — run (or re-use) the discovery state machine.
pub unsafe fn process(ctx: *mut ExecutorContext, input: pre::Input) -> Option<Output> {
    let state = unsafe { state_mut(ctx) };

    match input.kind {
        pre::InputKind::Spawn => {
            refresh_gpu_state(state);
            Some(Output::Report { title: "virtio-gpu boot discovery" })
        }
        pre::InputKind::Report => {
            if state.probe_complete == 0 {
                refresh_gpu_state(state);
            }
            Some(Output::Report { title: "virtio-gpu status" })
        }
    }
}
