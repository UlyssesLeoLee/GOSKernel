# 硬化日志 V2.91 —— 反馈弧集（DFS 三色标记）

**日期：** 2026-07-05
**分支：** feat/vk-auto-live-surface
**宿主测试总计：** 883（此前 873，+10 新增）

---

## 功能：`graph feedback arc` / `gfas` / `feedback arc` / `gcycledges`

### 动机

V2.88–V2.90 构建了 DAG 分析链：关键路径（V2.88）、拓扑分层（V2.89）、
支配树（V2.90）。V2.91 通过直接回答以下问题，完成了环分析的收尾：

> **"图中的环具体是由哪些有向边导致的？"**

**反馈弧集（feedback arc set，FAS）**是指一组边，移除它们即可使图变为无环图。
它是 DFS 中回边概念在有向图上的对应物。

| 问题 | 操作系统类比 |
|---|---|
| 是否存在环？ | 依赖图中是否存在循环启动依赖？ |
| 是哪些边导致的？ | 究竟是哪些 `requires`/`after` 链接造成了死锁？ |
| 需要移除多少条边？ | 恢复拓扑启动顺序所需的最小结构性改动 |

与运行时中已有的相关算法对比：

| 算法 | 发现的内容 |
|---|---|
| `graph_girth`（V2.69） | 最短有向环的长度 |
| `graph_dag_longest`（V2.88） | 若存在任何环则返回 `is_dag=false` |
| `graph_dag_layers`（V2.89） | 若存在任何环则返回 `is_dag=false` |
| `graph_domtree`（V2.90） | 从单一入口出发的最近支配节点 |
| `graph_feedback_arc`（V2.91） | **导致环存在的具体边** |

FAS 是可直接采取行动的输出：它准确指出了为恢复干净的启动顺序，
必须打破（或反转）哪些依赖关系。

---

## 算法：迭代 DFS 三色标记

### 为什么使用 DFS 回边

最小 FAS 问题对一般有向图而言是 NP-hard 的（Karp 1972）。
然而，基于 DFS 的 FAS 是一种标准的 O(V+E) 近似算法：
- 它始终是一个有效的 FAS（移除返回的弧后图必然无环）
- 实践中结果很紧凑（对稀疏图往往是最优或接近最优）
- 在边扫描顺序固定的情况下结果是确定的

### 三色 DFS

经典 DFS 为每个节点分配三种颜色之一：

| 颜色 | 含义 | 数组值 |
|---|---|---|
| `UNVISITED`（白色） | 尚未访问 | 0 |
| `IN_STACK`（灰色） | 处于当前 DFS 调用栈上 | 1 |
| `DONE`（黑色） | 已完全处理；所有后继均已探索 | 2 |

按目标节点颜色对边分类：

| 目标节点颜色 | 边类型 | 是否为反馈弧？ |
|---|---|---|
| `UNVISITED` | 树边 | 否 —— DFS 继续下探 |
| `IN_STACK` | **回边** | **是 —— 形成一个环** |
| `DONE` | 前向边/交叉边 | 否 —— 已处理过 |

### 自环

自环 `(u→u)` 会被自然捕获：处理节点 `u` 时，`u` 本身正处于 `IN_STACK` 状态。
自环的目标槽位等于 `u` 自身的槽位，颜色为 `IN_STACK` → 被记录为反馈弧。
无需特殊处理。

### 迭代实现

在 `no_std` 内核代码中递归并不安全。DFS 使用显式栈：

```
dfs_stack: [(slot, next_edge_index); MAX_NODES]
```

每个帧存储 `(current_slot, ei)` —— 从 `current_slot` 开始下一个待扫描的边索引。
发现树边时：
1. 将 `ei + 1` 保存为当前帧的恢复点
2. 以 `ei = 0` 压入邻居节点槽位

当没有未访问的邻居时：
1. 设置 `color[current_slot] = DONE`
2. 弹出该帧

### 非连通图

外层循环按紧凑顺序遍历所有 `node_slots[ki]`。若外层循环遍历到某个节点时
其状态仍为 `UNVISITED`，则从该节点开始新的一棵 DFS 树。
这确保每个连通分量都会被处理。

---

## API

```rust
pub fn graph_feedback_arc<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], usize, usize)
```

返回 `(from_vecs, to_vecs, arc_count, node_count)`：
- `from_vecs[0..arc_count]` / `to_vecs[0..arc_count]` —— 反馈弧集合
- `arc_count`  —— 找到的回边数量
- `node_count` —— 存活节点总数

输出按 `(from.as_u64(), to.as_u64())` 升序排序，以保证确定性。

N 应取 `MAX_EDGES`（512）以实现完整覆盖；k-shell 使用 `MAX_EDGES`。

---

## 关键不变量

| 不变量 | 说明 |
|---|---|
| 自环被记录为反馈弧 | 自环触发时 `cur_slot` 的颜色必然是 `IN_STACK` |
| 交叉边/前向边（DONE）不被记录 | 只有回边（IN_STACK）才算作弧 |
| 移除所有返回的弧可得到 DAG | 证明：不再存在 `IN_STACK` 边 → DFS 找不到回边 |
| `arc_count == 0` 当且仅当 `is_dag` | 在测试 10 中与 `graph_dag_layers` 交叉验证 |
| 输出按 `(from.as_u64(), to.as_u64())` 排序 | 多次运行结果一致 |
| 无需父节点追踪 | 与关节点/桥不同，FAS 只需三色状态 |

---

## Shell 命令

```
graph feedback arc   列出所有反馈弧（有向回边）
gfas                 别名
feedback arc         别名
gcycledges           别名
```

显示：
- 标题：`graph feedback arc`
- 若 `arc_count == 0`：绿色显示 "no feedback arcs (graph is a DAG)"
- 否则：以红色显示 `from → to` 对的表格
- 页脚：弧数、节点数、DAG 状态（`acyclic` 或 `cyclic (N arcs to remove)`）

---

## 测试套件：`gos-graph-fas-harness`（L4=67）

10 项测试覆盖：

| # | 场景 | 预期结果 |
|---|---|---|
| 1 | 空图 | arc_count=0, node_count=0 |
| 2 | 单节点，无边 | arc_count=0 |
| 3 | 自环 A→A | arc_count=1, arc=(A,A) |
| 4 | 二元环 A→B→A | arc_count=1 |
| 5 | DAG 链 A→B→C | arc_count=0 |
| 6 | 菱形 DAG A→{B,C}→D | arc_count=0 |
| 7 | 三角环 A→B→C→A | arc_count=1，回边为 C→A |
| 8 | 两个独立的二元环 | arc_count=2 |
| 9 | 非连通：DAG + 二元环 | arc_count=1（仅环所在分量） |
| 10 | 与 `graph_dag_layers` 交叉验证 | arc_count==0 当且仅当 is_dag==true |

10 项全部通过，零警告。

---

## VectorAddress 命名空间

L4=67 保留给 `gos-graph-fas-harness`。

---

## 参考文献

- Karp 1972 —— 最小反馈弧集的 NP 完全性（锦标赛图中的 MFAS）
- Cormen, Leiserson, Rivest & Stein —— *Introduction to Algorithms* §22.3（DFS 回边）
- Eades, Lin & Smyth 1993 —— 用于分层图绘制的贪心 FAS 启发式算法
- Tarjan 1976 —— *"Edge-disjoint spanning trees and depth-first search"*
