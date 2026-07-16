# HARDENING LOG — V3.48 — 2026-07-16

## Summary

Added three new Neighborhood S-variant topological indices extending the S-power series to
the 11th degree and the generalised Sombor series to α=10, completing topo37:

- **NUC(G)** = Σ_v S(v)^11 — S-Undecic vertex sum (11th power)
- **NHDC(G)** = Σ_{uv∈E} (S_u+S_v)^10 — S-Decic edge-sum (10th power)
- **NTSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^5 — S-Tenth Sombor (SO^α with α=10; exact integer)

All three use S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum), the S-variant of degree.

## Changes

### crates/gos-runtime/src/lib.rs

Added `graph_topo_indices37_inner()` (Runtime impl) and public `graph_topo_indices37()`:

```
pub fn graph_topo_indices37() -> (u64, u64, u64, usize, usize)
  Returns (nuc, nhdc, ntso, edge_count, node_count)
```

Algorithm: O(V+E) — degree pass → S(v) pass → vertex scan (NUC) + edge scan (NHDC, NTSO).
All three use u128 accumulators with saturating_mul/add; no BFS; no isqrt.

### crates/k-shell/src/lib.rs

Added `dispatch_graph_topo_indices37()` — bright-yellow header, bright-cyan NUC,
bright-green NHDC, bright-magenta NTSO.

### crates/k-shell/src/proc.rs

Added dispatch routing for:
- `"graph topo37"` / `"gtopo37"` / `"neighborhood undecic"` / `"gnuc"`
- `"neighborhood decic edge"` / `"gnhdc"`
- `"neighborhood tenth sombor"` / `"gntso"`
- `"gnucnhdcntso"`

### host-tests/gos-graph-topo37-harness/

New 10-test harness (VectorAddress L4=124, plugin TOPIX_37, executor t37.exec).
All 10 tests pass.

## Mathematical Definitions

**NUC(G) = Σ_v S(v)^11** (S-Undecic vertex sum)

Extends the S-power-vertex series:
NM₁=Σ S² → NF=Σ S³ → NVQ=Σ S⁴ → NPS=Σ S⁵ → NSH=Σ S⁶ → NSHP=Σ S⁷
→ NOC=Σ S⁸ → NNC=Σ S⁹ → NDC=Σ S¹⁰ → **NUC=Σ S¹¹** (topo37)

- NUC = n·S^11 for S-regular
- Overflow: S^11 ≤ 16129^11 ≈ 4.2×10^45 > u128::MAX → saturating arithmetic

**NHDC(G) = Σ_{uv∈E} (S_u+S_v)^10** (S-Decic edge-sum)

Extends the S-power-edge series:
NHM₁=Σ(S+S)² → NHCS → NHQS → NHPS → NHSE → NHHS → NHOC=Σ(S+S)^8
→ NHNC=Σ(S+S)^9 → **NHDC=Σ(S+S)^10** (topo37)

- NHDC = |E|·(2S)^10 = 1024|E|·S^10 for S-regular
- Overflow per edge: (2×16129)^10 ≈ 5.6×10^44 > u128::MAX → saturating

**NTSO(G) = Σ_{uv∈E} (S_u²+S_v²)^5** (S-Tenth Sombor, α=10)

Generalised Sombor SO^α applied to S-values with α=10 (exact integer, no isqrt):
NSO(α=1) → NCSO(α=3) → NFSO(α=4) → NHSO(α=6) → NOSO(α=8) → **NTSO(α=10)** (topo37)

- NTSO = |E|·(2S²)^5 = 32|E|·S^10 for S-regular
- Per-edge overflow: (2×16129²)^5 ≈ 3.8×10^43 > u128::MAX → saturating arithmetic

## Cross-Check Table

| Graph    | NUC              | NHDC                | NTSO            | edges | nodes |
|----------|------------------|---------------------|-----------------|-------|-------|
| K₂       | 2                | 1_024               | 32              | 1     | 2     |
| P₃       | 6_144            | 2_097_152           | 65_536          | 2     | 3     |
| K₃       | 12_582_912       | 3_221_225_472       | 100_663_296     | 3     | 3     |
| K_{1,4}  | 20_971_520       | 4_294_967_296       | 134_217_728     | 4     | 5     |
| P₄       | 358_390          | 79_997_426          | 2_632_154       | 3     | 4     |
| K₄       | 125_524_238_436  | 21_422_803_359_744  | 669_462_604_992 | 6     | 4     |
| K_{2,3}  | 1_813_985_280    | 371_504_185_344     | 11_609_505_792  | 6     | 5     |

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

88=graph-topo through 123=graph-topo36, **124=graph-topo37**

## Host Test Suite Total

**1453 tests** (was 1443 through V3.47; +10 from gos-graph-topo37-harness)
