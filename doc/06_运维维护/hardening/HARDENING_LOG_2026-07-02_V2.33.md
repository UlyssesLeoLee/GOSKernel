# GOS 硬化日志 — V2.33 — 2026-07-02

## 摘要

V2.33 为活跃节点图新增了**拓扑排序**（`graph toposort`）——一种 Kahn's BFS
算法，产生的依赖排序中每个源节点（入度为 0）都排在其后继节点之前。类似于
POSIX 上的 `tsort(1)`、`cmake --build` 的依赖排序，或 `cargo build` 的
crate 图解析。与 V2.32 的环检测天然互补：先运行 `graph cycles` 验证图为
DAG，再运行 `graph toposort` 查看所有节点的启动/初始化顺序。

---

## 修改内容

### 1. `graph_toposort_inner<N>` — gos-runtime（`crates/gos-runtime/src/lib.rs`）

`Runtime` 上新增的私有方法：

```rust
pub fn graph_toposort_inner<const N: usize>(&self) -> ([VectorAddress; N], usize, bool)
```

**算法**：Kahn's BFS（入度队列）。

1. 将活跃节点槽位收集到一个紧凑的固定数组中。
2. 通过扫描边表计算每个槽位的入度；**自环被排除在外**，因此仅有自环的
   节点入度仍为 0，会被正常输出。
3. 用所有入度为 0 的节点（源节点）作为 BFS 队列的种子。
4. 出队 → 输出到结果 → 递减所有后继槽位的入度；若某后继的入度降为 0，
   则将其入队。
5. `is_dag = out_len == node_count`：当所有节点都被输出时，图是无环的；
   当存在有环节点始终无法被处理（入度始终无法降为 0）时，`is_dag` 为
   `false`。

特性：
- **O(V+E)** 时间复杂度，O(V) 工作状态。
- **no_std 安全**——仅使用固定栈数组，无堆分配。
- 返回 `(order, length, is_dag)` 三元组，使调用方在一次遍历中同时获得
  排序结果和 DAG 标志。

新增的公开包装函数：

```rust
pub fn graph_toposort<const N: usize>() -> ([VectorAddress; N], usize, bool)
```

获取全局 `RUNTIME` 锁并委托给 `graph_toposort_inner`。将 N 上限设为
128（= MAX_NODES）以覆盖全图。

### 2. `dispatch_graph_toposort` — k-shell（`crates/k-shell/src/lib.rs`）

新增的 shell 分发函数，将拓扑顺序渲染为编号列表：

- 标题横幅：黑底青色 `GRAPH TOPOSORT`。
- 若图为空：显示 `no nodes registered`。
- 若存在环：红色 WARNING，并提示运行 `graph cycles`。
- 节点列表：序号（从 1 开始）| vector 地址（青色，12 字符填充）| 节点
  key（绿色）| 插件名（暗淡色）。
- 页脚：已输出数 / 总数，以及 DAG 确认信息或有环分量计数。

### 3. Shell 路由 — k-shell（`crates/k-shell/src/proc.rs`）

分发分支中新增的命令别名：

| 输入 | 行为 |
|---|---|
| `graph toposort` | `dispatch_graph_toposort(sink)` |
| `toposort` | 别名 |
| `topo sort` | 别名（空格分隔变体） |
| `graph tsort` | 别名（对应 POSIX `tsort`） |
| `tsort` | 别名 |

help 文本已更新，新增：
```
  graph toposort     topological dependency ordering of all nodes (like tsort)
```

### 4. 测试套件 — `host-tests/gos-graph-toposort-harness/`（10 个测试，全部通过）

| # | 测试 | 验证内容 |
|---|------|----------|
| 1 | `empty_graph_toposort_is_empty_dag` | 空 runtime → 长度为 0，is_dag 为 true |
| 2 | `single_node_toposort` | 1 个节点，无边 → 被输出，is_dag 为 true |
| 3 | `linear_chain_toposort_order` | A→B→C → 排序为 A,B,C；pos_a < pos_b < pos_c |
| 4 | `two_node_cycle_is_not_dag` | A→B→A → is_dag 为 false |
| 5 | `diamond_dag_toposort_is_dag` | 菱形结构 → is_dag 为 true，全部 5 个节点被输出 |
| 6 | `diamond_dag_a_precedes_b_and_c` | 菱形结构中 A 排在 B 与 C 之前 |
| 7 | `diamond_dag_d_is_last` | D 排在 B 与 C 之后（共享汇聚点） |
| 8 | `self_loop_node_still_emitted` | A→A 自环：不计入入度，节点仍被输出 |
| 9 | `disconnected_chains_all_emitted` | 两条独立链全部被输出，is_dag 为 true |
|10 | `cyclic_graph_partial_sort` | A→B→C→A 环：仅两个孤立节点（D、E）被输出 |

---

## 验证

```
cd host-tests\gos-graph-toposort-harness
cargo test -- --test-threads=1
# test result: ok. 10 passed; 0 failed
```

---

## 生产质量考量

| 能力 | Linux/POSIX 对应物 | GOS V2.33 |
|---|---|---|
| 依赖排序 | `tsort(1)`（处理标准输入的节点对） | `graph toposort`（活跃图，O(V+E)） |
| 构建顺序解析 | `cmake --build` / `cargo build` | toposort 输出 = 启动/初始化顺序 |
| 环保护 | `tsort` 遇环时以非零退出码退出 | `is_dag` 标志 + WARNING 横幅 |
| 有环时的部分排序 | `tsort` 在第一个环处停止 | 输出所有无环节点；有环节点停滞 |
| Shell 接口 | `tsort < deps.txt` | `graph toposort`（无需文件） |
| 自环处理 | `tsort` 可能无限循环 | 自环被排除在入度计算之外 |

选用 Kahn's 算法而非基于 DFS 的拓扑排序，原因如下：
1. 它天然地将 `is_dag` 标志作为副产物产生（统计已输出数与总数）。
2. 其迭代式队列结构可以清晰地映射到固定大小数组（无需递归栈）。
3. 它无需额外的外层重启循环即可处理非连通分量。

---

## 图操作系统特性的保持

`graph toposort` 暴露了**活跃插件图的依赖排序**——这与包管理器用来确定
构建顺序、或初始化系统用来确定服务启动顺序的结构信息是相同的。在 GOS
中，这并非静态清单，而是针对 runtime 边表的实时查询，能反映自启动以来
（自 V2.13 起在 diff ring 中追踪）所做的任何拓扑变更。

---

## 与 V2.32（graph cycles）的联动

`graph toposort` 与 `graph cycles` 构成互补关系：

```
graph cycles    →  "is this a DAG?"        (DFS, 3-color, O(V+E))
graph toposort  →  "in what order?"        (Kahn's BFS, O(V+E))
```

建议的运维工作流：
```
> graph cycles      # verify acyclic
  no cycles detected  (directed acyclic graph)
> graph toposort    # get boot order
    1   1.0.0.1     boot.loader  (k-boot)
    2   1.0.0.2     mm.slab      (k-heap)
    ...
```

---

*自动化硬化流程 — GOS V2.33 — 2026-07-02*
