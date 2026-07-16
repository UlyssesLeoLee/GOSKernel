> **[归位说明 / 2026-07-15]** 本文件为原始英文存档，未做删改。经审校已归位并中文化至 [doc/06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.16.md](06_运维维护/hardening/HARDENING_LOG_2026-07-06_V3.16.md)，请以该中文版为准。

# GOS Hardening Log — V3.16 (2026-07-06)

## Summary

**feat(v3.16): HM₁ + HM₂ + AG topological indices + gos-graph-topo5-harness (10 tests)**

Adds three well-established degree-based topological indices as `gos_runtime::graph_topo_indices5()`,
continuing the V3.11–V3.15 topological-index series. Host-test suite now stands at **1133 tests**.

---

## New Algorithms

### graph_topo_indices5() → (hm1: u64, hm2: u64, ag_ppm: u64, edge_count: usize, node_count: usize)

**HM₁ — First Hyper-Zagreb index** (Shirdel, Rezapour & Sayadi 2013)
- HM₁(G) = Σ_{uv∈E} (d(u) + d(v))²
- Exact integer; contribution = s² where s = d_u + d_v
- HM₁ = 4·|E|·Δ² for any Δ-regular graph (since s = 2Δ → s² = 4Δ²)
- K₃ (Δ=2): 3×16 = 48; K₄ (Δ=3): 6×36 = 216 ✓

**HM₂ — Second Hyper-Zagreb index** (Das & Trinajstić 2011)
- HM₂(G) = Σ_{uv∈E} (d(u) × d(v))²
- Exact integer; contribution = p² where p = d_u × d_v
- HM₂ = |E|·Δ⁴ for any Δ-regular graph (since p = Δ² → p² = Δ⁴)
- K₃ (Δ=2): 3×16 = 48; K₄ (Δ=3): 6×81 = 486 ✓

**AG — Arithmetic-Geometric index** (Zheng, Li & Liu 2020)
- AG(G) = Σ_{uv∈E} (d(u) + d(v)) / (2√(d(u)·d(v)))
- ag_ppm = floor(s × 10¹² / (2 × isqrt64(p × 10¹²))) per edge, accumulated
- KEY INVARIANT: AG = |E| iff graph is regular (AM-GM equality: d_u = d_v → AM = GM = d_u)
- AG ≥ |E| always (AM ≥ GM with equality iff d_u = d_v for every edge)
- This is the multiplicative dual of GA (already implemented in V3.12 as `ga_ppm`)

### Integer Precision

| Index | Per-edge computation         | Error bound       |
|-------|------------------------------|-------------------|
| HM₁   | s² (exact u64)               | exact             |
| HM₂   | p² (exact u64)               | exact             |
| AG    | floor(s·10¹²/(2·isqrt64(p·10¹²))) | ≤1 ppm/edge |

### Overflow bounds (MAX_NODES=128, MAX_EDGES=512)
- HM₁: s ≤ 254; s² ≤ 64516; × 512 edges ≈ 33M → well within u64
- HM₂: p ≤ 127² = 16129; p² ≤ 260M; × 512 ≈ 133B → well within u64
- AG: per edge ≤ ~1.25×10^6 (worst case asymmetric); × 512 ≈ 640M → well within u64

---

## Analytical Cross-Check Table

| Graph        | HM₁  | HM₂ | AG_ppm    | edges | notes                     |
|--------------|------|-----|-----------|-------|---------------------------|
| Empty        |    0 |   0 |         0 |     0 |                           |
| Edge A-B     |    4 |   1 | 1_000_000 |     1 | da=db=1; regular (AG=m)   |
| Path P₃      |   18 |   8 | 2_121_320 |     2 | s=3,p=2 per edge          |
| Triangle K₃  |   48 |  48 | 3_000_000 |     3 | Δ=2 regular; AG=m=3       |
| Star K_{1,4} |  100 |  64 | 5_000_000 |     4 | s=5,p=4; (4+1)/(2√4)=5/4 exact |
| Path P₄      |   34 |  24 | 3_121_320 |     3 | mixed edges               |
| Complete K₄  |  216 | 486 | 6_000_000 |     6 | Δ=3 regular; AG=m=6       |
| K_{2,3}      |  150 | 216 | 6_123_726 |     6 | s=5,p=6; 6×1_020_621      |

### Critical precision note for K_{2,3}
- isqrt64(6×10¹²) = 2_449_489 (floor of √6 × 10^6 = 2_449_489.742...)
- 2x = 4_898_978
- floor(5×10¹² / 4_898_978) = 1_020_621  ← NOT 1_020_620
  - 4_898_978 × 1_020_621 = 4_999_999_825_338 < 5×10¹² ✓
  - 4_898_978 × 1_020_622 = 5_000_004_724_316 > 5×10¹² ✓
- Total = 6 × 1_020_621 = 6_123_726

---

## Shell Commands

| Command                                | Routes to         |
|----------------------------------------|-------------------|
| `graph topo5`                          | topo5 dispatch    |
| `gtopo5`                               | topo5 dispatch    |
| `hyper zagreb`                         | topo5 dispatch    |
| `ghm1`                                 | topo5 dispatch    |
| `hm2 index`                            | topo5 dispatch    |
| `ghm2`                                 | topo5 dispatch    |
| `arithmetic geometric`                 | topo5 dispatch    |
| `gag`                                  | topo5 dispatch    |
| `ghm1hm2ag`                            | topo5 dispatch    |

---

## Display Format

```
 graph topo5  (HM₁ + HM₂ + AG degree-based indices)
 ───────────────────────────────────────────────────────────
  hyper-zagreb 1st   HM₁=  48        [Σ (d+d)²]
  hyper-zagreb 2nd   HM₂=  48        [Σ (d·d)²]
  arith-geo index    AG  =  3.000     [Σ (d+d)/(2√d·d)]  (regular: AG=m)
 ───────────────────────────────────────────────────────────
3 node(s)  3 edge(s)  Shirdel et al. 2013  Das & Trinajstić 2011  Zheng et al. 2020
```

- Header: bright-yellow (color 14)
- HM₁: bright-cyan (color 11)
- HM₂: bright-green (color 10)
- AG: bright-magenta (color 13); `(regular: AG=m)` annotation in bright-green when AG = m × 10^6

---

## VectorAddress Namespace (updated)

```
88=graph-topo   (V3.12 SC+GA+AZI)
89=graph-topo2  (V3.13 H+ABC+F)
90=graph-topo3  (V3.14 SDD+ISI+NI)
91=graph-topo4  (V3.15 SO+RM₂+σ)
92=graph-topo5  (V3.16 HM₁+HM₂+AG)
```

---

## Test Results

**gos-graph-topo5-harness: 10/10 tests pass**

```
test test_01_empty           ... ok
test test_02_single_node     ... ok
test test_03_single_edge     ... ok  (regular: AG=1.0=|E|×1.0)
test test_04_path_p3         ... ok  (non-regular: AG>m)
test test_05_triangle_k3     ... ok  (regular invariants: HM1=4|E|Δ², HM2=|E|Δ⁴, AG=m)
test test_06_star_k14        ... ok  (exact AG=5.0; (4+1)/(2√4)=5/4)
test test_07_path_p4         ... ok  (mixed edges; inner edge B-C regular)
test test_08_complete_k4     ... ok  (regular invariants; x=3_000_000 exact)
test test_09_two_isolated    ... ok  (no edges)
test test_10_k23_cross_check ... ok  (precision: 6×1_020_621=6_123_726)
```

**Cumulative host-test suite: 1133 tests** (was 1123 after V3.15)

---

## OS Analogy

- **HM₁**: Aggregate squared-sum coupling pressure — amplifies hub-concentrated topologies (high HM₁/|E| → few highly-connected gateway nodes dominating the IPC graph)
- **HM₂**: Aggregate squared-product hub density — measures co-hub coupling intensity; HM₂/|E| = Δ⁴ for regular grids, spikes for hub-spoke topologies
- **AG**: Arithmetic-Geometric ratio index — measures degree asymmetry across channels; AG = |E| for balanced mesh (like NUMA uniform-access domains); AG > |E| signals asymmetric spoke-hub IPC (like I/O hub vs compute nodes)

---

## Literature

- Shirdel, Rezapour & Sayadi (2013): "The hyper-Zagreb index of graph operations". *Iranian J. Math. Chem.*
- Das & Trinajstić (2011): "Relationship between the Eccentric Connectivity Index and Zagreb Indices". *Computers & Math. with Applications*
- Zheng, Li & Liu (2020): "New bounds on the arithmetic-geometric index". *J. Math. Chem.*
