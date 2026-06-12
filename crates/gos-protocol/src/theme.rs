//! Theme palette data ([`doc/V2_DEVELOPMENT_PLAN.md`] Phase V2.3, deliverable 2).
//!
//! V2.3's plan calls for `theme.wabi` / `theme.shoji` to stop being
//! hardcoded `PAL_U32`-style constants private to one renderer, and instead
//! become palette data addressable by *theme identity* — the same
//! [`crate::DISPLAY_THEME_WABI`] / [`crate::DISPLAY_THEME_SHOJI`] kind bytes
//! that `k-shell`'s `theme.current -[Use]-> theme.wabi|shoji` edge already
//! switches between (ADR-004 §2.2 `rebind_use`).
//!
//! This module is that shared addressable layer: [`PALETTE_WABI`] /
//! [`PALETTE_SHOJI`] and [`palette_for_theme`] move out of `k-vga` (the only
//! current reader) into `gos-protocol`, keyed by the same theme-kind bytes
//! used everywhere else. A render node's path to a theme's colors is now
//! "theme-kind byte -> [`palette_for_theme`]" rather than a private const
//! array — the addressing half of deliverable 2. Storing the palette as
//! mutable state *on* the `theme.wabi`/`theme.shoji` graph nodes themselves
//! is deferred until a node-payload mechanism exists (V2.3c+).
//!
//! Each `[u8; 3]` entry is an `(r, g, b)` triple in the VGA DAC's 6-bit range
//! (`0..=63`) — [`palette_for_theme`]'s sole current caller,
//! `k-vga::apply_theme_palette`, writes these bytes directly to the DAC
//! palette registers (port `0x3C8`/`0x3C9`).
//!
//! ```cypher
//! CREATE
//!   (f:File {name: "theme.rs", type: "file", language: "rust"}),
//!   (pal:Class {name: "Palette", type: "type_alias"}),
//!   (pft:Function {name: "palette_for_theme", type: "function", visibility: "pub"}),
//!   (f)-[:CONTAINS]->(pal), (f)-[:CONTAINS]->(pft),
//!   (pft)-[:USES]->(pal);
//! ```

use crate::DISPLAY_THEME_SHOJI;

/// 16-entry VGA DAC palette: each `[r, g, b]` component is in `0..=63`.
pub type Palette = [[u8; 3]; 16];

pub const PALETTE_WABI: Palette = [
    [4, 4, 4],
    [9, 14, 21],
    [14, 21, 15],
    [17, 24, 22],
    [23, 16, 13],
    [22, 18, 24],
    [30, 24, 18],
    [43, 40, 34],
    [21, 21, 20],
    [19, 27, 36],
    [24, 34, 24],
    [29, 37, 34],
    [38, 26, 22],
    [34, 29, 37],
    [44, 37, 28],
    [58, 56, 50],
];

pub const PALETTE_SHOJI: Palette = [
    [5, 4, 3],
    [10, 16, 28],
    [18, 24, 16],
    [19, 29, 28],
    [28, 18, 16],
    [28, 18, 27],
    [41, 32, 19],
    [51, 47, 39],
    [25, 22, 18],
    [20, 32, 46],
    [30, 40, 24],
    [36, 46, 44],
    [43, 29, 24],
    [42, 30, 40],
    [55, 48, 24],
    [63, 60, 52],
];

/// Look up the palette for a `DISPLAY_THEME_*` kind byte. Unknown values fall
/// back to [`PALETTE_WABI`] — the same default `k-vga::apply_theme_palette`
/// applied before this module existed.
pub fn palette_for_theme(theme: u8) -> &'static Palette {
    match theme {
        DISPLAY_THEME_SHOJI => &PALETTE_SHOJI,
        _ => &PALETTE_WABI,
    }
}
