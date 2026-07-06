# GOSKernel Hardening Log — V3.01
**Date:** 2026-07-06  
**Algorithm:** Feedback Vertex Set (FVS) — Greedy Kahn-based  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.01): feedback vertex set -- greedy Kahn FVS + gos-graph-fvs-harness (10 tests)

---

## Summary

V3.01 adds **feedback vertex set (FVS)** — a classic NP-hard graph problem — to the GOSKernel graph theory runtime.  The FVS is the minimum set of vertices whose removal leaves the directed graph acyclic (a DAG).

This completes the **cycle-breaking toolkit**:
- V2.91 — feedback arc set (FAS): minimum *edges* to remove to break all cycles
- V3.01 — feedback vertex set (FVS): minimum *vertices* to remove to break all cycles

---

## Public API

### `gos_runtime::graph_fvs<const N: usize>() -> ([VectorAddress; N], usize, usize)`

Returns `(fvs_vecs, fvs_size, node_count)`:
- `fvs_vecs[0..fvs_size]` — FVS nodes sorted ascending by `VectorAddress.as_u64()`
- `fvs_size` — number of nodes in the greedy FVS (upper bound on min-FVS)
- `node_count` — total live nodes in the graph

**Acyclicity guarantee:** removing the returned FVS nodes from the graph always leaves a DAG.

---

## Algorithm: Iterative Kahn BFS

**Complexity:** O(V × (V + E)) per FVS call — V iterations, each O(V + E).  
For GOSKernel with MAX_NODES=128 and MAX_EDGES=512: ≤ 65K · 640 = 41M ops, well within budget.

**Each round:**
1. Compute `in_deg[ci]` and `out_deg[ci]` for live nodes (self-loops counted; self-loops do NOT enter `adj` bitmask)
2. Build `adj[ci]` = u128 bitmask of outgoing live edges (excluding self-loops)
3. Kahn BFS: seed with in-degree-0 nodes; drain by decrementing successor in-degrees
4. If `processed == live_count` → acyclic, done
5. Else: among undrained (cyclic) nodes, pick the one with max `in_deg × out_deg` score, add to FVS, mark dead

**Score heuristic:** `in_deg[ci] × out_deg[ci]` — nodes at the intersection of many in-paths and out-paths are most likely to appear in many cycles; removing them efficiently breaks multiple cycles per step.

**Self-loop handling:** A self-loop A→A sets `in_deg[A] += 1` but does NOT add A→A to `adj[A]`. Therefore Kahn never dequeues A (in_deg stays ≥ 1) → A is always classified as cyclic → correctly enters FVS.

**Stack arrays used:**
- `live[MAX_NODES]` — bool array of surviving nodes
- `fvs_cis[MAX_NODES]` — collected FVS compact indices
- `in_deg[MAX_NODES]`, `out_deg[MAX_NODES]`, `adj[MAX_NODES]` — recomputed each round
- `queue[MAX_NODES]`, `in_queue[MAX_NODES]` — Kahn BFS arrays
- `tmp[MAX_NODES]` — for final sort by VectorAddress.as_u64()

**No heap allocation** — all state on kernel stack (approximately 5 KB).

---

## Shell Interface

| Command | Aliases |
|---------|---------|
| `graph fvs` | `gfvs`, `feedback vertex set`, `graph fvset`, `gfvset`, `graph feedback vertex` |

**Display:** bright-red header (color 12); `fvs-member` role label per node; footer shows `FVS=N dag-status: cyclic/acyclic`.

---

## OS Analogy

**Minimum set of kernel subsystems to suspend / quarantine to break all boot-order dependency cycles.**  
Like running `systemctl mask` on cycle-causing services after `systemd-analyze verify` identifies the circular dependencies.

Complements `feedback arc` (V2.91): FAS removes edges (IPC channels), FVS removes vertices (subsystems).  
Both make the dependency graph a DAG, but they attack different structural elements.

---

## Key Invariants

- `fvs_size == 0` iff the graph is already a DAG (no cycles)
- Self-loops → `in_deg ≥ 1` → never drained → always in FVS
- Removing all `fvs_vecs[0..fvs_size]` leaves a DAG (acyclicity guarantee)
- `fvs_size ≤ node_count` (trivially)
- For K_n directed complete: `fvs_size == n-1` (optimal — any n-2 removal leaves 2 nodes in mutual cycle)
- Output sorted ascending by `VectorAddress.as_u64()`
- Pure read — does NOT bump graph epoch

---

## Test Suite: gos-graph-fvs-harness (10 tests)

| Test | Graph | Expected |
|------|-------|----------|
| 1 | Empty graph | `fvs_size=0` |
| 2 | Single node, no edges | `fvs_size=0` |
| 3 | DAG chain A→B→C→D | `fvs_size=0` |
| 4 | Self-loop A→A | `fvs_size=1`, FVS={A} |
| 5 | Mutual pair A↔B | `fvs_size=1` |
| 6 | Triangle A→B→C→A | `fvs_size=1` |
| 7 | Two disjoint cycles A↔B, C↔D | `fvs_size=2` |
| 8 | Diamond A→{B,C}→D + back-edge D→A | `fvs_size=1`, FVS∈{A,D} |
| 9 | K4 complete directed (12 edges) | `fvs_size=3 (=n-1)` |
| 10 | Cross-check: cyclic vs DAG vs mixed self-loop | all assertions pass |

**Result:** 10/10 tests pass.

---

## VectorAddress L4 Namespace (updated)

```
72=graph-indep, 73=graph-vc, 74=graph-domset, 75=graph-mpc,
76=graph-arborescence, 77=graph-fvs
```

---

## Host-Test Suite Totals

| Milestone | Tests | Notes |
|-----------|-------|-------|
| V3.00 | 973 | MSA (Chu-Liu/Edmonds) |
| V3.01 | **983** | +10 FVS tests |

---

## Literature

- **Karp 1972** — NP-completeness of minimum FVS and FAS (in 21 original NP-complete problems)
- **Erdős & Pósa 1965** — Erdős–Pósa theorem: FVS ≤ O(log n · OPT) for undirected graphs
- **Bafna, Berman & Fujito 1999** — 2-approximation for FVS via LP relaxation
- **Garey & Johnson 1979** — NP-hardness classification (problem [GT7])

---

## Relationship to Existing Algorithms

| Algorithm | Version | Removes | Goal |
|-----------|---------|---------|------|
| Feedback arc set | V2.91 | Edges | Break all directed cycles |
| **Feedback vertex set** | **V3.01** | **Vertices** | **Break all directed cycles** |
| Dominator tree | V2.90 | — | Single-entry domination structure |
| DAG layers | V2.89 | — | Assumes DAG; finds parallelism |
| Min path cover | V2.99 | — | Assumes DAG; min chain cover |

The FVS completes the cycle-breaking pair alongside FAS, giving operators the choice between removing edges (IPC channels) or vertices (subsystems) to achieve acyclicity.
