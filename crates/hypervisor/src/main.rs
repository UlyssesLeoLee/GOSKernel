#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;

use bootloader::{entry_point, BootInfo};
use core::fmt::{self, Write};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    raw_serial_println(format_args!("boot: kernel_main entered"));

    unsafe {
        use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

        let mut cr0 = Cr0::read();
        cr0.remove(Cr0Flags::EMULATE_COPROCESSOR);
        cr0.insert(Cr0Flags::MONITOR_COPROCESSOR);
        Cr0::write(cr0);

        let mut cr4 = Cr4::read();
        cr4.insert(Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE);
        Cr4::write(cr4);
    }
    raw_serial_println(format_args!("boot: cpu features enabled"));

    // Minimal bootstrap only owns compatibility addressing and metadata schemas.
    gos_hal::vaddr::init();
    gos_hal::meta::init();
    raw_serial_println(format_args!("boot: vaddr/meta initialized"));

    let mut ctx = gos_runtime::bootstrap_context(boot_info as *const _ as u64);
    raw_serial_println(format_args!("boot: loading builtin bundle"));
    let report = gos_loader::load_bundle(builtin_bundle::builtin_bundle(), &mut ctx)
        .expect("builtin boot bundle failed to load");
    raw_serial_println(format_args!("boot: bundle loaded"));

    k_serial::serial_println!("\n=== GOS v0.2 BUNDLE LOAD ===");
    k_serial::serial_println!(
        "plugins discovered={} loaded={} stable={}",
        report.discovered_plugins,
        report.loaded_plugins,
        report.stable_after_load
    );

    let snapshot = gos_runtime::snapshot();
    k_serial::serial_println!(
        "runtime nodes={} edges={} ready={} signals={}",
        snapshot.node_count,
        snapshot.edge_count,
        snapshot.ready_queue_len,
        snapshot.signal_queue_len
    );

    raw_serial_println(format_args!("boot: ai supervisor owns shell handoff"));
    gos_runtime::pump();

    x86_64::instructions::interrupts::enable();

    loop {
        x86_64::instructions::interrupts::without_interrupts(|| {
            gos_runtime::pump();
        });
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    raw_serial_println(format_args!("KERNEL PANIC"));
    if let Some(location) = info.location() {
        raw_serial_println(format_args!(
            "panic at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        ));
    }
    raw_serial_println(format_args!("{}", info));
    loop {
        x86_64::instructions::hlt();
    }
}

struct RawSerial;

impl Write for RawSerial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut port = x86_64::instructions::port::Port::<u8>::new(0x3F8);
        for byte in s.bytes() {
            unsafe { port.write(byte); }
        }
        Ok(())
    }
}

fn raw_serial_print(args: fmt::Arguments) {
    let _ = RawSerial.write_fmt(args);
}

fn raw_serial_println(args: fmt::Arguments) {
    raw_serial_print(format_args!("{}\n", args));
}
