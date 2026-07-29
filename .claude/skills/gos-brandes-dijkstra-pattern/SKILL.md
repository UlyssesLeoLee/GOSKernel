---
name: gos-brandes-dijkstra-pattern
description: When implementing Brandes weighted betweenness centrality with O(V²) scan-for-min Dijkstra (no heap) in GOSKernel, the Dijkstra extraction stack IS the Brandes S-stack — no separate sort needed. Also: sigma must REPLACE (not add) on strictly-shorter paths, and ADD (not replace) on equal-weight paths. Apply whenever writing or reviewing graph_between_inner or any weighted Brandes variant in crates/gos-runtime/src/lib.rs.
---

# Brandes + O(V²) Dijkstra: Extraction Stack = Brandes S-Stack

## The rule

When using the O(V²) scan-for-min Dijkstra (each node extracted exactly once
in non-decreasing distance order), the extraction order array is **directly
usable as the Brandes S-stack** for back-propagation — no separate sort or
secondary data structure needed:

```rust
let mut stk     = [0usize; MAX_NODES];
let mut stk_len = 0usize;

for _ in 0..node_count {
    // O(V²) scan for minimum unvisited node
    let mut u = usize::MAX;
    let mut u_dst = f32::MAX;
    for ni in 0..node_count {
        let sl = node_slots[ni];
        if !visited[sl] && dist[sl] < u_dst { u = sl; u_dst = dist[sl]; }
    }
    if u == usize::MAX || u_dst >= f32::MAX { break; }
    visited[u] = true;

    stk[stk_len] = u;   // ← extraction order IS non-decreasing distance order
    stk_len += 1;

    // relax out-edges from u ...
}

// Back-propagation: stk reversed = nodes in non-increasing distance order
for bi in (0..stk_len).rev() {
    let w = stk[bi];
    // ... Brandes back-prop here
}
```

This works because O(V²) Dijkstra extracts each node exactly once at its
final minimum distance.  Heap-based Dijkstra is NOT equivalent because it
can push a node multiple times and pop stale entries — requiring explicit S maintenance.

## Sigma: replace vs accumulate

The sigma counting rule in weighted Brandes is:

```rust
let nd = u_dst + w;  // candidate new distance to v via u
if nd < dist[v] - EPS {
    // Strictly shorter: replace path count (old shortest paths are superseded)
    dist[v]  = nd;
    sigma[v] = sigma[u];   // ← REPLACE, not +=
} else if (nd - dist[v]).abs() <= EPS && dist[v] < f32::MAX {
    // Equal-weight parallel path: accumulate
    sigma[v] = sigma[v].saturating_add(sigma[u]);  // ← ADD, not replace
}
```

The two conditions must be **mutually exclusive** (epsilon guards enforce this).
The `dist[v] < f32::MAX` guard in the second branch prevents false positives
when both `nd` and `dist[v]` are infinite (unreachable nodes).

## Predecessor detection in back-propagation

A node v is a predecessor of w iff `dist[v] + edge_weight(v→w) ≈ dist[w]`:

```rust
if (dist[v] + ew - dist[w]).abs() > EPS { continue; }
```

No explicit predecessor list needed — scan all in-edges of w and check the
distance condition.  This mirrors how graph_centrality_inner (V2.39) identifies
predecessors via `dist[w] == dist[v].saturating_add(1)` in the BFS version.

## Why it's non-obvious

In the textbook Brandes algorithm, S is built by pushing nodes as their
shortest-path distance is finalized.  With heap Dijkstra, a node can be
"finalized" after several relaxations, requiring a separate push to S.
With O(V²) scan-for-min, "first extraction = finalization" is guaranteed
because no node can be re-extracted.  The extraction loop body is the
"finalization event" for that node — recording it in stk[] is sufficient.

The sigma replacement rule (strictly shorter → replace) is often missed:
naive implementations always do `sigma[v] += sigma[u]`, which double-counts
the first shortest path to v (adds sigma[u] on discovery *and* on equal-check).

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_between_inner<N>` (V2.53)
- EPS = 1e-6 (coarser than graph_flow's 1e-9 — adequate for Dijkstra path equality)
- sigma type: `u64` (see gos-centrality-arithmetic — must be u64, never u32)
- Arithmetic rule: multiply before divide in back-prop (see gos-centrality-arithmetic)
- graph_between_inner uses `&self` (not snapshot) — consistent with graph_centrality_inner

## From this session

V2.53 `graph between`: implemented Brandes + Dijkstra in graph_between_inner.
All 10 harness tests passed on first compile — no debugging needed — because
the stack-equivalence and sigma rules were applied correctly from the start.
Test 6 (weight-sensitive) specifically verifies the Dijkstra divergence from BFS:
A→C (w=0.5), C→B (w=0.5), A→B (w=2.0) → WBC[C]=1 (Dijkstra picks indirect),
whereas graph_centrality would give WBC[C]=0 (BFS picks 1-hop direct path).
