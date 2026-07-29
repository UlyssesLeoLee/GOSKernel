---
name: gos-centrality-arithmetic
description: In GOSKernel's fixed-point Brandes betweenness centrality implementation, two arithmetic rules are critical: (1) always multiply sigma[v] × (SCALE + delta[w]) BEFORE dividing by sigma[w] — never divide first; (2) sigma (shortest-path count) must be u64, not u32, to prevent overflow in layered graphs. Apply when reading, writing, or documenting graph_centrality_inner in crates/gos-runtime/src/lib.rs.
---

# Fixed-Point Brandes Centrality: Arithmetic Rules

## Rule 1 — Multiply before divide in the back-propagation recurrence

```rust
// WRONG — integer-truncates sigma[v]/sigma[w] to 0 in almost all cases
let contribution = (sigma[v] / sigma[w]) * (SCALE + delta[w]);

// CORRECT — multiply first, then divide
let contribution = sigma[v]
    .saturating_mul(SCALE.saturating_add(delta[w]))
    / sigma[w];
```

The formula is `δ[v] += σ[v] × (SCALE + δ[w]) / σ[w]`. Writing it as `(σ[v] / σ[w]) × ...` causes integer division to truncate the ratio to 0 when `sigma[v] < sigma[w]`, silently undercounting centrality for any graph with multiple shortest paths.

## Rule 2 — sigma must be u64, not u32

```rust
// WRONG — u32 can overflow in layered graphs
let mut sigma = [0u32; MAX_NODES];

// CORRECT
let mut sigma = [0u64; MAX_NODES];
```

In a layered graph where multiple nodes at each layer connect to all nodes in the next layer, the number of shortest paths compounds multiplicatively. With MAX_NODES=128 this is unlikely to overflow u32 in practice, but theoretically reachable — widening to u64 eliminates the risk entirely.

## Why it's non-obvious

The mathematical formula is usually written as a fraction: `σ(s,v) / σ(s,w) × (1 + δ(w))`. In floating-point this is unambiguous. In integer arithmetic, the order of operations matters critically: integer division truncates, so the fraction must be the *last* operation. Documentation and code comments that copy the mathematical notation directly will show the division-first form, which looks correct but isn't.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, function `graph_centrality_inner<N>`
- SCALE = 1_000_000 (fixed-point precision for graphs with multiple equal-length paths)
- `delta` and `bc_scaled` are `u64` (wide enough for scaled values)
- `sigma` widened from `u32` to `u64` in commit `49d7e9a` (V2.39 fix)

## From this session

CodeRabbit review on V2.39 hardening log (doc/HARDENING_LOG_2026-07-02_V2.39.md):
- Comment 3510512756: pseudocode showed `(sigma[v] / sigma[w]) × (SCALE + delta[w])` — wrong order. The actual code was correct; only the doc had the truncation-prone formula.
- Comment 3510512762: sigma was u32, widened to u64 to handle layered graph overflow.
