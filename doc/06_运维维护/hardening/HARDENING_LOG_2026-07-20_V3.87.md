# HARDENING LOG — V3.87 (2026-07-20)

## Summary

V3.87 adds **NPENTAACTC + NHPENTAACTC + NASSO** — three new Neighborhood
S-variant topological indices — as `gos_runtime::graph_topo_indices76()`, opening
the pentacontic (50–59) power series. Includes `gos-graph-topo76-harness` (10 new tests).

**Host-test suite total: 1843 tests** (10 added, all passing).

---

## New Topological Indices (topo76)

All three indices use `S(v) = Σ_{w∈N(v)} deg(w)` — the neighbor-degree sum —
consistent with topo18/topo21–topo76 family.

### NPENTAACTC — S-Pentacontic Vertex Sum

```
NPENTAACTC(G) = Σ_v S(v)^50    (u128→u64 saturating)
```

- Extends NNONATETRAACTC = ΣS⁴⁹ (topo75) to the 50th power
- First index of the pentacontic (50–59) series
- S-regular formula: `NPENTAACTC = n·S^50`
- Implementation: `s^50 = s32 × s16 × s2` (50=32+16+2; 3 mults — efficient)

### NHPENTAACTC — S-Nonapentacontic Edge Sum

```
NHPENTAACTC(G) = Σ_{uv∈E} (S_u + S_v)^49    (u128→u64 saturating)
```

- Extends NHNONATETRAACTC = Σ(S+S)⁴⁸ (topo75) to the 49th power
- S-regular formula: `NHPENTAACTC = 562_949_953_421_312 · |E| · S^49`
  (coefficient = 2^49)
- Implementation: `ss^49 = ss32 × ss16 × ss` (49=32+16+1; 3 mults)

### NASSO — S-Variant Sombor (α=88)

```
NASSO(G) = Σ_{uv∈E} (S_u² + S_v²)^44    (u128→u64 saturating)
```

- Generalised Sombor SO^α with α=88 on S-variant; 3rd-pass double-letter "AS"
- Sequence: … → NARSO(α=86, topo75) → NASSO(α=88, topo76)
- S-regular formula: `NASSO = 17_592_186_044_416 · |E| · S^88` (coefficient = 2^44)
- Implementation: `s2s^44 = s2s32 × s2s8 × s2s4` (44=32+8+4; 3 mults — efficient!)

---

## VectorAddress L4 Namespace

| L4 value | Harness |
|----------|---------|
| 88–162   | graph-topo through graph-topo75 |
| **163**  | **graph-topo76** (this version) |

---

## Key Values

| Graph    | NPENTAACTC            | NHPENTAACTC           | NASSO               | edges | nodes |
|----------|-----------------------|-----------------------|---------------------|-------|-------|
| Empty    | 0                     | 0                     | 0                   | 0     | 0     |
| 1 node   | 0                     | 0                     | 0                   | 0     | 1     |
| K₂       | 2                     | 562_949_953_421_312   | 17_592_186_044_416  | 1     | 2     |
| P₃       | 3_377_699_720_527_872 | u64::MAX (sat.)       | u64::MAX (sat.)     | 2     | 3     |
| K₃       | u64::MAX (sat.)       | u64::MAX (sat.)       | u64::MAX (sat.)     | 3     | 3     |
| K_{1,4}  | u64::MAX (sat.)       | u64::MAX (sat.)       | u64::MAX (sat.)     | 4     | 5     |
| P₄       | u64::MAX (sat.)       | u64::MAX (sat.)       | u64::MAX (sat.)     | 3     | 4     |
| K₄       | u64::MAX (sat.)       | u64::MAX (sat.)       | u64::MAX (sat.)     | 6     | 4     |
| K_{2,3}  | u64::MAX (sat.)       | u64::MAX (sat.)       | u64::MAX (sat.)     | 6     | 5     |

**P₃ NPENTAACTC derivation:** 3 × 2^50 = 3 × 1_125_899_906_842_624 = 3_377_699_720_527_872 (fits u64 exactly).

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | +`graph_topo_indices76_inner()`, +`graph_topo_indices76()` |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_topo_indices76()` |
| `crates/k-shell/src/proc.rs` | +k-shell dispatch entry for topo76 |
| `host-tests/gos-graph-topo76-harness/` | New harness (10 tests, all pass) |

---

## k-shell Commands

```
graph topo76           gtopo76
neighborhood pentacontic              gnpentaactc
neighborhood nonapentacontic edge     gnhpentaactc
neighborhood octaocontyl sombor       gnnasso
gnpentaactcnhpentaactcnasso
```

---

## Test Results

```
running 10 tests
test test_01_empty        ... ok
test test_02_single_node  ... ok
test test_03_k2_edge      ... ok
test test_04_path_p3      ... ok
test test_05_triangle_k3  ... ok
test test_06_star_k14     ... ok
test test_07_path_p4      ... ok
test test_08_complete_k4  ... ok
test test_09_two_isolated  ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured
```

`cargo check --manifest-path crates/gos-runtime/Cargo.toml`: clean.
