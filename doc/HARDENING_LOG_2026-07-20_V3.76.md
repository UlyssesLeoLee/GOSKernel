# HARDENING LOG — V3.76 — 2026-07-20

## Summary

Added NNONATRIACTC + NHNONATRIACTC + NAHSO Neighborhood S-variant topological indices (topo65),
continuing the S-power-vertex/edge series and the 3rd-pass double-letter Sombor SO^α family.

## New Function

`gos_runtime::graph_topo_indices65() -> (nnonatriactc: u64, nhnonatriactc: u64, nahso: u64, edge_count: usize, node_count: usize)`

## Index Definitions

- **NNONATRIACTC(G)** = Σ_v S(v)^39  (S-Nonatriacontic vertex sum; u128→u64 saturating)
- **NHNONATRIACTC(G)** = Σ_{uv∈E} (S_u+S_v)^38  (S-Octatriacontic edge-sum; u128→u64 saturating)
- **NAHSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^33  (S-Hexahexacontyl Sombor SO^α, α=66; u128→u64 saturating; no isqrt)

where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum (S-variant).

## Series Position

- NNONATRIACTC extends NOCTATRIACTC=ΣS^38 (topo64) to 39th power
- NHNONATRIACTC extends NHOCTATRIACTC=Σ(S+S)^37 (topo64) to 38th power
- NAHSO = S-variant generalised Sombor SO^α with α=66: NAGSO(α=64,topo64)→NAHSO(α=66,topo65)

## Implementation Details

- s^39 = s32 × s4 × s2 × s  (39 = 32+4+2+1)
- ss^38 = ss32 × ss4 × ss2  (38 = 32+4+2)
- s2s^33 = s2s32 × s2s      (33 = 32+1; minimal multiplicative depth)

## S-Regular Formulas

- NNONATRIACTC = n·S^39
- NHNONATRIACTC = |E|·(2S)^38 = 274_877_906_944|E|·S^38
- NAHSO = |E|·(2S²)^33 = 8_589_934_592|E|·S^66

## Test Values

| Graph    | NNONATRIACTC            | NHNONATRIACTC  | NAHSO          | edges | nodes |
|----------|-------------------------|----------------|----------------|-------|-------|
| Empty    | 0                       | 0              | 0              | 0     | 0     |
| 1 node   | 0                       | 0              | 0              | 0     | 1     |
| K₂       | 2                       | 274_877_906_944| 8_589_934_592  | 1     | 2     |
| P₃       | 1_649_267_441_664       | u64::MAX(sat.) | u64::MAX(sat.) | 2     | 3     |
| K₃       | u64::MAX(sat.)          | u64::MAX(sat.) | u64::MAX(sat.) | 3     | 3     |
| K_{1,4}  | u64::MAX(sat.)          | u64::MAX(sat.) | u64::MAX(sat.) | 4     | 5     |
| P₄       | 8_105_111_405_549_580_310 | u64::MAX(sat.)| u64::MAX(sat.) | 3     | 4     |
| K₄       | u64::MAX(sat.)          | u64::MAX(sat.) | u64::MAX(sat.) | 6     | 4     |
| 2 iso    | 0                       | 0              | 0              | 0     | 2     |
| K_{2,3}  | u64::MAX(sat.)          | u64::MAX(sat.) | u64::MAX(sat.) | 6     | 5     |

## Key Derivations

- K₂ (S=1): NNONATRIACTC=2; NHNONATRIACTC=2^38=274_877_906_944; NAHSO=2^33=8_589_934_592
- P₃ (S=2 uniform): NNONATRIACTC=3×2^39=1_649_267_441_664; NHNONATRIACTC saturates (4^38=2^76>u64::MAX per-edge)
- P₄ (S∈{2,3}): NNONATRIACTC=2×2^39+2×3^39; 3^39=4_052_555_153_018_976_267; total=8_105_111_405_549_580_310

## VectorAddress

- L4=152 for gos-graph-topo65-harness
- 88=graph-topo through 151=graph-topo64, **152=graph-topo65**

## Shell Commands

`"graph topo65"` / `"gtopo65"` / `"gnnnonatriactc"` / `"gnnhnonatriactc"` / `"gnnahso"` / `"gnnnonatriactcnhnonatriactcnahso"`

## Plugin / Executor

- PluginId: `TOPIX_65`
- ExecutorId: `t65.exec`

## Files Changed

- `crates/gos-runtime/src/lib.rs` — added `graph_topo_indices65_inner` + `graph_topo_indices65` public fn
- `crates/k-shell/src/lib.rs` — added `dispatch_graph_topo_indices65`
- `crates/k-shell/src/proc.rs` — added routing for topo65 commands
- `host-tests/gos-graph-topo65-harness/` — new test harness (10 tests, all green)

## Test Results

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_k2_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Host Test Suite Total

**1733 tests** (1723 prior + 10 new)
