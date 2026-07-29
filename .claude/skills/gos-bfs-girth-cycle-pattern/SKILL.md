---
name: gos-bfs-girth-cycle-pattern
description: When implementing directed graph girth (shortest directed cycle) in GOSKernel, use BFS from each source and detect back-edges that return to the source; prune BFS frontiers where cur_dist+1 >= min_girth; break the outer loop when min_girth==1 (self-loop). Apply in crates/gos-runtime/src/lib.rs graph_girth_inner.
---

# BFS Girth Detection: Back-Edge-to-Source with Frontier Pruning

## The rule

To find the shortest directed cycle (girth), BFS from each source node `s`;
detect a cycle when any edge leads back to `s`; prune frontiers that can't improve:

```rust
let mut min_girth: u32 = u32::MAX;

for si in 0..node_count {
    if min_girth == 1 { break; }   // ← self-loop: can't get shorter

    let s = node_slots[si];
    let s_id = /* node_id of s */;

    let mut dist = [u32::MAX; MAX_NODES];
    dist[s] = 0;
    // BFS queue...

    while q_head < q_tail {
        let cur = queue[q_head]; q_head += 1;
        let cur_dist = dist[cur];

        // KEY PRUNE: frontier can't improve min_girth
        if cur_dist + 1 >= min_girth { continue; }

        for ei in 0..MAX_EDGES {
            let edge = /* directed out-edge from cur */;
            let nbr_id = edge.spec.to_node;

            if nbr_id == s_id {
                // BACK-EDGE TO SOURCE → cycle found
                let cycle = cur_dist + 1;
                if cycle < min_girth { min_girth = cycle; }
                continue;   // don't enqueue source again
            }

            // Only enqueue unvisited neighbors that could improve min_girth
            if dist[nbr_slot] == u32::MAX && cur_dist + 1 < min_girth {
                dist[nbr_slot] = cur_dist + 1;
                queue[q_tail] = nbr_slot; q_tail += 1;
            }
        }
    }
}

let is_acyclic = min_girth == u32::MAX;
```

## Why it's non-obvious

**Self-loop detection is implicit**: a self-loop A→A is detected naturally by the
back-edge check (`nbr_id == s_id`) when `cur == s` and `dist[s] = 0`.
No special-case needed — cycle = 0+1 = 1.

**The pruning guard `cur_dist + 1 >= min_girth` must appear BEFORE the edge scan**,
not just before enqueueing.  Without it, the algorithm still produces correct results
but processes entire BFS levels that can't possibly contribute a shorter cycle.

**Directed-only**: unlike `graph_clustering`, `graph_modularity`, `graph_rich_club`,
and `graph_assortativity` (which all project to undirected edges), girth uses
**directed edges only** (`edge.spec.from_node == cur_id`).  Do NOT follow reverse
edges — that would find undirected cycles, not directed ones.

**Don't enqueue source into itself**: after detecting a back-edge, `continue` before
the enqueue block.  Without this, dist[s] would be overwritten from 0 to some larger
value, corrupting future back-edge detection for the same source.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_girth_inner()` (V2.69)
- Public wrapper: `graph_girth() -> (u32, bool, usize)` = (girth, is_acyclic, node_count)
- Uses `&self` (not snapshot) — consistent with graph_clustering_inner, graph_transitivity_inner
- Shell: "graph girth" / "ggirth"
- VectorAddress L4=45 reserved for gos-graph-girth-harness test nodes
- Complexity: O(V × (V + E)) — 128 × 640 = ~82k operations max, no concern

## From this session

V2.69: implemented `graph_girth_inner` following this pattern. All 10 harness tests
passed on first compile. Key correctness tests:
- Test 4 (self-loop): girth=1 via back-edge-to-source at dist=0
- Test 6 (mutual pair A↔B): girth=2 via BFS from A finding B→A at dist=1
- Test 9 (C4 cycle A→B→C→D→A): girth=4; confirmed no shorter cycle exists
- Test 10 (triangle + mutual pair): min(3,2)=2; mutual pair in separate component wins
