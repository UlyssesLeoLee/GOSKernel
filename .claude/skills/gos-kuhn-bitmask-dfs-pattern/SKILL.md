---
name: gos-kuhn-bitmask-dfs-pattern
description: When the bipartite adjacency is already stored as u128 bitmasks (right_adj[ci]), implement Kuhn's augmenting-path DFS using dfs_rem[level]:u128 for per-level remaining candidates and visited_r:u128 as the global per-source visited set — avoids scanning all MAX_EDGES for each DFS step.
---

# Kuhn Augmenting-Path DFS with u128 Bitmask Candidates

## The rule

When bipartite adjacency is stored as `right_adj[ci]: u128` bitmasks, use this DFS
variant instead of the edge-index-scan approach in `graph_bipartite_match_inner`:

```rust
const NIL: usize = usize::MAX;

let mut match_l = [NIL; MAX_NODES]; // match_l[left_ci] = right_ci
let mut match_r = [NIL; MAX_NODES]; // match_r[right_ci] = left_ci
let mut match_count = 0usize;

// Process each free left node (in topological order for DAG inputs).
for ti in 0..nc {
    let start_ci = slot_to_ci[topo_order[ti]];
    if start_ci == NIL || match_l[start_ci] != NIL { continue; }

    let mut visited_r = 0u128;  // per-DFS visited set — reset each source
    let mut dfs_lci:  [usize; MAX_NODES] = [NIL; MAX_NODES]; // left_ci per level
    let mut dfs_rem:  [u128;  128]        = [0;   128];        // remaining candidates per level
    let mut chosen_r: [usize; MAX_NODES]  = [NIL; MAX_NODES]; // right_ci chosen per level
    let mut st_top = 1usize;
    dfs_lci[0] = start_ci;
    dfs_rem[0] = right_adj[start_ci];
    let mut augmented = false;

    'dfs: while st_top > 0 {
        let lvl = st_top - 1;
        // Remaining unvisited right neighbors for this level.
        let avail = dfs_rem[lvl] & !visited_r;
        if avail == 0 { st_top -= 1; continue; } // backtrack

        let r_ci = avail.trailing_zeros() as usize;
        dfs_rem[lvl]  &= !(1u128 << r_ci); // consume from remaining
        visited_r     |=   1u128 << r_ci;  // mark globally visited in this DFS

        if match_r[r_ci] == NIL {
            // Free right node — augment bottom-up via chosen_r[].
            chosen_r[lvl] = r_ci;
            let (mut cur_r, mut cur_lv) = (r_ci, lvl);
            loop {
                let cur_l = dfs_lci[cur_lv];
                match_l[cur_l] = cur_r; match_r[cur_r] = cur_l;
                if cur_lv == 0 { break; }
                cur_lv -= 1; cur_r = chosen_r[cur_lv];
            }
            augmented = true; match_count += 1;
            break 'dfs;
        } else {
            // Matched right — push its left partner for DFS continuation.
            chosen_r[lvl] = r_ci;
            if st_top < MAX_NODES {
                dfs_lci[st_top] = match_r[r_ci];
                dfs_rem[st_top] = right_adj[match_r[r_ci]];
                st_top += 1;
            }
        }
    }
    let _ = augmented;
}
```

## Why it's non-obvious

**1. `dfs_rem[level]` vs `dfs_rem[level] & !visited_r`.** The remaining bitmask per
level tracks which right nodes that level hasn't tried yet (consumed by
`dfs_rem[lvl] &= !(1u128 << r_ci)`). The global `visited_r` prevents any right node
from being tried by TWO DIFFERENT stack levels in the same DFS call. Both masks are
needed: `dfs_rem` for per-level consumption, `visited_r` for cross-level deduplication.

**2. `visited_r` is monotone — never un-visited on backtrack.** This is correct Kuhn
behaviour: once a right node is visited in a DFS, other paths in the same DFS cannot
use it. Un-visiting on backtrack would cause infinite DFS loops or incorrect matching.

**3. `_l_ci` unused warning.** If you read `dfs_lci[lvl]` into a variable but only use
it in the augmentation branch (`chosen_r[lvl] = r_ci; ... dfs_lci[cur_lv]`), Rust warns
about unused `l_ci` at the top of the loop. Prefix with `_l_ci` to silence it — the
variable IS used indirectly via the augmentation walk's `dfs_lci[cur_lv]`.

**4. Differs from edge-index-scan Kuhn (V2.92).** `graph_bipartite_match_inner` uses
`while ei < MAX_EDGES` to scan all edges, `dfs_stk[(a_slot, ei)]` to resume after
backtrack, and `visited_b: [bool; MAX_NODES]`. The bitmask variant here is faster when
adjacency is already encoded as u128 bitmasks (as in V2.99's bipartite expansion).

**5. Stack sizes.** `dfs_lci`, `chosen_r` use `[usize; MAX_NODES]` (128 × 8 = 1 KB each).
`dfs_rem` uses `[u128; 128]` (128 × 16 = 2 KB). Total DFS state ≈ 4 KB per source.
Combined with outer `match_l/match_r/right_adj` arrays (≈ 4 KB total), stays within
kernel stack limits.

## When to use vs. the edge-index variant

| Pattern | Use when | Key arrays |
|---------|----------|------------|
| `gos-bipartite-match-augment-pattern` (V2.92) | Adjacency must be built from edge scan; undirected graph | `dfs_stk[(slot, ei)]`, `visited_b[bool]` |
| This skill (V2.99) | Adjacency already as `right_adj[ci]: u128`; directed graph | `dfs_rem[u128]`, `visited_r: u128` |

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_min_path_cover_inner` (V2.99)
- `right_adj[u_ci]` bitmask built from directed edges (see `gos-mpc-dag-bipartite-expansion`)
- `MAX_NODES = 128` → u128 bitmask fits exactly; guard: `if nc >= 128 { u128::MAX }`

## From this session

V2.99 implemented `graph_min_path_cover`. The bitmask Kuhn approach was chosen because
the bipartite expansion adjacency is naturally encoded as u128 bitmasks (one bit per
compact index), making `trailing_zeros()` the cheapest way to iterate right neighbors.
The `_l_ci` unused-variable warning was the only issue; fixed with underscore prefix.
All 10 harness tests passed with 0 warnings.
