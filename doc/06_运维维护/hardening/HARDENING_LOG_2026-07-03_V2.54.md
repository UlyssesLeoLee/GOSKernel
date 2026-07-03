# Hardening Log — V2.54: Graph Attractor Set Detection

**Date:** 2026-07-03  
**Branch:** feat/vk-auto-live-surface  
**Version:** V2.54  
**Author:** Automated hardening cycle  

---

## Summary

Implemented `graph attractor` — **attractor-set classification** of every live
kernel node into one of three roles based on the condensation DAG of the SCC
decomposition.

An **attractor** (bottom SCC / sink SCC) is a strongly-connected component with
no outgoing edges to any node outside the component.  Once signal or execution
flow enters an attractor it can never escape — it is a "trap" or "stable
fixed-point" of the directed graph.

**Key insight for graph-theory OS:**  
Every finite directed graph has at least one attractor SCC.  Isolated nodes and
self-loop-only nodes are trivial attractor SCCs.  The `graph attractor` command
exposes which kernel service nodes form stable loops (attractors), which are
one hop from stability (drains), and which are far from any stable loop
(transients).

---

## Node Role Classification

| Role | Value | Definition |
|------|-------|------------|
| **attractor** | 0 | Member of a bottom SCC — no condensation out-edges; flow can never leave. |
| **drain** | 1 | SCC has a direct condensation edge to at least one attractor SCC (one step from stability). |
| **transient** | 2 | SCC has outgoing condensation edges, but none lead directly to an attractor SCC (≥2 hops from stability). |

Output is sorted role-ascending (attractors first, drains second, transients last).

---

## Algorithm

**Kosaraju two-pass DFS + two condensation edge scans.  O(V+E) total.**

1. **Phase 1 — Forward DFS:** Build finish-order stack (standard Kosaraju pass 1).
2. **Phase 2 — Transposed DFS:** Process nodes in reverse finish order; assign
   SCC IDs (standard Kosaraju pass 2).
3. **Phase 3a — Condensation scan:** For each live edge where `scc_id[from] ≠ scc_id[to]`,
   mark `scc_has_out[scc_id[from]] = true`.  Self-loop edges and intra-SCC edges
   are skipped.  SCCs with `scc_has_out == false` are attractor SCCs.
4. **Phase 3b — Drain scan:** For each cross-SCC edge where the destination SCC
   is an attractor (`!scc_has_out[scc_id[to]]`), mark `scc_adj_attract[scc_id[from]] = true`.
5. **Phase 4 — Pack output:** Emit nodes in role order (0→1→2) within stable
   slot order.

**Correctness notes:**
- Self-loops create no condensation edges (`from_slot == to_slot` guard) — a
  node with only a self-loop is a trivial attractor SCC, correctly classified.
- Bidirectional pairs `A↔B` form a single SCC; both `A→B` and `B→A` are
  intra-SCC edges and do not appear as condensation edges.
- Isolated nodes (no edges) are always attractor SCCs.

---

## Files Changed

### `crates/gos-runtime/src/lib.rs`

- **`GraphRuntime::graph_attractor_inner<N>()`** — private method inserted after
  `graph_between_inner`.  Implements Kosaraju SCC + two condensation edge scans.
  Returns `([VectorAddress; N], [u8; N], usize, usize)` — nodes, roles, total,
  attractor_count.

- **`pub fn graph_attractor<N>()`** — public API wrapper inserted after
  `pub fn graph_between`.  Routes to `RUNTIME.lock().graph_attractor_inner()`.

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_attractor(sink)`** — new dispatch function inserted
  before `dispatch_uname`.  Calls `gos_runtime::graph_attractor::<128>()`.
  Color scheme: bright green (10) for attractor, bright yellow (14) for drain,
  dark grey (8) for transient.  Footer shows per-role counts.

### `crates/k-shell/src/proc.rs`

- Added routing for `graph attractor` in the shell command dispatch chain
  (after the `graph between` branch):
  ```
  "graph attractor" | "attractor" | "gattractor" | "graph attract" | "attract"
  ```

### `host-tests/gos-graph-attractor-harness/` (new harness)

New harness: 10 tests, L4=31 VectorAddress namespace.

| Test | Scenario | Key Assertion |
|------|----------|---------------|
| 1 | Empty graph | total=0, attractor_count=0 |
| 2 | Single isolated node | role=0 (attractor), attractor_count=1 |
| 3 | A→B path | B=attractor(0), A=drain(1) |
| 4 | A→B→C path | C=attractor(0), B=drain(1), A=transient(2) |
| 5 | A↔B bidirectional | both role=0 (single attractor SCC) |
| 6 | Cycle A→B→A + C→A | A,B=attractor(0); C=drain(1) |
| 7 | Diamond A→{B,C}→D | D=attractor(0); B,C=drain(1); A=transient(2) |
| 8 | Two disconnected cycles | all 4 nodes attractor(0); attractor_count=4 |
| 9 | Sort order | roles[i-1] ≤ roles[i]; attractors before drains before transients |
| 10 | Self-loop A→A + isolated B | both attractor(0) (self-loop ≠ condensation edge) |

All 10 tests: **PASS** (0.01s).

---

## Shell Commands

| Command | Aliases |
|---------|---------|
| `graph attractor` | `attractor`, `gattractor`, `graph attract`, `attract` |

---

## OS Analogy

`systemctl list-units --state=running` service stability audit:
- **attractor** — always-running service loop; once entered, never leaves (e.g., init, PID 1, timer wheel)
- **drain** — converges in one step to a stable loop (e.g., a one-shot setup task that transfers control to init)
- **transient** — must pass through multiple intermediate services before reaching stability

---

## Version Sequence

```
V2.51  node checkpoint    — snapshot node state to diff ring
V2.52  graph sim          — xorshift32 random walk simulation
V2.53  graph between      — weighted betweenness centrality (Brandes + Dijkstra)
V2.54  graph attractor    — attractor-set classification (Kosaraju + condensation) ← THIS
```

**Next (V2.55):** PAL_U32 → attribute node refactor (Demo A prerequisite)  
**Total host tests:** 513 (503 + 10 new)
