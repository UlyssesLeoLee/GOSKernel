---
name: gos-deg-sum-sqrt-exact-pattern
description: For indices computing Σ √(da+db) × 10^6 via isqrt64((da+db)×10^12), the result is exact (no floor error) iff da+db is a perfect square (1, 4, 9, 16, 25, ...). Write exact assertions for K₃ (da+db=4→exact) but floor-value assertions for other graphs where da+db is not a perfect square.
---

# NI-Style Direct Sqrt Sum: Exact When da+db Is a Perfect Square

## The rule

For the Nirmala index NI = Σ_{uv∈E} √(da+db) and any similar "direct-sum-sqrt" topological index:

```rust
// NI contribution per edge:
ni_acc += isqrt64(s * 1_000_000_000_000u64);
// where s = da + db
```

`isqrt64(s × 10^12)` is **exact** (not floored) iff `s` is a perfect square:

| s = da+db | Perfect square? | isqrt64(s×10^12) | Exact? |
|-----------|----------------|-----------------|--------|
| 2 (da=db=1) | No (√2 irrational) | 1_414_213 | No (floor) |
| 3 (da=1,db=2) | No | 1_732_050 | No (floor) |
| 4 (da=db=2) | Yes (2²) | **2_000_000** | **Yes** |
| 5 (da=4,db=1 or da=3,db=2) | No | 2_236_067 | No (floor) |
| 6 (da=db=3) | No | 2_449_489 | No (floor) |
| 9 (da=db=4+1?) | Yes (3²) | 3_000_000 | Yes |
| 16 | Yes (4²) | 4_000_000 | Yes |

**Why:** `isqrt64(n)` = floor(√n). Since `10^12 = (10^6)²` is a perfect square, `isqrt64(s × 10^12) = isqrt64(s) × 10^6` exactly when `s` is a perfect square (then `√(s×10^12) = √s × 10^6 = integer`). When `s` is not a perfect square, √s is irrational and floor truncation applies.

**Test implication:** For K₃ (Δ=2 regular, all edges have da=db=2, s=4=2²):
- NI_ppm = 3 × 2_000_000 = 6_000_000 **exactly**
- This is also the `NI = |E|·√(2Δ)` regular invariant for Δ=2 (2Δ=4 perfect square)

For K₄ (Δ=3 regular, all edges have s=6, not a perfect square):
- NI_ppm = 6 × 2_449_489 = 14_696_934 **(floor)**
- Even though K₄ is regular, 2Δ=6 is not a perfect square → NI_ppm ≠ 6 × round(√6 × 10^6)

## Why it's non-obvious

The perfectness of `da+db` is a property of degree values, not of the graph class. Regular graphs DON'T automatically give exact NI — only those regular graphs where the degree Δ satisfies `2Δ` being a perfect square (Δ=2 → 2Δ=4 ✓; Δ=8 → 2Δ=16 ✓; Δ=3 → 2Δ=6 ✗).

This is distinct from:
- **Randić** (isqrt_ppm + reciprocal chain): exact when p = da·db is a perfect square — `gos-isqrt-ppm-randic-pattern`
- **ABC** (isqrt64 on ratio): exact when (s-2)/p is a perfect-square rational — `gos-abc-isqrt64-ratio-pattern`
- **NI** (isqrt64 on sum): exact when s = da+db is a perfect square — this skill

## GOSKernel context

- Used in `graph_topo_indices3_inner` (V3.14, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo3` / `gnirmala`
- Also applies to any future index computing Σ √(da+db) variants, e.g. Sombor-type indices
- Contrast: [[gos-abc-isqrt64-ratio-pattern]] uses isqrt64 on a ratio; [[gos-isqrt-ppm-randic-pattern]] uses it on a product

## From this session

V3.14 pre-calculation for K₄: confirmed 2_449_489² = 5_999_996_361_121 < 6×10^12 and 2_449_490² = 6_000_001_260_100 > 6×10^12. Used floor value 2_449_489 × 6 = 14_696_934 in test assertion (not 2_449_490).

V3.14 K₃ test: confirmed NI_ppm = 6_000_000 exactly (da+db=4, isqrt64(4×10^12)=2_000_000 exact — no floor). All three regular invariants held exactly for K₃.
