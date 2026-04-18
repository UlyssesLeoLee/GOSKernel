// ============================================================
// GOS KERNEL TOPOLOGY — k-chat::post
//
// MERGE (m:Module {id: "K_CHAT_POST", name: "k-chat::post"})
// SET m.role = "stage:post", m.responsibility = "render chat output to the VGA console"
// MERGE (p:Plugin {id: "K_CHAT"})
// MERGE (m)-[:BELONGS_TO]->(p)
//
// MERGE (fn_emit:Function {id: "K_CHAT_POST_EMIT", name: "emit"})
// MERGE (m)-[:DEFINES]->(fn_emit)
// MERGE (fn_emit)-[:CONSUMES]->(output_s:Struct {id: "K_CHAT_PROC_OUTPUT"})
// MERGE (fn_emit)-[:PRODUCES]->(status:Type {id: "EXEC_STATUS"})
// ============================================================

use gos_protocol::{ExecStatus, ExecutorContext};
use super::{proc, print_byte, print_str, print_bytes, set_color, sink_from_ctx, state_mut};

/// Stage 3 — render proc's Output to the VGA console.
pub unsafe fn emit(ctx: *mut ExecutorContext, output: proc::Output) -> ExecStatus {
    let sink  = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };

    match output {
        proc::Output::Spawned => {
            // Silent init — the shell will draw the chat UI when it enters chat mode.
        }

        proc::Output::Echo { byte } => {
            // The shell owns the input line; k-chat only echoes during direct injection
            // (currently unused — shell handles its own echo).
            let _ = byte;
        }

        proc::Output::ResponseReady => {
            // Print everything that was collected into resp_buf.
            // resp_buf lines are separated by '\n' with a "GRESP:" prefix already stripped.
            let resp_len = state.resp_len;
            if resp_len == 0 {
                set_color(&sink, 8, 0);
                print_str(&sink, "[AI]  (empty response)\n");
                set_color(&sink, 7, 0);
            } else {
                set_color(&sink, 11, 0); // cyan for AI label
                print_str(&sink, "AI  ▸ ");
                set_color(&sink, 7, 0);
                print_bytes(&sink, &state.resp_buf[..resp_len]);
                print_byte(&sink, b'\n');
            }
            state.resp_len = 0;
            // Re-draw the chat prompt so the user can type again.
            set_color(&sink, 14, 0); // yellow
            print_str(&sink, "You ▸ ");
            set_color(&sink, 7, 0);
        }

        proc::Output::Exited => {
            set_color(&sink, 8, 0);
            print_str(&sink, "[CHAT] session ended — returning to shell\n\n");
            set_color(&sink, 7, 0);
        }

        proc::Output::KeyStored => {
            set_color(&sink, 10, 0);
            print_str(&sink, "[CHAT] API key updated\n");
            set_color(&sink, 7, 0);
        }

        proc::Output::NoOp => {}
    }

    ExecStatus::Done
}
