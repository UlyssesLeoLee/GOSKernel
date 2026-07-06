# GOSKernel Hardening Log — V3.06
**Date:** 2026-07-06  
**Algorithm:** Edge Betweenness Centrality — Brandes (2001)  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.06): edge betweenness centrality -- Brandes edge-EBC + gos-graph-ebc-harness (10 tests)

---

## Summary

V3.06 adds **edge betweenness centrality (EBC)** to the GOSKernel graph theory
runtime.  This completes the betweenness family:

| Version | Algorithm | Output |
|---------|-----------|--------|
| V2.52   | Node betweenness centrality | Which nodes lie on the most shortest paths? |
| **V3.06** | **Edge betweenness centrality** | **Which links carry the most shortest-path traffic?** |

Edge betweenness answers a complementary question to node betweenness: given the
kernel's directed communication graph, **which directed links are the most critical
conduits**?  An edge with high betweenness is a single point-of-failure for routing
— analogous to a heavily trafficked network interface or bus in a real OS.

---

## Public API

### `gos_runtime::graph_betweenness_edge<const N: usize>() -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize)`

Returns `(from_vecs, to_vecs, scores, edge_count)`:
- `from_vecs[0..edge_count]` — source endpoint of each edge.
- `to_vecs[0..edge_count]` — target endpoint of each edge.
- `scores[0..edge_count]` — betweenness count: number of ordered (s, t) pairs whose
  **unique** shortest path traverses this edge (using integer arithmetic via SCALE=1_000_000).
- `edge_count` — number of live, non-self-loop directed edges.

**Output ordering:** Descending by score; ties broken by (from.as_u64(), to.as_u64()) ascending.

**Self-loops:** Excluded.  
**Direction:** Fully directed — edge (u→v) and edge (v→u) are treated independently.  
**Weights:** Uses edge `weight` field (Dijkstra); all weights default to 1.0 in standard registration.  
**Parallel paths:** When multiple equal-length shortest paths exist (e.g., diamond graphs),
betweenness is split proportionally by path count via Brandes' sigma-ratio formula.  Integer
division means the score is `floor(betweenness)`.

---

## Algorithm

### Brandes (2001) Dijkstra + Back-propagation, O(V * (V + E))

For each source node s:
1. **Dijkstra phase**: compute `dist[v]` and `sigma[v]` (number of shortest paths from s to v).
2. **Back-propagation phase** (reverse extraction order):
   - For each node w in reverse BFS/Dijkstra order (leaves first):
     - For each in-edge (v→w) that lies on a shortest path (dist[v] + w ≈ dist[w]):
       ```
       contribution = sigma[v] × (SCALE + delta[w]) / sigma[w]
       delta[v]      += contribution      ← feeds into v's node betweenness
       edge_bet[v→w] += contribution      ← edge betweenness accumulation
       ```

The key insight: **the contribution already computed for `delta[v]` is exactly the edge
betweenness contribution for edge (v→w)**.  The two accumulations share the same formula and
can be computed with a single multiply-divide.

**Why the SCALE factor?** Brandes' formula is `sigma[v]/sigma[w] * (1 + delta[w])`. Multiplying
through by SCALE = 1_000_000 converts to integer arithmetic: `sigma[v] * (SCALE + delta[w]) /
sigma[w]`.  Final score = `edge_bet[ei] / SCALE` (floor division).

---

## Difference from Node Betweenness (`graph_between`)

| Aspect | `graph_between` (V2.52) | `graph_betweenness_edge` (V3.06) |
|--------|------------------------|----------------------------------|
| Unit of output | Per-node | Per-directed-edge |
| Accumulation target | `bc_scaled[w]` (node) | `edge_bet[ei]` (edge slot) |
| Output arrays | `(vecs, scores, node_count)` | `(from_vecs, to_vecs, scores, edge_count)` |
| Sort key | Descending score | Descending score, then (from, to) asc |
| N generic | MAX_NODES=128 | MAX_EDGES=512 |

The inner back-propagation loop is structurally identical.  The addition of `edge_bet[ei] +=
contribution` piggybacks on the existing `delta[v]` computation at zero extra algorithmic cost.

---

## Key Invariants

| Graph | Edge | Score |
|-------|------|-------|
| Single edge A→B | A→B | 1 |
| Path A→B→C | A→B | 2 |
| Path A→B→C | B→C | 2 |
| Path A→B→C→D | A→B | 3 |
| Path A→B→C→D | B→C | **4** (highest) |
| Path A→B→C→D | C→D | 3 |
| Diamond A→{B,C}→D | all 4 edges | 1 (split evenly, floor) |
| Directed triangle A→B, B→C, A→C | all 3 edges | 1 |
| Out-star A→{B,C,D} | all 3 edges | 1 |

**Path graph law:** In a directed path of n nodes, edge at position k (0-indexed from start)
carries k*(n-1-k) path-pairs from sources ≤ k-1 plus internal pairs.  The middle edge(s)
always have the highest score.

---

## Shell Interface

| Command | Aliases |
|---------|---------|
| `graph ebc` | `gebc`, `edge between`, `edge betweenness`, `ebc` |

**Display:** bright-yellow header; 6-colour cycling per edge (10→11→13→9→14→15);
right-aligned score, from-vector → to-vector (Unicode → arrow U+2192); footer shows total
directed edge count and "Brandes 2001" attribution.

---

## VectorAddress Namespace

L4=82 for `gos-graph-ebc-harness`

---

## OS Analogy

Edge betweenness is the **link criticality metric** for the kernel dependency graph.  A
directed edge with high EBC is a **single-lane bottleneck** — most inter-subsystem shortest
paths flow through it.  Removing or rate-limiting this link most degrades reachability.

**Use case 1 — Bus saturation detection:** Edges with score above a threshold are candidates
for link duplication or load-balancing (analogous to `ethtool -S` + LACP bonding for busy NICs).

**Use case 2 — Fault injection targeting:** The highest-EBC edge is the most impactful target
for chaos engineering (analogous to `tc qdisc add netem delay` on critical network paths).

**Use case 3 — Security chokepoints:** High-EBC edges are natural IPC chokepoints for
reference monitor insertion (analogous to LSM hooks on frequent syscall paths).

The edge betweenness decomposition, combined with V3.05's biconnected components and V2.52's
node betweenness, provides a complete **fault-topology tripling**:
- BCC: which node *removal* fragments the graph?
- Node betweenness: which nodes carry the most path-pairs?
- Edge betweenness: which *links* carry the most path-pairs?

---

## Test Suite (gos-graph-ebc-harness, 10 tests, all green)

| # | Graph | Expected |
|---|-------|----------|
| 1 | Empty | edge_count=0 |
| 2 | Single isolated node | edge_count=0 |
| 3 | Single edge A→B | score(A→B)=1 |
| 4 | Path A→B→C | score(A→B)=score(B→C)=2 |
| 5 | Path A→B→C→D | score(A→B)=3, score(B→C)=4, score(C→D)=3 |
| 6 | Diamond A→B, A→C, B→D, C→D | all 4 edges score=1 |
| 7 | Directed triangle A→B, B→C, A→C | all 3 edges score=1 |
| 8 | Out-star A→{B,C,D} | all 3 edges score=1 |
| 9 | Disconnected A→B ∥ C→D | both edges score=1 |
| 10 | Path A→B→C→D: max-score edge first | scores[0]=4, from=B, to=C; output is non-increasing |

---

## Literature

- Brandes, U. (2001). *A faster algorithm for betweenness centrality.* Journal of Mathematical
  Sociology, 25(2), 163–177.
- Girvan, M. & Newman, M. E. J. (2002). *Community structure in social and biological networks.*
  PNAS, 99(12), 7821–7826. (First application of edge betweenness to community detection.)

---

## Cumulative Host-Test Count

**1033 tests** (1023 through V3.05 + 10 new EBC tests)
