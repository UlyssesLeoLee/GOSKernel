// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: feed the raw scancode through the PS/2 keyboard state machine
// and produce a decoded key output ready for routing.

use pc_keyboard::{DecodedKey, Keyboard, ScancodeSet1, layouts};
use gos_protocol::{INPUT_KEY_DOWN, INPUT_KEY_UP, INPUT_KEY_PAGE_DOWN, INPUT_KEY_PAGE_UP};

/// The decoded output produced from a scancode.
pub enum Output {
    /// Single-byte ASCII or control code — forwarded via the fast conditional-route path.
    Ascii(u8),
    /// Multi-byte UTF-8 sequence (exotic layouts) — requires the direct-emit fallback.
    Utf8([u8; 4], usize),
}

/// Decode `input.scancode` through the PS/2 keyboard state machine.
/// Returns `None` if the scancode is incomplete (modifier keys, release events, etc.).
pub fn process(
    keyboard: &mut Keyboard<layouts::Us104Key, ScancodeSet1>,
    input: &super::pre::Input,
) -> Option<Output> {
    let Ok(Some(key_event)) = keyboard.add_byte(input.scancode) else {
        return None;
    };
    let Some(key) = keyboard.process_keyevent(key_event) else {
        return None;
    };

    match key {
        DecodedKey::Unicode(ch) => {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            let bytes = s.as_bytes();
            if bytes.len() == 1 {
                Some(Output::Ascii(bytes[0]))
            } else {
                // Copy the multi-byte sequence into a fixed-size array.
                let mut arr = [0u8; 4];
                arr[..bytes.len()].copy_from_slice(bytes);
                Some(Output::Utf8(arr, bytes.len()))
            }
        }
        DecodedKey::RawKey(k) => match k {
            pc_keyboard::KeyCode::Backspace => Some(Output::Ascii(0x08)),
            pc_keyboard::KeyCode::ArrowUp   => Some(Output::Ascii(INPUT_KEY_UP)),
            pc_keyboard::KeyCode::ArrowDown => Some(Output::Ascii(INPUT_KEY_DOWN)),
            pc_keyboard::KeyCode::PageUp    => Some(Output::Ascii(INPUT_KEY_PAGE_UP)),
            pc_keyboard::KeyCode::PageDown  => Some(Output::Ascii(INPUT_KEY_PAGE_DOWN)),
            pc_keyboard::KeyCode::Escape    => Some(Output::Ascii(0x1B)),
            _ => None,
        },
    }
}
