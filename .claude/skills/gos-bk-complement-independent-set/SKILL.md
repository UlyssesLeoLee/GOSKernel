---
name: gos-bk-complement-independent-set
description: When implementing maximum independent set in GOSKernel, compute complement adjacency comp[i] = all_nodes & !adj[i] & !(1u128 << i) and run the existing BK iterative algorithm on comp[] — max clique in G̅ equals max IS in G. Reuses all BK frame logic from gos-bk-clique-iterative-pattern with a single adjacency swap.
---

# BK on Complement Graph: Maximum Independent Set

## The rule

Max independent set (α(G)) = max clique in the complement graph (ω(G̅)).

Build complement adjacency from the existing `adj[]` bitmasks in one pass:

```rust
let all_nodes: u128 = if nc >= 128 { u128::MAX } else { (1u128 << nc) - 1 };
let mut comp = [0u128; MAX_NODES];
for i in 0..nc {
    comp[i] = all_nodes & !adj[i] & !(1u128 << i);
}
```

Then run the *identical* iterative BK loop (BkFrame stack, Tomita pivot, choose_pivot_comp)
replacing every `adj[u]` reference with `comp[u]`. Return `(is_vecs, is_size, is_count, nc)`
with the same signature as `graph_clique`.

Key properties to verify with tests:

| Graph | α(G) | is_count |
|---|---|---|
| Empty (n=0) | 0 | 0 |
| Single node | 1 | 1 |
| Two isolated nodes | 2 | 1 (unique MIS = both) |
| Single edge A-B | 1 | 2 ({A} and {B}) |
| Triangle K3 | 1 | 3 (each singleton) |
| Path P4 | 2 | 3 ({A,C},{A,D},{B,D}) |
| K4 | 1 | 4 (each singleton) |
| Star K_{1,4} | 4 | 1 (all leaves) |

## Why it's non-obvious

The complement formula has three parts — all three are mandatory:
1. `all_nodes` — restricts to active node indices only (no phantom bits)
2. `!adj[i]` — inverts adjacency (non-neighbours become neighbours in complement)
3. `!(1u128 << i)` — removes the self-loop position from the complement

Forgetting part 3 adds a spurious self-edge in the complement, making every node appear adjacent to itself in G̅ — BK then finds only size-0 or size-1 independent sets for connected graphs (wrong).

The `all_nodes` guard is also required: if nc=128, `(1u128 << 128)` shifts 128 bits and overflows; use `u128::MAX` instead (same guard as in `graph_clique_inner`).

## Cross-validation invariants

- **König (bipartite):** α(G) = n − ν(G). For K_{3,3}: n=6, ν=3, α=3.
  Call `graph_bipartite_match` and assert `is_size == node_count - match_count`.
- **Perfect-graph K_n:** α(K_n) + ω(K_n) = 1 + n = n + 1.
- **Vertex-transitive:** α(G) · ω(G) ≥ n (tight for K_n).

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_independent_set_inner<N>` (V2.96)
- `pub fn graph_independent_set<N>()` — public wrapper
- `host-tests/gos-graph-indep-harness/tests/graph_independent_set.rs` — 10 tests
- Shell dispatch: `dispatch_graph_independent_set` in `crates/k-shell/src/lib.rs`
- Routing: `gindep` / `indep` / `independent set` / `graph independent set` in `proc.rs`
- Display: bright-magenta header (color 13), bright-blue IS members (color 9)

## From this session

V2.96: implemented `graph_independent_set_inner` by cloning `graph_clique_inner` and
replacing `adj` with `comp`. The only change in BK mechanics is using `comp[v]` instead
of `adj[v]` for masking new_p, new_x, and the pivot's neighbour count. All 10 tests
passed immediately; no structural BK changes were needed beyond the adjacency swap.
