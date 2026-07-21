# Hardening Log — V3.99 (2026-07-21)

## Summary

V3.99 adds **NHEXADYACTC + NHHEXADYACTC + NBESO** Neighborhood S-variant topological indices
(topo88), the third of the hexacontic (60–69) series. +10 host tests → **1963 total**.

## What Changed

### New Runtime Function

`gos_runtime::graph_topo_indices88() -> (nhexadyactc: u64, nhhexadyactc: u64, nbeso: u64, edge_count: usize, node_count: usize)`

- **NHEXADYACTC(G)** = Σ_v S(v)^62 — S-Hexadycontic vertex sum (u128→u64, exact)
- **NHHEXADYACTC(G)** = Σ_{uv∈E} (S_u+S_v)^61 — S-Hexaencontic edge-sum (u128→u64, exact)
- **NBESO(G)** = Σ_{uv∈E} (S_u²+S_v²)^56 — S-Variant Sombor α=112 (u128→u64, exact, no isqrt)

where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum.

### Series Context

- NHEXADYACTC extends NHEXAENACTC=Σ S^61 (topo87) to 62nd power; **third of hexacontic (60-69) series**
- NHHEXADYACTC extends NHHEXAENACTC=Σ(S+S)^60 (topo87) to 61st power
- NBESO = S-variant generalised Sombor SO^α with α=112: NBDSO(α=110,topo87)→NBESO(α=112,topo88); **5th of NB series (letter E)**

### Implementation Details

Power chains (binary decomposition):
- s^62 = s32 × s16 × s8 × s4 × s2 (62=32+16+8+4+2; 5 mults)
- ss^61 = ss32 × ss16 × ss8 × ss4 × ss (61=32+16+8+4+1; 5 mults)
- s2s^56 = s2s32 × s2s16 × s2s8 (56=32+16+8; **3 mults — efficient!** all three powers of 2)

### Analytical Test Values

| Graph     | NHEXADYACTC                     | NHHEXADYACTC                 | NBESO                     |
|-----------|---------------------------------|------------------------------|---------------------------|
| Empty     | 0                               | 0                            | 0                         |
| K₂        | 2                               | 2_305_843_009_213_693_952    | 72_057_594_037_927_936    |
| P₃        | 13_835_058_055_282_163_712      | u64::MAX (sat.)              | u64::MAX (sat.)           |
| K₃+       | u64::MAX (sat.)                 | u64::MAX (sat.)              | u64::MAX (sat.)           |

S-regular formulae:
- NHEXADYACTC = n·S^62
- NHHEXADYACTC = |E|·(2S)^61 = 2305843009213693952·|E|·S^61
- NBESO = |E|·(2S²)^56 = 72057594037927936·|E|·S^112

### VectorAddress

L4=175 for gos-graph-topo88-harness; plugin `TOPIX_88`; executor `t88.exec`

### Shell Commands

```
graph topo88  /  gtopo88  /  neighborhood hexadycontic  /  gnhexadyactc
neighborhood hexaencontic edge  /  gnnhhexadyactc
neighborhood dohectyl sombor be  /  gnnbeso
gnhexadyactnhhexadyactnbeso
```

## Test Coverage

**gos-graph-topo88-harness** (10 tests):
1. Empty graph → (0, 0, 0, 0, 0)
2. Single isolated node → (0, 0, 0, 0, 1)
3. K₂ edge → (2, 2_305_843_009_213_693_952, 72_057_594_037_927_936, 1, 2)
4. Path P₃ → (13_835_058_055_282_163_712, u64::MAX, u64::MAX, 2, 3)
5. Triangle K₃ → (u64::MAX, u64::MAX, u64::MAX, 3, 3)
6. Star K_{1,4} → (u64::MAX, u64::MAX, u64::MAX, 4, 5)
7. Path P₄ → (u64::MAX, u64::MAX, u64::MAX, 3, 4)
8. Complete K₄ → (u64::MAX, u64::MAX, u64::MAX, 6, 4)
9. Two isolated nodes → (0, 0, 0, 0, 2)
10. K_{2,3} bipartite → (u64::MAX, u64::MAX, u64::MAX, 6, 5)

## Cumulative State

- Host-test suite total: **1963 tests** (all green)
- Prior: 1953 through V3.98
- gos-graph-topo88-harness: 10 (V3.99, new)
- VectorAddress L4 namespace: 88=graph-topo through 174=graph-topo87, **175=graph-topo88**
- NB series: NBASO(α=104)→NBBSO(α=106)→NBCSO(α=108)→NBDSO(α=110)→**NBESO(α=112)** (letter E, 5th)
- Hexacontic series: NHEXAACTC(S^60,topo86)→NHEXAENACTC(S^61,topo87)→**NHEXADYACTC(S^62,topo88)** (3rd of 10)
