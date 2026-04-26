// gos-runtime host harness
//
// Drives gos_runtime in isolation on the host: registers a synthetic
// plugin + node, binds a native executor that returns ExecStatus::Fault,
// pushes a signal through route_signal, and asserts the runtime fault
// queue captures the offending vector address.  Locks in the Phase B.1
// fault-attribution contract end-to-end without needing a kernel.
//
// The runtime exposes a global RUNTIME singleton, so tests must serialize
// against the shared TEST_LOCK.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecStatus, ExecutorContext, ExecutorId, NodeEvent,
    NodeExecutorVTable, NodeSpec, PluginId, PluginManifest, RuntimeNodeType, Signal,
    VectorAddress, GOS_ABI_VERSION,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const TEST_PLUGIN_ID: PluginId = PluginId::from_ascii("HARNESS_RT");
const TEST_NODE_KEY: &str = "harness.entry";
const TEST_VECTOR: VectorAddress = VectorAddress::new(7, 7, 7, 7);
const TEST_EXECUTOR: ExecutorId = ExecutorId::from_ascii("native.harness");

const TEST_NODE_SPECS: &[NodeSpec] = &[NodeSpec {
    node_id: derive_node_id(TEST_PLUGIN_ID, TEST_NODE_KEY),
    local_node_key: TEST_NODE_KEY,
    node_type: RuntimeNodeType::Service,
    entry_policy: EntryPolicy::Manual,
    executor_id: TEST_EXECUTOR,
    state_schema_hash: 0xDEAD_BEEF,
    permissions: &[],
    exports: &[],
    vector_ref: None,
}];

const TEST_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: TEST_PLUGIN_ID,
    name: "HARNESS_RT",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: TEST_NODE_SPECS,
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

unsafe extern "C" fn faulting_on_event(
    _ctx: *mut ExecutorContext,
    _event: *const NodeEvent,
) -> ExecStatus {
    ExecStatus::Fault
}

const FAULTING_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: TEST_EXECUTOR,
    on_init: None,
    on_event: Some(faulting_on_event),
    on_suspend: None,
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

fn install_test_node() {
    gos_runtime::reset();
    gos_runtime::discover_plugin(TEST_MANIFEST).expect("discover plugin");
    gos_runtime::mark_plugin_loaded(TEST_PLUGIN_ID).expect("mark loaded");
    gos_runtime::register_node(TEST_PLUGIN_ID, TEST_VECTOR, TEST_NODE_SPECS[0])
        .expect("register node");
    gos_runtime::bind_native_executor(TEST_VECTOR, FAULTING_VTABLE)
        .expect("bind executor");
}

#[test]
fn route_signal_to_faulting_executor_pushes_vector_into_fault_queue() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    install_test_node();

    // Sanity: vector resolves to the registered node's plugin id, and no
    // instance is bound yet (boot-fallback regime).
    assert_eq!(
        gos_runtime::plugin_id_for_vec(TEST_VECTOR),
        Some(TEST_PLUGIN_ID)
    );
    assert!(gos_runtime::drain_next_fault().is_none());

    // Drive a signal through the dispatch path; the vtable returns
    // ExecStatus::Fault, so the runtime must enqueue TEST_VECTOR.
    let _ = gos_runtime::route_signal(TEST_VECTOR, Signal::Spawn { payload: 0 });

    let drained = gos_runtime::drain_next_fault();
    assert_eq!(drained, Some(TEST_VECTOR));
    assert!(
        gos_runtime::drain_next_fault().is_none(),
        "fault queue must be empty after a single drain"
    );
}

// Phase D.4: gos-log routes a formatted log call to both installed
// backends, respects min-level filtering, and truncates oversize
// payloads with the `truncated` flag set on the structured record.
#[test]
fn gos_log_routes_to_serial_and_structured_backends_with_filtering() {
    use gos_log::{
        install_serial_backend, install_structured_backend, set_min_level, LogLevel,
        LogRecord, SerialBackend, StructuredBackend, LOG_PAYLOAD_BYTES,
    };
    use std::sync::atomic::{AtomicU32, Ordering as AOrd};
    use std::sync::Mutex as StdMutex;

    static SERIAL_HITS: AtomicU32 = AtomicU32::new(0);
    static STRUCTURED_HITS: AtomicU32 = AtomicU32::new(0);
    static LAST_LEVEL: AtomicU32 = AtomicU32::new(0);
    static LAST_PAYLOAD: StdMutex<Vec<u8>> = StdMutex::new(Vec::new());
    static LAST_TRUNCATED: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn ser(level: u8, _src: *const u8, msg: *const u8, len: u32) {
        SERIAL_HITS.fetch_add(1, AOrd::SeqCst);
        LAST_LEVEL.store(level as u32, AOrd::SeqCst);
        let bytes = unsafe { core::slice::from_raw_parts(msg, len as usize) };
        let mut p = LAST_PAYLOAD.lock().unwrap();
        p.clear();
        p.extend_from_slice(bytes);
    }
    unsafe extern "C" fn structured(record: *const LogRecord) {
        STRUCTURED_HITS.fetch_add(1, AOrd::SeqCst);
        let r = unsafe { &*record };
        LAST_TRUNCATED.store(r.truncated as u32, AOrd::SeqCst);
    }

    SERIAL_HITS.store(0, AOrd::SeqCst);
    STRUCTURED_HITS.store(0, AOrd::SeqCst);
    LAST_TRUNCATED.store(0, AOrd::SeqCst);

    install_serial_backend(SerialBackend { write: ser });
    install_structured_backend(StructuredBackend { publish: structured });

    let src = *b"GOS_LOG_TEST\0\0\0\0";

    // 1. Default min-level is Info — Trace/Debug filtered, Info passes.
    set_min_level(LogLevel::Info);
    gos_log::log!(LogLevel::Trace, src, "filtered");
    gos_log::log!(LogLevel::Debug, src, "also filtered");
    assert_eq!(SERIAL_HITS.load(AOrd::SeqCst), 0);
    assert_eq!(STRUCTURED_HITS.load(AOrd::SeqCst), 0);

    gos_log::log!(LogLevel::Info, src, "hello {}", 42u32);
    assert_eq!(SERIAL_HITS.load(AOrd::SeqCst), 1);
    assert_eq!(STRUCTURED_HITS.load(AOrd::SeqCst), 1);
    assert_eq!(LAST_LEVEL.load(AOrd::SeqCst), LogLevel::Info as u32);
    {
        let p = LAST_PAYLOAD.lock().unwrap();
        assert_eq!(p.as_slice(), b"hello 42");
    }

    // 2. Lowering min-level lets Trace through.
    set_min_level(LogLevel::Trace);
    gos_log::log!(LogLevel::Trace, src, "now visible");
    assert_eq!(SERIAL_HITS.load(AOrd::SeqCst), 2);

    // 3. Oversize payload sets truncated.
    let big_str = "x".repeat(LOG_PAYLOAD_BYTES + 10);
    gos_log::log!(LogLevel::Info, src, "{}", big_str);
    assert_eq!(LAST_TRUNCATED.load(AOrd::SeqCst), 1);
}

// Phase F.1 / F.2: BlockDevice ABI + VFS trait surface compile cleanly
// and round-trip through a synthetic ramdisk implementation.  Locks in
// the contract so the FAT32 implementation slated for F.3 can build
// against this trait without re-shaping the API.
#[test]
fn vfs_trait_drives_a_synthetic_in_memory_filesystem() {
    use gos_protocol::block::{
        BlockDeviceVTable, BlockGeometry, BlockIoStatus, BLOCK_SECTOR_SIZE_DEFAULT,
    };
    use gos_vfs::{
        DirEntry, FileSystem, Inode, InodeKind, InodeNum, MountId, MountSource, VfsError,
    };

    // ── 1. BlockDevice round-trip: the ramdisk vtable can read back
    //    sectors written via its write callback. ──────────────────────
    use std::sync::Mutex as StdMutex;
    static DISK: StdMutex<[u8; 1024]> = StdMutex::new([0u8; 1024]);

    unsafe extern "C" fn read(_h: u64, lba: u64, buf: *mut u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let off = (lba as usize) * 512;
        let disk = DISK.lock().unwrap();
        if off + 512 > disk.len() {
            return BlockIoStatus::OutOfBounds as i32;
        }
        let dst = unsafe { core::slice::from_raw_parts_mut(buf, 512) };
        dst.copy_from_slice(&disk[off..off + 512]);
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn write(_h: u64, lba: u64, buf: *const u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let off = (lba as usize) * 512;
        let mut disk = DISK.lock().unwrap();
        if off + 512 > disk.len() {
            return BlockIoStatus::OutOfBounds as i32;
        }
        let src = unsafe { core::slice::from_raw_parts(buf, 512) };
        disk[off..off + 512].copy_from_slice(src);
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn flush(_h: u64) -> i32 {
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn geometry(_h: u64) -> BlockGeometry {
        BlockGeometry {
            sector_count: 2,
            sector_size: BLOCK_SECTOR_SIZE_DEFAULT,
            flags: 0,
        }
    }
    let vtable = BlockDeviceVTable {
        handle: 1,
        read_sector: read,
        write_sector: write,
        flush,
        geometry,
    };
    let geo = unsafe { (vtable.geometry)(vtable.handle) };
    assert_eq!(geo.sector_count, 2);
    assert_eq!(geo.sector_size, 512);

    let pattern = [0xABu8; 512];
    let status = unsafe { (vtable.write_sector)(vtable.handle, 0, pattern.as_ptr(), 512) };
    assert_eq!(BlockIoStatus::from_i32(status), BlockIoStatus::Ok);

    let mut readback = [0u8; 512];
    let status =
        unsafe { (vtable.read_sector)(vtable.handle, 0, readback.as_mut_ptr(), 512) };
    assert_eq!(BlockIoStatus::from_i32(status), BlockIoStatus::Ok);
    assert_eq!(readback, pattern);

    // Out-of-bounds read returns the right error class.
    let status =
        unsafe { (vtable.read_sector)(vtable.handle, 99, readback.as_mut_ptr(), 512) };
    assert_eq!(BlockIoStatus::from_i32(status), BlockIoStatus::OutOfBounds);

    // ── 2. FileSystem trait drives a synthetic in-memory FS. ───────
    struct TinyFs;
    impl FileSystem for TinyFs {
        fn mount_id(&self) -> MountId {
            MountId(1)
        }
        fn root(&self) -> Inode {
            Inode {
                mount: MountId(1),
                num: InodeNum(1),
                kind: InodeKind::Directory,
                size_bytes: 0,
            }
        }
        fn lookup(&self, parent: Inode, name: &[u8]) -> Result<Inode, VfsError> {
            if parent.num != InodeNum(1) {
                return Err(VfsError::NotFound);
            }
            if name == b"hello" {
                Ok(Inode {
                    mount: MountId(1),
                    num: InodeNum(2),
                    kind: InodeKind::File,
                    size_bytes: 5,
                })
            } else {
                Err(VfsError::NotFound)
            }
        }
        fn read(&self, inode: Inode, offset: u64, out: &mut [u8]) -> Result<usize, VfsError> {
            if inode.num != InodeNum(2) {
                return Err(VfsError::NotAFile);
            }
            let data = b"hello";
            let off = offset as usize;
            if off >= data.len() {
                return Ok(0);
            }
            let n = (data.len() - off).min(out.len());
            out[..n].copy_from_slice(&data[off..off + n]);
            Ok(n)
        }
        fn read_dir(
            &self,
            dir: Inode,
            cursor: u64,
            entries: &mut [DirEntry],
        ) -> Result<(usize, u64), VfsError> {
            if dir.kind != InodeKind::Directory {
                return Err(VfsError::NotADirectory);
            }
            if cursor > 0 {
                return Ok((0, u64::MAX));
            }
            let mut e = DirEntry::empty();
            e.inode = self.lookup(dir, b"hello").unwrap();
            e.name[..5].copy_from_slice(b"hello");
            e.name_len = 5;
            entries[0] = e;
            Ok((1, u64::MAX))
        }
    }

    let fs = TinyFs;
    let root = fs.root();
    let hello = fs.lookup(root, b"hello").expect("hello lookup");
    let mut buf = [0u8; 16];
    let n = fs.read(hello, 0, &mut buf).expect("read hello");
    assert_eq!(&buf[..n], b"hello");

    let mut entries = [DirEntry::empty(); 4];
    let (n, next) = fs.read_dir(root, 0, &mut entries).expect("readdir");
    assert_eq!(n, 1);
    assert_eq!(next, u64::MAX);
    assert_eq!(entries[0].name(), b"hello");

    // The `MountSource::from_block(...)` path compiles and the block
    // vtable round-trips through it.
    let _src = MountSource::from_block(vtable);
}

// Phase B.4.6: minimal ET_DYN ELF parser — locks in format detection so
// malformed payloads are rejected before they reach the supervisor.
#[test]
fn elf_parser_rejects_bad_inputs_and_walks_synthetic_etdyn() {
    use gos_loader::elf::{parse, ElfError, PF_R, PF_X, PT_LOAD};

    // Reject empty / short / wrong magic / wrong class.
    assert_eq!(parse(&[]).unwrap_err(), ElfError::TooSmall);
    assert_eq!(parse(&[0u8; 32]).unwrap_err(), ElfError::TooSmall);
    let mut bad_magic = [0u8; 64];
    bad_magic[..4].copy_from_slice(&[0x7F, b'X', b'L', b'F']);
    assert_eq!(parse(&bad_magic).unwrap_err(), ElfError::BadMagic);

    // Build a minimal valid ET_DYN ELF64-LE x86_64 header + 1 PT_LOAD.
    let mut elf = vec![0u8; 64 + 56];
    elf[..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    elf[4] = 2; // ELFCLASS64
    elf[5] = 1; // ELFDATA2LSB
    elf[6] = 1; // EI_VERSION
    elf[7] = 0; // EI_OSABI = SYSV
    elf[16..18].copy_from_slice(&3u16.to_le_bytes()); // ET_DYN
    elf[18..20].copy_from_slice(&62u16.to_le_bytes()); // EM_X86_64
    elf[24..32].copy_from_slice(&0x1234u64.to_le_bytes()); // e_entry
    elf[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
    elf[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
    elf[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum
    // PT_LOAD at offset 64
    elf[64..68].copy_from_slice(&PT_LOAD.to_le_bytes());
    elf[68..72].copy_from_slice(&(PF_R | PF_X).to_le_bytes());
    elf[72..80].copy_from_slice(&0u64.to_le_bytes()); // p_offset
    elf[80..88].copy_from_slice(&0x4000u64.to_le_bytes()); // p_vaddr
    elf[88..96].copy_from_slice(&0x4000u64.to_le_bytes()); // p_paddr
    elf[96..104].copy_from_slice(&0x100u64.to_le_bytes()); // p_filesz
    elf[104..112].copy_from_slice(&0x200u64.to_le_bytes()); // p_memsz

    let parsed = parse(&elf).expect("valid ET_DYN");
    assert_eq!(parsed.entry_offset, 0x1234);
    assert_eq!(parsed.program_headers, 1);

    let mut count = 0usize;
    let mut last_flags = 0u32;
    parsed.for_each_load_segment(|seg| {
        count += 1;
        assert_eq!(seg.virt_addr, 0x4000);
        assert_eq!(seg.mem_len, 0x200);
        assert_eq!(seg.file_offset, 0);
        assert_eq!(seg.file_len, 0x100);
        last_flags = seg.flags;
    });
    assert_eq!(count, 1);
    assert_eq!(last_flags, PF_R | PF_X);
    assert_eq!(parsed.highest_virt_end(), 0x4200);

    // Reject non-ET_DYN.
    let mut elf_exec = elf.clone();
    elf_exec[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    assert_eq!(parse(&elf_exec).unwrap_err(), ElfError::NotEtDyn);

    // Reject non-x86_64.
    let mut elf_arm = elf.clone();
    elf_arm[18..20].copy_from_slice(&183u16.to_le_bytes()); // EM_AARCH64
    assert_eq!(parse(&elf_arm).unwrap_err(), ElfError::NotX86_64);
}

// Phase D.5: ABI semver compatibility rules.  Major must match exactly;
// the host's minor must be >= the plugin's minor; patch is observational.
#[test]
fn abi_compatible_enforces_major_strict_minor_subset() {
    use gos_protocol::{abi_compatible, encode_abi, GOS_ABI_VERSION};

    // Same encoding -> compatible.
    assert!(abi_compatible(GOS_ABI_VERSION, GOS_ABI_VERSION));

    // Plugin built against an older minor on the same major -> compatible.
    let host = encode_abi(2, 5, 0);
    let older_minor = encode_abi(2, 3, 0);
    assert!(abi_compatible(older_minor, host));

    // Plugin built against a newer minor than host knows -> rejected.
    let newer_minor = encode_abi(2, 7, 0);
    assert!(!abi_compatible(newer_minor, host));

    // Different major -> rejected unconditionally.
    let bumped_major = encode_abi(3, 0, 0);
    assert!(!abi_compatible(bumped_major, host));
    assert!(!abi_compatible(host, bumped_major));

    // Patch is informational and never affects compatibility.
    let host_patched = encode_abi(2, 5, 42);
    let plugin_patched = encode_abi(2, 5, 7);
    assert!(abi_compatible(plugin_patched, host_patched));
}

// Decoding helpers should round-trip cleanly so manifest authors and
// loaders can read individual components without bit-twiddling.
#[test]
fn abi_components_round_trip() {
    use gos_protocol::{abi_major, abi_minor, abi_patch, encode_abi};

    for (maj, min, pat) in [(0, 0, 0), (2, 0, 0), (2, 7, 13), (255, 255, 65535)] {
        let v = encode_abi(maj, min, pat);
        assert_eq!(abi_major(v), maj);
        assert_eq!(abi_minor(v), min);
        assert_eq!(abi_patch(v), pat);
    }
}

// Phase E.1: soft preemption — when the supervisor flags the active
// instance during a dispatch, the runtime must re-enqueue it and
// surface the dispatch as Yield even if the executor returned Done.
#[test]
fn preempt_flag_re_enqueues_instance_and_reports_yield() {
    use gos_protocol::{CellResult, NodeInstanceId};
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering as AOrd};

    let _guard = TEST_LOCK.lock().expect("test lock");

    gos_runtime::reset();
    gos_runtime::discover_plugin(TEST_MANIFEST).expect("discover");
    gos_runtime::mark_plugin_loaded(TEST_PLUGIN_ID).expect("loaded");
    gos_runtime::register_node(TEST_PLUGIN_ID, TEST_VECTOR, TEST_NODE_SPECS[0])
        .expect("register node");

    unsafe extern "C" fn done_on_event(
        _ctx: *mut gos_protocol::ExecutorContext,
        _event: *const gos_protocol::NodeEvent,
    ) -> gos_protocol::ExecStatus {
        gos_protocol::ExecStatus::Done
    }
    let done_vtable = gos_protocol::NodeExecutorVTable {
        executor_id: TEST_EXECUTOR,
        on_init: None,
        on_event: Some(done_on_event),
        on_suspend: None,
        on_resume: None,
        on_teardown: None,
        on_telemetry: None,
    };
    gos_runtime::bind_native_executor(TEST_VECTOR, done_vtable).expect("bind");

    let inst = NodeInstanceId::new(99);
    gos_runtime::bind_plugin_instance(TEST_PLUGIN_ID, inst);

    static SHOULD_PREEMPT: AtomicBool = AtomicBool::new(false);
    static CLEAR_COUNT: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn no_tick() {}
    unsafe extern "C" fn check(_id: NodeInstanceId) -> bool {
        SHOULD_PREEMPT.load(AOrd::SeqCst)
    }
    unsafe extern "C" fn clear(_id: NodeInstanceId) {
        SHOULD_PREEMPT.store(false, AOrd::SeqCst);
        CLEAR_COUNT.fetch_add(1, AOrd::SeqCst);
    }

    SHOULD_PREEMPT.store(false, AOrd::SeqCst);
    CLEAR_COUNT.store(0, AOrd::SeqCst);

    gos_runtime::install_scheduler(gos_runtime::Scheduler {
        on_tick: no_tick,
        should_preempt: check,
        clear_preempt: clear,
    });

    let baseline = gos_runtime::preempt_count();

    // First call: no preemption requested -> Done passes through.
    let r = gos_runtime::route_signal(TEST_VECTOR, Signal::Spawn { payload: 0 })
        .expect("route");
    assert!(matches!(r, CellResult::Done));
    assert_eq!(gos_runtime::preempt_count(), baseline);
    assert_eq!(CLEAR_COUNT.load(AOrd::SeqCst), 0);

    // Second call: preempt flag set -> Yield + clear_preempt called.
    SHOULD_PREEMPT.store(true, AOrd::SeqCst);
    let r = gos_runtime::route_signal(TEST_VECTOR, Signal::Spawn { payload: 0 })
        .expect("route");
    assert!(
        matches!(r, CellResult::Yield),
        "preempted dispatch must surface as Yield"
    );
    assert_eq!(gos_runtime::preempt_count(), baseline + 1);
    assert_eq!(CLEAR_COUNT.load(AOrd::SeqCst), 1);
    assert!(
        !SHOULD_PREEMPT.load(AOrd::SeqCst),
        "clear hook must un-set the flag"
    );
}

// Phase B.4.4 / B.4.5: native dispatch is bracketed by a CR3 trampoline
// (DomainSwitch hook).  This test installs a counting hook and proves
// every native callback under route_signal increments enter+leave.
//
// Verifying actual CR3 transitions is impossible from host; the
// bookkeeping balance is what matters here — once an ELF-loaded plugin
// has its own root, the same hook does the real switch.
#[test]
fn domain_switch_hook_brackets_every_native_dispatch() {
    use gos_protocol::NodeInstanceId;
    use std::sync::atomic::{AtomicU32, Ordering as AOrd};

    let _guard = TEST_LOCK.lock().expect("test lock");
    install_test_node();

    static ENTER_COUNT: AtomicU32 = AtomicU32::new(0);
    static LEAVE_COUNT: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn count_enter(_id: NodeInstanceId) -> u64 {
        ENTER_COUNT.fetch_add(1, AOrd::SeqCst);
        0xC0FFEE
    }
    unsafe extern "C" fn count_leave(token: u64) {
        assert_eq!(token, 0xC0FFEE, "leave must receive enter's token");
        LEAVE_COUNT.fetch_add(1, AOrd::SeqCst);
    }

    ENTER_COUNT.store(0, AOrd::SeqCst);
    LEAVE_COUNT.store(0, AOrd::SeqCst);

    gos_runtime::install_domain_switch(gos_runtime::DomainSwitch {
        enter: count_enter,
        leave: count_leave,
    });

    // Bind the test plugin to a real instance (non-ZERO) so the
    // trampoline guard activates.
    let _ = gos_runtime::bind_plugin_instance(TEST_PLUGIN_ID, NodeInstanceId::new(7));

    let _ = gos_runtime::route_signal(TEST_VECTOR, Signal::Spawn { payload: 0 });

    assert_eq!(ENTER_COUNT.load(AOrd::SeqCst), 1, "trampoline enter on dispatch");
    assert_eq!(
        LEAVE_COUNT.load(AOrd::SeqCst),
        1,
        "trampoline leave must balance enter"
    );
    assert_eq!(
        gos_runtime::domain_switch_count(),
        1,
        "runtime-level transition counter"
    );
}

#[test]
fn instance_binding_propagates_through_dispatch_and_clears_on_unbind() {
    use gos_protocol::NodeInstanceId;

    let _guard = TEST_LOCK.lock().expect("test lock");
    install_test_node();

    // Initial: no instance bound.
    assert_eq!(
        gos_runtime::instance_id_for_vec(TEST_VECTOR),
        Some(NodeInstanceId::ZERO)
    );

    // Bind via plugin-wide helper; every node of the plugin should pick
    // up the new instance id.
    let inst = NodeInstanceId::new(42);
    let bound = gos_runtime::bind_plugin_instance(TEST_PLUGIN_ID, inst);
    assert_eq!(bound, 1);
    assert_eq!(gos_runtime::instance_id_for_vec(TEST_VECTOR), Some(inst));

    // Re-bind to ZERO simulating module teardown — the runtime must
    // forget the prior id.
    let cleared = gos_runtime::bind_plugin_instance(TEST_PLUGIN_ID, NodeInstanceId::ZERO);
    assert_eq!(cleared, 1);
    assert_eq!(
        gos_runtime::instance_id_for_vec(TEST_VECTOR),
        Some(NodeInstanceId::ZERO)
    );
}
