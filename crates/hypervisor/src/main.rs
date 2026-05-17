#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;
mod ring3;

use bootloader::{entry_point, BootInfo};
use core::fmt::{self, Write};
use gos_protocol::{GraphNodeSummary, RuntimeNodeType, VectorAddress};

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

    // Phase I.3.1 — bring up the VGA mode-13h framebuffer (the
    // bootloader's `vga_320x200` feature switched the card before
    // entering `kernel_main`).  Clearing to Background immediately
    // gives any observer in QEMU a "kernel is alive" signal instead
    // of the leftover BIOS splash.
    unsafe {
        k_fb::init(boot_info.physical_memory_offset);
    }
    k_fb::clear(k_fb::Color::Background);
    // Header bar serves as the boot-progress indicator: dim teal
    // throughout init, becomes solid at "entering steady-state".
    k_fb::fill_rect(0, 0, k_fb::WIDTH, 18, k_fb::Color::HeaderBar);
    raw_serial_println(format_args!("boot: framebuffer up (mode 13h, 320x200)"));

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
    raw_serial_println(format_args!("boot: ring3 syscall surface armed"));

    // Phase I.3.2 — paint the kernel-node UI before going interactive.
    // Static one-shot for now; the I.3.x refresh hook will repaint on
    // graph-generation ticks once `gos_runtime::graph_generation()` is
    // wired into the boot loop.
    paint_boot_ui();
    raw_serial_println(format_args!("boot: framebuffer UI painted"));

    raw_serial_println(format_args!("boot: enabling interrupts; entering steady-state"));
    x86_64::instructions::interrupts::enable();

    loop {
        x86_64::instructions::interrupts::without_interrupts(|| {
            gos_supervisor::service_system_cycle();
        });
        x86_64::instructions::hlt();
    }
}

// ─── Phase I.3.2 — boot UI painter ──────────────────────────────────
//
// Draws the kernel-node tile grid into the mode-13h framebuffer.  No
// text yet (font glyphs are I.3.x); each node is a colour-coded tile
// classified by `RuntimeNodeType`, framed with a thin DimWhite outline
// so identical-classified tiles still read as discrete entities.  A
// bottom progress bar shows live-vs-discovered ratio.
//
// Layout (320×200, top-left origin):
//   y  0..18   header bar     (HeaderBar, painted earlier)
//   y 18..19   underline      (DimWhite)
//   y 22..180  tile grid      (8 cols × N rows, 36×16 tiles, 4px pad)
//   y 184..185 footer divider (DimWhite)
//   y 185..189 progress bar   (Highlight, width = returned/total)
//   y 192..199 status row     (reserved for I.3.x text)

const UI_GRID_TOP: usize = 22;
const UI_TILE_COLS: usize = 8;
const UI_TILE_W: usize = 36;
const UI_TILE_H: usize = 16;
const UI_TILE_PAD: usize = 4;
const UI_GRID_LEFT: usize = 8;
const UI_FOOTER_Y: usize = 184;
const UI_PROGRESS_Y: usize = 185;
const UI_PROGRESS_H: usize = 4;

fn paint_boot_ui() {
    if !k_fb::ready() {
        return;
    }

    // Reset everything except the header bar (it was painted at
    // framebuffer init and serves as a "boot in progress" cue
    // throughout the long supervisor + plugin bring-up).
    k_fb::fill_rect(
        0,
        18,
        k_fb::WIDTH,
        k_fb::HEIGHT - 18,
        k_fb::Color::Background,
    );
    k_fb::hline(0, 18, k_fb::WIDTH, k_fb::Color::DimWhite);

    let mut nodes = [GraphNodeSummary::EMPTY; 64];
    let (total, returned) = gos_runtime::node_page(0, &mut nodes);

    let max_visible = UI_TILE_COLS * ((k_fb::HEIGHT - UI_GRID_TOP - 20) / (UI_TILE_H + UI_TILE_PAD));
    let shown = returned.min(max_visible);
    for i in 0..shown {
        let col = i % UI_TILE_COLS;
        let row = i / UI_TILE_COLS;
        let x = UI_GRID_LEFT + col * (UI_TILE_W + UI_TILE_PAD);
        let y = UI_GRID_TOP + row * (UI_TILE_H + UI_TILE_PAD);
        let color = classify_node(&nodes[i]);
        k_fb::fill_rect(x, y, UI_TILE_W, UI_TILE_H, color);
        k_fb::stroke_rect(x, y, UI_TILE_W, UI_TILE_H, k_fb::Color::DimWhite);
        // 4-pixel "vector level" accent at the top-left corner —
        // distinguishes nodes from the same plugin without text.
        let accent = accent_for(nodes[i].vector);
        k_fb::fill_rect(x + 2, y + 2, 4, 4, accent);
    }

    // Footer divider + discovered-vs-installed progress.
    k_fb::hline(0, UI_FOOTER_Y, k_fb::WIDTH, k_fb::Color::DimWhite);
    let progress_w = if total == 0 {
        0
    } else {
        returned.saturating_mul(k_fb::WIDTH) / total
    };
    k_fb::fill_rect(0, UI_PROGRESS_Y, k_fb::WIDTH, UI_PROGRESS_H, k_fb::Color::BarEmpty);
    if progress_w > 0 {
        k_fb::fill_rect(0, UI_PROGRESS_Y, progress_w, UI_PROGRESS_H, k_fb::Color::Highlight);
    }

    k_fb::present();
}

fn classify_node(node: &GraphNodeSummary) -> k_fb::Color {
    match node.node_type {
        RuntimeNodeType::Hardware => k_fb::Color::NodeKernel,
        RuntimeNodeType::Driver => k_fb::Color::NodeDriver,
        RuntimeNodeType::Service => k_fb::Color::NodeService,
        RuntimeNodeType::PluginEntry | RuntimeNodeType::Compute => k_fb::Color::NodeApp,
        _ => k_fb::Color::NodeOther,
    }
}

fn accent_for(vector: VectorAddress) -> k_fb::Color {
    // Cheap XOR hash over the 4 vector levels so sibling nodes from
    // one plugin still read as distinct in the tile grid.
    let h = vector.l4 ^ ((vector.l3 & 0xFF) as u8) ^ ((vector.l2 & 0xFF) as u8) ^ ((vector.offset & 0xFF) as u8);
    match h & 0x7 {
        0 => k_fb::Color::Highlight,
        1 => k_fb::Color::NodeKernel,
        2 => k_fb::Color::NodeService,
        3 => k_fb::Color::NodeDriver,
        4 => k_fb::Color::NodeApp,
        5 => k_fb::Color::NodeOther,
        6 => k_fb::Color::DimWhite,
        _ => k_fb::Color::Foreground,
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
    // Visual cue if the framebuffer is up: solid crimson with a tiny
    // amber band so a passer-by in QEMU can tell the kernel halted
    // even without serial visibility.
    if k_fb::ready() {
        k_fb::clear(k_fb::Color::Error);
        k_fb::fill_rect(0, 0, k_fb::WIDTH, 8, k_fb::Color::Highlight);
    }
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
