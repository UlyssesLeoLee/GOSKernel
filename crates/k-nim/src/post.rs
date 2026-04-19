// ============================================================
// GOS KERNEL TOPOLOGY — k-nim::post
//
// MERGE (m:Module {id: "K_NIM_POST", name: "k-nim::post"})
// SET m.role = "stage:post", m.responsibility = "render NIM inference output to the VGA console"
// MERGE (p:Plugin {id: "K_NIM"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (fn_emit:Function {id: "K_NIM_POST_EMIT", name: "emit"})
// MERGE (m)-[:DEFINES]->(fn_emit)
// MERGE (fn_emit)-[:CONSUMES]->(output_s:Struct {id: "K_NIM_PROC_OUTPUT"})
// MERGE (fn_emit)-[:PRODUCES]->(status:Type {id: "EXEC_STATUS"})
// ============================================================

use gos_protocol::{ExecStatus, ExecutorContext};
use super::{
    proc,
    print_byte, print_str, print_bytes, set_color,
    sink_from_ctx, state_mut,
    draw_nim_banner,
};

/// Stage 3 — render proc's Output to the VGA console.
pub unsafe fn emit(ctx: *mut ExecutorContext, output: proc::Output) -> ExecStatus {
    let sink  = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };

    match output {
        proc::Output::Spawned => {
            // Silent — the shell will trigger the banner when entering NIM mode.
            // The draw_nim_banner function is available for the shell to call.
            let _ = draw_nim_banner; // suppress dead-code lint
        }

        proc::Output::Echo { byte } => {
            // The shell owns the input line and handles its own echo.
            let _ = byte;
        }

        proc::Output::ResponseReady => {
            let resp_len = state.resp_len;
            if resp_len == 0 {
                set_color(&sink, 8, 0); // dark-grey
                print_str(&sink, "[NIM]  (empty response)\n");
                set_color(&sink, 7, 0);
            } else {
                // "NIM ▸ " in magenta (color 13)
                set_color(&sink, 13, 0);
                print_str(&sink, "NIM \u{25B8} "); // "NIM ▸ "
                set_color(&sink, 7, 0);
                print_bytes(&sink, &state.resp_buf[..resp_len]);
                print_byte(&sink, b'\n');
            }
            state.resp_len = 0;
            // Re-draw the user prompt
            set_color(&sink, 14, 0); // yellow
            print_str(&sink, "You \u{25B8} "); // "You ▸ "
            set_color(&sink, 7, 0);
        }

        proc::Output::Exited => {
            set_color(&sink, 8, 0);
            print_str(&sink, "[NIM] session ended -- returning to shell\n");
            set_color(&sink, 7, 0);
        }

        proc::Output::ConfigChanged => {
            set_color(&sink, 10, 0); // bright-green
            print_str(&sink, "[NIM] config updated\n");
            set_color(&sink, 7, 0);
        }

        proc::Output::HistoryCleared => {
            set_color(&sink, 11, 0); // cyan
            print_str(&sink, "[NIM] conversation history cleared\n");
            set_color(&sink, 7, 0);
        }

        proc::Output::NoOp => {}
    }

    ExecStatus::Done
}
