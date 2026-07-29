---
name: gos-undirected-dedup-seen-adj
description: When building an undirected edge projection from a directed GOSKernel graph (for min-cut, k-truss, bipartite check, etc.), deduplicate A→B and B→A into one undirected edge using `seen_adj[min_ci]: u128` with bit `max_ci` — normalize (a,b) so a<b first, then check and set the bitmask before adding the edge. Total stack: 128×16 = 2KB.
---

# Undirected Projection Dedup: seen_adj u128 Bitmask

## The rule

When converting a directed graph to its undirected projection, use a per-row u128 bitmask to skip already-seen pairs:

```rust
let mut seen_adj = [0u128; MAX_NODES]; // 2KB
for ei in 0..MAX_EDGES {
    // ... get fci, tci ...
    if fci == tci { continue; } // skip self-loops
    let (a, b) = if fci < tci { (fci, tci) } else { (tci, fci) }; // normalize a < b
    if b >= 128 { continue; } // safety: can't shift by ≥128
    if (seen_adj[a] >> b) & 1 != 0 { continue; } // already added
    seen_adj[a] |= 1u128 << b;
    // add undirected edge {a, b} to list
}
```

## Why it's non-obvious

Without the normalization `(a, b) = if fci < tci { ... }`, the bitmask check for A→B and B→A would use different rows (`seen_adj[A]` vs `seen_adj[B]`). After normalization (always min first), both directions hash to the same `seen_adj[min] bit max` location.

The `b >= 128` guard is critical: `1u128 << 128` panics in debug mode (shift overflow). Since `b = max(fci, tci)` and compact indices run `0..nc` where `nc ≤ MAX_NODES = 128`, `b` can equal 127 (safe) but theoretically never reaches 128 — the guard makes this explicit.

## GOSKernel context

Used in `graph_min_cut_inner<N>` (V3.02). Also applies when building undirected adjacency for truss decomposition, bipartite checking, k-core, any algorithm needing the undirected projection of the directed runtime graph.

Alternative (for very dense graphs): `[[bool; MAX_NODES]; MAX_NODES]` = 16KB but 8× larger. The u128 approach is 2KB and avoids the N×N layout entirely.

## From this session

V3.02: the directed graph can have both A→B and B→A as separate edges. For min-cut, these should count as ONE undirected edge (weight 1), not two (weight 2). The seen_adj bitmask ensures this without any O(E²) comparison scan during initialization.
