//! Edge: control
use crate::pluginGroup::K_VGA::node::{Color, WRITER};

pub fn clear_screen() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().clear();
    });
}

pub fn set_screen_color(fg: Color, bg: Color) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().set_color(fg, bg);
    });
}
