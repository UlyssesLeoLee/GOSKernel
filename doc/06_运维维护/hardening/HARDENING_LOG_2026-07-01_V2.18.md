# GOS Hardening Log — V2.18 — 2026-07-01

## Summary

V2.18 adds a holistic system health command (`graph health` / `health`) and two new
gos-runtime APIs (`faulted_node_count`, `diff_ring_fill`), bringing operator
observability up to the level of Linux's `systemctl status` + `dmesg --level=err,warn`.

---

## Changes

### 1. Two new APIs — gos-runtime (`crates/gos-runtime/src/lib.rs`)

#### `GraphRuntime::faulted_node_count() -> usize`
Private impl method — single-pass count of nodes whose `lifecycle == NodeLifecycle::Faulted`.
Iterates the `nodes` array directly (no order refresh needed; fault counting is
order-independent).

#### `GraphRuntime::diff_ring_fill() -> usize`
Private impl method — returns `self.diff_total.min(MAX_DIFF_RING as u64) as usize`.
Gives the current occupancy of the 128-slot structural diff ring without requiring a
walk of the ring itself.

#### Public module-level exports (after `node_page_l4`)
```rust
pub fn faulted_node_count() -> usize { RUNTIME.lock().faulted_node_count() }
pub fn diff_ring_fill() -> usize { RUNTIME.lock().diff_ring_fill() }
```

### 2. `graph health` shell command — k-shell (`crates/k-shell/src/lib.rs`, `crates/k-shell/src/proc.rs`)

#### `dispatch_graph_health(sink)`

Collects ten runtime metrics and renders a colour-coded health banner + detail table:

| Section | Metrics |
|---------|---------|
| **nodes** | total, faulted (highlighted red if > 0), edge count, subscribe pairs |
| **mutations** | graph epoch, total structural diffs ever pushed, diff ring fill (N/128) |
| **runtime** | scheduler preemption count, l4 domain switch count |
| **boot** | manifest rules checked, edges healed (highlighted yellow if > 0) |

**Health classification:**
- `DEGRADED` (white on red): faulted nodes exceed 25 % of total (or any fault when
  total < 4)
- `WARNING` (black on yellow): any faulted node, or diff ring ≥ 120/128 (near-full)
- `OK` (black on green): no faults, ring pressure nominal

Advisory lines printed below the table when not OK:
- DEGRADED: `run 'nodes faulted' to inspect faulted nodes`
- WARNING: `run 'nodes faulted' for fault details`

Shell dispatch additions in `proc.rs`:
- Exact match `"graph health"` and `"health"` → `dispatch_graph_health(sink)`.
- Help text updated with new entry.

### 3. Test harness — `host-tests/gos-graph-health-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `empty_faulted_node_count_is_zero` | Empty runtime → faulted_count == 0 |
| 2 | `registered_node_is_not_faulted` | Fresh node → not in Faulted state |
| 3 | `faulted_count_does_not_exceed_total` | faulted ≤ proc_count structural invariant |
| 4 | `empty_diff_ring_fill_is_zero` | Empty runtime → diff_ring_fill == 0 |
| 5 | `register_node_increases_diff_ring_fill` | register_node pushes diff entries |
| 6 | `diff_ring_fill_equals_min_total_cap` | fill == min(diff_total, 128) |
| 7 | `diff_ring_fill_never_exceeds_max` | fill ≤ MAX_DIFF_RING (128) always |
| 8 | `multiple_registrations_increase_diff_fill` | Fill increases per registration |
| 9 | `health_node_counts_consistent` | healthy + faulted == total |
|10 | `diff_ring_fill_monotonic_with_mutations` | Fill is non-decreasing |

---

## Verification

```
cd host-tests/gos-graph-health-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Kernel build:
```
cargo build --release
# Finished `release` profile [optimized]
```

Total host-test suite: **183 tests** (173 V2.17 + 10 new)

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.18 |
|---|---|---|
| System health overview | `systemctl status` | `graph health` banner (OK/WARNING/DEGRADED) |
| Fault triage | `systemctl --failed` | faulted count + `nodes faulted` advisory |
| Ring buffer pressure | kernel ring buffer full warnings | diff ring fill N/128 |
| Boot integrity | `systemd-analyze verify` | manifest rules + healed edge counts |
| Runtime throughput | `vmstat` preempt/context-switch columns | preempt_count + domain_switch_count |

The `graph health` command is the first GOS command that synthesises multiple telemetry
axes into a single actionable health verdict — operators need not inspect `nodes`,
`metrics export`, and `boot verify` separately; `graph health` does all three in one
call.

---

## Graph-OS Characteristic Preserved

The health model is graph-native: fault detection is lifecycle-state-driven (not
signal-based), diff ring pressure reflects graph topology mutation rate (not memory
pressure), and domain switch count is a graph-partitioning metric with no POSIX
analogue.  The `diff ring fill` metric is unique to GOS — it tracks the structural
change velocity of the graph itself, not any particular node's activity.

---

*Automated hardening pass — GOS V2.18 — 2026-07-01*
