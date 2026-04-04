//! Edge: init
use crate::pluginGroup::K_PS2::node::KEYBOARD;

pub fn init() {
    // Force lazy_static initialization before CPU interrupts are enabled
    let _ = KEYBOARD.lock();

    crate::serial_println!("[K_PS2]    online  Keyboard ready (buffer cleared)");
}
