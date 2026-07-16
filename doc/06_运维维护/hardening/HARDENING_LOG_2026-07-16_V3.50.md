# HARDENING LOG — V3.50 (2026-07-16)

## Summary

Added three new S-variant Neighborhood topological indices — NTC, NHDOC, NESO — plus the
`gos-graph-topo39-harness` (10 tests). Host-test suite now totals **1473 tests**.

## New indices: NTC + NHDOC + NESO (S-variant family, topo39)

### Mathematical definitions

Let S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant").

| Index | Formula | Type | Series |
|-------|---------|------|--------|
| NTC | Σ_v S(v)^13 | S-Tridecic vertex sum | extends NDoC=Σ S¹² (topo38) |
| NHDOC | Σ_{uv∈E} (S_u+S_v)^12 | S-Dodecic edge-sum | extends NHUC=Σ(S+S)¹¹ (topo38) |
| NESO | Σ_{uv∈E} (S_u²+S_v²)^7 | S-Tetradecic Sombor α=14 | extends NDSO=Σ(S²+S²)⁶ (topo38) |

### S-regular formulas

- NTC   = n·S^13                       (for S-regular)
- NHDOC = |E|·(2S)^12 = 4096|E|·S^12  (for S-regular)
- NESO  = |E|·(2S²)^7 = 128|E|·S^14   (for S-regular)

### Cross-check table

| Graph | NTC | NHDOC | NESO | edges | nodes |
|-------|-----|-------|------|-------|-------|
| K₂ | 2 | 4_096 | 128 | 1 | 2 |
| P₃ | 24_576 | 33_554_432 | 4_194_304 | 2 | 3 |
| K₃ | 201_326_592 | 206_158_430_208 | 103_079_215_104 | 3 | 3 |
| K_{1,4} | 335_544_320 | 274_877_906_944 | 137_438_953_472 | 4 | 5 |
| P₄ | 3_205_030 | 2_665_063_586 | 737_717_066 | 3 | 4 |
| K₄ | 10_167_463_313_316 | 6_940_988_288_557_056 | 17_569_376_605_410_048 | 6 | 4 |
| K_{2,3} | 65_303_470_080 | 53_496_602_689_536 | 60_183_678_025_728 | 6 | 5 |

### Implementation notes

- All three use u128 accumulators with saturating ops; NO isqrt anywhere (all exact integer)
- NTC: s^13 = s^8 × s^4 × s (all saturating_mul)
- NHDOC: ss^12 = ss^8 × ss^4 (all saturating_mul)
- NESO: s2s^7 = s2s^4 × s2s^2 × s2s (all saturating_mul)
- NESO is exact integer because (S_u²+S_v²)^7 has no fractional power
- VectorAddress L4=126 for gos-graph-topo39-harness; plugin TOPIX_39; executor t39.exec

### Shell commands

```
graph topo39 | gtopo39
neighborhood tridecic | gntc
neighborhood dodecic edge | gnhdoc
neighborhood tetradecic sombor | gneso
gntcnhdocneso
```

## Files changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices39_inner()` method + `graph_topo_indices39()` public fn |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices39()` |
| `crates/k-shell/src/proc.rs` | Added topo39 routing branch |
| `host-tests/gos-graph-topo39-harness/` | New harness (Cargo.toml, .cargo/config.toml, tests/graph_topo39.rs) |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.50.md` | This file |

## Test results

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## Cumulative state

- **Version**: V3.50
- **Branch**: feat/vk-auto-live-surface
- **Host-test suite total**: 1473 tests (1463 through V3.49 + 10 new)
- **VectorAddress L4 namespace**: 88=graph-topo through 126=graph-topo39
- **S-variant power-vertex series**: NM₁(2)→NF(3)→NVQ(4)→NPS(5)→NSH(6)→NSHP(7)→NOC(8)→NNC(9)→NDC(10)→NUC(11)→NDoC(12)→NTC(13)
- **S-variant power-edge series**: NHM1(2)→NHCS(3)→NHQS(4)→NHPS(5)→NHSE(6)→NHHS(7)→NHOC(8)→NHNC(9)→NHDC(10)→NHUC(11)→NHDOC(12)
- **S-variant Sombor α-series**: NSO(1)→NCSO(3)→NFSO(4)→NHSO(6)→NOSO(8)→NTSO(10)→NDSO(12)→NESO(14)
