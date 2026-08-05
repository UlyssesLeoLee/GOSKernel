// V2.5d — host harness for `gos_runtime::create_provisional_node`
// (ADR-005 §五 step 1: "give gos_runtime node records a provisional/promoted
// distinction").
//
// Finding: no new lifecycle/binding state is needed. `register_node` already
// defaults every new node to `NodeBinding::Unbound` + `NodeInstanceId::ZERO`
// (no claim/quota/instance) — exactly ADR-005 option A's "provisional"
// definition. What was actually missing was an allocation scheme for fresh
// `NodeId`/`VectorAddress` pairs (boot nodes use manifest-derived, *stable*
// ids; a Cypher `CREATE` needs a *fresh* one each time) plus a shared
// `NodeSpec` template usable for any dynamically-created node.
//
// This harness proves the resulting primitive end-to-end on the host:
// each call returns a distinct, tagged `NodeId`; the node is immediately
// visible via `node_page` (the same read path V2.5c's `vk_auto_refresh`
// walks); `graph_epoch` bumps (the signal `vk_auto_refresh` polls); and the
// registered record carries exactly the "unbound, no executor, manual
// entry" shape ADR-005 §四 already proved is render- and
// capability-transparent.
//
// ```cypher
// CREATE
//   (f:File {name: "provisional_node.rs", type: "file", language: "rust"}),
//   (cpn:Function {name: "create_provisional_node", type: "function", visibility: "pub"}),
//   (ipn:Function {name: "is_provisional_node_id", type: "function", visibility: "pub"}),
//   (t1:Function {name: "create_provisional_node_is_visible_unbound_and_renderable", type: "function"}),
//   (t2:Function {name: "create_provisional_node_allocates_distinct_ids_and_vectors", type: "function"}),
//   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2),
//   (t1)-[:USES]->(cpn), (t1)-[:USES]->(ipn), (t2)-[:USES]->(cpn);
// ```

use std::sync::Mutex;

use gos_protocol::{
    EntryPolicy, ExecutorId, GraphNodeSummary, NodeId, NodeLifecycle, RuntimeNodeType,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Walk `node_page` to find the summary for `node_id` (mirrors how
/// `k-vk-host::render_live_graph` enumerates the live graph — V2.5c).
fn find_node(node_id: NodeId) -> Option<GraphNodeSummary> {
    let mut offset = 0usize;
    loop {
        let mut page = [GraphNodeSummary::EMPTY; 8];
        let (total, returned) = gos_runtime::node_page::<8>(offset, &mut page);
        if let Some(found) = page.iter().take(returned).find(|s| s.node_id == node_id) {
            return Some(*found);
        }
        offset += returned;
        if offset >= total || returned == 0 {
            return None;
        }
    }
}

#[test]
fn create_provisional_node_is_visible_unbound_and_renderable() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    gos_runtime::reset();

    let epoch0 = gos_runtime::graph_epoch();
    let (id, vector) = gos_runtime::create_provisional_node(
        RuntimeNodeType::Vector,
        EntryPolicy::Manual,
        ExecutorId::ZERO,
    )
    .expect("create provisional node");

    assert!(
        gos_runtime::is_provisional_node_id(id),
        "allocated id must carry the provisional namespace tag"
    );
    assert_eq!(
        gos_runtime::graph_epoch(),
        epoch0 + 1,
        "create_provisional_node is a structural mutation (V2.5c vk_auto_refresh polls this)"
    );

    let summary = find_node(id).expect("provisional node visible via node_page");
    assert_eq!(summary.node_type, RuntimeNodeType::Vector);
    assert_eq!(summary.lifecycle, NodeLifecycle::Allocated);
    assert_eq!(summary.entry_policy, EntryPolicy::Manual);
    assert_eq!(summary.executor_id, ExecutorId::ZERO);
    assert_eq!(summary.plugin_id, gos_runtime::PROVISIONAL_PLUGIN_ID);
    assert_eq!(summary.local_node_key, "cypher.provisional");
    assert_eq!(summary.vector.as_u64(), vector.as_u64(), "returned vector matches the registered one");
}

#[test]
fn create_provisional_node_allocates_distinct_ids_and_vectors() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    gos_runtime::reset();

    let epoch0 = gos_runtime::graph_epoch();
    let (a, va) = gos_runtime::create_provisional_node(
        RuntimeNodeType::Vector,
        EntryPolicy::Manual,
        ExecutorId::ZERO,
    )
    .expect("create a");
    let (b, vb) = gos_runtime::create_provisional_node(
        RuntimeNodeType::Vector,
        EntryPolicy::Manual,
        ExecutorId::ZERO,
    )
    .expect("create b");

    assert_ne!(a, b, "each call allocates a fresh NodeId");
    assert!(gos_runtime::is_provisional_node_id(a));
    assert!(gos_runtime::is_provisional_node_id(b));
    assert_eq!(gos_runtime::graph_epoch(), epoch0 + 2, "two creates bump epoch twice");

    let sa = find_node(a).expect("a visible");
    let sb = find_node(b).expect("b visible");
    assert_eq!(sa.vector.as_u64(), va.as_u64());
    assert_eq!(sb.vector.as_u64(), vb.as_u64());
    assert_ne!(sa.vector.as_u64(), sb.vector.as_u64(), "distinct nodes get distinct vectors");
}

// V2.5e (ADR-005 §五 step 2) — `gos-cypher-mut::CypherMutation::CreateNode`
// dispatches through `RuntimeDispatcher::create_node` to the same
// `create_provisional_node` primitive, exercised end-to-end against the live
// runtime (mirrors mutation_visibility.rs's "real dispatcher" checks).
//
// ```cypher
// CREATE
//   (t3:Function {name: "create_node_mutation_dispatches_to_provisional_node", type: "function"}),
//   (t3)-[:USES]->(cpn);
// ```
#[test]
fn create_node_mutation_dispatches_to_provisional_node() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    gos_runtime::reset();

    use gos_cypher_mut::{apply_mutation, pre_validate, CypherMutation, MutationDispatcher};

    let create_mutation = CypherMutation::CreateNode {
        node_type: RuntimeNodeType::Vector,
        entry_policy: EntryPolicy::Manual,
        executor_id: ExecutorId::ZERO,
    };
    pre_validate(&create_mutation).expect("CreateNode is in the receptive subset");

    let epoch0 = gos_runtime::graph_epoch();
    let mut d = gos_runtime::RuntimeDispatcher;
    let id = apply_mutation(&mut d, create_mutation)
        .expect("create applies")
        .expect("CreateNode returns the allocated NodeId");

    assert!(gos_runtime::is_provisional_node_id(id));
    assert_eq!(gos_runtime::graph_epoch(), epoch0 + 1);
    assert!(find_node(id).is_some(), "node from apply_mutation is visible via node_page");

    // The trait method directly, too — a second, distinct provisional node.
    let id2 = d
        .create_node(RuntimeNodeType::Vector, EntryPolicy::Manual, ExecutorId::ZERO)
        .expect("dispatcher create_node");
    assert_ne!(id, id2);
    assert!(find_node(id2).is_some());
    assert_eq!(gos_runtime::graph_epoch(), epoch0 + 2);
}
