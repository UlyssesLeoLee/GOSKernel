# GOS Hardening Log — V2.36 — 2026-07-02

## Summary

V2.36 adds transitive reachability analysis via `graph reachable <vec>` — the
first graph *closure* operation in GOS.  It answers "which nodes are reachable
from this node via directed edges?" — the graph-OS equivalent of
`systemctl list-dependencies --all`, `cargo tree -p <crate>`, or
`ldd --recursive`.  This completes a natural triad with `graph path <from> <to>`
(point-to-point BFS, V2.31) and `graph scc` (component membership, V2.34).

---

## Changes

### 1. `graph_reachable_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New method on `Runtime`:

```rust
pub fn graph_reachable_inner<const N: usize>(
    &self,
    from: VectorAddress,
) -> ([VectorAddress; N], usize)
```

Algorithm:
- Iterative DFS with a `[bool; MAX_NODES]` visited bitmap.
- Source node is marked visited first (prevents infinite loops on cycles).
- All newly discovered neighbours are pushed onto the DFS stack and added to
  the reachable set (excluding the source slot itself).
- Output is sorted ascending by `VectorAddress.as_u64()` using insertion sort
  (N ≤ 128, negligible cost).
- Returns `(out, 0)` if `from` is not registered or has no outbound paths.
- Complexity: O(V + E), no_std safe, fixed-size stack arrays only.
- Self-loops are skipped (`if nbr_slot == cur_slot { continue; }`).

### 2. `graph_reachable<N>` public API — gos-runtime

```rust
pub fn graph_reachable<const N: usize>(from: VectorAddress) -> ([VectorAddress; N], usize)
```

Thin wrapper: acquires `RUNTIME.lock()` and delegates to `graph_reachable_inner`.
`N` controls the output buffer depth; cap at `MAX_NODES = 128` for full coverage.

### 3. `dispatch_graph_reachable` — k-shell (`crates/k-shell/src/lib.rs`)

New display function:

```
 graph reachable from 15.1.1.0
 ───────────────────────────────────────────────────────────
  15.1.2.0
  15.1.3.0
  15.1.4.0
 ───────────────────────────────────────────────────────────
  3 reachable  |  use 'graph path <from> <to>' to trace a specific route
```

- Color-coded header (cyan), separator lines (dark grey), count footer.
- Empty case prints: `(no reachable nodes — isolated or not registered)`.

### 4. Shell routing — k-shell (`crates/k-shell/src/proc.rs`)

New command patterns (dispatched after `graph condensation`):

```
graph reachable <vec>   primary form
reachable <vec>         short alias
reach <vec>             shorter alias
graph reach <vec>       graph-prefixed alias
```

`help` text updated with two new entries:
```
  graph reachable <V>   all nodes reachable from V via directed edges (like systemctl list-dependencies --all)
  reachable <V>         alias for graph reachable
```

### 5. Test harness — `host-tests/gos-graph-reachable-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `unregistered_source_returns_empty` | Returns 0 when `from` not in runtime |
| 2 | `isolated_node_returns_empty` | Node with no edges → 0 reachable |
| 3 | `single_edge_reaches_one_node` | A→B: reachable from A = {B} |
| 4 | `chain_reaches_transitive_node` | A→B→C: reachable from A = {B, C} |
| 5 | `fan_out_reaches_both_children` | A→B, A→C: both B and C reachable |
| 6 | `cycle_does_not_loop_forever` | A→B→A: terminates, returns {B} |
| 7 | `triangle_reaches_all_other_members` | A→B→C→A: reachable = {B, C} |
| 8 | `reachable_from_midpoint_excludes_predecessor` | B in A→B→C→D: reach = {C,D}, not A |
| 9 | `disconnected_components_not_reached` | A→B isolated from C→D: only {B} |
| 10 | `reachable_output_sorted_ascending` | 4-node fan-out in reverse order: output sorted |

---

## Verification

```
cd host-tests/gos-graph-reachable-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Regression — condensation harness still green:
```
cd host-tests/gos-graph-condensation-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.36 |
|---|---|---|
| Transitive dependency list | `systemctl list-dependencies --all <svc>` | `graph reachable <vec>` |
| Package closure | `cargo tree -p <crate>` | `graph reachable <vec>` |
| Shared-library closure | `ldd --recursive <binary>` | `graph reachable <vec>` |
| Network flood-fill | `traceroute --all-hops` (flood) | `graph reachable <vec>` |
| Cycle safety | BFS/DFS termination invariant | visited bitmap prevents revisits |
| Sort order | Stable, reproducible | ascending VectorAddress (as_u64) |

---

## Graph Algorithm Suite (V2.32–V2.36)

| Version | Command | Answers | Algorithm |
|---|---|---|---|
| V2.32 | `graph cycles` | "is there a cycle?" | DFS 3-color, O(V+E) |
| V2.33 | `graph toposort` | "dependency order?" | Kahn's BFS, O(V+E) |
| V2.34 | `graph scc` | "where are all cycles?" | Kosaraju 2-pass DFS, O(V+E) |
| V2.35 | `graph condensation` | "macro-structure?" | Kosaraju + adj matrix |
| **V2.36** | **`graph reachable`** | **"what can X reach?"** | **DFS visited bitmap, O(V+E)** |

With V2.36, GOS has five core structural analysis commands covering the
fundamental graph questions: connectivity, ordering, components, hierarchy,
and reachability.

---

## Graph-OS Characteristic Preserved

`graph reachable` exposes the **directed signal propagation closure** of a node:
which other nodes will eventually receive signals that originate at `<vec>`,
following all outbound edges transitively.  This is a uniquely graph-theoretic
view of OS runtime dependency — no traditional OS exposes this primitive.

---

*Automated hardening pass — GOS V2.36 — 2026-07-02*
