# Hardening Log — V2.45: `graph community` — Label Propagation Community Detection

**Date:** 2026-07-02  
**Branch:** `feat/vk-auto-live-surface`  
**Commit:** (see below)  
**Author:** Claude (automated hardening run)

---

## Summary

V2.45 adds **`graph community`** — Label Propagation Algorithm (LPA) community detection over the GOS kernel graph, completing the graph-analytics surface with the first *clustering* primitive.

Shell aliases: `graph community` / `community` / `lpa` / `graph lpa` / `graph cluster` / `cluster`

OS analogy: `iproute2 bridge vlan show` + `systemd-analyze critical-chain` — which kernel service nodes naturally cluster into tightly coupled sub-systems?

---

## Motivation

After the centrality/ranking suite (V2.38–V2.44, degree → betweenness → closeness → eccentricity → Katz → PageRank → HITS), the natural next step is **community structure**: not "which node is most important?" but "which nodes belong to the same functional sub-system?"

In a graph OS, community detection answers operational questions like:
- Which services are co-dependent and should be co-located / co-faulted?
- Which groups form natural isolation domains?
- Does a proposed architectural change split or merge existing communities?

---

## Algorithm: Asynchronous Label Propagation (LPA)

```text
Initialize: label[v] = slot_index(v)   // each node in its own community

For iter in 0..20:
  For each node v in slot order:
    freq[l] = |{neighbors u of v (in+out) where label[u] == l}|
    label[v] = argmax_l freq[l]         // tie-break: smallest l
    # IMMEDIATE update — later nodes see v's new label this round

Relabel: communities 0, 1, 2... sorted by size descending
Output: nodes sorted by (community_id asc, slot asc) for grouped display
```

**Key design choices:**

1. **Undirected treatment**: both in-edges and out-edges are used as undirected neighbor links. This makes the algorithm sensitive to co-location in the service graph regardless of signal direction, matching the OS subsystem intuition.

2. **Asynchronous updates**: each node's label is updated immediately (not buffered until the end of the round). This is critical — the synchronous variant oscillates on bipartite and chain topologies (two nodes swapping labels every round, never converging). The asynchronous variant converges in O(iterations) for all connected components.

3. **Tie-break: smallest label** — when two labels have equal frequency, the smaller label wins. This produces deterministic, stable output.

4. **20 iterations** — consistent with all other iterative algorithms in the V2 suite (PageRank, Katz, HITS).

5. **Community relabelling**: after LPA converges, communities are assigned ids 0, 1, 2... sorted by member count descending. The largest community always gets id 0, displayed as "major-community". This makes the output intuitive: C0 is the biggest cluster.

**Complexity:** O(V × E × 20) per call — same order as PageRank/HITS.

**Space:** O(MAX_NODES) = O(128) — all fixed arrays, no_std/no_alloc compatible.

---

## Implementation

### `crates/gos-runtime/src/lib.rs`

- **`RuntimeState::graph_community_inner<const N>()`** — core algorithm (asynchronous LPA, relabelling, sorting).
- **`pub fn graph_community<const N>()`** — free function wrapper (locks RUNTIME, calls inner).

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_community(sink)`** — display function:
  - Header: cyan `graph community`
  - Per-community block: `[C0]  N nodes  major-community / minor-community / isolated`
  - Member node vectors listed 4 per row (magenta = major, cyan = minor, grey = isolated)
  - Footer: `N nodes  LPA/20iter  communities: M`

### `crates/k-shell/src/proc.rs`

- Dispatch: `"graph community" | "community" | "lpa" | "graph lpa" | "graph cluster" | "cluster"` → `dispatch_graph_community`
- Help text: two new lines documenting the command and its aliases.

---

## Test Harness: `host-tests/gos-graph-community-harness`

10 tests covering the full community detection API:

| # | Scenario | Assertion |
|---|----------|-----------|
| 1 | Empty graph | total=0, comm_count=0 |
| 2 | Single isolated node | 1 node, 1 community, id=0 |
| 3 | Two disconnected nodes (no edges) | 2 nodes, 2 communities (cannot merge without edges) |
| 4 | Single edge A→B | 2 nodes, 1 community (undirected neighbor → merges) |
| 5 | Directed triangle A→B→C→A | 3 nodes, 1 community |
| 6 | Two disconnected pairs (A─B, C─D) | 4 nodes, 2 communities; A,B same; C,D same; pairs differ |
| 7 | Complete bipartite K_{2,2} (A,B→C,D) | 4 nodes, 1 community (all undirected-reachable) |
| 8 | Two triangles, no bridge | 6 nodes, 2 communities (one per triangle) |
| 9 | Sorted output contiguity | community ids in output are non-decreasing |
| 10 | Fully connected chain A─B─C─D | 4 nodes, 1 community, all ids=0 |

**Result:** 10/10 pass.

---

## Community Role Semantics

| Role | Condition | Color |
|------|-----------|-------|
| `major-community` | Largest community (id=0) with >1 node | Magenta (13) |
| `minor-community` | Multi-node community, not the largest | Cyan (11) |
| `isolated` | Single-node community (no undirected neighbors) | Dark grey (8) |

---

## Shell Command Surface

```text
graph community         label-propagation community detection
community               alias
lpa                     alias (Label Propagation Algorithm)
graph lpa               alias
graph cluster           alias
cluster                 alias
```

Example output (two sub-systems, one isolated service):

```text
 graph community
 ───────────────────────────────────────────────────────────
  [C0]  3 nodes  major-community
      1.0.1.0  1.0.2.0  1.0.3.0
  [C1]  2 nodes  minor-community
      2.0.1.0  2.0.2.0
  [C2]  1 node   isolated
      3.0.1.0
 ───────────────────────────────────────────────────────────
  6 nodes  LPA/20iter  communities: 3
```

---

## Invariants Preserved

- **No write ops**: `graph_community` is a pure read (no epoch bump, no mutation).
- **No alloc / no_std**: all buffers are fixed-size stack arrays.
- **TEST_LOCK + reset()**: harness uses the standard isolation pattern.
- **Sequential version**: V2.45 follows V2.44 (HITS) directly.
- **Doc archived**: this file at `doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.45.md`.

---

## Next Steps

Suggested V2.46 candidates:
- `graph spanning` — BFS/DFS spanning tree (minimal connector backbone)
- `node checkpoint <vec>` — snapshot node state to diff ring
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph sim <N>` — simulate N random-walk steps, emit signal traffic trace
