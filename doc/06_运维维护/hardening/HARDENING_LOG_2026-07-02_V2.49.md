# GOS 硬化日志 — V2.49（2026-07-02）

## 版本号: V2.49
## 功能: `graph shortest` — Dijkstra 单源最短路径

---

## 变更摘要

新增 `graph shortest <vec>` —— 在**有向**的 GOS 内核活跃图上，从指定源节点执行 Dijkstra 单源最短路径树（SPT）计算。与生成树、MST 算法（均把边视为无向）不同，Dijkstra 遵循边的方向，是 GOS 工具集中第一个有向带权路径分析原语。

Shell 别名：`graph shortest <vec>` / `shortest <vec>` / `graph dijkstra <vec>` / `dijkstra <vec>`

OS 类比：`ip route get <dst>` —— 以边权重作为路由度量，从一个内核子系统到所有可达对等节点的最小延迟有向路径。

本次完成了**带权图分析三部曲**：
- **V2.48 MST** —— 无向最小成本生成森林（结构骨架）
- **V2.49 SPT** —— 从单一源出发的有向最小成本路径树（路由表）
- 结合 V2.47（着色）、V2.46（生成树）、V2.45（社区），为运维人员提供了结构性、成本感知与方向性兼备的内核图分析完整工具集。

---

## 动机

V2.48 的 MST 基础设施（`GraphTopologySnapshot` 中的 `edge_weight`）为有向带权路径算法打开了大门。Dijkstra 是经典的单源最短路径算法，回答关键的 OS 观测性问题：

- **哪些内核子系统能被信号 X 到达？**（带成本的有向可达性）
- **从调度器到内存管理器的最小延迟路径是什么？**（路径成本）
- **哪些子系统从引导节点不可达？**（分区图检测）
- **是否存在所有最短路径都汇聚的瓶颈节点？**（SPT 结构）

与忽略权重、使用无向边的 BFS 生成树（V2.46）不同，Dijkstra 的 SPT 为运维人员提供了真实有向内核图中信号路由成本的精确图景。

---

## 算法：Dijkstra 单源最短路径（有向）

```text
初始化：
  visited[v]  = false，对所有 v
  dist[v]     = ∞，对所有 v
  parent[v]   = ∅，对所有 v

按 VectorAddress 定位源 slot：
  若未找到源：返回所有节点 dist=u32::MAX，无 SPT

dist[source] = 0.0
parent[source] = source

重复 n 次：
  u = argmin{ dist[v] : v 未访问 且 dist[v] < ∞ }
  若无满足条件的节点（其余节点均不可达）：跳出

  visited[u] = true

  对每条有向出边 (u → v)，权重为 w：
    若 v 未访问 且 dist[u] + w < dist[v]：
      dist[v]   = dist[u] + w
      parent[v] = u

构建输出：
  源节点排在最前（vecs[0]=source, dists[0]=0, parents[0]=source）
  随后按 slot 顺序输出所有其他活跃节点：
    dist[v] = (dist_f[v] * 1000) as u32   若可达
    dist[v] = u32::MAX                     若不可达
    parents[v] = slot_vec[parent[v]]       若可达
    parents[v] = ZERO_VEC                  若不可达
```

**关键设计选择：**

1. **仅遵循有向边**：按注册方向（`edge_from → edge_to`）遍历。需要无向语义的调用方应使用 MST 或生成树函数。
2. **贪心提取（无优先队列）**：与 Prim MST 相同的 O(V·E) 模式——O(V) 次外层迭代 × O(E) 松弛扫描。n≤128、E≤512 时最多 65,536 次操作，完全在 no_std 预算内。
3. **u32::MAX 作为不可达哨兵值**：无歧义（没有任何有效距离能达到 4,294,967）。Shell 展示层对不可达节点显示 `∞`。
4. **不可达节点的父节点为 ZERO_VEC**：无需额外标志数组即可区分"无父节点"与"父节点是某节点"。
5. **源节点恒排第一**：简化 Shell 渲染与调用方逻辑——找到源节点时 `vecs[0]` 恒为源。
6. **未知源节点**：若给定 VectorAddress 不匹配任何活跃节点，返回全部节点 `dist=u32::MAX`，不构建 SPT，Shell 对所有节点显示 `∞`。

**复杂度**：O(V·E) —— O(V) 外层 × O(E) 边扫描内层循环。
**空间**：O(MAX_NODES) —— `visited`、`dist`、`parent` 数组，no_std/no_alloc 安全。

---

## 实现细节

### `crates/gos-runtime/src/lib.rs`

**新内部函数：**
- **`RuntimeState::graph_shortest_inner<const N>(snap, source: VectorAddress)`** —— Dijkstra SPT：通过线性扫描按 VectorAddress 找到源 slot；外层循环 V 次迭代，每次 O(V) 查找最小距离未访问节点 + O(E) 松弛；只松弛有向出边（`snap.edge_from[ei] == u_id`）；打包输出（源第一，其余按快照顺序）

**新公开函数：**
```rust
pub fn graph_shortest<const N: usize>(
    source: VectorAddress,
) -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize)
```
锁定 RUNTIME，调用 `topology_snapshot()`，委托给 `graph_shortest_inner`。

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_shortest(sink, source: VectorAddress)`** —— 展示：
  - 标题：青色 `graph shortest [src_vec]`
  - 列标题：`status  dist  vector  parent`
  - 逐节点显示状态（洋红 `source` / 绿色 `reach` / 暗色 `∞`）、黄色距离 `D.mmm`、白色向量、父节点（或 `(source)` / `(unreachable)`）
  - 页脚：`N node(s)  Dijkstra SPT from [src]  reachable: R`

### `crates/k-shell/src/proc.rs`

- 路由（带向量解析）：`"graph shortest <v>" | "shortest <v>" | "graph dijkstra <v>" | "dijkstra <v>"` → `VectorAddress::parse(v)` → `dispatch_graph_shortest(sink, src)`
- 帮助文本新增2行；无效向量显示红色错误：`graph shortest: invalid vector (e.g. 1.0.0.1)`

---

## 测试用例（10/10 通过）：`host-tests/gos-graph-shortest-harness`

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | node_count=0 |
| 2 | 单节点，source=自身 | dist=0, parent=self，源排第一 |
| 3 | 未知源（无匹配） | 全部 dist=u32::MAX |
| 4 | K₂ A→B 权重=1.0，source=A | B dist=1000, B parent=A |
| 5 | K₂ A→B 权重=2.5，source=A | B dist=2500 |
| 6 | 路径 A→B(1)→C(2)，source=A | C dist=3000, C parent=B |
| 7 | 仅 A→B，source=B | A dist=u32::MAX（无 B→A 边） |
| 8 | 菱形 A→B(1)→D(1)，A→C(2)→D(2) | D dist=2000 经由 B，D parent=B |
| 9 | A→B 连通；C 孤立，source=A | C dist=u32::MAX |
| 10 | 源节点父不变量 | parents[0]==vecs[0]==source |

**结果：10/10 通过，零告警**

---

## Shell 命令一览

```text
graph shortest <vec>   从节点 <vec> 出发的 Dijkstra SPT（有向、带权）
shortest <vec>         别名
graph dijkstra <vec>   别名
dijkstra <vec>         别名
```

示例输出（路径 A→B(1)→C(2)）：

```text
 graph shortest [26:1:1:0]
 ─────────────────────────────────────────────────────────────
  status    dist      vector           parent
  source    0.000     [26:1:1:0]       (source)
  reach     1.000     [26:1:2:0]       [26:1:1:0]
  reach     3.000     [26:1:3:0]       [26:1:2:0]
 ─────────────────────────────────────────────────────────────
 3 node(s)  Dijkstra SPT from [26:1:1:0]  reachable: 2
```

含不可达节点的示例输出：

```text
 graph shortest [26:1:2:0]
 ─────────────────────────────────────────────────────────────
  status    dist      vector           parent
  source    0.000     [26:1:2:0]       (source)
  ∞         ∞         [26:1:1:0]       (unreachable)
 ─────────────────────────────────────────────────────────────
 2 node(s)  Dijkstra SPT from [26:1:2:0]  reachable: 0
```

---

## 带权算法套件完成情况（V2.47–V2.49）

| 版本 | 算法 | 方向 | 权重 | 输出 |
|------|------|------|------|------|
| V2.47 | Welsh-Powell 着色 | 无向 | 否 | 颜色索引 / 色度数 |
| V2.48 | Prim MST | 无向 | 是 | 生成森林 / 总成本 |
| V2.49 | Dijkstra SPT | **有向** | 是 | 路径树 / 距离 |

---

## 不变量确认

- [x] 纯读操作：`graph_shortest` 不推进 epoch，不做任何变更
- [x] 无堆分配 / no_std：所有缓冲区为固定大小栈数组
- [x] harness 使用标准的 `TEST_LOCK + reset()` 隔离方式
- [x] 版本顺序：V2.49 紧随 V2.48（MST）
- [x] 文档归档路径：`doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.49.md`

---

## 后续建议（V2.50 候选）

- `graph flow <from> <to>` —— 最大流（基于 BFS 的 Edmonds-Karp）
- `node checkpoint <vec>` —— 快照节点状态到 diff ring
- `graph sim <N>` —— 模拟 N 步随机游走，输出信号流量轨迹
- `graph between` —— 基于全对最短路径的介数中心性

---

*由自动强化任务生成 · 2026-07-02*
