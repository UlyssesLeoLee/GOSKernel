# 硬化日志 V2.88 — DAG 最长路径 / 关键路径分析

**日期：** 2026-07-04
**分支：** feat/vk-auto-live-surface
**宿主测试总计：** 853（此前 843，+10）

---

## 功能：`graph dag longest` / `gdaglongest` / `critical path` / `graph critical` / `gcritical`

### 动机

V2.85–V2.87 新增了结构分析原语（关节点、桥、欧拉回路）。V2.88 新增了一个
**调度与规划**原语：**DAG 最长路径 / 关键路径分析**——回答任何并行调度
在依赖图中必须遍历的最小串行深度是多少。

这是并行构建系统（`make -j`）、启动排序器（`systemd-analyze critical-chain`）
以及 PERT/CPM 项目调度背后的基本问题：

| 问题 | 操作系统类比 |
|---|---|
| 关键路径长度是多少？ | `systemd-analyze critical-chain`：并行启动的最小挂钟时间深度 |
| 该图是否为 DAG？ | 依赖图是否不含循环依赖（类似 `cargo` 的死锁检查）？ |
| 关键路径从哪里开始/结束？ | 哪个叶子服务和哪个根服务界定了这一不可避免的深度？ |

在生产级图平台（NetworkX、igraph）中，DAG 最长路径是任务调度、
编译排序和数据流分析中使用的核心规划原语。

---

## 算法：Kahn BFS 拓扑排序 + 距离动态规划（O(V+E)）

关键路径算法在一次遍历中结合了 **DAG 检测**与**最长路径动态规划**：

### 第一步 —— 入度统计

一次扫描边表，为每个存活节点计算 `in_deg[v]`。

**自环处理：** 自环（`from == to`）被计入入度统计。这可以防止 Kahn BFS
将自环节点耗尽——它会一直卡在 `in_deg ≥ 1`，永远不会被输出，
从而导致 `processed < node_count → is_dag = false`。这是正确的：
自环本身就是一个长度为 1 的有向环。

### 第二步 —— 带距离动态规划的 Kahn BFS

用所有 `in_deg == 0` 的节点（初始源点，`dist = 0`）作为 BFS 队列的种子。

对每个被输出的节点 `u`：
- 对每条边 `u → v`（松弛时跳过自环）：
  - `dist[v] = max(dist[v], dist[u] + 1)` —— 动态规划松弛
  - 若 `dist[u] + 1 > dist[v]` 则 `pred[v] = u` —— 前驱节点跟踪
  - 将 `in_deg[v]` 减一；若 `in_deg[v] == 0`，则将 `v` 入队

### 第三步 —— DAG 检查

若 `processed_count < node_count`，说明至少有一个节点未能被耗尽——
存在环。返回 `(0, false, zero, zero, node_count)`。

### 第四步 —— 关键路径提取

找到 `dist` 值最大的节点 `end_slot`（同分时以槽位索引最小者作为
决胜规则，以保证确定性）。沿 `pred[]` 回溯，直到
`pred[cur] ≥ MAX_NODES`，从而找到 `start_slot`（关键路径的源点）。

**平凡情形：** 若 `max_dist == 0`（无边，或全部为孤立节点），
返回 `(0, true, zero, zero, node_count)` —— 该图是一个没有路径的平凡 DAG。

---

## 返回值签名

```rust
pub fn graph_dag_longest() -> (u32, bool, VectorAddress, VectorAddress, usize)
//                             ^^^^  ^^^^^^ ^^^^^^^^^^^^ ^^^^^^^^^^^^ ^^^^^^^
//                        path_hops is_dag  start_vec    end_vec      node_count
```

| 字段 | 类型 | 含义 |
|---|---|---|
| `path_hops` | `u32` | 最长有向路径的跳数；若无边或图存在环则为 0 |
| `is_dag` | `bool` | 若不存在有向环（含自环）则为 true |
| `start_vec` | `VectorAddress` | 关键路径的源点；若无路径则为零值 |
| `end_vec` | `VectorAddress` | 关键路径的终点；若无路径则为零值 |
| `node_count` | `usize` | 存活节点总数 |

---

## Shell 显示效果

```
 graph dag longest
 ───────────────────────────────────────────────────────────
  ✓ DAG  critical path: 3 hops
  start  64.1.1.0   end  64.1.4.0
  (minimum serial depth any parallel schedule must traverse)
 ───────────────────────────────────────────────────────────
  is_dag: yes   nodes: 4
```
（意为：DAG，关键路径 3 跳；起点/终点；任何并行调度必须遍历的最小串行深度；is_dag: 是，节点数: 4）

对于存在环的图：
```
  ✗ graph has directed cycles (not a DAG)
  critical path is undefined for cyclic graphs
  use `graph cycles` or `graph scc` to inspect cycles
```
（意为：图含有向环（不是 DAG）；对含环图关键路径未定义；请使用 `graph cycles` 或 `graph scc` 检查环）

对于空图/孤立节点图：
```
  — no directed edges (trivial DAG)
  all nodes are isolated; critical path length = 0
```
（意为：无有向边（平凡 DAG）；所有节点均为孤立节点；关键路径长度 = 0）

---

## 测试覆盖（gos-graph-dag-longest-harness，L4=64）

| # | 场景 | 期望结果 |
|---|---|---|
| 1 | 空图 | is_dag=true, path_hops=0, nc=0 |
| 2 | 单个孤立节点（无边） | is_dag=true, path_hops=0, nc=1 |
| 3 | 单个自环 A→A | is_dag=false, path_hops=0 |
| 4 | 线性链 A→B→C→D | is_dag=true, path_hops=3, start=A, end=D |
| 5 | 菱形 A→B, A→C, B→D, C→D | is_dag=true, path_hops=2, start=A, end=D |
| 6 | 两条独立链 (A→B) 和 (C→D→E) | is_dag=true, path_hops=2, end=E |
| 7 | 有向 3 环 A→B→C→A | is_dag=false, path_hops=0 |
| 8 | 带捷径的 DAG A→B→C + A→C | is_dag=true, path_hops=2, start=A, end=C |
| 9 | 星形扇出 A→{B,C,D,E} | is_dag=true, path_hops=1, start=A |
| 10 | 5 跳链 A→B→C→D→E→F | is_dag=true, path_hops=5, start=A, end=F |

全部 10 个测试通过。

---

## 关键不变量

- 自环被计入入度计算 → 正确检测为环（is_dag=false）
- 自环在 BFS 松弛步骤中被跳过（不会产生虚假的距离更新）
- `pred[v] ≥ MAX_NODES` 哨兵值表示"v 无前驱"（是一个源节点）
- 最大距离决胜规则：槽位索引最小者 → 确定性的 end_vec 选择
- 平凡 DAG（无边）：path_hops=0，is_dag=true，start/end=零值
- 有环图：path_hops=0，is_dag=false，start/end=零值

---

## VectorAddress L4 命名空间

- **L4=64**：`gos-graph-dag-longest-harness`

---

## 文献

- Kahn, A. B. (1962)。《大型网络的拓扑排序》。*CACM* 5(11):558–562。
- CPM / 关键路径法：Kelley & Walker (1959)，杜邦工程公司。
- `systemd-analyze critical-chain` —— Linux 并行启动关键路径检查工具。

---

## 操作系统类比映射

| 图属性 | 操作系统对应物 |
|---|---|
| `is_dag=true` | 依赖图无环（类似 `cargo check`、`tsort`） |
| `is_dag=false` | 检测到循环依赖（类似 `cargo` 的"检测到循环"错误） |
| `path_hops` | 最小并行启动深度（`systemd-analyze critical-chain` 长度） |
| `start_vec` | 根启动服务（内核驱动 / hwclock） |
| `end_vec` | 终端服务（登录管理器 / 显示服务器） |
