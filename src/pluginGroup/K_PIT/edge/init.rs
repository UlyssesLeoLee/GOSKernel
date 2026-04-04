//! Edge: init
pub fn init() {
    // Actually the PIT starts firing as soon as interrupts are enabled.
    crate::serial_println!("[K_PIT]    online  System timer ready");
}
