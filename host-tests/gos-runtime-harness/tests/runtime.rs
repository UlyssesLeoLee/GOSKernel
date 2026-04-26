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
