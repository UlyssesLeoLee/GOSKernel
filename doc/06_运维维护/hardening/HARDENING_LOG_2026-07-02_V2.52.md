# HARDENING LOG — V2.52
**Date:** 2026-07-02  
**Branch:** feat/vk-auto-live-surface  
**Author:** Scheduled Hardening Bot (auto-2h)

---

## Summary

**V2.52: `graph sim <N>` — Random Walk Signal Traffic Simulation**

Implements a directed random-walk simulator over the live kernel graph,
reporting per-node visit counts sorted descending.  Identifies which graph
nodes attract the most traffic under simulated random signal load — the
kernel-native equivalent of `strace -e trace=signal`.

---

## Changes

### crates/gos-runtime/src/lib.rs

**New internal method: `GraphRuntime::graph_sim_inner<N>`** (inside `impl GraphRuntime`)

Algorithm:
1. Seed a deterministic xorshift32 PRNG from `seed` (0 is mapped to `0xDEAD_BEEF`).
2. Start at a random live node: `node_slots[xorshift32() % n]`.
3. Record initial visit (`raw_visits[cur_slot] += 1`).
4. For each of `steps` iterations:
   - Collect all live outgoing edges from `cur_slot` (matching `edge_from == slot_id[cur_slot]`).
   - Sum edge weights (× 1000 as u32; 0-weight edges count as 1).
   - If no outgoing edges: **teleport** — pick a random live node, increment its count, `stuck_steps++`.
   - Otherwise: sample edge proportional to weight using `xorshift32() % total_w`, traverse, increment target count, `actual_steps++`.
5. Sort per-slot visit counts into output arrays by insertion sort (descending).

**New public API: `pub fn graph_sim<const N: usize>(steps: u32, seed: u32) -> (...)`**

- `steps` is clamped to 256 before calling `graph_sim_inner`.
- Returns `(vecs, visits, node_count, actual_steps, stuck_steps)`.

**Key invariant (provably correct):**
```
sum(visits[0..n]) == 1 + actual_steps + stuck_steps == 1 + min(steps, 256)
```
Each of the N steps increments exactly one visit counter (the teleport destination or the traversal destination), plus the initial starting position.

### crates/k-shell/src/lib.rs

**New `pub fn dispatch_graph_sim(sink: &ConsoleSink, steps: u32)`**

Shell output format:
```
 graph sim  steps=32  seed=3735928559
 ───────────────────────────────────────────────────────────
  rank  visits  vector
     1      14  1.0.0.1        ← magenta (rank 1)
     2       9  6.1.0.0        ← cyan (rank 2-3)
     3       5  2.0.0.1
     4       4  3.0.0.1        ← white (rank 4+)
 ───────────────────────────────────────────────────────────
 4 node(s)  31 walk steps  1 teleport(s)
```
Footer shows either `N teleport(s)` (yellow) or `no dead ends` (green).

### crates/k-shell/src/proc.rs

New shell routing:
```
graph sim           → dispatch_graph_sim(sink, 16)   [default 16 steps]
sim                 → dispatch_graph_sim(sink, 16)
gsim                → dispatch_graph_sim(sink, 16)
graph walk          → dispatch_graph_sim(sink, 16)
walk                → dispatch_graph_sim(sink, 16)
graph sim <N>       → dispatch_graph_sim(sink, N.min(256).max(1))
sim <N>             → same
gsim <N>            → same
graph walk <N>      → same
walk <N>            → same
```
Invalid N (non-numeric) prints an error in red.

---

## New Harness: host-tests/gos-graph-sim-harness

**10 tests — all green**

| # | Test | What it verifies |
|---|------|-----------------|
| 1 | `empty_graph_returns_all_zeros` | Empty graph → node_count=0, actual=0, stuck=0 |
| 2 | `zero_steps_returns_all_zeros` | steps=0 → all zero (early return path) |
| 3 | `single_node_no_edges_all_stuck` | Dead-end node: stuck=8, actual=0, visits[0]=9 |
| 4 | `single_node_self_loop_no_stuck` | Self-loop: stuck=0, actual=8, visits[0]=9 |
| 5 | `steps_clamped_to_256` | steps=999 → actual+stuck ≤ 256 |
| 6 | `visit_sum_invariant_linear_dag` | 3-node DAG: sum(visits) == 1+steps |
| 7 | `actual_plus_stuck_equals_steps` | Step accounting: actual+stuck == min(steps,256) |
| 8 | `node_count_matches_registered` | 4 nodes registered → node_count=4 |
| 9 | `output_sorted_descending` | visits[i] ≥ visits[i+1] for all i |
| 10 | `two_cycle_sum_invariant_and_sorted` | 2-cycle: no stuck, sum invariant, sorted |

L4 namespace: **29** (reserved for graph-sim harness tests)

---

## Host Test Suite

| Before V2.52 | After V2.52 |
|---|---|
| 483 tests | **493 tests** (483 + 10 new) |

---

## Design Notes

### PRNG choice: xorshift32
- `no_std` safe — no heap, no OS entropy.
- Period 2³²−1 — sufficient for 256 steps.
- Deterministic given `seed` — test-friendly.
- The public API mixes `graph_epoch ^ steps ^ 0xDEAD_BEEF` as the seed so
  repeated shell invocations vary without needing a hardware clock.

### Teleportation semantics
Dead-end nodes trigger a uniform-random teleport (analogous to PageRank's
damping factor `d`).  This ensures the walker doesn't get permanently stuck
in isolated sub-graphs and keeps the invariant `actual + stuck == steps` clean.

### Weight-proportional sampling
Edge weights from `EdgeSpec.weight` (f32) are scaled × 1000 to u32 before
sampling.  Zero-weight edges are treated as weight=1 to avoid excluding
graph structure that was registered with default weight.

### No epoch bump
`graph_sim` is a pure read — it does NOT touch `graph_epoch`, the diff ring,
or any mutable state.  It is safe to call at any frequency.

### OS analogy
`graph sim` is the kernel-topology equivalent of:
- `strace -e trace=signal` — which subsystems are on the hot signal path?
- `perf record -g` → `perf report` — who dominates the call graph?
- `netstat -s | grep segments` — which network nodes handle most traffic?

---

## Next Steps

- V2.53: `graph between` — all-pairs Dijkstra betweenness centrality (directed, weighted)
- PAL_U32 → attribute node refactor (Demo A prerequisite)
