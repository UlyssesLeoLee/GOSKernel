# GOS Hardening Log — V2.22 — 2026-07-01

## Summary

V2.22 adds `resume_node()` API and the `resume <vec>` / `node resume <vec>` shell
commands — the complement to V2.21's `kill <vec>`.  Together they form a complete
node lifecycle control pair: fault to take a node out of service, resume to bring it
back to Ready without removing it from the graph.

This mirrors the `systemctl stop` / `systemctl start` (or `kill -9` / `systemctl
restart`) lifecycle control available in Linux for system services, closing a
significant gap in the GOS operational toolset.

---

## Changes

### 1. `resume_node()` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New method on `GraphRuntime`:

```rust
pub fn resume_node(&mut self, vector: VectorAddress) -> Result<(), RuntimeError>
```

- Finds the node slot by `vector`; returns `Err(NodeNotFound)` if absent.
- Sets `record.lifecycle = NodeLifecycle::Ready`.
- Emits a `StateDelta` control-plane event to propagate the state change.
- Does **not** bump `graph_epoch` (lifecycle change is not a structural mutation).
- Does **not** touch the fault queue (unlike `fault_node`, which enqueues for
  supervisor restart handling — resume is a direct state transition).

Public free-function wrapper added:

```rust
pub fn resume_node(vector: VectorAddress) -> Result<(), RuntimeError> {
    RUNTIME.lock().resume_node(vector)
}
```

### 2. `dispatch_node_resume()` — k-shell (`crates/k-shell/src/lib.rs`)

New public shell dispatch function, symmetric to `dispatch_node_kill()`:

- On `Ok`: green " resume: node ready" header with vector and new lifecycle state.
- On `Err`: red "resume: node not found: <vec>" error.
- Footer hint: "use `proc` to verify new lifecycle state".

### 3. Shell command routing — k-shell (`crates/k-shell/src/proc.rs`)

Two new command aliases added to `dispatch_text_command`:

| Command | Action |
|---|---|
| `resume <vec>` | Call `dispatch_node_resume(sink, vec)` |
| `node resume <vec>` | Alias — same action |

Help text updated:
```
  resume <vector>      resume a faulted/suspended node (like systemctl restart)
  node resume <vector>   alias for resume
```

### 4. Test harness — `host-tests/gos-resume-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `resume_node_unknown_vector_returns_not_found` | Unknown vector → `NodeNotFound` |
| 2 | `resume_node_on_faulted_node_returns_ok` | Faulted node → Ok |
| 3 | `resume_node_sets_lifecycle_to_ready` | Lifecycle becomes `Ready` |
| 4 | `resume_node_clears_faulted_count` | `faulted_node_count()` drops to 0 |
| 5 | `resume_node_does_not_bump_graph_epoch` | `graph_epoch` unchanged |
| 6 | `resume_node_preserves_signal_count` | `signal_count` unchanged after resume |
| 7 | `resume_node_does_not_enqueue_fault_queue` | Fault queue stays empty |
| 8 | `fault_then_resume_cycle_leaves_node_ready` | Full fault→resume round-trip |
| 9 | `resume_node_idempotent_on_ready_node` | Resume Ready node → still Ok + Ready |
|10 | `resume_one_of_two_faulted_nodes_leaves_one_faulted` | Selective resume, 2-node |

---

## Verification

```
cd host-tests/gos-resume-harness
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

| Capability | Linux/macOS equivalent | GOS V2.22 |
|---|---|---|
| Bring service back online | `systemctl start <unit>` | `resume <vec>` |
| Clear failure state | `systemctl reset-failed <unit>` | `resume <vec>` (sets Ready) |
| Paired with kill | `kill -9` + `systemctl restart` | `kill <vec>` + `resume <vec>` |
| Non-destructive | Does not remove node from graph | Preserves all node metadata |
| Signal count preserved | `/proc/<pid>/stat` reset on restart | `signal_count` unchanged |
| Epoch stability | No graph topology change | `graph_epoch` not bumped |

The `resume_node()` function is a pure lifecycle state flip — one field write plus
one control-plane emit — with zero allocation and no queue side-effects.

---

## Graph-OS Characteristic Preserved

`resume` operates on **vector addresses** (the graph's natural key space) rather
than opaque PIDs, keeping lifecycle control rooted in graph topology.  The
complementary pair `kill <vec>` / `resume <vec>` expresses fault injection and
recovery as first-class graph operations — a capability with no direct equivalent
in flat-PID operating systems.

---

## Shell Command Surface (cumulative, V2.22)

| Command | Added | Description |
|---|---|---|
| `kill <vec>` / `node fault <vec>` | V2.21 | Force-fault a node |
| `resume <vec>` / `node resume <vec>` | **V2.22** | **Resume a faulted node** |
| `plugins` / `lsmod` | V2.20 | Plugin inventory |
| `graph health` | V2.18 | Holistic health report |
| `proc` / `ps` | V2.14 | ps-style node table |
| `stat <vec>` | V2.15 | Per-node deep stat |
| `graph diff <N>` | V2.16 | Diff since epoch N |
| `graph topo` | V2.17 | L4-domain topology view |

---

*Automated hardening pass — GOS V2.22 — 2026-07-01*
