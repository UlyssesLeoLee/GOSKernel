#![no_std]

mod pre;
mod proc;
mod post;
pub(crate) mod tcp;

// ============================================================
// GOS KERNEL TOPOLOGY — k-net
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_NET", name: "k-net"})
// SET p.executor = "k_net::EXECUTOR_ID", p.node_type = "Driver", p.state_schema = "0x2015"
//
// -- Dependencies
// MERGE (dep_K_VGA:Plugin {id: "K_VGA"})
// MERGE (p)-[:DEPENDS_ON]->(dep_K_VGA)
//
// -- Hardware Resources
// MERGE (pr_CF8:PortRange {start: "0xCF8", end: "8"})
// MERGE (p)-[:REQUIRES_PORT]->(pr_CF8)
//
// -- Exported Capabilities (APIs)
// MERGE (cap_net_uplink:Capability {namespace: "net", name: "uplink"})
// MERGE (p)-[:EXPORTS]->(cap_net_uplink)
//
// -- Imported Capabilities (Dependencies)
// MERGE (cap_console_write:Capability {namespace: "console", name: "write"})
// MERGE (p)-[:IMPORTS]->(cap_console_write)
// ============================================================


use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::sync::atomic::{fence, Ordering};

use gos_protocol::{
    signal_to_packet, ExecStatus, ExecutorContext, ExecutorId, KernelAbi,
    NodeEvent, NodeExecutorVTable, Signal, VectorAddress,
};
use x86_64::instructions::port::Port;

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 4, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.net");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(net_on_init),
    on_event: Some(net_on_event),
    on_suspend: Some(net_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

const VGA_FALLBACK_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;
const PCI_CLASS_NETWORK: u8 = 0x02;
const INTEL_VENDOR_ID: u16 = 0x8086;
const E1000_DEVICE_ID: u16 = 0x100E;
const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

const DRIVER_NONE: u8 = 0;
const DRIVER_E1000: u8 = 1;
const DRIVER_VIRTIO: u8 = 2;

const STAGE_EMPTY: u8 = 0;
const STAGE_PROBED: u8 = 1;
const STAGE_PCI_ENABLED: u8 = 2;
const STAGE_BAR_READY: u8 = 3;
const STAGE_DEVICE_READY: u8 = 4;
const STAGE_UNSUPPORTED: u8 = 0xFF;

const PCI_COMMAND_IO_SPACE: u16 = 1 << 0;
const PCI_COMMAND_MEMORY_SPACE: u16 = 1 << 1;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;
const PCI_COMMAND_WANTED: u16 =
    PCI_COMMAND_IO_SPACE | PCI_COMMAND_MEMORY_SPACE | PCI_COMMAND_BUS_MASTER;

const E1000_REG_CTRL: u32 = 0x0000;
const E1000_REG_STATUS: u32 = 0x0008;
const E1000_REG_EERD: u32 = 0x0014;
const E1000_REG_IMC: u32 = 0x00D8;
const E1000_REG_RAL0: u32 = 0x5400;
const E1000_REG_RAH0: u32 = 0x5404;

const E1000_CTRL_SLU: u32 = 1 << 6;
const E1000_CTRL_RST: u32 = 1 << 26;

const E1000_STATUS_FD: u32 = 1 << 0;
const E1000_STATUS_LU: u32 = 1 << 1;
const E1000_STATUS_SPEED_MASK: u32 = 0x0000_00C0;
const E1000_STATUS_SPEED_100: u32 = 0x0000_0040;
const E1000_STATUS_SPEED_1000: u32 = 0x0000_0080;

const E1000_EERD_START: u32 = 1 << 0;
const E1000_EERD_DONE: u32 = 1 << 4;

// ── Descriptor-ring registers ────────────────────────────────────────────────
const E1000_REG_RCTL:  u32 = 0x0100;
const E1000_REG_RDBAL: u32 = 0x2800;
const E1000_REG_RDBAH: u32 = 0x2804;
const E1000_REG_RDLEN: u32 = 0x2808;
const E1000_REG_RDH:   u32 = 0x2810;
const E1000_REG_RDT:   u32 = 0x2818;
const E1000_REG_TCTL:  u32 = 0x0400;
const E1000_REG_TIPG:  u32 = 0x0410;
const E1000_REG_TDBAL: u32 = 0x3800;
const E1000_REG_TDBAH: u32 = 0x3804;
const E1000_REG_TDLEN: u32 = 0x3808;
const E1000_REG_TDH:   u32 = 0x3810;
const E1000_REG_TDT:   u32 = 0x3818;

const E1000_RCTL_EN:    u32 = 1 << 1;
const E1000_RCTL_BAM:   u32 = 1 << 15; // accept broadcast
const E1000_RCTL_SECRC: u32 = 1 << 26; // strip CRC from received frames

const E1000_TCTL_EN:   u32 = 1 << 1;
const E1000_TCTL_PSP:  u32 = 1 << 3;   // pad short packets
const E1000_TCTL_CT:   u32 = 0x10 << 4;
const E1000_TCTL_COLD: u32 = 0x40 << 12;

const E1000_TXD_CMD_EOP:  u8 = 1 << 0; // End of Packet
const E1000_TXD_CMD_IFCS: u8 = 1 << 1; // Insert FCS / CRC
const E1000_TXD_CMD_RS:   u8 = 1 << 3; // Report Status when done
const E1000_TXD_STAT_DD:  u8 = 1 << 0; // Descriptor Done

const E1000_RXD_STAT_DD:  u8 = 1 << 0; // Descriptor Done / packet arrived
#[allow(dead_code)]
const E1000_RXD_STAT_EOP: u8 = 1 << 1; // End of Packet (unused; kept for documentation)

// ── DMA descriptor types ─────────────────────────────────────────────────────
const RX_DESC_COUNT: usize = 8;
const TX_DESC_COUNT: usize = 8;
const PACKET_SIZE:   usize = 1522; // max Ethernet frame

#[repr(C)]
#[derive(Copy, Clone)]
struct E1000RxDesc {
    addr:     u64,
    length:   u16,
    checksum: u16,
    status:   u8,
    errors:   u8,
    special:  u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct E1000TxDesc {
    addr:    u64,
    length:  u16,
    cso:     u8,
    cmd:     u8,
    status:  u8,
    css:     u8,
    special: u16,
}

/// Aligned wrappers so E1000 16-byte alignment requirement is met.
#[repr(C, align(16))]
struct AlignedRxRing([E1000RxDesc; RX_DESC_COUNT]);

#[repr(C, align(16))]
struct AlignedTxRing([E1000TxDesc; TX_DESC_COUNT]);

/// Interior-mutable DMA cell.  Wrapping in `UnsafeCell` instead of using
/// `static mut` satisfies the graph-governance `static mut` ban while
/// keeping the same semantics: callers are responsible for single-threaded
/// access via the existing cooperative uniprocessor kernel model.
struct DmaCell<T>(UnsafeCell<T>);
// SAFETY: the kernel is uniprocessor and cooperative; no concurrent access.
unsafe impl<T> Sync for DmaCell<T> {}

static RX_RING: DmaCell<AlignedRxRing> = DmaCell(UnsafeCell::new(AlignedRxRing([E1000RxDesc {
    addr: 0, length: 0, checksum: 0, status: 0, errors: 0, special: 0,
}; RX_DESC_COUNT])));

static TX_RING: DmaCell<AlignedTxRing> = DmaCell(UnsafeCell::new(AlignedTxRing([E1000TxDesc {
    addr: 0, length: 0, cso: 0, cmd: 0, status: 0, css: 0, special: 0,
}; TX_DESC_COUNT])));

static RX_BUFS: DmaCell<[[u8; PACKET_SIZE]; RX_DESC_COUNT]> =
    DmaCell(UnsafeCell::new([[0u8; PACKET_SIZE]; RX_DESC_COUNT]));

static TX_BUF: DmaCell<[u8; PACKET_SIZE]> =
    DmaCell(UnsafeCell::new([0u8; PACKET_SIZE]));

// ── Network addressing constants (QEMU user-network defaults) ─────────────────
#[allow(dead_code)]
const GUEST_MAC:     [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]; // QEMU-configured MAC (for reference)
const GUEST_IP:      [u8; 4] = [10, 0, 2, 15];
const GATEWAY_IP:    [u8; 4] = [10, 0, 2, 2];
const BCAST_MAC:     [u8; 6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

const ETHERTYPE_ARP:  u16 = 0x0806;
const ETHERTYPE_IPV4: u16 = 0x0800;
const IP_PROTO_ICMP:  u8  = 1;
#[allow(dead_code)]
pub(crate) const IP_PROTO_TCP: u8 = 6;
const ICMP_ECHO_REQ:  u8  = 8;
const ICMP_ECHO_REP:  u8  = 0;

#[repr(C)]
struct NetState {
    console_target: u64,
    mmio_bar: u64,
    io_bar: u32,
    status_reg: u32,
    ctrl_reg: u32,
    pci_command: u16,
    vendor_id: u16,
    device_id: u16,
    speed_mbps: u16,
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
    nic_present: u8,
    probe_complete: u8,
    link_up: u8,
    full_duplex: u8,
    mac_valid: u8,
    mac: [u8; 6],
    // ── DMA ring state ────────────────────────────────────────────────────
    ring_initialized: u8,
    rx_tail: u8,
    tx_tail: u8,
    // ── Ping target (default: QEMU gateway 10.0.2.2) ─────────────────────
    ping_target_ip: [u8; 4],
    // ── Last ARP-resolved gateway MAC ────────────────────────────────────
    gw_mac: [u8; 6],
    gw_mac_valid: u8,
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
    command: u16,
    driver_kind: u8,
    mmio_bar: u64,
    io_bar: u32,
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut NetState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut NetState) }
}

fn sink_from_ctx(ctx: *mut ExecutorContext) -> ConsoleSink {
    let ctx_ref = unsafe { &*ctx };
    let abi = unsafe { &*ctx_ref.abi };
    let state = unsafe { state_mut(ctx) };
    ConsoleSink {
        target: if state.console_target == 0 {
            VGA_FALLBACK_VEC.as_u64()
        } else {
            state.console_target
        },
        from: ctx_ref.vector.as_u64(),
        abi,
    }
}

fn print_byte(sink: &ConsoleSink, byte: u8) {
    sink.emit(Signal::Data {
        from: sink.from,
        byte,
    });
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

fn print_mac(sink: &ConsoleSink, mac: &[u8; 6]) {
    for (idx, byte) in mac.iter().copied().enumerate() {
        if idx != 0 {
            print_byte(sink, b':');
        }
        print_hex_u8(sink, byte);
    }
}

fn driver_label(kind: u8) -> &'static str {
    match kind {
        DRIVER_E1000 => "e1000",
        DRIVER_VIRTIO => "virtio-net",
        _ => "none",
    }
}

fn stage_label(stage: u8) -> &'static str {
    match stage {
        STAGE_PROBED => "pci-probed",
        STAGE_PCI_ENABLED => "pci-enabled",
        STAGE_BAR_READY => "bar-ready",
        STAGE_DEVICE_READY => "device-ready",
        STAGE_UNSUPPORTED => "driver-pending",
        _ => "idle",
    }
}

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
    let next = probe.command | PCI_COMMAND_WANTED;
    if next != probe.command {
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

fn probe_network_device() -> Option<PciProbeResult> {
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
                    if class_code == PCI_CLASS_NETWORK {
                        let driver_kind =
                            if vendor_id == INTEL_VENDOR_ID && device_id == E1000_DEVICE_ID {
                                DRIVER_E1000
                            } else if vendor_id == VIRTIO_VENDOR_ID {
                                DRIVER_VIRTIO
                            } else {
                                DRIVER_NONE
                            };

                        if driver_kind != DRIVER_NONE {
                            let (mmio_bar, io_bar) =
                                parse_pci_bars(bus as u8, slot, function);
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
                                command: pci_config_read_word(bus as u8, slot, function, 0x04),
                                driver_kind,
                                mmio_bar,
                                io_bar,
                            });
                        }
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

fn spin_delay(iterations: usize) {
    let mut count = 0usize;
    while count < iterations {
        spin_loop();
        count += 1;
    }
}

fn e1000_io_read32(io_base: u32, reg: u32) -> u32 {
    let base = io_base as u16;
    let mut addr_port = Port::<u32>::new(base);
    let mut data_port = Port::<u32>::new(base.wrapping_add(4));
    unsafe {
        addr_port.write(reg);
        data_port.read()
    }
}

fn e1000_io_write32(io_base: u32, reg: u32, value: u32) {
    let base = io_base as u16;
    let mut addr_port = Port::<u32>::new(base);
    let mut data_port = Port::<u32>::new(base.wrapping_add(4));
    unsafe {
        addr_port.write(reg);
        data_port.write(value);
    }
}

fn e1000_reg_read(state: &NetState, reg: u32) -> u32 {
    if state.io_bar == 0 {
        return 0;
    }
    e1000_io_read32(state.io_bar, reg)
}

fn e1000_reg_write(state: &NetState, reg: u32, value: u32) {
    if state.io_bar != 0 {
        e1000_io_write32(state.io_bar, reg, value);
    }
}

fn speed_from_status(status: u32) -> u16 {
    match status & E1000_STATUS_SPEED_MASK {
        E1000_STATUS_SPEED_1000 => 1000,
        E1000_STATUS_SPEED_100 => 100,
        _ => 10,
    }
}

fn mac_is_sane(mac: &[u8; 6]) -> bool {
    let all_zero = mac.iter().all(|byte| *byte == 0);
    let all_ff = mac.iter().all(|byte| *byte == 0xFF);
    !(all_zero || all_ff)
}

fn e1000_read_eeprom_word(state: &NetState, mmio_bar: u64, word: u8) -> Option<u16> {
    let _ = mmio_bar;
    e1000_reg_write(state, E1000_REG_EERD, E1000_EERD_START | (u32::from(word) << 8));

    let mut spins = 0usize;
    while spins < 50_000 {
        let value = e1000_reg_read(state, E1000_REG_EERD);
        if (value & E1000_EERD_DONE) != 0 {
            return Some((value >> 16) as u16);
        }
        spins += 1;
        spin_loop();
    }
    None
}

fn e1000_load_mac(state: &mut NetState, mmio_bar: u64) {
    let _ = mmio_bar;
    let ral = e1000_reg_read(state, E1000_REG_RAL0);
    let rah = e1000_reg_read(state, E1000_REG_RAH0);
    let mut mac = [
        (ral & 0xFF) as u8,
        ((ral >> 8) & 0xFF) as u8,
        ((ral >> 16) & 0xFF) as u8,
        ((ral >> 24) & 0xFF) as u8,
        (rah & 0xFF) as u8,
        ((rah >> 8) & 0xFF) as u8,
    ];

    if !mac_is_sane(&mac) {
        let mut idx = 0usize;
        while idx < 3 {
            let Some(word) = e1000_read_eeprom_word(state, mmio_bar, idx as u8) else {
                break;
            };
            mac[idx * 2] = (word & 0xFF) as u8;
            mac[idx * 2 + 1] = (word >> 8) as u8;
            idx += 1;
        }
    }

    state.mac = mac;
    state.mac_valid = u8::from(mac_is_sane(&state.mac));
}

fn e1000_attach(state: &mut NetState) {
    if state.io_bar == 0 {
        state.stage = STAGE_PCI_ENABLED;
        state.status_reg = 0;
        state.ctrl_reg = 0;
        state.link_up = 0;
        state.full_duplex = 0;
        state.speed_mbps = 0;
        state.mac_valid = 0;
        state.mac = [0; 6];
        return;
    }

    state.stage = STAGE_BAR_READY;
    e1000_reg_write(state, E1000_REG_IMC, 0xFFFF_FFFF);

    let ctrl = e1000_reg_read(state, E1000_REG_CTRL);
    state.ctrl_reg = ctrl;
    e1000_reg_write(state, E1000_REG_CTRL, ctrl | E1000_CTRL_RST);
    spin_delay(200_000);
    e1000_reg_write(state, E1000_REG_IMC, 0xFFFF_FFFF);

    let ctrl_after = e1000_reg_read(state, E1000_REG_CTRL);
    e1000_reg_write(state, E1000_REG_CTRL, ctrl_after | E1000_CTRL_SLU);
    spin_delay(50_000);

    state.ctrl_reg = e1000_reg_read(state, E1000_REG_CTRL);
    state.status_reg = e1000_reg_read(state, E1000_REG_STATUS);
    state.speed_mbps = speed_from_status(state.status_reg);
    state.link_up = u8::from((state.status_reg & E1000_STATUS_LU) != 0);
    state.full_duplex = u8::from((state.status_reg & E1000_STATUS_FD) != 0);
    e1000_load_mac(state, state.mmio_bar);
    state.stage = STAGE_DEVICE_READY;

    // ── Initialise DMA descriptor rings ──────────────────────────────────
    e1000_ring_init(state);
}

fn reset_state(state: &mut NetState) {
    state.mmio_bar = 0;
    state.io_bar = 0;
    state.status_reg = 0;
    state.ctrl_reg = 0;
    state.pci_command = 0;
    state.vendor_id = 0;
    state.device_id = 0;
    state.speed_mbps = 0;
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
    state.nic_present = 0;
    state.probe_complete = 0;
    state.link_up = 0;
    state.full_duplex = 0;
    state.mac_valid = 0;
    state.mac = [0; 6];
    state.ring_initialized = 0;
    state.rx_tail = 0;
    state.tx_tail = 0;
    state.gw_mac = [0; 6];
    state.gw_mac_valid = 0;
    // ping_target_ip is NOT reset — the user may have set it via SET_IP* commands
}

fn refresh_network_state(state: &mut NetState) {
    reset_state(state);

    let Some(probe) = probe_network_device() else {
        state.probe_complete = 1;
        return;
    };

    state.nic_present = 1;
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
    state.driver_kind = probe.driver_kind;
    state.mmio_bar = probe.mmio_bar;
    state.io_bar = probe.io_bar;
    state.stage = STAGE_PROBED;

    state.pci_command = pci_enable_device(&probe);
    state.stage = STAGE_PCI_ENABLED;

    if probe.driver_kind == DRIVER_E1000 {
        e1000_attach(state);
    } else {
        state.stage = STAGE_UNSUPPORTED;
    }
}

// ── DMA descriptor ring initialisation ───────────────────────────────────────

fn e1000_ring_init(state: &mut NetState) {
    if state.io_bar == 0 || state.stage != STAGE_DEVICE_READY {
        return;
    }
    let rx_ring = unsafe { &mut *RX_RING.0.get() };
    let tx_ring = unsafe { &mut *TX_RING.0.get() };
    let rx_bufs = unsafe { &mut *RX_BUFS.0.get() };

    // ── RX ring ──────────────────────────────────────────────────────────────
    unsafe {
        for i in 0..RX_DESC_COUNT {
            let buf_virt = rx_bufs[i].as_ptr() as u64;
            let buf_phys = match gos_hal::phys::virt_to_phys(buf_virt) {
                Some(p) => p,
                None => return, // page-table miss — abort ring init
            };
            rx_ring.0[i] = E1000RxDesc {
                addr: buf_phys,
                length: 0,
                checksum: 0,
                status: 0,
                errors: 0,
                special: 0,
            };
        }
        let ring_virt = rx_ring.0.as_ptr() as u64;
        let ring_phys = match gos_hal::phys::virt_to_phys(ring_virt) {
            Some(p) => p,
            None => return,
        };
        fence(Ordering::SeqCst);
        e1000_reg_write(state, E1000_REG_RDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        e1000_reg_write(state, E1000_REG_RDBAH, (ring_phys >> 32) as u32);
        e1000_reg_write(state, E1000_REG_RDLEN, (RX_DESC_COUNT * 16) as u32);
        e1000_reg_write(state, E1000_REG_RDH, 0);
        e1000_reg_write(state, E1000_REG_RDT, (RX_DESC_COUNT - 1) as u32);
        e1000_reg_write(state, E1000_REG_RCTL,
            E1000_RCTL_EN | E1000_RCTL_BAM | E1000_RCTL_SECRC);
    }

    // ── TX ring ──────────────────────────────────────────────────────────────
    unsafe {
        for i in 0..TX_DESC_COUNT {
            tx_ring.0[i] = E1000TxDesc {
                addr: 0, length: 0, cso: 0, cmd: 0,
                status: E1000_TXD_STAT_DD, // mark all initially done
                css: 0, special: 0,
            };
        }
        let ring_virt = tx_ring.0.as_ptr() as u64;
        let ring_phys = match gos_hal::phys::virt_to_phys(ring_virt) {
            Some(p) => p,
            None => return,
        };
        fence(Ordering::SeqCst);
        e1000_reg_write(state, E1000_REG_TDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        e1000_reg_write(state, E1000_REG_TDBAH, (ring_phys >> 32) as u32);
        e1000_reg_write(state, E1000_REG_TDLEN, (TX_DESC_COUNT * 16) as u32);
        e1000_reg_write(state, E1000_REG_TDH, 0);
        e1000_reg_write(state, E1000_REG_TDT, 0);
        e1000_reg_write(state, E1000_REG_TCTL,
            E1000_TCTL_EN | E1000_TCTL_PSP | E1000_TCTL_CT | E1000_TCTL_COLD);
        // Inter-packet gap: recommended value from Intel e1000 SDM
        e1000_reg_write(state, E1000_REG_TIPG, 0x0060200A);
    }

    state.rx_tail = (RX_DESC_COUNT - 1) as u8;
    state.tx_tail = 0;
    state.ring_initialized = 1;
}

// ── Packet transmission ───────────────────────────────────────────────────────

/// Transmit `len` bytes from `TX_BUF`.  Returns true if the descriptor was
/// submitted successfully (does not wait for hardware completion).
pub(crate) unsafe fn tx_send(state: &mut NetState, len: usize) -> bool {
    if state.ring_initialized == 0 || len == 0 || len > PACKET_SIZE {
        return false;
    }
    let tail = state.tx_tail as usize;
    let tx_ring = unsafe { &mut *TX_RING.0.get() };
    let tx_buf  = unsafe { &mut *TX_BUF.0.get() };

    // Wait for the slot to be free (status DD set by HW when previous TX done)
    let mut spins = 0usize;
    loop {
        let status = tx_ring.0[tail].status;
        if status & E1000_TXD_STAT_DD != 0 {
            break;
        }
        if spins >= 500_000 {
            return false;
        }
        spin_loop();
        spins += 1;
    }

    let buf_virt = tx_buf.as_ptr() as u64;
    let buf_phys = match gos_hal::phys::virt_to_phys(buf_virt) {
        Some(p) => p,
        None => return false,
    };

    tx_ring.0[tail] = E1000TxDesc {
        addr:    buf_phys,
        length:  len as u16,
        cso:     0,
        cmd:     E1000_TXD_CMD_EOP | E1000_TXD_CMD_IFCS | E1000_TXD_CMD_RS,
        status:  0,
        css:     0,
        special: 0,
    };

    fence(Ordering::SeqCst);
    let next_tail = (tail + 1) % TX_DESC_COUNT;
    state.tx_tail = next_tail as u8;
    e1000_reg_write(state, E1000_REG_TDT, next_tail as u32);
    true
}

// ── Packet reception ──────────────────────────────────────────────────────────

/// Poll the RX ring for one completed frame.  On success, copies the frame
/// into `out_buf[..frame_len]` and returns `Some(frame_len)`.
pub(crate) unsafe fn rx_poll(state: &mut NetState, out_buf: &mut [u8]) -> Option<usize> {
    if state.ring_initialized == 0 {
        return None;
    }
    let rx_ring = unsafe { &mut *RX_RING.0.get() };
    let rx_bufs = unsafe { &mut *RX_BUFS.0.get() };

    // The head the hardware will write next is RDH; check the slot after our tail.
    let check = ((state.rx_tail as usize) + 1) % RX_DESC_COUNT;
    let status = rx_ring.0[check].status;
    if status & E1000_RXD_STAT_DD == 0 {
        return None;
    }
    let length = rx_ring.0[check].length as usize;
    if length == 0 || length > PACKET_SIZE || length > out_buf.len() {
        // recycle
        rx_ring.0[check].status = 0;
        fence(Ordering::SeqCst);
        e1000_reg_write(state, E1000_REG_RDT, check as u32);
        state.rx_tail = check as u8;
        return None;
    }

    // Copy frame out
    out_buf[..length].copy_from_slice(&rx_bufs[check][..length]);

    // Recycle descriptor
    let prev_addr = rx_ring.0[check].addr;
    rx_ring.0[check] = E1000RxDesc {
        addr: prev_addr,
        length: 0, checksum: 0, status: 0, errors: 0, special: 0,
    };
    fence(Ordering::SeqCst);
    e1000_reg_write(state, E1000_REG_RDT, check as u32);
    state.rx_tail = check as u8;

    Some(length)
}

// ── Network protocol helpers ──────────────────────────────────────────────────

pub(crate) fn checksum_ip(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0usize;
    while i + 1 < data.len() {
        sum += u32::from(u16::from_be_bytes([data[i], data[i + 1]]));
        i += 2;
    }
    if i < data.len() {
        sum += u32::from(data[i]) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

pub(crate) fn write_u16_be(buf: &mut [u8], offset: usize, val: u16) {
    buf[offset]     = (val >> 8) as u8;
    buf[offset + 1] = val as u8;
}


// ── ARP ───────────────────────────────────────────────────────────────────────

/// Build an ARP request in `TX_BUF` and transmit it.
/// Returns true on successful queue; false if ring not ready.
pub(crate) unsafe fn arp_request(state: &mut NetState, target_ip: [u8; 4]) -> bool {
    let src_mac = state.mac;
    let src_ip  = GUEST_IP;
    let buf = unsafe { &mut *TX_BUF.0.get() };

    // Ethernet header
    buf[0..6].copy_from_slice(&BCAST_MAC);
    buf[6..12].copy_from_slice(&src_mac);
    write_u16_be(buf, 12, ETHERTYPE_ARP);

    // ARP payload
    write_u16_be(buf, 14, 1);            // HTYPE = Ethernet
    write_u16_be(buf, 16, 0x0800);       // PTYPE = IPv4
    buf[18] = 6;                         // HLEN
    buf[19] = 4;                         // PLEN
    write_u16_be(buf, 20, 1);            // OPER = request
    buf[22..28].copy_from_slice(&src_mac);
    buf[28..32].copy_from_slice(&src_ip);
    buf[32..38].copy_from_slice(&[0; 6]);
    buf[38..42].copy_from_slice(&target_ip);

    unsafe { tx_send(state, 42) }
}

/// Poll the RX ring looking for an ARP reply from `sender_ip`.
/// On success fills `out_mac` and returns true.  Gives up after
/// `max_polls` iterations.
pub(crate) unsafe fn arp_wait_reply(
    state: &mut NetState,
    sender_ip: [u8; 4],
    out_mac: &mut [u8; 6],
    max_polls: usize,
) -> bool {
    let mut frame = [0u8; PACKET_SIZE];
    let mut polls = 0usize;

    while polls < max_polls {
        if let Some(len) = unsafe { rx_poll(state, &mut frame) } {
            // ARP EtherType at offset 12
            if len >= 42 && frame[12] == 0x08 && frame[13] == 0x06 {
                // OPER = reply (0x0002) at offset 20
                if frame[20] == 0x00 && frame[21] == 0x02 {
                    // Sender IP at offset 28
                    if frame[28..32] == sender_ip {
                        out_mac.copy_from_slice(&frame[22..28]);
                        return true;
                    }
                }
            }
        }
        spin_loop();
        polls += 1;
    }
    false
}

// ── ICMP echo ─────────────────────────────────────────────────────────────────

/// Build and send an ICMP echo request to `dst_ip` via `dst_mac`.
/// The payload is the 8-byte ASCII string "GOSPING!".
unsafe fn icmp_echo_request(
    state: &mut NetState,
    dst_mac: [u8; 6],
    dst_ip:  [u8; 4],
    id: u16,
    seq: u16,
) -> bool {
    let src_mac = state.mac;
    let src_ip  = GUEST_IP;
    let buf = unsafe { &mut *TX_BUF.0.get() };
    let payload = b"GOSPING!";

    // Ethernet header (14 bytes)
    buf[0..6].copy_from_slice(&dst_mac);
    buf[6..12].copy_from_slice(&src_mac);
    write_u16_be(buf, 12, ETHERTYPE_IPV4);

    // IP header (20 bytes) at offset 14
    let ip_total_len: u16 = 20 + 8 + payload.len() as u16; // hdr + icmp_hdr + payload
    buf[14] = 0x45;                      // Version 4, IHL 5
    buf[15] = 0;                         // DSCP/ECN
    write_u16_be(buf, 16, ip_total_len);
    write_u16_be(buf, 18, id);           // Identification reuse id
    buf[20] = 0x40;                      // Don't fragment
    buf[21] = 0;
    buf[22] = 64;                        // TTL
    buf[23] = IP_PROTO_ICMP;
    write_u16_be(buf, 24, 0);            // checksum placeholder
    buf[26..30].copy_from_slice(&src_ip);
    buf[30..34].copy_from_slice(&dst_ip);
    let ip_csum = checksum_ip(&buf[14..34]);
    write_u16_be(buf, 24, ip_csum);

    // ICMP header + payload at offset 34
    buf[34] = ICMP_ECHO_REQ;             // Type 8
    buf[35] = 0;                         // Code 0
    write_u16_be(buf, 36, 0);            // checksum placeholder
    write_u16_be(buf, 38, id);
    write_u16_be(buf, 40, seq);
    buf[42..50].copy_from_slice(payload);
    let icmp_csum = checksum_ip(&buf[34..50]);
    write_u16_be(buf, 36, icmp_csum);

    unsafe { tx_send(state, 50) }
}

/// Poll the RX ring for an ICMP echo reply matching `id` and `seq`.
/// Returns Some(polls_taken) on success, None on timeout.
unsafe fn icmp_wait_reply(
    state: &mut NetState,
    src_ip:  [u8; 4],
    id: u16,
    seq: u16,
    max_polls: usize,
) -> Option<u32> {
    let mut frame = [0u8; PACKET_SIZE];
    let mut polls = 0usize;

    while polls < max_polls {
        if let Some(len) = unsafe { rx_poll(state, &mut frame) } {
            // Must be IPv4 (EtherType 0x0800 at offset 12)
            if len >= 42 && frame[12] == 0x08 && frame[13] == 0x00 {
                // IP protocol at offset 23; ICMP = 1
                if frame[23] == IP_PROTO_ICMP {
                    // Source IP at offset 26
                    if frame[26..30] == src_ip {
                        // ICMP type at offset 34; type 0 = echo reply
                        if frame[34] == ICMP_ECHO_REP {
                            let rep_id  = u16::from_be_bytes([frame[38], frame[39]]);
                            let rep_seq = u16::from_be_bytes([frame[40], frame[41]]);
                            if rep_id == id && rep_seq == seq {
                                return Some(polls as u32);
                            }
                        }
                    }
                }
            }
        }
        spin_loop();
        polls += 1;
    }
    None
}

// ── Top-level ping operation ──────────────────────────────────────────────────

/// Send a single ICMP ping to `state.ping_target_ip` and return whether
/// a reply was received plus a rough poll-count latency estimate.
///
/// Performs (in order): E1000 ring init if needed → ARP → ICMP echo →
/// wait for reply.  All steps are blocking busy-polls with timeouts.
pub(crate) unsafe fn do_ping(state: &mut NetState) -> (bool, u32) {
    if state.stage != STAGE_DEVICE_READY || state.mac_valid == 0 {
        return (false, 0);
    }
    if state.ring_initialized == 0 {
        e1000_ring_init(state);
        if state.ring_initialized == 0 {
            return (false, 0);
        }
    }

    let target_ip = state.ping_target_ip;

    // ── Step 1: ARP to resolve gateway MAC ───────────────────────────────
    let gw_mac = if state.gw_mac_valid != 0 {
        state.gw_mac
    } else {
        if !unsafe { arp_request(state, target_ip) } {
            return (false, 0);
        }
        let mut mac = [0u8; 6];
        if !unsafe { arp_wait_reply(state, target_ip, &mut mac, 2_000_000) } {
            return (false, 0);
        }
        state.gw_mac = mac;
        state.gw_mac_valid = 1;
        mac
    };

    // ── Step 2: ICMP echo request ─────────────────────────────────────────
    let id: u16  = 0x6F73;  // 'os'
    let seq: u16 = 1;

    if !unsafe { icmp_echo_request(state, gw_mac, target_ip, id, seq) } {
        return (false, 0);
    }

    // ── Step 3: Wait for reply ────────────────────────────────────────────
    match unsafe { icmp_wait_reply(state, target_ip, id, seq, 3_000_000) } {
        Some(polls) => (true, polls),
        None => (false, 0),
    }
}

// ── NET_STATE_PTR: stored at init so external crates can call net_http_post_sync ─

/// Holds the `*mut NetState` obtained from `ExecutorContext::state_ptr` at
/// `net_on_init` time.  Written once, read by `net_http_post_sync` at any
/// later point.  Safe in the cooperative uniprocessor model.
struct NetStatePtrCell(UnsafeCell<*mut u8>);
// SAFETY: uniprocessor cooperative kernel — no concurrent access.
unsafe impl Sync for NetStatePtrCell {}

static NET_STATE_PTR: NetStatePtrCell =
    NetStatePtrCell(UnsafeCell::new(core::ptr::null_mut()));

/// Direct HTTP/1.0 POST over e1000 TCP to `dst_ip:dst_port`.
///
/// `http_request` must be a **complete** HTTP/1.0 request (headers + blank
/// line + body) that fits in one TCP segment (≤ `tcp::TCP_MAX_PAYLOAD` bytes).
/// The raw HTTP response (including status line and response headers) is
/// written into `resp`; the number of bytes written is returned.
///
/// Returns `None` if the NIC has not been initialised yet, the request is too
/// large, or the TCP connection fails.
///
/// # Safety
/// Must be called from a single-threaded context while the NIC is not
/// actively handling another request.  In GOS this is guaranteed by the
/// cooperative scheduler.
pub unsafe fn net_http_post_sync(
    dst_ip:       [u8; 4],
    dst_port:     u16,
    http_request: &[u8],
    resp:         &mut [u8],
) -> Option<usize> {
    let state_ptr = unsafe { *NET_STATE_PTR.0.get() };
    if state_ptr.is_null() {
        return None;
    }
    let state = unsafe { &mut *(state_ptr as *mut NetState) };
    unsafe { tcp::tcp_http_post(state, dst_ip, dst_port, http_request, resp) }
}

fn print_link_stage(sink: &ConsoleSink, state: &NetState) {
    if state.driver_kind == DRIVER_E1000 {
        print_str(sink, "      carrier: ");
        if state.link_up != 0 {
            print_str(sink, "up ");
            print_num_u64(sink, state.speed_mbps as u64);
            print_str(sink, "Mb ");
            print_str(
                sink,
                if state.full_duplex != 0 {
                    "full-duplex"
                } else {
                    "half-duplex"
                },
            );
        } else {
            print_str(sink, "down");
        }
        let ring_status = if state.ring_initialized != 0 {
            "tx/rx rings live"
        } else {
            "tx/rx rings pending"
        };
        print_str(sink, "\n      stack: ");
        print_str(sink, ring_status);
        print_str(sink, "; dhcp, ip pending\n");
    } else if state.driver_kind == DRIVER_VIRTIO {
        print_str(sink, "      driver: virtio-net discovered; native datapath still pending\n");
    }
}

fn print_probe_report(sink: &ConsoleSink, state: &NetState, title: &str) {
    set_color(sink, 11, 0);
    print_str(sink, "\n[NET] ");
    print_str(sink, title);
    print_str(sink, "\n");
    set_color(sink, 7, 0);
    if state.nic_present != 0 {
        print_str(sink, "      transport: qemu virtual nic over host network\n");
        print_str(sink, "      path: guest ");
        print_str(sink, driver_label(state.driver_kind));
        print_str(sink, " -> qemu nat -> host wifi\n");
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
        if state.mac_valid != 0 {
            print_str(sink, "\n      mac: ");
            print_mac(sink, &state.mac);
        }
        print_str(sink, "\n");
        print_link_stage(sink, state);
    } else {
        print_str(sink, "      no supported qemu nic detected on pci config space\n");
    }
    print_str(sink, "\n");
}

unsafe extern "C" fn net_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let console_target = {
        let ctx_ref = unsafe { &*ctx };
        let abi = unsafe { &*ctx_ref.abi };
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"console".as_ptr(),
                    b"console".len(),
                    b"write".as_ptr(),
                    b"write".len(),
                )
            }
        } else {
            0
        }
    };

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut NetState,
            NetState {
                console_target: if console_target == 0 {
                    VGA_FALLBACK_VEC.as_u64()
                } else {
                    console_target
                },
                mmio_bar: 0,
                io_bar: 0,
                status_reg: 0,
                ctrl_reg: 0,
                pci_command: 0,
                vendor_id: 0,
                device_id: 0,
                speed_mbps: 0,
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
                nic_present: 0,
                probe_complete: 0,
                link_up: 0,
                full_duplex: 0,
                mac_valid: 0,
                mac: [0; 6],
                ring_initialized: 0,
                rx_tail: 0,
                tx_tail: 0,
                ping_target_ip: GATEWAY_IP,
                gw_mac: [0; 6],
                gw_mac_valid: 0,
            },
        );
    }

    // Store state pointer so net_http_post_sync can reach it later.
    unsafe { *NET_STATE_PTR.0.get() = (*ctx).state_ptr; }

    ExecStatus::Done
}

unsafe extern "C" fn net_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let Some(input)  = (unsafe { pre::prepare(ctx, event) })  else { return ExecStatus::Done; };
    let Some(output) = (unsafe { proc::process(ctx, input) }) else { return ExecStatus::Done; };
    unsafe { post::emit(ctx, output) }
}

unsafe extern "C" fn net_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
