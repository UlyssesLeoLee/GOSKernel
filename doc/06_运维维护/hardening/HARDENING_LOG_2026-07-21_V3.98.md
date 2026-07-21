# Hardening Log — V3.98 (2026-07-21)

## Summary

V3.98 adds **NHEXAENACTC + NHHEXAENACTC + NBDSO** Neighborhood S-variant topological indices
(topo87), the second of the hexacontic (60–69) series. +10 host tests → **1953 total**.

## What Changed

### New Runtime Function

`gos_runtime::graph_topo_indices87() -> (nhexaenactc: u64, nhhexaenactc: u64, nbdso: u64, edge_count: usize, node_count: usize)`

- **NHEXAENACTC(G)** = Σ_v S(v)^61 — S-Hexaencontic vertex sum (u128→u64, exact)
- **NHHEXAENACTC(G)** = Σ_{uv∈E} (S_u+S_v)^60 — S-Hexacontic edge-sum (u128→u64, exact)
- **NBDSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^55 — S-Variant Sombor α=110 (u128→u64, exact, no isqrt)

where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum.

### Series Context

- NHEXAENACTC extends NHEXAACTC=Σ S^60 (topo86) to 61st power; **second of hexacontic (60-69) series**
- NHHEXAENACTC extends NHHEXAACTC=Σ(S+S)^59 (topo86) to 60th power
- NBDSO = S-variant generalised Sombor SO^α with α=110: NBCSO(α=108,topo86)→NBDSO(α=110,topo87); **4th of NB series (letter D)**

### Implementation Details

Power chains (binary decomposition):
- s^61 = s32 × s16 × s8 × s4 × s (61=32+16+8+4+1; 5 mults)
- ss^60 = ss32 × ss16 × ss8 × ss4 (60=32+16+8+4; **4 mults — efficient!** all four powers of 2)
- s2s^55 = s2s32 × s2s16 × s2s4 × s2s2 × s2s (55=32+16+4+2+1; 5 mults)

### Analytical Test Values

| Graph     | NHEXAENACTC                    | NHHEXAENACTC                 | NBDSO                    |
|-----------|--------------------------------|------------------------------|--------------------------|
| Empty     | 0                              | 0                            | 0                        |
| K₂        | 2                              | 1_152_921_504_606_846_976    | 36_028_797_018_963_968   |
| P₃        | 6_917_529_027_641_081_856      | u64::MAX (sat.)              | u64::MAX (sat.)          |
| K₃+       | u64::MAX (sat.)                | u64::MAX (sat.)              | u64::MAX (sat.)          |

S-regular formulae:
- NHEXAENACTC = n·S^61
- NHHEXAENACTC = |E|·(2S)^60 = 1152921504606846976·|E|·S^60
- NBDSO = |E|·(2S²)^55 = 36028797018963968·|E|·S^110

### VectorAddress

L4=174 for gos-graph-topo87-harness; plugin `TOPIX_87`; executor `t87.exec`

### Shell Commands

```
graph topo87  /  gtopo87  /  neighborhood hexaencontic  /  gnhexaenactc
neighborhood hexacontic edge  /  gnnhhexaenactc
neighborhood dohectyl sombor bd  /  gnnbdso
gnhexaenactcnhhexaenactcnbdso
```

## Test Coverage

**gos-graph-topo87-harness** (10 tests):
1. Empty graph → (0, 0, 0, 0, 0)
2. Single isolated node → (0, 0, 0, 0, 1)
3. K₂ edge → (2, 1_152_921_504_606_846_976, 36_028_797_018_963_968, 1, 2)
4. Path P₃ → (6_917_529_027_641_081_856, u64::MAX, u64::MAX, 2, 3)
5. Triangle K₃ → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
6. Star K_{1,4} → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
7. Path P₄ → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
8. Complete K₄ → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
9. Two isolated nodes → (0, 0, 0, 0, 2)
10. K_{2,3} bipartite → (u64::MAX, u64::MAX, u64::MAX, 6, 5)

## Cumulative State

- Host-test suite total: **1953 tests** (all green)
- Prior: 1943 through V3.97
- gos-graph-topo87-harness: 10 (V3.98, new)
- VectorAddress L4 namespace: 88=graph-topo through 173=graph-topo86, **174=graph-topo87**
- NB series: NBASO(α=104)→NBBSO(α=106)→NBCSO(α=108)→**NBDSO(α=110)** (letter D, 4th)
- Hexacontic series: NHEXAACTC(S^60,topo86) → **NHEXAENACTC(S^61,topo87)** (2nd of 10)
