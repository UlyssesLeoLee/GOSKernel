// ── Post-processing ───────────────────────────────────────────────────────────
// Responsibility: route the decoded key output to the appropriate downstream node.
// ASCII keys use the conditional-route table (zero capability-lookup overhead).
// Multi-byte UTF-8 sequences fall back to direct emit via `abi.emit_signal`.

use gos_protocol::{signal_to_packet, ExecStatus, ExecutorContext, Signal};

/// Emit the decoded key output.
/// - `Output::Ascii`  → sets `ctx.route_signal` / `ctx.route_key` and returns `ExecStatus::Route`.
/// - `Output::Utf8`   → lazy-resolves shell_target and emits each byte directly.
pub unsafe fn emit(
    ctx: *mut ExecutorContext,
    state: &mut super::Ps2State,
    output: super::proc::Output,
) -> ExecStatus {
    match output {
        super::proc::Output::Ascii(b) => {
            unsafe {
                (*ctx).route_signal =
                    signal_to_packet(Signal::Data { from: super::NODE_VEC.as_u64(), byte: b });
                (*ctx).route_key = super::PS2_ROUTE_SHELL;
            }
            ExecStatus::Route
        }

        super::proc::Output::Utf8(arr, len) => {
            super::lazy_resolve_shell(ctx, state);
            if state.shell_target != 0 {
                let abi = unsafe { &*(*ctx).abi };
                for &b in &arr[..len] {
                    if let Some(emit_signal) = abi.emit_signal {
                        unsafe {
                            let _ = emit_signal(
                                state.shell_target,
                                signal_to_packet(Signal::Data {
                                    from: super::NODE_VEC.as_u64(),
                                    byte: b,
                                }),
                            );
                        }
                    }
                }
            }
            ExecStatus::Done
        }
    }
}
