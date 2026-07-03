# Hardening Log — V2.47: `graph color` — Welsh-Powell Greedy Graph Coloring

**Date:** 2026-07-02  
**Branch:** `feat/vk-auto-live-surface`  
**Commit:** (see below)  
**Author:** Claude (automated hardening run)

---

## Summary

V2.47 adds **`graph color`** — Welsh-Powell greedy graph coloring over the undirected projection of the live GOS kernel graph. Every live node is assigned a color index (0-based) such that no two directly connected nodes share the same color. Nodes are processed in descending total-degree order (Welsh-Powell heuristic), then the smallest available color is assigned greedily.

Shell aliases: `graph color` / `color` / `gcolor` / `graph colour` / `colour`

OS analogy: colors = conflict-free scheduling domains / CPU-affinity groups — like Linux `cgroups cpuset.cpus` assignments or NUMA node binding. Each color represents a set of kernel subsystems that can be scheduled, locked, or isolated without resource conflict.

---

## Motivation

After the structural backbone view (V2.46 spanning forest), the natural next primitive is **graph coloring**: partitioning nodes into conflict-free groups. In a kernel graph OS context:

- **Graph coloring = scheduling domain assignment** — subsystems that share edges (signal routes) must not be in the same domain to avoid priority inversion or lock contention.
- **Chromatic number** = minimum number of isolation domains required to run all subsystems conflict-free.
- Provides a compact answer to: *"How many independent scheduling lanes does this kernel topology need?"*
- Useful for resource planning: if chromatic number = 2, the kernel can run in two fully isolated execution environments.

---

## Algorithm: Welsh-Powell Greedy Graph Coloring

```text
Step 1 — Compute total (undirected) degree for each live node:
  For each live edge (u→v):
    degree[u] += 1
    degree[v] += 1 (unless self-loop)

Step 2 — Sort nodes in descending degree order (Welsh-Powell ordering):
  order[] = live slots sorted by degree[slot] descending

Step 3 — Greedy assignment in sorted order:
  color_slot[] = NOT_COLORED
  For each slot s in order[]:
    Mark all colors already used by s's undirected neighbors as forbidden
    Assign s the smallest non-forbidden color
    Track max color assigned so far

Step 4 — Pack output in sorted order:
  out_vecs[i]   = snap.slot_vec[order[i]]
  out_colors[i] = color_slot[order[i]]
  chromatic     = max_color + 1  (or 0 if no nodes)

Output: (vecs, colors, node_count, chromatic_number)
```

**Key design choices:**

1. **Undirected treatment**: treats every directed edge as undirected (consistent with `graph community`, `graph bipartite`, `graph spanning`). Signal direction is irrelevant for scheduling conflict analysis.

2. **Welsh-Powell ordering**: descending degree → highest-degree (most-connected) nodes are colored first. Guarantees the center node gets color 0 (lowest-index color = highest-priority domain).

3. **Forbidden-color tracking**: per-iteration reset (`forbidden[ci] = false`) instead of `memset(0)` on all 256 bytes. Only the colors actually used by neighbors are reset, keeping the inner loop O(E) across all iterations rather than O(V × 256).

4. **Chromatic number is a greedy upper bound**: optimal chromatic number is NP-hard in general. Welsh-Powell is optimal for paths, bipartite graphs, complete graphs, and star graphs (the common kernel topology patterns), and provides a practical upper bound for real topologies.

5. **Role labels**: `center` (color 0 = first assigned, highest-degree node), `domain-N` (color N > 0 = conflict group N). `isolated` nodes (no edges) receive color 0 by default.

**Complexity:** O(V·E) per call — O(E) degree scan + O(V²) sort (n≤128) + O(V·E) greedy assignment.  
**Space:** O(MAX_NODES) = O(128) — all fixed-size stack arrays, no_std/no_alloc compatible.

---

## Implementation

### `crates/gos-runtime/src/lib.rs`

- **`RuntimeState::graph_color_inner<const N>()`** — core Welsh-Powell coloring algorithm:
  - Step 1: degree scan over all live edges (`O(MAX_EDGES)`).
  - Step 2: insertion sort in descending degree order (`O(N²)`, N≤128).
  - Step 3: greedy assignment with per-iteration forbidden-flag reset.
  - Step 4: pack `out_vecs` / `out_colors` in sorted order.

- **`pub fn graph_color<const N>()`** — public free function:
  ```rust
  pub fn graph_color<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, u8)
  ```
  Locks `RUNTIME`, calls `topology_snapshot()`, delegates to `graph_color_inner`.

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_color(sink)`** — display function:
  - Header: cyan `graph color`
  - Summary line: `chromatic number: N   nodes: M`
  - Column header: `color  vector  role`
  - Per node: color index (`C0`, `C1`, …), vector, role (`center` for C0, `domain-N` for CN)
  - Terminal color cycled from palette `[11, 14, 10, 13, 12, 15, 6, 2]` mod 8
  - Footer: horizontal separator line

- **Role labels:**
  - `center` — color 0 (highest-degree node, first assigned)
  - `domain-N` — color N > 0 (N-th conflict group)

### `crates/k-shell/src/proc.rs`

- Dispatch wiring (4 lines):
  ```
  "graph color" | "color" | "gcolor" | "graph colour" | "colour" → dispatch_graph_color
  ```
- Help text: 2 new lines documenting `graph color` and its aliases.

---

## Test Harness: `host-tests/gos-graph-color-harness`

10 tests covering the full coloring API:

| # | Scenario | Assertion |
|---|----------|-----------|
| 1 | Empty graph | node_count=0, chromatic=0 |
| 2 | Single isolated node | chromatic=1, color=0 |
| 3 | Two isolated nodes (no edges) | chromatic=1, both color 0 |
| 4 | K₂: single edge A→B | chromatic=2, A≠B colors |
| 5 | Path A→B→C | chromatic≤2, A≠B, B≠C |
| 6 | K₃ triangle | chromatic=3, all three different |
| 7 | K₄ complete graph | chromatic=4, colors {0,1,2,3} used |
| 8 | Bipartite K_{2,2} | chromatic≤2, cross-set pairs differ |
| 9 | Validity: no adjacent pair shares a color | forall edges (u,v): color[u]≠color[v] |
| 10 | Descending degree order: star graph K_{1,3} | center B is index 0 and color 0; leaves share a color |

**Fix applied during this run:** `graph_color.rs` line 350 had `K_{1,3}` in a format string literal — curly braces interpreted as format args by Rust. Escaped to `K_{{1,3}}`.

**Result:** 10/10 pass, zero warnings.

---

## Shell Command Surface

```text
graph color        Welsh-Powell greedy coloring — conflict-free scheduling domains
color              alias
gcolor             alias
graph colour       alias (British spelling)
colour             alias
```

Example output (triangle K₃ + isolated D):

```text
 graph color
 ─────────────────────────────────────────────────────────────
  chromatic number: 3   nodes: 4

  color  vector           role
  C0     [24:1:1:0]       center
  C1     [24:1:2:0]       domain-1
  C2     [24:1:3:0]       domain-2
  C0     [24:1:4:0]       center
 ─────────────────────────────────────────────────────────────
```

---

## Invariants Preserved

- **No write ops**: `graph_color` is a pure read (no epoch bump, no mutation).
- **No alloc / no_std**: all buffers are fixed-size stack arrays.
- **TEST_LOCK + reset()**: harness uses the standard isolation pattern.
- **Sequential version**: V2.47 follows V2.46 (spanning) directly.
- **Doc archived**: this file at `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.47.md`.

---

## Next Steps

Suggested V2.48 candidates:
- `graph mst` — Minimum Spanning Tree using Prim's algorithm (weighted edges)
- `node checkpoint <vec>` — snapshot node state to the per-node diff ring
- `graph sim <N>` — simulate N random-walk steps, emit signal traffic trace
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph flow` — max-flow / Ford-Fulkerson on the weighted kernel graph
