---
name: gos-global-efficiency-ppm-formula
description: When implementing global graph efficiency in GOSKernel, accumulate 1_000_000/dist[t] across ALL pairs globally (not per-node like harmonic), then divide by n*(n-1) — the directed pair count. This is NOT n*(n-1)/2 (that's the rich-club undirected max). Guard n<2 before dividing. Apply in crates/gos-runtime/src/lib.rs graph_global_efficiency_inner.
---

# Global Efficiency: Pair-Normalised Reciprocal Sum Pattern

## The rule

E(G) = 1/(n*(n-1)) × Σ_{i≠j, d(i,j)<∞} 1/d(i,j)

After BFS from every source, accumulate `1_000_000 / dist[t]` into a single global sum,
then divide by `n*(n-1)` (directed pairs, no ÷2):

```rust
pub fn graph_global_efficiency_inner(&self) -> (u64, usize, usize) {
    // ... collect node_slots, node_count ...
    if node_count < 2 { return (0, 0, node_count); }   // guard: no pairs

    const SCALE: u64 = 1_000_000;
    let mut sum_recip: u64 = 0;

    for si in 0..node_count {
        let s = node_slots[si];
        // ... standard BFS, fills dist[] ...

        for ti in 0..node_count {
            let t = node_slots[ti];
            if t == s { continue; }
            if dist[t] != u32::MAX && dist[t] > 0 {
                sum_recip = sum_recip.saturating_add(SCALE / dist[t] as u64);
            }
        }
    }

    let pairs_max = node_count * (node_count - 1);   // directed: n*(n-1), NOT /2
    let efficiency_ppm = sum_recip / pairs_max as u64;
    (efficiency_ppm, pairs_max, node_count)
}
```

Return type: `(efficiency_ppm: u64, pairs_max: usize, node_count: usize)`.
- `efficiency_ppm` ∈ [0, 1_000_000] (0 = fully disconnected, 1_000_000 = complete directed)
- `pairs_max = n*(n-1)` — the denominator used; 0 when n < 2

## Why it's non-obvious

**Three similar metrics, three different normalizers:**

| Metric | Inner sum | Normalizer | Skill |
|--------|-----------|-----------|-------|
| Harmonic HC[v] (V2.71) | Σ 1e6/d(v,u) per node v | none (absolute score) | gos-harmonic-centrality-reciprocal-pattern |
| Rich-club ρ(k) (V2.68) | E among rich nodes | N*(N-1)/2 (undirected max) | gos-rich-club-ppm-formula |
| **Global efficiency E(G) (V2.74)** | Σ 1e6/d(i,j) all pairs | **n*(n-1)** (directed max) | **this skill** |

Rich-club uses `2_000_000` multiplier to absorb the `/2` from undirected max-edges.
Global efficiency uses `1_000_000` and normalises by `n*(n-1)` directly — no factor-of-two.

**`dist[t] > 0` guard is optional but defensive**: BFS sets `dist[s]=0` and the `t==s`
skip already handles the source. But unlike harmonic (which is provably safe after `t!=s`),
adding `dist[t] > 0` makes the division invariant explicit with zero code cost.

**No `reachable_pairs` counter needed**: unlike Wiener, which tracks reachable pairs as a
denominator for average path length, global efficiency normalises by `n*(n-1)` regardless —
unreachable pairs naturally contribute 0 to `sum_recip`.

**Saturation safety**: for n=128, d=1 for all pairs: sum = 128×127×1_000_000 = 16_256_000_000
which fits in u64 (max ~1.84×10¹⁹). No overflow risk; `saturating_add` is belt-and-suspenders.

## Boundary cases

| Graph | ppm | pairs_max |
|-------|-----|-----------|
| Empty (n=0) | 0 | 0 |
| Single node (n=1) | 0 | 0 |
| n≥2, no edges | 0 | n*(n-1) |
| One-way A→B (n=2) | 500_000 | 2 |
| Bidirectional K2 (n=2) | 1_000_000 | 2 |
| Directed 3-cycle (n=3) | 750_000 | 6 |
| Complete Kn (all d=1) | 1_000_000 | n*(n-1) |

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_global_efficiency_inner()` (V2.74)
- Public wrapper: `graph_global_efficiency() -> (u64, usize, usize)`
- Shell: `graph efficiency` / `graph eff` / `geff` / `global efficiency`
- VectorAddress L4=50 reserved for gos-graph-global-eff-harness test nodes
- Complexity: O(V × (V+E)) — identical to Wiener, harmonic, peripheral, center
- Printed as X.XXXXXX (6 decimal places from ppm) — see gos-kshell-u64-decimal-print

## From this session

V2.74: implemented `graph_global_efficiency_inner`. All 10 harness tests passed on first
compile. Key verification:
- Test 7 (directed 3-cycle): d=1 for {A→B, B→C, C→A}, d=2 for {A→C, B→A, C→B};
  sum = 3×1_000_000 + 3×500_000 = 4_500_000; ppm = 4_500_000/6 = 750_000 ✓
- Test 8 (K3 complete): 6 pairs d=1; sum=6_000_000; ppm=1_000_000 ✓ (E=1.0)
- Test 9 (disconnected {A→B}∥{C→D}): only 2 reachable pairs d=1; sum=2_000_000;
  ppm=2_000_000/12=166_666 ✓
