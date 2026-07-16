> **[归位说明 / 2026-07-15]** 本文件为原始英文存档，未做删改。经审校已归位并中文化至 [doc/06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.17.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.17.md)，请以该中文版为准。

# Hardening Log — V3.17
**Date:** 2026-07-06  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.17): EM1 + ABS + RRR topological indices + gos-graph-topo6-harness (10 tests)

---

## Summary

Added three new degree-based topological indices to `gos_runtime`: **EM₁** (Reformulated First Zagreb), **ABS** (Atom-Bond Sum Connectivity), and **RRR** (Reduced Reciprocal Randić). These extend the topological index library to 6 groups (V3.12–V3.17), covering 18+ indices total.

---

## New Algorithms

### `graph_topo_indices6()` → `(em1: u64, abs_ppm: u64, rrr_ppm: u64, edge_count: usize, node_count: usize)`

**EM₁ (Reformulated First Zagreb)**  
- Formula: EM₁(G) = Σ_{uv∈E} (dₐ+d_b-2)²  
- Reference: Milićević, Nikolić, Trinajstić & Tolić-Stipčević (2004)  
- Computation: contribution = q² where q = dₐ+d_b-2; exact integer always  
- Invariant: EM₁ = 4m(Δ-1)² for Δ-regular graphs  
- EM₁ = 0 when q=0 (pendant-pair edges, dₐ=d_b=1)

**ABS (Atom-Bond Sum Connectivity)**  
- Formula: ABS(G) = Σ_{uv∈E} √((dₐ+d_b-2)/(dₐ+d_b))  
- Reference: Chen et al. (2022)  
- Computation: isqrt64(q × 10¹²/s) per edge (floor error ≤ 1 ppm)  
- ABS = m·√((Δ-1)/Δ) for Δ-regular graphs  
- ABS = 0 when q=0 (pendant pair) — naturally gives 0 without special case

**RRR (Reduced Reciprocal Randić)**  
- Formula: RRR(G) = Σ_{uv∈E} √((dₐ-1)(d_b-1))  
- Reference: Li & Shi (2008)  
- Computation: isqrt64((dₐ-1)·(d_b-1)·10¹²) per edge (floor error ≤ 1 ppm)  
- Invariant: RRR = m(Δ-1)×10⁶ for Δ-regular (exact: isqrt((Δ-1)²) = Δ-1)  
- RRR = 0 iff all edges are pendant (dₐ=1 or d_b=1)

---

## Cross-Check Table

| Graph | EM₁ | ABS_ppm | RRR_ppm | |E| | |V| |
|-------|-----|---------|---------|-----|-----|
| Empty | 0 | 0 | 0 | 0 | 0 |
| Single node | 0 | 0 | 0 | 0 | 1 |
| Edge A→B (da=db=1) | 0 | 0 | 0 | 1 | 2 |
| Path P₃ | 2 | 1_154_700 | 0 | 2 | 3 |
| Triangle K₃ (Δ=2) | 12 | 2_121_318 | 3_000_000 | 3 | 3 |
| Star K_{1,4} | 36 | 3_098_384 | 0 | 4 | 5 |
| Path P₄ | 6 | 1_861_806 | 1_000_000 | 3 | 4 |
| Complete K₄ (Δ=3) | 96 | 4_898_976 | 12_000_000 | 6 | 4 |
| Two isolated | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 54 | 4_647_576 | 8_485_278 | 6 | 5 |

### Key isqrt64 Values
- isqrt64(333_333_333_333) = 577_350 (√(1/3) × 10⁶; s=3,q=1 per-edge)
- isqrt64(500_000_000_000) = 707_106 (√(1/2) × 10⁶; s=4,q=2 per-edge)
- isqrt64(600_000_000_000) = 774_596 (√(3/5) × 10⁶; s=5,q=3 per-edge)
- isqrt64(666_666_666_666) = 816_496 (√(2/3) × 10⁶; s=6,q=4 per-edge)
- isqrt64(1_000_000_000_000) = 1_000_000 (√1 × 10⁶; (da-1)(db-1)=1)
- isqrt64(2_000_000_000_000) = 1_414_213 (√2 × 10⁶; (da-1)(db-1)=2)
- isqrt64(4_000_000_000_000) = 2_000_000 (exact: √4=2; (da-1)(db-1)=4)

---

## Implementation Details

**Algorithm** (`graph_topo_indices6_inner`): O(V+E) single undirected edge scan.
- All three indices computed in one pass (same a < b canonical dedup as prior indices)
- No special-case branches needed: q=0 or p1/p2=0 give isqrt64(0)=0 naturally
- EM₁ is exact integer accumulation; ABS and RRR use Newton-Raphson isqrt64

**Overflow safety:**
- EM₁ contribution: q² ≤ 252² = 63504; sum of |E| ≤ 512 terms: max ~32M → fits u64
- ABS numerator: q×10¹² ≤ 252×10¹² < u64::MAX ✓
- RRR: p1×p2×10¹² ≤ 126×126×10¹² = 1.59×10¹⁶ < u64::MAX ✓

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices6_inner` method + `graph_topo_indices6` public fn |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices6` with colored display |
| `crates/k-shell/src/proc.rs` | Added routing for "graph topo6"/"gtopo6"/8 aliases |
| `host-tests/gos-graph-topo6-harness/` | New harness: Cargo.toml + .cargo/config.toml + Cargo.lock + 10 tests |

---

## Shell Commands

```
graph topo6              (primary)
gtopo6                   (short alias)
reformulated zagreb      (EM₁ by name)
gem1                     (EM₁ index)
atom bond sum            (ABS by name)
gabs                     (ABS index)
reduced reciprocal randic (RRR by name)
grrr                     (RRR index)
gem1absrrr               (combined)
```

---

## OS Analogy

- **EM₁**: Squared excess coupling pressure per IPC channel — measures how far above the pendant threshold hub connections are; 0 for all-leaf topologies, grows as hub-degree increases
- **ABS**: Atom-bond coupling breadth ratio per channel — normalized indicator of asymmetric link utilization; approaches √(1/2) per edge for high-degree meshes
- **RRR**: Interior coupling geometric density — 0 for any graph where every edge touches a leaf node; equals m(Δ-1)×10⁶ for meshes (exact); measures "depth" of internal connectivity past the leaf layer

---

## Test Results

```
test test_01_empty         ... ok
test test_02_single_node   ... ok
test test_03_single_edge   ... ok
test test_04_path_p3       ... ok
test test_05_triangle_k3   ... ok
test test_06_star_k14      ... ok
test test_07_path_p4       ... ok
test test_08_complete_k4   ... ok
test test_09_two_isolated  ... ok
test test_10_k23_cross_check ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

**Host test suite total: 1143 tests** (+10 from V3.16's 1133)

---

## VectorAddress L4 Namespace (Updated)

```
88=graph-topo   (SC + GA + AZI, V3.12)
89=graph-topo2  (H + ABC + F, V3.13)
90=graph-topo3  (SDD + ISI + NI, V3.14)
91=graph-topo4  (SO + RM2 + Sigma, V3.15)
92=graph-topo5  (HM1 + HM2 + AG, V3.16)
93=graph-topo6  (EM1 + ABS + RRR, V3.17)  ← new
```
