// gos-pal-boot-harness — V2.56 palette-color boot wiring for theme nodes
//
// Verifies that theme nodes (wabi, shoji) have their primary palette colors
// stored as u32 node attributes via `gos_runtime::register_node_prop_u32`
// at boot time.  This is the first step of the PAL_U32 → graph-native refactor:
// the renderer can eventually call `node_attr_get(theme_vec)` instead of
// indexing the hardcoded `const PAL_U32: [u32; 4]` array in fbtest.rs.
//
// Palette mapping (matches fbtest.rs PAL_U32[DISPLAY_THEME_* index]):
//   theme.wabi  → 0x00DB_1C21  (RED,   PAL_U32[DISPLAY_THEME_WABI=0])
//   theme.shoji → 0x00ED_EDF2  (WHITE, PAL_U32[DISPLAY_THEME_SHOJI=1])
//
// Test matrix:
//  1.  wabi node: register_node_prop_u32(RED) → node_attr_get = Some(RED)
//  2.  shoji node: register_node_prop_u32(WHITE) → node_attr_get = Some(WHITE)
//  3.  two theme colors are independent (wabi change doesn't affect shoji)
//  4.  register_node_prop_u32 does NOT bump graph_epoch
//  5.  node_attr_set overwrites the boot-time register_node_prop_u32 value
//  6.  node_attr_get before registration returns None
//  7.  wabi red exact hex value: 0x00DB_1C21
//  8.  shoji white exact hex value: 0x00ED_EDF2
//  9.  adding more nodes does not corrupt existing palette attrs
// 10.  reset() clears both palette attrs (node_attr_get → None after reset)

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const PB_PLUGIN: PluginId   = PluginId::from_ascii("KL_PALBT_00");
const PB_EXEC:   ExecutorId = ExecutorId::from_ascii("palbt.exec0");

// PAL_U32 palette colors (matches fbtest.rs hardcoded array)
const PAL_RED:   u32 = 0x00DB_1C21; // PAL_U32[0] — wabi primary color
const PAL_WHITE: u32 = 0x00ED_EDF2; // PAL_U32[1] — shoji primary color

// Node VectorAddresses — L4=32 (unique to this harness)
const PB_VEC_WABI:  VectorAddress = VectorAddress::new(32, 1, 1, 0);
const PB_VEC_SHOJI: VectorAddress = VectorAddress::new(32, 1, 2, 0);
const PB_VEC_EXTRA: VectorAddress = VectorAddress::new(32, 1, 3, 0); // third node

const PB_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    PB_PLUGIN,
    name:         "kl-pal-boot-harness",
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
        node_id:           derive_node_id(PB_PLUGIN, key),
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       PB_EXEC,
        state_schema_hash: minor as u64,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(PB_MANIFEST).unwrap();
}

fn add_wabi_node() {
    let spec = node_spec("theme.wabi", 1);
    let id   = derive_node_id(PB_PLUGIN, "theme.wabi");
    gos_runtime::register_node(PB_PLUGIN, PB_VEC_WABI, spec).unwrap();
    gos_runtime::register_node_prop_u32(id, PAL_RED);
}

fn add_shoji_node() {
    let spec = node_spec("theme.shoji", 2);
    let id   = derive_node_id(PB_PLUGIN, "theme.shoji");
    gos_runtime::register_node(PB_PLUGIN, PB_VEC_SHOJI, spec).unwrap();
    gos_runtime::register_node_prop_u32(id, PAL_WHITE);
}

fn add_extra_node() {
    let spec = node_spec("theme.extra", 3);
    gos_runtime::register_node(PB_PLUGIN, PB_VEC_EXTRA, spec).unwrap();
}

// ── 1. wabi node: register_node_prop_u32(RED) → node_attr_get = Some(RED) ────

#[test]
fn wabi_node_gets_red_palette_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_wabi_node();
    assert_eq!(
        gos_runtime::node_attr_get(PB_VEC_WABI),
        Some(PAL_RED),
        "theme.wabi must carry PAL_RED (0x{:08X}) as its u32 node attribute", PAL_RED
    );
}

// ── 2. shoji node: register_node_prop_u32(WHITE) → node_attr_get = Some(WHITE)

#[test]
fn shoji_node_gets_white_palette_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_shoji_node();
    assert_eq!(
        gos_runtime::node_attr_get(PB_VEC_SHOJI),
        Some(PAL_WHITE),
        "theme.shoji must carry PAL_WHITE (0x{:08X}) as its u32 node attribute", PAL_WHITE
    );
}

// ── 3. two theme colors are independent ──────────────────────────────────────

#[test]
fn theme_palette_colors_are_independent() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_wabi_node();
    add_shoji_node();
    // Both colors coexist independently.
    assert_eq!(gos_runtime::node_attr_get(PB_VEC_WABI),  Some(PAL_RED),   "wabi must stay RED");
    assert_eq!(gos_runtime::node_attr_get(PB_VEC_SHOJI), Some(PAL_WHITE), "shoji must stay WHITE");
}

// ── 4. register_node_prop_u32 does NOT bump graph_epoch ──────────────────────

#[test]
fn palette_registration_does_not_bump_epoch() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Register a node (bumps epoch) then record.
    let spec = node_spec("theme.wabi", 1);
    let id   = derive_node_id(PB_PLUGIN, "theme.wabi");
    gos_runtime::register_node(PB_PLUGIN, PB_VEC_WABI, spec).unwrap();
    let epoch_before = gos_runtime::graph_epoch();
    gos_runtime::register_node_prop_u32(id, PAL_RED);
    let epoch_after = gos_runtime::graph_epoch();
    assert_eq!(
        epoch_after, epoch_before,
        "register_node_prop_u32 must not advance graph_epoch"
    );
}

// ── 5. node_attr_set overwrites register_node_prop_u32 value ─────────────────

#[test]
fn attr_set_overwrites_boot_palette_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_wabi_node(); // boots with PAL_RED
    // Override with a different color via node_attr_set.
    let new_color = 0x00FF_CC44u32; // GOLD (PAL_U32[3])
    gos_runtime::node_attr_set(PB_VEC_WABI, new_color).expect("attr_set must succeed");
    assert_eq!(
        gos_runtime::node_attr_get(PB_VEC_WABI),
        Some(new_color),
        "node_attr_set must overwrite the boot-time register_node_prop_u32 value"
    );
}

// ── 6. node_attr_get before registration returns None ────────────────────────

#[test]
fn attr_get_before_palette_registration_returns_none() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Register the node but NOT the palette color.
    let spec = node_spec("theme.wabi", 1);
    gos_runtime::register_node(PB_PLUGIN, PB_VEC_WABI, spec).unwrap();
    assert!(
        gos_runtime::node_attr_get(PB_VEC_WABI).is_none(),
        "node_attr_get before palette registration must return None"
    );
}

// ── 7. wabi red exact hex value: 0x00DB_1C21 ─────────────────────────────────

#[test]
fn wabi_red_exact_hex_value() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_wabi_node();
    let got = gos_runtime::node_attr_get(PB_VEC_WABI).expect("wabi must have palette attr");
    assert_eq!(
        got, 0x00DB_1C21u32,
        "wabi palette color must be exactly 0x00DB1C21 (RED), got 0x{:08X}", got
    );
}

// ── 8. shoji white exact hex value: 0x00ED_EDF2 ──────────────────────────────

#[test]
fn shoji_white_exact_hex_value() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_shoji_node();
    let got = gos_runtime::node_attr_get(PB_VEC_SHOJI).expect("shoji must have palette attr");
    assert_eq!(
        got, 0x00ED_EDF2u32,
        "shoji palette color must be exactly 0x00EDEFD2 (WHITE), got 0x{:08X}", got
    );
}

// ── 9. adding more nodes does not corrupt existing palette attrs ──────────────

#[test]
fn additional_nodes_do_not_corrupt_palette_attrs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_wabi_node();
    add_shoji_node();
    add_extra_node(); // third node with no palette attr
    // Palette attrs for wabi and shoji must be intact.
    assert_eq!(gos_runtime::node_attr_get(PB_VEC_WABI),  Some(PAL_RED),   "wabi intact after extra node");
    assert_eq!(gos_runtime::node_attr_get(PB_VEC_SHOJI), Some(PAL_WHITE), "shoji intact after extra node");
    // Extra node has no attr set.
    assert!(
        gos_runtime::node_attr_get(PB_VEC_EXTRA).is_none(),
        "extra node must have no palette attr"
    );
}

// ── 10. reset() clears both palette attrs ────────────────────────────────────

#[test]
fn reset_clears_palette_attrs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    add_wabi_node();
    add_shoji_node();
    // Confirm attrs exist before reset.
    assert!(gos_runtime::node_attr_get(PB_VEC_WABI).is_some(),  "wabi must have attr before reset");
    assert!(gos_runtime::node_attr_get(PB_VEC_SHOJI).is_some(), "shoji must have attr before reset");
    // Reset clears everything.
    reset();
    assert!(
        gos_runtime::node_attr_get(PB_VEC_WABI).is_none(),
        "reset must clear wabi palette attr"
    );
    assert!(
        gos_runtime::node_attr_get(PB_VEC_SHOJI).is_none(),
        "reset must clear shoji palette attr"
    );
}
