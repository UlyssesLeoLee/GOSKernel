// gos-node-attr-list-harness — V2.58 node_attr_list diagnostic enumeration
//
// Verifies `gos_runtime::node_attr_list` — enumerate all nodes that have
// a u32 attribute set, returning (VectorAddress, u32) pairs in table order.
//
// Algorithm summary:
//   node_attr_list<N>(out_vec, out_val):
//     For each slot in node_props_u32:
//       skip if NodeId::ZERO (free slot)
//       resolve NodeId → VectorAddress via node_vector()
//       write to out_vec[count] / out_val[count]
//       count += 1  (stops at N)
//     return count
//
// Test matrix:
//  1.  empty graph: node_attr_list returns 0
//  2.  one node with attr set: list returns 1 entry, correct vec + val
//  3.  two nodes: both appear in list
//  4.  node without attr: does NOT appear in list
//  5.  order: attrs appear in insertion order (table scan order)
//  6.  count-only: N=0 returns 0 without writing anything
//  7.  N smaller than count: only N entries returned
//  8.  list + overwrite: overwritten attr reflects new value
//  9.  reset: list returns 0 after reset
// 10.  table-full: all 32 slots appear in list

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const AL_PLUGIN: PluginId   = PluginId::from_ascii("KL_ATRL_000");
const AL_EXEC:   ExecutorId = ExecutorId::from_ascii("atrl.exec00");

// L4=34 (unique to this harness)
fn al_vec(minor: u8) -> VectorAddress { VectorAddress::new(34, 1, minor as u16, 0) }

fn al_key(minor: u8) -> &'static str {
    match minor {
        1  => "al.node.01",  2  => "al.node.02",  3  => "al.node.03",
        4  => "al.node.04",  5  => "al.node.05",  6  => "al.node.06",
        7  => "al.node.07",  8  => "al.node.08",  9  => "al.node.09",
        10 => "al.node.10",  11 => "al.node.11",  12 => "al.node.12",
        13 => "al.node.13",  14 => "al.node.14",  15 => "al.node.15",
        16 => "al.node.16",  17 => "al.node.17",  18 => "al.node.18",
        19 => "al.node.19",  20 => "al.node.20",  21 => "al.node.21",
        22 => "al.node.22",  23 => "al.node.23",  24 => "al.node.24",
        25 => "al.node.25",  26 => "al.node.26",  27 => "al.node.27",
        28 => "al.node.28",  29 => "al.node.29",  30 => "al.node.30",
        31 => "al.node.31",  32 => "al.node.32",  _  => "al.node.xx",
    }
}

const AL_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    AL_PLUGIN,
    name:         "kl-node-attr-list-harness",
    version:      1,
    depends_on:   &[],
    permissions:  &[],
    exports:      &[],
    imports:      &[],
    nodes:        &[],
    edges:        &[],
    signature:    None,
    policy_hash:  [0u8; 16],
};

fn node_spec(key: &'static str, minor: u8) -> NodeSpec {
    NodeSpec {
        node_id:           derive_node_id(AL_PLUGIN, key),
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       AL_EXEC,
        state_schema_hash: minor as u64,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(AL_MANIFEST).unwrap();
}

fn add_node(minor: u8) {
    let key  = al_key(minor);
    let spec = node_spec(key, minor);
    gos_runtime::register_node(AL_PLUGIN, al_vec(minor), spec).unwrap();
}

const CAP: usize = gos_runtime::MAX_NODE_PROPS_U32;

// ── 1. empty graph: node_attr_list returns 0 ─────────────────────────────────

#[test]
fn empty_graph_attr_list_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 0, "empty graph: node_attr_list must return 0");
}

// ── 2. one node with attr: list returns 1 entry ──────────────────────────────

#[test]
fn one_node_with_attr_appears_in_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(1);
    gos_runtime::node_attr_set(al_vec(1), 0xABCD_1234).unwrap();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 1, "one attr set: must list 1 entry");
    assert_eq!(vecs[0], al_vec(1), "listed vector must match the registered node");
    assert_eq!(vals[0], 0xABCD_1234, "listed value must match the set attr");
}

// ── 3. two nodes both appear in list ─────────────────────────────────────────

#[test]
fn two_nodes_both_appear_in_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(1);
    add_node(2);
    gos_runtime::node_attr_set(al_vec(1), 0x1111_1111).unwrap();
    gos_runtime::node_attr_set(al_vec(2), 0x2222_2222).unwrap();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 2, "two attrs set: must list 2 entries");
    // Both vecs present (order may vary — check set membership).
    let found_1 = (0..count).any(|i| vecs[i] == al_vec(1) && vals[i] == 0x1111_1111);
    let found_2 = (0..count).any(|i| vecs[i] == al_vec(2) && vals[i] == 0x2222_2222);
    assert!(found_1, "node 1 must appear in list");
    assert!(found_2, "node 2 must appear in list");
}

// ── 4. node without attr does NOT appear in list ──────────────────────────────

#[test]
fn node_without_attr_not_in_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(1);
    add_node(2); // no attr set on node 2
    gos_runtime::node_attr_set(al_vec(1), 0xDEAD_BEEF).unwrap();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 1, "only node 1 has attr: list must return 1");
    assert!(
        !(0..count).any(|i| vecs[i] == al_vec(2)),
        "node 2 (no attr) must NOT appear in list"
    );
}

// ── 5. insertion order preserved ─────────────────────────────────────────────

#[test]
fn list_preserves_insertion_order() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Insert nodes in a specific order.
    add_node(3);
    add_node(1);
    add_node(2);
    gos_runtime::node_attr_set(al_vec(3), 0x3333_3333).unwrap();
    gos_runtime::node_attr_set(al_vec(1), 0x1111_1111).unwrap();
    gos_runtime::node_attr_set(al_vec(2), 0x2222_2222).unwrap();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 3, "three attrs: must list 3");
    // node_props_u32 is a linear table — attrs appear in the order they were registered.
    assert_eq!(vecs[0], al_vec(3), "first inserted: node 3");
    assert_eq!(vals[0], 0x3333_3333);
    assert_eq!(vecs[1], al_vec(1), "second inserted: node 1");
    assert_eq!(vals[1], 0x1111_1111);
    assert_eq!(vecs[2], al_vec(2), "third inserted: node 2");
    assert_eq!(vals[2], 0x2222_2222);
}

// ── 6. N=0 returns 0 without writing ─────────────────────────────────────────

#[test]
fn zero_capacity_returns_zero_entries() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(1);
    gos_runtime::node_attr_set(al_vec(1), 0xFFFF_FFFF).unwrap();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); 0];
    let mut vals = [0u32; 0];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 0, "N=0: must return 0 even with attrs present");
}

// ── 7. N smaller than count: only N entries returned ─────────────────────────

#[test]
fn small_n_caps_returned_entries() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    for minor in 1u8..=5 {
        add_node(minor);
        gos_runtime::node_attr_set(al_vec(minor), minor as u32 * 0x1111).unwrap();
    }
    // Request only 3 entries.
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); 3];
    let mut vals = [0u32; 3];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 3, "N=3 with 5 attrs: must return exactly 3");
}

// ── 8. overwrite: list shows new value ───────────────────────────────────────

#[test]
fn list_shows_overwritten_attr_value() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(1);
    gos_runtime::node_attr_set(al_vec(1), 0xAAAA_AAAA).unwrap();
    gos_runtime::node_attr_set(al_vec(1), 0xBBBB_BBBB).unwrap(); // overwrite
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 1, "overwrite must not add a second slot");
    assert_eq!(vals[0], 0xBBBB_BBBB, "list must show the latest (overwritten) value");
}

// ── 9. reset clears attrs: list returns 0 ────────────────────────────────────

#[test]
fn reset_clears_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(1);
    add_node(2);
    gos_runtime::node_attr_set(al_vec(1), 0x1111_1111).unwrap();
    gos_runtime::node_attr_set(al_vec(2), 0x2222_2222).unwrap();
    reset(); // wipes everything
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, 0, "after reset: node_attr_list must return 0");
}

// ── 10. table-full: all 32 entries appear in list ────────────────────────────

#[test]
fn full_table_all_entries_appear_in_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    let cap = gos_runtime::MAX_NODE_PROPS_U32 as u8;
    for minor in 1..=cap {
        add_node(minor);
        gos_runtime::node_attr_set(al_vec(minor), minor as u32 * 0x0100_0001)
            .unwrap_or_else(|e| panic!("slot {minor} should succeed: {:?}", e));
    }
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); CAP];
    let mut vals = [0u32; CAP];
    let count = gos_runtime::node_attr_list(&mut vecs, &mut vals);
    assert_eq!(count, CAP, "full table: list must return all {} entries", CAP);
    // Verify every registered node appears exactly once.
    for minor in 1..=cap {
        let expected_val = minor as u32 * 0x0100_0001;
        let found = (0..count).any(|i| vecs[i] == al_vec(minor) && vals[i] == expected_val);
        assert!(found, "node {minor} must appear in full-table list");
    }
}
