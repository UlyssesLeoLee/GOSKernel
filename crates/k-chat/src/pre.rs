// ============================================================
// GOS KERNEL TOPOLOGY — k-chat::pre
//
// MERGE (m:Module {id: "K_CHAT_PRE", name: "k-chat::pre"})
// SET m.role = "stage:pre", m.responsibility = "decode incoming signals into typed Chat inputs"
// MERGE (p:Plugin {id: "K_CHAT"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (s:Struct {id: "K_CHAT_PRE_INPUT", name: "Input"})
// MERGE (m)-[:DEFINES]->(s)
// MERGE (fn_prepare:Function {id: "K_CHAT_PRE_PREPARE", name: "prepare"})
// MERGE (m)-[:DEFINES]->(fn_prepare)
// MERGE (fn_prepare)-[:PRODUCES]->(s)
// ============================================================

use gos_protocol::{
    packet_to_signal, NodeEvent, Signal,
    CHAT_CONTROL_SEND, CHAT_CONTROL_EXIT, CHAT_CONTROL_KEY_BEGIN, CHAT_CONTROL_KEY_COMMIT,
};

/// What kind of work the proc stage should perform.
pub enum InputKind {
    /// Boot-time spawn: initialise COM2 and display welcome.
    Spawn,
    /// A single character of user input to append to the message buffer.
    Byte(u8),
    /// Submit the buffered message to the AI bridge.
    Send,
    /// Exit chat mode and return control to the shell.
    Exit,
    /// Begin streaming API key bytes.
    KeyBegin,
    /// One byte of an in-progress API key stream.
    KeyByte(u8),
    /// Commit the streamed API key.
    KeyCommit,
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

        Signal::Data { byte, .. } => {
            // Filter non-printable control bytes except CR/LF (handled by shell Enter logic)
            if byte >= 0x20 || byte == b'\t' {
                Some(Input { kind: InputKind::Byte(byte) })
            } else {
                None
            }
        }

        Signal::Control { cmd: CHAT_CONTROL_SEND, .. }     => Some(Input { kind: InputKind::Send }),
        Signal::Control { cmd: CHAT_CONTROL_EXIT, .. }     => Some(Input { kind: InputKind::Exit }),
        Signal::Control { cmd: CHAT_CONTROL_KEY_BEGIN, .. }=> Some(Input { kind: InputKind::KeyBegin }),
        Signal::Control { cmd: CHAT_CONTROL_KEY_COMMIT, .. }=>Some(Input { kind: InputKind::KeyCommit }),
        // API key byte streaming: the shell re-uses Data signals with from=CHAT_KEY_STREAM_SENTINEL
        // For simplicity we reuse the CHAT_CONTROL_KEY_BEGIN val as a byte
        Signal::Control { cmd, val } if cmd >= 0xC2 && cmd <= 0xC3 => {
            // Any unmatched control in that range is treated as a key byte
            Some(Input { kind: InputKind::KeyByte(val) })
        }

        _ => None,
    }
}
