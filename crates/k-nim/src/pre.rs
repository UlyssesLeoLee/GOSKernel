// ============================================================
// GOS KERNEL TOPOLOGY — k-nim::pre
//
// MERGE (m:Module {id: "K_NIM_PRE", name: "k-nim::pre"})
// SET m.role = "stage:pre", m.responsibility = "decode incoming signals into typed NIM inputs"
// MERGE (p:Plugin {id: "K_NIM"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================

use gos_protocol::{
    packet_to_signal, NodeEvent, Signal,
    NIM_CONTROL_SEND, NIM_CONTROL_EXIT,
    NIM_CONTROL_MODEL_BEGIN, NIM_CONTROL_MODEL_COMMIT,
    NIM_CONTROL_CLEAR_HISTORY,
    NIM_CONTROL_PORT_BEGIN, NIM_CONTROL_PORT_COMMIT,
};

/// What kind of work the proc stage should perform.
pub enum InputKind {
    /// Boot-time spawn: display the NIM banner.
    Spawn,
    /// A single byte of data (character input, or streaming model/port byte
    /// depending on `state.streaming_mode`).
    Byte(u8),
    /// Submit the buffered user message to the NIM endpoint.
    Send,
    /// Exit the NIM session and return to the shell.
    Exit,
    /// Begin streaming a model-name byte-by-byte (clears current model).
    ModelBegin,
    /// Commit the streamed model name.
    ModelCommit,
    /// Begin streaming port digits (clears current port buffer).
    PortBegin,
    /// Commit the streamed port digits and parse into `nim_port`.
    PortCommit,
    /// Clear the multi-turn conversation history.
    ClearHistory,
}

pub struct Input {
    pub kind: InputKind,
}

/// Stage 1 — decode a raw `NodeEvent` into a typed `Input`.
///
/// Accepted bytes: printable ASCII (≥ 0x20) and horizontal tab (0x09).
/// CR / LF are handled at the shell level (arrive as `NIM_CONTROL_SEND`).
pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let event  = unsafe { &*event };
    let signal = packet_to_signal(event.signal);

    match signal {
        Signal::Spawn { .. } => Some(Input { kind: InputKind::Spawn }),

        Signal::Data { byte, .. } => {
            if byte >= 0x20 || byte == b'\t' {
                Some(Input { kind: InputKind::Byte(byte) })
            } else {
                None
            }
        }

        Signal::Control { cmd: NIM_CONTROL_SEND,          .. } => Some(Input { kind: InputKind::Send }),
        Signal::Control { cmd: NIM_CONTROL_EXIT,          .. } => Some(Input { kind: InputKind::Exit }),
        Signal::Control { cmd: NIM_CONTROL_MODEL_BEGIN,   .. } => Some(Input { kind: InputKind::ModelBegin }),
        Signal::Control { cmd: NIM_CONTROL_MODEL_COMMIT,  .. } => Some(Input { kind: InputKind::ModelCommit }),
        Signal::Control { cmd: NIM_CONTROL_PORT_BEGIN,    .. } => Some(Input { kind: InputKind::PortBegin }),
        Signal::Control { cmd: NIM_CONTROL_PORT_COMMIT,   .. } => Some(Input { kind: InputKind::PortCommit }),
        Signal::Control { cmd: NIM_CONTROL_CLEAR_HISTORY, .. } => Some(Input { kind: InputKind::ClearHistory }),

        _ => None,
    }
}
