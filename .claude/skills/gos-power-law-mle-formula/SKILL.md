---
name: gos-power-law-mle-formula
description: When implementing the Clauset-Newman-Shalizi power-law MLE (γ̂ = 1 + n/Σln(k)) in GOSKernel's no_std runtime, three rules apply: (1) with k_min=1, LN_TABLE[1]=0 so the sum collapses to Σ LN_TABLE[k_i] directly; (2) return gamma_ppm=0 when sum_ln=0 to signal the degenerate all-k=1 case; (3) n_fit × 10^12 / sum_ppm is safe in u64 (n ≤ 128). Apply in graph_power_law_inner and any future MLE-style estimators in crates/gos-runtime/src/lib.rs.
---

# Power-Law Exponent MLE: Three Integer Arithmetic Rules

## The rules

**Rule 1 — k_min=1 collapses the log-ratio sum:**
The CNS formula sums `ln(k_i / k_min)`. With k_min=1, `LN_TABLE[1]=0`, so the
subtraction disappears: `sum_ln_ppm = Σ LN_TABLE[k_i]` (no per-element subtract).

```rust
// k_min=1: LN_TABLE[1]=0, so ln(k_i/1) = ln(k_i), subtraction is free
sum_ln_ppm += LN_TABLE[k_capped] as u64;  // NOT: LN_TABLE[k] - LN_TABLE[k_min]
```

**Rule 2 — Degenerate case (all k=1) returns gamma_ppm=0:**
If every non-isolated node has degree 1, then `sum_ln_ppm = 0` and `γ̂ → ∞` (MLE undefined).
Return `(0, n_fit, n)` — gamma_ppm=0 signals "undefined", consistent with other GOSKernel
metrics that use 0 for impossible/degenerate cases (e.g., graph_girth returns u32::MAX for
acyclic, but u32::MAX would misrepresent a finite limit here).

```rust
if sum_ln_ppm == 0 {
    return (0, n_fit, n);  // 0 = undefined, NOT u32::MAX
}
```

**Rule 3 — u64 arithmetic is safe without u128:**
Unlike triple-ppm chains (which need u128, see gos-u128-ppm-chain-multiply), the MLE
formula `gamma_ppm = 1_000_000 + n_fit × 10^12 / sum_ln_ppm` stays in u64:
- max n_fit = MAX_NODES = 128
- max numerator = 128 × 10^12 = 1.28 × 10^14
- u64::MAX ≈ 1.84 × 10^19 → no overflow

```rust
let numer: u64 = (n_fit as u64).saturating_mul(1_000_000_000_000);
let gamma_ppm = 1_000_000u64 + numer / sum_ln_ppm;
```

## Why it's non-obvious

Three traps in one formula:
1. Looks like you need to subtract LN_TABLE[k_min] per element, but k_min=1 makes it free.
2. sum_ln=0 doesn't mean γ=1 — it means the sample is all-constant-k=1 and the MLE
   is genuinely undefined (all nodes are identical; no power-law tail to fit).
3. 10^12 looks like it needs u128, but the bounded n (≤128) keeps it in u64 range.

The third trap is dangerous because the formula *structurally* resembles V2.77's σ formula
(which does need u128), causing over-cautious u128 use that compiles but adds unnecessary
cost, or under-cautious u64 truncation if the bounds aren't checked.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_power_law_inner()` (V2.80)
- Isolated nodes (k=0) must be excluded before accumulation: `if k == 0 { continue; }`
  Track `n_fit` (nodes with k ≥ 1) separately from `n` (all alive nodes).
- LN_TABLE is the same 129-entry table as `graph_small_world_inner`; both embed it locally
  as a `const` inside the function body (no global constant in no_std crate).
- k ≥ 129 is impossible (MAX_NODES=128), but guard with `k.min(128)` for safety.
- Typical power-law exponents: γ ∈ [2, 3] (gamma_ppm ∈ [2_000_000, 3_000_000]).
  Pure stars have γ > 4; regular graphs have γ ∈ [1.5, 2].

## From this session

V2.80 (`graph_power_law`). Initial concern was whether `n_fit × 10^12` needed u128 (as in
V2.77's σ formula). Overflow analysis: 128 × 10^12 = 1.28×10^14 << u64::MAX; safe in u64.
The k_min=1 simplification was noticed during test derivation: test #3 (two nodes, k={1,1})
has sum_ln=0 (both LN[1]=0), so gamma must be 0 (degenerate), not a divide-by-zero crash.
