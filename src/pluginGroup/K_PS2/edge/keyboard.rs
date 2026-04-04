//! Edge: keyboard
use pc_keyboard::DecodedKey;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;
use crate::print;
use crate::pluginGroup::K_PIC::{PICS, InterruptIndex};
use crate::pluginGroup::K_PS2::node::KEYBOARD;

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    // Read the scancode from PS/2 controller data port
    let scancode: u8 = unsafe { port.read() };
    
    // Print every scancode immediately! This guarantees we see hardware interrupts.
    crate::print!("(SC:{:02x})", scancode);

    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    // For now, just print it. Later, K_SHELL will intercept this.
                    print!("{}", character);
                }
                DecodedKey::RawKey(_key) => {
                    // For backspace, tab, arrows, etc.
                    print!("<{:?}>", _key);
                }
            }
        }
    }

    // Acknowledge the interrupt
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
