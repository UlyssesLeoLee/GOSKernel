# GOSKernel 硬化日志 — V3.05
**日期：** 2026-07-06  
**算法：** 双连通分量 — Tarjan 迭代边栈 BCC  
**分支：** feat/vk-auto-live-surface  
**提交：** feat(v3.05): biconnected components -- Tarjan edge-stack BCC + gos-graph-bcc-harness (10 tests)

---

## 变更摘要

V3.05 为 GOSKernel 图论运行时新增了**双连通分量（Biconnected Components, BCC）**。这补全了始于 V2.85 的连通性三部曲：

| 版本 | 算法 | 输出 |
|---------|-----------|--------|
| V2.85   | 割点（articulation points） | 移除哪些节点会使图断连？ |
| V2.86   | 桥（割边，bridges）               | 移除哪些边会使图断连？ |
| V2.93   | 2-边连通分量       | 对任意单条边移除都具有韧性的最大子图 |
| **V3.05** | **双连通分量**      | **对任意单个顶点移除都具有韧性的最大子图** |

**双连通分量（BCC）**是一个最大 2-顶点连通子图：对于 BCC 内任意一对顶点 u、v，二者之间至少存在两条顶点不相交的路径。等价地说，从 BCC 内部移除任意单个顶点都不会使其断连。

---

## 公开 API

### `gos_runtime::graph_bcc<const N: usize>() -> ([VectorAddress; N], [u8; N], usize, usize)`

返回 `(vecs, bcc_ids, node_count, bcc_count)`：
- `vecs[0..node_count]` — 所有存活节点，按 `(bcc_id, vec.as_u64())` 升序排列。
- `bcc_ids[0..node_count]` — 每个节点所属的 BCC 索引：
  - 普通 BCC 成员：其 BCC 索引（从 0 开始）。
  - **割点**（属于 2 个及以上 BCC 的节点）：标记为 `255`。
  - 孤立节点（无无向边）：各自被赋予独立的单节点 BCC。
- `node_count` — 存活节点总数。
- `bcc_count` — 双连通分量总数（边-BCC 数 + 孤立单节点 BCC 数）。

**无向投影：** A→B 与 B→A 合计视为一条无向边。  
**自环：** 排除在 BCC 分析之外。  
**割点：** `graph_bcc` 输出中 bcc_id=255 的顶点，与 `graph_articulation`（V2.85）返回的顶点完全一致——已由测试 10 的交叉校验验证。

---

## 算法

### Tarjan 迭代边栈 BCC，O(V+E)

该算法是 Tarjan 双连通分量算法的迭代式 DFS 变体。它维护一个**边栈**，并利用 `disc/low` 链接值来识别 BCC 边界。

**状态数组（以紧凑索引 ci ∈ 0..nc 为下标）：**
- `disc[ci]` — DFS 发现时刻（初始为 UNVISITED = u32::MAX）
- `low[ci]` — 从 ci 子树经由回边可达的最小 disc 值
- `par[ci]` — DFS 父节点的紧凑索引（NIL = 根节点）
- `bcc_primary[ci]` — 分配给 ci 的第一个 BCC id（255 = 未分配）
- `bcc_mult[ci]` — 若 ci 出现在 2 个及以上 BCC 中（即是割点）则为 true

**边栈：** `[(u8, u8); MAX_EDGES]` —— 存储 DFS 过程中发现的每条无向边的 `(ci_u, ci_v)`。

**树边：** 子节点首次被发现时压入边栈。

**回边：** 仅当 `disc[nbr] < disc[cur]`（nbr 是祖先）时才压入边栈，确保每条无向回边只会从后代一侧被压入一次。

**BCC 判定条件：** 完成节点 `cur`（其父为 `p`）时：
```
if low[cur] >= disc[p]:
    bid = bcc_count; bcc_count += 1
    pop edge_stack until (p, cur) is popped:
        assign bid to all vertices in popped edges
```

条件 `low[cur] >= disc[par]` 意味着 cur 子树中没有任何回边能到达严格高于 par 的位置——par 是一个 BCC 边界（割点或 DFS 根）。

**孤立节点**（DFS 后 bcc_primary 仍为 255 的节点）：各自被赋予一个新的单节点 BCC id。

**父节点跟踪：** 采用基于槽位的方式（跳过所有指向父槽位的边），与 `graph_articulation` 的风格一致。这能正确处理反平行的有向边对（A→B + B→A 视为一条无向边），且不会重复压栈。

---

## 关键不变量

| 场景 | bcc_count | 割点（bcc_id=255） |
|----------|-----------|-----------------|
| 空图 | 0 | — |
| 单个孤立节点 | 1 | 无 |
| 路径 Pₙ（n 个节点） | n−1 | n−2 个内部节点 |
| 三角形 K₃ | 1 | 无（双连通） |
| K₄ | 1 | 无（3-连通） |
| 沙漏形（两个三角形共享顶点 C） | 2 | C |
| 星形 K₁,₄（中心 B，4 个叶子） | 4 | B |

**割点身份一致性：** `graph_bcc` 中 `bcc_id=255` 的节点，与 `graph_articulation` 返回的割点完全一致（测试 10 中已验证）。

**BCC 数与桥数的关系：** 对于有 n 个节点的树（n−1 条桥），bcc_count = n−1（每条桥对应一个边-BCC）。对于双连通图，bcc_count = 1。

---

## Shell 接口

| 命令 | 别名 |
|---------|---------|
| `graph bcc` | `gbcc`, `biconnected`, `gbiconn`, `bcc` |

**显示效果：** 亮黄色标题；每个节点显示 BCC id 与"BCC 成员"角色（6 种颜色循环）；割点以亮红色显示为 `AP  cut-vertex`；页脚显示节点数、BCC 数、割点数以及 "Tarjan 1972" 出处标注。

---

## VectorAddress 命名空间

`gos-graph-bcc-harness` 对应 L4=81

---

## 操作系统类比

双连通分量是内核依赖图的**故障隔离"区块"**。在单个 BCC 内，任何子系统都可以崩溃而不破坏该区块内部的连通性——就像 RAID 阵列中任意单块磁盘故障不影响整个阵列的完整性。

割点（bcc_id=255）是**单点故障**：移除后会将依赖图分割为互不连通的多个部分。这些是最高优先级的冗余改造目标——类似于通过 `systemd-analyze critical-chain` 识别出的 `systemctl mask` 候选对象。

BCC 分解构建出**块-割树（block-cut tree）**：树中的节点是 BCC（矩形）和割点（圆形），通过包含关系相连。这是描述图的故障拓扑组织方式的经典表示法。

---

## 测试套件（gos-graph-bcc-harness，10 个测试，全部通过）

| # | 图 | 期望 bcc_count | 期望割点 |
|---|-------|--------------------|--------------|
| 1 | 空图 | 0 | — |
| 2 | 单个孤立节点 | 1 | 无 |
| 3 | 两个孤立节点 | 2 | 无 |
| 4 | 单条边 A-B | 1 | 无 |
| 5 | 路径 A-B-C（2 条桥） | 2 | B |
| 6 | 三角形 K₃ | 1 | 无 |
| 7 | K₄ | 1 | 无 |
| 8 | 沙漏形（两个三角形共享 C） | 2 | C |
| 9 | 星形 K₁,₄（中心 B） | 4 | B |
| 10 | 交叉校验：BCC 割点 = graph_articulation 割点（路径 A-B-C-D） | 3 | B, C |

---

## 参考文献

- Tarjan, R. E. (1972). *Depth-first search and linear graph algorithms.* SIAM Journal on
  Computing, 1(2), 146–160.
- Hopcroft, J. & Tarjan, R. E. (1973). *Algorithm 447: Efficient algorithms for graph
  manipulation.* Communications of the ACM, 16(6), 372–378.

---

## 宿主测试累计数量

**1023 个测试**（V3.04 累计 1013 个 + 新增 10 个 BCC 测试）
