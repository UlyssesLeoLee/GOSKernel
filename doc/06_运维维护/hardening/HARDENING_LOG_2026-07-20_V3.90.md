# HARDENING LOG — V3.90 (2026-07-20)

## Summary

**Branch:** feat/vk-auto-live-surface  
**Version:** V3.90  
**Indices added:** NTRIPENTAACTC + NHTRIPENTAACTC + NAVSO  
**Harness:** gos-graph-topo79-harness (10 tests)  
**Total host tests:** 1873  

---

## New Topology Indices (topo79, L4=166)

### NTRIPENTAACTC — S-Tripentacontic Vertex Sum

```
NTRIPENTAACTC(G) = Σ_v S(v)^53
```

- **S-variant**: S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum)
- **Series position**: Fourth of the pentacontic (50–59) series; follows NDOPENTAACTC=Σ S^52 (topo78)
- **S-regular formula**: NTRIPENTAACTC = n·S^53
- **Implementation**: s^53 = s32 × s16 × s4 × s (53=32+16+4+1; 4 mults)
- **Overflow**: Saturating u128 accumulator, clamped to u64::MAX

### NHTRIPENTAACTC — S-Dopentacontic Edge Sum

```
NHTRIPENTAACTC(G) = Σ_{uv∈E} (S_u + S_v)^52
```

- **Series position**: Follows NHDOPENTAACTC=Σ(S+S)^51 (topo78)
- **S-regular formula**: NHTRIPENTAACTC = |E|·(2S)^52 = 4_503_599_627_370_496·|E|·S^52
- **Implementation**: ss^52 = ss32 × ss16 × ss4 (52=32+16+4; 3 mults — efficient!)

### NAVSO — S-Variant Sombor α=94

```
NAVSO(G) = Σ_{uv∈E} (S_u² + S_v²)^47
```

- **Alpha**: α = 94 (3rd-pass "AV"; follows NAUSO α=92, topo78)
- **S-regular formula**: NAVSO = |E|·(2S²)^47 = 140_737_488_355_328·|E|·S^94
- **Implementation**: s2s^47 = s2s32 × s2s8 × s2s4 × s2s2 × s2s (47=32+8+4+2+1; 5 mults)

---

## K₂ Reference Values

| Index | Value |
|-------|-------|
| NTRIPENTAACTC | 2 (= 1^53 + 1^53) |
| NHTRIPENTAACTC | 4_503_599_627_370_496 (= 2^52) |
| NAVSO | 140_737_488_355_328 (= 2^47) |

## P₃ Reference Values

| Index | Value |
|-------|-------|
| NTRIPENTAACTC | 27_021_597_764_222_976 (= 3×2^53) |
| NHTRIPENTAACTC | u64::MAX (saturated) |
| NAVSO | u64::MAX (saturated) |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices79_inner` + `graph_topo_indices79` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices79` |
| `crates/k-shell/src/proc.rs` | Added routing for `graph topo79` / `gtopo79` / `gntripentaactc` / etc. |
| `host-tests/gos-graph-topo79-harness/` | New 10-test harness (with `.cargo/config.toml` for host target) |

---

## Test Results

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 10 tests verified:
1. Empty graph → (0, 0, 0, 0, 0)
2. Single node → (0, 0, 0, 0, 1)
3. K₂ → (2, 4_503_599_627_370_496, 140_737_488_355_328, 1, 2)
4. P₃ → (27_021_597_764_222_976, u64::MAX, u64::MAX, 2, 3)
5. K₃ → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
6. K_{1,4} → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
7. P₄ → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
8. K₄ → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
9. Two isolated → (0, 0, 0, 0, 2)
10. K_{2,3} → (u64::MAX, u64::MAX, u64::MAX, 6, 5)
