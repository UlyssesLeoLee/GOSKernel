// ============================================================
// GOS KERNEL TOPOLOGY — k-chat::pre
//
// MERGE (m:Module {id: "K_CHAT_PRE", name: "k-chat::pre"})
// SET m.role = "stage:pre", m.responsibility = "decode incoming signals into typed Chat inputs"
// MERGE (p:Plugin {id: "K_CHAT"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================

use gos_protocol::{
    packet_to_signal, NodeEvent, Signal,
    CHAT_CONTROL_SEND, CHAT_CONTROL_EXIT,
    CHAT_CONTROL_KEY_BEGIN, CHAT_CONTROL_KEY_COMMIT,
    CHAT_CONTROL_MODEL_BEGIN, CHAT_CONTROL_MODEL_COMMIT,
    CHAT_CONTROL_API_TYPE, CHAT_CONTROL_HTTP_TOGGLE,
};

/// What kind of work the proc stage should perform.
pub enum InputKind {
    /// Boot-time spawn: initialise COM2 and display welcome.
    Spawn,
    /// A single byte of data (character input or streaming key/model byte
    /// depending on `state.streaming_mode`).
    Byte(u8),
    /// Submit the buffered message to the AI bridge / direct HTTP.
    Send,
    /// Exit chat mode.
    Exit,
    // ── Streaming control ─────────────────────────────────────────────────────
    /// Begin streaming API key bytes (clears current key).
    KeyBegin,
    /// Commit the streamed API key.
    KeyCommit,
    /// Begin streaming model-name bytes (clears current model).
    ModelBegin,
    /// Commit the streamed model name.
    ModelCommit,
    // ── Configuration ─────────────────────────────────────────────────────────
    /// Set the API backend.  `val`: 0=ollama, 1=openai, 2=anthropic.
    ApiType(u8),
    /// Set HTTP transport mode.  `val`: 0=COM2 bridge, 1=direct TCP.
    HttpMode(u8),
}

pub struct Input {
    pub kind: InputKind,
}

/// Stage 1 — decode a raw `NodeEvent` into a typed `Input`.
pub fn prepare(event: *const NodeEvent) -> Option<Input> {
    let event  = unsafe { &*event };
    let signal = packet_to_signal(event.signal);

    match signal {
        Signal::Spawn { .. } => Some(Input { kind: InputKind::Spawn }),

        Signal::Data { byte, .. } => {
            // Accept printable ASCII (≥0x20), tab, and NUL (used internally).
            // CR/LF → Enter is handled at the shell level and arrives as
            // CHAT_CONTROL_SEND, so we drop bare 0x0D / 0x0A here.
            if byte >= 0x20 || byte == b'\t' || byte == 0x00 {
                Some(Input { kind: InputKind::Byte(byte) })
            } else {
                None
            }
        }

        Signal::Control { cmd: CHAT_CONTROL_SEND,         .. } => Some(Input { kind: InputKind::Send }),
        Signal::Control { cmd: CHAT_CONTROL_EXIT,         .. } => Some(Input { kind: InputKind::Exit }),
        Signal::Control { cmd: CHAT_CONTROL_KEY_BEGIN,    .. } => Some(Input { kind: InputKind::KeyBegin }),
        Signal::Control { cmd: CHAT_CONTROL_KEY_COMMIT,   .. } => Some(Input { kind: InputKind::KeyCommit }),
        Signal::Control { cmd: CHAT_CONTROL_MODEL_BEGIN,  .. } => Some(Input { kind: InputKind::ModelBegin }),
        Signal::Control { cmd: CHAT_CONTROL_MODEL_COMMIT, .. } => Some(Input { kind: InputKind::ModelCommit }),

        Signal::Control { cmd: CHAT_CONTROL_API_TYPE,    val } => Some(Input { kind: InputKind::ApiType(val) }),
        Signal::Control { cmd: CHAT_CONTROL_HTTP_TOGGLE, val } => Some(Input { kind: InputKind::HttpMode(val) }),

        _ => None,
    }
}
