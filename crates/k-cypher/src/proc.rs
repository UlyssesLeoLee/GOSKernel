// ============================================================
// GOS KERNEL TOPOLOGY — k-cypher/proc
//
// MERGE (m:Module {id: "K_CYPHER_PROC", name: "proc", plugin: "K_CYPHER"})
// SET m.role = "Process"
// MERGE (p:Plugin {id: "K_CYPHER"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// -- Exports
// MERGE (fn_process:Function {id: "K_CYPHER_PROC_process", name: "process"})
// MERGE (m)-[:EXPORTS]->(fn_process)
// MERGE (struct_output:Struct {id: "K_CYPHER_PROC_Output", name: "Output"})
// MERGE (m)-[:EXPORTS]->(struct_output)
//
// -- Dependencies
// MERGE (m_pre:Module {id: "K_CYPHER_PRE"})
// MERGE (m)-[:USES]->(m_pre)
// ============================================================

use gos_protocol::{ExecutorContext, ExecStatus, Signal, CYPHER_CONTROL_QUERY_BEGIN, CYPHER_CONTROL_QUERY_COMMIT};

use super::pre::Input;
use super::{state_mut, set_color, print_str, run_query};

/// The result of core processing — carries the `ExecStatus` to be emitted.
pub struct Output {
    pub status: ExecStatus,
}

/// Execute the core cipher logic: state mutations and query dispatch.
///
/// Returns `None` if processing cannot continue (currently unused, kept for
/// pipeline symmetry).
pub unsafe fn process(ctx: *mut ExecutorContext, input: Input) -> Option<Output> {
    let Input { sink, signal } = input;
    let state = unsafe { state_mut(ctx) };

    let status = match signal {
        Signal::Spawn { .. } => ExecStatus::Done,

        Signal::Control { cmd, .. } if cmd == CYPHER_CONTROL_QUERY_BEGIN => {
            state.query = [0; 224];
            state.query_len = 0;
            state.capture_active = true;
            ExecStatus::Done
        }

        Signal::Control { cmd, .. } if cmd == CYPHER_CONTROL_QUERY_COMMIT => {
            state.capture_active = false;
            let query_len = state.query_len.min(state.query.len());
            let mut query_buf = [0u8; 224];
            query_buf[..query_len].copy_from_slice(&state.query[..query_len]);
            if let Ok(query) = core::str::from_utf8(&query_buf[..query_len]) {
                run_query(&sink, state, query);
            } else {
                set_color(&sink, 12, 0);
                print_str(&sink, "cypher> query payload must be utf-8 ascii subset\n");
                set_color(&sink, 7, 0);
                state.faults = state.faults.saturating_add(1);
            }
            ExecStatus::Done
        }

        Signal::Data { byte, .. } => {
            if state.capture_active && state.query_len < state.query.len() {
                state.query[state.query_len] = byte;
                state.query_len += 1;
            }
            ExecStatus::Done
        }

        _ => ExecStatus::Done,
    };

    Some(Output { status })
}
