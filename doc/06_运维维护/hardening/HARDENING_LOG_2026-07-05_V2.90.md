# Hardening Log V2.90 -- Graph Dominator Tree (Cooper et al. 2001)

**Date:** 2026-07-05
**Branch:** feat/vk-auto-live-surface
**Host-test total:** 873 (863 prior + 10 new)

---

## Feature: `graph domtree <start>` / `gdomtree <start>` / `dominator <start>` / `gdom <start>`

### Motivation

V2.88–V2.89 added DAG-specific structural analysis (critical path, topological layers).
V2.90 adds **dominator tree** analysis, which works on **general directed graphs**
(including cyclic graphs) and answers a deeper structural question:

> **"Which node is the mandatory predecessor — with no alternative route — for every other
> node reachable from a given entry?"**

The dominator tree is a foundational data structure in compiler theory and program analysis:

| Question | OS / compiler analogy |
|---|---|
| Which node dominates N? | Which kernel subsystem must be running before this component can start, with no bypass path? |
| What is the immediate dominator? | Which single ancestor is the closest mandatory gateway? |
| Is A the sole entry path to B? | Can the network partition between A and B be exploited (A dominates B)? |

Dominator trees appear in:
- **Compiler CFG analysis** (SSA construction, loop detection, code motion)
- **Security analysis** (control-flow integrity, post-dominator for backward slices)
- **System boot analysis** (which subsystem is the mandatory prerequisite for each service)
- **Network reliability** (which node's failure guarantees unreachability of downstream nodes)

Compared with related algorithms already in the runtime:
| Algorithm | What it finds |
|---|---|
| `graph_articulation` (V2.85) | Nodes whose removal increases connected-component count (undirected) |
| `graph_bridges` (V2.86) | Edges whose removal increases connected-component count |
| `graph_domtree` (V2.90) | For each node, the closest mandatory ancestor from a designated entry (directed) |

---

## Algorithm: Cooper–Harvey–Kennedy 2001 Simple Iterative Dominator

Reference: *"A Simple, Fast Dominance Algorithm"*, Cooper, Harvey & Kennedy, 2001.

### Why this algorithm

The classic Lengauer–Tarjan algorithm (1979) is O(V·α(V)) but requires complex data structures
(semi-dominator computation, link-cut trees for path compression) that are hard to implement
in `no_std` without dynamic allocation.

Cooper et al. 2001 uses the same RPO-based iteration but with a simple array-backed
lattice join. It is O(V² · E) worst-case but converges in 1–2 passes on DAGs and
typical control-flow graphs — well within bounds for MAX_NODES=128.

### Step 1 — Iterative DFS → RPO Order

From `start`, run an iterative DFS using an explicit stack (no recursion, `no_std` safe).
Track post-order as nodes finish. Reverse post-order (RPO) is the traversal order for
the iterative algorithm.

Key: `rpo_num[slot]` stores each node's RPO position (0 = start, which dominates all).
Unreachable nodes retain `rpo_num[slot] = UNDEF`.

### Step 2 — Initialize idom

```
idom[start_slot] = start_slot  // start dominates itself (lattice top)
idom[all others] = UNDEF        // unknown
```

### Step 3 — Iterative Convergence

Process all reachable nodes in RPO order (skip start at position 0).
For each node `b`, compute:

```
new_idom = intersect over all predecessors p of b where idom[p] != UNDEF
```

The `intersect(a, c)` function finds the LCA of `a` and `c` in the current partial
dominator tree by walking both up (toward `start`, decreasing RPO number) until they meet:

```
while a != c:
    while rpo[a] > rpo[c]:  a = idom[a]
    while rpo[c] > rpo[a]:  c = idom[c]
return a  // == c
```

This terminates because RPO numbers strictly decrease as we climb toward `start` (rpo=0).

Repeat until no `idom[b]` changes.

For a DAG: converges in exactly one pass.
For graphs with back edges: typically 2–3 passes.

### Self-loops

Self-loop edges (`from == to`) are skipped in the predecessor scan (`if p == b { continue }`).
A self-loop does not add a new dominator path.

### Unreachable nodes

Nodes not visited by DFS from `start` never appear in `rpo_slots`, so they are never
processed and their `idom` stays `UNDEF`. They are excluded from output.

---

## API

```rust
pub fn graph_domtree<const N: usize>(
    start: VectorAddress,
) -> ([VectorAddress; N], [VectorAddress; N], usize, usize)
```

Returns `(vecs, idoms, node_count, reachable_count)`:
- `vecs[0..reachable_count]`  — reachable nodes in RPO order
- `idoms[0..reachable_count]` — immediate dominator vector for each node
- For `start`: `idoms[i] == vecs[i]` (start dominates itself)
- `node_count`      — total live nodes in the graph
- `reachable_count` — nodes reachable from `start` (including start)

---

## Key Invariants

| Invariant | Notes |
|---|---|
| `idom[start] == start` | Start is the root; it dominates itself |
| `idom[b]` has strictly lower RPO than `b` | Guarantees the LCA walk terminates |
| Unreachable nodes excluded from output | `rpo_num[s] == UNDEF` for unreachable slots |
| Self-loops skipped in predecessor scan | Self-loop does not add a dominator path |
| Diamond `A→{B,C}→D`: `idom[D] == A` | LCA of B and C in the dominator tree is A |
| Back edges handled by iterative convergence | Not just for DAGs — general directed graphs |
| Guard `guard < MAX_NODES * 2` in LCA walk | Prevents pathological loops (defensive) |

---

## Shell Commands

```
graph domtree <v>   dominator tree from entry node <v>
gdomtree <v>        alias
dominator <v>       alias
gdom <v>            alias
```

Display: table of `node` | `immediate dominator`; root node shown in yellow with `← root` marker.

---

## Test Harness: `gos-graph-domtree-harness` (L4=66)

10 tests covering:
1. Empty graph → 0 reachable
2. Single node, start=self → idom = self
3. Start absent from graph → 0 reachable
4. Single edge A→B → idom[B]=A
5. Linear chain A→B→C → idom[B]=A, idom[C]=B
6. Diamond A→{B,C}→D → idom[D]=A (LCA of B and C)
7. Unreachable node excluded from output
8. Back edge cycle A→B→C→B → idom[B]=A, idom[C]=B
9. Isolated start — only itself reachable
10. Merge-then-extend A→{B,C}→D→E → idom[D]=A, idom[E]=D

All 10 pass, zero warnings.

---

## VectorAddress Namespace

L4=66 reserved for `gos-graph-domtree-harness`.

---

## Literature

- Cooper, Harvey & Kennedy 2001 — *"A Simple, Fast Dominance Algorithm"*
- Lengauer & Tarjan 1979 — original O(V·α(V)) algorithm (not used here; too complex for no_std)
- Aho, Lam, Sethi & Ullman 2006 — *Compilers: Principles, Techniques, and Tools* §9.6
- Cytron et al. 1991 — SSA construction uses dominator trees as the foundation
