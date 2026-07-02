# GOS Hardening Log — V2.29 — 2026-07-01

## Summary

V2.29 adds per-node signal-count reset and the `node stat clear` / `nstat clear` shell
commands, bringing graph-OS counter management up to production standards comparable to
`perf stat reset` and `echo 0 > /proc/<pid>/clear_refs` on Linux.

This completes the seven-command per-node observability surface and the read/write
symmetry for all per-node counters: every counter exposed by `stat` now has a reset.

---

## Changes

### 1. `reset_node_stat_inner()` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New method on `Runtime`:

```rust
pub fn reset_node_stat_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError> {
    let slot = self.node_slot_by_vec(vector).ok_or(RuntimeError::NodeNotFound)?;
    let record = self.nodes[slot].as_mut().ok_or(RuntimeError::NodeNotFound)?;
    record.signal_count = 0;
    Ok(())
}
```

Zeroes `NodeRecord::signal_count` for the target node via a single `u32` store.
Does not touch `node_trace`, `node_trace_count`, `node_log`, or any other per-node state.

### 2. `reset_node_stat()` — public API (gos-runtime)

```rust
pub fn reset_node_stat(vec: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().reset_node_stat_inner(vec)
}
```

Thin lock wrapper. Returns `Err(RuntimeError::NodeNotFound)` if the vector is not registered.

### 3. `dispatch_node_stat_clear()` — k-shell (`crates/k-shell/src/lib.rs`)

New public dispatch function. Calls `reset_node_stat()` and emits a colour-coded status line:

- **Green**: `node stat cleared  <vec>` + `signal_count -> 0  (trace ring and log unaffected)`
- **Red**: `node not found: <vec>`

### 4. Shell routing — k-shell (`crates/k-shell/src/proc.rs`)

`dispatch_text_command` now matches (inserted before `stat ` / `node stat ` to avoid prefix
collision with `node stat clear`):

```
node stat clear <vector>   →  dispatch_node_stat_clear(sink, vec)
nstat clear <vector>       →  dispatch_node_stat_clear(sink, vec)
```

Help text updated with two new entries under the `stat` section.

### 5. Test harness — `host-tests/gos-node-stat-clear-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `reset_stat_unknown_vector_returns_not_found` | Unknown vector → NodeNotFound |
| 2 | `reset_stat_fresh_node_returns_ok` | Fresh node (count == 0) → Ok(()) |
| 3 | `reset_stat_zeroes_signal_count_after_one_dispatch` | 1 dispatch → reset → count == 0 |
| 4 | `reset_stat_zeroes_signal_count_after_many_dispatches` | 7 dispatches → reset → count == 0 |
| 5 | `reset_stat_is_idempotent` | Double-reset stays at 0 |
| 6 | `reset_stat_new_dispatches_increment_from_zero` | 5 dispatches, reset, 3 more → count == 3 |
| 7 | `reset_stat_does_not_affect_sibling_node` | Reset A → B count unchanged |
| 8 | `reset_stat_preserves_trace_ring` | Reset stat → trace ring entries intact |
| 9 | `reset_stat_reflects_in_proc_page` | proc_page shows 0 after reset |
|10 | `reset_stat_returns_ok_for_live_node` | Live node → Ok(()) |

---

## Verification

```
cd host-tests/gos-node-stat-clear-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build --release
# Finished `release` profile [optimized]
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.29 |
|---|---|---|
| Counter reset | `perf stat reset` | `node stat clear <vec>` |
| Targeted reset | `echo 0 > /proc/<pid>/clear_refs` | `reset_node_stat()` API |
| Counter isolation | One counter, no side effects | Only `signal_count` zeroed |
| Symmetry | Every show has a clear | stat/clear pair complete |
| Measurement window | `perf stat` fresh run | Clear then dispatch N signals |
| Alias ergonomics | Short alias for frequent ops | `nstat clear <vec>` |

The `reset_node_stat` path takes a single Mutex lock and performs one `u32` store —
effectively zero overhead compared to signal dispatch itself.

---

## Graph-OS Characteristic Preserved

`node stat clear` acts only on the **counter abstraction** (signal_count) — the graph
topology (edges), structural mutation log (diff ring), and signal trace ring remain intact.
This preserves GOS's property that observability tools never destroy causal history;
only the specific measurement window is reset.

The graph model stays coherent: clearing a counter for one node does not cascade to its
neighbours or alter any edge relationships.

---

## Per-node Observability Surface — Complete as of V2.29

| Command | Analogue | Description |
|---|---|---|
| `node info <vec>` | `systemctl status` | Current state snapshot |
| `node trace <vec>` | `strace -p` | Signal dispatch history |
| `node trace clear <vec>` | `perf trace reset` | Discard signal trace ring |
| `node log <vec>` | `journalctl -u` | Lifecycle transition history |
| `node log clear <vec>` | `journalctl --vacuum-time` | Discard lifecycle log |
| `stat <vec>` | `/proc/<pid>/status` | Deep stat including signal_count |
| `node stat clear <vec>` | `perf stat reset` | Reset signal_count to 0 **(V2.29)** |

The seven-command per-node observability surface is now **complete**.

---

## Cumulative Test Suite (V2.29)

| Harness | Tests | Version |
|---|---|---|
| gos-runtime-harness | 26 | V2.2 |
| gos-supervisor-harness | 16 | V2.2 |
| gos-rewrite-harness | 12 | V2.3 |
| gos-rewrite-integration-harness | 6 | V2.3 |
| gos-subscribe-harness | 10 | V2.5 |
| gos-metrics-harness | 10 | V2.6 |
| gos-boot-harness | 11 | V2.9 |
| gos-node-inspect-harness | 8 | V2.8 |
| gos-journal-harness | 14 | V2.11 |
| gos-edge-inspect-harness | 10 | V2.12 |
| gos-graph-diff-harness | 10 | V2.13 |
| gos-proc-harness | 10 | V2.14 |
| gos-stat-harness | 10 | V2.15 |
| gos-graph-diff-epoch-harness | 10 | V2.16 |
| gos-graph-topo-harness | 10 | V2.17 |
| gos-graph-health-harness | 10 | V2.18 |
| gos-theme-node-harness | 10 | V2.19 |
| gos-plugin-list-harness | 10 | V2.20 |
| gos-kill-harness | 10 | V2.21 |
| gos-resume-harness | 10 | V2.22 |
| gos-node-info-harness | 10 | V2.23 |
| gos-node-trace-harness | 10 | V2.24 |
| gos-node-log-harness | 10 | V2.25 |
| gos-node-log-clear-harness | 10 | V2.26 |
| gos-node-trace-clear-harness | 10 | V2.27 |
| gos-uname-harness | 10 | V2.28 |
| **gos-node-stat-clear-harness** | **10** | **V2.29** |
| **Total** | **293** | |

---

*Automated hardening pass — GOS V2.29 — 2026-07-01*
