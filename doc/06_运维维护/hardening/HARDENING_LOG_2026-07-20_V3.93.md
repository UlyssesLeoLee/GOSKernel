# Hardening Log — V3.93 (2026-07-20)

## Summary

Added three new Neighborhood S-variant topological indices for the GOS graph kernel:

- **NHEXPENTAACTC** — S-Hexapentacontic vertex sum: `Σ_v S(v)^56`
- **NHHEXPENTAACTC** — S-Pentapentacontic edge-sum: `Σ_{uv∈E} (S_u+S_v)^55`
- **NAYSO** — S-Variant Sombor SO^α with α=100 (Centyl Sombor): `Σ_{uv∈E} (S_u²+S_v²)^50`

New harness `gos-graph-topo82-harness` added with 10 tests (all green).

Cumulative host-test count: **1903 tests**.

---

## Mathematical Definitions

Let `S(v) = Σ_{w∈N(v)} deg(w)` be the neighbor-degree sum of vertex `v`.

### NHEXPENTAACTC (S-Hexapentacontic vertex sum)

```
NHEXPENTAACTC(G) = Σ_v S(v)^56
```

- **Series position**: Seventh index in the pentacontic (50–59 power) series
- **Predecessor**: NPENTAPENTAACTC = Σ S^55 (V3.92, topo81)
- **S-regular formula**: NHEXPENTAACTC = n · S^56
- **Implementation**: s^56 = s32 × s16 × s8 (56 = 32+16+8; 3 multiplications — efficient!)
- **Overflow handling**: Saturating u128 accumulator, clamped to u64::MAX

### NHHEXPENTAACTC (S-Pentapentacontic edge-sum)

```
NHHEXPENTAACTC(G) = Σ_{uv∈E} (S_u + S_v)^55
```

- **Series position**: Extends NHPENTAPENTAACTC = Σ(S+S)^54 (topo81) to 55th power
- **S-regular formula**: NHHEXPENTAACTC = |E| · (2S)^55 = 36028797018963968 · |E| · S^55
- **Implementation**: ss^55 = ss32 × ss16 × ss4 × ss2 × ss (55 = 32+16+4+2+1; 5 mults)

### NAYSO (S-Centyl Sombor, α=100)

```
NAYSO(G) = Σ_{uv∈E} (S_u² + S_v²)^50
```

- **Series position**: 3rd-pass double-letter AY in the generalised Sombor SO^α family
- **Predecessor**: NAXSO (α=98, topo81)
- **S-regular formula**: NAYSO = |E| · (2S²)^50 = 1125899906842624 · |E| · S^100
- **Implementation**: s2s^50 = s2s32 × s2s16 × s2s2 (50 = 32+16+2; 3 mults)
- **Note**: s^56 = s32×s16×s8 is efficient (56 = 32+16+8, three powers of 2, only 3 final mults)

---

## Analytical Test Values

| Graph    | NHEXPENTAACTC         | NHHEXPENTAACTC              | NAYSO               | edges | nodes |
|----------|-----------------------|-----------------------------|---------------------|-------|-------|
| Empty    | 0                     | 0                           | 0                   | 0     | 0     |
| K₂       | 2                     | 36_028_797_018_963_968      | 1_125_899_906_842_624 | 1   | 2     |
| P₃       | 216_172_782_113_783_808 | u64::MAX (sat.)           | u64::MAX (sat.)     | 2     | 3     |
| K₃       | u64::MAX (sat.)       | u64::MAX (sat.)             | u64::MAX (sat.)     | 3     | 3     |
| K₄       | u64::MAX (sat.)       | u64::MAX (sat.)             | u64::MAX (sat.)     | 6     | 4     |

**K₂ derivation** (S=1 uniform):
- NHEXPENTAACTC: 1^56 + 1^56 = 2
- NHHEXPENTAACTC: (1+1)^55 = 2^55 = 36_028_797_018_963_968
- NAYSO: (1²+1²)^50 = 2^50 = 1_125_899_906_842_624

**P₃ derivation** (S=2 uniform, 3 nodes × S^56):
- NHEXPENTAACTC: 3 × 2^56 = 3 × 72_057_594_037_927_936 = 216_172_782_113_783_808 (fits in u64)
- NHHEXPENTAACTC: 2 × 4^55 = 2 × 2^110 → saturates
- NAYSO: 2 × 8^50 = 2 × 2^150 → saturates

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices82_inner()` + public `graph_topo_indices82()` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices82()` display function |
| `crates/k-shell/src/proc.rs` | Added routing for `graph topo82`/`gtopo82`/`gnhexpentaactc`/`gnnhhexpentaactc`/`gnnayso` |
| `host-tests/gos-graph-topo82-harness/` | New harness (10 tests, all green) |

---

## Shell Commands

```
graph topo82
gtopo82
neighborhood hexapentacontic       (→ NHEXPENTAACTC)
gnhexpentaactc
neighborhood pentapentacontic edge (→ NHHEXPENTAACTC)
gnnhhexpentaactc
neighborhood centyl sombor         (→ NAYSO)
gnnayso
gnhexpentaactcnhhexpentaactcnayso
```

---

## VectorAddress Namespace

- L4=168: gos-graph-topo81-harness (V3.92)
- **L4=169: gos-graph-topo82-harness (V3.93, this change)**

---

## Runtime API

```rust
gos_runtime::graph_topo_indices82() -> (nhexpentaactc: u64, nhhexpentaactc: u64, nayso: u64, edge_count: usize, node_count: usize)
```

- Plugin: `TOPIX_82`
- Executor: `t82.exec`

---

## Context

Part of the ongoing automated hardening cycle. The S-variant pentacontic series (topo76–topo85) implements
a systematic family of high-power vertex/edge topological indices on the neighbor-degree sum (S-variant).

Each firing adds three indices:
1. A vertex sum S^n (incrementing by 1 per version)
2. An edge sum (S_u+S_v)^(n-1) (one less power)  
3. A generalised S-variant Sombor SO^α with α = 2×(n-6) (incrementing α by 2 per version)

The double-letter SO naming (NAASO, NABSO, ..., NAXSO, NAYSO) tracks the α/2 offset from
the initial NSO (α=1, topo21) through the extended series.
