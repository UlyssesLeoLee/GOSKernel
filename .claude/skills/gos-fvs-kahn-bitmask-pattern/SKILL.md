---
name: gos-fvs-kahn-bitmask-pattern
description: When implementing feedback vertex set (FVS) in GOSKernel, use iterative Kahn BFS with a `live[ci]` mask: each round recompute in_deg/out_deg/adj (self-loops in in_deg, excluded from adj u128 bitmask), run Kahn, pick undrained node with max in_deg×out_deg score, mark dead; the self-loop asymmetry from gos-kahn-selfloop-indegree-pattern applies here too — count self-loops in in_deg, skip in adj.
---

# Feedback Vertex Set: Iterative Kahn BFS with u128 Adj Bitmask

## The rule

Use an outer loop over a `live[ci]` mask. **Every round**, recompute in_deg/out_deg/adj
from scratch (edges involving dead nodes are excluded). Run Kahn BFS. If all live nodes are
drained → acyclic, done. Otherwise pick the undrained node with max `in_deg × out_deg` and
mark it dead. Repeat until no cycles remain.

```rust
let mut live = [true; MAX_NODES];
let mut fvs_cis = [0u8; MAX_NODES];
let mut fvs_size = 0usize;

loop {
    let mut live_count = 0usize;
    for ci in 0..nc { if live[ci] { live_count += 1; } }
    if live_count == 0 { break; }

    // Recompute in_deg, out_deg, adj EACH round (live mask may have changed)
    let mut in_deg  = [0u32;  MAX_NODES];
    let mut out_deg = [0u32;  MAX_NODES];
    let mut adj     = [0u128; MAX_NODES]; // outgoing edges, self-loops excluded

    for ei in 0..MAX_EDGES {
        // ... look up fci and tci from edge; skip if !live[fci] || !live[tci]
        in_deg[tci]  = in_deg[tci].saturating_add(1);
        out_deg[fci] = out_deg[fci].saturating_add(1);
        if fci != tci && tci < 128 {  // ← guard: both exclude self-loops AND prevent u128 shift overflow
            adj[fci] |= 1u128 << tci;
        }
    }

    // Kahn BFS using bitmask adjacency
    let mut queue    = [0u8;   MAX_NODES];
    let mut q_head   = 0usize; let mut q_tail = 0usize;
    let mut in_queue = [false; MAX_NODES];
    let mut processed = 0usize;

    for ci in 0..nc {
        if live[ci] && in_deg[ci] == 0 { in_queue[ci] = true; queue[q_tail] = ci as u8; q_tail += 1; }
    }
    while q_head < q_tail {
        let ci = queue[q_head] as usize; q_head += 1; processed += 1;
        let mut nbrs = adj[ci];
        while nbrs != 0 {
            let tci = nbrs.trailing_zeros() as usize; nbrs &= nbrs - 1;
            if in_deg[tci] > 0 { in_deg[tci] -= 1; }
            if in_deg[tci] == 0 && !in_queue[tci] {
                in_queue[tci] = true; queue[q_tail] = tci as u8; q_tail += 1;
            }
        }
    }

    if processed == live_count { break; } // acyclic — done

    // Pick best undrained node: max in_deg × out_deg (= most cycle-crossing)
    let mut best_ci = usize::MAX; let mut best_score = 0u64;
    for ci in 0..nc {
        if live[ci] && !in_queue[ci] {
            let score = in_deg[ci] as u64 * out_deg[ci] as u64;
            if best_ci == usize::MAX || score > best_score { best_score = score; best_ci = ci; }
        }
    }
    if best_ci == usize::MAX { break; }

    fvs_cis[fvs_size] = best_ci as u8; fvs_size += 1;
    live[best_ci] = false;
}
```

## Why it's non-obvious

**Three independent subtleties in one algorithm:**

1. **Recompute every round.** Unlike single-pass Kahn, you must rebuild in_deg/out_deg/adj from scratch each round — the live mask changes, so last round's state is stale. Reusing it would give incorrect in-degrees for nodes adjacent to freshly-removed FVS nodes.

2. **Self-loops: count in in_deg, exclude from adj.** A self-loop A→A makes `in_deg[A] ≥ 1`. Kahn never dequeues A (its in-degree never drops to 0 from BFS propagation), so A stays `!in_queue` and correctly enters the FVS. If you also put A→A into `adj[A]`, Kahn would decrement `in_deg[A]` when processing A — but A is never processed (in_deg never hits 0), so the self-loop in adj is both harmless AND dangerous (if A somehow got processed, it would decrement itself). Safest and most correct: `if fci != tci` guard for adj.

3. **u128 shift guard `tci < 128`.** Since MAX_NODES = 128, compact index `ci` can be 0..127 (127 max), so `1u128 << 127` is safe. But the bound check `tci < 128` is required if any code path could yield tci = 128+ (e.g. after a `slot_to_ci` lookup). Always include it as a safety guard.

**Score choice:** `in_deg × out_deg` (not `in_deg + out_deg`). The product is 0 for any node with either in- or out-degree 0, ensuring we never waste an FVS slot on a node that can't be in any non-trivial cycle. The product strongly prefers nodes sitting at cycle "hubs" with multiple incoming and outgoing connections.

## Acyclicity guarantee

By construction: the loop terminates only when `processed == live_count` (all live nodes drained by Kahn). Since Kahn drains only DAGs, the remaining live set is always a DAG when the loop exits. The union of FVS nodes plus the final DAG = all original nodes. ✓

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs` — `graph_fvs_inner<const N: usize>` (V3.01)
- Public wrapper: `gos_runtime::graph_fvs::<128>()`
- Returns `([VectorAddress; N], usize, usize)` = (fvs_vecs, fvs_size, node_count)
- Output sorted ascending by `VectorAddress.as_u64()` (insertion sort over tmp[MAX_NODES])
- Shell: "graph fvs" / "gfvs" / "feedback vertex set" / "graph fvset"
- VectorAddress L4=77 for gos-graph-fvs-harness
- Complements `graph_feedback_arc` (V2.91): FAS removes edges; FVS removes vertices

## From this session

V3.01: FVS implemented in one pass without compile errors. The self-loop guard was designed
proactively based on the existing `gos-kahn-selfloop-indegree-pattern`. Key test: test_04
(`A→A` self-loop → fvs_size=1, FVS={A}) and test_09 (K4 complete directed → fvs_size=3 = n-1).
All 10 harness tests passed on first run.
