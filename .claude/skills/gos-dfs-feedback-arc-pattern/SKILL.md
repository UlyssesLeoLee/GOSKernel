---
name: gos-dfs-feedback-arc-pattern
description: When implementing feedback arc set (cycle-causing back-edges) in GOSKernel, use iterative DFS with 3-colour colouring (UNVISITED/IN_STACK/DONE); self-loops need NO special case; cross/forward edges (DONE) must be skipped; NO parent tracking is needed unlike articulation/bridges.
---

# DFS 3-Colour Feedback Arc Set Pattern

## The rule

Use UNVISITED(0)/IN_STACK(1)/DONE(2) colouring to classify edges.
Only edges to IN_STACK nodes are back-edges (= feedback arcs).

```rust
const UNVISITED: u8 = 0;
const IN_STACK:  u8 = 1;
const DONE:      u8 = 2;

let mut color = [UNVISITED; MAX_NODES];

// Stack: (slot, next_edge_index_to_scan)
let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];

for each unvisited start_slot:
    color[start_slot] = IN_STACK;
    push (start_slot, 0);

    while stack not empty:
        (cur_slot, ei) = top frame
        cur_id = nodes[cur_slot].spec.node_id

        found_child = false
        loop ei from scan_start:
            edge = edges[ei] (skip None / skip if from != cur_id)
            nbr_slot = node_slot_by_id(edge.to_node)

            match color[nbr_slot]:
                UNVISITED =>
                    color[nbr] = IN_STACK
                    save ei+1 as resume in current frame
                    push (nbr, 0)
                    found_child = true; break
                IN_STACK =>
                    // BACK-EDGE: record as feedback arc
                    from_vecs[arc_count] = nodes[cur_slot].vector
                    to_vecs[arc_count]   = nodes[nbr_slot].vector
                    arc_count += 1
                DONE => {}   // forward/cross edge — not a back-edge

            ei += 1

        if !found_child:
            color[cur_slot] = DONE
            pop frame
```

Key differences from articulation/bridge DFS:
- **No parent tracking** — FAS only needs colour state, not disc/low-link
- **No root special case** — the root check in articulation has no analogue here
- **No back-edge update** — just record the arc; no low-link propagation needed
- **Self-loops are natural** — nbr_slot==cur_slot means color[nbr]==IN_STACK → recorded automatically

## Why it's non-obvious

**Self-loops**: Many implementations skip `nbr_slot == cur_slot` at the top of the loop
(as articulation/bridges do). For FAS, this is WRONG — a self-loop IS a feedback arc.
The 3-colour check handles it automatically: cur_slot's colour is always IN_STACK when
the self-edge fires, so it falls into the `IN_STACK` arm and gets recorded.

**Cross/forward edges**: In directed DFS, an edge to a DONE node is a cross (to a
completed sibling subtree) or forward (to a descendant already finished) edge. Neither
is a back-edge. Skipping DONE nodes is REQUIRED to avoid false positives.

**No parent guard needed**: Articulation/bridges skip back-edges to the parent (to avoid
treating the tree-parent edge as a back-edge in undirected DFS). FAS operates on directed
edges only — there is no "undirected parent" to guard against.

**Result is a valid FAS but not minimum**: Minimum FAS (MFAS) is NP-hard (Karp 1972).
DFS back-edges form a valid FAS — removing them makes the graph acyclic — but the count
may exceed the minimum. Never claim minimality.

## Correctness invariant

`arc_count == 0` if and only if `graph_dag_layers` returns `is_dag == true`.
Cross-check this in test 10 of any FAS harness.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_feedback_arc_inner<N>` (V2.91)
- Public wrapper: `gos_runtime::graph_feedback_arc::<512>()` (cap at MAX_EDGES)
- Returns `([VectorAddress; N], [VectorAddress; N], usize, usize)` = (from_vecs, to_vecs, arc_count, node_count)
- Output sorted ascending by `(from.as_u64(), to.as_u64())`
- Shell: "graph feedback arc" / "gfas" / "feedback arc" / "gcycledges"
- VectorAddress L4=67 reserved for gos-graph-fas-harness test nodes
- k-shell dispatch uses `MAX_EDGES` (512) as the const generic N

## From this session

V2.91: Implemented without errors on first compile. The key design choice was
deliberately NOT skipping self-loops before the colour check (unlike articulation/bridges).
Test 3 (`A→A` self-loop → arc_count=1) and test 10 (dag_layers cross-check) validate both
the self-loop handling and the DAG consistency invariant.
