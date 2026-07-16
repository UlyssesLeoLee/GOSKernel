# HARDENING LOG — V3.42 — 2026-07-16

## Version
**V3.42** — NSig + NHQS + NPS Neighborhood S-variant topological indices + gos-graph-topo31-harness (10 tests)

## Branch
`feat/vk-auto-live-surface` (auto-hardening scheduled task)

## Motivation
Continuing the S-variant topological index family introduced in V3.29 (NM₁/NM₂/GA₂) through topo30 (NVQ/NRGS/NHCS). This release extends two ongoing vertex-power and edge-power series to their next terms, and introduces S-Sigma as the S-variant of the standard Sigma irregularity index.

## Indices Added

### NSig — S-Sigma Irregularity
```
NSig(G) = Σ_{uv∈E} (S_u − S_v)²
```
- S-variant of the classical Sigma irregularity index σ(G) = Σ(d_u−d_v)² (Gutman, Togan, Yurttas et al.)
- **NSig = 0 iff S-regular** (all neighbor-degree sums equal across each edge endpoint pair)
- Exact u64; no integer overflow for realistic graphs (max per edge ≤ 16129² ≈ 2.60×10⁸; sum < u64::MAX)
- Complements NM3 = Σ|S_u−S_v| (topo23): NM3 uses absolute value, NSig uses squares

### NHQS — Neighborhood Hyper Quartic Sum
```
NHQS(G) = Σ_{uv∈E} (S_u + S_v)^4
```
- Extends the edge-sum power series: NHM1 = Σ(S+S)² (topo23), NHCS = Σ(S+S)³ (topo30) → NHQS = Σ(S+S)⁴
- **NHQS = 16|E|S⁴ for S-regular** (since (2S)⁴ = 16S⁴)
- K₃ and K_{1,4}: both S-uniform S=4 → same per-edge NHQS (8⁴=4096); totals differ by |E|
- u128 accumulator → u64 output (per-edge values fit u64; edge-sum for max graphs could exceed)

### NPS — Neighborhood Penta Sum
```
NPS(G) = Σ_v S(v)^5
```
- Extends the vertex-power series: NM₁=Σ S² (topo18), NF=Σ S³ (topo22), NVQ=Σ S⁴ (topo30) → NPS=Σ S⁵
- **NPS = n·S⁵ for S-regular**
- u128 accumulator → u64 output (S⁵ can exceed u64::MAX for maximum-degree nodes in large graphs)

## Cross-Check Table

| Graph    | NSig | NHQS    | NPS     | edges | nodes |
|----------|------|---------|---------|-------|-------|
| Empty    | 0    | 0       | 0       | 0     | 0     |
| 1 node   | 0    | 0       | 0       | 0     | 1     |
| K₂       | 0    | 16      | 2       | 1     | 2     |
| P₃       | 0    | 512     | 96      | 2     | 3     |
| K₃       | 0    | 12,288  | 3,072   | 3     | 3     |
| K_{1,4}  | 0    | 16,384  | 5,120   | 4     | 5     |
| P₄       | 2    | 2,546   | 550     | 3     | 4     |
| K₄       | 0    | 629,856 | 236,196 | 6     | 4     |
| 2 isolated| 0   | 0       | 0       | 0     | 2     |
| K_{2,3}  | 0    | 124,416 | 38,880  | 6     | 5     |

Notable: P₄ is the only test case with NSig > 0 (S-irregular: S values 2,3,3,2 → two edges with S_u≠S_v).

## Algorithm
O(V+E) — degree pass → S(v) pass → vertex scan (NPS) + edge scan (NSig, NHQS); no BFS needed.

## Implementation

### gos-runtime/src/lib.rs
- Added `graph_topo_indices31_inner()` on `GosRuntime`
- Added `graph_topo_indices31() -> (u64, u64, u64, usize, usize)` public function
- Return order: (nsig, nhqs, nps, edge_count, node_count)

### k-shell/src/lib.rs
- Added `dispatch_graph_topo_indices31()` with colored output:
  - NSig: bright-cyan (exact; "NSig=0: S-regular" annotation when applicable)
  - NHQS: bright-green (exact)
  - NPS: bright-magenta (exact)

### k-shell/src/proc.rs
- Routing: `"graph topo31"` | `"gtopo31"` | `"neighborhood sigma"` | `"gnsig"` | `"neighborhood quartic edge"` | `"gnhqs"` | `"neighborhood penta"` | `"gnps"` | `"gnsignhqsnps"`

### host-tests/gos-graph-topo31-harness/
- New standalone test harness, VectorAddress L4=118
- 10 tests: all pass ✓

## Test Count
- Prior total: 1383 (V3.41)
- New tests: 10 (gos-graph-topo31-harness)
- **New total: 1393 host tests**

## VectorAddress L4 Namespace (updated)
88=graph-topo through 117=graph-topo30, **118=graph-topo31** (new)

## Series Context
This commit continues the "S-variant family" pattern of replacing degree d(v) with neighbor-degree sum S(v) = Σ_{w∈N(v)} deg(w) in classical topological index formulas:
- **Vertex power series**: NM₁(Σ S²) → NF(Σ S³) → NVQ(Σ S⁴) → **NPS(Σ S⁵)**
- **Edge-sum power series**: NHM1(Σ(S+S)²) → NHCS(Σ(S+S)³) → **NHQS(Σ(S+S)⁴)**
- **Irregularity**: NM3(Σ|S−S|) → **NSig(Σ(S−S)²)**
