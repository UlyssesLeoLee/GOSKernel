//! ADR-010 F.5-logic — `JournalRing::flush_to_device`/`gos_journal::replay_from_device`,
//! exercised end-to-end against a synthetic RAM `BlockDeviceVTable` — the
//! device-backed counterpart to the existing in-memory `flush_into`/`replay`
//! round-trip tests in `runtime.rs`.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "journal_device.rs", type: "file", language: "rust"}),
//!   (ftd:Function {name: "JournalRing::flush_to_device", type: "function", visibility: "pub"}),
//!   (rfd:Function {name: "replay_from_device", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "flush_to_device_then_replay_from_device_round_trips", type: "function"}),
//!   (t2:Function {name: "replay_from_device_survives_a_simulated_power_cycle", type: "function"}),
//!   (t3:Function {name: "flush_to_device_spans_multiple_sectors", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (t1)-[:USES]->(ftd), (t1)-[:USES]->(rfd), (t2)-[:USES]->(ftd), (t2)-[:USES]->(rfd),
//!   (t3)-[:USES]->(ftd), (t3)-[:USES]->(rfd);
//! ```

use gos_journal::{replay_from_device, JournalRing};
use gos_protocol::block::{BlockDeviceVTable, BlockGeometry, BlockIoStatus, BLOCK_SECTOR_SIZE_DEFAULT};
use gos_protocol::{ControlPlaneEnvelope, ControlPlaneMessageKind};
use std::sync::Mutex as StdMutex;

const SECTOR_SIZE: usize = 512;
const TOTAL_SECTORS: usize = 16;

/// A fresh, zeroed RAM "disk" behind a `BlockDeviceVTable`. Leaked so the
/// vtable's `handle: u64` can carry a stable pointer for the duration of
/// one test (mirrors the same pattern `fat32_write.rs` uses for the same
/// reason: each test needs its own isolated backing store under Rust's
/// default parallel test runner).
fn fresh_disk() -> BlockDeviceVTable {
    let disk: &'static StdMutex<Vec<u8>> =
        Box::leak(Box::new(StdMutex::new(vec![0u8; TOTAL_SECTORS * SECTOR_SIZE])));

    unsafe extern "C" fn read(h: u64, lba: u64, buf: *mut u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let disk = unsafe { &*(h as *const StdMutex<Vec<u8>>) };
        let img = disk.lock().unwrap();
        let off = (lba as usize) * SECTOR_SIZE;
        if off + SECTOR_SIZE > img.len() {
            return BlockIoStatus::OutOfBounds as i32;
        }
        let dst = unsafe { core::slice::from_raw_parts_mut(buf, SECTOR_SIZE) };
        dst.copy_from_slice(&img[off..off + SECTOR_SIZE]);
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn write(h: u64, lba: u64, buf: *const u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let disk = unsafe { &*(h as *const StdMutex<Vec<u8>>) };
        let mut img = disk.lock().unwrap();
        let off = (lba as usize) * SECTOR_SIZE;
        if off + SECTOR_SIZE > img.len() {
            return BlockIoStatus::OutOfBounds as i32;
        }
        let src = unsafe { core::slice::from_raw_parts(buf, SECTOR_SIZE) };
        img[off..off + SECTOR_SIZE].copy_from_slice(src);
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn flush(_h: u64) -> i32 {
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn geometry(_h: u64) -> BlockGeometry {
        BlockGeometry { sector_count: TOTAL_SECTORS as u64, sector_size: BLOCK_SECTOR_SIZE_DEFAULT, flags: 0 }
    }

    BlockDeviceVTable {
        handle: disk as *const StdMutex<Vec<u8>> as u64,
        read_sector: read,
        write_sector: write,
        flush,
        geometry,
    }
}

fn envelope(arg0: u64) -> ControlPlaneEnvelope {
    ControlPlaneEnvelope {
        version: gos_protocol::CONTROL_PLANE_PROTOCOL_VERSION,
        kind: ControlPlaneMessageKind::Metric,
        subject: *b"K_TEST\0\0\0\0\0\0\0\0\0\0",
        arg0,
        arg1: 0,
    }
}

#[test]
fn flush_to_device_then_replay_from_device_round_trips() {
    let vtable = fresh_disk();
    let mut ring: JournalRing<8> = JournalRing::new();
    for i in 0..5u64 {
        ring.append(&envelope(i)).expect("append");
    }

    let written = ring.flush_to_device(&vtable, 0).expect("flush_to_device");
    assert_eq!(written, 5);

    let mut seen = Vec::new();
    let replayed = replay_from_device(&vtable, 0, written, |env| seen.push(env.arg0)).expect("replay_from_device");
    assert_eq!(replayed, 5);
    assert_eq!(seen, vec![0, 1, 2, 3, 4], "records must replay back in the same oldest-first order they were flushed");
}

#[test]
fn replay_from_device_survives_a_simulated_power_cycle() {
    let vtable = fresh_disk();
    let written = {
        let mut ring: JournalRing<8> = JournalRing::new();
        ring.append(&envelope(42)).expect("append");
        ring.append(&envelope(43)).expect("append");
        ring.flush_to_device(&vtable, 0).expect("flush_to_device")
        // `ring` (the in-memory buffer) is dropped here -- simulating a
        // power cycle. Only the block device's contents (the real,
        // separate RAM "disk" backing `vtable`) persist.
    };

    let mut seen = Vec::new();
    let replayed = replay_from_device(&vtable, 0, written, |env| seen.push(env.arg0)).expect("replay after drop");
    assert_eq!(replayed, 2);
    assert_eq!(seen, vec![42, 43]);
}

#[test]
fn flush_to_device_spans_multiple_sectors() {
    // HEADER_BYTES (8) + N * ENVELOPE_RECORD_BYTES (40) with N large
    // enough to blow past one 512-byte sector (8 + 13*40 = 528 > 512),
    // exercising accumulate_and_flush's sector-boundary-crossing path on
    // write and replay_from_device's on read.
    let vtable = fresh_disk();
    let mut ring: JournalRing<32> = JournalRing::new();
    for i in 0..20u64 {
        ring.append(&envelope(i * 10)).expect("append");
    }

    let written = ring.flush_to_device(&vtable, 0).expect("flush_to_device");
    assert_eq!(written, 20);

    let mut seen = Vec::new();
    let replayed = replay_from_device(&vtable, 0, written, |env| seen.push(env.arg0)).expect("replay");
    assert_eq!(replayed, 20);
    let expected: Vec<u64> = (0..20).map(|i| i * 10).collect();
    assert_eq!(seen, expected, "records spanning multiple sectors must reassemble correctly");
}
