# HARDENING LOG — V3.79 (2026-07-20)

## Summary

V3.79 adds three new Neighborhood S-variant topological indices (topo68) to the graph-theory OS kernel runtime, wires the previously missing k-shell dispatch functions for topo66 and topo67, and delivers a complete 10-test verification harness.

---

## New Indices: NDOTETRAACTC + NHDOTETRAACTC + NAKSO

### Mathematical Definitions

**S(v) = Σ_{w∈N(v)} deg(w)** — neighbor-degree sum (S-variant, unchanged from topo18 family)

| Index | Formula | Name | α |
|-------|---------|------|---|
| NDOTETRAACTC | Σ_v S(v)^42 | S-Dotetracontic vertex sum | — |
| NHDOTETRAACTC | Σ_{uv∈E} (S_u+S_v)^41 | S-Hentetracontic edge-sum | — |
| NAKSO | Σ_{uv∈E} (S_u²+S_v²)^36 | S-Dotetracontyl Sombor | 72 |

### Series Positions

- **NDOTETRAACTC** extends NHENTETRAACTC=ΣS^41 (topo67) to the 42nd power
- **NHDOTETRAACTC** extends NHHENTETRAACTC=Σ(S+S)^40 (topo67) to the 41st power
- **NAKSO** is the generalised S-variant Sombor SO^α with α=72 (3rd-pass "AK"):
  NAISO(α=68)→NAJSO(α=70)→NAKSO(α=72)

### Expected Values

| Graph | NDOTETRAACTC | NHDOTETRAACTC | NAKSO | edges | nodes |
|-------|-------------|--------------|-------|-------|-------|
| Empty | 0 | 0 | 0 | 0 | 0 |
| 1 node | 0 | 0 | 0 | 0 | 1 |
| K₂ | 2 | 2_199_023_255_552 | 68_719_476_736 | 1 | 2 |
| P₃ | 13_194_139_533_312 | u64::MAX (sat.) | u64::MAX (sat.) | 2 | 3 |
| K₃ | u64::MAX (sat.) | u64::MAX (sat.) | u64::MAX (sat.) | 3 | 3 |
| K_{1,4} | u64::MAX (sat.) | u64::MAX (sat.) | u64::MAX (sat.) | 4 | 5 |
| P₄ | u64::MAX (sat.) | u64::MAX (sat.) | u64::MAX (sat.) | 3 | 4 |
| K₄ | u64::MAX (sat.) | u64::MAX (sat.) | u64::MAX (sat.) | 6 | 4 |
| K_{2,3} | u64::MAX (sat.) | u64::MAX (sat.) | u64::MAX (sat.) | 6 | 5 |

**Key derivations:**
- K₂ (S=1): NDOTETRAACTC=2×1^42=2; NHDOTETRAACTC=2^41=2_199_023_255_552; NAKSO=2^36=68_719_476_736
- P₃ (S=2): NDOTETRAACTC=3×2^42=13_194_139_533_312; NHDOTETRAACTC: 4^41=2^82>>u64::MAX → SAT
- P₄ (S=2,3,3,2): 3^42=109_418_989_131_512_359_209>u64::MAX → NDOTETRAACTC saturates

### S-Regular Formula

- NDOTETRAACTC = n·S^42
- NHDOTETRAACTC = |E|·(2S)^41 = 2_199_023_255_552·|E|·S^41
- NAKSO = |E|·(2S²)^36 = 68_719_476_736·|E|·S^72

### Power-Decomposition Implementation

| Index | Decomposition | Mults |
|-------|-------------|-------|
| s^42 | s32×s8×s2 (42=32+8+2) | 3 |
| ss^41 | ss32×ss8×ss (41=32+8+1) | 3 |
| s2s^36 | s2s32×s2s4 (36=32+4) | **2** (very efficient!) |

Note: s2s^36 uses only 2 multiplications after building the squaring ladder — 36=32+4 is a sum of two powers of 2.

---

## Bug Fix: Missing k-shell dispatch for topo66 and topo67

Previous automated hardening runs added topo66 and topo67 to the runtime and created their harnesses, but omitted the k-shell dispatch functions and proc.rs routing. V3.79 remedies both gaps:

### Added dispatch_graph_topo_indices66

- Displays NTETRAACTC (S^40), NHTETRAACTC ((S+S)^39), NAISO ((S_u²+S_v²)^34, α=68)
- Shell triggers: `graph topo66`, `gtopo66`, `gntetraactc`, `gnhtetraactc`, `gnnaiso`, `gntetraactcnhtetraactcnaiso`

### Added dispatch_graph_topo_indices67

- Displays NHENTETRAACTC (S^41), NHHENTETRAACTC ((S+S)^40), NAJSO ((S_u²+S_v²)^35, α=70)
- Shell triggers: `graph topo67`, `gtopo67`, `gnhentetraactc`, `gnhhentetraactc`, `gnnajso`, `gnhentetraactcnhhentetraactcnajso`

### Added dispatch_graph_topo_indices68

- Displays NDOTETRAACTC (S^42), NHDOTETRAACTC ((S+S)^41), NAKSO ((S_u²+S_v²)^36, α=72)
- Shell triggers: `graph topo68`, `gtopo68`, `gndotetraactc`, `gnhdotetraactc`, `gnnakso`, `gndotetraactcnhdotetraactcnakso`

---

## Files Changed

| File | Change |
|------|--------|
| `crates/gos-runtime/src/lib.rs` | Added `graph_topo_indices68_inner()` + `graph_topo_indices68()` |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_topo_indices66/67/68()` |
| `crates/k-shell/src/proc.rs` | Added routing for topo66, topo67, topo68 |
| `host-tests/gos-graph-topo68-harness/Cargo.toml` | New harness workspace |
| `host-tests/gos-graph-topo68-harness/.cargo/config.toml` | Host-target override |
| `host-tests/gos-graph-topo68-harness/tests/graph_topo68.rs` | 10 tests (all green) |

---

## Test Results

```
running 10 tests
..........
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Host-test suite total: **1763 tests** (1753 prior + 10 new)

---

## VectorAddress L4 Namespace

88=graph-topo through 154=graph-topo67, **155=graph-topo68**

---

## Plugin and Executor IDs

- Plugin: `TOPIX_68`
- Executor: `t68.exec`
- VectorAddress L4: 155
