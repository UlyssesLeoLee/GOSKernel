# GOS Hardening Log — V3.25

**Date**: 2026-07-08  
**Branch**: feat/vk-auto-live-surface  
**Commit**: 7da2fb2  
**Session**: Automated scheduled hardening (every 2h)

---

## Summary

V3.25 adds three eccentricity-based topological indices — Total Eccentricity (TE), Eccentric Distance Sum (EDS), and Geometric-Arithmetic Eccentricity (GEA) — along with a new 10-test host harness (`gos-graph-topo14-harness`).

These indices complement the existing eccentricity suite (V3.19: ECI+D+R+avg_ecc; V3.23: M1*+M2*+M3*) by adding the simplest aggregate (TE), the distance-eccentricity product (EDS), and the eccentricity analog of the geometric-arithmetic index (GEA).

**Total host-test suite: 1223 tests** (1213 through V3.24 + 10 new).

---

## New Feature: `graph_topo_indices14()` — TE + EDS + GEA

### API

```rust
pub fn graph_topo_indices14() -> (u64, u64, u64, usize, usize)
//                                 te  eds  gea   edges  nodes
```

### Definitions

- **TE(G) = Σ_v ecc(v)** — Total Eccentricity Index (exact u64; Dankelmann et al. 2004)
- **EDS(G) = Σ_v ecc(v)·T_v** — Eccentric Distance Sum (exact u64; Gupta et al. 2008)
- **GEA(G) × 10^6 = Σ_{uv∈E} 2√(ecc(u)·ecc(v))/(ecc(u)+ecc(v))** — Geometric-Arithmetic Eccentricity (floor ppm)

where:
- `ecc(v)` = max BFS distance from v to any reachable node (0 for isolated/single-node)
- `T_v` = vertex transmission = Σ_{w reachable, w≠v} d(v,w)

### Key Invariants

- **GEA = |E|×10^6** iff graph is **self-centered** (all ecc equal; AM=GM on eccentricities)
  - K_n (all ecc=1), K_{r,s} (all ecc=2), even cycles C_{2k} (all ecc=k): GEA = |E|×10^6
- **TE(K_n) = n** (all ecc=1); **EDS(K_n) = n(n-1)** (ecc=1, T=n-1)
- **Isolated nodes** (ecc=0, T=0): contribute 0 to all three indices; no edge contribution from GEA

### Algorithm

1. Build undirected adjacency bitmasks: O(E)
2. BFS from each source — computes ecc(v) and T_v simultaneously: O(n·(n+m))
3. Node scan: TE = Σ ecc(v); EDS = Σ ecc(v)·T_v
4. Edge scan (a < b): GEA = Σ isqrt64(4·ea·eb·10^12) / (ea+eb)

**isqrt64** — Newton-Raphson integer sqrt (no float, no_std safe).  
**Overflow safety**: 4·127²·10^12 ≈ 6.5×10^16 < u64::MAX = 1.84×10^19. No overflow possible.

### Stack Usage

- adj[128] (u128 × 128 = 2 KB)
- ecc[128] (u8 × 128 = 128 B)
- trans[128] (u64 × 128 = 1 KB)
- dist[128] + queue[128] (u8 × 256 = 256 B)
- **Total ≈ 3.5 KB** (same class as V3.23/V3.24)

### Cross-Check Table

| Graph       | TE | EDS | GEA (ppm)  | edges | nodes |
|-------------|----|-----|------------|-------|-------|
| Empty       | 0  | 0   | 0          | 0     | 0     |
| 1 node      | 0  | 0   | 0          | 0     | 1     |
| Edge A-B    | 2  | 2   | 1_000_000  | 1     | 2     |
| Path P₃     | 5  | 14  | 1_885_618  | 2     | 3     |
| Triangle K₃ | 3  | 6   | 3_000_000  | 3     | 3     |
| Star K₁,₄  | 9  | 60  | 3_771_236  | 4     | 5     |
| Path P₄     | 10 | 52  | 2_959_590  | 3     | 4     |
| Complete K₄ | 4  | 12  | 6_000_000  | 6     | 4     |
| 2 isolated  | 0  | 0   | 0          | 0     | 2     |
| K₂,₃        | 10 | 56  | 6_000_000  | 6     | 5     |

**P₃ GEA derivation**: per-edge {A,B}: isqrt64(4×2×1×10^12)/3 = isqrt64(8e12)/3 = 2_828_427/3 = 942_809. GEA = 2×942_809 = 1_885_618.  
**K₂,₃ GEA = 6_000_000**: confirms K₂,₃ is self-centered (all ecc=2). Cross-check: GEA/|E| = 1_000_000. ✓

### Shell Dispatch

```
"graph topo14" | "gtopo14" | "total eccentricity" | "gte"
| "eccentric distance sum" | "geds"
| "geometric arithmetic eccentricity" | "ggea"
| "gteedsge" | "gteedsegea"
```

### VectorAddress

**L4=101** for `gos-graph-topo14-harness`

### Display

- Header: bright-yellow
- TE: bright-cyan `[Σ_v ecc(v)] (exact)`
- EDS: bright-green `[Σ_v ecc(v)·T_v] (exact)`
- GEA: bright-magenta `[Σ 2√(ea·eb)/(ea+eb)] (self-centered | ppm)`
- Footer: `N node(s) M edge(s) Dankelmann et al. 2004 Gupta et al. 2008`

### OS Analogy

- **TE**: aggregate routing reach budget — total eccentricity load across all nodes; low TE = compact topology (hub nodes dominate), high TE = elongated chain
- **EDS**: eccentricity-weighted distance pressure — amplifies peripheral hubs that are both far-reaching (high ecc) and heavily loaded (high T_v); useful for identifying IPC bottleneck detection in deep dependency chains
- **GEA**: eccentricity channel balance ratio — =|E| for self-centered topologies (uniform routing reach); <|E| for asymmetric reach (some endpoints much farther from the graph center than others)

### Literature

- Dankelmann, Goddard & Swart 2004 (Total Eccentricity)
- Gupta, Singh & Madan 2008 (Eccentric Distance Sum ξ^d)
- Geometric-Arithmetic eccentricity index: analog of GA index (Vukičević & Furtula 2009) applied to eccentricities

---

## VectorAddress L4 Namespace (updated)

```
88=graph-topo, 89=graph-topo2, 90=graph-topo3, 91=graph-topo4, 92=graph-topo5,
93=graph-topo6, 94=graph-topo7, 95=graph-topo8, 96=graph-topo9, 97=graph-topo10,
98=graph-topo11, 99=graph-topo12, 100=graph-topo13, 101=graph-topo14
```

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | +133 lines: `graph_topo_indices14_inner()` + `graph_topo_indices14()` |
| `crates/k-shell/src/lib.rs` | +76 lines: `dispatch_graph_topo_indices14()` |
| `crates/k-shell/src/proc.rs` | +2 lines: shell routing for 10 aliases |
| `host-tests/gos-graph-topo14-harness/` | New: Cargo.toml, .cargo/config.toml, tests/graph_topo14.rs |

---

## Test Results

```
running 10 tests
test test_01_empty         ... ok
test test_02_single_node   ... ok
test test_03_single_edge   ... ok
test test_04_path_p3       ... ok
test test_05_triangle_k3   ... ok
test test_06_star_k14      ... ok
test test_07_path_p4       ... ok
test test_08_complete_k4   ... ok
test test_09_two_isolated  ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

---

## Hardening Quality Assessment

- **No_std safe**: only `core::` primitives, Newton-Raphson isqrt, no heap, no float
- **Overflow safe**: 4·127²·10^12 < u64::MAX verified analytically
- **Self-centered invariant**: GEA=|E|×10^6 verified on K₃, K₄, K_{2,3} (tests 5, 8, 10)
- **Isolated node invariant**: ecc=0, T=0 → zero contributions (tests 2, 9)
- **BFS correctness**: ecc and T computed in single O(n·(n+m)) pass (same as V3.19/V3.23)
- **Precision**: GEA per-edge = isqrt64(4·ea·eb·10^12)/(ea+eb); floor error ≤ 1 ppm/edge
