# GOS Hardening Log — V2.41

**Date:** 2026-07-02  
**Branch:** feat/vk-auto-live-surface  
**Author:** Scheduled hardening task (automated)  
**Scope:** Graph eccentricity — per-node worst-case hop count + graph radius / diameter

---

## 1. What was added

### Shell command surface

| Command aliases | Description |
|---|---|
| `graph eccentricity` / `eccentricity` / `graph ecc` / `ecc` / `graph radius` / `radius` | Per-node directed eccentricity, graph radius, graph diameter |

Output format (sorted ascending by eccentricity, centre nodes first):

```
 graph eccentricity
 ───────────────────────────────────────────────────────────
  vector              ecc   role
  6.1.3.0               1   center
  6.1.1.0               2   relay
  6.1.2.0               4   periphery
  6.1.4.0               0   isolated
 ───────────────────────────────────────────────────────────
  4 node(s)  radius: 1  diameter: 4  center: 1
```

**Role classification:**

| Role | Condition | Colour |
|---|---|---|
| `center` | ecc == radius (and radius > 0) | Bright yellow |
| `relay` | 0 < ecc < diameter, ecc ≠ radius | Cyan |
| `periphery` | ecc == diameter (and diameter ≠ radius) | Red |
| `isolated` | ecc == 0 (no reachable out-neighbours) | Dark grey |

When radius == diameter (e.g. a directed cycle), all non-isolated nodes are labelled `center`.

---

## 2. Algorithm — `graph_eccentricity_inner<const N>`

**Definition:**
```
ecc[v] = max d(v, u)   for all u reachable from v (u ≠ v, via directed edges)
ecc[v] = 0             if no u is reachable (isolated / pure sink)

radius   = min ecc[v]  for v with ecc[v] > 0   (0 if all nodes isolated)
diameter = max ecc[v]                            (0 if all nodes isolated)
```

**Approach:** One BFS per source node following outgoing directed edges.  
**Complexity:** O(V × (V+E)), no_std safe, static arrays only.

**OS analogy:** Like `traceroute` worst-case hop count — which kernel node guarantees the tightest maximum latency to all its reachable peers?

**Sort order:** Ascending eccentricity so centre nodes appear first. Isolated nodes (ecc=0) use u32::MAX as sort sentinel, placing them last in the output.

---

## 3. Files changed

| File | Change |
|---|---|
| `crates/gos-runtime/src/lib.rs` | Added `graph_eccentricity_inner<N>` (impl method) + `graph_eccentricity<N>` (public fn) |
| `crates/k-shell/src/lib.rs` | Added `dispatch_graph_eccentricity` |
| `crates/k-shell/src/proc.rs` | Added dispatch clause for `graph eccentricity` / `eccentricity` / `graph ecc` / `ecc` / `graph radius` / `radius` |
| `host-tests/gos-graph-eccentricity-harness/` | New harness crate (10 tests, all green) |

---

## 4. Test harness — gos-graph-eccentricity-harness (10 tests)

All 10 tests pass: `test result: ok. 10 passed; 0 failed`

| # | Test | Key assertion |
|---|---|---|
| 1 | `empty_graph_eccentricity_total_is_zero` | total=0, radius=0, diameter=0 |
| 2 | `isolated_node_has_zero_eccentricity` | ecc[A]=0, radius=0, diameter=0 |
| 3 | `two_node_edge_eccentricity` | ecc[A]=1, ecc[B]=0; radius=diameter=1 |
| 4 | `path_abc_eccentricity` | ecc[A]=2, ecc[B]=1, ecc[C]=0; radius=1, diameter=2; sort order B,A,C |
| 5 | `star_center_eccentricity` | ecc[A]=1, leaves=0; radius=diameter=1 |
| 6 | `directed_cycle_all_nodes_same_eccentricity` | all ecc=2; radius=diameter=2 |
| 7 | `diamond_eccentricity` | ecc[A]=2, ecc[B/C]=1, ecc[D]=0; radius=1, diameter=2 |
| 8 | `linear_five_node_chain_eccentricity_ordering` | ecc[D]=1..ecc[A]=4; sort D,C,B,A,E |
| 9 | `disconnected_pairs_eccentricity` | ecc[A]=ecc[C]=1, sinks=0; radius=diameter=1 |
| 10 | `self_loop_does_not_contribute_to_eccentricity` | ecc[A]=0, ecc[B]=1, ecc[C]=0; B first |

---

## 5. Invariants preserved

- All dispatch functions are pure reads — no epoch bump, no write operations.
- New harness uses `TEST_LOCK: Mutex<()>` + `reset()` with `unwrap_or_else(|e| e.into_inner())`.
- Harness has its own `.cargo/config.toml` with `target = "x86_64-pc-windows-msvc"` and `build-std`.
- Version sequence: V2.40=closeness → **V2.41=eccentricity**. Next: V2.42.

---

## 6. Graph algorithm suite status (V2.32–V2.41)

| Version | Command | Algorithm |
|---|---|---|
| V2.32 | `graph cycles` | DFS 3-color cycle detection |
| V2.33 | `graph toposort` | Kahn's BFS topological ordering |
| V2.34 | `graph scc` | Kosaraju 2-pass DFS |
| V2.35 | `graph condensation` | SCC condensation DAG |
| V2.36 | `graph reachable <vec>` | Iterative DFS reachability |
| V2.37 | `graph bipartite` | BFS 2-coloring |
| V2.38 | `graph degree` | In/out degree census |
| V2.39 | `graph centrality` | Brandes betweenness centrality |
| V2.40 | `graph closeness` | BFS outgoing closeness centrality |
| **V2.41** | **`graph eccentricity`** | **BFS eccentricity + radius/diameter** |

---

## 7. Next candidates (V2.42+)

- `node checkpoint <vec>` — snapshot node state to diff ring (observability)
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph katz` — Katz centrality (attenuation factor α, walk-length weights)
- PAL_U32 → attribute node refactor (Demo A prerequisite)
