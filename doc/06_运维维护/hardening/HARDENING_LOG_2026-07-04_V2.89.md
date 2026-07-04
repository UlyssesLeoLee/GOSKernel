# Hardening Log V2.89 -- DAG Topological Layers / Parallel Execution Level Assignment

**Date:** 2026-07-04
**Branch:** feat/vk-auto-live-surface
**Host-test total:** 863 (853 prior + 10 new)

---

## Feature: `graph dag layers` / `gdaglayers` / `glayers` / `dag layers`

### Motivation

V2.88 answered "what is the longest serial path in the DAG?" (critical path depth).
V2.89 answers the complementary question: **"what is the earliest possible execution
level for every node?"** -- i.e., topological layer assignment for parallel scheduling.

| Question | OS analogy |
|---|---|
| What layer is this node in? | systemd unit ordering level -- which boot stage does this service belong to? |
| Which nodes can run in parallel? | Services in the same layer have no ordering constraints between each other |
| How many distinct levels exist? | Total number of sequential boot stages before the system is fully up |
| Is the graph a DAG? | Circular dependency check (like `cargo`'s cycle detection) |

Topological layers appear in:
- **Build systems** (`make -j`, `ninja`, `bazel`): compute build layers to maximise parallelism
- **Init systems** (`systemd`): assign each unit to a dependency level
- **Pipeline compilers**: assign operators to pipeline stages
- **Workflow engines** (`Airflow`, `Prefect`): schedule DAG tasks across parallel workers

In production graph platforms (NetworkX `dag_longest_path_length`, igraph `topological_sorting`),
level assignment is the standard primitive for parallel scheduling.

---

## Algorithm: Multi-source Kahn BFS with Layer Propagation (O(V+E))

### Distinction from V2.88 (DAG Longest Path)

| V2.88 `graph_dag_longest` | V2.89 `graph_dag_layers` |
|---|---|
| Returns a single `(path_hops, start_vec, end_vec)` | Returns `layer[v]` for every node v |
| Answers "how deep is the critical chain?" | Answers "what level does each node belong to?" |
| Useful for deadline/latency analysis | Useful for parallel work scheduling |

Both use Kahn BFS under the hood but differ in what they propagate:
- V2.88: propagates `dist[v] = max(dist[v], dist[u] + 1)` with predecessor tracking
- V2.89: propagates `layer[v] = max(layer[v], layer[u] + 1)` for all nodes, no path tracing

### Step 1 -- In-Degree Census

Scan all edges once. Self-loops (`from == to`) are included in in-degree counts so that
Kahn's BFS can never drain a self-loop node (it stays at `in_deg >= 1` forever),
causing `processed < node_count` -> `is_dag = false`. Same invariant as V2.88.

### Step 2 -- Kahn BFS Seeded from All Sources

Initialize `layer[v] = u32::MAX` (unvisited). Seed the BFS queue with all nodes
whose `in_deg == 0`; assign them `layer = 0`.

For each dequeued node `u`:
- For each directed edge `u -> v` (self-loops skipped in relaxation):
  - `in_deg[v] -= 1`
  - `layer[v] = max(layer[v], layer[u] + 1)`  -- propagate deepest predecessor
  - If `in_deg[v] == 0`: update `max_layer`, enqueue `v`

### Step 3 -- Cycle Check

If `processed < node_count`, a cycle exists. Return `(_, _, node_count, 0, false)`.

### Step 4 -- Sorted Output

Sort the node array ascending by `(layer[v], v.as_u64())` for deterministic output.
Pack at most `N` entries into the output arrays.

`layer_count = max_layer + 1` (layer 0 through max_layer inclusive).

---

## Return Signature

```rust
pub fn graph_dag_layers<const N: usize>()
    -> ([VectorAddress; N], [u32; N], usize, u32, bool)
//     ^^^^^^^^^^^^^^^^     ^^^^^^^^  ^^^^^  ^^^  ^^^^^
//     vecs                 layers    nc     lc   is_dag
```

| Field | Type | Meaning |
|---|---|---|
| `vecs[0..nc]` | `[VectorAddress; N]` | Live nodes sorted by layer then VectorAddress |
| `layers[0..nc]` | `[u32; N]` | Layer number for each node (0 = source, 1 = one hop, ...) |
| `node_count` | `usize` | Total live nodes |
| `layer_count` | `u32` | Number of distinct layers (= max_layer + 1); 0 if cyclic |
| `is_dag` | `bool` | False iff the graph contains a directed cycle (layers undefined) |

---

## Shell Display

For a diamond DAG (A->{B,C}->D):
```
 graph dag layers
 -----------------------------------------------------------
  layer  vector
  -----  ------------
      0  65.1.1.0

      1  65.1.2.0
      1  65.1.3.0

      2  65.1.4.0
 -----------------------------------------------------------
  nodes: 4   layers: 3
```

For a cyclic graph:
```
  x graph has directed cycles (not a DAG)
  topological layers are undefined for cyclic graphs
  use `graph scc` or `graph dag longest` to inspect structure
```

For an empty graph:
```
  (no nodes registered)
```

---

## Test Coverage (gos-graph-dag-layers-harness, L4=65)

| # | Scenario | Expected |
|---|---|---|
| 1 | Empty graph | is_dag=true, nc=0, layer_count=0 |
| 2 | Single isolated node | layer=0, layer_count=1 |
| 3 | Self-loop A->A | is_dag=false |
| 4 | Single edge A->B | layer[A]=0, layer[B]=1, layer_count=2 |
| 5 | Linear chain A->B->C->D | layers=[0,1,2,3], layer_count=4 |
| 6 | Diamond A->{B,C}->D | A=0, B=C=1, D=2, layer_count=3 |
| 7 | Directed 3-cycle A->B->C->A | is_dag=false |
| 8 | Shortcut DAG A->B->C + A->C | C gets layer 2 (deepest predecessor wins, not shortcut's 1) |
| 9 | Star fan-out A->{B,C,D} | A=0, B=C=D=1, layer_count=2 |
| 10 | Two independent chains A->B and C->D->E->F | layer_count=4 (from longer chain); both chains correctly layered |

All 10 tests pass.

---

## Key Invariants

- Self-loops included in in-degree -> cycle detected via `processed < node_count`
- Self-loops skipped in BFS relaxation (no spurious `layer += 1` on self-edges)
- `layer[v]` initialized to `u32::MAX` (unvisited sentinel); set to `0` for sources
- Layer propagation: `layer[v] = max(layer[v], layer[u] + 1)` -- deepest predecessor wins
- `layer_count = max_layer + 1` (layer 0 is the first, layer_count is exclusive upper bound)
- Output sorted by `(layer, VectorAddress.as_u64())` for stable, deterministic ordering
- Blank separator rows in shell output between different layers for readability
- When `is_dag=false`: returns `(empty, empty, node_count, 0, false)` -- no partial layer data

---

## VectorAddress L4 Namespace

- **L4=64**: `gos-graph-dag-longest-harness` (V2.88)
- **L4=65**: `gos-graph-dag-layers-harness` (V2.89, new)

---

## Literature

- Kahn, A. B. (1962). "Topological sorting of large networks." *CACM* 5(11):558-562.
- Coffman, E. G. & Graham, R. L. (1972). "Optimal scheduling for two-processor systems." *Acta Informatica* 1:200-213.
- List scheduling / critical path scheduling: standard DAG parallelism concept in OS scheduling theory.
- `systemd --analyze` unit ordering levels; `make -jN` parallel build levels.

---

## OS Analogy Mapping

| Graph property | OS equivalent |
|---|---|
| `layer_count` | Number of sequential boot stages in the init dependency graph |
| `layer[v] == 0` | Root service (kernel drivers, early udev, hwclock) -- starts immediately |
| `layer[v] == k` | Service in the k-th boot stage (must wait for all k-1 stages to complete) |
| Two nodes with the same layer | Services that can start in parallel (no dependency between them) |
| `is_dag=false` | Circular dependency detected -- init system would deadlock (like `systemd` circular dep warning) |
