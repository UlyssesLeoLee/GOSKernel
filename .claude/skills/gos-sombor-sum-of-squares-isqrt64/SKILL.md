---
name: gos-sombor-sum-of-squares-isqrt64
description: The Sombor index SO = Σ √(da²+db²) uses isqrt64((da²+db²)×10^12) per edge. For Δ-regular graphs, (da²+db²)=2Δ² so SO_ppm = |E| × isqrt64(2Δ²×10^12). This is NEVER exact for integer Δ≥1 (would require √2 rational); all regular-graph SO assertions must use floor values, unlike NI which is exact when da+db is a perfect square.
---

# Sombor Index: isqrt64(Sum-of-Squares × 10^12), Never Exact for Integer-Δ Regular Graphs

## The rule

For the Sombor index SO = Σ_{uv∈E} √(da² + db²):

```rust
fn isqrt64(n: u64) -> u64 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}

// SO contribution per undirected edge (a, b):
so_acc += isqrt64((da * da + db * db) * 1_000_000_000_000u64);
// = floor(√(da²+db²) × 10^6)
```

**For Δ-regular graphs** (all da=db=Δ), each edge contributes:
```
isqrt64(2Δ² × 10^12) = isqrt64(2) × Δ × 10^6 (approx) = floor(Δ × √2 × 10^6)
```

This is **never exact** for integer Δ≥1 because √2 is irrational — so isqrt64(2Δ²×10^12) ≠ Δ×√2×10^6 (exact); it is always a floor value.

**Key isqrt64 values for SO (sum-of-squares input):**

| da, db | da²+db² | isqrt64(×10^12) | Note |
|--------|---------|----------------|------|
| 1, 1 | 2 | 1_414_213 | √2×10^6 floor |
| 1, 2 | 5 | 2_236_067 | √5×10^6 floor |
| 2, 2 | 8 | 2_828_427 | 2√2×10^6 floor; K₃ per-edge |
| 2, 3 | 13 | 3_605_551 | √13×10^6 floor; K_{2,3} per-edge |
| 1, 4 | 17 | 4_123_105 | √17×10^6 floor; K_{1,4} per-edge |
| 3, 3 | 18 | 4_242_640 | 3√2×10^6 floor; K₄ per-edge |

**Total SO_ppm values for test graphs:**

| Graph | SO_ppm | Computation |
|-------|--------|-------------|
| Edge A-B (da=db=1) | 1_414_213 | 1 × isqrt64(2×10^12) |
| P₃ | 4_472_134 | 2 × 2_236_067 (√5 each) |
| K₃ (Δ=2) | 8_485_281 | 3 × 2_828_427 (2√2 each) — floor |
| K_{1,4} | 16_492_420 | 4 × 4_123_105 (√17 each) |
| P₄ | 7_300_561 | 2 × 2_236_067 + 2_828_427 |
| K₄ (Δ=3) | 25_455_840 | 6 × 4_242_640 (3√2 each) — floor |
| K_{2,3} | 21_633_306 | 6 × 3_605_551 (√13 each) |

## Why it's non-obvious

**Contrast with NI (Nirmala) which IS exact for some regular graphs:**

| Index | Argument | Exact condition | K₃ (Δ=2) | K₄ (Δ=3) |
|-------|----------|----------------|-----------|-----------|
| NI | da+db = 2Δ | `2Δ` is a perfect square | **Exact** (2Δ=4=2²) | Floor (2Δ=6) |
| SO | da²+db² = 2Δ² | `2Δ²` is a perfect square | **Never** (2Δ²=8, √8 irrational) | **Never** (2Δ²=18) |

The SO argument `2Δ²` is a perfect square iff `√2·Δ` is rational iff Δ=0. So SO is never exact for integer-Δ regular graphs with any edges.

Overflow safety: max `da²+db² ≤ 2×127² = 32_258`; `32_258×10^12 = 3.23×10^16` < u64::MAX ≈ 1.8×10^19 ✓.

Compare with the three sibling degree-based isqrt64 patterns:
- **Randić**: `isqrt_ppm(p)` then reciprocal — see [[gos-isqrt-ppm-randic-pattern]]
- **NI**: `isqrt64(s × 10^12)` where s = da+db — see [[gos-deg-sum-sqrt-exact-pattern]]
- **ABC**: `isqrt64((s-2)×10^12 / p)` — see [[gos-abc-isqrt64-ratio-pattern]]
- **SO**: `isqrt64((da²+db²) × 10^12)` — this skill

## GOSKernel context

- Implemented in `graph_topo_indices4_inner` (V3.15, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo4` / `gsombor` / `sombor index`
- SO is the Euclidean norm of the (da, db) degree pair; high SO = large hub asymmetry
- All harness SO test assertions must use pinned floor values — never compute `Δ × √2 × 10^6` and round

## From this session

V3.15 design: initially considered whether K₃ or K₄ SO might be exact (since they are for NI). Verified analytically that 2Δ² is never a perfect square for Δ≥1 (√2 irrational), so all regular-graph SO assertions use isqrt64 floor values: K₃=8_485_281, K₄=25_455_840 (both floors, not exact).
