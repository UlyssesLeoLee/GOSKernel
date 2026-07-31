//! Boot-as-fixpoint — resolve boot order from `Depend` edges ([`doc/ADR-002`] §3-4).
//!
//! ADR-002 §3: boot order is *solved* from the dependency graph, not hardcoded.
//! A boot node fires once all its `Depend` predecessors have fired; the fire
//! order is any topological linearization of the `Depend` DAG. ADR-002 §4: a
//! dependency **cycle** means the boot graph can never reach quiescence — a
//! configuration bug, which this resolver reports (it never hangs).
//!
//! This is the V2.2b *core*, verifiable in isolation: it derives and validates
//! the order. Wiring it to actually drive `kernel_main` (replacing the
//! hardcoded call sequence) is the on-target step that follows — kept separate
//! so the working boot path is untouched until that change is boot-smoked.
//!
//! `no_std`, no `alloc`: fixed-capacity Kahn topological sort, ties broken by
//! input declaration order so the result is deterministic and assertable.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "boot.rs", type: "file", language: "rust"}),
//!   (bn:Class {name: "BootNodeId", type: "struct"}),
//!   (dep:Class {name: "Depend", type: "struct"}),
//!   (bo:Class {name: "BootOrder", type: "struct"}),
//!   (err:Class {name: "BootResolveError", type: "enum"}),
//!   (resolve:Function {name: "resolve_boot_order", type: "function", visibility: "pub"}),
//!   (slice:Function {name: "BootOrder::as_slice", type: "function", visibility: "pub"}),
//!   (idx:Function {name: "index_of", type: "function", visibility: "private"}),
//!   (f)-[:CONTAINS]->(bn), (f)-[:CONTAINS]->(dep), (f)-[:CONTAINS]->(bo), (f)-[:CONTAINS]->(err),
//!   (f)-[:CONTAINS]->(resolve),
//!   (bo)-[:HAS_METHOD]->(slice),
//!   (resolve)-[:CALLS]->(idx), (resolve)-[:USES]->(dep);
//! ```

/// Max boot nodes the resolver handles (the real boot graph is ~10 nodes).
pub const MAX_BOOT_NODES: usize = 32;

/// Opaque boot-step identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BootNodeId(pub u32);

/// A `Depend` edge: `node` must fire **after** `on` (i.e. `node` depends on `on`).
#[derive(Clone, Copy, Debug)]
pub struct Depend {
    pub node: BootNodeId,
    pub on: BootNodeId,
}

/// A resolved boot order — a valid topological linearization of the `Depend` DAG.
#[derive(Clone, Copy, Debug)]
pub struct BootOrder {
    order: [BootNodeId; MAX_BOOT_NODES],
    len: usize,
}

impl BootOrder {
    pub fn as_slice(&self) -> &[BootNodeId] {
        &self.order[..self.len]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BootResolveError {
    /// A dependency cycle — the boot graph can never quiesce (ADR-002 §4).
    Cycle,
    /// More nodes than [`MAX_BOOT_NODES`], or a `Depend` references an unknown node.
    Malformed,
}

fn index_of(nodes: &[BootNodeId], id: BootNodeId) -> Option<usize> {
    let mut i = 0;
    while i < nodes.len() {
        if nodes[i].0 == id.0 {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Resolve a deterministic boot order over the `Depend` graph (Kahn topological
/// sort, ties broken by `nodes` declaration order). Returns [`BootResolveError::Cycle`]
/// if no linearization exists — boot would never reach quiescence.
pub fn resolve_boot_order(
    nodes: &[BootNodeId],
    deps: &[Depend],
) -> Result<BootOrder, BootResolveError> {
    let n = nodes.len();
    if n > MAX_BOOT_NODES {
        return Err(BootResolveError::Malformed);
    }

    // in_degree[i] = number of unsatisfied dependencies of nodes[i].
    let mut in_degree = [0usize; MAX_BOOT_NODES];
    let mut d = 0;
    while d < deps.len() {
        // Both endpoints must be known nodes.
        if index_of(nodes, deps[d].on).is_none() {
            return Err(BootResolveError::Malformed);
        }
        match index_of(nodes, deps[d].node) {
            Some(i) => in_degree[i] += 1,
            None => return Err(BootResolveError::Malformed),
        }
        d += 1;
    }

    let mut placed = [false; MAX_BOOT_NODES];
    let mut order = [BootNodeId(0); MAX_BOOT_NODES];
    let mut placed_count = 0;

    while placed_count < n {
        // First unplaced node whose dependencies are all satisfied.
        let mut pick = None;
        let mut i = 0;
        while i < n {
            if !placed[i] && in_degree[i] == 0 {
                pick = Some(i);
                break;
            }
            i += 1;
        }

        let Some(idx) = pick else {
            // No zero-in-degree node left but nodes remain -> a cycle.
            return Err(BootResolveError::Cycle);
        };

        placed[idx] = true;
        order[placed_count] = nodes[idx];
        placed_count += 1;

        // Relax dependents: any node that depended on nodes[idx] loses one dep.
        let mut e = 0;
        while e < deps.len() {
            if deps[e].on.0 == nodes[idx].0 {
                if let Some(j) = index_of(nodes, deps[e].node) {
                    in_degree[j] -= 1;
                }
            }
            e += 1;
        }
    }

    Ok(BootOrder { order, len: placed_count })
}

/// GOS `kernel_main`'s real boot manifest (V2.2b *wiring*).
///
/// `hypervisor::main` resolves [`DEPS`] over [`NODES`] with
/// [`resolve_boot_order`] and dispatches each step through the result,
/// instead of hardcoding the call sequence as source order (ADR-002 §3:
/// "boot order is solved, not encoded"). `host-tests/gos-mutation-dispatch-harness`'s
/// `boot_order.rs` imports these same consts, so "the manifest under test"
/// and "the manifest the kernel actually boots from" cannot drift apart —
/// shuffling [`DEPS`]' declaration order still resolves to a valid sequence,
/// and a cycle is reported rather than hung (ADR-002 §4).
///
/// ```cypher
/// CREATE
///   (m:Module {name: "gos_mutation_dispatch::boot::gos_kernel", type: "module"}),
///   (nodes:Const {name: "NODES", type: "const"}),
///   (deps:Const {name: "DEPS", type: "const"}),
///   (m)-[:CONTAINS]->(nodes), (m)-[:CONTAINS]->(deps),
///   (deps)-[:USES]->(nodes);
/// ```
pub mod gos_kernel {
    use super::{BootNodeId, Depend};

    pub const CPU_FEATURES: BootNodeId = BootNodeId(1);
    pub const HAL_INIT: BootNodeId = BootNodeId(2);
    pub const SUPERVISOR_BOOTSTRAP: BootNodeId = BootNodeId(3);
    pub const INSTALL_MODULES: BootNodeId = BootNodeId(4);
    pub const BUILTIN_GRAPH: BootNodeId = BootNodeId(5);
    pub const REALIZE_MODULES: BootNodeId = BootNodeId(6);
    // V2.2d: the former single KERNEL_DRIVERS node is now its own
    // GDT -> IDT -> PIC -> PS2_DRAIN -> ACTIVATE_KERNEL_TIER sub-chain —
    // hardware ordering constraints expressed as Depend edges instead of a
    // hand-written call sequence in `init_kernel_tier_drivers`.
    pub const GDT_INIT: BootNodeId = BootNodeId(7);
    pub const IDT_INIT: BootNodeId = BootNodeId(8); // IDT gates encode GDT selectors
    pub const PIC_INIT: BootNodeId = BootNodeId(9); // 8259 remap before IRQs are live
    pub const PS2_DRAIN: BootNodeId = BootNodeId(10); // stale i8042 byte, after PIC remap
    pub const ACTIVATE_KERNEL_TIER: BootNodeId = BootNodeId(11); // on_init/on_resume pass
    pub const RING3: BootNodeId = BootNodeId(12); // syscall MSRs — needs GDT live
    pub const STEADY_STATE: BootNodeId = BootNodeId(13);

    pub const NODES: [BootNodeId; 13] = [
        CPU_FEATURES,
        HAL_INIT,
        SUPERVISOR_BOOTSTRAP,
        INSTALL_MODULES,
        BUILTIN_GRAPH,
        REALIZE_MODULES,
        GDT_INIT,
        IDT_INIT,
        PIC_INIT,
        PS2_DRAIN,
        ACTIVATE_KERNEL_TIER,
        RING3,
        STEADY_STATE,
    ];

    // The real dependency structure: a bootstrap chain, plus RING3 explicitly
    // depending on the end of the kernel-driver sub-chain (the GDT must be
    // live before syscall MSRs — see `hypervisor::ring3::init`).
    pub const DEPS: [Depend; 12] = [
        Depend { node: HAL_INIT, on: CPU_FEATURES },
        Depend { node: SUPERVISOR_BOOTSTRAP, on: HAL_INIT },
        Depend { node: INSTALL_MODULES, on: SUPERVISOR_BOOTSTRAP },
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
}
