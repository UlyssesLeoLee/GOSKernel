# GOSKernel Hardening Log — V3.11
**Date:** 2026-07-06  
**Algorithm:** Zagreb Indices M1/M2, Randić Connectivity R, Albertson Irregularity I  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.11): Zagreb indices M1/M2 + Randic R + Albertson I + gos-graph-zagreb-harness (10 tests)

---

## Summary

V3.11 adds **four classic topological graph indices** to the GOSKernel graph theory runtime:

| Index | Formula | Meaning |
|-------|---------|---------|
| **M1(G)** — first Zagreb | Σ_v deg(v)² | Degree-squared sum; coupling density |
| **M2(G)** — second Zagreb | Σ_{uv∈E} deg(u)×deg(v) | Co-degree product; hub interconnection |
| **R(G)** — Randić index | Σ_{uv∈E} 1/√(deg(u)×deg(v)) | Degree-product reciprocal; connectivity strength |
| **I(G)** — Albertson irregularity | Σ_{uv∈E} \|deg(u)−deg(v)\| | Degree imbalance across edges; 0 iff regular |

These are the canonical "first generation" topological molecular indices, originating in chemical
graph theory (1972–1997) and now fundamental in network science.  All four are computed in
**O(V+E)** with pure integer arithmetic — no float, no heap, no_std safe.

**OS analogy:**
- M1 = total coupling pressure on kernel subsystems (high M1 = hub nodes absorb many dependencies)
- M2 = co-dependency density between interconnected hubs (high M2 = tight clusters of high-degree nodes)
- R = connectivity strength weighted by reciprocal degree product (high R = many weak-coupling edges)
- I = structural imbalance of IPC channels (I=0 = all channels connect equally-loaded subsystems)

---

## Public API

### `gos_runtime::graph_zagreb() -> (u64, u64, u32, u32, usize, usize)`

Returns `(m1, m2, randic_ppm, irregularity, edge_count, node_count)`:

- `m1` — M1(G) = Σ_v deg(v)²  (first Zagreb index; Gutman & Trinajstić 1972)
- `m2` — M2(G) = Σ_{uv∈E} deg(u)×deg(v)  (second Zagreb index; undirected edges)
- `randic_ppm` — R(G) × 10^6 where R = Σ_{uv∈E} 1/√(deg(u)×deg(v))  (Randić 1975)
- `irregularity` — I(G) = Σ_{uv∈E} |deg(u)−deg(v)|  (Albertson 1997; 0 iff regular graph)
- `edge_count` — undirected edge count (directed→undirected dedup, self-loops excluded)
- `node_count` — live node count

**Shell keywords:** `graph zagreb` / `gzagreb` / `zagreb` / `zagreb index` / `graph topo index` / `randic` / `graph randic`  
**VectorAddress L4=87** for gos-graph-zagreb-harness.

---

## Algorithm

### Step 1 — Compact Node Index

Build `slot_to_ci[i]` mapping: for each live `nodes[i]`, assign compact index `ci`. This is
the standard compact-index construction used by all graph metrics.

### Step 2 — Undirected Adjacency Bitmasks

Scan all 512 edge slots. For each live edge (f_sl → t_sl):
- Map to compact indices `f_ci`, `t_ci`.
- Skip self-loops (f_ci == t_ci) and invalid slots.
- If `adj[f_ci]` bit `t_ci` not yet set, set both `adj[f_ci] |= 1<<t_ci` and `adj[t_ci] |= 1<<f_ci`.

This converts the directed edge store to an undirected deduped adjacency.

### Step 3 — Undirected Degrees

`deg[ci] = adj[ci].count_ones()` — hardware popcount instruction.

### Step 4 — M1

`M1 = Σ_{ci=0..nc} deg[ci]²` — a single pass over nodes.

### Step 5 — Edge Scan (M2, R, I)

For each `a ∈ [0..nc)` and each `b ∈ adj[a]` where `b > a` (canonical undirected edge `(a,b)`):

```
M2          += deg[a] × deg[b]
irregularity += |deg[a] − deg[b]|
s            = floor(sqrt(deg[a]×deg[b] × 10^12))    // Newton-Raphson isqrt_ppm
randic_acc  += floor(10^12 / s)                       // ppm contribution
edge_count  += 1
```

The `b > a` guard guarantees each undirected edge is counted exactly once without a
secondary `seen_adj` bitmask.

### Randić Precision

`isqrt_ppm(p)` computes `floor(sqrt(p × 10^12))` via Newton-Raphson integer sqrt:

```rust
fn isqrt_ppm(p: u64) -> u64 {
    if p == 0 { return 0; }
    let n = p.saturating_mul(1_000_000_000_000u64);  // p × 10^12
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x  // floor(sqrt(p) × 10^6)
}
```

Contribution per edge = `floor(10^12 / isqrt_ppm(p))` ≈ `10^6 / sqrt(p)`.

The floor error is at most 1 ppm per edge.  For graphs where all degree products are
perfect squares (regular graphs, stars with even degree) the result is exact.

---

## Key Mathematical Properties

### Identity: M1 via edge sums

M1(G) = Σ_{uv∈E} (deg(u) + deg(v)) — both definitions are equivalent (each node v contributes
deg(v) to the sum for each of its deg(v) incident edges, giving Σ_v deg(v)² = M1).

### Randić bounds

- **Matching (all deg=1):** R = m (edge count) — maximum connectivity strength per edge.
- **Complete Kₙ:** R = n(n-1)/2 / (n-1) = n/2 — scales with graph size.
- **Star K_{1,k}:** R = k/√k = √k — grows as square root of leaves.

### Regularity invariant

I(G) = 0 ↔ G is edge-regular (all edges connect nodes of equal degree).  Regular graphs
(all degrees equal) always satisfy I=0, but the converse is not true for general graphs.

### Relation to spectral radius (V3.09)

For d-regular graphs: M1 = n·d², M2 = m·d², R = m/d, I = 0, and ρ(A) = d.

---

## Validation Table

| Graph | M1 | M2 | R_ppm | I | edges | Exact? |
|-------|----|----|-------|---|-------|--------|
| Empty | 0 | 0 | 0 | 0 | 0 | ✓ exact |
| Single node | 0 | 0 | 0 | 0 | 0 | ✓ exact |
| Edge A-B | 2 | 1 | 1_000_000 | 0 | 1 | ✓ exact (p=1, sqrt=1) |
| Path P₃ | 6 | 4 | 1_414_214 | 2 | 2 | ✓ floor(10¹²/1_414_213)=707_107 |
| Triangle K₃ | 12 | 12 | 1_500_000 | 0 | 3 | ✓ exact (p=4, sqrt=2) |
| Star K_{1,4} | 20 | 16 | 2_000_000 | 12 | 4 | ✓ exact (p=4, R=2) |
| Path P₄ | 10 | 8 | 1_914_214 | 2 | 3 | ✓ 2×707_107+500_000 |
| Complete K₄ | 36 | 54 | 1_999_998 | 0 | 6 | ⚠ floor err -2 (p=9, 6×333_333) |
| Two isolated | 0 | 0 | 0 | 0 | 0 | ✓ exact |
| K_{2,3} | 30 | 36 | 2_449_488 | 6 | 6 | ⚠ floor err -2 (p=6, 6×408_248) |

Floor error in Randić: occurs when the edge degree product p is not a perfect square.
The error is bounded by 1 per edge.  M1, M2, and I are always exact integers.

---

## Shell Display

```
 graph zagreb  (M1 + M2 + Randić R + Albertson I)
 ───────────────────────────────────────────────────────────
  first Zagreb   M₁  =  20   [Σ deg(v)²]
  second Zagreb  M₂  =  16   [Σ deg(u)×deg(v)]
  Randić index   R   =  2.000   [Σ 1/√(deg(u)×deg(v))]
  irregularity   I   =  12   [Σ |deg(u)−deg(v)|]
 ───────────────────────────────────────────────────────────
5 node(s)  4 edge(s)  Gutman & Trinajstić 1972  Randić 1975
```

Header: bright-cyan (color 11).  M1/M2: bright-yellow values.  R: bright-green.
I: bright-green (=0, regular) or bright-red (>0, irregular).

---

## Test Harness

**gos-graph-zagreb-harness** (L4=87):
- 10 integration tests covering empty, single-node, edge, paths, triangle, star, K₄, K_{2,3}
- Exact integer assertions for M1, M2, irregularity; ppm assertions for Randić
- Covers both exact (perfect-square p) and floor-rounding (non-square p) Randić cases

All 10 tests pass on host target (x86_64-pc-windows-msvc).

**Total host-test suite: 1083 tests** (1073 prior + 10 new)

---

## Literature

- I. Gutman & N. Trinajstić (1972). Graph theory and molecular orbitals. Total φ-electron energy of alternant hydrocarbons. *Chemical Physics Letters* 17(4): 535–538. — **First Zagreb index M1, Second Zagreb index M2**
- M. Randić (1975). On characterization of molecular branching. *Journal of the American Chemical Society* 97(23): 6609–6615. — **Randić connectivity index R**
- M. O. Albertson (1997). The irregularity of a graph. *Ars Combinatoria* 46: 219–225. — **Albertson irregularity I**
- R. Todeschini & V. Consonni (2000). *Handbook of Molecular Descriptors*. Wiley-VCH. — **Topological index overview**

---

## Cumulative V3 Feature Table

| Version | Algorithm | L4 |
|---------|-----------|-----|
| V3.00 | Minimum spanning arborescence (Chu-Liu/Edmonds 1967) | 76 |
| V3.01 | Feedback vertex set (greedy Kahn) | 77 |
| V3.02 | Global minimum cut (Stoer-Wagner 1997) | 78 |
| V3.03 | Hamiltonian path/circuit (iterative backtracking) | 79 |
| V3.04 | Chordal recognition (LexBFS PEO) | 80 |
| V3.05 | Biconnected components (Tarjan iterative) | 81 |
| V3.06 | Edge betweenness centrality (Brandes 2001) | 82 |
| V3.07 | Vertex connectivity κ(G) (Even 1975) | 83 |
| V3.08 | Edge coloring χ'(G) (Vizing 1964) | 84 |
| V3.09 | Spectral analysis ρ(A)+λ₂(L) (Fiedler 1973) | 85 |
| V3.10 | Graph entropy H (Shannon 1948) | 86 |
| **V3.11** | **Zagreb M1/M2 + Randić R + Albertson I** | **87** |
