---
name: gos-ag-am-gm-regularity-invariant
description: For the arithmetic-geometric index AG = Σ (da+db)/(2√(da·db)), AG ≥ |E| always (AM ≥ GM), with equality iff every edge has da=db (regular graph). Use ag_ppm == edge_count × 1_000_000 as the regularity test — opposite direction from GA (GA ≤ |E|) but same equality condition.
---

# AG ≥ |E| (AM-GM Lower Bound on Per-Edge Ratio, Equality Iff Regular)

## The rule

The arithmetic-geometric index AG = Σ_{uv∈E} (da+db) / (2√(da·db)).

By the AM-GM inequality: (a+b)/2 ≥ √(ab), so (a+b)/(2√(ab)) ≥ 1.

Therefore:
- AG ≥ |E| **always** (each edge contributes ≥ 1)
- AG = |E| **if and only if** every edge has da = db, i.e. the graph is **regular**

In ppm representation:
```
ag_ppm == edge_count * 1_000_000   →  graph is regular (K_n, cycles, K_{r,r}, ...)
ag_ppm >  edge_count * 1_000_000   →  graph is irregular (stars, paths, K_{r,s} r≠s, ...)
```

Use as harness cross-checks:

```rust
// Regular graph K₄ (all da=db=3): AG must equal |E| exactly
assert_eq!(ag, ec as u64 * 1_000_000, "K₄: AG=m×10^6 (regular: AM=GM)");

// Irregular star K_{1,4}: AG must strictly exceed |E|
assert!(ag > ec as u64 * 1_000_000, "K_{{1,4}}: AG>m (non-regular: AM>GM)");
```

Also use as display annotation:
```rust
if edge_count > 0 && ag_ppm == edge_count as u64 * 1_000_000 {
    print_str(sink, "  (regular: AG=m)");
}
```

### Key computed values

| Graph        | AG_ppm    | = m? | Note                                       |
|--------------|-----------|------|--------------------------------------------|
| Edge A-B     | 1_000_000 | ✓    | da=db=1; (1+1)/(2√1)=1.0 exact             |
| K₃ (Δ=2)    | 3_000_000 | ✓    | 3×floor(4×10^12/4_000_000)=3×10^6          |
| K₄ (Δ=3)    | 6_000_000 | ✓    | 6×floor(6×10^12/6_000_000)=6×10^6 exact    |
| K_{1,4}     | 5_000_000 | ✗    | 4×1_250_000; (4+1)/(2√4)=5/4=1.25 **exact**|
| P₃           | 2_121_320 | ✗    | 2×1_060_660; s=3,p=2; floor bias ≤1 ppm    |
| K_{2,3}     | 6_123_726 | ✗    | 6×1_020_621; s=5,p=6; floor(5e12/4898978)  |

**Precision for regular graphs**: when da=db, p=da², isqrt64(p×10^12)=da×10^6 (exact when da is integer), so floor(s×10^12/(2×da×10^6)) = floor(2da×10^12/(2da×10^6)) = floor(10^6) = 10^6 exactly — no floor error for regular graphs.

## Why it's non-obvious

**AG and GA are exact duals that BOTH give regularity at the same boundary value (|E|), but from opposite sides:**

| Index | Formula | AM-GM direction | Bound | Equality |
|-------|---------|----------------|-------|---------|
| GA    | Σ 2√(da·db)/(da+db) | 2√(ab)/(a+b) ≤ 1 | GA ≤ |E| | da=db (regular) |
| AG    | Σ (da+db)/(2√(da·db)) | (a+b)/(2√(ab)) ≥ 1 | AG ≥ |E| | da=db (regular) |

Note: AG_per_edge × GA_per_edge = 1 for any edge — they are literal reciprocals! So GA = |E| ↔ AG = |E| ↔ regular graph.

The key confusion risk: both indices equal |E| for regular graphs, but AG *exceeds* |E| for irregular graphs while GA *is below* |E|. Checking `ag_ppm > ec * 1_000_000` means "irregular" (same conclusion as `ga_ppm < ec * 1_000_000`).

## GOSKernel context

- Implemented in `graph_topo_indices5_inner` (V3.16, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo5` / `gtopo5` / `gag` / `arithmetic geometric`
- AG uses: `ag_acc += (s * 1_000_000_000_000u64) / (2 * isqrt64(p * 1_000_000_000_000u64))`
- Contrast: [[gos-ga-regularity-invariant]] covers the dual GA ≤ |E| bound
- Contrast: [[gos-sdd-regularity-invariant]] covers SDD ≥ 2|E| (another AM-GM lower bound)

## From this session

V3.16 tests confirmed:
- K₃ (Δ=2 regular): ag=3_000_000=|E|×10^6 ✓ (per-edge: isqrt64(4×10^12)=2_000_000 exact)
- K₄ (Δ=3 regular): ag=6_000_000=|E|×10^6 ✓ (per-edge: isqrt64(9×10^12)=3_000_000 exact)
- K_{1,4} (non-regular): ag=5_000_000>4_000_000 ✓ (per-edge: (4+1)/(2√4)=5/4 exact since p=4 perfect square)
- K_{2,3} (non-regular): ag=6_123_726>6_000_000 ✓ (per-edge has floor error of +1 vs naive calc)
