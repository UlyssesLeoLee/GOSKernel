# GOS Hardening Log — V2.37 — 2026-07-02

## Summary

V2.37 adds bipartite-check analysis via `graph bipartite` — a fundamental graph
property query answering "can the live dependency graph be split into two
non-conflicting scheduling tiers?"  A graph is bipartite iff it contains no
odd-length cycle.  The algorithm runs on the *undirected* projection of the
directed live graph (every directed edge is treated as bidirectional), using
BFS 2-coloring.  This completes the graph structural analysis quintet begun in
V2.32 (cycle detection).

OS analogy: checking whether a service dependency graph can be cleanly split
into producers/consumers, or whether a module load order has odd-length
circular dependencies that block clean tier separation.

---

## Changes

### 1. `graph_bipartite_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New method on `GraphState` (inserted after `graph_reachable_inner`):

```rust
pub fn graph_bipartite_inner<const N: usize>(
    &self,
) -> ([VectorAddress; N], [u8; N], usize, bool)
```

**Algorithm**: BFS 2-coloring on the undirected projection of the live directed
graph. O(V+E), no_std safe, fixed-size stack arrays (no heap allocation).

**Invariants**:
- Every directed edge is treated as undirected (both `from→to` and `to→from`
  neighbors are explored from each node).
- Self-loops are skipped.
- Each connected component is seeded independently (handles disconnected graphs).
- When a conflict is found (same-color neighbors), `is_bipartite = false` is
  set but BFS continues — so `total` is always correct regardless of the result.

**Return layout** (consistent with `graph_scc` / `graph_condensation` convention):
- `vecs[0..total]`   — live node vectors in slot order.
- `colors[0..total]` — 0 = set A, 1 = set B (meaningful only when is_bipartite).
- `total`            — number of live nodes packed.
- `is_bipartite`     — true iff the graph admits a valid 2-coloring.

### 2. `pub fn graph_bipartite<const N>()` — gos-runtime public API

```rust
pub fn graph_bipartite<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, bool) {
    RUNTIME.lock().graph_bipartite_inner()
}
```

One-liner wrapper consistent with `graph_scc`, `graph_condensation`,
`graph_reachable`.

### 3. `dispatch_graph_bipartite` — k-shell (`crates/k-shell/src/lib.rs`)

New shell-level dispatch function (inserted before `dispatch_uname`).

Output format:
```
 graph bipartite
 ───────────────────────────────────────────────────────────
  result:   bipartite
  set A (3):  15.0.0.1  15.0.0.3  15.0.0.5
  set B (2):  15.0.0.2  15.0.0.4
 ───────────────────────────────────────────────────────────
  5 node(s) checked
```

```
 graph bipartite
 ───────────────────────────────────────────────────────────
  result:   NOT bipartite  (odd-length cycle detected)
  hint: use 'graph cycles' to find the cycle, 'graph scc' for components
 ───────────────────────────────────────────────────────────
  3 node(s) checked
```

Pure read — no epoch bump, no write ops.

### 4. Command routing — k-shell (`crates/k-shell/src/proc.rs`)

Aliases wired after the `graph condensation` branch:

```
graph bipartite  |  bipartite  |  graph bip  |  bip
```

Four aliases follow the same short-alias pattern as prior graph commands.

### 5. `gos-graph-bipartite-harness` — new host-test crate

`host-tests/gos-graph-bipartite-harness/` — 10 integration tests covering:

| # | Scenario | Expected |
|---|----------|----------|
| 1 | Empty graph | bipartite (vacuously) |
| 2 | Single isolated node | bipartite |
| 3 | Single edge A→B | bipartite |
| 4 | Path A→B→C | bipartite |
| 5 | Triangle A→B→C→A | NOT bipartite (odd cycle 3) |
| 6 | 4-cycle A→B→C→D→A | bipartite |
| 7 | 4-cycle + chord A→C | NOT bipartite (3-cycle) |
| 8 | Two disconnected bipartite components | bipartite |
| 9 | Star K₁,₄: centre→{A,B,C,D} | bipartite |
| 10 | Color assignment for path A→B→C | A,C same set; B opposite |

All 10 tests: **PASS** (`cargo +nightly test` in harness dir).

---

## Test Results

```
running 10 tests
test color_assignment_correct_for_path ... ok
test disconnected_bipartite_components_are_bipartite ... ok
test empty_graph_is_bipartite ... ok
test four_cycle_is_bipartite ... ok
test four_cycle_with_chord_is_not_bipartite ... ok
test path_three_nodes_is_bipartite ... ok
test single_edge_is_bipartite ... ok
test single_node_is_bipartite ... ok
test star_graph_is_bipartite ... ok
test triangle_is_not_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

---

## Shell Command Surface (V2.37 additions)

| Command | Aliases | Description |
|---------|---------|-------------|
| `graph bipartite` | `bipartite`, `graph bip`, `bip` | 2-coloring check — is the graph bipartite? Shows set A / set B when yes, odd-cycle hint when no. |

---

## Invariants Preserved

- `dispatch_graph_bipartite` is a pure read — no epoch bump, no write ops.
- Uses existing `TEST_LOCK: Mutex<()>` + `reset()` isolation pattern.
- Harness `.cargo/config.toml` sets `target = "x86_64-pc-windows-msvc"` +
  `build-std = ["std", "panic_abort"]`.
- Version number: V2.37 (sequential after V2.36 graph-reachable).

---

## Next Steps

- `graph degree` / `graph centrality` — in/out degree per node, hub identification
- `node checkpoint <vec>` — snapshot node state to diff ring
- `journal ring <N>` — runtime-configurable JournalRing capacity
- PAL_U32 → attribute node refactor (Demo A prerequisite)
