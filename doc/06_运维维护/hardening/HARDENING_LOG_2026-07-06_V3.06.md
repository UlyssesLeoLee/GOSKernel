# GOSKernel 硬化日志 — V3.06
**日期：** 2026-07-06  
**算法：** 边介数中心性 — Brandes（2001）  
**分支：** feat/vk-auto-live-surface  
**提交：** feat(v3.06): edge betweenness centrality -- Brandes edge-EBC + gos-graph-ebc-harness (10 tests)

---

## 变更摘要

V3.06 为 GOSKernel 图论运行时新增了**边介数中心性（Edge Betweenness Centrality, EBC）**。这补全了介数家族：

| 版本 | 算法 | 输出 |
|---------|-----------|--------|
| V2.52   | 节点介数中心性 | 哪些节点位于最多的最短路径上？ |
| **V3.06** | **边介数中心性** | **哪些链路承载了最多的最短路径流量？** |

边介数回答的是与节点介数互补的问题：给定内核的有向通信图，**哪些有向链路是最关键的传导通道**？高介数的边是路由中的单点故障——类似于真实操作系统中流量繁重的网络接口或总线。

---

## 公开 API

### `gos_runtime::graph_betweenness_edge<const N: usize>() -> ([VectorAddress; N], [VectorAddress; N], [u32; N], usize)`

返回 `(from_vecs, to_vecs, scores, edge_count)`：
- `from_vecs[0..edge_count]` — 每条边的源端点。
- `to_vecs[0..edge_count]` — 每条边的目标端点。
- `scores[0..edge_count]` — 介数计数：有多少有序 (s, t) 对的**唯一**最短路径经过这条边（通过 SCALE=1_000_000 的整数运算实现）。
- `edge_count` — 存活、非自环的有向边数量。

**输出排序：** 按 score 降序；分数相同时按 (from.as_u64(), to.as_u64()) 升序排列。

**自环：** 排除。  
**方向性：** 完全有向——边 (u→v) 与边 (v→u) 被独立处理。  
**权重：** 使用边的 `weight` 字段（Dijkstra）；标准注册中所有权重默认为 1.0。  
**并行路径：** 当存在多条等长最短路径时（例如菱形图），介数会按路径数量通过 Brandes 的 sigma 比例公式按比例分摊。整数除法意味着分数取 `floor(betweenness)`。

---

## 算法

### Brandes（2001）Dijkstra + 反向传播，O(V * (V + E))

对每个源节点 s：
1. **Dijkstra 阶段**：计算 `dist[v]` 和 `sigma[v]`（从 s 到 v 的最短路径数量）。
2. **反向传播阶段**（按逆向提取顺序）：
   - 对每个节点 w（按逆向 BFS/Dijkstra 顺序，叶节点优先）：
     - 对每条位于最短路径上的入边 (v→w)（即 dist[v] + w ≈ dist[w]）：
       ```
       contribution = sigma[v] × (SCALE + delta[w]) / sigma[w]
       delta[v]      += contribution      ← feeds into v's node betweenness
       edge_bet[v→w] += contribution      ← edge betweenness accumulation
       ```

核心洞察：**为 `delta[v]` 计算出的贡献值，恰好就是边 (v→w) 的边介数贡献值**。两种累加共享同一个公式，只需一次乘除运算即可同时算出。

**为什么要用 SCALE 因子？** Brandes 的公式是 `sigma[v]/sigma[w] * (1 + delta[w])`。乘以 SCALE = 1_000_000 后转换为整数运算：`sigma[v] * (SCALE + delta[w]) / sigma[w]`。最终分数 = `edge_bet[ei] / SCALE`（向下取整除法）。

---

## 与节点介数（`graph_between`）的区别

| 方面 | `graph_between`（V2.52） | `graph_betweenness_edge`（V3.06） |
|--------|------------------------|----------------------------------|
| 输出单位 | 每节点 | 每有向边 |
| 累加目标 | `bc_scaled[w]`（节点） | `edge_bet[ei]`（边槽位） |
| 输出数组 | `(vecs, scores, node_count)` | `(from_vecs, to_vecs, scores, edge_count)` |
| 排序键 | 按分数降序 | 按分数降序，再按 (from, to) 升序 |
| 泛型 N | MAX_NODES=128 | MAX_EDGES=512 |

内部反向传播循环在结构上完全相同。新增的 `edge_bet[ei] += contribution` 是在既有的 `delta[v]` 计算上顺带完成的，不增加额外的算法开销。

---

## 关键不变量

| 图 | 边 | 分数 |
|-------|------|-------|
| 单条边 A→B | A→B | 1 |
| 路径 A→B→C | A→B | 2 |
| 路径 A→B→C | B→C | 2 |
| 路径 A→B→C→D | A→B | 3 |
| 路径 A→B→C→D | B→C | **4**（最高） |
| 路径 A→B→C→D | C→D | 3 |
| 菱形 A→{B,C}→D | 全部 4 条边 | 1（均分，向下取整） |
| 有向三角形 A→B, B→C, A→C | 全部 3 条边 | 1 |
| 出射星形 A→{B,C,D} | 全部 3 条边 | 1 |

**路径图规律：** 在一个 n 节点的有向路径中，位于第 k 位（从起点 0 开始索引）的边承载了来自 ≤k-1 位源点的 k*(n-1-k) 条路径对，加上内部路径对。中间的边总是分数最高的。

---

## Shell 接口

| 命令 | 别名 |
|---------|---------|
| `graph ebc` | `gebc`, `edge between`, `edge betweenness`, `ebc` |

**显示效果：** 亮黄色标题；每条边按 6 种颜色循环（10→11→13→9→14→15）；右对齐的分数，源向量 → 目标向量（Unicode → 箭头 U+2192）；页脚显示有向边总数和 "Brandes 2001" 出处标注。

---

## VectorAddress 命名空间

`gos-graph-ebc-harness` 对应 L4=82

---

## 操作系统类比

边介数是内核依赖图的**链路关键性指标**。EBC 高的有向边是**单车道瓶颈**——大多数子系统间最短路径都流经它。移除或限流这条链路对可达性的破坏最大。

**用例 1 —— 总线饱和检测：** 分数超过阈值的边是链路复制或负载均衡的候选对象（类似于对繁忙网卡使用 `ethtool -S` + LACP 绑定）。

**用例 2 —— 故障注入目标：** EBC 最高的边是混沌工程测试中最具影响力的目标（类似于对关键网络路径使用 `tc qdisc add netem delay`）。

**用例 3 —— 安全检查点：** 高 EBC 的边是插入参考监视器的天然 IPC 检查点（类似于对高频系统调用路径设置 LSM 钩子）。

边介数分解与 V3.05 的双连通分量、V2.52 的节点介数相结合，构成完整的**故障拓扑三元组**：
- BCC：移除哪个节点会使图碎片化？
- 节点介数：哪些节点承载最多的路径对？
- 边介数：哪些*链路*承载最多的路径对？

---

## 测试套件（gos-graph-ebc-harness，10 个测试，全部通过）

| # | 图 | 期望结果 |
|---|-------|----------|
| 1 | 空图 | edge_count=0 |
| 2 | 单个孤立节点 | edge_count=0 |
| 3 | 单条边 A→B | score(A→B)=1 |
| 4 | 路径 A→B→C | score(A→B)=score(B→C)=2 |
| 5 | 路径 A→B→C→D | score(A→B)=3, score(B→C)=4, score(C→D)=3 |
| 6 | 菱形 A→B, A→C, B→D, C→D | 全部 4 条边 score=1 |
| 7 | 有向三角形 A→B, B→C, A→C | 全部 3 条边 score=1 |
| 8 | 出射星形 A→{B,C,D} | 全部 3 条边 score=1 |
| 9 | 不连通 A→B ∥ C→D | 两条边 score 均为 1 |
| 10 | 路径 A→B→C→D：最高分边在前 | scores[0]=4, from=B, to=C；输出非递增 |

---

## 参考文献

- Brandes, U. (2001). *A faster algorithm for betweenness centrality.* Journal of Mathematical
  Sociology, 25(2), 163–177.
- Girvan, M. & Newman, M. E. J. (2002). *Community structure in social and biological networks.*
  PNAS, 99(12), 7821–7826.（边介数首次应用于社区发现。）

---

## 宿主测试累计数量

**1033 个测试**（V3.05 累计 1023 个 + 新增 10 个 EBC 测试）
