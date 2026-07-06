# GOSKernel Hardening Log — V2.97

**Date:** 2026-07-05  
**Branch:** feat/vk-auto-live-surface  
**Commit:** 863434e  
**Total host tests after this session:** 943 (933 + 10 new)

---

## V2.97 — Minimum Vertex Cover (König exact + greedy 2-approx)

### Feature Summary

Added `gos_runtime::graph_vertex_cover<N>()` — the minimum vertex cover τ(G) of the live kernel graph.

A **vertex cover** is a set of nodes T such that every edge (u,v) has at least one endpoint in T. The minimum vertex cover (min-VC) is the smallest such set.

**Two-path algorithm:**
- **Bipartite graphs:** Exact minimum vertex cover via König's theorem. Runs BFS 2-colouring + Kuhn's augmenting-path matching + alternating-path BFS (König construction). Returns τ(G) = ν(G) exactly.
- **General (non-bipartite) graphs:** 2-approximation via greedy maximal matching. For each uncovered edge (u,v), adds both endpoints to the cover. Guaranteed: cover_size ≤ 2 × τ(G).

### API

```rust
pub fn graph_vertex_cover<N: usize>() -> ([VectorAddress; N], usize, bool, usize)
// Returns: (cover_vecs, cover_size, is_exact, node_count)
//   cover_vecs[0..cover_size] — cover vertices sorted ascending by as_u64()
//   cover_size                — |min vertex cover|; exact for bipartite, ≤2× for general
//   is_exact                  — true iff bipartite (König)
//   node_count                — total live nodes
```

### Key Invariants

| Invariant | Statement | Verified in test |
|-----------|-----------|-----------------|
| **Gallai** | α(G) + τ(G) = n (bipartite) | Test 9 (P4) |
| **König** | τ(G) = ν(G) for bipartite G | Test 10 (K_{3,3}) |
| **Cover validity** | Every edge has ≥1 endpoint in T | Tests 4,5,6,8 |
| **Star** | K_{1,k}: τ=1, cover={center} | Test 7 |
| **2-approx bound** | cover_size ≤ 2 × τ(G) for general | Tests 5,6 |

### König's Construction (Bipartite Case)

1. BFS 2-colouring → partition A (color=0), B (color=1)
2. Kuhn's augmenting-path DFS → max matching M, arrays `match_a[]`, `match_b[]`
3. König BFS from unmatched A-nodes:
   - A-node in Z → follow all non-matching edges to B-side (b ≠ match_a[a])
   - B-node in Z → follow matched edge to A-side (a = match_b[b])
4. Cover T = **(A \ Z_A) ∪ (B ∩ Z_B)**

**Proof sketch:** T covers all edges because any edge a-b:
- If b ∉ Z_B: then b was never reachable → a must be in A\Z_A (in T) for M to be max
- If b ∈ Z_B: then b ∈ T directly

### Shell Commands

```
graph vertex cover   gvc   vertex cover   gvertexcover   min vertex cover   gmincover
```

### Display

- Header: bright-cyan (color 11)
- Cover vertices: bright-green (color 10) with role "cover-vertex"
- Footer: node count, τ(G), "exact (bipartite König)" or "2-approx (non-bipartite)"

### VectorAddress Namespace

L4=73 for gos-graph-vc-harness

### OS Analogy

Minimum set of kernel modules such that every IPC channel, system call, or driver interaction passes through at least one of these modules — the smallest possible "audit checkpoint" set for all cross-module communication in the system dependency graph.

Analogous to the minimal set of network nodes that must be monitored to observe all traffic flows (minimum monitoring set / traffic interception cover).

### Relationships with Existing Algorithms

| Relationship | Algorithm | Version |
|-------------|-----------|---------|
| Gallai dual: α(G) = n − τ(G) | graph_independent_set | V2.96 |
| König source: τ = ν (bipartite) | graph_bipartite_match | V2.92 |
| τ ≤ n − max_matching (general) | Uses internal Kuhn matching | Internal |
| Edge cover dual: ρ = n − ν (Gallai 2) | — | Future |

### Test Suite (10 tests)

| Test | Graph | Expected τ | is_exact |
|------|-------|-----------|---------|
| 1 | Empty | 0 | true |
| 2 | Single node (no edges) | 0 | true |
| 3 | Single edge A-B | 1 | true |
| 4 | Path P4 | 2 | true |
| 5 | Triangle K3 | ≤4 (2-approx) | false |
| 6 | K4 | ≤6 (2-approx) | false |
| 7 | Star K_{1,4} | 1, cover={center} | true |
| 8 | K_{3,3} bipartite | 3, cover validity | true |
| 9 | Gallai cross-check P4 | α+τ=n=4 | true |
| 10 | König cross-check K_{3,3} | τ=ν=3 | true |

### Algorithm Complexity

- Bipartite: O(V·E) — Kuhn's matching + O(V+E) König BFS
- General: O(E) — greedy maximal matching scan
- Space: O(V) additional arrays on kernel stack (~10 KB total)

### Literature

- König 1931: "Graphen und Matrizen", exact min-VC for bipartite via max matching
- Gallai 1959: α(G) + τ(G) = n (independence + cover complementarity)
- Kuhn 1955 / Hungarian method: augmenting-path bipartite matching
- Garey & Johnson 1979: min-VC NP-complete for general graphs (Karp reduction 6)
- Bar-Yehuda & Even 1981: 2-approximation via maximal matching

---

## Cumulative Statistics

| Version | Algorithm | New Tests | Total Tests |
|---------|-----------|-----------|-------------|
| V2.93 | 2-edge-connected components | 10 | 903 |
| V2.94 | k-truss decomposition | 10 | 913 |
| V2.95 | Maximum clique (BK + Tomita) | 10 | 923 |
| V2.96 | Maximum independent set (BK complement) | 10 | 933 |
| **V2.97** | **Minimum vertex cover (König + 2-approx)** | **10** | **943** |
