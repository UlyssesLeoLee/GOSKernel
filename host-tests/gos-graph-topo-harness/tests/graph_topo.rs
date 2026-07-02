// gos-graph-topo-harness — V2.17 L4-domain topology API tests
//
// Verifies node_count_for_l4() and node_page_l4() added in V2.17,
// underpinning the new `graph topo` / `graph topo <L4>` shell commands.
//
//  1. node_count_for_l4 returns 0 on empty runtime.
//  2. Single node appears in its correct l4 domain.
//  3. Node is not counted in a wrong l4 domain.
//  4. Two nodes in the same l4 are both counted.
//  5. Two nodes in different l4s each count in their own domain.
//  6. node_page_l4 returns only nodes matching the l4 filter.
//  7. node_page_l4 on an empty domain returns (0, 0).
//  8. node_page_l4 total matches node_count_for_l4.
//  9. node_page_l4 offset skips the correct number of entries.
// 10. node_page_l4 caps filled at PAGE while total reflects all matches.

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    GraphNodeSummary, NodeId, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Shared fixtures ───────────────────────────────────────────────────────────

const GT_PLUGIN: PluginId = PluginId::from_ascii("GT_TOPO");
const GT_EXEC:   ExecutorId = ExecutorId::from_ascii("gt.exec");

const GT_KEY_A: &str = "gt.alpha";
const GT_KEY_B: &str = "gt.beta";
const GT_KEY_C: &str = "gt.gamma";
const GT_KEY_D: &str = "gt.delta";
const GT_KEY_E: &str = "gt.epsilon";

const GT_ID_A: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_A);
const GT_ID_B: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_B);
const GT_ID_C: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_C);
const GT_ID_D: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_D);
const GT_ID_E: NodeId = derive_node_id(GT_PLUGIN, GT_KEY_E);

// l4 domain values used in tests
const L4_5:   u8 = 5;
const L4_6:   u8 = 6;
const L4_3:   u8 = 3;
const L4_4:   u8 = 4;
const L4_8:   u8 = 8;
const L4_99:  u8 = 99;

const fn gt_spec(key: &'static str, node_id: NodeId) -> NodeSpec {
    NodeSpec {
        node_id,
        local_node_key: key,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: GT_EXEC,
        state_schema_hash: 0xAB00,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    }
}

const GT_SPEC_A: NodeSpec = gt_spec(GT_KEY_A, GT_ID_A);
const GT_SPEC_B: NodeSpec = gt_spec(GT_KEY_B, GT_ID_B);
const GT_SPEC_C: NodeSpec = gt_spec(GT_KEY_C, GT_ID_C);
const GT_SPEC_D: NodeSpec = gt_spec(GT_KEY_D, GT_ID_D);
const GT_SPEC_E: NodeSpec = gt_spec(GT_KEY_E, GT_ID_E);

const GT_MANIFEST: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: GT_PLUGIN,
    name: "GT_TOPO",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[GT_SPEC_A, GT_SPEC_B, GT_SPEC_C, GT_SPEC_D, GT_SPEC_E],
    edges: &[],
    signature: None,
    policy_hash: [0; 16],
};

// ── Test 1: empty runtime → node_count_for_l4 == 0 ──────────────────────────

#[test]
fn empty_graph_count_for_l4_returns_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();

    assert_eq!(gos_runtime::node_count_for_l4(L4_5), 0,
        "empty runtime must report 0 nodes for any l4 domain");
}

// ── Test 2: single node counted in the correct l4 domain ─────────────────────

#[test]
fn single_node_counted_in_correct_l4_domain() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    let vec = VectorAddress::new(L4_5, 1, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec, GT_SPEC_A);

    assert_eq!(gos_runtime::node_count_for_l4(L4_5), 1,
        "node registered at l4=5 must be counted in domain 5");
}

// ── Test 3: node not counted in wrong l4 domain ──────────────────────────────

#[test]
fn node_not_counted_in_wrong_l4_domain() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    let vec = VectorAddress::new(L4_5, 1, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec, GT_SPEC_A);

    assert_eq!(gos_runtime::node_count_for_l4(L4_6), 0,
        "node at l4=5 must NOT be counted in l4=6");
}

// ── Test 4: two nodes with same l4 both counted ───────────────────────────────

#[test]
fn two_nodes_same_l4_counted_correctly() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    let vec_a = VectorAddress::new(L4_3, 1, 0, 0);
    let vec_b = VectorAddress::new(L4_3, 2, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_a, GT_SPEC_A);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_b, GT_SPEC_B);

    assert_eq!(gos_runtime::node_count_for_l4(L4_3), 2,
        "two nodes at l4=3 must both be counted in domain 3");
}

// ── Test 5: two nodes in different l4 domains each counted separately ─────────

#[test]
fn two_nodes_different_l4_each_counted_separately() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    let vec_a = VectorAddress::new(L4_5, 1, 0, 0);
    let vec_b = VectorAddress::new(L4_6, 1, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_a, GT_SPEC_A);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_b, GT_SPEC_B);

    assert_eq!(gos_runtime::node_count_for_l4(L4_5), 1,
        "l4=5 must count only its own node");
    assert_eq!(gos_runtime::node_count_for_l4(L4_6), 1,
        "l4=6 must count only its own node");
}

// ── Test 6: node_page_l4 returns only nodes matching the l4 filter ───────────

#[test]
fn node_page_l4_returns_only_matching_domain() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    // Register one node in l4=5 and one in l4=6.
    let vec_5 = VectorAddress::new(L4_5, 1, 0, 0);
    let vec_6 = VectorAddress::new(L4_6, 1, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_5, GT_SPEC_A);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_6, GT_SPEC_B);

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::node_page_l4::<8>(L4_5, 0, &mut out);

    assert_eq!(total, 1, "node_page_l4(l4=5) total must be 1");
    assert_eq!(filled, 1, "node_page_l4(l4=5) filled must be 1");
    assert_eq!(out[0].vector, vec_5, "returned node must be the l4=5 node");
    assert_ne!(out[0].vector, vec_6, "l4=6 node must not appear in l4=5 page");
}

// ── Test 7: node_page_l4 on empty domain returns (0, 0) ─────────────────────

#[test]
fn node_page_l4_empty_domain_returns_zero() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    let vec = VectorAddress::new(L4_5, 1, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec, GT_SPEC_A);

    let mut out = [GraphNodeSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::node_page_l4::<8>(L4_99, 0, &mut out);

    assert_eq!(total, 0, "node_page_l4 on empty domain must return total=0");
    assert_eq!(filled, 0, "node_page_l4 on empty domain must return filled=0");
}

// ── Test 8: node_page_l4 total matches node_count_for_l4 ─────────────────────

#[test]
fn node_page_l4_total_matches_count_api() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    let vec_a = VectorAddress::new(L4_4, 1, 0, 0);
    let vec_b = VectorAddress::new(L4_4, 2, 0, 0);
    let vec_c = VectorAddress::new(L4_4, 3, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_a, GT_SPEC_A);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_b, GT_SPEC_B);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_c, GT_SPEC_C);

    let count  = gos_runtime::node_count_for_l4(L4_4);
    let mut out = [GraphNodeSummary::EMPTY; 16];
    let (total, _) = gos_runtime::node_page_l4::<16>(L4_4, 0, &mut out);

    assert_eq!(total, count,
        "node_page_l4 total ({}) must match node_count_for_l4 ({})", total, count);
}

// ── Test 9: node_page_l4 offset skips the correct entries ────────────────────

#[test]
fn node_page_l4_offset_skips_correctly() {
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    let vec_a = VectorAddress::new(L4_4, 1, 0, 0);
    let vec_b = VectorAddress::new(L4_4, 2, 0, 0);
    let vec_c = VectorAddress::new(L4_4, 3, 0, 0);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_a, GT_SPEC_A);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_b, GT_SPEC_B);
    let _ = gos_runtime::register_node(GT_PLUGIN, vec_c, GT_SPEC_C);

    let mut out = [GraphNodeSummary::EMPTY; 16];
    // offset=1 means skip the first entry, return the remaining 2.
    let (total, filled) = gos_runtime::node_page_l4::<16>(L4_4, 1, &mut out);

    assert_eq!(total, 3, "total must be 3 regardless of offset");
    assert_eq!(filled, 2, "filled must be 2 when offset=1 and 3 nodes exist");
}

// ── Test 10: node_page_l4 caps filled at PAGE while total reflects all ────────

#[test]
fn node_page_l4_caps_filled_at_page_size() {
    const PAGE: usize = 2;
    let _lock = TEST_LOCK.lock().unwrap();
    gos_runtime::reset();
    gos_runtime::discover_plugin(GT_MANIFEST).unwrap();

    // Register 5 nodes all in l4=8.
    let vecs = [
        VectorAddress::new(L4_8, 1, 0, 0),
        VectorAddress::new(L4_8, 2, 0, 0),
        VectorAddress::new(L4_8, 3, 0, 0),
        VectorAddress::new(L4_8, 4, 0, 0),
        VectorAddress::new(L4_8, 5, 0, 0),
    ];
    let specs = [GT_SPEC_A, GT_SPEC_B, GT_SPEC_C, GT_SPEC_D, GT_SPEC_E];
    for (v, s) in vecs.iter().zip(specs.iter()) {
        let _ = gos_runtime::register_node(GT_PLUGIN, *v, *s);
    }

    let mut out = [GraphNodeSummary::EMPTY; PAGE];
    let (total, filled) = gos_runtime::node_page_l4::<PAGE>(L4_8, 0, &mut out);

    assert_eq!(total, 5, "total must reflect all 5 nodes in l4=8; got {}", total);
    assert_eq!(filled, PAGE,
        "filled must be capped at PAGE={}; got {}", PAGE, filled);
}
