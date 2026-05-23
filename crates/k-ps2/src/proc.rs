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
                // Phase I.5 — mirror printable + control ASCII into
                // the kernel-UI command-bar ring.  The shell still
                // receives the same byte via the conditional-route
                // table (Output::Ascii path below), so both consumers
                // see the keystroke.
                k_fb::push_typed_char(bytes[0]);
                Some(Output::Ascii(bytes[0]))
            } else {
                // Copy the multi-byte sequence into a fixed-size array.
                let mut arr = [0u8; 4];
                arr[..bytes.len()].copy_from_slice(bytes);
                Some(Output::Utf8(arr, bytes.len()))
            }
        }
        DecodedKey::RawKey(k) => {
            use core::sync::atomic::Ordering;
            // Phase I.3.9 — F-keys drive the boot-UI 3D camera.  No
            // shell route is emitted for these (the shell would just
            // ignore them anyway); the camera state lives in k-fb
            // atomics that `paint_3d_view` snapshots each frame.
            const YAW_STEP_MRAD: i32 = 80;   // ~4.6° per keypress
            const PITCH_STEP_MRAD: i32 = 80;
            const ZOOM_STEP_MM: i32 = 250;   // 0.25 world units
            match k {
                pc_keyboard::KeyCode::F1 => {
                    let cur = k_fb::CAMERA_AUTO_ROTATE.load(Ordering::Relaxed);
                    k_fb::CAMERA_AUTO_ROTATE.store(!cur, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F2 => {
                    k_fb::CAMERA_YAW_BIAS_MRAD.fetch_sub(YAW_STEP_MRAD, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F3 => {
                    k_fb::CAMERA_YAW_BIAS_MRAD.fetch_add(YAW_STEP_MRAD, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F4 => {
                    k_fb::CAMERA_PITCH_BIAS_MRAD.fetch_add(PITCH_STEP_MRAD, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F5 => {
                    k_fb::CAMERA_PITCH_BIAS_MRAD.fetch_sub(PITCH_STEP_MRAD, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F6 => {
                    // N.13 — Unity-style "F" frame-all.  Asks the
                    // painter to recompute the orbit radius from the
                    // current node-set bounding sphere on the next
                    // frame, then resets yaw/pitch to the canonical
                    // front quarter view.
                    k_fb::CAMERA_FRAME_REQUEST.store(true, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F7 => {
                    k_fb::CAMERA_RADIUS_MM.fetch_sub(ZOOM_STEP_MM, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F8 => {
                    k_fb::CAMERA_RADIUS_MM.fetch_add(ZOOM_STEP_MM, Ordering::Relaxed);
                    return None;
                }
                pc_keyboard::KeyCode::F9 => {
                    // Phase I.5 — toggle scrollback panel.
                    let cur = k_fb::UI_SCROLLBACK_EXPANDED.load(Ordering::Relaxed);
                    k_fb::UI_SCROLLBACK_EXPANDED.store(!cur, Ordering::Relaxed);
                    return None;
                }
                _ => {}
            }
            // Phase I.5 + I.8 — mirror control keys (Backspace / Esc
            // / arrows) into the UI ring as well, so the command bar
            // can edit / cancel / scroll history without going
            // through the shell.
            let mirror = match k {
                pc_keyboard::KeyCode::Backspace => Some(0x08u8),
                pc_keyboard::KeyCode::Escape => Some(0x1Bu8),
                pc_keyboard::KeyCode::ArrowUp => Some(INPUT_KEY_UP),
                pc_keyboard::KeyCode::ArrowDown => Some(INPUT_KEY_DOWN),
                _ => None,
            };
            if let Some(b) = mirror {
                k_fb::push_typed_char(b);
            }
            match k {
                pc_keyboard::KeyCode::Backspace => Some(Output::Ascii(0x08)),
                pc_keyboard::KeyCode::ArrowUp   => Some(Output::Ascii(INPUT_KEY_UP)),
                pc_keyboard::KeyCode::ArrowDown => Some(Output::Ascii(INPUT_KEY_DOWN)),
                pc_keyboard::KeyCode::PageUp    => Some(Output::Ascii(INPUT_KEY_PAGE_UP)),
                pc_keyboard::KeyCode::PageDown  => Some(Output::Ascii(INPUT_KEY_PAGE_DOWN)),
                pc_keyboard::KeyCode::Escape    => Some(Output::Ascii(0x1B)),
                _ => None,
            }
        }
    }
}
