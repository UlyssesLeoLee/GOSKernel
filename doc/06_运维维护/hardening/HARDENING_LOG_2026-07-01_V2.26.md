# GOS Hardening Log — V2.26 — 2026-07-01

## Summary

V2.26 adds `node log clear <vec>` / `nlog clear <vec>` shell commands and the
underlying `clear_node_log()` runtime API, enabling operators to discard stale
lifecycle history for a node after recovery — analogous to
`journalctl --vacuum-time` or `truncate -s0 /var/log/syslog` on Linux.

This completes the **per-node lifecycle-log management triad**:
- `node log <vec>` — read the log (V2.25)
- `node log clear <vec>` — discard the log (V2.26, this version)

---

## Changes

### 1. `clear_node_log_inner()` + `clear_node_log()` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New method on `GraphRuntime`:

```rust
pub fn clear_node_log_inner(&mut self, vector: VectorAddress) -> Result<(), RuntimeError>
```

- Resolves the node slot from `vector`; returns `Err(RuntimeError::NodeNotFound)` if absent.
- Zeroes `node_log[slot]` to `NodeLogEntry::EMPTY`, resets `node_log_head[slot]` to 0,
  and resets `node_log_total[slot]` to 0.
- Operation is O(MAX_NODE_LOG) — the ring is 16 entries; negligible cost.

New public API function:

```rust
pub fn clear_node_log(vec: VectorAddress) -> Result<(), RuntimeError>
```

Delegates to `RUNTIME.lock().clear_node_log_inner(vec)`.

### 2. `dispatch_node_log_clear()` — k-shell (`crates/k-shell/src/lib.rs`)

New public function analogous to `dispatch_node_log()`:

- Calls `gos_runtime::clear_node_log(vec)`.
- On success: green confirmation line `" node log cleared  <vec>"`.
- On error: red `" node not found: <vec>"`.

### 3. Shell dispatch — k-shell (`crates/k-shell/src/proc.rs`)

Two new dispatch branches added **before** the existing `node log <vec>` branch
(so the longer `"node log clear "` prefix is matched first):

```
node log clear <vec>   — clear lifecycle log
nlog clear <vec>       — alias for node log clear
```

Help text extended with two new entries documenting the `--vacuum-time` analogue.

---

## Test Harness — `host-tests/gos-node-log-clear-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `clear_unknown_vector_returns_not_found` | Unknown vec → NodeNotFound |
| 2 | `clear_fresh_node_gives_zero_entries` | After clear, (total=0, returned=0) |
| 3 | `clear_does_not_unregister_node` | node_log_page succeeds after clear (node alive) |
| 4 | `clear_discards_faulted_entry` | Faulted entry removed by clear |
| 5 | `clear_discards_ready_entry` | Ready entry removed by clear |
| 6 | `clear_is_idempotent` | Double-clear still yields (0, 0) |
| 7 | `clear_then_new_events_logged_correctly` | Post-clear events recorded correctly |
| 8 | `clear_does_not_affect_sibling_node` | Clearing A leaves B's log intact |
| 9 | `clear_resets_total_counter_to_zero` | total=0 after clear; 1 event → total=1 |
|10 | `clear_returns_ok_for_live_node` | Returns Ok(()) for registered node |

---

## Verification

```
cd host-tests/gos-node-log-clear-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build -p gos-runtime -p gos-protocol
# Finished dev profile — no errors
```

Regression: V2.25 `gos-node-log-harness` — 10 passed; 0 failed.

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.26 |
|---|---|---|
| Clear per-service log | `journalctl --vacuum-time=0 -u <svc>` | `node log clear <vec>` |
| Clean slate after restart | `truncate -s0 /var/log/svc.log` | `nlog clear <vec>` |
| Idempotent clear | `journalctl --vacuum-time` is safe to repeat | double-clear still returns Ok(()) |
| Sibling isolation | logs are per-unit, isolated | each node has its own ring |

The clear operation is a **write API** — it is the only mutation in the
node-log subsystem and is deliberately explicit (`node log clear <vec>`)
to prevent accidental data loss.

---

## Graph-OS Characteristic Preserved

`node log clear` operates on a single vector address, staying within the
graph-native addressing model.  The log ring is a per-node sub-resource
accessed by the same vector lookup used throughout the GOS runtime — no
PID-to-unit mapping, no filename look-up.

---

*Automated hardening pass — GOS V2.26 — 2026-07-01*
