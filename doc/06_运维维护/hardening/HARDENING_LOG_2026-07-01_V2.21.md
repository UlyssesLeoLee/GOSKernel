# GOS Hardening Log — V2.21 — 2026-07-01

## Summary

V2.21 adds `kill <vec>` / `node fault <vec>` shell commands and the underlying
`fault_node()` runtime API, giving operators the ability to manually fault any
live graph node from the console — the graph-OS equivalent of `kill -9 <pid>`
on Linux/Unix.  This closes a critical operator-control gap: previously the only
way for a node to enter `NodeLifecycle::Faulted` was a CPU exception or a native
plugin returning `ExecStatus::Fault`.

---

## Changes

### 1. `fault_node()` API — gos-runtime (`crates/gos-runtime/src/lib.rs`)

#### `impl GraphRuntime` method

```rust
pub fn fault_node(&mut self, vector: VectorAddress) -> Result<(), RuntimeError>
```

- Resolves the node slot by `vector`; returns `Err(NodeNotFound)` for unknown vectors.
- Sets `record.lifecycle = NodeLifecycle::Faulted`.
- Calls `state_delta(node_id, Faulted)` — emits `StateDelta` control-plane event.
- Enqueues `vector` on `fault_queue` (the supervisor's restart policy drains this
  on the next `pump()` tick).
- Does **not** bump `graph_epoch` — faulting is a lifecycle state change, not a
  structural topology mutation.  This keeps `graph diff` / `graph health` diffs clean.

#### Module-level wrapper

```rust
pub fn fault_node(vector: VectorAddress) -> Result<(), RuntimeError>
```

Standard `RUNTIME.lock().fault_node(vector)` delegation, consistent with all
other gos-runtime public API.

### 2. `dispatch_node_kill()` — k-shell (`crates/k-shell/src/lib.rs`)

New display function added after `dispatch_node_stat`:

- On success (green): prints `kill: node faulted`, the vector, lifecycle transition
  text, and a hint to run `nodes faulted` to confirm.
- On failure (red): prints `kill: node not found: <vec>`.

### 3. Shell commands — k-shell (`crates/k-shell/src/proc.rs`)

Three command aliases dispatching to `dispatch_node_kill`:

| Command | Notes |
|---|---|
| `kill <vec>` | Primary form — mirrors Unix `kill -9` |
| `node fault <vec>` | Verbose form — explicit graph-OS intent |
| `fault <vec>` | Short alias for interactive use |

Help text updated with both forms and a description.

### 4. Test harness — `host-tests/gos-kill-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `fault_node_unknown_vector_returns_not_found` | Bogus vector → Err(NodeNotFound) |
| 2 | `fault_node_registered_returns_ok` | Registered node → Ok(()) |
| 3 | `fault_node_sets_lifecycle_to_faulted` | proc_stat shows Faulted after kill |
| 4 | `fault_node_does_not_remove_node_from_graph` | Node still in proc_page after fault |
| 5 | `fault_node_enqueues_to_fault_queue` | drain_next_fault returns the vector |
| 6 | `fault_node_increases_faulted_node_count` | faulted_node_count goes 0 → 1 |
| 7 | `fault_node_idempotent_on_already_faulted_node` | Re-fault → Ok, count stays 1 |
| 8 | `fault_node_does_not_bump_graph_epoch` | graph_epoch unchanged after fault |
| 9 | `fault_node_preserves_signal_count` | signal_count not reset by fault |
| 10 | `two_fault_nodes_enqueue_two_vectors` | Two kills → two dequeues, then None |

---

## Verification

```
cd host-tests/gos-kill-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Kernel build:
```
cargo build --release
# Finished `release` profile
```

Regression check:
```
cd host-tests/gos-proc-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
cd host-tests/gos-graph-health-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.21 |
|---|---|---|
| Force-kill a process | `kill -9 <pid>` / `SIGKILL` | `kill <vec>` / `node fault <vec>` |
| Immediate lifecycle change | kernel removes from run queue | lifecycle → Faulted, fault_queue enqueued |
| Supervisor notification | kernel sends SIGCHLD to parent | fault_queue drained on next pump(), triggers restart policy |
| Topology preserved | process removed from PID table | node stays in graph (graph_epoch unchanged) |
| Observability | `ps` shows zombie briefly, then gone | `nodes faulted` shows the faulted node |

The key design decision: `fault_node` does **not** remove the node from the graph
because GOS nodes are graph vertices with attached edges — topology removal is a
separate operation (`unregister_edge` + future `unregister_node`) that must respect
dependent subscribers and rewrite rules.  Faulting transitions lifecycle only; the
supervisor's restart policy decides the next step (restart, demote, or drain).

---

## Graph-OS Characteristic Preserved

`kill <vec>` targets nodes by their **vector address** (a graph coordinate) rather
than an opaque numeric PID, keeping the operator mental model rooted in graph
topology.  `node fault <vec>` makes this intent explicit: you are performing a
lifecycle mutation on a graph vertex, not sending a Unix signal to a flat process.

---

*Automated hardening pass — GOS V2.21 — 2026-07-01*
