# GOSKernel Hardening Log — V3.14

**Date:** 2026-07-06  
**Branch:** feat/vk-auto-live-surface  
**Host tests:** 1113 (1103 prior + 10 new)

---

## V3.14 — SDD + ISI + Nirmala degree-based topological indices

### New runtime API

```rust
pub fn graph_topo_indices3() -> (u64, u64, u64, usize, usize)
// Returns (sdd_ppm, isi_ppm, ni_ppm, edge_count, node_count)
```

### Indices added

| Symbol | Name | Formula | Reference |
|--------|------|---------|-----------|
| SDD | Symmetric Division Degree | Σ_{uv∈E} (da²+db²)/(da·db) | Vasilyev 2014 / Gupta et al. 2000 |
| ISI | Inverse Sum Indeg | Σ_{uv∈E} da·db/(da+db) | Sedlar et al. 2011 |
| NI | Nirmala index | Σ_{uv∈E} √(da+db) | Rather et al. 2021 |

### Integer precision

- **SDD**: `sdd_ppm = Σ floor((da²+db²) × 10^6 / (da·db))` — exact when `da=db` (regular graph)
- **ISI**: `isi_ppm = Σ floor(da·db × 10^6 / (da+db))` — exact when `(da+db)` divides `da·db × 10^6`
- **NI**: `ni_ppm = Σ isqrt64((da+db) × 10^12)` — Newton-Raphson floor sqrt; exact when `da+db` is a perfect square

### Key invariants

**SDD:**
- SDD ≥ 2|E| always (AM-GM inequality: (da²+db²)/(da·db) ≥ 2)
- SDD = 2|E| **iff graph is regular** (da=db for all edges)
- Shell annotation shown when equality holds

**ISI:**
- ISI = |E|·Δ/2 for any Δ-regular graph (exact)
- K₃ (Δ=2, 3 edges): ISI = 3 exactly
- K₄ (Δ=3, 6 edges): ISI = 9 exactly

**NI:**
- NI = |E|·√(2Δ) for Δ-regular (exact when 2Δ is a perfect square)
- K₃ (Δ=2): NI = 3·√4 = 6 **exactly** (da+db=4, isqrt64(4×10^12) = 2_000_000)
- K₄ (Δ=3): NI = 6·√6 ≈ 14.697 (floor; 2Δ=6 not a perfect square)

### Key isqrt64 values

```
isqrt64(2_000_000_000_000) = 1_414_213  (√2 × 10^6; floor)
isqrt64(3_000_000_000_000) = 1_732_050  (√3 × 10^6; floor)
isqrt64(4_000_000_000_000) = 2_000_000  (√4 × 10^6; exact)
isqrt64(5_000_000_000_000) = 2_236_067  (√5 × 10^6; floor)
isqrt64(6_000_000_000_000) = 2_449_489  (√6 × 10^6; floor)
```

### Analytical cross-check table

| Graph | SDD_ppm | ISI_ppm | NI_ppm | edges | notes |
|-------|---------|---------|--------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | |
| 1 node | 0 | 0 | 0 | 0 | |
| Edge A-B | 2_000_000 | 500_000 | 1_414_213 | 1 | da=db=1; AM-GM eq |
| P₃ | 5_000_000 | 1_333_332 | 3_464_100 | 2 | |
| K₃ | 6_000_000 | 3_000_000 | 6_000_000 | 3 | all 3 invariants exact |
| K_{1,4} | 17_000_000 | 3_200_000 | 8_944_268 | 4 | SDD > 2|E| strict |
| P₄ | 7_000_000 | 2_333_332 | 5_464_100 | 3 | |
| K₄ | 12_000_000 | 9_000_000 | 14_696_934 | 6 | SDD=2|E|; ISI=|E|Δ/2 exact |
| 2 isolated | 0 | 0 | 0 | 0 | |
| K_{2,3} | 12_999_996 | 7_200_000 | 13_416_402 | 6 | SDD > 2|E| strict; ISI exact |

### Algorithm (O(V+E))

Same compact-index + undirected adjacency bitmask setup as V3.12/V3.13.  
Edge scan with `a < b` canonical order:

```rust
// SDD: floor((da²+db²) × 10^6 / (da·db))
sdd_acc += (da * da + db * db) * 1_000_000 / p;

// ISI: floor(da·db × 10^6 / (da+db))
isi_acc += p * 1_000_000 / s;

// NI: isqrt64((da+db) × 10^12)
ni_acc += isqrt64(s * 1_000_000_000_000u64);
```

Overflow safety: max `da²+db² ≤ 2·128² = 32_768`; `(da²+db²)×10^6 ≤ 3.27×10^10` — fits u64.
Max `s×10^12 = 256×10^12 = 2.56×10^14` — fits u64 (max ~1.8×10^19).

### Shell commands

```
graph topo3   gtopo3
symmetric division deg   gsdd
inverse sum indeg        gisi
nirmala index            gnirmala
gsddisini
```

### OS analogy

- **SDD** = bandwidth asymmetry factor across IPC channels — high SDD = hub-and-spoke topology with unbalanced coupling; SDD=2|E| = fully balanced (mesh/ring)
- **ISI** = harmonic-mean degree product per channel — measures effective coupling strength; ISI=|E|·Δ/2 when all endpoints are equally loaded
- **NI** = total "width" of IPC pathways — √(da+db) weights wider channels more than narrow; high NI = fat-pipe dominant topology

### VectorAddress namespace

```
L4=90: gos-graph-topo3-harness
```

(Previous: L4=89 gos-graph-topo2-harness, L4=88 gos-graph-topo-harness, L4=87 gos-graph-zagreb-harness)

### Literature

- Vasilyev / Gupta et al. 2000 (SDD)
- Sedlar, Stevanović & Vasilyev 2011 (ISI)
- Rather, Imran & Degree 2021 (Nirmala index)
- Follows degree-based index series: V3.11 (M1/M2/R/I) → V3.12 (SC/GA/AZI) → V3.13 (H/ABC/F) → V3.14 (SDD/ISI/NI)
