#![no_std]

use core::hint::spin_loop;

use gos_protocol::{
    packet_to_signal, signal_to_packet, ExecStatus, ExecutorContext, ExecutorId, KernelAbi,
    NET_CONTROL_PROBE, NET_CONTROL_REPORT, NET_CONTROL_RESET, NodeEvent, NodeExecutorVTable,
    Signal, VectorAddress,
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
        print_str(sink, "\n      stack: nic registers live; tx/rx rings, arp, dhcp, ip pending\n");
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
            },
        );
    }

    ExecStatus::Done
}

unsafe extern "C" fn net_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let sink = sink_from_ctx(ctx);
    let state = unsafe { state_mut(ctx) };
    let signal = packet_to_signal(unsafe { (*event).signal });

    match signal {
        Signal::Spawn { .. } => {
            refresh_network_state(state);
            print_probe_report(&sink, state, "uplink boot sync");
            ExecStatus::Done
        }
        Signal::Control { cmd, .. } => {
            match cmd {
                NET_CONTROL_REPORT => {
                    if state.probe_complete == 0 {
                        refresh_network_state(state);
                    }
                    print_probe_report(&sink, state, "uplink status");
                }
                NET_CONTROL_RESET => {
                    refresh_network_state(state);
                    print_probe_report(&sink, state, "uplink reset");
                }
                NET_CONTROL_PROBE | 1 => {
                    refresh_network_state(state);
                    print_probe_report(&sink, state, "uplink reprobe");
                }
                _ => {
                    if state.probe_complete == 0 {
                        refresh_network_state(state);
                    }
                    print_probe_report(&sink, state, "uplink status");
                }
            }
            ExecStatus::Done
        }
        _ => ExecStatus::Done,
    }
}

unsafe extern "C" fn net_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
