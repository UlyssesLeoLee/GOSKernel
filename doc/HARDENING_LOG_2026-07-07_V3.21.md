# Hardening Log — V3.21
**Date:** 2026-07-07  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.21): Szeged Sz + Revised Szeged rSz + Mostar Mo edge-partition distance indices + gos-graph-topo10-harness (10 tests)

---

## Summary

Added three new **edge-partition distance topological indices** to `gos_runtime`: **Sz** (Szeged index), **rSz** (Revised Szeged index), and **Mo** (Mostar index). These characterise how each edge partitions the vertex set by BFS proximity — for each edge {u,v}, vertices are classified into those closer to u (n_u), closer to v (n_v), or equidistant (n_0). This extends the distance-based index family (V3.18 Wiener/Harary/HyperWiener, V3.19 eccentricity, V3.20 degree-distance hybrids) with edge-centric partition geometry.

Host-test suite: **1183 tests total** (10 new in gos-graph-topo10-harness; all pass).

---

## New Algorithms

### `graph_topo_indices10()` → `(sz: u64, rsz_ppm: u64, mo: u64, edge_count: usize, node_count: usize)`

**Sz — Szeged Index**  
- Formula: Sz(G) = Σ_{uv∈E} n_u(uv) · n_v(uv)  
- Reference: Gutman & Klavžar (1995), *Journal of Chemical Information and Computer Sciences*  
- Computation: exact integer; for each undirected edge, count BFS-closer vertices on each side  
- Tree invariant: n_0 = 0 for every tree edge → Sz = Wiener index (Sz ≥ W in general)  
- K_n: Sz = n(n-1)/2 × 1 × 1 = m (each edge: n_u=1, n_v=1 on complete graphs with n≤3; n_u=1, n_v=1, n_0=n-2 for K_n with n≥3)  

**rSz — Revised Szeged Index**  
- Formula: rSz(G) = Σ_{uv∈E} (n_u + n_0/2) · (n_v + n_0/2)  
- Reference: Pisanski & Randić (2010), *Acta Chimica Slovenica*  
- Computation: stored as (4·rSz_int) × 250_000 to avoid quarter-integer fractions  
  - 4·rSz_int = Σ_{uv∈E} (2n_u + n_0)(2n_v + n_0) — always an exact integer  
  - rSz_ppm = 4·rSz_int × 250_000 = rSz × 10^6  
- rSz ≥ Sz always; rSz = Sz iff n_0 = 0 for all edges (e.g. trees and bipartite graphs)  
- K₃: rSz = 27/4 = 6.75 (quarter-integer; first non-integer rSz example)  

**Mo — Mostar Index**  
- Formula: Mo(G) = Σ_{uv∈E} |n_u(uv) − n_v(uv)|  
- Reference: Doslić, Martinjak, Škrekovski, Tipurić Spužević & Zubac (2018), *Journal of Mathematical Chemistry*  
- Computation: exact integer; measures total edge-bisection imbalance across the graph  
- Vertex-transitive invariant: Mo = 0 iff n_u = n_v for all edges (e.g. K_n, C_{2k})  
- Named after the city Mostar in Bosnia, analogy to Wiener named after chemist Wiener

---

## Algorithm Details

Single O(n·(n+m)) BFS loop over all vertices:
1. Build undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded)
2. Build undirected edge list (a < b canonical ordering), stored as `ue_a[]`, `ue_b[]`
3. BFS from each vertex w (0..nc): after BFS, for each undirected edge (a,b):
   - if dist[a] = INF: skip (w unreachable from edge's component)  
   - if dist[a] < dist[b]: ue_nu[edge]++  
   - if dist[a] > dist[b]: ue_nv[edge]++  
   - if dist[a] = dist[b]: ue_n0[edge]++  
4. Accumulate: sz += nu×nv; rsz_4 += (2nu+n0)(2nv+n0); mo += |nu−nv|
5. rsz_ppm = rsz_4 × 250_000

Stack arrays: `adj[MAX_NODES]` (u128 ×128 = 2KB), `ue_a/ue_b/ue_nu/ue_nv/ue_n0[MAX_EDGES]` (u8 ×512 ×5 = 2.5KB), `dist[MAX_NODES]` (128B), `queue[MAX_NODES]` (128B) — zero heap allocation, ~5KB total.

---

## Cross-Check Table

| Graph | Sz | rSz_ppm | Mo | \|E\| | \|V\| |
|-------|-----|---------|-----|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| Single node | 0 | 0 | 0 | 0 | 1 |
| Edge A-B | 1 | 1_000_000 | 0 | 1 | 2 |
| Path P₃ | 4 | 4_000_000 | 2 | 2 | 3 |
| Triangle K₃ | 3 | 6_750_000 | 0 | 3 | 3 |
| Star K_{1,4} | 16 | 16_000_000 | 12 | 4 | 5 |
| Path P₄ | 10 | 10_000_000 | 4 | 3 | 4 |
| Complete K₄ | 6 | 24_000_000 | 0 | 6 | 4 |
| Two isolated | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 36 | 36_000_000 | 6 | 6 | 5 |

### Key Derivations

**P₃ (tree):**  
Edge {A,B}: n_u=1(A), n_v=2(B,C), n_0=0 → Sz+=2; rsz_4+=8; mo+=1  
Edge {B,C}: n_u=2(A,B), n_v=1(C), n_0=0 → Sz+=2; rsz_4+=8; mo+=1  
Sz=4 = Wiener(P₃) ✓ (tree invariant); rSz=16/4=4.0 = Sz ✓

**K₃ (triangle, n_0=1 per edge):**  
Each edge: n_u=1, n_v=1, n_0=1 → Sz=3; rsz_4=3×9=27; rsz_ppm=6_750_000; Mo=0  
rSz=27/4=6.75 is a quarter-integer (only case in the table)

**K_{1,4} (star, tree):**  
Each of 4 edges: n_u=4(center+3 leaves), n_v=1(the leaf), n_0=0 → Sz=16=Wiener ✓; Mo=4×3=12

**K₄ (n_0=2 per edge):**  
Each of 6 edges: n_u=1, n_v=1, n_0=2 → Sz=6; rsz_4=6×16=96; rsz_ppm=24_000_000; Mo=0

**K_{2,3} (bipartite, n_0=0 for all edges):**  
Each of 6 cross-edges: n_u=3, n_v=2, n_0=0 → Sz=36; rsz_ppm=36_000_000 (=Sz, n_0=0); Mo=6  
Mo=6>0 confirms K_{2,3} is NOT vertex-transitive (left deg=3 ≠ right deg=2)

---

## Shell Interface

**Command routing** (k-shell/proc.rs):
```
"graph topo10" | "gtopo10" | "szeged index" | "gszeged" |
"revised szeged" | "grszg" | "mostar index" | "gmostar" | "gszgrsmo"
```

**Display** (`dispatch_graph_topo_indices10`):
- Header: bright-yellow "graph topo10 (Sz + rSz + Mo edge-partition distance indices)"
- Sz: bright-cyan, exact integer, formula annotation [Σ nᵤ·n_v, uv∈E]
- rSz: bright-green, ppm decimal (3 decimal places), formula [(Σ (nᵤ+n₀/2)·(n_v+n₀/2))]
- Mo: bright-magenta, exact integer, formula [Σ |nᵤ−n_v|], with "(Mo=0: vertex-transitive)" annotation when zero
- Footer: "N node(s)  M edge(s)  Gutman & Klavžar 1995  Pisanski & Randić 2010  Doslić et al. 2018"

---

## VectorAddress Namespace

| L4 | Harness |
|----|---------|
| 88 | graph-topo |
| 89 | graph-topo2 |
| 90 | graph-topo3 |
| 91 | graph-topo4 |
| 92 | graph-topo5 |
| 93 | graph-topo6 |
| 94 | graph-topo7 |
| 95 | graph-topo8 |
| 96 | graph-topo9 |
| **97** | **graph-topo10** (V3.21, new) |

---

## OS Analogy

- **Sz (Szeged)**: total edge-bisection volume — each edge (IPC channel) partitions the kernel graph into two sides; Sz sums the product of both side sizes. High Sz = many large-partition edges (structural load-balancing bottlenecks). Sz = Wiener for tree-structured dependency graphs (no cycles).
- **rSz (Revised Szeged)**: corrected bisection volume counting equidistant vertices symmetrically (half to each side). rSz > Sz only when the graph has cycles creating equidistant vertices (n_0 > 0). For a ring-topology kernel, rSz captures the "shared boundary" overhead that plain Sz ignores.
- **Mo (Mostar)**: total bisection imbalance — the absolute difference between the two sides of each edge's partition. Mo = 0 for vertex-transitive graphs (perfectly symmetric IPC topology: every channel bisects equally). High Mo = some channels are highly asymmetric load splitters (like a hub-spoke star where one side has nearly all vertices).

---

## Test Coverage

10 new tests in `gos-graph-topo10-harness`:
1. Empty graph → all zeros
2. Single isolated node → all zeros
3. Single edge A-B → (1, 1_000_000, 0, 1, 2)
4. Path P₃ → (4, 4_000_000, 2, 2, 3)
5. Triangle K₃ → (3, 6_750_000, 0, 3, 3)
6. Star K_{1,4} → (16, 16_000_000, 12, 4, 5)
7. Path P₄ → (10, 10_000_000, 4, 3, 4)
8. Complete K₄ → (6, 24_000_000, 0, 6, 4)
9. Two isolated nodes → all zeros
10. K_{2,3} bipartite cross-check → (36, 36_000_000, 6, 6, 5)

All 10 tests pass. Total host-test suite: **1183 tests** (1173 prior + 10 new).
