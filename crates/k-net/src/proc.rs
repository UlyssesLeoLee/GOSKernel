// ============================================================
// GOS KERNEL TOPOLOGY — k-net::proc
//
// MERGE (m:Module {id: "K_NET_PROC", name: "k-net::proc"})
// SET m.role = "stage:proc", m.responsibility = "core network driver logic and routing decisions"
// MERGE (p:Plugin {id: "K_NET"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (s:Struct {id: "K_NET_PROC_OUTPUT", name: "Output"})
// MERGE (m)-[:DEFINES]->(s)
// MERGE (fn_process:Function {id: "K_NET_PROC_PROCESS", name: "process"})
// MERGE (m)-[:DEFINES]->(fn_process)
// MERGE (fn_process)-[:CONSUMES]->(input_s:Struct {id: "K_NET_PRE_INPUT"})
// MERGE (fn_process)-[:PRODUCES]->(s)
// ============================================================

use gos_protocol::{
    ExecutorContext, NET_CONTROL_PROBE, NET_CONTROL_REPORT, NET_CONTROL_RESET, Signal,
};

use super::{pre, refresh_network_state, state_mut};

/// The resolved title for the probe report to be emitted in the post stage.
pub struct Output {
    pub title: &'static str,
}

/// Stage 2 — Core business logic: drive PCI enumeration, NIC initialisation, and
/// routing decisions based on the incoming signal.
///
/// Updates `NetState` in place via the shared `super` helpers, then returns an
/// `Output` describing what the post stage should report.  Returns `None` only
/// when no report is warranted (currently unreachable given pre-stage filtering,
/// but kept for extensibility).
pub unsafe fn process(ctx: *mut ExecutorContext, input: pre::Input) -> Option<Output> {
    let state = unsafe { state_mut(ctx) };

    let title = match input.signal {
        Signal::Spawn { .. } => {
            refresh_network_state(state);
            "uplink boot sync"
        }
        Signal::Control { cmd, .. } => match cmd {
            NET_CONTROL_REPORT => {
                if state.probe_complete == 0 {
                    refresh_network_state(state);
                }
                "uplink status"
            }
            NET_CONTROL_RESET => {
                refresh_network_state(state);
                "uplink reset"
            }
            NET_CONTROL_PROBE | 1 => {
                refresh_network_state(state);
                "uplink reprobe"
            }
            _ => {
                if state.probe_complete == 0 {
                    refresh_network_state(state);
                }
                "uplink status"
            }
        },
        // pre::prepare only passes Spawn and Control; this branch is unreachable.
        _ => return None,
    };

    Some(Output { title })
}
