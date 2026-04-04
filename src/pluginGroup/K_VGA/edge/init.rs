//! Edge: init
use crate::pluginGroup::K_VGA::node::WRITER;

pub fn init() {
    let _ = &*WRITER;
    crate::serial_println!("[K_VGA]    online  0xb8000  80x25");
}
