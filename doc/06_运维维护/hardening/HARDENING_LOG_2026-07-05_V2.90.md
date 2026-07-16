# 硬化日志 V2.90 —— 图支配树（Cooper et al. 2001）

**日期：** 2026-07-05
**分支：** feat/vk-auto-live-surface
**宿主测试总计：** 873（此前 863，+10 新增）

---

## 功能：`graph domtree <start>` / `gdomtree <start>` / `dominator <start>` / `gdom <start>`

### 动机

V2.88–V2.89 增加了针对 DAG 的结构分析（关键路径、拓扑分层）。
V2.90 增加了**支配树（dominator tree）**分析，它适用于**一般有向图**
（包括含环图），回答一个更深层的结构性问题：

> **"对于从给定入口可达的每个节点，哪个节点是其必经的前驱——不存在替代路径？"**

支配树是编译器理论和程序分析中的基础数据结构：

| 问题 | 操作系统 / 编译器类比 |
|---|---|
| 哪个节点支配 N？ | 该组件启动前，哪个内核子系统必须已在运行，且没有绕行路径？ |
| 最近支配节点是谁？ | 哪个最近的祖先是唯一的必经关卡？ |
| A 是否是通往 B 的唯一入口路径？ | A 与 B 之间的网络分区是否可被利用（A 支配 B）？ |

支配树出现在以下场景：
- **编译器 CFG 分析**（SSA 构造、循环检测、代码移动）
- **安全分析**（控制流完整性、用于后向切片的后支配树）
- **系统启动分析**（每个服务的必要前置子系统是谁）
- **网络可靠性**（哪个节点故障会必然导致下游节点不可达）

与运行时中已有的相关算法对比：
| 算法 | 发现的内容 |
|---|---|
| `graph_articulation`（V2.85） | 移除后会增加连通分量数的节点（无向图） |
| `graph_bridges`（V2.86） | 移除后会增加连通分量数的边 |
| `graph_domtree`（V2.90） | 对每个节点，找到从指定入口出发的最近必经祖先（有向图） |

---

## 算法：Cooper–Harvey–Kennedy 2001 简单迭代支配算法

参考文献：*"A Simple, Fast Dominance Algorithm"*，Cooper, Harvey & Kennedy，2001。

### 为什么选用这个算法

经典的 Lengauer–Tarjan 算法（1979）复杂度为 O(V·α(V))，但需要复杂的数据结构
（半支配节点计算、用于路径压缩的 link-cut 树），在不使用动态分配的 `no_std`
环境中难以实现。

Cooper 等人 2001 年的方法使用同样基于 RPO 的迭代，但采用简单的数组实现的
格（lattice）汇合操作。最坏情况复杂度为 O(V² · E)，但在 DAG 和典型控制流图上
通常 1–2 轮即可收敛——在 MAX_NODES=128 的规模下完全在预算范围内。

### 第一步 —— 迭代 DFS → RPO 顺序

从 `start` 出发，使用显式栈运行迭代 DFS（无递归，`no_std` 安全）。
在节点结束访问时记录后序。逆后序（RPO）就是该迭代算法所使用的遍历顺序。

关键点：`rpo_num[slot]` 存储每个节点的 RPO 位置（0 = start，它支配所有节点）。
不可达节点保持 `rpo_num[slot] = UNDEF`。

### 第二步 —— 初始化 idom

```
idom[start_slot] = start_slot  // start 支配自身（格的顶点）
idom[all others] = UNDEF        // 未知
```

### 第三步 —— 迭代收敛

按 RPO 顺序处理所有可达节点（跳过位置 0 的 start）。
对每个节点 `b`，计算：

```
new_idom = intersect over all predecessors p of b where idom[p] != UNDEF
```

`intersect(a, c)` 函数通过在当前部分支配树中同时向上（朝 `start` 方向，
RPO 编号递减）遍历 `a` 和 `c`，直到二者相遇，从而求得二者的最近公共祖先（LCA）：

```
while a != c:
    while rpo[a] > rpo[c]:  a = idom[a]
    while rpo[c] > rpo[a]:  c = idom[c]
return a  // == c
```

由于向 `start`（rpo=0）攀升时 RPO 编号严格递减，此过程必然终止。

重复直到没有任何 `idom[b]` 再发生变化。

对于 DAG：恰好一轮即收敛。
对于含回边的图：通常需要 2–3 轮。

### 自环

自环边（`from == to`）在前驱扫描中被跳过（`if p == b { continue }`）。
自环不会带来新的支配路径。

### 不可达节点

未被从 `start` 出发的 DFS 访问到的节点，永远不会出现在 `rpo_slots` 中，
因此永远不会被处理，其 `idom` 保持 `UNDEF`。它们会被排除在输出之外。

---

## API

```rust
pub fn graph_domtree<const N: usize>(
    start: VectorAddress,
) -> ([VectorAddress; N], [VectorAddress; N], usize, usize)
```

返回 `(vecs, idoms, node_count, reachable_count)`：
- `vecs[0..reachable_count]`  —— 按 RPO 顺序排列的可达节点
- `idoms[0..reachable_count]` —— 每个节点对应的最近支配节点向量
- 对于 `start`：`idoms[i] == vecs[i]`（start 支配自身）
- `node_count`      —— 图中存活节点总数
- `reachable_count` —— 从 `start` 可达的节点数（包含 start 本身）

---

## 关键不变量

| 不变量 | 说明 |
|---|---|
| `idom[start] == start` | start 是根节点，支配自身 |
| `idom[b]` 的 RPO 严格小于 `b` | 保证 LCA 遍历必然终止 |
| 不可达节点被排除在输出之外 | 对不可达槽位有 `rpo_num[s] == UNDEF` |
| 前驱扫描中跳过自环 | 自环不会增加支配路径 |
| 菱形结构 `A→{B,C}→D`：`idom[D] == A` | 支配树中 B 和 C 的 LCA 是 A |
| 迭代收敛可处理回边 | 不仅限于 DAG——适用于一般有向图 |
| LCA 遍历中的守卫 `guard < MAX_NODES * 2` | 防止病态循环（防御性设计） |

---

## Shell 命令

```
graph domtree <v>   从入口节点 <v> 出发的支配树
gdomtree <v>        别名
dominator <v>       别名
gdom <v>            别名
```

显示：`节点` | `最近支配节点` 的表格；根节点以黄色显示，并带 `← root` 标记。

---

## 测试套件：`gos-graph-domtree-harness`（L4=66）

10 项测试覆盖：
1. 空图 → 0 个可达节点
2. 单节点，start=自身 → idom = 自身
3. start 不在图中 → 0 个可达节点
4. 单条边 A→B → idom[B]=A
5. 线性链 A→B→C → idom[B]=A, idom[C]=B
6. 菱形 A→{B,C}→D → idom[D]=A（B 和 C 的 LCA）
7. 不可达节点被排除在输出之外
8. 回边环 A→B→C→B → idom[B]=A, idom[C]=B
9. 孤立 start —— 只有自身可达
10. 汇合后延伸 A→{B,C}→D→E → idom[D]=A, idom[E]=D

10 项全部通过，零警告。

---

## VectorAddress 命名空间

L4=66 保留给 `gos-graph-domtree-harness`。

---

## 参考文献

- Cooper, Harvey & Kennedy 2001 —— *"A Simple, Fast Dominance Algorithm"*
- Lengauer & Tarjan 1979 —— 原始的 O(V·α(V)) 算法（此处未采用；对 no_std 而言过于复杂）
- Aho, Lam, Sethi & Ullman 2006 —— *Compilers: Principles, Techniques, and Tools* §9.6
- Cytron et al. 1991 —— SSA 构造以支配树为基础
