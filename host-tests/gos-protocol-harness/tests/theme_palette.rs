//! Property tests for the V2.3b shared theme palette
//! (`gos_protocol::theme`, V2.3 plan deliverable 2).
//!
//! `PALETTE_WABI` / `PALETTE_SHOJI` moved out of `k-vga` (their only reader)
//! into `gos-protocol`, addressable by the same `DISPLAY_THEME_WABI` /
//! `DISPLAY_THEME_SHOJI` kind bytes `k-shell`'s `theme.current -[Use]->
//! theme.wabi|shoji` edge already switches between. These tests pin down
//! `palette_for_theme`'s lookup/default behaviour and the VGA-DAC shape
//! invariant (`0..=63` per channel) that `k-vga::apply_theme_palette` relies
//! on when writing these bytes to the DAC palette registers.
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "theme_palette.rs", type: "file", language: "rust"}),
//!   (pft:Function {name: "palette_for_theme", type: "function"}),
//!   (t1:Function {name: "wabi_kind_resolves_to_palette_wabi", type: "function"}),
//!   (t2:Function {name: "shoji_kind_resolves_to_palette_shoji", type: "function"}),
//!   (t3:Function {name: "unknown_kind_defaults_to_wabi", type: "function"}),
//!   (t4:Function {name: "palettes_are_within_vga_dac_range", type: "function"}),
//!   (t5:Function {name: "wabi_and_shoji_palettes_differ", type: "function"}),
//!   (f)-[:CONTAINS]->(t1), (f)-[:CONTAINS]->(t2), (f)-[:CONTAINS]->(t3),
//!   (f)-[:CONTAINS]->(t4), (f)-[:CONTAINS]->(t5),
//!   (t1)-[:USES]->(pft), (t2)-[:USES]->(pft), (t3)-[:USES]->(pft);
//! ```

use gos_protocol::{palette_for_theme, DISPLAY_THEME_SHOJI, DISPLAY_THEME_WABI, PALETTE_SHOJI, PALETTE_WABI};

#[test]
fn wabi_kind_resolves_to_palette_wabi() {
    assert_eq!(palette_for_theme(DISPLAY_THEME_WABI), &PALETTE_WABI);
}

#[test]
fn shoji_kind_resolves_to_palette_shoji() {
    assert_eq!(palette_for_theme(DISPLAY_THEME_SHOJI), &PALETTE_SHOJI);
}

/// Unrecognized theme bytes fall back to wabi — the same default
/// `k-vga::apply_theme_palette` applied before this module existed.
#[test]
fn unknown_kind_defaults_to_wabi() {
    assert_eq!(palette_for_theme(0xFF), &PALETTE_WABI);
}

/// Every channel must fit the VGA DAC's 6-bit register range, or
/// `apply_theme_palette`'s raw port writes would silently truncate.
#[test]
fn palettes_are_within_vga_dac_range() {
    for palette in [&PALETTE_WABI, &PALETTE_SHOJI] {
        for entry in palette {
            for &channel in entry {
                assert!(channel <= 63, "DAC channel {channel} out of 6-bit range in {entry:?}");
            }
        }
    }
}

/// The two themes must actually look different — a regression here would
/// make theme-switching a no-op.
#[test]
fn wabi_and_shoji_palettes_differ() {
    assert_ne!(PALETTE_WABI, PALETTE_SHOJI);
}
