---
name: gos-diff-ring-epoch-invariant
description: push_diff() in gos-runtime advances diff_ring_head and diff_total but does NOT bump graph_epoch. Only structural mutations (register_node, register_edge, remove_edge) advance the epoch. Observability writes like NodeCheckpoint must use push_diff, never bump_epoch. Apply when implementing any function that writes to the diff ring without modifying graph topology.
---

# Diff Ring vs Graph Epoch: Separate Monotonic Counters

## The rule

`push_diff()` and `graph_epoch` are independent:

- **`push_diff(kind, from, to, label)`** → advances `diff_ring_head` and `diff_total` only.  
  The captured `epoch` field in the `GraphDiffEntry` is a snapshot of the current epoch — it does **not** change the epoch.

- **`graph_epoch` advances** only on structural mutations: `register_node`, `register_edge`, and edge-remove paths — each calls `self.graph_epoch = self.graph_epoch.wrapping_add(1)` before `push_diff`.

This means:
- Multiple `push_diff` calls at the same epoch all share the same `epoch` value.
- `graph_diff_since(epoch, ...)` uses `entry.epoch > since_epoch` — entries pushed at the pinned epoch itself will NOT appear.
- Observability checkpoints (like `NodeCheckpoint`) correctly appear in `graph diff since <prior_epoch>` queries without polluting the structural change count.

## Why it's non-obvious

`push_diff` is called both by structural mutations (after bumping epoch) and by observability writes (without bumping). The epoch field in `GraphDiffEntry` is read-only after push — it records "what epoch was the graph at when this was pushed", not "what epoch does this create". If you added an epoch bump to `node_checkpoint_inner`, every checkpoint would advance the epoch counter, breaking epoch-based diffing and `graph diff since` queries.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs:770` — `push_diff` implementation (does NOT touch `graph_epoch`)
- `crates/gos-runtime/src/lib.rs:1034` — `register_node` bumps epoch then calls `push_diff`
- `crates/gos-runtime/src/lib.rs:1059` — `register_edge` same pattern
- `crates/gos-runtime/src/lib.rs:1528` — `node_checkpoint_inner` calls `push_diff` without epoch bump
- `graph_diff_since` filter: `entry.epoch > since_epoch` (strictly greater, not >=)

## From this session

V2.51 `node checkpoint`: intentionally skips epoch bump so checkpoints are purely observability marks. Verified by test 5 in gos-node-checkpoint-harness: `epoch_before == epoch_after` after checkpoint.
