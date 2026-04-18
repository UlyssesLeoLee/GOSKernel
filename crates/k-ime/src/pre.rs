// ── Pre-processing ────────────────────────────────────────────────────────────
// Responsibility: lazy-resolve the shell capability, then decode the signal.
// IME boots before Shell (by load order), so shell_target may still be 0 here.

use gos_protocol::{packet_to_signal, ExecutorContext, NodeEvent, Signal};

/// Decoded IME input ready for processing.
pub enum Input {
    /// IME mode-switch control command.
    ModeControl { val: u8 },
    /// A key byte to be processed through the active IME mode.
    Key { byte: u8 },
}

/// Lazy-resolve shell_target, then decode the signal.
/// Returns `None` for unrecognised signal kinds.
pub unsafe fn prepare(ctx: *mut ExecutorContext, event: *const NodeEvent) -> Option<Input> {
    // Lazy resolution: IME may boot before Shell registers.
    let state = unsafe { super::state_mut(ctx) };
    if state.shell_target == 0 {
        let abi = super::abi_from_ctx(ctx);
        if let Some(resolve) = abi.resolve_capability {
            let resolved = unsafe { resolve(b"shell".as_ptr(), 5, b"input".as_ptr(), 5) };
            if resolved != 0 {
                state.shell_target = resolved;
            }
        }
    }

    let signal = packet_to_signal(unsafe { (*event).signal });
    match signal {
        Signal::Control { cmd, val }
            if cmd == gos_protocol::IME_CONTROL_SET_MODE => Some(Input::ModeControl { val }),
        Signal::Data { byte, .. } => Some(Input::Key { byte }),
        _ => None,
    }
}
