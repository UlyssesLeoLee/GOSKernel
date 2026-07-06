# GOSKernel Hardening Log — V3.03
**Date:** 2026-07-06  
**Algorithm:** Hamiltonian Path/Circuit Detection — Iterative Backtracking DFS  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.03): Hamiltonian path/circuit -- iterative backtracking DFS + gos-graph-hamiltonian-harness (10 tests)

---

## Summary

V3.03 adds **Hamiltonian path and circuit detection** to the GOSKernel graph theory runtime.

A **Hamiltonian path** visits every node in the graph exactly once.  
A **Hamiltonian circuit** is a Hamiltonian path that returns to the starting node.

This is the natural vertex-traversal complement to **Eulerian** (V2.87), which visits every **edge** once:

| Concept | Visits | Condition | Complexity |
|---------|--------|-----------|------------|
| Eulerian path | Every **edge** once | ≤2 nodes with odd in/out-degree | O(V+E) |
| Hamiltonian path | Every **vertex** once | NP-complete in general | Backtracking |

---

## Public API

### `gos_runtime::graph_hamiltonian<const N: usize>() -> ([VectorAddress; N], usize, bool, bool, usize)`

Returns `(path_vecs, path_len, has_circuit, has_path, node_count)`:
- `path_vecs[0..path_len]` — nodes of the found Hamiltonian path/circuit in traversal order
- `path_len` — equals `node_count` when a Ham. path was found; 0 if none found
- `has_circuit` — true iff a directed Hamiltonian circuit was found (path_len > 0 and last→first edge exists)
- `has_path` — true iff a directed Hamiltonian path was found (`has_circuit` ⇒ `has_path`)
- `node_count` — total live nodes

**Directed graph:** Edge A→B does NOT imply B→A.  
**Self-loops:** Excluded from adjacency (don't count toward Ham. traversal).  
**Single node:** Trivially `has_circuit = has_path = true` (degenerate case).  
**Step limit:** 5 000 000 — prevents hanging on adversarial graphs; OS subsystem graphs terminate well below this limit.

---

## Algorithm: Iterative Backtracking DFS with Dead-End Pruning

**Approach:** Iterative DFS (no recursion, no heap — kernel stack only).

**Core state:**
- `path[0..depth]` — current partial path (compact node indices, `u8`)
- `visited: u128` — bitmask of nodes currently in path
- `cand[d]: u128` — remaining successors of `path[d]` not yet tried for position `d+1`

**Outer loop:** Try each node as the starting point (break early if circuit found).

**Inner loop:**
1. If `depth == nc`: all nodes placed → Ham. path found; check circuit (last→start edge); backtrack.
2. If `cand[depth-1] == 0`: no more candidates → backtrack (remove `path[depth-1]` from visited).
3. Otherwise: pick next candidate `v` from `cand[depth-1]`, apply **dead-end pruning**, then push `v`.

**Dead-end pruning:**  
After tentatively pushing node `v`, count unvisited nodes `w` where `adj[w] & unvisited_after == 0`  
(i.e., `w` has no successors in the remaining unvisited set — it can only be the path terminus).  
If **≥2 such nodes** exist, at most one can be the last node → prune this branch.  
This is sound: pruning is only applied when `remaining > 1`, and having two "dead-end" nodes is a contradiction.

**Stack usage (no heap):**
- `adj: [u128; MAX_NODES]` = 2 048 bytes (directed adjacency bitmasks)
- `path: [u8; MAX_NODES]` = 128 bytes
- `cand: [u128; MAX_NODES]` = 2 048 bytes
- `best_path: [u8; MAX_NODES]` = 128 bytes
- Total: ≈ 4.5 KB (well within kernel stack budget)

---

## Shell Commands

- `graph hamiltonian` — detect Hamiltonian path/circuit, show traversal order
- `gham` — alias
- `hamiltonian` — alias
- `graph ham` — alias
- `ghamiltonian` — alias
- `ham circuit` — alias
- `hamiltonian path` — alias

**Display:**
- Bright-green (color 10) header and path nodes when circuit found
- Bright-yellow (color 14) path nodes when path-only (no circuit)
- Bright-red (color 12) when no Ham. path found
- Footer shows node count, circuit/path/none status, and `↺ back to start (circuit)` annotation

---

## VectorAddress L4 Namespace

L4=79 for `gos-graph-hamiltonian-harness`

---

## OS Analogy

`graph hamiltonian` = **single-pass maintenance sweep** — the minimum-overhead firmware update or audit procedure that visits every kernel subsystem exactly once.

- **Ham. circuit:** maintenance daemon can start and end at the same base module (like a `systemd` oneshot that traverses all services and returns control to `init`)
- **Ham. path only:** sweep visits all modules but cannot return to the start (like a destructive one-way upgrade chain)
- **No Ham. path:** no sequential single-pass exists — parallel or repeated visits required (like services with mutual exclusive-write dependencies that prevent a linear audit order)

Contrasts with:
- `graph eulerian` (V2.87) — visits every IPC **channel** once (edge-covering sweep)
- `graph dag layers` (V2.89) — **parallel** batch execution levels (not sequential single-pass)
- `graph toposort` — linear ordering that respects dependencies (not guaranteed to exist without Ham. structure)

---

## Literature

- Hamiltonian 1859 — "Icosian game" (find a cycle through all vertices of a dodecahedron)
- Ore 1960 — sufficient condition: deg(u)+deg(v)≥n for all non-adjacent u,v ⇒ Ham. circuit
- Dirac 1952 — deg(v) ≥ n/2 for all v ⇒ Ham. circuit
- Karp 1972 — Hamiltonian path/circuit is NP-complete (reduction from 3-SAT)
- Held & Karp 1962 — O(2ⁿ·n²) DP algorithm (exact for n≤20 with bitmask)
- Comparison: Eulerian (V2.87) — O(V+E) polynomial; Hamiltonian — NP-complete

---

## Test Suite

10 host tests in `gos-graph-hamiltonian-harness`:

| # | Graph | has_path | has_circuit | Notes |
|---|-------|----------|-------------|-------|
| 1 | Empty | false | false | No nodes |
| 2 | Single node | true | true | Trivially Ham. |
| 3 | Two nodes, no edges | false | false | Disconnected |
| 4 | A→B (one-way) | true | false | Path but no return edge |
| 5 | A↔B (bidirectional) | true | true | A→B→A circuit |
| 6 | A→B→C (chain) | true | false | Path only, no back edge |
| 7 | A→B→C→A (triangle) | true | true | Directed 3-cycle |
| 8 | K4 complete directed | true | true | All 12 directed edges |
| 9 | Diamond A→B, A→C, B→D, C→D | false | false | Fork-join blocks Ham. path |
| 10 | Two isolated pairs A↔B, C↔D | false | false | Disconnected → no global path |

All 10 tests pass (`cargo test` in harness directory: 0.01s).
