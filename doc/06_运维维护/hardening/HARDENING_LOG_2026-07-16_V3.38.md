# HARDENING LOG — V3.38 (2026-07-16)

## Summary

Added three Neighborhood S-variant topological indices (topo27 family) to the GOSKernel runtime and k-shell: **NRR** (Neighborhood Reciprocal Randić), **NSO\*** (Neighborhood Modified Sombor), and **NrSO** (Neighborhood Reduced Sombor). This is the 16th installment of the S-variant index series (V3.22–V3.38) and extends the index count to L4=114 in VectorAddress namespace. All computations are integer-only (`no_std` safe).

---

## Mathematical Background

S-variant indices replace the degree `d(v)` in classical formulae with the **neighbor-degree sum**:

```
S(v) = Σ_{w ∈ N(v)} deg(w)
```

### NRR — Neighborhood Reciprocal Randić

```
NRR(G) = Σ_{uv ∈ E} 1 / (S_u · S_v)
```

S-analogue of the reciprocal Randić index `R_{-1}` (Bollobás & Erdős 1998).

Runtime formula (no float): `floor(10^6 / (S_u · S_v))` per edge (ppm units).

### NSO\* — Neighborhood Modified Sombor

```
NSO*(G) = Σ_{uv ∈ E} (S_u · S_v) / √(S_u² + S_v²)
```

S-analogue of the modified Sombor index SO\* (Ghanbari & Rajabi-Parsa 2021).

Runtime formula: `isqrt128(S_u² · S_v² · 10^12 / (S_u² + S_v²))` per edge (ppm units).

### NrSO — Neighborhood Reduced Sombor

```
NrSO(G) = Σ_{uv ∈ E} √((S_u - 1)² + (S_v - 1)²)
```

S-analogue of the reduced Sombor index rSO (Doslic et al. 2022).

Runtime formula: `isqrt128(((S_u-1)² + (S_v-1)²) · 10^12)` per edge (ppm units).

**Overflow note**: `(S_u-1)² + (S_v-1)²` × 10^12 can reach ~5.2×10^20 for high-degree graphs, exceeding u64::MAX (~1.84×10^19). Handled by using u128 intermediate before isqrt128.

---

## Key Invariants

| Invariant | Condition |
|---|---|
| NRR = \|E\| × 10^6 | All S=1 (only K₂) |
| NSO\* = NrSO per edge | S-uniform S=2 (P₃ edges: both equal √2 × 10^6 = 1_414_213) |
| NrSO = 0 | All S=1 (K₂: both endpoints have (S-1)²=0) |
| K₃ ≡ K_{1,4} on all three indices | S-uniform S=4 coincidence |
| K_{2,3} NSO\* per edge = K₃ NrSO per edge | Both equal isqrt128(18×10^12) = 4_242_640 |

---

## Implementation

### Files Modified

| File | Change |
|---|---|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices27_inner()` + `graph_topo_indices27()` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices27()` |
| `crates/k-shell/src/proc.rs` | Added routing for `graph topo27` / `gtopo27` / `gnrr` / `gnsos` / `gnrso2` etc. |

### Files Created

| File | Description |
|---|---|
| `host-tests/gos-graph-topo27-harness/Cargo.toml` | Independent workspace manifest |
| `host-tests/gos-graph-topo27-harness/.cargo/config.toml` | Host target override (x86_64-pc-windows-msvc) |
| `host-tests/gos-graph-topo27-harness/tests/graph_topo27.rs` | 10-test harness with full analytical cross-check |

### Shell Commands

```
graph topo27
gtopo27
gnrr
gnsos
gnrso2
gnrrnsosnrso
neighborhood reciprocal randic
neighborhood modified sombor
neighborhood reduced sombor
```

### Return Signature

```rust
pub fn graph_topo_indices27() -> (u64, u64, u64, usize, usize)
//                                nrr  nsos nrso  edges  nodes
// All three indices in parts-per-million (floor).
```

### VectorAddress

L4 = **114** (topo27 harness namespace)

---

## Test Results

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Test Matrix

| Test | Graph | NRR (ppm) | NSO\* (ppm) | NrSO (ppm) | Edges | Nodes |
|---|---|---|---|---|---|---|
| 01 | Empty | 0 | 0 | 0 | 0 | 0 |
| 02 | Single isolated node | 0 | 0 | 0 | 0 | 1 |
| 03 | K₂ (single edge) | 1_000_000 | 707_106 | 0 | 1 | 2 |
| 04 | P₃ (path) | 500_000 | 2_828_426 | 2_828_426 | 2 | 3 |
| 05 | K₃ (triangle) | 187_500 | 8_485_281 | 12_727_920 | 3 | 3 |
| 06 | K_{1,4} (star) | 250_000 | 11_313_708 | 16_970_560 | 4 | 5 |
| 07 | P₄ (path) | 444_443 | 5_449_520 | 7_300_561 | 3 | 4 |
| 08 | K₄ (complete) | 74_070 | 38_183_766 | 67_882_248 | 6 | 4 |
| 09 | Two isolated nodes | 0 | 0 | 0 | 0 | 2 |
| 10 | K_{2,3} (bipartite) | 166_662 | 25_455_840 | 42_426_402 | 6 | 5 |

---

## References

- Bollobás, B. & Erdős, P. (1998). Graphs of extremal weights. *Ars Combinatoria*, 50, 225–233. (Original Randić-type index R_{-1})
- Ghanbari, N. & Rajabi-Parsa, S. (2021). A variant of the Sombor index. *MATCH Commun. Math. Comput. Chem.*, 86, 669–683. (SO\*)
- Doslic, T., Réti, T., & Ali, A. (2022). On the reduced Sombor index and its applications. *MATCH Commun. Math. Comput. Chem.*, 88, 529–543. (rSO)

---

## Version State After V3.38

- **Branch**: `feat/vk-auto-live-surface`
- **Host tests**: 1353 (1343 + 10 new from topo27 harness)
- **S-variant indices**: topo22–topo27 complete (V3.33–V3.38)
  - topo22: NR, NF, NSC
  - topo23: NHM1, NSDD, NM3
  - topo24: NISI, NAZI, NEM1
  - topo25: NHM2, NAG, NABS
  - topo26: NPC, NRM₂, NRSO
  - topo27: NRR, NSO\*, NrSO
- **VectorAddress L4**: 114 (topo27 harness)
