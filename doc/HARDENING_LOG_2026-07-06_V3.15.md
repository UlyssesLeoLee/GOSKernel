# GOSKernel Hardening Log — V3.15

**Date:** 2026-07-06  
**Branch:** feat/vk-auto-live-surface  
**Host tests:** 1123 (1113 prior + 10 new)

---

## V3.15 — Sombor + Reduced Second Zagreb + Sigma degree-based topological indices

### New runtime API

```rust
pub fn graph_topo_indices4() -> (u64, u64, u64, usize, usize)
// Returns (so_ppm, rm2, sigma, edge_count, node_count)
```

### Indices added

| Symbol | Name | Formula | Reference |
|--------|------|---------|-----------|
| SO | Sombor index | Σ_{uv∈E} √(da²+db²) | Gutman 2021 |
| RM₂ | Reduced Second Zagreb | Σ_{uv∈E} (da-1)·(db-1) | Furtula, Gutman & Ediz 2014 |
| σ | Sigma index | Σ_{uv∈E} (da-db)² | Gutman et al. 2014 |

### Integer precision

- **SO**: `so_ppm = Σ isqrt64((da²+db²) × 10^12)` — Newton-Raphson floor sqrt; error ≤ 1 ppm per edge
- **RM₂**: exact integer; `(da-1)·(db-1)`; 0 for pendant edges (da=1 or db=1)
- **σ**: exact integer; `(da-db)²`; never negative; 0 iff graph is regular

### Key invariants

**SO:**
- SO = |E|·da·√2 for Δ-regular graph (floor; exact only if 2Δ² is a perfect square — never for Δ≥1)
- SO monotone: adding edges or increasing degrees always raises SO

**RM₂:**
- RM₂ = 0 for any pendant edge endpoint (da=1 or db=1) → star graphs always have RM₂=0
- RM₂ = |E|·(Δ-1)² for any Δ-regular graph (exact integer)
- K₃ (Δ=2, 3 edges): RM₂ = 3·1² = 3 exactly
- K₄ (Δ=3, 6 edges): RM₂ = 6·2² = 24 exactly

**σ (Sigma index):**
- σ = 0 **iff graph is regular** — rigorous regularity certificate
- Shell annotation shown when σ=0 and edge_count>0 (confirms regularity)
- σ = Σ (da-db)² ≥ 0 always; 0 for K₃, K₄, Kₙ, Cₙ, regular grids

### Key isqrt64 values for SO

```
isqrt64(2_000_000_000_000)  = 1_414_213  (√2 × 10^6;  √(1²+1²))
isqrt64(5_000_000_000_000)  = 2_236_067  (√5 × 10^6;  √(1²+2²))
isqrt64(8_000_000_000_000)  = 2_828_427  (2√2 × 10^6; √(2²+2²); K₃ per-edge)
isqrt64(13_000_000_000_000) = 3_605_551  (√13 × 10^6; √(2²+3²); K_{2,3} per-edge)
isqrt64(17_000_000_000_000) = 4_123_105  (√17 × 10^6; √(1²+4²); K_{1,4} per-edge)
isqrt64(18_000_000_000_000) = 4_242_640  (3√2 × 10^6; √(3²+3²); K₄ per-edge)
```

### Analytical cross-check table

| Graph | SO_ppm | RM₂ | σ | edges | notes |
|-------|--------|-----|---|-------|-------|
| Empty | 0 | 0 | 0 | 0 | |
| 1 node | 0 | 0 | 0 | 0 | |
| Edge A-B | 1_414_213 | 0 | 0 | 1 | da=db=1; rm2=0 pendant; σ=0 regular |
| P₃ | 4_472_134 | 0 | 2 | 2 | pendant edges; σ=(1-2)²×2 |
| K₃ | 8_485_281 | 3 | 0 | 3 | σ=0 certifies regularity; rm2=|E|·(Δ-1)² |
| K_{1,4} | 16_492_420 | 0 | 36 | 4 | rm2=0 (all pendant); σ=4·9=36 |
| P₄ | 7_300_561 | 1 | 2 | 3 | inner edge B-C: rm2=1; pendants: σ=2 |
| K₄ | 25_455_840 | 24 | 0 | 6 | σ=0 regular; rm2=6·4=24=|E|·(Δ-1)² |
| 2 isolated | 0 | 0 | 0 | 0 | |
| K_{2,3} | 21_633_306 | 12 | 6 | 6 | σ>0 non-regular; rm2=6·2=12 exact |

### Algorithm (O(V+E))

Same compact-index + undirected adjacency bitmask setup as V3.12–V3.14.  
Edge scan with `a < b` canonical order:

```rust
// SO: isqrt64((da²+db²) × 10^12)
so_acc += isqrt64((da * da + db * db) * 1_000_000_000_000u64);

// RM₂: (da-1)·(db-1); pendant edges contribute 0
if da > 0 && db > 0 {
    rm2_acc += (da - 1) * (db - 1);
}

// σ: (da-db)²; 0 for regular graphs
let diff = if da >= db { da - db } else { db - da };
sigma_acc += diff * diff;
```

Overflow safety:
- SO: max `da²+db² ≤ 2·128² = 32_768`; `32_768×10^12 = 3.28×10^16` — fits u64 (max ~1.8×10^19) ✓
- RM₂: max per edge `(127)·(127) = 16_129`; max total `512×16_129 ≈ 8.3×10^6` — trivially fits u64 ✓
- σ: max per edge `(128-1)² = 16_129`; max total ~8.3×10^6 — trivially fits u64 ✓

### Shell commands

```
graph topo4      gtopo4
sombor index     gsombor
reduced zagreb   grm2
sigma index      gsigma
gsomborrm2sigma
```

### OS analogy

- **SO** = Euclidean bond norm of (da, db) endpoint-degree pair — high SO = large hub-spoke
  asymmetry; SO = |E|·Δ·√2 for uniform meshes (regular topologies)
- **RM₂** = "internal coupling density" — product of excess connections beyond degree-1;
  0 for all star-topology edges; = |E|·(Δ-1)² for ring/mesh (regular) topologies
- **σ** = total squared degree imbalance across IPC channels — σ=0 certifies a perfectly
  balanced (regular) kernel dependency graph; high σ = hub-dominant, fault-asymmetric topology

### VectorAddress namespace

```
L4=91: gos-graph-topo4-harness
```

(Previous: L4=90 gos-graph-topo3-harness, L4=89 gos-graph-topo2-harness, L4=88 gos-graph-topo-harness)

### Literature

- Gutman 2021 (Sombor index — MATCH Commun. Math. Comput. Chem. 86:11–16)
- Furtula, Gutman & Ediz 2014 (Reduced Second Zagreb — Applied Mathematics and Computation)
- Gutman, Togan, Yurttas, Cevik & Cangul 2014 (Sigma index — MATCH Commun. Math. Comput. Chem.)
- Follows degree-based index series: V3.12 (SC/GA/AZI) → V3.13 (H/ABC/F) → V3.14 (SDD/ISI/NI) → V3.15 (SO/RM₂/σ)
