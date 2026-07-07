# GOSKernel Hardening Log — V3.24
**Date:** 2026-07-07  
**Branch:** feat/vk-auto-live-surface  
**Host-test suite total:** 1213 tests (all green)

---

## Summary

V3.24 introduces **Transmission Zagreb indices** — three metrics derived from vertex transmissions T(v) (the BFS distance sum from each node to all reachable nodes). These extend the transmission-based index family established in V3.22 (Balaban J, TI, PI_v), adding squared-transmission and product-transmission variants. The new Geometric-Arithmetic transmission index GA_t uses an `isqrt128` Newton-Raphson implementation to handle large u128 intermediate products that overflow u64.

---

## New Feature: `graph topo13` — TM₁ + TM₂ + GA_t Transmission Zagreb Indices

### API

```rust
pub fn graph_topo_indices13() -> (u64, u64, u64, usize, usize)
// Returns: (tm1, tm2, ga_t_ppm, edge_count, node_count)
```

### Indices

| Symbol | Formula | Type | Literature |
|--------|---------|------|-----------|
| TM₁ | Σ_v T_v² | exact u64 | Xing & Gutman 2012 |
| TM₂ | Σ_{uv∈E} T_u·T_v | exact u64 | Xing & Gutman 2012 |
| GA_t | Σ_{uv∈E} 2√(T_u·T_v)/(T_u+T_v) | floor ppm (×10⁶) | Alizadeh et al. 2013 |

Where **T_v = Σ_{w reachable, w≠v} d(v,w)** is the vertex transmission within the connected component of v.

### Key Invariants

- `GA_t = |E| × 10⁶` iff the graph is **transmission-regular** (all T_v equal)
  - Examples: K_n (all T=n-1), K₃ (all T=2), K₄ (all T=3), even cycles
- `GA_t < |E| × 10⁶` for non-transmission-regular graphs (e.g., K_{2,3}, stars, paths)
- Isolated nodes: T_v=0, contribute 0 to TM₁; no edge contribution to TM₂ or GA_t

### Algorithm

1. **BFS O(n·(n+m))**: compute T_v for all nodes
2. **O(n) node scan**: TM₁ = Σ T_v²
3. **O(m) undirected edge scan (a < b)**:
   - TM₂ += T_a × T_b
   - GA_t: `isqrt128(4·T_a·T_b·10¹²) / (T_a + T_b)` (u128 arithmetic)

### isqrt128 Implementation

GA_t per edge = `floor(2√(T_u·T_v) / (T_u+T_v) × 10⁶) = isqrt128(4·T_u·T_v·10¹²) / (T_u+T_v)`

Since max T_v ≈ 8128 for MAX_NODES=128 nodes, `4·T_u·T_v·10¹² ≤ 2.64×10²⁰` which overflows u64 (max 1.84×10¹⁹). A u128 Newton-Raphson isqrt is required:

```rust
fn isqrt128(n: u128) -> u128 {
    if n == 0 { return 0; }
    let bits = 128u32 - n.leading_zeros();
    let mut x: u128 = 1u128 << ((bits + 1) / 2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x { return x; }
        x = y;
    }
}
```

No float, no_std-safe, converges in O(log log n) Newton-Raphson steps.

### Stack Usage

- `adj[128]` (u128 × 128 = 2KB)
- `trans[128]` (u64 × 128 = 1KB)
- `dist[128]` + `queue[128]` = 256B
- **Total: ~3.5KB**

### Cross-Check Table

| Graph | TM₁ | TM₂ | GA_t | edges | nodes |
|-------|-----|-----|------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| 1 node | 0 | 0 | 0 | 0 | 1 |
| Edge A-B | 2 | 1 | 1_000_000 | 1 | 2 |
| Path P₃ | 22 | 12 | 1_959_590 | 2 | 3 |
| Triangle K₃ | 12 | 12 | 3_000_000 | 3 | 3 |
| Star K_{1,4} | 212 | 112 | 3_848_364 | 4 | 5 |
| Path P₄ | 104 | 64 | 2_959_590 | 3 | 4 |
| Complete K₄ | 36 | 54 | 6_000_000 | 6 | 4 |
| Two isolated | 0 | 0 | 0 | 0 | 2 |
| K_{2,3} | 158 | 180 | 5_975_154 | 6 | 5 |

### Derivation Samples

**K₃**: T_A=T_B=T_C=2. TM₁=3×4=12. TM₂=3×4=12.
GA_t: isqrt128(4×2×2×10¹²)/4 = 4_000_000/4 = 1_000_000 per edge → 3_000_000 (trans-regular ✓)

**K_{2,3}**: T_left=5, T_right=6.
GA_t: isqrt128(4×5×6×10¹²)/11 = isqrt128(120×10¹²)/11 = 10_954_451/11 = 995_859 per edge → 5_975_154

**P₄**: T_A=T_D=6, T_B=T_C=4.
Edge {B,C}: isqrt128(4×4×4×10¹²)/8 = 8_000_000/8 = 1_000_000 (exact, same transmissions).

### Shell Aliases

```
graph topo13 | gtopo13 | transmission zagreb | gtm1tm2
tm1 index    | gtm1    | tm2 index           | gtm2
geometric arithmetic transmission | ggat | gtm1tm2gat
```

---

## OS Analogy

| Index | OS Interpretation |
|-------|------------------|
| TM₁ | Squared routing-load pressure — amplifies nodes with high distance-weighted reach (hub amplifier) |
| TM₂ | Edge co-load product — measures channel-pair load; high TM₂ = heavily loaded endpoint pairs |
| GA_t | Geometric-arithmetic channel load balance — equals \|E\|×10⁶ for balanced routing; < \|E\|×10⁶ for hub-spoke asymmetry |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices13_inner()` + `graph_topo_indices13()` with isqrt128 |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices13()` with ppm display |
| `crates/k-shell/src/proc.rs` | Added routing for "graph topo13" / "gtopo13" / aliases |
| `host-tests/gos-graph-topo13-harness/` | New 10-test harness (VectorAddress L4=100) |

**Commit:** `feat(v3.24): Transmission Zagreb TM1 + TM2 + GA_t transmission-based indices + gos-graph-topo13-harness (10 tests)`
