// ============================================================
// GOS KERNEL TOPOLOGY — k-cuda-host/proc
//
// MERGE (m:Module {id: "k_cuda_host::proc", name: "proc"})
// SET m.role = "command", m.stage = 1
// MERGE (p:Plugin {id: "K_CUDA"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Structs
// MERGE (s:Struct {id: "k_cuda_host::proc::Output"})
// MERGE (m)-[:DEFINES]->(s)
//
// -- Functions
// MERGE (f:Function {id: "k_cuda_host::proc::process"})
// SET f.unsafe = true, f.returns = "Option<Output>"
// MERGE (m)-[:DEFINES]->(f)
// MERGE (f)-[:PRODUCES]->(s)
// ============================================================

use gos_protocol::{
    ExecStatus, Signal,
    CUDA_CONTROL_JOB_BEGIN, CUDA_CONTROL_JOB_COMMIT, CUDA_CONTROL_REPORT, CUDA_CONTROL_RESET,
};
use super::{
    append_capture_byte, begin_capture, clear_capture, commit_capture, emit_console_str,
    emit_reset_frame, emit_serial_hello, emit_status_report, set_color, state_mut, ExecutorContext,
};
use crate::pre::Input;

pub struct Output {
    pub status: ExecStatus,
}

/// Execute CUDA host command logic for the decoded signal.
/// Mutates plugin state and calls console/serial emit helpers as needed.
/// Returns `None` only on an unrecoverable internal error (currently infallible).
pub unsafe fn process(ctx: *mut ExecutorContext, input: Input) -> Option<Output> {
    let Input { sink, signal } = input;
    let state = unsafe { state_mut(ctx) };

    let status = match signal {
        Signal::Spawn { .. } => {
            set_color(&sink, 13, 0);
            emit_console_str(&sink, "\n[CUDA] host bridge online\n");
            set_color(&sink, 7, 0);
            emit_console_str(&sink, "       graph-native bridge for host-backed CUDA jobs via serial\n");
            emit_serial_hello(&sink, state);
            ExecStatus::Done
        }
        Signal::Control { cmd, .. } => {
            match cmd {
                CUDA_CONTROL_JOB_BEGIN => begin_capture(state),
                CUDA_CONTROL_JOB_COMMIT => commit_capture(&sink, state),
                CUDA_CONTROL_REPORT => emit_status_report(&sink, state),
                CUDA_CONTROL_RESET => {
                    emit_reset_frame(&sink, state);
                    clear_capture(state);
                    state.jobs_submitted = 0;
                    state.last_payload_len = 0;
                    set_color(&sink, 11, 0);
                    emit_console_str(&sink, "cuda> bridge counters reset\n");
                    set_color(&sink, 7, 0);
                }
                _ => {}
            }
            ExecStatus::Done
        }
        Signal::Data { byte, .. } => {
            append_capture_byte(state, byte);
            ExecStatus::Done
        }
        Signal::Call { .. } | Signal::Interrupt { .. } | Signal::Terminate => ExecStatus::Done,
    };

    Some(Output { status })
}
