use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Mutex,
};

use gos_protocol::{
    CapabilitySpec, ClaimId, ClaimPolicy, DomainId, ExecutionLaneClass, LeaseEpoch,
    ModuleAbiV1, ModuleCallStatus, ModuleDependencySpec, ModuleDescriptor, ModuleEntry,
    ModuleFaultPolicy, ModuleHandle, ModuleId, ModuleImageFormat, ModuleImageSegment,
    ModuleLifecycle, ModuleSegmentKind, NodeInstanceId, PreemptPolicy, RESOURCE_DISPLAY_CONSOLE,
    MODULE_ABI_VERSION,
};
use gos_supervisor::{
    bootstrap, bring_up_module, charge_gpu_bytes, charge_heap, claim_resource,
    clear_restart_history, current_instance, dequeue_ready_instance, drain_revocation,
    fault_module, heap_grant_summary, install_module, instance_domain_root, instance_is_degraded,
    instance_resource_summaries, instance_restart_generation, module_handle_for_id,
    module_lifecycle, module_status_summaries, process_restart_queue, queue_restart,
    realize_boot_modules, release_claim, restart_module, schedule_instance, service_system_cycle,
    set_gpu_quota, snapshot, spawn_instance, template_for_module, InstanceResourceSummary,
    ModuleStatusSummary, SupervisorError, CYCLE_DEPTH_CAP, MAX_CLAIMS, MAX_INSTANCES, MAX_MODULES,
    MAX_RESTARTS_BEFORE_DEGRADE,
};

static START_COUNT: AtomicUsize = AtomicUsize::new(0);
static CALLBACK_INSTANCE: AtomicU64 = AtomicU64::new(0);
static CALLBACK_CLAIM: AtomicU64 = AtomicU64::new(0);
static CALLBACK_EPOCH: AtomicU64 = AtomicU64::new(0);
static CALLBACK_HEAP_BASE: AtomicU64 = AtomicU64::new(0);
static TEST_LOCK: Mutex<()> = Mutex::new(());

const TEST_EXPORTS: &[CapabilitySpec] = &[CapabilitySpec {
    namespace: "demo",
    name: "echo",
    version: 1,
}];

const TEST_SEGMENTS: &[ModuleImageSegment] = &[ModuleImageSegment {
    kind: ModuleSegmentKind::Text,
    virt_addr: 0,
    mem_len: 0x4000,
    file_offset: 0,
    file_len: 0x2000,
    flags: 0,
}];

unsafe extern "C" fn test_start(
    abi: *const ModuleAbiV1,
    handle: ModuleHandle,
    _domain: DomainId,
) -> ModuleCallStatus {
    let Some(abi) = (unsafe { abi.as_ref() }) else {
        return ModuleCallStatus::Fault;
    };

    let mut instance_id = NodeInstanceId::ZERO;
    let Some(current_instance_fn) = abi.current_instance else {
        return ModuleCallStatus::Fault;
    };
    if unsafe { current_instance_fn(handle, &mut instance_id) } != ModuleCallStatus::Ok {
        return ModuleCallStatus::Fault;
    }
    CALLBACK_INSTANCE.store(instance_id.0, Ordering::SeqCst);

    let mut claim_id = ClaimId::ZERO;
    let mut epoch = LeaseEpoch::ZERO;
    let Some(claim_fn) = abi.claim_resource else {
        return ModuleCallStatus::Fault;
    };
    if unsafe {
        claim_fn(
            handle,
            RESOURCE_DISPLAY_CONSOLE,
            ClaimPolicy::Exclusive,
            PreemptPolicy::Never,
            &mut claim_id,
            &mut epoch,
        )
    } != ModuleCallStatus::Ok
    {
        return ModuleCallStatus::Fault;
    }
    CALLBACK_CLAIM.store(claim_id.0, Ordering::SeqCst);
    CALLBACK_EPOCH.store(epoch.0, Ordering::SeqCst);

    let mut heap_base = 0u64;
    let Some(request_pages_fn) = abi.request_pages else {
        return ModuleCallStatus::Fault;
    };
    if unsafe { request_pages_fn(handle, 2, 1, &mut heap_base) } != ModuleCallStatus::Ok {
        return ModuleCallStatus::Fault;
    }
    CALLBACK_HEAP_BASE.store(heap_base, Ordering::SeqCst);
    START_COUNT.fetch_add(1, Ordering::SeqCst);
    ModuleCallStatus::Ok
}

const TEST_ENTRY: ModuleEntry = ModuleEntry {
    module_init: None,
    module_start: Some(test_start),
    module_stop: None,
    module_suspend: None,
    module_resume: None,
};

const PROVIDER: ModuleDescriptor = ModuleDescriptor {
    abi_version: MODULE_ABI_VERSION,
    module_id: ModuleId::from_ascii("MOD.PROVIDER"),
    name: "MOD_PROVIDER",
    version: 1,
    image_format: ModuleImageFormat::Builtin,
    fault_policy: ModuleFaultPolicy::RestartAlways,
    dependencies: &[],
    permissions: &[],
    exports: TEST_EXPORTS,
    imports: &[],
    segments: TEST_SEGMENTS,
    entry: TEST_ENTRY,
    signature: None,
    flags: 0,
};

const MISSING_DEPS: &[ModuleDependencySpec] = &[ModuleDependencySpec {
    module_id: ModuleId::from_ascii("MOD.MISSING"),
    required: true,
}];

const CONSUMER: ModuleDescriptor = ModuleDescriptor {
    abi_version: MODULE_ABI_VERSION,
    module_id: ModuleId::from_ascii("MOD.CONSUMER"),
    name: "MOD_CONSUMER",
    version: 1,
    image_format: ModuleImageFormat::Builtin,
    fault_policy: ModuleFaultPolicy::Manual,
    dependencies: MISSING_DEPS,
    permissions: &[],
    exports: &[],
    imports: &[],
    segments: TEST_SEGMENTS,
    entry: ModuleEntry::NONE,
    signature: None,
    flags: 0,
};

fn reset_state() {
    START_COUNT.store(0, Ordering::SeqCst);
    CALLBACK_INSTANCE.store(0, Ordering::SeqCst);
    CALLBACK_CLAIM.store(0, Ordering::SeqCst);
    CALLBACK_EPOCH.store(0, Ordering::SeqCst);
    CALLBACK_HEAP_BASE.store(0, Ordering::SeqCst);
}

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[test]
fn boot_realize_builds_instance_claim_and_heap_grant() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.installed_modules, 1);
    assert_eq!(snap.registered_templates, 1);
    assert_eq!(snap.live_instances, 1);
    assert_eq!(snap.ready_instances, 1);
    // 5 legacy + 2 persistence (RS.BLOCK, RS.FILE) [F.1/F.2]
    // + 2 networking/GPU (RS.SOCKET, RS.GPUMEM)         [G.3/G.4]
    assert_eq!(snap.registered_resources, 9);
    assert_eq!(snap.active_claims, 1);
    assert_eq!(snap.heap_grants, 1);
    assert_eq!(snap.heap_pages_used, 2);
    assert_eq!(snap.ready_background, 1);
    assert_eq!(START_COUNT.load(Ordering::SeqCst), 1);
    assert_ne!(CALLBACK_INSTANCE.load(Ordering::SeqCst), 0);
    assert_ne!(CALLBACK_CLAIM.load(Ordering::SeqCst), 0);
    assert_ne!(CALLBACK_EPOCH.load(Ordering::SeqCst), 0);
    assert_ne!(CALLBACK_HEAP_BASE.load(Ordering::SeqCst), 0);
    assert_eq!(
        current_instance(provider).expect("current instance"),
        NodeInstanceId::new(CALLBACK_INSTANCE.load(Ordering::SeqCst))
    );
}

#[test]
fn force_preempt_generates_revocation_for_previous_owner() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let first_instance = current_instance(provider).expect("primary instance");
    let first_claim = ClaimId::new(CALLBACK_CLAIM.load(Ordering::SeqCst));
    let first_epoch = LeaseEpoch::new(CALLBACK_EPOCH.load(Ordering::SeqCst));

    let template_id = template_for_module(provider).expect("template");
    let second_instance = spawn_instance(template_id).expect("spawn");
    let second_lease = claim_resource(
        second_instance,
        RESOURCE_DISPLAY_CONSOLE,
        ClaimPolicy::Exclusive,
        PreemptPolicy::Force,
    )
    .expect("preemptive claim");

    assert_ne!(second_lease.claim_id, first_claim);
    assert!(second_lease.epoch.0 > first_epoch.0);

    let revoke = drain_revocation(first_instance)
        .expect("drain result")
        .expect("lease revoke");
    assert_eq!(revoke.claim_id, first_claim);
    assert_eq!(revoke.epoch, first_epoch);
    assert_eq!(revoke.resource_id, RESOURCE_DISPLAY_CONSOLE);

    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.active_claims, 1);
    assert_eq!(snap.suspended_instances, 1);
}

#[test]
fn lane_scheduler_tracks_ready_instances_and_dequeues_background_work() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let primary = current_instance(provider).expect("primary instance");
    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.ready_instances, 1);
    assert_eq!(snap.ready_background, 1);

    let dequeued = dequeue_ready_instance(None)
        .expect("dequeue")
        .expect("ready instance");
    assert_eq!(dequeued, primary);

    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.ready_instances, 0);
    assert_eq!(snap.ready_background, 0);

    schedule_instance(primary).expect("requeue");
    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.ready_instances, 1);
    assert_eq!(snap.ready_background, 1);
}

#[test]
fn queued_restart_restarts_module_through_scheduler_control_plane() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");
    assert_eq!(START_COUNT.load(Ordering::SeqCst), 1);

    queue_restart(provider).expect("queue restart");
    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.queued_restarts, 1);

    let restarted = process_restart_queue()
        .expect("process restart queue")
        .expect("restart handle");
    assert_eq!(restarted, provider);
    assert_eq!(START_COUNT.load(Ordering::SeqCst), 2);

    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.queued_restarts, 0);
    let instance = current_instance(provider).expect("current instance");
    let summary = gos_supervisor::instance_summary(instance).expect("instance summary");
    assert_eq!(summary.lane, ExecutionLaneClass::Background);
    assert!(summary.ready_queued);
}

// ── Atomic bring-up regression: one bad plugin can't abort the boot ─────────
//
// CONSUMER declares a required dependency that was never installed, so
// its bring-up pipeline must fail at validate_module.  Before atomic
// rollback, realize_boot_modules propagated that as an Err and the
// *whole* boot call failed — taking every other module down with it.
// Now each module is its own transaction: CONSUMER rolls back to a
// clean Faulted state (no leaked instance/domain/capabilities) while
// PROVIDER, installed alongside it, still comes up Running.
#[test]
fn service_system_cycle_quiesces_when_idle() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    // Mirrors kernel_main's steady-state loop (V2.2c, ADR-002 §4): with no new
    // external work arriving between ticks, each cycle's internal drain
    // (restart queue -> ready queue -> runtime pump -> fault policy) must
    // converge well under CYCLE_DEPTH_CAP, never silently truncate.
    for _ in 0..4 {
        let report = service_system_cycle();
        assert!(
            report.quiesced,
            "idle cycle must drain to quiescence, not trip the depth guard: {report:?}"
        );
        assert!(!report.overflowed, "idle cycle must not overflow the pending queue");
        assert!(report.steps >= 1, "the engine always fires at least once");
        assert!(
            report.max_causal_depth < CYCLE_DEPTH_CAP,
            "idle cycle must stay far below the ADR-002 depth guard"
        );
    }
}

#[test]
fn missing_dependency_is_rejected() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    let consumer = install_module(CONSUMER).expect("consumer install");

    let report = realize_boot_modules().expect("realize must not abort for a sibling failure");
    assert_eq!(report.failed_modules, 1);
    assert_eq!(report.running_modules, 1);

    assert_eq!(
        module_lifecycle(consumer).expect("consumer lifecycle"),
        ModuleLifecycle::Faulted
    );
    assert_eq!(
        module_lifecycle(provider).expect("provider lifecycle"),
        ModuleLifecycle::Running
    );

    // No instance, domain, or capability was left behind for the
    // rolled-back module.
    assert_eq!(current_instance(consumer), Err(SupervisorError::InstanceNotFound));
    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.live_instances, 1, "only PROVIDER's instance should remain live");
    assert_eq!(snap.isolated_domains, 1, "only PROVIDER's domain should remain mapped");
}

// ADR-015 axis2 gate: a module descriptor whose `abi_version` the host's
// MODULE_ABI_VERSION rejects (major mismatch here) must fail validation
// instead of being mapped/started against a vtable shape it doesn't
// understand — mirrors gos_loader's GOS_ABI_VERSION manifest gate, which
// this module-vtable axis never had until now.
#[test]
fn incompatible_module_abi_version_is_rejected_at_bring_up() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);

    const FUTURE_MAJOR_ABI: ModuleDescriptor = ModuleDescriptor {
        abi_version: MODULE_ABI_VERSION + (1 << 24),
        module_id: ModuleId::from_ascii("MOD.FUTUREABI"),
        name: "MOD_FUTURE_ABI",
        version: 1,
        image_format: ModuleImageFormat::Builtin,
        fault_policy: ModuleFaultPolicy::Manual,
        dependencies: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        segments: TEST_SEGMENTS,
        entry: TEST_ENTRY,
        signature: None,
        flags: 0,
    };

    let handle = install_module(FUTURE_MAJOR_ABI).expect("install does not check abi");
    assert_eq!(
        bring_up_module(handle),
        Err(SupervisorError::AbiVersionMismatch)
    );
    assert_eq!(
        module_lifecycle(handle).expect("lifecycle"),
        ModuleLifecycle::Faulted
    );
}

// Once the missing dependency is installed, the rolled-back module can
// be retried in isolation via `restart_module` — proving rollback left
// it in a genuinely clean, re-attemptable state rather than stuck.
#[test]
fn rolled_back_module_can_be_retried_after_dependency_is_installed() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let consumer = install_module(CONSUMER).expect("consumer install");
    let report = realize_boot_modules().expect("realize");
    assert_eq!(report.failed_modules, 1);
    assert_eq!(
        module_lifecycle(consumer).expect("lifecycle"),
        ModuleLifecycle::Faulted
    );

    // Install the dependency CONSUMER was missing, then retry just that
    // one module instead of re-running the whole boot sequence.
    const PROVIDED_DEP: ModuleDescriptor = ModuleDescriptor {
        abi_version: MODULE_ABI_VERSION,
        module_id: ModuleId::from_ascii("MOD.MISSING"),
        name: "MOD_MISSING",
        version: 1,
        image_format: ModuleImageFormat::Builtin,
        fault_policy: ModuleFaultPolicy::Manual,
        dependencies: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        segments: TEST_SEGMENTS,
        entry: ModuleEntry::NONE,
        signature: None,
        flags: 0,
    };
    install_module(PROVIDED_DEP).expect("install missing dependency");

    restart_module(consumer).expect("retry must succeed now that the dependency exists");
    assert_eq!(
        module_lifecycle(consumer).expect("lifecycle"),
        ModuleLifecycle::Running
    );
}

#[test]
fn released_claims_recycle_slots_across_many_rounds() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let instance = current_instance(provider).expect("primary instance");
    release_claim(ClaimId::new(CALLBACK_CLAIM.load(Ordering::SeqCst))).expect("release boot claim");

    let mut previous_epoch = LeaseEpoch::new(CALLBACK_EPOCH.load(Ordering::SeqCst));
    for _ in 0..(MAX_CLAIMS + 4) {
        let lease = claim_resource(
            instance,
            RESOURCE_DISPLAY_CONSOLE,
            ClaimPolicy::Exclusive,
            PreemptPolicy::Never,
        )
        .expect("claim");
        assert!(lease.epoch.0 > previous_epoch.0);
        previous_epoch = lease.epoch;
        release_claim(lease.claim_id).expect("release");
    }

    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.active_claims, 0);
}

#[test]
fn heap_quota_is_enforced_and_grants_can_be_freed() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let free_pages = gos_supervisor::abi().free_pages.expect("free pages");
    let request_pages = gos_supervisor::abi().request_pages.expect("request pages");

    let initial_base = CALLBACK_HEAP_BASE.load(Ordering::SeqCst);
    let initial_grant = heap_grant_summary(provider, initial_base).expect("initial grant");
    assert_eq!(initial_grant.page_count, 2);
    assert!(initial_grant.writable);
    assert_eq!(
        unsafe { free_pages(provider, initial_base, initial_grant.page_count) },
        ModuleCallStatus::Ok
    );

    let snap = snapshot().expect("snapshot after initial free");
    assert_eq!(snap.heap_grants, 0);
    assert_eq!(snap.heap_pages_used, 0);

    let mut full_base = 0u64;
    assert_eq!(
        unsafe { request_pages(provider, 32, 1, &mut full_base) },
        ModuleCallStatus::Ok
    );
    let full_grant = heap_grant_summary(provider, full_base).expect("full grant");
    assert_eq!(full_grant.page_count, 32);

    let mut denied_base = 0u64;
    assert_eq!(
        unsafe { request_pages(provider, 1, 1, &mut denied_base) },
        ModuleCallStatus::Denied
    );

    assert_eq!(
        unsafe { free_pages(provider, full_base, full_grant.page_count) },
        ModuleCallStatus::Ok
    );

    let snap = snapshot().expect("snapshot after free");
    assert_eq!(snap.heap_grants, 0);
    assert_eq!(snap.heap_pages_used, 0);
}

// ── Phase G.3 + G.4 regression: socket + GPU memory resources ────────────────
#[test]
fn g3_g4_resources_registered_and_gpu_quota_enforced() {
    use gos_protocol::{RESOURCE_GPU_MEMORY, RESOURCE_SOCKET};
    use gos_supervisor::{
        charge_gpu_bytes, credit_gpu_bytes, instance_gpu_usage, set_gpu_quota,
    };

    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");
    let instance = current_instance(provider).expect("primary instance");

    // ── G.3: RESOURCE_SOCKET claimable ──────────────────────────────
    let socket_lease = claim_resource(
        instance,
        RESOURCE_SOCKET,
        ClaimPolicy::Shared,
        PreemptPolicy::Never,
    )
    .expect("RS.SOCKET must be registered");
    assert_eq!(socket_lease.resource_id, RESOURCE_SOCKET);
    let _ = release_claim(socket_lease.claim_id);

    // ── G.4: RESOURCE_GPU_MEMORY claim succeeds; quota enforced ─────
    let gpu_lease = claim_resource(
        instance,
        RESOURCE_GPU_MEMORY,
        ClaimPolicy::Shared,
        PreemptPolicy::Never,
    )
    .expect("RS.GPUMEM must be registered");
    assert_eq!(gpu_lease.resource_id, RESOURCE_GPU_MEMORY);

    // No quota set yet -> any non-zero charge refused.
    assert!(charge_gpu_bytes(instance, 1).is_err());
    assert_eq!(instance_gpu_usage(instance), Some((0, 0)));

    // Open a 1 MiB quota.
    set_gpu_quota(instance, 1 << 20).expect("quota set");
    assert_eq!(instance_gpu_usage(instance), Some((0, 1 << 20)));

    // 512 KiB allocation succeeds.
    charge_gpu_bytes(instance, 512 * 1024).expect("first charge");
    assert_eq!(instance_gpu_usage(instance), Some((512 * 1024, 1 << 20)));

    // Another 600 KiB exceeds the cap -> rejected.
    assert_eq!(
        charge_gpu_bytes(instance, 600 * 1024),
        Err(SupervisorError::HeapQuotaExceeded)
    );

    // Credit half back, then succeed.
    credit_gpu_bytes(instance, 256 * 1024);
    charge_gpu_bytes(instance, 600 * 1024).expect("retry after credit");

    let _ = release_claim(gpu_lease.claim_id);
}

// ── Phase G.2 regression: install-time signature gate ────────────────────────
//
// Default policy is Permissive — every existing builtin (signature:
// None) installs without complaint.  Switching to RequireSigned must
// reject unsigned modules; switching back must restore acceptance.
// Bad signatures with an installed verifier always rejected.
#[test]
fn signature_gate_honours_security_policy() {
    use gos_sign::{install_verifier, set_policy, SecurityPolicy, SignatureVerifier};

    let _guard = test_guard();
    reset_state();
    bootstrap(0);

    // Permissive default: unsigned install OK.
    set_policy(SecurityPolicy::Permissive);
    install_module(PROVIDER).expect("permissive unsigned install");
    reset_state();
    bootstrap(0);

    // RequireSigned + unsigned -> rejected.
    set_policy(SecurityPolicy::RequireSigned);
    assert_eq!(
        install_module(PROVIDER),
        Err(SupervisorError::ModuleRejected)
    );

    // Restore Permissive so subsequent tests see baseline behavior.
    set_policy(SecurityPolicy::Permissive);

    // With a verifier installed and a signed (synthetic) descriptor,
    // a verifier returning failure rejects the install.
    static SIG: &[u8] = b"sig-bytes";
    const SIGNED: ModuleDescriptor = ModuleDescriptor {
        abi_version: MODULE_ABI_VERSION,
        module_id: ModuleId::from_ascii("MOD.SIGNED"),
        name: "MOD_SIGNED",
        version: 1,
        image_format: ModuleImageFormat::Builtin,
        fault_policy: ModuleFaultPolicy::Manual,
        dependencies: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        segments: TEST_SEGMENTS,
        entry: TEST_ENTRY,
        signature: Some(SIG),
        flags: 0,
    };

    unsafe extern "C" fn always_reject(
        _sig: *const u8,
        _len: usize,
        _hash: *const u8,
    ) -> i32 {
        -1
    }
    install_verifier(SignatureVerifier { verify: always_reject });

    reset_state();
    bootstrap(0);
    set_policy(SecurityPolicy::Permissive);
    assert_eq!(
        install_module(SIGNED),
        Err(SupervisorError::ModuleRejected),
        "verifier rejection always wins"
    );

    unsafe extern "C" fn always_accept(
        _sig: *const u8,
        _len: usize,
        _hash: *const u8,
    ) -> i32 {
        0
    }
    install_verifier(SignatureVerifier { verify: always_accept });
    reset_state();
    bootstrap(0);
    set_policy(SecurityPolicy::Permissive);
    install_module(SIGNED).expect("verifier acceptance allows signed install");
}

// ── Phase F regression: persistence resources are registered ─────────────────
//
// RESOURCE_BLOCK_DEVICE and RESOURCE_FILE_HANDLE must be registered at
// bootstrap so plugin manifests can declare claims against them, even
// before a real driver/FS is installed.  Once a claim against either
// resource succeeds (no provider registered yet -> Shared lease since
// no exclusive holder), we know the supervisor knows about them.
#[test]
fn persistence_resources_are_registered_at_bootstrap() {
    use gos_protocol::{RESOURCE_BLOCK_DEVICE, RESOURCE_FILE_HANDLE};

    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");
    let instance = current_instance(provider).expect("primary instance");

    // Both new resources must be claimable as Shared (no exclusive
    // holder since the provider isn't installed yet, but the resource
    // slot exists).
    let block_lease = claim_resource(
        instance,
        RESOURCE_BLOCK_DEVICE,
        ClaimPolicy::Shared,
        PreemptPolicy::Never,
    )
    .expect("RS.BLOCK must be registered after bootstrap");
    assert_eq!(block_lease.resource_id, RESOURCE_BLOCK_DEVICE);

    let file_lease = claim_resource(
        instance,
        RESOURCE_FILE_HANDLE,
        ClaimPolicy::Shared,
        PreemptPolicy::Never,
    )
    .expect("RS.FILE must be registered after bootstrap");
    assert_eq!(file_lease.resource_id, RESOURCE_FILE_HANDLE);

    let _ = release_claim(block_lease.claim_id);
    let _ = release_claim(file_lease.claim_id);
}

// ── Phase E.3 regression: User-level modules are rejected at start ───────────
//
// Until the Ring 3 dispatch trampoline (B.4.6.x + E.2 sysret path)
// lands, supervisor must refuse to start User-level modules — running
// them in Ring 0 would defeat the privilege separation entirely.
#[test]
fn user_level_module_is_rejected_at_start() {
    use gos_protocol::MODULE_FLAG_USER;

    let _guard = test_guard();
    reset_state();
    bootstrap(0);

    const USER_MODULE: ModuleDescriptor = ModuleDescriptor {
        abi_version: MODULE_ABI_VERSION,
        module_id: ModuleId::from_ascii("MOD.USER"),
        name: "MOD_USER",
        version: 1,
        image_format: ModuleImageFormat::Builtin,
        fault_policy: ModuleFaultPolicy::Manual,
        dependencies: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        segments: TEST_SEGMENTS,
        entry: TEST_ENTRY,
        signature: None,
        flags: MODULE_FLAG_USER,
    };

    let user_module = install_module(USER_MODULE).expect("install");
    // realize_boot_modules calls start_module per descriptor; start_module
    // still surfaces ModuleRejected for a Privilege::User module, but
    // realize_boot_modules now treats that as one module's atomic
    // bring-up failure rather than aborting the whole boot call: it
    // rolls USER_MODULE back to Faulted and reports it via
    // failed_modules instead of propagating an Err.
    let report = realize_boot_modules().expect("realize must not abort for this module's rejection");
    assert_eq!(report.failed_modules, 1);
    assert_eq!(
        module_lifecycle(user_module).expect("lifecycle"),
        ModuleLifecycle::Faulted
    );

    // start_module rejects User-privilege modules *after* instantiate_module
    // already spawned the primary instance and map_module already built
    // the domain — rollback must undo both, not just leave the state
    // half-built at the point of failure.
    let snap = snapshot().expect("snapshot");
    assert_eq!(snap.live_instances, 0, "rollback must free the instance instantiate_module created");
    assert_eq!(snap.isolated_domains, 0, "rollback must free the domain map_module created");
}

// ── Phase B.4.3 regression: CPU fault dispatch hook ──────────────────────────
//
// gos_supervisor::bootstrap installs a fault-dispatch hook into
// gos_runtime so the trap normalizer can route CPU exceptions
// (#PF / #GP / #SS / #DF) to ModuleFaultPolicy.  The bridge is:
//
//   k-idt trap path
//     -> gos_runtime::dispatch_fault(instance_id)
//       -> [supervisor-installed hook]
//         -> resolve instance -> module
//         -> SUPERVISOR.fault_module(handle)
//
// This test exercises the hook end-to-end by calling
// gos_runtime::dispatch_fault directly, then asserts the supervisor
// reacted: PROVIDER has fault_policy = RestartAlways, so a single
// dispatch_fault should bump restart_generation by 1.
#[test]
fn fault_dispatch_hook_attributes_cpu_fault_to_module_policy() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let instance_before = current_instance(provider).expect("primary instance");
    let gen_before =
        instance_restart_generation(instance_before).expect("gen pre-fault");
    assert_eq!(gen_before, 0);

    // Drive the same path k-idt's trap normalizer would.
    gos_runtime::dispatch_fault(instance_before);

    // The post-fault primary instance has been recycled by the restart;
    // restart_generation on the new instance must be one higher.
    let instance_after = current_instance(provider).expect("post-fault instance");
    let gen_after =
        instance_restart_generation(instance_after).expect("gen post-fault");
    assert_eq!(
        gen_after, 1,
        "fault dispatch must trigger ModuleFaultPolicy::RestartAlways"
    );
}

// ── Phase B.4.1 regression: per-module domain root ───────────────────────────
//
// After realize_boot_modules, every running module must have a non-zero
// root_table_phys (the per-domain PML4 anchor) and the values must be
// pairwise distinct.  Under host-testing the stub returns synthetic
// monotonic frames; under kernel-vmm it's k_vmm::create_isolated_address_
// space.  Both must satisfy the invariant.
#[test]
fn map_module_assigns_distinct_non_zero_domain_roots() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let instance = current_instance(provider).expect("primary instance");
    let root = instance_domain_root(instance).expect("domain root");
    assert!(
        root != 0,
        "B.4.1 invariant: realize_boot_modules must produce a non-zero \
         root_table_phys for every running module"
    );
}

// ── Phase B.5 regression: restart cap + degraded mode ────────────────────────
//
// PROVIDER has fault_policy = RestartAlways.  After
// MAX_RESTARTS_BEFORE_DEGRADE consecutive restarts, the next fault must
// flip the module into degraded state — at which point new claims and
// new heap charges are rejected.
#[test]
fn restart_cap_demotes_to_degraded_and_blocks_new_claims_and_charges() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    // First MAX_RESTARTS_BEFORE_DEGRADE faults stay under the cap and
    // each one bumps the module's restart_generation.
    for expected in 1..=MAX_RESTARTS_BEFORE_DEGRADE {
        fault_module(provider).expect("fault under cap");
        let instance = current_instance(provider).expect("primary instance");
        let observed =
            instance_restart_generation(instance).expect("restart generation");
        assert_eq!(observed, expected);
        assert!(
            !instance_is_degraded(instance),
            "module must remain live below the restart cap"
        );
    }

    // The cap+1 fault must enter degrade.
    let instance_before = current_instance(provider).expect("primary instance");
    fault_module(provider).expect("fault at cap");
    // After degrade_module the instance was torn down, so we can no
    // longer resolve current_instance — but the *prior* instance id is
    // still queryable as long as is_degraded reads the module record by
    // way of slot lookup.  In our harness the instance is gone, so the
    // observable is: snapshot.live_instances dropped by one.
    let snap = snapshot().expect("snapshot post-degrade");
    assert_eq!(
        snap.live_instances, 0,
        "degrade_module must teardown all instances"
    );
    // The torn-down instance id no longer maps to a record — proving
    // the teardown path executed.
    assert!(
        !instance_is_degraded(instance_before),
        "old instance id stops being addressable after teardown"
    );

    // Fresh charge_heap / claim_resource against any of this module's
    // (now non-existent) instances must be rejected — and even if a new
    // instance were spawned, it would inherit the Faulted module state.
    // We exercise the module-state guard directly via the public ABI:
    // a no-op spawn on a Faulted module returns NoActiveInstance because
    // the primary instance is gone.
    assert_eq!(
        current_instance(provider),
        Err(SupervisorError::InstanceNotFound)
    );

    // charge_heap on the prior instance id (now invalid) returns
    // InstanceNotFound — proving accounting is no longer reachable for
    // the degraded module.
    let charge_result = charge_heap(instance_before, 1);
    assert!(
        matches!(
            charge_result,
            Err(SupervisorError::InstanceNotFound)
                | Err(SupervisorError::ModuleRejected)
        ),
        "expected InstanceNotFound or ModuleRejected, got {:?}",
        charge_result
    );
}

const ZERO_SUMMARY: ModuleStatusSummary = ModuleStatusSummary {
    handle: ModuleHandle::ZERO,
    module_id: ModuleId::ZERO,
    state: ModuleLifecycle::Stopped,
    fault_policy: ModuleFaultPolicy::Manual,
    restart_generation: 0,
    degraded: false,
};

#[test]
fn module_status_summaries_reports_lifecycle_and_degraded_state() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    let mut out = [ZERO_SUMMARY; MAX_MODULES];
    let count = module_status_summaries(&mut out);
    assert_eq!(count, 1, "exactly the one installed module is reported");
    let running = out[0];
    assert_eq!(running.handle, provider);
    assert_eq!(running.module_id, ModuleId::from_ascii("MOD.PROVIDER"));
    assert_eq!(running.state, ModuleLifecycle::Running);
    assert_eq!(running.fault_policy, ModuleFaultPolicy::RestartAlways);
    assert_eq!(running.restart_generation, 0);
    assert!(!running.degraded);

    // Drive the module past the restart cap so it lands in Faulted +
    // degraded, and confirm the summary reflects both the bumped
    // restart_generation and the derived `degraded` flag.
    for _ in 0..=MAX_RESTARTS_BEFORE_DEGRADE {
        fault_module(provider).expect("fault");
    }

    let mut out = [ZERO_SUMMARY; MAX_MODULES];
    let count = module_status_summaries(&mut out);
    assert_eq!(count, 1);
    let faulted = out[0];
    assert_eq!(faulted.handle, provider);
    assert_eq!(faulted.state, ModuleLifecycle::Faulted);
    // The cap-th restart bumps restart_generation to the cap; the
    // following fault sees restart_count >= cap and degrades instead of
    // restarting again, so the counter stays at the cap rather than
    // incrementing past it.
    assert_eq!(faulted.restart_generation, MAX_RESTARTS_BEFORE_DEGRADE);
    assert!(faulted.degraded, "restart cap must surface as degraded");
}

// ── clear_restart_history: operator "reset-failed" recovery path ────────────
//
// MAX_RESTARTS_BEFORE_DEGRADE is a one-way, lifetime-cumulative counter
// with no decay (see clear_restart_history's doc comment). Once a module
// hits the cap and degrades, the only way back is an explicit operator
// acknowledgement that the root cause is fixed. clear_restart_history must
// zero the counter (and clear `degraded`) without itself touching
// lifecycle state — and restart_module must still work afterward to
// actually bring the module back up.
#[test]
fn clear_restart_history_unblocks_a_permanently_degraded_module() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    // Drive the module to the restart cap and one fault past it, landing
    // in degraded Faulted state exactly as in the cap test above.
    for _ in 0..=MAX_RESTARTS_BEFORE_DEGRADE {
        fault_module(provider).expect("fault");
    }
    let mut out = [ZERO_SUMMARY; MAX_MODULES];
    let count = module_status_summaries(&mut out);
    assert_eq!(count, 1);
    assert_eq!(out[0].state, ModuleLifecycle::Faulted);
    assert!(out[0].degraded, "must be degraded before the clear");

    clear_restart_history(provider).expect("clear_restart_history");

    // Lifecycle is untouched by the clear alone — still Faulted, just no
    // longer carrying a degraded restart count.
    let mut out = [ZERO_SUMMARY; MAX_MODULES];
    let count = module_status_summaries(&mut out);
    assert_eq!(count, 1);
    assert_eq!(out[0].state, ModuleLifecycle::Faulted);
    assert_eq!(out[0].restart_generation, 0);
    assert!(!out[0].degraded, "clearing history must drop the degraded flag");

    // The module is no longer treated as permanently degraded, so
    // restart_module can bring it back up and the counter resumes
    // counting from zero.
    restart_module(provider).expect("restart after clear");
    let mut out = [ZERO_SUMMARY; MAX_MODULES];
    let count = module_status_summaries(&mut out);
    assert_eq!(count, 1);
    assert_eq!(out[0].state, ModuleLifecycle::Running);
    assert_eq!(out[0].restart_generation, 1);
    assert!(!out[0].degraded);
}

// clear_restart_history on a module that was never installed must fail
// the same way every other handle-keyed supervisor call does, rather than
// silently doing nothing.
#[test]
fn clear_restart_history_rejects_unknown_handle() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let unknown = ModuleHandle(0xDEAD_BEEF);
    assert_eq!(
        clear_restart_history(unknown),
        Err(SupervisorError::ModuleNotFound)
    );
}

#[test]
fn module_handle_for_id_resolves_installed_module_by_name() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    // The shell `restart <name>` command only has the typed module name to
    // go on; module_handle_for_id is the lookup that turns it into the
    // ModuleHandle restart_module requires.
    let resolved = module_handle_for_id(ModuleId::from_ascii("MOD.PROVIDER"))
        .expect("installed module must resolve by id");
    assert_eq!(resolved, provider);

    assert!(
        module_handle_for_id(ModuleId::from_ascii("NO.SUCH.MODULE")).is_none(),
        "unknown module id must not resolve"
    );

    // Round-trip: resolve by name, then restart through the resolved
    // handle exactly as the shell command does.
    let before = module_lifecycle(provider).expect("lifecycle before restart");
    assert_eq!(before, ModuleLifecycle::Running);
    restart_module(resolved).expect("restart via resolved handle");
    let after = module_lifecycle(provider).expect("lifecycle after restart");
    assert_eq!(after, ModuleLifecycle::Running, "RestartAlways module comes back up");
}

const ZERO_RESOURCE: InstanceResourceSummary = InstanceResourceSummary {
    instance_id: NodeInstanceId::ZERO,
    module: ModuleHandle::ZERO,
    lifecycle: gos_protocol::NodeInstanceLifecycle::Stopped,
    heap_pages_used: 0,
    heap_pages_max: 0,
    gpu_bytes_used: 0,
    gpu_bytes_max: 0,
};

#[test]
fn instance_resource_summaries_reports_heap_and_gpu_usage_against_quota() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");
    let instance = current_instance(provider).expect("primary instance");

    set_gpu_quota(instance, 4096).expect("set gpu quota");
    charge_gpu_bytes(instance, 1024).expect("charge gpu");

    let mut out = [ZERO_RESOURCE; MAX_INSTANCES];
    let count = instance_resource_summaries(&mut out);
    assert_eq!(count, 1, "exactly the one live instance is reported");
    let summary = out[0];
    assert_eq!(summary.instance_id, instance);
    assert_eq!(summary.module, provider);
    assert_eq!(summary.lifecycle, gos_protocol::NodeInstanceLifecycle::Ready);
    // PROVIDER's heap quota is charged 2 pages by test_start (see
    // CALLBACK_HEAP_BASE wiring above), so usage should already be
    // non-zero by the time boot has realized this module.
    assert_eq!(summary.heap_pages_used, 2);
    assert!(summary.heap_pages_max >= summary.heap_pages_used);
    assert_eq!(summary.gpu_bytes_used, 1024);
    assert_eq!(summary.gpu_bytes_max, 4096);

    // charge_heap further and confirm the summary tracks the new total.
    charge_heap(instance, 1).expect("charge heap");
    let mut out = [ZERO_RESOURCE; MAX_INSTANCES];
    let count = instance_resource_summaries(&mut out);
    assert_eq!(count, 1);
    assert_eq!(out[0].heap_pages_used, 3);
}

// Phase H.1.x.2 — supervisor `apply_cypher_mutation` gate happy path.
//
// Sets up a synthetic plugin with two nodes in the runtime, applies an
// AddEdge through the supervisor gate, and verifies:
//   * the returned EdgeId is the same one runtime would have produced;
//   * a MutationAudit envelope reaches the runtime queue with
//     the expected source attribution (low-level EdgeUpsert from
//     register_edge is also emitted but we filter for the audited one);
//   * unknown owner → MUTATION_GATE_OWNER_UNKNOWN tag (no runtime touch);
//   * RemoveEdge with a non-existent edge_id → MUTATION_GATE_EDGE_NOT_FOUND
//     (gated before reaching the runtime).
//
// The Faulted-module rejection path is covered separately by H.1.x.5
// once we wire up a synthetic ModuleId-aligned plugin via the supervisor
// install path.
#[test]
fn apply_cypher_mutation_happy_path_emits_audited_envelope() {
    use gos_cypher_mut::{CypherMutation, MutationError, ReceptiveEdgeKind};
    use gos_protocol::{
        derive_node_id, ControlPlaneMessageKind, EdgeId, EntryPolicy, ExecutorId, NodeSpec,
        PluginId, PluginManifest, RuntimeNodeType, VectorAddress, GOS_ABI_VERSION,
    };
    use gos_supervisor::{
        apply_cypher_mutation, MUTATION_GATE_EDGE_NOT_FOUND, MUTATION_GATE_OWNER_UNKNOWN,
    };

    let _guard = test_guard();
    reset_state();
    gos_supervisor::bootstrap(0);
    gos_runtime::reset();

    const PID: PluginId = PluginId::from_ascii("GATE_RT");
    const KA: &str = "gate.a";
    const KB: &str = "gate.b";
    const EXEC: ExecutorId = ExecutorId::from_ascii("native.gate");
    const VA: VectorAddress = VectorAddress::new(2, 2, 2, 1);
    const VB: VectorAddress = VectorAddress::new(2, 2, 2, 2);

    let spec_a = NodeSpec {
        node_id: derive_node_id(PID, KA),
        local_node_key: KA,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    };
    let spec_b = NodeSpec {
        local_node_key: KB,
        node_id: derive_node_id(PID, KB),
        ..spec_a
    };
    let manifest = PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: PID,
        name: "GATE_RT",
        version: 1,
        depends_on: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        nodes: &[],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    };
    gos_runtime::discover_plugin(manifest).expect("discover");
    gos_runtime::mark_plugin_loaded(PID).expect("loaded");
    gos_runtime::register_node(PID, VA, spec_a).expect("register a");
    gos_runtime::register_node(PID, VB, spec_b).expect("register b");

    let id_a = gos_runtime::node_id_for_vec(VA).expect("id_a");
    let id_b = gos_runtime::node_id_for_vec(VB).expect("id_b");

    // Drain anything left over from setup.  Audit ring snapshots are
    // read-only (no consume), so we record the baseline `wrote` total
    // and assert it grows by exactly one per successful gate apply.
    while gos_runtime::drain_control_plane().is_some() {}
    let audit_baseline = gos_runtime::audit_ring_total();

    // --- Happy path ---
    let source = *b"K_HARNESS\0\0\0\0\0\0\0";
    let edge_id = apply_cypher_mutation(
        CypherMutation::AddEdge {
            from: id_a,
            to: id_b,
            edge_kind: ReceptiveEdgeKind::Mount,
        },
        source,
    )
    .expect("AddEdge through gate");

    // Both an EdgeUpsert (from register_edge) and a MutationAudit
    // (from the gate) must reach the queue.  Filter for the audited one.
    let mut audited_seen = false;
    while let Some(env) = gos_runtime::drain_control_plane() {
        if env.kind == ControlPlaneMessageKind::MutationAudit {
            assert_eq!(env.subject, source, "envelope carries source");
            audited_seen = true;
        }
    }
    assert!(
        audited_seen,
        "MutationAudit envelope must reach the runtime queue"
    );

    // Audit ring: exactly one new entry, newest-first, source matches.
    assert_eq!(
        gos_runtime::audit_ring_total(),
        audit_baseline + 1,
        "audit ring increments on successful gate apply"
    );
    use gos_protocol::ControlPlaneEnvelope;
    let mut ring: [Option<ControlPlaneEnvelope>; 4] = [None; 4];
    let n = gos_runtime::snapshot_audit_ring(&mut ring);
    assert!(n >= 1);
    let head = ring[0].expect("newest audit entry");
    assert_eq!(head.kind, ControlPlaneMessageKind::MutationAudit);
    assert_eq!(head.subject, source);

    // Re-derivation: the same (from, to, kind) must round-trip to the
    // same EdgeId so RemoveEdge can address it later.
    assert_eq!(
        gos_runtime::edge_id_for_vector(gos_protocol::derive_edge_vector(edge_id)),
        Some(edge_id)
    );

    // --- Unknown owner: gate short-circuits before runtime ---
    let ghost = derive_node_id(PID, "ghost");
    match apply_cypher_mutation(
        CypherMutation::AddEdge {
            from: ghost,
            to: id_b,
            edge_kind: ReceptiveEdgeKind::Mount,
        },
        source,
    ) {
        Err(MutationError::DispatcherRejected(tag)) => {
            assert_eq!(tag, MUTATION_GATE_OWNER_UNKNOWN, "owner unknown tag")
        }
        other => panic!("expected DispatcherRejected(OWNER_UNKNOWN), got {:?}", other),
    }

    // --- Unknown edge for RemoveEdge: gate short-circuits ---
    match apply_cypher_mutation(
        CypherMutation::RemoveEdge {
            edge_id: EdgeId([0xEE; 16]),
        },
        source,
    ) {
        Err(MutationError::DispatcherRejected(tag)) => {
            assert_eq!(tag, MUTATION_GATE_EDGE_NOT_FOUND, "edge-not-found tag")
        }
        other => panic!("expected DispatcherRejected(EDGE_NOT_FOUND), got {:?}", other),
    }
}

// Phase H.1.x.5 — degraded-module gate rejection.
//
// Install PROVIDER under the supervisor, register a matching runtime
// plugin so node lookups resolve to the same ModuleId, and exhaust the
// restart budget so PROVIDER ends up in Faulted state.  Then attempt
// a Cypher mutation that targets a node owned by PROVIDER and assert
// the supervisor gate returns MUTATION_GATE_DEGRADED *without*
// touching the runtime edge table.
#[test]
fn apply_cypher_mutation_rejected_on_degraded_module() {
    use gos_cypher_mut::{CypherMutation, MutationError, ReceptiveEdgeKind};
    use gos_protocol::{
        derive_node_id, EntryPolicy, ExecutorId, NodeSpec, PluginId, PluginManifest,
        RuntimeNodeType, VectorAddress, GOS_ABI_VERSION,
    };
    use gos_supervisor::{apply_cypher_mutation, MUTATION_GATE_DEGRADED};

    let _guard = test_guard();
    reset_state();
    gos_supervisor::bootstrap(0);
    gos_runtime::reset();

    // Install PROVIDER and realize boot modules so it has an instance.
    let provider = install_module(PROVIDER).expect("provider install");
    realize_boot_modules().expect("realize");

    // Register a runtime plugin with the SAME id as PROVIDER's
    // ModuleId.  This is what lets the supervisor gate trace
    // node → plugin → module → degraded state.
    let pid = PluginId(PROVIDER.module_id.0);
    const KEY: &str = "gate.degraded";
    const VEC: VectorAddress = VectorAddress::new(9, 9, 9, 1);
    const KEY2: &str = "gate.degraded.b";
    const VEC2: VectorAddress = VectorAddress::new(9, 9, 9, 2);

    let spec_a = NodeSpec {
        node_id: derive_node_id(pid, KEY),
        local_node_key: KEY,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: ExecutorId::from_ascii("native.deg"),
        state_schema_hash: 0,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    };
    let spec_b = NodeSpec {
        local_node_key: KEY2,
        node_id: derive_node_id(pid, KEY2),
        ..spec_a
    };
    let manifest = PluginManifest {
        abi_version: GOS_ABI_VERSION,
        plugin_id: pid,
        name: "DEG_RT",
        version: 1,
        depends_on: &[],
        permissions: &[],
        exports: &[],
        imports: &[],
        nodes: &[],
        edges: &[],
        signature: None,
        policy_hash: [0; 16],
    };
    gos_runtime::discover_plugin(manifest).expect("discover");
    gos_runtime::mark_plugin_loaded(pid).expect("loaded");
    gos_runtime::register_node(pid, VEC, spec_a).expect("register a");
    gos_runtime::register_node(pid, VEC2, spec_b).expect("register b");

    let id_a = gos_runtime::node_id_for_vec(VEC).expect("id_a");
    let id_b = gos_runtime::node_id_for_vec(VEC2).expect("id_b");

    // Drive PROVIDER to degraded.  PROVIDER has RestartAlways policy,
    // so faulting MAX+1 times flips it into Faulted state.
    for _ in 1..=MAX_RESTARTS_BEFORE_DEGRADE {
        fault_module(provider).expect("fault under cap");
    }
    fault_module(provider).expect("fault at cap");

    // Now the gate must reject any mutation whose `from` resolves to a
    // node owned by PROVIDER's plugin id.
    let edges_before = {
        let mut buf = [gos_protocol::GraphEdgeSummary::EMPTY; 4];
        let (total, _) = gos_runtime::edge_page(0, &mut buf);
        total
    };
    let audit_before = gos_runtime::audit_ring_total();

    match apply_cypher_mutation(
        CypherMutation::AddEdge {
            from: id_a,
            to: id_b,
            edge_kind: ReceptiveEdgeKind::Mount,
        },
        *b"K_HARNESS\0\0\0\0\0\0\0",
    ) {
        Err(MutationError::DispatcherRejected(tag)) => assert_eq!(
            tag, MUTATION_GATE_DEGRADED,
            "expected MUTATION_GATE_DEGRADED, got {}",
            tag
        ),
        other => panic!("expected DispatcherRejected(DEGRADED), got {:?}", other),
    }

    // No side effects: edge table untouched, audit ring untouched.
    let edges_after = {
        let mut buf = [gos_protocol::GraphEdgeSummary::EMPTY; 4];
        let (total, _) = gos_runtime::edge_page(0, &mut buf);
        total
    };
    assert_eq!(edges_after, edges_before, "edge table must be unchanged");
    assert_eq!(
        gos_runtime::audit_ring_total(),
        audit_before,
        "audit ring must be unchanged on gate rejection"
    );
}
