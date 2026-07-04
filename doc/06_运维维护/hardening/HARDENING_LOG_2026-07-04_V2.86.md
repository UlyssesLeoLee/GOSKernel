# Hardening Log V2.86 — Graph Bridges (Cut Edges / Tarjan's Algorithm)

**Date:** 2026-07-04  
**Branch:** feat/vk-auto-live-surface  
**Commit:** 1b18cfb  
**Host-test total:** 833 (823 prior + 10 new)

---

## Feature: `graph bridges` / `gbridges` / `cut edges` / `gcute`

### Motivation

V2.85 added articulation point (cut vertex) detection — identifying nodes whose removal
disconnects the graph. V2.86 adds the natural **edge-dual** primitive: **bridge (cut edge)
detection** — identifying edges whose removal increases the number of connected components.

Together, cut vertices and cut edges form the foundational **2-connectivity** toolkit used
in production graph analysis platforms (NetworkX, igraph, Boost.Graph):

| Primitive | Removes | Disconnects if... |
|---|---|---|
| Articulation point (V2.85) | Node | It is the sole path between some pair |
| Bridge (V2.86) | Edge | It is the sole connection between two subgraphs |

In an OS dependency graph, a bridge edge represents a **single uplink** between two clusters
of kernel subsystems — analogous to a network link whose failure partitions the routing fabric.

OS analogy: a NIC or switch uplink with no redundant path — its removal silently isolates a
subnet (e.g., a leaf-spine topology where a leaf switch has exactly one uplink to the spine).

---

## Algorithm: Iterative Tarjan DFS — Bridge Detection

Bridge detection uses the same disc/low-link framework as articulation points (V2.85), but
with a **stricter condition** and **edge-indexed parent tracking**:

**Bridge condition:** `low[child] > disc[parent]`  (strictly `>`, not `≥`)
- Articulation point: `low[child] >= disc[parent]` (≥) — node is a single-point-of-failure
- Bridge: `low[child] > disc[parent]` (>) — no back-edge can even reach the parent itself

**Parent tracked by edge-index, not parent-slot:**

The critical difference from V2.85 is how the parent is tracked. If parent-slot is used,
two anti-parallel directed edges `A→B` and `B→A` would cause B to skip all edges to A
(treating both as the parent relationship). With edge-index tracking, B skips only the
specific edge it arrived from; the reverse edge `B→A` remains visible as a back-edge,
correctly setting `low[B] = disc[A]`, which prevents a false bridge detection.

| Approach | Anti-parallel A→B + B→A | Correct? |
|---|---|---|
| Parent-slot (V2.85 style) | Skips B→A entirely → `low[B]=disc[B]` → false bridge | ✗ |
| Parent-edge-index (V2.86) | B→A is a back-edge → `low[B]=disc[A]` → no bridge | ✓ |

**No root special case:** Unlike articulation points (which require a special DFS-root check
for nodes with ≥ 2 DFS children), bridge detection has no root special case. The condition
`low[child] > disc[parent]` applies uniformly at every non-root node.

**State arrays:**

| Array | Type | Meaning |
|---|---|---|
| `disc[slot]` | `u32` | DFS discovery time; `u32::MAX` = unvisited |
| `low[slot]` | `u32` | Min disc reachable from subtree via back-edges |
| `par_ei[slot]` | `usize` | Edge-index we arrived on; `MAX_EDGES` = root |
| `par_slot[slot]` | `usize` | Parent node slot (for bridge emit only) |

**Complexity:** O(V + E), no heap allocation, no_std safe.

---

## Implementation

### crates/gos-runtime/src/lib.rs

**New method** on `GraphRuntime` (inside `impl GraphRuntime`):
```rust
pub fn graph_bridges_inner<const N: usize>(&self)
    -> ([VectorAddress; N], [VectorAddress; N], usize, usize)
// returns (from_vecs, to_vecs, bridge_count, node_count)
```

**New public function:**
```rust
/// V2.86: Find all bridge edges (cut edges) in the undirected projection.
pub fn graph_bridges<const N: usize>()
    -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_bridges_inner()
}
```

**Return value:**
- `from_vecs[i]`, `to_vecs[i]`: canonicalized bridge endpoints (smaller `as_u64()` in `from`)
- `bridge_count`: number of bridges found (bounded by `N`)
- `node_count`: total live node count

**Key invariants:**
- Graph treated as undirected: both endpoint directions followed for each edge.
- Self-loops skipped: `nbr_slot == cur_slot` guard.
- Skip specifically the arrival edge by index (`par_ei[cur_slot]`), not all edges to parent slot.
- Back-edge update: `low[cur] = min(low[cur], disc[nbr])` for already-visited non-parent edges.
- Bridge emitted on pop when `low[child] > disc[parent]` (strictly >).
- Canonical order per bridge: `from = min(a,b)` by `as_u64()`, `to = max(a,b)`.
- Output sorted ascending by `(from.as_u64(), to.as_u64())` via insertion sort.

### crates/k-shell/src/lib.rs

**New function** `dispatch_graph_bridges(sink: &ConsoleSink)`:
- Header: ` graph bridges (cut edges)` (cyan)
- If `node_count == 0`: prints `(no nodes registered)`.
- If `bridge_count == 0`: prints green `no bridges (graph is 2-edge-connected or acyclic-free)`.
- Otherwise: lists each bridge in red as `bridge  <from>  ──  <to>`.
- Footer: `N bridge(s)  of  M node(s)  link resilience: 2-edge-connected / moderate risk / high risk`
  - `2-edge-connected`: bridge_count == 0 (green)
  - `moderate risk`: bridge_count ≤ node_count / 4 (yellow)
  - `high risk`: bridge_count > node_count / 4 (red)

### crates/k-shell/src/proc.rs

**New routing** (inserted after `graph articulation` / `gcutv` dispatch):
```
graph bridges   →  dispatch_graph_bridges
gbridges        →  alias
cut edges       →  alias
gcute           →  alias
```

---

## Test Harness: `host-tests/gos-graph-bridges-harness`

**VectorAddress L4=62** identifies this harness namespace.

| Test | Graph topology | Expected |
|---|---|---|
| 1 | Empty graph | bridge_count=0, node_count=0 |
| 2 | Single isolated node A | bridge_count=0, node_count=1 |
| 3 | A→B (single directed edge) | bridge_count=1, bridge=(A,B) |
| 4 | A→B→C→A (triangle) | bridge_count=0 (2-edge-connected) |
| 5 | A→B→C (path, 2 edges) | bridge_count=2, bridges=[(A,B),(B,C)] |
| 6 | A→B + B→A (anti-parallel) | bridge_count=0 (reverse is a back-edge) |
| 7 | Star H→{A,B,C,D} (4 spokes) | bridge_count=4 (all spokes are bridges) |
| 8 | Square A→B→C→D→A (4-cycle) | bridge_count=0 (2-edge-connected) |
| 9 | Two triangles + bridge C→F | bridge_count=1, bridge=(C,F) |
| 10 | Chain A→B→C→D (3 edges) | bridge_count=3, bridges=[(A,B),(B,C),(C,D)] |

**Result:** 10/10 pass.

---

## VectorAddress L4 Namespace Update

| L4 | Harness |
|---|---|
| 60 | gos-graph-link-predict-harness (V2.84) |
| 61 | gos-graph-articulation-harness (V2.85) |
| **62** | **gos-graph-bridges-harness (V2.86)** |

---

## Key Graph Theory Facts

A bridge (cut edge) {u, v} satisfies:
> Removing {u, v} from the undirected graph increases the number of connected components.

Equivalently (Tarjan's criterion for a DFS tree edge u→v):
> `low[v] > disc[u]`  — no vertex in v's subtree has a back-edge to u or any ancestor of u.

**Relation to other V2.x metrics:**
- Bridges complement articulation points (V2.85): every bridge's endpoints are articulation
  points (when the bridge is the only connection), but the converse is not true.
- A tree has exactly `n-1` bridges (every edge is a bridge); a 2-edge-connected graph has 0.
- Biconnected-component decomposition (a natural V2.87 candidate) partitions the graph into
  maximal 2-edge-connected subgraphs separated by bridges.
- Bridges map cleanly to `graph_global_efficiency` (V2.74): every bridge is a bottleneck that
  lengthens the average pairwise distance when removed.

**2-connectivity vs 2-edge-connectivity:**
- **2-connected** (no cut vertices): every pair of nodes has ≥ 2 internally vertex-disjoint paths.
- **2-edge-connected** (no bridges): every pair of nodes has ≥ 2 edge-disjoint paths.
- 2-connected implies 2-edge-connected; the converse is false.

---

## Literature Reference

- R. Tarjan, "Depth-First Search and Linear Graph Algorithms," SIAM J. Comput. 1(2), 1972.
  Original disc/low-link framework; V2.86 applies the strict `>` bridge condition with
  edge-indexed parent tracking for correctness on multi-edge / anti-parallel pairs.
- D. Eppstein, "Finding Bridges in Graphs," https://ics.uci.edu/~eppstein/ (lecture notes).
  Clarifies the edge-index vs node-index parent tracking distinction for multi-edge safety.
