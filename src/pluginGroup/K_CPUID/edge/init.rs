//! Edge: init
use core::arch::x86_64::__cpuid;
use crate::serial_println;

pub fn init() {
    let res = __cpuid(0);
    let mut vendor = [0u8; 12];
    vendor[0..4].copy_from_slice(&res.ebx.to_le_bytes());
    vendor[4..8].copy_from_slice(&res.edx.to_le_bytes());
    vendor[8..12].copy_from_slice(&res.ecx.to_le_bytes());
    let vendor_str = core::str::from_utf8(&vendor).unwrap_or("Unknown CPU");
    serial_println!("[K_CPUID]  online  Vendor: {}", vendor_str);
}
