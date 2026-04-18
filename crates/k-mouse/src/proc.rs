// ── Main processing ───────────────────────────────────────────────────────────
// Responsibility: accumulate 3-byte PS/2 mouse packets and compute pointer motion.
// Emits a PointerUpdate when a complete, valid packet is received or on flush.

use core::sync::atomic::Ordering;
use gos_protocol::ExecutorContext;

/// A ready-to-emit pointer state update.
pub struct Output {
    pub col: u8,
    pub row: u8,
    pub visible: u8,
}

/// Process the input and produce a pointer update if ready.
pub unsafe fn process(ctx: *mut ExecutorContext, input: super::pre::Input) -> Option<Output> {
    match input {
        super::pre::Input::FlushMotion => {
            flush_motion(ctx)
        }
        super::pre::Input::PacketByte(byte) => {
            accumulate_byte(ctx, byte)
        }
    }
}

/// Flush accumulated deltas to the display immediately.
fn flush_motion(ctx: *mut ExecutorContext) -> Option<Output> {
    let state = unsafe { super::state_mut(ctx) };
    let dx = super::PENDING_DX.swap(0, Ordering::Relaxed);
    let dy = super::PENDING_DY.swap(0, Ordering::Relaxed);
    let buttons = super::PENDING_BUTTONS.load(Ordering::Relaxed);

    if dx == 0 && dy == 0 && buttons == state.buttons {
        return None;
    }

    apply_delta(state, dx, dy, buttons);
    Some(Output { col: state.col, row: state.row, visible: state.visible })
}

/// Accumulate a PS/2 packet byte.  Returns `Some` when a complete, valid
/// 3-byte packet has been assembled and deltas applied.
fn accumulate_byte(ctx: *mut ExecutorContext, byte: u8) -> Option<Output> {
    let slot = super::PACKET_INDEX.load(Ordering::Relaxed);
    match slot {
        0 => { super::PACKET0.store(byte, Ordering::Relaxed); super::PACKET_INDEX.store(1, Ordering::Relaxed); None }
        1 => { super::PACKET1.store(byte, Ordering::Relaxed); super::PACKET_INDEX.store(2, Ordering::Relaxed); None }
        _ => {
            super::PACKET2.store(byte, Ordering::Relaxed);
            super::PACKET_INDEX.store(0, Ordering::Relaxed);

            let p0 = super::PACKET0.load(Ordering::Relaxed);
            let p1 = super::PACKET1.load(Ordering::Relaxed);
            let p2 = super::PACKET2.load(Ordering::Relaxed);

            if (p0 & 0x08) != 0 && (p0 & 0xC0) == 0 {
                super::PENDING_DX.fetch_add((p1 as i8) as i32, Ordering::Relaxed);
                super::PENDING_DY.fetch_add((p2 as i8) as i32, Ordering::Relaxed);
                super::PENDING_BUTTONS.store(p0 & 0x07, Ordering::Relaxed);

                let dx = super::PENDING_DX.swap(0, Ordering::Relaxed);
                let dy = super::PENDING_DY.swap(0, Ordering::Relaxed);
                let buttons = super::PENDING_BUTTONS.load(Ordering::Relaxed);

                let state = unsafe { super::state_mut(ctx) };
                if dx != 0 || dy != 0 || buttons != state.buttons {
                    apply_delta(state, dx, dy, buttons);
                    return Some(Output { col: state.col, row: state.row, visible: state.visible });
                }
            }
            None
        }
    }
}

fn apply_delta(state: &mut super::MouseState, dx: i32, dy: i32, buttons: u8) {
    state.x_px = super::clamp(state.x_px + dx, 0, 639);
    state.y_px = super::clamp(state.y_px - dy, 0, 399);
    state.col = (state.x_px / 8) as u8;
    state.row = (state.y_px / 16) as u8;
    state.buttons = buttons;
    state.visible = 1;
}
