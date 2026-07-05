---
name: gos-bipartite-match-augment-pattern
description: When implementing iterative Kuhn augmenting-path bipartite matching in no_std Rust, use a chosen_b[] array to record each DFS level's selected B-node so the augmentation walk can reconstruct the full alternating path without recursion or separate path buffers.
---

# Iterative Kuhn Augmenting-Path Pattern for Bipartite Matching

## The rule

Kuhn's bipartite matching requires augmenting an alternating path once a free B-node is found. In iterative (non-recursive) form, use three parallel arrays:

```rust
let mut dfs_stk:  [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];  // (a_slot, ei)
let mut chosen_b: [usize; MAX_NODES] = [NIL; MAX_NODES];              // B chosen at each level
let mut visited_b: [bool; MAX_NODES] = [false; MAX_NODES];            // per-DFS B mask
```

When augmenting (free B-node found at level `lvl`):
```rust
chosen_b[lvl] = free_b_slot;
let mut cur_b = free_b_slot;
let mut cur_lv = lvl;
loop {
    let cur_a = dfs_stk[cur_lv].0;
    match_a[cur_a] = cur_b;
    match_b[cur_b] = cur_a;
    if cur_lv == 0 { break; }
    cur_lv -= 1;
    cur_b = chosen_b[cur_lv];  // ← the B that led to cur_a via the old match
}
```

Reset `visited_b` to `[false; MAX_NODES]` for each new free A-node's DFS. Never share it across DFS invocations.

## Why it's non-obvious

Three subtle failure modes if the pattern is wrong:

1. **No path buffer needed** — unlike BFS augmenting paths (which need `prev_a[b]` / `prev_b[a]` arrays), the `dfs_stk` already holds the A-nodes in order; `chosen_b[k]` just records which B was chosen at level k. The augmentation walk is a simple loop from `lvl` down to 0.

2. **`chosen_b[lvl]` must be written BEFORE augmenting** — the free-B case sets `chosen_b[lvl] = free_b` so the walk starts correctly. If omitted, `chosen_b[lvl]` holds a stale value from a previous DFS attempt at this level.

3. **`visited_b` reset is per free-A-node, not global** — if shared globally, a B-node rejected (dead end) in an earlier DFS prevents it being found as the terminal free node in a later DFS, causing under-counting of match_count. The visited mask only prevents cycles *within* a single DFS.

4. **Backtrack resume via `dfs_stk[lvl].1 = ei`** — when pushing a child (matched B's A), save the current edge scan index in `dfs_stk[lvl].1`. The outer while loop does `let (a_slot, mut ei) = dfs_stk[lvl]` at the top, restoring `ei` when the child backtracks. Combined with `found_next` being re-initialized to `false` each outer iteration, scanning correctly continues from the right position without spurious early backtrack.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_bipartite_match_inner` (V2.92)
- All inner graph algorithms use iterative DFS (no recursion: stack overflow risk in no_std kernel)
- `MAX_NODES = 128`, `MAX_EDGES = 512` — all stack arrays are fixed-size

## From this session

V2.92 implemented `graph_bipartite_match` using Kuhn's O(V·E) algorithm.
The `chosen_b` array eliminated the need for a separate path-reconstruction BFS
and made the augmentation walk a single loop: `cur_b = chosen_b[level]; level -= 1`.
All 10 harness tests passed first try with this pattern.
