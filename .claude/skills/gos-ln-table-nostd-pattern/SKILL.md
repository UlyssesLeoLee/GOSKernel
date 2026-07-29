---
name: gos-ln-table-nostd-pattern
description: When a no_std GOSKernel graph metric needs ln(x) for x in 1..=128 (e.g. to compute log-ratio baselines like L_rand = ln(n)/ln(k)), use a compile-time const table LN_TABLE: [u32; 129] storing ln(x) × 1_000_000. Apply whenever implementing E-R random-graph baselines or any metric requiring a natural log in crates/gos-runtime/src/lib.rs.
---

# Compile-time ln table for no_std fixed-point math

## The rule

Define `const LN_TABLE: [u32; 129]` inside the function (or as a module-level const) storing `⌊ln(x) × 1_000_000⌋` for x ∈ 0..=128. Index 0 is unused (ln(0) is undefined); index 1 is 0 (ln(1) = 0). Guard the index before use: if `n == 0 || n > 128` or `k < 2` (since LN_TABLE[1] = 0 would cause division by zero), return 0 and skip the computation.

## Why it's non-obvious

`no_std` forbids `f64::ln()` and the `libm` float library. Simple integer approximations (Bhaskara I, Newton series) require many multiply-divide steps and still lose precision. A precomputed table is both simpler and more accurate: it's exact (truncated to 6 decimal places), zero-cost at runtime, and fits entirely in the function body as a compile-time constant.

The critical guard: `LN_TABLE[1] = 0` because ln(1) = 0. If `avg_k = 1` (sparse graphs where 2m/n rounds to 1), dividing by LN_TABLE[1] = 0 is a divide-by-zero. Always check `avg_k >= 2` before indexing.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_small_world_inner()` at line ~3871
- MAX_NODES = 128, so the table covers all possible n and ⟨k⟩ values in this runtime
- The table is defined as a local `const` inside the function, avoiding any global namespace pollution

## From this session

V2.77 (`graph_small_world`) needed `L_rand ≈ ln(n)/ln(⟨k⟩)` where ⟨k⟩ = 2m/n. Both n and ⟨k⟩ are bounded by MAX_NODES=128, making a 129-entry table sufficient and exact.

```rust
const LN_TABLE: [u32; 129] = [
    0,
    0,         693_147, 1_098_612, 1_386_294, 1_609_437,
    // ... 128 entries total ...
    4_836_281, 4_844_187, 4_852_030,
];
let avg_k = (2 * m_undir) / n;
if avg_k < 2 || avg_k >= LN_TABLE.len() { return (0, ...); }
let ln_n = LN_TABLE[n.min(128)] as u64;
let ln_k = LN_TABLE[avg_k] as u64;
let l_rand_ppm: u64 = (ln_n * 1_000_000) / ln_k;
```
