// ============================================================
// GOS KERNEL TOPOLOGY — k-chat::proc
//
// MERGE (m:Module {id: "K_CHAT_PROC", name: "k-chat::proc"})
// SET m.role = "stage:proc", m.responsibility = "chat session state machine and bridge I/O"
// MERGE (p:Plugin {id: "K_CHAT"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================

use gos_protocol::ExecutorContext;
use super::{pre, state_mut, MSG_BUF_SIZE, API_KEY_BUF, API_OLLAMA, API_OPENAI, API_ANTHROPIC};

/// Streaming mode constants (mirrors ChatState::streaming_mode).
const STREAM_NONE:  u8 = 0;
const STREAM_KEY:   u8 = 1;
const STREAM_MODEL: u8 = 2;

/// What the post stage should render.
pub enum Output {
    /// Newly spawned: display the chat banner.
    Spawned,
    /// A character was buffered — echo it to the input line.
    Echo { byte: u8 },
    /// Message submitted — the bridge call has completed and
    /// the response text is stored in state.resp_buf[..state.resp_len].
    ResponseReady,
    /// Chat session closed.
    Exited,
    /// Configuration change committed — nothing to display.
    ConfigChanged,
    /// Nothing to do.
    NoOp,
}

/// Stage 2 — drive the chat state machine.
///
/// Heavy I/O (COM2 or TCP) happens here so that post only renders.
pub unsafe fn process(ctx: *mut ExecutorContext, input: pre::Input) -> Option<Output> {
    let state = unsafe { state_mut(ctx) };

    match input.kind {
        // ── Boot ──────────────────────────────────────────────────────────────
        pre::InputKind::Spawn => Some(Output::Spawned),

        // ── Byte input ────────────────────────────────────────────────────────
        pre::InputKind::Byte(b) => {
            match state.streaming_mode {
                STREAM_KEY => {
                    // Accumulate into api_key buffer
                    if (state.api_key_len as usize) < API_KEY_BUF {
                        let idx = state.api_key_len as usize;
                        state.api_key[idx] = b;
                        state.api_key_len = state.api_key_len.saturating_add(1);
                    }
                    Some(Output::NoOp)
                }
                STREAM_MODEL => {
                    // Accumulate into model buffer
                    if (state.model_len as usize) < state.model.len() {
                        let idx = state.model_len as usize;
                        state.model[idx] = b;
                        state.model_len = state.model_len.saturating_add(1);
                    }
                    Some(Output::NoOp)
                }
                _ => {
                    // Normal chat input
                    if state.input_len < MSG_BUF_SIZE {
                        state.input_buf[state.input_len] = b;
                        state.input_len += 1;
                    }
                    Some(Output::Echo { byte: b })
                }
            }
        }

        // ── Send message ──────────────────────────────────────────────────────
        pre::InputKind::Send => {
            if state.input_len == 0 {
                return Some(Output::NoOp);
            }
            state.resp_len = 0;

            if state.http_mode == 1 {
                // Direct TCP/HTTP to Ollama (or configured endpoint)
                unsafe { super::chat_direct_http(state) };
            } else if state.com2_ready == 0 {
                // Bridge not detected
                let err = b"[CHAT] COM2 bridge not ready. Start chat-bridge.py on the host.";
                let n = err.len().min(super::RESP_BUF_SIZE);
                state.resp_buf[..n].copy_from_slice(&err[..n]);
                state.resp_len = n;
            } else {
                // COM2 bridge path
                super::com2_write_line(b"GCHAT:", &state.input_buf[..state.input_len]);
                super::collect_bridge_response(state);
            }

            state.input_len = 0;
            Some(Output::ResponseReady)
        }

        // ── Exit ──────────────────────────────────────────────────────────────
        pre::InputKind::Exit => {
            state.input_len = 0;
            state.resp_len  = 0;
            state.streaming_mode = STREAM_NONE;
            Some(Output::Exited)
        }

        // ── API key streaming ─────────────────────────────────────────────────
        pre::InputKind::KeyBegin => {
            state.api_key_len    = 0;
            state.streaming_mode = STREAM_KEY;
            Some(Output::NoOp)
        }
        pre::InputKind::KeyCommit => {
            state.streaming_mode = STREAM_NONE;
            Some(Output::ConfigChanged)
        }

        // ── Model name streaming ──────────────────────────────────────────────
        pre::InputKind::ModelBegin => {
            state.model_len      = 0;
            state.streaming_mode = STREAM_MODEL;
            Some(Output::NoOp)
        }
        pre::InputKind::ModelCommit => {
            state.streaming_mode = STREAM_NONE;
            Some(Output::ConfigChanged)
        }

        // ── API type ──────────────────────────────────────────────────────────
        pre::InputKind::ApiType(t) => {
            state.api_type = match t {
                API_OPENAI    => API_OPENAI,
                API_ANTHROPIC => API_ANTHROPIC,
                _             => API_OLLAMA,
            };
            // Adjust default port when switching to/from Ollama
            if state.api_type == API_OLLAMA && state.http_port != 11434 {
                // Keep user's port choice; only reset if it was untouched
            }
            Some(Output::ConfigChanged)
        }

        // ── HTTP mode toggle ──────────────────────────────────────────────────
        pre::InputKind::HttpMode(m) => {
            state.http_mode = if m == 0 { 0 } else { 1 };
            Some(Output::ConfigChanged)
        }
    }
}
