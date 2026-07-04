# Hardening Log V2.88 — DAG Longest Path / Critical Path Analysis

**Date:** 2026-07-04  
**Branch:** feat/vk-auto-live-surface  
**Host-test total:** 853 (843 prior + 10 new)

---

## Feature: `graph dag longest` / `gdaglongest` / `critical path` / `graph critical` / `gcritical`

### Motivation

V2.85–V2.87 added structural analysis primitives (articulation points, bridges, Eulerian circuits).
V2.88 adds a **scheduling and planning** primitive: **DAG longest path / critical path analysis**
— answering the minimum serial depth any parallel schedule must traverse in a dependency graph.

This is the fundamental question behind parallel build systems (`make -j`), boot sequencers
(`systemd-analyze critical-chain`), and PERT/CPM project scheduling:

| Question | OS analogy |
|---|---|
| What is the critical path length? | `systemd-analyze critical-chain`: minimum wall-clock depth for parallel boot |
| Is the graph a DAG? | Is the dependency graph free of circular dependencies (like `cargo`'s deadlock check)? |
| Where does the critical path start/end? | Which leaf service and which root service define the unavoidable depth? |

In production graph platforms (NetworkX, igraph), DAG longest path is a core planning primitive
used in task scheduling, compilation ordering, and dataflow analysis.

---

## Algorithm: Kahn's BFS Topological Sort + Distance DP (O(V+E))

The critical path algorithm combines **DAG detection** and **longest path DP** in a single pass:

### Step 1 — In-Degree Census

Scan the edge table once to compute `in_deg[v]` for every live node.

**Self-loop handling:** Self-loops (`from == to`) are included in the in-degree count.
This prevents Kahn's BFS from ever draining a self-loop node — it stays stuck at
`in_deg ≥ 1`, is never emitted, and causes `processed < node_count → is_dag = false`.
This is correct: a self-loop IS a directed cycle of length 1.

### Step 2 — Kahn's BFS with Distance DP

Seed the BFS queue with all `in_deg == 0` nodes (initial sources, `dist = 0`).

For each emitted node `u`:
- For each edge `u → v` (skipping self-loops in relaxation):
  - `dist[v] = max(dist[v], dist[u] + 1)`  — DP relaxation
  - `pred[v] = u` if `dist[u] + 1 > dist[v]` — predecessor tracking
  - Decrement `in_deg[v]`; if `in_deg[v] == 0`, enqueue `v`

### Step 3 — DAG Check

If `processed_count < node_count`, at least one node could not be drained — a cycle exists.
Return `(0, false, zero, zero, node_count)`.

### Step 4 — Critical Path Extraction

Find the node `end_slot` with the maximum `dist` value (tie-break: smallest slot index
for determinism). Trace back through `pred[]` until `pred[cur] ≥ MAX_NODES` to find
`start_slot` (the source of the critical path).

**Vacuous case:** If `max_dist == 0` (no edges, or all isolated nodes), return
`(0, true, zero, zero, node_count)` — the graph is a trivial DAG with no path.

---

## Return Signature

```rust
pub fn graph_dag_longest() -> (u32, bool, VectorAddress, VectorAddress, usize)
//                             ^^^^  ^^^^^^ ^^^^^^^^^^^^ ^^^^^^^^^^^^ ^^^^^^^
//                        path_hops is_dag  start_vec    end_vec      node_count
```

| Field | Type | Meaning |
|---|---|---|
| `path_hops` | `u32` | Hop count of longest directed path; 0 if no edges or graph has cycle |
| `is_dag` | `bool` | True iff no directed cycles (self-loops included) |
| `start_vec` | `VectorAddress` | Source of the critical path; zero if no path |
| `end_vec` | `VectorAddress` | Sink of the critical path; zero if no path |
| `node_count` | `usize` | Total live nodes |

---

## Shell Display

```
 graph dag longest
 ───────────────────────────────────────────────────────────
  ✓ DAG  critical path: 3 hops
  start  64.1.1.0   end  64.1.4.0
  (minimum serial depth any parallel schedule must traverse)
 ───────────────────────────────────────────────────────────
  is_dag: yes   nodes: 4
```

For a cyclic graph:
```
  ✗ graph has directed cycles (not a DAG)
  critical path is undefined for cyclic graphs
  use `graph cycles` or `graph scc` to inspect cycles
```

For an empty/isolated graph:
```
  — no directed edges (trivial DAG)
  all nodes are isolated; critical path length = 0
```

---

## Test Coverage (gos-graph-dag-longest-harness, L4=64)

| # | Scenario | Expected |
|---|---|---|
| 1 | Empty graph | is_dag=true, path_hops=0, nc=0 |
| 2 | Single isolated node (no edges) | is_dag=true, path_hops=0, nc=1 |
| 3 | Single self-loop A→A | is_dag=false, path_hops=0 |
| 4 | Linear chain A→B→C→D | is_dag=true, path_hops=3, start=A, end=D |
| 5 | Diamond A→B, A→C, B→D, C→D | is_dag=true, path_hops=2, start=A, end=D |
| 6 | Two independent chains (A→B) and (C→D→E) | is_dag=true, path_hops=2, end=E |
| 7 | Directed 3-cycle A→B→C→A | is_dag=false, path_hops=0 |
| 8 | DAG with shortcut A→B→C + A→C | is_dag=true, path_hops=2, start=A, end=C |
| 9 | Star fan-out A→{B,C,D,E} | is_dag=true, path_hops=1, start=A |
| 10 | Chain of 5 hops A→B→C→D→E→F | is_dag=true, path_hops=5, start=A, end=F |

All 10 tests pass.

---

## Key Invariants

- Self-loops included in in-degree computation → correctly detected as cycles (is_dag=false)
- Self-loops skipped in BFS relaxation step (no infinite distance updates)
- `pred[v] ≥ MAX_NODES` sentinel means "v has no predecessor" (is a source node)
- Tie-breaking for max-dist: smallest slot index → deterministic end_vec selection
- Vacuous DAG (no edges): path_hops=0, is_dag=true, start/end=zero
- Cyclic graph: path_hops=0, is_dag=false, start/end=zero

---

## VectorAddress L4 Namespace

- **L4=64**: `gos-graph-dag-longest-harness`

---

## Literature

- Kahn, A. B. (1962). "Topological sorting of large networks." *CACM* 5(11):558–562.
- CPM / Critical Path Method: Kelley & Walker (1959), DuPont Engineering.
- `systemd-analyze critical-chain` — Linux parallel boot critical path inspector.

---

## OS Analogy Mapping

| Graph property | OS equivalent |
|---|---|
| `is_dag=true` | Dependency graph is acyclic (like `cargo check`, `tsort`) |
| `is_dag=false` | Circular dependency detected (like `cargo` "cycle detected" error) |
| `path_hops` | Minimum parallel boot depth (`systemd-analyze critical-chain` length) |
| `start_vec` | Root boot service (kernel driver / hwclock) |
| `end_vec` | Terminal service (login manager / display server) |
