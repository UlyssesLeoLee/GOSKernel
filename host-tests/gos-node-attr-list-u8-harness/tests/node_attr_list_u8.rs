// gos-node-attr-list-u8-harness — V2.60 node u8 attribute list
//
// Verifies `gos_runtime::node_attr_list_u8` — enumerate all nodes with a u8
// attribute registered via `register_node_prop_u8`.
//
// The u8 property table (MAX_NODE_PROPS_U8 = 16 slots) stores reactive signal
// vals used by theme nodes (DISPLAY_THEME_WABI=0, DISPLAY_THEME_SHOJI=1).
// This list function is the u8 parallel to `node_attr_list` (V2.58 for u32).
//
// VectorAddress namespace: L4=36 (node-attr-list-u8 harness).
//
// Test matrix:
//  1.  Empty table: list returns 0
//  2.  Single entry: list returns 1, correct vector and val
//  3.  Two entries: both appear in list
//  4.  Update idempotency: re-registering same node overwrites val, list size stays same
//  5.  Order stable: entries appear in registration order (table order)
//  6.  Reset clears list: after reset, list returns 0
//  7.  MAX_NODE_PROPS_U8 entries fit: all 16 slots visible in list
//  8.  Overflow protection: 17th register_node_prop_u8 returns false, list still has 16
//  9.  Different vals (0..=255): val=0 and val=255 both appear correctly
// 10.  node_attr_list_u8 does NOT bump graph_epoch

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeId, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const U8L_PLUGIN: PluginId   = PluginId::from_ascii("KL_U8LST_00");
const U8L_EXEC:   ExecutorId = ExecutorId::from_ascii("u8lst.exec0");

// L4=36 (unique to this harness)
const U8L_VEC_A:  VectorAddress = VectorAddress::new(36, 1, 1, 0);
const U8L_VEC_B:  VectorAddress = VectorAddress::new(36, 1, 2, 0);
const U8L_VEC_C:  VectorAddress = VectorAddress::new(36, 1, 3, 0);

const U8L_ID_A:  NodeId = derive_node_id(U8L_PLUGIN, "u8l.alpha");
const U8L_ID_B:  NodeId = derive_node_id(U8L_PLUGIN, "u8l.beta");
const U8L_ID_C:  NodeId = derive_node_id(U8L_PLUGIN, "u8l.gamma");

const U8L_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    U8L_PLUGIN,
    name:         "kl-node-attr-list-u8-harness",
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

fn node_spec(key: &'static str, id: NodeId, schema: u64) -> NodeSpec {
    NodeSpec {
        node_id:           id,
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       U8L_EXEC,
        state_schema_hash: schema,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(U8L_MANIFEST).unwrap();
}

fn add_node(vec: VectorAddress, key: &'static str, id: NodeId, schema: u64) {
    gos_runtime::register_node(U8L_PLUGIN, vec, node_spec(key, id, schema)).unwrap();
}

// ── 1. Empty table: list returns 0 ───────────────────────────────────────────

#[test]
fn empty_table_list_returns_zero() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(count, 0, "empty table: list must return 0");
}

// ── 2. Single entry: list returns 1, correct vector and val ──────────────────

#[test]
fn single_entry_appears_in_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(U8L_VEC_A, "u8l.alpha", U8L_ID_A, 1);
    let ok = gos_runtime::register_node_prop_u8(U8L_ID_A, 42);
    assert!(ok, "register_node_prop_u8 must succeed");

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(count, 1, "list must return 1 entry");
    assert_eq!(vecs[0], U8L_VEC_A, "vector must match node's vector");
    assert_eq!(vals[0], 42, "val must be 42");
}

// ── 3. Two entries: both appear in list ──────────────────────────────────────

#[test]
fn two_entries_appear_in_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(U8L_VEC_A, "u8l.alpha", U8L_ID_A, 1);
    add_node(U8L_VEC_B, "u8l.beta",  U8L_ID_B, 2);
    gos_runtime::register_node_prop_u8(U8L_ID_A, 0);
    gos_runtime::register_node_prop_u8(U8L_ID_B, 1);

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(count, 2, "two entries registered: list must return 2");

    let found_a = (0..count).any(|i| vecs[i] == U8L_VEC_A && vals[i] == 0);
    let found_b = (0..count).any(|i| vecs[i] == U8L_VEC_B && vals[i] == 1);
    assert!(found_a, "entry for node A (val=0) must appear in list");
    assert!(found_b, "entry for node B (val=1) must appear in list");
}

// ── 4. Update idempotency: re-register same node, list size stays the same ───

#[test]
fn update_does_not_grow_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(U8L_VEC_A, "u8l.alpha", U8L_ID_A, 1);
    gos_runtime::register_node_prop_u8(U8L_ID_A, 7);
    gos_runtime::register_node_prop_u8(U8L_ID_A, 99); // overwrite

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(count, 1, "idempotent re-register must not grow the list");
    assert_eq!(vals[0], 99, "re-register must overwrite val to 99");
}

// ── 5. Order stable: entries appear in table order ────────────────────────────

#[test]
fn entries_appear_in_table_order() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(U8L_VEC_A, "u8l.alpha", U8L_ID_A, 1);
    add_node(U8L_VEC_B, "u8l.beta",  U8L_ID_B, 2);
    add_node(U8L_VEC_C, "u8l.gamma", U8L_ID_C, 3);
    gos_runtime::register_node_prop_u8(U8L_ID_A, 10);
    gos_runtime::register_node_prop_u8(U8L_ID_B, 20);
    gos_runtime::register_node_prop_u8(U8L_ID_C, 30);

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(count, 3, "three entries: count=3");
    // Table-order: A first, then B, then C (registration order)
    assert_eq!(vecs[0], U8L_VEC_A, "first entry must be A");
    assert_eq!(vals[0], 10,        "first val must be 10");
    assert_eq!(vecs[1], U8L_VEC_B, "second entry must be B");
    assert_eq!(vals[1], 20,        "second val must be 20");
    assert_eq!(vecs[2], U8L_VEC_C, "third entry must be C");
    assert_eq!(vals[2], 30,        "third val must be 30");
}

// ── 6. Reset clears list ──────────────────────────────────────────────────────

#[test]
fn reset_clears_u8_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(U8L_VEC_A, "u8l.alpha", U8L_ID_A, 1);
    gos_runtime::register_node_prop_u8(U8L_ID_A, 5);

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let before = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(before, 1, "before reset: 1 entry");

    reset();
    let after = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(after, 0, "after reset: list must be empty");
}

// ── 7. MAX_NODE_PROPS_U8 entries all fit in list ─────────────────────────────

#[test]
fn full_table_all_entries_appear_in_list() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();

    // Register MAX_NODE_PROPS_U8 distinct nodes, each with a unique val.
    // Use derive_node_id with minor index to get distinct NodeIds without
    // needing explicit VectorAddress registration for every slot.
    for minor in 0u16..gos_runtime::MAX_NODE_PROPS_U8 as u16 {
        let vec = VectorAddress::new(36, 2, minor, 0);
        let mut key_buf = [0u8; 16];
        key_buf[0] = b'u'; key_buf[1] = b'8'; key_buf[2] = b'l'; key_buf[3] = b'.';
        key_buf[4] = b's'; key_buf[5] = b'l'; key_buf[6] = b'o'; key_buf[7] = b't';
        key_buf[8] = b'.';
        key_buf[9]  = b'0' + (minor / 10) as u8;
        key_buf[10] = b'0' + (minor % 10) as u8;
        let key_str = core::str::from_utf8(&key_buf[..11]).unwrap();
        let node_id = derive_node_id(U8L_PLUGIN, key_str);
        let spec = NodeSpec {
            node_id,
            local_node_key: "u8l.slot",
            node_type:         RuntimeNodeType::Service,
            entry_policy:      EntryPolicy::Manual,
            executor_id:       U8L_EXEC,
            state_schema_hash: minor as u64 + 1,
            permissions:       &[],
            exports:           &[],
            vector_ref:        None,
        };
        let _ = gos_runtime::register_node(U8L_PLUGIN, vec, spec);
        let val = (minor % 256) as u8;
        gos_runtime::register_node_prop_u8(node_id, val);
    }

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(
        count,
        gos_runtime::MAX_NODE_PROPS_U8,
        "all {} slots must appear in the list",
        gos_runtime::MAX_NODE_PROPS_U8
    );
}

// ── 8. Overflow: 17th slot returns false, list stays at 16 ───────────────────

#[test]
fn overflow_returns_false_list_capped() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();

    // Fill all 16 slots
    for minor in 0u16..gos_runtime::MAX_NODE_PROPS_U8 as u16 {
        let vec = VectorAddress::new(36, 3, minor, 0);
        let mut key_buf = [0u8; 12];
        key_buf[0] = b'o'; key_buf[1] = b'v'; key_buf[2] = b'f';
        key_buf[3] = b'.'; key_buf[4] = b'0' + (minor / 10) as u8;
        key_buf[5] = b'0' + (minor % 10) as u8;
        let key_str = core::str::from_utf8(&key_buf[..6]).unwrap();
        let node_id = derive_node_id(U8L_PLUGIN, key_str);
        let spec = NodeSpec {
            node_id,
            local_node_key: "ovf.slot",
            node_type:         RuntimeNodeType::Service,
            entry_policy:      EntryPolicy::Manual,
            executor_id:       U8L_EXEC,
            state_schema_hash: minor as u64 + 100,
            permissions:       &[],
            exports:           &[],
            vector_ref:        None,
        };
        let _ = gos_runtime::register_node(U8L_PLUGIN, vec, spec);
        let ok = gos_runtime::register_node_prop_u8(node_id, minor as u8);
        assert!(ok, "slot {} must register successfully", minor);
    }

    // 17th attempt — a node NOT in the table
    let extra_id = derive_node_id(U8L_PLUGIN, "ovf.extra");
    let overflow_ok = gos_runtime::register_node_prop_u8(extra_id, 99);
    assert!(!overflow_ok, "17th register must return false (table full)");

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(
        count,
        gos_runtime::MAX_NODE_PROPS_U8,
        "list count must equal MAX_NODE_PROPS_U8 after overflow"
    );
}

// ── 9. Boundary vals (0 and 255) appear correctly ────────────────────────────

#[test]
fn boundary_vals_zero_and_max_appear() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(U8L_VEC_A, "u8l.alpha", U8L_ID_A, 1);
    add_node(U8L_VEC_B, "u8l.beta",  U8L_ID_B, 2);
    gos_runtime::register_node_prop_u8(U8L_ID_A, 0);
    gos_runtime::register_node_prop_u8(U8L_ID_B, 255);

    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [128u8; gos_runtime::MAX_NODE_PROPS_U8]; // non-zero sentinel
    let count = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    assert_eq!(count, 2, "two entries");

    let found_zero = (0..count).any(|i| vecs[i] == U8L_VEC_A && vals[i] == 0);
    let found_max  = (0..count).any(|i| vecs[i] == U8L_VEC_B && vals[i] == 255);
    assert!(found_zero, "val=0 (boundary low) must appear correctly");
    assert!(found_max,  "val=255 (boundary high) must appear correctly");
}

// ── 10. node_attr_list_u8 does NOT bump graph_epoch ──────────────────────────

#[test]
fn node_attr_list_u8_does_not_bump_epoch() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_node(U8L_VEC_A, "u8l.alpha", U8L_ID_A, 1);
    gos_runtime::register_node_prop_u8(U8L_ID_A, 7);

    let epoch_before = gos_runtime::graph_epoch();
    let mut vecs = [VectorAddress::new(0, 0, 0, 0); gos_runtime::MAX_NODE_PROPS_U8];
    let mut vals = [0u8; gos_runtime::MAX_NODE_PROPS_U8];
    let _ = gos_runtime::node_attr_list_u8(&mut vecs, &mut vals);
    let epoch_after = gos_runtime::graph_epoch();
    assert_eq!(epoch_after, epoch_before, "node_attr_list_u8 must not advance graph_epoch");
}
