// gos-pal-render-harness — V2.57 graph-native palette for Desktop renderer
//
// Verifies the "Desktop.pal_u32 population" pattern: at init time the renderer
// reads node_attr_get(theme_vec) for wabi/shoji and overrides the default
// PAL_U32 constants.  The other two palette entries (CYAN, GOLD) stay at their
// compile-time defaults since they have no graph-backed theme node yet.
//
// This harness models the Desktop.pal_u32 caching logic in host-testable form:
//   pal_u32 = PAL_U32;  // compile-time default
//   if let Some(c) = node_attr_get(THEME_WABI_VEC)  { pal_u32[0] = c; }
//   if let Some(c) = node_attr_get(THEME_SHOJI_VEC) { pal_u32[1] = c; }
//
// PAL_CONTRAST invariant (from fbtest.rs):
//   PAL_CONTRAST = [1, 0, 3, 2]  — each color's contrasting partner
//
// Test matrix:
//  1.  wabi attr set → pal_u32[0] overrides default RED
//  2.  shoji attr set → pal_u32[1] overrides default WHITE
//  3.  no attr set → pal_u32[0] stays at PAL_RED (fallback)
//  4.  no attr set → pal_u32[1] stays at PAL_WHITE (fallback)
//  5.  pal_u32[2] (CYAN) unchanged by theme attrs — no graph-backing yet
//  6.  pal_u32[3] (GOLD) unchanged by theme attrs — no graph-backing yet
//  7.  override wabi color → rope uses new contrasting color via PAL_CONTRAST
//  8.  override shoji color → rope uses new contrasting color via PAL_CONTRAST
//  9.  reset then re-register: attr survives reset + re-registration cycle
// 10.  partial init: only wabi set → shoji stays at WHITE default

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const PR_PLUGIN: PluginId   = PluginId::from_ascii("KL_PALRD_00");
const PR_EXEC:   ExecutorId = ExecutorId::from_ascii("palrd.exec0");

// Hardcoded PAL_U32 defaults (matches fbtest.rs)
const PAL_U32: [u32; 4] = [0x00db_1c21, 0x00ed_edf2, 0x0000_ccff, 0x00ff_cc44];
const PAL_CONTRAST: [usize; 4] = [1, 0, 3, 2];

// New colors for override tests
const NEW_WABI_COLOR:  u32 = 0x0044_AAFF; // a hypothetical new wabi color
const NEW_SHOJI_COLOR: u32 = 0x00FF_AA44; // a hypothetical new shoji color

// L4=33 (unique to this harness)
const PR_VEC_WABI:  VectorAddress = VectorAddress::new(33, 1, 1, 0);
const PR_VEC_SHOJI: VectorAddress = VectorAddress::new(33, 1, 2, 0);

const PR_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    PR_PLUGIN,
    name:         "kl-pal-render-harness",
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
        node_id:           derive_node_id(PR_PLUGIN, key),
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       PR_EXEC,
        state_schema_hash: minor as u64,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(PR_MANIFEST).unwrap();
}

/// Simulate Desktop.pal_u32 population from graph node attrs.
/// Returns the 4-entry palette array the renderer would use.
fn populate_pal(wabi_vec: VectorAddress, shoji_vec: VectorAddress) -> [u32; 4] {
    let mut pal = PAL_U32;
    if let Some(c) = gos_runtime::node_attr_get(wabi_vec)  { pal[0] = c; }
    if let Some(c) = gos_runtime::node_attr_get(shoji_vec) { pal[1] = c; }
    pal
}

// ── 1. wabi attr set → pal_u32[0] overrides default RED ─────────────────────

#[test]
fn wabi_override_replaces_pal_index_0() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_WABI, node_spec("theme.wabi", 1)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_WABI, NEW_WABI_COLOR).unwrap();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(pal[0], NEW_WABI_COLOR, "wabi override must replace pal[0]");
}

// ── 2. shoji attr set → pal_u32[1] overrides default WHITE ───────────────────

#[test]
fn shoji_override_replaces_pal_index_1() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_SHOJI, node_spec("theme.shoji", 2)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_SHOJI, NEW_SHOJI_COLOR).unwrap();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(pal[1], NEW_SHOJI_COLOR, "shoji override must replace pal[1]");
}

// ── 3. no wabi attr → pal[0] stays at PAL_RED (fallback) ────────────────────

#[test]
fn no_wabi_attr_falls_back_to_default_red() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    // No nodes registered — node_attr_get returns None for both.
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(pal[0], PAL_U32[0], "no wabi attr: pal[0] must be default RED 0x{:08X}", PAL_U32[0]);
}

// ── 4. no shoji attr → pal[1] stays at PAL_WHITE (fallback) ─────────────────

#[test]
fn no_shoji_attr_falls_back_to_default_white() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(pal[1], PAL_U32[1], "no shoji attr: pal[1] must be default WHITE 0x{:08X}", PAL_U32[1]);
}

// ── 5. pal[2] (CYAN) unchanged by theme attrs ────────────────────────────────

#[test]
fn pal_index_2_stays_at_cyan_regardless_of_theme_attrs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_WABI,  node_spec("theme.wabi",  1)).unwrap();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_SHOJI, node_spec("theme.shoji", 2)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_WABI,  NEW_WABI_COLOR).unwrap();
    gos_runtime::node_attr_set(PR_VEC_SHOJI, NEW_SHOJI_COLOR).unwrap();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(
        pal[2], PAL_U32[2],
        "CYAN (pal[2]=0x{:08X}) must not be changed by theme node attrs", PAL_U32[2]
    );
}

// ── 6. pal[3] (GOLD) unchanged by theme attrs ────────────────────────────────

#[test]
fn pal_index_3_stays_at_gold_regardless_of_theme_attrs() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_WABI,  node_spec("theme.wabi",  1)).unwrap();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_SHOJI, node_spec("theme.shoji", 2)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_WABI,  NEW_WABI_COLOR).unwrap();
    gos_runtime::node_attr_set(PR_VEC_SHOJI, NEW_SHOJI_COLOR).unwrap();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(
        pal[3], PAL_U32[3],
        "GOLD (pal[3]=0x{:08X}) must not be changed by theme node attrs", PAL_U32[3]
    );
}

// ── 7. wabi override → rope contrast color from new pal[0] ───────────────────

#[test]
fn wabi_override_changes_rope_contrast_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_WABI, node_spec("theme.wabi", 1)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_WABI, NEW_WABI_COLOR).unwrap();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    // node_color[a]=0 (wabi) → rope half-a uses contrast = PAL_CONTRAST[0] = pal[1]
    // pal[1] unchanged (no shoji attr) so rope still sees default WHITE.
    let rope_color_from_wabi_node = pal[PAL_CONTRAST[0]];
    assert_eq!(
        rope_color_from_wabi_node, PAL_U32[1],
        "rope half touching a wabi-colored node must use pal[PAL_CONTRAST[0]]=pal[1] (WHITE)"
    );
    // pal[0] itself is the new wabi color now.
    assert_eq!(pal[0], NEW_WABI_COLOR, "pal[0] must be new wabi color");
}

// ── 8. shoji override → rope contrast color from new pal[1] ──────────────────

#[test]
fn shoji_override_changes_rope_contrast_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_SHOJI, node_spec("theme.shoji", 2)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_SHOJI, NEW_SHOJI_COLOR).unwrap();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    // node_color[b]=1 (shoji) → rope half-b uses contrast = PAL_CONTRAST[1] = pal[0]
    // pal[0] unchanged (no wabi attr) so rope still sees default RED.
    let rope_color_from_shoji_node = pal[PAL_CONTRAST[1]];
    assert_eq!(
        rope_color_from_shoji_node, PAL_U32[0],
        "rope half touching a shoji-colored node must use pal[PAL_CONTRAST[1]]=pal[0] (RED)"
    );
    // pal[1] itself is the new shoji color.
    assert_eq!(pal[1], NEW_SHOJI_COLOR, "pal[1] must be new shoji color");
}

// ── 9. reset then re-register: attr reappears correctly ──────────────────────

#[test]
fn reset_and_re_register_restores_palette_attr() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_WABI, node_spec("theme.wabi", 1)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_WABI, NEW_WABI_COLOR).unwrap();

    // After reset, attr is cleared.
    reset();
    let pal_after_reset = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(pal_after_reset[0], PAL_U32[0], "after reset, pal[0] must be default RED");

    // Re-register and set again.
    register_plugin();
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_WABI, node_spec("theme.wabi", 1)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_WABI, NEW_WABI_COLOR).unwrap();
    let pal_after_rereg = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(
        pal_after_rereg[0], NEW_WABI_COLOR,
        "after re-registration, pal[0] must be new wabi color"
    );
}

// ── 10. partial init: only wabi set → shoji stays at WHITE default ────────────

#[test]
fn partial_init_only_wabi_leaves_shoji_at_default() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Only wabi registered and attr set; shoji has no node at all.
    gos_runtime::register_node(PR_PLUGIN, PR_VEC_WABI, node_spec("theme.wabi", 1)).unwrap();
    gos_runtime::node_attr_set(PR_VEC_WABI, NEW_WABI_COLOR).unwrap();
    let pal = populate_pal(PR_VEC_WABI, PR_VEC_SHOJI);
    assert_eq!(pal[0], NEW_WABI_COLOR, "wabi override must apply");
    assert_eq!(pal[1], PAL_U32[1],     "shoji not set: must fall back to default WHITE");
    assert_eq!(pal[2], PAL_U32[2],     "CYAN unchanged");
    assert_eq!(pal[3], PAL_U32[3],     "GOLD unchanged");
}
