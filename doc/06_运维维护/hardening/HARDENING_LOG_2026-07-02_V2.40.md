# GOS Hardening Log — V2.40 (2026-07-02)

## Feature: `graph closeness` — Outgoing Closeness Centrality

**Branch:** feat/vk-auto-live-surface  
**Commit scope:** feat(v2.40): graph closeness / closeness — outgoing closeness centrality  
**Test suite added:** gos-graph-closeness-harness (10 tests)  
**Total host-test count after this slice:** 403

---

## What Was Built

### Shell Command Surface

| Command | Aliases | Description |
|---------|---------|-------------|
| `graph closeness` | `closeness`, `graph close`, `close centrality`, `cc` | Outgoing closeness centrality per node, sorted descending |

### Algorithm: Outgoing Closeness Centrality (BFS per source)

**Definition:**

For each live node v, the outgoing closeness centrality is:

```
CC[v] = r_v × SCALE / Σ_{u reachable from v, u≠v} d(v,u)
```

Where:
- `r_v` = number of nodes reachable from `v` via directed edges (excluding `v` itself)
- `d(v,u)` = BFS shortest-path distance from `v` to `u`
- `SCALE` = 1,000,000 (fixed-point, avoids floating-point in `no_std`)
- Isolated nodes (`r_v = 0`): `CC[v] = 0`

**Complexity:** O(V × (V + E)) — one BFS per source node.

**Output:** Sorted descending by CC score. Role annotations:
- `central` — highest CC score: node that can broadcast to all others most efficiently
- `relay` — moderate CC: reaches others, but not at maximum efficiency
- `peripheral` — CC = 0: isolated, pure sink, or disconnected from any reachable subgraph

**Fixed-point note:** CC scores are reported as integers × 10⁻⁶. For example:
- CC = 1,000,000 → exact closeness = 1.0 (reaches all reachable nodes in exactly 1 hop)
- CC = 666,666 → exact closeness ≈ 0.6667 (average 1.5 hops to reachable nodes)
- CC = 500,000 → exact closeness = 0.5 (average 2 hops)

**Disconnected graph handling:** The formula uses `r_v` (reachable count) in the numerator, which naturally gives credit to nodes that reach many nodes even if the graph is partitioned. Nodes that cannot reach any other node get CC = 0.

### Comparison with Betweenness Centrality (V2.39)

| Dimension | Betweenness (V2.39) | Closeness (V2.40) |
|-----------|--------------------|--------------------|
| Question | "Which nodes sit on the most shortest paths?" | "Which nodes can reach all others fastest?" |
| Algorithm | Brandes 2001 (O(V×E)) | BFS per source (O(V×(V+E))) |
| High score means | Critical routing bottleneck | Efficient broadcaster / broadcaster hub |
| Zero score | Never an intermediary (leaf/isolated) | Cannot reach any other node (sink/isolated) |
| OS analogy | `traceroute` hop frequency | `ping` RTT average |

Together, betweenness + closeness centrality complete a "structural bottleneck + reach efficiency" analysis pair:
- Betweenness answers **dependency risk**: removing a high-BC node breaks many paths.
- Closeness answers **latency reach**: a high-CC node disseminates signals most quickly.

---

## Files Modified

### `crates/gos-runtime/src/lib.rs`
- Added `graph_closeness_inner<const N>(&self)` method on `GosRuntime` struct
- Added `pub fn graph_closeness<const N>()` public wrapper (acquires `RUNTIME` lock)

### `crates/k-shell/src/lib.rs`
- Added `pub fn dispatch_graph_closeness(sink: &ConsoleSink)` with full color-coded table output

### `crates/k-shell/src/proc.rs`
- Added routing: `"graph closeness" || "closeness" || "graph close" || "close centrality" || "cc"`
- Added help-text entries for `graph closeness` and `closeness / cc`

### `host-tests/gos-graph-closeness-harness/` (new)
- `Cargo.toml` — workspace-isolated harness crate
- `.cargo/config.toml` — `target = "x86_64-pc-windows-msvc"`, `build-std = ["std", "panic_abort"]`
- `tests/graph_closeness.rs` — 10 tests

---

## Test Coverage (10 tests)

| # | Test | Assertion |
|---|------|-----------|
| 1 | Empty graph | `total=0`, no panics |
| 2 | Single isolated node | `CC[A]=0, total=1` |
| 3 | Two-node A→B | `CC[A]=1_000_000, CC[B]=0` |
| 4 | Path A→B→C | `CC[B]=1_000_000, CC[A]=666_666, CC[C]=0`; B first |
| 5 | Star A→{B,C,D} | `CC[A]=1_000_000`, leaves `=0`; A first |
| 6 | Directed 3-cycle A→B→C→A | All equal `666_666` (rotational symmetry) |
| 7 | Diamond A→{B,C}→D | `CC[B]=CC[C]=1_000_000, CC[A]=750_000, CC[D]=0` |
| 8 | Linear 5-node chain A→B→C→D→E | D>C>B>A>E order; exact values asserted |
| 9 | Disconnected {A→B} ∥ {C→D} | `CC[A]=CC[C]=1_000_000`, sinks `=0` |
| 10 | Self-loop A→A + B→C | `CC[A]=0` (self-loop = no external reach) |

All 10 tests: **PASS** (verified locally via `cargo +nightly test`).

---

## OS Analogy

**`graph closeness`** ↔ **`ping` RTT average census**

Just as `ping -c 100 <host>` measures the average latency to a remote endpoint, closeness centrality measures how quickly a kernel service node can "reach" all other nodes in the service graph via directed signal edges. A high-CC node is like a core routing daemon with sub-millisecond RTT to all peers — it can propagate signals to the widest set of nodes in the fewest hops.

```
# Equivalent conceptual operation in a POSIX OS:
for host in $(hosts); do
    avg_rtt=$(ping -c 10 $host | awk '/avg/{print $4}' | cut -d/ -f2)
    echo "$host: avg_rtt=$avg_rtt"
done | sort -t= -k2 -n

# GOS equivalent:
graph closeness
```

---

## Graph Algorithm Suite — Status After V2.40

| Version | Command | Algorithm | Complexity |
|---------|---------|-----------|------------|
| V2.32 | `graph cycles` | DFS 3-color | O(V+E) |
| V2.33 | `graph toposort` | Kahn BFS | O(V+E) |
| V2.34 | `graph scc` | Kosaraju | O(V+E) |
| V2.35 | `graph condensation` | Kosaraju+adj | O(V+E+V²) |
| V2.36 | `graph reachable <V>` | Iterative DFS | O(V+E) |
| V2.37 | `graph bipartite` | BFS 2-coloring | O(V+E) |
| V2.38 | `graph degree` | Edge census | O(V×E) |
| V2.39 | `graph centrality` | Brandes BC | O(V×E) |
| **V2.40** | **`graph closeness`** | **BFS per source** | **O(V×(V+E))** |

**Next candidates:**
- `graph eccentricity` — max shortest-path distance from each node (graph radius/diameter)
- `node checkpoint <vec>` — snapshot node state to diff ring
- `journal ring <N>` — runtime-configurable JournalRing capacity
- PAL_U32 → attribute node refactor (Demo A prerequisite)
