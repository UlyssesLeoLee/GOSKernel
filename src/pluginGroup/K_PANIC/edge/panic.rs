//! Edge: panic
use core::panic::PanicInfo;
use crate::{println, serial_println};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use crate::pluginGroup::K_VGA::{Color, WRITER};
    WRITER.lock().set_color(Color::White, Color::Red);

    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║          !!! KERNEL PANIC !!!            ║");
    println!("╚══════════════════════════════════════════╝");
    println!("{}", info);

    serial_println!("[K_PANIC] {}", info);

    loop {
        x86_64::instructions::hlt();
    }
}
