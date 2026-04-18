// ============================================================
// k-shell :: pre — signal decoding stage
//
// MERGE (pre:Module {id: "k_shell::pre", name: "pre"})
// MERGE (lib:Module {id: "k_shell::lib", name: "lib"})
// MERGE (pre)-[:STAGE_OF]->(lib)
// MERGE (pre)-[:PRODUCES]->(:Struct {name: "Input"})
// ============================================================

use gos_protocol::{ExecutorContext, NodeEvent, Signal, packet_to_signal};

// ---------------------------------------------------------------------------
// Signal source — identifies who sent the incoming data byte.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    /// Byte forwarded from the IME node (composed character from k-ime).
    Ime,
    /// Byte pasted / relayed from the clipboard node.
    Clipboard,
    /// Streaming token byte from the AI supervisor node.
    Ai,
    /// Raw keystroke from the PS/2 or keyboard driver (local operator input).
    Keyboard,
}

// ---------------------------------------------------------------------------
// Input — everything proc::process needs; produced by pre::prepare.
// ---------------------------------------------------------------------------
pub enum Input {
    /// A data byte arrived, tagged with its source so proc can route it.
    Data { source: DataSource, byte: u8 },

    /// A spawn signal: play boot cinema and bring the console live.
    Spawn,

    /// A heartbeat interrupt (irq == 32) while the console is live.
    Heartbeat,

    /// Any other signal (ignored by proc, status Done is returned).
    Other,
}

// ---------------------------------------------------------------------------
// prepare — decode the raw NodeEvent into a typed Input.
//
// Returns None when the event can be ignored outright (irq != 32 while live,
// or an irq when the console is not yet live).  In both cases the caller
// returns ExecStatus::Done immediately.
// ---------------------------------------------------------------------------
pub unsafe fn prepare(ctx: *mut ExecutorContext, event: *const NodeEvent) -> Option<Input> {
    let signal = packet_to_signal(unsafe { (*event).signal });

    let state = unsafe { super::state_mut(ctx) };

    match signal {
        Signal::Data { from, byte } => {
            let source = if from == state.ime_target {
                DataSource::Ime
            } else if from == state.clipboard_target {
                DataSource::Clipboard
            } else if from == state.ai_target {
                DataSource::Ai
            } else {
                DataSource::Keyboard
            };
            Some(Input::Data { source, byte })
        }

        Signal::Spawn { .. } => Some(Input::Spawn),

        Signal::Interrupt { irq } => {
            if irq == 32 && state.console_live != 0 {
                Some(Input::Heartbeat)
            } else {
                // irq != 32, or console not yet live — nothing to do.
                Some(Input::Other)
            }
        }

        _ => Some(Input::Other),
    }
}
