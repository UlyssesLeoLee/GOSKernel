# GOSKernel Hardening Log — V3.02
**Date:** 2026-07-06  
**Algorithm:** Global Minimum Cut — Stoer-Wagner 1997  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.02): global min cut -- Stoer-Wagner 1997 + gos-graph-min-cut-harness (10 tests)

---

## Summary

V3.02 adds **global minimum edge cut** (Stoer-Wagner 1997) to the GOSKernel graph theory runtime.  The minimum cut κ'(G) is the smallest number of undirected edges whose removal disconnects the graph — the edge connectivity.

This completes the **fault-isolation toolkit**:
- V2.86 — graph bridges: individual cut-edges (1-edge-connectivity gaps)
- V2.93 — 2-edge-connected components: clusters resilient to any single link failure
- V3.02 — global minimum cut: the precise minimum partition cost κ'(G)

---

## Public API

### `gos_runtime::graph_min_cut<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, u32, usize)`

Returns `(vecs, sides, node_count, min_cut, side_b_size)`:
- `vecs[0..node_count]` — all live nodes; side-A (sides==0) first, side-B (sides==1) after
- `sides[0..node_count]` — partition assignment: 0=side A, 1=side B
- `node_count` — total live nodes
- `min_cut` — minimum undirected edge cut = edge connectivity κ'(G)
- `side_b_size` — count of nodes on side B

**Undirected projection:** A→B and B→A count as one edge (deduped via seen_adj u128 bitmask).  
**Disconnected graphs:** `min_cut = 0` (already partitioned at zero cost).

---

## Algorithm: Stoer-Wagner 1997

**Complexity:** O(V² × E) with edge-list representation — V-1 phases, each O(V × E).  
For GOSKernel with MAX_NODES=128, MAX_EDGES=512: ≤ 128² × 512 = 8M ops.

**Each phase:**
1. **Maximum adjacency ordering:** greedily add the active non-A node `best` with highest `key[best]` (sum of edge weights to already-added nodes in A) until all active nodes are in A.  Ties broken by smallest compact index.
2. **Cut-of-phase:** `key[last_t]` = total weight of edges from the last-added node to the rest.  If this is a new minimum, record it as `min_cut` and save `group_members[last_t]` as `best_b_mask`.
3. **Merge:** Redirect all edges of `last_t` to `last_s`; kill self-loops; deduplicate parallel edges by summing weights.

**Partition tracking:** `group_members[si]` is a u128 bitmask of original compact indices in super-node `si`.  When a new minimum is found, `best_b_mask = group_members[last_t]` captures the B-side partition at that phase.

**Stack usage (no heap):**
- `uf, ut: [u8; MAX_EDGES]` = 512 bytes each (compact-index endpoints, u8 since ci < 128)
- `uw: [u16; MAX_EDGES]` = 1024 bytes (weights, u16 since max accumulated = N-1 < 65535)
- `ue_live: [bool; MAX_EDGES]` = 512 bytes
- `seen_adj: [0u128; MAX_NODES]` = 2048 bytes (undirected-pair dedup bitmask)
- `group_mbrs: [0u128; MAX_NODES]` = 2048 bytes (super-node membership tracking)
- `key, in_a`: per-phase arrays = ~384 bytes
- Total: ~8 KB (well within kernel stack budget)

**Key invariants:**
- Cut-of-phase = `key[last_t]` = exact Stoer-Wagner phase value
- `best_b_mask` always corresponds to the phase that achieved `min_cut`
- For K_n: min_cut = n-1 (each node has degree n-1, isolating one costs n-1 edges)
- For paths/trees: min_cut = 1 (bridge is the minimum cut)
- For disconnected graphs: min_cut = 0 (first phase gives key[last_t]=0)

---

## Shell Commands

- `graph min cut` — show global minimum edge cut with partition A/B
- `gmincut` — alias
- `min cut` — alias
- `edge connectivity` — alias
- `gedge connectivity` — alias
- `graph cut` / `gcut` — aliases

**Display:** bright-cyan header; side-A nodes in bright-green, side-B in bright-magenta; footer shows `κ'(G)=<value>  Stoer-Wagner 1997`.

---

## VectorAddress L4 Namespace

L4=78 for `gos-graph-min-cut-harness`

---

## OS Analogy

`graph min cut` = **minimum fault-isolation boundary** — the fewest IPC channels to sever to partition the kernel into two fully independent fault domains.  Like `ip link set <iface> down` on the minimum set of network interfaces to split a cluster into two isolated segments.

Complements:
- `graph bridges` (V2.86) — individual 1-edge cuts (λ=1 bottlenecks)
- `graph 2ecc` (V2.93) — components resilient to any single link failure
- `graph flow` (V2.50) — max-flow / min-cut between a specific source-sink pair

---

## Literature

- Stoer & Wagner 1997 — "A simple min-cut algorithm", J. ACM 44(4):585–591
- Nagamochi & Ibaraki 1992 — MA-ordering (maximum adjacency) used by Stoer-Wagner
- Ford & Fulkerson 1956 — max-flow min-cut theorem (global cut ≤ max s-t flow)
- Whitney 1932 — edge connectivity defined; κ'(G) = min over all vertex pairs of max s-t flow

---

## Test Suite

10 host tests in `gos-graph-min-cut-harness`:

| # | Graph | Expected min_cut |
|---|-------|-----------------|
| 1 | Empty | 0 |
| 2 | Single node | 0 |
| 3 | Two nodes, no edges | 0 (disconnected) |
| 4 | K2 (one edge) | 1 |
| 5 | Path A-B-C | 1 (bridge) |
| 6 | Triangle K3 | 2 |
| 7 | K4 complete | 3 |
| 8 | Two triangles + bridge | 1 (bridge) |
| 9 | Star K_{1,4} | 1 (leaf degree) |
| 10 | Square C4 + partition invariant | 2 |

All 10 tests pass (`cargo test` in harness directory).
