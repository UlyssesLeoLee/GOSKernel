---
name: gos-density-return-order
description: graph_density() returns (density_ppm, node_count, edge_count) — NOT (density_ppm, edge_count, node_count). Apply whenever destructuring the result of gos_runtime::graph_density() in harnesses, dispatch functions, or summary aggregators.
---

# graph_density() Return Order: node_count Before edge_count

## The rule

`gos_runtime::graph_density()` returns a tuple of three values in this order:

```rust
// CORRECT — (ppm, node_count, edge_count)
let (density_ppm, node_count, edge_count) = gos_runtime::graph_density();

// WRONG — silently swaps n and e, causing all downstream node_count assertions to fail
let (density_ppm, edge_count, node_count) = gos_runtime::graph_density();
```

When n < 2, the function returns `(0, n, e)` where n is alive node count and e is directed edge count. When n ≥ 2, it returns `(density_ppm, n, e)`.

## Why it's non-obvious

The intuitive ordering would be `(density, edges, nodes)` — "density is computed from edges, so edges comes next." But the actual implementation puts `node_count` second because `n` is the primary loop variable computed first in `graph_density_inner()`:

```rust
let n = self.nodes.iter().filter(|s| s.is_some()).count();
let e = self.edges.iter().filter(|s| s.is_some()).count();
// ...
(density_ppm, n, e)  // n before e — matches computation order, not semantic order
```

The return order follows computation order, not alphabetical or semantic order.

## GOSKernel context

- Defined in `crates/gos-runtime/src/lib.rs` around line 7707 (public) and 1803 (inner)
- Also note: `graph_global_efficiency()` → `(u64, pairs_max, node_count)` (node_count last)
- `graph_avg_clustering()` → `(u32, nodes_computed, node_count)` (node_count last)
- `graph_scale_free()` → `(kappa_ppm, max_degree, avg_degree_ppm, node_count, m_undir)` (node_count 4th)
- **Only `graph_density()` has node_count in position 2 (before edge_count)**

## From this session

V2.79 `gos-graph-summary-harness` initial run: 8 of 10 tests failed because `summary()` helper had `let (density_ppm, edge_count, node_count)` — node_count received the edge_count value (0 for isolated nodes, wrong counts for connected graphs). Fix: swap to `let (density_ppm, node_count, _edge_count)`.
