# 硬化日志 V2.92 —— 最大二分图匹配（Kuhn 算法）

**日期：** 2026-07-05
**分支：** feat/vk-auto-live-surface
**宿主测试总计：** 893（此前 883，+10 新增）

---

## 功能：`graph bipartite match` / `gbimatch` / `bipartite match`

### 动机

V2.37 增加了 `graph_bipartite`——二分性检测（该图是否是二分图？每个节点属于哪一侧？）。
V2.92 将其扩展为匹配问题：

> **"给定一个二分图，A–B 之间不相交配对的最大集合是多少？"**

**最大二分图匹配**解决了贯穿操作系统调度和资源分配的最优分配问题：

| 问题 | 操作系统类比 |
|---|---|
| 哪些任务可以绑定到哪些 CPU？ | `taskset` / `numactl --cpunodebind` 亲和图 |
| 有多少任务可以在不共享 CPU 的情况下并发运行？ | 最大独立调度槽位数 |
| 是否每个服务都能分配到唯一的网络接口？ | 网卡↔服务的独占绑定 |
| 哪些 IRQ 处理程序可以固定到不同的 CPU 核？ | `/proc/irq/N/smp_affinity` 分配 |

与运行时中已有的相关算法对比：

| 算法 | 发现的内容 |
|---|---|
| `graph_bipartite`（V2.37） | 该图是否为二分图？每个节点属于哪一侧？ |
| `graph_community`（V2.44） | 标签传播社区划分（非二分） |
| `graph_spanning`（V2.52） | 生成子图（所有节点，边的子集） |
| `graph_bipartite_match`（V2.92） | **最大匹配：最大的不相交 A↔B 配对集合** |

匹配数还给出了 König 定理的上界：在二分图中，最大匹配 = 最小顶点覆盖。

---

## 算法：Kuhn 增广路径 DFS

### 为什么使用增广路径

一个匹配 M 是最大匹配，当且仅当不存在**增广路径**——即一条起止均为
自由（未匹配）节点、并在非匹配边和匹配边之间交替的路径。沿增广路径
翻转边可使 |M| 增加 1。

Kuhn 算法（又称匈牙利 DFS 方法）对每个自由的 A 侧节点重复以下过程：

```
for each free A-node a:
    DFS to find alternating path from a to a free B-node
    if found: augment (flip all edges along path), match_count += 1
```

每次 DFS 复杂度为 O(E)。有 O(V) 个自由 A 节点，总复杂度为 **O(V·E)**。
在我们的容量约束下（V ≤ 128，E ≤ 512），最多为 65,536 次操作——
完全在实时预算范围内。

### 带显式路径追踪的迭代 DFS

在 `no_std` 内核代码中递归并不安全。DFS 使用以下结构：

```
dfs_stk:  [(a_slot, ei); MAX_NODES]   —— 显式 DFS 栈
chosen_b: [b_slot; MAX_NODES]         —— 每个 DFS 层级选中的 B 节点
visited_b: [bool; MAX_NODES]          —— 本次 DFS（针对某个自由 A 节点）中尝试过的 B 节点
```

**栈前进（找到已匹配的 B 节点）：**
1. 标记 b_slot 已访问；记录 `chosen_b[level] = b_slot`
2. 保存边扫描位置 `dfs_stk[level].1 = ei`（用于回溯恢复）
3. 在 level+1 处压入已匹配的 A 节点 `match_b[b_slot]`

**增广（找到自由的 B 节点）：**
1. 记录 `chosen_b[level] = free_b_slot`
2. 从当前 level 向下遍历至 0：
   - `match_a[a] = cur_b; match_b[cur_b] = a`
   - 前进：`cur_b = chosen_b[level-1]`
3. match_count 加一；跳出 DFS

**回溯（该 A 节点无可行路径）：**
- `st_top -= 1`（弹出 DFS 帧）
- 外层 while 循环重新读取已保存的 `ei`（`dfs_stk[lvl].1`），继续扫描

### 二分划分（第一步）

二分二着色使用与 `graph_bipartite`（V2.37）相同的 BFS 技术：
将边视为无向边；BFS 交替分配颜色 0（A 侧）和 1（B 侧）。
若任一节点与其 BFS 父节点颜色相同，则该图非二分图，
立即返回 `match_count = 0, is_bipartite = false`。

### visited_b 语义

`visited_b` 在每个新的自由 A 节点开始 DFS 时被重置。在单次 DFS 中，
每个 B 节点最多被尝试一次——这可防止交替路径搜索中出现环，
并确保 DFS 在 O(E) 步内终止。

---

## API

```rust
pub fn graph_bipartite_match<const N: usize>(
) -> ([VectorAddress; N], [VectorAddress; N], usize, bool, usize)
```

返回 `(left_vecs, right_vecs, match_count, is_bipartite, node_count)`：
- `left_vecs[0..match_count]`  —— 已匹配的 A 侧（颜色 0）节点
- `right_vecs[0..match_count]` —— 已匹配的 B 侧（颜色 1）节点；`left_vecs[i]` 与 `right_vecs[i]` 匹配
- `match_count`                —— 最大匹配规模；若非二分图则为 0
- `is_bipartite`               —— 若检测到奇数长度环则为 false
- `node_count`                 —— 存活节点总数

输出按节点槽位顺序排序（A 侧内按 VectorAddress.as_u64() 升序）。
N 应取 `MAX_NODES`（128）以实现完整覆盖。

---

## 关键不变量

| 不变量 | 说明 |
|---|---|
| `is_bipartite` 与 `graph_bipartite` 保持一致 | 二者使用相同的 BFS 二着色；测试 10 交叉验证 |
| `match_count ≤ min(\|A\|, \|B\|)` | 匹配是一组不相交的配对 |
| 输出中左侧节点两两不同 | 每个 A 节点最多出现一次 |
| 输出中右侧节点两两不同 | 每个 B 节点最多出现一次 |
| `!is_bipartite` 时 `match_count = 0` | 非二分图 → 完全跳过匹配 |
| 自环无影响 | BFS 着色跳过自环（否则会分配相同颜色） |
| 增广路径正确性 | `chosen_b[k]` 记录第 k 层选中的 B；增广遍历 0..lvl |
| 回溯恢复 | `dfs_stk[lvl].1` 保存 `ei`，使子节点回溯后能继续扫描 |

---

## Shell 命令

```
graph bipartite match   最大二分图匹配
gbimatch                别名
bipartite match         别名
```

显示：
- 标题：`graph bipartite match`
- 若非二分图：红色显示 "NOT bipartite (odd-length cycle detected)" 及提示
- 若为二分图但无边：绿色显示 "bipartite graph with no edges (empty matching)"
- 否则：以亮黄色显示 `A 侧 ↔ B 侧` 配对表格
- 页脚：匹配规模 + 节点数

---

## 测试套件：`gos-graph-bimatch-harness`（L4=68）

10 项测试覆盖：

| # | 场景 | 预期结果 |
|---|---|---|
| 1 | 空图 | match_count=0, is_bipartite=true |
| 2 | 单个孤立节点 | match_count=0, is_bipartite=true |
| 3 | 三角形（奇数环，非二分图） | is_bipartite=false, match_count=0 |
| 4 | 单条 A–B 边 | match_count=1，配对 (A0,B0) |
| 5 | 路径链 A0–B0–A1 | match_count=1（B0 被共享，只能匹配一个） |
| 6 | K_{2,2} 完全二分图 | match_count=2（完美匹配） |
| 7 | K_{2,3}：左 2 右 3 | match_count=2（受较小一侧约束） |
| 8 | 需要增广路径交换 | match_count=2（需要 DFS 压栈并增广） |
| 9 | 两个不连通的二分子图 | match_count=2（每个分量各 1 个） |
| 10 | K_{3,3}：不变量交叉验证 | match_count=3；is_bipartite 与 graph_bipartite 一致；所有输出配对节点不相交 |

10 项全部通过，零警告。

---

## VectorAddress 命名空间

L4=68 保留给 `gos-graph-bimatch-harness`。

---

## 参考文献

- Kuhn 1955 —— *"The Hungarian method for the assignment problem"*（增广路径思想的来源）
- Hopcroft & Karp 1973 —— O(E√V) 改进算法（此处因 V≤128 规模较小而采用 Kuhn 算法以求简洁）
- König 1931 —— König 定理：二分图中，最大匹配 = 最小顶点覆盖
- Hall 1935 —— Hall 婚姻定理（匹配存在性条件）
- Cormen, Leiserson, Rivest & Stein —— *Introduction to Algorithms* §26.3（基于最大流的二分图匹配）
