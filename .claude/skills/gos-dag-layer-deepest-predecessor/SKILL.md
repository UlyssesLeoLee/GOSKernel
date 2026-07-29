---
name: gos-dag-layer-deepest-predecessor
description: When assigning topological layers to DAG nodes (parallel execution levels), propagate `layer[v] = max(layer[v], layer[u]+1)` — NOT just `layer[u]+1`. A shortcut edge from an early-layer node must NOT override a deeper predecessor's contribution. Apply whenever implementing Kahn BFS layer assignment in GOSKernel.
---

# DAG Layer Assignment: Deepest Predecessor Wins

## The rule

When computing topological layers (parallel execution levels) via Kahn BFS, update
each neighbor's layer using the **max** of its current layer and `(cur_layer + 1)`:

```rust
// WRONG — first predecessor to arrive wins (shortcut overrides deeper path)
layer[nbr_slot] = layer[cur_slot] + 1;

// CORRECT — deepest predecessor wins
let new_layer = layer[cur_slot].saturating_add(1);
if layer[nbr_slot] == u32::MAX || new_layer > layer[nbr_slot] {
    layer[nbr_slot] = new_layer;
}
```

Initialize unvisited nodes to `layer[v] = u32::MAX` (sentinel), source nodes to `layer[v] = 0`.
Only enqueue a node when `in_deg[v]` reaches 0 (all its predecessors have been processed).

## Why it's non-obvious

Consider a shortcut DAG: A→B→C + A→C.

With a naive `layer[v] = layer[u]+1` (first write wins):
- BFS processes A (layer 0) → sets layer[B]=1, layer[C]=1 (via shortcut A→C)
- BFS processes B (layer 1) → tries to set layer[C]=2, but C is already in queue at 1
- Result: layer[C]=1 ← **wrong** (C's deepest predecessor is B at layer 1, giving C layer 2)

With max propagation:
- layer[C] starts at u32::MAX
- A→C: layer[C] = max(MAX, 0+1) = 1
- B→C: layer[C] = max(1, 1+1) = 2 ← **correct**
- C is enqueued only when its last predecessor (B) fully processes it

This ensures that `layer[v]` reflects the **minimum number of sequential steps** required
before v can start — not just the layer of whichever predecessor happened to run first.

Note: this property is also why you must enqueue only when `in_deg[v]` drops to 0,
not when first visited. The last incoming edge may upgrade the layer.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_dag_layers_inner()` (V2.89)
- Harness: `host-tests/gos-graph-dag-layers-harness/`, test 8 (shortcut DAG) exercises this
- Contrast with V2.88 `graph_dag_longest_inner`: same max propagation, but V2.89 emits all layers, not just the longest chain endpoint
- Shell: "graph dag layers" / "gdaglayers" / "glayers" / "dag layers"
- VectorAddress L4=65 for `gos-graph-dag-layers-harness`

## From this session (2026-07-04)

Test 8 of gos-graph-dag-layers-harness:
```
// A->B->C + A->C (shortcut)
assert_eq!(find_layer(&vecs, &layers, nc, DL_VEC_C).unwrap(), 2, "C = max(A+1, B+1) = 2");
```
Without the max propagation, C would get layer 1 (set by shortcut A→C before B is processed)
instead of the correct layer 2 (set by B→C after B is drained from the queue).
