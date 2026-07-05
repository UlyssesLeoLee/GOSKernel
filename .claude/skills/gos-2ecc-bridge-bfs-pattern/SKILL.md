---
name: gos-2ecc-bridge-bfs-pattern
description: When implementing 2-edge-connected components in GOSKernel, run Tarjan bridge-finding (Phase 1, identical to graph_bridges_inner) to mark is_bridge[] by edge-slot index, then BFS ignoring bridge edges (Phase 2) to assign one component ID per vertex. Every vertex belongs to exactly one 2ECC — simpler API than BCC. Connected-graph invariant: comp_count = bridge_count + 1. Apply in crates/gos-runtime/src/lib.rs graph_2ecc_inner.
---

# 2-Edge-Connected Components: Two-Phase Bridge + BFS Pattern

## The rule

`graph_2ecc_inner` uses exactly two phases, both O(V+E):

**Phase 1 — Tarjan bridge-finding (copy of graph_bridges_inner logic):**
```rust
let mut is_bridge = [false; MAX_EDGES];
// ... same DFS as graph_bridges_inner ...
// On pop: if low[child] > disc[parent] (strictly >):
if low[cur_slot] > disc[p] {
    let ei_b = par_ei[cur_slot];  // edge-index, NOT slot
    if ei_b < MAX_EDGES { is_bridge[ei_b] = true; }
}
```

**Phase 2 — BFS on non-bridge undirected edges:**
```rust
let mut comp_slot: [u8; MAX_NODES] = [u8::MAX; MAX_NODES];
for each unvisited start_slot:
    assign cid = comp_count; comp_count += 1;
    BFS queue from start_slot, traversing edges where !is_bridge[ei]
    all reached nodes get comp_slot[slot] = cid
```

The `is_bridge[]` array (bool[512]) sits on the stack and bridges Phase 1 output to Phase 2 input — no second RUNTIME lock needed since both phases run inside the same `_inner` method.

## Key invariant

For any **connected** undirected graph:
```
comp_count == bridge_count + 1
```
This cross-validates `graph_2ecc` against `graph_bridges` (used in test 10 of gos-graph-2ecc-harness).

## Why it's non-obvious

**2ECC vs BCC (bi-connected components) API difference:**
- BCCs are edge-based: articulation points belong to MULTIPLE blocks, so a simple per-vertex label array isn't valid for BCC.
- 2ECCs are vertex-based: every node belongs to **exactly one** 2ECC. This makes a clean `comp_ids[0..node_count]` output array correct.

The temptation is to implement the more general BCC (used in compiler liveness analysis), but 2ECC is strictly simpler for the GOSKernel use case (fault-tolerant link clusters) and produces cleaner output.

**Phase 2 BFS also needs the self-loop guard:**
```rust
if nbr_slot == cur_slot { continue; } // self-loops in Phase 2 BFS too
```
A self-loop would re-enqueue the same node, which is harmless (comp_slot is already set), but wastes a BFS slot.

**Phase 2 is NOT a re-run of bridge-finding.** It's a plain BFS on the undirected edge set with one filter: `if is_bridge[ei] { continue; }`. No disc[], low[], or par_ei[] needed.

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_2ecc_inner<N>` (V2.93)
- Public wrapper: `gos_runtime::graph_2ecc::<128>()` → `([VectorAddress; N], [u8; N], usize, usize)`
- Returns `(vecs, comp_ids, node_count, comp_count)` sorted by `(comp_id, vecs.as_u64())`
- Shell: `"graph 2ecc"` / `"g2ecc"` / `"2ecc"` / `"edge connected components"`
- VectorAddress L4=69 for gos-graph-2ecc-harness test nodes
- `comp_ids[i]` is `u8` (0–254); capped at 254 for graphs with > 254 components
- Complements: `graph_bridges` (V2.86, cut edges), `graph_articulation` (V2.85, cut vertices)

## From this session

V2.93: The `is_bridge[]` array design emerged from recognizing that Phase 2 cannot call `graph_bridges()` (that would require a second RUNTIME lock). Instead, Phase 1 writes into `is_bridge[MAX_EDGES]` on the stack, and Phase 2 reads from it — both inside the same `RUNTIME.lock()` hold. The implementation compiled and all 10 tests passed on the first run with no debugging required.

Test 7 (`two_triangles_sharing_one_edge`): edge A-B appears in two separate cycles (A-B-C-A and A-B-D-A), so `low[B] ≤ disc[A]` (cycle back-edge reaches A) → A-B is NOT a bridge → all 4 nodes {A,B,C,D} are in one 2ECC. This is the key correctness case: shared edges in two cycles are never bridges.
