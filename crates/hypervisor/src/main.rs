#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;
mod fbtest;
mod kfont;
mod ring3;

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
    // Store the physical memory offset for DMA address translation in k-net and other drivers.
    gos_hal::phys::set_phys_offset(boot_info.physical_memory_offset);
    raw_serial_println(format_args!("boot: vaddr/meta initialized, phys_offset={:#x}", boot_info.physical_memory_offset));

    raw_serial_println(format_args!("boot: staging supervisor domains"));
    gos_supervisor::bootstrap(boot_info as *const _ as u64);
    for descriptor in builtin_bundle::builtin_supervisor_modules() {
        gos_supervisor::install_module(*descriptor)
            .expect("supervisor failed to install module descriptor");
    }
    raw_serial_println(format_args!("boot: supervisor registered module descriptors"));

    raw_serial_println(format_args!("boot: bootstrapping builtin graph"));
    let report = builtin_bundle::boot_builtin_graph(boot_info as *const _ as u64)
        .expect("builtin graph boot failed");
    raw_serial_println(format_args!("boot: builtin graph booted"));

    let supervisor_report = gos_supervisor::realize_boot_modules()
        .expect("supervisor failed to realize isolated domains");
    raw_serial_println(format_args!("boot: supervisor staged isolated domains"));

    k_serial::serial_println!(
        "supervisor modules={} running={} domains={} caps={}",
        supervisor_report.discovered_modules,
        supervisor_report.running_modules,
        supervisor_report.isolated_domains,
        supervisor_report.published_capabilities
    );

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

    // Phase G.1: synchronously initialize kernel-tier drivers (GDT,
    // IDT, PIC) before interrupts come up.  Builtin modules' on_init
    // never ran via runtime pump because their ModuleEntry was None;
    // hardware setup must happen on the direct path here.
    raw_serial_println(format_args!("boot: kernel-tier drivers init"));
    builtin_bundle::init_kernel_tier_drivers();
    raw_serial_println(format_args!("boot: kernel-tier drivers ready (GDT/IDT/PIC)"));

    // Phase E.2: program the syscall MSRs once the GDT is live.
    raw_serial_println(format_args!("boot: arming ring3 syscall surface"));
    unsafe { ring3::init(); }

    raw_serial_println(format_args!("boot: enabling interrupts; entering steady-state"));
    x86_64::instructions::interrupts::enable();

    // One-shot supervisor drain: clears the post-boot ready queue before the
    // render loop starts. service_system_cycle loops until dispatched==0 &&
    // !restarted (hard cap: 2048 iterations). Running it outside the loop keeps
    // the first frame latency bounded regardless of startup cascade depth.
    fbtest::init();
    x86_64::instructions::interrupts::without_interrupts(gos_supervisor::service_system_cycle);
    // Steady-state loop: per-frame supervisor cycle (delegates input + signals +
    // ready-work to gos_supervisor::service_system_cycle) then one render frame
    // then hlt. In steady state the supervisor queue is empty so the cycle exits
    // on the first iteration (dispatched==0).
    // RDTSC loop-level timing (svc=service_system_cycle, rf=render_frame) is
    // logged on the first few iterations and then every 60 — a permanent,
    // lightweight FPS/latency trace (see also fbtest's PERF/FBF logs).
    let mut loop_iter: u64 = 0;
    // Host-bridged GPU surface (Phase B): in addition to the in-guest software
    // desktop, emit the live graph as an `@gos.vk` display-list frame to COM3
    // (TCP:14445) so `gos-vk-viewer --live` can render it on the host GPU
    // (smooth, high-res, host-VRAM). Throttled on the 120 Hz PIT and gated on
    // graph_epoch inside vk_auto_refresh, so an idle graph costs ~one epoch
    // read. Runs OUTSIDE without_interrupts (the slow UART emit must not stall
    // IRQs). fbtest stays during the transition; once the viewer is solid the
    // in-guest renderer can be retired.
    const VK_REFRESH_TICKS: u64 = 30; // ~0.25s: emit promptly when the graph changes
    const VK_KEEPALIVE_TICKS: u64 = 120; // ~1s: full frame so a late viewer still gets the scene
    let mut vk_last_tick = k_pit::get_ticks() as u64;
    let mut vk_keepalive_tick = vk_last_tick;
    loop {
        let lt0 = unsafe { core::arch::x86_64::_rdtsc() };
        x86_64::instructions::interrupts::without_interrupts(gos_supervisor::service_system_cycle);
        let lt1 = unsafe { core::arch::x86_64::_rdtsc() };
        fbtest::render_frame();
        let lt2 = unsafe { core::arch::x86_64::_rdtsc() };
        let now = k_pit::get_ticks() as u64;
        if now.wrapping_sub(vk_last_tick) >= VK_REFRESH_TICKS {
            vk_last_tick = now;
            k_vk_host::vk_auto_refresh();
        }
        if now.wrapping_sub(vk_keepalive_tick) >= VK_KEEPALIVE_TICKS {
            vk_keepalive_tick = now;
            k_vk_host::vk_force_refresh();
        }
        // B3b: drain viewer→kernel input (COM3 RX) and feed it to the key queue.
        // Echo each byte to the boot serial (COM1) so the round-trip is
        // observable in terminal A. k_ps2::inject_byte mirrors the real PS/2
        // IRQ path (push into the desktop ring buffer + route to k-shell), so
        // viewer keystrokes drive the same graph CLI (theme switches, cypher
        // commands, ...) that a physical keyboard does.
        while let Some(b) = k_vk_host::vk_drain_input() {
            raw_serial_println(format_args!("vk-input: {:#04x}", b));
            k_ps2::inject_byte(b);
        }
        loop_iter = loop_iter.wrapping_add(1);
        // Print timing on iteration 1, 2, 3, then every 60 iterations.
        if loop_iter <= 3 || loop_iter % 60 == 0 {
            raw_serial_println(format_args!("LOOP #{} svc={}us rf={}us",
                loop_iter,
                lt1.wrapping_sub(lt0) / 3_000,
                lt2.wrapping_sub(lt1) / 3_000,
            ));
        }
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
