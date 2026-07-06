# GOS Hardening Log — V2.98 (2026-07-06)

## Feature: Minimum Dominating Set (greedy ln(Δ)+1 approximation)

### Summary

Added `graph_dominating_set<N>()` to gos-runtime — the first graph monitoring-placement
metric in the kernel.  A dominating set D ⊆ V satisfies: every node not in D has at
least one neighbour in D.  γ(G) = |D_min|.

Greedy algorithm achieves the best polynomial-time approximation ratio: ≤ H(Δ)+1 ≈ ln(Δ)+1
(where Δ = max degree), matching the NP-hardness lower bound (Johnson 1974).

### OS Analogy

Minimum monitor deployment: place health-watchers on the fewest subsystems such that
every uninstrumented module is directly adjacent to at least one instrumented neighbour.
Equivalent to the classic facility-location / coverage problem from production networks.

This completes the "coverage trio":
- τ(G) vertex cover (V2.97): every EDGE touches ≥1 covered node (IPC audit checkpoint)
- α(G) independent set (V2.96): maximum set with no shared edges (parallel boot frontier)
- γ(G) dominating set (V2.98): every non-monitor has a monitor neighbour (telemetry net)

### Implementation

**gos-runtime/src/lib.rs**
- New method: `GraphRuntime::graph_dominating_set_inner<N>()`
  - Step 1: compact live node slots (same pattern as vertex cover)
  - Step 2: slot→compact-index mapping for bitmask operations
  - Step 3: build `dominated[ci]` = {ci} ∪ undirected-neighbours as u128 bitmask
  - Step 4: greedy loop — pick node with max `(dominated[ci] & undominated).count_ones()`
  - Step 5: collect and insertion-sort result ascending by `vector.as_u64()`
- New public function: `graph_dominating_set<N>() -> ([VectorAddress; N], usize, usize)`
  - Returns `(dom_vecs, dom_size, node_count)`
  - `dom_vecs[0..dom_size]` = dominating set sorted ascending by `as_u64()`

**crates/k-shell/src/lib.rs**
- New `dispatch_graph_dominating_set()`:
  - Bright-yellow header (color 14), bright-cyan members (color 11)
  - Footer: `γ(G)=N  greedy ≤ ln(Δ)+1 approx`

**crates/k-shell/src/proc.rs**
- Routing added after "graph vertex cover":
  `"graph domset" || "gdomset" || "dominating set" || "graph dominating set" || "gdominate" || "min domset"`

**host-tests/gos-graph-domset-harness/**
- VectorAddress L4=74
- 10 tests, all pass:
  1. Empty graph → γ=0, node_count=0
  2. Single isolated node → γ=1 (must include itself)
  3. Two isolated nodes → γ=2 (no mutual coverage)
  4. K_2 edge → γ=1 (one endpoint covers both)
  5. Triangle K_3 → γ=1 (any node covers all)
  6. Path P_4 → γ=2 (validity: every node covered)
  7. Star K_{1,4} → γ=1 (centre always selected first; covers all 5)
  8. K_4 complete → γ=1 (any node covers all 4)
  9. Mixed (2 isolated + 1 edge) → γ=3 (isolated×2 + one edge endpoint)
  10. γ ≤ τ cross-check K_3 → dom_size=1 ≤ cover_size=2

### Key Invariants

| Invariant | Value | Notes |
|-----------|-------|-------|
| K_n domination | γ=1 | Any node dominates all |
| Star K_{1,k} | γ=1 | Centre dominates all |
| Path P_n | γ=⌈n/3⌉ | Greedy achieves optimum |
| Isolated nodes | Forced into D | Only they can cover themselves |
| γ(G) ≤ τ(G) | Always holds | Every vertex cover is a dominating set |
| Approximation ratio | ≤ H(Δ)+1 | Best poly-time guarantee (Johnson 1974) |

### Literature

- Ore 1962: domination number concept
- Johnson 1974: greedy ln(n)+1 approximation, NP-hardness lower bound
- Garey & Johnson 1979: NP-completeness of minimum dominating set
- Hedetniemi & Laskar 1990: domination in graphs survey

### Host-Test Suite Total

**953 tests** (943 prior + 10 new from gos-graph-domset-harness)

### VectorAddress L4 Namespace Update

```
73=graph-vc (V2.97)
74=graph-domset (V2.98, new)
```
