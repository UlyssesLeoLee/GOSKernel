# GOSKernel Hardening Log — V3.05
**Date:** 2026-07-06  
**Algorithm:** Biconnected Components — Tarjan Iterative Edge-Stack BCC  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.05): biconnected components -- Tarjan edge-stack BCC + gos-graph-bcc-harness (10 tests)

---

## Summary

V3.05 adds **biconnected components (BCCs)** to the GOSKernel graph theory runtime.  This
completes the connectivity trilogy started at V2.85:

| Version | Algorithm | Output |
|---------|-----------|--------|
| V2.85   | Articulation points (cut vertices) | Which nodes disconnect the graph when removed? |
| V2.86   | Bridges (cut edges)               | Which edges disconnect the graph when removed? |
| V2.93   | 2-edge-connected components       | Maximal subgraphs resilient to any single edge removal |
| **V3.05** | **Biconnected components**      | **Maximal subgraphs resilient to any single vertex removal** |

A **biconnected component (BCC)** is a maximal 2-vertex-connected subgraph: for any pair of
vertices u, v within the BCC, there exist at least two vertex-disjoint paths between them.
Equivalently, removing any single vertex from within the BCC does not disconnect it.

---

## Public API

### `gos_runtime::graph_bcc<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize)`

Returns `(vecs, bcc_ids, node_count, bcc_count)`:
- `vecs[0..node_count]` — all live nodes, sorted ascending by `(bcc_id, vec.as_u64())`.
- `bcc_ids[0..node_count]` — BCC index for each node:
  - Regular BCC members: their BCC index (0-based).
  - **Articulation points** (nodes in 2+ BCCs): `255`.
  - Isolated nodes (no undirected edges): each assigned their own singleton BCC.
- `node_count` — total live nodes.
- `bcc_count` — total biconnected components (edge-BCCs + isolated-singletons).

**Undirected projection:** A→B and B→A together count as one undirected edge.  
**Self-loops:** Excluded from BCC analysis.  
**Articulation points** in `graph_bcc` output (bcc_id=255) correspond exactly to the vertices
returned by `graph_articulation` (V2.85) — verified by cross-check test 10.

---

## Algorithm

### Tarjan Iterative Edge-Stack BCC, O(V+E)

The algorithm is an iterative DFS variant of Tarjan's biconnected component algorithm.  It
maintains an **edge stack** and uses the `disc/low` link values to identify BCC boundaries.

**State arrays (indexed by compact index ci ∈ 0..nc):**
- `disc[ci]` — DFS discovery time (UNVISITED = u32::MAX initially)
- `low[ci]` — lowest disc reachable from ci's subtree via back-edges
- `par[ci]` — compact index of DFS parent (NIL = root)
- `bcc_primary[ci]` — first BCC id assigned to ci (255 = unassigned)
- `bcc_mult[ci]` — true iff ci appears in 2+ BCCs (is an articulation point)

**Edge stack:** `[(u8, u8); MAX_EDGES]` — stores `(ci_u, ci_v)` for each undirected edge
discovered during DFS.

**Tree edges:** pushed to the edge stack when the child node is first discovered.

**Back edges:** pushed to the edge stack only when `disc[nbr] < disc[cur]` (nbr is an ancestor),
ensuring each undirected back-edge is pushed exactly once from the descendant's side.

**BCC condition:** When completing node `cur` with parent `p`:
```
if low[cur] >= disc[p]:
    bid = bcc_count; bcc_count += 1
    pop edge_stack until (p, cur) is popped:
        assign bid to all vertices in popped edges
```

The condition `low[cur] >= disc[par]` means no back-edge from cur's subtree can reach strictly
above par — par is a BCC boundary (articulation point or DFS root).

**Isolated nodes** (bcc_primary still 255 after DFS): each assigned a fresh singleton BCC id.

**Parent tracking:** Slot-based (skip all edges to parent slot), matching `graph_articulation`
style.  This correctly handles anti-parallel directed edge pairs (A→B + B→A treated as one
undirected edge) without double-pushing.

---

## Key Invariants

| Scenario | bcc_count | APs (bcc_id=255) |
|----------|-----------|-----------------|
| Empty graph | 0 | — |
| Single isolated node | 1 | none |
| Path Pₙ (n nodes) | n−1 | n−2 internal nodes |
| Triangle K₃ | 1 | none (biconnected) |
| K₄ | 1 | none (3-connected) |
| Hourglass (2 triangles sharing vertex C) | 2 | C |
| Star K₁,₄ (center B, 4 leaves) | 4 | B |

**Articulation point identity:** nodes with `bcc_id=255` in `graph_bcc` are exactly the
articulation points returned by `graph_articulation` (verified in test 10).

**BCC count vs bridge count:** For a tree with n nodes (n−1 bridges), bcc_count = n−1
(one edge-BCC per bridge). For a biconnected graph, bcc_count = 1.

---

## Shell Interface

| Command | Aliases |
|---------|---------|
| `graph bcc` | `gbcc`, `biconnected`, `gbiconn`, `bcc` |

**Display:** bright-yellow header; BCC id and "BCC-member" role per node (6 colours cycling);
articulation points shown as `AP  cut-vertex` in bright-red; footer shows node count, BCC count,
AP count, and "Tarjan 1972" attribution.

---

## VectorAddress Namespace

L4=81 for `gos-graph-bcc-harness`

---

## OS Analogy

Biconnected components are the **fault-isolation "blocks"** of the kernel dependency graph.
Within a single BCC, any subsystem can crash without disconnecting the block's internal
connectivity — like a RAID array where any single disk failure leaves the set intact.

Articulation points (bcc_id=255) are the **single points-of-failure** that, when removed,
partition the dependency graph into disconnected pieces.  These are the highest-priority
redundancy targets — analogous to `systemctl mask` candidates identified by
`systemd-analyze critical-chain`.

The BCC decomposition builds the **block-cut tree**: a tree whose nodes are BCCs (rectangles)
and cut vertices (circles), connected by containment.  This is the canonical representation of
how a graph's fault topology is organised.

---

## Test Suite (gos-graph-bcc-harness, 10 tests, all green)

| # | Graph | Expected bcc_count | Expected APs |
|---|-------|--------------------|--------------|
| 1 | Empty | 0 | — |
| 2 | Single isolated node | 1 | none |
| 3 | Two isolated nodes | 2 | none |
| 4 | Single edge A-B | 1 | none |
| 5 | Path A-B-C (2 bridges) | 2 | B |
| 6 | Triangle K₃ | 1 | none |
| 7 | K₄ | 1 | none |
| 8 | Hourglass (2 triangles sharing C) | 2 | C |
| 9 | Star K₁,₄ (center B) | 4 | B |
| 10 | Cross-check: BCC APs = graph_articulation APs (path A-B-C-D) | 3 | B, C |

---

## Literature

- Tarjan, R. E. (1972). *Depth-first search and linear graph algorithms.* SIAM Journal on
  Computing, 1(2), 146–160.
- Hopcroft, J. & Tarjan, R. E. (1973). *Algorithm 447: Efficient algorithms for graph
  manipulation.* Communications of the ACM, 16(6), 372–378.

---

## Cumulative Host-Test Count

**1023 tests** (1013 through V3.04 + 10 new BCC tests)
