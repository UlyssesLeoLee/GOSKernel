# Hardening Log — V2.96 Maximum Independent Set
**Date:** 2026-07-05  
**Branch:** feat/vk-auto-live-surface  
**Commit:** af1fa8e

## Summary

Implemented `graph_independent_set<N>()` — maximum independent set (MIS) via
Bron-Kerbosch with Tomita pivot applied to the complement graph G̅.

**Key insight:** Max clique in G̅ = Max independent set in G.  
The complement adjacency `comp[i] = all_nodes & !adj[i] & !(1 << i)` is
computed in O(n) bitwise ops; BK runs on `comp[]` with the same iterative
stack as V2.95 `graph_clique`.

## New Artifacts

### gos-runtime
- `graph_independent_set_inner<N>` — BK on complement, returns (is_vecs, α, is_count, n)
- `graph_independent_set<N>` — public wrapper

### k-shell
- `dispatch_graph_independent_set` — bright-magenta header, bright-blue IS members
- Shell: `graph independent set` / `graph indep` / `gindep` / `independent set` / `indep`

### gos-graph-indep-harness (L4=72)
10 tests — all green:
1. Empty graph: α=0
2. Single node: α=1
3. Two isolated nodes: α=2 (unique MIS = both)
4. Single edge A-B: α=1, is_count=2
5. Triangle K3: α=1, is_count=3
6. Path P4: α=2, is_count=3
7. K4 complete: α=1, is_count=4
8. Star K_{1,4}: α=4, is_count=1 (all leaves)
9. Bipartite K_{3,3}: α=3, is_count=2; König cross-check α=n-ν
10. K4 cross-check: α·ω≥n, α+ω=n+1 (perfect graph)

## Algorithm Invariants

| Property | Formula |
|---|---|
| Independence number | α(G) = ω(G̅) |
| König (bipartite) | α(G) = n − ν(G) |
| Complement | comp[i] = all_nodes & !adj[i] & !self |
| Perfect-graph bound | α(G) + ω(G) = n+1 for K_n |
| General bound | α(G) · ω(G) ≥ n (vertex-transitive) |

## OS Analogy

α(G) = size of the largest set of kernel subsystems with **no direct
dependencies between them** — the maximally parallel startup or hot-patch
frontier.  Equivalent to finding the widest "independent work batch" in a
`make -jN` dependency graph (those nodes with no mutual edges can be scheduled
in parallel without synchronization).

## Test Count: 933 total host tests
- Prior: 923 through V2.95
- gos-graph-indep-harness: +10
