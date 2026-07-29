---
name: gos-abc-isqrt64-ratio-pattern
description: When computing ABC(G) = Σ √((s-2)/p) × 10^6 in no_std integer arithmetic, use isqrt64((s-2)×10^12/p) directly — NOT isqrt_ppm+reciprocal. This gives 707_106 for ratio=1/2 (not 707_107 as Randić does), because floor order matters: floor(sqrt(ratio×10^12)) ≠ floor(10^12/floor(sqrt(denominator×10^12))).
---

# ABC Index: Direct isqrt64 Ratio Pattern

## The rule

To compute `sqrt((s-2)/p) × 10^6` as a ppm integer for the Atom-Bond Connectivity index:

```rust
fn isqrt64(n: u64) -> u64 {
    // Returns floor(sqrt(n)) via Newton-Raphson — plain integer square root.
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}

// ABC contribution per undirected edge (a, b):
let da = deg[a];
let db = deg[b];
let p  = da * db;   // product
let s  = da + db;   // sum
// Pendant-pendant: s=2, (s-2)=0 → contribution=0 (same skip as AZI).
if s > 2 && p > 0 {
    let numer = (s - 2).saturating_mul(1_000_000_000_000u64);
    abc_acc += isqrt64(numer / p);
    // = floor(sqrt((s-2)/p × 10^12))
    // = floor(sqrt((s-2)/p) × 10^6)  ← the ABC ppm contribution
}
```

Overflow safety: max `s-2` = 254 (128+128-2); `254 × 10^12` = 2.54 × 10^14 < u64::MAX ✓.

## Why it's non-obvious

**The floor order is different from Randić, and it produces a different value for the same input:**

| Formula | Method | Result for ratio 1/2 |
|---------|--------|----------------------|
| Randić: `1/sqrt(p)` | `floor(10^12 / isqrt_ppm(p))` = `floor(10^12 / floor(sqrt(p×10^12)))` | **707_107** |
| ABC: `sqrt((s-2)/p)` | `isqrt64((s-2)×10^12 / p)` = `floor(sqrt((s-2)×10^12/p))` | **707_106** |

For ratio = 1/2:
- `isqrt64(500_000_000_000)` = **707_106**  
  - 707_106² = 499_998_895_236 < 5×10^11 ✓
  - 707_107² = 500_000_309_449 > 5×10^11 ✗
- Randić reciprocal: `isqrt_ppm(2) = 1_414_213`; `floor(10^12/1_414_213)` = **707_107** (different!)

**Graphs where all edges hit ratio (s-2)/p = 1/2 → 707_106 per edge:**
- P₃ outer edge (da=1, db=2): (1+2-2)/(1×2) = 1/2
- K₃ edge (da=db=2): (2+2-2)/(2×2) = 2/4 = 1/2
- P₄ all edges: outer same as P₃; inner (da=db=2) same as K₃
- K_{2,3} edge (da=3, db=2): (3+2-2)/(3×2) = 3/6 = 1/2

These numerically coincident ratios cause ABC_ppm to equal `n_edges × 707_106` for those graphs.

**Other ratios (not 1/2):**
- K_{1,4} edge (da=4, db=1): (5-2)/4 = 3/4 → `isqrt64(750_000_000_000)` = **866_025**
- K₄ edge (da=db=3): (6-2)/9 = 4/9 → `isqrt64(444_444_444_444)` = **666_666**

## GOSKernel context

- Used in `graph_topo_indices2_inner()` (V3.13, `crates/gos-runtime/src/lib.rs`)
- Companion indices in same function: H (floor(2_000_000/s), exact when s divides 2M) and F (Σ deg³, exact integer)
- Contrast with [[gos-isqrt-ppm-randic-pattern]]: Randić uses the isqrt_ppm + reciprocal chain; ABC uses isqrt64 directly on the ratio numerator

## From this session

V3.13 initial test values used 707_107 per ABC edge (from intuition "1/√2 × 10^6 ≈ 707_107"). Tests failed:

```
assertion `left == right` failed: K₃: ABC_ppm=2_121_321 (3×707_107; ABC=3/√2)
  left: 2121318
  right: 2121321
```

Correct value is `3 × 707_106 = 2_121_318`. The root cause is the direct isqrt64 formula floors at 707_106, not 707_107. Always pin ABC test values from `cargo test` output, not manual approximation.
