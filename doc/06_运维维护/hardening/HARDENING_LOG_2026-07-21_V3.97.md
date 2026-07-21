# Hardening Log — V3.97 (2026-07-21)

## Summary

V3.97 adds **NHEXAACTC + NHHEXAACTC + NBCSO** Neighborhood S-variant topological indices
(topo86), opening the hexacontic (60–69) series. +10 host tests → **1943 total**.

## What Changed

### New Runtime Function

`gos_runtime::graph_topo_indices86() -> (nhexaactc: u64, nhhexaactc: u64, nbcso: u64, edge_count: usize, node_count: usize)`

- **NHEXAACTC(G)** = Σ_v S(v)^60 — S-Hexacontic vertex sum (u128→u64, exact)
- **NHHEXAACTC(G)** = Σ_{uv∈E} (S_u+S_v)^59 — S-Nonapentacontic edge-sum (u128→u64, exact)
- **NBCSO(G)** = Σ_{uv∈E} (S_u²+S_v²)^54 — S-Variant Sombor α=108 (u128→u64, exact, no isqrt)

where S(v) = Σ_{w∈N(v)} deg(w) is the neighbor-degree sum.

### Series Context

- NHEXAACTC extends NNONAPENTAACTC=Σ S^59 (topo85) to 60th power; **first of hexacontic (60-69) series**
- NHHEXAACTC extends NHNONAPENTAACTC=Σ(S+S)^58 (topo85) to 59th power
- NBCSO = S-variant generalised Sombor SO^α with α=108: NBBSO(α=106,topo85)→NBCSO(α=108,topo86); **3rd of NB series (letter C)**

### Implementation Details

Power chains (binary decomposition):
- s^60 = s32 × s16 × s8 × s4 (60=32+16+8+4; **4 mults — efficient!** all four powers of 2)
- ss^59 = ss32 × ss16 × ss8 × ss2 × ss (59=32+16+8+2+1; 5 mults)
- s2s^54 = s2s32 × s2s16 × s2s4 × s2s2 (54=32+16+4+2; 4 mults)

### Analytical Test Values

| Graph     | NHEXAACTC                      | NHHEXAACTC                  | NBCSO                    |
|-----------|--------------------------------|-----------------------------|--------------------------|
| Empty     | 0                              | 0                           | 0                        |
| K₂        | 2                              | 576_460_752_303_423_488      | 18_014_398_509_481_984   |
| P₃        | 3_458_764_513_820_540_928      | u64::MAX (sat.)             | u64::MAX (sat.)          |
| K₃+       | u64::MAX (sat.)                | u64::MAX (sat.)             | u64::MAX (sat.)          |

S-regular formulae:
- NHEXAACTC = n·S^60
- NHHEXAACTC = |E|·(2S)^59 = 576460752303423488·|E|·S^59
- NBCSO = |E|·(2S²)^54 = 18014398509481984·|E|·S^108

### VectorAddress

L4=173 for gos-graph-topo86-harness; plugin `TOPIX_86`; executor `t86.exec`

### Shell Commands

`graph topo86` / `gtopo86` / `gnhexaactc` / `gnnhhexaactc` / `gnnbcso` / `gnhexaactcnhhexaactcnbcso`

## Files Modified

- `crates/gos-runtime/src/lib.rs` — added `graph_topo_indices86_inner()` + `graph_topo_indices86()`
- `crates/k-shell/src/lib.rs` — added `dispatch_graph_topo_indices86()`
- `crates/k-shell/src/proc.rs` — added routing for `graph topo86`
- `host-tests/gos-graph-topo86-harness/` — new standalone harness (10 tests, all green)

## Test Results

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_k2_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Host-test suite total: 1943** (1933 prior + 10 new)
