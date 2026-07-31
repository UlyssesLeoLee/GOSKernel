//! V2.2b — boot-as-fixpoint resolution ([`doc/ADR-002`] §3-4).
//!
//! Models GOS's actual `kernel_main` boot steps as a `Depend` graph and proves:
//!   * the resolver derives a valid topological order (every dep respected);
//!   * that order matches the real `kernel_main` sequence (so the engine could
//!     drive boot identically — the V2.2b wiring step);
//!   * a dependency cycle is reported, not hung (ADR-002 §4: a cycle = a boot
//!     graph that can never reach quiescence).
//!
//! `NODES`/`DEPS` come from [`gos_mutation_dispatch::boot::gos_kernel`] — the *same*
//! consts `hypervisor::main::run_boot` resolves to order real boot. This is
//! no longer a hand-modeled copy of the boot graph: it *is* the boot graph,
//! so this test and the kernel cannot drift apart.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "boot_order.rs", type: "file", language: "rust"}),
//!   (h:Function {name: "respects_deps", type: "function", visibility: "private"}),
//!   (t1:Function {name: "resolves_real_boot_order_matching_kernel_main", type: "function"}),
//!   (t2:Function {name: "reordered_dep_declarations_still_resolve_validly", type: "function"}),
//!   (t3:Function {name: "dependency_cycle_is_reported_not_hung", type: "function"}),
//!   (gk:Module {name: "gos_mutation_dispatch::boot::gos_kernel", type: "module"}),
//!   (f)-[:CONTAINS]->(h), (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (t1)-[:CALLS]->(h), (t2)-[:CALLS]->(h),
//!   (t1)-[:USES]->(gk), (t2)-[:USES]->(gk), (t3)-[:USES]->(gk);
//! ```

use gos_mutation_dispatch::boot::gos_kernel::{
    ACTIVATE_KERNEL_TIER, BUILTIN_GRAPH, CPU_FEATURES, DEPS, GDT_INIT, HAL_INIT, IDT_INIT,
    INSTALL_MODULES, NODES, PIC_INIT, PS2_DRAIN, REALIZE_MODULES, RING3, STEADY_STATE,
    SUPERVISOR_BOOTSTRAP,
};
use gos_mutation_dispatch::boot::{resolve_boot_order, BootNodeId, BootResolveError, Depend};

/// True iff `order` lists every `on` before its dependent `node`.
fn respects_deps(order: &[BootNodeId], deps: &[Depend]) -> bool {
    let pos = |id: BootNodeId| order.iter().position(|n| n.0 == id.0);
    deps.iter().all(|d| match (pos(d.on), pos(d.node)) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    })
}

#[test]
fn resolves_real_boot_order_matching_kernel_main() {
    let order = resolve_boot_order(&NODES, &DEPS).expect("acyclic boot graph resolves");
    let got = order.as_slice();

    assert!(respects_deps(got, &DEPS), "resolved order must respect every Depend");

    // With a linear chain + input-order tie-breaking, the resolved order is
    // exactly the kernel_main sequence — i.e. the engine boots identically.
    assert_eq!(got, &NODES[..], "resolved order should match kernel_main");
}

#[test]
fn reordered_dep_declarations_still_resolve_validly() {
    // ADR-002 demo: shuffling how dependencies are *declared* must not change
    // correctness — the order is solved, not authored. Same edges, reversed
    // declaration order.
    let mut shuffled = DEPS;
    shuffled.reverse();
    let order = resolve_boot_order(&NODES, &shuffled).expect("still acyclic");
    assert!(
        respects_deps(order.as_slice(), &shuffled),
        "a valid order must exist regardless of declaration order"
    );
}

#[test]
fn dependency_cycle_is_reported_not_hung() {
    // Inject a back-edge SUPERVISOR_BOOTSTRAP -> CPU_FEATURES, closing a cycle.
    let mut cyclic = [
        Depend { node: HAL_INIT, on: CPU_FEATURES },
        Depend { node: SUPERVISOR_BOOTSTRAP, on: HAL_INIT },
        Depend { node: CPU_FEATURES, on: SUPERVISOR_BOOTSTRAP }, // <- cycle
        Depend { node: BUILTIN_GRAPH, on: INSTALL_MODULES },
        Depend { node: REALIZE_MODULES, on: BUILTIN_GRAPH },
        Depend { node: GDT_INIT, on: REALIZE_MODULES },
        Depend { node: IDT_INIT, on: GDT_INIT },
        Depend { node: PIC_INIT, on: IDT_INIT },
        Depend { node: PS2_DRAIN, on: PIC_INIT },
        Depend { node: ACTIVATE_KERNEL_TIER, on: PS2_DRAIN },
        Depend { node: RING3, on: ACTIVATE_KERNEL_TIER },
        Depend { node: STEADY_STATE, on: RING3 },
    ];
    // (keep the array used; ordering inside doesn't matter)
    cyclic.reverse();
    let result = resolve_boot_order(&NODES, &cyclic);
    assert_eq!(
        result.err(),
        Some(BootResolveError::Cycle),
        "a dependency cycle must be reported as non-quiescent, not silently dropped"
    );
}
