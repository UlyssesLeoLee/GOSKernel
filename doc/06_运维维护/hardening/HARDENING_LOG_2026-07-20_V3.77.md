# HARDENING LOG — V3.77 (2026-07-20)

## Summary

Added NTETRAACTC + NHTETRAACTC + NAISO Neighborhood S-variant topological indices (topo66),
and `gos-graph-topo66-harness` with 10 tests.

**Host-test suite total: 1743 tests** (1733 through V3.76 + 10 new).

---

## What Changed

### New API: `gos_runtime::graph_topo_indices66()`

```rust
pub fn graph_topo_indices66() -> (u64, u64, u64, usize, usize)
// returns (ntetraactc, nhtetraactc, naiso, edge_count, node_count)
```

All three indices are based on the S-variant neighbor-degree sum:
`S(v) = Σ_{w∈N(v)} deg(w)`.

#### NTETRAACTC — S-Tetracontic vertex sum (α=40)

```
NTETRAACTC(G) = Σ_v S(v)^40
```

- Extends the S-power-vertex series: `NNONATRIACTC=ΣS^39` (topo65) → `NTETRAACTC=ΣS^40` (topo66)
- S-regular formula: `NTETRAACTC = n·S^40`
- Implementation: `s^40 = s32 × s8` (40 = 32+8; highly efficient — only 1 extra multiply after s32)

#### NHTETRAACTC — S-Nonatriacontic edge-sum (power=39)

```
NHTETRAACTC(G) = Σ_{uv∈E} (S_u + S_v)^39
```

- Extends: `NHNONATRIACTC=Σ(S+S)^38` (topo65) → `NHTETRAACTC=Σ(S+S)^39` (topo66)
- S-regular formula: `NHTETRAACTC = |E|·(2S)^39 = 549_755_813_888·|E|·S^39`
- Implementation: `ss^39 = ss32 × ss4 × ss2 × ss` (39 = 32+4+2+1)

#### NAISO — S-Octahexacontyl Sombor (α=68)

```
NAISO(G) = Σ_{uv∈E} (S_u² + S_v²)^34
```

- 3rd-pass double-letter series: NAASO(α=52)…NAHSO(α=66) → **NAISO(α=68)**
- Exact integer computation (no isqrt needed): α=68 is even, j=34
- S-regular formula: `NAISO = |E|·(2S²)^34 = 17_179_869_184·|E|·S^68`
- Implementation: `s2s^34 = s2s32 × s2s2` (34 = 32+2; highly efficient)

---

## Test Values

| Graph      | NTETRAACTC           | NHTETRAACTC      | NAISO            | edges | nodes |
|-----------|----------------------|------------------|------------------|-------|-------|
| Empty      | 0                    | 0                | 0                | 0     | 0     |
| 1 node     | 0                    | 0                | 0                | 0     | 1     |
| K₂         | 2                    | 549_755_813_888  | 17_179_869_184   | 1     | 2     |
| P₃         | 3_298_534_883_328    | u64::MAX         | u64::MAX         | 2     | 3     |
| K₃         | u64::MAX             | u64::MAX         | u64::MAX         | 3     | 3     |
| K_{1,4}    | u64::MAX             | u64::MAX         | u64::MAX         | 4     | 5     |
| P₄         | u64::MAX             | u64::MAX         | u64::MAX         | 3     | 4     |
| K₄         | u64::MAX             | u64::MAX         | u64::MAX         | 6     | 4     |
| 2 isolated | 0                    | 0                | 0                | 0     | 2     |
| K_{2,3}    | u64::MAX             | u64::MAX         | u64::MAX         | 6     | 5     |

Note: P₄ saturation is new at topo66. At topo65, NNONATRIACTC for P₄ was
`8_105_111_405_549_580_310` (fits in u64). At topo66, NTETRAACTC for P₄
requires `2×3^40 = 24_315_330_918_113_857_602 > u64::MAX`, so all three
indices saturate for P₄ onward.

---

## New Harness

**`host-tests/gos-graph-topo66-harness/`** (10 tests)

- Plugin: `TOPIX_66` / Executor: `t66.exec`
- VectorAddress L4=153
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
graph topo66 / gtopo66 / gntetraactc / gnhtetraactc / gnnaiso
gntetraactcnhtetraactcnaiso
```

---

## VectorAddress L4 Namespace Update

88=graph-topo through 152=graph-topo65, **153=graph-topo66**

---

## Implementation Notes

- `s^40 = s32 × s8`: highly efficient (40 = 32+8, only 2 squarings after s2)
- `ss^39 = ss32 × ss4 × ss2 × ss`: same 4-term decomposition as topo65's `ss^38` but with one extra `×ss`
- `s2s^34 = s2s32 × s2s2`: highly efficient (34 = 32+2, only 1 extra multiply)
- All computations use u128 saturating arithmetic, clamped to u64::MAX at the end
