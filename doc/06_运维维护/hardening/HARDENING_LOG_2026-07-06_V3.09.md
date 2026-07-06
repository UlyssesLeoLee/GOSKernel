# Hardening Log V3.09 — Graph Spectral Analysis ρ(A) + λ₂(L)

**Date**: 2026-07-06  
**Branch**: feat/vk-auto-live-surface  
**Previous baseline**: V3.08 (edge coloring χ'(G), 1053 host tests)  
**New total**: 1063 host tests (+10)

---

## Algorithm: Graph Spectral Analysis

**Graph spectral analysis** studies the eigenvalues of matrices associated with a graph — the adjacency matrix A and the Laplacian L = D − A — to extract structural properties. This hardening adds two fundamental spectral invariants:

1. **Spectral radius ρ(A)** — the largest eigenvalue of the adjacency matrix
2. **Algebraic connectivity λ₂(L)** — the Fiedler value, the second-smallest Laplacian eigenvalue

### Theoretical Background

**Spectral radius ρ(A)** (Perron-Frobenius, 1907–1912):
- For a connected undirected graph, ρ(A) = max |λᵢ| over all eigenvalues λᵢ of A
- For d-regular graphs: ρ = d
- For complete graph K_n: ρ = n − 1  
- For star K_{1,k}: ρ = √k
- **Epidemic threshold**: disease spreads on a network iff βρ(A) > δ (infection rate × ρ > recovery rate). Above threshold (SIS model), epidemics persist; below, they die out.

**Algebraic connectivity λ₂(L)** (Fiedler 1973):
- λ₂(L) = 0 iff the graph is disconnected (by Kirchhoff's matrix-tree theorem)
- Larger λ₂ ↔ harder to disconnect ↔ faster consensus / diffusion
- **Cheeger inequality**: h(G) ≥ λ₂/2, where h(G) is the edge conductance (isoperimetric number)
- **Expander graphs**: high λ₂ relative to λ₁ characterises efficient communication topologies

---

## Implementation

### Phase 1 — ρ(A) via A² Power Iteration

**Key challenge**: A has eigenvalues ±ρ for many common graphs (paths P_n, even cycles, bipartite graphs). Simple A-iteration oscillates between the +ρ and −ρ eigenvectors without converging.

**Solution**: Iterate A² instead. Since A is real symmetric, A² is PSD with eigenvalues λᵢ² ≥ 0. No sign oscillation occurs. The Rayleigh quotient R(x, A²x) → ρ(A²) = ρ(A)².

**No f32::sqrt in no_std**: Recovering ρ(A) = √ρ(A²) requires sqrt. Instead, use integer Newton-Raphson (`isqrt64`) on the ppm-scaled value:

```rust
fn isqrt64(n: u64) -> u64 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}
// rho_sq_u = floor(ρ(A)² × 1e6) as u64
// rho_ppm  = isqrt64(rho_sq_u × 1_000_000) = floor(ρ(A) × 1e6)
```

Error bound: ≤1 ppm from integer floor; well within 5000 ppm tolerance.

### Phase 2 — λₙ(L) via Laplacian Iteration

Iterates L·x with mean-centering to deflate the zero eigenspace (all-ones direction). Rayleigh quotient converges to λₙ(L) = max Laplacian eigenvalue.

### Phase 3 — λ₂(L) via Shifted Deflation

Sets B = μI − L with μ = λₙ + 1. B's eigenvalues are μ−λᵢ, all positive. The largest B-eigenvalue (= μ, from the zero Laplacian mode) is deflated by mean-centering. Power iteration on the deflated B converges to the second-largest B-eigenvalue = μ − λ₂. Thus λ₂ = μ − (converged Rayleigh quotient).

Guard: for nc ≤ 1, λ₂ is undefined; return (rho_ppm, 0, nc) early.

### Iteration Counts

- Phase 1 (A²): 60 steps
- Phase 2 (λₙ): 60 steps  
- Phase 3 (λ₂): 80 steps

### Stack Usage

Additional over previous (~128 bytes):
- `w1 [f32; MAX_NODES]` = 512 B (Phase 1 intermediate)
- `w2 [f32; MAX_NODES]` = 512 B (Phase 1 A² product)
- All other arrays reuse existing slots

---

## Runtime API

```rust
pub fn graph_spectral() -> (u32, u32, usize)
```

Returns `(rho_ppm, lambda2_ppm, node_count)`:
- `rho_ppm` — ρ(A) × 1_000_000 as u32 (spectral radius of adjacency matrix)
- `lambda2_ppm` — λ₂(L) × 1_000_000 as u32 (Fiedler value; 0 if disconnected or nc≤1)
- `node_count` — number of active compact-indexed nodes

---

## K-Shell Commands

```
graph spectral   — display ρ(A) spectral radius + λ₂(L) algebraic connectivity
gspectral        — alias
spectral radius  — alias
spectral         — alias
gspectrum        — alias
graph spectrum   — alias
```

**Display**: bright-blue header; yellow values; epidemic threshold annotation on ρ; Cheeger bound (h≥λ₂/2) annotation on λ₂; green "connected" / red "disconnected" indicator; footer: `N node(s)  power iteration (60 steps)  Fiedler 1973`

---

## VectorAddress Namespace

**L4=85** for `gos-graph-spectral-harness`

---

## Test Harness: gos-graph-spectral-harness (10 tests)

| Test | Graph | ρ(A) | λ₂(L) |
|------|-------|------|--------|
| 1 | Empty (0 nodes) | 0 | 0 |
| 2 | Single node | 0 | 0 (nc≤1 guard) |
| 3 | Single edge K₂ | 1.000 | 2.000 |
| 4 | Path P₃ (A-B-C) | √2 ≈ 1.414 | 1.000 |
| 5 | Triangle K₃ | 2.000 | 3.000 |
| 6 | Complete K₄ | 3.000 | 4.000 |
| 7 | Star K_{1,4} | 2.000 | 1.000 |
| 8 | 2 isolated nodes | 0.000 | 0 (disconnected) |
| 9 | K₂ + isolated node | 1.000 | 0 (disconnected) |
| 10 | Cycle C₄ | 2.000 | 2.000 |

All 10 pass with TOLERANCE = 5_000 ppm (±0.5%).

**Notable eigenvalue corrections during development**:
- P₃ Laplacian spectrum is {0, 1, 3} → λ₂=1 (NOT 2-√2 as initially estimated). The {0, 2-√2, 2+√2} values belong to a different graph.

---

## Engineering Challenges Resolved

1. **A-iteration oscillation** (P₃, K_{1,4}): Single A-iteration cycles between ±λ eigenvectors without converging. Fix: iterate A² — eigenvalues are all λᵢ² ≥ 0, oscillation impossible.

2. **No f32::sqrt in no_std**: Standard `sqrt` is a `std` method. Fix: integer Newton-Raphson `isqrt64` on the u64 ppm-squared value gives ≤1 ppm error.

3. **nc=1 degenerate case**: Single-node graph has 1D Laplacian with eigenvalue 0 only; λ₂ is undefined. Fix: early return with lambda2_ppm=0 before Phase 3 setup.

4. **Mutex poisoning cascade**: A test panic while holding `TEST_LOCK` poisons the mutex, causing all subsequent tests to fail. Fix: `lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoning.

---

## OS Analogy

The spectral radius ρ(A) and algebraic connectivity λ₂(L) describe **information propagation** and **fault tolerance** in the OS inter-process communication graph:

- **ρ(A) ↔ broadcast amplification**: The epidemic threshold 1/ρ is the critical message fanout below which broadcast storms self-terminate. Kernel message routing should target ρ < 1/β where β is the per-hop duplication rate.
- **λ₂(L) ↔ partition resistance**: Low λ₂ means the process graph has a bottleneck — one IPC channel failure can isolate a subsystem. High λ₂ means the graph is well-connected (expander property). Production deployments target λ₂ > 0.5 for 3-fault tolerance.
- **Cheeger h ≥ λ₂/2**: The minimum edge cut (as a fraction of volume) is lower-bounded by the Fiedler value. A kernel scheduler can use λ₂ to determine whether load-balancing across subsystems is safe.

This mirrors Linux's use of NUMA topology analysis for task placement and macOS's Grand Central Dispatch queue graph connectivity checks.

---

## Relation to Existing Algorithms

| Algorithm | Version | Relation |
|-----------|---------|---------|
| PageRank | V2.xx | PageRank principal eigenvector; ρ(A) is the corresponding eigenvalue |
| Vertex connectivity κ(G) | V3.07 | both measure robustness; κ≥λ₂/Δ (Fiedler bound) |
| Community detection (LPA) | V3.xx | communities correspond to near-zero λ₂ cuts |
| Edge betweenness | V3.06 | high-betweenness edges are typically low-λ₂ bridges |

---

## Literature

- Fiedler, M. (1973). "Algebraic connectivity of graphs." *Czechoslovak Mathematical Journal* 23(98): 298–305. (Algebraic connectivity λ₂.)
- Perron, O. (1907). "Zur Theorie der Matrizen." *Math. Ann.* 64: 248–263. (Dominant eigenvalue theory.)
- Frobenius, G. (1912). "Über Matrizen aus nicht negativen Elementen." *Sitzungsber. Kgl. Preuss. Akad. Wiss.* (Perron-Frobenius theorem.)
- Wang, Y. et al. (2003). "Epidemic spreading in real networks: An eigenvalue viewpoint." *SRDS 2003*. (ρ(A) and epidemic threshold.)
- Cheeger, J. (1970). "A lower bound for the smallest eigenvalue of the Laplacian." *Problems in Analysis*. Princeton UP. (Cheeger inequality.)
