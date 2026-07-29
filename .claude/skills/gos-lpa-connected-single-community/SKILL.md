---
name: gos-lpa-connected-single-community
description: In GOSKernel's LPA (Label Propagation Algorithm), any connected graph always converges to a single community — so graph_modularity returns Q=0 for ANY connected input, regardless of structure (star, path, cycle, complete graph). Only disconnected graphs can yield Q > 0. Apply when designing test cases for graph_community or graph_modularity in host-tests/.
---

# LPA on Connected Graphs Always Yields Q = 0

## The rule

When writing test cases for `graph_community`, `graph_modularity`, or any future LPA-based metric:
- **Connected graph** (any path exists between all node pairs) → **1 community, Q = 0**
- **Disconnected graph** (≥ 2 components) → ≥ 2 communities, Q > 0 (if components are balanced)

Never expect Q > 0 from a connected graph in these tests.

```
K4 (fully connected)           → (0, 1, 6, 4)   ← Q=0, 1 community
Directed triangle A→B→C→A     → (0, 1, 3, 3)   ← Q=0, 1 community
Star hub→B,C,D                 → (0, 1, 3, 4)   ← Q=0, 1 community
Two disconnected K3 cliques    → (500_000, 2, 6, 6)  ← Q=0.5, 2 communities ✓
```

## Why it's non-obvious

LPA initialises every node with its own label, then iterates: each node adopts the most-frequent label among its neighbours. In a connected graph, the highest-degree node's label propagates and "infects" the entire connected component within a few iterations. No matter how sparse the connections, connectivity ensures convergence to one label.

This means a star graph (hub connects to all, leaves connect only to hub) still ends up with Q=0 — the hub's label propagates to all leaves in one iteration, and all leaves already share the hub's label back, so equilibrium is reached at 1 community.

**Consequence for test design:** To get Q > 0 you must add at least two completely disconnected components. A "bridged" graph (two dense clusters connected by a single edge) may or may not split — LPA is not deterministic in that case, and the bridge edge may pull both sides into one community.

## GOSKernel context

- LPA algorithm: `graph_modularity_inner` and `graph_community_inner` in `crates/gos-runtime/src/lib.rs`
- 20 fixed iterations, deterministic tie-break (lower slot index wins)
- Undirected treatment: edges in both directions count as neighbour relationships
- Tests: `host-tests/gos-graph-modularity-harness/tests/graph_modularity.rs` (tests 3, 4, 5, 10 all verify connected→Q=0)

## From this session

V2.67 modularity test matrix: tests 3 (single edge), 4 (directed triangle), 5 (K4), 10 (star) all return Q=0. The insight was initially confusing because a star graph "looks like" it has structure (hub vs. leaves), but LPA doesn't see any cross-cluster evidence — the hub's label dominates all leaves immediately.
