# GOSKernel Hardening Log — V3.22
**Date:** 2026-07-07  
**Branch:** feat/vk-auto-live-surface  
**Host-test suite total:** 1193 tests (all green)

---

## Summary

V3.22 introduces **transmission-based topological indices** — a set of three metrics derived from vertex transmittances T(v) (the row-sum of the distance matrix within each connected component). These complement V3.21's edge-partition Szeged/Mostar indices and V3.18's pairwise Wiener/Harary indices, completing a family of distance-based structural descriptors. All three indices are computed in a single O(n·(n+m)) BFS pass.

---

## New Feature: `graph topo11` — Balaban J + Transmission Irregularity TI + Vertex PI

### API

```rust
pub fn graph_topo_indices11() -> (u64, u64, u64, usize, usize)
// Returns: (j_ppm, ti, piv, edge_count, node_count)
```

### Indices

| Symbol | Formula | Type | Literature |
|--------|---------|------|-----------|
| J | (m/μ) × Σ_{uv∈E} 1/√(T_u·T_v) | floor ppm (×10⁶) | Balaban 1982 |
| TI | Σ_{uv∈E} \|T_u − T_v\| | exact u64 | Abdo & Dimitrov 2014 |
| PI_v | Σ_{uv∈E} (T_u + T_v) | exact u64 | Khalifeh et al. 2008 |

**T_v (vertex transmittance):** Σ_{w reachable, w≠v} d(v,w) — BFS distance sum within connected component.  
**μ:** max(1, m−n+2) — cyclomatic-number proxy; = 1 for trees; ≥ 2 for unicyclic and denser graphs.  
**Disconnected graphs:** T_v counts only within-component distances (inter-component d = ∞ is excluded from BFS).

### Algorithm

1. Compact node index from node slots.
2. Build undirected adjacency bitmasks (directed→undirected dedup, self-loops excluded).
3. Build undirected edge list (canonical a < b).
4. BFS from each vertex src; accumulate T[src] = Σ_{v≠src, d finite} d(src,v).
5. For each edge {a,b}:
   - J contribution: `isqrt64(10^12 / (T_a × T_b))` — uses identity `floor(A/√B) = floor(√(A²/B))`
   - TI contribution: `|T_a − T_b|`
   - PI_v contribution: `T_a + T_b`
6. J_ppm = j_raw × m / μ.

Stack allocation: `adj[128](u128)` + `trans[128](u64)` + `dist/queue[128](u8)` + edge lists ≈ 4 KB total.  
`isqrt64` uses pure integer Newton-Raphson (no-std safe, no f32/f64).

### Key Invariants

```
K_n (complete, n≥2): T_v = n-1 ∀v; TI = 0 (vertex-transitive)
                     μ = n(n-1)/2 - n + 2 = (n-1)(n-2)/2 + 1
Trees:               μ = 1; J_ppm = m × Σ_e isqrt64(10^12/(T_a·T_b))
Transmission-regular: TI = 0 iff all T_v equal (sufficient but not necessary for vertex-transitive)
PI_v = Σ_v deg(v)·T_v  (equivalent degree-weighted-transmission formula)
Disconnected (no edges): J = TI = PI_v = 0
```

### Cross-Check Table (analytical)

| Graph      | J_ppm     | TI | PI_v | edges | nodes |
|------------|-----------|-----|------|-------|-------|
| Empty      | 0         | 0   | 0    | 0     | 0     |
| 1 node     | 0         | 0   | 0    | 0     | 1     |
| Edge A-B   | 1_000_000 | 0   | 2    | 1     | 2     |
| Path P₃    | 1_632_992 | 2   | 10   | 2     | 3     |
| Triangle K₃| 2_250_000 | 0   | 12   | 3     | 3     |
| Star K_{1,4}| 3_023_712| 12  | 44   | 4     | 5     |
| Path P₄    | 1_974_744 | 4   | 28   | 3     | 4     |
| Complete K₄| 2_999_997 | 0   | 36   | 6     | 4     |
| 2 isolated | 0         | 0   | 0    | 0     | 2     |
| K_{2,3}    | 2_190_888 | 6   | 66   | 6     | 5     |

**Key derivations:**

- **Edge A-B:** T_A=T_B=1; μ=1. j_raw=isqrt64(10^12)=1_000_000; J_ppm=1_000_000 (exact J=1). TI=0. PI_v=2.
- **K₃:** T_u=2 ∀u; μ=2. j_raw=3×500_000=1_500_000; J_ppm=2_250_000 (exact J=9/4). TI=0. PI_v=12.
- **K_{1,4}:** T(center)=4; T(leaf)=7; μ=1. j_raw=4×188_982=755_928; J_ppm=3_023_712 (exact≈3.0237). TI=4×3=12. PI_v=4×11=44.
- **K₄:** T_u=3 ∀u; μ=4. j_raw=6×333_333=1_999_998; J_ppm=2_999_997 (exact J=3; floor error 3 ppm). TI=0. PI_v=36.
- **K_{2,3}:** T(left)=5; T(right)=6; μ=3. j_raw=6×182_574=1_095_444; J_ppm=2_190_888. TI=6 (confirms non-transmission-regular). PI_v=66.

**isqrt64 precision constants:**
```
isqrt64(10^12/1)  = 1_000_000  (exact: 10^6)
isqrt64(10^12/4)  =   500_000  (exact: 5×10^5)
isqrt64(10^12/6)  =   408_248  (floor: √(10^12/6) = 408248.29…)
isqrt64(10^12/9)  =   333_333  (floor: 10^6/3 = 333333.33…)
isqrt64(10^12/16) =   250_000  (exact: 2.5×10^5)
isqrt64(10^12/24) =   204_124  (floor: √(10^12/24) = 204124.14…)
isqrt64(10^12/28) =   188_982  (floor: 10^6/√28 = 188982.23…)
isqrt64(10^12/30) =   182_574  (floor: 10^6/√30 = 182574.18…)
```

### Shell Commands

```
graph topo11           gtopo11         balaban j         gbalaban
transmission irregularity              gti               vertex pi
gpiv                   gjtipiv
```

### OS Analogies

- **J (Balaban connectivity):** Structural compactness score weighted by vertex centrality (transmittance). High J = more branched or tree-like topology with short-distance hubs. Low J = highly cyclic, uniform-load graph. In kernel dep-graphs: J measures how "hub-centric" the dependency structure is relative to its cycle density.
- **TI (Transmission Irregularity):** Measures transmittance imbalance across edges. TI = 0 iff graph is transmission-regular (all vertices have equal sum of distances). TI > 0 indicates asymmetric routing load across IPC channels — edges bridge vertices with unequal "global load." Useful for detecting overloaded gateway subsystems.
- **PI_v (Vertex PI):** Aggregate degree-weighted transmittance = Σ_v deg(v)·T(v). Captures total "routing pressure" weighted by both local connectivity and global reach. A subsystem that is highly connected AND far from others contributes disproportionately to PI_v — ideal candidate for load balancing or caching layers.

### Display

- Bright-yellow header: `graph topo11 (J + TI + PI_v transmission indices)`
- J: bright-cyan (ppm decimal display)
- TI: bright-green; annotates "(TI=0: transmission-regular)" when 0
- PI_v: bright-magenta (exact)
- Footer: `N node(s)  M edge(s)  Balaban 1982  Abdo & Dimitrov 2014  Khalifeh et al. 2008`

---

## Test Harness: `gos-graph-topo11-harness`

**Location:** `host-tests/gos-graph-topo11-harness/`  
**VectorAddress L4:** 98  
**Plugin ID:** `TOPIX_11`

10 tests, all pass:

1. Empty graph → (0, 0, 0, 0, 0)
2. Single node → (0, 0, 0, 0, 1)
3. Single edge A→B → (1_000_000, 0, 2, 1, 2)
4. Path P₃ → (1_632_992, 2, 10, 2, 3)
5. Triangle K₃ → (2_250_000, 0, 12, 3, 3)
6. Star K_{1,4} → (3_023_712, 12, 44, 4, 5)
7. Path P₄ → (1_974_744, 4, 28, 3, 4)
8. Complete K₄ → (2_999_997, 0, 36, 6, 4)
9. Two isolated nodes → (0, 0, 0, 0, 2)
10. K_{2,3} bipartite cross-check → (2_190_888, 6, 66, 6, 5)

---

## VectorAddress L4 Namespace (updated)

| L4 | Harness |
|----|---------|
| 88 | graph-topo (SC/GA/AZI) |
| 89 | graph-topo2 (H/ABC/F) |
| 90 | graph-topo3 (SDD/ISI/NI) |
| 91 | graph-topo4 (Sombor/RM2/sigma) |
| 92 | graph-topo5 (HM1/HM2/AG) |
| 93 | graph-topo6 (EM1/ABS/RRR) |
| 94 | graph-topo7 (W/H/WW) |
| 95 | graph-topo8 (ECI/D/R/avg-ecc) |
| 96 | graph-topo9 (W_S/W_G/CξE) |
| 97 | graph-topo10 (Sz/rSz/Mo) |
| **98** | **graph-topo11 (J/TI/PI_v)** |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | +`graph_topo_indices11_inner()` + `graph_topo_indices11()` export |
| `crates/k-shell/src/lib.rs` | +`dispatch_graph_topo_indices11()` |
| `crates/k-shell/src/proc.rs` | +shell routing for topo11 (9 aliases) |
| `host-tests/gos-graph-topo11-harness/` | new harness (4 files: Cargo.toml, .cargo/config.toml, tests/graph_topo11.rs) |

---

## Metrics

- **New functions:** 2 (inner + public export)
- **New shell aliases:** 9
- **New tests:** 10
- **Cumulative host tests:** 1193
- **Algorithmic category:** Transmission-based distance (BFS all-pairs, O(n·(n+m)))
- **Return type:** 5-tuple `(u64, u64, u64, usize, usize)` — matches topo7/topo9/topo10 pattern
- **Integer arithmetic:** isqrt64 (pure Newton-Raphson, no-std) for J; exact integers for TI and PI_v
