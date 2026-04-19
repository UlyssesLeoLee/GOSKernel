// ============================================================
// GOS KERNEL TOPOLOGY — k-nim::proc
//
// MERGE (m:Module {id: "K_NIM_PROC", name: "k-nim::proc"})
// SET m.role = "stage:proc", m.responsibility = "NIM session state machine and inference dispatch"
// MERGE (p:Plugin {id: "K_NIM"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================

use gos_protocol::ExecutorContext;
use super::{pre, state_mut, append_history, nim_do_request, INPUT_BUF_SIZE};

/// Streaming mode constants (mirrors NimState::streaming_mode).
const STREAM_NONE:  u8 = 0;
const STREAM_MODEL: u8 = 1;
const STREAM_PORT:  u8 = 2;

/// What the post stage should render.
pub enum Output {
    /// Newly spawned: display the NIM banner and initial prompt.
    Spawned,
    /// A character was buffered — the shell handles its own echo.
    Echo { byte: u8 },
    /// Inference request completed — response is in `state.resp_buf`.
    ResponseReady,
    /// NIM session closed.
    Exited,
    /// Configuration (model/port) updated.
    ConfigChanged,
    /// Multi-turn history cleared.
    HistoryCleared,
    /// Nothing to do.
    NoOp,
}

/// Stage 2 — drive the NIM state machine.
///
/// All heavy I/O (TCP inference request) happens here so that post only renders.
pub unsafe fn process(ctx: *mut ExecutorContext, input: pre::Input) -> Option<Output> {
    let state = unsafe { state_mut(ctx) };

    match input.kind {
        // ── Boot ──────────────────────────────────────────────────────────────
        pre::InputKind::Spawn => Some(Output::Spawned),

        // ── Byte input ────────────────────────────────────────────────────────
        pre::InputKind::Byte(b) => {
            match state.streaming_mode {
                STREAM_MODEL => {
                    // Accumulate into model buffer (max 63 bytes + guard)
                    let idx = state.model_len as usize;
                    if idx < state.model.len() {
                        state.model[idx] = b;
                        state.model_len = state.model_len.saturating_add(1);
                    }
                    Some(Output::NoOp)
                }
                STREAM_PORT => {
                    // Accumulate digits only
                    if b.is_ascii_digit() {
                        let idx = state.port_digit_len as usize;
                        if idx < state.port_digits.len() {
                            state.port_digits[idx] = b;
                            state.port_digit_len = state.port_digit_len.saturating_add(1);
                        }
                    }
                    Some(Output::NoOp)
                }
                _ => {
                    // Normal chat input
                    if state.input_len < INPUT_BUF_SIZE {
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

            // Snapshot the user message before nim_do_request clears nothing
            // (input_buf is cleared after the request).
            let user_msg_len = state.input_len;
            let mut user_msg_copy = [0u8; INPUT_BUF_SIZE];
            user_msg_copy[..user_msg_len].copy_from_slice(&state.input_buf[..user_msg_len]);

            // Perform inference
            unsafe { nim_do_request(state) };

            // Append exchange to history
            let resp_len = state.resp_len;
            let mut resp_copy = [0u8; 512];
            let copy_len = resp_len.min(512);
            resp_copy[..copy_len].copy_from_slice(&state.resp_buf[..copy_len]);
            append_history(state, &user_msg_copy[..user_msg_len], &resp_copy[..copy_len]);

            state.input_len = 0;
            Some(Output::ResponseReady)
        }

        // ── Exit ──────────────────────────────────────────────────────────────
        pre::InputKind::Exit => {
            state.input_len     = 0;
            state.resp_len      = 0;
            state.streaming_mode = STREAM_NONE;
            Some(Output::Exited)
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

        // ── Port number streaming ─────────────────────────────────────────────
        pre::InputKind::PortBegin => {
            state.port_digit_len = 0;
            state.streaming_mode = STREAM_PORT;
            Some(Output::NoOp)
        }
        pre::InputKind::PortCommit => {
            state.streaming_mode = STREAM_NONE;
            // Parse accumulated decimal digits into nim_port
            if state.port_digit_len > 0 {
                let mut val: u32 = 0;
                for i in 0..state.port_digit_len as usize {
                    val = val.wrapping_mul(10)
                             .wrapping_add((state.port_digits[i] - b'0') as u32);
                }
                // Clamp to valid u16 range; keep existing port if 0
                if val > 0 && val <= 65535 {
                    state.nim_port = val as u16;
                }
            }
            state.port_digit_len = 0;
            Some(Output::ConfigChanged)
        }

        // ── Clear history ─────────────────────────────────────────────────────
        pre::InputKind::ClearHistory => {
            state.history_len = 0;
            state.turn_count  = 0;
            Some(Output::HistoryCleared)
        }
    }
}
