# 硬化日志 V2.89 — DAG 拓扑层级 / 并行执行层级分配

**日期：** 2026-07-04
**分支：** feat/vk-auto-live-surface
**宿主测试总计：** 863（此前 853，+10）

---

## 功能：`graph dag layers` / `gdaglayers` / `glayers` / `dag layers`

### 动机

V2.88 回答了"DAG 中最长的串行路径是什么？"（关键路径深度）。
V2.89 回答了互补的问题：**"每个节点最早可能的执行层级是什么？"**——
即用于并行调度的拓扑层级分配。

| 问题 | 操作系统类比 |
|---|---|
| 该节点属于哪一层？ | systemd 单元排序层级——该服务属于哪个启动阶段？ |
| 哪些节点可以并行运行？ | 同一层级中的服务之间没有排序约束 |
| 存在多少个不同的层级？ | 系统完全启动前的顺序启动阶段总数 |
| 该图是否为 DAG？ | 循环依赖检查（类似 `cargo` 的循环检测） |

拓扑层级出现在：
- **构建系统**（`make -j`、`ninja`、`bazel`）：计算构建层级以最大化并行度
- **初始化系统**（`systemd`）：为每个单元分配一个依赖层级
- **流水线编译器**：将算子分配到流水线阶段
- **工作流引擎**（`Airflow`、`Prefect`）：跨并行工作节点调度 DAG 任务

在生产级图平台（NetworkX 的 `dag_longest_path_length`、igraph 的
`topological_sorting`）中，层级分配是并行调度的标准原语。

---

## 算法：带层级传播的多源 Kahn BFS（O(V+E)）

### 与 V2.88（DAG 最长路径）的区别

| V2.88 `graph_dag_longest` | V2.89 `graph_dag_layers` |
|---|---|
| 返回单一的 `(path_hops, start_vec, end_vec)` | 为每个节点 v 返回 `layer[v]` |
| 回答"关键链有多深？" | 回答"每个节点属于哪一层？" |
| 适用于截止时间/延迟分析 | 适用于并行工作调度 |

两者底层都使用 Kahn BFS，但传播的内容不同：
- V2.88：传播 `dist[v] = max(dist[v], dist[u] + 1)`，并跟踪前驱节点
- V2.89：为所有节点传播 `layer[v] = max(layer[v], layer[u] + 1)`，不进行路径回溯

### 第一步 —— 入度统计

一次扫描所有边。自环（`from == to`）被计入入度统计，这样 Kahn BFS
就永远无法耗尽自环节点（它会永远停留在 `in_deg >= 1`），
导致 `processed < node_count` → `is_dag = false`。与 V2.88 采用相同的不变量。

### 第二步 —— 从所有源点播种的 Kahn BFS

将 `layer[v] = u32::MAX` 初始化（未访问）。用所有 `in_deg == 0` 的节点
作为 BFS 队列的种子；赋予它们 `layer = 0`。

对每个被出队的节点 `u`：
- 对每条有向边 `u -> v`（松弛时跳过自环）：
  - `in_deg[v] -= 1`
  - `layer[v] = max(layer[v], layer[u] + 1)` —— 传播最深前驱的层级
  - 若 `in_deg[v] == 0`：更新 `max_layer`，将 `v` 入队

### 第三步 —— 环检查

若 `processed < node_count`，说明存在环。返回 `(_, _, node_count, 0, false)`。

### 第四步 —— 排序输出

按 `(layer[v], v.as_u64())` 对节点数组升序排序，以保证确定性输出。
最多将 `N` 个条目打包进输出数组。

`layer_count = max_layer + 1`（层级 0 到 max_layer，含两端）。

---

## 返回值签名

```rust
pub fn graph_dag_layers<const N: usize>()
    -> ([VectorAddress; N], [u32; N], usize, u32, bool)
//     ^^^^^^^^^^^^^^^^     ^^^^^^^^  ^^^^^  ^^^  ^^^^^
//     vecs                 layers    nc     lc   is_dag
```

| 字段 | 类型 | 含义 |
|---|---|---|
| `vecs[0..nc]` | `[VectorAddress; N]` | 存活节点，按层级再按 VectorAddress 排序 |
| `layers[0..nc]` | `[u32; N]` | 每个节点的层级编号（0 = 源点，1 = 一跳，……） |
| `node_count` | `usize` | 存活节点总数 |
| `layer_count` | `u32` | 不同层级的数量（= max_layer + 1）；若有环则为 0 |
| `is_dag` | `bool` | 若图含有向环则为 false（此时层级未定义） |

---

## Shell 显示效果

对于菱形 DAG（A->{B,C}->D）：
```
 graph dag layers
 -----------------------------------------------------------
  layer  vector
  -----  ------------
      0  65.1.1.0

      1  65.1.2.0
      1  65.1.3.0

      2  65.1.4.0
 -----------------------------------------------------------
  nodes: 4   layers: 3
```

对于存在环的图：
```
  x graph has directed cycles (not a DAG)
  topological layers are undefined for cyclic graphs
  use `graph scc` or `graph dag longest` to inspect structure
```
（意为：图含有向环（不是 DAG）；对含环图拓扑层级未定义；请使用 `graph scc` 或 `graph dag longest` 检查结构）

对于空图：
```
  (no nodes registered)
```
（意为：未注册任何节点）

---

## 测试覆盖（gos-graph-dag-layers-harness，L4=65）

| # | 场景 | 期望结果 |
|---|---|---|
| 1 | 空图 | is_dag=true, nc=0, layer_count=0 |
| 2 | 单个孤立节点 | layer=0, layer_count=1 |
| 3 | 自环 A->A | is_dag=false |
| 4 | 单边 A->B | layer[A]=0, layer[B]=1, layer_count=2 |
| 5 | 线性链 A->B->C->D | layers=[0,1,2,3], layer_count=4 |
| 6 | 菱形 A->{B,C}->D | A=0, B=C=1, D=2, layer_count=3 |
| 7 | 有向 3 环 A->B->C->A | is_dag=false |
| 8 | 带捷径的 DAG A->B->C + A->C | C 得到层级 2（最深前驱胜出，而非捷径给出的 1） |
| 9 | 星形扇出 A->{B,C,D} | A=0, B=C=D=1, layer_count=2 |
| 10 | 两条独立链 A->B 和 C->D->E->F | layer_count=4（由较长链决定）；两条链均被正确分层 |

全部 10 个测试通过。

---

## 关键不变量

- 自环被计入入度 → 通过 `processed < node_count` 检测为环
- 自环在 BFS 松弛过程中被跳过（不会在自环上产生虚假的 `layer += 1`）
- `layer[v]` 初始化为 `u32::MAX`（未访问哨兵值）；源点设为 `0`
- 层级传播：`layer[v] = max(layer[v], layer[u] + 1)` —— 最深前驱胜出
- `layer_count = max_layer + 1`（层级 0 是第一层，layer_count 是不含上界的上限）
- 输出按 `(layer, VectorAddress.as_u64())` 排序，以保证稳定、确定性的顺序
- Shell 输出中不同层级之间插入空行分隔以提高可读性
- 当 `is_dag=false` 时：返回 `(empty, empty, node_count, 0, false)` —— 不返回部分层级数据

---

## VectorAddress L4 命名空间

- **L4=64**：`gos-graph-dag-longest-harness`（V2.88）
- **L4=65**：`gos-graph-dag-layers-harness`（V2.89，新增）

---

## 文献

- Kahn, A. B. (1962)。《大型网络的拓扑排序》。*CACM* 5(11):558-562。
- Coffman, E. G. & Graham, R. L. (1972)。《双处理器系统的最优调度》。
  *Acta Informatica* 1:200-213。
- 列表调度 / 关键路径调度：操作系统调度理论中标准的 DAG 并行化概念。
- `systemd --analyze` 单元排序层级；`make -jN` 并行构建层级。

---

## 操作系统类比映射

| 图属性 | 操作系统对应物 |
|---|---|
| `layer_count` | 初始化依赖图中顺序启动阶段的数量 |
| `layer[v] == 0` | 根服务（内核驱动、早期 udev、hwclock）—— 立即启动 |
| `layer[v] == k` | 属于第 k 个启动阶段的服务（必须等待前 k-1 个阶段全部完成） |
| 两个层级相同的节点 | 可以并行启动的服务（彼此之间没有依赖关系） |
| `is_dag=false` | 检测到循环依赖 —— 初始化系统会发生死锁（类似 `systemd` 的循环依赖警告） |
