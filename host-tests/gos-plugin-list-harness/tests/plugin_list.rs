// gos-plugin-list-harness — V2.20 plugin inventory API tests
//
// Verifies plugin_page() and plugin_count() added in V2.20,
// underpinning the new `plugins` / `lsmod` shell command.
//
//  1. plugin_count returns 0 on empty runtime.
//  2. plugin_count is 1 after one discover_plugin call.
//  3. plugin_count is 2 after two distinct discover_plugin calls.
//  4. plugin_page returns (0, 0) on empty runtime.
//  5. plugin_page fills PluginSummary with correct name after discover.
//  6. plugin_page reports PluginState::Discovered before mark_plugin_loaded.
//  7. plugin_page reports PluginState::Loaded after mark_plugin_loaded.
//  8. plugin_page node_count is 0 before any nodes are registered.
//  9. plugin_page node_count equals the number of registered nodes under that plugin.
// 10. plugin_page respects the page cap (filled <= array size).

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId, GOS_ABI_VERSION,
    NodeId, NodeSpec, PluginId, PluginManifest, PluginState, PluginSummary,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

const PL_PLUGIN_A: PluginId  = PluginId::from_ascii("PL_PLGN_A");
const PL_PLUGIN_B: PluginId  = PluginId::from_ascii("PL_PLGN_B");
const PL_EXEC:     ExecutorId = ExecutorId::from_ascii("pl.exec");

const PL_KEY_A: &str = "pl.alpha";
const PL_KEY_B: &str = "pl.beta";

const PL_ID_A: NodeId = derive_node_id(PL_PLUGIN_A, PL_KEY_A);
const PL_ID_B: NodeId = derive_node_id(PL_PLUGIN_A, PL_KEY_B);

const MANIFEST_A: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: PL_PLUGIN_A,
    name: "pl-plugin-alpha",
    version: 2,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[],
    edges: &[],
    signature: None,
    policy_hash: [0u8; 16],
};

const MANIFEST_B: PluginManifest = PluginManifest {
    abi_version: GOS_ABI_VERSION,
    plugin_id: PL_PLUGIN_B,
    name: "pl-plugin-beta",
    version: 1,
    depends_on: &[],
    permissions: &[],
    exports: &[],
    imports: &[],
    nodes: &[],
    edges: &[],
    signature: None,
    policy_hash: [0u8; 16],
};

fn make_vec(l4: u8, l3: u16, l2: u16, offset: u16) -> VectorAddress {
    VectorAddress::new(l4, l3, l2, offset)
}

fn reset() {
    gos_runtime::reset();
}

// ── 1. plugin_count returns 0 on empty runtime ───────────────────────────────

#[test]
fn plugin_count_zero_on_empty_runtime() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    assert_eq!(gos_runtime::plugin_count(), 0);
}

// ── 2. plugin_count is 1 after one discover_plugin call ─────────────────────

#[test]
fn plugin_count_one_after_single_discover() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();
    assert_eq!(gos_runtime::plugin_count(), 1);
}

// ── 3. plugin_count is 2 after two distinct discover_plugin calls ─────────────

#[test]
fn plugin_count_two_after_two_discovers() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();
    gos_runtime::discover_plugin(MANIFEST_B).unwrap();
    assert_eq!(gos_runtime::plugin_count(), 2);
}

// ── 4. plugin_page returns (0, 0) on empty runtime ──────────────────────────

#[test]
fn plugin_page_zero_on_empty_runtime() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    let mut page = [PluginSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::plugin_page::<8>(0, &mut page);
    assert_eq!(total, 0);
    assert_eq!(filled, 0);
}

// ── 5. plugin_page fills PluginSummary with correct name after discover ──────

#[test]
fn plugin_page_correct_name_after_discover() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();
    let mut page = [PluginSummary::EMPTY; 8];
    let (total, filled) = gos_runtime::plugin_page::<8>(0, &mut page);
    assert_eq!(total, 1);
    assert_eq!(filled, 1);
    assert_eq!(page[0].name, "pl-plugin-alpha");
    assert_eq!(page[0].version, 2);
}

// ── 6. plugin_page reports Discovered state before mark_plugin_loaded ────────

#[test]
fn plugin_page_state_discovered_before_load() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();
    let mut page = [PluginSummary::EMPTY; 8];
    gos_runtime::plugin_page::<8>(0, &mut page);
    assert_eq!(page[0].state, PluginState::Discovered);
}

// ── 7. plugin_page reports Loaded state after mark_plugin_loaded ─────────────

#[test]
fn plugin_page_state_loaded_after_mark_loaded() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();
    gos_runtime::mark_plugin_loaded(PL_PLUGIN_A).unwrap();
    let mut page = [PluginSummary::EMPTY; 8];
    gos_runtime::plugin_page::<8>(0, &mut page);
    assert_eq!(page[0].state, PluginState::Loaded);
}

// ── 8. plugin_page node_count is 0 before any nodes are registered ───────────

#[test]
fn plugin_page_node_count_zero_before_registration() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();
    let mut page = [PluginSummary::EMPTY; 8];
    gos_runtime::plugin_page::<8>(0, &mut page);
    assert_eq!(page[0].node_count, 0);
}

// ── 9. plugin_page node_count equals registered nodes under that plugin ───────

#[test]
fn plugin_page_node_count_matches_registered_nodes() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();

    let spec_a = NodeSpec {
        node_id: PL_ID_A,
        local_node_key: PL_KEY_A,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: PL_EXEC,
        state_schema_hash: 0xAB20,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    };
    let spec_b = NodeSpec {
        node_id: PL_ID_B,
        local_node_key: PL_KEY_B,
        node_type: RuntimeNodeType::Service,
        entry_policy: EntryPolicy::Manual,
        executor_id: PL_EXEC,
        state_schema_hash: 0xAB20,
        permissions: &[],
        exports: &[],
        vector_ref: None,
    };

    gos_runtime::register_node(PL_PLUGIN_A, make_vec(7, 1, 1, 0), spec_a).unwrap();
    gos_runtime::register_node(PL_PLUGIN_A, make_vec(7, 1, 1, 1), spec_b).unwrap();

    let mut page = [PluginSummary::EMPTY; 8];
    gos_runtime::plugin_page::<8>(0, &mut page);
    assert_eq!(page[0].node_count, 2);
}

// ── 10. plugin_page respects the page cap (filled <= array size) ──────────────

#[test]
fn plugin_page_respects_page_cap() {
    let _g = TEST_LOCK.lock().unwrap();
    reset();
    gos_runtime::discover_plugin(MANIFEST_A).unwrap();
    gos_runtime::discover_plugin(MANIFEST_B).unwrap();

    // Ask for a page of size 1 — should only fill 1 even though 2 exist
    let mut page = [PluginSummary::EMPTY; 1];
    let (total, filled) = gos_runtime::plugin_page::<1>(0, &mut page);
    assert_eq!(total, 2);
    assert_eq!(filled, 1);
}
