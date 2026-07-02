# Hardening Log — V2.46: `graph spanning` — BFS Spanning Forest

**Date:** 2026-07-02  
**Branch:** `feat/vk-auto-live-surface`  
**Commit:** (see below)  
**Author:** Claude (automated hardening run)

---

## Summary

V2.46 adds **`graph spanning`** — BFS spanning forest over the undirected projection of the GOS kernel graph. Every live node is assigned to a tree, with roots chosen in ascending slot order. The output shows each tree's parent–child structure with BFS depth, enabling operators to visualize the *minimal backbone* connecting kernel sub-systems.

Shell aliases: `graph spanning` / `spanning` / `span` / `graph span` / `graph tree` / `gtree`

OS analogy: `ip route show` / STP (Spanning Tree Protocol) — the minimal acyclic backbone connecting all kernel nodes without redundant cross-links.

---

## Motivation

After the community detection suite completed (V2.45), the next natural primitive is the **spanning tree / spanning forest**: a structural backbone view that reveals which parent–child relationships form the minimum acyclic connector of the live graph.

Spanning tree analysis in a graph OS answers:
- What is the shortest path tree rooted at the most-connected sub-system?
- Which nodes are leaves (no children) vs branches (internal connectors)?
- How many disconnected components exist (tree count = component count)?
- What is the maximum depth (longest tree arm) in the kernel's connectivity graph?

This also completes the classical graph primitives set: connectivity analysis (cycles, toposort, SCC, condensation, reachability, bipartite) → metric analysis (degree, centrality, closeness, eccentricity, Katz, PageRank, HITS) → clustering (community) → structural backbone (spanning).

---

## Algorithm: BFS Spanning Forest (Undirected Projection)

```text
Initialize: visited[v] = false for all v

For each node root in ascending slot order:
  If visited[root]: skip
  tree_count++
  visited[root] = true
  parent[root] = root   // root is its own parent
  depth[root] = 0
  BFS queue = [root]

  While queue not empty:
    cur = dequeue
    Emit cur to output (BFS order)
    For each neighbor nb of cur (both in-edges and out-edges treated as undirected):
      If nb == cur: skip  // no self-loops
      If visited[nb]: skip
      visited[nb] = true
      parent[nb] = cur
      depth[nb] = depth[cur] + 1
      enqueue nb

Output: (vecs, parents, depths, node_count, tree_count)
```

**Key design choices:**

1. **Undirected treatment**: both in-edges and out-edges are used as undirected neighbor links (same as `graph community` and `graph bipartite`). This ensures coverage of all live nodes regardless of signal direction — consistent with how the GOS kernel graph is viewed as an undirected service mesh for structural analysis.

2. **Root selection: ascending slot order** — the node with the lowest slot index in each connected component becomes the root. This produces deterministic, reproducible output.

3. **BFS (not DFS)** — BFS minimizes depth values (shortest-path tree from root). This gives operators the most balanced view of the graph structure: all nodes at the same "hop distance" from the root appear at the same depth level.

4. **Root parent = self** — a root node's parent vector is set to its own vector (`parents[i] == vecs[i]`). This makes roots easy to detect without a separate flag array.

5. **Output in BFS visit order** — nodes are emitted as BFS visits them: root (depth 0), then all depth-1 children, then all depth-2 grandchildren, etc. Trees are emitted one after another.

**Complexity:** O(V + E) per call — single BFS pass over all live nodes and edges.

**Space:** O(MAX_NODES) = O(128) — all fixed-size stack arrays, no_std/no_alloc compatible.

---

## Implementation

### `crates/gos-runtime/src/lib.rs`

- **`RuntimeState::graph_spanning_inner<const N>()`** — core BFS spanning forest algorithm.
  - Iterates `snap.node_slots[0..node_count]` in slot order.
  - Per-slot BFS queue (`[usize; MAX_NODES]`).
  - Tracks `visited[MAX_NODES]`, `parent_slot[MAX_NODES]`, `depth_arr[MAX_NODES]`.
  - Emits slots in BFS visit order into `out_slots[MAX_NODES]`, then maps to vec/parent/depth output arrays.

- **`pub fn graph_spanning<const N>()`** — free function wrapper:
  ```rust
  pub fn graph_spanning<const N: usize>(
  ) -> ([VectorAddress; N], [VectorAddress; N], [u8; N], usize, usize)
  ```
  Locks `RUNTIME`, calls `topology_snapshot()`, delegates to `graph_spanning_inner`.

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_spanning(sink)`** — display function:
  - Header: cyan `graph spanning`
  - Per-tree block: `[T0]  root: [vec]  ──  N nodes`
  - Column header: `depth  vector  parent  role`
  - Per node: depth, vector (color-coded by role), parent (`(root)` for roots), role label
  - Footer: `N node(s)  BFS spanning-forest  trees: M`
  - Colors: magenta (13) = root, cyan (11) = branch (has children), white (7) = leaf

- **Role classification** (computed post-call by scanning for children):
  - `root` — depth 0
  - `branch` — depth ≥ 1 and at least one other node has this node as parent
  - `leaf` — depth ≥ 1 and no children in the spanning tree

### `crates/k-shell/src/proc.rs`

- Dispatch:  
  `"graph spanning" | "spanning" | "span" | "graph span" | "graph tree" | "gtree"` → `dispatch_graph_spanning`
- Help text: two new lines documenting the command and its aliases.

---

## Test Harness: `host-tests/gos-graph-spanning-harness`

10 tests covering the full spanning forest API:

| # | Scenario | Assertion |
|---|----------|-----------|
| 1 | Empty graph | total=0, tree_count=0 |
| 2 | Single node | 1 node, 1 tree, depth=0, parent=self |
| 3 | Two isolated nodes (no edge) | 2 trees, both depth 0, both parents=self |
| 4 | Single edge A→B | 1 tree: A root (depth 0), B child (depth 1), B.parent=A |
| 5 | Chain A→B→C→D | 1 tree, depths 0/1/2/3; parent chain B←A, C←B, D←C |
| 6 | Directed triangle A→B→C→A | 1 tree, all depths ≤ 2 (cycle = 1 undirected component) |
| 7 | Two disconnected pairs (A─B, C─D) | 2 trees, exactly 2 roots |
| 8 | Root parent == self | For all i where depth[i]==0: parents[i]==vecs[i] |
| 9 | Non-root parent is a known node | For all non-root i: parents[i] appears in vecs[0..total] |
| 10 | tree_count matches connected components | 3 isolated → 3 trees; 2+1 → 2 trees |

**Result:** 10/10 pass, zero warnings.

---

## Node Role Semantics

| Role | Condition | Color |
|------|-----------|-------|
| `root` | depth == 0 | Magenta (13) |
| `branch` | depth ≥ 1 and has at least one child in the spanning tree | Cyan (11) |
| `leaf` | depth ≥ 1 and no children in the spanning tree | White (7) |

---

## Shell Command Surface

```text
graph spanning     BFS spanning forest over all live nodes (minimal backbone)
spanning           alias
span               alias
graph span         alias
graph tree         alias
gtree              alias
```

Example output (two sub-systems):

```text
 graph spanning
 ───────────────────────────────────────────────────────────
  [T0]  root: [1:0:1:0]  ──  4 nodes
    depth  vector           parent           role
    0      [1:0:1:0]        (root)           root
    1      [1:0:2:0]        [1:0:1:0]        branch
    1      [1:0:3:0]        [1:0:1:0]        leaf
    2      [1:0:4:0]        [1:0:2:0]        leaf

  [T1]  root: [2:0:1:0]  ──  2 nodes
    depth  vector           parent           role
    0      [2:0:1:0]        (root)           root
    1      [2:0:2:0]        [2:0:1:0]        leaf
 ───────────────────────────────────────────────────────────
  6 node(s)  BFS spanning-forest  trees: 2
```

---

## Invariants Preserved

- **No write ops**: `graph_spanning` is a pure read (no epoch bump, no mutation).
- **No alloc / no_std**: all buffers are fixed-size stack arrays.
- **TEST_LOCK + reset()**: harness uses the standard isolation pattern.
- **Sequential version**: V2.46 follows V2.45 (community) directly.
- **Doc archived**: this file at `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.46.md`.

---

## Next Steps

Suggested V2.47 candidates:
- `node checkpoint <vec>` — snapshot node state to the per-node diff ring (observability)
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph sim <N>` — simulate N random-walk steps, emit signal traffic trace
- `graph mst` — minimum spanning tree (Prim's/Kruskal's, uses edge weights)
- PAL_U32 → attribute node refactor (Demo A prerequisite)
