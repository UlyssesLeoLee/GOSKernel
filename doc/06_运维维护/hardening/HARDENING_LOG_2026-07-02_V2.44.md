# HARDENING LOG — V2.44: `graph hits`
**Date:** 2026-07-02  
**Branch:** `feat/vk-auto-live-surface` (automated hardening)  
**Version:** V2.44  
**Prior version:** V2.43 (graph pagerank — random-walk stationary distribution)

---

## 变更摘要 / Change Summary

新增 `graph hits` shell 命令及底层 `gos_runtime::graph_hits` API，实现 Kleinberg HITS 算法（hub/authority 二部图分解），并新建 `gos-graph-hits-harness`（10 个宿主测试）。

Added `graph hits` shell command and `gos_runtime::graph_hits` public API — Kleinberg's HITS (Hyperlink-Induced Topic Search) hub/authority bipartite decomposition — plus `gos-graph-hits-harness` (10 host tests).

---

## 动机 / Motivation

至此图论中心性系列算法覆盖三种经典视角：

| V2.42 Katz   | 所有路径长度的 walk 数量（原始影响力）         |
| V2.43 PageRank | 随机游走稳定分布（归一化权威性）             |
| V2.44 HITS     | 二部图分解：哪些节点是最好的"指针"，哪些节点是最被引用的"目标" |

HITS 与 PageRank 的核心区别：PageRank 只给每个节点一个分数；HITS 给每个节点**两个**分数（hub + authority），将图分解为"转发者"和"目标"两类，在有向二部结构（发送节点 → 服务节点）中尤为有价值。

OS analogy: `vmstat` / `top` bipartite — which kernel nodes are the best signal-forwarders (**hub**) vs the most-cited signal-destinations (**authority**)?

---

## 算法 / Algorithm

Kleinberg's HITS with L∞ normalization:

```
Initialise: h[v] = a[v] = SCALE for all live nodes

For each iteration (20 total):
  new_a[v] = Σ_{u→v} h[u]          (authority = sum of in-neighbor hub scores)
  new_h[v] = Σ_{v→w} a[w]          (hub = sum of out-neighbor authority scores)
  [updates are simultaneous — use old h and a values]

  max_a = max over all v of new_a[v]
  max_h = max over all v of new_h[v]

  a[v] = new_a[v] × SCALE / max_a   (if max_a > 0, else 0)
  h[v] = new_h[v] × SCALE / max_h   (if max_h > 0, else 0)

Output sorted descending by authority score.
```

Parameters:
- `SCALE = 1_000_000`
- `ITERS = 20`
- Dangling nodes: no out-edges → hub = 0; no in-edges → auth = 0

**Complexity:** O(K × V × E) where K=20, V ≤ 128, E ≤ 512.

### Converged values (verified by harness)

| Graph shape              | hub              | authority        |
|--------------------------|------------------|------------------|
| Isolated node            | 0                | 0                |
| Pure source A (A→B)      | 1,000,000        | 0                |
| Pure sink B (A→B)        | 0                | 1,000,000        |
| Middle of chain B (A→B→C)| 1,000,000        | 1,000,000        |
| Star-out center A→{B,C,D}| 1,000,000        | 0                |
| Star-in center {A,B,C}→D | 0                | 1,000,000        |
| Mutual cycle A↔B         | 1,000,000        | 1,000,000        |
| 3-cycle (any node)       | 1,000,000        | 1,000,000        |
| Bipartite hub (no in)    | 1,000,000        | 0                |
| Bipartite authority (no out) | 0            | 1,000,000        |

---

## 修改文件 / Files Changed

### `crates/gos-runtime/src/lib.rs`

**Added: `GosRuntime::graph_hits_inner<N>()`** (method, ~100 lines)
- Builds compact node-slot list
- Initialises hub[v] = auth[v] = SCALE for all live nodes
- 20-iteration double-buffer simultaneous update (new_auth from old hub; new_hub from old auth)
- L∞ normalization: divide by max_auth and max_hub independently
- Insertion sort descending by authority
- Packs output into `([VectorAddress; N], [u32; N], [u32; N], usize)` — (vecs, hub, auth, total)

**Added: `graph_hits<N>()`** (public free function)
- Locks `RUNTIME`, calls `graph_hits_inner()`
- Full doc comment with algorithm, OS analogy, role interpretation

### `crates/k-shell/src/lib.rs`

**Added: `dispatch_graph_hits(sink: &ConsoleSink)`** (~110 lines)
- Column layout: `vector (16) | hub (6k) | authority (6k) | role`
- Role colors:
  - **hub+authority** ≥ 800k: magenta (e.g. cycle nodes)
  - **authority** ≥ 800k: bright yellow
  - **hub** ≥ 800k: cyan
  - **isolated** < 200k both: dark grey
  - **relay**: white (some score, but not top in either dimension)
- Footer: node count, iteration count, top-hub count, top-authority count

### `crates/k-shell/src/proc.rs`

- Dispatch branch: `"graph hits"`, `"hits"`, `"graph ha"`, `"ha"`, `"hub authority"`

### `host-tests/gos-graph-hits-harness/`

New harness crate (isolated `[workspace]`, own `.cargo/config.toml`):
- **10 tests** — all pass (`10 passed; 0 failed`)
- Tests 1–2: empty graph, isolated node
- Tests 3–6: single edge, chain, star-out, star-in
- Tests 7–8: mutual cycle, 3-cycle
- Tests 9–10: bipartite structure, sort order verification

---

## Shell 命令 / Shell Commands

```
graph hits          HITS hub+authority bipartite decomposition
hits                alias
graph ha            alias
ha                  alias
hub authority       alias
```

### 示例输出 / Example Output

```
 graph hits
 ─────────────────────────────────────────────────────────
  vector             hub   authority  role
  [21:0:3:0]         0k      1000k  authority
  [21:0:4:0]         0k      1000k  authority
  [21:0:1:0]      1000k         0k  hub
  [21:0:2:0]      1000k         0k  hub
 ─────────────────────────────────────────────────────────
  4 node(s)  HITS/20iter  hubs: 2  authorities: 2
```

---

## 角色语义 / Role Semantics

| Role          | Hub threshold | Auth threshold | Meaning |
|---------------|---------------|----------------|---------|
| **hub+authority** | ≥ 800k | ≥ 800k | Symmetric role: cycle node, relay in dense structure |
| **authority** | any | ≥ 800k | Cited by top hubs; the best signal-destinations |
| **hub** | ≥ 800k | any | Points to top authorities; the best signal-forwarders |
| **relay** | 200k–800k | 200k–800k | Partial structural role |
| **isolated** | < 200k | < 200k | No in-edges and no out-edges to scored nodes |

---

## HITS vs PageRank vs Katz / 三者对比

| Metric | Katz (V2.42) | PageRank (V2.43) | HITS (V2.44) |
|--------|--------------|------------------|--------------|
| Scores per node | 1 (authority) | 1 (authority) | 2 (hub + authority) |
| Normalisation | None | ÷ outdeg | L∞ per iteration |
| Isolated nodes | 0 | 150,000 (TELE floor) | 0, 0 |
| Cycle nodes | SCALE/7 | 1,000,000 | hub=auth=1M |
| Best question | "most walk traffic?" | "random walk frequency?" | "pointer vs cited-target?" |
| OS analogy | `netstat -s` | `top` | `vmstat` bipartite |

---

## 测试摘要 / Test Summary

**Host test suite total: 443 tests** (was 433, +10)

```
gos-graph-hits-harness: 10/10 ✓
```

---

## 不变量 / Invariants Maintained

- `graph_hits_inner` is a pure read — no epoch bump, no write operations
- Isolated nodes: hub=0, auth=0 (distinct from Katz=0 and PageRank=150k)
- Output always sorted descending by authority: `auth[0] ≥ auth[1] ≥ ...`
- No alloc, no_std safe, O(K×V×E) with K=20

---

## 后续建议 / Next Steps

- `node checkpoint <vec>` — snapshot node state to diff ring (observability)
- `journal ring <N>` — runtime-configurable JournalRing capacity
- `graph sim <N>` — simulate N random-walk steps, emit signal traffic trace
- PAL_U32 → attribute node refactor (Demo A prerequisite)
