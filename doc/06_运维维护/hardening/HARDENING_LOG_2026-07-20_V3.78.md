# HARDENING LOG — V3.78 (2026-07-20)

## Summary

Added NHENTETRAACTC + NHHENTETRAACTC + NAJSO Neighborhood S-variant topological indices (topo67),
and `gos-graph-topo67-harness` with 10 tests.

**Host-test suite total: 1753 tests** (1743 through V3.77 + 10 new).

---

## What Changed

### New API: `gos_runtime::graph_topo_indices67()`

```rust
pub fn graph_topo_indices67() -> (u64, u64, u64, usize, usize)
// returns (nhentetraactc, nhhentetraactc, najso, edge_count, node_count)
```

All three indices are based on the S-variant neighbor-degree sum:
`S(v) = Σ_{w∈N(v)} deg(w)`.

#### NHENTETRAACTC — S-Hentetracontic vertex sum (power=41)

```
NHENTETRAACTC(G) = Σ_v S(v)^41
```

- Extends the S-power-vertex series: `NTETRAACTC=ΣS^40` (topo66) → `NHENTETRAACTC=ΣS^41` (topo67)
- S-regular formula: `NHENTETRAACTC = n·S^41`
- Implementation: `s^41 = s32 × s8 × s` (41 = 32+8+1)

#### NHHENTETRAACTC — S-Tetracontic edge-sum (power=40)

```
NHHENTETRAACTC(G) = Σ_{uv∈E} (S_u + S_v)^40
```

- Extends: `NHTETRAACTC=Σ(S+S)^39` (topo66) → `NHHENTETRAACTC=Σ(S+S)^40` (topo67)
- S-regular formula: `NHHENTETRAACTC = |E|·(2S)^40 = 1_099_511_627_776·|E|·S^40`
- Implementation: `ss^40 = ss32 × ss8` (40 = 32+8; highly efficient — only 2 squarings sum)

#### NAJSO — S-Tetracontyl Sombor (α=70)

```
NAJSO(G) = Σ_{uv∈E} (S_u² + S_v²)^35
```

- 3rd-pass double-letter series: NAISO(α=68, topo66) → **NAJSO(α=70)**
- Exact integer computation (no isqrt needed): α=70 is even, j=35
- S-regular formula: `NAJSO = |E|·(2S²)^35 = 34_359_738_368·|E|·S^70`
- Implementation: `s2s^35 = s2s32 × s2s2 × s2s` (35 = 32+2+1)

---

## Test Values

| Graph      | NHENTETRAACTC        | NHHENTETRAACTC       | NAJSO            | edges | nodes |
|-----------|----------------------|----------------------|------------------|-------|-------|
| Empty      | 0                    | 0                    | 0                | 0     | 0     |
| 1 node     | 0                    | 0                    | 0                | 0     | 1     |
| K₂         | 2                    | 1_099_511_627_776    | 34_359_738_368   | 1     | 2     |
| P₃         | 6_597_069_766_656    | u64::MAX             | u64::MAX         | 2     | 3     |
| K₃         | u64::MAX             | u64::MAX             | u64::MAX         | 3     | 3     |
| K_{1,4}    | u64::MAX             | u64::MAX             | u64::MAX         | 4     | 5     |
| P₄         | u64::MAX             | u64::MAX             | u64::MAX         | 3     | 4     |
| K₄         | u64::MAX             | u64::MAX             | u64::MAX         | 6     | 4     |
| 2 isolated | 0                    | 0                    | 0                | 0     | 2     |
| K_{2,3}    | u64::MAX             | u64::MAX             | u64::MAX         | 6     | 5     |

Key derivations:
- K₂ (S=1): NHENTETRAACTC=2; NHHENTETRAACTC=2^40=1_099_511_627_776; NAJSO=2^35=34_359_738_368
- P₃ (S=2 uniform): NHENTETRAACTC=3×2^41=6_597_069_766_656; others saturate (4^40=2^80>u64::MAX per-edge)
- P₄ continues to saturate all three indices (same as topo66 onward)

---

## New Harness

**`host-tests/gos-graph-topo67-harness/`** (10 tests)

- Plugin: `TOPIX_67` / Executor: `t67.exec`
- VectorAddress L4=154
- All 10 tests: PASS

```
test test_01_empty          ... ok
test test_02_single_node    ... ok
test test_03_k2_edge        ... ok
test test_04_path_p3        ... ok
test test_05_triangle_k3    ... ok
test test_06_star_k14       ... ok
test test_07_path_p4        ... ok
test test_08_complete_k4    ... ok
test test_09_two_isolated   ... ok
test test_10_k23_bipartite  ... ok

test result: ok. 10 passed; 0 failed
```

---

## Shell Commands

```
graph topo67 / gtopo67 / gnhentetraactc / gnhhentetraactc / gnnajso
gnhentetraactcnhhentetraactcnajso
```

---

## VectorAddress L4 Namespace Update

88=graph-topo through 153=graph-topo66, **154=graph-topo67**

---

## Implementation Notes

- `ss^40 = ss32 × ss8`: highly efficient (40 = 32+8, two powers of 2 — just 1 final multiply after squarings)
- `s^41 = s32 × s8 × s`: 41 = 32+8+1 (3-term decomposition)
- `s2s^35 = s2s32 × s2s2 × s2s`: 35 = 32+2+1 (3-term decomposition)
- All computations use u128 saturating arithmetic, clamped to u64::MAX at the end
- Note: `NHHENTETRAACTC` at power 40 is particularly efficient (40=32+8, both powers of 2)
