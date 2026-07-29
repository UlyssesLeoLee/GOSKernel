---
name: gos-sdd-regularity-invariant
description: For the symmetric division degree index SDD = Σ (da²+db²)/(da·db), SDD ≥ 2|E| always (AM-GM lower bound), with equality iff the graph is regular. Use sdd_ppm == edge_count × 2_000_000 as an exact test invariant for regular graphs and sdd_ppm > edge_count × 2_000_000 to confirm irregular graphs.
---

# SDD ≥ 2|E| (AM-GM Lower Bound, Equality Iff Regular)

## The rule

The symmetric division degree index: SDD = Σ_{uv∈E} (da² + db²) / (da · db) = Σ (da/db + db/da).

By AM-GM inequality, a/b + b/a ≥ 2, with equality iff a = b.

Therefore:
- SDD ≥ 2|E| **always** (since each edge contributes ≥ 2)
- SDD = 2|E| **if and only if** every edge (u,v) has deg(u) = deg(v), i.e. the graph is **regular**

In ppm representation:
```
sdd_ppm == edge_count * 2_000_000   →  graph is regular (K_n, cycles, K_{r,r}, ...)
sdd_ppm >  edge_count * 2_000_000   →  graph is irregular (stars, paths, K_{r,s} r≠s, ...)
```

Use both as harness cross-checks:

```rust
// Regular graph K₃ (Δ=2, 3 edges): SDD must equal 2|E| exactly
assert_eq!(sdd, 2 * ec as u64 * 1_000_000,
    "K₃: SDD = 2|E| exactly (regular AM-GM equality)");

// Non-regular star K_{1,4}: SDD must strictly exceed 2|E|
assert!(sdd > 2 * ec as u64 * 1_000_000,
    "K_{{1,4}}: SDD > 2|E| (non-regular; AM-GM strict inequality)");
```

Implementation note: SDD per edge = floor((da²+db²) × 10^6 / (da·db)). For regular graphs (da=db), (da²+db²)/(da·db) = 2da²/da² = 2 exactly, so the floor has no effect.

## Why it's non-obvious

SDD and GA are **dual** AM-GM bounds:
- GA = Σ 2√(da·db)/(da+db) ≤ |E| (AM-GM upper bound; equality iff regular) — see `gos-ga-regularity-invariant`
- SDD = Σ (da²+db²)/(da·db) ≥ 2|E| (AM-GM lower bound; equality iff regular)

Both use AM-GM but in opposite directions. GA checks whether a metric is at its **maximum** (|E|) for regular graphs; SDD checks whether a metric is at its **minimum** (2|E|) for regular graphs. Getting these confused leads to wrong test assertions (checking `sdd == edge_count × 1_000_000` instead of `× 2_000_000`).

## GOSKernel context

- Implemented in `graph_topo_indices3_inner` (V3.14, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo3` / `gtopo3` / `gsdd`
- Display annotates SDD with "≡2|E| (regular)" when equality holds
- Contrast: [[gos-ga-regularity-invariant]] covers the dual GA ≤ |E| bound

## From this session

V3.14 test 5 (K₃, regular Δ=2): confirmed SDD_ppm = 6_000_000 = 2×3×10^6 = 2|E|×10^6 ✓
V3.14 test 6 (K_{1,4}, non-regular): confirmed SDD_ppm = 17_000_000 > 8_000_000 = 2|E|×10^6 ✓
V3.14 test 10 (K_{2,3}, non-regular): confirmed SDD_ppm = 12_999_996 > 12_000_000 = 2|E|×10^6 ✓
