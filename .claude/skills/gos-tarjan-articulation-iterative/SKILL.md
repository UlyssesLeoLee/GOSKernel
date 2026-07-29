---
name: gos-tarjan-articulation-iterative
description: When implementing Tarjan's articulation point (cut vertex) detection in GOSKernel, use an iterative DFS with (slot, edge_scan_index) stack frames; the root vs non-root AP conditions are DIFFERENT; and the parent guard must use slot equality not just the visited flag. Apply in crates/gos-runtime/src/lib.rs graph_articulation_inner.
---

# Tarjan Articulation Points: Iterative DFS with Root/Non-Root Distinction

## The rule

Articulation point detection requires two separate AP conditions — applying either
one to all nodes produces wrong results:

```rust
// ── Pop frame: propagate low and check AP ──────────────────────────────────
st_top -= 1;
let p = par[cur_slot];
if p != NO_PAR {
    // Propagate low upward first
    if low[cur_slot] < low[p] {
        low[p] = low[cur_slot];
    }
    // Non-root AP: child's subtree can't reach past parent via any back-edge
    if low[cur_slot] >= disc[p] && par[p] != NO_PAR {
        is_ap[p] = true;
    }
}
// Root AP: checked AFTER the full DFS tree for each start_slot
// (not inline, because dfs_children might still grow during the DFS)
if dfs_children[start_slot] >= 2 {
    is_ap[start_slot] = true;
}
```

Key constraints:
- **Non-root check**: `low[child] >= disc[parent]` AND `par[parent] != NO_PAR`.
- **Root check**: `dfs_children[root] >= 2` — done AFTER the DFS tree completes,
  not when popping the root frame. Each disconnected component has its own root.
- **Back-edge guard**: skip `low` update only when `nbr_slot == par[cur_slot]`,
  NOT when `disc[nbr_slot] != UNVISITED` in general. Visited non-parent ancestors
  ARE valid back-edges and MUST update `low[cur_slot]`.

## Why it's non-obvious

**Root vs non-root**: The low-link condition (`low[v] >= disc[u]`) is only correct for
non-root nodes. For a root, it would wrongly mark a root with one DFS child as an AP
(because `low[child] >= disc[root]` is always true when root has disc=0). The correct
root rule counts DFS-tree children pushed from the root.

**Parent guard precision**: In an undirected-projection DFS, edge u→v and v→u both exist.
When at v (arrived via parent u), we see the back-edge v→u (undirected). We must skip
updating `low[v] = min(low[v], disc[u])` — because u IS the tree-parent, not a back-edge
ancestor. The guard is `nbr_slot != par[cur_slot]`. Note: if both directed edges A→B and
B→A exist, both are correctly skipped (both resolve to parent slot).

**Propagation timing**: `low[parent] = min(low[parent], low[child])` must happen on pop,
AFTER the child's subtree is fully explored. Doing it on push (before the child DFS) would
propagate stale (initial) low values.

## Iterative DFS stack structure

```rust
const UNVISITED: u32 = u32::MAX;
const NO_PAR: usize  = MAX_NODES;   // sentinel: no parent = DFS root

let mut disc         = [UNVISITED; MAX_NODES];
let mut low          = [0u32; MAX_NODES];
let mut par          = [NO_PAR; MAX_NODES];
let mut dfs_children = [0u8; MAX_NODES];  // DFS-tree children per node
let mut is_ap        = [false; MAX_NODES];
let mut timer        = 0u32;

// Stack: (slot, edge_scan_start_index)
let mut dfs_stack: [(usize, usize); MAX_NODES] = [(0, 0); MAX_NODES];
let mut st_top = 0usize;

for each unvisited start_slot:
    disc[start_slot] = low[start_slot] = timer; timer += 1;
    dfs_stack[0] = (start_slot, 0);
    st_top = 1;

    while st_top > 0:
        fi = st_top - 1; (cur_slot, ei) = dfs_stack[fi]

        loop over edges from ei:
            if nbr_slot unvisited:          // tree edge
                disc/low[nbr] = timer++
                par[nbr] = cur_slot
                dfs_children[cur_slot] += 1
                dfs_stack[fi].1 = ei + 1   // save resume position
                push (nbr, 0)
                break
            elif nbr_slot != par[cur_slot]: // back edge (not to parent)
                low[cur] = min(low[cur], disc[nbr])

        if no child pushed:                 // pop
            st_top -= 1
            p = par[cur_slot]
            if p != NO_PAR:
                low[p] = min(low[p], low[cur])
                if low[cur] >= disc[p] and par[p] != NO_PAR:
                    is_ap[p] = true

    // Root check AFTER DFS tree:
    if dfs_children[start_slot] >= 2: is_ap[start_slot] = true
```

## GOSKernel context

- Implementation: `crates/gos-runtime/src/lib.rs`, `graph_articulation_inner<N>` (V2.85)
- Public wrapper: `gos_runtime::graph_articulation::<128>()` (no RUNTIME snapshot — uses `&self`)
- Returns `([VectorAddress; N], usize, usize)` = (art_vecs sorted ascending by as_u64(), art_count, node_count)
- Shell: "graph articulation" / "garticulate" / "cut vertices" / "gcutv"
- VectorAddress L4=61 reserved for gos-graph-articulation-harness test nodes
- Uses same `(slot, edge_scan_index)` stack frame pattern as `graph_scc_inner` Kosaraju Phase 1
- Undirected projection: both `from_node==cur_id` and `to_node==cur_id` edges yield neighbours

## From this session

V2.85: First compile attempt had `r.vec` instead of `r.vector` (NodeRecord field name).
After that fix, all 10 harness tests passed immediately — no algorithmic debugging needed
because the root/non-root distinction and parent guard were applied correctly from the design.

Test 9 (chain A-B-C-D) confirms `art_count=2` (B and C).
Test 10 (two triangles + bridge) confirms bridge endpoint detection: both C and F are cut vertices.
