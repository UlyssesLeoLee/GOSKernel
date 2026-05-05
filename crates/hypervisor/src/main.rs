#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;
mod ring3;

use bootloader::{entry_point, BootInfo};
use core::fmt::{self, Write};
use gos_log::{LogLevel, log};

entry_point!(kernel_main);

/// `extern "C"` serial write backend for gos-log — writes the message
/// bytes directly to COM1 followed by a newline.
unsafe extern "C" fn log_serial_write(
    _level: u8,
    _source: *const u8,
    msg: *const u8,
    len: u32,
) {
    let mut port = x86_64::instructions::port::Port::<u8>::new(0x3F8);
    let msg_bytes = unsafe { core::slice::from_raw_parts(msg, len as usize) };
    for &b in msg_bytes {
        unsafe { port.write(b); }
    }
    unsafe { port.write(b'\n'); }
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // Install gos-log serial backend so log!() calls populate both the
    // COM1 serial stream and the in-kernel ring buffer (for the shell's
    // `log` command).
    gos_log::install_serial_backend(gos_log::SerialBackend {
        write: log_serial_write,
    });
    gos_log::set_min_level(gos_log::LogLevel::Info);

    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "kernel_main entered");

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
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "cpu features enabled (FPU/SSE)");

    // Minimal bootstrap only owns compatibility addressing and metadata schemas.
    gos_hal::vaddr::init();
    gos_hal::meta::init();
    // Store the physical memory offset for DMA address translation in k-net and other drivers.
    gos_hal::phys::set_phys_offset(boot_info.physical_memory_offset);
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "vaddr/meta/phys initialized");

    gos_supervisor::bootstrap(boot_info as *const _ as u64);
    for descriptor in builtin_bundle::builtin_supervisor_modules() {
        gos_supervisor::install_module(*descriptor)
            .expect("supervisor failed to install module descriptor");
    }
    log!(LogLevel::Info, *b"SUPERVISOR\0\0\0\0\0\0", "module descriptors registered");

    let report = builtin_bundle::boot_builtin_graph(boot_info as *const _ as u64)
        .expect("builtin graph boot failed");
    log!(LogLevel::Info, *b"RUNTIME\0\0\0\0\0\0\0\0\0", "builtin graph booted: plugins={} loaded={} stable={}",
        report.discovered_plugins, report.loaded_plugins, report.stable_after_load);

    let supervisor_report = gos_supervisor::realize_boot_modules()
        .expect("supervisor failed to realize isolated domains");
    log!(LogLevel::Info, *b"SUPERVISOR\0\0\0\0\0\0", "boot realized: modules={} running={} domains={} caps={}",
        supervisor_report.discovered_modules, supervisor_report.running_modules,
        supervisor_report.isolated_domains, supervisor_report.published_capabilities);

    let snapshot = gos_runtime::snapshot();
    log!(LogLevel::Info, *b"RUNTIME\0\0\0\0\0\0\0\0\0", "graph ready: nodes={} edges={} ready={} signals={}",
        snapshot.node_count, snapshot.edge_count, snapshot.ready_queue_len, snapshot.signal_queue_len);

    // Phase G.1: synchronously initialize kernel-tier drivers (GDT,
    // IDT, PIC) before interrupts come up.  Builtin modules' on_init
    // never ran via runtime pump because their ModuleEntry was None;
    // hardware setup must happen on the direct path here.
    builtin_bundle::init_kernel_tier_drivers();
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "kernel-tier drivers ready (GDT/IDT/PIC)");

    // Phase E.2: program the syscall MSRs once the GDT is live.
    unsafe { ring3::init(); }
    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "ring3 syscall surface armed (STAR/LSTAR)");

    log!(LogLevel::Info, *b"BOOT\0\0\0\0\0\0\0\0\0\0\0\0", "interrupts enabled — steady-state loop");
    x86_64::instructions::interrupts::enable();

    loop {
        x86_64::instructions::interrupts::without_interrupts(|| {
            gos_supervisor::service_system_cycle();
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

pub(crate) fn raw_serial_println(args: fmt::Arguments) {
    raw_serial_print(format_args!("{}\n", args));
}
