# GOSKernel Hardening Log — V3.19
**Date:** 2026-07-07  
**Branch:** feat/vk-auto-live-surface  
**Host-test suite total:** 1163 tests (all green)

---

## Summary

V3.19 introduces **eccentricity-based topological indices** — the natural algorithmic sequel to V3.18's distance-based (Wiener/Harary/Hyper-Wiener) indices. All four metrics are computed from the same BFS pass that produces per-node eccentricities, giving a richer structural picture of the graph's "compactness" and "centrality by extremes."

---

## New Feature: `graph topo8` — ECI + D + R + avg-ecc

### API

```rust
pub fn graph_topo_indices8() -> (u64, u64, u32, u32, usize, usize)
// Returns: (eci, avg_ecc_ppm, diameter, radius, edge_count, node_count)
```

Note: returns a 6-tuple (matching the pattern of `graph_zagreb`), since diameter and radius are separate structural quantities.

### Indices

| Symbol | Formula | Type | Literature |
|--------|---------|------|-----------|
| ξ (ECI) | Σ_v deg(v) × ecc(v) | exact u64 | Sharma, Goswami & Madan 1997 |
| avg_ecc | (Σ_v ecc(v)) / n × 10⁶ | floor ppm | Buckley & Harary 1990 |
| D | max_{v} ecc(v) | exact u32 | classical graph theory |
| R | min{ecc(v) \| ecc(v) > 0} | exact u32 | classical graph theory |

**ecc(v):** max BFS distance from v to any reachable node (0 for isolated or single-node graphs).  
**Disconnected nodes:** ecc = 0; contribute 0 to ECI and avg_ecc. D reflects max reachable ecc; R reflects min positive ecc (0 if no connected pairs).

### Algorithm

BFS from each node on undirected projection, O(n·(n+m)).  
Integer arithmetic only: no floating-point, no heap.  
Stack-allocated: `dist[MAX_NODES: u8]`, `queue[MAX_NODES: u8]`, `ecc[MAX_NODES: u32]`, `deg[MAX_NODES: u32]`.  
Single BFS pass per source computes ecc[src]; second scan over ecc[] accumulates ECI, avg_ecc_sum, D, R.

### Key Invariants

```
Complete K_n:     D=R=1 (self-centered); ECI=n*(n-1); avg_ecc_ppm=1_000_000
Path P_n:         D=n-1; R=⌈(n-1)/2⌉; endpoints have ecc=n-1, centre has ecc=⌈(n-1)/2⌉
Star K_{1,k}:     D=2 (leaves), R=1 (centre); ECI=k+2k=3k
Self-centered:    D=R (e.g. K_n, complete bipartite K_{m,n})
All isolated:     ECI=0; avg=0; D=0; R=0
Regularity clue:  ECI = n * Δ * D for Δ-regular D-diameter-1 (complete) graphs
```

### Cross-Check Table (analytical)

| Graph    | ECI | avg_ecc_ppm | D | R | edges | nodes |
|----------|-----|-------------|---|---|-------|-------|
| Empty    | 0   | 0           | 0 | 0 | 0     | 0     |
| 1 node   | 0   | 0           | 0 | 0 | 0     | 1     |
| Edge A-B | 2   | 1_000_000   | 1 | 1 | 1     | 2     |
| P₃       | 6   | 1_666_666   | 2 | 1 | 2     | 3     |
| K₃       | 6   | 1_000_000   | 1 | 1 | 3     | 3     |
| K_{1,4}  | 12  | 1_800_000   | 2 | 1 | 4     | 5     |
| P₄       | 14  | 2_500_000   | 3 | 2 | 3     | 4     |
| K₄       | 12  | 1_000_000   | 1 | 1 | 6     | 4     |
| 2 isol.  | 0   | 0           | 0 | 0 | 0     | 2     |
| K_{2,3}  | 24  | 2_000_000   | 2 | 2 | 6     | 5     |

**Derivations:**
- P₃ (A-B-C): ecc(A)=2, ecc(B)=1, ecc(C)=2; deg(A)=deg(C)=1, deg(B)=2 → ECI=1×2+2×1+1×2=6; avg=5/3→1_666_666
- K_{1,4}: centre ecc=1 deg=4; 4 leaves ecc=2 deg=1 → ECI=4+8=12; avg=9/5=1.8→1_800_000
- P₄: ecc={3,2,2,3}, deg={1,2,2,1} → ECI=3+4+4+3=14; avg=10/4=2.5→2_500_000
- K_{2,3}: all ecc=2; ECI=3×2+3×2+2×2+2×2+2×2=24; avg=2→2_000_000; D=R=2 (self-centered)

### Shell Commands

```
graph topo8          gtopo8        eccentric connectivity   geci
graph eci            graph diameter gdiameter               graph radius
gradius              gecidrc
```

### OS Analogies

- **ECI (ξ):** Weighted "reach pressure" — high-degree nodes that are also far from others impose disproportionate routing cost. Kernel hub subsystems with high ECI are candidates for replication or caching.
- **Diameter D:** Worst-case IPC latency (max hop count across any pair). D is the bottleneck for fault propagation in a ring-free kernel dependency graph.
- **Radius R:** Best-case "centre" accessibility — the minimum number of hops any subsystem needs to reach the most central node. R=1 means there is a universal hub reachable in one hop.
- **avg_ecc:** Average structural distance from any node to its furthest reachable peer. Lower avg_ecc = more tightly clustered subsystem graph. For completely regular D=1 graphs (complete mesh), avg_ecc=1.

### Display

- Bright-yellow header: `graph topo8  (ECI + D + R + avg-ecc eccentricity-based indices)`
- ξ: bright-cyan (exact)
- D: bright-green; annotates "(all isolated)" if D=0 and nc>1; "(self-centered)" if D=R>0
- R: bright-magenta
- avg_ecc: bright-blue (ppm decimal display)
- Footer: `N node(s)  M edge(s)  Sharma et al. 1997  Buckley & Harary 1990`

---

## Test Harness: `gos-graph-topo8-harness`

**Location:** `host-tests/gos-graph-topo8-harness/`  
**VectorAddress L4:** 95  
**Plugin ID:** `TOPO_IX8`

10 tests, all pass:

1. Empty graph → (0, 0, 0, 0, 0, 0)
2. Single node → (0, 0, 0, 0, 0, 1)
3. Single edge A→B → (2, 1_000_000, 1, 1, 1, 2)
4. Path P₃ → (6, 1_666_666, 2, 1, 2, 3)
5. Triangle K₃ → (6, 1_000_000, 1, 1, 3, 3)
6. Star K_{1,4} → (12, 1_800_000, 2, 1, 4, 5)
7. Path P₄ → (14, 2_500_000, 3, 2, 3, 4)
8. Complete K₄ → (12, 1_000_000, 1, 1, 6, 4)
9. Two isolated nodes → (0, 0, 0, 0, 0, 2)
10. K_{2,3} cross-check → (24, 2_000_000, 2, 2, 6, 5)

---

## VectorAddress L4 Namespace (updated)

| L4 | Harness |
|----|---------|
| 88 | graph-topo (SC/GA/AZI) |
| 89 | graph-topo2 (H/ABC/F) |
| 90 | graph-topo3 (SDD/ISI/NI) |
| 91 | graph-topo4 (Sombor/RM2/sigma) |
| 92 | graph-topo5 (HM1/HM2/AG) |
| 93 | graph-topo6 (EM1/ABS/RRR) |
| 94 | graph-topo7 (W/H/WW) |
| **95** | **graph-topo8 (ECI/D/R/avg-ecc)** |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | +`graph_topo_indices8_inner()` + `graph_topo_indices8()` export |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_topo_indices8()` |
| `crates/k-shell/src/proc.rs` | +shell routing for topo8 (9 aliases) |
| `host-tests/gos-graph-topo8-harness/` | new harness (5 files) |

---

## Metrics

- **New functions:** 2 (inner + public export)
- **New shell aliases:** 9
- **New tests:** 10
- **Cumulative host tests:** 1163
- **Algorithmic category:** Eccentricity-based (BFS all-pairs, O(n·(n+m)), same as V3.18)
- **Return type:** 6-tuple `(u64, u64, u32, u32, usize, usize)` — matches `graph_zagreb` pattern
