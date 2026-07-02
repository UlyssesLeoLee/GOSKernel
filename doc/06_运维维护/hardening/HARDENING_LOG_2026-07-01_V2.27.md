# GOS Hardening Log — V2.27 — 2026-07-01

## Summary

V2.27 adds `node trace clear <vec>` / `ntrace clear <vec>` — a shell command and API to
discard the per-node signal dispatch trace ring, symmetric with `node log clear` (V2.26).
This completes the observability quartet's clear operations: both the lifecycle log and the
signal trace ring can now be discarded independently without affecting cumulative proc stats.

---

## Changes

### 1. `node_trace_count` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

Added a new per-node counter array `node_trace_count: [u32; MAX_NODES]` to `GraphRuntime`:

- **Purpose**: tracks how many signal dispatches have been written to the trace ring since
  last clear, independent of `signal_count` (which is cumulative and used by `proc`).
- **Initialisation**: `[0u32; MAX_NODES]` in `GraphRuntime::new()`.
- **Increment**: `prepare_signal_dispatch()` increments `node_trace_count[slot]` via
  `saturating_add(1)` on every trace write, alongside the existing `signal_count`.
- **Why separate**: `signal_count` is a monotonic proc metric that must not be reset;
  `node_trace_count` is the ring-level total that resets on clear, enabling the same
  `(total=0, returned=0)` semantics that `node_log_total` provides for the log ring.

`node_trace_page()` updated to use `self.node_trace_count[slot]` instead of
`record.signal_count` for the `total_traced` return value.

### 2. `clear_node_trace_inner()` — gos-runtime

New method on `GraphRuntime`:

```rust
pub fn clear_node_trace_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
    let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
    self.node_trace[slot] = [NodeTraceEntry::EMPTY; MAX_NODE_TRACE];
    self.node_trace_head[slot] = 0;
    self.node_trace_count[slot] = 0;
    Ok(())
}
```

- Zeroes the trace ring entries, resets the head pointer, and resets `node_trace_count`.
- `signal_count` inside `NodeRecord` is deliberately untouched — `proc` stats remain valid.
- Returns `NodeNotFound` for unregistered vectors.

### 3. `clear_node_trace()` — gos-runtime public API

```rust
pub fn clear_node_trace(vec: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().clear_node_trace_inner(vec)
}
```

### 4. `dispatch_node_trace_clear()` — k-shell (`crates/k-shell/src/lib.rs`)

New shell dispatch function:

- On success: prints `" node trace cleared  <vec>"` in green/grey.
- On error: prints `" node not found: <vec>"` in red.
- Analogous to `dispatch_node_log_clear()` (V2.26).

### 5. `node trace clear` / `ntrace clear` routing — k-shell (`crates/k-shell/src/proc.rs`)

Shell routing added **before** the existing `node trace <vec>` arm so
`"node trace clear X"` matches before `"node trace X"`:

```
node trace clear <vector>   →  dispatch_node_trace_clear(sink, vec)
ntrace clear <vector>       →  dispatch_node_trace_clear(sink, vec)   [alias]
```

Help text updated to include both new commands.

### 6. Test harness — `host-tests/gos-node-trace-clear-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `clear_unknown_vector_returns_not_found` | Unregistered vector → NodeNotFound |
| 2 | `clear_fresh_node_gives_zero_entries` | Clear on fresh node → (0, 0) |
| 3 | `clear_does_not_unregister_node` | Node still accessible after clear |
| 4 | `clear_discards_single_dispatch_entry` | 1 dispatch then clear → (0, 0) |
| 5 | `clear_discards_multiple_dispatch_entries` | 5 dispatches then clear → (0, 0) |
| 6 | `clear_is_idempotent` | Double-clear still returns (0, 0) |
| 7 | `clear_then_new_dispatches_traced_correctly` | Ring is fresh after clear; new kind/cmd correct |
| 8 | `clear_does_not_affect_sibling_node` | Clear A leaves B trace intact |
| 9 | `clear_resets_total_counter_to_zero` | total=0 after clear, then 1 after next dispatch |
|10 | `clear_returns_ok_for_live_node` | Returns Ok(()) for registered node |

---

## Verification

```
cd host-tests/gos-node-trace-clear-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-node-trace-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed   (backward compat — total now uses node_trace_count)

cargo build --release
# Finished `release` profile [optimized]
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.27 |
|---|---|---|
| Clear trace buffer | `perf trace reset` / `truncate -s0 strace.log` | `node trace clear <vec>` |
| Selective clear (one process) | `strace -p <pid>` restart | `ntrace clear <vec>` |
| Preserve proc stats | `signal_count` unaffected | ✓ separate `node_trace_count` |
| Idempotent | clearing empty buffer is safe | ✓ double-clear safe |
| Error on unknown target | no-op or error | ✓ `NodeNotFound` |

---

## Graph-OS Characteristic Preserved

`node trace clear` operates on graph **vector addresses** (not flat PIDs), reinforcing
GOS's topology-rooted identity model: every observability operation names a node by its
position in the graph.

---

## Observability Quartet — Complete

| Command | Analogue | Version |
|---|---|---|
| `node info <vec>` | `systemctl status` | V2.23 |
| `node trace <vec>` | `strace -p <pid>` | V2.24 |
| `node trace clear <vec>` | `perf trace reset` | **V2.27** |
| `node log <vec>` | `journalctl -u <svc>` | V2.25 |
| `node log clear <vec>` | `journalctl --vacuum-time` | V2.26 |

Both observability rings now have symmetric read + clear operations.

---

*Automated hardening pass — GOS V2.27 — 2026-07-01*
