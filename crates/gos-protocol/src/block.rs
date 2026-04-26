//! Phase F.1 — block device ABI.
//!
//! `BlockDeviceVTable` is the C-ABI handle a kernel-side block driver
//! (AHCI, NVMe, ramdisk for tests) registers with the runtime so that
//! upper layers (gos-vfs, gos-supervisor's RESOURCE_BLOCK_DEVICE
//! claims) can address it uniformly.
//!
//! Until a real AHCI or NVMe driver lands, the only registered
//! provider is the in-tree ramdisk stub (Phase F.1.1) — the trait and
//! capability constants exist now so plugins can already declare the
//! dependency.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BlockIoStatus {
    Ok = 0,
    /// Sector index out of range.
    OutOfBounds = -1,
    /// Underlying hardware reported an error.
    DeviceError = -2,
    /// No driver registered for this block device.
    Unmounted = -3,
    /// Caller's buffer was the wrong size.
    BadBuffer = -4,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BlockGeometry {
    pub sector_count: u64,
    pub sector_size: u32,
    /// Bit 0 = read-only.  Bits 1..63 reserved.
    pub flags: u32,
}

pub const BLOCK_GEOMETRY_FLAG_READONLY: u32 = 1 << 0;

/// Common 512-byte sector size — used by the stub ramdisk and the
/// majority of historical block devices.  AHCI / NVMe drivers report
/// their actual sector size via `BlockDeviceVTable::geometry`.
pub const BLOCK_SECTOR_SIZE_DEFAULT: u32 = 512;

/// V-table a block-device driver fills in and registers with the
/// runtime.  All callbacks are `unsafe extern "C"`:
///   * `read_sector(handle, lba, buf, len)` reads exactly one sector
///     of `BlockGeometry::sector_size` bytes; returns `BlockIoStatus`.
///   * `write_sector(...)` mirror.
///   * `flush(...)` ensures all pending writes are durable; returns
///     `BlockIoStatus`.  Drivers without a write cache may no-op.
///   * `geometry(...)` returns the device geometry — sector count,
///     size, RO flag.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlockDeviceVTable {
    pub handle: u64,
    pub read_sector: unsafe extern "C" fn(handle: u64, lba: u64, buf: *mut u8, len: u32) -> i32,
    pub write_sector:
        unsafe extern "C" fn(handle: u64, lba: u64, buf: *const u8, len: u32) -> i32,
    pub flush: unsafe extern "C" fn(handle: u64) -> i32,
    pub geometry: unsafe extern "C" fn(handle: u64) -> BlockGeometry,
}

impl BlockIoStatus {
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Ok,
            -1 => Self::OutOfBounds,
            -2 => Self::DeviceError,
            -3 => Self::Unmounted,
            -4 => Self::BadBuffer,
            _ => Self::DeviceError,
        }
    }
}
