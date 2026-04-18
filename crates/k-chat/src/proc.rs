// ============================================================
// GOS KERNEL TOPOLOGY — k-chat::proc
//
// MERGE (m:Module {id: "K_CHAT_PROC", name: "k-chat::proc"})
// SET m.role = "stage:proc", m.responsibility = "chat session state machine and bridge I/O"
// MERGE (p:Plugin {id: "K_CHAT"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (s:Struct {id: "K_CHAT_PROC_OUTPUT", name: "Output"})
// MERGE (m)-[:DEFINES]->(s)
// MERGE (fn_process:Function {id: "K_CHAT_PROC_PROCESS", name: "process"})
// MERGE (m)-[:DEFINES]->(fn_process)
// MERGE (fn_process)-[:CONSUMES]->(input_s:Struct {id: "K_CHAT_PRE_INPUT"})
// MERGE (fn_process)-[:PRODUCES]->(s)
// ============================================================

use gos_protocol::ExecutorContext;
use super::{pre, state_mut, MSG_BUF_SIZE};

/// What the post stage should render.
pub enum Output {
    /// Newly spawned: display the chat banner.
    Spawned,
    /// A character was buffered — echo it to the input line.
    Echo { byte: u8 },
    /// Message submitted — the bridge call has already completed and the
    /// response text is stored in state.resp_buf[..state.resp_len].
    ResponseReady,
    /// Chat session closed.
    Exited,
    /// API key flushed — nothing to display.
    KeyStored,
    /// Nothing to do.
    NoOp,
}

/// Stage 2 — drive the chat state machine.
///
/// Heavy bridge I/O (COM2 read/write) happens here so that post only renders.
pub unsafe fn process(ctx: *mut ExecutorContext, input: pre::Input) -> Option<Output> {
    let state = unsafe { state_mut(ctx) };

    match input.kind {
        pre::InputKind::Spawn => Some(Output::Spawned),

        pre::InputKind::Byte(b) => {
            if state.input_len < MSG_BUF_SIZE {
                state.input_buf[state.input_len] = b;
                state.input_len += 1;
            }
            Some(Output::Echo { byte: b })
        }

        pre::InputKind::Send => {
            if state.input_len == 0 {
                return Some(Output::NoOp);
            }
            if state.com2_ready == 0 {
                // Bridge not detected — store a short error message as "response"
                let err = b"[CHAT] COM2 bridge not ready. Start chat-bridge.py on the host.";
                let n = err.len().min(super::RESP_BUF_SIZE);
                state.resp_buf[..n].copy_from_slice(&err[..n]);
                state.resp_len = n;
                state.input_len = 0;
                return Some(Output::ResponseReady);
            }
            // Send: "GCHAT:<message>\n"
            super::com2_write_line(b"GCHAT:", &state.input_buf[..state.input_len]);
            // Clear input
            state.input_len = 0;
            // Collect response into resp_buf
            state.resp_len = 0;
            super::collect_bridge_response(state);
            Some(Output::ResponseReady)
        }

        pre::InputKind::Exit => {
            state.input_len = 0;
            state.resp_len = 0;
            Some(Output::Exited)
        }

        pre::InputKind::KeyBegin => {
            state.api_key_len = 0;
            Some(Output::NoOp)
        }

        pre::InputKind::KeyByte(b) => {
            if (state.api_key_len as usize) < super::API_KEY_BUF {
                let idx = state.api_key_len as usize;
                state.api_key[idx] = b;
                state.api_key_len += 1;
            }
            Some(Output::NoOp)
        }

        pre::InputKind::KeyCommit => Some(Output::KeyStored),
    }
}
