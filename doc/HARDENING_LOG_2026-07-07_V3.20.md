> **[归位说明 / 2026-07-15]** 本文件为原始英文存档，未做删改。经审校已归位并中文化至 [doc/06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.20.md](06_运维维护/hardening/HARDENING_LOG_2026-07-07_V3.20.md)，请以该中文版为准。

# Hardening Log — V3.20
**Date:** 2026-07-07  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.20): Schultz MTI W_S + Gutman W_G + Connective Eccentric CxiE degree-distance hybrid topological indices + gos-graph-topo9-harness (10 tests)

---

## Summary

Added three new **degree-distance hybrid topological indices** to `gos_runtime`: **W_S** (Schultz Molecular Topological Index), **W_G** (Gutman Index), and **CξE** (Connective Eccentric Index). These bridge the pure-distance indices of V3.18 (Wiener/Harary/Hyper-Wiener) and the eccentricity indices of V3.19 (ECI/Diameter/Radius) by weighting pairwise distances with degree products or ratios.

Host-test suite: **1173 tests total** (10 new in gos-graph-topo9-harness; all pass).

---

## New Algorithms

### `graph_topo_indices9()` → `(ws: u64, wg: u64, cxe_ppm: u64, edge_count: usize, node_count: usize)`

**W_S — Schultz Molecular Topological Index (MTI)**  
- Formula: W_S(G) = Σ_{u<v} (deg(u)+deg(v)) × d(u,v)  
- Reference: Schultz (1989), *Journal of Chemical Information and Computer Sciences*  
- Computation: accumulated during BFS over pairs (src < v); exact integer (no overflow for realistic graphs)  
- Invariant: W_S = 2Δ × W(G) for Δ-regular graphs (sum-degree = 2Δ for all pairs)  
- K_n: W_S = 2(n-1) × n(n-1)/2 = n(n-1)²  
- Disconnected pairs (d=∞): contribute 0

**W_G — Gutman Index**  
- Formula: W_G(G) = Σ_{u<v} deg(u) × deg(v) × d(u,v)  
- Reference: Gutman (1994), *Journal of Mathematical Chemistry*  
- Computation: accumulated during same BFS pass; exact integer always  
- Invariant: W_G = Δ² × W(G) for Δ-regular graphs  
- K_n: W_G = (n-1)² × n(n-1)/2 = n(n-1)³/2  
- Disconnected pairs (d=∞): contribute 0

**CξE — Connective Eccentric Index**  
- Formula: CξE(G) = Σ_v deg(v)/ecc(v) × 10⁶  
- Reference: Gupta, Singh & Madan (2000), *Journal of Chemical Information and Computer Sciences*  
- Computation: BFS from each node computes ecc[v]; then CξE = Σ_v floor(deg[v] × 10⁶ / ecc[v])  
- Isolated nodes (ecc=0, deg=0): contribute 0 — no division by zero  
- Regular self-centered graph (D=R): CξE = n × Δ/D × 10⁶  
- K_n: CξE = n × (n-1)/1 × 10⁶ = n(n-1) × 10⁶  

---

## Algorithm Details

All three indices share a single O(n·(n+m)) BFS loop:
1. Build undirected adjacency bitmasks + degree array (directed→undirected dedup, self-loops excluded)
2. BFS from each source `src` (0..nc):
   - After BFS, for each `v > src` with dist[v] ≠ INF:
     - `ws += (deg[src]+deg[v]) × dist[v]`
     - `wg += deg[src] × deg[v] × dist[v]`
   - Track `ecc[src]` = max finite distance from src
3. After BFS loop: `cxe_ppm = Σ_v (if ecc[v]>0 { deg[v]×10⁶/ecc[v] } else { 0 })`

Stack arrays: `adj[MAX_NODES]` (u128), `deg[MAX_NODES]` (u32), `ecc[MAX_NODES]` (u32), `dist[MAX_NODES]` (u8), `queue[MAX_NODES]` (u8) — zero heap allocation.

---

## Cross-Check Table

| Graph | W_S | W_G | CξE_ppm | \|E\| | \|V\| |
|-------|-----|-----|---------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| Single node | 0 | 0 | 0 | 0 | 1 |
| Edge A-B | 2 | 1 | 2_000_000 | 1 | 2 |
| Path P₃ | 10 | 6 | 3_000_000 | 2 | 3 |
| Triangle K₃ | 12 | 12 | 6_000_000 | 3 | 3 |
| Star K_{1,4} | 44 | 28 | 6_000_000 | 4 | 5 |
| Path P₄ | 28 | 19 | 2_666_666 | 3 | 4 |
| Complete K₄ | 36 | 54 | 12_000_000 | 6 | 4 |
| Two isolated | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 66 | 78 | 6_000_000 | 6 | 5 |

### Key Derivations

**K₃ (Δ=2, ecc=1 all):**  
W_S = 3 pairs × (2+2)×1 = 12; W_G = 3×4 = 12; CξE = 3×2/1×10⁶ = 6_000_000

**K₄ (Δ=3, ecc=1 all):**  
W_S = 6×6 = 36; W_G = 6×9 = 54; CξE = 4×3×10⁶ = 12_000_000

**K_{1,4} (center deg=4 ecc=1; leaves deg=1 ecc=2):**  
W_S = 4×(4+1)×1 + 6×(1+1)×2 = 20+24 = 44  
W_G = 4×4×1×1 + 6×1×1×2 = 16+12 = 28  
CξE = 4/1×10⁶ + 4×(1/2×10⁶) = 4_000_000+2_000_000 = 6_000_000

**P₄ (deg=[1,2,2,1], ecc=[3,2,2,3]):**  
W_S = 3+6+6+4+6+3 = 28; W_G = 2+4+3+4+4+2 = 19  
CξE = ⌊10⁶/3⌋×2 + 10⁶×2 = 333_333×2+2_000_000 = 2_666_666

**K_{2,3} (left deg=3 ecc=2, right deg=2 ecc=2):**  
W_S = 12+8+8+8+5×6 = 66; W_G = 18+8+8+8+6×6 = 78  
CξE = ⌊3×10⁶/2⌋×2 + ⌊2×10⁶/2⌋×3 = 1_500_000×2+1_000_000×3 = 6_000_000

---

## Shell Interface

**Command routing** (k-shell/proc.rs):
```
"graph topo9" | "gtopo9" | "schultz mti" | "gws" |
"gutman index" | "gwg" | "connective eccentric" | "gcxe" | "gwsgwgcxe"
```

**Display** (`dispatch_graph_topo_indices9`):
- Header: bright-yellow "graph topo9 (W_S + W_G + CxiE degree-distance hybrid indices)"
- W_S: bright-cyan, exact integer, formula annotation [Σ (dᵤ+d_v)·d(u,v), u<v]
- W_G: bright-green, exact integer, formula annotation [Σ dᵤ·d_v·d(u,v), u<v]
- CξE: bright-magenta, ppm decimal (3 decimal places), formula [Σ deg(v)/ecc(v)]
- Footer: "N node(s)  M edge(s)  Schultz 1989  Gutman 1994  Gupta et al. 2000"

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
| **96** | **graph-topo9** (V3.20, new) |

---

## OS Analogy

- **W_S (Schultz MTI)**: total degree-weighted routing load — each hop is amplified by the combined degree of both endpoints; hub-adjacent long paths are doubly penalised (high degree AND far apart)
- **W_G (Gutman Index)**: product-degree routing pressure — quadratically amplifies load at hub-to-hub long-range connections; more sensitive to hub concentration than W_S
- **CξE (Connective Eccentric)**: per-node throughput-to-reach ratio — nodes with high degree but small eccentricity (universal hubs) contribute the most; 0 for isolated/leaf nodes with large radius

---

## Test Coverage

10 new tests in `gos-graph-topo9-harness`:
1. Empty graph → all zeros
2. Single isolated node → all zeros
3. Single edge A-B → (2, 1, 2_000_000, 1, 2)
4. Path P₃ → (10, 6, 3_000_000, 2, 3)
5. Triangle K₃ → (12, 12, 6_000_000, 3, 3)
6. Star K_{1,4} → (44, 28, 6_000_000, 4, 5)
7. Path P₄ → (28, 19, 2_666_666, 3, 4)
8. Complete K₄ → (36, 54, 12_000_000, 6, 4)
9. Two isolated nodes → all zeros
10. K_{2,3} bipartite cross-check → (66, 78, 6_000_000, 6, 5)

All 10 tests pass. Total host-test suite: **1173 tests** (1163 prior + 10 new).
