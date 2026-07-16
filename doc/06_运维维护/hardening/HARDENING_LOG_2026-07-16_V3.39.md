# HARDENING LOG — V3.39 — 2026-07-16

## Summary

Added **NNI + NNMI + NSM1** Neighborhood Nirmala S-variant topological indices as `gos-graph-topo28-harness` (L4=115). This continues the S-variant family expansion building on V3.29–V3.38 Neighborhood S-index series.

## Version

- **Version**: V3.39
- **Branch**: feat/vk-auto-live-surface
- **Date**: 2026-07-16
- **Prior test count**: 1353 (V3.38)
- **New test count**: 1363 (+10)

## New Indices: NNI + NNMI + NSM1 (gos-graph-topo28)

Function signature: `gos_runtime::graph_topo_indices28() -> (nni_ppm: u64, nnmi_ppm: u64, nsm1: u64, edge_count: usize, node_count: usize)`

S(v) = Σ_{w∈N(v)} deg(w) = neighbor-degree sum (same S as topo18/topo21–topo28 family)

- **nni_ppm**  = NNI(G) × 10^6  = Σ_{uv∈E} isqrt64((S_u+S_v)×10^12)              (floor ppm; S-Nirmala)
- **nnmi_ppm** = NNMI(G) × 10^6 = Σ_{uv∈E} (S_u+S_v)×isqrt64((S_u+S_v)×10^12)   (floor ppm; S-Modified Nirmala)
- **nsm1**     = NSM1(G)         = Σ_{uv∈E} (S_u+S_v)                             (exact u64; S-edge M₁ = Σ_v S(v)·deg(v))

### Definitions

- NNI(G)  = Σ_{uv∈E} √(S_u+S_v)            — S-analogue of Nirmala N (Nirmala, Mathad & Usha 2021)
- NNMI(G) = Σ_{uv∈E} (S_u+S_v)^{3/2}       — S-analogue of Modified Nirmala N* (Kumar et al. 2022)
- NSM1(G) = Σ_{uv∈E} (S_u+S_v)             — S-analogue of M₁ edge form (= Σ_v S(v)·deg(v))

### Key Identity

NNMI per edge = (S_u+S_v) × NNI per edge  
because floor((S_u+S_v)^{3/2}×10^6) = (S_u+S_v)×floor(√(S_u+S_v)×10^6) when (S_u+S_v)∈ℤ.  
This means NNMI shares the isqrt64 computation with NNI — a single `nni_e` value serves both.

### Key Invariants

- NNI  = |E|·√(2S)·10^6 for S-regular graphs (all S equal)
- NNMI = |E|·(2S)^{3/2}·10^6 for S-regular graphs
- NSM1 = 2|E|·S for S-regular graphs
- K₃ and K_{1,4}: both S-uniform S=4 → same per-edge NNI and NNMI; totals differ by edge count ratio
- All three indices are zero for edgeless graphs; NSM1 is exact (no approximation)

### Cross-Check Table

| Graph      | NNI (ppm)  | NNMI (ppm)  | NSM1 | edges | nodes |
|------------|------------|-------------|------|-------|-------|
| Empty      | 0          | 0           | 0    | 0     | 0     |
| 1 node     | 0          | 0           | 0    | 0     | 1     |
| K₂         | 1_414_213  | 2_828_426   | 2    | 1     | 2     |
| P₃         | 4_000_000  | 16_000_000  | 8    | 2     | 3     |
| K₃         | 8_485_281  | 67_882_248  | 24   | 3     | 3     |
| K_{1,4}    | 11_313_708 | 90_509_664  | 32   | 4     | 5     |
| P₄         | 6_921_623  | 37_057_604  | 16   | 3     | 4     |
| K₄         | 25_455_840 | 458_205_120 | 108  | 6     | 4     |
| 2 isolated | 0          | 0           | 0    | 0     | 2     |
| K_{2,3}    | 20_784_606 | 249_415_272 | 72   | 6     | 5     |

### Algorithm

O(V+E) — adj+deg pass → S(v) pass → edge scan (a<b); isqrt64 only; no BFS, no u128.  
Single isqrt64 call per edge computes NNI_e; NNMI_e = ssum × NNI_e reuses it.

### Overflow Safety

- ssum×10^12: max ssum=32258 (K₁₂₈); 32258×10^12 = 3.23×10^16 < u64::MAX ✓
- NNMI accumulator: ≤8128 edges × 32258 × 179_606_381 ≈ 4.71×10^16 < u64::MAX ✓
- NSM1: ≤8128 × 32258 ≈ 2.62×10^8 << u64::MAX ✓

### Shell Commands

"graph topo28" / "gtopo28" / "neighborhood nirmala" / "gnni" / "neighborhood modified nirmala" / "gnnmi" / "neighborhood sm1" / "gnsm1" / "gnninnminsm1"

### VectorAddress L4

L4=115 for gos-graph-topo28-harness

### References

Nirmala, Mathad & Usha 2021 (Nirmala index N)  
Kumar et al. 2022 (Modified Nirmala N*)  
(S-variant family)

## Files Changed

- `crates/gos-runtime/src/lib.rs`: added `graph_topo_indices28_inner()` + `graph_topo_indices28()` public function
- `host-tests/gos-graph-topo28-harness/Cargo.toml`: new workspace
- `host-tests/gos-graph-topo28-harness/.cargo/config.toml`: host target override
- `host-tests/gos-graph-topo28-harness/tests/graph_topo28.rs`: 10 tests (all passing)

## Test Results

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
