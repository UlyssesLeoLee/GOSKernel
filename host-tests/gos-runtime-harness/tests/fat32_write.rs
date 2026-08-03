//! ADR-010 F.5-logic — FAT32 write path (`k-fat32`) + `gos_vfs::FileSystem`
//! `write`/`create`/`fsync`, exercised end-to-end against a synthetic RAM
//! `BlockDeviceVTable` — the same "ramdisk + host-tests harness" pattern
//! `fat32_minimal_image_round_trips_lookup_read_and_readdir` (F.3.1, this
//! same crate's `runtime.rs`) already established for the read path, and
//! [`OPTIMIZATION_PLAN.md`]'s `vfs_trait_drives_a_synthetic_in_memory_filesystem`
//! demonstrated for the VFS trait itself.
//!
//! Correctness here (does the FAT table update right, does the directory
//! entry's size field track reality) is orthogonal to *which* block device
//! backs it — this harness never touches real hardware, matching the F.1
//! precedent ADR-010 §一.1 cites.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "fat32_write.rs", type: "file", language: "rust"}),
//!   (w:Function {name: "FileSystem::write", type: "function", visibility: "pub"}),
//!   (c:Function {name: "FileSystem::create", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "create_then_write_then_read_back_round_trips", type: "function"}),
//!   (t2:Function {name: "write_spans_multiple_clusters_and_extends_the_chain", type: "function"}),
//!   (t3:Function {name: "create_rejects_duplicate_names", type: "function"}),
//!   (t4:Function {name: "create_a_subdirectory_and_a_file_inside_it", type: "function"}),
//!   (t5:Function {name: "allocate_cluster_reports_no_space_when_volume_is_full", type: "function"}),
//!   (t6:Function {name: "overwrite_within_an_existing_file_does_not_grow_it", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5), (f)-[:CONTAINS]->(t6),
//!   (t1)-[:USES]->(c), (t1)-[:USES]->(w), (t2)-[:USES]->(w), (t3)-[:USES]->(c),
//!   (t4)-[:USES]->(c), (t6)-[:USES]->(w);
//! ```

use gos_protocol::block::{BlockDeviceVTable, BlockGeometry, BlockIoStatus, BLOCK_SECTOR_SIZE_DEFAULT};
use gos_vfs::{FileSystem, InodeKind, MountId, VfsError};
use k_fat32::Fat32;
use std::sync::Mutex as StdMutex;

const SECTOR_SIZE: usize = 512;
const TOTAL_SECTORS: usize = 64;

/// Build a minimal, empty FAT32 volume: 1 sector/cluster, 2 reserved
/// sectors, 2 one-sector FAT copies, root at cluster 2 (empty — one
/// all-zero sector, i.e. "no entries"). Data region is clusters 2..62 (60
/// clusters total, 59 free after root), giving `create`/`write` plenty of
/// room without needing a huge image.
fn empty_volume_image() -> Vec<u8> {
    let mut img = vec![0u8; TOTAL_SECTORS * SECTOR_SIZE];

    img[0..3].copy_from_slice(&[0xEB, 0x58, 0x90]);
    img[3..0x0B].copy_from_slice(b"GOS_FAT3");
    img[0x0B..0x0D].copy_from_slice(&(SECTOR_SIZE as u16).to_le_bytes());
    img[0x0D] = 1; // sectors/cluster
    img[0x0E..0x10].copy_from_slice(&2u16.to_le_bytes()); // reserved sectors
    img[0x10] = 2; // num FATs
    img[0x13..0x15].copy_from_slice(&0u16.to_le_bytes());
    img[0x16..0x18].copy_from_slice(&0u16.to_le_bytes());
    img[0x20..0x24].copy_from_slice(&(TOTAL_SECTORS as u32).to_le_bytes());
    img[0x24..0x28].copy_from_slice(&1u32.to_le_bytes()); // sectors/FAT
    img[0x2C..0x30].copy_from_slice(&2u32.to_le_bytes()); // root cluster
    img[0x1FE] = 0x55;
    img[0x1FF] = 0xAA;

    // FAT 1 + FAT 2: entries 0/1 reserved sentinels, entry 2 (root) = EOC.
    // Everything else (clusters 3..) starts at 0 (free).
    for fat in 0..2 {
        let off = (2 + fat) * SECTOR_SIZE;
        img[off..off + 4].copy_from_slice(&0x0FFFFFF8u32.to_le_bytes());
        img[off + 4..off + 8].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
        img[off + 8..off + 12].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
    }
    // Root directory (sector 4, cluster 2) is already all-zero -- "no
    // entries" (first byte 0x00 of the first slot is the end marker).
    img
}

/// Mount a fresh empty volume behind a writable RAM-backed
/// `BlockDeviceVTable`. Each test gets its own `Box::leak`ed image so
/// concurrent tests (Rust's default parallel runner) never share state --
/// mirrors how `fat32_minimal_image_round_trips_lookup_read_and_readdir`
/// scopes its own `static IMAGE` per test function, except we need a
/// fresh instance per *call* here since several tests in this file mount
/// more than once conceptually (they don't, but this keeps the helper
/// reusable without a footgun).
fn mount_fresh() -> Fat32 {
    let image: &'static StdMutex<Vec<u8>> = Box::leak(Box::new(StdMutex::new(empty_volume_image())));

    unsafe extern "C" fn read(_h: u64, lba: u64, buf: *mut u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let handle = unsafe { &*(_h as *const StdMutex<Vec<u8>>) };
        let img = handle.lock().unwrap();
        let off = (lba as usize) * SECTOR_SIZE;
        if off + SECTOR_SIZE > img.len() {
            return BlockIoStatus::OutOfBounds as i32;
        }
        let dst = unsafe { core::slice::from_raw_parts_mut(buf, SECTOR_SIZE) };
        dst.copy_from_slice(&img[off..off + SECTOR_SIZE]);
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn write(_h: u64, lba: u64, buf: *const u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let handle = unsafe { &*(_h as *const StdMutex<Vec<u8>>) };
        let mut img = handle.lock().unwrap();
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
        BlockGeometry {
            sector_count: TOTAL_SECTORS as u64,
            sector_size: BLOCK_SECTOR_SIZE_DEFAULT,
            flags: 0, // writable
        }
    }

    let vtable = BlockDeviceVTable {
        handle: image as *const StdMutex<Vec<u8>> as u64,
        read_sector: read,
        write_sector: write,
        flush,
        geometry,
    };
    Fat32::mount(MountId(1), vtable).expect("mount fresh empty volume")
}

#[test]
fn create_then_write_then_read_back_round_trips() {
    let fs = mount_fresh();
    let root = fs.root();

    let file = fs.create(root, b"HELLO.TXT", InodeKind::File).expect("create");
    assert_eq!(file.kind, InodeKind::File);
    assert_eq!(file.size_bytes, 0);

    let n = fs.write(file, 0, b"hello, fat32").expect("write");
    assert_eq!(n, 12);

    // The Inode the caller already holds is not mutated in place (per the
    // trait doc) -- re-lookup to see the updated size.
    let refreshed = fs.lookup(root, b"HELLO.TXT").expect("re-lookup");
    assert_eq!(refreshed.size_bytes, 12);

    let mut buf = [0u8; 32];
    let n = fs.read(refreshed, 0, &mut buf).expect("read back");
    assert_eq!(&buf[..n], b"hello, fat32");

    fs.fsync(refreshed).expect("fsync");
}

#[test]
fn write_spans_multiple_clusters_and_extends_the_chain() {
    let fs = mount_fresh();
    let root = fs.root();
    let file = fs.create(root, b"BIG.BIN", InodeKind::File).expect("create");

    // 1 sector = 1 cluster = 512 bytes here, so >512 bytes forces at least
    // one allocate_cluster() call inside write().
    let payload: Vec<u8> = (0u32..1200).map(|i| (i % 256) as u8).collect();
    let n = fs.write(file, 0, &payload).expect("write spanning clusters");
    assert_eq!(n, payload.len());

    let refreshed = fs.lookup(root, b"BIG.BIN").expect("re-lookup");
    assert_eq!(refreshed.size_bytes, payload.len() as u64);

    let mut buf = vec![0u8; payload.len()];
    let n = fs.read(refreshed, 0, &mut buf).expect("read back");
    assert_eq!(n, payload.len());
    assert_eq!(buf, payload, "content must round-trip byte-for-byte across the cluster boundary");
}

#[test]
fn overwrite_within_an_existing_file_does_not_grow_it() {
    let fs = mount_fresh();
    let root = fs.root();
    let file = fs.create(root, b"NOTE.TXT", InodeKind::File).expect("create");
    fs.write(file, 0, b"AAAAAAAAAA").expect("initial write");
    let after_initial = fs.lookup(root, b"NOTE.TXT").expect("lookup");
    assert_eq!(after_initial.size_bytes, 10);

    // Overwrite the middle 4 bytes -- offset (2) + len (4) stays within
    // the existing size (10), so size must not change.
    let n = fs.write(after_initial, 2, b"BBBB").expect("in-place overwrite");
    assert_eq!(n, 4);
    let after_overwrite = fs.lookup(root, b"NOTE.TXT").expect("lookup");
    assert_eq!(after_overwrite.size_bytes, 10, "in-bounds overwrite must not change file size");

    let mut buf = [0u8; 16];
    let n = fs.read(after_overwrite, 0, &mut buf).expect("read back");
    assert_eq!(&buf[..n], b"AABBBBAAAA");
}

#[test]
fn create_rejects_duplicate_names() {
    let fs = mount_fresh();
    let root = fs.root();
    fs.create(root, b"DUP.TXT", InodeKind::File).expect("first create");
    match fs.create(root, b"DUP.TXT", InodeKind::File) {
        Err(VfsError::AlreadyExists) => {}
        other => panic!("expected AlreadyExists, got {:?}", other.map(|i| i.num.0)),
    }
}

#[test]
fn create_a_subdirectory_and_a_file_inside_it() {
    let fs = mount_fresh();
    let root = fs.root();

    let sub = fs.create(root, b"SUBDIR", InodeKind::Directory).expect("create subdir");
    assert_eq!(sub.kind, InodeKind::Directory);

    let inner = fs.create(sub, b"INNER.TXT", InodeKind::File).expect("create inside subdir");
    assert_eq!(inner.kind, InodeKind::File);

    // The new file is discoverable via the subdirectory's own listing.
    let found = fs.lookup(sub, b"INNER.TXT").expect("lookup inside subdir");
    assert_eq!(found.num.0, inner.num.0);

    // write() must still succeed and land the bytes on disk -- the data
    // write itself doesn't care which directory owns the entry.
    let n = fs.write(inner, 0, b"nested").expect("write inside subdir");
    assert_eq!(n, 6);

    // Documented v1 boundary: the on-disk directory *entry*'s size field
    // can't be re-located (patch_dir_entry_size only searches from root),
    // so a subsequent lookup still reports the pre-write size. This is a
    // real, asserted limitation, not silently glossed over -- read()
    // trusts inode.size_bytes to bound how much it returns, so callers
    // that need accurate sizes for subdirectory files must stay at the
    // documented root-only boundary until a future slice generalizes
    // patch_dir_entry_size to walk subdirectories.
    let after_write = fs.lookup(sub, b"INNER.TXT").expect("re-lookup inside subdir");
    assert_eq!(
        after_write.size_bytes, 0,
        "v1 limitation: subdirectory entries don't get their size patched (root-only search)"
    );

    // The bytes are still really there -- reading with an inode whose
    // size_bytes we set ourselves (standing in for a future
    // subdirectory-aware patch) proves the data write itself is correct,
    // independent of the metadata limitation above.
    let mut manual = inner;
    manual.size_bytes = 6;
    let mut buf = [0u8; 8];
    let n = fs.read(manual, 0, &mut buf).expect("read back inside subdir");
    assert_eq!(&buf[..n], b"nested");
}

#[test]
fn allocate_cluster_reports_no_space_when_volume_is_full() {
    let fs = mount_fresh();
    let root = fs.root();

    // This tiny volume has 59 free clusters after root (see
    // empty_volume_image's doc comment). Force exhaustion with fewer,
    // larger files instead of exactly filling the count with 0-byte
    // creates (0-byte files don't consume a data cluster beyond the one
    // `create` always allocates for the entry itself).
    let mut created = 0usize;
    let mut last_err = None;
    for i in 0..80u32 {
        let name = format!("F{i}.BIN");
        let name = name.as_bytes();
        match fs.create(root, name, InodeKind::File) {
            Ok(_) => created += 1,
            Err(e) => {
                last_err = Some(e);
                break;
            }
        }
    }
    assert_eq!(
        last_err,
        Some(VfsError::NoSpace),
        "volume must report NoSpace once every free cluster is allocated, not silently corrupt the FAT"
    );
    assert!(created > 0 && created < 80, "expected exhaustion partway through, got {created} successful creates");
}
