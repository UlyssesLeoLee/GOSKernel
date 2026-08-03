#![no_std]

mod pre;
mod proc;
mod post;

// ============================================================
// GOS KERNEL TOPOLOGY — k-virtio-gpu
//
// ADR-013 §选项A — virtio-gpu discovery skeleton: PCI device discovery +
// BAR mapping + driver state machine, stopping at STAGE_BAR_READY (mirrors
// k-net's current virtio-net status: "discovered; datapath pending"). No
// virtqueue/2D command-queue setup, no wiring to k-vk-host/gos-hal::display
// (ADR-013's own gate) — those are follow-on slices. UEFI GOP real-hardware
// display is explicitly a *separate*, future ADR (bootloader migration),
// not this crate's concern.
//
// MERGE (p:Plugin {id: "K_VIRTIO_GPU", name: "k-virtio-gpu"})
// SET p.executor = "k_virtio_gpu::EXECUTOR_ID", p.node_type = "Driver", p.state_schema = "0x2021"
//
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
//
// MERGE (pr_CF8:PortRange {start: "0xCF8", end: "8"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_CF8)
//
// MERGE (cap_gpu_status:Capability {namespace: "gpu", name: "status"})
// MERGE (p)-[:EXPORTS]->(cap_gpu_status)
//
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
// ============================================================

use gos_protocol::{
    signal_to_packet, ExecStatus, ExecutorContext, ExecutorId, KernelAbi,
    NodeExecutorVTable, NodeEvent, Signal, VectorAddress,
};
use x86_64::instructions::port::Port;

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 6, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.gpu");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(gpu_on_init),
    on_event: Some(gpu_on_event),
    on_suspend: Some(gpu_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

const VGA_FALLBACK_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;
/// PCI class 0x03 = display controller (virtio spec §5.7.1: virtio-gpu
/// presents as class 0x03, subclass 0x80 "other"). Matched on class alone
/// (not subclass) -- same looseness k-net already uses for virtio-net
/// (vendor+class match, no device_id check), since QEMU's transitional vs.
/// modern virtio PCI transport picks different device ids (0x1010 vs
/// 0x1050) depending on `disable-modern`.
const PCI_CLASS_DISPLAY: u8 = 0x03;
const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

const DRIVER_NONE: u8 = 0;
const DRIVER_VIRTIO_GPU: u8 = 1;

const STAGE_EMPTY: u8 = 0;
const STAGE_PROBED: u8 = 1;
const STAGE_PCI_ENABLED: u8 = 2;
const STAGE_BAR_READY: u8 = 3;
const STAGE_UNSUPPORTED: u8 = 0xFF;
// Deliberately no STAGE_DEVICE_READY: that would mean the virtqueue /
// 2D command-queue is live, which ADR-013 §三 puts out of this MVP's gate.

const PCI_COMMAND_IO_SPACE: u16 = 1 << 0;
const PCI_COMMAND_MEMORY_SPACE: u16 = 1 << 1;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;
const PCI_COMMAND_WANTED: u16 =
    PCI_COMMAND_IO_SPACE | PCI_COMMAND_MEMORY_SPACE | PCI_COMMAND_BUS_MASTER;

#[repr(C)]
struct GpuState {
    console_target: u64,
    mmio_bar: u64,
    io_bar: u32,
    pci_command: u16,
    vendor_id: u16,
    device_id: u16,
    bus: u8,
    slot: u8,
    function: u8,
    class_code: u8,
    subclass: u8,
    revision: u8,
    irq_line: u8,
    irq_pin: u8,
    driver_kind: u8,
    stage: u8,
    gpu_present: u8,
    probe_complete: u8,
}

#[derive(Clone, Copy)]
struct ConsoleSink {
    target: u64,
    from: u64,
    abi: &'static KernelAbi,
}

impl ConsoleSink {
    fn emit(&self, signal: Signal) {
        if let Some(emit_signal) = self.abi.emit_signal {
            unsafe {
                let _ = emit_signal(self.target, signal_to_packet(signal));
            }
        }
    }
}

#[derive(Clone, Copy)]
struct PciProbeResult {
    bus: u8,
    slot: u8,
    function: u8,
    vendor_id: u16,
    device_id: u16,
    class_code: u8,
    subclass: u8,
    revision: u8,
    irq_line: u8,
    irq_pin: u8,
    mmio_bar: u64,
    io_bar: u32,
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut GpuState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut GpuState) }
}

fn sink_from_ctx(ctx: *mut ExecutorContext) -> ConsoleSink {
    let ctx_ref = unsafe { &*ctx };
    let abi = unsafe { &*ctx_ref.abi };
    let state = unsafe { state_mut(ctx) };
    ConsoleSink {
        target: if state.console_target == 0 { VGA_FALLBACK_VEC.as_u64() } else { state.console_target },
        from: ctx_ref.vector.as_u64(),
        abi,
    }
}

fn print_byte(sink: &ConsoleSink, byte: u8) {
    sink.emit(Signal::Data { from: sink.from, byte });
}

fn print_str(sink: &ConsoleSink, s: &str) {
    for byte in s.bytes() {
        print_byte(sink, byte);
    }
}

fn set_color(sink: &ConsoleSink, fg: u8, bg: u8) {
    sink.emit(Signal::Control { cmd: 1, val: fg });
    sink.emit(Signal::Control { cmd: 2, val: bg });
}

fn print_num_u64(sink: &ConsoleSink, mut value: u64) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        print_byte(sink, b'0');
        return;
    }
    while value > 0 {
        buf[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }
    while len > 0 {
        len -= 1;
        print_byte(sink, buf[len]);
    }
}

fn print_hex_u8(sink: &ConsoleSink, value: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    print_byte(sink, HEX[((value >> 4) & 0x0F) as usize]);
    print_byte(sink, HEX[(value & 0x0F) as usize]);
}

fn print_hex_u16(sink: &ConsoleSink, value: u16) {
    print_hex_u8(sink, (value >> 8) as u8);
    print_hex_u8(sink, value as u8);
}

fn print_hex_u32(sink: &ConsoleSink, value: u32) {
    print_hex_u16(sink, (value >> 16) as u16);
    print_hex_u16(sink, value as u16);
}

fn print_hex_u64(sink: &ConsoleSink, value: u64) {
    print_hex_u32(sink, (value >> 32) as u32);
    print_hex_u32(sink, value as u32);
}

fn stage_label(stage: u8) -> &'static str {
    match stage {
        STAGE_PROBED => "pci-probed",
        STAGE_PCI_ENABLED => "pci-enabled",
        STAGE_BAR_READY => "bar-ready",
        STAGE_UNSUPPORTED => "not-found",
        _ => "idle",
    }
}

// ── PCI config-space access (mirrors k-net's own, byte-for-byte — no
// shared crate for this since both are small, independent, and it isn't
// worth an extra dependency edge for ~40 lines) ─────────────────────────

fn pci_config_address(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000u32
        | (u32::from(bus) << 16)
        | (u32::from(slot) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xFC)
}

fn pci_config_read_dword(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    let mut addr_port = Port::<u32>::new(PCI_CONFIG_ADDR);
    let mut data_port = Port::<u32>::new(PCI_CONFIG_DATA);
    unsafe {
        addr_port.write(pci_config_address(bus, slot, function, offset));
        data_port.read()
    }
}

fn pci_config_write_dword(bus: u8, slot: u8, function: u8, offset: u8, value: u32) {
    let mut addr_port = Port::<u32>::new(PCI_CONFIG_ADDR);
    let mut data_port = Port::<u32>::new(PCI_CONFIG_DATA);
    unsafe {
        addr_port.write(pci_config_address(bus, slot, function, offset));
        data_port.write(value);
    }
}

fn pci_config_read_word(bus: u8, slot: u8, function: u8, offset: u8) -> u16 {
    let raw = pci_config_read_dword(bus, slot, function, offset);
    let shift = u32::from((offset & 0x02) * 8);
    ((raw >> shift) & 0xFFFF) as u16
}

fn pci_config_read_byte(bus: u8, slot: u8, function: u8, offset: u8) -> u8 {
    let raw = pci_config_read_dword(bus, slot, function, offset);
    let shift = u32::from((offset & 0x03) * 8);
    ((raw >> shift) & 0xFF) as u8
}

fn pci_config_write_word(bus: u8, slot: u8, function: u8, offset: u8, value: u16) {
    let aligned = offset & 0xFC;
    let shift = u32::from((offset & 0x02) * 8);
    let mask = !(0xFFFFu32 << shift);
    let current = pci_config_read_dword(bus, slot, function, aligned);
    let next = (current & mask) | ((u32::from(value)) << shift);
    pci_config_write_dword(bus, slot, function, aligned, next);
}

fn pci_enable_device(probe: &PciProbeResult) -> u16 {
    let command = pci_config_read_word(probe.bus, probe.slot, probe.function, 0x04);
    let next = command | PCI_COMMAND_WANTED;
    if next != command {
        pci_config_write_word(probe.bus, probe.slot, probe.function, 0x04, next);
    }
    pci_config_read_word(probe.bus, probe.slot, probe.function, 0x04)
}

fn parse_pci_bars(bus: u8, slot: u8, function: u8) -> (u64, u32) {
    let mut mmio_bar = 0u64;
    let mut io_bar = 0u32;
    let mut index = 0u8;
    while index < 6 {
        let offset = 0x10 + index * 4;
        let raw = pci_config_read_dword(bus, slot, function, offset);
        if raw == 0 || raw == 0xFFFF_FFFF {
            index += 1;
            continue;
        }

        if (raw & 0x1) != 0 {
            if io_bar == 0 {
                io_bar = raw & !0x3;
            }
        } else if mmio_bar == 0 {
            let mem_type = (raw >> 1) & 0x3;
            if mem_type == 0x2 && index < 5 {
                let hi = pci_config_read_dword(bus, slot, function, offset + 4);
                mmio_bar = ((u64::from(hi)) << 32) | (u64::from(raw) & !0xFu64);
                index += 1;
            } else {
                mmio_bar = u64::from(raw) & !0xFu64;
            }
        }

        index += 1;
    }
    (mmio_bar, io_bar)
}

/// Scan all PCI bus/slot/function combinations for a virtio display device
/// (class 0x03, vendor 0x1AF4) — same brute-force scan shape as k-net's
/// `probe_network_device`, filtered on a different class code.
fn probe_gpu_device() -> Option<PciProbeResult> {
    let mut bus = 0u16;
    while bus < 256 {
        let mut slot = 0u8;
        while slot < 32 {
            let mut function = 0u8;
            while function < 8 {
                let vendor_device = pci_config_read_dword(bus as u8, slot, function, 0x00);
                let vendor_id = (vendor_device & 0xFFFF) as u16;
                if vendor_id != 0xFFFF {
                    let device_id = (vendor_device >> 16) as u16;
                    let class_reg = pci_config_read_dword(bus as u8, slot, function, 0x08);
                    let class_code = (class_reg >> 24) as u8;
                    let subclass = (class_reg >> 16) as u8;
                    if class_code == PCI_CLASS_DISPLAY && vendor_id == VIRTIO_VENDOR_ID {
                        let (mmio_bar, io_bar) = parse_pci_bars(bus as u8, slot, function);
                        return Some(PciProbeResult {
                            bus: bus as u8,
                            slot,
                            function,
                            vendor_id,
                            device_id,
                            class_code,
                            subclass,
                            revision: pci_config_read_byte(bus as u8, slot, function, 0x08),
                            irq_line: pci_config_read_byte(bus as u8, slot, function, 0x3C),
                            irq_pin: pci_config_read_byte(bus as u8, slot, function, 0x3D),
                            mmio_bar,
                            io_bar,
                        });
                    }
                }

                if function == 0 {
                    let header_type = pci_config_read_byte(bus as u8, slot, function, 0x0E);
                    if (header_type & 0x80) == 0 {
                        break;
                    }
                }
                function += 1;
            }
            slot += 1;
        }
        bus += 1;
    }
    None
}

fn reset_state(state: &mut GpuState) {
    state.mmio_bar = 0;
    state.io_bar = 0;
    state.pci_command = 0;
    state.vendor_id = 0;
    state.device_id = 0;
    state.bus = 0;
    state.slot = 0;
    state.function = 0;
    state.class_code = 0;
    state.subclass = 0;
    state.revision = 0;
    state.irq_line = 0;
    state.irq_pin = 0;
    state.driver_kind = DRIVER_NONE;
    state.stage = STAGE_EMPTY;
    state.gpu_present = 0;
    state.probe_complete = 0;
}

/// The driver state machine's ceiling for this MVP: discover -> enable ->
/// map BARs -> STAGE_BAR_READY. No virtqueue negotiation, no display-info
/// query, no command-queue setup — those need a real datapath slice, out
/// of ADR-013's gate for this crate.
fn refresh_gpu_state(state: &mut GpuState) {
    reset_state(state);

    let Some(probe) = probe_gpu_device() else {
        state.probe_complete = 1;
        return;
    };

    state.gpu_present = 1;
    state.probe_complete = 1;
    state.bus = probe.bus;
    state.slot = probe.slot;
    state.function = probe.function;
    state.vendor_id = probe.vendor_id;
    state.device_id = probe.device_id;
    state.class_code = probe.class_code;
    state.subclass = probe.subclass;
    state.revision = probe.revision;
    state.irq_line = probe.irq_line;
    state.irq_pin = probe.irq_pin;
    state.driver_kind = DRIVER_VIRTIO_GPU;
    state.stage = STAGE_PROBED;

    state.pci_command = pci_enable_device(&probe);
    state.stage = STAGE_PCI_ENABLED;

    let (mmio_bar, io_bar) = parse_pci_bars(probe.bus, probe.slot, probe.function);
    state.mmio_bar = mmio_bar;
    state.io_bar = io_bar;
    state.stage = STAGE_BAR_READY;
}

fn print_probe_report(sink: &ConsoleSink, state: &GpuState, title: &str) {
    set_color(sink, 13, 0); // magenta
    print_str(sink, "\n[GPU] ");
    print_str(sink, title);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
    if state.gpu_present != 0 {
        print_str(sink, "      device: virtio-gpu (paravirtualized -- QEMU/cloud only, not real hardware)\n");
        print_str(sink, "      pci: ");
        print_hex_u8(sink, state.bus);
        print_byte(sink, b':');
        print_hex_u8(sink, state.slot);
        print_byte(sink, b'.');
        print_hex_u8(sink, state.function);
        print_str(sink, " vendor 0x");
        print_hex_u16(sink, state.vendor_id);
        print_str(sink, " device 0x");
        print_hex_u16(sink, state.device_id);
        print_str(sink, " rev 0x");
        print_hex_u8(sink, state.revision);
        print_str(sink, " irq ");
        print_num_u64(sink, state.irq_line as u64);
        print_str(sink, "\n      cmd 0x");
        print_hex_u16(sink, state.pci_command);
        print_str(sink, "  stage ");
        print_str(sink, stage_label(state.stage));
        print_str(sink, "\n      bar: mmio 0x");
        print_hex_u64(sink, state.mmio_bar);
        print_str(sink, "  io 0x");
        print_hex_u32(sink, state.io_bar);
        print_str(sink, "\n      datapath: not wired (ADR-013 MVP -- discovery + BAR mapping only)\n");
    } else {
        print_str(sink, "      no virtio-gpu detected on pci config space\n");
        print_str(sink, "      (expected on real hardware -- virtio-gpu is QEMU-only; try -device virtio-gpu-pci)\n");
    }
    print_str(sink, "\n");
}

// ── Node lifecycle callbacks ───────────────────────────────────────────────

unsafe extern "C" fn gpu_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let console_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"console".as_ptr(), b"console".len(),
                    b"write".as_ptr(), b"write".len(),
                )
            }
        } else {
            0
        }
    };

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut GpuState,
            GpuState {
                console_target: if console_target == 0 { VGA_FALLBACK_VEC.as_u64() } else { console_target },
                mmio_bar: 0,
                io_bar: 0,
                pci_command: 0,
                vendor_id: 0,
                device_id: 0,
                bus: 0,
                slot: 0,
                function: 0,
                class_code: 0,
                subclass: 0,
                revision: 0,
                irq_line: 0,
                irq_pin: 0,
                driver_kind: DRIVER_NONE,
                stage: STAGE_EMPTY,
                gpu_present: 0,
                probe_complete: 0,
            },
        );
    }

    ExecStatus::Done
}

unsafe extern "C" fn gpu_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let Some(input) = (unsafe { pre::prepare(event) }) else { return ExecStatus::Done; };
    let Some(output) = (unsafe { proc::process(ctx, input) }) else { return ExecStatus::Done; };
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn gpu_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
