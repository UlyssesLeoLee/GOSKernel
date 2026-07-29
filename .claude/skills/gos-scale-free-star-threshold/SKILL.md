---
name: gos-scale-free-star-threshold
description: For a star graph with n spokes, the degree heterogeneity index κ crosses the "heterogeneous" threshold (κ > 2⟨k⟩) only when n ≥ 6. Use this when writing harness tests for graph_scale_free() or when building classification examples for the shell output.
---

# Star Graph Scale-Free Threshold: Minimum 6 Spokes

## The rule

A star graph with hub A and n spoke nodes (directed edges A→B₁…A→Bₙ) satisfies the
heterogeneity threshold **κ > 2⟨k⟩** only when **n ≥ 6**.

```
κ = ⟨k²⟩/⟨k⟩  for star with n spokes (n+1 total nodes):
  hub degree = n, spoke degree = 1
  sum_k  = n + n×1 = 2n
  sum_k² = n² + n×1 = n(n+1)
  κ      = n(n+1)/(2n) = (n+1)/2
  ⟨k⟩    = 2n/(n+1)
  κ/⟨k⟩  = (n+1)² / (4n)

Threshold κ > 2⟨k⟩:
  (n+1)²/(4n) > 2
  (n+1)² > 8n
  n² - 6n + 1 > 0
  n > (6 + √32)/2 ≈ 5.83  →  n ≥ 6
```

Key values for writing harness assertions:

| Spokes n | κ_ppm   | avg_k_ppm | 2×avg_k_ppm | κ > 2⟨k⟩? |
|----------|---------|-----------|-------------|-----------|
| 3        | 2_000_000 | 1_500_000 | 3_000_000   | ✗ (2 < 3) |
| 4        | 2_500_000 | 1_600_000 | 3_200_000   | ✗ (2.5 < 3.2) |
| 5        | 3_000_000 | 1_666_666 | 3_333_332   | ✗ (3 < 3.33) |
| 6        | 3_500_000 | 1_714_285 | 3_428_570   | ✓ (3.5 > 3.43) |

## Why it's non-obvious

Intuitively, any star graph looks like a scale-free network (one hub, many spokes). But
the κ > 2⟨k⟩ threshold quantifies the *degree* of heterogeneity — small stars don't have
enough hub-to-spoke degree contrast to cross it. Writing a test with 5 spokes and asserting
`kappa > 2 * avg_k` will fail at runtime even though the graph "feels" scale-free.

## GOSKernel context

- `gos_runtime::graph_scale_free()` → `(kappa_ppm, max_degree, avg_degree_ppm, node_count, m_undir)`
- Shell thresholds in `crates/k-shell/src/lib.rs` `dispatch_graph_scale_free()`:
  - `kappa_ppm > 3 × avg_degree_ppm` → "likely scale-free"
  - `kappa_ppm > 2 × avg_degree_ppm` → "heterogeneous"
  - else → "homogeneous (regular/random-like)"
- "likely scale-free" (κ > 3⟨k⟩) requires n > 9.9 → star with ≥10 spokes

## From this session

V2.78 harness test 10 used a 5-spoke star and asserted `kappa > 2*avg_k`. It failed:
`kappa=3000000 should exceed 2*avg_k=3333332`. Fixed by adding a 6th spoke (7 nodes total),
making kappa_ppm=3_500_000 > 2×avg_k_ppm=3_428_570.
