# HARDENING LOG — V2.51 — node checkpoint observability

**Date:** 2026-07-02
**Branch:** feat/vk-auto-live-surface
**Version:** V2.51
**Author:** auto-hardening (scheduled task)

---

## Summary

Implemented `node checkpoint <vec>` — a graph-native observability primitive that
snapshots a live node's current state into the structural diff ring as a
`GraphDiffKind::NodeCheckpoint` entry.  Analogous to `perf record --event=mark`
or `gdb checkpoint`: captures the node's vector address, key, signal_count,
lifecycle, and edge_out_count at the moment of invocation without modifying the
node or bumping the graph epoch.

---

## Changes

### `crates/gos-protocol/src/lib.rs`

- Added `NodeCheckpoint = 4` variant to `GraphDiffKind` (`#[repr(u8)]`).
- Updated `is_node()` to include `NodeCheckpoint` (so diff display renders it as
  a node-style entry showing vector + label rather than an edge pair).
- `is_addition()` returns `false` for `NodeCheckpoint` (it is neither an
  addition nor a removal — it is an observability mark).

### `crates/gos-runtime/src/lib.rs`

- Added `GraphRuntime::node_checkpoint_inner(vector)`:
  - Resolves the node by `VectorAddress` via `proc_stat_for_vector`.
  - Calls `push_diff(GraphDiffKind::NodeCheckpoint, vector, ZERO, key_bytes)`.
  - Graph epoch is **not** bumped — only `diff_ring_head` and `diff_total` advance.
  - Returns `Ok(NodeProcSummary)` or `Err(RuntimeError::NodeNotFound)`.
- Added public `node_checkpoint(vec) -> Result<NodeProcSummary, RuntimeError>`.

### `crates/k-shell/src/lib.rs`

- Added `dispatch_node_checkpoint(sink, vec)`:
  - On `Err`: prints red "node not found".
  - On `Ok`: prints the captured key, lifecycle (color-coded), signal_count,
    edge_out_count, and a hint that the entry is visible via `graph diff`.
- Updated `dispatch_graph_diff` match on `GraphDiffKind`:
  - `NodeCheckpoint` renders as `[ckpt  ]` in yellow (fg=14) with prefix `·`.
  - All four structural kinds kept with padded labels for alignment.

### `crates/k-shell/src/proc.rs`

- Wired `node checkpoint <vec>` / `ncp <vec>` / `checkpoint <vec>` before the
  `node stat clear` branch.

### `host-tests/gos-node-checkpoint-harness/` (new)

- Cargo.toml, .cargo/config.toml (host target override), tests/node_checkpoint.rs.
- 10 tests — all green in 0.01 s.

---

## Shell surface

| Command | Aliases | Action |
|---|---|---|
| `node checkpoint <vec>` | `ncp <vec>`, `checkpoint <vec>` | Snapshot node state → diff ring |

**Display after checkpoint:**
```
 node checkpoint  28.1.1.0
  key:          cp.alpha
  lifecycle:     running
  signal_count:  0
  edge_out:      1
  → recorded in diff ring as [ckpt]  (graph diff to view)
```

**`graph diff` output (NodeCheckpoint entries):**
```
 · [ckpt  ] 28.1.1.0  cp.alpha
```

---

## Test matrix (10 tests)

| # | Scenario | Expected |
|---|---|---|
| 1 | Empty graph, unknown vector | `Err(NodeNotFound)` |
| 2 | Graph with nodes, unknown vector | `Err(NodeNotFound)` |
| 3 | Known node → Ok | signal_count=0 returned |
| 4 | Diff ring fill increases by 1 | fill(after) = fill(before) + 1 |
| 5 | Graph epoch unchanged | epoch same before/after |
| 6 | Diff entry kind | `GraphDiffKind::NodeCheckpoint` |
| 7 | Diff entry from_vector | equals checkpointed node's vector |
| 8 | Diff entry label | equals node's local_node_key |
| 9 | Two checkpoints | fill grows by 2 |
| 10 | Node with edges | edge_out_count correct in summary |

---

## Invariants preserved

- All existing diff ring callers unaffected — `push_diff` path unchanged.
- `graph_epoch` not bumped — checkpoint is a pure observability mark.
- `GraphDiffKind` exhaustive match in `dispatch_graph_diff` updated in the same
  commit (no latent non-exhaustive match warning).
- `is_node()` returns `true` for `NodeCheckpoint` — diff renderer shows
  vector + label (consistent with node events, not edge events).
- Host test suite: 483 tests (473 prior + 10 new) — all green.

---

## Next candidates (V2.52+)

- `graph sim <N>` — simulate N random-walk steps, emit signal traffic trace
- `graph between` — betweenness centrality via all-pairs Dijkstra (directed weighted)
- PAL_U32 → attribute node refactor (Demo A prerequisite)
