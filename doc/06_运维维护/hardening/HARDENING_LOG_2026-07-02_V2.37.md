# GOS 硬化日志 — V2.37 — 2026-07-02

## 概述

V2.37 通过 `graph bipartite` 新增二分图检测分析 —— 一个基础图属性查询，用于回答"当前存活依赖图能否被拆分为两个互不冲突的调度层？"。当且仅当图中不存在奇数长度环时，该图是二分图。该算法运行在有向存活图的**无向投影**上（每条有向边都被视为双向边），使用 BFS 二染色实现。这补全了始于 V2.32（环检测）的图结构分析五件套。

操作系统类比：检查一个服务依赖图能否被干净地拆分为生产者/消费者两层，或者一个模块加载顺序中是否存在奇数长度的循环依赖从而阻碍层级的干净分离。

---

## 修改内容

### 1. `graph_bipartite_inner<N>` — gos-runtime (`crates/gos-runtime/src/lib.rs`)

`GraphState` 上新增的方法（插入在 `graph_reachable_inner` 之后）：

```rust
pub fn graph_bipartite_inner<const N: usize>(
    &self,
) -> ([VectorAddress; N], [u8; N], usize, bool)
```

**算法**：在存活有向图的无向投影上进行 BFS 二染色。时间复杂度 O(V+E)，no_std 安全，使用固定大小的栈数组（无堆分配）。

**不变式**：
- 每条有向边都被当作无向边处理（从每个节点出发，同时探索 `from→to` 和 `to→from` 两个方向的邻居）。
- 自环会被跳过。
- 每个连通分量独立播种（可处理不连通的图）。
- 当发现冲突（相邻节点同色）时，会设置 `is_bipartite = false`，但 BFS 会继续进行 —— 因此无论结果如何，`total` 始终是正确的。

**返回值布局**（与 `graph_scc` / `graph_condensation` 的约定保持一致）：
- `vecs[0..total]`   — 按槽位顺序排列的存活节点向量。
- `colors[0..total]` — 0 = 集合 A，1 = 集合 B（仅当 is_bipartite 为真时有意义）。
- `total`            — 已打包的存活节点数量。
- `is_bipartite`     — 当且仅当该图存在合法二染色方案时为 true。

### 2. `pub fn graph_bipartite<const N>()` — gos-runtime 公开 API

```rust
pub fn graph_bipartite<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, bool) {
    RUNTIME.lock().graph_bipartite_inner()
}
```

与 `graph_scc`、`graph_condensation`、`graph_reachable` 风格一致的单行包装函数。

### 3. `dispatch_graph_bipartite` — k-shell (`crates/k-shell/src/lib.rs`)

新增的 shell 层调度函数（插入在 `dispatch_uname` 之前）。

输出格式：
```
 graph bipartite
 ───────────────────────────────────────────────────────────
  result:   bipartite
  set A (3):  15.0.0.1  15.0.0.3  15.0.0.5
  set B (2):  15.0.0.2  15.0.0.4
 ───────────────────────────────────────────────────────────
  5 node(s) checked
```

```
 graph bipartite
 ───────────────────────────────────────────────────────────
  result:   NOT bipartite  (odd-length cycle detected)
  hint: use 'graph cycles' to find the cycle, 'graph scc' for components
 ───────────────────────────────────────────────────────────
  3 node(s) checked
```

纯读取操作 —— 不产生 epoch 递增，不涉及写操作。

### 4. 命令路由 — k-shell (`crates/k-shell/src/proc.rs`)

别名接在 `graph condensation` 分支之后：

```
graph bipartite  |  bipartite  |  graph bip  |  bip
```

四个别名遵循与之前 graph 命令相同的短别名模式。

### 5. `gos-graph-bipartite-harness` — 新建 host-test crate

`host-tests/gos-graph-bipartite-harness/` —— 10 个集成测试，覆盖：

| # | 场景 | 预期结果 |
|---|----------|----------|
| 1 | 空图 | 二分图（空图恒真） |
| 2 | 单个孤立节点 | 二分图 |
| 3 | 单条边 A→B | 二分图 |
| 4 | 路径 A→B→C | 二分图 |
| 5 | 三角形 A→B→C→A | 非二分图（奇数环，长度 3） |
| 6 | 4 元环 A→B→C→D→A | 二分图 |
| 7 | 4 元环 + 弦 A→C | 非二分图（3 元环） |
| 8 | 两个不连通的二分图分量 | 二分图 |
| 9 | 星形图 K₁,₄：centre→{A,B,C,D} | 二分图 |
| 10 | 路径 A→B→C 的染色分配 | A、C 同一集合；B 属于另一集合 |

全部 10 个测试：**通过**（在 harness 目录下执行 `cargo +nightly test`）。

---

## 测试结果

```
running 10 tests
test color_assignment_correct_for_path ... ok
test disconnected_bipartite_components_are_bipartite ... ok
test empty_graph_is_bipartite ... ok
test four_cycle_is_bipartite ... ok
test four_cycle_with_chord_is_not_bipartite ... ok
test path_three_nodes_is_bipartite ... ok
test single_edge_is_bipartite ... ok
test single_node_is_bipartite ... ok
test star_graph_is_bipartite ... ok
test triangle_is_not_bipartite ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

---

## Shell 命令一览（V2.37 新增）

| 命令 | 别名 | 说明 |
|---------|---------|-------------|
| `graph bipartite` | `bipartite`, `graph bip`, `bip` | 二染色检测 —— 该图是否为二分图？是则显示集合 A / 集合 B，否则给出奇数环提示。 |

---

## 保持的不变式

- `dispatch_graph_bipartite` 是纯读取操作 —— 不产生 epoch 递增，不涉及写操作。
- 使用既有的 `TEST_LOCK: Mutex<()>` + `reset()` 隔离模式。
- Harness 的 `.cargo/config.toml` 设置 `target = "x86_64-pc-windows-msvc"` + `build-std = ["std", "panic_abort"]`。
- 版本号：V2.37（紧接在 V2.36 graph-reachable 之后）。

---

## 下一步计划

- `graph degree` / `graph centrality` —— 每个节点的入/出度、枢纽节点识别
- `node checkpoint <vec>` —— 将节点状态快照到 diff ring
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- PAL_U32 → attribute node 重构（Demo A 前置条件）
