# Hardening Log V3.41 — NVQ + NRGS + NHCS S-variant Topological Indices

**Date:** 2026-07-16
**Branch:** feat/vk-auto-live-surface
**Commit:** (see git log)
**Host-test total:** 1383 (1373 prior + 10 new)

---

## Summary

Three new S-variant topological indices — NVQ, NRGS, NHCS — exposed as `graph_topo_indices30()` in `gos_runtime`, `dispatch_graph_topo_indices30` in k-shell, and validated by `gos-graph-topo30-harness` (10 tests, all green).

This extends the S-variant family (topo18, topo21–topo30) with higher-order power indices: NVQ extends the vertex-power series (NM₁=Σ S², NF=Σ S³ → NVQ=Σ S⁴), NRGS is the S-analogue of generalized Randić at exponent 3/2, and NHCS is the cubic extension of NHM₁=Σ(S+S)².

---

## Feature: `graph topo30` — NVQ + NRGS + NHCS

### Definitions

Where S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant"):

| Index | Formula | Type | Description |
|-------|---------|------|-------------|
| **NVQ** | Σ_v S(v)⁴ | exact u64 | S-Quartic vertex sum; extends NM₁=Σ S² and NF=Σ S³ to 4th power |
| **NRGS** | Σ_{uv∈E} (S_u·S_v)^{3/2} | ppm (floor) | S-Generalized Randić at α=3/2; S-analogue of χ_{3/2}(G) |
| **NHCS** | Σ_{uv∈E} (S_u+S_v)³ | exact u64 | S-Cubic edge-sum; extends NHM₁=Σ(S+S)² (topo23) to 3rd power |

### Implementation

- **Algorithm**: O(V+E) — degree pass → S(v) pass → vertex scan (NVQ) + edge scan (NRGS, NHCS)
- **No BFS required** (same O(V+E) class as all topo18–topo30)
- **Overflow safety**:
  - NVQ: S(v)⁴ ≤ 16129⁴ ≈ 6.77×10^16 < u64::MAX; sum ≤ 128 × 6.77×10^16 ≈ 8.67×10^18 < u64::MAX ✓
  - NRGS: `sp = (S_u·S_v) as u128; isqrt128(sp³×10^12)` — intermediate ≤ ~1.76×10^37 < u128::MAX ✓
  - NHCS: (S_u+S_v)³ ≤ 32258³ ≈ 3.36×10^13 per edge; sum ≤ 2.73×10^17 < u64::MAX ✓
- **Return**: `(nvq: u64, nrgs_ppm: u64, nhcs: u64, edge_count: usize, node_count: usize)`

### Shell commands

```
graph topo30 / gtopo30
neighborhood quartic / gnvq
neighborhood randic32 / gnrgs
neighborhood cubic sum / gnhcs
gnvqnrgsnhcs
```

### Key invariants

- NVQ = n·S⁴ for S-regular graphs
- NRGS = |E|·S³·10^6 for S-regular graphs (exact when S is a perfect square; NRGS = |E|·(S²)^{3/2}·10^6 = |E|·S³·10^6)
- NHCS = 8·|E|·S³ for S-regular graphs (since (2S)³ = 8S³)
- K₃ and K_{1,4}: S-uniform S=4 → same per-edge NRGS (64_000_000) and NHCS (512); differ in NVQ (768 vs 1280) and total NRGS/NHCS (by |E| factor)
- K₄ (S=9): NRGS per edge = 729_000_000; NHCS per edge = 5832
- K_{2,3} (S=6): NRGS per edge = 216_000_000; NHCS per edge = 1728

### S-regularity (all common test graphs)

- K₂: S=1. P₃: S=2. K₃, K_{1,4}: S=4. K₄: S=9. K_{2,3}: S=6. P₄: mixed (2,3,3,2).

### Cross-check table

| Graph | NVQ | NRGS (ppm) | NHCS | edges | nodes |
|-------|-----|-----------|------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| K₂ | 2 | 1_000_000 | 8 | 1 | 2 |
| P₃ | 48 | 16_000_000 | 128 | 2 | 3 |
| K₃ | 768 | 192_000_000 | 1_536 | 3 | 3 |
| K_{1,4} | 1_280 | 256_000_000 | 2_048 | 4 | 5 |
| P₄ | 194 | 56_393_876 | 466 | 3 | 4 |
| K₄ | 26_244 | 4_374_000_000 | 34_992 | 6 | 4 |
| K_{2,3} | 6_480 | 1_296_000_000 | 10_368 | 6 | 5 |

### Derivation highlights

**P₄ NRGS cross-check** (mixed S=2,3,3,2):
- {A,B}: (2·3)^{3/2}·10^6 = isqrt128(216·10^12) = 14_696_938 (√216 = 14.6969...)
- {B,C}: (3·3)^{3/2}·10^6 = isqrt128(729·10^12) = 27_000_000 (exact: 27²=729)
- {C,D}: same as {A,B} = 14_696_938
- Total: 56_393_876 ✓

**K₄ NRGS** (S=9 uniform): 9³ = 729; 6 × 729_000_000 = 4_374_000_000 ✓

**K_{2,3} NRGS** (S=6 uniform): 6³ = 216; 6 × 216_000_000 = 1_296_000_000 ✓

### OS analogy

- **NVQ** = 4th-order neighborhood routing pressure (amplifies high-S hub-of-hub nodes more than NF cubic)
- **NRGS** = S-geometric mean channel coupling at 3/2 order (interpolates between NM₂ linear and NHM₂ quadratic)
- **NHCS** = cubic S-edge-sum pressure (high for asymmetric hub-spoke edges; 8× NHM₁ for S-regular)

---

## VectorAddress L4 namespace (updated)

..., 115=graph-topo28, 116=graph-topo29, **117=graph-topo30**

---

## Files changed

- `crates/gos-runtime/src/lib.rs` — `graph_topo_indices30_inner` + `graph_topo_indices30()`
- `crates/k-shell/src/lib.rs` — `dispatch_graph_topo_indices30`
- `crates/k-shell/src/proc.rs` — routing for topo30 (5 aliases)
- `host-tests/gos-graph-topo30-harness/` — new harness (10 tests, all green)
- `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.41.md` — this file
