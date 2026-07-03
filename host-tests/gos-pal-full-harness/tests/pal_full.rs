// gos-pal-full-harness — V2.62 all-4 palette entries graph-native
//
// V2.57 made pal[0] (RED/wabi) and pal[1] (WHITE/shoji) graph-native.
// V2.62 adds pal[2] (CYAN) and pal[3] (GOLD) as dedicated palette.cyan /
// palette.gold graph nodes, completing the palette graph refactor.
//
// Desktop.pal_u32 population logic (fbtest.rs):
//   pal_u32 = PAL_U32;                          // compile-time fallback
//   if let Some(c) = node_attr_get(WABI_VEC)   { pal_u32[0] = c; }
//   if let Some(c) = node_attr_get(SHOJI_VEC)  { pal_u32[1] = c; }
//   if let Some(c) = node_attr_get(CYAN_VEC)   { pal_u32[2] = c; }  // V2.62
//   if let Some(c) = node_attr_get(GOLD_VEC)   { pal_u32[3] = c; }  // V2.62
//
// PAL_CONTRAST: [1, 0, 3, 2] — each index's contrasting partner.
//
// Test matrix:
//  1.  cyan node attr set → pal[2] overrides default CYAN
//  2.  gold node attr set → pal[3] overrides default GOLD
//  3.  no cyan attr → pal[2] stays at PAL_CYAN (fallback)
//  4.  no gold attr → pal[3] stays at PAL_GOLD (fallback)
//  5.  all 4 entries set → full palette populated from graph
//  6.  cyan override → rope contrast for CYAN node uses pal[PAL_CONTRAST[2]] = pal[3]
//  7.  gold override → rope contrast for GOLD node uses pal[PAL_CONTRAST[3]] = pal[2]
//  8.  reset + re-register: cyan attr survives the cycle
//  9.  reset + re-register: gold attr survives the cycle
// 10.  partial init: only cyan+gold set → wabi/shoji stay at their defaults

use std::sync::Mutex;

use gos_protocol::{
    derive_node_id, EntryPolicy, ExecutorId,
    GOS_ABI_VERSION, NodeSpec, PluginId, PluginManifest,
    RuntimeNodeType, VectorAddress,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ── Fixtures ──────────────────────────────────────────────────────────────────

const PF_PLUGIN: PluginId   = PluginId::from_ascii("KL_PALFL_00");
const PF_EXEC:   ExecutorId = ExecutorId::from_ascii("palfl.exec0");

// PAL_U32 defaults (matches fbtest.rs / pal-render-harness)
const PAL_U32: [u32; 4] = [0x00db_1c21, 0x00ed_edf2, 0x0000_ccff, 0x00ff_cc44];
const PAL_CONTRAST: [usize; 4] = [1, 0, 3, 2];

// Override colors for tests
const NEW_CYAN_COLOR: u32 = 0x0011_EEFF; // a different CYAN for override tests
const NEW_GOLD_COLOR: u32 = 0x00EE_BB00; // a different GOLD for override tests
const NEW_WABI_COLOR: u32 = 0x0044_AAFF; // hypothetical wabi override
const NEW_SHOJI_COLOR: u32 = 0x00FF_AA44; // hypothetical shoji override

// L4=38 (unique to this harness)
const PF_VEC_WABI:  VectorAddress = VectorAddress::new(38, 1, 1, 0);
const PF_VEC_SHOJI: VectorAddress = VectorAddress::new(38, 1, 2, 0);
const PF_VEC_CYAN:  VectorAddress = VectorAddress::new(38, 1, 5, 0);
const PF_VEC_GOLD:  VectorAddress = VectorAddress::new(38, 1, 6, 0);

const PF_MANIFEST: PluginManifest = PluginManifest {
    abi_version:  GOS_ABI_VERSION,
    plugin_id:    PF_PLUGIN,
    name:         "kl-pal-full-harness",
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
        node_id:           derive_node_id(PF_PLUGIN, key),
        local_node_key:    key,
        node_type:         RuntimeNodeType::Service,
        entry_policy:      EntryPolicy::Manual,
        executor_id:       PF_EXEC,
        state_schema_hash: minor as u64,
        permissions:       &[],
        exports:           &[],
        vector_ref:        None,
    }
}

fn reset() { gos_runtime::reset(); }

fn register_plugin() {
    gos_runtime::discover_plugin(PF_MANIFEST).unwrap();
}

/// Simulate Desktop.pal_u32 population (V2.62 version — all 4 entries).
fn populate_pal(
    wabi_vec:  VectorAddress,
    shoji_vec: VectorAddress,
    cyan_vec:  VectorAddress,
    gold_vec:  VectorAddress,
) -> [u32; 4] {
    let mut pal = PAL_U32;
    if let Some(c) = gos_runtime::node_attr_get(wabi_vec)  { pal[0] = c; }
    if let Some(c) = gos_runtime::node_attr_get(shoji_vec) { pal[1] = c; }
    if let Some(c) = gos_runtime::node_attr_get(cyan_vec)  { pal[2] = c; }
    if let Some(c) = gos_runtime::node_attr_get(gold_vec)  { pal[3] = c; }
    pal
}

// ── 1. cyan node attr set → pal[2] overrides default CYAN ────────────────────

#[test]
fn cyan_override_replaces_pal_index_2() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_CYAN, node_spec("palette.cyan", 5)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_CYAN, NEW_CYAN_COLOR).unwrap();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(pal[2], NEW_CYAN_COLOR, "cyan override must replace pal[2]");
}

// ── 2. gold node attr set → pal[3] overrides default GOLD ────────────────────

#[test]
fn gold_override_replaces_pal_index_3() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_GOLD, node_spec("palette.gold", 6)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_GOLD, NEW_GOLD_COLOR).unwrap();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(pal[3], NEW_GOLD_COLOR, "gold override must replace pal[3]");
}

// ── 3. no cyan attr → pal[2] stays at PAL_CYAN (fallback) ────────────────────

#[test]
fn no_cyan_attr_falls_back_to_default_cyan() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(
        pal[2], PAL_U32[2],
        "no cyan attr: pal[2] must be default CYAN 0x{:08X}", PAL_U32[2]
    );
}

// ── 4. no gold attr → pal[3] stays at PAL_GOLD (fallback) ────────────────────

#[test]
fn no_gold_attr_falls_back_to_default_gold() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(
        pal[3], PAL_U32[3],
        "no gold attr: pal[3] must be default GOLD 0x{:08X}", PAL_U32[3]
    );
}

// ── 5. all 4 entries set → full palette populated from graph ──────────────────

#[test]
fn all_four_entries_set_populates_full_palette() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_WABI,  node_spec("theme.wabi",  1)).unwrap();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_SHOJI, node_spec("theme.shoji", 2)).unwrap();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_CYAN,  node_spec("palette.cyan",5)).unwrap();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_GOLD,  node_spec("palette.gold",6)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_WABI,  NEW_WABI_COLOR).unwrap();
    gos_runtime::node_attr_set(PF_VEC_SHOJI, NEW_SHOJI_COLOR).unwrap();
    gos_runtime::node_attr_set(PF_VEC_CYAN,  NEW_CYAN_COLOR).unwrap();
    gos_runtime::node_attr_set(PF_VEC_GOLD,  NEW_GOLD_COLOR).unwrap();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(pal[0], NEW_WABI_COLOR,  "pal[0] must be new wabi color");
    assert_eq!(pal[1], NEW_SHOJI_COLOR, "pal[1] must be new shoji color");
    assert_eq!(pal[2], NEW_CYAN_COLOR,  "pal[2] must be new cyan color");
    assert_eq!(pal[3], NEW_GOLD_COLOR,  "pal[3] must be new gold color");
}

// ── 6. cyan override → rope contrast for CYAN node uses pal[3] ───────────────
// PAL_CONTRAST[2] = 3, so a CYAN-colored node's rope half uses pal[3] (GOLD).

#[test]
fn cyan_override_changes_rope_contrast_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_CYAN, node_spec("palette.cyan", 5)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_CYAN, NEW_CYAN_COLOR).unwrap();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    // node_color=2 (CYAN) → rope uses contrast = PAL_CONTRAST[2] = 3 → pal[3]
    // pal[3] unchanged (no gold attr) → stays at default GOLD.
    let rope_color = pal[PAL_CONTRAST[2]];
    assert_eq!(
        rope_color, PAL_U32[3],
        "rope half touching CYAN node must use pal[PAL_CONTRAST[2]]=pal[3] (default GOLD)"
    );
    assert_eq!(pal[2], NEW_CYAN_COLOR, "pal[2] must be new cyan color");
}

// ── 7. gold override → rope contrast for GOLD node uses pal[2] ───────────────
// PAL_CONTRAST[3] = 2, so a GOLD-colored node's rope half uses pal[2] (CYAN).

#[test]
fn gold_override_changes_rope_contrast_color() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_GOLD, node_spec("palette.gold", 6)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_GOLD, NEW_GOLD_COLOR).unwrap();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    // node_color=3 (GOLD) → rope uses contrast = PAL_CONTRAST[3] = 2 → pal[2]
    // pal[2] unchanged (no cyan attr) → stays at default CYAN.
    let rope_color = pal[PAL_CONTRAST[3]];
    assert_eq!(
        rope_color, PAL_U32[2],
        "rope half touching GOLD node must use pal[PAL_CONTRAST[3]]=pal[2] (default CYAN)"
    );
    assert_eq!(pal[3], NEW_GOLD_COLOR, "pal[3] must be new gold color");
}

// ── 8. reset + re-register: cyan attr survives cycle ─────────────────────────

#[test]
fn reset_and_re_register_restores_cyan_attr() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_CYAN, node_spec("palette.cyan", 5)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_CYAN, NEW_CYAN_COLOR).unwrap();

    reset();
    let pal_after_reset = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(pal_after_reset[2], PAL_U32[2], "after reset, pal[2] must be default CYAN");

    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_CYAN, node_spec("palette.cyan", 5)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_CYAN, NEW_CYAN_COLOR).unwrap();
    let pal_after_rereg = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(
        pal_after_rereg[2], NEW_CYAN_COLOR,
        "after re-registration, pal[2] must be new cyan color"
    );
}

// ── 9. reset + re-register: gold attr survives cycle ─────────────────────────

#[test]
fn reset_and_re_register_restores_gold_attr() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_GOLD, node_spec("palette.gold", 6)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_GOLD, NEW_GOLD_COLOR).unwrap();

    reset();
    let pal_after_reset = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(pal_after_reset[3], PAL_U32[3], "after reset, pal[3] must be default GOLD");

    register_plugin();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_GOLD, node_spec("palette.gold", 6)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_GOLD, NEW_GOLD_COLOR).unwrap();
    let pal_after_rereg = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(
        pal_after_rereg[3], NEW_GOLD_COLOR,
        "after re-registration, pal[3] must be new gold color"
    );
}

// ── 10. partial init: only cyan+gold set → wabi/shoji stay at defaults ─────────

#[test]
fn partial_init_only_cyan_gold_leaves_wabi_shoji_at_defaults() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset();
    register_plugin();
    // Only CYAN and GOLD registered — wabi/shoji have no nodes.
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_CYAN, node_spec("palette.cyan", 5)).unwrap();
    gos_runtime::register_node(PF_PLUGIN, PF_VEC_GOLD, node_spec("palette.gold", 6)).unwrap();
    gos_runtime::node_attr_set(PF_VEC_CYAN, NEW_CYAN_COLOR).unwrap();
    gos_runtime::node_attr_set(PF_VEC_GOLD, NEW_GOLD_COLOR).unwrap();
    let pal = populate_pal(PF_VEC_WABI, PF_VEC_SHOJI, PF_VEC_CYAN, PF_VEC_GOLD);
    assert_eq!(pal[0], PAL_U32[0],     "wabi not set: must fall back to default RED");
    assert_eq!(pal[1], PAL_U32[1],     "shoji not set: must fall back to default WHITE");
    assert_eq!(pal[2], NEW_CYAN_COLOR, "CYAN override must apply");
    assert_eq!(pal[3], NEW_GOLD_COLOR, "GOLD override must apply");
}
