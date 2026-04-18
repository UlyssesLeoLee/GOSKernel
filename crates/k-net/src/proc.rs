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
    ExecutorContext, Signal,
    NET_CONTROL_PROBE, NET_CONTROL_REPORT, NET_CONTROL_RESET, NET_CONTROL_PING,
    NET_CONTROL_SET_IP0, NET_CONTROL_SET_IP1, NET_CONTROL_SET_IP2, NET_CONTROL_SET_IP3,
};

use super::{pre, do_ping, refresh_network_state, state_mut};

/// The output produced by the proc stage.
pub enum Output {
    /// A probe/status/reset report — post will print a summary.
    Report { title: &'static str },
    /// An ICMP ping result.
    Ping { reply: bool, rtt_polls: u32 },
}

/// Stage 2 — Core business logic.
pub unsafe fn process(ctx: *mut ExecutorContext, input: pre::Input) -> Option<Output> {
    let state = unsafe { state_mut(ctx) };

    match input.signal {
        Signal::Spawn { .. } => {
            refresh_network_state(state);
            Some(Output::Report { title: "uplink boot sync" })
        }

        Signal::Control { cmd, val } => match cmd {
            NET_CONTROL_REPORT => {
                if state.probe_complete == 0 {
                    refresh_network_state(state);
                }
                Some(Output::Report { title: "uplink status" })
            }
            NET_CONTROL_RESET => {
                refresh_network_state(state);
                Some(Output::Report { title: "uplink reset" })
            }
            NET_CONTROL_PROBE => {
                refresh_network_state(state);
                Some(Output::Report { title: "uplink reprobe" })
            }

            // ── Target IP configuration ───────────────────────────────────
            NET_CONTROL_SET_IP0 => {
                state.ping_target_ip[0] = val;
                state.gw_mac_valid = 0; // invalidate cached ARP entry on IP change
                None
            }
            NET_CONTROL_SET_IP1 => { state.ping_target_ip[1] = val; None }
            NET_CONTROL_SET_IP2 => { state.ping_target_ip[2] = val; None }
            NET_CONTROL_SET_IP3 => {
                state.ping_target_ip[3] = val;
                state.gw_mac_valid = 0;
                None
            }

            // ── ICMP ping ─────────────────────────────────────────────────
            NET_CONTROL_PING => {
                let (reply, rtt_polls) = unsafe { do_ping(state) };
                Some(Output::Ping { reply, rtt_polls })
            }

            _ => {
                if state.probe_complete == 0 {
                    refresh_network_state(state);
                }
                Some(Output::Report { title: "uplink status" })
            }
        },

        _ => None,
    }
}
