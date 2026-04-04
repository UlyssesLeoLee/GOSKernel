//! Edge: init
use crate::pluginGroup::K_PIC::node::PICS;

pub fn init() {
    unsafe {
        PICS.lock().initialize();
        // Unmask ONLY Timer (IRQ0) and Keyboard (IRQ1) to prevent interrupt storms
        // 0xFC = 11111100b (Master), 0xFF = 11111111b (Slave)
        PICS.lock().write_masks(0xFC, 0xFF);
    }
    crate::serial_println!("[K_PIC]    online  8259A mapped to 32-47 and IRQ0/1 unmasked");
}
