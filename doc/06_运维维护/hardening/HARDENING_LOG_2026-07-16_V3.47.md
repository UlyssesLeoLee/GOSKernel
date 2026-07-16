# HARDENING LOG — V3.47 — 2026-07-16

## Summary

Added three new Neighborhood S-variant topological indices extending the S-power series to
the 10th degree, completing topo36:

- **NDC(G)** = Σ_v S(v)^10 — S-Decic vertex sum (10th power)
- **NHNC(G)** = Σ_{uv∈E} (S_u+S_v)^9 — S-Nonic edge-sum (9th power)
- **NOSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^4 — S-Octic Sombor (SO^α with α=8; exact integer)

All three use S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum), the S-variant of degree.

## Changes

### crates/gos-runtime/src/lib.rs

Added `graph_topo_indices36_inner()` (Runtime impl) and public `graph_topo_indices36()`:

```
pub fn graph_topo_indices36() -> (u64, u64, u64, usize, usize)
  Returns (ndc, nhnc, noso, edge_count, node_count)
```

Algorithm: O(V+E) — degree pass → S(v) pass → vertex scan (NDC) + edge scan (NHNC, NOSO).
All three use u128 accumulators with saturating_mul/add; no BFS; no isqrt.

### crates/k-shell/src/lib.rs

Added `dispatch_graph_topo_indices36()` — bright-yellow header, bright-cyan NDC,
bright-green NHNC, bright-magenta NOSO.

### crates/k-shell/src/proc.rs

Added dispatch routing for:
- `"graph topo36"` / `"gtopo36"` / `"neighborhood decic"` / `"gndc"`
- `"neighborhood nonic edge"` / `"gnhnc"`
- `"neighborhood octic sombor"` / `"gnoso"`
- `"gndcnhncnoso"`

### host-tests/gos-graph-topo36-harness/

New 10-test harness (VectorAddress L4=123, plugin TOPIX_36, executor t36.exec).
All 10 tests pass.

## Mathematical Definitions

**NDC(G) = Σ_v S(v)^10** (S-Decic vertex sum)

Extends the S-power-vertex series:
NM₁=Σ S² → NF=Σ S³ → NVQ=Σ S⁴ → NPS=Σ S⁵ → NSH=Σ S⁶ → NSHP=Σ S⁷
→ NOC=Σ S⁸ → NNC=Σ S⁹ → **NDC=Σ S¹⁰** (topo36)

- NDC = n·S^10 for S-regular
- Overflow: S^10 ≤ 16129^10 ≈ 2.6×10^41 > u128::MAX → saturating arithmetic

**NHNC(G) = Σ_{uv∈E} (S_u+S_v)^9** (S-Nonic edge-sum)

Extends the S-power-edge series:
NHM₁=Σ(S+S)² → NHCS → NHQS → NHPS → NHSE → NHHS → NHOC=Σ(S+S)^8
→ **NHNC=Σ(S+S)^9** (topo36)

- NHNC = |E|·(2S)^9 = 512|E|·S^9 for S-regular
- Overflow per edge: (2×16129)^9 ≈ 3.5×10^40 > u128::MAX → saturating

**NOSO(G) = Σ_{uv∈E} (S_u²+S_v²)^4** (S-Octic Sombor, α=8)

Generalised Sombor SO^α applied to S-values with α=8 (exact integer, no isqrt):
NSO(α=1) → NCSO(α=3) → NFSO(α=4) → NHSO(α=6) → **NOSO(α=8)** (topo36)

- NOSO = |E|·(2S²)^4 = 16|E|·S^8 for S-regular
- Per-edge max: (2×16129²)^4 ≈ 7.3×10^34 < u128::MAX ✓

## Cross-Check Table

| Graph    | NDC             | NHNC               | NOSO          | edges | nodes |
|----------|-----------------|--------------------|---------------|-------|-------|
| K₂       | 2               | 512                | 16            | 1     | 2     |
| P₃       | 3_072           | 524_288            | 8_192         | 2     | 3     |
| K₃       | 3_145_728       | 402_653_184        | 3_145_728     | 3     | 3     |
| K_{1,4}  | 5_242_880       | 536_870_912        | 4_194_304     | 4     | 5     |
| P₄       | 120_146         | 13_983_946         | 162_098       | 3     | 4     |
| K₄       | 13_947_137_604  | 1_190_155_742_208  | 4_132_485_216 | 6     | 4     |
| K_{2,3}  | 302_330_880     | 30_958_682_112     | 161_243_136   | 6     | 5     |

## Test Results

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_single_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

## VectorAddress L4 Namespace (updated)

88=graph-topo through 122=graph-topo35, **123=graph-topo36**

## Host Test Suite Total

**1443 tests** (was 1433 through V3.46; +10 from gos-graph-topo36-harness)
