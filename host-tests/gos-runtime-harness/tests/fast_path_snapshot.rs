//! ADR-012 option B — the equivalence obligation for
//! `PermissionKind::FastPathSnapshot`.
//!
//! Any node declaring `FastPathSnapshot` (today: `k-vk-host`'s
//! `render_live_graph`, via `VK_PERMS` in `builtin_bundle.rs`) is permitted
//! to bulk-read `node_page`/`edge_page` instead of receiving per-edge
//! `on_event`/`Subscribe` delivery. That permission is only sound if the
//! bulk snapshot is a **projection** of the same state a per-edge observer
//! would see, never a second, independently-diverging source of truth
//! (`doc/ADR-012` §一.2's "the table is the graph" framing, mirroring
//! `doc/ADR-006`).
//!
//! `register_node`/`register_edge` are the *only* write path regardless of
//! how a reader later observes the result — there's no separate "fast" vs
//! "slow" write side to diverge — so the obligation reduces to: does
//! `node_page`/`edge_page`'s bulk walk return exactly the same records
//! `node_summary`/individual edge lookups. This harness constructs a small
//! synthetic graph and proves that page-by-page.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "fast_path_snapshot.rs", type: "file", language: "rust"}),
//!   (np:Function {name: "node_page", type: "function", visibility: "pub"}),
//!   (ep:Function {name: "edge_page", type: "function", visibility: "pub"}),
//!   (ns:Function {name: "node_summary", type: "function", visibility: "pub"}),
//!   (t1:Function {name: "node_page_snapshot_matches_individual_node_queries", type: "function"}),
//!   (t2:Function {name: "edge_page_snapshot_matches_individually_registered_edges", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2),
//!   (t1)-[:USES]->(np), (t1)-[:USES]->(ns), (t2)-[:USES]->(ep);
//! ```

use std::sync::Mutex;

use gos_protocol::{
    derive_edge_id, derive_node_id, EdgeSpec, EntryPolicy, ExecutorId, GraphEdgeSummary,
    GraphNodeSummary, NodeSpec, PluginId, RoutePolicy, RuntimeEdgeType, RuntimeNodeType,
    VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const P_ID: PluginId = PluginId::from_ascii("FASTPATH_HARNESS");
const EXEC: ExecutorId = ExecutorId::from_ascii("fastpath.exec");

fn node_spec(key: &'static str) -> NodeSpec {
    NodeSpec {
        node_id: derive_node_id(P_ID, key),
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: EXEC,
        state_schema_hash: 0,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

fn edge_spec(from: VectorAddress, to: VectorAddress, key: &'static str) -> EdgeSpec {
    let from_node = from_node_id(from);
    let to_node = from_node_id(to);
    EdgeSpec {
        edge_id: derive_edge_id(from_node, to_node, key),
        from_node,
        to_node,
        edge_type: RuntimeEdgeType::Mount,
        weight: 1.0,
        acl_mask: u64::MAX,
        route_policy: RoutePolicy::Direct,
        capability_namespace: None,
        capability_binding: None,
        vector_ref: None,
    }
}

// Small helper so `edge_spec` can resolve a VectorAddress back to the
// NodeId that was registered at it, keeping the two test functions
// self-contained (no cross-test shared state).
fn from_node_id(vector: VectorAddress) -> gos_protocol::NodeId {
    gos_runtime::node_id_for_vec(vector).expect("vector must already be registered")
}

#[test]
fn node_page_snapshot_matches_individual_node_queries() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    gos_runtime::reset();

    let vectors = [
        VectorAddress::new(0xF0, 0, 0, 0),
        VectorAddress::new(0xF0, 0, 0, 1),
        VectorAddress::new(0xF0, 0, 0, 2),
        VectorAddress::new(0xF0, 0, 0, 3),
    ];
    let keys = ["fp.a", "fp.b", "fp.c", "fp.d"];
    for (vector, key) in vectors.iter().zip(keys.iter()) {
        gos_runtime::register_node(P_ID, *vector, node_spec(key)).expect("register_node");
    }

    // Ground truth: query each node individually, exactly the per-node path
    // a Subscribe-driven observer that only ever looked at one node at a
    // time would use.
    let individually_queried: Vec<GraphNodeSummary> = vectors
        .iter()
        .map(|v| gos_runtime::node_summary(*v).expect("node_summary for a just-registered vector"))
        .collect();

    // The bulk fast-path read: walk node_page to exhaustion.
    let mut collected: Vec<GraphNodeSummary> = Vec::new();
    let mut offset = 0usize;
    loop {
        let mut page = [GraphNodeSummary::EMPTY; 4];
        let (total, returned) = gos_runtime::node_page::<4>(offset, &mut page);
        collected.extend_from_slice(&page[..returned]);
        offset += returned;
        if offset >= total || returned == 0 {
            break;
        }
    }

    // Every individually-queried node must appear, verbatim, in the bulk
    // snapshot -- the snapshot is a projection, not a second data source.
    // (GraphNodeSummary doesn't derive PartialEq, so compare the fields
    // that identify and describe the node explicitly.)
    for expected in &individually_queried {
        let found = collected.iter().find(|got| got.node_id == expected.node_id);
        let got = found.unwrap_or_else(|| {
            panic!(
                "node_page snapshot is missing the individually-queried record for {:?}",
                expected.local_node_key
            )
        });
        assert_eq!(got.vector, expected.vector);
        assert_eq!(got.local_node_key, expected.local_node_key);
        assert_eq!(got.node_type, expected.node_type);
        assert_eq!(got.lifecycle, expected.lifecycle);
        assert_eq!(got.entry_policy, expected.entry_policy);
        assert_eq!(got.executor_id, expected.executor_id);
    }

    // And no *extra* records leaked in beyond what individual queries can
    // account for among our harness-registered nodes.
    let our_ids: Vec<_> = individually_queried.iter().map(|n| n.node_id).collect();
    let extra_of_ours: usize = collected
        .iter()
        .filter(|got| our_ids.contains(&got.node_id))
        .count();
    assert_eq!(
        extra_of_ours,
        individually_queried.len(),
        "node_page must return exactly one entry per registered node, not duplicates or ghosts"
    );
}

#[test]
fn edge_page_snapshot_matches_individually_registered_edges() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    gos_runtime::reset();

    let va = VectorAddress::new(0xF1, 0, 0, 0);
    let vb = VectorAddress::new(0xF1, 0, 0, 1);
    let vc = VectorAddress::new(0xF1, 0, 0, 2);
    gos_runtime::register_node(P_ID, va, node_spec("fp.e.a")).expect("register a");
    gos_runtime::register_node(P_ID, vb, node_spec("fp.e.b")).expect("register b");
    gos_runtime::register_node(P_ID, vc, node_spec("fp.e.c")).expect("register c");

    let e1 = edge_spec(va, vb, "fp.edge.ab");
    let e2 = edge_spec(vb, vc, "fp.edge.bc");
    gos_runtime::register_edge(e1).expect("register_edge ab");
    gos_runtime::register_edge(e2).expect("register_edge bc");

    // Ground truth: the per-edge lookup path (what a Subscribe delivery to
    // a->b / b->c specifically would have carried).
    let ab = gos_runtime::edge_id_for_vector(gos_protocol::derive_edge_vector(e1.edge_id))
        .expect("edge ab must resolve");
    let bc = gos_runtime::edge_id_for_vector(gos_protocol::derive_edge_vector(e2.edge_id))
        .expect("edge bc must resolve");
    assert_eq!(ab, e1.edge_id);
    assert_eq!(bc, e2.edge_id);

    // The bulk fast-path read.
    let mut collected: Vec<GraphEdgeSummary> = Vec::new();
    let mut offset = 0usize;
    loop {
        let mut page = [GraphEdgeSummary::EMPTY; 4];
        let (total, returned) = gos_runtime::edge_page::<4>(offset, &mut page);
        collected.extend_from_slice(&page[..returned]);
        offset += returned;
        if offset >= total || returned == 0 {
            break;
        }
    }

    assert!(
        collected.iter().any(|e| e.edge_id == e1.edge_id),
        "edge_page snapshot must contain the individually-registered a->b edge"
    );
    assert!(
        collected.iter().any(|e| e.edge_id == e2.edge_id),
        "edge_page snapshot must contain the individually-registered b->c edge"
    );
    assert_eq!(
        collected.iter().filter(|e| e.edge_id == e1.edge_id || e.edge_id == e2.edge_id).count(),
        2,
        "edge_page must return exactly one entry per registered edge, not duplicates"
    );
}
