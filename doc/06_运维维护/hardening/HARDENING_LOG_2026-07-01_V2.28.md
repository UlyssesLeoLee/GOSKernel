# GOS Hardening Log — V2.28 — 2026-07-01

## Summary

V2.28 adds `uname` / `ver` / `version` shell commands and a `runtime_capacity()` public
API that exposes all compile-time capacity limits as a typed `RuntimeCapacity` struct.
This is GOS's equivalent of `uname -a` + `sysctl kern.*` + `getrlimit` on Linux — an
operator-queryable view of what the running kernel was built to support, without reading source.

---

## Changes

### 1. `RuntimeCapacity` struct — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New public struct exported from gos-runtime:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeCapacity {
    pub max_nodes: usize,           // MAX_NODES  = 128
    pub max_edges: usize,           // MAX_EDGES  = 512
    pub max_plugins: usize,         // MAX_PLUGINS = 32
    pub max_ready_queue: usize,     // MAX_READY_QUEUE = 256
    pub max_signal_queue: usize,    // MAX_SIGNAL_QUEUE = 512
    pub max_fault_queue: usize,     // MAX_FAULT_QUEUE = 32
    pub max_diff_ring: usize,       // MAX_DIFF_RING = 128
    pub max_node_trace: usize,      // MAX_NODE_TRACE = 16
    pub max_node_log: usize,        // MAX_NODE_LOG = 16
    pub max_subscribe_pairs: usize, // MAX_SUBSCRIBE_PAIRS = 64
    pub abi_major: u8,              // GOS_ABI_MAJOR = 2
    pub abi_minor: u8,              // GOS_ABI_MINOR = 0
    pub abi_patch: u16,             // GOS_ABI_PATCH = 0
    pub protocol_version: u16,      // CONTROL_PLANE_PROTOCOL_VERSION = 1
}
```

### 2. `runtime_capacity()` — gos-runtime public API

```rust
pub fn runtime_capacity() -> RuntimeCapacity { ... }
```

- Pure constant read — **no lock, no allocation**.
- Reads all limits from compile-time constants in gos-runtime and gos-protocol.
- Never panics; always returns a valid struct.

### 3. `dispatch_uname()` — k-shell (`crates/k-shell/src/lib.rs`)

New shell display function that prints:

```
 kernel info
  GOS v2.28 (graph-kernel)  abi: 2.0.0  protocol: 1
  capacity
    nodes:          N / 128
    edges:          N / 512
    plugins:        N / 32
    ready-queue:    256  signal-queue: 512  fault-queue: 32
    diff-ring:      128  subscribe-pairs: 64
    node-trace:     16 (ring depth per node)
    node-log:       16 (ring depth per node)
  arch: x86_64  no_std  tick: T
```

The live snapshot values (current node/edge/plugin count and tick) are shown alongside
capacity limits, giving operators an at-a-glance view of utilisation.

### 4. `uname` / `ver` routing — k-shell (`crates/k-shell/src/proc.rs`)

Added before the `graph health` arm:

```
uname        →  dispatch_uname(sink)
uname -a     →  dispatch_uname(sink)   [flag alias]
ver          →  dispatch_uname(sink)   [short alias]
version      →  dispatch_uname(sink)   [long alias]
```

Help text updated to include `uname` and `ver` / `version` entries.

### 5. Test harness — `host-tests/gos-uname-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `capacity_max_nodes_matches_constant` | max_nodes == MAX_NODES (128) |
| 2 | `capacity_max_edges_matches_constant` | max_edges == MAX_EDGES (512) |
| 3 | `capacity_max_plugins_matches_constant` | max_plugins == MAX_PLUGINS (32) |
| 4 | `capacity_max_ready_queue_matches_constant` | max_ready_queue == MAX_READY_QUEUE (256) |
| 5 | `capacity_max_signal_queue_matches_constant` | max_signal_queue == MAX_SIGNAL_QUEUE (512) |
| 6 | `capacity_max_fault_queue_matches_constant` | max_fault_queue == MAX_FAULT_QUEUE (32) |
| 7 | `capacity_max_diff_ring_matches_constant` | max_diff_ring == MAX_DIFF_RING (128) |
| 8 | `capacity_max_node_trace_matches_constant` | max_node_trace == MAX_NODE_TRACE (16) |
| 9 | `capacity_max_node_log_matches_constant` | max_node_log == MAX_NODE_LOG (16) |
|10 | `capacity_abi_and_protocol_version_correct` | abi_major == 2, abi_minor == 0, protocol_version == 1 |

---

## Verification

```
cd host-tests/gos-uname-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cargo build --release
# Finished `release` profile [optimized]
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.28 |
|---|---|---|
| Kernel version | `uname -a` | `uname` shell command |
| Capacity limits | `getrlimit` / `sysctl kern.*` | `RuntimeCapacity` struct |
| ABI version | `/proc/version` | `abi_major.abi_minor.abi_patch` |
| Protocol version | `/proc/net/protocols` | `protocol_version` field |
| Live utilisation | `free` / `vmstat` | node/edge/plugin current vs. max |
| Zero-overhead query | reading `/proc` | pure const — no lock, no alloc |

`runtime_capacity()` is a compile-time pure read — no Mutex lock is taken, no heap
allocation is performed.  It is safe to call from interrupt context.

The `RuntimeCapacity` struct is `#[derive(Debug, PartialEq, Eq)]`, enabling test harnesses
to compare the entire struct in one assertion when regression-testing capacity invariants
across ABI bumps.

---

## Graph-OS Characteristic Preserved

`uname` shows GOS graph capacity limits (max_nodes, max_edges, max_subscribe_pairs)
rather than traditional OS concepts like RAM or CPU count — keeping the operator's
mental model anchored to the graph topology layer that defines GOS's resource model.

---

## Cumulative Test Suite (V2.28)

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
| **gos-uname-harness** | **10** | **V2.28** |
| **Total** | **283** | |

---

*Automated hardening pass — GOS V2.28 — 2026-07-01*
