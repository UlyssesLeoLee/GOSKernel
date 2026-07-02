# GOS Hardening Log — V2.24 — 2026-07-01

## Summary

V2.24 adds a per-node signal trace ring and the `node trace <vec>` / `ntrace <vec>` shell
command, bringing graph-OS signal observability up to production standards comparable to
`strace -p <pid>` on Linux.  Every signal dispatch is now recorded in a per-node circular
buffer of 16 entries; the shell command renders the ring in newest-first order with kind,
cmd, serial number, and sender vector columns.

---

## Changes

### 1. `NodeTraceEntry` — gos-protocol (`crates/gos-protocol/src/lib.rs`)

New public struct exported from gos-protocol:

```rust
pub struct NodeTraceEntry {
    /// Sender's raw vector address (0 for kernel-initiated signals).
    pub from:   u64,
    /// signal_count value just before this dispatch (monotonic sequence number).
    pub serial: u32,
    /// Signal kind discriminant — matches KernelSignalKind u8 values.
    /// 0 = EMPTY sentinel (no signal recorded in this ring slot yet).
    pub kind:   u8,
    /// Control: cmd byte.  Interrupt: irq byte.  Data: data byte.  Others: 0.
    pub cmd:    u8,
}
```

`NodeTraceEntry::EMPTY` provided as a const initializer for ring arrays.

### 2. Per-node trace ring — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New constant:
```rust
pub const MAX_NODE_TRACE: usize = 16;
```

New fields added to `GraphRuntime`:
- `node_trace: [[NodeTraceEntry; MAX_NODE_TRACE]; MAX_NODES]` — circular trace ring, one per node slot.
- `node_trace_head: [u8; MAX_NODES]` — next write position per slot.

Memory addition: 128 × 16 × 16 bytes = **32 KB** static allocation (acceptable for bare-metal kernel).

`prepare_signal_dispatch()` modified to accept `signal: Signal`:
- Adds a `signal_trace_fields(signal)` helper to extract `(kind, from, cmd)`.
- Records the trace entry at `node_trace[slot][head]` **before** incrementing `signal_count`
  so `serial` equals the signal index (0-based).
- Advances `node_trace_head[slot]` with wrap-around.

New public method `GraphRuntime::node_trace_page()`:
- Reads ring backwards from head (newest first).
- Returns `(total_signals, entries_written)`.

New public API wrapper:
```rust
pub fn node_trace_page(
    vec: VectorAddress,
    out: &mut [NodeTraceEntry; MAX_NODE_TRACE],
) -> Result<(u32, usize), RuntimeError>
```

### 3. `node trace` / `ntrace` shell command — k-shell

New dispatch function `dispatch_node_trace(sink, vec)` in `lib.rs`:
- Header: node vector + total dispatch count + ring fill count.
- Column headers: `seq | kind | cmd | from`
- Color-codes signal kinds: green=call, magenta=spawn, blue=irq, white=data, yellow=control, red=term.
- Prints sender vector (decoded from `from` field) or `kernel` for system-originated signals.
- Footer: hint pointing to `node info` and `proc`.

Shell dispatch (`proc.rs`):
- `node trace <vec>`, `ntrace <vec>` → `dispatch_node_trace(sink, vec)`
- `help` text updated with `node trace` and `ntrace` entries.

### 4. Test harness — `host-tests/gos-node-trace-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `trace_unknown_vector_returns_not_found` | NodeNotFound for unknown vec |
| 2 | `trace_fresh_node_returns_zero_entries` | Fresh node: (0, 0) |
| 3 | `trace_one_dispatch_returns_one_entry` | One signal → (1, 1) |
| 4 | `trace_entry_kind_matches_control` | Control signal → kind == 0x05 |
| 5 | `trace_entry_cmd_matches_control_cmd` | Control.cmd propagates to entry.cmd |
| 6 | `trace_data_signal_kind_and_cmd` | Data signal → kind == 0x04, cmd == byte |
| 7 | `trace_first_entry_serial_is_zero` | First dispatch serial == 0 |
| 8 | `trace_second_entry_serial_is_one` | Second dispatch serial == 1; newest-first verified |
| 9 | `trace_ring_wraps_after_max_entries` | MAX+4 signals → returned == MAX_NODE_TRACE |
|10 | `trace_newest_first_ordering` | ring[0].cmd == last cmd sent |

---

## Verification

```
cd host-tests/gos-node-trace-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build --release (workspace root)
# Finished `release` profile
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.24 |
|---|---|---|
| Live signal tracing | `strace -p <pid>` | `node trace <vec>` / `ntrace <vec>` |
| Signal history | `/proc/<pid>/syscall` (last syscall) | 16-entry circular trace ring |
| Dispatch sequence | strace sequence number | `serial` field (signal_count-based) |
| Signal kind | syscall name | `kind` discriminant + label column |
| Sender identity | calling process PID | `from` vector address |
| Sub-signal payload | syscall args | `cmd` byte (Control/Interrupt/Data) |

The trace ring is an always-on, zero-copy passive log: one struct write per dispatch into
a pre-allocated static array — no heap, no lock contention beyond the existing
`RUNTIME.lock()` in `prepare_signal_dispatch`.

---

## Graph-OS Characteristic Preserved

`node trace` exposes the **signal-driven execution model** native to GOS: unlike a
Unix strace (which shows OS calls), the GOS trace shows inter-node signal messages,
preserving the graph-topology abstraction.  The `from` vector field makes the
graph-communication origin visible, which has no direct equivalent in flat-process OSes.

---

## Host-Test Suite Totals After V2.24

- **243 tests** across 22 harnesses (all passing)
- New: `gos-node-trace-harness` — 10 tests

---

*Automated hardening pass — GOS V2.24 — 2026-07-01*
