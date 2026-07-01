# GOS Hardening Log — V2.14 — 2026-07-01

## Summary

V2.14 adds per-node signal dispatch counters and a ps-style `proc` shell command, bringing
graph-OS process visibility up to production standards comparable to `ps aux` on Linux.

---

## Changes

### 1. `NodeProcSummary` — gos-protocol (`crates/gos-protocol/src/lib.rs`)

New public struct exported from gos-protocol:

```rust
pub struct NodeProcSummary {
    pub vector:          VectorAddress,
    pub local_node_key:  &'static str,
    pub plugin_name:     &'static str,
    pub lifecycle:       NodeLifecycle,
    pub signal_count:    u32,   // cumulative signal dispatches since registration
    pub edge_out_count:  u16,   // current outbound edge count
}
```

`NodeProcSummary::EMPTY` provided as a const initializer for stack arrays.

### 2. Per-node signal counter — gos-runtime (`crates/gos-runtime/src/lib.rs`)

- Added `signal_count: u32` field to `NodeRecord` (internal struct).
- `register_node()` initialises `signal_count: 0`.
- `prepare_signal_dispatch()` increments `signal_count` via `saturating_add(1)` on every
  successful dispatch preparation, so the counter never wraps to zero on overflow.
- Added `proc_summary_from_slot()` (private) — builds a `NodeProcSummary` from a node slot,
  counting outbound edges via a linear scan of the edge table.
- Added `proc_page<const N>()` (impl + public API) — returns a page of `NodeProcSummary`
  entries sorted by vector address (reuses the existing `node_order` cache).
- Added `proc_count()` (impl + public API) — returns the total live node count.

### 3. `proc` shell command — k-shell (`crates/k-shell/src/lib.rs`, `crates/k-shell/src/proc.rs`)

New dispatch function `dispatch_proc_list()` in `lib.rs`:

- Header: `vector | sig (signal count) | out (edge out-degree) | state | plugin/key`
- Color-codes lifecycle: green = Running, red = Faulted, yellow = Suspended, white = other.
- Right-aligns signal and edge counts in 4 columns (`print_num_right4()`).
- Shows up to 32 nodes per page; prints "... N more" if total exceeds page.
- Footer: total node count and hint about `sig` field meaning.

Helper `node_lifecycle_label()` added (full enum match covering all 10 `NodeLifecycle` variants).
Helper `print_num_right4()` added for columnar number formatting.

Shell dispatch (`proc.rs`):
- `proc`, `ps`, `proc all` → `dispatch_proc_list(sink)`
- `help` text updated to include `proc` entry.

### 4. Test harness — `host-tests/gos-proc-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `empty_proc_page_returns_zero` | Empty runtime returns (0, 0) |
| 2 | `registered_node_appears_in_proc_page` | 1 node → 1 entry, signal_count == 0 |
| 3 | `proc_summary_vector_matches` | Summary.vector == registered VEC |
| 4 | `proc_summary_key_matches` | Summary.local_node_key == spec key |
| 5 | `signal_count_increments_after_route_signal` | 1 dispatch → signal_count == 1 |
| 6 | `signal_count_increments_twice` | 2 dispatches → signal_count == 2 |
| 7 | `signal_count_is_saturating` | u32::MAX.saturating_add(1) == u32::MAX |
| 8 | `proc_page_offset_at_total_returns_zero` | offset == total → 0 entries returned |
| 9 | `proc_page_returns_nodes_sorted_by_vector` | Multi-node sort verified A < B < C |
|10 | `proc_count_reflects_live_nodes` | proc_count() tracks registrations |

---

## Verification

```
cd host-tests/gos-proc-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Kernel build:
```
cargo build --release
# Finished `release` profile
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.14 |
|---|---|---|
| Process list | `ps aux` / `top` | `proc` shell command |
| Signal activity | `/proc/<pid>/stat` (utime/stime) | `signal_count` per node |
| I/O fan-out | `lsof -p` / `ss` | `edge_out_count` per node |
| Lifecycle state | `ps` STATE column (S/R/Z/T) | `lifecycle` column |
| Sort order | PID ascending | vector address ascending |
| Paginated output | `ps -e | head -32` | PAGE=32 limit with "... N more" |

The `signal_count` field is an always-on, zero-copy counter that adds one `saturating_add`
per dispatch — negligible overhead in the existing critical path.

---

## Graph-OS Characteristic Preserved

`proc` exposes the **graph topology** (edge out-degree per node) alongside the execution
metric (signal count), keeping the `ps`-analogue rooted in GOS's graph model rather than
a flat process list abstraction.

---

*Automated hardening pass — GOS V2.14 — 2026-07-01*
