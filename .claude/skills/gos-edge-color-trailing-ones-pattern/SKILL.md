---
name: gos-edge-color-trailing-ones-pattern
description: For greedy edge coloring (and any greedy slot-allocation on ≤128 items), encode used slots as a u128 bitmask per node and use `forbidden.trailing_ones() as u8` to find the lowest free slot in O(1) — avoids a loop over candidate colors entirely.
---

# Edge Color: trailing_ones() for Lowest Free Slot

## The rule

```rust
// Per-node bitmask: bit k set ⟹ colour k already assigned to an incident edge.
let mut node_colors = [0u128; MAX_NODES];

for each undirected edge (a, b):
    let forbidden = node_colors[a] | node_colors[b];
    // trailing_ones() = index of the lowest 0-bit = lowest free colour slot
    let colour = forbidden.trailing_ones() as u8;
    node_colors[a] |= 1u128 << colour;
    node_colors[b] |= 1u128 << colour;
```

`trailing_ones()` counts consecutive 1-bits from the LSB before the first 0. For example:
- `0b0000` → 0 (colour 0 free)
- `0b0001` → 1 (colour 0 used, colour 1 free)
- `0b0011` → 2 (colours 0–1 used, colour 2 free)
- `0b0101` → 1 (colour 0 used, colour 1 free despite colour 2 being used)

## Why it's non-obvious

The natural greedy approach scans `for c in 0..=255 { if !forbidden[c] { break; } }` — O(Δ) per edge.
The bitmask + `trailing_ones()` approach is O(1) per edge (single hardware instruction on x86-64).

For u128, `trailing_ones()` never returns > 127 for valid graphs (Vizing: χ'(G) ≤ Δ+1 ≤ MAX_NODES = 128).
So `as u8` is always safe (0..128 fits u8).

The `forbidden.trailing_ones()` idiom works for ANY "lowest free slot in a bitmask" problem:
thread IDs, interrupt vectors, DMA channels, IPC slots — anywhere ≤128 items need round-robin slot assignment.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_edge_color_inner` (V3.08)
- `node_colors: [u128; MAX_NODES]` — 2KB stack, indexed by compact node index
- Vizing guarantees max colour index ≤ Δ ≤ 127, so u128 bitmask is always sufficient
- Compare: `graph_color_inner` (V2.48, node coloring) uses `forbidden: [bool; 256]` — O(Δ) scan

## From this session

V3.08: implemented greedy edge coloring. The `trailing_ones()` trick replaced what would have been a 0..255 scan loop, reducing the per-edge work to a single bitwise OR + hardware instruction.
Empirically verified on all 10 test cases including K_4 (Δ=3, χ'=3) and star K_{1,4} (Δ=4, χ'=4).
