# Hardening Log V2.93 -- 2-Edge-Connected Components (2ECC)

**Date:** 2026-07-05
**Branch:** feat/vk-auto-live-surface
**Host-test total:** 903 (893 prior + 10 new)

---

## Feature: `graph 2ecc` / `g2ecc` / `2ecc` / `edge connected components`

### Motivation

V2.85 added articulation points (nodes whose removal disconnects the graph) and V2.86 added
bridges (edges whose removal disconnects the graph). V2.93 completes this connectivity
decomposition trilogy by partitioning the graph into **2-edge-connected components (2ECCs)**:
maximal subgraphs where no single edge failure can partition the group.

> **"Which clusters of kernel subsystems are internally fault-tolerant against any single
> IPC link failure?"**

| Question | OS analogy |
|---|---|
| Which subsystem groups survive any single link cut? | Bonded NIC pairs, RAID-1 storage paths |
| Which nodes are only reachable via a critical single link? | Single-point-of-failure IPC channels |
| How many independent fault-tolerance zones exist? | VLAN segment count, `ip link` bond groups |
| Which edges are the weakest points in the communication fabric? | Bridges = `graph bridges` output |

2ECCs complement the prior connectivity work:

| Version | What it identifies |
|---|---|
| V2.85 — graph articulation | Cut *vertices* — remove one node to disconnect |
| V2.86 — graph bridges | Cut *edges* — remove one edge to disconnect |
| V2.93 — graph 2ecc | *Components* grouped by 2-edge-connectivity |

---

### Algorithm: Two-Phase O(V+E)

**Phase 1 — Tarjan bridge-finding (identical to V2.86):**
- Iterative DFS on the undirected projection of the graph
- Maintains `disc[]` (discovery time), `low[]` (lowest reachable disc), and `par_ei[]` (parent edge index)
- Bridge condition: `low[child] > disc[parent]` (strictly greater, not ≥)
- Marks `is_bridge[ei] = true` for each bridge edge index

**Phase 2 — BFS on non-bridge undirected edges:**
- For each unvisited node, BFS across non-bridge undirected edges
- Each BFS flood-fill discovers one 2ECC and assigns a 0-indexed component ID
- Isolated nodes (no edges) each form their own singleton 2ECC

**Key invariant for connected graphs:**
```
comp_count = bridge_count + 1
```
Verified in test 10 (path of 4 nodes: 3 bridges → 4 components).

**Output contract:**
- Every node belongs to exactly one 2ECC (unlike articulation points which can be in multiple BCCs)
- Nodes sorted ascending by `(comp_id, VectorAddress.as_u64())` for deterministic output
- `comp_ids[i]` is a `u8` (0–254); capped at 254 for graphs with > 254 components

---

### API

```rust
pub fn graph_2ecc<N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize)
// (vecs, comp_ids, node_count, comp_count)
```

- `vecs[0..node_count]` — live nodes sorted by (comp_id, VectorAddress)
- `comp_ids[0..node_count]` — 0-indexed 2ECC for each corresponding node
- `node_count` — total live nodes
- `comp_count` — number of distinct 2-edge-connected components

**Shell commands:**
- `graph 2ecc` — primary
- `g2ecc` / `2ecc` / `edge connected components` — aliases

**VectorAddress L4=69** for `gos-graph-2ecc-harness`.

---

### Display Format

```
 graph 2-edge-connected components
 ───────────────────────────────────────────────────────────
  comp 0  [3 nodes]
    1.0.0.1  1.0.0.2  1.0.0.3
  comp 1  [2 nodes]
    2.0.0.1  2.0.0.2
 ───────────────────────────────────────────────────────────
 2 components across 5 nodes
```

Nodes are grouped and indented under their component header.

---

### Test Suite: gos-graph-2ecc-harness (10 tests)

| # | Scenario | Expected comp_count |
|---|---|---|
| 1 | Empty graph | 0 |
| 2 | Single isolated node | 1 (singleton 2ECC) |
| 3 | Single directed edge A→B | 2 (bridge → {A}, {B}) |
| 4 | Triangle / directed 3-cycle | 1 (no bridges) |
| 5 | Path of 4 nodes A→B→C→D | 4 (every edge is a bridge) |
| 6 | Two triangles joined by one bridge | 2 |
| 7 | Two triangles sharing one edge | 1 (shared edge not a bridge) |
| 8 | Star (1 center + 4 leaves) | 5 (all spokes are bridges) |
| 9 | Two disconnected triangles | 2 |
| 10 | Cross-check: `comp_count == bridge_count + 1` for path | verified |

---

### Implementation Details

**Self-loops ignored:** Self-loops can never be bridges (they don't connect two distinct
nodes) and are explicitly skipped in both phases (`nbr_slot == cur_slot` guard).

**Parent edge tracked by index not slot:** The `par_ei[cur_slot] == ei` guard in Phase 1
skips the exact tree-edge we arrived on, not just edges to the parent node. This correctly
handles anti-parallel edges (A→B and B→A both present): only the DFS-tree edge is skipped,
the reverse parallel edge correctly updates `low[]`.

**Phase 2 uses `is_bridge[]` array (512 bools):** Allocated on the stack, indexed by
edge slot. Zero-cost compared to re-running bridge detection: no additional RUNTIME lock
needed since both phases run within the same `RUNTIME.lock()` hold.

---

### Literature & Theory

| Reference | Contribution |
|---|---|
| Tarjan 1972 | DFS disc/low-link bridge detection |
| Whitney 1932 | k-edge-connectivity definition |
| Even & Tarjan 1975 | Edge-connectivity and max-flow relationship |
| Nagamochi & Ibaraki 1992 | Linear-time minimum cut |
| König's theorem | Bridges are minimum edge cuts of size 1 |

**Relationship to vertex connectivity:**
- If the graph has any bridge → edge connectivity λ(G) = 1
- If every node is in a singleton 2ECC → λ(G) = 1 (all edges are bridges)
- If comp_count = 1 → no bridge exists → λ(G) ≥ 2

---

### Invariants Added

- `graph_2ecc`: every node belongs to exactly one 2ECC (unlike BCC where articulation points span multiple blocks)
- `graph_2ecc` + `graph_bridges`: for connected graphs, `comp_count = bridge_count + 1`
- Self-loops are ignored in both bridge-finding and component BFS
- Output sorted ascending by `(comp_id, vecs.as_u64())` for determinism
- `comp_ids` are 0-indexed starting from 0 (assigned in BFS-seed order)
