# GOS Hardening Log — V3.00 (2026-07-06)

## Feature: Minimum Spanning Arborescence (Chu-Liu / Edmonds 1967)

### Summary

Added `graph_arborescence<N>(root: VectorAddress)` to gos-runtime — minimum spanning
arborescence (directed MST) from a root node using the Chu-Liu / Edmonds algorithm.

An arborescence rooted at r is a directed spanning tree where every non-root node v
has exactly one directed path from r to v.  The *minimum* arborescence minimises the
total weight of its edges.

**Key theorem (Edmonds 1967)**:
A minimum spanning arborescence always exists if and only if every non-root node is
reachable from the root.  Chu-Liu/Edmonds finds it in O(V·E) via cycle contraction.

### OS Analogy

Minimum-weight directed boot dependency tree: given a weighted service dependency
graph (where edge weight = start latency ms), the arborescence from `init` gives the
optimal single-parent dependency assignment for every service — the minimum total
startup overhead.  Equivalent to choosing which predecessor each kernel module should
wait on, when multiple valid predecessors exist.

Complements the directed-graph analysis suite:
- Directed shortest paths (V2.xx): minimum latency from root to each node
- Dominator tree (V2.90): mandatory predecessors (graph-theoretic must-reach)
- MST (undirected): minimum spanning tree for undirected topology
- MSA (V3.00): minimum spanning arborescence for directed topology

### Algorithm: Chu-Liu / Edmonds Cycle Contraction (O(V·E))

**Initialisation:**
- Compact live nodes into a `slot_to_ci[]` array (nc total compact indices).
- Identify `root_ci` by matching `node_slot_by_vec(root)`.
- Build `e_from[]`, `e_to[]`, `e_wt[]`, `e_adj[]` edge arrays (all float weights;
  `e_adj[ei]` tracks the weight adjustment from prior cycle contractions).
- `group[ci]` maps each compact index to its current super-node ID (initially ci).
- `num_sg = nc` — initial number of super-nodes.

**Iterative rounds (`for _round in 0..nc`):**

*Step A — select minimum incoming edge per non-root super-node:*
```
for each super-node sg != root_sg:
    find edge ei with e_from[ei] in different super-node, e_adj[ei] minimum
    in_src[sg] = sg of e_from[ei]
    in_wt[sg]  = e_adj[ei]
    in_ei[sg]  = ei
    sel_parent[ci] = e_from[ei_mapped_back]  ← updated for root members of sg
    sel_wt[ci]     = e_adj[in_ei[sg]] × 1000 (milliweight u32)
```
If any super-node has no incoming edge, arborescence is impossible (`is_connected=false`).

*Step B — cycle detection via DFS (colours: 0=white, 1=gray, 2=black):*
```
for each non-root super-node sg (starting gray, follow in_src[]):
    gray → cycle_sg found when revisiting a gray node
```
If no cycle found → arborescence is complete; break.

*Step C — trace the cycle:*
```
walk in_src[] from cycle_sg back to cycle_sg → collect cycle_nodes[]
```

*Step D — assign new super-node `new_sg = num_sg++` to cycle members:*
```
for each ci where group[ci] is in cycle:
    group[ci] = new_sg
```

*Step E — adjust weights for edges entering the cycle from outside:*
```
for each edge ei with e_to in cycle and e_from outside cycle:
    t_sg = original super-node of e_to[ei]
    e_adj[ei] -= in_wt[t_sg]
```
This encodes the savings from displacing the currently-selected cycle edge,
so the net weight to "break the cycle by entering at this node" is correct.

**Convergence:** each round either terminates (no cycle) or contracts one cycle,
reducing `num_sg` by at least 1.  Maximum `nc` rounds → guaranteed termination.

**Output:** `sel_parent[]` and `sel_wt[]` accumulate the arborescence structure
across rounds; total weight = Σ sel_wt[i] / 1000 (reconstructed from milliweights).

### Why Chu-Liu/Edmonds Beats Naive Greedy

In a directed graph, independently selecting the minimum-weight incoming edge per
node can select edges that form a cycle — violating the tree property.  For example:

```
A(root) → B (w=5)
A(root) → C (w=3)
B → C (w=1)
C → B (w=1)
```

Naive: picks B←C(1) and C←B(1) — cycle!  Falls back to B←A(5) + C←A(3) = total 8.
Edmonds: contracts {B,C} cycle, adjusts A→B effective weight to 5−1=4 and
A→C effective weight to 3−1=2; picks C as cycle entry point (min=2); expands →
C←A(3), B←C(1); total = **4** (vs. naive 8 — 50% savings).

Test 06 in the harness explicitly verifies this optimality gap.

### Implementation

**crates/gos-runtime/src/lib.rs**
- New method: `GraphRuntime::graph_arborescence_inner<const N: usize>(root: VectorAddress)`
  - Fixed-size stack arrays: `node_slots[N]`, `e_from/e_to/e_wt/e_adj[MAX_EDGES]`
    (MAX_EDGES=512), `group[MAX_SG]`, `in_src/in_wt/in_ei[MAX_SG]`,
    `sel_parent[N]`, `sel_wt[N]`, DFS state arrays — zero heap allocation.
  - MAX_SG=256 (≥ 2×MAX_NODES=128); each cycle contraction allocates one new super-node.
  - `node_slot_by_vec(root)` used to locate root; falls back to `node_slots[0]` for
    empty root (empty graph case).
  - Returns `([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32, bool)`:
    `(vecs, parents, weights_milli, nc, total_milli, is_connected)`.
    - `vecs[0..nc]`   — live nodes; root is always at index 0.
    - `parents[0..nc]` — arborescence parent (self for root).
    - `weights_milli[0..nc]` — incoming edge weight × 1000 (root=0).
    - `nc`           — number of live nodes.
    - `total_milli`  — total arborescence weight × 1000 (0 if not connected).
    - `is_connected` — false iff any non-root super-node has no incoming edge.
- New public function:
  `graph_arborescence<const N: usize>(root: VectorAddress)`
  — thin lock wrapper calling `RUNTIME.lock().graph_arborescence_inner(root)`.

**crates/k-shell/src/lib.rs**
- New `dispatch_graph_arborescence(sink, root: VectorAddress)`:
  - Bright-cyan header (color 11): `"=== Minimum Spanning Arborescence (Chu-Liu/Edmonds) ==="`
  - Handles: empty graph (nc=0), not connected (no arborescence), normal display.
  - Per-node table: role (Root / Child), weight, VectorAddress, parent VectorAddress.
  - Footer: `"N node(s)  MSA-weight=W.WWW  (Chu-Liu/Edmonds)"`

**crates/k-shell/src/proc.rs**
- Routing added after `"graph mpc"` branch:
  - Commands: `"graph arborescence <vec>"`, `"garborescence <vec>"`,
    `"arborescence <vec>"`, `"gmsa <vec>"`, `"min arborescence <vec>"`
  - Parses trailing `VectorAddress` from the command suffix.
  - Error message on parse failure: `"arborescence: invalid VectorAddress '<str>'"`

**host-tests/gos-graph-arborescence-harness/** (L4=76, VectorAddress namespace)
- VectorAddress L4=76
- 10 tests, all pass (0 warnings):
  1. Empty graph → nc=0, total_w=0, is_connected=true (vacuously).
  2. Single node → nc=1, parent=self, total_w=0.
  3. Single directed edge A→B, root=A → B parent=A, weight=1000, total=1000.
  4. Chain A→B→C→D, root=A → B←A, C←B, D←C, total=3000; all parent links verified.
  5. Directed 3-cycle A→B→C→A, root=A → A reachable to all; total=2000 (back-edge unused).
  6. Cycle contraction: A→B(5), A→C(3), B→C(1), C→B(1); Edmonds=4 vs. greedy=8.
  7. Disconnected: D has no incoming edges → is_connected=false, total_w=0.
  8. Complete directed triangle K3 (all 6 edges), root=A → 2 optimal non-root edges, total=2000.
  9. Star out-tree A→{B,C,D,E} → trivial arborescence, 4 unit edges, total=4000.
  10. Diamond+tail A→B(1),A→C(3),B→D(1),C→D(2),D→E(1): D picks B not C; total=6000;
      exactly nc-1=4 non-root edges; root count=1.

### Key Invariants

- `nc − 1` non-root edges (self-parent count = 1) in every connected arborescence.
- `total_milli = 0` and `is_connected = false` when any non-root SG has no incoming edge.
- Empty graph: `nc = 0`, `is_connected = true` (vacuously spanning), `total_milli = 0`.
- Each cycle contraction creates exactly one new super-node (`num_sg += 1`) and may
  never exceed `MAX_SG = 256`.
- `group[]` is monotonically-reassigned; `sel_parent[]` / `sel_wt[]` are updated
  only for the break-point node when a cycle is expanded.
- Stack depth bounded by `nc ≤ MAX_NODES = 128`; `num_sg ≤ MAX_SG = 256`.

### Host-Test Totals

| Milestone | Tests |
|---|---|
| V2.93 | 903 |
| V2.94–V2.99 (+6 × 10) | 963 |
| **V3.00 (+10)** | **973** |

### Literature

- Chu, Y. J.; Liu, T. H. (1965). On the Shortest Arborescence of a Directed Graph.
  *Science Sinica* 14: 1396–1400.
- Edmonds, J. (1967). Optimum branchings. *Journal of Research of the National Bureau
  of Standards B* 71(4): 233–240.
- Tarjan, R. E. (1977). Finding optimum branchings. *Networks* 7(1): 25–35.
  (Improved O(E log V) implementation using Fibonacci heaps; GOSKernel uses O(V·E).)
