# Hardening Log — V3.27 (2026-07-08)

## Summary
Implemented three generalized Randić-family + Lanzhou degree-based topological indices as the next scheduled hardening increment. Added 10 host tests (all green). Total host-test count: **1243**.

## Indices Added

### R_{1/2} — Product Connectivity (Bollobás & Erdős 1998)
- `ir_ppm = R_{1/2}(G) × 10^6 = Σ_{uv∈E} √(d_u·d_v) × 10^6` (floor ppm via isqrt64)
- Generalizes Randić index (α=−½) to α=+½; measures geometric-mean degree product across edges
- Regular Δ-graph: IR = m·Δ (exact; ppm = m·Δ·10^6)
- Non-pendant star K_{1,k}: IR = k·√k·10^6 (ppm)
- Complement to classic Randić R: R×IR ≥ m² (Cauchy-Schwarz)

### R_{-1} — Reciprocal Randić (Bollobás & Erdős 1998)
- `rr_ppm = R_{-1}(G) × 10^6 = Σ_{uv∈E} ⌊10^6/(d_u·d_v)⌋` (floor ppm; pure integer, no sqrt)
- Generalizes Randić index (α=−½) to α=−1; penalizes high-degree edge products
- Regular Δ-graph: RR = m/Δ² (ppm = floor(m·10^6/Δ²))
- Star K_{1,k}: RR = k·10^6/(1·k) = k·10^6/k = 10^6 (exact, independent of k)

### Lz — Lanzhou Index (Xia et al. 2019)
- `lz = Lz(G) = Σ_v d_v²·(n−1−d_v)` (exact u64; no sqrt, no BFS)
- Algebraic identity: Lz = (n−1)·M₁(G) − F(G) (First Zagreb × (n-1) minus Forgotten index)
- Lz = 0 for any complete graph K_n (n−1−d_v = 0 for all v)
- Measures degree-weighted "room to grow" — amplifies high-degree nodes with many absent edges
- OS analogy: kernel modules with high IPC degree but many unused connection slots (fragmented hub topology)

## Reference Values

| Graph        | IR_ppm     | RR_ppm  | lz | edges | nodes |
|--------------|------------|---------|-----|-------|-------|
| Empty        | 0          | 0       | 0   | 0     | 0     |
| 1 node       | 0          | 0       | 0   | 0     | 1     |
| Edge A-B     | 1_000_000  | 1_000_000 | 0  | 1    | 2     |
| Path P₃      | 2_828_426  | 1_000_000 | 2  | 2    | 3     |
| Triangle K₃  | 6_000_000  | 750_000 | 0   | 3     | 3     |
| Star K_{1,4} | 8_000_000  | 1_000_000 | 12 | 4    | 5     |
| Path P₄      | 4_828_426  | 1_250_000 | 12 | 3    | 4     |
| Complete K₄  | 18_000_000 | 666_666 | 0   | 6     | 4     |
| Two isolated | 0          | 0       | 0   | 0     | 2     |
| K_{2,3}      | 14_696_934 | 999_996 | 42  | 6     | 5     |

## Key Derivations

- **P₃ IR**: isqrt64(1×2×10^12) = isqrt64(2×10^12) = 1_414_213; ×2 = 2_828_426
- **K₃ RR**: 3 × floor(10^6/4) = 3 × 250_000 = 750_000
- **K₄ RR**: 6 × floor(10^6/9) = 6 × 111_111 = 666_666 (10^6/9 = 111_111.1...; floor = 111_111)
- **K_{2,3} IR**: isqrt64(6×10^12) = 2_449_489; ×6 = 14_696_934 (√6 = 2.44948974…)
- **K_{2,3} RR**: 6 × floor(10^6/6) = 6 × 166_666 = 999_996
- **K_{2,3} Lz**: (5−1)×M₁ − F = 4×30 − 78 = 120 − 78 = 42; M₁=2×9+3×4=30, F=2×27+3×8=78

## Algorithm

- Phase 1: Compact node index (O(V))
- Phase 2: Undirected adjacency bitmask construction + edge dedup (O(E))
- Phase 3: Degree array from adj.count_ones() (O(V))
- Phase 4: Node scan for Lz: Σ_v d²·(n−1−d) (O(V))
- Phase 5: Edge scan for IR and RR (O(E)):
  - IR per edge: isqrt64(da·db·10^12)
  - RR per edge: floor(10^6/(da·db))
  - Overflow guard: da·db ≤ 127² = 16_129; 16_129×10^12 < u64::MAX ✓
- Total: O(V+E) — fastest category (no BFS needed)

## Stack Usage
- adj[128] (u128 = 2KB) + deg[128] (u64 = 1KB) ≈ 3KB total

## Files Modified
- `crates/gos-runtime/src/lib.rs`: added `graph_topo_indices16_inner` + `graph_topo_indices16` pub fn
- `crates/k-shell/src/lib.rs`: added `dispatch_graph_topo_indices16`
- `crates/k-shell/src/proc.rs`: routing entry for "graph topo16" / "gtopo16" / "product connectivity" / "gpc" / "reciprocal randic" / "grr" / "lanzhou index" / "glz" / "gpcrrlz"
- `host-tests/gos-graph-topo16-harness/`: new harness (10 tests, VectorAddress L4=103)

## Shell Commands
```
graph topo16   gtopo16
product connectivity   gpc
reciprocal randic      grr
lanzhou index          glz
gpcrrlz
```

## OS Analogy
- **IR (R_{1/2})**: cross-channel geometric-mean coupling intensity — high-degree pairs amplified; >m for hub-spoke topologies
- **RR (R_{-1})**: inverse degree-product per link — high means uniform low-degree mesh; suppressed by hubs
- **Lz**: per-module "unused slot pressure" — modules with many current connections but even more absent ones dominate; zero for complete meshes

## Literature
- Bollobás, B. & Erdős, P. (1998). "Graphs of extremal weights." Ars Combinatoria, 50, 225-233.
- Xia, Z., Chen, T., Wei, W. (2019). "The Lanzhou Index." MATCH Commun. Math. Comput. Chem., 82, 675-686.
