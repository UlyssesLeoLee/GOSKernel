//! Edge: print
use core::fmt;
use crate::pluginGroup::K_SERIAL::node::SERIAL1;

#[doc(hidden)]
pub fn _serial_print(args: fmt::Arguments) {
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).unwrap();
    });
}
