---
name: gos-abs-sum-denominator-isqrt64
description: When computing ABS = Σ √((da+db-2)/(da+db)) per edge, use isqrt64(q*10^12/s) where q=s-2 and s=da+db — NOT isqrt64 on product. Gives 707_106 (not 707_107) for K₃ edges because ABS uses sum s as denominator, while ABC uses product p; both produce 707_106 for ratio 1/2 but via different numerator/denominator pairs.
---

# ABS Index: Sum-Denominator isqrt64 Pattern

## The rule

To compute `sqrt((s-2)/s) × 10^6` per edge for the Atom-Bond Sum Connectivity index:

```rust
fn isqrt64(n: u64) -> u64 {
    if n == 0 { return 0; }
    let mut x = n; let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}

// ABS contribution per undirected edge (a, b):
let da = deg[a]; let db = deg[b];
let s = da + db;  // degree sum
let q = s - 2;    // s - 2 (= 0 when da=db=1 pendant pair)
// When q=0: isqrt64(0) = 0 naturally — no explicit skip needed.
abs_acc += isqrt64(q * 1_000_000_000_000u64 / s);
```

Overflow safety: max q = 252; 252 × 10¹² = 2.52 × 10¹⁴ < u64::MAX ✓

## Key precision values

| Edge type | da, db | s | q | q×10¹²/s | isqrt64 | ABS per edge |
|-----------|--------|---|---|-----------|---------|--------------|
| pendant-pair | 1, 1 | 2 | 0 | 0 | 0 | 0 (exact) |
| P₃ leaf | 1, 2 | 3 | 1 | 333_333_333_333 | **577_350** | 577_350 |
| K₃ / B-C in P₄ | 2, 2 | 4 | 2 | 500_000_000_000 | **707_106** | 707_106 |
| K_{1,4} / K_{2,3} | varies | 5 | 3 | 600_000_000_000 | **774_596** | 774_596 |
| K₄ | 3, 3 | 6 | 4 | 666_666_666_666 | **816_496** | 816_496 |

**Critical: 707_106, not 707_107.** Floor of √(1/2) × 10⁶ = 707_106 exactly:
- 707_106² = 499_998_895_236 < 500_000_000_000 ✓
- 707_107² = 500_000_309_449 > 500_000_000_000 ✗

## Why it's non-obvious

**ABS (sum denominator) vs ABC (product denominator):**

| Index | Formula | Denominator | Example K₃ (da=db=2) |
|-------|---------|-------------|----------------------|
| ABS   | √((s-2)/s)   | s = da+db = 4 | isqrt64(2×10¹²/4) = **707_106** |
| ABC   | √((s-2)/p)   | p = da·db = 4 | isqrt64(2×10¹²/4) = **707_106** |

For K₃ both happen to give the same value because s=4=p=4 when da=db=2. But they diverge for irregular graphs:

| Graph | da, db | ABS: isqrt64(q×10¹²/s) | ABC: isqrt64(q×10¹²/p) |
|-------|--------|------------------------|------------------------|
| P₃ leaf | 1, 2 | isqrt64(10¹²/3) = 577_350 | isqrt64(10¹²/2) = 707_106 |
| K₄ | 3, 3 | isqrt64(4×10¹²/6) = 816_496 | isqrt64(4×10¹²/9) = 666_666 |

**ABS_ppm is always < 10⁶ per edge** because q < s always (since q=s-2 < s for s ≥ 2), so ratio q/s < 1 and its square root < 1.

**Integer division ordering**: compute `q * 10^12 / s` (not `(q / s) * 10^12`) to avoid early truncation. The former gives floor(q·10¹²/s), which is what we want inside isqrt64.

## GOSKernel context

- Implemented in `graph_topo_indices6_inner` (V3.17, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo6` / `gtopo6` / `atom bond sum` / `gabs`
- Contrast with [[gos-abc-isqrt64-ratio-pattern]]: ABC uses product p in denominator; ABS uses sum s
- ABS = 0 for pendant-pair edges (q=0, both da=db=1) — isqrt64(0)=0 naturally

## From this session

V3.17 initial test computation: K₃ ABS predicted 2_121_321 (3×707_107), actual 2_121_318 (3×707_106). The floor of √(1/2)×10⁶ is 707_106, not 707_107. Pin test values from isqrt64 computation, not mental rounding.
