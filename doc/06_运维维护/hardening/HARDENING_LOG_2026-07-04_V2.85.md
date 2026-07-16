# 硬化日志 V2.85 — 图的关节点（割点 / Tarjan 算法）

**日期：** 2026-07-04
**分支：** feat/vk-auto-live-surface
**提交：** fff103c
**宿主测试总计：** 823（此前 813，+10）

---

## 功能：`graph articulation` / `garticulate` / `cut vertices` / `gcutv`

### 动机

生产级图分析平台（NetworkX、igraph、Gephi）都将**关节点（割点）检测**
作为网络韧性的基本原语提供。关节点是这样一个节点：将其移除会使连通分量数
增加——它是网络的单点故障（single point of failure）。

GOSKernel 此前已具备丰富的拓扑分析能力（SCC、密度、聚类、社区检测、
链接预测），但缺乏任何机制来识别那些一旦缺失就会使内核依赖图分裂的
结构关键节点。

V2.85 新增了 Tarjan 迭代式 disc/low-link DFS 算法，用于检测有向图的
无向投影中所有割点。这直接映射到一个操作系统场景：识别哪些内核子系统
一旦被移除或发生故障，会导致其他子系统在依赖图中变得不可达。

操作系统类比：`systemctl list-dependencies --reverse` 识别没有冗余依赖路径的
单点故障内核服务。

---

## 算法：用于关节点检测的 Tarjan 迭代式 DFS

经典递归版 Tarjan 算法使用 O(V) 个栈帧，这在没有充足栈保证的 no_std
内核环境中是不安全的。V2.85 使用完全迭代的版本，采用显式的
`(slot, edge_scan_index)` 对组成的 DFS 栈——与 SCC Kosaraju 实现（V2.34）
使用的模式相同。

**状态数组（均以槽位索引，栈上分配）：**

| 数组 | 类型 | 含义 |
|---|---|---|
| `disc[slot]` | `u32` | DFS 发现时间；`u32::MAX` = 未访问 |
| `low[slot]` | `u32` | 通过回边可到达的最小 disc 值 |
| `par[slot]` | `usize` | DFS 父节点槽位；`MAX_NODES` = 根节点 / 无父节点 |
| `dfs_children[slot]` | `u8` | 从该槽位压入的 DFS 树子节点数量 |
| `is_ap[slot]` | `bool` | 若该槽位是关节点则为 true |

**迭代协议：**

1. 对每个未访问节点，将 `(start_slot, 0)` 压入 DFS 栈。
2. 在每个帧 `(cur_slot, ei)` 处：
   - 从 `ei` 开始向后扫描边（无向投影：同时跟随 `from_node==cur_id` 与 `to_node==cur_id`）。
   - **树边**（邻居未访问）：设置 disc/low，设置父节点，`dfs_children[cur]` 加一，
     压入子帧，跳出循环。
   - **回边**（邻居已访问且非父节点）：`low[cur] = min(low[cur], disc[nbr])`。
3. **出栈**（无更多邻居）：
   - 传播：`low[par] = min(low[par], low[cur])`。
   - **非根关节点判定**：若 `low[cur] >= disc[par]` 且 `par[par] != NO_PAR` → 标记 `par` 为关节点。
4. **根节点关节点判定**：每棵 DFS 树遍历完成后，若 `dfs_children[root] >= 2` → 标记根节点为关节点。

**输出排序：** 返回前按 `as_u64()` 升序对关节点的 VectorAddress 做插入排序，
与 `graph_peripheral` / `graph_center` 的约定保持一致。

**复杂度：** O(V + E)，无堆分配，no_std 安全。

---

## 实现

### crates/gos-runtime/src/lib.rs

**新增方法**（位于 `GraphRuntime` 内，即 `impl GraphRuntime` 中）：
```rust
pub fn graph_articulation_inner<const N: usize>(&self)
    -> ([VectorAddress; N], usize, usize)
// 返回 (art_vecs, art_count, node_count)
```

**新增公开函数：**
```rust
/// V2.85：实时内核图的关节点（割点）。
pub fn graph_articulation<const N: usize>() -> ([VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_articulation_inner()
}
```

**关键不变量：**
- 图被视为**无向**：同时跟随 `from_node==cur_id` 与 `to_node==cur_id`；
  邻居为对端节点。
- **自环**被跳过：在所有处理之前先检查 `nbr_slot == cur_slot`。
- 回边更新的**父节点保护**：`nbr_slot != par[cur_slot]` 防止在无向投影中
  把父节点的树边误判为回边。
- **根节点关节点规则**使用 `dfs_children[root] >= 2`，而非 low-link
  （low-link 规则仅适用于非根节点）。
- **非根节点关节点规则**：`low[child] >= disc[parent]`（≥而非>）——相等
  意味着子节点的子树无法到达父节点的任何祖先节点。
- 结果按 `as_u64()` 升序排序 → 保证跨测试运行的确定性顺序。
- `art_count` 受 `N`（缓冲区容量）限制；典型用法为 `N=128 = MAX_NODES`。

### crates/k-shell/src/lib.rs

**新增函数** `dispatch_graph_articulation(sink: &ConsoleSink)`：
- 标题：` graph articulation points`（图关节点）
- 若 `node_count == 0`：打印 `(no nodes registered)`（未注册任何节点）。
- 若 `art_count == 0`：以绿色打印 `no single points of failure (fully biconnected)`
  （无单点故障，完全双连通）。
- 否则：以红色列出每个割点，格式为 `cut vertex  <VectorAddress>`。
- 脚注：`N cut vertices  of  M node(s)  resilience: fully biconnected / moderate risk / high risk`
  （共 M 个节点中有 N 个割点，韧性等级：完全双连通 / 中度风险 / 高风险）
  - `fully biconnected`（完全双连通）：art_count == 0（绿色）
  - `moderate risk`（中度风险）：art_count ≤ node_count / 4（黄色）
  - `high risk`（高风险）：art_count > node_count / 4（红色）

### crates/k-shell/src/proc.rs

**新增路由**（插入在 `graph compare` / `gcompare` 分发之后）：
```
graph articulation   →  dispatch_graph_articulation
garticulate          →  别名
cut vertices         →  别名
gcutv                →  别名
```

---

## 测试装置：`host-tests/gos-graph-articulation-harness`

**VectorAddress L4=61** 标识本装置的命名空间。

| 测试 | 图拓扑 | 期望结果 |
|---|---|---|
| 1 | 空图 | art_count=0, node_count=0 |
| 2 | 单个孤立节点 A | art_count=0, node_count=1 |
| 3 | A→B（单边） | art_count=0（移除任一节点后剩下单节点连通分量） |
| 4 | A→B→C（路径） | art_count=1, cut=B |
| 5 | A→B→C→A（三角形） | art_count=0（双连通） |
| 6 | 星形 E→{A,B,C,D} | art_count=1, cut=E（中心节点） |
| 7 | 共享 C 的蝴蝶形 A-B-C-D-E | art_count=1, cut=C（共享的顶点） |
| 8 | 方形 A→B→C→D→A（4 环） | art_count=0（双连通） |
| 9 | 链 A-B-C-D（4 节点路径） | art_count=2, cuts=[B, C]（排序后） |
| 10 | 两个三角形通过桥 C→F 相连 | art_count=2, cuts=[C, F]（桥的两端点） |

**结果：** 10/10 通过。

---

## VectorAddress L4 命名空间更新

| L4 | 装置 |
|---|---|
| 60 | gos-graph-link-predict-harness (V2.84) |
| **61** | **gos-graph-articulation-harness (V2.85)** |

---

## 关键图论事实

关节点（割点）v 满足：
> 存在 s, t ≠ v，使得从 s 到 t 的每条无向路径都经过 v。

等价地（Tarjan 判据）：
- v 是 **DFS 根节点**且有 ≥ 2 个 DFS 树子节点，或者
- v 是**非根节点**且存在子节点 w 满足 `low[w] ≥ disc[v]`
  （w 的子树无法"回溯"越过 v 到达 v 的祖先节点）。

**与其他 V2.x 指标的关系：**
- 关节点与 `graph_scc`（V2.34）互补：SCC 识别强连通子图；
  关节点在较弱的无向意义下识别结构脆弱点。
- 关节点不同于 `graph_attractor`（V2.54）（底部 SCC）——一个节点
  可以同时是吸引子和关节点。
- 桥边（移除后会使图断开的边）可以通过树边 u→v 满足 `low[v] > disc[u]`
  （严格不等式）来检测；这是 V2.85 的自然后续方向。

---

## 文献参考

- R. Tarjan，《深度优先搜索与线性图算法》，SIAM J. Comput. 1(2)，1972。
  原始的递归算法；V2.85 使用相同的 disc/low-link 判据，但改用迭代栈
  以避免在 no_std 内核环境中递归。
- J. Hopcroft & R. Tarjan，《算法 447：图操作的高效算法》，
  CACM 16(6)，1973。给出了实用的双连通分量表述形式。
