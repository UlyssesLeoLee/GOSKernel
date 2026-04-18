// ============================================================
// GOS KERNEL TOPOLOGY — k-net::post
//
// MERGE (m:Module {id: "K_NET_POST", name: "k-net::post"})
// SET m.role = "stage:post", m.responsibility = "emit probe report downstream and finalise execution"
// MERGE (p:Plugin {id: "K_NET"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (fn_emit:Function {id: "K_NET_POST_EMIT", name: "emit"})
// MERGE (m)-[:DEFINES]->(fn_emit)
// MERGE (fn_emit)-[:CONSUMES]->(output_s:Struct {id: "K_NET_PROC_OUTPUT"})
// MERGE (fn_emit)-[:PRODUCES]->(status:Type {id: "EXEC_STATUS"})
// ============================================================

use gos_protocol::{ExecStatus, ExecutorContext};

use super::{print_probe_report, print_str, set_color, proc, sink_from_ctx, state_mut};

fn print_ping_result(
    sink: &super::ConsoleSink,
    state: &super::NetState,
    reply: bool,
    rtt_polls: u32,
) {
    if reply {
        set_color(sink, 10, 0); // green
        print_str(sink, "\n[NET] ping ");
        // print target IP
        for (i, &byte) in state.ping_target_ip.iter().enumerate() {
            if i != 0 { print_str(sink, "."); }
            super::print_num_u64(sink, byte as u64);
        }
        print_str(sink, " — reply received");
        if rtt_polls > 0 {
            print_str(sink, " (");
            super::print_num_u64(sink, rtt_polls as u64);
            print_str(sink, " poll cycles)");
        }
        print_str(sink, "\n\n");
    } else {
        set_color(sink, 12, 0); // red
        print_str(sink, "\n[NET] ping ");
        for (i, &byte) in state.ping_target_ip.iter().enumerate() {
            if i != 0 { print_str(sink, "."); }
            super::print_num_u64(sink, byte as u64);
        }
        print_str(sink, " — request timed out\n\n");
    }
    set_color(sink, 7, 0);
}

/// Stage 3 — Emit the result to the console sink.
pub unsafe fn emit(ctx: *mut ExecutorContext, output: proc::Output) -> ExecStatus {
    let sink  = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };

    match output {
        proc::Output::Report { title } => {
            print_probe_report(&sink, state, title);
        }
        proc::Output::Ping { reply, rtt_polls } => {
            print_ping_result(&sink, state, reply, rtt_polls);
        }
    }

    ExecStatus::Done
}
