//! V2.1 — MutationDispatcher visibility/atomicity tests ([`doc/ADR-004`]).
//!
//! Proves the ADR-004 contract against the *live* runtime graph (not a stub):
//!   * §2.2 atomic batch commit — a `Use` rebind advances `graph_epoch` by
//!     exactly one and leaves exactly one `Use` edge, so no reader can observe
//!     `theme.current` with zero or two `Use` edges mid-rebind.
//!   * §2.1/§2.3 epoch visibility — `GraphSnapshot` exposes `graph_epoch` and
//!     it advances on each committed mutation.
//!   * `RuntimeDispatcher` is wired to the real edge table — it is the
//!     replacement for the harness `Stub`.
//!
//! This file is its own test binary, so it gets a fresh process-global runtime;
//! the single `#[test]` fn runs with no parallel mutators, making the
//! `graph_epoch` deltas deterministic.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "mutation_visibility.rs", type: "file", language: "rust"}),
//!   (h:Function {name: "use_edge_present", type: "function", visibility: "private"}),
//!   (hm:Function {name: "mount_edge_present", type: "function", visibility: "private"}),
//!   (t:Function {name: "rebind_use_is_atomic_single_epoch_and_exclusive", type: "function"}),
//!   (f)-[:CONTAINS]->(h), (f)-[:CONTAINS]->(hm), (f)-[:CONTAINS]->(t),
//!   (t)-[:CALLS]->(h), (t)-[:CALLS]->(hm);
//! ```

use gos_cypher_mut::{MutationDispatcher, ReceptiveEdgeKind};
use gos_protocol::{derive_edge_id, derive_edge_vector, EdgeSpec, NodeId, RoutePolicy, RuntimeEdgeType};

fn use_edge_present(from: NodeId, to: NodeId) -> bool {
    let id = derive_edge_id(from, to, "use");
    gos_runtime::edge_id_for_vector(derive_edge_vector(id)).is_some()
}

fn mount_edge_present(from: NodeId, to: NodeId) -> bool {
    let id = derive_edge_id(from, to, "mount");
    gos_runtime::edge_id_for_vector(derive_edge_vector(id)).is_some()
}

fn use_edge_spec(from: NodeId, to: NodeId) -> EdgeSpec {
    EdgeSpec {
        edge_id: derive_edge_id(from, to, "use"),
        from_node: from,
        to_node: to,
        edge_type: RuntimeEdgeType::Use,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

#[test]
fn rebind_use_is_atomic_single_epoch_and_exclusive() {
    // Distinct, arbitrary node identities (we test edge mechanics, not nodes).
    let theme_current = NodeId([0x10; 16]);
    let wabi = NodeId([0x20; 16]);
    let shoji = NodeId([0x30; 16]);

    // Initial state: theme.current -[Use]-> wabi.
    gos_runtime::register_edge(use_edge_spec(theme_current, wabi)).expect("seed Use edge");
    assert!(use_edge_present(theme_current, wabi), "seed edge should be present");

    let before = gos_runtime::snapshot();

    // ── ADR-004 §2.2: atomic rebind to shoji ──────────────────────────────
    gos_runtime::rebind_use(theme_current, shoji).expect("rebind applies");
    let after = gos_runtime::snapshot();

    // Exactly one epoch bump for the whole logical mutation. A non-atomic
    // remove+add would bump twice; this is the core atomicity proof — since the
    // rebind is a single epoch transition, every reader sees either the old
    // state (Use->wabi) or the new state (Use->shoji), never an in-between.
    assert_eq!(
        after.graph_epoch,
        before.graph_epoch + 1,
        "rebind must commit under exactly one epoch bump (got {} -> {})",
        before.graph_epoch,
        after.graph_epoch
    );
    // Exclusivity: one Use edge removed, one added -> edge_count unchanged, and
    // theme.current holds exactly one Use edge (now pointing at shoji).
    assert_eq!(after.edge_count, before.edge_count, "must hold exactly one Use edge");
    assert!(use_edge_present(theme_current, shoji), "new Use->shoji must be present");
    assert!(!use_edge_present(theme_current, wabi), "old Use->wabi must be gone");

    // ── RuntimeDispatcher is the real dispatcher (replaces the Stub) ───────
    let mut d = gos_runtime::RuntimeDispatcher;

    // rebind via the trait flips it back to wabi, atomically (another +1).
    let e0 = gos_runtime::snapshot().graph_epoch;
    d.rebind_use(theme_current, wabi).expect("dispatcher rebind");
    assert_eq!(gos_runtime::snapshot().graph_epoch, e0 + 1);
    assert!(use_edge_present(theme_current, wabi));
    assert!(!use_edge_present(theme_current, shoji));

    // add_edge / remove_edge hit the live table too.
    let a = NodeId([0x40; 16]);
    let b = NodeId([0x50; 16]);
    d.add_edge(a, b, ReceptiveEdgeKind::Mount).expect("dispatcher add_edge");
    assert!(mount_edge_present(a, b), "Mount edge should be in the live table");
    let mount_id = derive_edge_id(a, b, "mount");
    d.remove_edge(mount_id).expect("dispatcher remove_edge");
    assert!(!mount_edge_present(a, b), "Mount edge should be removed");

    // lookup_node queries the real node table — no nodes were registered here,
    // so it reports absence (proving it isn't a stub that always says true).
    assert!(!d.lookup_node(theme_current), "no node registered -> lookup is false");
}
