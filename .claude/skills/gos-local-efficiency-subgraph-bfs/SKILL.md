---
name: gos-local-efficiency-subgraph-bfs
description: When implementing metrics on the subgraph induced by a node's neighbours in GOSKernel (e.g., local efficiency E_loc, neighbourhood clustering), index the BFS dist[] array by POSITION in the neighbors[] array (0..nb), not by global node slot. This naturally restricts BFS to the induced subgraph and avoids needing a separate "is this node in the subgraph?" boolean array. Apply in graph_local_efficiency_inner and any future induced-subgraph metric in crates/gos-runtime/src/lib.rs.
---

# Induced-Subgraph BFS: Neighbour-Position Indexing

## The rule

When running BFS within G_v (the subgraph induced by node v's neighbours), index the BFS
distance array by **position in the neighbour buffer** (0..nb), not by global node slot:

```rust
// Collect undirected neighbours into neighbors[0..nb]
let mut neighbors = [NodeId::ZERO; MAX_NODES];
let mut nb = 0usize;
for edge in self.edges.iter().flatten() {
    let other = if edge.spec.from_node == vid { edge.spec.to_node }
                else if edge.spec.to_node == vid { edge.spec.from_node }
                else { continue };
    if other == vid { continue; }
    if !neighbors[..nb].contains(&other) {
        neighbors[nb] = other;
        nb += 1;
    }
}

// BFS from each source position si in 0..nb
for si in 0..nb {
    let mut dist = [u32::MAX; MAX_NODES]; // indexed 0..nb (position), not global slot
    let mut queue = [0usize; MAX_NODES];
    dist[si] = 0;
    queue[0] = si;
    let mut q_head = 0usize;
    let mut q_tail = 1usize;

    while q_head < q_tail {
        let vi = queue[q_head]; q_head += 1;
        let v_id = neighbors[vi];           // NodeId of current position

        for edge in self.edges.iter().flatten() {
            if edge.spec.from_node != v_id { continue; }
            let w_id = edge.spec.to_node;

            // Linear scan: is w_id a neighbour of the centre node?
            let mut wi_opt = None;
            for ni in 0..nb {
                if neighbors[ni] == w_id { wi_opt = Some(ni); break; }
            }
            let wi = match wi_opt { Some(i) => i, None => continue }; // not in subgraph

            if dist[wi] == u32::MAX {
                dist[wi] = dist[vi].saturating_add(1);
                if q_tail < MAX_NODES { queue[q_tail] = wi; q_tail += 1; }
            }
        }
    }

    // Accumulate 1/d for all reachable targets in the subgraph
    for ti in 0..nb {
        if ti == si { continue; }
        if dist[ti] != u32::MAX && dist[ti] > 0 {
            sum_recip = sum_recip.saturating_add(SCALE / dist[ti] as u64);
        }
    }
}

// E(G_v) = sum_recip / (nb * (nb - 1))
let ev_ppm = sum_recip / (nb * (nb - 1)) as u64;
```

## Why it's non-obvious

**Global BFS (Wiener, global efficiency, harmonic) uses `dist[global_slot]`**: the standard
approach in GOSKernel indexes `dist[]` by the global node slot (0..MAX_NODES) and calls
`node_slot_by_id()` to map NodeId → slot. This does NOT restrict the BFS to any subgraph.

**Induced-subgraph BFS must be restricted**: when computing E(G_v), only edges WITHIN
the neighbour set of v should be traversed. The cleanest way in no_std Rust without dynamic
allocation is to:
1. Store neighbours as a `[NodeId; MAX_NODES]` array with a count `nb`
2. Index `dist[]` as `dist[position_in_neighbors]`, not `dist[global_slot]`
3. Restrict traversal by linear-scanning for `w_id` in `neighbors[0..nb]` — O(k) per edge

**Why not use a boolean subgraph mask?** A `[bool; MAX_NODES]` mask indexed by global slot
would work but requires a `node_slot_by_id()` call for every edge. The position-indexed
approach avoids that call entirely and is self-contained.

**nb*(nb-1) denominator is always safe**: nb ≥ 2 is guarded before entering the BFS loop,
so the denominator is at least 2. No divide-by-zero risk.

**This is distinct from global efficiency**: global efficiency does BFS over the WHOLE graph;
local efficiency does nb separate BFS computations, each over a DIFFERENT k-node subgraph.

## Comparison: global vs. local efficiency BFS

| Property | Global efficiency (V2.74) | Local efficiency (V2.76) |
|----------|--------------------------|--------------------------|
| `dist[]` indexed by | global slot | neighbour position |
| BFS scope | all nodes | neighbours of v only |
| Denominator | n*(n-1) (global) | nb*(nb-1) per node |
| Outer loop | once per source node | once per (centre node, source pair) |
| Complexity | O(V*(V+E)) | O(V * k * (k+E)) where k=avg degree |

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_local_efficiency_inner()` (V2.76)
- Public wrapper: `graph_local_efficiency() -> (u32, usize, usize)`
- Shell: "graph local efficiency" / "graph local eff" / "gleff" / "local efficiency"
- VectorAddress L4=52 for gos-graph-local-eff-harness test nodes
- Denominator rule: divide sum of E(G_v) by n (ALL nodes), not nodes_computed — see [[gos-avg-clustering-denominator-pattern]]
- Neighbour collection: same undirected union as [[gos-clustering-undirected-neighbors]]

## From this session

V2.76: implemented `graph_local_efficiency_inner`. All 10 harness tests passed on first
compile. Key verification:
- Test 5 (directed triangle): G_A={B,C}, B→C but not C→B. sum_recip=1_000_000. E(G_A)=500_000 ✓
- Test 8 (bidirectional triangle): G_A={B,C}, B↔C. sum_recip=2_000_000. E(G_A)=1_000_000 ✓
- Test 7 (complete K4): G_A={B,C,D} has chain structure. E(G_A)=3_000_000/6=500_000 ✓
- Test 10 (four-cycle): each G_v pair has no directed path. E_loc=0 ✓

Critical invariant: the linear scan `for ni in 0..nb { if neighbors[ni] == w_id }` is O(k)
per edge, making total complexity O(V*k*(k+E)). For the GOSKernel max of 128 nodes and
512 edges with k≤128, this is at most ~8M operations — acceptable for a kernel metric.
