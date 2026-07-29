---
name: gos-clustering-undirected-neighbors
description: When implementing the Watts-Strogatz clustering coefficient for a directed graph in GOSKernel, the neighbor set for each node must be the UNION of in-neighbors and out-neighbors (undirected treatment). Using only out-neighbors gives wrong results — directed 3-cycles show 0% clustering instead of 100%. Apply when implementing graph_clustering_inner or any other local-clustering variant in crates/gos-runtime/src/lib.rs.
---

# Clustering Coefficient: Use Undirected Neighbor Sets

## The rule

When computing the Watts-Strogatz clustering coefficient on a directed graph, collect each node v's neighbor set as the **union of in-neighbors and out-neighbors**, treating all edges as undirected:

```rust
for edge in self.edges.iter().flatten() {
    let other = if edge.spec.from_node == vid {
        edge.spec.to_node           // v→other: other is out-neighbor
    } else if edge.spec.to_node == vid {
        edge.spec.from_node         // other→v: other is in-neighbor
    } else {
        continue;
    };
    // deduplicate and add `other` to neighbors
}
```

Then count edge-pairs among those neighbors (in either direction) as triangle pairs.

## Why it's non-obvious

If you use only out-neighbors, the directed 3-cycle A→B→C→A gives each node exactly ONE out-neighbor — not the two needed to form a triplet. The metric returns 0% clustering even though the graph is perfectly triangulated. This is wrong from a small-world perspective.

The undirected treatment correctly captures that A "knows" both B (A→B) and C (C→A), so the pair (B, C) forms a triplet that is closed (B→C), yielding 100% clustering.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_clustering_inner()`
- Neighbor buffer: `[NodeId; MAX_NODES]` — 128 entries, 2048 bytes, safe for no_std stack
- Triangle check: scan `self.edges` for edge(B,C) OR edge(C,B) — both directions count
- The formula: `clustering_ppm = total_triangle_pairs * 1_000_000 / total_pair_triplets`
- Returns `(0, n)` when no node has >= 2 neighbors (metric undefined, not an error)

## Formula equivalence — clustering vs. transitivity

`graph_clustering_inner` uses the **same global ratio** as `graph_transitivity_inner` (V2.63):
`ppm = total_triangles * 1_000_000 / total_triplets`

The label "Watts-Strogatz" on `graph_clustering` is a misnomer — true WS CC is a per-node average,
but both implementations compute the **global transitivity** ratio. They are not distinct metrics.

The only API difference: `graph_clustering()` returns `(ppm, node_count)` while
`graph_transitivity()` returns `(ppm, triangle_count, triplet_count, node_count)`.

## From this session

V2.61: initial design considered out-neighbors only. Realized a directed 3-cycle A→B→C→A would incorrectly show 0% clustering because each node has k=1 (one out-neighbor). Switched to undirected neighbor union — test 6 (`directed_3_cycle_full_clustering`) confirmed ppm=1_000_000. The undirected approach also correctly handles the mixed-direction case (A→B, A→C, B→C) and the partial case (600_000 ppm).

V2.63: added `graph_transitivity_inner` using the same formula. Test 9 in gos-graph-transitivity-harness confirms `trans_ppm == clust_ppm` for identical graph state. The mismatch was the original design intent — both turned out identical.

V2.75: added `graph_avg_clustering_inner` — the TRUE Watts-Strogatz per-node average that IS distinct from global transitivity. See [[gos-avg-clustering-denominator-pattern]] for the key invariant: denominator is n (all nodes), not nodes_computed.
