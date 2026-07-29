---
name: gos-graph-diff-kind-extension
description: When adding a new variant to GraphDiffKind in gos-protocol, three sites must be updated in the same commit — the enum body, the is_node()/is_addition() impl, and the exhaustive match in dispatch_graph_diff in k-shell. Omitting any one causes a compile error or wrong visual rendering. Apply whenever crates/gos-protocol/src/lib.rs GraphDiffKind is modified.
---

# GraphDiffKind Extension: Three Required Update Sites

## The rule

Adding a new `GraphDiffKind` variant requires exactly three updates, all in the same commit:

1. **`crates/gos-protocol/src/lib.rs` — the enum body**  
   Add the variant with a unique `u8` discriminant (`#[repr(u8)]`).

2. **`crates/gos-protocol/src/lib.rs` — `is_node()` and `is_addition()` impl**  
   Decide classification for the new variant:
   - `is_addition()` → `true` = green "+" display, `false` = red "-" or neutral
   - `is_node()` → `true` = render as `[kind] vec  label`, `false` = render as `from -[key]-> to`

3. **`crates/k-shell/src/lib.rs` — `dispatch_graph_diff` exhaustive match**  
   Add an arm: `GraphDiffKind::NewVariant => "label"`.  
   Also update the `(prefix, fg)` match above it if the new kind needs a non-standard color.

## Why it's non-obvious

`GraphDiffKind` has no `#[non_exhaustive]` attribute, so the compiler enforces exhaustiveness. The two match expressions in `dispatch_graph_diff` are separate — one for `(prefix, fg)` and one for `kind_label` — so both must be updated. The `is_node()` classification also controls layout: `true` shows `vec + label`, `false` shows `from -[edge]-> to`.

## GOSKernel context

- `crates/gos-protocol/src/lib.rs:1788` — `GraphDiffKind` enum
- `crates/k-shell/src/lib.rs:~1467` — `dispatch_graph_diff` exhaustive matches
- Existing variants: `NodeAdded=0`, `NodeRemoved=1`, `EdgeAdded=2`, `EdgeRemoved=3`, `NodeCheckpoint=4`
- `is_node()` currently returns true for: `NodeAdded`, `NodeRemoved`, `NodeCheckpoint`
- `is_addition()` currently returns true for: `NodeAdded`, `EdgeAdded`

## From this session

V2.51: Added `NodeCheckpoint = 4`. Classification: `is_node()=true` (renders as vec+label), `is_addition()=false` (neutral yellow fg=14, prefix `·`). Both match arms in `dispatch_graph_diff` were updated simultaneously.
