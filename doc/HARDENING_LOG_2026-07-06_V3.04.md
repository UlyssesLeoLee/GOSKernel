# GOSKernel Hardening Log — V3.04
**Date:** 2026-07-06  
**Algorithm:** Chordal Graph Recognition — LexBFS + PEO Verification  
**Branch:** feat/vk-auto-live-surface  
**Commit:** feat(v3.04): chordal graph recognition -- LexBFS PEO + gos-graph-chordal-harness (10 tests)

---

## Summary

V3.04 adds **chordal graph recognition** to the GOSKernel graph theory runtime.

A graph is **chordal** (also called a *triangulated* graph) if every cycle of length ≥ 4 has a
**chord** — an edge connecting two non-adjacent vertices of the cycle.  Equivalently, a graph is
chordal iff it admits a **Perfect Elimination Ordering (PEO)**: an ordering v₁, v₂, …, vₙ such
that for each vᵢ, the neighbours of vᵢ among {v₁, …, vᵢ₋₁} (already-eliminated vertices) form
a clique.

Chordal graphs are a rich intersection of combinatorics and computer science. They are precisely
the graphs on which many NP-hard graph problems (chromatic number, maximum clique, maximum
independent set, vertex cover) become polynomial-time solvable (via PEO-based algorithms).  They
also characterise Gaussian elimination with no fill-in and appear in probabilistic graphical
models as the class of graphs admitting exact junction-tree inference.

---

## Public API

### `gos_runtime::graph_chordal<const N: usize>() -> ([VectorAddress; N], bool, usize)`

Returns `(peo_vecs, is_chordal, node_count)`:
- `peo_vecs[0..node_count]` — nodes in LexBFS perfect elimination ordering
- `is_chordal` — true iff the graph is chordal (PEO is valid)
- `node_count` — total live nodes

**Undirected projection:** A→B and B→A together count as one undirected edge.  
**Self-loops:** Excluded from the adjacency structure.  
**Edge cases:** Empty graph and graphs with ≤ 2 nodes are trivially chordal.

---

## Algorithm

### Phase 1: LexBFS — Lexicographic Breadth-First Search

Algorithm from Rose, Tarjan & Lueker (1976), O(V+E):

```
label[ci] ← 0  for all compact indices ci ∈ 0..n
for pos in 0..n:
    best_ci ← argmax_{unnumbered ci} label[ci]
    peo[pos] ← best_ci;  pos_of[best_ci] ← pos
    for each unnumbered neighbour nci of best_ci:
        label[nci] |= 1u128 << pos
```

Labels are stored as u128 bitmasks where bit `pos` is set if the node is adjacent to the node
numbered at position `pos`.  Comparing labels by u128 value correctly implements lex-max:
nodes adjacent to more recently numbered neighbours win ties (higher bits = higher priority).

### Phase 2: PEO Verification — Fulkerson & Gross (1965)

For each vertex v at position `pos` in the PEO:
- **N⁺(v)** = neighbours of v numbered **before** v (pos_of < pos)
- **N⁺(v)** must form a clique
- Efficient O(1) per vertex: let **w** = the member of N⁺(v) with the **largest** pos_of
  (most recently numbered).  Then N⁺(v)\{w} ⊆ N(w) is necessary and sufficient.

Key insight: because LexBFS gives a valid PEO for chordal graphs, the PEO check will succeed iff
the graph is chordal.

### Important Correctness Note: PEO Direction

**N⁺(v) = earlier-numbered neighbours**, NOT later-numbered.  In LexBFS, pos=0 is first numbered
(like step n=4 in the original backward-numbering convention).  The PEO invariant is:

> each vᵢ is simplicial in the subgraph induced by {v₁, …, vᵢ} (itself and already-eliminated vertices)

so N⁺(v) = {neighbours with smaller pos_of}, and w = argmax pos_of among N⁺(v).

This is a common source of implementation bugs: the PEO direction is opposite to what intuition
from "forward" orderings might suggest.

---

## Key Invariants and Test Cases

| Graph | is_chordal | Reason |
|-------|-----------|--------|
| Empty graph | true | Vacuous — no cycles |
| Single node | true | No cycles |
| K₂ | true | No 4+ cycle |
| K₃ (triangle) | true | Only 3-cycles; 4+ cycles absent |
| C₄ (chordless 4-cycle) | **false** | 4-cycle has no chord |
| C₄ + chord A–C | true | Chord splits into two triangles |
| K₄ (complete 4) | true | Every pair adjacent — all cycles have chords |
| C₅ (chordless 5-cycle) | **false** | 5-cycle has no chord |
| Path P₅ (tree) | true | Trees have no cycles |
| K₅ (complete 5) | true | Every pair adjacent — trivially chordal |

---

## Shell Commands

```
graph chordal          # Full chordal check + PEO display
gchordal               # Short alias
chordal                # Short alias
graph chord            # Short alias
gchord                 # Short alias
```

**Display:** Header in bright-cyan; `✓ chordal` in bright-green / `✗ not chordal` in bright-red;
PEO table in bright-cyan (chordal) or bright-magenta (non-chordal); footer: node count, verdict,
algorithm citation.

---

## VectorAddress Namespace

- `L4=80` for `gos-graph-chordal-harness`

---

## OS Analogy

A **chordal dependency graph** means the kernel subsystems admit a **perfect elimination order**:
you can bring subsystems online one at a time so that each new subsystem's already-active
dependencies all inter-operate pairwise.  This is like a clean systemd ordering where every
"boot group" is a clique — no hidden circular pre-requisites requiring special-casing.

A **non-chordal** kernel dependency graph contains a "dependency ring" of 4 or more subsystems
where no two non-adjacent members have a direct dependency, making clean isolation harder (you
cannot remove any single subsystem without leaving dangling dependencies in a non-clique set).

Chordal graphs also correspond to kernel structures that can be decomposed with **zero fill-in**
via Gaussian elimination — directly applicable to sparse kernel scheduling problems (like systemd
unit ordering with sparse dependency matrices).

---

## Harness: `gos-graph-chordal-harness` (10 tests)

| # | Graph | Expected |
|---|-------|----------|
| 1 | Empty | is_chordal=true, node_count=0 |
| 2 | Single node A | is_chordal=true, PEO=[A] |
| 3 | K₂ (A–B bidirectional) | is_chordal=true |
| 4 | K₃ (triangle) | is_chordal=true |
| 5 | C₄ (4-cycle, no chord) | is_chordal=false |
| 6 | C₄ + chord A–C | is_chordal=true |
| 7 | K₄ (complete 4 nodes) | is_chordal=true |
| 8 | C₅ (5-cycle, no chord) | is_chordal=false |
| 9 | Path P₅ (A–B–C–D–E) | is_chordal=true |
| 10 | K₅ (complete 5 nodes) | is_chordal=true |

All 10 tests green.  Host-test suite total: **1013 tests**.

---

## Complementary Algorithms in GOSKernel

| Algorithm | Version | Relationship |
|-----------|---------|-------------|
| graph_clique (BK) | V2.95 | In chordal graphs, ω(G) = max PEO clique-size (polynomial) |
| graph_independent_set | V2.96 | In chordal graphs, α(G) = n − ν(G) (polynomial via König) |
| graph_vertex_cover | V2.97 | In chordal graphs, τ(G) = ν(G) exactly (König, polynomial) |
| graph_color | V2.37 | In chordal graphs, χ(G) = ω(G) (perfect graph theorem, polynomial) |
| graph_kcore | V2.64 | k-core decomposition (related to clique structure) |
| graph_truss | V2.94 | k-truss decomposition (triangle-based density) |

---

## Literature

- Rose, Tarjan & Lueker (1976) — "Algorithmic Aspects of Vertex Elimination on Graphs" (LexBFS, O(V+E) recognition)
- Fulkerson & Gross (1965) — "Incidence matrices and interval graphs" (PEO characterisation)
- Gavril (1974) — "The intersection graphs of subtrees of a tree are exactly the chordal graphs"
- Golumbic (1980) — "Algorithmic Graph Theory and Perfect Graphs" (chap. 4, comprehensive treatment)
- Blair & Peyton (1993) — "Introduction to chordal graphs and clique trees" (Gaussian elimination connection)
