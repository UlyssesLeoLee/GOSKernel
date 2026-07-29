---
name: gos-domtree-rpo-idom-pattern
description: When implementing a dominator tree in GOSKernel (graph_domtree_inner), use Cooper et al. 2001 iterative RPO + idom lattice join; the LCA walk climbs both fingers by RPO number; predecessors with undefined idom must be skipped; idom[start]=start is the lattice top. Apply in crates/gos-runtime/src/lib.rs graph_domtree_inner.
---

# Dominator Tree: Cooper et al. 2001 Iterative RPO Algorithm

## The rule

Compute the dominator tree by:
1. DFS from `start` → reverse post-order (RPO) numbering
2. Initialize `idom[start] = start`; all others `UNDEF`
3. Iterate in RPO order (skip start), computing `idom[b]` as the LCA of all processed predecessors

```rust
// Step 3 — iterate until no change
let mut changed = true;
while changed {
    changed = false;
    for rpo_i in 1..rpo_count {        // skip start at position 0
        let b = rpo_slots[rpo_i];
        let b_id = self.nodes[b].spec.node_id;
        let mut new_idom = UNDEF;

        for each predecessor p of b:
            if p == b { continue; }          // skip self-loops
            if idom[p] == UNDEF { continue; } // back edge not yet processed

            if new_idom == UNDEF {
                new_idom = p;
            } else {
                new_idom = intersect(p, new_idom, &idom, &rpo_num);
            }

        if new_idom != UNDEF && idom[b] != new_idom {
            idom[b] = new_idom;
            changed = true;
        }
    }
}

// LCA via RPO-number walk — terminates because rpo[idom[n]] < rpo[n] always
fn intersect(mut a: usize, mut c: usize, idom: &[usize; MAX_NODES], rpo: &[usize; MAX_NODES]) -> usize {
    while a != c {
        while rpo[a] > rpo[c] { a = idom[a]; }
        while rpo[c] > rpo[a] { c = idom[c]; }
    }
    a  // == c
}
```

## Why it's non-obvious

**Back-edge guard**: In the predecessor scan, any predecessor `p` whose `idom[p] == UNDEF`
has not yet been assigned a dominator. Including it in the join would corrupt the result.
Skip it — the iterative algorithm will pick it up in a later pass once `idom[p]` is defined.

**RPO order is mandatory**: Processing nodes in RPO ensures that for any tree edge `u → v`,
`u` is processed before `v`. This guarantees that at least one predecessor of `v` has a
defined `idom` on the first pass, so `new_idom` is never permanently UNDEF for reachable nodes.

**LCA termination**: The `intersect` walk terminates because `idom[n]` always has strictly
lower RPO number than `n` (closer to start = lower RPO). Start itself (rpo=0) is the
fixed point: `idom[start] = start`, `rpo[start] = 0` — the walk cannot go below 0.

**Diamond `A→{B,C}→D`: `idom[D] = A`**: Both B and C appear as predecessors of D.
`intersect(B, C)` walks: rpo[B]=rpo[C]=1 (same), walks both up to `idom[B]=idom[C]=A`,
returns A. This is correct — A is on every path from start to D.

**Cyclic graphs**: Back edges (e.g., C→B in A→B→C→B) have `idom[C] == UNDEF` on the
first pass when processing B. The guard skips C, idom[B] = A. On pass 2, idom[C]=B is
defined, intersect(A, B) = A, idom[B] = A (no change). Converges in 2 passes.

## DFS implementation for RPO (no recursion, no_std)

```rust
let mut stk_slot   = [0usize; MAX_NODES];
let mut stk_cursor = [0usize; MAX_NODES]; // edge-scan resume index per frame
let mut stk_depth  = 1usize;              // initialized to 1, not 0
stk_slot[0]         = start_slot;
stk_cursor[0]       = 0;
visited[start_slot] = true;

'dfs: while stk_depth > 0 {
    let cur = stk_slot[stk_depth - 1];
    let cur_id = self.nodes[cur].spec.node_id;
    let mut ei = stk_cursor[stk_depth - 1];
    while ei < MAX_EDGES {
        let edge = self.edges[ei]; // ...
        if edge.from == cur_id {
            if nbr != cur && !visited[nbr] {
                stk_cursor[stk_depth - 1] = ei + 1;  // save resume position
                visited[nbr] = true;
                stk_slot[stk_depth]   = nbr;
                stk_cursor[stk_depth] = 0;
                stk_depth += 1;
                continue 'dfs;                         // recurse
            }
        }
        ei += 1;
    }
    // all successors visited — emit in post-order
    post_buf[post_count] = cur; post_count += 1;
    stk_depth -= 1;
}
let rpo_count = post_count;  // assign after DFS, not before (avoids unused-assignment warning)
// RPO = reverse of post_buf
```

Note: initialize `stk_depth = 1` directly (not `0` then `= 1`) to avoid compiler unused-assignment warning.
Note: `let rpo_count = post_count;` (not `let mut rpo_count = 0; ... rpo_count = post_count;`) for the same reason.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_domtree_inner<N>` (V2.90)
- Public wrapper: `gos_runtime::graph_domtree::<64>(start: VectorAddress)` → `(vecs, idoms, node_count, reachable_count)`
- Shell: "graph domtree <v>" / "gdomtree <v>" / "dominator <v>" / "gdom <v>"
- VectorAddress L4=66 reserved for gos-graph-domtree-harness test nodes
- Contrast with `graph_articulation` (V2.85): domtree is directed + entry-specific; articulation is undirected + global
- Contrast with `graph_dag_layers` (V2.89): DAG-only (Kahn); domtree works on any directed graph

## From this session

V2.90 implemented cleanly on the first compile attempt after applying this design.
10/10 harness tests passed immediately. Key test validations:
- Test 6 (diamond): idom[D]=A ← confirms LCA walk correctly identifies A not B or C
- Test 8 (back edge): idom[B]=A, idom[C]=B ← confirms multi-pass convergence for cycles
- Test 10 (merge+extend): idom[D]=A, idom[E]=D ← confirms chain-after-merge is correct
