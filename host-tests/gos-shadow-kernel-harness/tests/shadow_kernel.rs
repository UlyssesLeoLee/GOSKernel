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
//!   (t6:Function {name: "gpm_install_mints_a_package_node_mounted_under_packages_root", type: "function"}),
//!   (t7:Function {name: "adr017_ask_stages_mutations_and_chat_approve_applies_through_the_standard_gate", type: "function"}),
//!   (t8:Function {name: "adr017_mirror_bad_gmut_line_rejects_the_whole_turn", type: "function"}),
//!   (f)-[:CONTAINS]->(boot), (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2),
//!   (f)-[:CONTAINS]->(t3), (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5), (f)-[:CONTAINS]->(t6),
//!   (f)-[:CONTAINS]->(t7), (f)-[:CONTAINS]->(t8),
//!   (t1)-[:CALLS]->(boot), (t2)-[:CALLS]->(boot), (t3)-[:CALLS]->(boot), (t4)-[:CALLS]->(boot),
//!   (t5)-[:CALLS]->(boot), (t6)-[:CALLS]->(boot), (t7)-[:CALLS]->(boot), (t8)-[:CALLS]->(boot);
//! ```

use std::sync::Mutex;

use gos_cypher_mut::{apply_mutation, CypherMutation, MutationDispatcher, ReceptiveEdgeKind};
use gos_protocol::{
    derive_edge_id, derive_edge_vector, derive_node_id, EntryPolicy, ExecutorId, GraphEdgeSummary,
    ModuleAbiV1, ModuleCallStatus, ModuleDescriptor, ModuleEntry, ModuleFaultPolicy, ModuleHandle,
    ModuleId, ModuleImageFormat, ModuleImageSegment, ModuleSegmentKind, NodeId, NodeSpec,
    PluginId, RuntimeEdgeType, RuntimeNodeType, VectorAddress, MODULE_ABI_VERSION,
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
fn create_node(
    node_type: gos_protocol::RuntimeNodeType,
    entry_policy: gos_protocol::EntryPolicy,
    executor_id: gos_protocol::ExecutorId,
) -> NodeId {
    let mut d = gos_runtime::RuntimeDispatcher;
    apply_mutation(&mut d, CypherMutation::CreateNode { node_type, entry_policy, executor_id })
        .expect("create applies")
        .expect("CreateNode returns the allocated NodeId")
}

/// `create_node` with the plain "passive data node" shape every caller used
/// before ADR-019 §五-2 parameterized `CreateNode` — most of this file's
/// tests don't care what shape the node has, just that one exists.
fn create_plain_node() -> NodeId {
    create_node(
        gos_protocol::RuntimeNodeType::Vector,
        gos_protocol::EntryPolicy::Manual,
        gos_protocol::ExecutorId::ZERO,
    )
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

    let a = create_plain_node();
    let b = create_plain_node();
    let c = create_plain_node();
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

    let theme_current = create_plain_node();
    let wabi = create_plain_node();
    let shoji = create_plain_node();

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
// ADR-019 §五-2 closed a gap this test used to document as open: ADR-014
// §2.1 wants process nodes tagged `RuntimeNodeType::Compute` /
// `EntryPolicy::OnDemand` / a real interpreter `ExecutorId`, but
// `CypherMutation::CreateNode` (ADR-005 option A) used to only ever
// produce the generic `RuntimeNodeType::Vector` / `EntryPolicy::Manual` /
// `ExecutorId::ZERO` shape -- no parameter existed to customize it.
// `CreateNode` now carries all three fields, so `proc` below is minted
// with the actual §2.1 shape instead of standing in for it. What's still
// NOT wired (out of scope here, tracked under ADR-019 §五 items 4-6): a
// real loader/interpreter that would make `test.proc` a *registered*
// ExecutorId whose vtable actually runs something -- `register_node`
// doesn't validate `executor_id` against any table, so an unregistered
// id is accepted and stored, but the node stays `NodeBinding::Unbound`
// regardless (same as the `ZERO` case) until a real binding step exists.
#[test]
fn capability_granting_mutation_is_the_promote_trigger_adr014() {
    let _guard = test_guard();
    boot_two_modules();
    run_steady_state_cycles(2);

    // §2.1: a process instance = a provisional node, now minted with its
    // real intended shape.
    let proc = create_node(
        gos_protocol::RuntimeNodeType::Compute,
        gos_protocol::EntryPolicy::OnDemand,
        gos_protocol::ExecutorId::from_ascii("test.proc"),
    );
    // §2.4: the target resource a `path_open`-class call would name --
    // stays a plain data node, not a process.
    let resource = create_plain_node();

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

// ── ADR-016 option A — gpm install/list mechanism ─────────────────────────
//
// Mirrors `k-shell::gpm_install_raw`/`gpm_list_raw` exactly (CreateNode +
// AddEdge{Mount} to a packages-root anchor, through the standard
// supervisor-gated mutation path with a b"K_GPM" source stamp), but against
// a synthetic anchor node registered here directly rather than the real
// builtin `packages.root` (k-shell is a no_std kernel crate tied to VGA/PS2
// hardware access and isn't host-compilable, matching every other
// k-shell-adjacent test in this codebase -- `cargo check -p gos-kernel`
// is what proves k-shell's own gpm_install_raw/gpm_list_raw wiring
// compiles; this test proves the *mechanism* those functions perform).
fn register_packages_root_anchor() -> (PluginId, VectorAddress, NodeId) {
    let plugin = PluginId::from_ascii("GPM_TEST_HARNESS");
    let vector = VectorAddress::new(0xF2, 0, 0, 0);
    let node_id = derive_node_id(plugin, "packages.root");
    let spec = NodeSpec {
        node_id,
        local_node_key: "packages.root",
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: ExecutorId::ZERO,
        state_schema_hash: 0,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    };
    gos_runtime::register_node(plugin, vector, spec).expect("register packages.root anchor");
    (plugin, vector, node_id)
}

#[test]
fn gpm_install_mints_a_package_node_mounted_under_packages_root() {
    let _guard = test_guard();
    boot_two_modules();
    run_steady_state_cycles(2);

    let (_plugin, root_vector, root_id) = register_packages_root_anchor();

    // gpm_install_raw's CreateNode step -- mirrors k-shell exactly (bypasses
    // the audited-envelope gate the same way k-cypher's own `CREATE (n)`
    // does; see gpm_install_raw's doc comment for why).
    let pkg_id = create_plain_node();

    // gpm_install_raw's Mount step -- through the standard supervisor gate,
    // stamped b"K_GPM" instead of b"K_SHELL"/b"K_AI".
    gos_supervisor::apply_cypher_mutation(
        CypherMutation::AddEdge { from: pkg_id, to: root_id, edge_kind: ReceptiveEdgeKind::Mount },
        *b"K_GPM\0\0\0\0\0\0\0\0\0\0\0",
    )
    .expect("gpm install's mount mutation must apply through the standard gate");

    // gpm_list_raw's read step: walk packages.root's edges (inbound, since
    // the package is `from` and packages.root is `to`) and find the Mount.
    let mut buf = [GraphEdgeSummary::EMPTY; 8];
    let (_total, returned) =
        gos_runtime::edge_page_for_node(root_vector, 0, &mut buf).expect("edge_page_for_node");
    let found = buf[..returned]
        .iter()
        .any(|e| e.edge_type == RuntimeEdgeType::Mount && e.to_vector == root_vector);
    assert!(found, "gpm list must see the freshly installed package as a Mount edge into packages.root");

    // The installed package is a real, individually-addressable node too --
    // node_page/render_live_graph (ADR-012's fast-path read) would surface
    // it in the 3D view, matching Appendix B's "subgraph appears in 3D
    // view" demo criterion.
    assert!(
        gos_runtime::node_summary_by_id(pkg_id).is_some(),
        "the installed package must be a real, individually-queryable node"
    );

    run_steady_state_cycles(2);
}

// ── ADR-017 §选项A — gos-ai-bridge as the single AI mutation gate ────────
//
// k-chat is a no_std kernel crate tied to COM2/asm hardware access and
// isn't host-compilable, matching every other k-chat/k-shell-adjacent test
// in this codebase. Two things follow from that split:
//
//   1. `gos_ai_bridge::wire::parse_gmut_line` and the `ask()`/`MutationGate`
//      machinery are *real*, directly host-testable code (they're pure
//      no_std logic, no asm) -- the tests below call them directly, not a
//      mirror. This is the actual "does the reversed control flow work"
//      proof (ADR-017 §1.1's `LlmBackend` reversal + §1.3's gate ownership).
//   2. k-chat's `llm_backend_query` frame-classification loop (which frame
//      prefix does what) is mirrored here as a pure function taking canned
//      lines instead of live COM2 reads -- this is the B3b-regression proof
//      that the pre-ADR-017 GRESP/GTOOL/GDONE round trip is unchanged by
//      the GMUT extension (`mirror_llm_backend_query` below).

fn hex32(byte: u8) -> String {
    let mut s = String::new();
    for _ in 0..16 {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

// ── wire::parse_gmut_line — real code, direct tests ──────────────────────

#[test]
fn adr017_wire_parse_gmut_create_node() {
    assert_eq!(
        gos_ai_bridge::wire::parse_gmut_line(b"create_node"),
        Some(CypherMutation::CreateNode {
            node_type: gos_protocol::RuntimeNodeType::Vector,
            entry_policy: gos_protocol::EntryPolicy::Manual,
            executor_id: gos_protocol::ExecutorId::ZERO,
        })
    );
}

#[test]
fn adr017_wire_parse_gmut_add_edge_mount_and_use() {
    let from = hex32(0x11);
    let to = hex32(0x22);
    for (kind_str, expect_use) in [("mount", false), ("use", true)] {
        let line = format!("add_edge:{from}:{to}:{kind_str}");
        let parsed = gos_ai_bridge::wire::parse_gmut_line(line.as_bytes());
        match parsed {
            Some(CypherMutation::AddEdge { edge_kind, .. }) => {
                assert_eq!(edge_kind == ReceptiveEdgeKind::Use, expect_use);
            }
            other => panic!("expected AddEdge for {kind_str}, got {other:?}"),
        }
    }
}

#[test]
fn adr017_wire_parse_gmut_remove_edge_and_rebind_use() {
    let edge_hex = hex32(0x33);
    let remove_line = format!("remove_edge:{edge_hex}");
    assert!(matches!(
        gos_ai_bridge::wire::parse_gmut_line(remove_line.as_bytes()),
        Some(CypherMutation::RemoveEdge { .. })
    ));

    let from = hex32(0x44);
    let to = hex32(0x55);
    let rebind_line = format!("rebind_use:{from}:{to}");
    assert!(matches!(
        gos_ai_bridge::wire::parse_gmut_line(rebind_line.as_bytes()),
        Some(CypherMutation::RebindUse { .. })
    ));
}

#[test]
fn adr017_wire_parse_gmut_rejects_depend_and_link_edge_kinds() {
    // ReceptiveEdgeKind::Depend/Link both pass pre_validate (boot-manifest /
    // interface-file mutations are legal in general) but must be
    // unreachable from the AI wire surface -- the wire parser is the actual
    // enforcement point (ADR-017 §1.2), not pre_validate.
    let from = hex32(0x66);
    let to = hex32(0x77);
    for kind in ["depend", "link", "bogus"] {
        let line = format!("add_edge:{from}:{to}:{kind}");
        assert!(
            gos_ai_bridge::wire::parse_gmut_line(line.as_bytes()).is_none(),
            "AI must not be able to mint a {kind} edge over the wire"
        );
    }
}

#[test]
fn adr017_wire_parse_gmut_rejects_malformed_frames() {
    assert!(gos_ai_bridge::wire::parse_gmut_line(b"remove_edge:tooshort").is_none());
    assert!(gos_ai_bridge::wire::parse_gmut_line(b"bogus_verb").is_none());
    assert!(gos_ai_bridge::wire::parse_gmut_line(b"").is_none());
}

// ── k-chat's llm_backend_query frame loop — mirrored (not host-compilable) ─

/// Mirrors `k-chat::llm_backend_query`'s frame-classification loop exactly
/// (COM2 reads replaced by a canned line iterator). Returns `Err(1)` for
/// "ran out of lines before GDONE" (timeout-equivalent) and `Err(2)` for an
/// unparseable `GMUT:` line (whole-turn rejection), matching the real
/// function's return codes.
fn mirror_llm_backend_query(lines: &[&[u8]]) -> Result<gos_ai_bridge::LlmResponse, i32> {
    let mut response = gos_ai_bridge::LlmResponse::empty();
    for &frame in lines {
        if let Some(text) = frame.strip_prefix(b"GRESP:") {
            let start = response.text_len as usize;
            let remaining = gos_ai_bridge::MAX_RESPONSE_BYTES - start;
            let to_copy = text.len().min(remaining.saturating_sub(1));
            response.text[start..start + to_copy].copy_from_slice(&text[..to_copy]);
            response.text_len += to_copy as u16;
            if (response.text_len as usize) < gos_ai_bridge::MAX_RESPONSE_BYTES {
                response.text[response.text_len as usize] = b'\n';
                response.text_len += 1;
            }
        } else if frame.strip_prefix(b"GTOOL:").is_some() {
            // Side-effecting inline dispatch in the real function (VGA +
            // k-net signals) -- deliberately not reflected into the typed
            // response here, same as the real function.
        } else if let Some(mutation_line) = frame.strip_prefix(b"GMUT:") {
            match gos_ai_bridge::wire::parse_gmut_line(mutation_line) {
                Some(mutation) => {
                    let idx = response.mutation_count as usize;
                    if idx < gos_ai_bridge::MAX_SUGGESTED_MUTATIONS {
                        response.mutations[idx] = Some(mutation);
                        response.mutation_count += 1;
                    }
                }
                None => return Err(2),
            }
        } else if frame == b"GDONE:" {
            while response.text_len > 0 && response.text[response.text_len as usize - 1] == b'\n' {
                response.text_len -= 1;
            }
            return Ok(response);
        }
        // Unknown frame prefix -- silently ignored and keep reading, same
        // as the real function.
    }
    Err(1)
}

#[test]
fn adr017_mirror_resp_and_done_round_trip_unaffected_by_gmut_extension() {
    let response = mirror_llm_backend_query(&[b"GRESP:hello", b"GRESP:world", b"GDONE:"])
        .expect("well-formed turn must succeed");
    assert_eq!(response.text_bytes(), b"hello\nworld");
    assert_eq!(response.mutation_count, 0);
}

#[test]
fn adr017_mirror_gtool_frames_are_excluded_from_response_text() {
    let response = mirror_llm_backend_query(&[
        b"GRESP:before",
        b"GTOOL:ping:1.1.1.1",
        b"GRESP:after",
        b"GDONE:",
    ])
    .expect("turn with a tool frame must still succeed");
    assert_eq!(
        response.text_bytes(),
        b"before\nafter",
        "GTOOL frames must not leak into the typed response text"
    );
}

#[test]
fn adr017_mirror_valid_gmut_add_edge_is_captured() {
    let from = hex32(0x11);
    let to = hex32(0x22);
    let line = format!("GMUT:add_edge:{from}:{to}:mount");
    let response = mirror_llm_backend_query(&[b"GRESP:ok", line.as_bytes(), b"GDONE:"])
        .expect("valid GMUT line must succeed");
    assert_eq!(response.mutation_count, 1);
    assert!(matches!(
        response.mutations[0],
        Some(CypherMutation::AddEdge { edge_kind: ReceptiveEdgeKind::Mount, .. })
    ));
}

#[test]
fn adr017_mirror_bad_gmut_line_rejects_the_whole_turn() {
    let err = mirror_llm_backend_query(&[
        b"GRESP:ok",
        b"GMUT:add_edge:not-hex:also-not-hex:mount",
        b"GDONE:",
    ])
    .expect_err("an unparseable GMUT line must reject the entire turn");
    assert_eq!(err, 2);
}

#[test]
fn adr017_mirror_missing_gdone_is_a_timeout() {
    let err = mirror_llm_backend_query(&[b"GRESP:ok"])
        .expect_err("no GDONE before lines run out must be treated as a timeout");
    assert_eq!(err, 1);
}

// ── ask() + MutationGate lifecycle — proves §1.1's reversal end-to-end ────

struct StubPlan {
    text: &'static [u8],
    mutations: Vec<CypherMutation>,
    rc: i32,
}

static STUB_PLAN: Mutex<Option<StubPlan>> = Mutex::new(None);

/// Deterministic `LlmBackend` stub (mirrors the doc comment on
/// `gos_ai_bridge::LlmBackend`: "host harnesses install a deterministic
/// stub for tests"). Reads a plan set by `set_stub_plan` -- callers must set
/// one before calling `ask()`.
unsafe extern "C" fn stub_backend_query(
    _prompt: *const u8,
    _prompt_len: u32,
    _context: *const u8,
    _context_len: u32,
    out_response: *mut gos_ai_bridge::LlmResponse,
) -> i32 {
    let plan = STUB_PLAN
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .take()
        .expect("set_stub_plan must be called before ask()");
    if plan.rc != 0 {
        return plan.rc;
    }
    let mut response = gos_ai_bridge::LlmResponse::empty();
    let n = plan.text.len().min(gos_ai_bridge::MAX_RESPONSE_BYTES);
    response.text[..n].copy_from_slice(&plan.text[..n]);
    response.text_len = n as u16;
    for m in plan.mutations.iter().take(gos_ai_bridge::MAX_SUGGESTED_MUTATIONS) {
        let idx = response.mutation_count as usize;
        response.mutations[idx] = Some(*m);
        response.mutation_count += 1;
    }
    unsafe { *out_response = response; }
    0
}

fn install_stub_backend() {
    gos_ai_bridge::install_backend(gos_ai_bridge::LlmBackend { query: stub_backend_query });
}

fn set_stub_plan(text: &'static [u8], mutations: Vec<CypherMutation>) {
    *STUB_PLAN.lock().unwrap_or_else(|poison| poison.into_inner()) =
        Some(StubPlan { text, mutations, rc: 0 });
}

#[test]
fn adr017_ask_stages_mutations_and_chat_approve_applies_through_the_standard_gate() {
    let _guard = test_guard();
    boot_two_modules();
    run_steady_state_cycles(1);
    gos_ai_bridge::gate_clear();
    install_stub_backend();

    let a = create_plain_node();
    let b = create_plain_node();

    set_stub_plan(
        b"sure, mounting a under b",
        vec![CypherMutation::AddEdge { from: a, to: b, edge_kind: ReceptiveEdgeKind::Mount }],
    );

    let req = gos_ai_bridge::LlmRequest {
        prompt: b"mount a under b",
        context: &[],
        mode: gos_ai_bridge::AcceptanceMode::Confirmed,
    };
    let response = gos_ai_bridge::ask(&req).expect("stub backend turn must succeed");
    assert_eq!(response.text_bytes(), b"sure, mounting a under b");

    // §1.1's reversal, mirroring k-chat's send_via_ai_bridge exactly: the
    // suggestion is staged, not applied.
    let staged = gos_ai_bridge::gate_enqueue(&response, req.mode);
    assert_eq!(staged, 1);
    assert!(
        !use_edge_present(a, b, "cypher.mount"),
        "AI suggestions must not auto-apply -- operator approval required"
    );

    // `chat approve 0` -- mirrors k-chat's ai_approve exactly: pull off the
    // gate, apply through gos_supervisor::apply_cypher_mutation stamped
    // b"K_AI" (same gate a human/gpm mutation uses -- Parity invariant).
    let mutation = gos_ai_bridge::gate_accept_index(0).expect("mutation must still be pending");
    const AI_SOURCE: [u8; 16] = *b"K_AI\0\0\0\0\0\0\0\0\0\0\0\0";
    gos_supervisor::apply_cypher_mutation(mutation, AI_SOURCE)
        .expect("approved AI mutation must apply through the standard gate");

    assert!(use_edge_present(a, b, "cypher.mount"), "approved mutation must now be live");
    assert_eq!(
        gos_ai_bridge::gate_len(),
        0,
        "gate must be empty after the only pending suggestion was approved"
    );

    run_steady_state_cycles(1);
}

#[test]
fn adr017_dry_run_mode_never_stages_anything() {
    let _guard = test_guard();
    boot_two_modules();
    gos_ai_bridge::gate_clear();
    install_stub_backend();

    let a = create_plain_node();
    let b = create_plain_node();
    set_stub_plan(
        b"here is what I would do",
        vec![CypherMutation::AddEdge { from: a, to: b, edge_kind: ReceptiveEdgeKind::Mount }],
    );

    let req = gos_ai_bridge::LlmRequest {
        prompt: b"what would you do",
        context: &[],
        mode: gos_ai_bridge::AcceptanceMode::DryRun,
    };
    let response = gos_ai_bridge::ask(&req).expect("dry-run turn must still succeed and return text");
    let staged = gos_ai_bridge::gate_enqueue(&response, req.mode);
    assert_eq!(staged, 0, "DryRun must never stage a mutation for approval");
    assert_eq!(gos_ai_bridge::gate_len(), 0);
}

#[test]
fn adr017_reject_drops_the_suggestion_without_applying_it() {
    let _guard = test_guard();
    boot_two_modules();
    gos_ai_bridge::gate_clear();
    install_stub_backend();

    let a = create_plain_node();
    let b = create_plain_node();
    set_stub_plan(
        b"suggestion",
        vec![CypherMutation::AddEdge { from: a, to: b, edge_kind: ReceptiveEdgeKind::Use }],
    );

    let req = gos_ai_bridge::LlmRequest {
        prompt: b"x",
        context: &[],
        mode: gos_ai_bridge::AcceptanceMode::Confirmed,
    };
    let response = gos_ai_bridge::ask(&req).expect("turn must succeed");
    gos_ai_bridge::gate_enqueue(&response, req.mode);

    assert!(gos_ai_bridge::gate_reject_index(0), "rejecting a pending index must succeed");
    assert!(!use_edge_present(a, b, "cypher.use"), "rejected mutation must never be applied");
    assert_eq!(gos_ai_bridge::gate_len(), 0);
}
