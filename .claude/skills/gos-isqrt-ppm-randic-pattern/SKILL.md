---
name: gos-isqrt-ppm-randic-pattern
description: When computing the Randić index R(G) = Σ 1/√(deg(u)×deg(v)) in no_std integer arithmetic, use isqrt_ppm(p) = floor(sqrt(p × 10^12)) via Newton-Raphson, then contribution = floor(10^12 / isqrt_ppm(p)). Gives 6-digit precision with ≤1 ppm floor error per edge. Apply in graph_zagreb_inner and any future reciprocal-sqrt ppm metric.
---

# isqrt_ppm: Six-Digit Precision Reciprocal-Sqrt for Randić Index

## The rule

To compute `1/sqrt(p)` as a ppm integer without float:

```rust
fn isqrt_ppm(p: u64) -> u64 {
    // Returns floor(sqrt(p × 10^12)) — six decimal digits of sqrt(p).
    if p == 0 { return 0; }
    let n = p.saturating_mul(1_000_000_000_000u64);  // p × 10^12; max p=16129, no overflow
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x  // = floor(sqrt(p) × 10^6)
}

// Contribution of edge (u,v) to R_ppm:
let p = deg[u] as u64 * deg[v] as u64;
let s = isqrt_ppm(p);                          // floor(sqrt(p) × 10^6)
let contribution = if s > 0 { 1_000_000_000_000u64 / s } else { 0 };
// contribution ≈ floor(10^6 / sqrt(p))  with ≤1 ppm error
randic_acc += contribution;
```

Final: `randic_ppm = randic_acc as u32` (safe: max R_ppm < 512_000_000 for ≤512 edges).

## Why it's non-obvious

Two compound floors make manual pre-calculation unreliable:

1. `isqrt_ppm(2) = floor(sqrt(2_000_000_000_000)) = 1_414_213`  
   (since √2 × 10^6 = 1_414_213.56...)
2. `floor(10^12 / 1_414_213) = 707_107`  
   (not 707_106 as naive mental math suggests — verify: 1_414_213 × 707_107 = 999_999_911_791 < 10^12 ✓)

The floor in step 1 shifts the denominator down slightly, making the overall fraction slightly larger. Always pin Randić test values from actual `cargo test` output, not pencil-and-paper approximations.

**Floor error characterization:**
- `p = 1`: exact (both floors hit integers; contribution = 1_000_000 exactly)
- `p = 4, 9, 16, ...` (perfect squares): exact (sqrt is an integer × 10^6)
- `p = 2`: contribution = 707_107 (floor error = +0.5 ppm vs actual 707_106.78...)
- `p = 6`: contribution = 408_248 per edge (floor error negligible; 6 × 408_248 = 2_449_488 vs actual 2_449_490)

## GOSKernel context

- First used in `graph_zagreb_inner` (V3.11, `crates/gos-runtime/src/lib.rs`)
- Distinct from the `LN_TABLE` approach used by entropy/power-law: those use precomputed ln values; this uses runtime Newton-Raphson
- Overflow safety: `p ≤ 127² = 16_129`; `16_129 × 10^12 = 1.6 × 10^16` < `u64::MAX ≈ 1.8 × 10^19` ✓
- Max `randic_acc`: 512 edges × max_contribution ≈ 512 × 1_000_000 = 512_000_000 < u32::MAX ✓

## From this session

V3.11 test failures for P₃ and P₄: pinned 1_414_212 and 1_914_212 respectively based on hand calculation, but actual values were 1_414_214 and 1_914_214. The error was computing `floor(10^12/1_414_213) = 707_106` when the real floor is 707_107. Fixed by pinning from cargo test failure output.

See [[gos-ppm-assertion-pin-from-runtime]] for the general principle of always pinning ppm values from runtime output, not manual arithmetic.
