// ============================================================
// GOS KERNEL TOPOLOGY — k-virtio-gpu::pre
//
// MERGE (m:Module {id: "K_VIRTIO_GPU_PRE", name: "k-virtio-gpu::pre"})
// SET m.role = "stage:pre", m.responsibility = "decode incoming signals into typed gpu inputs"
// MERGE (p:Plugin {id: "K_VIRTIO_GPU"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================

use gos_protocol::{packet_to_signal, NodeEvent, Signal, GPU_CONTROL_REPORT};

/// What kind of work the proc stage should perform.
pub enum InputKind {
    /// Boot-time spawn: run the PCI discovery + BAR-mapping state machine.
    Spawn,
    /// Report current discovery/BAR status (re-probes first if the device
    /// hasn't been probed yet).
    Report,
}

pub struct Input {
    pub kind: InputKind,
}

/// Stage 1 — decode a raw `NodeEvent` into a typed `Input`.
pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let event = unsafe { &*event };
    let signal = packet_to_signal(event.signal);

    match signal {
        Signal::Spawn { .. } => Some(Input { kind: InputKind::Spawn }),
        Signal::Control { cmd: GPU_CONTROL_REPORT, .. } => Some(Input { kind: InputKind::Report }),
        _ => None,
    }
}
