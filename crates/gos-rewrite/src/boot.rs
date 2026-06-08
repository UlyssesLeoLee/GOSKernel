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
