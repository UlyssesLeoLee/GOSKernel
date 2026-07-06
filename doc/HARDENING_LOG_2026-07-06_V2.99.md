# GOS Hardening Log — V2.99 (2026-07-06)

## Feature: Minimum Path Cover in DAG (König / Dilworth)

### Summary

Added `graph_min_path_cover<N>()` to gos-runtime — minimum path cover (MPC) of a
directed acyclic graph.  A path cover is a set of vertex-disjoint directed paths that
collectively visit every node; the minimum such cover has MPC = n − ν paths.

**Key theorem (König 1931 / Dilworth 1950)**:
```
MPC(G) = n − ν(B(G))
```
where B(G) is the bipartite expansion (left_u → right_v for each directed edge u→v
in G) and ν is the maximum matching of B(G).

This is an exact algorithm for DAGs, polynomial O(V·E), with no approximation gap.

### OS Analogy

Minimum sequential upgrade chains: the fewest ordered installation sequences needed
to apply a kernel patch across all modules in a dependency DAG, where each sequence
must follow directed dependency edges.  Equivalent to `make -j<MPC>` job allocation
where each job is one linear dependency chain.

Complements the existing DAG analysis suite:
- DAG longest path / critical chain (V2.88): serial depth lower bound
- DAG topological layers (V2.89): parallel-execution level assignment (width)
- DAG dominator tree (V2.90): mandatory boot-order predecessors
- DAG feedback arc set (V2.91): edges causing circular dependencies
- MPC (V2.99): minimum number of linear upgrade/deployment sequences (König depth)

### Implementation

**crates/gos-runtime/src/lib.rs**
- New method: `GraphRuntime::graph_min_path_cover_inner<N>()`
  - Phase 1: compact live nodes; build `slot_to_ci[]` mapping.
  - Phase 2: Kahn's BFS — verify DAG; record topological order in `topo_order[]`.
    Self-loops keep `in_deg > 0` so Kahn never drains them → `is_dag = false`.
  - Phase 3: build bipartite expansion adjacency as u128 bitmask per compact index:
    `right_adj[u_ci] |= 1u128 << v_ci` for each directed edge u→v.
  - Phase 4: Kuhn's augmenting-path matching (iterative DFS, O(V·E)):
    - Per-source DFS state: `dfs_lci[level]` (left node), `dfs_rem[level]` (u128
      remaining candidates), `chosen_r[level]` (selected right node per level).
    - Globally-visited set `visited_r` (u128) prevents DFS revisits within one call.
    - Free right node found → augment matching bottom-up via `chosen_r[]` trail.
    - Matched right node → push its current left partner for DFS continuation.
    - Left nodes processed in topological order for natural path structure.
  - Phase 5: `path_count = nc − match_count` (König / Dilworth equality).
  - Phase 6: reconstruct paths by following `match_l[]` successor chains.
    Path starts = nodes where `match_r[ci] == NIL` (no in-matching predecessor).
    Enumerate starts in topological order → path IDs assigned top-down.
- New public function:
  `graph_min_path_cover<N>() -> ([VectorAddress; N], [u8; N], usize, bool, usize)`
  - Returns `(path_vecs, path_ids, path_count, is_dag, node_count)`
  - `path_vecs[0..node_count]` — all live nodes in path-then-topo order.
  - `path_ids[0..node_count]` — 0-indexed path ID per node (same ID = same path).
  - `path_count` — minimum number of vertex-disjoint paths (n − ν).
  - `is_dag` — false if directed cycle detected (MPC undefined for cyclic graphs).
  - `node_count` — total live nodes.

**crates/k-shell/src/lib.rs**
- New `dispatch_graph_min_path_cover()`:
  - Bright-yellow header (color 14); error in bright-red (12) if not a DAG.
  - Per-path color cycling through 6 bright colors: green(10), cyan(11),
    magenta(13), blue(9), yellow(14), red(12).
  - Dashed separator line (┈) between consecutive paths for readability.
  - Footer: `N node(s)  MPC=K  (n−ν=N−M)  König/Dilworth`

**crates/k-shell/src/proc.rs**
- Routing added after "graph domset":
  `"graph mpc" || "gmpc" || "min path cover" || "graph min path cover" || "path cover" || "gdagcover" || "graph path cover"`

**host-tests/gos-graph-mpc-harness/**
- VectorAddress L4=75
- 10 tests, all pass (0 warnings):
  1. Empty graph → MPC=0, is_dag=true, nc=0.
  2. Single node → MPC=1 (singleton path), is_dag=true.
  3. Single directed edge A→B → MPC=1 (path [A,B]), vecs order verified.
  4. Two isolated nodes → MPC=2 (two singletons), different path IDs verified.
  5. Chain A→B→C→D → MPC=1 (Hamiltonian); vecs order [A,B,C,D] verified.
  6. Diamond A→{B,C}→D → MPC=2 (D_R contested; ν=2, n=4).
  7. Parallel chains A→B, C→D → MPC=2 (two independent chains; ν=2, n=4).
  8. K_3 DAG (A→B, A→C, B→C) → MPC=1 (Hamiltonian A→B→C); vecs order verified.
  9. Directed cycle A→B→C→A → is_dag=false, MPC=0 (undefined).
  10. Star DAG A→{B,C,D,E} → MPC=4; Dilworth cross-check: MPC+ν=n (4+1=5) ✓;
      A's path length=2 (A + one matched leaf); 3 singleton leaf paths verified.

### Key Invariants

- `is_dag = false` iff Kahn's BFS drains fewer than `nc` nodes (cycle or self-loop).
- `path_count + match_count == node_count` at all times (König/Dilworth equality).
- `match_l[]` successor chains are acyclic because B(G) edges follow G's DAG edges.
- `visited_r` is monotone within one DFS call (never un-visited on backtrack) —
  this is standard Kuhn behaviour to prevent infinite DFS loops.
- Stack depth bounded by `nc ≤ MAX_NODES = 128`; no heap allocation.
- u128 bitmask adjacency: `right_adj[u_ci] |= 1u128 << v_ci` — safe for nc ≤ 128.

### Test Cross-References

- Test 5 (chain) validates the Hamiltonian case: ν = n−1, MPC = 1.
- Test 10 (star) validates the antichain case: ν = 1 (one center→leaf pair), MPC = n−1.
- Test 9 (cycle) validates early return on non-DAG: `processed < nc` ↔ cycle.
- Test 6 (diamond) validates contested right nodes: D_R has two left-side competitors,
  matching picks exactly one, so path_count = n − 2 = 2.

### Literature

- König, D. (1931). Graphen und Matrizen. Matematikai Lapok 38: 116–119.
- Dilworth, R. P. (1950). A decomposition theorem for partially ordered sets.
  Annals of Mathematics. 51(1): 161–166.
- Kuhn, H. W. (1955). The Hungarian method for the assignment problem.
  Naval Research Logistics Quarterly. 2(1–2): 83–97.
- Kahn, A. B. (1962). Topological sorting of large networks. CACM 5(11): 558–562.
