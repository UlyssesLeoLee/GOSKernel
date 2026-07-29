---
name: gos-wiener-sum-distances-pattern
description: When implementing the Wiener index (sum of all pairwise directed BFS distances) in GOSKernel, run BFS from every source node, skip the source slot itself when accumulating, and use u64 for the accumulator. Self-loops and unreachable pairs are excluded automatically — no special cases needed. Apply in crates/gos-runtime/src/lib.rs graph_wiener_inner.
---

# Wiener Index BFS: Sum-All-Distances Pattern

## The rule

To compute the Wiener index W(G) = Σ_{u≠v, d(u,v)<∞} d(u,v), run plain BFS from every
source and accumulate finite distances (excluding the source itself):

```rust
pub fn graph_wiener_inner(&self) -> (u64, usize, usize) {
    // collect live slots...
    let mut wiener_index: u64 = 0;
    let mut reachable_pairs: usize = 0;

    for si in 0..node_count {
        let s = node_slots[si];
        if self.nodes[s].is_none() { continue; }

        let mut dist = [u32::MAX; MAX_NODES];
        dist[s] = 0;
        // standard BFS queue...

        while q_head < q_tail {
            let cur = queue[q_head]; q_head += 1;
            let cur_dist = dist[cur];
            let cur_id = /* node_id of cur */;

            for ei in 0..MAX_EDGES {
                let edge = /* directed out-edge from cur */;
                let nbr_slot = /* slot of edge.spec.to_node */;
                if dist[nbr_slot] == u32::MAX {
                    dist[nbr_slot] = cur_dist + 1;
                    queue[q_tail] = nbr_slot; q_tail += 1;
                }
            }
        }

        // Accumulate: skip source, skip unreachable
        for ti in 0..node_count {
            let t = node_slots[ti];
            if t == s { continue; }                    // skip self
            if dist[t] != u32::MAX {
                wiener_index += dist[t] as u64;        // accumulate
                reachable_pairs += 1;
            }
        }
    }
    (wiener_index, reachable_pairs, node_count)
}
```

## Why it's non-obvious

**Self-loops are excluded automatically**: a self-loop A→A sets the source as `dist[s]=0`
(already visited), so when BFS processes A and encounters the edge A→A, `dist[s] != u32::MAX`
and the neighbor is skipped. No explicit check `if nbr_id == s_id` is needed (unlike girth,
which *requires* that check to detect cycles).

**Contrast with girth BFS**: girth needs `s_id` to detect back-edges; Wiener doesn't.
Wiener BFS is simpler — plain flood-fill with distance accumulation. No `s_id`, no pruning.

**u64 accumulator is required**: theoretical worst case for V=128 linear chain is
W ≈ 350,000, well within u64. Using u32 would be fine in practice but u64 is the correct
type for a sum that scales as O(V³) in dense graphs.

**Unreachable pairs**: disconnected components (no directed path) naturally produce
`dist[t] == u32::MAX` and are excluded from both the sum and the pair count.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_wiener_inner()` (V2.70)
- Public wrapper: `graph_wiener() -> (u64, usize, usize)` = (wiener_index, reachable_pairs, node_count)
- Uses `&self` (not snapshot) — consistent with graph_girth_inner, graph_clustering_inner
- Shell: "graph wiener" / "gwiener"
- VectorAddress L4=46 reserved for gos-graph-wiener-harness test nodes
- Complexity: O(V × (V + E)) — same as girth but without pruning (simpler)

## From this session

V2.70: implemented `graph_wiener_inner` following this pattern. All 10 harness tests passed
on first compile. Key correctness tests verified:
- Test 5 (chain A→B→C): d(A,B)=1, d(A,C)=2, d(B,C)=1 → W=4 ✓
- Test 6 (triangle): 6 pairwise distances {1,2,1,2,1,2} → W=9 ✓
- Test 8 (self-loop A→A): W=0, pairs=0 — self-loop auto-excluded ✓
- Test 9 (disconnected): only (A,B) reachable, W=1, pairs=1 ✓
