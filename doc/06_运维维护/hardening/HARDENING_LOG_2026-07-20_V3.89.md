# HARDENING LOG — V3.89 (2026-07-20)

## Summary

**Branch:** feat/vk-auto-live-surface  
**Version:** V3.89  
**Indices added:** NDOPENTAACTC + NHDOPENTAACTC + NAUSO  
**Harness:** gos-graph-topo78-harness (10 tests)  
**Total host tests:** 1863  

---

## New Topology Indices (topo78, L4=165)

### NDOPENTAACTC — S-Dopentacontic Vertex Sum

```
NDOPENTAACTC(G) = Σ_v S(v)^52
```

- **S-variant**: S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum)
- **Series position**: Third of the pentacontic (50–59) series; follows NHENPENTAACTC=Σ S^51 (topo77)
- **S-regular formula**: NDOPENTAACTC = n·S^52
- **Implementation**: s^52 = s32 × s16 × s4 (52=32+16+4; 3 mults — efficient!)
- **Overflow**: Saturating u128 accumulator, clamped to u64::MAX

### NHDOPENTAACTC — S-Henpentacontic Edge Sum

```
NHDOPENTAACTC(G) = Σ_{uv∈E} (S_u + S_v)^51
```

- **Series position**: Follows NHHENPENTAACTC=Σ(S+S)^50 (topo77)
- **S-regular formula**: NHDOPENTAACTC = |E|·(2S)^51 = 2_251_799_813_685_248·|E|·S^51
- **Implementation**: ss^51 = ss32 × ss16 × ss2 × ss (51=32+16+2+1; 4 mults)

### NAUSO — S-Variant Sombor α=92

```
NAUSO(G) = Σ_{uv∈E} (S_u² + S_v²)^46
```

- **Alpha**: α = 92 (3rd-pass "AU"; follows NATSO α=90, topo77)
- **S-regular formula**: NAUSO = |E|·(2S²)^46 = 70_368_744_177_664·|E|·S^92
- **Implementation**: s2s^46 = s2s32 × s2s8 × s2s4 × s2s2 (46=32+8+4+2; 4 mults)

---

## K₂ Reference Values

| Index | Value |
|-------|-------|
| NDOPENTAACTC | 2 (= 1^52 + 1^52) |
| NHDOPENTAACTC | 2_251_799_813_685_248 (= 2^51) |
| NAUSO | 70_368_744_177_664 (= 2^46) |

## P₃ Reference Values

| Index | Value |
|-------|-------|
| NDOPENTAACTC | 13_510_798_882_111_488 (= 3×2^52) |
| NHDOPENTAACTC | u64::MAX (saturated) |
| NAUSO | u64::MAX (saturated) |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices78_inner` + `graph_topo_indices78` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices78` |
| `crates/k-shell/src/proc.rs` | Added routing for `graph topo78` / `gtopo78` / `gndopentaactc` / etc. |
| `host-tests/gos-graph-topo78-harness/` | New 10-test harness |

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
3. K₂ → (2, 2_251_799_813_685_248, 70_368_744_177_664, 1, 2)
4. P₃ → (13_510_798_882_111_488, u64::MAX, u64::MAX, 2, 3)
5. K₃ → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
6. K_{1,4} → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
7. P₄ → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
8. K₄ → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
9. Two isolated → (0, 0, 0, 0, 2)
10. K_{2,3} → (u64::MAX, u64::MAX, u64::MAX, 6, 5)
