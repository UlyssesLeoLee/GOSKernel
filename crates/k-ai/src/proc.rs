// ============================================================
// GOS KERNEL TOPOLOGY — k-ai::proc
//
// MERGE (m:Module {id: "K_AI_PROC", name: "proc", file: "crates/k-ai/src/proc.rs"})
// MERGE (p:Plugin {id: "K_AI"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Structs defined here
// MERGE (s:Struct {id: "K_AI_PROC_OUTPUT", name: "Output"})
// MERGE (m)-[:DEFINES]->(s)
//
// -- Functions defined here
// MERGE (f:Function {id: "K_AI_PROC_PROCESS", name: "process"})
// MERGE (m)-[:DEFINES]->(f)
// ============================================================

use gos_protocol::{
    ExecStatus, ExecutorContext, Signal,
    AI_CONTROL_API_BEGIN, AI_CONTROL_API_COMMIT,
    AI_CONTROL_CHAT_BEGIN, AI_CONTROL_CHAT_COMMIT,
};
use super::{
    append_api_byte, append_chat_byte, begin_api_capture, begin_chat_capture,
    commit_api_capture, commit_chat_capture, drain_control_plane_into, emit_shell_chat_line,
    handoff_shell, print_runtime_brief,
};
use super::pre::Input;

pub struct Output {
    pub status: ExecStatus,
}

/// Core AI processing logic: dispatch on signal kind, mutate state, emit to
/// console / shell as needed.  Returns `None` if processing cannot continue.
pub unsafe fn process(_ctx: *mut ExecutorContext, input: Input) -> Option<Output> {
    let Input { sink, state, signal } = input;

    let status = match signal {
        Signal::Spawn { .. } => {
            drain_control_plane_into(state);
            print_runtime_brief(&sink, state);
            handoff_shell(&sink, state);
            emit_shell_chat_line(&sink, state, "ai> supervisor online");
            emit_shell_chat_line(&sink, state, "ai> type ask <prompt> on the left");
            ExecStatus::Done
        }
        Signal::Control { .. } => {
            if let Signal::Control { cmd, .. } = signal {
                if cmd == AI_CONTROL_API_BEGIN {
                    begin_api_capture(state);
                } else if cmd == AI_CONTROL_API_COMMIT {
                    commit_api_capture(state);
                    emit_shell_chat_line(&sink, state, "ai> uplink key armed");
                } else if cmd == AI_CONTROL_CHAT_BEGIN {
                    begin_chat_capture(state);
                } else if cmd == AI_CONTROL_CHAT_COMMIT {
                    commit_chat_capture(&sink, state);
                } else {
                    drain_control_plane_into(state);
                }
            }
            ExecStatus::Done
        }
        Signal::Data { byte, .. } => {
            if state.api_capture_active {
                append_api_byte(state, byte);
            } else if state.prompt_capture_active {
                append_chat_byte(state, byte);
            }
            ExecStatus::Done
        }
        Signal::Call { .. } | Signal::Interrupt { .. } | Signal::Terminate => {
            ExecStatus::Done
        }
    };

    Some(Output { status })
}
