# GOSKernel Hardening Log — V3.18
**Date:** 2026-07-06  
**Branch:** feat/vk-auto-live-surface  
**Commit:** 89f7342  
**Host-test suite total:** 1153 tests (all green)

---

## Summary

V3.18 introduces the first **distance-based topological indices** in GOSKernel, crossing into a new algorithmic category. All previous topological indices (V3.12–V3.17) used pure degree-scan O(m) algorithms. V3.18 requires **BFS all-pairs shortest paths** O(n·(n+m)), producing three classical molecular-graph indices with direct OS analogies.

---

## New Feature: `graph topo7` — Wiener W + Harary H + Hyper-Wiener WW

### API

```rust
pub fn graph_topo_indices7() -> (u64, u64, u64, usize, usize)
// Returns: (wiener, harary_ppm, hyper_wiener, edge_count, node_count)
```

### Indices

| Index | Formula | Type | Literature |
|-------|---------|------|-----------|
| W  | Σ_{u<v} d(u,v) | exact u64 | Wiener 1947 |
| H  | Σ_{u<v} 1/d(u,v) × 10⁶ | floor ppm | Plavšić et al. 1993 |
| WW | (1/2) Σ_{u<v} [d + d²] = Σ d(d+1)/2 | exact u64 | Klein & Randić 1993 |

**Disconnected pairs:** d=∞ → contribute 0 to all three.

### Algorithm

BFS from each source node on undirected projection, O(n·(n+m)).  
Integer arithmetic only: no floating-point, no heap allocation.  
BFS uses stack-allocated `dist[MAX_NODES: u8]` and `queue[MAX_NODES: u8]` (INF=255, max BFS depth=126 for 128-node graph).  
Harary: `1_000_000 / d` per connected pair (floor).  
Hyper-Wiener: `d * (d + 1) / 2` per pair (always integer: d*(d+1) is even for all d≥1).

### Key Invariants

```
W(K_n)  = H_ppm/10^6 = WW(K_n) = n*(n-1)/2   (all pairs d=1)
W(P_n)  = n*(n²-1)/6                            (path formula; P₃=4, P₄=10)
WW ≥ W  always (equality iff graph is complete)
H ≥ W   always in ppm sense
Disconnected graph: W=H=WW=0
```

### Cross-Check Table (analytical)

| Graph    | W   | H_ppm     | WW  | edges | nodes |
|----------|-----|-----------|-----|-------|-------|
| Empty    | 0   | 0         | 0   | 0     | 0     |
| 1 node   | 0   | 0         | 0   | 0     | 1     |
| Edge A-B | 1   | 1_000_000 | 1   | 1     | 2     |
| P₃       | 4   | 2_500_000 | 5   | 2     | 3     |
| K₃       | 3   | 3_000_000 | 3   | 3     | 3     |
| K_{1,4}  | 16  | 7_000_000 | 22  | 4     | 5     |
| P₄       | 10  | 4_333_333 | 15  | 3     | 4     |
| K₄       | 6   | 6_000_000 | 6   | 6     | 4     |
| 2 isol.  | 0   | 0         | 0   | 0     | 2     |
| K_{2,3}  | 14  | 8_000_000 | 18  | 6     | 5     |

### Shell Commands

```
graph topo7   gtopo7   wiener index   gwiener
harary index  gharary  hyper wiener   ghyperw   gwienerhw
```

### OS Analogies

- **W (Wiener):** Total message-routing budget in kernel dependency graph. Minimizing W = minimizing average IPC hop cost across all subsystem pairs.
- **H (Harary):** Aggregate connectivity score — closer subsystems contribute more. Higher H = more efficiently coupled kernel. H→max for complete mesh.
- **WW (Hyper-Wiener):** Quadratic latency penalty. Amplifies long-range dependencies (d² term). Useful for identifying kernels where a few distant module pairs dominate overall routing cost.

### Display

- Bright-yellow header: `graph topo7  (W + H + WW distance-based indices)`
- W: bright-cyan (exact annotation)
- H: bright-green (ppm decimal display)
- WW: bright-magenta (exact annotation; disconnected annotation when wiener=0 and node_count>1)
- Footer: `N node(s)  M edge(s)  Wiener 1947  Plavšić et al. 1993  Klein & Randić 1993`

---

## Test Harness: `gos-graph-topo7-harness`

**Location:** `host-tests/gos-graph-topo7-harness/`  
**VectorAddress L4:** 94  
**Plugin ID:** `TOPO_IX7`

10 tests, all pass:

1. Empty graph → (0, 0, 0, 0, 0)
2. Single node → (0, 0, 0, 0, 1)
3. Single edge A→B → (1, 1_000_000, 1, 1, 2)
4. Path P₃ → (4, 2_500_000, 5, 2, 3)
5. Triangle K₃ → (3, 3_000_000, 3, 3, 3)
6. Star K_{1,4} → (16, 7_000_000, 22, 4, 5)
7. Path P₄ → (10, 4_333_333, 15, 3, 4)
8. Complete K₄ → (6, 6_000_000, 6, 6, 4)
9. Two isolated nodes → (0, 0, 0, 0, 2)
10. K_{2,3} cross-check → (14, 8_000_000, 18, 6, 5)

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
| **94** | **graph-topo7 (W/H/WW)** |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | +`graph_topo_indices7_inner()` + `graph_topo_indices7()` export |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_topo_indices7()` |
| `crates/k-shell/src/proc.rs` | +shell routing for topo7 |
| `host-tests/gos-graph-topo7-harness/` | new harness (4 files) |

---

## Metrics

- **New functions:** 2 (inner + public export)
- **New shell aliases:** 9
- **New tests:** 10
- **Cumulative host tests:** 1153
- **Algorithmic category:** First distance-based indices (requires BFS, not just degree scan)
