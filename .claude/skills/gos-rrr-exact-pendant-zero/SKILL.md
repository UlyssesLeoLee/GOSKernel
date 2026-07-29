---
name: gos-rrr-exact-pendant-zero
description: For RRR = Σ √((da-1)(db-1)), pendant edges (da=1 or db=1) give 0 automatically via isqrt64(0) — no explicit pendant skip needed. For Δ-regular graphs, RRR = m·(Δ-1)·10^6 exactly (no floor error) because isqrt64((Δ-1)²·10¹²) = (Δ-1)·10⁶ exactly.
---

# RRR: Zero-Numerator Pendant Auto-Skip and Exact Regular Invariant

## The rule

The Reduced Reciprocal Randić index: RRR = Σ_{uv∈E} √((da-1)(db-1)).

```rust
// RRR contribution per undirected edge (a, b):
let da = deg[a]; let db = deg[b];
let p1 = da - 1;  // da − 1 (= 0 if pendant endpoint)
let p2 = db - 1;  // db − 1 (= 0 if pendant endpoint)
// No explicit pendant skip needed: isqrt64(0) = 0 automatically.
rrr_acc += isqrt64(p1 * p2 * 1_000_000_000_000u64);
```

Overflow: max p1=p2=126; 126×126×10¹² = 1.59×10¹⁶ < u64::MAX ✓

## Key invariants

**Pendant auto-zero:** When da=1 → p1=0 → p1×p2=0 → isqrt64(0)=0. No branch needed.
- Star K_{1,k}: all edges pendant → RRR=0 always
- Any graph where every edge has at least one degree-1 endpoint → RRR=0

**Exact regular formula:** For Δ-regular: per edge (da-1)(db-1) = (Δ-1)² (perfect square).
- isqrt64((Δ-1)² × 10¹²) = (Δ-1) × 10⁶ **exactly** (no floor error)
- RRR = m × (Δ-1) × 10⁶  (exact integer, no ppm rounding)

```rust
// Exact cross-check for regular graphs:
assert_eq!(rrr_ppm, ec as u64 * (delta - 1) as u64 * 1_000_000,
           "regular: RRR = m·(Δ-1)·10^6 exactly");
```

| Graph | Δ | |E| | RRR_ppm formula | Value | Exact? |
|-------|---|-----|-----------------|-------|--------|
| Edge A-B (Δ=1) | 1 | 1 | m·0·10⁶ = 0 | 0 | exact ✓ |
| K₃ (Δ=2) | 2 | 3 | 3×1×10⁶ | 3_000_000 | exact ✓ |
| K₄ (Δ=3) | 3 | 6 | 6×2×10⁶ | 12_000_000 | exact ✓ |
| K_{1,4} (star) | — | 4 | all pendant | 0 | exact ✓ |
| K_{2,3} | — | 6 | per edge: √2×10⁶ | 6×1_414_213 | floor ≤1 ✓ |

## Why it's non-obvious

**Contrast with AZI (which needs explicit skip):** AZI = Σ (da·db/(da+db-2))³ has the expression `da+db-2` in the **denominator** — when da=db=1, this gives 0/0, so an explicit pendant skip is required. RRR has the pendant-degree factor in the **numerator** — when da=1, the numerator is 0, so division-by-zero cannot occur.

**Contrast with ABC/ABS (which have explicit skip comments):** ABC and ABS skip when `s-2=0` to note that the contribution is 0. For RRR, `p1×p2=0` gives the same result without any comment needed.

**Exact vs floor:** isqrt64((Δ-1)²×10¹²) is exact because (Δ-1) is an integer, so (Δ-1)²×10¹² is a perfect square times 10¹². Since 10⁶ is exact, (Δ-1)²×10¹² = ((Δ-1)×10⁶)². The Newton-Raphson isqrt64 converges exactly for perfect squares.

For non-regular edges (da≠db): e.g. K_{2,3} (da=3,db=2): (da-1)(db-1)=2; isqrt64(2×10¹²)=1_414_213 (floor of √2×10⁶). This is off by < 1 ppm.

## GOSKernel context

- Implemented in `graph_topo_indices6_inner` (V3.17, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo6` / `gtopo6` / `reduced reciprocal randic` / `grrr`
- Contrast: [[gos-sigma-regularity-exact-certificate]] (σ=0 iff regular); RRR=0 only means all pendant
- Contrast: AZI in `graph_topo_indices_inner` (V3.12) requires explicit skip for s-2=0
- RRR=0 with edge_count>0 → all edges are pendant (star topology or pendant chains)

## From this session

V3.17 test 06 (K_{1,4}, star): all edges da=4,db=1 → p2=(db-1)=0 → RRR=0 automatically ✓  
V3.17 test 08 (K₄, Δ=3): RRR=12_000_000=6×2×10⁶ exactly (isqrt64(4×10¹²)=2×10⁶ exact) ✓  
No special-case pendant skip was needed in the implementation; isqrt64(0)=0 handled it naturally.
