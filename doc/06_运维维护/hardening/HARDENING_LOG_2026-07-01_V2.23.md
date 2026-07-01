# GOS Hardening Log — V2.23
**Date:** 2026-07-01  
**Branch:** main  
**Commit range:** V2.22 → V2.23

---

## Summary

V2.23 adds `node info <vec>` — a comprehensive single-node status view that
combines `stat` + inline edge listing into one command, analogous to
`systemctl status <unit>` on Linux or `kubectl describe pod <name>` in
Kubernetes.  The command gives operators a single pane of glass for any node:
identity, lifecycle, cumulative signal count, and all edges touching that node
(both outbound and inbound).

---

## New Shell Command Surface

| Command | Aliases | Analogous Linux cmd | Description |
|---------|---------|---------------------|-------------|
| `node info <vec>` | `ninfo <vec>` | `systemctl status <unit>` | Comprehensive single-node view: stat + edges |

### Output Layout

```
 node info
  vector:        6.1.0.0
  key:           shell.main
  plugin:        K_SHELL
  lifecycle:     ready
  signal_count:  0
  edge_out:      2
  edges (3):
    out  6.1.0.0 -[use]-> 6.1.1.0  theme.use
    out  6.1.0.0 -[mount]-> 6.1.4.0  clipboard.mount
    in   6.1.3.0 -[use]-> 6.1.0.0
  hint: stat <vec> for counters | edges <type> for type filter
```

- **out** (green) = edge originating from this node (outbound)  
- **in** (magenta) = edge targeting this node (inbound)  
- Lifecycle color-codes: green = running, yellow = suspended, red = faulted

---

## API Surface Used (no new API additions)

V2.23 composes two existing V2.x APIs into a single dispatch function:

| API | Introduced | Used by `node info` for |
|-----|-----------|------------------------|
| `gos_runtime::proc_stat_for_vector(vec)` | V2.15 | stat block: key, plugin, lifecycle, signal_count, edge_out_count |
| `gos_runtime::edge_page_for_node(vec, 0, &mut edges)` | V2.12 | inline edge listing with direction tags |

No new runtime state was added.  All dispatch logic is a pure read — no epoch
bump, no write operations.

---

## Code Changes

### `crates/k-shell/src/lib.rs`

- Added `pub fn dispatch_node_info(sink: &ConsoleSink, vec: VectorAddress)`
  - Calls `proc_stat_for_vector` → prints identity + lifecycle block
  - Calls `edge_page_for_node` → prints inline edge list with out/in direction tags
  - Color scheme: out-edges green, in-edges magenta, error red, hint gray
  - Handles "not found" (red) and "no edges" (gray) gracefully

### `crates/k-shell/src/proc.rs`

- Added command routing in `dispatch_text_command`:
  - `node info <vec>` → `dispatch_node_info`
  - `ninfo <vec>` → `dispatch_node_info` (short alias)
- Updated `help` to list `node info <vector>` and `ninfo <vector>`

---

## Host Test Harness

**Crate:** `host-tests/gos-node-info-harness`  
**Test file:** `tests/node_info.rs`  
**Test count:** 10 / 10 passed

| # | Test | What it verifies |
|---|------|-----------------|
| 1 | `node_info_stat_unknown_returns_none` | `proc_stat_for_vector` → None for unregistered vector |
| 2 | `node_info_stat_registered_node_returns_correct_key` | stat returns correct `local_node_key` |
| 3 | `node_info_edge_page_unknown_returns_not_found` | `edge_page_for_node` → `NodeNotFound` for unregistered vec |
| 4 | `node_info_no_edges_returns_zero` | Node without edges → (total=0, returned=0) |
| 5 | `node_info_one_edge_returned_for_source_node` | After `register_edge` → source sees 1 edge |
| 6 | `node_info_edge_directions_correct` | Source → Outbound; target → Inbound |
| 7 | `node_info_signal_count_starts_at_zero` | `signal_count == 0` for fresh node |
| 8 | `node_info_edge_out_count_matches_registered_edges` | `edge_out_count` increments after `register_edge` |
| 9 | `node_info_edges_visible_after_fault` | Faulted node still has edges visible |
| 10 | `node_info_edges_visible_after_resume` | Resumed node: lifecycle=Ready, edges intact |

---

## Total Host-Test Suite

| Harness | Tests | Notes |
|---------|-------|-------|
| gos-runtime-harness | 26 | |
| gos-supervisor-harness | 16 | |
| gos-rewrite-harness | 12 | |
| gos-rewrite-integration-harness | 6 | |
| gos-subscribe-harness | 10 | |
| gos-metrics-harness | 10 | |
| gos-boot-harness | 11 | |
| gos-node-inspect-harness | 8 | |
| gos-journal-harness | 14 | |
| gos-edge-inspect-harness | 10 | |
| gos-graph-diff-harness | 10 | |
| gos-proc-harness | 10 | V2.14 |
| gos-stat-harness | 10 | V2.15 |
| gos-graph-diff-epoch-harness | 10 | V2.16 |
| gos-graph-topo-harness | 10 | V2.17 |
| gos-graph-health-harness | 10 | V2.18 |
| gos-theme-node-harness | 10 | V2.19 |
| gos-plugin-list-harness | 10 | V2.20 |
| gos-kill-harness | 10 | V2.21 |
| gos-resume-harness | 10 | V2.22 |
| **gos-node-info-harness** | **10** | **V2.23 (new)** |
| **Total** | **233** | **all green** |

---

## Invariants Preserved

- `dispatch_node_info` is a pure read — no epoch bump, no write ops
- `edge_page_for_node` called with `N=16`; pagination hint printed if total > 16
- `TEST_LOCK: Mutex<()>` + `reset()` used in all 10 tests for isolation
- Harness has its own `.cargo/config.toml` with `target = "x86_64-pc-windows-msvc"`

---

## Next Steps (V2.24 candidates)

- `graph watch` / `watch nodes` — auto-refreshing node table (like `watch -n1 proc`)
- `journal ring <N>` — runtime-configurable JournalRing capacity
- PAL_U32 → attribute node refactor (Demo A prerequisite)
- `node trace <vec>` — signal dispatch history for one node
