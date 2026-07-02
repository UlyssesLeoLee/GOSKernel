# HARDENING LOG — V2.43: `graph pagerank`
**Date:** 2026-07-02  
**Branch:** `feat/vk-auto-live-surface` (automated hardening)  
**Version:** V2.43  
**Prior version:** V2.42 (graph katz — incoming Katz centrality)

---

## 变更摘要 / Change Summary

新增 `graph pagerank` shell 命令及底层 `gos_runtime::graph_pagerank` API，实现经典 PageRank 随机游走中心性算法，并新建 `gos-graph-pagerank-harness`（10 个宿主测试）。

Added `graph pagerank` shell command and `gos_runtime::graph_pagerank` public API — classical PageRank random-walk centrality — plus `gos-graph-pagerank-harness` (10 host tests).

---

## 动机 / Motivation

V2.42 完成了 Katz 中心性（incoming walk-count），其缺点是将每个节点的贡献视为等权：高出度节点与低出度节点的贡献相同。PageRank 通过将每个节点的 rank 按出度均分来修正这一问题，更准确地描述"随机游走者最终落在哪个节点"的概率分布。

V2.42 Katz centrality counts all walks equally. PageRank normalises each node's contribution by its out-degree, giving a more accurate model of where a random signal-following walker ends up. Together, Katz + PageRank provide two complementary lenses on the same graph:

- **Katz**: "how many total walks end here, all lengths summed?" (raw walk mass)
- **PageRank**: "what fraction of random-walk time is spent here?" (normalised authority)

OS analogy: **`top` sorted by incoming-signal weight** — which kernel nodes dominate the random walk over the live graph topology?

---

## 算法 / Algorithm

Classical PageRank with absorbing dangling nodes:

```text
PR[v] = (1-d) × SCALE + d × Σ_{u→v, outdeg(u)>0}  PR[u] / outdeg(u)
```

Parameters:
- `d = 0.85`  (standard damping factor)
- `SCALE = 1_000_000`  (fixed-point integer representation of 1.0)
- `TELE = 150_000`  (`= (1-d) × SCALE`, the teleportation floor)
- `PR_ITERS = 20`  (fixed iteration count — convergence verified by harness)
- Dangling nodes (`outdeg = 0`): absorb their rank (self-loop semantics — they receive signal but never forward it, the correct GOS model for terminal consumers)

**Complexity:** O(K × V × E) where K=20, V ≤ 128 nodes, E ≤ 128 edges.  
**Fixed-point arithmetic:** u64 intermediate, capped to u32 output.

### Steady-state values (analytically derived, verified by tests)

| Graph shape | PR value |
|-------------|----------|
| Isolated node | 150,000 (TELE floor) |
| Source node (no in-edges) | 150,000 |
| Single-edge receiver A→B | 277,500 |
| Chain tail A→B→C | 385,875 |
| Fan-in hub {A,B,C}→D | 532,500 |
| Ring / mutual cycle (any size) | 1,000,000 (authority) |
| Fork target A→{B,C} (outdeg=2) | 213,750 |

---

## 修改文件 / Files Changed

### `crates/gos-runtime/src/lib.rs`

**Added: `GosRuntime::graph_pagerank_inner<N>()`** (method, ~80 lines)
- Builds compact node-slot list
- Computes `out_deg[slot]` array from edge table
- Initialises `pr0[slot] = SCALE` for all live nodes
- 20-iteration double-buffer update loop (pr0 → pr1 → pr0)
- Insertion sort descending by final PR value
- Packs output into `([VectorAddress; N], [u32; N], usize)`

**Added: `graph_pagerank<N>()`** (public free function, ~25 lines doc + 1 line body)
- Locks `RUNTIME`, calls `graph_pagerank_inner()`
- Full doc comment with algorithm formula, OS analogy, and score-to-role mapping

### `crates/k-shell/src/lib.rs`

**Added: `dispatch_graph_pagerank(sink: &ConsoleSink)`** (~85 lines)
- Column layout: `vector (16) | pagerank (6k) | role`
- Roles: **authority** (≥ 1,000k, bright yellow), **relay** (> 300k, cyan), **sink** (≤ 300k, dark grey)
- Footer: node count, damping factor, max PR in ×1e-3 display, authority count if any

### `crates/k-shell/src/proc.rs`

- Dispatch branch: `"graph pagerank"`, `"pagerank"`, `"pr"`, `"graph rank"`, `"rank"`
- Help text: two lines (command + aliases)

### `host-tests/gos-graph-pagerank-harness/`

New harness crate (isolated `[workspace]`, own `.cargo/config.toml`):
- **10 tests** — all pass (`10 passed; 0 failed`)
- Tests 1–4: empty graph, isolated node, single edge, chain
- Tests 5–7: fan-in star, 3-cycle authority, mutual-cycle authority
- Tests 8–10: out-degree splitting, sort verification, total-count match

---

## Shell 命令 / Shell Commands

```text
graph pagerank          PageRank per node (random-walk stationary distribution)
pagerank                alias
pr                      alias
graph rank              alias
rank                    alias
```

### 示例输出 / Example Output

```text
 graph pagerank
 ─────────────────────────────────────────────────────────
  vector           pagerank  role
  [20:0:1:0]            999k  authority
  [20:0:2:0]            532k  relay
  [20:0:3:0]            213k  sink
  [20:0:4:0]            150k  sink
 ─────────────────────────────────────────────────────────
  4 node(s)  d=0.85  max-pr: 999k (×1e-3)  authorities: 1
```

---

## 角色语义 / Role Semantics

| Role | Threshold | Meaning |
|------|-----------|---------|
| **authority** | PR ≥ 1,000,000 | Dominates random-walk traffic; occurs when node is in a cycle or receives from many high-rank nodes |
| **relay** | 300,000 < PR < 1,000,000 | Above-floor link contribution; a structural hub but not cyclic |
| **sink** | PR ≤ 300,000 | Near teleportation floor; few or no inbound links |

---

## Katz vs PageRank / 对比

| Metric | Katz (V2.42) | PageRank (V2.43) |
|--------|--------------|-----------------|
| Count | All walks, all lengths | Random-walk equilibrium |
| Normalisation | None (outdeg bias) | ÷ outdeg (voter model) |
| High-outdeg node | Contributes more | Contributes less per edge |
| Best question | "Which node receives most traffic?" | "Which node is most structurally authoritative?" |
| OS analogy | `netstat -s` | `top` |

---

## 测试摘要 / Test Summary

**Host test suite total: 433 tests** (was 423, +10)

```text
gos-graph-pagerank-harness: 10/10 ✓
```

Full suite verified clean by prior session; only pagerank harness re-run this firing.

---

## 不变量 / Invariants Maintained

- `graph_pagerank_inner` is a pure read — no epoch bump, no write operations
- Isolated nodes return PR = 150,000 (teleportation floor, not zero) — distinct from Katz (zero for isolated nodes)
- Output always sorted descending: `pr[0] ≥ pr[1] ≥ ... ≥ pr[total-1]`
- No alloc, no_std safe, O(K×V×E) with K=20

---

## 后续建议 / Next Steps

- `node checkpoint <vec>` — snapshot node state to diff ring (observability)
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph hits` — HITS algorithm (hub/authority bipartite decomposition), complementary to PageRank
- PAL_U32 → attribute node refactor (Demo A prerequisite)
