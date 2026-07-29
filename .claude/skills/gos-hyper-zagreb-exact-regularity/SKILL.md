---
name: gos-hyper-zagreb-exact-regularity
description: The Hyper-Zagreb indices HM₁ = Σ (da+db)² and HM₂ = Σ (da·db)² are exact integers with no floor error. For Δ-regular graphs, HM₁ = 4·|E|·Δ² and HM₂ = |E|·Δ⁴. Use these as exact cross-check assertions: `hm1 == 4 * ec * delta_sq` and `hm2 == ec * delta_4th`.
---

# HM₁ = 4|E|Δ² and HM₂ = |E|Δ⁴ for Δ-Regular Graphs (Exact Integer Invariants)

## The rule

The Hyper-Zagreb indices are exact sums of perfect squares — no isqrt, no floor:

```rust
// HM₁ and HM₂ contribution per undirected edge (a, b):
let s = da + db;  // sum
let p = da * db;  // product
hm1_acc += s * s;   // (da+db)²  ← exact integer
hm2_acc += p * p;   // (da·db)²  ← exact integer
```

For Δ-regular graphs (all da = db = Δ):
- Each edge: s = 2Δ, so s² = 4Δ² → **HM₁ = |E| × 4Δ²**
- Each edge: p = Δ², so p² = Δ⁴  → **HM₂ = |E| × Δ⁴**

Use as exact harness assertions:

```rust
// K₃ (Δ=2, |E|=3): HM₁ = 4×3×4 = 48; HM₂ = 3×16 = 48
assert_eq!(hm1, 4 * ec as u64 * 4, "K₃: HM1=4·|E|·Δ²=48");
assert_eq!(hm2, ec as u64 * 16,    "K₃: HM2=|E|·Δ⁴=48");

// K₄ (Δ=3, |E|=6): HM₁ = 4×6×9 = 216; HM₂ = 6×81 = 486
assert_eq!(hm1, 4 * ec as u64 * 9, "K₄: HM1=4·|E|·Δ²=216");
assert_eq!(hm2, ec as u64 * 81,    "K₄: HM2=|E|·Δ⁴=486");
```

### Cross-check table

| Graph        | Δ | |E| | HM₁ = 4|E|Δ² | HM₂ = |E|Δ⁴ | Exact? |
|--------------|---|-----|--------------|-------------|--------|
| Edge A-B     | 1 |  1  | 4            | 1           | ✓      |
| K₃           | 2 |  3  | 48           | 48          | ✓      |
| K₄           | 3 |  6  | 216          | 486         | ✓      |
| C₄ (cycle-4) | 2 |  4  | 64           | 64          | ✓      |
| K_{1,4} (non-reg) | — | 4 | 100 ≠ 4×4×1=16 | 64 ≠ 4×1=4 | N/A (irregular) |
| K_{2,3} (non-reg) | — | 6 | 150 ≠ any  | 216 ≠ any  | N/A    |

**For irregular graphs, use exact per-edge computation** (no shortcut formula):
- K_{1,4}: each edge (da=4, db=1): s=5, p=4; HM₁=4×25=100; HM₂=4×16=64
- K_{2,3}: each edge (da=3, db=2): s=5, p=6; HM₁=6×25=150; HM₂=6×36=216

## Why it's non-obvious

HM₁ and HM₂ are **not** the same as M₁ and M₂:
- M₁ = Σ (da+db)     (first Zagreb — linear)
- M₂ = Σ (da×db)     (second Zagreb — product, not squared)
- **HM₁ = Σ (da+db)²** (first Hyper-Zagreb — sum-squared)
- **HM₂ = Σ (da×db)²** (second Hyper-Zagreb — product-squared)

The "Hyper" prefix means squaring the per-edge contribution, not applying any new index type. The regularity formulas follow directly:
- HM₁ regular: each s=2Δ, so s²=4Δ²; sum over |E| edges → 4Δ²|E|
- HM₂ regular: each p=Δ², so p²=Δ⁴; sum over |E| edges → Δ⁴|E|

**HM₁ = HM₂ for regular Δ=2 (e.g. K₃, C₄)**:
- K₃: HM₁ = 4×3×4 = 48 = HM₂ = 3×16 = 48 (coincidence: 4Δ² = Δ⁴ when Δ=2)
- This coincidence disappears for Δ≠2: K₄ has HM₁=216 ≠ HM₂=486

**Overflow safety**: max s=254, s²=64516; max p=127²=16129, p²≈260M; times 512 edges:
- HM₁ ≤ 64516×512 ≈ 33M — well within u64
- HM₂ ≤ 260M×512 ≈ 133B — well within u64

## GOSKernel context

- Implemented in `graph_topo_indices5_inner` (V3.16, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo5` / `gtopo5` / `ghm1` / `ghm2` / `hyper zagreb`
- Contrast with M₁/M₂ (V3.11): those are linear; HM₁/HM₂ are squared — different magnitudes
- HM₁ and HM₂ have no precision issues (pure exact integer arithmetic); assertions can use exact formulas
- Related: [[gos-ag-am-gm-regularity-invariant]] for the companion AG index in graph_topo_indices5

## From this session

V3.16 test 5 (K₃, Δ=2, |E|=3):
- hm1=48 = 4×3×4 ✓; hm2=48 = 3×16 ✓ (coincidentally equal since Δ=2)

V3.16 test 8 (K₄, Δ=3, |E|=6):
- hm1=216 = 4×6×9 ✓; hm2=486 = 6×81 ✓ (now distinct: 216 ≠ 486)

The coincidence HM₁=HM₂ for K₃ was noticed and verified as specific to Δ=2 (4Δ²=Δ⁴ → 4=Δ²/1 → Δ²=4 → Δ=2).
