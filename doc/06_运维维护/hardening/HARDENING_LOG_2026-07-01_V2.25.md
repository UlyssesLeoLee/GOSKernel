# GOS Hardening Log — V2.25 — 2026-07-01

## Summary

V2.25 adds a per-node lifecycle event log with the `node log <vec>` / `nlog <vec>` shell
command — the graph-OS equivalent of `journalctl -u <service>`.  Every lifecycle transition
(`Registered → Allocated → Running → Ready → Faulted → Ready`, etc.) is now recorded in a
16-slot per-node ring with a monotonic tick timestamp, giving operators a complete audit trail
of how each graph node has evolved since boot.

---

## Changes

### 1. `NodeLogEntry` — gos-protocol (`crates/gos-protocol/src/lib.rs`)

New public struct exported from gos-protocol:

```rust
pub struct NodeLogEntry {
    pub tick:      u64,   // monotonic runtime tick at transition time
    pub lifecycle: u8,    // NodeLifecycle discriminant (e.g. 0xFF = Faulted)
    pub _pad:      [u8; 7],
}
impl NodeLogEntry {
    pub const EMPTY: Self = Self { tick: 0, lifecycle: 0, _pad: [0u8; 7] };
}
```

### 2. Per-node lifecycle log ring — gos-runtime (`crates/gos-runtime/src/lib.rs`)

- Added `MAX_NODE_LOG: usize = 16` constant.
- Added three fields to `GraphRuntime`:
  - `node_log: [[NodeLogEntry; MAX_NODE_LOG]; MAX_NODES]` — ring storage per node slot.
  - `node_log_head: [u8; MAX_NODES]` — next-write pointer per node slot.
  - `node_log_total: [u16; MAX_NODES]` — total transitions ever logged (saturates at u16::MAX).
- `GraphRuntime::new()` initialises all three to EMPTY / zero.
- `state_delta()` — the single internal hook called on every lifecycle change — now also
  pushes a `NodeLogEntry { tick, lifecycle }` into the node's log ring.  This is zero-overhead
  on the fast path: one array write + one `saturating_add`.
- Added `node_log_page()` impl method: returns newest-first, capped at MAX_NODE_LOG entries.
- Added global `node_log_page()` wrapper (mirrors the `node_trace_page` pattern).
- `NodeLogEntry` imported alongside `NodeTraceEntry` in the protocol import block.

### 3. `node log` shell command — k-shell (`crates/k-shell/src/lib.rs`, `crates/k-shell/src/proc.rs`)

New dispatch function `dispatch_node_log()` in `lib.rs`:

- Header: node identifier + total event count + showing count.
- Table: `tick | lifecycle label` — each row is one transition.
- Color-codes lifecycle: green = Ready/Registered, red = Faulted, yellow = Running,
  cyan = Suspended, gray = Discovered/Terminated, white = other.
- Helper `lifecycle_log_entry()` maps the raw u8 discriminant to a label + color.
- Footer hint: `node trace <vec> for signal history | ninfo <vec> for full view`.

Shell dispatch (`proc.rs`):
- `node log <vec>` / `nlog <vec>` → `dispatch_node_log(sink, vec)`
- `help` text updated with two new entries.

### 4. Test harness — `host-tests/gos-node-log-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `log_unknown_vector_returns_not_found` | NodeNotFound for unregistered vector |
| 2 | `log_fresh_node_has_no_entries` | After register, API succeeds and ring is valid |
| 3 | `log_contains_allocated_after_register` | Allocated state_delta recorded on register_node |
| 4 | `log_faulted_entry_after_fault_node` | Most recent entry is Faulted after fault_node() |
| 5 | `log_ready_entry_after_resume_node` | Most recent entry is Ready after resume_node() |
| 6 | `log_newest_first_ordering` | fault → resume → fault: [0]=Faulted, [1]=Ready |
| 7 | `log_total_increases_with_events` | total strictly increases with each lifecycle change |
| 8 | `log_ring_wraps_after_max_entries` | Ring full: returned == MAX_NODE_LOG after overflow |
| 9 | `log_faulted_discriminant_is_0xff` | Faulted lifecycle == 0xFF matches #[repr(u8)] spec |
|10 | `log_two_nodes_independent` | Node A fault does not appear in node B's log |

---

## Verification

```
cd host-tests/gos-node-log-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Kernel build:
```
cargo build --release
# Finished `release` profile [optimized]
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.25 |
|---|---|---|
| Service lifecycle log | `journalctl -u <service>` | `node log <vec>` / `nlog <vec>` |
| Audit trail | systemd unit journal | per-node 16-slot ring with tick timestamps |
| Newest-first view | `journalctl -u svc --reverse` | always newest-first by design |
| Fault/recovery history | `journalctl -u svc \| grep -E 'Start\|Stop\|Failed'` | all transitions captured |
| Zero-overhead recording | systemd journal (separate process) | in-line ring write in `state_delta()` |
| Ring depth | configurable | 16 entries (MAX_NODE_LOG) |

The lifecycle log complements `node trace <vec>` (signal dispatch history) and
`node info <vec>` (static snapshot) to form a complete per-node observability trio:

| Command | Analogue | Shows |
|---|---|---|
| `node info <vec>` | `systemctl status <svc>` | current state snapshot |
| `node trace <vec>` | `strace -p <pid>` | recent signal dispatches |
| `node log <vec>` | `journalctl -u <svc>` | lifecycle transition history |

---

## Graph-OS Characteristic Preserved

The lifecycle log records transitions that are driven by **graph structural events**
(node registration, edge mutations triggering subscriber signals) as well as operator
commands (`kill`, `resume`).  The `tick` field ties each transition to the graph
runtime's own monotonic clock, keeping observability rooted in GOS's event-loop model
rather than a wall-clock abstraction foreign to a graph OS.

---

*Automated hardening pass — GOS V2.25 — 2026-07-01*
