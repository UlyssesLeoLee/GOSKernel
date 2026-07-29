---
name: gos-runtime-error-per-table-variant
description: When adding a new fixed-capacity table to GraphRuntime, always add a dedicated RuntimeError variant for that table's overflow — never reuse generic names like CapacityExceeded. Each table (node_props_u8, node_props_u32, subscribe_pairs, etc.) has its own *TableFull variant. Apply whenever adding a new bounded collection to crates/gos-runtime/src/lib.rs.
---

# RuntimeError: one variant per table, never generic

## The rule

Every bounded table in `GraphRuntime` gets its own `RuntimeError` variant:

```rust
pub enum RuntimeError {
    PluginTableFull,
    NodeTableFull,
    EdgeTableFull,
    NodeArenaFull,
    SubscribeTableFull,
    PropTableFull,      // V2.55: for node_props_u32 (and node_props_u8 if needed)
    // ...
}
```

There is no `CapacityExceeded` or `Full` catch-all. Adding a new table means adding a new variant.

## Why it's non-obvious

The pattern is clear in hindsight but easy to miss under time pressure. It's tempting to use a vague `CapacityExceeded` that seems general-purpose — but that variant doesn't exist in the enum, causing an immediate compile error. The GOSKernel convention is explicit: each failure mode has a dedicated discriminant so callers can match exactly which resource was exhausted.

## GOSKernel context

`crates/gos-runtime/src/lib.rs` — the `RuntimeError` enum at the top of the file. When the match is exhaustive in callers (k-shell `match gos_runtime::node_attr_set(...)`), any unrecognised variant causes a compile error there too.

## From this session

V2.55: First wrote `Err(RuntimeError::CapacityExceeded)` — compile error since that variant doesn't exist. Correct fix: add `PropTableFull` to the enum and use `Err(RuntimeError::PropTableFull)`.
