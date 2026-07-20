# HARDENING LOG — V3.86 (2026-07-20)

## Summary

V3.86 adds **NNONATETRAACTC + NHNONATETRAACTC + NARSO** — three new Neighborhood
S-variant topological indices — as `gos_runtime::graph_topo_indices75()`, continuing
the S-variant high-power series. Includes `gos-graph-topo75-harness` (10 new tests).

**Host-test suite total: 1833 tests** (10 added, all passing).

---

## New Topological Indices (topo75)

All three indices use `S(v) = Σ_{w∈N(v)} deg(w)` — the neighbor-degree sum —
consistent with topo18/topo21–topo75 family.

### NNONATETRAACTC — S-Nonatetracontic Vertex Sum

```
NNONATETRAACTC(G) = Σ_v S(v)^49    (u128→u64 saturating)
```

- Extends NOCTOTETRAACTC = ΣS⁴⁸ (topo74) to the 49th power
- S-regular formula: `NNONATETRAACTC = n·S^49`
- Implementation: `s^49 = s32 × s16 × s` (49=32+16+1; 3 mults — efficient)

### NHNONATETRAACTC — S-Octotetracontic Edge Sum

```
NHNONATETRAACTC(G) = Σ_{uv∈E} (S_u + S_v)^48    (u128→u64 saturating)
```

- Extends NHOCTOTETRAACTC = Σ(S+S)⁴⁷ (topo74) to the 48th power
- S-regular formula: `NHNONATETRAACTC = 281_474_976_710_656 · |E| · S^48`
  (coefficient = 2^48)
- Implementation: `ss^48 = ss32 × ss16` (48=32+16; 2 mults — very efficient!)

### NARSO — S-Hexaoctacontyl Sombor (α=86)

```
NARSO(G) = Σ_{uv∈E} (S_u² + S_v²)^43    (u128→u64 saturating)
```

- Generalised Sombor SO^α with α=86 on S-variant; 3rd-pass double-letter "AR"
- Sequence: … → NAQSO(α=84, topo74) → NARSO(α=86, topo75)
- S-regular formula: `NARSO = 8_796_093_022_208 · |E| · S^86` (coefficient = 2^43)
- Implementation: `s2s^43 = s2s32 × s2s8 × s2s2 × s2s` (43=32+8+2+1; 4 mults)

---

## VectorAddress L4 Namespace

| L4 value | Harness |
|----------|---------|
| 88–161   | graph-topo through graph-topo74 |
| **162**  | **graph-topo75** (this version) |

---

## Key Values

| Graph    | NNONATETRAACTC      | NHNONATETRAACTC        | NARSO              | edges | nodes |
|----------|---------------------|------------------------|--------------------|-------|-------|
| Empty    | 0                   | 0                      | 0                  | 0     | 0     |
| 1 node   | 0                   | 0                      | 0                  | 0     | 1     |
| K₂       | 2                   | 281_474_976_710_656     | 8_796_093_022_208  | 1     | 2     |
| P₃       | 1_688_849_860_263_936 | u64::MAX (sat.)       | u64::MAX (sat.)    | 2     | 3     |
| K₃       | u64::MAX (sat.)     | u64::MAX (sat.)        | u64::MAX (sat.)    | 3     | 3     |
| K_{1,4}  | u64::MAX (sat.)     | u64::MAX (sat.)        | u64::MAX (sat.)    | 4     | 5     |
| P₄       | u64::MAX (sat.)     | u64::MAX (sat.)        | u64::MAX (sat.)    | 3     | 4     |
| K₄       | u64::MAX (sat.)     | u64::MAX (sat.)        | u64::MAX (sat.)    | 6     | 4     |
| K_{2,3}  | u64::MAX (sat.)     | u64::MAX (sat.)        | u64::MAX (sat.)    | 6     | 5     |

**P₃ NNONATETRAACTC derivation:** 3 × 2^49 = 3 × 562_949_953_421_312 = 1_688_849_860_263_936 (fits u64 exactly).

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | +`graph_topo_indices75_inner()`, +`graph_topo_indices75()` |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_topo_indices75()` |
| `crates/k-shell/src/proc.rs` | +k-shell dispatch entry for topo75 |
| `host-tests/gos-graph-topo75-harness/` | New harness (10 tests, all pass) |

---

## k-shell Commands

```
graph topo75           gtopo75
neighborhood nonatetracontic          gnnnonatetraactc
neighborhood octotetracontic edge     gnnhnonatetraactc
neighborhood hexaoctacontyl sombor    gnnarso
gnnnonatetraactcnhnonatetraactcnarso
```

---

## Test Results

```
running 10 tests
test test_01_empty       ... ok
test test_02_single_node ... ok
test test_03_k2_edge     ... ok
test test_04_path_p3     ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14    ... ok
test test_07_path_p4     ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

`cargo check -p gos-kernel`: clean (no errors or warnings).
