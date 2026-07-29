---
name: gos-avg-clustering-denominator-pattern
description: When implementing the Watts-Strogatz average clustering coefficient in GOSKernel, divide sum_cc_ppm by n (ALL alive nodes), NOT by nodes_computed (only nodes with k≥2). The WS standard includes degree-0 and degree-1 nodes as CC=0 in the average. Apply in graph_avg_clustering_inner and any future per-node average metric where low-degree nodes contribute 0.
---

# Average Clustering: Denominator is ALL Nodes, Not nodes_computed

## The rule

When computing `avg_CC = (1/n) × Σ CC(v)`, the denominator `n` must be the count of
**all alive nodes**, not `nodes_computed` (the count of nodes with undirected degree ≥ 2):

```rust
// CORRECT — WS standard: degree-0 and degree-1 nodes contribute CC(v)=0
let avg_ppm = (sum_cc_ppm / n as u64).min(1_000_000) as u32;

// WRONG — would inflate avg_CC for sparse graphs
let avg_ppm = (sum_cc_ppm / nodes_computed as u64).min(1_000_000) as u32;
```

Return both `nodes_computed` and `node_count` so callers can observe how many nodes
actually contributed to the sum.

## Why it's non-obvious

It seems counter-intuitive to divide by `n` when nodes with k < 2 contribute exactly 0 —
why include them in the denominator? But the Watts-Strogatz (1998) definition and NetworkX's
`average_clustering()` both divide by n. The reason: avg_CC is meant to measure the *average
density of the local neighbourhood*, which is genuinely 0 for isolated or leaf nodes.
Dividing by `nodes_computed` instead would give a metric that ignores graph sparsity and
overcounts dense subgraphs.

**Concretely:** a triangle {A,B,C} + isolated D:
- With denominator n=4: avg_CC = 3_000_000/4 = 750_000 (correct WS)
- With denominator nodes_computed=3: avg_CC = 3_000_000/3 = 1_000_000 (wrong — equal to a pure triangle)

## Relationship to graph_clustering (V2.61) and graph_transitivity (V2.63)

These two compute the same **global transitivity ratio** `total_triangles/total_triplets`,
not a per-node average. They are high-degree-weighted. `graph_avg_clustering` (V2.75) is the
true unweighted average — the two metrics differ for all graphs where degree is non-uniform.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_avg_clustering_inner()` (V2.75)
- Public wrapper: `graph_avg_clustering() -> (u32, usize, usize)`
- Shell: `graph avg clustering` / `gavgcc`
- VectorAddress L4=51 for gos-graph-avg-clustering-harness test nodes
- `nodes_computed` increments for ALL nodes with k ≥ 2, even those with 0 triangles

## From this session

V2.75: test 8 (triangle + isolated D) verified the denominator choice explicitly:
- sum_cc_ppm = 3_000_000 (three nodes each CC=1.0)
- n = 4 (triangle nodes + isolated D)
- Expected ppm = 750_000 = 3_000_000/4 ✓
If denominator had been nodes_computed=3, result would be 1_000_000 — indistinguishable
from a pure 3-node triangle, which would be wrong.
