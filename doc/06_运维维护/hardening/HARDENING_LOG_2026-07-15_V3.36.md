# Hardening Log — V3.36 (2026-07-15)

**Branch**: feat/vk-auto-live-surface  
**Commit**: feat(v3.36): NHM₂ + NAG + NABS Neighborhood S-variant indices + gos-graph-topo25-harness (10 tests)

---

## Summary

Added three new Neighborhood S-variant topological indices continuing the S-family introduced in V3.29–V3.35.  
This version adds S-analogues of the Hyper-Second Zagreb Index (HM₂), Arithmetic-Geometric ratio (AG), and Atom-Bond Sum Connectivity (ABS).

---

## New Functionality

### `gos_runtime::graph_topo_indices25() -> (nhm2: u64, nag_ppm: u64, nabs_ppm: u64, edge_count: usize, node_count: usize)`

**S(v) = Σ_{w∈N(v)} deg(w)** — neighbor-degree sum (same S as topo18/topo21–topo25 family)

| Index | Formula | Scale | Reference |
|-------|---------|-------|-----------|
| NHM₂ | Σ_{uv∈E} (S_u·S_v)² | exact u64 | S-analogue of HM₂ (Das & Trinajstić 2011) |
| NAG  | Σ_{uv∈E} (S_u+S_v)/(2√(S_u·S_v)) | floor ppm | S-analogue of AG ratio (Zheng et al. 2020) |
| NABS | Σ_{uv∈E} √((S_u+S_v−2)/(S_u+S_v)) | floor ppm | S-analogue of ABS (Chen et al. 2022) |

**Key invariants:**
- NAG ≥ |E|×10⁶ always (AM≥GM for S_u, S_v ≥ 1); equal iff S-uniform (every edge has S_u=S_v)
- NABS = 0 only when S_u+S_v=2 for every edge (only K₂: both S=1)
- K₃ and K_{1,4} coincide on NAG and NABS per edge (both S-uniform S=4; ssum=8, sp=16)
- K₄ (S=9) and K_{2,3} (S=6) both give NAG=|E|×10⁶=6_000_000 (S-uniform, |E|=6), but NHM₂ and NABS differ

**Implementation formulas (no float, no_std):**
- NHM₂ per edge: `(sp as u128) * (sp as u128)` where sp=S_u·S_v; u128 accumulator → cast u64
- NAG per edge: `floor(ssum·10¹² / (2·isqrt128(sp·10¹²)))`
  — sp·10¹² can reach ~2.6×10²⁰ for max-degree graphs → u128 required for isqrt128 argument
- NABS per edge: `isqrt64((ssum-2)·10¹² / ssum)`
  — (ssum-2)·10¹² ≤ 32256·10¹² ≈ 3.2×10¹⁶ < u64::MAX ✓; natural 0 when ssum=2

**Overflow safety:**
- NHM₂: sp=S_u·S_v ≤ 16129²=260_144_641; sp²≤6.77×10¹⁶ < u64::MAX per edge ✓; u128 accumulator for sum
- NAG: ssum·10¹² ≤ 32258·10¹² ≈ 3.2×10¹⁶ < u64::MAX ✓; sp·10¹² uses u128 (can exceed u64::MAX)
- NABS: (ssum-2)·10¹² ≤ 3.2×10¹⁶ < u64::MAX ✓

**Algorithm:** O(V+E) — adj+deg pass → S(v) pass → edge scan; no BFS needed

**Cross-check table:**

| Graph | NHM₂ | NAG (ppm) | NABS (ppm) | edges | nodes |
|-------|------|-----------|-----------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| K₂ | 1 | 1_000_000 | 0 | 1 | 2 |
| P₃ | 32 | 2_000_000 | 1_414_212 | 2 | 3 |
| K₃ | 768 | 3_000_000 | 2_598_075 | 3 | 3 |
| K_{1,4} | 1024 | 4_000_000 | 3_464_100 | 4 | 5 |
| P₄ | 153 | 3_041_242 | 2_365_688 | 3 | 4 |
| K₄ | 39366 | 6_000_000 | 5_656_854 | 6 | 4 |
| K_{2,3} | 7776 | 6_000_000 | 5_477_220 | 6 | 5 |

Note: K₄ NABS uses `isqrt64(888_888_888_888)=942_809` (verified: 942_809²=888_888_810_481 ≤ target < 942_810²).

**Shell commands:** `graph topo25` / `gtopo25` / `neighborhood hm2` / `gnhm2` / `neighborhood ag` / `gnag` / `neighborhood abs` / `gnabs` / `gnhm2nagnabs`

**VectorAddress L4=112** for gos-graph-topo25-harness

**OS analogy:**
- NHM₂ = squared S-product coupling intensity (amplifies hub-to-hub S-weight; (S_u·S_v)²=0 only for empty)
- NAG  = S-arithmetic-geometric channel balance (=|E| for S-uniform; >|E| for mixed S, by AM≥GM)
- NABS = S-atom-bond-sum breadth ratio (0 for K₂ topology; increases with S-excess above threshold)

**Display:** bright-yellow header; NHM₂ bright-cyan (exact); NAG bright-green (ppm + "≡|E| (S-regular)" annotation); NABS bright-magenta (ppm + "NABS=0: all S₁+S₂=2" annotation)

**Footer:** "Das & Trinajstić 2011  Zheng et al. 2020  Chen et al. 2022  (S-variant family)"

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices25_inner()` + `graph_topo_indices25()` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices25()` |
| `crates/k-shell/src/proc.rs` | Added routing for topo25 commands |
| `host-tests/gos-graph-topo25-harness/` | New harness crate (10 tests, VectorAddress L4=112) |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.36.md` | This log |

---

## Test Results

```
running 10 tests
test test_01_empty ... ok
test test_02_single_node ... ok
test test_03_single_edge ... ok
test test_04_path_p3 ... ok
test test_05_triangle_k3 ... ok
test test_06_star_k14 ... ok
test test_07_path_p4 ... ok
test test_08_complete_k4 ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 filtered out; 0 measured
```

**Host-test suite total: 1333 tests** (1323 through V3.35 + 10 new)

---

## VectorAddress L4 Namespace (updated)

88=graph-topo, 89=graph-topo2, 90=graph-topo3, 91=graph-topo4, 92=graph-topo5,  
93=graph-topo6, 94=graph-topo7, 95=graph-topo8, 96=graph-topo9, 97=graph-topo10,  
98=graph-topo11, 99=graph-topo12, 100=graph-topo13, 101=graph-topo14, 102=graph-topo15,  
103=graph-topo16, 104=graph-topo17, 105=graph-topo18, 106=graph-topo19, 107=graph-topo20,  
108=graph-topo21, 109=graph-topo22, 110=graph-topo23, 111=graph-topo24, **112=graph-topo25**
