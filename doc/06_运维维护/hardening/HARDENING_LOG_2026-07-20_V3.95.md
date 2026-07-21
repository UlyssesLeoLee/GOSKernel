# HARDENING LOG — V3.95 (2026-07-20)

## Summary

Added three Neighborhood S-variant topological indices (topo84) and a 10-test harness.

## Changes

### crates/gos-runtime/src/lib.rs
- Added `graph_topo_indices84_inner()` — computes NOCTOPENTAACTC + NHOCTOPENTAACTC + NBASO
- Added `pub fn graph_topo_indices84()` public API wrapper

### crates/k-shell/src/lib.rs
- Added `dispatch_graph_topo_indices84()` — colored terminal output for all three indices

### crates/k-shell/src/proc.rs
- Added routing for topo84 commands:
  - `"graph topo84"`, `"gtopo84"`
  - `"neighborhood octopentacontic"`, `"gnoctopentaactc"`
  - `"neighborhood heptapentacontic edge"`, `"gnnhoctopentaactc"`
  - `"neighborhood tetrahectyl sombor"`, `"gnnbaso"`
  - `"gnoctopentaactcnhoctopentaactcnbaso"`

### host-tests/gos-graph-topo84-harness/ (new)
- 10 tests: empty, single node, K₂, P₃, K₃, K_{1,4}, P₄, K₄, two isolated, K_{2,3}
- All 10 pass (verified)

## Indices Implemented

### NOCTOPENTAACTC(G) = Σ_v S(v)^58
- S-Octopentacontic vertex sum (9th of pentacontic 50–59 series)
- Extends NHEPTPENTAACTC=Σ S^57 (topo83)
- K₂: 2; P₃: 864_691_128_455_135_232; all larger graphs saturate
- Implementation: s^58 = s32×s16×s8×s2 (58=32+16+8+2; 4 mults)

### NHOCTOPENTAACTC(G) = Σ_{uv∈E} (S_u+S_v)^57
- S-Heptapentacontic edge-sum
- Extends NHHEPTPENTAACTC=Σ(S+S)^56 (topo83)
- K₂: 144_115_188_075_855_872 (=2^57); P₃ and above saturate
- Implementation: ss^57 = ss32×ss16×ss8×ss (57=32+16+8+1; 4 mults)

### NBASO(G) = Σ_{uv∈E} (S_u²+S_v²)^52
- S-Variant Sombor SO^α with α=104 (first of NB series; 4th-pass BA)
- NAZSO(α=102,topo83) → NBASO(α=104,topo84)
- K₂: 4_503_599_627_370_496 (=2^52); P₃ and above saturate
- Implementation: s2s^52 = s2s32×s2s16×s2s4 (52=32+16+4; 3 mults — efficient!)

## VectorAddress
- L4=171 for gos-graph-topo84-harness
- Plugin: TOPIX_84, executor: t84.exec

## Test Results
- 10/10 tests passed
- Host test suite total: **1923 tests** (1913 prior + 10 new)

## Commit
9a5f892 feat(v3.95): NOCTOPENTAACTC + NHOCTOPENTAACTC + NBASO Neighborhood S-variant indices + gos-graph-topo84-harness (10 tests)
