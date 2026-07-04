# GOS 硬化日志 — V2.46（2026-07-02）

## 版本号: V2.46
## 功能: `graph spanning` — BFS 生成森林

---

## 变更摘要

新增 `graph spanning` —— 在 GOS 内核图的无向投影上执行 BFS 生成森林。每个活跃节点被分配到一棵树，根节点按 slot 升序选取。输出展示每棵树的父子结构及 BFS 深度，帮助运维人员可视化连接各内核子系统的**最小骨架**。

Shell 别名：`graph spanning` / `spanning` / `span` / `graph span` / `graph tree` / `gtree`

OS 类比：`ip route show` / STP（生成树协议）——连接所有内核节点、不含冗余交叉链路的最小无环骨架。

---

## 动机

社区发现套件完成（V2.45）后，下一个自然的原语是**生成树/生成森林**：一种揭示哪些父子关系构成活跃图最小无环连接器的结构骨架视图。

生成树分析在图论 OS 中回答：
- 以连接度最高的子系统为根的最短路径树是什么？
- 哪些节点是叶子（无子节点），哪些是分支（内部连接点）？
- 存在多少个不连通的分量（树的数量 = 连通分量数量）？
- 内核连通图中的最大深度（最长树臂）是多少？

这也完成了经典图原语集合的闭环：连通性分析（环、拓扑排序、SCC、缩点、可达性、二部性）→ 度量分析（度数、中心度、紧密度、离心率、Katz、PageRank、HITS）→ 聚类（社区）→ 结构骨架（生成树）。

---

## 算法：BFS 生成森林（无向投影）

```text
初始化：visited[v] = false，对所有 v

按 slot 升序遍历每个节点 root：
  若 visited[root]：跳过
  tree_count++
  visited[root] = true
  parent[root] = root   // 根节点的父节点是自身
  depth[root] = 0
  BFS 队列 = [root]

  队列非空时：
    cur = 出队
    将 cur 加入输出（BFS 顺序）
    对 cur 的每个邻居 nb（入边和出边均视为无向）：
      若 nb == cur：跳过（无自环）
      若 visited[nb]：跳过
      visited[nb] = true
      parent[nb] = cur
      depth[nb] = depth[cur] + 1
      入队 nb

输出：(vecs, parents, depths, node_count, tree_count)
```

**关键设计选择：**

1. **无向处理**：入边和出边均视为无向邻居连接（与 `graph community`、`graph bipartite` 一致），确保无论信号方向如何都能覆盖所有活跃节点。
2. **根选择：slot 升序**——每个连通分量中 slot 最小的节点成为根，输出确定且可复现。
3. **BFS（非 DFS）**——BFS 使深度最小化（从根出发的最短路径树），给运维人员最均衡的图结构视图：所有与根"跳数"相同的节点位于同一深度层级。
4. **根节点的父节点为自身**——根节点的父向量等于自身向量（`parents[i] == vecs[i]`），无需额外标志数组即可判断根节点。
5. **按 BFS 访问顺序输出**——节点按 BFS 访问顺序输出：根（深度0），随后深度1的所有子节点，再是深度2的所有孙节点等；各树依次输出。

**复杂度**：O(V + E) 每次调用——对所有活跃节点和边做单趟 BFS。
**空间**：O(MAX_NODES) = O(128)——固定大小栈数组，兼容 no_std/no_alloc。

---

## 实现细节

### `crates/gos-runtime/src/lib.rs`

- **`RuntimeState::graph_spanning_inner<const N>()`** —— 核心 BFS 生成森林算法，按 slot 顺序遍历、每 slot 一个 BFS 队列，追踪 `visited`/`parent_slot`/`depth_arr`
- **`pub fn graph_spanning<const N>()`** —— 公开包装函数：锁定 RUNTIME，调用 `topology_snapshot()`，委托给 `graph_spanning_inner`

### `crates/k-shell/src/lib.rs`

- **`pub fn dispatch_graph_spanning(sink)`** —— 展示函数：
  - 标题：青色 `graph spanning`
  - 每树一块：`[T0]  root: [vec]  ──  N nodes`
  - 列标题：`depth  vector  parent  role`
  - 逐节点显示深度、向量（按角色着色）、父节点（根节点显示 `(root)`）、角色标签
  - 页脚：`N node(s)  BFS spanning-forest  trees: M`
  - 颜色：洋红 (13) = root，青色 (11) = branch（有子节点），白色 (7) = leaf

- **角色分类**（调用后通过扫描子节点计算）：
  - `root` —— 深度 0
  - `branch` —— 深度 ≥ 1 且至少有一个其他节点以其为父节点
  - `leaf` —— 深度 ≥ 1 且在生成树中无子节点

### `crates/k-shell/src/proc.rs`

- 路由：`"graph spanning" | "spanning" | "span" | "graph span" | "graph tree" | "gtree"` → `dispatch_graph_spanning`
- 帮助文本新增两行，说明命令及别名

---

## 测试用例（10/10 通过）：`host-tests/gos-graph-spanning-harness`

| 编号 | 用例 | 验证点 |
|------|------|--------|
| 1 | 空图 | total=0, tree_count=0 |
| 2 | 单节点 | 1 节点，1 树，depth=0，parent=self |
| 3 | 两个孤立节点（无边） | 2 树，均 depth 0，均 parent=self |
| 4 | 单边 A→B | 1 树：A 为根(depth 0)，B 为子(depth 1)，B.parent=A |
| 5 | 链 A→B→C→D | 1 树，depths 0/1/2/3；父链 B←A, C←B, D←C |
| 6 | 有向三角环 A→B→C→A | 1 树，全部 depth ≤ 2（环 = 1 个无向连通分量） |
| 7 | 两对不连通节点（A─B, C─D） | 2 树，恰好 2 个根 |
| 8 | 根 parent == self | 对所有 depth[i]==0 的 i：parents[i]==vecs[i] |
| 9 | 非根 parent 为已知节点 | 对所有非根 i：parents[i] 出现在 vecs[0..total] 中 |
| 10 | tree_count 匹配连通分量数 | 3 个孤立 → 3 树；2+1 → 2 树 |

**结果：10/10 通过，零告警**

---

## 节点角色语义

| 角色 | 条件 | 颜色 |
|------|------|------|
| `root` | depth == 0 | 洋红 (13) |
| `branch` | depth ≥ 1 且生成树中至少有一个子节点 | 青色 (11) |
| `leaf` | depth ≥ 1 且生成树中无子节点 | 白色 (7) |

---

## Shell 命令一览

```text
graph spanning     所有活跃节点的 BFS 生成森林（最小骨架）
spanning           别名
span               别名
graph span         别名
graph tree         别名
gtree              别名
```

示例输出（两个子系统）：

```text
 graph spanning
 ───────────────────────────────────────────────────────────
  [T0]  root: [1:0:1:0]  ──  4 nodes
    depth  vector           parent           role
    0      [1:0:1:0]        (root)           root
    1      [1:0:2:0]        [1:0:1:0]        branch
    1      [1:0:3:0]        [1:0:1:0]        leaf
    2      [1:0:4:0]        [1:0:2:0]        leaf

  [T1]  root: [2:0:1:0]  ──  2 nodes
    depth  vector           parent           role
    0      [2:0:1:0]        (root)           root
    1      [2:0:2:0]        [2:0:1:0]        leaf
 ───────────────────────────────────────────────────────────
  6 node(s)  BFS spanning-forest  trees: 2
```

---

## 不变量确认

- [x] 纯读操作：`graph_spanning` 不推进 epoch，不做任何变更
- [x] 无堆分配 / no_std：所有缓冲区为固定大小栈数组
- [x] harness 使用标准的 `TEST_LOCK + reset()` 隔离方式
- [x] 版本顺序：V2.46 紧随 V2.45（community）
- [x] 文档归档路径：`doc/06_运维维护/hardening/HARDENING_LOG_2026-07-02_V2.46.md`

---

## 后续建议（V2.47 候选）

- `node checkpoint <vec>` —— 快照节点状态到 diff ring（观测性）
- `journal ring <N>` —— 运行时可配置的 JournalRing 容量
- `graph sim <N>` —— 模拟 N 步随机游走，输出信号流量轨迹
- `graph mst` —— 最小生成树（Prim's/Kruskal's，使用边权重）
- PAL_U32 → attribute node 重构（Demo A 前置条件）

---

*由自动强化任务生成 · 2026-07-02*
