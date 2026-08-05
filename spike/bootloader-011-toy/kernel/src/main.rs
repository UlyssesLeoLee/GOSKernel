#![no_std]
#![no_main]

use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // ADR-018 spike: the only thing this proves is that BootInfo now
    // carries a framebuffer and that a raw pixel write reaches the
    // screen under both BiosBoot and UefiBoot images.
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        let info = fb.info();
        let buf = fb.buffer_mut();
        let bytes_per_pixel = info.bytes_per_pixel;
        for y in 0..info.height.min(64) {
            for x in 0..info.width.min(64) {
                let offset = y * info.stride * bytes_per_pixel + x * bytes_per_pixel;
                if offset + bytes_per_pixel <= buf.len() {
                    buf[offset] = 0x00; // B
                    buf[offset + 1] = 0xFF; // G
                    buf[offset + 2] = 0x00; // R
                }
            }
        }
    }

    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
