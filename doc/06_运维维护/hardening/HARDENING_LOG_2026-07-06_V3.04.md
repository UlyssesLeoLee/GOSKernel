# GOSKernel 硬化日志 — V3.04
**日期：** 2026-07-06
**算法：** 弦图识别 — LexBFS + PEO 验证
**分支：** feat/vk-auto-live-surface
**提交：** feat(v3.04): chordal graph recognition -- LexBFS PEO + gos-graph-chordal-harness (10 tests)

---

## 摘要

V3.04 为 GOSKernel 图论运行时新增**弦图（chordal graph）识别**能力。

若一个图中所有长度 ≥ 4 的环都存在**弦**——即连接该环上两个不相邻顶点的边——则称该图为**弦图**（也叫*三角化图*）。等价地，一个图是弦图当且仅当它存在一个**完美消去序（Perfect Elimination Ordering, PEO）**：一个排序 v₁, v₂, …, vₙ，使得对每个 vᵢ，其在 {v₁, …, vᵢ₋₁}（已消去顶点）中的邻居构成一个团（clique）。

弦图是组合数学与计算机科学的一个丰富交汇点。许多 NP-难的图问题（色数、最大团、最大独立集、顶点覆盖）恰好在弦图上可以多项式时间求解（借助基于 PEO 的算法）。弦图还刻画了高斯消元无填充（no fill-in）的情形，并在概率图模型中作为支持精确联结树（junction-tree）推断的图类出现。

---

## 公开 API

### `gos_runtime::graph_chordal<const N: usize>() -> ([VectorAddress; N], bool, usize)`

返回 `(peo_vecs, is_chordal, node_count)`：
- `peo_vecs[0..node_count]` —— 按 LexBFS 完美消去序排列的节点
- `is_chordal` —— 若图为弦图（PEO 有效）则为 true
- `node_count` —— 存活节点总数

**无向投影：** A→B 与 B→A 合计视为一条无向边。
**自环：** 从邻接结构中排除。
**边界情形：** 空图与节点数 ≤ 2 的图恒为弦图。

---

## 算法

### 第一阶段：LexBFS —— 字典序广度优先搜索

算法出自 Rose, Tarjan & Lueker (1976)，复杂度 O(V+E)：

```
label[ci] ← 0  对所有紧凑索引 ci ∈ 0..n
for pos in 0..n:
    best_ci ← argmax_{未编号 ci} label[ci]
    peo[pos] ← best_ci;  pos_of[best_ci] ← pos
    对 best_ci 的每个未编号邻居 nci：
        label[nci] |= 1u128 << pos
```

标签以 u128 位掩码存储，若某节点与编号为 `pos` 的节点相邻，则该节点标签的第 `pos` 位被置位。按 u128 数值比较标签可正确实现字典序最大：与更晚编号的邻居相邻的节点在打破平局时优先（高位 = 高优先级）。

### 第二阶段：PEO 验证 —— Fulkerson & Gross (1965)

对 PEO 中位置为 `pos` 的每个顶点 v：
- **N⁺(v)** = v 在 v **之前**编号的邻居（pos_of < pos）
- **N⁺(v)** 必须构成一个团
- 每个顶点可在 O(1) 内高效验证：设 **w** 为 N⁺(v) 中 pos_of **最大**（最近编号）的成员。则 N⁺(v)\{w} ⊆ N(w) 是充要条件。

关键洞察：由于 LexBFS 对弦图能给出有效的 PEO，PEO 检验成功当且仅当该图是弦图。

### 正确性要点：PEO 方向

**N⁺(v) = 更早编号的邻居**，而非更晚编号。在 LexBFS 中，pos=0 是最先编号的（类似原始反向编号约定中的第 n 步）。PEO 不变量为：

> 每个 vᵢ 在 {v₁, …, vᵢ}（自身及已消去顶点）诱导的子图中都是单纯的（simplicial）

因此 N⁺(v) = {pos_of 更小的邻居}，w = N⁺(v) 中 pos_of 最大者。

这是实现中常见的错误来源：PEO 的方向与"正向"排序直觉可能相反。

---

## 关键不变量与测试用例

| 图 | is_chordal | 原因 |
|-------|-----------|--------|
| 空图 | true | 平凡成立——无环 |
| 单节点 | true | 无环 |
| K₂ | true | 无 4+ 环 |
| K₃（三角形） | true | 仅有 3-环；不存在 4+ 环 |
| C₄（无弦 4-环） | **false** | 4-环没有弦 |
| C₄ + 弦 A–C | true | 弦将其分割为两个三角形 |
| K₄（完全 4 图） | true | 所有节点对均相邻——所有环都有弦 |
| C₅（无弦 5-环） | **false** | 5-环没有弦 |
| 路径 P₅（树） | true | 树无环 |
| K₅（完全 5 图） | true | 所有节点对均相邻——平凡为弦图 |

---

## Shell 命令

```
graph chordal          # 完整弦图检验 + PEO 显示
gchordal               # 简称别名
chordal                # 简称别名
graph chord            # 简称别名
gchord                 # 简称别名
```

**显示：** 标题为亮青色；`✓ chordal` 为亮绿色 / `✗ not chordal` 为亮红色；PEO 表格为亮青色（弦图）或亮洋红色（非弦图）；页脚显示节点数、判定结果、算法出处。

---

## VectorAddress 命名空间

- `gos-graph-chordal-harness` 对应 `L4=80`

---

## 操作系统类比

**弦依赖图**意味着内核各子系统存在一个**完美消去序**：可以逐一按顺序将子系统上线，使得每个新上线子系统的所有已激活依赖两两都能互操作。这类似于一个干净的 systemd 排序，其中每个"启动组"都是一个团——不存在需要特殊处理的隐藏循环前置依赖。

**非弦**内核依赖图则包含一个由 4 个或更多子系统构成的"依赖环"，其中任意两个不相邻的成员之间都没有直接依赖，这使得干净的隔离更加困难（无法在不留下悬空依赖的情况下从一个非团集合中移除任意单个子系统）。

弦图还对应于可通过高斯消元实现**零填充**分解的内核结构——这可直接应用于稀疏内核调度问题（例如具有稀疏依赖矩阵的 systemd 单元排序）。

---

## Harness：`gos-graph-chordal-harness`（10 项测试）

| # | 图 | 期望结果 |
|---|-------|----------|
| 1 | 空图 | is_chordal=true, node_count=0 |
| 2 | 单节点 A | is_chordal=true, PEO=[A] |
| 3 | K₂（A–B 双向） | is_chordal=true |
| 4 | K₃（三角形） | is_chordal=true |
| 5 | C₄（4-环，无弦） | is_chordal=false |
| 6 | C₄ + 弦 A–C | is_chordal=true |
| 7 | K₄（完全 4 节点） | is_chordal=true |
| 8 | C₅（5-环，无弦） | is_chordal=false |
| 9 | 路径 P₅（A–B–C–D–E） | is_chordal=true |
| 10 | K₅（完全 5 节点） | is_chordal=true |

全部 10 项测试通过。宿主测试套件总计：**1013 个测试**。

---

## GOSKernel 中的互补算法

| 算法 | 版本 | 关系 |
|-----------|---------|-------------|
| graph_clique (BK) | V2.95 | 在弦图中，ω(G) = PEO 最大团规模（多项式可解） |
| graph_independent_set | V2.96 | 在弦图中，α(G) = n − ν(G)（经 König 定理多项式可解） |
| graph_vertex_cover | V2.97 | 在弦图中，τ(G) = ν(G) 精确成立（König 定理，多项式可解） |
| graph_color | V2.37 | 在弦图中，χ(G) = ω(G)（完美图定理，多项式可解） |
| graph_kcore | V2.64 | k-核分解（与团结构相关） |
| graph_truss | V2.94 | k-truss 分解（基于三角形的密度指标） |

---

## 参考文献

- Rose, Tarjan & Lueker (1976) — "Algorithmic Aspects of Vertex Elimination on Graphs"（LexBFS，O(V+E) 识别算法）
- Fulkerson & Gross (1965) — "Incidence matrices and interval graphs"（PEO 刻画）
- Gavril (1974) — "The intersection graphs of subtrees of a tree are exactly the chordal graphs"
- Golumbic (1980) — "Algorithmic Graph Theory and Perfect Graphs"（第 4 章，完整论述）
- Blair & Peyton (1993) — "Introduction to chordal graphs and clique trees"（与高斯消元的关联）

---

*本文件于 2026-07-15 按文档管理规范就地中文化，仅译语言，不改动版本号 / 文件路径 / 函数名 / 测试名 / 测试计数等既成事实。*
