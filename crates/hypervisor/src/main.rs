#![no_std]
#![no_main]

extern crate alloc;

mod builtin_bundle;
mod ring3;

use bootloader::{entry_point, BootInfo};
use core::fmt::{self, Write};
use gos_protocol::{GraphNodeSummary, RuntimeNodeType};

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

    // I.3.5 — live repaint loop.  `gos_runtime::graph_generation()`
    // bumps on every Cypher mutation (H.1.x) and every internal edge
    // mutation; the framebuffer UI tracks it and re-paints only when
    // the graph actually changes.  Cheap atomic compare; idle ticks
    // pay nothing.
    let mut last_painted_gen = gos_runtime::graph_generation();
    loop {
        x86_64::instructions::interrupts::without_interrupts(|| {
            gos_supervisor::service_system_cycle();
        });
        let gen_now = gos_runtime::graph_generation();
        if gen_now != last_painted_gen {
            paint_boot_ui();
            last_painted_gen = gen_now;
        }
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

    let snapshot = gos_runtime::snapshot();
    let mut nodes = [GraphNodeSummary::EMPTY; 64];
    let (total, returned) = gos_runtime::node_page(0, &mut nodes);

    // Header: solid teal band + "GOS · N MOD · M NODES · K EDGES"
    // The leading dot-glyph is ASCII '.', not unicode middle dot —
    // BASIC_LEGACY only covers 7-bit ASCII.
    k_fb::fill_rect(0, 0, k_fb::WIDTH, 18, k_fb::Color::HeaderBar);
    let mut hdr = TextBuf::<40>::new();
    hdr.push_str("GOS  ");
    hdr.push_dec(returned as u64);
    hdr.push_str(" NOD  ");
    hdr.push_dec(snapshot.edge_count as u64);
    hdr.push_str(" EDG  G");
    hdr.push_dec(gos_runtime::graph_generation());
    k_fb::draw_text(4, 5, hdr.as_str(), k_fb::Color::Foreground);

    // Reset body region (everything below header) so a previous repaint
    // doesn't leave ghost text behind.
    k_fb::fill_rect(
        0,
        18,
        k_fb::WIDTH,
        k_fb::HEIGHT - 18,
        k_fb::Color::Background,
    );
    k_fb::hline(0, 18, k_fb::WIDTH, k_fb::Color::DimWhite);

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
        // First 4 chars of plugin_name (4 × 8 = 32 px fits inside the
        // 36-px tile with a 2 px inset both sides).  Drawn over the
        // tile body in Foreground; the per-tile classify_node fill
        // colour stays the dominant visual cue when reading at a
        // glance.
        let label = first_n_chars(nodes[i].plugin_name, 4);
        k_fb::draw_text(x + 2, y + 4, label, k_fb::Color::Foreground);
    }

    // Footer: divider + progress bar + status text.
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
    let mut footer = TextBuf::<40>::new();
    footer.push_str("READY  RDY=");
    footer.push_dec(snapshot.ready_queue_len as u64);
    footer.push_str(" SIG=");
    footer.push_dec(snapshot.signal_queue_len as u64);
    k_fb::draw_text(4, 192, footer.as_str(), k_fb::Color::Foreground);

    k_fb::present();
}

/// Tiny no_std string builder used by the framebuffer UI to format
/// counts inline.  Stack-only; truncates silently if the caller asks
/// to push past `N`.  Kept inline here rather than promoting to k-fb
/// because it's specific to the kernel-side panel layout.
struct TextBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> TextBuf<N> {
    fn new() -> Self {
        Self { buf: [0u8; N], len: 0 }
    }

    fn push_str(&mut self, s: &str) {
        for b in s.bytes() {
            if self.len >= N {
                break;
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
    }

    fn push_dec(&mut self, mut value: u64) {
        if value == 0 {
            self.push_str("0");
            return;
        }
        let mut digits = [0u8; 20];
        let mut n = 0;
        while value > 0 {
            digits[n] = b'0' + (value % 10) as u8;
            value /= 10;
            n += 1;
        }
        for i in (0..n).rev() {
            if self.len >= N {
                break;
            }
            self.buf[self.len] = digits[i];
            self.len += 1;
        }
    }

    fn as_str(&self) -> &str {
        // SAFETY: every byte we pushed was either ASCII from a `&str`
        // (which is UTF-8) or an ASCII digit, so the prefix is valid
        // UTF-8.
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

/// Return the first `n` characters of `s`.  Byte-truncates (the input
/// is always a kernel-supplied `&'static str` with ASCII-only
/// plugin/node names, so byte-boundary == char-boundary).
fn first_n_chars(s: &str, n: usize) -> &str {
    if s.len() <= n {
        s
    } else {
        &s[..n]
    }
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
