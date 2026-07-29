---
name: gos-kahn-selfloop-indegree-pattern
description: When implementing Kahn's BFS topological sort in GOSKernel, self-loops MUST be counted in in-degree (not skipped) but MUST be skipped during BFS relaxation — this asymmetry is what makes Kahn's correctly detect self-loops as cycles (is_dag=false). Apply in graph_dag_longest_inner and any future Kahn-based DAG detection.
---

# Kahn's BFS: Self-Loops Count in In-Degree, Skip in Relaxation

## The rule

When implementing Kahn's BFS for DAG detection and/or topological sort, handle self-loops
**asymmetrically**: include them in in-degree computation, but skip them in BFS relaxation:

```rust
// Step 1 — in-degree computation: DO count self-loops (fs == ts)
let mut in_deg = [0u16; MAX_NODES];
for ei in 0..MAX_EDGES {
    let edge = match self.edges[ei] { Some(e) => e, None => continue };
    let fs = match self.node_slot_by_id(edge.spec.from_node) { Some(s) => s, None => continue };
    let ts = match self.node_slot_by_id(edge.spec.to_node)   { Some(s) => s, None => continue };
    let _ = fs;  // self-loops: fs==ts intentional — KEEP THEM
    in_deg[ts] = in_deg[ts].saturating_add(1);
}

// Step 2 — seed BFS queue with in_deg == 0 nodes only
// (a self-loop node has in_deg >= 1 → never enters queue → correctly blocked)

// Step 3 — BFS relaxation: SKIP self-loops
while q_head < q_tail {
    let cur_slot = queue[q_head]; q_head += 1;
    // ...
    for ei in 0..MAX_EDGES {
        // ...
        if nbr_slot == cur_slot { continue; }  // ← SKIP self-loops in relaxation

        // relax: dist[nbr_slot] = max(dist[nbr_slot], dist[cur_slot] + 1)
        // decrement in_deg[nbr_slot]; if 0, enqueue
    }
}

// Step 4 — DAG check
let is_dag = processed == node_count;
// A self-loop node was never added to queue → processed < node_count → is_dag=false ✓
```

## Why it's non-obvious

The natural reflex is to skip self-loops **everywhere** (like the toposort harness does
for the main ordering pass). But if self-loops are also skipped in in-degree computation:

- `in_deg[A] = 0` for a node A with only a self-loop
- A is added to the BFS queue as a zero-in-degree source
- A is processed normally, `processed = 1 = node_count` → `is_dag = true` ← **wrong**

The fix is asymmetric: count self-loops in `in_deg` (so the node stays stuck), but skip
them during BFS relaxation (so they don't trigger infinite distance updates or corrupt
in-degree bookkeeping for other nodes).

**This is different from graph_toposort_inner (V2.x)** which skips self-loops in both
places and doesn't need to detect them — toposort just produces an ordering and may silently
ignore self-loops. The DAG-longest function needs exact is_dag semantics.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_dag_longest_inner()` (V2.88)
- Contrast with `graph_toposort_inner` which skips self-loops in in-degree for ordering
- Shell: `graph dag longest` / `gdaglongest` / `critical path` / `gcritical`
- VectorAddress L4=64 for `gos-graph-dag-longest-harness`

## From this session

V2.88 first draft skipped self-loops in in-degree computation (matched the existing
`graph_toposort_inner` pattern). Test 3 (self-loop A→A) failed:
- Expected: `is_dag=false`, `path_hops=0`
- Got: `is_dag=true`, `path_hops=0`

Root cause: `if fs != ts { in_deg[ts] += 1; }` excluded the self-loop from in-degree,
so A entered the BFS queue with `in_deg=0` and was processed normally.

Fix: removed the `if fs != ts` guard from in-degree (added `let _ = fs;` to suppress
unused-variable warning). Self-loop stays at `in_deg=1`, never enters queue,
`processed=0 < node_count=1`, `is_dag=false`. All 10 tests pass after the fix.
