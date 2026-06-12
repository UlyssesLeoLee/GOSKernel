#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;
mod ring3;

use bootloader::{entry_point, BootInfo};
use core::fmt::{self, Write};
use gos_rewrite::boot::gos_kernel::{
    ACTIVATE_KERNEL_TIER, BUILTIN_GRAPH, CPU_FEATURES, DEPS, GDT_INIT, HAL_INIT, IDT_INIT,
    INSTALL_MODULES, NODES, PIC_INIT, PS2_DRAIN, REALIZE_MODULES, RING3, STEADY_STATE,
    SUPERVISOR_BOOTSTRAP,
};
use gos_rewrite::boot::{resolve_boot_order, BootNodeId};

entry_point!(kernel_main);

/// State threaded across boot steps (V2.2b wiring): replaces the local `let`
/// bindings of the old hardcoded sequence so each step can run independently
/// of source order.
struct BootContext {
    boot_info: &'static BootInfo,
    builtin_report: Option<builtin_bundle::BuiltinBootReport>,
}

type BootStep = fn(&mut BootContext);

/// Binds each [`gos_rewrite::boot::gos_kernel`] `BootNodeId` to its step
/// implementation. [`run_boot`] resolves the fire order from [`DEPS`]
/// (ADR-002 §3), so this table's *row order* is cosmetic — only [`DEPS`]
/// determines what runs before what.
const STEPS: [(BootNodeId, BootStep); 13] = [
    (CPU_FEATURES, step_cpu_features),
    (HAL_INIT, step_hal_init),
    (SUPERVISOR_BOOTSTRAP, step_supervisor_bootstrap),
    (INSTALL_MODULES, step_install_modules),
    (BUILTIN_GRAPH, step_builtin_graph),
    (REALIZE_MODULES, step_realize_modules),
    (GDT_INIT, step_gdt_init),
    (IDT_INIT, step_idt_init),
    (PIC_INIT, step_pic_init),
    (PS2_DRAIN, step_ps2_drain),
    (ACTIVATE_KERNEL_TIER, step_activate_kernel_tier),
    (RING3, step_ring3),
    (STEADY_STATE, step_steady_state),
];

fn step_cpu_features(_ctx: &mut BootContext) {
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
}

fn step_hal_init(ctx: &mut BootContext) {
    // Minimal bootstrap only owns compatibility addressing and metadata schemas.
    gos_hal::vaddr::init();
    gos_hal::meta::init();
    // Store the physical memory offset for DMA address translation in k-net and other drivers.
    gos_hal::phys::set_phys_offset(ctx.boot_info.physical_memory_offset);
    raw_serial_println(format_args!(
        "boot: vaddr/meta initialized, phys_offset={:#x}",
        ctx.boot_info.physical_memory_offset
    ));
}

fn step_supervisor_bootstrap(ctx: &mut BootContext) {
    raw_serial_println(format_args!("boot: staging supervisor domains"));
    gos_supervisor::bootstrap(ctx.boot_info as *const _ as u64);
}

fn step_install_modules(_ctx: &mut BootContext) {
    for descriptor in builtin_bundle::builtin_supervisor_modules() {
        gos_supervisor::install_module(*descriptor)
            .expect("supervisor failed to install module descriptor");
    }
    raw_serial_println(format_args!("boot: supervisor registered module descriptors"));
}

fn step_builtin_graph(ctx: &mut BootContext) {
    raw_serial_println(format_args!("boot: bootstrapping builtin graph"));
    let report = builtin_bundle::boot_builtin_graph(ctx.boot_info as *const _ as u64)
        .expect("builtin graph boot failed");
    raw_serial_println(format_args!("boot: builtin graph booted"));
    ctx.builtin_report = Some(report);
}

fn step_realize_modules(ctx: &mut BootContext) {
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

    let report = ctx
        .builtin_report
        .as_ref()
        .expect("BUILTIN_GRAPH must run before REALIZE_MODULES");
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
}

// Phase G.1 / V2.2d: kernel-tier hardware bring-up (GDT, IDT, PIC, PS/2
// drain, on_init activation) — formerly one hardcoded
// `init_kernel_tier_drivers` call, now five `Depend`-chained boot nodes
// (GDT_INIT -> IDT_INIT -> PIC_INIT -> PS2_DRAIN -> ACTIVATE_KERNEL_TIER).
// Builtin modules' on_init never ran via runtime pump because their
// ModuleEntry was None; hardware setup must happen on this direct path,
// before interrupts come up. The chain is still linear because the
// ordering IS the hardware constraint:
//   GDT before IDT  — IDT gate descriptors reference GDT segment selectors.
//   IDT before PIC  — an IRQ arriving before the IDT is loaded would jump
//                      into an unmapped vector and triple-fault.
//   PIC before PS2_DRAIN/ACTIVATE — draining/activating happens once the
//                      interrupt controller is in its final state, right
//                      before main.rs enables interrupts (STEADY_STATE).

fn step_gdt_init(_ctx: &mut BootContext) {
    // Write the GdtState into HAL_MATRIX, then lgdt + reload CS + load_tss.
    // `boot_init_gdt` combines what `gdt_on_init` and `init_gdt` would do
    // separately under the runtime-pump path that doesn't run pre-interrupts.
    raw_serial_println(format_args!("boot: kernel-tier GDT init"));
    k_gdt::boot_init_gdt();
}

fn step_idt_init(_ctx: &mut BootContext) {
    // k-idt::init_idt is self-contained: writes the IDT into HAL_MATRIX and
    // lidts it in one call. Its gate descriptors reference GDT selectors, so
    // this must run after GDT_INIT.
    raw_serial_println(format_args!("boot: kernel-tier IDT init"));
    unsafe {
        k_idt::init_idt();
    }
}

fn step_pic_init(_ctx: &mut BootContext) {
    // Initialise the 8259 chain and unmask Timer / Keyboard / Cascade /
    // Mouse. PIT was already programmed via pit_register_hook (120 Hz)
    // during the register_hooks pass.
    raw_serial_println(format_args!("boot: kernel-tier PIC init"));
    k_pic::init_pic();
}

fn step_ps2_drain(_ctx: &mut BootContext) {
    // BIOS/QEMU init leaves a stale byte in port 0x60 (observed IRQ1=1
    // right after boot); drain it so the i8042 controller can raise IRQ1 /
    // IRQ12 again on the next real keypress.
    builtin_bundle::drain_ps2_buffer();
    raw_serial_println(format_args!("boot: kernel-tier drivers ready (GDT/IDT/PIC)"));
}

fn step_activate_kernel_tier(_ctx: &mut BootContext) {
    // Activate every Kernel-tier node through the runtime so its on_init /
    // on_resume run synchronously. Without this, plugins like k-ps2 never
    // enter their Keyboard parse state and a keyboard IRQ would dereference
    // an uninitialized Ps2State.
    builtin_bundle::activate_kernel_tier_nodes();
}

fn step_ring3(_ctx: &mut BootContext) {
    // Phase E.2: program the syscall MSRs once the GDT is live.
    raw_serial_println(format_args!("boot: arming ring3 syscall surface"));
    unsafe {
        ring3::init();
    }
    raw_serial_println(format_args!("boot: ring3 syscall surface armed"));
}

fn step_steady_state(_ctx: &mut BootContext) {
    raw_serial_println(format_args!("boot: enabling interrupts; entering steady-state"));
    x86_64::instructions::interrupts::enable();
}

/// V2.2b wiring (ADR-002 §3): resolve the fire order from [`DEPS`] (a
/// `Depend` graph over [`NODES`]) and dispatch each step through it, instead
/// of hardcoding the call sequence as source order. [`DEPS`]/[`NODES`] are
/// shared with `host-tests/gos-rewrite-harness/tests/boot_order.rs`, which
/// proves shuffling `DEPS`' declaration order still resolves validly and that
/// a cyclic manifest is reported, not hung.
fn run_boot(ctx: &mut BootContext) {
    let order = resolve_boot_order(&NODES, &DEPS)
        .expect("boot manifest has a dependency cycle (ADR-002 §4): boot can never quiesce");
    for &id in order.as_slice() {
        let (_, step) = STEPS
            .iter()
            .find(|(node, _)| node.0 == id.0)
            .expect("resolved boot order references a step missing from STEPS");
        step(ctx);
    }
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    let mut ctx = BootContext {
        boot_info,
        builtin_report: None,
    };
    run_boot(&mut ctx);

    // Auto-live visual surface: throttle on the 120 Hz PIT so we re-check the
    // graph ~4x/s. k-vk-host re-emits the @gos.vk frame only when the structural
    // graph_epoch changed (idle graph -> just an epoch read). Runs OUTSIDE the
    // without_interrupts block so the slow UART emit never stalls IRQs; the
    // RUNTIME reads inside vk_auto_refresh bracket themselves. Manual `vk` still
    // forces a redraw via VK_CONTROL_REPORT.
    const VK_REFRESH_TICKS: usize = 30;
    let mut vk_last_tick = k_pit::get_ticks();

    loop {
        let report = x86_64::instructions::interrupts::without_interrupts(
            gos_supervisor::service_system_cycle,
        );
        if !report.quiesced {
            // ADR-002 §4: a non-quiescent cycle is reported, not silently
            // truncated — see gos_supervisor::CYCLE_DEPTH_CAP.
            raw_serial_println(format_args!(
                "service_system_cycle: causal-depth guard tripped at depth={} (steps={})",
                report.max_causal_depth, report.steps
            ));
        }
        let now = k_pit::get_ticks();
        if now.wrapping_sub(vk_last_tick) >= VK_REFRESH_TICKS {
            vk_last_tick = now;
            k_vk_host::vk_auto_refresh();
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
            unsafe {
                port.write(byte);
            }
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
