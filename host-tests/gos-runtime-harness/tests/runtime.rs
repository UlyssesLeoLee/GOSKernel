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

// Phase G.3: socket ABI shape — install a stub vtable, drive an
// open->send->recv->close cycle, assert each callback receives the
// expected arguments and the SocketStatus decoder round-trips.
#[test]
fn socket_vtable_round_trips_open_send_recv_close() {
    use gos_protocol::socket::{
        SocketAddress, SocketDeviceVTable, SocketKind, SocketOptions, SocketStatus,
    };
    use std::sync::atomic::{AtomicU32, Ordering as AOrd};
    use std::sync::Mutex as StdMutex;

    static OPEN_HITS: AtomicU32 = AtomicU32::new(0);
    static SEND_HITS: AtomicU32 = AtomicU32::new(0);
    static RECV_HITS: AtomicU32 = AtomicU32::new(0);
    static CLOSE_HITS: AtomicU32 = AtomicU32::new(0);
    static LAST_ADDR: StdMutex<[u8; 16]> = StdMutex::new([0u8; 16]);

    unsafe extern "C" fn open(
        _h: u64,
        _kind: SocketKind,
        addr: *const SocketAddress,
        _opts: *const SocketOptions,
        out_socket: *mut u64,
    ) -> i32 {
        OPEN_HITS.fetch_add(1, AOrd::SeqCst);
        let a = unsafe { &*addr };
        *LAST_ADDR.lock().unwrap() = a.addr;
        unsafe { *out_socket = 7 };
        SocketStatus::Ok as i32
    }
    unsafe extern "C" fn send(
        _h: u64,
        socket: u64,
        _buf: *const u8,
        len: u32,
        out: *mut u32,
    ) -> i32 {
        SEND_HITS.fetch_add(1, AOrd::SeqCst);
        assert_eq!(socket, 7);
        unsafe { *out = len };
        SocketStatus::Ok as i32
    }
    unsafe extern "C" fn recv(
        _h: u64,
        _socket: u64,
        buf: *mut u8,
        len: u32,
        out: *mut u32,
    ) -> i32 {
        RECV_HITS.fetch_add(1, AOrd::SeqCst);
        let n = (len as usize).min(5);
        let dst = unsafe { core::slice::from_raw_parts_mut(buf, n) };
        dst.copy_from_slice(&b"hello"[..n]);
        unsafe { *out = n as u32 };
        SocketStatus::Ok as i32
    }
    unsafe extern "C" fn close(_h: u64, _socket: u64) -> i32 {
        CLOSE_HITS.fetch_add(1, AOrd::SeqCst);
        SocketStatus::Ok as i32
    }
    let vtable = SocketDeviceVTable {
        handle: 0xCAFE,
        open,
        send,
        recv,
        close,
    };

    OPEN_HITS.store(0, AOrd::SeqCst);
    SEND_HITS.store(0, AOrd::SeqCst);
    RECV_HITS.store(0, AOrd::SeqCst);
    CLOSE_HITS.store(0, AOrd::SeqCst);

    let addr = SocketAddress::ipv4(10, 0, 2, 2, 80);
    let opts = SocketOptions::default_blocking();
    let mut sock: u64 = 0;
    let rc = unsafe { (vtable.open)(vtable.handle, SocketKind::Tcp, &addr, &opts, &mut sock) };
    assert_eq!(SocketStatus::from_i32(rc), SocketStatus::Ok);
    assert_eq!(sock, 7);
    let last = *LAST_ADDR.lock().unwrap();
    assert_eq!(&last[..4], &[10, 0, 2, 2]);

    let payload = b"GET /";
    let mut sent = 0u32;
    let rc =
        unsafe { (vtable.send)(vtable.handle, sock, payload.as_ptr(), payload.len() as u32, &mut sent) };
    assert_eq!(SocketStatus::from_i32(rc), SocketStatus::Ok);
    assert_eq!(sent, payload.len() as u32);

    let mut buf = [0u8; 16];
    let mut got = 0u32;
    let rc = unsafe {
        (vtable.recv)(vtable.handle, sock, buf.as_mut_ptr(), buf.len() as u32, &mut got)
    };
    assert_eq!(SocketStatus::from_i32(rc), SocketStatus::Ok);
    assert_eq!(got, 5);
    assert_eq!(&buf[..5], b"hello");

    let rc = unsafe { (vtable.close)(vtable.handle, sock) };
    assert_eq!(SocketStatus::from_i32(rc), SocketStatus::Ok);

    assert_eq!(OPEN_HITS.load(AOrd::SeqCst), 1);
    assert_eq!(SEND_HITS.load(AOrd::SeqCst), 1);
    assert_eq!(RECV_HITS.load(AOrd::SeqCst), 1);
    assert_eq!(CLOSE_HITS.load(AOrd::SeqCst), 1);

    // SocketStatus error-class decoder round-trips for every variant.
    for s in [
        SocketStatus::Ok,
        SocketStatus::NotConnected,
        SocketStatus::TransportError,
        SocketStatus::BadBuffer,
        SocketStatus::Unbound,
        SocketStatus::OutOfHandles,
        SocketStatus::WouldBlock,
    ] {
        assert_eq!(SocketStatus::from_i32(s as i32), s);
    }
}

// Phase F.4: graph state journal — serialize a few envelopes through
// JournalRing, flush to a blob, replay, assert round-trip equality.
#[test]
fn journal_round_trips_envelopes_through_blob() {
    use gos_journal::{
        deserialize_envelope, replay, serialize_envelope, JournalError, JournalHeader,
        JournalRing, ENVELOPE_RECORD_BYTES, HEADER_BYTES,
    };
    use gos_protocol::{ControlPlaneEnvelope, ControlPlaneMessageKind};

    // ── 1. Single-record encode/decode round-trip. ──────────────────
    let env = ControlPlaneEnvelope {
        version: 1,
        kind: ControlPlaneMessageKind::NodeUpsert,
        subject: *b"K_SHELL\0\0\0\0\0\0\0\0\0",
        arg0: 0xDEAD_BEEF_CAFE_BABE,
        arg1: 42,
    };
    let mut record = [0u8; ENVELOPE_RECORD_BYTES];
    serialize_envelope(&env, &mut record);
    let decoded = deserialize_envelope(&record).expect("decode");
    assert_eq!(decoded.version, env.version);
    assert_eq!(decoded.kind, env.kind);
    assert_eq!(decoded.subject, env.subject);
    assert_eq!(decoded.arg0, env.arg0);
    assert_eq!(decoded.arg1, env.arg1);

    // ── 2. JournalRing append + flush + replay. ─────────────────────
    let mut ring: JournalRing<8> = JournalRing::new();
    let envelopes = [
        env,
        ControlPlaneEnvelope {
            version: 1,
            kind: ControlPlaneMessageKind::EdgeUpsert,
            subject: *b"K_NIM\0\0\0\0\0\0\0\0\0\0\0",
            arg0: 0x1111,
            arg1: 0x2222,
        },
        ControlPlaneEnvelope {
            version: 1,
            kind: ControlPlaneMessageKind::Fault,
            subject: *b"MOD.PROVIDER\0\0\0\0",
            arg0: 3,
            arg1: 5,
        },
    ];
    for env in &envelopes {
        ring.append(env).expect("append");
    }
    assert_eq!(ring.len(), 3);
    assert!(!ring.is_full());

    let mut blob = vec![0u8; HEADER_BYTES + 3 * ENVELOPE_RECORD_BYTES];
    let written = ring.flush_into(&mut blob).expect("flush");
    assert_eq!(written, blob.len());

    // Header parses cleanly.
    let header = JournalHeader::parse(&blob).expect("header");
    assert_eq!(header.version, 1);
    assert_eq!(header.record_size as usize, ENVELOPE_RECORD_BYTES);

    // Replay yields envelopes in order.
    let mut replayed = Vec::new();
    let n = replay(&blob, |env| replayed.push(env)).expect("replay");
    assert_eq!(n, 3);
    assert_eq!(replayed.len(), 3);
    for (a, b) in replayed.iter().zip(envelopes.iter()) {
        assert_eq!(a.version, b.version);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.subject, b.subject);
        assert_eq!(a.arg0, b.arg0);
        assert_eq!(a.arg1, b.arg1);
    }

    // ── 3. Bad header rejected. ──────────────────────────────────────
    let mut tampered = blob.clone();
    tampered[0] = b'X';
    match JournalHeader::parse(&tampered) {
        Err(JournalError::BadHeader) => {}
        other => panic!("expected BadHeader, got {:?}", other.map(|h| h.version)),
    }

    // Wrong version.
    let mut wrong_ver = blob.clone();
    wrong_ver[4..6].copy_from_slice(&99u16.to_le_bytes());
    match JournalHeader::parse(&wrong_ver) {
        Err(JournalError::UnsupportedVersion(99)) => {}
        other => panic!(
            "expected UnsupportedVersion(99), got {:?}",
            other.map(|h| h.version)
        ),
    }

    // Trailing garbage byte at the end.
    let mut trailing = blob.clone();
    trailing.push(0x55);
    match replay(&trailing, |_| {}) {
        Err(JournalError::TrailingBytes) => {}
        other => panic!("expected TrailingBytes, got {:?}", other),
    }
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

// Phase F.3.1: minimal FAT32 reader — hand-craft a 32 KiB FAT32 image
// in memory, expose it through BlockDeviceVTable, mount via k-fat32,
// and exercise the FileSystem trait end-to-end (lookup, read, readdir).
//
// Layout:
//   sector  0      — boot sector + BPB
//   sector  1      — FSInfo (we don't read it, just zero-fill)
//   sectors 2..3   — FAT 1 (single sector, 128 entries)
//   sectors 3..4   — FAT 2 (mirror)
//   sectors 4+     — data region; cluster N maps to sector 4 + (N-2)
//   cluster 2 (sector 4) — root directory: one file entry "HELLO.TXT"
//   cluster 3 (sector 5) — file content: b"hello, fat32"
#[test]
fn fat32_minimal_image_round_trips_lookup_read_and_readdir() {
    use gos_protocol::block::{
        BlockDeviceVTable, BlockGeometry, BlockIoStatus, BLOCK_SECTOR_SIZE_DEFAULT,
    };
    use gos_vfs::{DirEntry, FileSystem, InodeKind, MountId};
    use k_fat32::Fat32;
    use std::sync::Mutex as StdMutex;

    const TOTAL_SECTORS: usize = 64;
    const SECTOR_SIZE: usize = 512;
    static IMAGE: StdMutex<Vec<u8>> = StdMutex::new(Vec::new());

    {
        let mut img = IMAGE.lock().unwrap();
        img.clear();
        img.resize(TOTAL_SECTORS * SECTOR_SIZE, 0u8);

        // ── sector 0: boot sector + BPB ──────────────────────────────
        // OEM name (irrelevant)
        img[0..3].copy_from_slice(&[0xEB, 0x58, 0x90]); // jmp short
        img[3..0x0B].copy_from_slice(b"GOS_FAT3");

        img[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes()); // bytes/sector
        img[0x0D] = 1; // sectors/cluster
        img[0x0E..0x10].copy_from_slice(&2u16.to_le_bytes()); // reserved sectors
        img[0x10] = 2; // num FATs
        img[0x11..0x13].copy_from_slice(&0u16.to_le_bytes()); // root entry count (FAT32 = 0)
        img[0x13..0x15].copy_from_slice(&0u16.to_le_bytes()); // total sectors 16
        img[0x15] = 0xF8; // media descriptor
        img[0x16..0x18].copy_from_slice(&0u16.to_le_bytes()); // FATSz16 (FAT32 = 0)
        img[0x20..0x24].copy_from_slice(&(TOTAL_SECTORS as u32).to_le_bytes()); // total sectors 32
        img[0x24..0x28].copy_from_slice(&1u32.to_le_bytes()); // FATSz32
        img[0x2C..0x30].copy_from_slice(&2u32.to_le_bytes()); // root cluster
        img[0x1FE] = 0x55;
        img[0x1FF] = 0xAA;

        // ── FAT 1 + FAT 2: cluster 0 reserved, cluster 1 reserved,
        //    cluster 2 (root dir) = EOC, cluster 3 (file) = EOC ──────
        for fat in 0..2 {
            let off = (2 + fat) * SECTOR_SIZE;
            // entry 0/1: media + EOC sentinels
            img[off..off + 4].copy_from_slice(&0x0FFFFFF8u32.to_le_bytes());
            img[off + 4..off + 8].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
            // entry 2 (root dir cluster): EOC
            img[off + 8..off + 12].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
            // entry 3 (file cluster):    EOC
            img[off + 12..off + 16].copy_from_slice(&0x0FFFFFFFu32.to_le_bytes());
        }

        // ── root directory at sector 4 (cluster 2): one entry ───────
        let root_off = 4 * SECTOR_SIZE;
        let entry = &mut img[root_off..root_off + 32];
        entry[0..8].copy_from_slice(b"HELLO   "); // name padded with spaces
        entry[8..11].copy_from_slice(b"TXT"); // ext
        entry[11] = 0x20; // ATTR_ARCHIVE
        // first cluster split: high@20..22, low@26..28
        entry[20..22].copy_from_slice(&0u16.to_le_bytes());
        entry[26..28].copy_from_slice(&3u16.to_le_bytes()); // cluster 3
        entry[28..32].copy_from_slice(&12u32.to_le_bytes()); // file size

        // ── file content at sector 5 (cluster 3) ────────────────────
        let file_off = 5 * SECTOR_SIZE;
        img[file_off..file_off + 12].copy_from_slice(b"hello, fat32");
    }

    // BlockDevice vtable backed by the static IMAGE buffer.
    unsafe extern "C" fn read(_h: u64, lba: u64, buf: *mut u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let img = IMAGE.lock().unwrap();
        let off = (lba as usize) * SECTOR_SIZE;
        if off + SECTOR_SIZE > img.len() {
            return BlockIoStatus::OutOfBounds as i32;
        }
        let dst = unsafe { core::slice::from_raw_parts_mut(buf, SECTOR_SIZE) };
        dst.copy_from_slice(&img[off..off + SECTOR_SIZE]);
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn write(_h: u64, _lba: u64, _buf: *const u8, _len: u32) -> i32 {
        BlockIoStatus::DeviceError as i32 // read-only
    }
    unsafe extern "C" fn flush(_h: u64) -> i32 {
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn geometry(_h: u64) -> BlockGeometry {
        BlockGeometry {
            sector_count: TOTAL_SECTORS as u64,
            sector_size: BLOCK_SECTOR_SIZE_DEFAULT,
            flags: 1, // RO
        }
    }
    let vtable = BlockDeviceVTable {
        handle: 0,
        read_sector: read,
        write_sector: write,
        flush,
        geometry,
    };

    // ── Mount + verify BPB ───────────────────────────────────────────
    let fs = Fat32::mount(MountId(1), vtable).expect("mount FAT32");
    let bpb = fs.bpb();
    assert_eq!(bpb.bytes_per_sector, 512);
    assert_eq!(bpb.sectors_per_cluster, 1);
    assert_eq!(bpb.num_fats, 2);
    assert_eq!(bpb.root_cluster, 2);
    assert_eq!(bpb.data_start_sector, 4);

    // ── readdir ─────────────────────────────────────────────────────
    let root = fs.root();
    let mut entries = [DirEntry::empty(); 8];
    let (n, next) = fs.read_dir(root, 0, &mut entries).expect("readdir");
    assert_eq!(n, 1);
    assert_eq!(next, u64::MAX);
    assert_eq!(entries[0].name(), b"HELLO.TXT");
    assert_eq!(entries[0].inode.kind, InodeKind::File);
    assert_eq!(entries[0].inode.size_bytes, 12);

    // ── lookup + read ────────────────────────────────────────────────
    let hello = fs.lookup(root, b"HELLO.TXT").expect("lookup");
    assert_eq!(hello.kind, InodeKind::File);
    let mut buf = [0u8; 32];
    let n = fs.read(hello, 0, &mut buf).expect("read");
    assert_eq!(n, 12);
    assert_eq!(&buf[..n], b"hello, fat32");

    // ── partial read at offset ───────────────────────────────────────
    let n = fs.read(hello, 7, &mut buf).expect("offset read");
    assert_eq!(n, 5);
    assert_eq!(&buf[..n], b"fat32");

    // ── lookup miss ──────────────────────────────────────────────────
    use gos_vfs::VfsError;
    match fs.lookup(root, b"NOPE") {
        Err(VfsError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other.map(|i| i.num.0)),
    }
}

// Phase F.3.2 / F.3.3: FAT chain walking + subdirectory traversal.
// Build a 64 KiB image (128 sectors) with:
//   * a multi-cluster file (3 clusters, 1500-byte content)
//   * a subdirectory containing one file
// Verify read() walks the chain, lookup() recurses into the subdir,
// and read_dir() resumes correctly across cluster boundaries.
#[test]
fn fat32_walks_cluster_chains_and_subdirectories() {
    use gos_protocol::block::{
        BlockDeviceVTable, BlockGeometry, BlockIoStatus, BLOCK_SECTOR_SIZE_DEFAULT,
    };
    use gos_vfs::{DirEntry, FileSystem, InodeKind, MountId};
    use k_fat32::Fat32;
    use std::sync::Mutex as StdMutex;

    const TOTAL_SECTORS: usize = 128;
    const SECTOR_SIZE: usize = 512;
    static IMAGE2: StdMutex<Vec<u8>> = StdMutex::new(Vec::new());

    {
        let mut img = IMAGE2.lock().unwrap();
        img.clear();
        img.resize(TOTAL_SECTORS * SECTOR_SIZE, 0u8);

        // Layout
        //   sector 0       BPB
        //   sectors 2..4   FAT 1 (2 sectors)  — chain entries below
        //   sectors 4..6   FAT 2 (mirror)
        //   sector 6+      data, cluster N -> sector 6 + (N-2)
        //
        //   cluster 2 (root dir):    dir entries (BIGFILE, SUB, ...)
        //   clusters 3,4,5 (BIGFILE) chained: 3 -> 4 -> 5 -> EOC
        //   cluster 6 (SUB dir):     contains "INNER" file
        //   cluster 7 (INNER):       "from-subdir"
        // BPB
        img[0..3].copy_from_slice(&[0xEB, 0x58, 0x90]);
        img[3..0x0B].copy_from_slice(b"GOS_FAT3");
        img[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
        img[0x0D] = 1; // sectors/cluster
        img[0x0E..0x10].copy_from_slice(&2u16.to_le_bytes()); // reserved
        img[0x10] = 2;
        img[0x15] = 0xF8;
        img[0x16..0x18].copy_from_slice(&0u16.to_le_bytes());
        img[0x20..0x24].copy_from_slice(&(TOTAL_SECTORS as u32).to_le_bytes());
        img[0x24..0x28].copy_from_slice(&2u32.to_le_bytes()); // FATSz32 = 2 sectors
        img[0x2C..0x30].copy_from_slice(&2u32.to_le_bytes()); // root cluster
        img[0x1FE] = 0x55;
        img[0x1FF] = 0xAA;

        // FAT layout: cluster 0/1 reserved; 2 EOC; 3->4; 4->5; 5 EOC;
        // 6 EOC (subdir single cluster); 7 EOC (inner file).
        for fat in 0..2 {
            let off = (2 + fat * 2) * SECTOR_SIZE;
            // Reserved sentinels
            let put = |off: usize, idx: u32, val: u32, img: &mut [u8]| {
                let p = off + (idx as usize) * 4;
                img[p..p + 4].copy_from_slice(&val.to_le_bytes());
            };
            put(off, 0, 0x0FFFFFF8, &mut img);
            put(off, 1, 0x0FFFFFFF, &mut img);
            put(off, 2, 0x0FFFFFFF, &mut img); // root
            put(off, 3, 4, &mut img);            // BIGFILE chain
            put(off, 4, 5, &mut img);
            put(off, 5, 0x0FFFFFFF, &mut img);
            put(off, 6, 0x0FFFFFFF, &mut img); // SUB
            put(off, 7, 0x0FFFFFFF, &mut img); // INNER
        }

        // Root dir (sector 6 = cluster 2)
        let root_off = 6 * SECTOR_SIZE;
        // Entry 0: BIGFILE.BIN, cluster 3, size 1500
        {
            let e = &mut img[root_off..root_off + 32];
            e[0..8].copy_from_slice(b"BIGFILE ");
            e[8..11].copy_from_slice(b"BIN");
            e[11] = 0x20;
            e[26..28].copy_from_slice(&3u16.to_le_bytes());
            e[28..32].copy_from_slice(&1500u32.to_le_bytes());
        }
        // Entry 1: SUB (directory), cluster 6
        {
            let e = &mut img[root_off + 32..root_off + 64];
            e[0..8].copy_from_slice(b"SUB     ");
            e[8..11].copy_from_slice(b"   ");
            e[11] = ATTR_DIRECTORY_TEST;
            e[26..28].copy_from_slice(&6u16.to_le_bytes());
            e[28..32].copy_from_slice(&0u32.to_le_bytes());
        }

        // BIGFILE clusters (3,4,5).  Sectors 7,8,9.  Fill with
        // increasing pattern so we can detect crossing boundaries.
        for sec in 7..=9 {
            let off = sec * SECTOR_SIZE;
            for i in 0..512 {
                img[off + i] = ((sec - 7) * 100 + (i & 0xFF)) as u8;
            }
        }

        // SUB dir (cluster 6 = sector 10).
        {
            let sub_off = 10 * SECTOR_SIZE;
            let e = &mut img[sub_off..sub_off + 32];
            e[0..8].copy_from_slice(b"INNER   ");
            e[8..11].copy_from_slice(b"TXT");
            e[11] = 0x20;
            e[26..28].copy_from_slice(&7u16.to_le_bytes());
            e[28..32].copy_from_slice(&11u32.to_le_bytes());
        }

        // INNER content (cluster 7 = sector 11).
        let inner_off = 11 * SECTOR_SIZE;
        img[inner_off..inner_off + 11].copy_from_slice(b"from-subdir");
    }
    const ATTR_DIRECTORY_TEST: u8 = 0x10;

    unsafe extern "C" fn read(_h: u64, lba: u64, buf: *mut u8, len: u32) -> i32 {
        if len != BLOCK_SECTOR_SIZE_DEFAULT {
            return BlockIoStatus::BadBuffer as i32;
        }
        let img = IMAGE2.lock().unwrap();
        let off = (lba as usize) * SECTOR_SIZE;
        if off + SECTOR_SIZE > img.len() {
            return BlockIoStatus::OutOfBounds as i32;
        }
        let dst = unsafe { core::slice::from_raw_parts_mut(buf, SECTOR_SIZE) };
        dst.copy_from_slice(&img[off..off + SECTOR_SIZE]);
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn nowrite(_h: u64, _: u64, _: *const u8, _: u32) -> i32 {
        BlockIoStatus::DeviceError as i32
    }
    unsafe extern "C" fn flush(_h: u64) -> i32 {
        BlockIoStatus::Ok as i32
    }
    unsafe extern "C" fn geometry(_h: u64) -> BlockGeometry {
        BlockGeometry {
            sector_count: TOTAL_SECTORS as u64,
            sector_size: BLOCK_SECTOR_SIZE_DEFAULT,
            flags: 1,
        }
    }
    let vtable = BlockDeviceVTable {
        handle: 0,
        read_sector: read,
        write_sector: nowrite,
        flush,
        geometry,
    };

    let fs = Fat32::mount(MountId(2), vtable).expect("mount");
    let root = fs.root();

    // ── 1. Multi-cluster file: 1500 bytes spans clusters 3,4,5. ─────
    let big = fs.lookup(root, b"BIGFILE.BIN").expect("lookup BIGFILE");
    assert_eq!(big.size_bytes, 1500);
    let mut buf = vec![0u8; 2048];
    let n = fs.read(big, 0, &mut buf).expect("read BIGFILE");
    assert_eq!(n, 1500);
    // First byte of cluster 4 is at offset 512 — should encode (1*100 + 0)
    assert_eq!(buf[512], 100);
    // First byte of cluster 5 is at offset 1024 — should encode (2*100 + 0)
    assert_eq!(buf[1024], 200);
    // Past-EOF read returns 0
    let n2 = fs.read(big, 1500, &mut buf).expect("eof read");
    assert_eq!(n2, 0);
    // Mid-chain offset
    let n3 = fs.read(big, 1024, &mut buf).expect("offset read");
    assert_eq!(n3, 476);
    assert_eq!(buf[0], 200);

    // ── 2. Subdirectory traversal. ───────────────────────────────────
    let sub = fs.lookup(root, b"SUB").expect("lookup SUB");
    assert_eq!(sub.kind, InodeKind::Directory);
    let inner = fs.lookup(sub, b"INNER.TXT").expect("lookup INNER");
    assert_eq!(inner.kind, InodeKind::File);
    let n = fs.read(inner, 0, &mut buf).expect("read INNER");
    assert_eq!(&buf[..n], b"from-subdir");

    // ── 3. read_dir resumes across the cursor encoding. ─────────────
    let mut entries = [DirEntry::empty(); 1];
    let (n, cursor) = fs.read_dir(root, 0, &mut entries).expect("readdir page 1");
    assert_eq!(n, 1);
    assert_eq!(entries[0].name(), b"BIGFILE.BIN");
    assert_ne!(cursor, u64::MAX, "must continue with cursor");
    let (n, cursor) =
        fs.read_dir(root, cursor, &mut entries).expect("readdir page 2");
    assert_eq!(n, 1);
    assert_eq!(entries[0].name(), b"SUB");
    let (n, cursor) =
        fs.read_dir(root, cursor, &mut entries).expect("readdir page 3");
    assert_eq!(n, 0);
    assert_eq!(cursor, u64::MAX);
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

// Phase B.4.6.3 — full .gosmod load pipeline.  Build an ET_DYN with a
// PT_LOAD covering symtab/strtab/.rela.dyn, run load_etdyn, and assert:
//   * relocations applied
//   * module_init / module_event / module_stop offsets resolved
//   * image buffer carries the relocated 8-byte target
#[test]
fn gosmod_load_etdyn_pipelines_relocs_and_symbols() {
    use gos_loader::elf::{
        DT_NULL, DT_RELA, DT_RELAENT, DT_RELASZ, DT_STRSZ, DT_STRTAB, DT_SYMENT,
        DT_SYMTAB, ELF64_RELA_SIZE, ELF64_SYM_SIZE, PT_DYNAMIC, PT_LOAD,
    };
    use gos_loader::gosmod::{load_etdyn, LoadError};

    // Layout (everything inside one PT_LOAD so vaddr == file offset):
    //   0x000  ELF header
    //   0x040  PHDRs (2 * 56)
    //   0x0B0  PT_DYNAMIC payload (7 * 16 = 112 bytes)
    //   0x120  symtab (3 * 24 = 72)
    //   0x168  strtab (NUL + 3 names + sentinel)
    //   0x1C0  .rela.dyn (1 * 24)
    //   0x1D8  reloc target (8 bytes)
    //   ...
    let mut elf = vec![0u8; 0x300];
    let phoff: u64 = 0x40;
    let dyn_off: u64 = 0xB0;
    let symtab_off: u64 = 0x120;
    let strtab_off: u64 = 0x168;
    let rela_off: u64 = 0x1C0;
    let target_off: u64 = 0x1D8;

    let strtab = b"\0module_init\0module_event\0module_stop\0";
    let strsz = strtab.len() as u64;

    // ELF header
    elf[..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    elf[4] = 2;
    elf[5] = 1;
    elf[6] = 1;
    elf[7] = 0;
    elf[16..18].copy_from_slice(&3u16.to_le_bytes());
    elf[18..20].copy_from_slice(&62u16.to_le_bytes());
    elf[24..32].copy_from_slice(&0u64.to_le_bytes());
    elf[32..40].copy_from_slice(&phoff.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&2u16.to_le_bytes());

    // PT_LOAD covers entire file.
    let p0 = phoff as usize;
    elf[p0..p0 + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
    elf[p0 + 4..p0 + 8].copy_from_slice(&5u32.to_le_bytes());
    elf[p0 + 8..p0 + 16].copy_from_slice(&0u64.to_le_bytes()); // offset
    elf[p0 + 16..p0 + 24].copy_from_slice(&0u64.to_le_bytes()); // vaddr
    elf[p0 + 24..p0 + 32].copy_from_slice(&0u64.to_le_bytes()); // paddr
    elf[p0 + 32..p0 + 40].copy_from_slice(&0x300u64.to_le_bytes()); // filesz
    elf[p0 + 40..p0 + 48].copy_from_slice(&0x300u64.to_le_bytes()); // memsz

    // PT_DYNAMIC
    let p1 = p0 + 56;
    elf[p1..p1 + 4].copy_from_slice(&PT_DYNAMIC.to_le_bytes());
    elf[p1 + 4..p1 + 8].copy_from_slice(&6u32.to_le_bytes());
    elf[p1 + 8..p1 + 16].copy_from_slice(&dyn_off.to_le_bytes());
    elf[p1 + 16..p1 + 24].copy_from_slice(&dyn_off.to_le_bytes());
    elf[p1 + 24..p1 + 32].copy_from_slice(&0u64.to_le_bytes());
    elf[p1 + 32..p1 + 40].copy_from_slice(&112u64.to_le_bytes());
    elf[p1 + 40..p1 + 48].copy_from_slice(&112u64.to_le_bytes());

    // Dynamic payload
    let dyn_o = dyn_off as usize;
    let mut put = |i: usize, tag: u64, val: u64| {
        let b = dyn_o + i * 16;
        elf[b..b + 8].copy_from_slice(&tag.to_le_bytes());
        elf[b + 8..b + 16].copy_from_slice(&val.to_le_bytes());
    };
    put(0, DT_SYMTAB, symtab_off);
    put(1, DT_SYMENT, ELF64_SYM_SIZE as u64);
    put(2, DT_STRTAB, strtab_off);
    put(3, DT_STRSZ, strsz);
    put(4, DT_RELA, rela_off);
    put(5, DT_RELASZ, ELF64_RELA_SIZE as u64);
    put(6, DT_RELAENT, ELF64_RELA_SIZE as u64);
    // (DT_NULL would be put(7, ...) — but we sized the dynamic
    // payload at 7 entries; the parse loop terminates either on
    // DT_NULL or when out of bytes.  Pad with NULL just in case.)
    let _ = DT_NULL;

    // Symbol table — STN_UNDEF + 3 named entries
    let st_o = symtab_off as usize;
    let mut put_sym = |i: usize, name_off: u32, value: u64| {
        let b = st_o + i * ELF64_SYM_SIZE;
        elf[b..b + 4].copy_from_slice(&name_off.to_le_bytes());
        elf[b + 4] = 0;
        elf[b + 5] = 0;
        elf[b + 6..b + 8].copy_from_slice(&0u16.to_le_bytes());
        elf[b + 8..b + 16].copy_from_slice(&value.to_le_bytes());
        elf[b + 16..b + 24].copy_from_slice(&0u64.to_le_bytes());
    };
    put_sym(0, 0, 0);
    put_sym(1, 1, 0x100); // module_init -> offset 0x100
    put_sym(2, 13, 0x200); // module_event -> 0x200
    // We deliberately skip module_stop here to test the None case.

    let stt = strtab_off as usize;
    elf[stt..stt + strtab.len()].copy_from_slice(strtab);

    // RELA: one R_X86_64_RELATIVE @ target_off, addend = 0x42
    let rel = rela_off as usize;
    elf[rel..rel + 8].copy_from_slice(&target_off.to_le_bytes());
    elf[rel + 8..rel + 16].copy_from_slice(&8u64.to_le_bytes()); // type 8
    elf[rel + 16..rel + 24].copy_from_slice(&0x42u64.to_le_bytes());

    // Run the pipeline.
    let mut image = vec![0u8; 0x300];
    let image_base: u64 = 0xFFFF_8000_FACE_0000;
    let loaded = load_etdyn(&elf, &mut image, image_base).expect("load OK");

    assert_eq!(loaded.relocations_applied, 1);
    assert_eq!(loaded.module_init_offset, Some(0x100));
    assert_eq!(loaded.module_event_offset, Some(0x200));
    assert_eq!(loaded.module_stop_offset, None);
    assert!(loaded.image_size > 0);
    assert!(loaded.segment_count >= 1);

    // Reloc target now reads back as image_base + addend.
    let observed =
        u64::from_le_bytes(image[target_off as usize..target_off as usize + 8].try_into().unwrap());
    assert_eq!(observed, image_base + 0x42);

    // Mismatch: too-small image buffer surfaces as ImageBufferTooSmall.
    let mut tiny = vec![0u8; 16];
    match load_etdyn(&elf, &mut tiny, image_base) {
        Err(LoadError::ImageBufferTooSmall { required }) => {
            assert!(required > 16);
        }
        other => panic!("expected ImageBufferTooSmall, got {:?}", other),
    }
}

// Phase B.4.6.2 — dynamic symbol resolution.
//
// Hand-craft an ET_DYN with PT_LOAD covering the symtab + strtab and
// PT_DYNAMIC pointing at them.  Verify lookup_dynamic_symbol returns
// the right vaddr for "module_init" and None for unknown names.
#[test]
fn elf_lookup_dynamic_symbol_resolves_named_entries() {
    use gos_loader::elf::{
        parse, DT_NULL, DT_STRSZ, DT_STRTAB, DT_SYMENT, DT_SYMTAB, ELF64_SYM_SIZE,
        PT_DYNAMIC, PT_LOAD,
    };

    // We lay everything end-to-end inside a single PT_LOAD so vaddrs
    // and file offsets coincide (p_vaddr = p_offset = 0).  Layout:
    //   0x000  ELF header                (64 bytes)
    //   0x040  program headers (2 * 56)  (112 bytes)
    //   0x0B0  PT_DYNAMIC payload (5*16) (80 bytes)
    //   0x100  symtab — 3 sym entries    (3 * 24 = 72 bytes)
    //   0x148  strtab                    (variable)
    //          "\0module_init\0module_event\0"
    let mut elf = vec![0u8; 0x200];
    let phoff: u64 = 0x40;
    let dyn_offset: u64 = 0xB0;
    let symtab_offset: u64 = 0x100;
    let strtab_offset: u64 = 0x148;
    let strtab_bytes = b"\0module_init\0module_event\0";
    let strtab_size: u64 = strtab_bytes.len() as u64;

    // ── ELF header ───────────────────────────────────────────────────
    elf[..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    elf[4] = 2;
    elf[5] = 1;
    elf[6] = 1;
    elf[7] = 0;
    elf[16..18].copy_from_slice(&3u16.to_le_bytes());
    elf[18..20].copy_from_slice(&62u16.to_le_bytes());
    elf[24..32].copy_from_slice(&0u64.to_le_bytes());
    elf[32..40].copy_from_slice(&phoff.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&2u16.to_le_bytes());

    // ── PT_LOAD covering everything: vaddr=0, filesz=0x200 ──────────
    let ph0 = phoff as usize;
    elf[ph0..ph0 + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
    elf[ph0 + 4..ph0 + 8].copy_from_slice(&5u32.to_le_bytes());
    elf[ph0 + 8..ph0 + 16].copy_from_slice(&0u64.to_le_bytes());   // p_offset
    elf[ph0 + 16..ph0 + 24].copy_from_slice(&0u64.to_le_bytes()); // p_vaddr
    elf[ph0 + 24..ph0 + 32].copy_from_slice(&0u64.to_le_bytes()); // p_paddr
    elf[ph0 + 32..ph0 + 40].copy_from_slice(&0x200u64.to_le_bytes()); // p_filesz
    elf[ph0 + 40..ph0 + 48].copy_from_slice(&0x200u64.to_le_bytes()); // p_memsz

    // ── PT_DYNAMIC ───────────────────────────────────────────────────
    let ph1 = ph0 + 56;
    elf[ph1..ph1 + 4].copy_from_slice(&PT_DYNAMIC.to_le_bytes());
    elf[ph1 + 4..ph1 + 8].copy_from_slice(&6u32.to_le_bytes());
    elf[ph1 + 8..ph1 + 16].copy_from_slice(&dyn_offset.to_le_bytes());
    elf[ph1 + 16..ph1 + 24].copy_from_slice(&dyn_offset.to_le_bytes());
    elf[ph1 + 24..ph1 + 32].copy_from_slice(&0u64.to_le_bytes());
    elf[ph1 + 32..ph1 + 40].copy_from_slice(&80u64.to_le_bytes());
    elf[ph1 + 40..ph1 + 48].copy_from_slice(&80u64.to_le_bytes());

    // ── PT_DYNAMIC payload: SYMTAB / SYMENT / STRTAB / STRSZ / NULL
    let dyn_off = dyn_offset as usize;
    let mut put = |i: usize, tag: u64, val: u64| {
        let base = dyn_off + i * 16;
        elf[base..base + 8].copy_from_slice(&tag.to_le_bytes());
        elf[base + 8..base + 16].copy_from_slice(&val.to_le_bytes());
    };
    put(0, DT_SYMTAB, symtab_offset);
    put(1, DT_SYMENT, ELF64_SYM_SIZE as u64);
    put(2, DT_STRTAB, strtab_offset);
    put(3, DT_STRSZ, strtab_size);
    put(4, DT_NULL, 0);

    // ── Symbol table: STN_UNDEF, module_init, module_event ──────────
    let sym_off = symtab_offset as usize;
    let mut put_sym = |i: usize, name_off: u32, value: u64| {
        let base = sym_off + i * ELF64_SYM_SIZE;
        elf[base..base + 4].copy_from_slice(&name_off.to_le_bytes());
        elf[base + 4] = 0; // st_info
        elf[base + 5] = 0; // st_other
        elf[base + 6..base + 8].copy_from_slice(&0u16.to_le_bytes()); // st_shndx
        elf[base + 8..base + 16].copy_from_slice(&value.to_le_bytes()); // st_value
        elf[base + 16..base + 24].copy_from_slice(&0u64.to_le_bytes()); // st_size
    };
    put_sym(0, 0, 0);
    // strtab offset 1 -> "module_init"
    put_sym(1, 1, 0xCAFE);
    // strtab offset 13 -> "module_event"
    put_sym(2, 13, 0xBEEF);

    let strtab_off = strtab_offset as usize;
    elf[strtab_off..strtab_off + strtab_bytes.len()].copy_from_slice(strtab_bytes);

    // ── Resolve ─────────────────────────────────────────────────────
    let parsed = parse(&elf).expect("valid ELF");
    let init = parsed
        .lookup_dynamic_symbol(b"module_init")
        .expect("dyn lookup")
        .expect("symbol present");
    assert_eq!(init, 0xCAFE);
    let event = parsed
        .lookup_dynamic_symbol(b"module_event")
        .expect("dyn lookup")
        .expect("symbol present");
    assert_eq!(event, 0xBEEF);
    let stop = parsed
        .lookup_dynamic_symbol(b"module_stop")
        .expect("dyn lookup");
    assert!(stop.is_none(), "missing symbol must return None");
}

// Phase B.4.6.1 — R_X86_64_RELATIVE relocation walker.  Hand-craft an
// ET_DYN ELF with one PT_LOAD + one PT_DYNAMIC pointing at a one-entry
// .rela.dyn table; apply against a loaded-image buffer; verify the
// 8-byte target now contains image_base + addend.
#[test]
fn elf_apply_relative_relocations_patches_image_correctly() {
    use gos_loader::elf::{
        parse, ElfError, DT_NULL, DT_RELA, DT_RELAENT, DT_RELASZ, ELF64_RELA_SIZE,
        PT_DYNAMIC, PT_LOAD,
    };

    // Layout (file offsets):
    //   0x000  ELF header (64 bytes)
    //   0x040  program headers — 2 entries (each 56 bytes) = 112 bytes
    //   0x0B0  PT_DYNAMIC payload — 4 dyn entries * 16 = 64 bytes
    //   0x0F0  RELA table — 1 entry * 24 = 24 bytes
    //   total file = 0x108 (264 bytes)
    let mut elf = vec![0u8; 0x108];
    let phoff: u64 = 0x40;
    let dyn_offset: u64 = 0xB0;
    let dyn_size: u64 = 64;
    let rela_offset: u64 = 0xF0;
    let rela_size: u64 = ELF64_RELA_SIZE as u64;

    // ── e_ident + header ─────────────────────────────────────────────
    elf[..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    elf[4] = 2; // ELFCLASS64
    elf[5] = 1; // ELFDATA2LSB
    elf[6] = 1;
    elf[7] = 0;
    elf[16..18].copy_from_slice(&3u16.to_le_bytes()); // ET_DYN
    elf[18..20].copy_from_slice(&62u16.to_le_bytes()); // EM_X86_64
    elf[24..32].copy_from_slice(&0u64.to_le_bytes()); // e_entry
    elf[32..40].copy_from_slice(&phoff.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
    elf[56..58].copy_from_slice(&2u16.to_le_bytes()); // e_phnum

    // ── program header 0: PT_LOAD, vaddr 0, memsz 64, file 0..40 ─────
    let ph0 = phoff as usize;
    elf[ph0..ph0 + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
    elf[ph0 + 4..ph0 + 8].copy_from_slice(&5u32.to_le_bytes()); // R+X
    elf[ph0 + 8..ph0 + 16].copy_from_slice(&0u64.to_le_bytes()); // p_offset
    elf[ph0 + 16..ph0 + 24].copy_from_slice(&0u64.to_le_bytes()); // p_vaddr
    elf[ph0 + 24..ph0 + 32].copy_from_slice(&0u64.to_le_bytes()); // p_paddr
    elf[ph0 + 32..ph0 + 40].copy_from_slice(&40u64.to_le_bytes()); // p_filesz
    elf[ph0 + 40..ph0 + 48].copy_from_slice(&64u64.to_le_bytes()); // p_memsz

    // ── program header 1: PT_DYNAMIC ─────────────────────────────────
    let ph1 = ph0 + 56;
    elf[ph1..ph1 + 4].copy_from_slice(&PT_DYNAMIC.to_le_bytes());
    elf[ph1 + 4..ph1 + 8].copy_from_slice(&6u32.to_le_bytes()); // R+W
    elf[ph1 + 8..ph1 + 16].copy_from_slice(&dyn_offset.to_le_bytes());
    elf[ph1 + 16..ph1 + 24].copy_from_slice(&0u64.to_le_bytes());
    elf[ph1 + 24..ph1 + 32].copy_from_slice(&0u64.to_le_bytes());
    elf[ph1 + 32..ph1 + 40].copy_from_slice(&dyn_size.to_le_bytes());
    elf[ph1 + 40..ph1 + 48].copy_from_slice(&dyn_size.to_le_bytes());

    // ── PT_DYNAMIC payload: DT_RELA, DT_RELASZ, DT_RELAENT, DT_NULL ──
    let dyn_off = dyn_offset as usize;
    let mut put_dyn = |i: usize, tag: u64, val: u64| {
        let base = dyn_off + i * 16;
        elf[base..base + 8].copy_from_slice(&tag.to_le_bytes());
        elf[base + 8..base + 16].copy_from_slice(&val.to_le_bytes());
    };
    put_dyn(0, DT_RELA, rela_offset);
    put_dyn(1, DT_RELASZ, rela_size);
    put_dyn(2, DT_RELAENT, ELF64_RELA_SIZE as u64);
    put_dyn(3, DT_NULL, 0);

    // ── RELA table: one R_X86_64_RELATIVE @ image[0x10], addend=0x1234
    let rel = rela_offset as usize;
    elf[rel..rel + 8].copy_from_slice(&0x10u64.to_le_bytes()); // r_offset
    elf[rel + 8..rel + 16].copy_from_slice(&8u64.to_le_bytes()); // r_info: type 8 = RELATIVE
    elf[rel + 16..rel + 24].copy_from_slice(&0x1234u64.to_le_bytes()); // r_addend

    // Parse + apply.
    let parsed = parse(&elf).expect("valid ELF");
    let dyn_table = parsed.parse_dynamic().expect("dynamic table");
    assert_eq!(dyn_table.rela_offset, Some(rela_offset));
    assert_eq!(dyn_table.rela_size, rela_size);

    // Pretend the loader has copied PT_LOAD into a 64-byte image at
    // image_base = 0xFFFF_8000_DEAD_0000.
    let mut image = vec![0u8; 64];
    let image_base: u64 = 0xFFFF_8000_DEAD_0000;
    let count = parsed
        .apply_relative_relocations(&mut image, image_base)
        .expect("apply RELA");
    assert_eq!(count, 1);

    // image[0x10..0x18] == image_base + 0x1234 (little-endian).
    let expected = image_base + 0x1234;
    let observed = u64::from_le_bytes(image[0x10..0x18].try_into().unwrap());
    assert_eq!(observed, expected);

    // ── Tampering: rewrite the relocation to an unsupported type. ───
    let mut bad = elf.clone();
    bad[rel + 8..rel + 16].copy_from_slice(&1u64.to_le_bytes()); // R_X86_64_64
    let parsed_bad = parse(&bad).expect("still valid header");
    let mut image2 = vec![0u8; 64];
    match parsed_bad.apply_relative_relocations(&mut image2, image_base) {
        Err(ElfError::UnsupportedRelocation(1)) => {}
        other => panic!("expected UnsupportedRelocation(1), got {:?}", other),
    }
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
