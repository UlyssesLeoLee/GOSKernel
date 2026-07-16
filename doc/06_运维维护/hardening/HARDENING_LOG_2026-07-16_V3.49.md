# HARDENING LOG — V3.49 · 2026-07-16

## Summary

Added **NDoC + NHUC + NDSO** Neighborhood S-variant topological indices
(`graph_topo_indices38`) with full harness coverage (10 tests, 0 failures).

## New Topology Indices

### NDoC(G) = Σ_v S(v)¹²  — S-Dodecic Vertex Sum
- Exact u64 (u128 accumulator, saturating, clamped)
- Extends vertex power series: NM₁=Σ S² … NUC=Σ S¹¹ (topo37) → NDoC=Σ S¹² (topo38)
- S-regular: NDoC = n·S¹²

### NHUC(G) = Σ_{uv∈E} (S_u+S_v)¹¹  — S-Undecic Edge-Sum
- Exact u64 (u128 accumulator, saturating, clamped)
- Extends edge power series: NHM1=Σ(S+S)² … NHDC=Σ(S+S)¹⁰ (topo37) → NHUC=Σ(S+S)¹¹
- S-regular: NHUC = 2048·|E|·S¹¹

### NDSO(G) = Σ_{uv∈E} (S_u²+S_v²)⁶  — S-Duodecic Sombor (α=12)
- Exact u64, no isqrt (even power)
- Generalised Sombor SO^α series: NSO(α=1) … NTSO(α=10,topo37) → NDSO(α=12,topo38)
- S-regular: NDSO = 64·|E|·S¹²

## Cross-Check Table

| Graph     | NDoC              | NHUC                | NDSO               | edges | nodes |
|-----------|-------------------|---------------------|--------------------|-------|-------|
| Empty     | 0                 | 0                   | 0                  | 0     | 0     |
| K₂        | 2                 | 2_048               | 64                 | 1     | 2     |
| P₃        | 12_288            | 8_388_608           | 524_288            | 2     | 3     |
| K₃        | 50_331_648        | 25_769_803_776      | 3_221_225_472      | 3     | 3     |
| K_{1,4}   | 83_886_080        | 34_359_738_368      | 4_294_967_296      | 4     | 5     |
| P₄        | 1_071_074         | 460_453_306         | 43_665_842         | 3     | 4     |
| K₄        | 1_129_718_145_924 | 385_610_460_475_392 | 108_452_942_008_704| 6     | 4     |
| K_{2,3}   | 10_883_911_680    | 4_458_050_224_128   | 835_884_417_024    | 6     | 5     |

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | `graph_topo_indices38_inner` + `graph_topo_indices38` |
| `crates/k-shell/src/lib.rs` | `dispatch_graph_topo_indices38` |
| `crates/k-shell/src/proc.rs` | routing: `graph topo38 / gtopo38 / gndoc / gnhuc / gndso / gndocnhucndso` |
| `host-tests/gos-graph-topo38-harness/` | new harness: Cargo.toml + .cargo/config.toml + tests/graph_topo38.rs |

## Test Results

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed
```

## Metrics

- Host test suite total: **1463 tests** (+10 from topo38-harness)
- VectorAddress L4 namespace: 125 = graph-topo38
- Plugin: TOPIX_38 / Executor: t38.exec
- Shell aliases: `graph topo38`, `gtopo38`, `gndoc`, `gnhuc`, `gndso`, `gndocnhucndso`
