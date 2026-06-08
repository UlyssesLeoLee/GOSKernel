//! V2.2b core — boot-as-fixpoint resolution ([`doc/ADR-002`] §3-4).
//!
//! Models GOS's actual `kernel_main` boot steps as a `Depend` graph and proves:
//!   * the resolver derives a valid topological order (every dep respected);
//!   * that order matches the real `kernel_main` sequence (so the engine could
//!     drive boot identically — the V2.2b wiring step);
//!   * a dependency cycle is reported, not hung (ADR-002 §4: a cycle = a boot
//!     graph that can never reach quiescence).
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "boot_order.rs", type: "file", language: "rust"}),
//!   (h:Function {name: "respects_deps", type: "function", visibility: "private"}),
//!   (t1:Function {name: "resolves_real_boot_order_matching_kernel_main", type: "function"}),
//!   (t2:Function {name: "reordered_dep_declarations_still_resolve_validly", type: "function"}),
//!   (t3:Function {name: "dependency_cycle_is_reported_not_hung", type: "function"}),
//!   (f)-[:CONTAINS]->(h), (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (t1)-[:CALLS]->(h), (t2)-[:CALLS]->(h);
//! ```

use gos_rewrite::boot::{resolve_boot_order, BootNodeId, BootResolveError, Depend};

// GOS kernel_main boot steps, in their actual source order (main.rs:16-128).
const CPU_FEATURES: BootNodeId = BootNodeId(1);
const HAL_INIT: BootNodeId = BootNodeId(2);
const SUPERVISOR_BOOTSTRAP: BootNodeId = BootNodeId(3);
const INSTALL_MODULES: BootNodeId = BootNodeId(4);
const BUILTIN_GRAPH: BootNodeId = BootNodeId(5);
const REALIZE_MODULES: BootNodeId = BootNodeId(6);
const KERNEL_DRIVERS: BootNodeId = BootNodeId(7); // GDT/IDT/PIC
const RING3: BootNodeId = BootNodeId(8); // syscall MSRs — needs GDT live
const STEADY_STATE: BootNodeId = BootNodeId(9);

fn nodes() -> [BootNodeId; 9] {
    [
        CPU_FEATURES,
        HAL_INIT,
        SUPERVISOR_BOOTSTRAP,
        INSTALL_MODULES,
        BUILTIN_GRAPH,
        REALIZE_MODULES,
        KERNEL_DRIVERS,
        RING3,
        STEADY_STATE,
    ]
}

// The real dependency structure: a bootstrap chain, plus RING3 explicitly
// depending on KERNEL_DRIVERS (the GDT must be live before syscall MSRs — see
// main.rs "ring3: skipping IA32_STAR ... kernel data segment not yet in GDT").
fn deps() -> [Depend; 8] {
    [
        Depend { node: HAL_INIT, on: CPU_FEATURES },
        Depend { node: SUPERVISOR_BOOTSTRAP, on: HAL_INIT },
        Depend { node: INSTALL_MODULES, on: SUPERVISOR_BOOTSTRAP },
        Depend { node: BUILTIN_GRAPH, on: INSTALL_MODULES },
        Depend { node: REALIZE_MODULES, on: BUILTIN_GRAPH },
        Depend { node: KERNEL_DRIVERS, on: REALIZE_MODULES },
        Depend { node: RING3, on: KERNEL_DRIVERS },
        Depend { node: STEADY_STATE, on: RING3 },
    ]
}

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
    let order = resolve_boot_order(&nodes(), &deps()).expect("acyclic boot graph resolves");
    let got = order.as_slice();

    assert!(respects_deps(got, &deps()), "resolved order must respect every Depend");

    // With a linear chain + input-order tie-breaking, the resolved order is
    // exactly the kernel_main sequence — i.e. the engine would boot identically.
    assert_eq!(got, &nodes()[..], "resolved order should match kernel_main");
}

#[test]
fn reordered_dep_declarations_still_resolve_validly() {
    // ADR-002 demo: shuffling how dependencies are *declared* must not change
    // correctness — the order is solved, not authored. Same edges, reversed
    // declaration order.
    let mut shuffled = deps();
    shuffled.reverse();
    let order = resolve_boot_order(&nodes(), &shuffled).expect("still acyclic");
    assert!(
        respects_deps(order.as_slice(), &shuffled),
        "a valid order must exist regardless of declaration order"
    );
}

#[test]
fn dependency_cycle_is_reported_not_hung() {
    // Inject a back-edge STEADY_STATE -> CPU_FEATURES, closing a cycle.
    let mut cyclic = [
        Depend { node: HAL_INIT, on: CPU_FEATURES },
        Depend { node: SUPERVISOR_BOOTSTRAP, on: HAL_INIT },
        Depend { node: CPU_FEATURES, on: SUPERVISOR_BOOTSTRAP }, // <- cycle
        Depend { node: BUILTIN_GRAPH, on: INSTALL_MODULES },
        Depend { node: REALIZE_MODULES, on: BUILTIN_GRAPH },
        Depend { node: KERNEL_DRIVERS, on: REALIZE_MODULES },
        Depend { node: RING3, on: KERNEL_DRIVERS },
        Depend { node: STEADY_STATE, on: RING3 },
    ];
    // (keep the array used; ordering inside doesn't matter)
    cyclic.reverse();
    let result = resolve_boot_order(&nodes(), &cyclic);
    assert_eq!(
        result.err(),
        Some(BootResolveError::Cycle),
        "a dependency cycle must be reported as non-quiescent, not silently dropped"
    );
}
