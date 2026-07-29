---
name: gos-u128-ppm-chain-multiply
description: When a GOSKernel formula multiplies three or more ppm (×1_000_000) values together before dividing, cast all operands to u128 before the first multiply — u64 overflows when two ~1_000_000 ppm values are multiplied and the product is then scaled by another 1_000_000. Apply in graph_small_world_inner and any future composite metric that chains ppm ratios in crates/gos-runtime/src/lib.rs.
---

# u128 intermediate for triple-ppm multiplication chains

## The rule

When computing a formula of the form `(A_ppm × B_ppm × SCALE) / (C_ppm × D_ppm)` where each value is a ppm quantity (up to ~1_000_000 or multi-million for larger graphs), cast all operands to `u128` before multiplication, then clamp the final result back to `u32` or `u64`:

```rust
let numer = (a_ppm as u128) * (b_ppm as u128) * 1_000_000u128;
let denom = (c_ppm as u128) * (d_ppm as u128);
let result = if denom == 0 { 0u32 } else {
    (numer / denom).min(u32::MAX as u128) as u32
};
```

## Why it's non-obvious

A single ppm value fits in u32 (max 1_000_000). Two ppm values multiplied together can produce ~10^12, which fits in u64. But when the product must be multiplied by a third scaling factor of 1_000_000, the intermediate reaches ~10^18 — right at the edge of u64::MAX (~1.8×10^19). Graphs with sigma > 1 push both `cc_ppm` and `l_rand_ppm` above 1_000_000 simultaneously, causing silent truncation or wrapping in release mode. The safe pattern is always u128 for any triple-product.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_small_world_inner()`: σ = (cc_ppm × l_rand_ppm × 1_000_000) / (cc_rand_ppm × l_ppm)
- The pattern will recur in any future composite metric that normalizes by two random-graph baselines simultaneously

## From this session

V2.77 `sigma_ppm` formula: cc_ppm (u32, ≤ 1_000_000) × l_rand_ppm (u64, can be 2_321_928 for n=5, k=2) × 1_000_000 → numerator ~2.3×10^18 before divide. Fits in u128 (~3.4×10^38), overflows u64 for sigma > 1 cases.
