# GOS Hardening Log — V2.31 — 2026-07-02

## Summary

V2.31 adds BFS-based graph path-finding to the runtime and a `graph path <from> <to>`
shell command, giving operators the ability to trace the shortest directed hop sequence
between any two node vector addresses — the graph-theory equivalent of `traceroute`.

---

## Changes

### 1. `find_graph_path_inner<const N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

New private method on `GraphRuntime`:

```rust
pub fn find_graph_path_inner<const N: usize>(
    &self,
    from: VectorAddress,
    to: VectorAddress,
) -> ([VectorAddress; N], usize)
```

Algorithm:
- **BFS** over the flat edge table using fixed-size stack arrays only (no heap, `no_std` safe).
- `visited: [bool; MAX_NODES]` + `prev: [usize; MAX_NODES]` predecessor tracking.
- Ring queue `q: [usize; MAX_NODES]` for BFS frontier.
- Special-cases: `from == to` → trivial single-element path, `from/to` not registered → 0.
- Path reconstruction: traces `prev[]` from `to_slot` back to `from_slot`, then reverses in-place.
- Returns `(path_array, path_length)` where `path_length == 0` means no path found.

### 2. `find_graph_path<const N>` — public API

```rust
pub fn find_graph_path<const N: usize>(
    from: VectorAddress,
    to: VectorAddress,
) -> ([VectorAddress; N], usize)
```

Delegates to `RUNTIME.lock().find_graph_path_inner(from, to)`.

### 3. `dispatch_graph_path()` — k-shell (`crates/k-shell/src/lib.rs`)

New public function `dispatch_graph_path(sink, from, to)`:

- **Header banner**: `GRAPH PATH  <from> → <to>` (black-on-cyan accent).
- **Hop list**: one line per hop with hop number, vector address, node key, and plugin name.
  - First and last hops coloured green (endpoints); intermediate hops yellow.
- **Error case**: `no path found (nodes unreachable or not registered)` in red.
- **Footer**: `N hop(s) | from: <vec> | to: <vec>`.

Output example (A→B→C chain):
```
 GRAPH PATH  10.3.1.0 → 10.3.3.0

  hop  1   10.3.1.0    gp.alpha  (kl-graph-path-harness)
  hop  2   10.3.2.0    gp.beta   (kl-graph-path-harness)
  hop  3   10.3.3.0    gp.gamma  (kl-graph-path-harness)

 3 hops  |  from: 10.3.1.0  |  to: 10.3.3.0
```

### 4. Shell routing — k-shell (`crates/k-shell/src/proc.rs`)

- Added `graph path <from> <to>` branch in the command dispatch chain.
- Parses two `VectorAddress::parse()` tokens separated by whitespace.
- Error messages for missing/malformed vector arguments.
- Added `graph path` entry to `help` output.

### 5. Test harness — `host-tests/gos-graph-path-harness/` (10 tests, all passing)

| # | Test | Verifies |
|---|------|----------|
| 1 | `empty_graph_no_path` | No edges → path length 0 |
| 2 | `self_path_returns_one_hop` | from == to → length 1, contains from |
| 3 | `direct_edge_returns_two_hops` | A→B → [A, B], length 2 |
| 4 | `path_starts_with_from_vector` | path[0] == from_vector |
| 5 | `path_ends_with_to_vector` | path[len-1] == to_vector |
| 6 | `two_hop_chain_path` | A→B→C, query A→C → [A, B, C], length 3 |
| 7 | `bfs_finds_shorter_path` | A→B→C and A→C direct; BFS picks length 2 |
| 8 | `unregistered_from_returns_zero` | Unknown from_vec → 0 |
| 9 | `unregistered_to_returns_zero` | Unknown to_vec → 0 |
|10 | `reverse_direction_not_traversable` | Directed: only A→B; B→A returns 0 |

---

## Verification

```
cd host-tests/gos-graph-path-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

Regression:
```
cd host-tests/gos-graph-diff-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed

cd host-tests/gos-edge-inspect-harness && cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## Production Quality Rationale

| Capability | Linux/macOS equivalent | GOS V2.31 |
|---|---|---|
| Network path trace | `traceroute <host>` / `pathping <host>` | `graph path <from> <to>` |
| Graph reachability | `ip route get <dst>` | BFS over edge table |
| Hop-by-hop visibility | `traceroute -n` | per-hop vector + node key + plugin |
| Directed awareness | routing table direction | edge direction respected (no reverse) |
| Shortest path | dijkstra in routing daemons | BFS (unit weights) |

The BFS uses only fixed-size stack arrays (`visited[128]`, `prev[128]`, `q[128]`)
with no heap allocation, making it safe in the `no_std` kernel context and
O(V + E) time, O(V) space.

---

## Graph-OS Characteristic Preserved

`graph path` exposes the **directed graph topology** as a first-class operator
primitive: every edge in the graph is part of the searchable connectivity fabric.
Rather than tracing IP packets through routers, GOS traces **signal-dispatch paths**
through graph nodes — making graph reachability a core shell capability alongside
`ps`, `top`, and `traceroute` analogues already present.

---

## Shell Command Surface (V2.31 addition)

```
graph path <from> <to>    BFS shortest path from node at <from> to node at <to>
                          (like traceroute / pathping)
```

---

*Automated hardening pass — GOS V2.31 — 2026-07-02*
