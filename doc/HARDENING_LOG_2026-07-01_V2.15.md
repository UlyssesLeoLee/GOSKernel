# GOS Hardening Log — V2.15 — 2026-07-01

## Summary

V2.15 adds per-node single-lookup stat via `proc_stat_for_vector()` and a `stat <vec>`
shell command, providing `cat /proc/<pid>/status`-level per-node introspection to
complement the `proc` / `ps` table view added in V2.14.

---

## Changes

### 1. `proc_stat_for_vector()` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New inner method on the runtime struct:

```rust
pub fn proc_stat_for_vector(&self, vec: VectorAddress) -> Option<NodeProcSummary> {
    let slot = self.nodes.iter().position(|s| {
        s.map(|r| r.vector == vec).unwrap_or(false)
    })?;
    self.proc_summary_from_slot(slot)
}
```

New public API function:

```rust
pub fn proc_stat_for_vector(vec: VectorAddress) -> Option<NodeProcSummary> {
    RUNTIME.lock().proc_stat_for_vector(vec)
}
```

- O(nodes) linear scan to find the slot — acceptable at boot-time node counts.
- Returns `None` if no registered node has the given vector address.
- Reuses the existing `proc_summary_from_slot()` to build the `NodeProcSummary`
  (signal_count, edge_out_count, lifecycle, key, plugin_name).

### 2. `dispatch_node_stat()` — k-shell (`crates/k-shell/src/lib.rs`)

New public function `dispatch_node_stat(sink: &ConsoleSink, vec: VectorAddress)`:

- Calls `gos_runtime::proc_stat_for_vector(vec)`.
- On `None`: prints red "not found: <vec>" and returns.
- On `Some(s)`: prints a labeled block with all six fields, color-coding the
  vector and lifecycle (green=Running, red=Faulted, yellow=Suspended, white=other)
  and the signal_count in cyan.

Sample output for a Running node:
```
 node stat
  vector:        6.1.0.0       ← green
  key:           k_shell::console
  plugin:        k-shell
  lifecycle:     running        ← green
  signal_count:  1234           ← cyan
  edge_out:      3
```

### 3. `stat <vec>` / `node stat <vec>` shell commands — k-shell (`crates/k-shell/src/proc.rs`)

New dispatch branch:

```rust
} else if let Some(vec_str) = cmd.strip_prefix("stat ").or_else(|| cmd.strip_prefix("node stat ")) {
    if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
        super::dispatch_node_stat(sink, vec);
    } else {
        // error: not a valid vector
    }
```

- `stat <vec>` — primary form (mirrors `stat` / `cat /proc/<pid>/status` on Linux).
- `node stat <vec>` — alternative for discoverability alongside `node <vec>`.
- Help text updated with the new entry.

### 4. Test harness — `host-tests/gos-stat-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `unknown_vector_returns_none` | Unregistered vector → None |
| 2 | `registered_node_returns_some` | Registered node → Some |
| 3 | `stat_vector_matches` | summary.vector == queried vector |
| 4 | `stat_key_matches` | summary.local_node_key == spec key |
| 5 | `stat_plugin_name_matches` | summary.plugin_name == manifest name |
| 6 | `fresh_node_signal_count_is_zero` | New node → signal_count == 0 |
| 7 | `stat_signal_count_after_one_dispatch` | 1 dispatch → signal_count == 1 |
| 8 | `stat_signal_count_after_two_dispatches` | 2 dispatches → signal_count == 2 |
| 9 | `stat_edge_out_count_zero_when_no_edges` | No edges → edge_out_count == 0 |
|10 | `wrong_vector_returns_none_not_other_node` | Unregistered vec → None even when others exist |

---

## Verification

```
cd host-tests/gos-stat-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-proc-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed  (regression check)

cargo build --release
# Finished `release` profile
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.15 |
|---|---|---|
| Single-process detail | `cat /proc/<pid>/status` | `stat <vec>` shell command |
| Status fields | Name, State, VmSize, Threads… | key, plugin, lifecycle, signal_count, edge_out |
| Lookup by identity | `ps -p <pid>` | `stat <vec>` by VectorAddress |
| Not-found handling | exit 1 + error | red "not found: <vec>" message |
| Composite view | `ps aux` | `proc` (V2.14) |
| Single-node view | `cat /proc/<pid>/status` | `stat <vec>` (V2.15) ← new |

The two commands form a natural pair: `proc` for the wide table, `stat <vec>` for
the per-node deep dive — mirroring the Linux `ps aux` / `/proc/<pid>/status` split.

---

## Graph-OS Characteristic Preserved

`stat` exposes VectorAddress as the primary identity (not a raw integer PID), and
reports edge out-degree — connecting the per-node view back to the graph substrate.
The vector address is the stable, human-readable process identity in GOS.

---

*Automated hardening pass — GOS V2.15 — 2026-07-01*
