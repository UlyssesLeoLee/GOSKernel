# Hardening Log — V2.49: `graph shortest` — Dijkstra Single-Source Shortest Paths

**Date:** 2026-07-02  
**Branch:** `feat/vk-auto-live-surface`  
**Commit:** (see below)  
**Author:** Claude (automated hardening run)

---

## Summary

V2.49 adds **`graph shortest <vec>`** — Dijkstra's single-source shortest-path tree (SPT) over the **directed** live GOS kernel graph from a specified source node. Unlike the spanning and MST algorithms (which treat edges as undirected), Dijkstra follows edge directions, making it the first directed weighted path analysis primitive in the GOS toolset.

Shell aliases: `graph shortest <vec>` / `shortest <vec>` / `graph dijkstra <vec>` / `dijkstra <vec>`

OS analogy: `ip route get <dst>` — the minimum-latency directed path from one kernel sub-system to all reachable peers, using edge weights as routing metrics.

This completes the **weighted graph analysis triad**:
- **V2.48 MST** — undirected minimum-cost spanning forest (structural backbone).
- **V2.49 SPT** — directed minimum-cost path tree from one source (routing table).
- Together with V2.47 (coloring), V2.46 (spanning), and V2.45 (community), this gives operators a complete toolkit for structural, cost-aware, and directional kernel graph analysis.

---

## Motivation

The V2.48 MST infrastructure (`edge_weight` in `GraphTopologySnapshot`) opened the door for directed weighted path algorithms. Dijkstra is the canonical single-source shortest-path algorithm and answers critical OS observability questions:

- **Which kernel sub-systems can signal X reach?** (directed reachability with cost)
- **What is the minimum latency path from scheduler → memory manager?** (path cost)
- **Which subsystems are unreachable from the boot node?** (partitioned graph detection)
- **Are there bottleneck nodes where all shortest paths converge?** (SPT structure)

Unlike the BFS spanning tree (V2.46) which ignores weights and uses undirected edges, Dijkstra's SPT gives operators a precise picture of signal routing cost in the real directed kernel graph.

---

## Algorithm: Dijkstra Single-Source Shortest Paths (Directed)

```text
Initialize:
  visited[v]  = false  for all v
  dist[v]     = ∞      for all v
  parent[v]   = ∅      for all v

Locate source slot by VectorAddress:
  If source not found: return all nodes with dist=u32::MAX, no SPT

dist[source] = 0.0
parent[source] = source

Repeat n times:
  u = argmin{ dist[v] : v not visited and dist[v] < ∞ }
  If none found (all remaining nodes unreachable): break

  visited[u] = true

  For each directed out-edge (u → v) with weight w:
    If v not visited and dist[u] + w < dist[v]:
      dist[v]   = dist[u] + w
      parent[v] = u

Build output:
  Source first (vecs[0]=source, dists[0]=0, parents[0]=source)
  Then all other live nodes in slot order:
    dist[v] = (dist_f[v] * 1000) as u32   if reachable
    dist[v] = u32::MAX                      if unreachable
    parents[v] = slot_vec[parent[v]]        if reachable
    parents[v] = ZERO_VEC                   if unreachable
```

**Key design choices:**

1. **Directed edges only**: follows edge directions as registered (`edge_from → edge_to`). Callers wanting undirected behaviour should use the MST or spanning functions.

2. **Greedy extraction (no priority queue)**: same O(V·E) pattern as Prim's MST — O(V) outer iterations × O(E) relaxation scan. For n≤128, E≤512, this is ≤65,536 operations, well within the no_std budget.

3. **u32::MAX sentinel for unreachable**: unambiguous sentinel (no valid distance ever reaches 4,294,967). Shell display shows `∞` for unreachable nodes.

4. **ZERO_VEC parent for unreachable**: distinguishes "no parent" from "parent is some node" without an extra flag array.

5. **Source always first in output**: simplifies shell rendering and caller logic — `vecs[0]` is always the source when one is found.

6. **Unknown source**: if the given VectorAddress matches no live node, all nodes are returned with `dist=u32::MAX` and no SPT is built. Shell shows `∞` for all.

**Complexity:** O(V·E) — O(V) outer × O(E) edge scan inner loop.  
**Space:** O(MAX_NODES) — `visited`, `dist`, `parent` arrays; no_std/no_alloc safe.

---

## Implementation

### `crates/gos-runtime/src/lib.rs`

**New inner function:**
- **`RuntimeState::graph_shortest_inner<const N>(snap, source: VectorAddress)`** — Dijkstra SPT:
  - Finds source slot by VectorAddress linear scan.
  - Outer loop: V iterations, each O(V) to find min-dist unvisited node + O(E) relaxation.
  - Only relaxes directed out-edges (`snap.edge_from[ei] == u_id`).
  - Packs output: source first, remaining slots in snap order.

**New public function:**
```rust
pub fn graph_shortest<const N: usize>(
    source: VectorAddress,
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize)
```
Locks `RUNTIME`, calls `topology_snapshot()`, delegates to `graph_shortest_inner`.

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_shortest(sink, source: VectorAddress)`** — display:
  - Header: cyan `graph shortest [src_vec]`
  - Column header: `status  dist  vector  parent`
  - Per node: status (magenta `source` / green `reach` / dark `∞`), yellow distance `D.mmm`, white vector, parent (or `(source)` / `(unreachable)`)
  - Footer: `N node(s)  Dijkstra SPT from [src]  reachable: R`

### `crates/k-shell/src/proc.rs`

- Dispatch (with vector parsing):
  ```text
  "graph shortest <v>" | "shortest <v>" | "graph dijkstra <v>" | "dijkstra <v>"
  → VectorAddress::parse(v) → dispatch_graph_shortest(sink, src)
  ```
- Help text: 2 new lines documenting `graph shortest <v>` and alias.
- Invalid vector shows red error: `graph shortest: invalid vector (e.g. 1.0.0.1)`.

---

## Test Harness: `host-tests/gos-graph-shortest-harness`

10 tests covering the full shortest-path API:

| # | Scenario | Assertion |
|---|----------|-----------|
| 1 | Empty graph | node_count=0 |
| 2 | Single node, source=itself | dist=0, parent=self, source first |
| 3 | Unknown source (no match) | all dists=u32::MAX |
| 4 | K₂ A→B weight=1.0, source=A | B dist=1000, B parent=A |
| 5 | K₂ A→B weight=2.5, source=A | B dist=2500 |
| 6 | Path A→B(1)→C(2), source=A | C dist=3000, C parent=B |
| 7 | Directed A→B only, source=B | A dist=u32::MAX (no B→A edge) |
| 8 | Diamond A→B(1)→D(1), A→C(2)→D(2) | D dist=2000 via B, D parent=B |
| 9 | A→B connected; C isolated, source=A | C dist=u32::MAX |
| 10 | Source parent invariant | parents\[0\]==vecs\[0\]==source |

**Result:** 10/10 pass, zero warnings.

---

## Shell Command Surface

```text
graph shortest <vec>   Dijkstra SPT from node <vec> (directed, weighted)
shortest <vec>         alias
graph dijkstra <vec>   alias
dijkstra <vec>         alias
```

Example output (path A→B(1)→C(2)):

```text
 graph shortest [26:1:1:0]
 ─────────────────────────────────────────────────────────────
  status    dist      vector           parent
  source    0.000     [26:1:1:0]       (source)
  reach     1.000     [26:1:2:0]       [26:1:1:0]
  reach     3.000     [26:1:3:0]       [26:1:2:0]
 ─────────────────────────────────────────────────────────────
 3 node(s)  Dijkstra SPT from [26:1:1:0]  reachable: 2
```

Example output with unreachable node:

```text
 graph shortest [26:1:2:0]
 ─────────────────────────────────────────────────────────────
  status    dist      vector           parent
  source    0.000     [26:1:2:0]       (source)
  ∞         ∞         [26:1:1:0]       (unreachable)
 ─────────────────────────────────────────────────────────────
 2 node(s)  Dijkstra SPT from [26:1:2:0]  reachable: 0
```

---

## Weighted Algorithm Suite Completion (V2.47–V2.49)

| Version | Algorithm | Direction | Weight | Output |
|---------|-----------|-----------|--------|--------|
| V2.47 | Welsh-Powell coloring | Undirected | No | Color index / chromatic number |
| V2.48 | Prim's MST | Undirected | Yes | Spanning forest / total cost |
| V2.49 | Dijkstra SPT | **Directed** | Yes | Path tree / distances |

---

## Invariants Preserved

- **No write ops**: `graph_shortest` is a pure read (no epoch bump, no mutation).
- **No alloc / no_std**: all buffers are fixed-size stack arrays.
- **TEST_LOCK + reset()**: harness uses the standard isolation pattern.
- **Sequential version**: V2.49 follows V2.48 (MST) directly.
- **Doc archived**: this file at `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.49.md`.

---

## Next Steps

Suggested V2.50 candidates:
- `graph flow <from> <to>` — max-flow (Edmonds-Karp BFS-based)
- `node checkpoint <vec>` — snapshot node state to the per-node diff ring
- `graph sim <N>` — simulate N random-walk steps, emit signal traffic trace
- `graph between` — betweenness centrality using all-pairs shortest paths
