# 硬化日志 V2.86 — 图的桥（割边 / Tarjan 算法）

**日期：** 2026-07-04
**分支：** feat/vk-auto-live-surface
**提交：** 1b18cfb
**宿主测试总计：** 833（此前 823，+10）

---

## 功能：`graph bridges` / `gbridges` / `cut edges` / `gcute`

### 动机

V2.85 新增了关节点（割点）检测——识别移除后会使图断开的节点。V2.86 新增了
自然的**边对偶**原语：**桥（割边）检测**——识别移除后会使连通分量数增加的边。

割点与割边共同构成了生产级图分析平台（NetworkX、igraph、Boost.Graph）
所使用的基础性 **2-连通性** 工具集：

| 原语 | 移除对象 | 何时导致断开... |
|---|---|---|
| 关节点（V2.85） | 节点 | 它是某对节点之间的唯一路径 |
| 桥（V2.86） | 边 | 它是两个子图之间的唯一连接 |

在操作系统依赖图中，桥边代表两组内核子系统集群之间的**单一上行链路**——
类似于一条一旦故障就会分裂路由结构的网络链路。

操作系统类比：一个没有冗余路径的网卡或交换机上行链路——其移除会
悄然隔离某个子网（例如叶脊拓扑中一个叶交换机仅有一条到脊交换机的
上行链路的情形）。

---

## 算法：迭代式 Tarjan DFS —— 桥检测

桥检测使用与关节点（V2.85）相同的 disc/low-link 框架，但采用**更严格的条件**
和**按边索引跟踪父节点**：

**桥判据：** `low[child] > disc[parent]`（严格 `>`，非 `≥`）
- 关节点：`low[child] >= disc[parent]`（≥）——节点是单点故障
- 桥：`low[child] > disc[parent]`（>）——甚至没有任何回边能到达父节点本身

**按边索引而非父节点槽位跟踪父节点：**

与 V2.85 的关键差异在于父节点的跟踪方式。如果使用父节点槽位来跟踪，
两条反向平行的有向边 `A→B` 和 `B→A` 会导致 B 跳过所有到 A 的边
（将两者都当作父节点关系处理）。而使用边索引跟踪时，B 只跳过它
到达时所走的那条特定边；反向边 `B→A` 仍作为回边可见，正确地
设置 `low[B] = disc[A]`，从而避免了错误的桥判定。

| 方法 | 反向平行 A→B + B→A | 是否正确？ |
|---|---|---|
| 父节点槽位（V2.85 方式） | 完全跳过 B→A → `low[B]=disc[B]` → 错误地判定为桥 | ✗ |
| 父节点边索引（V2.86） | B→A 作为回边 → `low[B]=disc[A]` → 不判定为桥 | ✓ |

**无根节点特殊情形：** 与关节点（对于有 ≥ 2 个 DFS 子节点的节点需要
特殊的 DFS 根节点判定）不同，桥检测没有根节点特殊情形。条件
`low[child] > disc[parent]` 在每个非根节点上都统一适用。

**状态数组：**

| 数组 | 类型 | 含义 |
|---|---|---|
| `disc[slot]` | `u32` | DFS 发现时间；`u32::MAX` = 未访问 |
| `low[slot]` | `u32` | 通过回边可达的最小 disc 值 |
| `par_ei[slot]` | `usize` | 到达时所走的边索引；`MAX_EDGES` = 根节点 |
| `par_slot[slot]` | `usize` | 父节点槽位（仅用于桥的输出） |

**复杂度：** O(V + E)，无堆分配，no_std 安全。

---

## 实现

### crates/gos-runtime/src/lib.rs

**新增方法**（位于 `GraphRuntime` 内，即 `impl GraphRuntime` 中）：
```rust
pub fn graph_bridges_inner<const N: usize>(&self)
    -> ([VectorAddress; N], [VectorAddress; N], usize, usize)
// 返回 (from_vecs, to_vecs, bridge_count, node_count)
```

**新增公开函数：**
```rust
/// V2.86：在无向投影中查找所有桥边（割边）。
pub fn graph_bridges<const N: usize>()
    -> ([VectorAddress; N], [VectorAddress; N], usize, usize) {
    RUNTIME.lock().graph_bridges_inner()
}
```

**返回值：**
- `from_vecs[i]`、`to_vecs[i]`：规范化后的桥端点（`from` 中放 `as_u64()` 较小者）
- `bridge_count`：找到的桥数量（受 `N` 限制）
- `node_count`：存活节点总数

**关键不变量：**
- 图被视为无向：每条边的两个端点方向都会被跟随。
- 自环被跳过：`nbr_slot == cur_slot` 保护。
- 仅按索引跳过到达时所走的那条边（`par_ei[cur_slot]`），而非父节点槽位的所有边。
- 回边更新：对已访问的非父节点边执行 `low[cur] = min(low[cur], disc[nbr])`。
- 出栈时若 `low[child] > disc[parent]`（严格 >）则输出一条桥。
- 每条桥按 `as_u64()` 规范化顺序：`from = min(a,b)`，`to = max(a,b)`。
- 输出按 `(from.as_u64(), to.as_u64())` 通过插入排序升序排列。

### crates/k-shell/src/lib.rs

**新增函数** `dispatch_graph_bridges(sink: &ConsoleSink)`：
- 标题：` graph bridges (cut edges)`（图的桥（割边））（青色）
- 若 `node_count == 0`：打印 `(no nodes registered)`（未注册任何节点）。
- 若 `bridge_count == 0`：以绿色打印 `no bridges (graph is 2-edge-connected or acyclic-free)`
  （无桥，图是 2-边连通的或无环）。
- 否则：以红色列出每座桥，格式为 `bridge  <from>  ──  <to>`。
- 脚注：`N bridge(s)  of  M node(s)  link resilience: 2-edge-connected / moderate risk / high risk`
  （共 M 个节点中有 N 座桥，链路韧性：2-边连通 / 中度风险 / 高风险）
  - `2-edge-connected`（2-边连通）：bridge_count == 0（绿色）
  - `moderate risk`（中度风险）：bridge_count ≤ node_count / 4（黄色）
  - `high risk`（高风险）：bridge_count > node_count / 4（红色）

### crates/k-shell/src/proc.rs

**新增路由**（插入在 `graph articulation` / `gcutv` 分发之后）：
```
graph bridges   →  dispatch_graph_bridges
gbridges        →  别名
cut edges       →  别名
gcute           →  别名
```

---

## 测试装置：`host-tests/gos-graph-bridges-harness`

**VectorAddress L4=62** 标识本装置的命名空间。

| 测试 | 图拓扑 | 期望结果 |
|---|---|---|
| 1 | 空图 | bridge_count=0, node_count=0 |
| 2 | 单个孤立节点 A | bridge_count=0, node_count=1 |
| 3 | A→B（单条有向边） | bridge_count=1, bridge=(A,B) |
| 4 | A→B→C→A（三角形） | bridge_count=0（2-边连通） |
| 5 | A→B→C（路径，2 条边） | bridge_count=2, bridges=[(A,B),(B,C)] |
| 6 | A→B + B→A（反向平行） | bridge_count=0（反向边是回边） |
| 7 | 星形 H→{A,B,C,D}（4 条辐条） | bridge_count=4（所有辐条都是桥） |
| 8 | 方形 A→B→C→D→A（4 环） | bridge_count=0（2-边连通） |
| 9 | 两个三角形通过桥 C→F 相连 | bridge_count=1, bridge=(C,F) |
| 10 | 链 A→B→C→D（3 条边） | bridge_count=3, bridges=[(A,B),(B,C),(C,D)] |

**结果：** 10/10 通过。

---

## VectorAddress L4 命名空间更新

| L4 | 装置 |
|---|---|
| 60 | gos-graph-link-predict-harness (V2.84) |
| 61 | gos-graph-articulation-harness (V2.85) |
| **62** | **gos-graph-bridges-harness (V2.86)** |

---

## 关键图论事实

桥（割边）{u, v} 满足：
> 从无向图中移除 {u, v} 会增加连通分量的数量。

等价地（对于 DFS 树边 u→v 的 Tarjan 判据）：
> `low[v] > disc[u]` —— v 的子树中没有任何顶点存在回边到 u 或 u 的任何祖先。

**与其他 V2.x 指标的关系：**
- 桥与关节点（V2.85）互补：每座桥的两个端点都是关节点（当该桥是唯一连接时），
  但反之不成立。
- 一棵树恰好有 `n-1` 座桥（每条边都是桥）；2-边连通图有 0 座桥。
- 双连通分量分解（V2.87 的自然候选方向）将图划分为由桥分隔的
  最大 2-边连通子图。
- 桥与 `graph_global_efficiency`（V2.74）之间有清晰的映射关系：每座桥都是一个
  瓶颈，移除后会使平均成对距离变长。

**2-连通性 与 2-边连通性 的区别：**
- **2-连通**（无割点）：每对节点之间存在 ≥ 2 条内部顶点不相交的路径。
- **2-边连通**（无桥）：每对节点之间存在 ≥ 2 条边不相交的路径。
- 2-连通蕴含 2-边连通；反之不成立。

---

## 文献参考

- R. Tarjan，《深度优先搜索与线性图算法》，SIAM J. Comput. 1(2)，1972。
  原始的 disc/low-link 框架；V2.86 应用了严格的 `>` 桥判据，并采用
  边索引父节点跟踪以在多重边/反向平行边对上保证正确性。
- D. Eppstein，《在图中查找桥》，https://ics.uci.edu/~eppstein/（讲义）。
  阐明了为保证多重边安全而采用边索引而非节点索引跟踪父节点的区别。
