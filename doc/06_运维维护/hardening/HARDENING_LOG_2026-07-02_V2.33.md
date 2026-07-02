# GOS Hardening Log — V2.33 — 2026-07-02

## Summary

V2.33 adds **topological sort** (`graph toposort`) to the live node graph — a Kahn's BFS
algorithm that produces a dependency ordering where every source (in-degree 0) precedes its
successors.  Analogous to `tsort(1)` on POSIX, `cmake --build` dependency ordering, or
`cargo build`'s crate graph resolution.  Naturally complements V2.32's cycle detection: run
`graph cycles` first to verify the graph is a DAG, then `graph toposort` to see the boot/
init order of all nodes.

---

## Changes

### 1. `graph_toposort_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New private method on `Runtime`:

```rust
pub fn graph_toposort_inner<const N: usize>(&self) -> ([VectorAddress; N], usize, bool)
```

**Algorithm**: Kahn's BFS (in-degree queue).

1. Collect live node slots into a compact fixed array.
2. Compute in-degree for every slot by scanning the edge table; **self-loops are excluded** so
   a node with only a self-loop still has in-degree 0 and is emitted normally.
3. Seed the BFS queue with all in-degree-0 nodes (sources).
4. Pop from queue → emit to output → decrement in-degree of all successor slots; if a
   successor's in-degree reaches 0, enqueue it.
5. `is_dag = out_len == node_count`: when all nodes are emitted the graph is acyclic; when
   cyclic nodes remain stuck (in-degree never reaches 0) `is_dag` is `false`.

Properties:
- **O(V+E)** time, O(V) working state.
- **no_std safe** — fixed stack arrays only, no heap allocation.
- Returns `(order, length, is_dag)` triple so callers get the ordering AND the DAG flag in
  one pass.

New public wrapper:

```rust
pub fn graph_toposort<const N: usize>() -> ([VectorAddress; N], usize, bool)
```

Acquires the global `RUNTIME` lock and delegates to `graph_toposort_inner`.
Cap N at 128 (= MAX_NODES) for full-graph coverage.

### 2. `dispatch_graph_toposort` — k-shell (`crates/k-shell/src/lib.rs`)

New shell dispatch function rendering the topological order as a numbered list:

- Header banner: black-on-cyan `GRAPH TOPOSORT`.
- If graph is empty: `no nodes registered`.
- If cyclic: red WARNING + hint to run `graph cycles`.
- Node list: rank (1-based) | vector address (cyan, 12-char padded) | node key (green) | plugin name (dim).
- Footer: total emitted / total, plus DAG confirmation or cyclic-component count.

### 3. Shell routing — k-shell (`crates/k-shell/src/proc.rs`)

New command aliases wired in the dispatch branch:

| Input | Action |
|---|---|
| `graph toposort` | `dispatch_graph_toposort(sink)` |
| `toposort` | alias |
| `topo sort` | alias (space-separated variant) |
| `graph tsort` | alias (POSIX `tsort` analogue) |
| `tsort` | alias |

Help text updated to include:
```
  graph toposort     topological dependency ordering of all nodes (like tsort)
```

### 4. Test harness — `host-tests/gos-graph-toposort-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `empty_graph_toposort_is_empty_dag` | Empty runtime → length 0, is_dag true |
| 2 | `single_node_toposort` | 1 node, no edges → emitted, is_dag true |
| 3 | `linear_chain_toposort_order` | A→B→C → order A,B,C; pos_a < pos_b < pos_c |
| 4 | `two_node_cycle_is_not_dag` | A→B→A → is_dag false |
| 5 | `diamond_dag_toposort_is_dag` | Diamond → is_dag true, all 5 nodes emitted |
| 6 | `diamond_dag_a_precedes_b_and_c` | A precedes both B and C in diamond |
| 7 | `diamond_dag_d_is_last` | D follows B and C (shared sink) |
| 8 | `self_loop_node_still_emitted` | A→A self-loop: excluded from in-degree, node still emitted |
| 9 | `disconnected_chains_all_emitted` | Two independent chains all emitted, is_dag true |
|10 | `cyclic_graph_partial_sort` | A→B→C→A cycle: only 2 isolated nodes (D,E) emitted |

---

## Verification

```
cd host-tests\gos-graph-toposort-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## Production Quality Rationale

| Capability | Linux/POSIX equivalent | GOS V2.33 |
|---|---|---|
| Dependency ordering | `tsort(1)` (processes stdin pairs) | `graph toposort` (live graph, O(V+E)) |
| Build order resolution | `cmake --build` / `cargo build` | toposort output = boot/init order |
| Cycle guard | `tsort` exits non-zero on cycle | `is_dag` flag + WARNING banner |
| Partial sort on cycle | `tsort` stops at first cycle | Emits all acyclic nodes; cyclic stuck |
| Shell surface | `tsort < deps.txt` | `graph toposort` (no file needed) |
| Self-loop handling | `tsort` may loop forever | Self-loops excluded from in-degree |

The Kahn's algorithm approach was chosen over DFS-based toposort because:
1. It naturally produces the `is_dag` flag as a free by-product (counts emitted vs. total).
2. Its iterative queue structure maps cleanly to fixed-size arrays (no recursion stack).
3. It handles disconnected components without needing an outer restart loop.

---

## Graph-OS Characteristic Preserved

`graph toposort` exposes the **dependency ordering of the live plugin graph** — the same
structural information that a package manager uses to determine build order, or that an init
system uses to determine service startup order.  In GOS this is not a static manifest but a
live query against the runtime edge table, reflecting any topology mutations that have been
made since boot (tracked in the diff ring since V2.13).

---

## Interaction with V2.32 (graph cycles)

`graph toposort` and `graph cycles` form a complementary pair:

```
graph cycles    →  "is this a DAG?"        (DFS, 3-color, O(V+E))
graph toposort  →  "in what order?"        (Kahn's BFS, O(V+E))
```

Recommended operator workflow:
```
> graph cycles      # verify acyclic
  no cycles detected  (directed acyclic graph)
> graph toposort    # get boot order
    1   1.0.0.1     boot.loader  (k-boot)
    2   1.0.0.2     mm.slab      (k-heap)
    ...
```

---

*Automated hardening pass — GOS V2.33 — 2026-07-02*
