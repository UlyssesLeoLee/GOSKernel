# Hardening Log — V3.35 (2026-07-15)

**Branch**: feat/vk-auto-live-surface  
**Commit**: feat(v3.35): NISI + NAZI + NEM1 Neighborhood S-variant indices + gos-graph-topo24-harness (10 tests)

---

## Summary

Added three new Neighborhood S-variant topological indices continuing the S-family introduced in V3.29–V3.34.  
This version adds S-analogues of the Inverse Sum Indegree (ISI), Augmented Zagreb Index (AZI), and Reformulated First Zagreb Index (EM₁).

---

## New Functionality

### `gos_runtime::graph_topo_indices24() -> (nisi_ppm: u64, nazi_milli: u64, nem1: u64, edge_count: usize, node_count: usize)`

**S(v) = Σ_{w∈N(v)} deg(w)** — neighbor-degree sum (same S as topo18/topo21–topo24 family)

| Index | Formula | Scale | Reference |
|-------|---------|-------|-----------|
| NISI | Σ_{uv∈E} S_u·S_v/(S_u+S_v) | floor ppm | S-analogue of ISI (Sedlar et al. 2011) |
| NAZI | Σ_{uv∈E} (S_u·S_v/(S_u+S_v−2))³ | floor milli | S-analogue of AZI (Furtula et al. 2010) |
| NEM1 | Σ_{uv∈E} (S_u+S_v−2)² | exact u64 | S-analogue of EM₁ (Milićević et al. 2004) |

**Key invariants:**
- NISI = |E|×S/2×10⁶ for S-regular graphs (S-uniform)
- NAZI = 0 when every edge has S_u+S_v=2 (only K₂ type: S=1 both endpoints)
- NEM1 = 0 iff (S_u+S_v=2) for all edges
- K₃ and K_{1,4} share same per-edge values (both S-uniform S=4, same ssum=8, sp=16, q=6)
- K₄ (S=9) and K_{2,3} (S=6) give DIFFERENT values (unlike some prior S-family indices)

**Overflow safety:**
- NISI: S_u·S_v·10⁶ ≤ 16129²×10⁶ ≈ 2.6×10¹⁷ < u64::MAX ✓
- NAZI: (S_u·S_v)³ needs u128 intermediate; per-edge result after division ≤ ~5.24×10¹⁴ fits u64 ✓
- NEM1: (ssum−2)² ≤ 32256² ≈ 10⁹ per edge × 8065 ≈ 8×10¹² < u64::MAX ✓

**Algorithm:** O(V+E) — adj+deg pass → S(v) pass → edge scan; no BFS needed

**Cross-check table:**

| Graph | NISI (ppm) | NAZI (milli) | NEM1 | edges | nodes |
|-------|-----------|-------------|------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| K₂ | 500_000 | 0 | 0 | 1 | 2 |
| P₃ | 2_000_000 | 16_000 | 8 | 2 | 3 |
| K₃ | 6_000_000 | 56_886 | 108 | 3 | 3 |
| K_{1,4} | 8_000_000 | 75_848 | 144 | 4 | 5 |
| P₄ | 3_900_000 | 27_390 | 34 | 3 | 4 |
| K₄ | 27_000_000 | 778_476 | 1_536 | 6 | 4 |
| K_{2,3} | 18_000_000 | 279_936 | 600 | 6 | 5 |

**Shell commands:** `graph topo24` / `gtopo24` / `neighborhood isi` / `gnisi` / `neighborhood azi` / `gnazi` / `neighborhood em1` / `gnem1` / `gnisinazinemm1`

**VectorAddress L4=111** for gos-graph-topo24-harness

**OS analogy:**
- NISI = S-harmonic coupling intensity per channel (S-uniform = |E|×S/2; balanced load)
- NAZI = S-augmented bond pressure cubed (0 for pendant-pair leaf topology; high for dense hubs)
- NEM1 = squared S-excess per channel (0 for K₂-type; measures S-surplus above threshold 2)

**Display:** bright-yellow header; NISI bright-cyan (ppm); NAZI bright-green (milli + "NAZI=0: all pendant-pair" annotation); NEM1 bright-magenta (exact + "NEM1=0: all S₁+S₂=2" annotation)

**Footer:** "Sedlar et al. 2011  Furtula et al. 2010  Milicevic et al. 2004  (S-variant family)"

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices24_inner()` + `graph_topo_indices24()` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices24()` |
| `crates/k-shell/src/proc.rs` | Added routing for topo24 commands |
| `host-tests/gos-graph-topo24-harness/` | New harness crate (10 tests, VectorAddress L4=111) |
| `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-15_V3.35.md` | This log |

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

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Host-test suite total: 1323 tests** (1313 through V3.34 + 10 new)

---

## VectorAddress L4 Namespace (updated)

88=graph-topo, 89=graph-topo2, 90=graph-topo3, 91=graph-topo4, 92=graph-topo5,  
93=graph-topo6, 94=graph-topo7, 95=graph-topo8, 96=graph-topo9, 97=graph-topo10,  
98=graph-topo11, 99=graph-topo12, 100=graph-topo13, 101=graph-topo14, 102=graph-topo15,  
103=graph-topo16, 104=graph-topo17, 105=graph-topo18, 106=graph-topo19, 107=graph-topo20,  
108=graph-topo21, 109=graph-topo22, 110=graph-topo23, **111=graph-topo24**
