---
name: gos-rich-club-ppm-formula
description: When implementing the rich-club coefficient in GOSKernel's no_std runtime, use ρ_ppm = E_{>k} × 2_000_000 / (N_{>k} × (N_{>k}−1)) — the factor is 2_000_000 (not 1_000_000) because the maximum undirected edges among N nodes is N*(N-1)/2, and the ÷2 is absorbed into the numerator multiplier. Apply in graph_rich_club_inner and any future degree-threshold density metrics in crates/gos-runtime/src/lib.rs.
---

# Rich-Club Coefficient: Factor-of-Two in PPM Formula

## The rule

The rich-club coefficient is defined as:

```
ρ(k) = E_{>k} / [N_{>k} × (N_{>k}−1) / 2]
```

Rewriting for ppm (×1_000_000) without floating point:

```rust
// WRONG — missing the factor of 2 from the denominator's /2
let rho_ppm = ((e_rich as u64) * 1_000_000 / denom) as u32;

// CORRECT — multiply by 2_000_000 to absorb the /2 from max-edges formula
let denom   = (n_rich as u64) * ((n_rich as u64) - 1);   // N*(N-1), NOT N*(N-1)/2
let rho_ppm = ((e_rich as u64) * 2_000_000 / denom) as u32;
```

**Derivation:**
```
ρ(k) = E / [N*(N−1)/2]  = 2E / [N*(N−1)]
ρ_ppm = 2E × 1_000_000 / [N*(N−1)]  =  E × 2_000_000 / [N*(N−1)]
```

The denominator `denom = N*(N-1)` (no division by 2). The factor-of-2 lives in the numerator multiplier.

## Why it's non-obvious

The formula `ρ = E / max_edges` looks like a standard density formula using 1_000_000 ppm. But `max_edges` for **undirected** graphs is `N*(N-1)/2`, not `N*(N-1)`. Forgetting this halves the result — a clique would return 500_000 instead of 1_000_000.

The correct mental model: "multiply E by 2_000_000 to move the ÷2 into the numerator".

## Boundary cases

| Condition | Return value |
|-----------|-------------|
| Empty graph | (0, 0, 0) — early exit before division |
| N_rich = 0 | (0, 0, 0) — no rich nodes |
| N_rich = 1 | (0, 1, 0) — denom = 0, return 0 (undefined) |
| N_rich ≥ 2, E_rich = 0 | (0, N_rich, 0) |
| Rich nodes form a clique | (1_000_000, N_rich, N*(N-1)/2) |

**Always check `n_rich < 2` before computing denom** to avoid division by zero when `n_rich = 1` (denom = 1×0 = 0).

## Overflow analysis (no_std safe)

- e_rich ≤ MAX_EDGES = 512
- n_rich ≤ MAX_NODES = 128
- max numerator: 512 × 2_000_000 = 1,024,000,000 < u64::MAX
- min denominator (n_rich = 2): 2 × 1 = 2 (safe, never 0 after guard)

All arithmetic fits in u64; cast result to u32 (max value = 1_000_000 < 2³²).

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_rich_club_inner(snap, k)` (static method)
- Public API: `gos_runtime::graph_rich_club(k: u8) -> (u32, usize, usize)` (rho_ppm, rich_node_count, edges_among_rich)
- Shell: `graph rich club <k>` / `richclub <k>` / `grichclub <k>` → `dispatch_graph_rich_club`
- VectorAddress L4=44 reserved for gos-graph-rich-club-harness test nodes
- Directed edges treated as undirected (same pattern as modularity, assortativity, transitivity)
- Pure read, does NOT bump epoch

## From this session

V2.68: initial implementation verified with test 5 (K4, k=2): 6 edges among 4 rich nodes.
Hand-check: 6 × 2_000_000 / (4 × 3) = 12_000_000 / 12 = 1_000_000 ✓
Using 1_000_000 instead of 2_000_000 would yield 500_000, which fails the clique invariant.
