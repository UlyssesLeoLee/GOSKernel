//! Edge: init
use crate::pluginGroup::K_SERIAL::node::SERIAL1;

pub fn init() {
    let _ = &*SERIAL1;
    crate::serial_println!("[K_SERIAL] online  COM1 @ 0x3F8");
}
