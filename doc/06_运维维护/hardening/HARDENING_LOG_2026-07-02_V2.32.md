# GOS Hardening Log — V2.32 — 2026-07-02

## Summary

V2.32 adds **directed cycle detection** to the GOS runtime graph via iterative
3-color DFS, exposing a `graph cycles` / `cycles` shell command analogous to
`tsort` detecting circular dependencies or `cargo`'s dependency-cycle error.
This is the most fundamental graph-theory safety check for a graph-OS: circular
signal routing creates deadlocks; circular dependency declarations prevent
deterministic boot ordering; circular rewrite-rule chains cause infinite
oscillation.  The new API lets operators confirm the live graph is a DAG at any
time.

---

## Changes

### 1. `find_graph_cycle_inner<const N>` + `is_cyclic_inner` — gos-runtime

New methods on `GraphRuntime` (`crates/gos-runtime/src/lib.rs`):

```rust
pub fn find_graph_cycle_inner<const N: usize>(&self) -> ([VectorAddress; N], usize)
pub fn is_cyclic_inner(&self) -> bool
```

**Algorithm**: iterative DFS with 3-color node marking:
- `WHITE` (0) = unvisited
- `GRAY`  (1) = on the current DFS path (ancestor)
- `BLACK` (2) = fully explored

A *back edge* — an edge from the current GRAY node to any GRAY ancestor — closes
a cycle.  When detected, the cycle path is reconstructed from the DFS stack by
finding the index of the back-edge target in the current path and slicing from
that index to the current position, then appending the target node once more to
close the loop (so `path[0] == path[len-1]`).

Properties:
- **O(V+E)** time, same asymptotic cost as BFS path from V2.31.
- **no_std safe** — all working storage is fixed-size stack arrays.
- **No recursion** — explicit DFS stack avoids stack overflow on large graphs.
- Correctly handles self-loops (A→A), multi-node cycles, and disconnected graphs
  with isolated cyclic components.

`is_cyclic_inner` delegates to `find_graph_cycle_inner::<2>()` (capacity-2
path), which detects any cycle while allocating minimal stack memory.

### 2. Public API — gos-runtime

```rust
pub fn find_graph_cycle<const N: usize>() -> ([VectorAddress; N], usize)
pub fn is_cyclic() -> bool
```

`find_graph_cycle` locks `RUNTIME`, calls `find_graph_cycle_inner`, and returns
`(path, length)` where `length == 0` means the graph is acyclic.
`is_cyclic` is a convenience wrapper returning only the boolean.

### 3. `dispatch_graph_cycles` shell command — k-shell (`crates/k-shell/src/lib.rs`)

Output format:
- Banner: `GRAPH CYCLES` (cyan header)
- **DAG case**: green "no cycles detected  (directed acyclic graph)"
- **Cycle case**: red "CYCLE DETECTED  N nodes", then each node of the cycle
  numbered with arrows showing flow direction, closing with a ↩ symbol at the
  back-edge node

Color coding:
- Green header on cycle absence
- Red on cycle detection
- Yellow for intermediate cycle nodes
- Cyan for vector addresses
- ↓ arrows between non-closing hops, ↩ at the closing back-edge

### 4. Shell routing — k-shell (`crates/k-shell/src/proc.rs`)

New branches added to the command dispatcher:

```
"graph cycles" | "cycles" | "graph cyclic" | "cyclic"
    → dispatch_graph_cycles(sink)
```

Help text updated with:
```
graph cycles       detect directed cycles in the graph (like tsort cycle-check)
```

### 5. Test harness — `host-tests/gos-graph-cycles-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `empty_graph_no_cycle` | Empty runtime returns cycle_len == 0 |
| 2 | `single_node_no_cycle` | Isolated node with no edges → acyclic |
| 3 | `linear_chain_is_acyclic` | A→B→C chain → DAG, no cycle |
| 4 | `self_loop_is_cyclic` | A→A self-loop detected, len >= 2 |
| 5 | `two_node_cycle_detected` | A→B→A detected, len >= 3 |
| 6 | `three_node_cycle_detected` | A→B→C→A detected, len >= 4 |
| 7 | `diamond_dag_is_acyclic` | A→B, A→C, B→D, C→D (diamond DAG) → acyclic |
| 8 | `mixed_dag_and_cycle_detected` | DAG subgraph + isolated D→E→D cycle → detected |
| 9 | `is_cyclic_false_for_dag` | `is_cyclic()` returns false for pure DAG |
|10 | `is_cyclic_true_when_cycle_exists` | `is_cyclic()` returns true when A→B→C→A exists |

---

## Verification

```
cd host-tests/gos-graph-cycles-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Regression (graph-path harness):
```
cd host-tests/gos-graph-path-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.32 |
|---|---|---|
| Circular dependency detection | `tsort` / `cargo check` | `graph cycles` shell command |
| DAG verification | `toposort` assert in build systems | `is_cyclic()` API + shell |
| Deadlock-path detection | `systemd` dependency-cycle check | `graph cycles` on signal graph |
| Algorithm | DFS 3-color | Iterative DFS, O(V+E), no_std |
| Output | `tsort: input contains a loop:` | cycle path with vector addresses + node keys |

The `find_graph_cycle` function uses the same stack-array approach as BFS path
(V2.31) and diff-ring (V2.13) — no heap, no recursion, constant compile-time
working memory — preserving the graph-OS's deterministic resource footprint.

---

## Graph-OS Characteristic Preserved

`graph cycles` operates directly on the **directed edge topology** of the live
graph — not on a process list or file-system hierarchy.  The cycle path output
shows node vectors and plugin keys, grounding the abstract graph-theory concept
in the runtime's actual topology.  This keeps the observability surface rooted
in the graph model that defines GOS.

---

*Automated hardening pass — GOS V2.32 — 2026-07-02*
