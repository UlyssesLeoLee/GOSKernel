# Hardening Log — V2.53: Weighted Betweenness Centrality (Brandes + Dijkstra)

**Date:** 2026-07-03  
**Branch:** feat/vk-auto-live-surface  
**Version:** V2.53  
**Author:** Automated hardening cycle  

---

## Summary

Implemented `graph between` — **weighted betweenness centrality** via Brandes
algorithm with O(V²) Dijkstra per source node.  Complements the existing
`graph centrality` (V2.39, unweighted BFS Brandes) by honouring `edge.spec.weight`
when finding shortest paths.

**Key distinction:**
- `graph centrality` (V2.39) — minimum *hop-count* paths (BFS), weight-blind
- `graph between` (V2.53)    — minimum *weighted* paths (Dijkstra), weight-aware

When a low-weight indirect path is cheaper than a high-weight direct edge, the
two algorithms diverge.  On uniform-weight graphs they produce identical results.

---

## Algorithm

**Brandes with Dijkstra** (directed, weighted betweenness):

```
WBC[v] = Σ_{s≠v≠t} σ_w(s,t,v) / σ_w(s,t)
```

where `σ_w(s,t)` counts minimum-weight directed paths from s to t.

For each source node s:
1. **Forward pass** — O(V²) Dijkstra (no heap):
   - Find `dist[v]` = shortest weighted distance from s to v
   - Track `sigma[v]` = number of minimum-weight paths from s to v
   - Record `stk[]` = nodes in non-decreasing dist order (Brandes stack)
2. **Back-propagation** — reverse `stk` order:
   - For each node w, find predecessors v via in-edges where `dist[v]+weight ≈ dist[w]`
   - `delta[v] += sigma[v] × (SCALE + delta[w]) / sigma[w]`
   - `bc[w] += delta[w]` for w ≠ s
3. Sort descending; output `bc_scaled[v] / 1_000_000` as `u32`.

**Complexity:** O(V² × (V+E)) — one O(V²) Dijkstra pass per source node.  
**Float epsilon:** 1e-6 for predecessor detection (`dist[v]+weight ≈ dist[w]`).

---

## Files Changed

### `crates/gos-runtime/src/lib.rs`

- **`GraphRuntime::graph_between_inner<N>()`** — private method after `graph_sim_inner`.
  Implements Brandes with Dijkstra using `self.edges[ei].spec.weight` for path weights.
  Output: `([VectorAddress; N], [u32; N], usize)` — same shape as `graph_centrality_inner`.

- **`pub fn graph_between<N>()`** — public API wrapper after `pub fn graph_sim`.
  Acquires `RUNTIME.lock()` and delegates to `graph_between_inner`.

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_between(sink)`** — display function inserted before `dispatch_graph_closeness`.
  Color scheme: bright magenta (13) for keystones (vs bright yellow (14) for unweighted centrality),
  cyan (11) for relays, grey (8) for endpoints.
  Header: `graph between  (weighted Dijkstra)`.
  Footer: `N node(s)  max-wbc: X  keystones: Y`.

### `crates/k-shell/src/proc.rs`

- Routing block inserted after `graph sim` block (~line 993):
  ```
  graph between | between | gbetween | graph wbc | wbc | weighted betweenness
  ```

### `host-tests/gos-graph-between-harness/`

New harness (10 tests, L4=30 VectorAddress namespace):

| Test | Scenario | Key Assertion |
|------|----------|---------------|
| 1 | Empty graph | `total=0`, no panic |
| 2 | Single isolated node | `WBC[A]=0`, `total=1` |
| 3 | Two-node A→B | `WBC[A]=WBC[B]=0` |
| 4 | Path A→B→C (w=1.0) | `WBC[B]=1` |
| 5 | Bottleneck {A,B}→X→{C,D} | `WBC[X]=4` |
| 6 | **Weight-sensitive**: A→C (w=0.5), C→B (w=0.5), A→B (w=2.0) | `WBC[C]=1` (unlike BFS which gives 0) |
| 7 | Bottleneck {A,B,C}→X→{D,E,F} | `WBC[X]=9` |
| 8 | Linear 5-node (w=1.0) | `WBC[C]=4`, `WBC[B]=WBC[D]=3` |
| 9 | Sort order | `wbc[i-1] >= wbc[i]` for all i |
| 10 | Self-loop + isolated node | No crash, `WBC[A]=0` |

---

## Shell Commands

| Command | Alias |
|---------|-------|
| `graph between` | `between` |
| `gbetween` | `graph wbc` |
| `wbc` | `weighted betweenness` |

---

## Invariants Maintained

- Pure read: no epoch bump, no write operations
- All stack arrays bounded by `MAX_NODES=128` / `MAX_EDGES=512`
- Self-loops (`v == u`) explicitly skipped in Dijkstra relaxation
- Zero-weight edges handled via `weight.max(0.0)`
- Float epsilon 1e-6 for predecessor detection (consistent with `graph_flow` at 1e-9)
- `sigma[v]=0` guards division in back-propagation (no divide-by-zero)
- Output scale: `bc_scaled[v] / 1_000_000` as u32 (same as `graph_centrality`)

---

## OS Analogy

`traceroute` with measured latencies — `graph between` answers "which kernel
service node sits on the most minimum-latency paths between other service pairs?"

Unlike `graph centrality` (hop-count betweenness), this correctly identifies
low-latency relay nodes that BFS would overlook because they require more hops
than a direct but high-latency alternative path.

---

## Host Test Suite — Running Total

**503 tests** across 50 harnesses (all green):
- Previous (V2.52): 493 tests / 49 harnesses
- **Added (V2.53): +10 tests** via `gos-graph-between-harness`
