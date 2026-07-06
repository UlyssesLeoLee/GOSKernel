# GOSKernel Hardening Log — V3.13
**Date:** 2026-07-06  
**Algorithm:** Harmonic Index H, Atom-Bond Connectivity ABC, Forgotten Index F  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.13): H + ABC + F topological indices + gos-graph-topo2-harness (10 tests)

---

## Summary

V3.13 adds **three degree-based topological indices** to the GOSKernel graph theory runtime:

| Index | Formula | Literature |
|-------|---------|------------|
| **H(G)** — Harmonic index | Σ_{uv∈E} 2/(deg(u)+deg(v)) | Zhong 2012 |
| **ABC(G)** — Atom-Bond Connectivity | Σ_{uv∈E} √((deg(u)+deg(v)−2)/(deg(u)·deg(v))) | Estrada et al. 2008 |
| **F(G)** — Forgotten topological index | Σ_v deg(v)³  (exact integer) | Furtula & Gutman 2015 |

These indices form the "second wave" of degree-based descriptors in chemical graph theory.
H is the harmonic mean analog of Randić connectivity; ABC was originally derived from molecular
energy modelling (transition states); F was "forgotten" in the Zagreb index literature until
rediscovered in 2015 and shown to encode complementary information to M2.

All three are computed in **O(V+E)** with integer arithmetic — no float, no heap, no_std safe.

**OS analogies:**
- **H** = channel load distribution (uniform IPC = H = |E|/Δ for regular graphs)
- **ABC** = bond-weakness index; high ABC = many edges connecting unequal-degree nodes (fragile IPC topology)
- **F** = cubic coupling pressure (amplifies hub skew more aggressively than Zagreb M1's squared degrees)

---

## Public API

### `gos_runtime::graph_topo_indices2() -> (u64, u64, u64, usize, usize)`

Returns `(h_ppm, abc_ppm, f_index, edge_count, node_count)`:

- `h_ppm` — H(G) × 10^6 where H = Σ_{uv∈E} 2/(deg(u)+deg(v))  (Zhong 2012)
- `abc_ppm` — ABC(G) × 10^6 where ABC = Σ_{uv∈E} √((s−2)/p), s=deg-sum, p=deg-product  (Estrada et al. 2008)
- `f_index` — F(G) = Σ_v deg(v)³  (exact integer, Furtula & Gutman 2015)
- `edge_count` — undirected edge count (directed→undirected dedup, self-loops excluded)
- `node_count` — live node count

**Shell keywords:** `graph topo2` / `gtopo2` / `harmonic index` / `gh index` / `atom bond connectivity` / `gabc` / `forgotten index` / `gforgotten` / `ghabcf`  
**VectorAddress L4=89** for gos-graph-topo2-harness.

---

## Algorithm

All three indices share the O(V+E) undirected edge scan pattern:

```rust
// H contribution per edge: floor(2_000_000 / (da + db))
h_acc += 2_000_000 / s;

// ABC contribution per edge: isqrt64((s-2) * 10^12 / p)
// where isqrt64(n) = floor(sqrt(n)) via Newton-Raphson
// Pendant-pendant edges (s=2, da=db=1): contribution = 0
if s > 2 && p > 0 {
    let numer = (s - 2).saturating_mul(1_000_000_000_000u64);
    abc_acc += isqrt64(numer / p);
}

// F index: separate node scan after edge scan
let mut f_index: u64 = 0;
for ci in 0..nc {
    f_index += deg[ci] * deg[ci] * deg[ci];
}
```

### Integer Precision

**H:** contribution = floor(2_000_000 / s). Error ≤ 1 ppm per edge. Exact when s divides 2_000_000 (e.g., s=5 for K_{2,3} → H=12/5 exact).

**ABC:** contribution = floor(√((s−2) × 10^12 / p)).
- Computes floor(√((s−2)/p) × 10^6) via integer Newton-Raphson isqrt64.
- Max overflow check: (s−2) ≤ 254 (MAX_NODES−1+MAX_NODES−1−2); 254 × 10^12 < 2^64 ✓
- **Critical precision result:** isqrt64(500_000_000_000) = **707_106** (not 707_107).
  - 707_106² = 499_998_895_236 < 5×10^11 ✓
  - 707_107² = 500_000_309_449 > 5×10^11 ✗
  - All edges with ratio (s−2)/p = 1/2 (P₃, K₃, P₄, K_{2,3}) yield ABC = 707_106 per edge.

**F:** Exact integer — Σ_v deg(v)³. Max value: 128 × 127³ ≈ 262 M, fits u64.

---

## Key Invariants and Cross-Checks

| Graph | H_ppm | ABC_ppm | F_index | Notes |
|-------|-------|---------|---------|-------|
| Empty | 0 | 0 | 0 | — |
| Single node | 0 | 0 | 0 | deg=0 → F=0 |
| Edge A-B | 1_000_000 | 0 | 2 | H=1 exact; ABC=0 (s-2=0); F=1+1=2 |
| P₃ | 1_333_332 | 1_414_212 | 10 | H=4/3; ABC=2×707_106; F=1+8+1 |
| K₃ | 1_500_000 | 2_121_318 | 24 | H=3/2 exact; ABC=3×707_106; F=3×8 |
| K_{1,4} star | 1_600_000 | 3_464_100 | 68 | H=8/5 exact; ABC=4×866_025; F=64+4 |
| P₄ | 1_833_332 | 2_121_318 | 18 | H mix; ABC=3×707_106 (all edges ratio 1/2) |
| K₄ | 1_999_998 | 3_999_996 | 108 | H≈2 floor; ABC=6×666_666; F=4×27 |
| K_{2,3} | 2_400_000 | 4_242_636 | 78 | H exact (s=5); ABC=6×707_106; F=2×27+3×8 |

**H regular-graph invariant:** For Δ-regular graphs, H = |E| / Δ (exact when s divides 2_000_000).
- K₃: H = 3/2, H_ppm = 1_500_000 = 3 × 10^6 / 2 ✓
- K₄: H = 6/3 = 2, H_ppm ≈ 1_999_998 (floor: 6 × 333_333) — off by 2 ppm

**ABC pendant invariant:** Edges where da=db=1 (s=2): (s−2)/p = 0 → ABC contribution = 0 (same skip rule as AZI).

**ABC ratio invariant:** Edges from P₃, K₃, P₄, K_{2,3} all have (s−2)/p = 1/2 → 707_106 ppm each. This is a cross-graph numerical coincidence arising from different degree pairs sharing the same ratio:
- P₃ outer (1,2): (1+2−2)/(1×2) = 1/2
- K₃ (2,2): (2+2−2)/(2×2) = 2/4 = 1/2
- P₄ inner (2,2): same = 1/2
- K_{2,3} (3,2): (3+2−2)/(3×2) = 3/6 = 1/2

---

## Test Suite (gos-graph-topo2-harness)

10 host tests, all green:

| # | Graph | H_ppm | ABC_ppm | F | ec | nc |
|---|-------|-------|---------|---|----|----|
| 1 | Empty | 0 | 0 | 0 | 0 | 0 |
| 2 | 1 node | 0 | 0 | 0 | 0 | 1 |
| 3 | Edge A-B | 1_000_000 | 0 | 2 | 1 | 2 |
| 4 | P₃ | 1_333_332 | 1_414_212 | 10 | 2 | 3 |
| 5 | K₃ | 1_500_000 | 2_121_318 | 24 | 3 | 3 |
| 6 | K_{1,4} | 1_600_000 | 3_464_100 | 68 | 4 | 5 |
| 7 | P₄ | 1_833_332 | 2_121_318 | 18 | 3 | 4 |
| 8 | K₄ | 1_999_998 | 3_999_996 | 108 | 6 | 4 |
| 9 | 2 isolated | 0 | 0 | 0 | 0 | 2 |
| 10 | K_{2,3} | 2_400_000 | 4_242_636 | 78 | 6 | 5 |

Test 10 includes two cross-check assertions:
1. `H exact`: `h == 6 * 2_000_000 / 5 = 2_400_000` (s=5 divides 2_000_000 exactly)
2. `ABC ratio`: `abc == 6 * 707_106` (all edges at ratio (s−2)/p = 1/2)

---

## Shell Display

```
 graph topo2  (H + ABC + F degree-based indices)
 ───────────────────────────────────────────────────────────
  harmonic index     H   =  X.XXX   [Σ 2/(deg(u)+deg(v))]
  atom-bond conn     ABC =  X.XXX   [Σ √((d+d−2)/(d·d))]
  forgotten index    F   =  N       [Σ_v deg(v)³]  (exact)
 ───────────────────────────────────────────────────────────
 N node(s)  M edge(s)  Zhong 2012  Estrada et al. 2008  Furtula & Gutman 2015
```

Colors: header bright-yellow (14); H bright-cyan (11); ABC bright-green (10); F bright-magenta (13).

---

## Cumulative Host-Test Count

| Session | Added | Cumulative |
|---------|-------|-----------|
| V3.12 (SC+GA+AZI) | 10 | 1093 |
| **V3.13 (H+ABC+F)** | **10** | **1103** |

---

## Literature

- Zhong L. (2012). "The harmonic index for graphs." *Applied Mathematics Letters*, 25(3):561-566.
- Estrada E., Torres L., Rodríguez L., Gutman I. (2008). "An atom-bond connectivity index: Modelling the enthalpy of formation of alkanes." *Indian Journal of Chemistry*, 47A:711-717.
- Furtula B., Gutman I. (2015). "A forgotten topological index." *Journal of Mathematical Chemistry*, 53(4):1184-1190.
