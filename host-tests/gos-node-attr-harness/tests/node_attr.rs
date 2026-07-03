// gos-node-attr-harness — V2.55 per-node u32 attribute storage
//
// Verifies `gos_runtime::node_attr_set` / `gos_runtime::node_attr_get` —
// store and retrieve arbitrary u32 scalars (palette colors, flags, counters)
// keyed by node VectorAddress without touching graph epoch or structural state.
//
// Algorithm summary:
//   node_attr_set(vec, val):
//     1. Resolve vec → NodeId (NodeNotFound if absent)
//     2. Linear scan node_props_u32 for existing entry; overwrite if found
//     3. Else claim first free slot (PropTableFull if all MAX_NODE_PROPS_U32 used)
//     4. Return Ok(())
//
//   node_attr_get(vec):
//     1. Resolve vec → NodeId; return None if absent
//     2. Linear scan node_props_u32 for matching entry
//     3. Return Some(val) or None
//
// Key invariants:
//   Unknown vector (set)    → Err(NodeNotFound)
//   Unknown vector (get)    → None
//   Set then get            → Some(val) matches
//   Overwrite               → latest val is returned
//   Max u32 (0xFFFF_FFFF)  → stored and retrieved exactly
//   Zero value (0x0000_0000)→ stored and retrieved exactly
//   Multiple nodes          → attrs are independent per node
//   Table full              → Err(PropTableFull)
//   Graph epoch             → NOT bumped by attr set/get
//   Node lifecycle          → NOT changed by attr set/get
//
// Test matrix:
//  1.  Unknown vector, empty graph: node_attr_get → None
//  2.  Unknown vector, empty graph: node_attr_set → Err(NodeNotFound)
//  3.  Known node: node_attr_get before set → None
//  4.  node_attr_set → Ok, then node_attr_get → Some(val)
//  5.  Overwrite: second set replaces first value
//  6.  Max u32 (0xFFFF_FFFF) stored and retrieved exactly
//  7.  Zero value (0x0000_0000) stored and retrieved exactly
//  8.  Multiple nodes have independent attributes
//  9.  node_attr_set does NOT bump graph_epoch
// 10.  Table full (MAX_NODE_PROPS_U32 nodes) → Err(PropTableFull)

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const NA_PLUGIN: PluginId   = PluginId::from_ascii("KL_NATTR_00");
const NA_EXEC:   ExecutorId = ExecutorId::from_ascii("nattr.exec0");

const NA_VEC_X: VectorAddress = VectorAddress::new(29, 9, 9, 9); // never registered

fn na_vec(minor: u8) -> VectorAddress { VectorAddress::new(29, 1, minor as u16, 0) }
fn na_key(minor: u8) -> &'static str {
    match minor {
        1  => "na.node.01",
        2  => "na.node.02",
        3  => "na.node.03",
        4  => "na.node.04",
        5  => "na.node.05",
        6  => "na.node.06",
        7  => "na.node.07",
        8  => "na.node.08",
        9  => "na.node.09",
        10 => "na.node.10",
        11 => "na.node.11",
        12 => "na.node.12",
        13 => "na.node.13",
        14 => "na.node.14",
        15 => "na.node.15",
        16 => "na.node.16",
        17 => "na.node.17",
        18 => "na.node.18",
        19 => "na.node.19",
        20 => "na.node.20",
        21 => "na.node.21",
        22 => "na.node.22",
        23 => "na.node.23",
        24 => "na.node.24",
        25 => "na.node.25",
        26 => "na.node.26",
        27 => "na.node.27",
        28 => "na.node.28",
        29 => "na.node.29",
        30 => "na.node.30",
        31 => "na.node.31",
        32 => "na.node.32",
        33 => "na.node.33",
        _  => "na.node.xx",
    }
}

const NA_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    NA_PLUGIN,
    name:         "kl-node-attr-harness",
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
        node_id:           derive_node_id(NA_PLUGIN, key),
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       NA_EXEC,
        state_schema_hash: minor as u64,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(NA_MANIFEST).unwrap();
}

fn add_node(minor: u8) {
    let key = na_key(minor);
    let spec = node_spec(key, minor);
    gos_runtime::register_node(NA_PLUGIN, na_vec(minor), spec).unwrap();
}

// ── 1. Unknown vector, empty graph: node_attr_get → None ─────────────────────

#[test]
fn empty_graph_attr_get_returns_none() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    assert!(
        gos_runtime::node_attr_get(NA_VEC_X).is_none(),
        "empty graph: attr_get must return None for unknown vector"
    );
}

// ── 2. Unknown vector, empty graph: node_attr_set → Err(NodeNotFound) ────────

#[test]
fn empty_graph_attr_set_returns_err_node_not_found() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let err = gos_runtime::node_attr_set(NA_VEC_X, 0xDEAD_BEEF);
    assert!(
        matches!(err, Err(gos_runtime::RuntimeError::NodeNotFound)),
        "empty graph: attr_set must return NodeNotFound, got {:?}", err
    );
}

// ── 3. Known node: attr_get before any set → None ────────────────────────────

#[test]
fn known_node_attr_get_before_set_returns_none() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(1);
    assert!(
        gos_runtime::node_attr_get(na_vec(1)).is_none(),
        "known node with no attr set: must return None"
    );
}

// ── 4. node_attr_set → Ok, then node_attr_get → Some(val) ───────────────────

#[test]
fn attr_set_then_get_returns_value() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(2);
    let val = 0x00_DB_1C_21u32; // RED from PAL_U32
    gos_runtime::node_attr_set(na_vec(2), val).expect("attr_set must succeed");
    assert_eq!(
        gos_runtime::node_attr_get(na_vec(2)),
        Some(val),
        "attr_get must return the value that was set"
    );
}

// ── 5. Overwrite: second set replaces first value ─────────────────────────────

#[test]
fn attr_set_overwrites_existing_value() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(3);
    gos_runtime::node_attr_set(na_vec(3), 0x1111_1111).expect("first set");
    gos_runtime::node_attr_set(na_vec(3), 0x2222_2222).expect("second set");
    assert_eq!(
        gos_runtime::node_attr_get(na_vec(3)),
        Some(0x2222_2222),
        "second attr_set must overwrite the first"
    );
}

// ── 6. Max u32 (0xFFFF_FFFF) stored and retrieved exactly ────────────────────

#[test]
fn attr_set_max_u32_roundtrips() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(4);
    gos_runtime::node_attr_set(na_vec(4), u32::MAX).expect("set max u32");
    assert_eq!(
        gos_runtime::node_attr_get(na_vec(4)),
        Some(u32::MAX),
        "max u32 (0xFFFF_FFFF) must round-trip exactly"
    );
}

// ── 7. Zero value (0x0000_0000) stored and retrieved exactly ─────────────────

#[test]
fn attr_set_zero_roundtrips() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(5);
    gos_runtime::node_attr_set(na_vec(5), 0u32).expect("set zero");
    // After setting zero, the entry exists but val is 0.
    assert_eq!(
        gos_runtime::node_attr_get(na_vec(5)),
        Some(0u32),
        "zero value must round-trip exactly (distinct from 'no attr set')"
    );
}

// ── 8. Multiple nodes have independent attributes ─────────────────────────────

#[test]
fn multiple_nodes_have_independent_attrs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(6);
    add_node(7);
    add_node(8);
    gos_runtime::node_attr_set(na_vec(6), 0xAAAA_AAAA).expect("set A");
    gos_runtime::node_attr_set(na_vec(7), 0xBBBB_BBBB).expect("set B");
    gos_runtime::node_attr_set(na_vec(8), 0xCCCC_CCCC).expect("set C");
    assert_eq!(gos_runtime::node_attr_get(na_vec(6)), Some(0xAAAA_AAAA), "node 6");
    assert_eq!(gos_runtime::node_attr_get(na_vec(7)), Some(0xBBBB_BBBB), "node 7");
    assert_eq!(gos_runtime::node_attr_get(na_vec(8)), Some(0xCCCC_CCCC), "node 8");
}

// ── 9. node_attr_set does NOT bump graph_epoch ───────────────────────────────

#[test]
fn attr_set_does_not_bump_epoch() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(9);
    let epoch_before = gos_runtime::graph_epoch();
    gos_runtime::node_attr_set(na_vec(9), 0x1234_5678).expect("set attr");
    let epoch_after = gos_runtime::graph_epoch();
    assert_eq!(epoch_after, epoch_before, "attr_set must not advance graph_epoch");
}

// ── 10. Table full → Err(PropTableFull) ──────────────────────────────────────

#[test]
fn attr_table_full_returns_prop_table_full() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Fill all MAX_NODE_PROPS_U32 = 32 slots with distinct nodes.
    let cap = gos_runtime::MAX_NODE_PROPS_U32;
    for minor in 1..=(cap as u8) {
        add_node(minor);
        gos_runtime::node_attr_set(na_vec(minor), minor as u32)
            .unwrap_or_else(|e| panic!("slot {minor} should succeed: {:?}", e));
    }
    // One extra node beyond capacity.
    let extra_minor = (cap as u8) + 1;
    add_node(extra_minor);
    let result = gos_runtime::node_attr_set(na_vec(extra_minor), 0xFF);
    assert!(
        matches!(result, Err(gos_runtime::RuntimeError::PropTableFull)),
        "table full: must return PropTableFull, got {:?}", result
    );
}
