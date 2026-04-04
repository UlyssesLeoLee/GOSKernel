pub mod print;
pub mod init;
pub use print::*;
pub use init::*;

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::pluginGroup::K_SERIAL::edge::print::_serial_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    ()            => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
