# GOS Hardening Log — V2.39 — 2026-07-02

## Summary

V2.39 adds betweenness centrality via `graph centrality` — answering "which node
sits on the most shortest communication paths in the kernel service graph?"

Betweenness centrality BC[v] = Σ_{s≠v≠t} σ(s,t,v)/σ(s,t) where σ(s,t) is the
number of shortest directed paths from s to t and σ(s,t,v) is the count of those
that pass through v.  A node with high BC is a structural bottleneck: removing
it disrupts the most inter-node routing paths.

Implementation uses Brandes' 2001 algorithm (O(V×E), directed, unweighted) with
fixed-point arithmetic (SCALE = 1_000_000) to avoid floating-point in the no_std
kernel context.

OS analogies: `traceroute` hop-frequency analysis, BGP betweenness in network
topology, or `htop` process tree critical-path identification.

---

## Changes

### 1. `graph_centrality_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New method on `GraphState` (inserted after `graph_degree_inner`):

```rust
pub fn graph_centrality_inner<const N: usize>(
    &self,
) -> ([VectorAddress; N], [u32; N], usize)
```

**Algorithm: Brandes' betweenness centrality (directed, unweighted)**

For each source node s:

1. **BFS forward phase** — compute `dist[v]` (shortest-path distance from s to v)
   and `sigma[v]` (number of distinct shortest paths from s to v).
   Accumulate BFS traversal order in `bfs_ord[]` for the back-propagation phase.

2. **Back-propagation phase** (reverse BFS order) — for each node w ≠ s,
   scan all in-edges (v → w) where `dist[w] == dist[v] + 1` (i.e., v is a
   predecessor of w in the s-rooted shortest-path DAG):
   ```
   delta[v] += (sigma[v] / sigma[w]) × (SCALE + delta[w])
   ```
   This is the Brandes pair-dependency recurrence, computed in integer arithmetic
   with fixed-point scaling (SCALE = 1_000_000) to avoid fractions.
   Accumulate into `bc_scaled[w] += delta[w]` for each w ≠ s.

3. **Output** — divide `bc_scaled[slot] / SCALE` to get the integer truncation
   of BC[v].  Nodes are sorted descending by raw `bc_scaled` (not truncated)
   to ensure that ties from fixed-point rounding preserve the natural ordering.

**Complexity**: O(V × (V + E)) — O(V) BFS passes, each O(V + E).
For V=128, E=512: ~81K operations per BFS × 128 sources = ~10M ops total.
Acceptable for the kernel's MAX_NODES=128 / MAX_EDGES=512 bounds.

**Overflow safety**: `sigma` uses `u32` with `saturating_add`; `delta` and
`bc_scaled` use `u64` with `saturating_mul`/`saturating_add`.

**Return layout**:
- `vecs[0..total]` — live node vectors, descending betweenness order.
- `bc[0..total]`   — truncated integer betweenness per node (raw_scaled / SCALE).
- `total`          — number of live nodes packed.

### 2. `pub fn graph_centrality<const N>()` — gos-runtime public API

```rust
pub fn graph_centrality<const N: usize>() -> ([VectorAddress; N], [u32; N], usize) {
    RUNTIME.lock().graph_centrality_inner()
}
```

One-liner wrapper consistent with `graph_degree`, `graph_bipartite`, etc.

### 3. `dispatch_graph_centrality` — k-shell (`crates/k-shell/src/lib.rs`)

New shell-level dispatch function (inserted before `dispatch_uname`).

Output format (colour-coded: yellow=bottleneck, cyan=relay, grey=endpoint):
```
 graph centrality
 ───────────────────────────────────────────────────────────
  vector                bc    role
  16.1.7.0               9  bottleneck
  16.1.1.0               0  endpoint
  16.1.2.0               0  endpoint
  16.1.3.0               0  endpoint
  16.1.4.0               0  endpoint
  16.1.5.0               0  endpoint
  16.1.6.0               0  endpoint
 ───────────────────────────────────────────────────────────
  7 node(s)  max-bc: 9  bottlenecks: 1
```

Role annotations:
- **bottleneck** — BC == max_bc > 0: the most critical routing intermediary.
- **relay** — BC > 0 but not the maximum: carries some cross-node traffic.
- **endpoint** — BC = 0: leaf, source, sink, or isolated node.

Also adds `print_num_right6()` helper for 6-column right-aligned numbers.

Pure read — no epoch bump, no write ops.

### 4. Command routing — k-shell (`crates/k-shell/src/proc.rs`)

Aliases wired after the `graph degree` branch:

```
graph centrality  |  centrality  |  graph central  |  central  |  betweenness
```

### 5. `gos-graph-centrality-harness` — new host-test crate

`host-tests/gos-graph-centrality-harness/` — 10 integration tests covering:

| # | Scenario | Expected |
|---|----------|----------|
| 1 | Empty graph | total=0, no panics |
| 2 | Single isolated node | BC=0, total=1 |
| 3 | Two nodes A→B | BC[A]=BC[B]=0 (no intermediary possible) |
| 4 | Path A→B→C | BC[B]=1 (B is sole intermediary for A→C) |
| 5 | Bottleneck {A,B}→X→{C,D} | BC[X]=4 (4 cross-pairs all through X) |
| 6 | Bottleneck {A,B,C}→X→{D,E,F} | BC[X]=9 (9 cross-pairs all through X) |
| 7 | Linear 5-node A→B→C→D→E | BC[C]=4, BC[B]=BC[D]=3, BC[A]=BC[E]=0 |
| 8 | Fork-join A→{B,C}→E→F | BC[E]=3 (post-fork bottleneck for all paths to F) |
| 9 | Sort order verified | output strictly non-increasing by BC |
| 10 | Self-loop A→A + A→B | BC[A]=0 (self-loop not a valid s≠v≠t path) |

All 10 tests: **PASS**.

---

## Test Results

```
running 10 tests
test bottleneck_three_into_three_centrality_nine ... ok
test bottleneck_two_into_two_centrality_four ... ok
test empty_graph_centrality_total_is_zero ... ok
test fork_join_bottleneck_centrality ... ok
test isolated_node_has_zero_centrality ... ok
test linear_five_node_path_centrality_values ... ok
test output_sorted_descending_by_bc_score ... ok
test path_abc_middle_node_centrality_is_one ... ok
test self_loop_does_not_panic_and_bc_is_zero ... ok
test two_nodes_one_edge_both_zero_centrality ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## Shell Command Surface (V2.39 additions)

| Command | Aliases | Description |
|---------|---------|-------------|
| `graph centrality` | `centrality`, `graph central`, `central`, `betweenness` | Brandes' betweenness centrality sorted descending; bottleneck/relay/endpoint role annotation |

---

## Betweenness Centrality — Worked Examples

### Path graph A→B→C→D→E (test 7)

BC[B] = 3: pairs (A,C), (A,D), (A,E) each have their unique shortest path go
through B. From any other source, B is not on a shortest path (B can't be
reached from C, D, or E in a directed path graph).

BC[C] = 4: pairs (A,D), (A,E), (B,D), (B,E) — C is the structural middle.

BC[D] = 3: pairs (A,E), (B,E), (C,E) — D is the penultimate node.

### Bottleneck graph {A,B,C}→X→{D,E,F} (test 6)

X is the *only* path from any source-tier node to any sink-tier node.
9 ordered pairs (A,D), (A,E), (A,F), (B,D), (B,E), (B,F), (C,D), (C,E), (C,F)
each contribute σ(s,t,X)/σ(s,t) = 1/1 = 1 to BC[X]. Sum = 9.

---

## Invariants Preserved

- `dispatch_graph_centrality` is a pure read — no epoch bump, no write ops.
- Uses existing `TEST_LOCK: Mutex<()>` + `reset()` isolation pattern.
- Harness `.cargo/config.toml` sets `target = "x86_64-pc-windows-msvc"` +
  `build-std = ["std", "panic_abort"]`.
- Version number: V2.39 (sequential after V2.38 graph-degree).
- All arithmetic uses saturating operations to prevent overflow in dense graphs.
- SCALE = 1_000_000 preserves fractional accuracy for graphs with multiple
  equal-length paths (diamond topologies, parallel routes).

---

## Next Steps

- `node checkpoint <vec>` — snapshot node state to diff ring (observability)
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph closeness` — closeness centrality (inverse sum of shortest-path distances)
- PAL_U32 → attribute node refactor (Demo A prerequisite)
