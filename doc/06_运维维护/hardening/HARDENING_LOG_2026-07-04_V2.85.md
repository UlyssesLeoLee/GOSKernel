# Hardening Log V2.85 — Graph Articulation Points (Cut Vertices / Tarjan's Algorithm)

**Date:** 2026-07-04  
**Branch:** feat/vk-auto-live-surface  
**Commit:** fff103c  
**Host-test total:** 823 (813 prior + 10 new)

---

## Feature: `graph articulation` / `garticulate` / `cut vertices` / `gcutv`

### Motivation

Production graph analysis platforms (NetworkX, igraph, Gephi) provide **articulation point
(cut vertex) detection** as a fundamental network resilience primitive. An articulation point
is a node whose removal increases the number of connected components — the network's single
point of failure.

GOSKernel already had rich topology analytics (SCC, density, clustering, community detection,
link prediction) but lacked any mechanism to identify structurally critical nodes whose
absence would partition the kernel dependency graph.

V2.85 adds Tarjan's iterative disc/low-link DFS algorithm to detect all cut vertices of the
undirected projection of the live directed graph. This directly maps to an OS use case:
identifying which kernel subsystems, if removed or faulted, would leave other subsystems
unreachable through the dependency graph.

OS analogy: `systemctl list-dependencies --reverse` identifying single-point-of-failure
kernel services with no redundant dependency path.

---

## Algorithm: Tarjan's Iterative DFS for Articulation Points

Classic recursive Tarjan uses O(V) stack frames, which is unsafe in no_std kernels without
a guaranteed large stack. V2.85 uses a fully iterative version with an explicit DFS stack of
`(slot, edge_scan_index)` pairs — the same pattern used by the SCC Kosaraju implementation
(V2.34).

**State arrays (all slot-indexed, stack-allocated):**

| Array | Type | Meaning |
|---|---|---|
| `disc[slot]` | `u32` | DFS discovery time; `u32::MAX` = unvisited |
| `low[slot]` | `u32` | Minimum disc reachable from subtree via back-edges |
| `par[slot]` | `usize` | DFS parent slot; `MAX_NODES` = root / no parent |
| `dfs_children[slot]` | `u8` | Number of DFS-tree children pushed from this slot |
| `is_ap[slot]` | `bool` | True if this slot is an articulation point |

**Iteration protocol:**

1. For each unvisited node, push `(start_slot, 0)` onto the DFS stack.
2. At each frame `(cur_slot, ei)`:
   - Scan edges from `ei` onward (undirected projection: follow both `from_node==cur_id` and `to_node==cur_id`).
   - **Tree edge** (neighbour unvisited): set disc/low, set parent, increment `dfs_children[cur]`, push child frame, break.
   - **Back edge** (neighbour visited and not parent): `low[cur] = min(low[cur], disc[nbr])`.
3. **Pop** (no more neighbours):
   - Propagate: `low[par] = min(low[par], low[cur])`.
   - **Non-root AP check**: if `low[cur] >= disc[par]` and `par[par] != NO_PAR` → mark `par` as AP.
4. **Root AP check** after each DFS tree: if `dfs_children[root] >= 2` → mark root as AP.

**Output sorting:** Articulation point VectorAddresses are insertion-sorted ascending by
`as_u64()` before return, matching the convention of `graph_peripheral` / `graph_center`.

**Complexity:** O(V + E), no heap allocation, no_std safe.

---

## Implementation

### crates/gos-runtime/src/lib.rs

**New method** on `GraphRuntime` (inside `impl GraphRuntime`):
```rust
pub fn graph_articulation_inner<const N: usize>(&self)
    -> ([VectorAddress; N], usize, usize)
// returns (art_vecs, art_count, node_count)
```

**New public function:**
```rust
/// V2.85: Articulation points (cut vertices) of the live kernel graph.
pub fn graph_articulation<const N: usize>() -> ([VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_articulation_inner()
}
```

**Key invariants:**
- Graph is treated as **undirected**: both `from_node==cur_id` and `to_node==cur_id` are
  followed; neighbour = opposite endpoint.
- **Self-loops** skipped: `nbr_slot == cur_slot` check before all processing.
- **Parent guard** on back-edge update: `nbr_slot != par[cur_slot]` prevents treating the
  parent's tree edge as a back-edge in the undirected projection.
- **Root AP rule** uses `dfs_children[root] >= 2`, not low-link (low-link rule only applies
  to non-root nodes).
- **Non-root AP rule**: `low[child] >= disc[parent]` (≥, not >) — equality means child's
  subtree cannot reach any ancestor of parent.
- Results sorted ascending by `as_u64()` → deterministic ordering across test runs.
- `art_count` is bounded by `N` (buffer capacity); typical use is `N=128 = MAX_NODES`.

### crates/k-shell/src/lib.rs

**New function** `dispatch_graph_articulation(sink: &ConsoleSink)`:
- Header: ` graph articulation points`
- If `node_count == 0`: prints `(no nodes registered)`.
- If `art_count == 0`: prints green `no single points of failure (fully biconnected)`.
- Otherwise: lists each cut vertex in red with `cut vertex  <VectorAddress>`.
- Footer: `N cut vertices  of  M node(s)  resilience: fully biconnected / moderate risk / high risk`
  - `fully biconnected`: art_count == 0 (green)
  - `moderate risk`: art_count ≤ node_count / 4 (yellow)
  - `high risk`: art_count > node_count / 4 (red)

### crates/k-shell/src/proc.rs

**New routing** (inserted after `graph compare` / `gcompare` dispatch):
```
graph articulation   →  dispatch_graph_articulation
garticulate          →  alias
cut vertices         →  alias
gcutv                →  alias
```

---

## Test Harness: `host-tests/gos-graph-articulation-harness`

**VectorAddress L4=61** identifies this harness namespace.

| Test | Graph topology | Expected |
|---|---|---|
| 1 | Empty graph | art_count=0, node_count=0 |
| 2 | Single isolated node A | art_count=0, node_count=1 |
| 3 | A→B (single edge) | art_count=0 (removing either leaves 1-node component) |
| 4 | A→B→C (path) | art_count=1, cut=B |
| 5 | A→B→C→A (triangle) | art_count=0 (biconnected) |
| 6 | Star E→{A,B,C,D} | art_count=1, cut=E (centre) |
| 7 | Bowtie A-B-C-D-E sharing C | art_count=1, cut=C (shared apex) |
| 8 | Square A→B→C→D→A (4-cycle) | art_count=0 (biconnected) |
| 9 | Chain A-B-C-D (4-node path) | art_count=2, cuts=[B, C] sorted |
| 10 | Two triangles + bridge C→F | art_count=2, cuts=[C, F] (bridge endpoints) |

**Result:** 10/10 pass.

---

## VectorAddress L4 Namespace Update

| L4 | Harness |
|---|---|
| 60 | gos-graph-link-predict-harness (V2.84) |
| **61** | **gos-graph-articulation-harness (V2.85)** |

---

## Key Graph Theory Facts

An articulation point (cut vertex) v satisfies:
> ∃ s, t ≠ v such that every undirected path from s to t passes through v.

Equivalently (Tarjan's criterion):
- v is a **DFS root** with ≥ 2 DFS-tree children, OR
- v is a **non-root** with a child w where `low[w] ≥ disc[v]`
  (w's subtree cannot "reach back" past v to v's ancestors).

**Relation to other V2.x metrics:**
- Articulation points complement `graph_scc` (V2.34): SCCs identify strongly connected subgraphs;
  articulation points identify structural vulnerabilities in the weaker undirected sense.
- Articulation points are distinct from `graph_attractor` (V2.54) (bottom SCCs) — a node
  can be both an attractor and an articulation point.
- Bridge edges (edges whose removal disconnects the graph) can be detected by `low[v] > disc[u]`
  (strict inequality) for a tree edge u→v; this is a natural follow-on to V2.85.

---

## Literature Reference

- R. Tarjan, "Depth-First Search and Linear Graph Algorithms," SIAM J. Comput. 1(2), 1972.
  Original recursive algorithm; V2.85 uses the same disc/low-link criterion with an
  iterative stack to avoid recursion in no_std kernel context.
- J. Hopcroft & R. Tarjan, "Algorithm 447: Efficient Algorithms for Graph Manipulation,"
  CACM 16(6), 1973.  Practical biconnected-components formulation.
