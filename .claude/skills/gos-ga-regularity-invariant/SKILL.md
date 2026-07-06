---
name: gos-ga-regularity-invariant
description: For the geometric-arithmetic index GA = Σ 2√(da·db)/(da+db), GA = |E| iff the graph is regular (all nodes same degree). Use ga_ppm == edge_count × 1_000_000 as a test cross-check for regular-graph cases, and ga_ppm < edge_count × 1_000_000 to confirm irregular graphs.
---

# GA = |E| iff Regular Graph (Geometric-Arithmetic Invariant)

## The rule

The geometric-arithmetic index GA = Σ_{uv∈E} 2√(deg(u)·deg(v)) / (deg(u)+deg(v)).

By AM-GM inequality, 2√(ab)/(a+b) ≤ 1, with equality iff a = b.

Therefore:
- GA = |E| **if and only if** every edge (u,v) has deg(u) = deg(v), i.e. the graph is **regular**.
- GA < |E| for any non-regular graph.

In ppm representation:
```
ga_ppm == edge_count * 1_000_000   →  graph is regular (K_n, cycles, K_{r,r}, ...)
ga_ppm <  edge_count * 1_000_000   →  graph is irregular (stars, paths, K_{r,s} r≠s, ...)
```

Use as a harness cross-check:

```rust
// Regular graph (K₄, all deg=3): GA must equal edge_count
assert_eq!(ga,  6_000_000, "K₄: GA_ppm=|E|×10^6 (regular graph)");

// Bipartite K_{2,3} (deg 3 and 2 mixed): GA must be strictly less
assert_ne!(ga, ec as u64 * 1_000_000,
    "K_{{2,3}} is not regular so GA_ppm must differ from |E|×10^6");
```

## Why it's non-obvious

AM-GM gives a per-edge bound but the equality condition (a=b) translates to a global graph property only at the graph level — it holds for all edges simultaneously iff the graph is **regular** (not just if some edge has equal endpoint degrees). Students often check individual edge contributions instead of the graph-global condition.

Also: GA uses the *sum* a+b in the denominator, not the product — unlike the Randić index which uses √(a·b) directly. The AM-GM bound (2√(ab)/(a+b) ≤ 1) applies specifically to this ratio form.

## GOSKernel context

- Implemented in `graph_topo_indices_inner` (V3.12, `crates/gos-runtime/src/lib.rs`)
- Shell: `graph topo` / `gtopo`
- Related: [[gos-isqrt-ppm-randic-pattern]] for the isqrt computation shared by SC/GA/AZI
- K_{r,r} regular bipartite graphs also satisfy GA=|E|; K_{r,s} with r≠s do not

## From this session

V3.12 test 8 (K₄) verified GA_ppm = 6_000_000 = 6 × 10^6 = |E| × 10^6 ✓
V3.12 test 10 (K_{2,3}) verified GA_ppm = 5_878_770 < 6_000_000 — confirmed non-regular.
Analytical: K_{2,3} GA = 6 × 2√6/5 = 12√6/5 ≈ 5.87877 → GA_ppm ≈ 5_878_770. ✓
