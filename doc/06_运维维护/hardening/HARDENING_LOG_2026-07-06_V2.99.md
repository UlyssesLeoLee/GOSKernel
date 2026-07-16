# GOS 硬化日志 — V2.99（2026-07-06）

## 功能：DAG 中的最小路径覆盖（König / Dilworth）

### 变更摘要

在 gos-runtime 中新增 `graph_min_path_cover<N>()`——有向无环图（DAG）的最小路径覆盖（MPC）。路径覆盖是一组顶点不相交的有向路径，它们共同经过每个节点；最小覆盖的路径数为 MPC = n − ν。

**关键定理（König 1931 / Dilworth 1950）**：
```
MPC(G) = n − ν(B(G))
```
其中 B(G) 是二分图展开（G 中每条有向边 u→v 对应 left_u → right_v），ν 为 B(G) 的最大匹配数。

这是针对 DAG 的精确算法，多项式时间 O(V·E)，不存在近似误差。

### 操作系统类比

最小顺序升级链：在一个依赖 DAG 中，跨所有模块应用内核补丁所需的最少有序安装序列数，其中每个序列都必须遵循有向依赖边。等价于 `make -j<MPC>` 的作业分配，每个作业对应一条线性依赖链。

补充了现有的 DAG 分析套件：
- DAG 最长路径/关键链（V2.88）：串行深度下界
- DAG 拓扑分层（V2.89）：并行执行层级分配（宽度）
- DAG 支配树（V2.90）：强制性启动顺序前驱
- DAG 反馈弧集（V2.91）：造成循环依赖的边
- MPC（V2.99）：最少线性升级/部署序列数（König 深度）

### 实现细节

**crates/gos-runtime/src/lib.rs**
- 新方法：`GraphRuntime::graph_min_path_cover_inner<N>()`
  - 阶段 1：压缩活跃节点；构建 `slot_to_ci[]` 映射。
  - 阶段 2：Kahn BFS——验证是否为 DAG；在 `topo_order[]` 中记录拓扑序。
    自环会使 `in_deg > 0` 恒定，导致 Kahn 算法永远无法排空它 → `is_dag = false`。
  - 阶段 3：以 u128 位掩码按紧凑索引构建二分图展开的邻接关系：
    每条有向边 u→v 对应 `right_adj[u_ci] |= 1u128 << v_ci`。
  - 阶段 4：Kuhn 增广路径匹配算法（迭代式 DFS，O(V·E)）：
    - 每个起点的 DFS 状态：`dfs_lci[level]`（左侧节点）、`dfs_rem[level]`（u128
      表示的剩余候选）、`chosen_r[level]`（每层已选的右侧节点）。
    - 全局已访问集合 `visited_r`（u128）防止一次调用内 DFS 重复访问。
    - 找到空闲的右侧节点 → 沿 `chosen_r[]` 轨迹自底向上增广匹配。
    - 遇到已匹配的右侧节点 → 将其当前左侧伙伴压栈，继续 DFS。
    - 左侧节点按拓扑序处理，以获得自然的路径结构。
  - 阶段 5：`path_count = nc − match_count`（König / Dilworth 等式）。
  - 阶段 6：沿 `match_l[]` 的后继链重建路径。
    路径起点 = 满足 `match_r[ci] == NIL`（没有匹配前驱）的节点。
    按拓扑序枚举起点 → 路径 ID 自上而下分配。
- 新公开函数：
  `graph_min_path_cover<N>() -> ([VectorAddress; N], [u8; N], usize, bool, usize)`
  - 返回 `(path_vecs, path_ids, path_count, is_dag, node_count)`
  - `path_vecs[0..node_count]`——所有活跃节点，按"先分路径再按拓扑序"排列。
  - `path_ids[0..node_count]`——每个节点的 0 起始路径 ID（同 ID 表示同一条路径）。
  - `path_count`——最少顶点不相交路径数（n − ν）。
  - `is_dag`——若检测到有向环则为 false（此时 MPC 无定义）。
  - `node_count`——活跃节点总数。

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_min_path_cover()`：
  - 亮黄色表头（颜色 14）；若非 DAG 则以亮红色（12）显示错误。
  - 每条路径循环使用 6 种亮色：绿(10)、青(11)、
    洋红(13)、蓝(9)、黄(14)、红(12)。
  - 连续路径之间用虚线分隔符（┈）以提高可读性。
  - 页脚：`N node(s)  MPC=K  (n−ν=N−M)  König/Dilworth`

**crates/k-shell/src/proc.rs**
- 在"graph domset"之后新增路由：
  `"graph mpc" || "gmpc" || "min path cover" || "graph min path cover" || "path cover" || "gdagcover" || "graph path cover"`

**host-tests/gos-graph-mpc-harness/**
- VectorAddress L4=75
- 10 个测试，全部通过（0 警告）：
  1. 空图 → MPC=0，is_dag=true，nc=0。
  2. 单节点 → MPC=1（单点路径），is_dag=true。
  3. 单条有向边 A→B → MPC=1（路径 [A,B]），已验证 vecs 顺序。
  4. 两个孤立节点 → MPC=2（两条单点路径），已验证不同路径 ID。
  5. 链 A→B→C→D → MPC=1（哈密顿路径）；已验证 vecs 顺序 [A,B,C,D]。
  6. 菱形 A→{B,C}→D → MPC=2（D_R 被竞争；ν=2，n=4）。
  7. 并行链 A→B、C→D → MPC=2（两条独立链；ν=2，n=4）。
  8. K_3 DAG（A→B, A→C, B→C）→ MPC=1（哈密顿路径 A→B→C）；已验证 vecs 顺序。
  9. 有向环 A→B→C→A → is_dag=false，MPC=0（无定义）。
  10. 星形 DAG A→{B,C,D,E} → MPC=4；Dilworth 交叉验证：MPC+ν=n（4+1=5）✓；
      A 所在路径长度=2（A + 一个匹配的叶子）；已验证 3 条单点叶子路径。

### 关键不变量

- 当且仅当 Kahn BFS 排空的节点数少于 `nc` 时（存在环或自环），`is_dag = false`。
- `path_count + match_count == node_count` 恒成立（König/Dilworth 等式）。
- `match_l[]` 后继链无环，因为 B(G) 的边遵循 G 的 DAG 边方向。
- `visited_r` 在一次 DFS 调用内单调（回溯时不会取消已访问标记）——
  这是 Kuhn 算法防止 DFS 无限循环的标准做法。
- 栈深度受 `nc ≤ MAX_NODES = 128` 限制；无堆分配。
- u128 位掩码邻接：`right_adj[u_ci] |= 1u128 << v_ci`——在 nc ≤ 128 时安全。

### 测试交叉引用

- 测试 5（链）验证了哈密顿情形：ν = n−1，MPC = 1。
- 测试 10（星形）验证了反链情形：ν = 1（一对中心→叶子），MPC = n−1。
- 测试 9（环）验证了非 DAG 情形下的提前返回：`processed < nc` ↔ 存在环。
- 测试 6（菱形）验证了被竞争的右侧节点：D_R 有两个左侧竞争者，
  匹配算法恰好选中一个，因此 path_count = n − 2 = 2。

### 文献

- König, D. (1931). Graphen und Matrizen. Matematikai Lapok 38: 116–119.
- Dilworth, R. P. (1950). A decomposition theorem for partially ordered sets.
  Annals of Mathematics. 51(1): 161–166.
- Kuhn, H. W. (1955). The Hungarian method for the assignment problem.
  Naval Research Logistics Quarterly. 2(1–2): 83–97.
- Kahn, A. B. (1962). Topological sorting of large networks. CACM 5(11): 558–562.
