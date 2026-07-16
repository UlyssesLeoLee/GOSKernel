# GOS 硬化日志 — V3.00（2026-07-06）

## 功能：最小生成树形图（Chu-Liu / Edmonds 1967）

### 变更摘要

在 gos-runtime 中新增 `graph_arborescence<N>(root: VectorAddress)`——使用 Chu-Liu / Edmonds 算法从根节点计算最小生成树形图（有向 MST）。

以 r 为根的树形图是一棵有向生成树，其中每个非根节点 v 都恰好存在一条从 r 到 v 的有向路径。*最小*树形图使其边的总权重最小。

**关键定理（Edmonds 1967）**：
当且仅当每个非根节点都可从根节点到达时，最小生成树形图必然存在。Chu-Liu/Edmonds 算法通过环收缩以 O(V·E) 求解。

### 操作系统类比

最小权重有向启动依赖树：给定一个带权服务依赖图（边权 = 启动延迟毫秒数），从 `init` 出发的树形图给出了每个服务的最优单一父依赖分配——即最小总启动开销。等价于在存在多个有效前驱时，为每个内核模块选择应等待的前驱。

补充了有向图分析套件：
- 有向最短路径（V2.xx）：从根节点到各节点的最小延迟
- 支配树（V2.90）：强制性前驱（图论意义上的必经节点）
- MST（无向）：无向拓扑的最小生成树
- MSA（V3.00）：有向拓扑的最小生成树形图

### 算法：Chu-Liu / Edmonds 环收缩（O(V·E)）

**初始化：**
- 将活跃节点压缩为 `slot_to_ci[]` 数组（共 nc 个紧凑索引）。
- 通过匹配 `node_slot_by_vec(root)` 确定 `root_ci`。
- 构建 `e_from[]`、`e_to[]`、`e_wt[]`、`e_adj[]` 边数组（均为浮点权重；
  `e_adj[ei]` 记录先前环收缩带来的权重调整）。
- `group[ci]` 将每个紧凑索引映射到其当前超级节点 ID（初始为 ci）。
- `num_sg = nc`——超级节点的初始数量。

**迭代轮次（`for _round in 0..nc`）：**

*步骤 A——为每个非根超级节点选择最小入边：*
```
对每个超级节点 sg != root_sg：
    找到边 ei，其中 e_from[ei] 属于不同超级节点，且 e_adj[ei] 最小
    in_src[sg] = e_from[ei] 所属的超级节点
    in_wt[sg]  = e_adj[ei]
    in_ei[sg]  = ei
    sel_parent[ci] = e_from[ei_mapped_back]  ← 为 sg 中的根成员更新
    sel_wt[ci]     = e_adj[in_ei[sg]] × 1000（毫权重 u32）
```
若任一超级节点没有入边，则树形图不可能存在（`is_connected=false`）。

*步骤 B——通过 DFS 检测环（颜色：0=白，1=灰，2=黑）：*
```
对每个非根超级节点 sg（从灰色开始，沿 in_src[] 前进）：
    灰色 → 重新访问一个灰色节点时即发现 cycle_sg
```
若未发现环 → 树形图已完整；跳出循环。

*步骤 C——追溯该环：*
```
从 cycle_sg 沿 in_src[] 回溯直到再次回到 cycle_sg → 收集 cycle_nodes[]
```

*步骤 D——为环成员分配新的超级节点 `new_sg = num_sg++`：*
```
对每个 group[ci] 属于该环的 ci：
    group[ci] = new_sg
```

*步骤 E——调整从环外进入环内的边的权重：*
```
对每条 e_to 在环内、e_from 在环外的边 ei：
    t_sg = e_to[ei] 原来所属的超级节点
    e_adj[ei] -= in_wt[t_sg]
```
这体现了"替换掉当前环内被选中边"所节省的成本，从而使"从该节点进入环以打破环"的净权重正确。

**收敛性：** 每一轮要么终止（无环），要么收缩一个环，
将 `num_sg` 至少减 1。最多 `nc` 轮 → 保证终止。

**输出：** `sel_parent[]` 和 `sel_wt[]` 在各轮中累积构成树形图结构；
总权重 = Σ sel_wt[i] / 1000（由毫权重还原）。

### 为什么 Chu-Liu/Edmonds 优于朴素贪心

在有向图中，独立地为每个节点选择权重最小的入边可能会选出构成环的边——违反树的性质。例如：

```
A(root) → B (w=5)
A(root) → C (w=3)
B → C (w=1)
C → B (w=1)
```

朴素做法：选择 B←C(1) 和 C←B(1)——形成环！只能退回到 B←A(5) + C←A(3) = 总计 8。
Edmonds 算法：收缩 {B,C} 环，将 A→B 的有效权重调整为 5−1=4，
A→C 的有效权重调整为 3−1=2；选择 C 作为环的入口点（最小值=2）；展开后 →
C←A(3)，B←C(1)；总计 = **4**（相比朴素方法的 8，节省 50%）。

测试用例 06 在测试集中明确验证了这一最优性差距。

### 实现细节

**crates/gos-runtime/src/lib.rs**
- 新方法：`GraphRuntime::graph_arborescence_inner<const N: usize>(root: VectorAddress)`
  - 固定大小的栈数组：`node_slots[N]`、`e_from/e_to/e_wt/e_adj[MAX_EDGES]`
    （MAX_EDGES=512）、`group[MAX_SG]`、`in_src/in_wt/in_ei[MAX_SG]`、
    `sel_parent[N]`、`sel_wt[N]`、DFS 状态数组——零堆分配。
  - MAX_SG=256（≥ 2×MAX_NODES=128）；每次环收缩分配一个新超级节点。
  - `node_slot_by_vec(root)` 用于定位根节点；对空根（空图情形）回退至 `node_slots[0]`。
  - 返回 `([VectorAddress; N], [VectorAddress; N], [u32; N], usize, u32, bool)`：
    `(vecs, parents, weights_milli, nc, total_milli, is_connected)`。
    - `vecs[0..nc]`——活跃节点；根节点始终位于索引 0。
    - `parents[0..nc]`——树形图父节点（根节点的父节点为自身）。
    - `weights_milli[0..nc]`——入边权重 × 1000（根节点为 0）。
    - `nc`——活跃节点数。
    - `total_milli`——树形图总权重 × 1000（不连通时为 0）。
    - `is_connected`——若任一非根超级节点没有入边则为 false。
- 新公开函数：
  `graph_arborescence<const N: usize>(root: VectorAddress)`
  ——薄封装，调用 `RUNTIME.lock().graph_arborescence_inner(root)`。

**crates/k-shell/src/lib.rs**
- 新增 `dispatch_graph_arborescence(sink, root: VectorAddress)`：
  - 亮青色表头（颜色 11）：`"=== Minimum Spanning Arborescence (Chu-Liu/Edmonds) ==="`
  - 处理：空图（nc=0）、不连通（无树形图）、正常显示三种情形。
  - 逐节点表格：角色（根/子节点）、权重、VectorAddress、父节点 VectorAddress。
  - 页脚：`"N node(s)  MSA-weight=W.WWW  (Chu-Liu/Edmonds)"`

**crates/k-shell/src/proc.rs**
- 在 `"graph mpc"` 分支之后新增路由：
  - 命令：`"graph arborescence <vec>"`、`"garborescence <vec>"`、
    `"arborescence <vec>"`、`"gmsa <vec>"`、`"min arborescence <vec>"`
  - 从命令后缀解析尾随的 `VectorAddress`。
  - 解析失败时的错误消息：`"arborescence: invalid VectorAddress '<str>'"`

**host-tests/gos-graph-arborescence-harness/**（VectorAddress 命名空间 L4=76）
- VectorAddress L4=76
- 10 个测试，全部通过（0 警告）：
  1. 空图 → nc=0，total_w=0，is_connected=true（空真情形）。
  2. 单节点 → nc=1，parent=自身，total_w=0。
  3. 单条有向边 A→B，root=A → B 的父节点=A，权重=1000，total=1000。
  4. 链 A→B→C→D，root=A → B←A，C←B，D←C，total=3000；已验证所有父节点链接。
  5. 有向 3-环 A→B→C→A，root=A → A 可达所有节点；total=2000（返回边未被使用）。
  6. 环收缩：A→B(5)，A→C(3)，B→C(1)，C→B(1)；Edmonds=4，朴素贪心=8。
  7. 不连通：D 没有入边 → is_connected=false，total_w=0。
  8. 完全有向三角形 K3（全部 6 条边），root=A → 2 条最优非根边，total=2000。
  9. 星形出树 A→{B,C,D,E} → 平凡树形图，4 条单位边，total=4000。
  10. 菱形+尾部 A→B(1)，A→C(3)，B→D(1)，C→D(2)，D→E(1)：D 选择 B 而非 C；total=6000；
      恰好 nc-1=4 条非根边；根节点计数=1。

### 关键不变量

- 每个连通树形图中恰有 `nc − 1` 条非根边（自父计数为 1）。
- 任一非根超级节点无入边时，`total_milli = 0` 且 `is_connected = false`。
- 空图：`nc = 0`，`is_connected = true`（空真意义上生成），`total_milli = 0`。
- 每次环收缩恰好创建一个新超级节点（`num_sg += 1`），且不会超过 `MAX_SG = 256`。
- `group[]` 单调重新赋值；展开环时仅为断点节点更新 `sel_parent[]` / `sel_wt[]`。
- 栈深度受 `nc ≤ MAX_NODES = 128` 限制；`num_sg ≤ MAX_SG = 256`。

### 宿主测试总计

| 里程碑 | 测试数 |
|---|---|
| V2.93 | 903 |
| V2.94–V2.99（+6 × 10） | 963 |
| **V3.00（+10）** | **973** |

### 文献

- Chu, Y. J.; Liu, T. H. (1965). On the Shortest Arborescence of a Directed Graph.
  *Science Sinica* 14: 1396–1400.
- Edmonds, J. (1967). Optimum branchings. *Journal of Research of the National Bureau
  of Standards B* 71(4): 233–240.
- Tarjan, R. E. (1977). Finding optimum branchings. *Networks* 7(1): 25–35.
  （使用斐波那契堆的改进 O(E log V) 实现；GOSKernel 使用 O(V·E)。）
