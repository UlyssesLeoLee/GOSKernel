---
name: gos-truss-cascade-peeling-pattern
description: When implementing k-truss decomposition in GOSKernel, the cascade peeling makes ALL edges of two triangles sharing one edge get the same trussness (not the shared edge getting higher). Also: max_truss = max_core + 1 for cliques (K4: truss=4, core=3) — use as cross-check invariant.
---

# K-Truss Peeling: Cascade and Truss-vs-Core Invariant

## The rule

**Cascade peeling — shared-edge scenario:**

Two triangles ABC and ABD share edge A-B. Initial supports:
- A-B: support=2 (triangles with C and D)
- A-C, B-C, A-D, B-D: support=1 (one triangle each)

At k=4 (threshold=2), A-C, B-C, A-D, B-D all have support=1 < 2 → removed.
Each removal decrements A-B's support (once per triangle). After all four outer edges
are removed, A-B has support=0. It then also falls below threshold=2 and is removed
at the same k=4 round. Result: **all five edges get trussness=3**, not 4.

This is correct: the k-truss is a COHESIVE subgraph property. After the outer edges
leave the subgraph, A-B has no triangles remaining within the subgraph → can't be in
the 4-truss. The 4-truss requires A-B to be in ≥2 triangles *simultaneously surviving*.

**K4 truss-vs-core invariant (useful test cross-check):**

For K4 (complete graph on 4 nodes):
- k-core max_coreness = 3 (every node has degree 3 = max_core)
- k-truss max_trussness = 4 (every edge is in 2 triangles; survives k=4, removed at k=5)
- Relationship: **max_truss = max_core + 1 = 4**

General rule (not just K4): for a graph with degeneracy d, max_truss ≤ d + 1.
Equality holds for cliques Kₙ (n≥3): max_truss(Kₙ) = n-1 = max_core(Kₙ) + 1.

## Why it's non-obvious

Intuition says: "A-B is in 2 triangles, so it should have higher trussness than the
outer edges which are each in only 1 triangle." But that's wrong because the k-truss
is not evaluated on the full graph — it's evaluated on the REMAINING subgraph after
removing lower-trussness edges. Once A-C, B-C, A-D, B-D are gone, A-B has zero
triangles remaining and must be removed at the same level.

## GOSKernel context

- `crates/gos-runtime/src/lib.rs` — `graph_truss_inner<const N: usize>()`
- Returns `([VectorAddress; N], [u8; N], usize, u8)` = (vecs, trussness, n, max_truss)
- Shell: "graph truss" / "gtruss" / "truss" / "k-truss" / "ktruss"
- VectorAddress L4=70 for gos-graph-truss-harness test nodes
- Complement to graph_kcore (V2.64, L4=40): kcore is node-level, ktruss is edge-level

## From this session

V2.94: test_06 verifies that two triangles sharing one edge give max_trussness=3 (not 4).
test_10 cross-checks: K4 max_truss=4, max_core=3, and asserts max_truss == max_core + 1.
Both tests confirmed the cascade behavior and the invariant are correctly implemented.
