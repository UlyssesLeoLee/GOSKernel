# Hardening Log V2.91 -- Feedback Arc Set (DFS 3-Colouring)

**Date:** 2026-07-05
**Branch:** feat/vk-auto-live-surface
**Host-test total:** 883 (873 prior + 10 new)

---

## Feature: `graph feedback arc` / `gfas` / `feedback arc` / `gcycledges`

### Motivation

V2.88–V2.90 built the DAG analysis chain: critical path (V2.88), topological layers (V2.89),
and dominator tree (V2.90). V2.91 closes the cycle-analysis story by directly answering:

> **"Which specific directed edges are responsible for the cycles in this graph?"**

A **feedback arc set (FAS)** is a set of edges whose removal makes the graph acyclic.
It is the directed-graph counterpart of back-edges in DFS.

| Question | OS analogy |
|---|---|
| Are there cycles? | Does the dependency graph have circular boot requirements? |
| Which edges cause them? | Exactly which `requires`/`after` links create the deadlock? |
| How many must be removed? | Minimum structural changes to enable topological boot order |

Compared with related algorithms already in the runtime:

| Algorithm | What it finds |
|---|---|
| `graph_girth` (V2.69) | Length of the shortest directed cycle |
| `graph_dag_longest` (V2.88) | Returns `is_dag=false` if any cycle exists |
| `graph_dag_layers` (V2.89) | Returns `is_dag=false` if any cycle exists |
| `graph_domtree` (V2.90) | Immediate dominators from a single entry point |
| `graph_feedback_arc` (V2.91) | **The actual edges causing cycles** |

The FAS is the actionable output: it names exactly which dependencies must be broken
(or reversed) to restore a clean boot order.

---

## Algorithm: Iterative DFS 3-Colouring

### Why DFS back-edges

The minimum-FAS problem is NP-hard for general directed graphs (Karp 1972).
However, the DFS-based FAS is a standard O(V+E) approximation:
- It is always a valid FAS (removing the returned arcs makes the graph acyclic)
- In practice it is tight (often optimal or near-optimal for sparse graphs)
- It is deterministic given a fixed edge-scan order

### 3-Colour DFS

Classic DFS assigns each node one of three colours:

| Colour | Meaning | Array value |
|---|---|---|
| `UNVISITED` (white) | Not yet seen | 0 |
| `IN_STACK` (gray) | On the current DFS call stack | 1 |
| `DONE` (black) | Fully processed; all successors explored | 2 |

Edge classification by destination colour:

| Destination colour | Edge type | Feedback arc? |
|---|---|---|
| `UNVISITED` | Tree edge | No — DFS descends |
| `IN_STACK` | **Back-edge** | **Yes — closes a cycle** |
| `DONE` | Forward/cross edge | No — already processed |

### Self-Loops

A self-loop `(u→u)` is naturally caught: when processing node `u`, `u` is `IN_STACK`.
The self-edge's destination slot equals `u`'s slot, colour = `IN_STACK` → recorded as a
feedback arc. No special case needed.

### Iterative Implementation

Recursion is not safe in `no_std` kernel code. The DFS uses an explicit stack:

```
dfs_stack: [(slot, next_edge_index); MAX_NODES]
```

Each frame stores `(current_slot, ei)` — the next edge index to scan from `current_slot`.
When a tree edge is found:
1. Save `ei + 1` as the resume point for the current frame
2. Push the neighbour slot with `ei = 0`

When no unvisited neighbour remains:
1. Set `color[current_slot] = DONE`
2. Pop the frame

### Disconnected Graphs

The outer loop iterates over all `node_slots[ki]` in compact order. If any node is
`UNVISITED` when reached by the outer loop, a new DFS tree is started from it.
This ensures every connected component is processed.

---

## API

```rust
pub fn graph_feedback_arc<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], usize, usize)
```

Returns `(from_vecs, to_vecs, arc_count, node_count)`:
- `from_vecs[0..arc_count]` / `to_vecs[0..arc_count]` — feedback arcs
- `arc_count`  — number of back-edges found
- `node_count` — total live nodes

Output sorted ascending by `(from.as_u64(), to.as_u64())` for determinism.

N should be `MAX_EDGES` (512) for full coverage; k-shell uses `MAX_EDGES`.

---

## Key Invariants

| Invariant | Notes |
|---|---|
| Self-loops recorded as feedback arcs | Colour of `cur_slot` is always `IN_STACK` when self-edge fires |
| Cross/forward edges (DONE) not recorded | Only back-edges (IN_STACK) are arcs |
| Removing all returned arcs yields a DAG | Proof: no `IN_STACK` edge remains → DFS finds no back-edges |
| `arc_count == 0` iff `is_dag` | Cross-checked in test 10 with `graph_dag_layers` |
| Output sorted by `(from.as_u64(), to.as_u64())` | Deterministic across runs |
| No parent-tracking needed | Unlike articulation/bridges, FAS needs only 3-colour state |

---

## Shell Commands

```
graph feedback arc   list all feedback arcs (directed back-edges)
gfas                 alias
feedback arc         alias
gcycledges           alias
```

Display:
- Header: `graph feedback arc`
- If `arc_count == 0`: green "no feedback arcs (graph is a DAG)"
- Otherwise: table of `from → to` pairs in red
- Footer: arc count, node count, dag status (`acyclic` or `cyclic (N arcs to remove)`)

---

## Test Harness: `gos-graph-fas-harness` (L4=67)

10 tests covering:

| # | Scenario | Expected |
|---|---|---|
| 1 | Empty graph | arc_count=0, node_count=0 |
| 2 | Single node, no edges | arc_count=0 |
| 3 | Self-loop A→A | arc_count=1, arc=(A,A) |
| 4 | 2-cycle A→B→A | arc_count=1 |
| 5 | DAG chain A→B→C | arc_count=0 |
| 6 | Diamond DAG A→{B,C}→D | arc_count=0 |
| 7 | Triangle A→B→C→A | arc_count=1, back-edge is C→A |
| 8 | Two independent 2-cycles | arc_count=2 |
| 9 | Disconnected: DAG + 2-cycle | arc_count=1 (cycle component only) |
| 10 | Cross-check with `graph_dag_layers` | arc_count==0 iff is_dag==true |

All 10 pass, zero warnings.

---

## VectorAddress Namespace

L4=67 reserved for `gos-graph-fas-harness`.

---

## Literature

- Karp 1972 — NP-completeness of minimum feedback arc set (MFAS in tournaments)
- Cormen, Leiserson, Rivest & Stein — *Introduction to Algorithms* §22.3 (DFS back-edges)
- Eades, Lin & Smyth 1993 — greedy FAS heuristic for layered graph drawing
- Tarjan 1976 — *"Edge-disjoint spanning trees and depth-first search"*
