# Hardening Log — V3.29 (2026-07-08)

## Summary

Added **Neighborhood Zagreb NM₁ + NM₂ + GA₂** topological indices to GOS runtime.
These are degree-based indices using the **neighbor-degree sum** S(v) = Σ_{u∈N(v)} deg(u)
as a second-order degree measure, giving a richer view of local topology than first-order degree.

## New Capability: `graph topo18`

**Shell commands**: `graph topo18` / `gtopo18` / `neighborhood zagreb` / `gnm1nm2` /
`nm1 index` / `gnm1` / `nm2 index` / `gnm2` / `neighborhood ga` / `gga2` / `gnm1nm2ga2`

**Function**: `gos_runtime::graph_topo_indices18() -> (nm1: u64, nm2: u64, ga2_ppm: u64, edge_count: usize, node_count: usize)`

### Index Definitions

Let S(v) = Σ_{u∈N(v)} deg(u) (sum of degrees of v's neighbors — "2nd-order degree").

| Index | Formula | Type | Reference |
|-------|---------|------|-----------|
| NM₁(G) | Σ_v S(v)² | exact u64 | Mondal et al. 2019 |
| NM₂(G) | Σ_{uv∈E} S(u)·S(v) | exact u64 | Mondal et al. 2019 |
| GA₂(G) | Σ_{uv∈E} 2√(S_u·S_v)/(S_u+S_v) | floor ppm (isqrt128) | — |

### Key Invariants

- **S-uniform invariant**: GA₂ = |E| × 10^6 when all S(v) are equal. This holds for:
  K_n (complete graphs), K_{r,s} (complete bipartite), K_{1,k} (stars), P₃, regular graphs.
- **Isolated nodes**: S(v) = 0 → contribute 0 to all three indices.
- **NM₁ = NM₂ = GA₂ = 0** for empty graph or all-isolated nodes.

### Cross-Check Table

| Graph | NM₁ | NM₂ | GA₂ (ppm) | S-values |
|-------|-----|-----|-----------|----------|
| Empty | 0 | 0 | 0 | — |
| Single node | 0 | 0 | 0 | S(A)=0 |
| Edge A-B | 2 | 1 | 1_000_000 | S=1 all |
| Path P₃ | 12 | 8 | 2_000_000 | S=2 all |
| Triangle K₃ | 48 | 48 | 3_000_000 | S=4 all |
| Star K_{1,4} | 80 | 64 | 4_000_000 | S=4 all |
| Path P₄ | 26 | 21 | 2_959_590 | S=(2,3,3,2) |
| Complete K₄ | 324 | 486 | 6_000_000 | S=9 all |
| Two isolated | 0 | 0 | 0 | S=0 all |
| K_{2,3} | 180 | 216 | 6_000_000 | S=6 all |

### P₄ GA₂ Derivation

For P₄ = A-B-C-D: S(A)=2, S(B)=3, S(C)=3, S(D)=2.

- {A,B}: isqrt128(4·2·3·10^12)/5 = isqrt128(24·10^12)/5 = 4_898_979/5 = **979_795**
- {B,C}: isqrt128(4·3·3·10^12)/6 = 6·10^6/6 = **1_000_000**
- {C,D}: same as {A,B} = **979_795**
- GA₂ = 979_795 + 1_000_000 + 979_795 = **2_959_590** ✓

### Algorithm

1. Compact node indexing: O(V)
2. Build undirected adj bitmasks: O(E)
3. Degree array: deg[ci] = adj[ci].count_ones()
4. S(v) = Σ_{u∈N(v)} deg(u): traverse adj bits per node
5. NM₁ = Σ_v S(v)²: node scan
6. NM₂, GA₂: undirected edge scan (a < b)
   - GA₂ per edge = isqrt128(4·S_a·S_b·10^12) / (S_a+S_b)
   - Overflow: S(v) ≤ 127·127 = 16129; 4·S²·10^12 ≤ ~10^21 fits in u128 ✓

**Total complexity**: O(V+E), no BFS needed.
**Stack**: adj[128](2KB) + deg[128](1KB) + sv[128](1KB) ≈ 4KB total.

### OS Analogy

| Metric | Meaning in graph-OS context |
|--------|-----------------------------|
| NM₁ | 2nd-hop routing pressure squared (amplifies high-connectivity neighborhoods) |
| NM₂ | 2nd-hop edge co-load product (both endpoints in dense neighborhoods) |
| GA₂ | Neighborhood balance ratio (=|E| for S-uniform: K_n, K_{r,s}; <|E| for asymmetric) |

### VectorAddress

L4 = 105 for `gos-graph-topo18-harness`.

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices18_inner()` method (O(V+E) NM₁+NM₂+GA₂) + `graph_topo_indices18()` public API |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices18()` display function |
| `crates/k-shell/src/proc.rs` | Added routing for `graph topo18` / `gtopo18` / 9 aliases |
| `host-tests/gos-graph-topo18-harness/` | New harness (10 tests, all passing) |

## Test Results

```
test test_01_empty        ... ok
test test_02_single_node  ... ok
test test_03_single_edge  ... ok
test test_04_path_p3      ... ok
test test_05_triangle_k3  ... ok
test test_06_star_k14     ... ok
test test_07_path_p4      ... ok
test test_08_complete_k4  ... ok
test test_09_two_isolated ... ok
test test_10_k23_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Host test suite total: 1263 tests** (1253 prior + 10 new).
