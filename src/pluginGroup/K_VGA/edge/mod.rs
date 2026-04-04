pub mod print;
pub mod writer_ops;
pub mod init;
pub mod control;
pub use print::*;
pub use writer_ops::*;
pub use init::*;
pub use control::*;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::pluginGroup::K_VGA::edge::print::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    ()              => ($crate::print!("\n"));
    ($($arg:tt)*)   => ($crate::print!("{}\n", format_args!($($arg)*)));
}
