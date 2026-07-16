# Hardening Log V3.40 — NZ₀ + NEM₂ + NSe S-variant Topological Indices + Fix topo28 k-shell gap

**Date:** 2026-07-16
**Branch:** feat/vk-auto-live-surface
**Commit:** (see git log)
**Host-test total:** 1373 (1363 prior + 10 new)

---

## Summary

Two changes in this slice:

1. **Fix (V3.39 gap)**: `dispatch_graph_topo_indices28` + k-shell routing for topo28 were missing from the V3.39 commit. Added now (`crates/k-shell/src/lib.rs` + `crates/k-shell/src/proc.rs`).

2. **Feature (V3.40)**: Three new S-variant topological indices — NZ₀, NEM₂, NSe — exposed as `graph_topo_indices29()` in `gos_runtime`, `dispatch_graph_topo_indices29` in k-shell, and validated by `gos-graph-topo29-harness` (10 tests, all green).

---

## Feature: `graph topo29` — NZ₀ + NEM₂ + NSe

### Definitions

Where S(v) = Σ_{w∈N(v)} deg(w) (neighbor-degree sum, "S-variant" as per Mondal et al. 2019 family):

| Index | Formula | Type | Reference |
|-------|---------|------|-----------|
| **NZ₀** | Σ_{v: S(v)>0} 1/√S(v) | ppm (floor) | S-analogue of zeroth-order Randić χ₀ (Randić 1975) |
| **NEM₂** | Σ_{uv∈E} S_u·S_v·(S_u+S_v−2) | exact u64 | S-analogue of Reformulated 2nd Zagreb EM₂ (Miličević et al. 2004) |
| **NSe** | Σ_v √S(v) | ppm (floor) | S-sqrt vertex sum (companion to NF=Σ_v S³ from topo22) |

### Implementation

- **Algorithm**: O(V+E) — degree pass → S(v) pass → node scan (NZ₀, NSe) → edge scan (NEM₂)
- **No BFS required** (same O(V+E) class as topo18–topo28)
- **Overflow safety**:
  - NZ₀: `isqrt64(10^12/S(v))` — max input 10^12 < u64::MAX ✓; isolated nodes (S=0) skipped
  - NEM₂: per-edge max ≈ 8.39×10^12; sum ≤ 6.82×10^16 < u64::MAX ✓
  - NSe: `isqrt64(S(v)×10^12)` — max input 16129×10^12 = 1.61×10^16 < u64::MAX ✓
- **Return**: `(nz0_ppm: u64, nem2: u64, nse_ppm: u64, edge_count: usize, node_count: usize)`

### Shell commands

```
graph topo29 / gtopo29
neighborhood zero randic / gnz0
neighborhood em2 / gnem2
neighborhood sqrt vertex / gnse
gnz0nem2nse
```

### Key invariants

- NZ₀ = n × isqrt64(10^12/S) for S-regular graphs
- NEM₂ = 0 iff all edges have S_u+S_v=2 (only K₂-type; annotated "NEM2=0: all S=1 edges")
- NEM₂ = |E|·S²·(2S−2) for S-regular
- NSe = n × isqrt64(S×10^12) for S-regular
- K₃ and K_{1,4}: S-uniform S=4 → same per-vertex NZ₀ and NSe; NEM₂ differs by |E| factor
- K₄ and K_{2,3}: both have S-uniform S (9 and 6 respectively) → NEM₂ exact formula applies

### Cross-check table

| Graph | NZ₀ (ppm) | NEM₂ | NSe (ppm) | edges | nodes |
|-------|-----------|------|-----------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| K₂ | 2_000_000 | 0 | 2_000_000 | 1 | 2 |
| P₃ | 2_121_318 | 16 | 4_242_639 | 2 | 3 |
| K₃ | 1_500_000 | 288 | 6_000_000 | 3 | 3 |
| K_{1,4} | 2_500_000 | 384 | 10_000_000 | 4 | 5 |
| P₄ | 2_568_912 | 72 | 6_292_526 | 3 | 4 |
| K₄ | 1_333_332 | 7_776 | 12_000_000 | 6 | 4 |
| K_{2,3} | 2_041_240 | 2_160 | 12_247_445 | 6 | 5 |

### OS analogy

- **NZ₀** = inverse-square-root neighborhood load (high = many low-load routing nodes; max for K₂=star-of-stars)
- **NEM₂** = S-reformulated second Zagreb pressure (0 for K₂-type edges; amplifies high-S edges quadratically × sum−2)
- **NSe** = square-root neighborhood load sum (complement to NF=cubic; moderate growth relative to S)

---

## Fix: topo28 k-shell gap (V3.39 retroactive)

V3.39 added `gos_runtime::graph_topo_indices28()` and `gos-graph-topo28-harness` but omitted the k-shell display dispatch and proc.rs routing. This commit adds:

- `crates/k-shell/src/lib.rs`: `dispatch_graph_topo_indices28()` — NNI/NNMI/NSM1 display with bright-yellow header, bright-cyan NNI (ppm), bright-green NNMI (ppm), bright-magenta NSM1 (exact)
- `crates/k-shell/src/proc.rs`: routing for `graph topo28 / gtopo28 / neighborhood nirmala / gnni / neighborhood modified nirmala / gnnmi / gnsm1 / gnnigsm1 / gnninnminsm1`

---

## VectorAddress L4 namespace (updated)

..., 114=graph-topo27, 115=graph-topo28, **116=graph-topo29**

---

## Files changed

- `crates/gos-runtime/src/lib.rs` — `graph_topo_indices29_inner` + `graph_topo_indices29()`
- `crates/k-shell/src/lib.rs` — `dispatch_graph_topo_indices28` (fix) + `dispatch_graph_topo_indices29` (new)
- `crates/k-shell/src/proc.rs` — routing for topo28 (fix) + topo29 (new)
- `host-tests/gos-graph-topo29-harness/` — new harness (10 tests, all green)
- `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-16_V3.40.md` — this file
