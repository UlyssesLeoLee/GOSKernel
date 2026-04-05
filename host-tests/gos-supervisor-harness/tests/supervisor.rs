use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Mutex,
};

use gos_protocol::{
    CapabilitySpec, ClaimId, ClaimPolicy, DomainId, ExecutionLaneClass, LeaseEpoch,
    ModuleAbiV1, ModuleCallStatus, ModuleDependencySpec, ModuleDescriptor, ModuleEntry,
    ModuleFaultPolicy, ModuleHandle, ModuleId, ModuleImageFormat, ModuleImageSegment,
    ModuleSegmentKind, NodeInstanceId, PreemptPolicy, RESOURCE_DISPLAY_CONSOLE,
    MODULE_ABI_VERSION,
};
use gos_supervisor::{
    bootstrap, claim_resource, current_instance, dequeue_ready_instance, drain_revocation,
    heap_grant_summary, install_module, process_restart_queue, queue_restart, realize_boot_modules,
    release_claim, schedule_instance, snapshot, spawn_instance, template_for_module,
    SupervisorError, MAX_CLAIMS,
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
    assert_eq!(snap.registered_resources, 5);
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

#[test]
fn missing_dependency_is_rejected() {
    let _guard = test_guard();
    reset_state();
    bootstrap(0);
    install_module(CONSUMER).expect("consumer install");
    assert_eq!(realize_boot_modules(), Err(SupervisorError::ModuleRejected));
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
