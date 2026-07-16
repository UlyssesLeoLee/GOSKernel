> **[归位说明 / 2026-07-15]** 本文件为原始英文存档，未做删改。经审校已归位并中文化至 [doc/06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.23.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.23.md)，请以该中文版为准。

# Hardening Log — V3.23
**Date:** 2026-07-07  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.23): Zagreb eccentricity M1* + M2* + M3* indices + gos-graph-topo12-harness (10 tests)

---

## Summary

Added three new **Zagreb eccentricity indices** to `gos_runtime`: **M1\*** (first Zagreb eccentricity), **M2\*** (second Zagreb eccentricity), and **M3\*** (third Zagreb eccentricity). These capture structural information via vertex eccentricities — the maximum BFS distances from each node — computed on the undirected graph projection. They extend the eccentricity index family (V3.19: ECI + Diameter + Radius + avg-ecc) with squared, edge-product, and edge-difference eccentricity aggregations.

Host-test suite: **1203 tests total** (10 new in gos-graph-topo12-harness; all pass).

---

## New Algorithms

### `graph_topo_indices12()` → `(m1e: u64, m2e: u64, m3e: u64, edge_count: usize, node_count: usize)`

**M1\* — First Zagreb Eccentricity Index**  
- Formula: M1\*(G) = Σ_v ecc(v)²  
- Reference: Vukičević & Graovac (2010), *Acta Chimica Slovenica* 57:524–528  
- Computation: exact integer; node-level scan of squared eccentricities  
- Special values: M1\*(K_n) = n (all ecc=1); M1\*(isolated graph) = 0  
- Analogous to the first Zagreb index M1 = Σ_v deg(v)², but with eccentricity instead of degree  

**M2\* — Second Zagreb Eccentricity Index**  
- Formula: M2\*(G) = Σ_{uv∈E} ecc(u) × ecc(v)  
- Reference: Das, Narayankar & Mangala Lavanya (2013), *Bulletin of the Malaysian Mathematical Sciences Society*  
- Computation: exact integer; undirected edge scan  
- Special values: M2\*(K_n) = m = n(n−1)/2 (all ecc=1)  
- Analogous to the second Zagreb index M2 = Σ_{uv∈E} deg(u)×deg(v)  

**M3\* — Third Zagreb Eccentricity Index**  
- Formula: M3\*(G) = Σ_{uv∈E} |ecc(u) − ecc(v)|  
- Reference: Farooq & Ali (2021), *Proceedings of ICODAM*  
- Computation: exact integer; undirected edge scan  
- Self-centered invariant: M3\* = 0 iff graph is self-centered (all vertices have equal eccentricity)  
- Self-centered graphs: K_n (all ecc=1), K_{r,s} (all ecc=2), even cycles C_{2k} (all ecc=k)  
- Analogous to the third Zagreb/Albertson index I = Σ_{uv∈E} |deg(u)−deg(v)| (= 0 iff regular)  

---

## Algorithm Details

Single O(n·(n+m)) BFS loop over all vertices:
1. Build compact node index (`slot_to_ci[MAX_NODES]`)
2. Build undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded)
3. BFS from each source vertex (0..nc): compute ecc[src] = max reachable BFS distance (0 for isolated nodes)
4. M1\* accumulation (node scan): `m1e += ecc[ci] * ecc[ci]`
5. M2\*/M3\* accumulation (undirected edge scan via a < b): `m2e += ecc[a]*ecc[b]`; `m3e += |ecc[a]-ecc[b]|`

Stack arrays: `adj[MAX_NODES]` (u128×128 = 2KB), `ecc[MAX_NODES]` (u64×128 = 1KB), `dist[MAX_NODES]` (128B), `queue[MAX_NODES]` (128B) — zero heap allocation, ~3.5KB total.

No floating-point arithmetic; no `isqrt64`; all results are exact integers.

---

## Cross-Check Table

| Graph | M1\* | M2\* | M3\* | \|E\| | \|V\| |
|-------|------|------|------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| Single node | 0 | 0 | 0 | 0 | 1 |
| Edge A-B | 2 | 1 | 0 | 1 | 2 |
| Path P₃ | 9 | 4 | 2 | 2 | 3 |
| Triangle K₃ | 3 | 3 | 0 | 3 | 3 |
| Star K_{1,4} | 17 | 8 | 4 | 4 | 5 |
| Path P₄ | 26 | 16 | 2 | 3 | 4 |
| Complete K₄ | 4 | 6 | 0 | 6 | 4 |
| Two isolated | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 20 | 24 | 0 | 6 | 5 |

### Key Derivations

**Edge A-B:** ecc(A)=ecc(B)=1. M1\*=1+1=2. M2\*=1×1=1. M3\*=|1-1|=0.

**P₃ (A-B-C):** ecc(A)=ecc(C)=2 (end nodes); ecc(B)=1 (centre).  
M1\*=4+1+4=9. {A,B}: 2×1=2; {B,C}: 1×2=2 → M2\*=4. M3\*=|2-1|+|1-2|=2.

**K₃ (triangle):** all ecc=1 (diameter=1, self-centered).  
M1\*=3. M2\*=3×1=3. M3\*=0 (self-centered invariant).

**K_{1,4} (star):** center A ecc=1; each leaf B,C,D,E ecc=2 (max dist=2 to opposite leaf).  
M1\*=1²+4×2²=1+16=17. M2\*=4×(1×2)=8. M3\*=4×|1-2|=4.

**P₄ (A-B-C-D):** ecc(A)=ecc(D)=3; ecc(B)=ecc(C)=2.  
M1\*=9+4+4+9=26. {A,B}:3×2=6; {B,C}:2×2=4; {C,D}:2×3=6 → M2\*=16. M3\*=1+0+1=2.

**K₄:** all ecc=1 (diameter=1, self-centered).  
M1\*=4 (=n). M2\*=6 (=m). M3\*=0 (self-centered invariant).

**K_{2,3}:** left={A,B}, right={C,D,E}. Left-right d=1; same-side d=2.  
All 5 nodes have ecc=2 (max dist=2 to same-side peer). M1\*=5×4=20. M2\*=6×4=24. M3\*=0 (self-centered: K_{2,3} is vertex-transitive-by-eccentricity even though not degree-regular).

---

## Shell Interface

**Command routing** (k-shell/proc.rs):
```
"graph topo12" | "gtopo12" | "zagreb eccentricity" | "gzagreecc" |
"m1 eccentricity" | "gm1e" | "m2 eccentricity" | "gm2e" |
"m3 eccentricity" | "gm3e" | "gm1em2em3e"
```

**Display** (`dispatch_graph_topo_indices12`):
- Header: bright-yellow "graph topo12 (M1\* + M2\* + M3\* Zagreb eccentricity indices)"
- M1\*: bright-cyan, exact integer, formula [Σ_v ecc(v)²]
- M2\*: bright-green, exact integer, formula [Σ_{uv∈E} ecc(u)×ecc(v)]
- M3\*: bright-magenta, exact integer, formula [Σ |ecc(u)−ecc(v)|], with "(M3\*=0: self-centered)" annotation when zero
- Footer: "N node(s)  M edge(s)  Vukičević & Graovac 2010  Das et al. 2013  Farooq & Ali 2021"

---

## VectorAddress Namespace

| L4 | Harness |
|----|---------|
| 88 | graph-topo (V3.12) |
| 89 | graph-topo2 (V3.13) |
| 90 | graph-topo3 (V3.14) |
| 91 | graph-topo4 (V3.15) |
| 92 | graph-topo5 (V3.16) |
| 93 | graph-topo6 (V3.17) |
| 94 | graph-topo7 (V3.18) |
| 95 | graph-topo8 (V3.19) |
| 96 | graph-topo9 (V3.20) |
| 97 | graph-topo10 (V3.21) |
| 98 | graph-topo11 (V3.22) |
| **99** | **graph-topo12** (V3.23, new) |

---

## OS Analogy

- **M1\* (First Zagreb eccentricity)**: sum of squared reach-radii — amplifies nodes with large eccentricity (far-from-center subsystems). High M1\* = graph has many structurally peripheral nodes. M1\*(K_n)=n shows complete graphs minimise squared eccentricity.
- **M2\* (Second Zagreb eccentricity)**: total edge-endpoint eccentricity product — measures how much each IPC channel connects two structurally distant nodes. High M2\* = many channels bridge structurally remote parts of the kernel dependency graph.
- **M3\* (Third Zagreb eccentricity)**: total eccentricity imbalance per edge — measures how unequal the endpoint eccentricities are across all channels. M3\*=0 for self-centered graphs (e.g. complete graphs, bipartite complete, even cycles): every IPC channel connects nodes at equal structural distance from the graph periphery. High M3\* = many "bridge" channels with one near-center and one near-periphery endpoint (characteristic of path-like or hub-and-spoke topologies).

---

## Test Coverage

10 new tests in `gos-graph-topo12-harness`:
1. Empty graph → (0, 0, 0, 0, 0)
2. Single isolated node → (0, 0, 0, 0, 1)
3. Single edge A-B → (2, 1, 0, 1, 2)
4. Path P₃ → (9, 4, 2, 2, 3)
5. Triangle K₃ → (3, 3, 0, 3, 3)
6. Star K_{1,4} → (17, 8, 4, 4, 5)
7. Path P₄ → (26, 16, 2, 3, 4)
8. Complete K₄ → (4, 6, 0, 6, 4)
9. Two isolated nodes → (0, 0, 0, 0, 2)
10. K_{2,3} bipartite cross-check → (20, 24, 0, 6, 5)

All 10 tests pass. Total host-test suite: **1203 tests** (1193 prior + 10 new).
