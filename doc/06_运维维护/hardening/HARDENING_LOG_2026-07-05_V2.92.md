# Hardening Log V2.92 -- Maximum Bipartite Matching (Kuhn's Algorithm)

**Date:** 2026-07-05
**Branch:** feat/vk-auto-live-surface
**Host-test total:** 893 (883 prior + 10 new)

---

## Feature: `graph bipartite match` / `gbimatch` / `bipartite match`

### Motivation

V2.37 added `graph_bipartite` — the membership check (is this graph bipartite? which
set does each node belong to?). V2.92 extends this to the matching problem:

> **"Given a bipartite graph, what is the maximum set of disjoint A–B pairings?"**

A **maximum bipartite matching** answers optimal assignment problems that arise
throughout OS scheduling and resource allocation:

| Question | OS analogy |
|---|---|
| Which tasks can bind to which CPUs? | `taskset` / `numactl --cpunodebind` affinity graph |
| How many tasks can run concurrently without sharing a CPU? | Maximum independent scheduling slots |
| Can every service be assigned a unique network interface? | NIC↔service exclusive binding |
| Which IRQ handlers can be pinned to distinct CPU cores? | `/proc/irq/N/smp_affinity` assignment |

Compared with related algorithms already in the runtime:

| Algorithm | What it finds |
|---|---|
| `graph_bipartite` (V2.37) | Is the graph bipartite? Which side is each node on? |
| `graph_community` (V2.44) | Label propagation community assignment (non-bipartite) |
| `graph_spanning` (V2.52) | Spanning subgraph (all nodes, subset of edges) |
| `graph_bipartite_match` (V2.92) | **Maximum matching: largest set of disjoint A↔B pairs** |

The matching count also gives an upper bound for König's theorem:
in a bipartite graph, maximum matching = minimum vertex cover.

---

## Algorithm: Kuhn's Augmenting-Path DFS

### Why augmenting paths

A matching M is maximum if and only if there is no **augmenting path** — a path
starting and ending at free (unmatched) nodes that alternates between non-matching
and matching edges. Flipping edges along an augmenting path increases |M| by 1.

Kuhn's algorithm (also known as the Hungarian DFS method) repeats this for each
free side-A node:

```
for each free A-node a:
    DFS to find alternating path from a to a free B-node
    if found: augment (flip all edges along path), match_count += 1
```

Each DFS is O(E). With O(V) free A-nodes, total complexity is **O(V·E)**.
For our capacity constraints (V ≤ 128, E ≤ 512), this is at most 65,536 operations —
well within real-time budget.

### Iterative DFS with explicit path tracking

Recursion is not safe in `no_std` kernel code. The DFS uses:

```
dfs_stk:  [(a_slot, ei); MAX_NODES]   — explicit DFS stack
chosen_b: [b_slot; MAX_NODES]         — B-node chosen at each DFS level
visited_b: [bool; MAX_NODES]          — B-nodes tried in this DFS (per free A-node)
```

**Stack advance (found matched B-node):**
1. Mark b_slot visited; record `chosen_b[level] = b_slot`
2. Save edge scan position `dfs_stk[level].1 = ei` (for backtrack resume)
3. Push matched A-node `match_b[b_slot]` at level+1

**Augmentation (found free B-node):**
1. Record `chosen_b[level] = free_b_slot`
2. Walk from current level down to 0:
   - `match_a[a] = cur_b; match_b[cur_b] = a`
   - Advance: `cur_b = chosen_b[level-1]`
3. Increment match_count; break DFS

**Backtrack (no viable path from this A-node):**
- `st_top -= 1` (pop DFS frame)
- Outer while loop re-reads saved `ei` from `dfs_stk[lvl].1`, continues scanning

### Bipartition (step 1)

The bipartite 2-colouring uses the same BFS technique as `graph_bipartite` (V2.37):
edges are treated as undirected; BFS assigns alternating colours 0 (side A) and 1 (side B).
If any node has the same colour as its BFS parent, the graph is not bipartite and
`match_count = 0, is_bipartite = false` is returned immediately.

### visited_b semantics

`visited_b` is reset for each new free A-node's DFS. Within a single DFS, each
B-node is tried at most once — this prevents cycles in the alternating-path search
and ensures the DFS terminates in O(E) steps.

---

## API

```rust
pub fn graph_bipartite_match<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], usize, bool, usize)
```

Returns `(left_vecs, right_vecs, match_count, is_bipartite, node_count)`:
- `left_vecs[0..match_count]`  — matched side-A (color 0) nodes
- `right_vecs[0..match_count]` — matched side-B (color 1) nodes; `left_vecs[i]` matches `right_vecs[i]`
- `match_count`                — maximum matching size; 0 if not bipartite
- `is_bipartite`               — false if an odd-length cycle was detected
- `node_count`                 — total live nodes

Output sorted by node_slot order (ascending VectorAddress.as_u64() within side A).
N should be `MAX_NODES` (128) for full coverage.

---

## Key Invariants

| Invariant | Notes |
|---|---|
| `is_bipartite` consistent with `graph_bipartite` | Both use same BFS 2-colouring; test 10 cross-checks |
| `match_count ≤ min(\|A\|, \|B\|)` | Matching is a set of disjoint pairs |
| Left nodes in output pairwise distinct | Each A-node appears at most once |
| Right nodes in output pairwise distinct | Each B-node appears at most once |
| `match_count = 0` when `!is_bipartite` | Not bipartite → skip matching entirely |
| Self-loops have no effect | BFS coloring skips self-loops (would assign same colour) |
| Augmenting path correctness | `chosen_b[k]` records B chosen at level k; augmentation walks 0..lvl |
| Backtrack resume | `dfs_stk[lvl].1` saves `ei` so scan continues after child backtracks |

---

## Shell Commands

```
graph bipartite match   maximum bipartite matching
gbimatch                alias
bipartite match         alias
```

Display:
- Header: `graph bipartite match`
- If not bipartite: red "NOT bipartite (odd-length cycle detected)" + hint
- If bipartite, no edges: green "bipartite graph with no edges (empty matching)"
- Otherwise: table of `side A ↔ side B` pairs in bright yellow
- Footer: matching size + node count

---

## Test Harness: `gos-graph-bimatch-harness` (L4=68)

10 tests covering:

| # | Scenario | Expected |
|---|---|---|
| 1 | Empty graph | match_count=0, is_bipartite=true |
| 2 | Single isolated node | match_count=0, is_bipartite=true |
| 3 | Triangle (odd cycle, not bipartite) | is_bipartite=false, match_count=0 |
| 4 | Single A–B edge | match_count=1, pair (A0,B0) |
| 5 | Path chain A0–B0–A1 | match_count=1 (B0 shared, can only match one) |
| 6 | K_{2,2} complete bipartite | match_count=2 (perfect matching) |
| 7 | K_{2,3}: 2 left, 3 right | match_count=2 (bounded by smaller side) |
| 8 | Augmenting path swap needed | match_count=2 (requires DFS to push and augment) |
| 9 | Two disconnected bipartite components | match_count=2 (1 per component) |
| 10 | K_{3,3}: invariant cross-checks | match_count=3; is_bipartite consistent with graph_bipartite; all output pairs vertex-disjoint |

All 10 pass, zero warnings.

---

## VectorAddress Namespace

L4=68 reserved for `gos-graph-bimatch-harness`.

---

## Literature

- Kuhn 1955 — *"The Hungarian method for the assignment problem"* (augmenting path foundation)
- Hopcroft & Karp 1973 — O(E√V) improvement (Kuhn used here for simplicity at V≤128)
- König 1931 — König's theorem: in bipartite graphs, max matching = min vertex cover
- Hall 1935 — Hall's marriage theorem (matching existence condition)
- Cormen, Leiserson, Rivest & Stein — *Introduction to Algorithms* §26.3 (bipartite matching via max flow)
