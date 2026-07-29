---
name: gos-mpc-dag-bipartite-expansion
description: When implementing minimum path cover for a DAG in no_std Rust, use the König/Dilworth theorem MPC=n−ν with a bipartite expansion (left_u→right_v per directed edge), then reconstruct paths by following match_l[] successor chains from nodes where match_r[ci]==NIL.
---

# Minimum Path Cover in DAG via Bipartite Expansion

## The rule

**Theorem (König 1931 / Dilworth 1950):**
```
MPC(DAG) = n − ν(B(G))
```
where B(G) is the bipartite expansion (left copy → right copy for each directed edge)
and ν is the maximum matching of B(G).

**Implementation pattern:**

```rust
// 1. Build bipartite expansion as u128 bitmask adjacency.
//    right_adj[u_ci] = bitmask of right-side CIs v where (u,v) ∈ G (directed only).
let mut right_adj = [0u128; MAX_NODES];
for ei in 0..MAX_EDGES {
    let edge = match self.edges[ei] { Some(e) => e, None => continue };
    if edge.spec.from_node == edge.spec.to_node { continue; } // skip self-loops
    let fci = slot_to_ci[fs]; let tci = slot_to_ci[ts];
    if fci == NIL || tci == NIL { continue; }
    right_adj[fci] |= 1u128 << tci;  // directed only — NOT symmetric
}

// 2. Run Kuhn matching → match_l[ci] / match_r[ci].
//    path_count = nc - match_count.

// 3. Reconstruct paths.
//    Path start: nodes where match_r[ci] == NIL (no in-matching predecessor).
//    Successor: match_l[ci] (the matched right-side node, i.e., next in chain).
for ti in 0..nc {
    let ci = slot_to_ci[topo_order[ti]];
    if match_r[ci] != NIL { continue; } // not a start
    let pid = path_id_ctr; path_id_ctr += 1;
    let mut cur_ci = ci;
    while cur_ci != NIL {
        // emit cur_ci with path_id = pid
        cur_ci = match_l[cur_ci]; // follow successor chain
    }
}
```

## Why it's non-obvious

**1. Expansion is DIRECTED only.** Unlike undirected bipartite matching (V2.92/V2.97
where edges are undirected and BFS 2-coloring assigns sides), the MPC expansion uses
only directed edges: u→v in G becomes u_L→v_R in B(G). Do NOT add v_R→u_L. Adding
the reverse edge would merge distinct paths and give wrong MPC counts.

**2. Path starts = unmatched RIGHT-side copies.** After matching, a node ci is a path
start iff `match_r[ci] == NIL` (nothing was matched INTO it). Do not confuse with
`match_l[ci] == NIL` (nothing was matched OUT of it — that means ci is a path END).

**3. match_l[] IS the successor chain.** If match_l[u] = v, then the matching chose
directed edge u→v, meaning v follows u in the path. Following match_l[] chains from
path starts reconstructs entire paths without any additional bookkeeping.

**4. Path_count + match_count = nc** at all times. This is the König/Dilworth equality
and a useful invariant to assert in tests: MPC + ν = n.

**5. Topological order for Kuhn** produces natural path chains (earlier sources first).
Not required for correctness but makes output deterministic and human-readable.

**6. match_l[] chains are acyclic** because the matching only uses directed DAG edges,
and the DAG is acyclic. Following the chain always terminates.

## Complexity

- Phase 1 (Kahn's BFS): O(V+E)
- Phase 2 (bipartite expansion): O(E)
- Phase 3 (Kuhn matching): O(V·E)
- Phase 4 (path reconstruction): O(V)
- Total: O(V·E)

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_min_path_cover_inner` (V2.99)
- Shell: "graph mpc" / "gmpc" / "min path cover" / "graph min path cover" / "path cover" / "gdagcover"
- VectorAddress L4=75 for gos-graph-mpc-harness
- Requires is_dag=true (Kahn BFS check); returns (path_vecs, path_ids, path_count, is_dag, nc)

## From this session

V2.99 implemented `graph_min_path_cover`. The key non-obvious element: the bipartite
expansion uses DIRECTED edges only (not undirected as in V2.92/V2.97), and path
reconstruction just follows match_l[] chains from match_r[ci]==NIL nodes. All 10
harness tests passed including: Hamiltonian chain (MPC=1, chain A→B→C→D), diamond
MPC=2 (D_R contested), cycle rejection (is_dag=false), and Dilworth cross-check
(MPC+ν=n: star gives MPC=4, ν=1, n=5).
