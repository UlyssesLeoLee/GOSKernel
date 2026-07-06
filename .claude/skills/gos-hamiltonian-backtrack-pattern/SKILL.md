---
name: gos-hamiltonian-backtrack-pattern
description: When implementing iterative backtracking DFS for path-finding (Hamiltonian, TSP-prefix, etc.) in GOSKernel no_std, use flat arrays path[]/cand[]/visited indexed by depth — NOT frame structs; cand[d] holds remaining successors of path[d] for position d+1; dead-end pruning via counting ≥2 unvisited sink nodes prunes exponential branches early; add a step limit for NP-hard algorithms.
---

# Iterative Backtracking DFS: Flat-Array Depth-Indexed Pattern

## The rule

For Hamiltonian-style path-finding, use three flat arrays indexed by depth (not a stack of frame structs):

```rust
let mut path    = [0u8;   MAX_NODES]; // path[0..depth] = current partial path (compact ci)
let mut cand    = [0u128; MAX_NODES]; // cand[d] = remaining adj of path[d] not yet tried
let mut visited = 0u128;              // bitmask of nodes in path[0..depth]

// Push initial start node:
path[0]  = start_ci as u8;
visited  = 1u128 << start_ci;
cand[0]  = adj[start_ci] & !visited; // candidates for depth=1 (successors of start)
depth    = 1;

'inner: loop {
    if depth == nc {
        // Full Hamiltonian path found — check circuit (last → start edge?)
        let last = path[nc - 1] as usize;
        if (adj[last] >> start_ci) & 1 != 0 { has_circuit = true; break 'start_loop; }
        // Not circuit: backtrack
        depth  -= 1;
        visited &= !(1u128 << path[depth]);
        continue 'inner;
    }

    let d = depth - 1;  // ← KEY: cand[d] holds candidates for POSITION depth (= path[depth])
    if cand[d] == 0 {
        if depth == 1 { break 'inner; }   // exhausted all from this start_ci
        depth  -= 1;
        visited &= !(1u128 << path[depth]);
    } else {
        let v = cand[d].trailing_zeros() as usize;
        cand[d] &= cand[d] - 1; // remove v BEFORE dead-end check and push

        // Dead-end pruning: count unvisited nodes with no unvisited successors.
        let next_visited = visited | (1u128 << v);
        let unvisited    = all_mask & !next_visited;
        let remaining    = nc.saturating_sub(depth + 1);
        if remaining > 1 {
            let mut dead_ends = 0usize;
            let mut um = unvisited;
            while um != 0 {
                let w = um.trailing_zeros() as usize; um &= um - 1;
                if adj[w] & unvisited == 0 { dead_ends += 1; if dead_ends > 1 { continue 'inner; } }
            }
        }

        // Push v
        path[depth]  = v as u8;
        visited      = next_visited;
        cand[depth]  = adj[v] & !visited;
        depth       += 1;
    }
}
```

**Step limit for NP-hard algorithms:**
```rust
let mut steps: u64 = 0;
const STEP_LIMIT: u64 = 5_000_000;
// In the inner loop: steps += 1; if steps > STEP_LIMIT { break 'start_loop; }
```

**Outer loop over start nodes** (for Ham. path) or single fixed start (for Ham. circuit from known start):
```rust
'start_loop: for start_ci in 0..nc {
    if has_circuit { break; } // can't do better than circuit
    // ... push start, run 'inner ...
}
```

## The cand[d] invariant (the hardest part)

`cand[d]` stores remaining successors of `path[d]` that we haven't yet tried for position `d+1`.

- **Set when pushing `path[d]`:** `cand[d] = adj[path[d]] & !visited_at_that_time`
- **Remove before branching:** `cand[d] &= cand[d] - 1` removes the chosen `v` BEFORE pushing it
- **Read when backtracking to depth `d+1`:** look at `cand[depth-1]` where `depth = d+1`

After backtracking: `cand[d]` already has the remaining untried candidates. Never re-add previously tried candidates — each is tried exactly once per position per start node.

**d = depth - 1 always** when looking for the next candidate at the current depth. This is the source of most off-by-one bugs.

## Dead-end pruning (sound, not exhaustive)

After tentatively visiting node `v`, compute `unvisited = all_mask & !(visited | (1<<v))`.  
Count nodes `w ∈ unvisited` where `adj[w] & unvisited == 0` (no successors within remaining unvisited).  
**If ≥2 such "sink" nodes:** at most one can be the path terminus → impossible to complete → prune.

This pruning is SOUND (never prunes valid paths) because:
- Each "sink" node's only position in the path can be the very last node
- Two sinks can't both be last → contradiction → pruning valid
- Apply only when `remaining > 1` (if exactly 1 node left to place, sinks are irrelevant)

The inner bitmask scan is O(n) per step — acceptable for GOSKernel's MAX_NODES=128.

## Single-node and empty special cases

```rust
if nc == 0 { return (out_vecs, 0, false, false, 0); }
if nc == 1 {
    // Single node trivially satisfies Hamiltonian (visits the one node, returns to it)
    out_vecs[0] = node_vec[0]; return (out_vecs, 1, true, true, 1);
}
```

## Step limit rationale

Hamiltonian is NP-complete. For adversarial dense graphs, backtracking without pruning visits O(n!) states. The step limit prevents infinite loops in kernel context:
- 5_000_000 steps: terminates in <100ms on worst-case OS graphs seen in GOSKernel (≤50 nodes, sparse)
- If limit hit: return `path_len=0, has_path=false` (conservative — reports "not found" even if inconclusive)
- OS dependency graphs are typically sparse DAG-like structures → rarely hit the limit in practice

Apply same pattern to any future NP-hard algorithm: TSP prefix search, exact chromatic number, Steiner tree, etc.

## Difference from gos-bk-clique-iterative-pattern

| Aspect | BK Clique (gos-bk-clique-iterative-pattern) | Hamiltonian Backtrack |
|--------|----------------------------------------------|----------------------|
| State per level | BkFrame struct (r, p, x, to_try, came_from_v) | Flat arrays path[], cand[], visited |
| Parent update | When POPPING child frame, via came_from_v | Not needed (visited bitmask handles it) |
| Termination check | `p == 0` or `to_try == 0` | `depth == nc` (full path) |
| Pruning | Tomita pivot (minimises branching on P∩X) | Dead-end sink count |
| Multiple starts | No (BK runs all maximal cliques from root) | Yes (outer loop over start_ci) |

Use the flat-array pattern for PATH-FINDING; use BkFrame struct for SET-ENUMERATION (clique, IS).

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs` — `graph_hamiltonian_inner<const N: usize>` (V3.03)
- Public wrapper: `gos_runtime::graph_hamiltonian::<128>()`
- Returns `([VectorAddress; N], usize, bool, bool, usize)` = (path_vecs, path_len, has_circuit, has_path, node_count)
- Shell: "graph hamiltonian" / "gham" / "hamiltonian" / "ham circuit" / "hamiltonian path"
- VectorAddress L4=79 for gos-graph-hamiltonian-harness
- Stack usage: adj (2KB) + path (128B) + cand (2KB) + best_path (128B) ≈ 4.5KB

## From this session

V3.03: Implemented in one pass, all 10 tests green on first compile. The dead-end pruning was designed during algorithm planning — specifically test_09 (pure diamond A→B, A→C, B→D, C→D → no Ham path) validated the pruning by correctly returning path_len=0.

Key correctness insight discovered during implementation: the `depth == nc` backtrack must use `depth -= 1; visited &= !(1u128 << path[depth]);` (removing `path[nc-1]`, the newly-written last node), NOT `path[nc]` (out of bounds). After decrement, `path[depth]` correctly refers to the last pushed node.
