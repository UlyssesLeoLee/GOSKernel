# Hardening Log V3.11 — Zagreb Indices M1/M2 + Randić R + Albertson I

**Date**: 2026-07-06  
**Branch**: feat/vk-auto-live-surface  
**Previous baseline**: V3.10 (graph entropy H(G), 1073 host tests)  
**New total**: 1083 host tests (+10)

---

## Algorithm: Zagreb / Randić / Albertson Topological Indices

V3.11 adds four classical degree-based topological indices in a single O(V+E) scan:

### First Zagreb Index M₁ (Gutman & Trinajstić 1972)

> M₁(G) = Σ_v deg(v)²

The sum of squared degrees. Equivalently, M₁ = Σ_{uv∈E} (deg(u) + deg(v)) — both formulations give the same result. M₁ captures the "degree heterogeneity pressure" of the graph; for regular graphs, M₁ = n × d² where d is the uniform degree.

### Second Zagreb Index M₂ (Gutman & Trinajstić 1972)

> M₂(G) = Σ_{uv∈E} deg(u) × deg(v)

The sum of degree-products over undirected edges. M₂ measures hub-to-hub co-dependency: high M₂ means high-degree nodes tend to be directly connected.

### Randić Connectivity Index R (Randić 1975)

> R(G) = Σ_{uv∈E} 1/√(deg(u) × deg(v))

One of the most widely studied topological descriptors in chemical graph theory. Introduced by Randić as a branching index for molecular graphs. Computed via Newton-Raphson isqrt: contribution = floor(10¹²/isqrt_ppm(deg(u)×deg(v))), error ≤ 1 ppm per edge.

### Albertson Irregularity Index I (Albertson 1997)

> I(G) = Σ_{uv∈E} |deg(u) − deg(v)|

Measures total degree-imbalance across edges. I = 0 if and only if the graph is regular. Provides a simple, computationally cheap irregularity measure.

## Implementation

- `gos_runtime::graph_zagreb()` → `(m1: u64, m2: u64, randic_ppm: u32, irregularity: u32, edge_count: usize, node_count: usize)`
- Single pass over undirected adjacency bitmasks (a < b canonical to avoid double-counting)
- Builds undirected `adj[]` and `deg[]` arrays from directed edge list
- M₁ computed in a separate O(V) node scan; M₂/R/I in the O(E) edge scan
- isqrt_ppm(p) = Newton-Raphson floor(√p × 10⁶) — shared with spectral module

## Shell Commands

`graph zagreb` · `gzagreb` · `zagreb` · `zagreb index` · `graph topo index` · `randic` · `graph randic`

## Test Harness

**gos-graph-zagreb-harness** — 10 tests, VectorAddress L4=87:

| # | Graph | M1 | M2 | R_ppm | I |
|---|-------|----|----|-------|---|
| 1 | Empty | 0 | 0 | 0 | 0 |
| 2 | Single node | 0 | 0 | 0 | 0 |
| 3 | Edge A→B | 2 | 1 | 1_000_000 | 0 |
| 4 | Path P₃ | 6 | 4 | 1_414_214 | 2 |
| 5 | Triangle K₃ | 12 | 12 | 1_500_000 | 0 |
| 6 | Star K_{1,4} | 20 | 16 | 2_000_000 | 12 |
| 7 | Path P₄ | 10 | 8 | 1_914_214 | 2 |
| 8 | Complete K₄ | 36 | 54 | 1_999_998 | 0 |
| 9 | Two isolated | 0 | 0 | 0 | 0 |
| 10 | K_{2,3} | 30 | 36 | 2_449_488 | 6 |

## OS Analogy

- **M₁**: Degree-squared coupling pressure — sum of squared dependency fan-outs across all kernel subsystems.
- **M₂**: Hub-to-hub co-dependency — how tightly high-fan-out modules are directly coupled to each other.
- **R**: Randić connectivity index — a branching measure for the kernel dependency graph; low R = star topology (hub-and-spoke IPC), high R = regular mesh.
- **I**: IPC channel load imbalance — total degree mismatch across edges; I = 0 means all subsystems have equal coupling, the ideal for balanced scheduling.

## Literature

- Gutman, I. & Trinajstić, N. (1972). Graph theory and molecular orbitals. *Chemical Physics Letters*, 17(4), 535–538.
- Randić, M. (1975). Characterization of molecular branching. *JACS*, 97(23), 6609–6615.
- Albertson, M.O. (1997). The irregularity of a graph. *Ars Combinatoria*, 46, 219–225.
