# Hardening Log — V2.48: `graph mst` — Prim's Minimum Spanning Forest

**Date:** 2026-07-02  
**Branch:** `feat/vk-auto-live-surface`  
**Commit:** (see below)  
**Author:** Claude (automated hardening run)

---

## Summary

V2.48 adds **`graph mst`** — Prim's minimum spanning forest over the undirected projection of the live GOS kernel graph. Every directed edge is treated as undirected with its registered `weight` (default 1.0). Disconnected components each receive their own MST root, producing a spanning **forest** rather than a single spanning tree. The total MST weight (sum of all selected edges) is reported as a fixed-point integer (× 1000) for no_std/no_alloc compatibility.

Shell aliases: `graph mst` / `mst` / `gmst` / `graph tree mst` / `min spanning`

OS analogy: `ip route show metric` — the minimum-cost routing backbone that keeps all kernel sub-systems reachable, analogous to a routing table built for minimum total latency/bandwidth cost.

**Infrastructure change (V2.48):** Extended `GraphTopologySnapshot` to carry `edge_weight: [f32; MAX_EDGES]`, populated from `EdgeRecord.spec.weight` in `topology_snapshot()`. Future algorithms that need weights (flow, shortest-path, etc.) can use this field without additional runtime locking.

---

## Motivation

After the structural-analysis suite (V2.41–V2.47), the next natural primitive is **weighted graph algorithms**. The kernel graph already stores a `weight: f32` on every edge (`EdgeSpec.weight`, defaults to 1.0). Until V2.48, no API exposed these weights to analysis functions.

MST in a kernel graph OS answers:
- What is the minimum-cost set of signal routes that keeps all subsystems connected?
- Which edges are load-bearing (in the MST) vs. redundant (not in the MST)?
- What is the total minimum bandwidth cost of the kernel's inter-subsystem communication?
- Which subsystems are in separate network partitions (multiple MST roots)?

MST is the foundation for further weighted primitives: shortest paths (Dijkstra), max-flow (Ford-Fulkerson), and minimum-cost flow.

---

## Algorithm: Prim's Minimum Spanning Forest (Undirected Projection)

```text
Initialize:
  in_mst[v]      = false  for all v
  key[v]         = ∞      for all v
  parent_slot[v] = ∅      for all v
  remaining      = node_count

While remaining > 0:
  u = argmin{ key[v] : v not in MST }

  If key[u] == ∞ (new component — u has no edge to existing MST):
    parent_slot[u] = u      // u is a new component root
    key[u] = 0.0

  in_mst[u] = true
  Emit u to output; record out_key[u] = key[u]
  remaining -= 1

  For each live edge e incident to u (undirected):
    v = neighbor of u through e
    w = edge_weight[e]
    If v not in MST and w < key[v]:
      key[v]         = w
      parent_slot[v] = u

Build output:
  out_vecs[i]    = slot_vec[order[i]]
  out_parents[i] = slot_vec[parent_slot[order[i]]]
  out_weights[i] = (out_key[i] × 1000) as u32
  total_mst_w    = sum(out_key[i] for non-root i) × 1000 as u32
```

**Key design choices:**

1. **Undirected treatment**: both in-edges and out-edges are used as undirected neighbor links. Consistent with `graph spanning`, `graph community`, and `graph bipartite`.

2. **Prim's (not Kruskal's)**: Prim's gives a natural visit order (nodes emitted as they join the MST, grouped by component) and requires no edge sorting — important for `no_std` fixed-size arrays.

3. **Weight default 1.0**: edges registered without an explicit weight (`EdgeSpec.weight`) are stored as 1.0, making MST on an unweighted graph equivalent to BFS spanning (but not necessarily the same structure due to degree-order differences in tie-breaking).

4. **Fixed-point output (× 1000)**: avoids f32 format printing in the `no_std` kernel display layer. Integer arithmetic suffices for weight display.

5. **Tie-breaking**: among nodes with the same minimum key, the one with the smallest slot index wins. This ensures deterministic output for equal-weight graphs.

6. **Root detection**: `parents[i] == vecs[i]` and `weights[i] == 0` identifies component roots. All other nodes are children with a positive weight.

**Complexity:** O(V·E) — O(V) outer iterations × O(E) neighbor scan per iteration. For n≤128 and E≤512, this is at most 65,536 operations per call.

**Space:** O(MAX_NODES + MAX_EDGES) — fixed-size stack arrays, no_std/no_alloc compatible.

---

## Implementation

### `crates/gos-runtime/src/lib.rs`

**Snapshot extension:**
- `GraphTopologySnapshot.edge_weight: [f32; MAX_EDGES]` — new field, initialized to `1.0f32` (default weight).
- `topology_snapshot()` now copies `e.spec.weight` into `snap.edge_weight[i]` for each live edge.

**New inner function:**
- **`RuntimeState::graph_mst_inner<const N>()`** — Prim's spanning forest:
  - `key[MAX_NODES]`, `in_mst[MAX_NODES]`, `parent_slot[MAX_NODES]` (all fixed-size).
  - `out_slots[MAX_NODES]`, `out_key[MAX_NODES]` — emit buffers.
  - Three-pass structure: (1) find min-key unvisited node, (2) mark in MST and emit, (3) relax neighbors.
  - Disconnected component detection: if selected node has no initialized key (key==INF), it starts a new root with key=0.

**New public function:**
```rust
pub fn graph_mst<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32)
```
Locks `RUNTIME`, calls `topology_snapshot()`, delegates to `graph_mst_inner`.

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_mst(sink)`** — display function:
  - Header: cyan `graph mst`
  - Column header: `role  weight  vector  parent`
  - Per node: role (magenta `root` / cyan `child`), yellow `W.mmm` weight, white vector, parent (gray `(root)` for roots)
  - Footer: `N node(s)  Prim MST  total weight: W.mmm`

### `crates/k-shell/src/proc.rs`

- Dispatch (2 lines):
  ```text
  "graph mst" | "mst" | "gmst" | "graph tree mst" | "min spanning" → dispatch_graph_mst
  ```
- Help text: 2 new lines documenting `graph mst` and its aliases.

---

## Test Harness: `host-tests/gos-graph-mst-harness`

10 tests covering the full MST API:

| # | Scenario | Assertion |
|---|----------|-----------|
| 1 | Empty graph | node_count=0, total_mst_w=0 |
| 2 | Single node | node_count=1, weight=0, parent=self |
| 3 | Two isolated nodes (no edge) | total_mst_w=0, both roots |
| 4 | K₂ edge weight=1.0 | total_mst_w=1000, one root + one child |
| 5 | K₂ edge weight=2.5 | total_mst_w=2500 |
| 6 | Path A─B─C all weight=1.0 | total_mst_w=2000 |
| 7 | K₃ triangle (weights 1, 2, 3) | MST selects 1+2=3 (heaviest excluded); total=3000 |
| 8 | Two components (A─B, C isolated) | total_mst_w=1000; C is second root |
| 9 | Root invariant | parents\[i\]==vecs\[i\] whenever weights\[i\]==0 |
| 10 | Connectivity | every non-root has a parent present in output vecs |

**Result:** 10/10 pass, zero warnings.

---

## Shell Command Surface

```text
graph mst          Prim's minimum spanning forest — minimum-cost routing backbone
mst                alias
gmst               alias
graph tree mst     alias
min spanning       alias
```

Example output (path A─2─B─3─C):

```text
 graph mst
 ─────────────────────────────────────────────────────────────
  role    weight    vector           parent
  root    0.000     [25:1:1:0]       (root)
  child   2.000     [25:1:2:0]       [25:1:1:0]
  child   3.000     [25:1:3:0]       [25:1:2:0]
 ─────────────────────────────────────────────────────────────
 3 node(s)  Prim MST  total weight: 5.000
```

---

## Infrastructure: `GraphTopologySnapshot` Extension

`edge_weight: [f32; MAX_EDGES]` is now part of the topology snapshot captured under the RUNTIME lock. This is a **load-bearing infrastructure change** enabling all future weighted graph algorithms to access edge weights without additional runtime queries:

| Algorithm | Uses `edge_weight` |
|-----------|-------------------|
| V2.48 `graph_mst_inner` | Yes |
| Future `graph_shortest_path` (Dijkstra) | Yes |
| Future `graph_flow` (Ford-Fulkerson/Edmonds-Karp) | Yes |

---

## Invariants Preserved

- **No write ops**: `graph_mst` is a pure read (no epoch bump, no mutation).
- **No alloc / no_std**: all buffers are fixed-size stack arrays.
- **TEST_LOCK + reset()**: harness uses the standard isolation pattern.
- **Sequential version**: V2.48 follows V2.47 (graph coloring) directly.
- **Doc archived**: this file at `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.48.md`.

---

## Next Steps

Suggested V2.49 candidates:
- `graph shortest <vec>` — Dijkstra shortest-path tree from a given node
- `graph flow <from> <to>` — max-flow between two nodes (Ford-Fulkerson)
- `node checkpoint <vec>` — snapshot node state to the per-node diff ring
- `graph sim <N>` — simulate N random-walk steps, emit signal traffic trace
