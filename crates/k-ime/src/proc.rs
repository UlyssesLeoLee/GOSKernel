// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: run the IME state machine.
//   - Mode control: switch between ASCII and Pinyin modes.
//   - Key input (ASCII mode): pass through directly.
//   - Key input (Pinyin mode): compose, lookup candidates, commit selection.
// Produces zero or more bytes to emit to Shell.

use gos_protocol::{ExecutorContext, IME_MODE_ASCII, IME_MODE_ZH_PINYIN};

/// Bytes to send to the Shell node (maximum: one committed CJK string ≤ 32 bytes).
pub struct Output {
    /// Byte buffer — valid range is `[0..len]`.
    pub buf: [u8; 64],
    pub len: usize,
}

impl Output {
    fn empty() -> Self { Self { buf: [0; 64], len: 0 } }

    fn push_bytes(&mut self, src: &[u8]) {
        let room = self.buf.len() - self.len;
        let n = src.len().min(room);
        self.buf[self.len..self.len + n].copy_from_slice(&src[..n]);
        self.len += n;
    }

    fn push_byte(&mut self, b: u8) {
        if self.len < self.buf.len() {
            self.buf[self.len] = b;
            self.len += 1;
        }
    }
}

/// Run the IME state machine and produce the bytes to forward to Shell.
pub unsafe fn process(ctx: *mut ExecutorContext, input: super::pre::Input) -> Option<Output> {
    let state = unsafe { super::state_mut(ctx) };
    let mut out = Output::empty();

    match input {
        super::pre::Input::ModeControl { val } => {
            state.mode = if val == IME_MODE_ZH_PINYIN {
                IME_MODE_ZH_PINYIN
            } else {
                IME_MODE_ASCII
            };
            super::clear_composition(state);
            // No bytes to emit on mode switch.
        }

        super::pre::Input::Key { byte } => {
            if state.mode == IME_MODE_ASCII {
                out.push_byte(byte);
            } else {
                process_pinyin(state, byte, &mut out);
            }
        }
    }

    Some(out)
}

/// Pinyin composition state machine.
fn process_pinyin(state: &mut super::ImeState, byte: u8, out: &mut Output) {
    match byte {
        b'a'..=b'z' | b'A'..=b'Z' => {
            if state.len < state.composing.len() {
                state.composing[state.len] = super::normalize_letter(byte);
                state.len += 1;
            }
        }
        0x08 | 0x7F => {
            if state.len > 0 {
                state.len -= 1;
                state.composing[state.len] = 0;
            }
        }
        0x1B | 0x03 => super::clear_composition(state),
        b'1'..=b'9' => commit_into(state, usize::from(byte - b'1'), out),
        b' ' | b'\n' | b'\r' => commit_into(state, 0, out),
        _ if super::is_ascii_punctuation(byte) => {
            if state.len > 0 {
                commit_into(state, 0, out);
            }
            out.push_byte(byte);
        }
        _ => {}
    }
}

/// Commit the current composition (first matching candidate or raw bytes) into `out`.
fn commit_into(state: &mut super::ImeState, selector: usize, out: &mut Output) {
    if state.len == 0 {
        return;
    }
    let composing = &state.composing[..state.len];
    if let Some(entry) = super::lookup_candidate(composing) {
        let index = selector.min(entry.choices.len().saturating_sub(1));
        out.push_bytes(entry.choices[index].as_bytes());
    } else {
        out.push_bytes(composing);
    }
    super::clear_composition(state);
}
