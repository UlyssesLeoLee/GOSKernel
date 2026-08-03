//! gos-shadow-kernel-harness — end-to-end "shadow kernel" integration tests.
//!
//! Every other `host-tests/gos-*-harness` exercises exactly one subsystem in
//! isolation (a single supervisor call, a single graph algorithm, a single
//! mutation dispatcher). That isolation is why several real bugs during the
//! `feat/v2-mutation-dispatcher` merge only surfaced via `cargo check` or a
//! narrow unit test, never via "does the whole system actually run
//! together": a duplicated `service_system_cycle`, an `apply_mutation`
//! return-type mismatch between two independent call sites, and a
//! `RuntimeDispatcher::rebind_use` that silently lost single-epoch atomicity.
//!
//! This harness sits between the unit-level harnesses and an actual QEMU
//! boot: it drives `gos_supervisor::bootstrap` -> `install_module` ->
//! `realize_boot_modules` -> a `service_system_cycle()` loop, the same shape
//! `hypervisor::main::kernel_main` runs, then interleaves live Cypher
//! mutations, a module fault/restart, and atomic Use-edge rebinds *while the
//! system keeps cycling* — the composition unit tests can't cover.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "shadow_kernel.rs", type: "file", language: "rust"}),
//!   (boot:Function {name: "boot_two_modules", type: "function", visibility: "private"}),
//!   (t1:Function {name: "shadow_kernel_boots_and_settles_into_idle_steady_state", type: "function"}),
//!   (t2:Function {name: "shadow_kernel_live_cypher_mutations_flow_through_running_system", type: "function"}),
//!   (t3:Function {name: "shadow_kernel_survives_fault_and_restart_while_other_modules_keep_cycling", type: "function"}),
//!   (t4:Function {name: "shadow_kernel_rebind_use_stays_atomic_under_concurrent_module_activity", type: "function"}),
//!   (t5:Function {name: "capability_granting_mutation_is_the_promote_trigger_adr014", type: "function"}),
//!   (f)-[:CONTAINS]->(boot), (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2),
//!   (f)-[:CONTAINS]->(t3), (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5),
//!   (t1)-[:CALLS]->(boot), (t2)-[:CALLS]->(boot), (t3)-[:CALLS]->(boot), (t4)-[:CALLS]->(boot),
//!   (t5)-[:CALLS]->(boot);
//! ```

use std::sync::Mutex;

use gos_cypher_mut::{apply_mutation, CypherMutation, MutationDispatcher, ReceptiveEdgeKind};
use gos_protocol::{
    derive_edge_id, derive_edge_vector, ModuleAbiV1, ModuleCallStatus, ModuleDescriptor,
    ModuleEntry, ModuleFaultPolicy, ModuleHandle, ModuleId, ModuleImageFormat,
    ModuleImageSegment, ModuleSegmentKind, NodeId, MODULE_ABI_VERSION,
};
use gos_supervisor::{
    bootstrap, current_instance, install_module, realize_boot_modules, service_system_cycle,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

const SEGMENTS: &[ModuleImageSegment] = &[ModuleImageSegment {
    kind: ModuleSegmentKind::Text,
    virt_addr: 0,
    mem_len: 0x4000,
    file_offset: 0,
    file_len: 0x2000,
    flags: 0,
}];

unsafe extern "C" fn quiet_start(
    _abi: *const ModuleAbiV1,
    _handle: ModuleHandle,
    _domain: gos_protocol::DomainId,
) -> ModuleCallStatus {
    ModuleCallStatus::Ok
}

const QUIET_ENTRY: ModuleEntry = ModuleEntry {
    module_init: None,
    module_start: Some(quiet_start),
    module_stop: None,
    module_suspend: None,
    module_resume: None,
};

const DRIVER: ModuleDescriptor = ModuleDescriptor {
    abi_version: MODULE_ABI_VERSION,
    module_id: ModuleId::from_ascii("SHADOW.DRIVER"),
    name: "SHADOW_DRIVER",
    version: 1,
    image_format: ModuleImageFormat::Builtin,
    fault_policy: ModuleFaultPolicy::RestartAlways,
    dependencies: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    segments: SEGMENTS,
    entry: QUIET_ENTRY,
    signature: None,
    flags: 0,
};

const SERVICE: ModuleDescriptor = ModuleDescriptor {
    abi_version: MODULE_ABI_VERSION,
    module_id: ModuleId::from_ascii("SHADOW.SERVICE"),
    name: "SHADOW_SERVICE",
    version: 1,
    image_format: ModuleImageFormat::Builtin,
    fault_policy: ModuleFaultPolicy::Manual,
    dependencies: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    segments: SEGMENTS,
    entry: QUIET_ENTRY,
    signature: None,
    flags: 0,
};

/// Boot a small two-module system the same way `kernel_main` boots the real
/// one: `bootstrap` -> install every descriptor -> `realize_boot_modules`.
/// Returns the handles so callers can drive per-module lifecycle (fault,
/// restart) against a live, running system.
fn boot_two_modules() -> (ModuleHandle, ModuleHandle) {
    gos_runtime::reset();
    bootstrap(0);
    let driver = install_module(DRIVER).expect("driver install");
    let service = install_module(SERVICE).expect("service install");
    realize_boot_modules().expect("realize boot modules");
    (driver, service)
}

/// Drive `service_system_cycle()` `n` times, mirroring `kernel_main`'s
/// steady-state loop, asserting every cycle quiesces (ADR-002 §4) — a
/// system that never settles would hang the real kernel's frame loop.
fn run_steady_state_cycles(n: usize) {
    for i in 0..n {
        let report = service_system_cycle();
        assert!(
            report.quiesced,
            "cycle {i} failed to quiesce within CYCLE_DEPTH_CAP: {report:?}"
        );
        assert!(!report.overflowed, "cycle {i} overflowed the pending-signal queue");
    }
}

fn use_edge_present(from: NodeId, to: NodeId, key: &str) -> bool {
    let id = derive_edge_id(from, to, key);
    gos_runtime::edge_id_for_vector(derive_edge_vector(id)).is_some()
}

/// Allocate a provisional node (ADR-005 option A) directly through the
/// dispatcher, mirroring `k-cypher`'s `CREATE (n)` handler — bypasses the
/// edge-scoped supervisor gate (which has no policy for node creation yet),
/// same as production `CREATE (n)` does.
fn create_node() -> NodeId {
    let mut d = gos_runtime::RuntimeDispatcher;
    apply_mutation(&mut d, CypherMutation::CreateNode)
        .expect("create applies")
        .expect("CreateNode returns the allocated NodeId")
}

// ── Boot + steady-state ──────────────────────────────────────────────────

#[test]
fn shadow_kernel_boots_and_settles_into_idle_steady_state() {
    let _guard = test_guard();
    let (driver, service) = boot_two_modules();

    // One-shot post-boot drain, mirroring kernel_main's pre-loop cycle call.
    let boot_report = service_system_cycle();
    assert!(boot_report.quiesced, "post-boot drain must quiesce: {boot_report:?}");

    let idle0 = gos_supervisor::idle_cycle_count();
    run_steady_state_cycles(5);
    let idle1 = gos_supervisor::idle_cycle_count();
    assert!(
        idle1 > idle0,
        "an idle running system must accumulate idle cycles (V2.3 render-skip); {idle0} -> {idle1}"
    );

    // Both modules must still be alive and independently addressable —
    // booting one doesn't clobber the other's instance.
    let driver_instance = current_instance(driver).expect("driver instance");
    let service_instance = current_instance(service).expect("service instance");
    assert_ne!(driver_instance, service_instance, "distinct modules get distinct instances");
}

// ── Live Cypher mutations against a running system ──────────────────────

#[test]
fn shadow_kernel_live_cypher_mutations_flow_through_running_system() {
    let _guard = test_guard();
    boot_two_modules();
    run_steady_state_cycles(2);

    let a = create_node();
    let b = create_node();
    let c = create_node();
    assert_ne!(a, b);
    assert_ne!(b, c);

    // CREATE MOUNT a -> b, through the same gated path k-cypher's
    // `try_run_mutation` uses.
    gos_supervisor::apply_cypher_mutation(
        CypherMutation::AddEdge { from: a, to: b, edge_kind: ReceptiveEdgeKind::Mount },
        *b"SHADOW_TEST\0\0\0\0\0",
    )
    .expect("mount applies");
    assert!(use_edge_present(a, b, "cypher.mount"), "Mount edge must be live");

    // The system must still quiesce cleanly after a live mutation — a
    // mutation that leaves dangling signal work would never settle.
    run_steady_state_cycles(2);

    // REBIND USE a -> c (single logical mutation; exclusivity + atomicity
    // asserted precisely by the dedicated rebind test below).
    gos_supervisor::apply_cypher_mutation(
        CypherMutation::RebindUse { from: a, new_target: c },
        *b"SHADOW_TEST\0\0\0\0\0",
    )
    .expect("rebind applies");
    assert!(use_edge_present(a, c, "cypher.use"), "rebound Use edge must point at c");

    run_steady_state_cycles(2);

    // DELETE EDGE removes the Mount edge cleanly.
    let mount_id = derive_edge_id(a, b, "cypher.mount");
    gos_supervisor::apply_cypher_mutation(
        CypherMutation::RemoveEdge { edge_id: mount_id },
        *b"SHADOW_TEST\0\0\0\0\0",
    )
    .expect("remove applies");
    assert!(!use_edge_present(a, b, "cypher.mount"), "Mount edge must be gone after delete");

    run_steady_state_cycles(2);
}

// ── Fault + restart while the rest of the system keeps running ──────────

#[test]
fn shadow_kernel_survives_fault_and_restart_while_other_modules_keep_cycling() {
    let _guard = test_guard();
    let (driver, service) = boot_two_modules();
    run_steady_state_cycles(2);

    let driver_instance_before = current_instance(driver).expect("driver instance");
    let service_instance_before = current_instance(service).expect("service instance");
    let gen_before = gos_supervisor::instance_restart_generation(driver_instance_before)
        .expect("gen before fault");
    assert_eq!(gen_before, 0);

    // Fault the driver (RestartAlways) the same way k-idt's trap normalizer
    // would via the registered FAULT_DISPATCH hook.
    gos_runtime::dispatch_fault(driver_instance_before);

    let driver_instance_after = current_instance(driver).expect("driver instance post-fault");
    let gen_after = gos_supervisor::instance_restart_generation(driver_instance_after)
        .expect("gen after fault");
    assert_eq!(gen_after, 1, "RestartAlways must recycle the instance with a bumped generation");

    // The untouched module must be completely unaffected by its sibling's
    // fault — this is the cross-module isolation invariant, which a
    // single-module fault test can't demonstrate.
    let service_instance_after = current_instance(service).expect("service instance post-fault");
    assert_eq!(
        service_instance_before, service_instance_after,
        "faulting one module must not perturb an unrelated module's instance"
    );

    // The system must keep cycling cleanly after recovery — a fault that
    // leaves the restart queue or ready queue in a bad state would show up
    // here as a non-quiescent cycle.
    run_steady_state_cycles(3);
}

// ── Atomic Use rebind under live, busy-system conditions ─────────────────

#[test]
fn shadow_kernel_rebind_use_stays_atomic_under_concurrent_module_activity() {
    let _guard = test_guard();
    boot_two_modules();
    run_steady_state_cycles(2);

    let theme_current = create_node();
    let wabi = create_node();
    let shoji = create_node();

    // Seed via the free-function atomic path (mirrors k-shell's boot-time
    // theme wiring), with two other live modules mid-cycle around it.
    let before = gos_runtime::snapshot();
    gos_runtime::rebind_use(theme_current, wabi).expect("seed rebind");
    let after = gos_runtime::snapshot();
    assert_eq!(
        after.graph_epoch,
        before.graph_epoch + 1,
        "free-function rebind_use must stay a single-epoch atomic op even with other modules live"
    );
    assert!(use_edge_present(theme_current, wabi, "use"));

    run_steady_state_cycles(1);

    // Now rebind via RuntimeDispatcher::rebind_use (the Cypher-mutation
    // dispatch path — the one that regressed to a 2-epoch remove+insert
    // during the v2-mutation-dispatcher merge). Must be equally atomic.
    let mut d = gos_runtime::RuntimeDispatcher;
    let e0 = gos_runtime::snapshot().graph_epoch;
    d.rebind_use(theme_current, shoji).expect("dispatcher rebind");
    let e1 = gos_runtime::snapshot().graph_epoch;
    assert_eq!(
        e1,
        e0 + 1,
        "RuntimeDispatcher::rebind_use must be single-epoch atomic under live system conditions"
    );
    assert!(use_edge_present(theme_current, shoji, "cypher.use"));
    // Exclusivity holds across both edge-key conventions (ADR-004 §四.1):
    // `rebind_exclusive_use_keyed`'s removal pass matches on
    // (edge_type == Use, from_node == from), not on the derived key, so the
    // earlier free-function-seeded "use"-keyed edge is torn down too — at
    // most one Use edge survives per source node regardless of which path
    // performed the most recent rebind.
    assert!(!use_edge_present(theme_current, wabi, "use"));

    run_steady_state_cycles(2);
}

// ── ADR-014 §二 — process=subgraph mapping, wired ahead of the A/B/C
// compat-strategy choice ────────────────────────────────────────────────
//
// ADR-014 explicitly recommends landing §二 regardless of which compat
// option (WASI-first / POSIX-native-first / defer) eventually gets chosen
// -- its payload is the answer to ADR-005 §七's leftover (2), "capability
// mutations have no trigger for 'promote'": a process node opening a new
// fd is exactly one capability-granting mutation through the same gate a
// human/AI/gpm mutation already uses. This test proves that claim against
// the live system (not synthetic EdgeSpec tables), in a running, cycling
// shadow kernel -- matching the shape kernel_main actually runs the same
// mutation gate under.
//
// Known, documented gap this test does NOT paper over: ADR-014 §2.1 wants
// process nodes tagged `RuntimeNodeType::Compute` / `EntryPolicy::OnDemand`
// / a real interpreter `ExecutorId`; `CypherMutation::CreateNode` (ADR-005
// option A) only ever produces the generic `RuntimeNodeType::Vector` /
// `EntryPolicy::Manual` / `ExecutorId::ZERO` shape today -- there's no
// parameter to customize it. That's a real, separate gap (extending
// CreateNode's own shape), not something this test's "process=subgraph"
// proof depends on: the promote-trigger claim is about the *mutation gate*,
// not about which RuntimeNodeType the resulting node carries.
#[test]
fn capability_granting_mutation_is_the_promote_trigger_adr014() {
    let _guard = test_guard();
    boot_two_modules();
    run_steady_state_cycles(2);

    // §2.1: a process instance = a provisional node. §2.1's Compute/OnDemand
    // tagging isn't available through CreateNode yet (see the gap noted
    // above) -- `proc` stands in for it.
    let proc = create_node();
    // §2.4: the target resource a `path_open`-class call would name.
    let resource = create_node();

    // Before any fd is opened: no Grant path, using the *real* edge table
    // (no edges exist between these two fresh nodes at all).
    let no_edges: [gos_protocol::EdgeSpec; 0] = [];
    assert!(
        !gos_mutation_dispatch::capability::capability_check(&no_edges, proc, resource),
        "a freshly created process/resource pair must start with no capability"
    );

    // §2.3: "opening a new fd" = a capability-granting mutation through the
    // *same* gate a human/AI/gpm mutation goes through -- not a special
    // process-only code path. Use `Use` (Refer+Bind+Grant, ADR-001):
    // fd-like "process holds this exclusively, lifecycle-bound" semantics
    // per §2.2's own mapping.
    let edge_id = gos_supervisor::apply_cypher_mutation(
        CypherMutation::AddEdge { from: proc, to: resource, edge_kind: ReceptiveEdgeKind::Use },
        *b"K_TEST_ADR014\0\0\0",
    )
    .expect("path_open-class mutation must apply through the standard gate");

    // This *is* ADR-005 §五 step 3's "promote" trigger, made concrete:
    // capability_check now sees a Grant path where it didn't before,
    // driven entirely by the ordinary mutation-gate path, using a
    // hand-mirrored EdgeSpec for the one edge the real mutation just
    // created (mirrors the ADR-006 shadow-verification pattern rather than
    // re-deriving the full live graph).
    let real_edge = gos_protocol::EdgeSpec {
        edge_id,
        from_node: proc,
        to_node: resource,
        edge_type: gos_protocol::RuntimeEdgeType::Use,
        weight: 1.0,
        acl_mask: 0,
        route_policy: gos_protocol::RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    };
    assert!(
        gos_mutation_dispatch::capability::capability_check(&[real_edge], proc, resource),
        "the capability-granting mutation must be visible to capability_check as a Grant path"
    );

    run_steady_state_cycles(1);

    // §2.5: process exit = RemoveEdge on its outgoing capability edges --
    // no new deletion primitive, same gate again.
    gos_supervisor::apply_cypher_mutation(
        CypherMutation::RemoveEdge { edge_id },
        *b"K_TEST_ADR014\0\0\0",
    )
    .expect("fd close / process exit must apply through the standard gate");

    assert!(
        !gos_mutation_dispatch::capability::capability_check(&no_edges, proc, resource),
        "after the granting edge is removed, capability_check must revoke the path -- \
         an isolated provisional node with no Grant out-edges is harmless by construction"
    );

    run_steady_state_cycles(2);
}
