---
name: gos-degree-entropy-histogram-pattern
description: When computing Shannon entropy H = -Σ p(d) ln p(d) of a degree distribution in GOSKernel's no_std runtime, use the histogram formula H = (1/n) Σ count[d] × (LN_TABLE[n] - LN_TABLE[count[d]]) to avoid per-node fraction arithmetic; also compute normalized_ppm = entropy_ppm × 10^6 / LN_TABLE[n] via u64. Apply in graph_entropy_inner and any future graph information-theoretic metrics in crates/gos-runtime/src/lib.rs.
---

# Shannon Entropy of Degree Distribution: Histogram Formula

## The rule

**Never compute p(d) = count[d]/n as a fraction** — integer division truncates it to 0 for small count[d] < n. Instead, transform H analytically:

```
H = -Σ p(d) ln p(d)
  = (1/n) Σ count[d] × (ln(n) − ln(count[d]))

entropy_scaled = Σ_{d: count[d]>0} count[d] × (LN_TABLE[n] − LN_TABLE[count[d]])
entropy_ppm    = entropy_scaled / n          -- H × 10^6 (floor, ≤1 ppm rounding error)
normalized_ppm = entropy_ppm × 10^6 / LN_TABLE[n]   -- H/ln(n) × 10^6, via u64
```

For the degree histogram, iterate `d` over `0..nc` (all possible degree values from 0 to n-1):

```rust
let nc_ln = LN_TABLE[nc.min(128)];
let mut entropy_scaled: u64 = 0;
for d in 0..nc {
    let cnt = deg_count[d];
    if cnt == 0 { continue; }
    let cnt_u = (cnt as usize).min(128);
    entropy_scaled += cnt as u64 * (nc_ln - LN_TABLE[cnt_u]) as u64;
}
let entropy_ppm = (entropy_scaled / nc as u64) as u32;

let normalized_ppm = if nc > 1 && nc_ln > 0 {
    (entropy_ppm as u64 * 1_000_000 / nc_ln as u64) as u32
} else {
    0
};
```

## Why it's non-obvious

Three traps:

1. **Fraction avoidance**: p(d) = count[d]/n as integers is always 0 when count[d] < n. The only exact approach is to keep numerator and denominator separate: `count[d] × (ln n − ln count[d])` accumulates before dividing by n.

2. **Both indices are safe**: count[d] ≤ nc ≤ 128, so `LN_TABLE[count[d]]` never overflows the 129-entry table. And `nc ≤ MAX_NODES = 128`, so `LN_TABLE[nc]` is also always valid.

3. **Overflow is impossible**: `entropy_scaled` ≤ nc × LN_TABLE[nc] ≤ 128 × 4,852,030 = 620M, fits u64. After dividing by nc: entropy_ppm ≤ 4,852,030, fits u32. The normalized intermediate `entropy_ppm × 10^6` ≤ 4.85 × 10^12, needs u64 but fits easily.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_entropy_inner()` (V3.10, line ~11300)
- `deg_count[d]` is built from `adj[ci].count_ones()` after the undirected bitmask construction
- Degree histogram indices: `d ∈ 0..nc` (nodes can have degree 0 up to nc-1)
- Regular graphs (all same degree) always give entropy_ppm = 0: `count[d]` = nc for one d, so `LN_TABLE[nc] - LN_TABLE[nc] = 0`
- `nc ≤ 1` → normalized_ppm = 0 (ln(1) = 0; H/ln(1) undefined)

## Exact cross-check: P₄ path (n=4 nodes)

P₄ has two equal groups: {deg-1: 2 nodes, deg-2: 2 nodes}:

```
entropy_scaled = 2 × (LN_TABLE[4] - LN_TABLE[2]) + 2 × (LN_TABLE[4] - LN_TABLE[2])
              = 4 × (1,386,294 - 693,147) = 4 × 693,147 = 2,772,588
entropy_ppm   = 2,772,588 / 4 = 693,147   (= ln(2) × 10^6, exact integer)
normalized_ppm = 693,147 × 10^6 / 1,386,294 = 500,000   (= 1/2 exactly; ln(4) = 2 ln(2))
```

This is the canonical cross-check: if your entropy_ppm ≠ 693,147 or normalized_ppm ≠ 500,000 for P₄, the formula has a bug.

## From this session

V3.10 (`graph_entropy`). The formula transformation from p(d) ln p(d) to count[d] × (ln n - ln count[d]) / n was derived to avoid fraction arithmetic. First run compiled and all 10 tests passed with exact integer assertions — no tolerance needed because the entire computation is deterministic integer arithmetic.
